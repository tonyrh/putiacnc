//! "Ramp in" for pockets: opt-in per project, the pocket reaches the same depth, and old projects are unchanged.
use putiacnc_core::{generate, load_project, Units};

fn pocket() -> putiacnc_core::model::ProjectFile {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/pocket_island.project.json");
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn lines(p: &putiacnc_core::model::ProjectFile) -> Vec<String> { generate(&p.data, Units::Metric).unwrap().lines }

fn min_z(l: &[String]) -> f64 {
    l.iter().filter_map(|x| x.split(' ').find(|t| t.starts_with('Z')).and_then(|t| t[1..].parse::<f64>().ok())).fold(f64::MAX, f64::min)
}

#[test]
fn old_projects_do_not_ramp_pockets() {
    let mut p = pocket();
    assert!(!p.data.job.ramp_pockets, "the setting is off unless the project has it");
    let k = p.data.ops.iter().position(|o| o.p.kind == "pocket").unwrap();
    p.data.ops[k].p.ramp = true;
    let a = lines(&p);
    p.data.ops[k].p.ramp = false;
    assert_eq!(a, lines(&p), "with the setting off, Ramp in does nothing for a pocket, as before");
}

#[test]
fn a_ramped_pocket_reaches_the_same_depth_with_fewer_straight_plunges() {
    let mut p = pocket();
    let k = p.data.ops.iter().position(|o| o.p.kind == "pocket").unwrap();
    p.data.ops[k].p.ramp = true;
    let off = lines(&p);
    p.data.job.ramp_pockets = true;
    let on = lines(&p);
    assert_ne!(off, on);
    assert!((min_z(&off) - min_z(&on)).abs() < 1e-6, "same depth: {} vs {}", min_z(&off), min_z(&on));
    // Ramp in unticked: the setting alone changes nothing
    p.data.ops[k].p.ramp = false;
    assert_eq!(off, lines(&p));
}

#[test]
fn the_inlay_pocket_ramps_its_flat_mill_when_the_project_allows_it() {
    use putiacnc_core::shapes::{add_operation, blank_project, new_rect};
    let mut f = blank_project();
    let d = &mut f.data;
    d.loops.push(new_rect([30.0, 40.0], [50.0, 50.0], 0.0, 0));
    let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
    assert!(add_operation(d, &sel, "inlay").is_none());
    let k = d.ops.iter().position(|o| o.p.kind == "inlay").unwrap();
    d.ops[k].p.ramp = true;
    d.job.ramp_pockets = false;
    let off = generate(d, Units::Metric).unwrap().lines;
    d.ops[k].p.ramp = false;
    assert_eq!(off, generate(d, Units::Metric).unwrap().lines, "setting off: no change");
    d.ops[k].p.ramp = true;
    d.job.ramp_pockets = true;
    let on = generate(d, Units::Metric).unwrap().lines;
    assert_ne!(off, on, "the clearing rings ramp");
    assert!((min_z(&off) - min_z(&on)).abs() < 1e-6, "same depth");
    d.ops[k].p.ramp = false;
    assert_eq!(off, generate(d, Units::Metric).unwrap().lines, "Ramp in unticked: plunges as before");
}

/// Moves that go down while they travel sideways: the ramps.
fn ramp_moves(l: &[String]) -> usize {
    l.iter().filter(|x| x.starts_with("G1") && x.contains(" Z") && (x.contains(" X") || x.contains(" Y"))).count()
}

#[test]
fn only_the_rings_next_to_the_walls_ramp_when_the_stepover_allows_it() {
    use putiacnc_core::model::CustomFeeds;
    let mut p = pocket();
    p.data.job.ramp_pockets = true;
    let k = p.data.ops.iter().position(|o| o.p.kind == "pocket").unwrap();
    p.data.ops[k].p.ramp = true;
    let with_so = |p: &mut putiacnc_core::model::ProjectFile, so: f64| {
        p.data.ops[k].p.custom = Some(CustomFeeds { rpm: None, feed: None, plunge: None, so: Some(so) });
        lines(p)
    };
    let half = with_so(&mut p, 50.0);
    let more = with_so(&mut p, 51.0);
    assert!(ramp_moves(&half) < ramp_moves(&more), "at 50 % only the outer rings ramp: {} vs {}", ramp_moves(&half), ramp_moves(&more));
    assert!((min_z(&half) - min_z(&more)).abs() < 1e-6, "same depth");
}
