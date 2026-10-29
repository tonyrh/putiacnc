//! Path ordering: the order of the cuts does not depend on the order the shapes were drawn in.
use putiacnc_core::{generate, load_project, Units};
use std::path::PathBuf;

fn load(name: &str) -> putiacnc_core::model::ProjectFile {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(name);
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn lengths(o: &putiacnc_core::Output) -> (f64, f64) {
    let len = |cut: bool| o.moves.iter().filter(|m| m.cut == cut).map(|m| (m.to[0] - m.from[0]).hypot(m.to[1] - m.from[1])).sum::<f64>();
    (len(true), len(false))
}

#[test]
fn the_order_does_not_depend_on_the_drawing_order() {
    for name in ["outside_holes.project.json", "pocket_island.project.json"] {
        let p = load(name);
        let a = lengths(&generate(&p.data, Units::Metric).unwrap());
        let mut q = p.clone();
        q.data.loops.reverse();
        let b = lengths(&generate(&q.data, Units::Metric).unwrap());
        assert!((a.0 - b.0).abs() < 0.5, "{name}: cutting length {} vs {}", a.0, b.0);
        // nearest first: the travel is about the same whichever shape was drawn first
        assert!((a.1 - b.1).abs() < 0.1 * a.1.max(10.0), "{name}: rapids {} vs {}", a.1, b.1);
    }
}

#[test]
fn nearest_first_beats_a_bad_drawing_order_on_two_operations() {
    // two operations with a few shapes each: the rapids stay short (the unordered program of this sample travelled 3026 mm)
    let o = generate(&load("two_ops_toolchange.project.json").data, Units::Metric).unwrap();
    assert!(lengths(&o).1 < 1000.0, "rapids {}", lengths(&o).1);
}
