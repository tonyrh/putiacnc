//! The time estimate split by operation adds up to the estimate.
use putiacnc_core::{generate, load_project, Units};

fn sample(name: &str) -> putiacnc_core::model::ProjectData {
    load_project(&std::fs::read_to_string(format!("{}/tests/data/{name}.project.json", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap().data
}

#[test]
fn the_parts_add_up_to_the_estimate() {
    for name in ["outside_holes", "two_ops_toolchange", "pocket_island", "tabs", "vbit_profile", "vcarve_circle", "outside_ramp", "imperial_center"] {
        let d = sample(name);
        let o = generate(&d, Units::Metric).unwrap();
        let t = o.timing(d.ops.len());
        let sum: f64 = t.ops.iter().map(|x| x.total()).sum::<f64>() + t.waits;
        assert!((sum - o.time_min).abs() < 1e-9, "{name}: {sum} against {}", o.time_min);
        let cut: f64 = t.ops.iter().map(|x| x.cut).sum();
        assert!((cut - o.cut_time_min).abs() < 1e-9, "{name}: cutting {cut} against {}", o.cut_time_min);
        assert!(t.ops.len() >= d.ops.len() && t.ops.iter().all(|x| x.cut >= 0.0 && x.travel >= 0.0) && t.waits >= 0.0, "{name}");
    }
}

#[test]
fn each_operation_gets_its_own_share_and_the_spindle_wait_is_what_is_left() {
    let mut d = sample("two_ops_toolchange");
    assert!(d.ops.len() >= 2);
    let o = generate(&d, Units::Metric).unwrap();
    let t = o.timing(d.ops.len());
    assert!(t.ops[0].cut > 0.0 && t.ops[1].cut > 0.0, "both operations cut: {:?}", t.ops);
    assert!(t.waits > 0.0, "the default machine waits for the spindle");
    // no wait: nothing is left over
    d.machine.get_or_insert_with(Default::default).spin_up = Some(0.0);
    let o = generate(&d, Units::Metric).unwrap();
    assert!(o.timing(d.ops.len()).waits < 1e-9);
    // two seconds for each time the spindle comes up
    d.machine.as_mut().unwrap().spin_up = Some(2.0);
    let o = generate(&d, Units::Metric).unwrap();
    let starts = o.lines.iter().filter(|l| l.starts_with("G4 P")).count() as f64;
    assert!((o.timing(d.ops.len()).waits - starts * 2.0 / 60.0).abs() < 1e-9, "{starts} waits");
}

#[test]
fn a_list_of_no_moves_is_empty_and_finite() {
    let o = putiacnc_core::toolpath::Output { lines: vec![], moves: vec![], time_min: 0.5, cut_time_min: 0.0, cut_len_mm: 0.0, notes: vec![], tab_marks: vec![] };
    let t = o.timing(3);
    assert_eq!(t.ops.len(), 3);
    assert!((t.waits - 0.5).abs() < 1e-12);
}
