use putiacnc_core::{generate, load_project, Units};

fn sample() -> String { std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/outside_holes.project.json")).unwrap() }

fn fs(g: &str) -> Vec<f64> {
    g.lines().flat_map(|l| l.split_whitespace().filter(|w| w.starts_with('F')).filter_map(|w| w[1..].parse::<f64>().ok()).collect::<Vec<_>>()).collect()
}
fn speeds(g: &str) -> Vec<f64> {
    g.lines().flat_map(|l| l.split_whitespace().filter(|w| w.starts_with('S') && w.len() > 1 && w[1..].chars().all(|c| c.is_ascii_digit())).filter_map(|w| w[1..].parse::<f64>().ok()).collect::<Vec<_>>()).collect()
}

#[test]
fn machine_limits_hold_the_tool_to_them() {
    let mut p = load_project(&sample()).unwrap();
    let free = generate(&p.data, Units::Metric).unwrap();
    assert!(!free.notes.iter().any(|n| n.contains("machine's limits")));
    let (f0, s0) = (fs(&free.gcode()), speeds(&free.gcode()));
    let fmax = f0.iter().cloned().fold(0.0, f64::max);
    let mut m = p.data.machine.clone().unwrap_or_default();
    m.feed_max = Some(fmax / 2.0);
    m.plunge_max = Some(100.0);
    m.rpm_max = Some(s0[0] / 2.0);
    p.data.machine = Some(m);
    let held = generate(&p.data, Units::Metric).unwrap();
    let g = held.gcode();
    assert!(fs(&g).iter().all(|&f| f <= fmax / 2.0 + 1e-9 && f <= fmax));
    assert!(fs(&g).iter().any(|&f| (f - 100.0).abs() < 1e-9), "plunge held to 100");
    assert!(speeds(&g).iter().all(|&s| s <= s0[0] / 2.0 + 1e-9));
    assert!(held.notes.iter().any(|n| n.contains("machine's limits")));
    // a spindle minimum lifts a slow tool
    let mut m = p.data.machine.clone().unwrap();
    m.feed_max = None; m.plunge_max = None; m.rpm_max = None; m.rpm_min = Some(s0[0] * 2.0);
    p.data.machine = Some(m);
    assert!(speeds(&generate(&p.data, Units::Metric).unwrap().gcode()).iter().all(|&s| s >= s0[0] * 2.0 - 1e-9));
}
