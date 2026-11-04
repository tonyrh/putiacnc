//! Top speed of each axis: the time estimate holds a move down to what its slowest axis allows.
use putiacnc_core::{generate, load_project, Units};
use std::fs;
use std::path::PathBuf;

fn project() -> putiacnc_core::ProjectData {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join("outside_ramp.project.json");
    load_project(&fs::read_to_string(p).unwrap()).unwrap().data
}

#[test]
fn a_slow_z_lengthens_the_plunges_and_says_so() {
    let mut d = project();
    let plain = generate(&d, Units::Metric).unwrap();
    assert!(!plain.notes.iter().any(|n| n.contains("top speed")), "{:?}", plain.notes);
    let mc = d.machine.as_mut().expect("a machine");
    mc.rate = Some(3000.0);
    mc.rate_z = Some(60.0); // far slower than the plunge of the tool
    let slow = generate(&d, Units::Metric).unwrap();
    assert!(slow.time_min > plain.time_min, "{} {}", slow.time_min, plain.time_min);
    assert!(slow.notes.iter().any(|n| n.contains("top speed")), "{:?}", slow.notes);
    // no move is slower than asked for anywhere it is not held down, and none is faster than the Z limit allows along Z
    for m in slow.moves.iter().filter(|m| (m.to[0] - m.from[0]).abs() < 1e-9 && (m.to[1] - m.from[1]).abs() < 1e-9 && (m.to[2] - m.from[2]).abs() > 1e-9) {
        assert!(m.feed <= 60.0 + 1e-6, "a plunge at {} mm/min", m.feed);
    }
    // the G-code itself does not change: the controller does the holding down
    assert_eq!(slow.lines, plain.lines);
}
