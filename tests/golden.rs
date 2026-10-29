//! Golden tests: the generator's output for ten sample projects is compared with snapshots kept in tests/data.
//!
//! `NAME.project.json` is the input, `NAME.snapshot.nc` the reviewed G-code, `NAME.snapshot.json` holds the units and
//! the time estimate. When the generator changes on purpose, run
//! `UPDATE_SNAPSHOTS=1 cargo test --test golden`, look at `git diff tests/data` and commit what you accept.
//! Coordinates are compared with a tolerance (not byte for byte) because trig functions differ in the last digits
//! between platforms; everything else (line count, words, feeds) must match exactly.
use putiacnc_core::{generate, load_project, Units};
use std::fs;
use std::path::PathBuf;

fn data(name: &str) -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(name) }

fn check(name: &str, units: Units) {
    let proj = load_project(&fs::read_to_string(data(&format!("{name}.project.json"))).unwrap()).unwrap();
    let out = generate(&proj.data, units).unwrap_or_else(|e| panic!("{name}: {e}"));
    let nc = data(&format!("{name}.snapshot.nc"));
    let meta = data(&format!("{name}.snapshot.json"));
    let got = out.lines.join("\n");
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        fs::write(&nc, &got).unwrap();
        let u = if units == Units::Imperial { "in" } else { "mm" };
        fs::write(&meta, format!("{{\"units\":\"{u}\",\"time_s\":{:.1}}}\n", out.time_min * 60.0)).unwrap();
        return;
    }
    let want = fs::read_to_string(&nc).unwrap_or_else(|_| panic!("{name}: no snapshot; run with UPDATE_SNAPSHOTS=1"));
    let exp: Vec<&str> = want.split('\n').collect();
    assert_eq!(out.lines.len(), exp.len(), "{name}: line count");
    // 0.0011 mm: coordinates are printed with 3 decimals (4 in inches), so a last-digit rounding flip is the largest legitimate difference
    let tol = 0.0011 / units.factor();
    for (i, (a, b)) in exp.iter().zip(out.lines.iter()).enumerate() {
        let (ta, tb): (Vec<&str>, Vec<&str>) = (a.split(' ').collect(), b.split(' ').collect());
        assert_eq!(ta.len(), tb.len(), "{name} line {i}: `{a}` vs `{b}`");
        let motion = a.starts_with("G0") || a.starts_with("G1");
        for (x, y) in ta.iter().zip(tb.iter()) {
            if motion && x.chars().next().is_some_and(|c| "XYZ".contains(c)) {
                let (vx, vy): (f64, f64) = (x[1..].parse().unwrap(), y[1..].parse().unwrap());
                assert!((vx - vy).abs() <= tol, "{name} line {i}: `{a}` vs `{b}`");
            } else {
                assert_eq!(x, y, "{name} line {i}: `{a}` vs `{b}`");
            }
        }
    }
    let m: serde_json::Value = serde_json::from_str(&fs::read_to_string(&meta).unwrap()).unwrap();
    let want_s = m["time_s"].as_f64().unwrap();
    let got_s = out.time_min * 60.0;
    assert!((want_s - got_s).abs() <= 0.5 + want_s * 0.001, "{name}: time {got_s:.1}s vs {want_s:.1}s");
}

#[test] fn outside_ramp() { check("outside_ramp", Units::Metric); }
#[test] fn outside_holes() { check("outside_holes", Units::Metric); }
#[test] fn pocket_island() { check("pocket_island", Units::Metric); }
#[test] fn vcarve_circle() { check("vcarve_circle", Units::Metric); }
#[test] fn tabs() { check("tabs", Units::Metric); }
#[test] fn vbit_profile() { check("vbit_profile", Units::Metric); }
#[test] fn two_ops_toolchange() { check("two_ops_toolchange", Units::Metric); }
#[test] fn imperial_center() { check("imperial_center", Units::Imperial); }

#[test]
fn rejects_other_files() {
    assert!(load_project("{\"app\":\"something-else\",\"data\":{\"stock\":{\"w\":1,\"h\":1,\"t\":1}}}").is_err());
    assert!(load_project("not json").is_err());
}

#[test]
fn vcarve_with_flat_tool_is_an_error() {
    let mut proj = load_project(&fs::read_to_string(data("vcarve_circle.project.json")).unwrap()).unwrap();
    for t in proj.data.tools.iter_mut() { t.shape = "flat".into(); }
    assert!(generate(&proj.data, Units::Metric).unwrap_err().contains("V-carve needs"));
}

/// The flat lap that levels the floor of a ramped profile is flagged, shares the last pass number, and only the last pass has one.
#[test] fn ramp_levelling_lap_is_flagged() {
    let proj = load_project(&fs::read_to_string(data("outside_ramp.project.json")).unwrap()).unwrap();
    let out = generate(&proj.data, Units::Metric).unwrap();
    let last = out.moves.iter().map(|m| m.pass).max().unwrap();
    let lap: Vec<_> = out.moves.iter().filter(|m| m.lap).collect();
    assert!(!lap.is_empty(), "a ramped profile has a levelling lap");
    assert!(lap.iter().all(|m| m.pass == last && m.cut && m.from[2] == m.to[2]), "the lap is flat and belongs to the last pass");
    assert!(out.moves.iter().filter(|m| m.pass < last).all(|m| !m.lap));
}

/// The tool starts at the XY zero, wherever it is: a centre zero must not start the first rapid from the stock's corner.
#[test] fn the_first_rapid_starts_from_the_xy_zero() {
    let mut proj = load_project(&fs::read_to_string(data("outside_ramp.project.json")).unwrap()).unwrap();
    proj.data.stock.zr = "c".into();
    let (w, h) = (proj.data.stock.w, proj.data.stock.h);
    let out = generate(&proj.data, Units::Metric).unwrap();
    let first = out.moves.iter().find(|m| m.from[0] != m.to[0] || m.from[1] != m.to[1]).unwrap();
    assert!((first.from[0] - w / 2.0).abs() < 1e-9 && (first.from[1] - h / 2.0).abs() < 1e-9, "{:?}", first.from);
    proj.data.stock.zr = "ll".into();
    let out = generate(&proj.data, Units::Metric).unwrap();
    let first = out.moves.iter().find(|m| m.from[0] != m.to[0] || m.from[1] != m.to[1]).unwrap();
    assert_eq!([first.from[0], first.from[1]], [0.0, 0.0]);
}
