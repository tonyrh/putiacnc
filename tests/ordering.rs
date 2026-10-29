//! Path ordering: the same cuts in a shorter order, and nothing changes while the setting is off.
use putiacnc_core::{generate, load_project, Units};
use std::path::PathBuf;

fn load(name: &str) -> putiacnc_core::model::ProjectFile {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(name);
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn lengths(o: &putiacnc_core::Output) -> (f64, f64) {
    let len = |cut: bool| o.moves.iter().filter(|m| m.cut == cut).map(|m| (m.to[0] - m.from[0]).hypot(m.to[1] - m.from[1])).sum::<f64>();
    (len(true), len(false))
}

#[test]
fn ordering_shortens_rapids_and_keeps_the_cuts() {
    let name = "outside_holes.project.json";
    let mut p = load(name);
    p.data.job.order_opt = false;
    let (off, off_lines) = { let o = generate(&p.data, Units::Metric).unwrap(); (lengths(&o), o.lines.len()) };
    p.data.job.order_opt = true;
    let (on, on_lines) = { let o = generate(&p.data, Units::Metric).unwrap(); (lengths(&o), o.lines.len()) };
    assert_eq!(off_lines, on_lines, "{name}: same number of lines");
    assert!((off.0 - on.0).abs() < 0.5, "{name}: cutting length {} vs {}", off.0, on.0);
    assert!(on.1 < off.1, "{name}: rapids {} should be below {}", on.1, off.1);
}

#[test]
fn old_projects_keep_their_order() {
    // the golden files were made without ordering; a project that does not mention the setting must not turn it on
    assert!(!load("two_ops_toolchange.project.json").data.job.order_opt);
}
