//! A relief built from closed shapes: each shape gets a height and a profile, and the result is a height map like an imported model's,
//! carved by the same 3D relief operation. A shape inside another one is a hole in it (as in a pocket), so the counters of letters work.
use crate::model::{LoopData, Relief};
use crate::shapes::{bbox_many, point_in_loop};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Profile {
    /// A flat top at the full height, with vertical sides.
    Flat,
    /// A flat top with a rounded rim: the rim is `edge` wide (a quarter ellipse), the rest is at the full height.
    Rounded,
    /// A dome over the whole shape: round at the edge, highest along the middle.
    Dome,
    /// A slope from the edge up to a ridge along the middle.
    Cone,
}
pub const PROFILES: [(Profile, &str); 4] = [(Profile::Flat, "Flat"), (Profile::Rounded, "Rounded"), (Profile::Dome, "Dome"), (Profile::Cone, "Slope")];

#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Height of the highest point, mm.
    pub height: f64,
    pub profile: Profile,
    /// Width of the rounded rim, mm (Rounded edge only).
    pub edge: f64,
    /// Background kept around the shapes, mm (it stays at the lowest level).
    pub margin: f64,
}
impl Default for Params { fn default() -> Self { Params { height: 5.0, profile: Profile::Dome, edge: 2.0, margin: 0.0 } } }

const MAX_CELLS: f64 = 600_000.0;

/// The profile's height (0 to 1 of the full height) at distance `d` mm inside the edge of a piece whose deepest point is `dmax` mm in.
pub fn profile_height(p: &Params, d: f64, dmax: f64) -> f64 {
    let round = |t: f64| { let t = t.clamp(0.0, 1.0); (1.0 - (1.0 - t) * (1.0 - t)).max(0.0).sqrt() };
    match p.profile {
        Profile::Flat => 1.0,
        Profile::Rounded => round(d / p.edge.min(dmax).max(1e-6)),
        Profile::Dome => round(d / dmax.max(1e-6)),
        Profile::Cone => (d / dmax.max(1e-6)).clamp(0.0, 1.0),
    }
}

/// Fills the cells whose centres are inside the loop into `mask`, toggling them (so a hole drawn on top of a shape clears it).
fn toggle_fill(mask: &mut [bool], nx: usize, ny: usize, ox: f64, oy: f64, res: f64, l: &LoopData) {
    let v: Vec<[f64; 2]> = l.pts.chunks_exact(2).map(|c| [c[0] as f64 / 1000.0, c[1] as f64 / 1000.0]).collect();
    let n = v.len();
    let mut xs: Vec<f64> = Vec::new();
    for j in 0..ny {
        let y = oy + (j as f64 + 0.5) * res;
        xs.clear();
        for i in 0..n {
            let (a, b) = (v[i], v[(i + 1) % n]);
            if (a[1] > y) != (b[1] > y) { xs.push(a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0])); }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.chunks_exact(2) {
            let i0 = ((pair[0] - ox) / res - 0.5).ceil().max(0.0) as usize;
            let i1 = ((pair[1] - ox) / res - 0.5).floor();
            if i1 < 0.0 { continue; }
            for i in i0..=(i1 as usize).min(nx - 1) { if i >= i0 { mask[j * nx + i] = !mask[j * nx + i]; } }
        }
    }
}

/// Squared distance transform of one line (Felzenszwalb and Huttenlocher): `out[q]` = the least of `(q - r)^2 + f[r]` over r.
fn pass(f: &[f64], out: &mut [f64]) {
    let n = f.len();
    let mut v = vec![0usize; n];
    let mut z = vec![0f64; n + 1];
    let mut k = 0usize;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        let mut s;
        loop {
            let r = v[k];
            s = ((f[q] + (q * q) as f64) - (f[r] + (r * r) as f64)) / (2.0 * q as f64 - 2.0 * r as f64);
            if s <= z[k] { k -= 1; } else { break; }
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, o) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f64 { k += 1; }
        let d = q as f64 - v[k] as f64;
        *o = d * d + f[v[k]];
    }
}

/// Distance in cells from every cell of the mask to the nearest cell outside it (0 for cells outside). The area beyond the grid counts as
/// outside, so a shape that fills the grid still has its rim at the grid's edge.
fn distance_inside(mask: &[bool], nx: usize, ny: usize) -> Vec<f64> {
    const INF: f64 = 1e12;
    let (px, py) = (nx + 2, ny + 2);
    let mut g = vec![0f64; px * py];
    for j in 0..ny { for i in 0..nx { if mask[j * nx + i] { g[(j + 1) * px + i + 1] = INF; } } }
    let mut col = vec![0f64; py];
    let mut colo = vec![0f64; py];
    for i in 0..px {
        for j in 0..py { col[j] = g[j * px + i]; }
        pass(&col, &mut colo);
        for j in 0..py { g[j * px + i] = colo[j]; }
    }
    let mut rowo = vec![0f64; px];
    for j in 0..py {
        pass(&g[j * px..(j + 1) * px], &mut rowo);
        for i in 0..px { g[j * px + i] = rowo[i].min(INF).sqrt(); }
    }
    let mut out = vec![0f64; nx * ny];
    for j in 0..ny { for i in 0..nx { out[j * nx + i] = g[(j + 1) * px + i + 1]; } }
    out
}

/// The relief of the closed shapes `loops`, lower-left corner at the shapes' own lower-left (less the margin), scale 1.
pub fn from_shapes(loops: &[&LoopData], p: &Params, name: &str) -> Result<Relief, String> {
    if !(p.height > 0.0) || !p.height.is_finite() { return Err("The height must be more than zero.".into()); }
    let shapes: Vec<&LoopData> = loops.iter().copied().filter(|l| l.closed() && l.pts.len() >= 6).collect();
    if shapes.is_empty() { return Err("Select closed shapes to build a relief from.".into()); }
    let b = bbox_many(shapes.iter().copied()).ok_or("The shapes have no size.")?;
    let m = p.margin.max(0.0);
    let (x0, y0, x1, y1) = (b[0] - m, b[1] - m, b[2] + m, b[3] + m);
    let (w, h) = (x1 - x0, y1 - y0);
    if w < 1e-3 || h < 1e-3 { return Err("The shapes have no width or height.".into()); }
    let res = (((w * h) / MAX_CELLS).sqrt().max(0.2) * 100.0).round() / 100.0;
    let (nx, ny) = (((w / res).ceil() as usize).max(2), ((h / res).ceil() as usize).max(2));
    // nesting: a shape inside an odd number of others is a hole of the shape that most closely holds it
    let first = |l: &LoopData| [l.pts[0] as f64 / 1000.0, l.pts[1] as f64 / 1000.0];
    let holders: Vec<Vec<usize>> = (0..shapes.len()).map(|i| (0..shapes.len()).filter(|&j| j != i && point_in_loop(shapes[j], first(shapes[i]))).collect()).collect();
    let mut z = vec![0f32; nx * ny];
    for i in 0..shapes.len() {
        if holders[i].len() % 2 == 1 { continue; }
        // this shape and the holes directly inside it
        let holes: Vec<usize> = (0..shapes.len()).filter(|&k| holders[k].len() == holders[i].len() + 1 && holders[k].contains(&i)).collect();
        let mut mask = vec![false; nx * ny];
        toggle_fill(&mut mask, nx, ny, x0, y0, res, shapes[i]);
        for &k in &holes { toggle_fill(&mut mask, nx, ny, x0, y0, res, shapes[k]); }
        if !mask.iter().any(|&v| v) { continue; }
        let dist = distance_inside(&mask, nx, ny);
        let dmax = dist.iter().fold(0f64, |a, &d| a.max(d)) - 0.5;
        for c in 0..nx * ny {
            if !mask[c] { continue; }
            let d = (dist[c] - 0.5).max(0.0) * res + res * 0.5;
            let v = (profile_height(p, d - res * 0.5, dmax.max(0.0) * res) * p.height) as f32;
            // a thin sliver still shows as a ridge at least one level up
            if v > z[c] { z[c] = v; }
        }
    }
    let bytes: Vec<u8> = z.iter().flat_map(|&v| ((v as f64 / p.height * 65535.0).round().clamp(0.0, 65535.0) as u16).to_le_bytes()).collect();
    Ok(Relief { name: name.into(), nx, ny, res, zmax: p.height, hm: crate::relief::b64_encode(&bytes), x: x0, y: y0, scale: 1.0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::{new_circle, new_rect};

    fn at(r: &Relief, x: f64, y: f64) -> f64 {
        let h = r.heights();
        let (i, j) = ((((x - r.x) / r.res) as usize).min(r.nx - 1), (((y - r.y) / r.res) as usize).min(r.ny - 1));
        h[j * r.nx + i] as f64 * r.zmax
    }

    #[test]
    fn a_flat_shape_is_a_plateau_at_the_height() {
        let l = new_rect([10.0, 10.0], [40.0, 30.0], 0.0, 0);
        let r = from_shapes(&[&l], &Params { height: 4.0, profile: Profile::Flat, edge: 2.0, margin: 5.0 }, "t").unwrap();
        assert!((r.zmax - 4.0).abs() < 1e-9 && (r.x - 5.0).abs() < 1e-9 && (r.y - 5.0).abs() < 1e-9);
        assert!((at(&r, 25.0, 20.0) - 4.0).abs() < 0.01, "{}", at(&r, 25.0, 20.0));
        assert!(at(&r, 7.0, 7.0) < 0.01, "the margin is the lowest level");
    }

    #[test]
    fn a_dome_is_round_and_highest_in_the_middle() {
        let l = new_circle([30.0, 30.0], 20.0, 0);
        let r = from_shapes(&[&l], &Params { height: 10.0, profile: Profile::Dome, edge: 2.0, margin: 0.0 }, "t").unwrap();
        let mid = at(&r, 30.0, 30.0);
        assert!(mid > 9.7, "middle {mid}");
        // a sphere cap: at half the radius the height is sqrt(1 - 0.25) of the full one
        let q = at(&r, 40.0, 30.0);
        assert!((q - 10.0 * 0.75f64.sqrt()).abs() < 0.4, "quarter {q}");
        assert!(at(&r, 49.0, 30.0) < 4.0 && at(&r, 49.0, 30.0) > 0.0, "edge {}", at(&r, 49.0, 30.0));
    }

    #[test]
    fn a_slope_rises_in_a_straight_line() {
        let l = new_rect([0.0, 0.0], [40.0, 20.0], 0.0, 0);
        let r = from_shapes(&[&l], &Params { height: 8.0, profile: Profile::Cone, edge: 2.0, margin: 0.0 }, "t").unwrap();
        let (a, b, c) = (at(&r, 2.0, 10.0), at(&r, 5.0, 10.0), at(&r, 8.0, 10.0));
        assert!(a < b && b < c, "{a} {b} {c}");
        assert!(((c - b) - (b - a)).abs() < 0.3, "straight: {a} {b} {c}");
        assert!(at(&r, 20.0, 10.0) > 7.6);
    }

    #[test]
    fn a_rounded_edge_is_round_at_the_rim_and_flat_inside() {
        let l = new_rect([0.0, 0.0], [40.0, 40.0], 0.0, 0);
        let r = from_shapes(&[&l], &Params { height: 6.0, profile: Profile::Rounded, edge: 4.0, margin: 0.0 }, "t").unwrap();
        assert!((at(&r, 20.0, 20.0) - 6.0).abs() < 0.01 && (at(&r, 10.0, 20.0) - 6.0).abs() < 0.01);
        let rim = at(&r, 1.0, 20.0);
        assert!(rim > 1.0 && rim < 5.0, "rim {rim}");
    }

    #[test]
    fn a_shape_inside_another_is_a_hole_and_overlaps_take_the_higher() {
        let outer = new_rect([0.0, 0.0], [60.0, 60.0], 0.0, 0);
        let hole = new_rect([20.0, 20.0], [40.0, 40.0], 0.0, 0);
        let r = from_shapes(&[&outer, &hole], &Params { height: 5.0, profile: Profile::Flat, edge: 2.0, margin: 0.0 }, "t").unwrap();
        assert!(at(&r, 30.0, 30.0) < 0.01, "the inner shape is a hole");
        assert!((at(&r, 10.0, 30.0) - 5.0).abs() < 0.01);
        // an island inside the hole is a shape of its own
        let island = new_rect([27.0, 27.0], [33.0, 33.0], 0.0, 0);
        let r = from_shapes(&[&outer, &hole, &island], &Params { height: 5.0, profile: Profile::Flat, edge: 2.0, margin: 0.0 }, "t").unwrap();
        assert!((at(&r, 30.0, 30.0) - 5.0).abs() < 0.01, "the island stands in the hole");
        assert!(at(&r, 23.0, 30.0) < 0.01);
    }

    #[test]
    fn bad_input_is_refused() {
        let l = new_rect([0.0, 0.0], [10.0, 10.0], 0.0, 0);
        assert!(from_shapes(&[], &Params::default(), "t").is_err());
        assert!(from_shapes(&[&l], &Params { height: 0.0, ..Params::default() }, "t").is_err());
    }
}
