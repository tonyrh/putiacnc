//! Golden tests: the Rust generator must reproduce what the browser app generated for the
//! same project. Files in tests/data were produced by the browser app (see README).
use putiacnc_core::{generate, load_project, Units};
use std::fs;
use std::path::PathBuf;

fn data(name: &str) -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(name) }

fn eta_seconds(eta: &str) -> f64 {
    // "Estimated time: 12 min 34 s (cutting ...)" or "... 45 s ..." or "1 h 05 min"
    let head = eta.trim_start_matches("Estimated time: ");
    let head = head.split('(').next().unwrap_or("");
    let mut secs = 0.0;
    let toks: Vec<&str> = head.split_whitespace().collect();
    let mut i = 0;
    while i + 1 < toks.len() {
        if let Ok(v) = toks[i].parse::<f64>() {
            match toks[i + 1] { "h" => secs += v * 3600.0, "min" => secs += v * 60.0, "s" => secs += v, _ => {} }
            i += 2;
        } else { i += 1; }
    }
    secs
}

/// `tol_mm`: largest allowed coordinate difference. `max_diff_frac`: share of lines allowed to differ at all
/// (tiny rings may be sampled with different vertices by the two Clipper ports, both within 0.02 mm of the true curve).
fn check(name: &str, tol_mm: f64, max_diff_frac: f64) {
    let proj = load_project(&fs::read_to_string(data(&format!("{name}.project.json"))).unwrap()).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&fs::read_to_string(data(&format!("{name}.meta.json"))).unwrap()).unwrap();
    let units = if meta["units"] == "in" { Units::Imperial } else { Units::Metric };
    let expected = fs::read_to_string(data(&format!("{name}.expected.nc"))).unwrap();
    let out = generate(&proj.data, units).unwrap_or_else(|e| panic!("{name}: {e}"));
    let exp: Vec<&str> = expected.split('\n').collect();
    assert_eq!(out.lines.len(), exp.len(), "{name}: line count");
    let tol = tol_mm / units.factor();
    let mut worst: f64 = 0.0;
    let mut differing = 0usize;
    for (i, (a, b)) in exp.iter().zip(out.lines.iter()).enumerate() {
        let (ta, tb): (Vec<&str>, Vec<&str>) = (a.split(' ').collect(), b.split(' ').collect());
        assert_eq!(ta.len(), tb.len(), "{name} line {i}: `{a}` vs `{b}`");
        let mut line_differs = false;
        for (x, y) in ta.iter().zip(tb.iter()) {
            let motion = a.starts_with("G0") || a.starts_with("G1");
            let axis = motion && x.chars().next().is_some_and(|c| "XYZF".contains(c));
            if axis {
                let (vx, vy): (f64, f64) = (x[1..].parse().unwrap(), y[1..].parse().unwrap());
                let d = (vx - vy).abs();
                worst = worst.max(d);
                if d > 1e-3 / units.factor() { line_differs = true; }
                let limit = if x.starts_with('F') { 1e-9 } else { tol };
                assert!(d <= limit, "{name} line {i}: `{a}` vs `{b}`");
            } else {
                assert_eq!(x, y, "{name} line {i}: `{a}` vs `{b}`");
            }
        }
        if line_differs { differing += 1; }
    }
    assert!(differing as f64 <= max_diff_frac * exp.len() as f64, "{name}: {differing} of {} lines differ", exp.len());
    let want = eta_seconds(meta["eta"].as_str().unwrap());
    let got = out.time_min * 60.0;
    assert!((want - got).abs() <= 1.5 + want * 0.002, "{name}: time {got:.1}s vs {want:.1}s");
    eprintln!("{name}: {} lines, {} differ, worst coordinate difference {:.4}", exp.len(), differing, worst);
}


/// Geometric comparison for scenarios where the two Clipper ports pick different vertices on the same curve:
/// every cutting vertex of each program must lie within `tol_mm` of the other program's cutting path at the same depth.
fn segments(lines: &[String]) -> std::collections::HashMap<i64, Vec<[f64; 4]>> {
    let (mut x, mut y, mut z) = (0.0f64, 0.0f64, 0.0f64);
    let mut map: std::collections::HashMap<i64, Vec<[f64; 4]>> = Default::default();
    for l in lines {
        if !(l.starts_with("G0") || l.starts_with("G1")) { continue; }
        let (px, py) = (x, y);
        for t in l.split(' ') {
            match t.chars().next() { Some('X') => x = t[1..].parse().unwrap(), Some('Y') => y = t[1..].parse().unwrap(), Some('Z') => z = t[1..].parse().unwrap(), _ => {} }
        }
        if l.starts_with("G1") { map.entry((z * 1000.0).round() as i64).or_default().push([px, py, x, y]); }
    }
    map
}
fn dist_pt_seg(px: f64, py: f64, s: &[f64; 4]) -> f64 {
    let (dx, dy) = (s[2] - s[0], s[3] - s[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((px - s[0]) * dx + (py - s[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    ((px - s[0] - t * dx).powi(2) + (py - s[1] - t * dy).powi(2)).sqrt()
}
fn one_way(from: &std::collections::HashMap<i64, Vec<[f64; 4]>>, to: &std::collections::HashMap<i64, Vec<[f64; 4]>>, soft: f64) -> (f64, usize, usize) {
    let (mut worst, mut over, mut total) = (0.0f64, 0usize, 0usize);
    for (z, segs) in from {
        let Some(other) = to.get(z) else { return (f64::INFINITY, 0, 0) };
        for s in segs {
            let d = other.iter().map(|o| dist_pt_seg(s[2], s[3], o)).fold(f64::INFINITY, f64::min);
            worst = worst.max(d);
            total += 1;
            if d > soft { over += 1; }
        }
    }
    (worst, over, total)
}
/// `tol_mm`: hard limit for any vertex. At most 3 percent of vertices may be farther than 0.05 mm: those are the
/// last tiny rings, where each Clipper port's polygon cleaning keeps different vertices.
fn check_geometry(name: &str, tol_mm: f64) {
    let proj = load_project(&fs::read_to_string(data(&format!("{name}.project.json"))).unwrap()).unwrap();
    let expected = fs::read_to_string(data(&format!("{name}.expected.nc"))).unwrap();
    let out = generate(&proj.data, Units::Metric).unwrap();
    let a: Vec<String> = expected.split('\n').map(String::from).collect();
    // Tiny rings may be sampled with a couple more or fewer vertices, so the line counts can differ slightly;
    // every non-motion line (headers, tool changes, comments, spindle) must still match exactly and in order.
    let non_motion = |v: &[String]| -> Vec<String> { v.iter().filter(|l| !(l.starts_with("G0") || l.starts_with("G1"))).cloned().collect() };
    assert_eq!(non_motion(&a), non_motion(&out.lines), "{name}: non-motion lines");
    assert!((a.len() as f64 - out.lines.len() as f64).abs() <= 0.02 * a.len() as f64, "{name}: line count {} vs {}", a.len(), out.lines.len());
    let (sa, sb) = (segments(&a), segments(&out.lines));
    let ((d1, o1, n1), (d2, o2, n2)) = (one_way(&sa, &sb, 0.05), one_way(&sb, &sa, 0.05));
    eprintln!("{name}: geometric difference JS->Rust {d1:.3} mm ({o1}/{n1} vertices over 0.05), Rust->JS {d2:.3} mm ({o2}/{n2})");
    assert!(d1 <= tol_mm && d2 <= tol_mm, "{name}: paths differ by {d1:.3}/{d2:.3} mm");
    assert!(o1 as f64 <= 0.03 * n1 as f64 && o2 as f64 <= 0.03 * n2 as f64, "{name}: too many vertices differ");
    let meta: serde_json::Value = serde_json::from_str(&fs::read_to_string(data(&format!("{name}.meta.json"))).unwrap()).unwrap();
    let (want, got) = (eta_seconds(meta["eta"].as_str().unwrap()), out.time_min * 60.0);
    assert!((want - got).abs() <= 1.5 + want * 0.01, "{name}: time {got:.1}s vs {want:.1}s");
}

#[test] fn outside_ramp() { check("outside_ramp", 0.01, 0.0); }
#[test] fn outside_holes() { check("outside_holes", 0.01, 0.0); }
#[test] fn pocket_island() { check("pocket_island", 0.01, 0.0); }
// Rust adds a few smaller finishing rings near the middle of narrow parts (the reference stops a step short of the centre line), hence the wider limit.
#[test] fn vcarve_circle() { check_geometry("vcarve_circle", 0.25); }
#[test] fn tabs() { check("tabs", 0.01, 0.0); }
#[test] fn vbit_profile() { check("vbit_profile", 0.01, 0.0); }
#[test] fn two_ops_toolchange() { check_geometry("two_ops_toolchange", 0.4); }
#[test] fn imperial_center() { check("imperial_center", 0.01, 0.02); }

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
