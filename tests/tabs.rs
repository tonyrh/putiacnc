use putiacnc_core::{generate, load_project, Units};

fn sample() -> String { std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/outside_holes.project.json")).unwrap() }

#[test]
fn manual_tabs_leave_material_only_where_placed() {
    let mut p = load_project(&sample()).unwrap();
    let op = &mut p.data.ops[0].p;
    op.depth = 3.0;
    op.tabs = true;
    op.tab_w = Some(6.0);
    op.tab_h = Some(2.0);
    let centres = vec![[70.0, 30.0], [110.0, 55.0]];
    op.tab_pts = Some(centres.clone());
    let o = generate(&p.data, Units::Metric).unwrap();
    // moves that run along the outline at tab height (z = -(3 - 2) = -1)
    let on_tab: Vec<_> = o.moves.iter().filter(|m| m.cut && m.pass >= 2 && (m.from[2] + 1.0).abs() < 1e-9 && (m.to[2] + 1.0).abs() < 1e-9 && (m.from[0] != m.to[0] || m.from[1] != m.to[1])).collect();
    assert!(!on_tab.is_empty(), "no tab moves found");
    let reach = (6.0 + 3.175) / 2.0 + 1.7 + 0.6;
    for m in &on_tab {
        let near = centres.iter().any(|c| (m.to[0] - c[0]).hypot(m.to[1] - c[1]) < reach + 2.0);
        assert!(near, "tab move away from any placed tab: {:?}", m.to);
    }
    // no tabs at all in manual mode with an empty list
    p.data.ops[0].p.tab_pts = Some(vec![]);
    let o2 = generate(&p.data, Units::Metric).unwrap();
    assert!(!o2.moves.iter().any(|m| m.cut && m.pass >= 2 && (m.to[2] + 1.0).abs() < 1e-9 && (m.from[2] + 1.0).abs() < 1e-9));
}

#[test]
fn far_away_manual_tabs_are_reported() {
    let mut p = load_project(&sample()).unwrap();
    let op = &mut p.data.ops[0].p;
    op.tabs = true;
    op.tab_pts = Some(vec![[190.0, 140.0]]);
    let o = generate(&p.data, Units::Metric).unwrap();
    assert!(o.notes.iter().any(|n| n.contains("manual tab")), "{:?}", o.notes);
}

#[test]
fn project_file_round_trips_tab_points() {
    let mut p = load_project(&sample()).unwrap();
    p.data.ops[0].p.tab_pts = Some(vec![[1.0, 2.0]]);
    let s = putiacnc_core::save_project(&p).unwrap();
    assert!(s.contains("tabPts"));
    let q = load_project(&s).unwrap();
    assert_eq!(q.data.ops[0].p.tab_pts, Some(vec![[1.0, 2.0]]));
}

#[test]
fn pass_depth_override_roundtrips() {
    let j = r#"{"type":"outside","depth":3,"passD":0.5}"#;
    let p: putiacnc_core::model::OpParams = serde_json::from_str(j).unwrap();
    assert_eq!(p.pass_depth, Some(0.5));
    assert!(serde_json::to_string(&p).unwrap().contains("passD"));
}
