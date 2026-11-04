//! Adaptive clearing: a light stepover and rounded inner rings clear the same pocket.
use putiacnc_core::sim::Sim;
use putiacnc_core::{generate, load_project, Units};
use std::fs;
use std::path::PathBuf;

fn project(name: &str, adaptive: bool) -> putiacnc_core::ProjectData {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(format!("{name}.project.json"));
    let mut d = load_project(&fs::read_to_string(p).unwrap()).unwrap().data;
    for o in &mut d.ops { if o.p.kind == "pocket" { o.p.adaptive = adaptive; } }
    d
}

#[test]
fn the_pocket_is_cleared_as_before_with_a_lighter_stepover() {
    {
        let name = "pocket_island";
        let (a, b) = (project(name, false), project(name, true));
        let (plain, light) = (generate(&a, Units::Metric).unwrap(), generate(&b, Units::Metric).unwrap());
        assert!(light.lines.len() > plain.lines.len(), "{name}: a lighter stepover means more rings");
        let (mut s1, mut s2) = (Sim::new(&a, 160_000), Sim::new(&b, 160_000));
        s1.advance(&plain.moves, plain.moves.len());
        s2.advance(&light.moves, light.moves.len());
        let worst = (0..s1.nx).flat_map(|x| (0..s1.ny).map(move |y| (x, y))).map(|(x, y)| (s1.at(x, y) - s2.at(x, y)).abs()).fold(0.0f32, f32::max);
        assert!(worst < 0.3, "{name}: the stock differs by {worst} mm");
    }
}
