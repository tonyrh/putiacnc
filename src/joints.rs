//! Joints for CNC parts: dogbone corner reliefs (and, below, finger joints).
//!
//! A round cutter cannot reach into a sharp inside corner: it leaves a radius, and a square corner of a mating part will not go
//! in. A dogbone cuts a little further into the walls at the corner, with the cutter's centre going out to a point one radius
//! from the corner, so the corner itself is reached.
use crate::clip::{self, BoolKind, Path};
use crate::model::LoopData;
use crate::shapes::S;

/// Which corners get a relief.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    /// The cutter works inside the shapes (pockets, slots, holes for square parts): their sharp corners are the inside corners.
    Inside,
    /// The cutter works around the shapes (the outline of a part with notches): the notches' corners are the inside corners.
    Outside,
}

/// Corners opener than this (the angle between the two walls, degrees) are left alone: the cutter nearly reaches them, and the
/// vertices of a flattened curve are never touched.
const MAX_ANGLE: f64 = 140.0;
/// Corners sharper than this are spikes, not corners.
const MIN_ANGLE: f64 = 5.0;

/// The polygon of two discs of radius `r` at `a` and `b` and everything between them (a slot with round ends).
fn capsule(a: [f64; 2], b: [f64; 2], r: f64) -> Path {
    let steps = ((std::f64::consts::PI / (1.0 - 0.01 / r.max(0.02)).clamp(-1.0, 1.0).acos()).ceil() as usize).clamp(12, 72);
    let dir = [b[0] - a[0], b[1] - a[1]];
    let ang = dir[1].atan2(dir[0]);
    let mut pts: Vec<[f64; 2]> = Vec::new();
    // half a circle round `b`, from the left of the direction to the right of it, then half round `a`
    for k in 0..=steps { let t = ang + std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * k as f64 / steps as f64; pts.push([b[0] + r * t.cos(), b[1] + r * t.sin()]); }
    for k in 0..=steps { let t = ang - std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * k as f64 / steps as f64; pts.push([a[0] + r * t.cos(), a[1] + r * t.sin()]); }
    // the walk above goes clockwise: turn it round so the capsule is counter-clockwise like a region
    pts.reverse();
    pts.iter().map(|p| [(p[0] * S).round() as i64, (p[1] * S).round() as i64]).collect()
}

fn closed_paths(loops: &[&LoopData]) -> Vec<Path> {
    loops.iter().filter(|l| l.closed() && l.pts.len() >= 6).map(|l| {
        let mut p: Path = l.pts.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
        if p.len() > 1 && p[0] == p[p.len() - 1] { p.pop(); }
        p
    }).collect()
}

/// The shapes with a dogbone relief at each inside corner, for a cutter of diameter `tool_dia` (mm). The shapes are joined as one
/// region first (a shape inside another is a hole, as everywhere in the app), so a result may have fewer or more shapes than the
/// input; all take operation `op`. Returns the new shapes and the number of corners done, or `None` when there is no such corner.
pub fn dogbones(loops: &[&LoopData], tool_dia: f64, cut: Cut, op: i64) -> Option<(Vec<LoopData>, usize)> {
    if !(tool_dia > 1e-3) { return None; }
    let r = tool_dia / 2.0;
    let region = clip::simplify_even_odd(&closed_paths(loops));
    if region.is_empty() { return None; }
    // which side of its path the region lies on: the largest path is an outer one
    let big = region.iter().map(|p| clip::area(p)).fold(0.0_f64, |m, a| if a.abs() > m.abs() { a } else { m });
    let left_is_region = big > 0.0;
    let mut caps: Vec<Path> = Vec::new();
    for path in &region {
        let n = path.len();
        if n < 3 { continue; }
        let pt = |i: usize| { let q = path[i % n]; [q[0] as f64 / S, q[1] as f64 / S] };
        for i in 0..n {
            let (a, p, b) = (pt(i + n - 1), pt(i), pt(i + 1));
            let (va, vb) = ([a[0] - p[0], a[1] - p[1]], [b[0] - p[0], b[1] - p[1]]);
            let (da, db) = (va[0].hypot(va[1]), vb[0].hypot(vb[1]));
            if da < 0.05 || db < 0.05 { continue; }
            let (ua, ub) = ([va[0] / da, va[1] / da], [vb[0] / db, vb[1] / db]);
            let psi = (ua[0] * ub[0] + ua[1] * ub[1]).clamp(-1.0, 1.0).acos().to_degrees();
            if !(MIN_ANGLE..=MAX_ANGLE).contains(&psi) { continue; }
            // a turn towards the region's side is a corner of the region itself (an inside corner for a cutter working inside it)
            let turn = (p[0] - a[0]) * (b[1] - p[1]) - (p[1] - a[1]) * (b[0] - p[0]);
            let towards_region = (turn > 0.0) == left_is_region;
            if (cut == Cut::Inside) != towards_region { continue; }
            // the bisector of the two walls points into the narrow side, where the cutter is
            let bis = [ua[0] + ub[0], ua[1] + ub[1]];
            let bl = bis[0].hypot(bis[1]);
            if bl < 1e-9 { continue; }
            let u = [bis[0] / bl, bis[1] / bl];
            // the cutter's centre runs along the bisector as far as r/sin(angle/2) from the corner; going on to a distance r reaches it
            let far = r / (psi.to_radians() / 2.0).sin();
            // a hair wider than the cutter (0.03 mm): at exactly the cutter's width an offset by the cutter radius collapses to nothing and the toolpath never enters the relief
            caps.push(capsule([p[0] + u[0] * r, p[1] + u[1] * r], [p[0] + u[0] * far, p[1] + u[1] * far], r + 0.03));
        }
    }
    if caps.is_empty() { return None; }
    let count = caps.len();
    let caps = clip::simplify_even_odd(&caps.iter().map(|c| c.clone()).collect::<Vec<_>>());
    // the capsules were joined with the even-odd rule only to merge overlaps: orient them as regions again
    let caps: Vec<Path> = caps.into_iter().collect();
    let res = clip::boolean(if cut == Cut::Inside { BoolKind::Union } else { BoolKind::Difference }, &region, &caps);
    let out: Vec<LoopData> = res.into_iter().filter(|p| p.len() >= 3).map(|p| LoopData {
        c: 1, op, rc: None, cr: None, el: None, pt: None, grp: None, ug: None, pts: p.iter().flat_map(|q| [q[0], q[1]]).collect(),
    }).collect();
    if out.is_empty() { None } else { Some((out, count)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::{new_rect, bbox};

    fn area_mm2(ls: &[LoopData]) -> f64 {
        // signed sum: outers add, holes subtract (the result is oriented as a region)
        ls.iter().map(|l| { let p: Path = l.pts.chunks_exact(2).map(|c| [c[0], c[1]]).collect(); clip::area(&p) }).sum::<f64>().abs() / (S * S)
    }

    #[test]
    fn a_square_pocket_gets_four_reliefs_that_reach_the_corners() {
        let sq = new_rect([10.0, 10.0], [30.0, 30.0], 0.0, 0);
        let (out, n) = dogbones(&[&sq], 3.0, Cut::Inside, 0).unwrap();
        assert_eq!(n, 4);
        assert_eq!(out.len(), 1);
        // each relief adds a little to the area, and the shape stays inside a box a cutter radius larger on each side
        let a = area_mm2(&out);
        assert!(a > 400.0 + 4.0 * 0.3 && a < 400.0 + 4.0 * 5.0, "{a}");
        let b = bbox(&out[0]).unwrap();
        assert!(b[0] < 10.0 && b[0] > 10.0 - 1.5 && b[3] > 30.0 && b[3] < 30.0 + 1.5, "{b:?}");
        // the cutter centre goes to 1.5 mm from the corner: the relief circle of radius 1.5 there passes through the corner
        let pts: Vec<[f64; 2]> = out[0].pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect();
        let c = [10.0 + 1.5 * 0.5_f64.sqrt(), 10.0 + 1.5 * 0.5_f64.sqrt()];
        // some vertex of the result lies a radius from that centre and beyond the walls
        assert!(pts.iter().any(|p| ((p[0] - c[0]).hypot(p[1] - c[1]) - 1.5).abs() < 0.05 && p[0] < 10.0), "overcut into the wall");
    }

    #[test]
    fn a_part_with_a_notch_gets_reliefs_only_in_the_notch_corners_when_cut_around() {
        // a 40 x 30 plate with a 10 x 5 notch in the top edge
        let plate = crate::shapes::new_poly(&[[0.0, 0.0], [40.0, 0.0], [40.0, 30.0], [25.0, 30.0], [25.0, 25.0], [15.0, 25.0], [15.0, 30.0], [0.0, 30.0]], true, 0).unwrap();
        let (out, n) = dogbones(&[&plate], 3.0, Cut::Outside, 0).unwrap();
        assert_eq!(n, 2, "the two corners at the bottom of the notch");
        // material is taken out of the plate
        assert!(area_mm2(&out) < 40.0 * 30.0 - 50.0 && area_mm2(&out) > 40.0 * 30.0 - 50.0 - 4.0 * 3.0, "{}", area_mm2(&out));
        // cut inside (a pocket of this shape) the same corners are the outer ones: six corners of the outline are convex
        let (_, m) = dogbones(&[&plate], 3.0, Cut::Inside, 0).unwrap();
        assert_eq!(m, 6);
    }

    #[test]
    fn holes_inside_a_pocket_count_from_the_region_not_the_shape() {
        let outer = new_rect([0.0, 0.0], [40.0, 40.0], 0.0, 0);
        let island = new_rect([15.0, 15.0], [25.0, 25.0], 0.0, 0);
        // an island in a pocket: its corners point into the pocket, so only the four outer corners are inside corners
        let (out, n) = dogbones(&[&outer, &island], 3.0, Cut::Inside, 0).unwrap();
        assert_eq!(n, 4);
        assert_eq!(out.len(), 2, "the pocket and the island");
        // a hole in a part, cut around: the hole's corners are the part's reflex corners
        let (_, m) = dogbones(&[&outer, &island], 3.0, Cut::Outside, 0).unwrap();
        assert_eq!(m, 4);
    }

    #[test]
    fn round_shapes_obtuse_corners_and_bad_sizes_get_nothing() {
        let circle = crate::shapes::new_ellipse([10.0, 10.0], [30.0, 30.0], 0);
        assert!(dogbones(&[&circle], 3.0, Cut::Inside, 0).is_none());
        // a hexagon's 120 degree corners still need a relief for a nut to go in; the 150 degree corners of a 12-sided shape do not
        let hex = crate::shapes::new_polygon([0.0, 0.0], 10.0, 0.0, 6, None, 0).unwrap();
        assert_eq!(dogbones(&[&hex], 3.0, Cut::Inside, 0).unwrap().1, 6);
        let dodecagon = crate::shapes::new_polygon([0.0, 0.0], 10.0, 0.0, 12, None, 0).unwrap();
        assert!(dogbones(&[&dodecagon], 3.0, Cut::Inside, 0).is_none());
        let sq = new_rect([0.0, 0.0], [10.0, 10.0], 0.0, 0);
        assert!(dogbones(&[&sq], 0.0, Cut::Inside, 0).is_none());
        assert!(dogbones(&[], 3.0, Cut::Inside, 0).is_none());
        let line = crate::shapes::new_poly(&[[0.0, 0.0], [5.0, 5.0]], false, 0).unwrap();
        assert!(dogbones(&[&line], 3.0, Cut::Inside, 0).is_none());
    }
}
