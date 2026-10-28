//! Custom feeds and speeds of one operation: they replace the tool's values for that operation only.
use putiacnc_core::model::CustomFeeds;
use putiacnc_core::{generate, load_project, Units};

fn project() -> putiacnc_core::model::ProjectFile {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/outside_holes.project.json");
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn gcode(p: &putiacnc_core::model::ProjectFile) -> (Vec<String>, Vec<String>) {
    let o = generate(&p.data, Units::Metric).unwrap();
    (o.lines, o.notes)
}

#[test]
fn without_custom_values_nothing_changes() {
    let mut p = project();
    let (a, _) = gcode(&p);
    p.data.ops[0].p.custom = Some(CustomFeeds::default());
    let (b, _) = gcode(&p);
    assert_eq!(a, b, "an empty custom block is the tool's own values");
}

#[test]
fn custom_feed_plunge_and_rpm_reach_the_gcode() {
    let mut p = project();
    let t = p.data.tools.iter().find(|t| t.id == p.data.ops[0].p.tool).unwrap().clone();
    p.data.ops[0].p.custom = Some(CustomFeeds { rpm: Some(9000.0), feed: Some(321.0), plunge: Some(123.0), so: None });
    let (g, _) = gcode(&p);
    assert!(g.iter().any(|l| l.starts_with("G1") && l.contains("F321")), "cutting feed");
    assert!(g.iter().any(|l| l.starts_with("G1") && l.contains("F123")), "plunge feed");
    assert!(g.iter().any(|l| l.contains("S9000")), "spindle speed");
    assert!(!g.iter().any(|l| l.starts_with("G1") && l.contains(&format!("F{}", t.feed))), "the tool's feed is not used");
}

#[test]
fn only_the_set_value_changes() {
    let mut p = project();
    let (a, _) = gcode(&p);
    p.data.ops[0].p.custom = Some(CustomFeeds { feed: Some(321.0), ..Default::default() });
    let (b, _) = gcode(&p);
    assert_eq!(a.len(), b.len());
    assert!(a.iter().zip(&b).any(|(x, y)| x != y));
    // plunge and spindle lines are the same as before
    assert!(a.iter().filter(|l| l.contains(" S")).eq(b.iter().filter(|l| l.contains(" S"))));
}

#[test]
fn machine_limits_still_apply_and_are_reported() {
    let mut p = project();
    let mut m = p.data.machine.clone().unwrap_or_default();
    m.feed_max = Some(500.0);
    p.data.machine = Some(m);
    p.data.ops[0].p.custom = Some(CustomFeeds { feed: Some(5000.0), ..Default::default() });
    let (g, notes) = gcode(&p);
    assert!(!g.iter().any(|l| l.contains("F5000")), "held to the machine's maximum");
    assert!(notes.iter().any(|n| n.contains("Custom feeds and speeds held")), "{notes:?}");
}

#[test]
fn old_files_load_and_the_block_round_trips() {
    let mut p = project();
    assert!(p.data.ops.iter().all(|o| o.p.custom.is_none()));
    p.data.ops[0].p.custom = Some(CustomFeeds { feed: Some(321.0), ..Default::default() });
    let json = serde_json::to_string(&p.data).unwrap();
    assert!(json.contains("\"custom\":{\"feed\":321.0}"), "{json}");
    let back: putiacnc_core::model::ProjectData = serde_json::from_str(&json).unwrap();
    assert_eq!(back.ops[0].p.custom, p.data.ops[0].p.custom);
}

#[test]
fn custom_stepover_changes_the_pocket() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/pocket_island.project.json");
    let mut p = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
    let a = generate(&p.data, Units::Metric).unwrap();
    let k = p.data.ops.iter().position(|o| o.p.kind == "pocket").expect("a pocket operation");
    p.data.ops[k].p.custom = Some(CustomFeeds { so: Some(15.0), ..Default::default() });
    let b = generate(&p.data, Units::Metric).unwrap();
    assert!(b.lines.len() > a.lines.len(), "a smaller stepover means more passes: {} vs {}", b.lines.len(), a.lines.len());
}

/// A pocket with its plug: operation 0 is the inlay pocket, 1 the plug; each cuts with a V-bit and a flat end mill.
fn inlay() -> putiacnc_core::model::ProjectData {
    use putiacnc_core::shapes::{add_operation, blank_project, new_rect};
    let mut f = blank_project();
    let d = &mut f.data;
    d.loops.push(new_rect([30.0, 40.0], [50.0, 50.0], 0.0, 0));
    let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
    assert!(add_operation(d, &sel, "inlay").is_none());
    f.data
}

#[test]
fn inlay_tools_have_their_own_custom_values() {
    let mut d = inlay();
    let base = generate(&d, Units::Metric).unwrap().lines;
    // flat end mill of the pocket: only its lines change
    d.ops[0].p.custom2 = Some(CustomFeeds { feed: Some(111.0), ..Default::default() });
    let flat = generate(&d, Units::Metric).unwrap().lines;
    assert!(flat.iter().any(|l| l.starts_with("G1") && l.contains("F111")), "the flat mill's feed");
    assert_eq!(flat.len(), base.len());
    // the V-bit of the pocket: a different value, and the flat mill keeps F111
    d.ops[0].p.custom = Some(CustomFeeds { feed: Some(222.0), ..Default::default() });
    let both = generate(&d, Units::Metric).unwrap().lines;
    assert!(both.iter().any(|l| l.starts_with("G1") && l.contains("F222")), "the V-bit's feed");
    assert!(both.iter().any(|l| l.starts_with("G1") && l.contains("F111")), "the flat mill's feed is still there");
    // without the flat mill's block, the V-bit's value alone leaves the flat mill's lines as they were
    d.ops[0].p.custom2 = None;
    let v_only = generate(&d, Units::Metric).unwrap().lines;
    assert!(!v_only.iter().any(|l| l.contains("F111")));
}

#[test]
fn inlay_limits_name_the_tool() {
    let mut d = inlay();
    let mut m = d.machine.clone().unwrap_or_default();
    m.feed_max = Some(500.0);
    d.machine = Some(m);
    d.ops[0].p.custom2 = Some(CustomFeeds { feed: Some(5000.0), ..Default::default() });
    let notes = generate(&d, Units::Metric).unwrap().notes;
    assert!(notes.iter().any(|n| n.contains("(flat end mill)") && n.contains("held")), "{notes:?}");
}
