use putiacnc_core::model::TabPt;
use putiacnc_core::shapes;
use putiacnc_core::{generate, load_project, Units};

fn sample() -> String { std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/outside_holes.project.json")).unwrap() }

/// Anchors a tab at `p` on the shape nearest to it.
fn anchor(p: &putiacnc_core::ProjectFile, pt: [f64; 2]) -> TabPt {
    let li = (0..p.data.loops.len())
        .min_by(|&a, &b| shapes::dist_to_loop(&p.data.loops[a], pt).partial_cmp(&shapes::dist_to_loop(&p.data.loops[b], pt)).unwrap()).unwrap();
    shapes::tab_anchor(&p.data.loops, li, pt).unwrap()
}

#[test]
fn manual_tabs_leave_material_only_where_placed() {
    let mut p = load_project(&sample()).unwrap();
    {
        let op = &mut p.data.ops[0].p;
        op.depth = 3.0;
        op.tabs = true;
        op.tab_w = Some(6.0);
        op.tab_h = Some(2.0);
    }
    let tabs = vec![anchor(&p, [70.0, 30.0]), anchor(&p, [110.0, 55.0])];
    p.data.ops[0].p.tab_pts = Some(tabs.clone());
    let id = p.data.ops[0].id;
    let centres: Vec<[f64; 2]> = tabs.iter().map(|t| shapes::tab_resolve(&p.data.loops, id, t).unwrap().0).collect();
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
fn tabs_on_missing_or_foreign_shapes_are_ignored() {
    let mut p = load_project(&sample()).unwrap();
    let id = p.data.ops[0].id;
    p.data.ops[0].p.tabs = true;
    p.data.ops[0].p.tab_pts = Some(vec![TabPt { l: 99, u: 0.5, v: 0.5 }]);
    assert!(shapes::tab_resolve(&p.data.loops, id, &TabPt { l: 99, u: 0.5, v: 0.5 }).is_none());
    assert!(generate(&p.data, Units::Metric).is_ok());
    // a shape that belongs to no operation does not resolve either
    p.data.loops[0].op = 0;
    assert!(shapes::tab_resolve(&p.data.loops, id, &TabPt { l: 0, u: 0.5, v: 0.0 }).is_none());
}

#[test]
fn tabs_follow_a_moved_or_resized_shape() {
    let mut p = load_project(&sample()).unwrap();
    let id = p.data.ops[0].id;
    let t = shapes::tab_anchor(&p.data.loops, 0, [50.0, 20.0]).unwrap(); // bottom edge of the rectangle
    let (before, _) = shapes::tab_resolve(&p.data.loops, id, &t).unwrap();
    shapes::translate(&mut p.data.loops[0], 12.0, -7.0);
    let (moved, _) = shapes::tab_resolve(&p.data.loops, id, &t).unwrap();
    assert!((moved[0] - before[0] - 12.0).abs() < 0.01 && (moved[1] - before[1] + 7.0).abs() < 0.01, "{before:?} -> {moved:?}");
    // stretch to twice the width about the left edge: the tab stays at the same relative spot on the bottom edge
    let b = shapes::bbox(&p.data.loops[0]).unwrap();
    shapes::scale_about(&mut p.data.loops[0], b[0], b[1], 2.0, 1.0);
    let (wide, _) = shapes::tab_resolve(&p.data.loops, id, &t).unwrap();
    assert!((wide[0] - (b[0] + (moved[0] - b[0]) * 2.0)).abs() < 0.05, "{moved:?} -> {wide:?}");
    assert!((wide[1] - moved[1]).abs() < 0.05);
}

#[test]
fn deleting_and_duplicating_shapes_keeps_tab_anchors_right() {
    let mut p = load_project(&sample()).unwrap();
    p.data.ops[0].p.tab_pts = Some(vec![TabPt { l: 0, u: 0.5, v: 0.0 }, TabPt { l: 1, u: 0.0, v: 0.5 }, TabPt { l: 2, u: 1.0, v: 0.5 }]);
    shapes::tabs_after_delete(&mut p.data.ops, &[1]);
    let v = p.data.ops[0].p.tab_pts.clone().unwrap();
    assert_eq!(v.iter().map(|t| t.l).collect::<Vec<_>>(), vec![0, 1]); // shape 1's tab gone, shape 2 is now 1
    shapes::tabs_copy(&mut p.data.ops, 0, 5);
    let v = p.data.ops[0].p.tab_pts.clone().unwrap();
    assert_eq!(v.len(), 3);
    assert_eq!(v[2], TabPt { l: 5, u: 0.5, v: 0.0 });
}

#[test]
fn project_file_round_trips_tab_points() {
    let mut p = load_project(&sample()).unwrap();
    p.data.ops[0].p.tab_pts = Some(vec![TabPt { l: 1, u: 0.25, v: 0.75 }]);
    let s = putiacnc_core::save_project(&p).unwrap();
    assert!(s.contains("tabPts"));
    let q = load_project(&s).unwrap();
    assert_eq!(q.data.ops[0].p.tab_pts, Some(vec![TabPt { l: 1, u: 0.25, v: 0.75 }]));
}

#[test]
fn pass_depth_override_roundtrips() {
    let j = r#"{"type":"outside","depth":3,"passD":0.5}"#;
    let p: putiacnc_core::model::OpParams = serde_json::from_str(j).unwrap();
    assert_eq!(p.pass_depth, Some(0.5));
    assert!(serde_json::to_string(&p).unwrap().contains("passD"));
}

#[test]
fn automatic_tab_marks_match_the_uncut_parts_of_the_toolpath() {
    let mut p = load_project(&sample()).unwrap();
    {
        let op = &mut p.data.ops[0].p;
        op.depth = 3.0;
        op.tabs = true;
        op.tab_n = Some(4.0);
        op.tab_w = Some(6.0);
        op.tab_h = Some(2.0);
        op.tab_pts = None;
    }
    let o = generate(&p.data, Units::Metric).unwrap();
    assert!(!o.tab_marks.is_empty());
    assert!(o.tab_marks.iter().all(|m| m.op == 0));
    // every cut move at tab height (z = -1) lies near one of the marks, and every mark has such moves near it
    let on_tab: Vec<_> = o.moves.iter().filter(|m| m.cut && m.pass >= 2 && (m.from[2] + 1.0).abs() < 1e-9 && (m.to[2] + 1.0).abs() < 1e-9 && (m.from[0] != m.to[0] || m.from[1] != m.to[1])).collect();
    assert!(!on_tab.is_empty());
    let reach = (6.0 + 3.175) / 2.0 + 0.5;
    for m in &on_tab {
        assert!(o.tab_marks.iter().any(|k| (m.to[0] - k.mid[0]).hypot(m.to[1] - k.mid[1]) < reach), "tab move away from every mark: {:?}", m.to);
    }
    for k in &o.tab_marks {
        assert!(on_tab.iter().any(|m| (m.to[0] - k.mid[0]).hypot(m.to[1] - k.mid[1]) < reach), "mark without tab moves: {:?}", k.mid);
    }
}
