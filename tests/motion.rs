//! The acceleration model in the time estimate: off by default, and when a machine has an acceleration the moves take what the machine needs.
use putiacnc_core::{generate, load_project, replay::replay, Units};
use std::fs;
use std::path::PathBuf;

fn project(name: &str) -> putiacnc_core::ProjectData {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(format!("{name}.project.json"));
    load_project(&fs::read_to_string(p).unwrap()).unwrap().data
}

fn with_accel(mut d: putiacnc_core::ProjectData, accel: Option<f64>, tsc: f64) -> putiacnc_core::ProjectData {
    let mut m = d.machine.clone().unwrap_or_default();
    m.accel = accel; m.tsc = tsc;
    d.machine = Some(m);
    d
}

#[test]
fn without_an_acceleration_the_time_is_length_over_feed() {
    let d = with_accel(project("vcarve_circle"), None, 1.0);
    let out = generate(&d, Units::Metric).unwrap();
    let plain: f64 = out.moves.iter().map(|m| ((m.to[0] - m.from[0]).powi(2) + (m.to[1] - m.from[1]).powi(2) + (m.to[2] - m.from[2]).powi(2)).sqrt() / m.feed).sum();
    let spins = out.time_min - out.moves.iter().map(|m| m.dt).sum::<f64>();
    assert!((out.time_min - spins - plain).abs() < 1e-9);
}

#[test]
fn an_acceleration_makes_a_job_of_short_moves_take_longer() {
    for name in ["vcarve_circle", "pocket_island", "outside_ramp"] {
        let plain = generate(&with_accel(project(name), None, 1.0), Units::Metric).unwrap();
        let slow = generate(&with_accel(project(name), Some(50.0), 1.0), Units::Metric).unwrap();
        let fast = generate(&with_accel(project(name), Some(2000.0), 1.0), Units::Metric).unwrap();
        assert!(slow.time_min > fast.time_min, "{name}: {} {}", slow.time_min, fast.time_min);
        assert!(fast.time_min >= plain.time_min - 1e-9, "{name}: ramps cannot make it faster than the feed allows");
        assert!(slow.time_min > plain.time_min * 1.02, "{name}: 50 mm/s² should cost something");
        // the G-code is the same whatever the model says
        assert_eq!(plain.lines, slow.lines, "{name}");
    }
}

#[test]
fn the_time_per_operation_still_adds_up_to_the_total() {
    let d = with_accel(project("two_ops_toolchange"), Some(200.0), 1.0);
    let out = generate(&d, Units::Metric).unwrap();
    let t = out.timing(d.ops.len());
    let sum: f64 = t.ops.iter().map(|o| o.total()).sum::<f64>() + t.waits;
    assert!((sum - out.time_min).abs() < 1e-9, "{sum} {}", out.time_min);
    assert!((out.cut_time_min - out.moves.iter().filter(|m| m.cut).map(|m| m.dt).sum::<f64>()).abs() < 1e-9);
}

#[test]
fn the_playback_and_the_estimate_agree() {
    let d = with_accel(project("pocket_island"), Some(200.0), 1.0);
    let out = generate(&d, Units::Metric).unwrap();
    let r = replay(&out.lines, &d);
    let (a, b): (f64, f64) = (out.moves.iter().map(|m| m.dt).sum(), r.moves.iter().map(|m| m.dt).sum());
    // the playback cuts arcs into short pieces, so it differs a little
    assert!((a - b).abs() / a < 0.08, "{a} {b}");
}
