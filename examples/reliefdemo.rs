//! Builds a demo project for "Relief from shapes": a plate with a rounded edge, a hole and an island in the hole, carved as a 3D relief.
//! Usage: cargo run --release --example reliefdemo -- <out.json> [flat|rounded|dome|cone]
use putiacnc_core::reliefshapes::{self, Params, Profile};
use putiacnc_core::{save_project, shapes};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let profile = match a.get(2).map(String::as_str) { Some("flat") => Profile::Flat, Some("dome") => Profile::Dome, Some("cone") => Profile::Cone, _ => Profile::Rounded };
    let mut f = shapes::blank_project();
    let d = &mut f.data;
    d.tools = putiacnc_core::setup::starter_tools(putiacnc_core::setup::ToolChoice { ball: true, ..Default::default() });
    putiacnc_core::sample::fill_kind(d, false, putiacnc_core::sample::Kind::Relief).unwrap();
    d.loops.clear();
    d.loops.push(shapes::new_rect([15.0, 20.0], [75.0, 70.0], 6.0, 0));
    d.loops.push(shapes::new_circle([45.0, 45.0], 11.0, 0));
    d.loops.push(shapes::new_circle([45.0, 45.0], 5.0, 0));
    let refs: Vec<&putiacnc_core::model::LoopData> = d.loops.iter().collect();
    let p = Params { height: 5.0, profile, edge: 3.0, margin: 3.0 };
    let rel = reliefshapes::from_shapes(&refs, &p, "Shapes").unwrap();
    d.relief = Some(rel);
    for o in d.ops.iter_mut() { o.p.depth = 5.0; o.p.pass_depth = Some(1.5); }
    d.notes = String::new();
    std::fs::write(&a[1], save_project(&f).unwrap()).unwrap();
}
