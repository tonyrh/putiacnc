//! Feeds and speeds by material: what the Suggest button does, end to end.
use putiacnc_core::feeds::{self, Rigidity};
use putiacnc_core::model::{CustomFeeds, ProjectFile};
use putiacnc_core::{generate, load_project, Units};

fn project() -> ProjectFile {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/outside_holes.project.json");
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn a_suggestion_put_into_an_operation_is_what_the_gcode_runs() {
    let mut f = project();
    f.data.stock.cut = "hardwood".into();
    let tool = f.data.tools.iter().find(|t| t.id == f.data.ops[0].p.tool).unwrap().clone();
    let s = feeds::suggest(&f.data.stock.cut, &tool, Rigidity::Light, None).unwrap();
    f.data.ops[0].p.custom = Some(CustomFeeds { rpm: Some(s.rpm), feed: Some(s.feed), plunge: Some(s.plunge), so: Some(s.so) });
    f.data.ops[0].p.pass_depth = s.pass;
    let out = generate(&f.data, Units::Metric).unwrap();
    let text = out.lines.join("\n");
    assert!(text.contains(&format!("S{}", s.rpm)), "spindle {}: {}", s.rpm, text.lines().find(|l| l.contains("M3")).unwrap_or(""));
    assert!(text.contains(&format!("F{}", s.feed)), "feed {}", s.feed);
    assert!(text.contains(&format!("F{}", s.plunge)), "plunge {}", s.plunge);
    // hardwood takes lighter cuts than the tool's own values for soft wood
    let own = generate(&project().data, Units::Metric).unwrap().lines.join("\n");
    assert_ne!(text, own);
}

#[test]
fn material_and_rigidity_are_saved_and_old_projects_have_none() {
    let mut f = project();
    assert!(f.data.stock.cut.is_empty(), "an old project has no material");
    let before = serde_json::to_string(&f).unwrap();
    assert!(!before.contains("\"cut\":\"") && !before.contains("rigid"), "nothing saved while unset");
    f.data.stock.cut = "plywood".into();
    if let Some(m) = f.data.machine.as_mut() { m.rigid = "medium".into(); }
    let back = load_project(&serde_json::to_string(&f).unwrap()).unwrap();
    assert_eq!(back.data.stock.cut, "plywood");
    if f.data.machine.is_some() { assert_eq!(back.data.machine.as_ref().unwrap().rigid, "medium"); }
    assert_eq!(Rigidity::parse(&back.data.machine.map(|m| m.rigid).unwrap_or_default()), if f.data.machine.is_some() { Rigidity::Medium } else { Rigidity::Light });
    // the material does not change what a project makes until Suggest is used
    let mut g = project();
    g.data.stock.cut = "aluminium".into();
    assert_eq!(generate(&g.data, Units::Metric).unwrap().lines, generate(&project().data, Units::Metric).unwrap().lines);
}

#[test]
fn every_bundled_cutter_gets_a_suggestion_in_every_material() {
    let mut libs = putiacnc_core::toollib::ToolLibs::default();
    for (_, json) in putiacnc_core::toollib::BUNDLED.iter() { let _ = libs.import(json); }
    let mut n = 0;
    for l in &libs.libs {
        for t in l.tools.iter().filter(|t| t.shape != "pen") {
            for m in feeds::MATERIALS.iter() {
                for r in [Rigidity::Light, Rigidity::Heavy] {
                    let s = feeds::suggest(m.id, t, r, None).unwrap_or_else(|| panic!("{} in {}", t.name, m.id));
                    assert!(s.feed.is_finite() && s.feed > 0.0 && s.rpm > 0.0 && s.plunge > 0.0, "{} in {}: {s:?}", t.name, m.id);
                    n += 1;
                }
            }
        }
    }
    assert!(n > 500, "{n} suggestions checked");
}
