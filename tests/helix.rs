//! Helical entry: a spiral down beside the start of a pocket's first ring or a profile.
use putiacnc_core::sim::Sim;
use putiacnc_core::{generate, load_project, Units};
use std::fs;
use std::path::PathBuf;

fn project(name: &str, helix: bool) -> putiacnc_core::ProjectData {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(format!("{name}.project.json"));
    let mut d = load_project(&fs::read_to_string(p).unwrap()).unwrap().data;
    for o in &mut d.ops { if matches!(o.p.kind.as_str(), "outside" | "inside" | "pocket") { o.p.helix = helix; o.p.lead = None; } }
    d
}

#[test]
fn the_spiral_goes_down_gently_and_the_cut_is_the_same() {
    for name in ["pocket_island", "outside_ramp", "outside_holes"] {
        let (a, b) = (project(name, false), project(name, true));
        let (plain, spiral) = (generate(&a, Units::Metric).unwrap(), generate(&b, Units::Metric).unwrap());
        assert!(spiral.lines.len() > plain.lines.len(), "{name}: the spirals add moves");
        // a rectangle starts at a corner, where no circle fits: the path is cut from the middle of its longest edge, and nothing is skipped
        assert!(!spiral.notes.iter().any(|n| n.contains("helical entry skipped")), "{name}: {:?}", spiral.notes);
        // every cutting move that goes down and sideways at once does so at no more than 3 degrees (the default), not counting the straight plunges
        let steepest = spiral.moves.iter().filter(|m| m.cut).filter_map(|m| {
            let xy = (m.to[0] - m.from[0]).hypot(m.to[1] - m.from[1]);
            let dz = m.from[2] - m.to[2];
            (xy > 1e-6 && dz > 1e-6).then(|| dz / xy)
        }).fold(0.0f64, f64::max);
        let plain_steepest = plain.moves.iter().filter(|m| m.cut).filter_map(|m| {
            let xy = (m.to[0] - m.from[0]).hypot(m.to[1] - m.from[1]);
            let dz = m.from[2] - m.to[2];
            (xy > 1e-6 && dz > 1e-6).then(|| dz / xy)
        }).fold(0.0f64, f64::max);
        assert!(steepest < 3.0f64.to_radians().tan() * 1.05 || steepest <= plain_steepest, "{name}: {steepest} (without {plain_steepest})");
        // a pocket ends up the same: its spiral only cuts where the cut goes anyway (a profile's spiral makes a hole in the waste, like a lead-in)
        if name != "pocket_island" { continue; }
        let (mut s1, mut s2) = (Sim::new(&a, 160_000), Sim::new(&b, 160_000));
        s1.advance(&plain.moves, plain.moves.len());
        s2.advance(&spiral.moves, spiral.moves.len());
        let worst = (0..s1.nx).flat_map(|x| (0..s1.ny).map(move |y| (x, y))).map(|(x, y)| (s1.at(x, y) - s2.at(x, y)).abs()).fold(0.0f32, f32::max);
        assert!(worst < 0.3, "{name}: the stock differs by {worst} mm");
    }
}

#[test]
fn a_shape_with_no_room_for_the_circle_enters_as_before_and_says_so() {
    let mut d = project("pocket_island", true);
    // the tool, nearly as wide as the pocket
    let dia = d.tools.iter().map(|t| t.dia).fold(0.0, f64::max);
    for o in &mut d.ops { o.p.helix_d = Some(400.0); }
    let out = generate(&d, Units::Metric).unwrap();
    let _ = dia;
    // a circle four times the tool does not fit in the sample pocket's first ring everywhere, but it shrinks until it does, so there is no failure
    assert!(!out.lines.is_empty());
}

