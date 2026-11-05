//! Layers: a layer organizes the drawing; hiding one does not take its shapes out of the G-code.
use putiacnc_core::model::*;
use putiacnc_core::shapes;

fn project() -> putiacnc_core::model::ProjectData {
    let mut d = shapes::blank_project().data;
    d.loops = vec![shapes::new_rect([10.0, 10.0], [30.0, 30.0], 0.0, 0), shapes::new_rect([50.0, 10.0], [70.0, 30.0], 0.0, 0)];
    d
}

#[test]
fn the_first_layer_always_exists_and_old_files_have_no_layers() {
    let d = project();
    assert!(d.layers.is_empty());
    let all = d.all_layers();
    assert_eq!(all.len(), 1);
    assert_eq!((all[0].id, all[0].name.as_str()), (0, "Layer 1"));
    assert!(!d.shape_hidden(&d.loops[0]));
    let json = serde_json::to_string(&d).unwrap();
    assert!(!json.contains("layers") && !json.contains("active_layer") && !json.contains("\"lay\""), "nothing is saved until layers are used");
}

#[test]
fn hiding_and_locking_follow_the_layer() {
    let mut d = project();
    let b = d.add_layer("Box");
    d.loops[1].lay = Some(b);
    d.layer_mut(b).unwrap().hidden = true;
    assert!(!d.shape_hidden(&d.loops[0]) && d.shape_hidden(&d.loops[1]));
    d.layer_mut(b).unwrap().hidden = false;
    d.layer_mut(0).unwrap().locked = true;
    assert!(d.shape_locked(&d.loops[0]) && !d.shape_locked(&d.loops[1]));
    // a shape on a layer that no longer exists counts as the first layer
    d.loops[1].lay = Some(99);
    assert_eq!(d.layer_of(&d.loops[1]), 0);
}

#[test]
fn layers_survive_a_save_and_a_load() {
    let mut d = project();
    let b = d.add_layer("Box");
    d.layer_mut(b).unwrap().color = hex_rgb([1, 2, 3]);
    d.layer_mut(b).unwrap().hidden = true;
    d.loops[1].lay = Some(b);
    d.active_layer = b;
    let back: ProjectData = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
    assert_eq!(back.layers, d.layers);
    assert_eq!(back.active_layer, b);
    assert_eq!(back.loops[1].lay, Some(b));
    assert_eq!(back.layers[1].rgb(), Some([1, 2, 3]));
}

#[test]
fn picking_skips_the_hidden() {
    let mut d = project();
    let b = d.add_layer("Box");
    d.loops[1].lay = Some(b);
    assert_eq!(shapes::pick(&d.loops, [50.0, 20.0], 1.0), Some(1));
    d.layer_mut(b).unwrap().hidden = true;
    let hit = shapes::pick_where(&d.loops, [50.0, 20.0], 1.0, |l| !d.shape_hidden(l));
    assert_eq!(hit, None);
}

#[test]
fn a_hidden_layer_is_still_cut() {
    use putiacnc_core::{generate, load_project, Units};
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join("outside_ramp.project.json");
    let mut d = load_project(&std::fs::read_to_string(p).unwrap()).unwrap().data;
    let before = generate(&d, Units::Metric).unwrap().gcode();
    let b = d.add_layer("Box");
    for l in &mut d.loops { l.lay = Some(b); }
    d.layer_mut(b).unwrap().hidden = true;
    let after = generate(&d, Units::Metric).unwrap();
    assert_eq!(after.gcode(), before, "the eye changes nothing in the program");
    let rows = putiacnc_core::summary::describe(&d, &after, Units::Metric, false);
    assert!(rows.iter().any(|r| r.label == "Hidden layers" && r.warn), "the summary says it");
}
