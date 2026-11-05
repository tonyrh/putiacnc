//! The work coordinate system word in the header: G54 unless the job says another.
use putiacnc_core::{generate, load_project, Units};
use std::fs;
use std::path::PathBuf;

fn project() -> putiacnc_core::ProjectData {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join("outside_ramp.project.json");
    load_project(&fs::read_to_string(p).unwrap()).unwrap().data
}

fn words(g: &str) -> Vec<String> {
    g.split(|c: char| c.is_whitespace()).filter(|w| w.starts_with("G5")).map(String::from).collect()
}

#[test]
fn the_default_is_g54_and_any_choice_appears_once() {
    let mut d = project();
    assert_eq!(words(&generate(&d, Units::Metric).unwrap().gcode()), ["G54"]);
    for (i, name) in ["G54", "G55", "G56", "G57", "G58", "G59"].iter().enumerate() {
        d.job.wcs = i as u8;
        let g = generate(&d, Units::Metric).unwrap().gcode();
        assert_eq!(words(&g), [name.to_string()], "{name}");
    }
}

#[test]
fn grbl_has_six_systems_and_mach_nine() {
    let mut d = project();
    d.job.wcs = 7; // G59.2: not for GRBL, so it falls back to G54 rather than writing a word the controller rejects
    assert_eq!(words(&generate(&d, Units::Metric).unwrap().gcode()), ["G54"]);
    d.job.post = "mach".into();
    assert_eq!(words(&generate(&d, Units::Metric).unwrap().gcode()), ["G59.2"]);
}

#[test]
fn the_choice_is_saved_with_the_project_and_the_default_is_not() {
    let mut d = project();
    assert!(!serde_json::to_string(&d.job).unwrap().contains("wcs"));
    d.job.wcs = 2;
    let back: putiacnc_core::model::Job = serde_json::from_str(&serde_json::to_string(&d.job).unwrap()).unwrap();
    assert_eq!(back.wcs, 2);
}
