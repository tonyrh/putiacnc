//! Dogbone corners: the toolpath of a pocket made from them really reaches the corners.
use putiacnc_core::joints::{dogbones, Cut};
use putiacnc_core::{generate, load_project, shapes, Units};

fn project() -> putiacnc_core::model::ProjectFile {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/outside_holes.project.json");
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// Closest approach of the cutter centre to (x, y) over all cutting moves of a G-code program.
fn closest(lines: &[String], x: f64, y: f64) -> f64 {
    let (mut cx, mut cy, mut z, mut best) = (0.0, 0.0, 5.0, f64::MAX);
    for s in lines.iter().filter(|s| s.starts_with("G0") || s.starts_with("G1")) {
        let v = |c: char| s.split_whitespace().find(|w| w.starts_with(c)).and_then(|w| w[1..].parse::<f64>().ok());
        (cx, cy, z) = (v('X').unwrap_or(cx), v('Y').unwrap_or(cy), v('Z').unwrap_or(z));
        if s.starts_with("G1") && z < 0.0 { best = best.min((cx - x).hypot(cy - y)); }
    }
    best
}

#[test]
fn a_pocket_of_a_dogboned_shape_takes_the_cutter_into_the_corner() {
    let mut f = project();
    f.data.loops.clear();
    f.data.ops.clear();
    let tool = f.data.tools[0].clone();
    let r = tool.dia / 2.0;
    let sq = shapes::new_rect([20.0, 20.0], [50.0, 50.0], 0.0, 0);
    let pocket = |f: &mut putiacnc_core::model::ProjectFile, ls: Vec<putiacnc_core::model::LoopData>| {
        f.data.loops = ls;
        f.data.ops.clear();
        let sel: std::collections::BTreeSet<usize> = (0..f.data.loops.len()).collect();
        shapes::add_operation(&mut f.data, &sel, "pocket");
        f.data.ops[0].p.tool = tool.id.clone();
        generate(&f.data, Units::Metric).unwrap().lines
    };
    let plain = closest(&pocket(&mut f, vec![sq.clone()]), 20.0, 20.0);
    // without a relief the centre stays the cutter's radius away from both walls: r * sqrt(2) from the corner
    assert!((plain - r * 2.0_f64.sqrt()).abs() < 0.05, "{plain} for r {r}");
    let (ls, n) = dogbones(&[&sq], tool.dia, Cut::Inside, 0).unwrap();
    assert_eq!(n, 4);
    let with = pocket(&mut f, ls);
    for (x, y) in [(20.0, 20.0), (50.0, 20.0), (50.0, 50.0), (20.0, 50.0)] {
        let d = closest(&with, x, y);
        assert!(d < r + 0.05, "corner ({x}, {y}): the centre comes within {d} (radius {r})");
    }
}

#[test]
fn a_profile_cut_around_a_notched_part_reaches_the_notch_corners() {
    let mut f = project();
    f.data.ops.clear();
    let tool = f.data.tools[0].clone();
    let r = tool.dia / 2.0;
    let part = shapes::new_poly(&[[10.0, 10.0], [60.0, 10.0], [60.0, 40.0], [45.0, 40.0], [45.0, 30.0], [25.0, 30.0], [25.0, 40.0], [10.0, 40.0]], true, 0).unwrap();
    let (ls, n) = dogbones(&[&part], tool.dia, Cut::Outside, 0).unwrap();
    assert_eq!(n, 2);
    f.data.loops = ls;
    let sel: std::collections::BTreeSet<usize> = (0..f.data.loops.len()).collect();
    shapes::add_operation(&mut f.data, &sel, "outside");
    f.data.ops[0].p.tool = tool.id.clone();
    f.data.ops[0].p.depth = 2.0;
    let lines = generate(&f.data, Units::Metric).unwrap().lines;
    // the cutter centre comes within a radius of both bottom corners of the notch
    for (x, y) in [(25.0, 30.0), (45.0, 30.0)] {
        let d = closest(&lines, x, y);
        assert!(d < r + 0.05, "notch corner ({x}, {y}): {d}");
    }
}
