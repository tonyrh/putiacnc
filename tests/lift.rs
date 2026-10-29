//! The program lifts the tool to safe Z before anything moves sideways, whatever the tool's height was after zeroing.
use putiacnc_core::{generate, load_project, Units};

fn first_moves(lines: &[String]) -> (Option<usize>, Option<usize>) {
    let lift = lines.iter().position(|l| l.starts_with("G0 Z"));
    let sideways = lines.iter().position(|l| (l.starts_with("G0 ") || l.starts_with("G1 ")) && (l.contains(" X") || l.contains(" Y")));
    (lift, sideways)
}

fn project(name: &str) -> putiacnc_core::ProjectFile {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/data/{name}.project.json"));
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn every_program_lifts_before_its_first_sideways_move() {
    for name in ["outside_holes", "pocket_island", "vbit_profile", "tabs", "imperial_center", "vcarve_circle"] {
        for units in [Units::Metric, Units::Imperial] {
            let f = project(name);
            let out = generate(&f.data, units).unwrap();
            let (lift, side) = first_moves(&out.lines);
            assert!(lift.is_some() && lift < side, "{name}: lift {lift:?} before the first sideways move {side:?}");
            // safe Z above the stock top, in the program's units
            let st = &f.data.stock;
            let zt = if st.zm == "top" { 0.0 } else { st.t - st.zh };
            let z: f64 = out.lines[lift.unwrap()][4..].parse().unwrap();
            assert!((z - (f.data.job.safe + zt) / units.factor()).abs() < 1e-3, "{name}: safe Z {z}");
        }
    }
}

#[test]
fn the_lift_comes_before_the_spindle_starts_and_follows_the_mach_header() {
    let mut f = project("outside_holes");
    f.data.job.post = "mach".into();
    let l = generate(&f.data, Units::Metric).unwrap().lines;
    let (lift, spindle) = (l.iter().position(|x| x.starts_with("G0 Z")).unwrap(), l.iter().position(|x| x.contains("M3 S")).unwrap());
    assert!(lift < spindle);
    assert!(l[..lift].iter().any(|x| x.starts_with("G40 G49 G80")), "after the Mach3 / LinuxCNC header");
}

#[test]
fn an_air_cut_lifts_to_the_safe_height_of_the_air_cut() {
    let mut f = project("outside_holes");
    f.data.job.air = true;
    let l = generate(&f.data, Units::Metric).unwrap().lines;
    assert_eq!(l[l.iter().position(|x| x.starts_with("G0 Z")).unwrap()], "G0 Z15");
}
