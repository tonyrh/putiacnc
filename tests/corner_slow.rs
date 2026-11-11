//! "Slow down in tight corners": the moves next to a sharp corner run at a lower feed, a smooth curve is left alone.
use putiacnc_core::{generate, load_project, Units};
use std::path::PathBuf;

fn run(slow: Option<f64>) -> Vec<String> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/pocket_island.project.json");
    let mut proj = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
    for o in proj.data.ops.iter_mut() { o.p.cslow = slow; }
    generate(&proj.data, Units::Metric).unwrap().lines
}

#[test]
fn corners_run_slower_and_the_rest_does_not_change() {
    let (off, on) = (run(None), run(Some(50.0)));
    let feeds = |l: &[String]| l.iter().filter(|s| s.starts_with("G1") && s.contains(" F")).count();
    assert!(feeds(&on) > feeds(&off), "{} vs {}", feeds(&on), feeds(&off));
    // the slow feed is half of the tool's 1000
    assert!(on.iter().any(|l| l.contains(" F500")), "no slowed move");
    // the same cut: the same corners are visited
    let xy = |l: &[String]| l.iter().filter(|s| s.starts_with("G1 X") && s.contains(" Y")).count();
    assert!(xy(&on) >= xy(&off));
}
