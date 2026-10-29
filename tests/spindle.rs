//! Spindle waits, tool changes and the header: what the program says before and between the cuts.
use putiacnc_core::model::{CustomFeeds, ProjectFile};
use putiacnc_core::{generate, load_project, summary, verify, Units};

fn project(name: &str) -> ProjectFile {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/data/{name}.project.json"));
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap()
}
fn with_wait(name: &str, s: Option<f64>) -> putiacnc_core::Output {
    let mut p = project(name);
    let mut m = p.data.machine.clone().unwrap_or_default();
    m.spin_up = s;
    p.data.machine = Some(m);
    generate(&p.data, Units::Metric).unwrap()
}

#[test]
fn the_wait_after_the_spindle_starts_is_the_machines_setting() {
    let two = with_wait("outside_holes", None);
    let i = two.lines.iter().position(|l| l.starts_with("M3 ")).unwrap();
    assert_eq!(two.lines[i + 1], "G4 P2", "2 s when nothing is set");
    let five = with_wait("outside_holes", Some(4.5));
    assert!(five.lines.iter().any(|l| l == "G4 P4.5"));
    assert!(!five.lines.iter().any(|l| l == "G4 P2"));
    let none = with_wait("outside_holes", Some(0.0));
    assert!(!none.lines.iter().any(|l| l.starts_with("G4")), "0 means no wait at all");
    // the estimate counts the wait: 4.5 s against none
    assert!(((five.time_min - none.time_min) * 60.0 - 4.5).abs() < 0.01, "{} {}", five.time_min, none.time_min);
    // and the verifier is happy with every variant
    for o in [&two, &five, &none] { assert!(verify::hazards(&o.lines).is_empty(), "{:?}", verify::hazards(&o.lines)); }
}

#[test]
fn a_tool_change_tells_you_to_touch_off_z_again() {
    let p = project("two_ops_toolchange");
    let o = generate(&p.data, Units::Metric).unwrap();
    let m0 = o.lines.iter().find(|l| l.starts_with("M0 ; Change tool")).expect("a pause for the change");
    assert!(m0.contains("touch off Z0"), "{m0}");
    let rows = summary::describe(&p.data, &o, Units::Metric, false);
    assert!(rows.iter().any(|r| r.label == "At a tool change" && r.warn && r.text.contains("Touch off Z0")));
    // a program with one tool has no such row
    let one = project("outside_holes");
    let o = generate(&one.data, Units::Metric).unwrap();
    assert!(!summary::describe(&one.data, &o, Units::Metric, false).iter().any(|r| r.label == "At a tool change"));
}

#[test]
fn a_speed_change_between_operations_waits_too() {
    let mut p = project("two_ops_toolchange");
    // both operations use the first tool, the second one at another speed
    let first = p.data.ops[0].p.tool.clone();
    p.data.ops[1].p.tool = first;
    p.data.ops[1].p.custom = Some(CustomFeeds { rpm: Some(8000.0), ..Default::default() });
    let o = generate(&p.data, Units::Metric).unwrap();
    let i = o.lines.iter().position(|l| l.starts_with("M3 S8000")).expect("the second speed");
    assert_eq!(o.lines[i + 1], "G4 P2");
    assert!(!o.lines.iter().any(|l| l.starts_with("M0")), "no tool change for the same tool");
    assert!(verify::hazards(&o.lines).is_empty());
}

#[test]
fn the_grbl_header_resets_units_per_minute_and_the_work_coordinate_system() {
    let o = generate(&project("outside_holes").data, Units::Metric).unwrap();
    assert_eq!(o.lines[2], "G21 G90 G17 G94 G54");
    let o = generate(&project("imperial_center").data, Units::Imperial).unwrap();
    assert_eq!(o.lines[2], "G20 G90 G17 G94 G54");
}

#[test]
fn every_program_ends_at_safe_z_over_x0_y0() {
    for name in ["outside_holes", "pocket_island", "vbit_profile", "tabs", "two_ops_toolchange", "vcarve_circle"] {
        let p = project(name);
        let o = generate(&p.data, Units::Metric).unwrap();
        let l = &o.lines;
        let n = l.len();
        assert_eq!((l[n - 3].as_str(), l[n - 2].as_str(), l[n - 1].as_str()), ("M5", "G0 X0 Y0", "M2"), "{name}");
        let safe = l[n - 4].strip_prefix("G0 Z").unwrap_or_else(|| panic!("{name}: lifts before stopping: {}", l[n - 4]));
        let st = &p.data.stock;
        let zt = if st.zm == "top" { 0.0 } else { st.t - st.zh };
        assert!((safe.parse::<f64>().unwrap() - (p.data.job.safe + zt)).abs() < 1e-3, "{name}: safe Z {safe}");
    }
}
