//! The project for trying the pauses (`M0`) on a real machine: all air cuts, one "Pause afterwards" and one tool change.
//! It is made by `cargo run --release --example m0check -- tests/data/hardware_m0_aircut.project.json`.
use putiacnc_core::{generate, load_project, verify, Units};

fn gcode() -> Vec<String> {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/hardware_m0_aircut.project.json");
    let f = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
    assert_eq!(f.data.ops.len(), 3);
    assert!(f.data.ops.iter().all(|o| o.p.air), "every operation is an air cut");
    generate(&f.data, Units::Metric).unwrap().lines
}

#[test]
fn the_machine_test_has_both_kinds_of_pause_and_cuts_nothing() {
    let l = gcode();
    let m0: Vec<&String> = l.iter().filter(|x| x.starts_with("M0")).collect();
    assert_eq!(m0.len(), 2, "{m0:?}");
    assert_eq!(m0[0].as_str(), "M0 ; TEST 1: spindle off, tool up. Press Resume", "the asked-for pause, whole on one line (GRBL reads 80 characters)");
    assert!(m0[1].starts_with("M0 ; Change tool to 6 mm"), "{}", m0[1]);
    // nothing goes into the stock: the lowest cutting move is 7 mm above its top (10 mm of air, 3 mm of depth)
    let lowest = l.iter().filter(|x| x.starts_with("G1")).filter_map(|x| x.split_whitespace().find(|w| w.starts_with('Z')).and_then(|w| w[1..].parse::<f64>().ok())).fold(f64::MAX, f64::min);
    assert!(lowest >= 7.0 - 1e-6, "lowest Z {lowest}");
    assert!(l.iter().all(|x| x.len() <= 80), "lines GRBL can read: {:?}", l.iter().find(|x| x.len() > 80));
    assert!(verify::hazards(&l).is_empty(), "{:?}", verify::hazards(&l));
}

#[test]
fn every_stop_is_followed_by_the_spindle_starting_again_before_any_cut() {
    let l = gcode();
    for (i, x) in l.iter().enumerate().filter(|(_, x)| x.starts_with("M0")) {
        assert_eq!(l[i - 1], "M5", "the spindle is stopped before the stop");
        let m3 = i + l[i..].iter().position(|y| y.starts_with("M3 ")).expect("the spindle starts again");
        assert!(l[i..m3].iter().all(|y| !y.starts_with("G1")), "nothing cuts between {x} and the spindle start");
        assert!(l[m3 + 1].starts_with("G4 P"), "a wait for the spindle after {x}");
    }
}
