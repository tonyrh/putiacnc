//! The roughing of a 3D relief must not drop the cutter in a rapid move into stock that is still there.
//! `data/relief-ledges.project.json` is a portrait with fine high detail: its first roughing layers leave ledges of uncut
//! stock beside the high parts, and a later layer starts next to one (the old code dropped to 0.3 mm above the previous
//! layer's floor there, so the edge of the 6 mm cutter went 1 mm into a ledge).
use putiacnc_core::{checks, generate, load_project, Units};

#[test]
fn roughing_does_not_rapid_into_the_ledges_it_leaves() {
    let p = load_project(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/relief-ledges.project.json")).unwrap()).unwrap();
    let out = generate(&p.data, Units::Metric).unwrap();
    let found: Vec<String> = checks::run(&p.data, &out).into_iter().map(|f| format!("{:?}: {}", f.kind, f.msg)).collect();
    assert!(found.is_empty(), "{found:?}");
}
