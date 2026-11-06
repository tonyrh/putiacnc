//! V-carve with a flat depth: the V-bit carves the sloped band along the edges, a flat end mill clears the floor inside, no ridge between them.
use putiacnc_core::{generate, shapes, sim::Sim, Units};

fn project(flat: Option<f64>, clear: bool) -> putiacnc_core::ProjectData {
    let mut d = shapes::blank_project().data;
    d.stock.w = 100.0; d.stock.h = 80.0; d.stock.t = 12.0;
    let mut v = d.tools.iter().find(|t| t.shape == "vbit").expect("a V-bit").clone();
    v.id = "v90".into(); v.angle = 90.0; v.dia = 12.0; v.name = "90 deg V-bit".into();
    d.tools.push(v);
    let flat_id = d.tools.iter().find(|t| t.shape == "flat" && t.dia < 3.5).expect("a small flat end mill").id.clone();
    let mut p = shapes::default_op_params("v90");
    p.kind = "vcarve".into();
    p.depth = 5.0;
    p.flat = flat;
    if clear { p.tool2 = Some(flat_id); }
    d.ops.push(putiacnc_core::model::Operation { id: 1, p });
    d.oid = 2; d.act = 1;
    d.loops.push(shapes::new_rect([20.0, 20.0], [80.0, 60.0], 0.0, 1));
    d
}

fn floor_of(d: &putiacnc_core::ProjectData) -> (Vec<f64>, f64, f64) {
    let out = generate(d, Units::Metric).unwrap();
    let mut s = Sim::new(d, 400_000);
    s.advance(&out.moves, out.moves.len());
    let h = |x: f64, y: f64| s.at((x / s.cell).round() as usize, (y / s.cell).round() as usize) as f64;
    // every point of the interior, 3 mm or more from the edge
    let mut v = Vec::new();
    let mut y = 23.0;
    while y <= 57.0 { let mut x = 23.0; while x <= 77.0 { v.push(h(x, y)); x += 1.7; } y += 1.7; }
    (v, h(21.0, 40.0), h(22.0, 40.0))
}

#[test]
fn the_flat_floor_is_level_and_the_walls_slope() {
    let d = project(Some(2.0), true);
    let out = generate(&d, Units::Metric).unwrap();
    assert!(out.moves.iter().any(|m| m.alt), "the flat end mill takes part");
    assert!(out.moves.iter().filter(|m| !m.alt && m.cut).all(|m| m.to[2] >= -2.0 - 1e-6), "the V-bit stops at the flat depth");
    let (floor, near_edge, two_in) = floor_of(&d);
    let (lo, hi) = floor.iter().fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    assert!(lo > -2.06 && hi < -1.94, "the floor must be level at -2: {lo:.3} .. {hi:.3}");
    // a 90 degree V-bit: the wall is one mm deeper for every mm in
    assert!((near_edge + 1.0).abs() < 0.12, "1 mm from the edge: {near_edge}");
    assert!((two_in + 2.0).abs() < 0.12, "2 mm from the edge: {two_in}");
}

#[test]
fn without_a_flat_end_mill_the_v_bit_carves_the_floor_itself_to_the_same_depth() {
    let d = project(Some(2.0), false);
    let out = generate(&d, Units::Metric).unwrap();
    assert!(!out.moves.iter().any(|m| m.alt));
    let (floor, ..) = floor_of(&d);
    let (lo, hi) = floor.iter().fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    // the V-bit leaves small ridges between its rings (that is what the flat end mill is for), but never above the flat depth
    assert!(lo > -2.06 && hi < -1.8, "{lo:.3} .. {hi:.3}");
}

#[test]
fn a_shape_too_narrow_for_the_flat_floor_is_carved_by_the_v_bit_alone() {
    let mut d = project(Some(5.0), true);
    d.loops[0] = shapes::new_rect([20.0, 20.0], [80.0, 26.0], 0.0, 1); // 6 mm wide: the V reaches its middle at 3 mm deep
    let out = generate(&d, Units::Metric).unwrap();
    assert!(!out.moves.iter().any(|m| m.alt), "nothing for the flat end mill");
    assert!(out.notes.iter().any(|n| n.contains("flat")), "{:?}", out.notes);
}

#[test]
fn the_middle_of_a_thin_arm_that_dies_out_between_two_rings_is_carved_to_its_bottom() {
    // a T: its bar is 10 mm wide, so its middle is 5 mm deep for a 90 degree bit, but the bar's ring vanishes between two 0.3 mm steps
    let mut d = project(None, false);
    d.ops[0].p.depth = 10.0;
    let t = [[20.0, 50.0], [70.0, 50.0], [70.0, 60.0], [50.0, 60.0], [50.0, 80.0], [40.0, 80.0], [40.0, 60.0], [20.0, 60.0]];
    d.loops[0] = shapes::new_poly(&t, true, 1).unwrap();
    let out = generate(&d, Units::Metric).unwrap();
    let mut s = Sim::new(&d, 1_000_000);
    s.advance(&out.moves, out.moves.len());
    let h = |x: f64, y: f64| s.at((x / s.cell).round() as usize, (y / s.cell).round() as usize) as f64;
    // along the middle of the bar, away from the stem: 5 mm (it used to stop 0.3 mm short)
    let worst = (0..8).map(|i| h(24.0 + i as f64 * 2.5, 55.0)).fold(f64::MIN, f64::max);
    assert!(worst < -4.9, "the middle of the bar is only {worst:.2} deep");
}
