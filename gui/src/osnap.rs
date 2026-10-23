//! Object snap: the pointer jumps to the corners, midpoints, centres and crossings of the drawing
//! (and the corners, edge midpoints and centre of the stock) when it comes within a few pixels of one.
use putiacnc_core::model::LoopData;
use putiacnc_core::shapes::{self, S};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind { Corner, Midpoint, Centre, Crossing, Stock }

impl Kind {
    pub fn label(self) -> &'static str {
        match self { Kind::Corner => "Corner", Kind::Midpoint => "Midpoint", Kind::Centre => "Centre", Kind::Crossing => "Crossing", Kind::Stock => "Stock" }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Hit { pub p: [f64; 2], pub kind: Kind }

fn pts(l: &LoopData) -> Vec<[f64; 2]> { l.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect() }

/// The snap points of one shape. Round shapes (circles, ellipses, rounded rectangles) are sampled as
/// many short segments, so only their centre and the middle of each side count, not every sample.
fn points_of(l: &LoopData, tol: f64, out: &mut Vec<Hit>) {
    let Some(b) = shapes::bbox(l) else { return };
    let c = [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
    if l.pt.is_some() { out.push(Hit { p: c, kind: Kind::Centre }); return; }
    let round = l.el.is_some() || l.cr.is_some() || l.rc.map_or(false, |r| r > 1e-9);
    if round {
        out.push(Hit { p: c, kind: Kind::Centre });
        for p in [[c[0], b[1]], [c[0], b[3]], [b[0], c[1]], [b[2], c[1]]] { out.push(Hit { p, kind: Kind::Midpoint }); }
        if l.el.is_none() { for p in [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]] { out.push(Hit { p, kind: Kind::Corner }); } }
        return;
    }
    let v = pts(l);
    let n = v.len();
    for (i, p) in v.iter().enumerate() {
        // a vertex on a smooth curve (a sampled circle, say) is not a corner: it must turn the path by 25 degrees or more
        let prev = if i > 0 { Some(v[i - 1]) } else if l.closed() && n > 2 { Some(v[n - 1]) } else { None };
        let next = if i + 1 < n { Some(v[i + 1]) } else if l.closed() && n > 2 { Some(v[0]) } else { None };
        let sharp = match (prev, next) {
            (Some(a), Some(c)) => {
                let (u, w) = ([p[0] - a[0], p[1] - a[1]], [c[0] - p[0], c[1] - p[1]]);
                let turn = (u[0] * w[1] - u[1] * w[0]).atan2(u[0] * w[0] + u[1] * w[1]).abs();
                turn >= 25.0_f64.to_radians()
            }
            _ => true, // the ends of an open path
        };
        if sharp { out.push(Hit { p: *p, kind: Kind::Corner }); }
        // the middle of a segment only when it is long enough to tell apart from its ends
        if let Some(q) = next {
            if (q[0] - p[0]).hypot(q[1] - p[1]) >= 3.0 * tol { out.push(Hit { p: [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0], kind: Kind::Midpoint }); }
        }
    }
    if l.closed() && n > 2 { out.push(Hit { p: c, kind: Kind::Centre }); }
}

fn seg_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<[f64; 2]> {
    let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-12 { return None; }
    let (qp0, qp1) = (c[0] - a[0], c[1] - a[1]);
    let t = (qp0 * s[1] - qp1 * s[0]) / den;
    let u = (qp0 * r[1] - qp1 * r[0]) / den;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) { Some([a[0] + t * r[0], a[1] + t * r[1]]) } else { None }
}

/// The snap point nearest to `p` within `tol` mm, ignoring the shapes listed in `skip`
/// (the ones being moved or edited). `stock` is the stock's width and height.
pub fn find(loops: &[LoopData], skip: &[usize], stock: (f64, f64), p: [f64; 2], tol: f64) -> Option<Hit> {
    let near = |q: [f64; 2]| (q[0] - p[0]).hypot(q[1] - p[1]);
    let mut cand: Vec<Hit> = Vec::new();
    let (w, h) = stock;
    for q in [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]] { cand.push(Hit { p: q, kind: Kind::Stock }); }
    for q in [[w / 2.0, 0.0], [w, h / 2.0], [w / 2.0, h], [0.0, h / 2.0], [w / 2.0, h / 2.0]] { cand.push(Hit { p: q, kind: Kind::Stock }); }
    // segments close enough to the pointer, for crossings
    let mut segs: Vec<([f64; 2], [f64; 2])> = Vec::new();
    for (i, l) in loops.iter().enumerate() {
        if skip.contains(&i) { continue; }
        let Some(b) = shapes::bbox(l) else { continue };
        if p[0] < b[0] - tol || p[0] > b[2] + tol || p[1] < b[1] - tol || p[1] > b[3] + tol {
            // the pointer is outside this shape's box: its corners and centre can still be near, a crossing cannot
            let mut c = Vec::new();
            points_of(l, tol, &mut c);
            cand.extend(c.into_iter().filter(|h| near(h.p) <= tol));
            continue;
        }
        points_of(l, tol, &mut cand);
        if l.pt.is_some() { continue; }
        let v = pts(l);
        let n = v.len();
        let m = if l.closed() { n } else { n.saturating_sub(1) };
        for k in 0..m {
            let (a, c) = (v[k], v[(k + 1) % n]);
            if a[0].min(c[0]) - tol <= p[0] && a[0].max(c[0]) + tol >= p[0] && a[1].min(c[1]) - tol <= p[1] && a[1].max(c[1]) + tol >= p[1] && segs.len() < 400 {
                segs.push((a, c));
            }
        }
    }
    for i in 0..segs.len() {
        for j in i + 1..segs.len() {
            if let Some(x) = seg_cross(segs[i].0, segs[i].1, segs[j].0, segs[j].1) {
                // shapes that merely share an end point are already caught as corners
                let at_end = [segs[i].0, segs[i].1, segs[j].0, segs[j].1].iter().any(|e| (e[0] - x[0]).hypot(e[1] - x[1]) < 1e-6);
                if !at_end && near(x) <= tol { cand.push(Hit { p: x, kind: Kind::Crossing }); }
            }
        }
    }
    // nearest wins; on a tie (within a hair) corners beat crossings beat centres beat midpoints
    let rank = |k: Kind| match k { Kind::Corner => 0, Kind::Stock => 1, Kind::Crossing => 2, Kind::Centre => 3, Kind::Midpoint => 4 };
    cand.into_iter().filter(|h| near(h.p) <= tol)
        .min_by(|a, b| {
            let (da, db) = (near(a.p), near(b.p));
            if (da - db).abs() < tol * 0.05 { rank(a.kind).cmp(&rank(b.kind)) } else { da.partial_cmp(&db).unwrap() }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use putiacnc_core::shapes::{new_ellipse, new_poly, new_rect};

    const ST: (f64, f64) = (200.0, 150.0);

    #[test]
    fn corners_midpoints_and_centres() {
        let r = new_rect([10.0, 10.0], [50.0, 30.0], 0.0, 0);
        let l = vec![r];
        let h = find(&l, &[], ST, [10.3, 10.2], 1.0).unwrap();
        assert_eq!((h.kind, h.p), (Kind::Corner, [10.0, 10.0]));
        let h = find(&l, &[], ST, [30.2, 9.8], 1.0).unwrap();
        assert_eq!((h.kind, h.p), (Kind::Midpoint, [30.0, 10.0]));
        let h = find(&l, &[], ST, [30.4, 20.3], 1.0).unwrap();
        assert_eq!((h.kind, h.p), (Kind::Centre, [30.0, 20.0]));
        assert!(find(&l, &[], ST, [30.0, 25.0], 1.0).is_none(), "nothing near");
    }

    #[test]
    fn circles_snap_to_centre_and_quadrants_only() {
        let c = new_ellipse([0.0, 0.0], [40.0, 40.0], 0);
        let l = vec![c];
        let h = find(&l, &[], ST, [20.2, 19.8], 1.0).unwrap();
        assert_eq!((h.kind, h.p), (Kind::Centre, [20.0, 20.0]));
        let h = find(&l, &[], ST, [20.2, 0.1], 1.0).unwrap();
        assert_eq!(h.kind, Kind::Midpoint);
        // a random spot on the rim has no snap point
        let a = 0.7_f64;
        assert!(find(&l, &[], ST, [20.0 + 20.0 * a.cos(), 20.0 + 20.0 * a.sin()], 1.0).is_none());
    }

    #[test]
    fn smooth_vertices_of_a_sampled_curve_are_not_corners() {
        // a 72-sided "circle" that is not flagged as round (an imported one, say)
        let p: Vec<[f64; 2]> = (0..72).map(|k| { let t = k as f64 * std::f64::consts::TAU / 72.0; [50.0 + 20.0 * t.cos(), 50.0 + 20.0 * t.sin()] }).collect();
        let l = vec![new_poly(&p, true, 0).unwrap()];
        assert!(find(&l, &[], ST, [70.0, 50.0], 1.0).is_none(), "{:?}", find(&l, &[], ST, [70.0, 50.0], 1.0));
        // its centre still works
        assert_eq!(find(&l, &[], ST, [50.3, 50.2], 1.0).unwrap().kind, Kind::Centre);
        // a hexagon's corners do
        let hx: Vec<[f64; 2]> = (0..6).map(|k| { let t = k as f64 * std::f64::consts::TAU / 6.0; [100.0 + 20.0 * t.cos(), 100.0 + 20.0 * t.sin()] }).collect();
        let l = vec![new_poly(&hx, true, 0).unwrap()];
        assert_eq!(find(&l, &[], ST, [120.2, 100.1], 1.0).unwrap().kind, Kind::Corner);
    }

    #[test]
    fn crossings_and_skipping() {
        let a = new_poly(&[[0.0, 50.0], [100.0, 50.0]], false, 0).unwrap();
        let b = new_poly(&[[40.0, 0.0], [60.0, 90.0]], false, 0).unwrap();
        let l = vec![a, b];
        let h = find(&l, &[], ST, [51.3, 50.3], 1.0).unwrap();
        assert_eq!(h.kind, Kind::Crossing);
        assert!((h.p[0] - (40.0 + 20.0 * 50.0 / 90.0)).abs() < 1e-6 && (h.p[1] - 50.0).abs() < 1e-6, "{:?}", h.p);
        // the moving shape is ignored
        assert!(find(&l, &[0], ST, [51.3, 50.3], 1.0).is_none());
    }

    #[test]
    fn stock_corners_centre_and_edge_middles() {
        let h = find(&[], &[], ST, [199.5, 0.4], 1.0).unwrap();
        assert_eq!((h.kind, h.p), (Kind::Stock, [200.0, 0.0]));
        let h = find(&[], &[], ST, [100.3, 75.2], 1.0).unwrap();
        assert_eq!((h.kind, h.p), (Kind::Stock, [100.0, 75.0]));
    }
}
