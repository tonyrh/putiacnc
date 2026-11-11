//! Pocket rings that start beside the ring just cut are reached at depth, without lifting the tool and plunging again.
use putiacnc_core::{generate, load_project, Units};
use std::path::PathBuf;

#[test]
fn neighbouring_pocket_rings_are_linked_at_depth() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/pocket_island.project.json");
    let proj = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
    let out = generate(&proj.data, Units::Metric).unwrap();
    let entries = out.lines.iter().filter(|l| l.starts_with("G0 Z0.5")).count();
    // 24 before the rings were linked
    assert!(entries <= 18, "{entries} plunges");
}
