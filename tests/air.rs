use putiacnc_core::{generate, load_project, Units};

fn sample() -> String { std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/outside_holes.project.json")).unwrap() }

fn max_z(g: &str) -> f64 {
    g.lines().filter_map(|l| l.split_whitespace().find(|w| w.starts_with('Z')).and_then(|w| w[1..].parse::<f64>().ok())).fold(f64::MIN, f64::max)
}

#[test]
fn air_cut_is_per_operation() {
    let mut p = load_project(&sample()).unwrap();
    let normal = generate(&p.data, Units::Metric).unwrap().gcode();
    p.data.ops[0].p.air = true;
    let air = generate(&p.data, Units::Metric).unwrap().gcode();
    assert!((max_z(&air) - max_z(&normal) - 10.0).abs() < 1e-6);
    assert!(!air.contains("Z-1") || air.contains("Z9"));
}

#[test]
fn legacy_job_air_moves_to_operations() {
    let s = sample().replace("\"air\":false", "\"air\":true");
    let p = load_project(&s).unwrap();
    assert!(!p.data.job.air);
    assert!(p.data.ops.iter().all(|o| o.p.air));
}
