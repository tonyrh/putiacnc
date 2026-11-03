use putiacnc_core::model::Operation;
use putiacnc_core::{generate, shapes, Units};

fn project(kind: &str, leave: Option<f64>, w: f64) -> putiacnc_core::ProjectData {
    let mut f = shapes::blank_project();
    let d = &mut f.data;
    d.stock.w = 80.0; d.stock.h = 60.0; d.stock.t = 12.0;
    let tool = d.tools.iter().find(|t| t.name.starts_with("3 mm flat")).unwrap().id.clone();
    let mut p = shapes::default_op_params(&tool);
    p.kind = kind.into();
    p.depth = 6.0; p.d_man = true;
    p.pass_depth = Some(2.0);
    p.w_leave = leave;
    d.loops.push(shapes::new_rect([20.0, 15.0], [20.0 + w, 45.0], 0.0, 1));
    d.ops.push(Operation { id: 1, p });
    d.oid = 1; d.act = 1;
    f.data
}

#[test]
fn a_profile_leaves_stock_in_the_depth_passes_and_cuts_to_size_in_a_last_pass() {
    let plain = generate(&project("outside", None, 30.0), Units::Metric).unwrap();
    let o = generate(&project("outside", Some(0.3), 30.0), Units::Metric).unwrap();
    assert!(!plain.gcode().contains("finishing pass"));
    assert!(o.gcode().contains("finishing pass to size"));
    // the tool is 3 mm: its centre runs 1.5 mm outside the rectangle (x = 20 - 1.5 on the left) at size, 0.3 mm further out in the depth passes
    let xs: Vec<f64> = o.moves.iter().filter(|m| m.cut && m.from[2] == m.to[2] && m.from[0] == m.to[0] && (m.from[1] - m.to[1]).abs() > 5.0 && m.to[0] < 30.0).map(|m| m.to[0]).collect();
    assert!(xs.iter().any(|x| (x - 18.5).abs() < 0.01), "nothing cut to size: {xs:?}");
    assert!(xs.iter().any(|x| (x - 18.2).abs() < 0.01), "nothing cut oversize: {xs:?}");
    // the last cut on the left wall is the one to size
    assert!((xs.last().unwrap() - 18.5).abs() < 0.01);
    assert!(o.moves.iter().filter(|m| m.cut).count() > plain.moves.iter().filter(|m| m.cut).count());
}

#[test]
fn a_pocket_gets_a_wall_finishing_pass_and_a_narrow_one_is_cut_in_one_go() {
    let o = generate(&project("pocket", Some(0.3), 30.0), Units::Metric).unwrap();
    assert!(o.gcode().contains("finishing pass to size"));
    // inside the pocket (x 20..50) the tool centre reaches 1.5 mm from the wall at size and 1.8 mm in the depth passes
    let xs: Vec<f64> = o.moves.iter().filter(|m| m.cut && m.from[0] == m.to[0] && (m.from[1] - m.to[1]).abs() > 5.0 && m.to[0] < 25.0).map(|m| m.to[0]).collect();
    assert!(xs.iter().any(|x| (x - 21.5).abs() < 0.01), "{xs:?}");
    assert!(xs.iter().all(|x| *x >= 21.5 - 0.01), "the wall is gouged: {xs:?}");
    // a slot 3.4 mm wide: the 3 mm tool fits, but not with 0.3 mm left on each side
    let n = generate(&project("pocket", Some(0.3), 3.4), Units::Metric).unwrap();
    assert!(n.notes.iter().any(|t| t.contains("too narrow to leave")), "{:?}", n.notes);
    assert!(n.moves.iter().any(|m| m.cut));
}
