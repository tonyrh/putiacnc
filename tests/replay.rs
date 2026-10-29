//! The 3D preview plays the G-code text. These tests hold the replay of the text to what the generator says it wrote.
mod common;
use putiacnc_core::model::ProjectData;
use putiacnc_core::replay::{replay, Kind};
use putiacnc_core::{generate, load_project, shapes, Units};

fn sample(name: &str) -> ProjectData {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/data/{name}.project.json"));
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap().data
}

fn variants(base: &ProjectData) -> Vec<(String, ProjectData)> {
    let mut v = vec![("plain".to_string(), base.clone())];
    let mut d = base.clone(); d.job.post = "mach".into(); v.push(("mach".into(), d));
    let mut d = base.clone(); d.job.numbered = true; v.push(("numbered".into(), d));
    let mut d = base.clone(); d.job.air = true; v.push(("air".into(), d));
    let mut d = base.clone(); for o in d.ops.iter_mut() { o.p.ramp = true; } v.push(("ramp".into(), d));
    let mut d = base.clone(); d.machine.get_or_insert_with(Default::default).arcs = Some(true); v.push(("arcs".into(), d));
    let mut d = base.clone(); d.ops[0].p.pause = Some("Flip the stock".into()); v.push(("pause".into(), d));
    v
}

#[test]
fn the_replay_agrees_with_the_generator_on_every_sample_and_variant() {
    for name in ["outside_holes", "pocket_island", "vbit_profile", "tabs", "imperial_center", "vcarve_circle", "outside_ramp", "two_ops_toolchange"] {
        for (v, d) in variants(&sample(name)) {
            for units in [Units::Metric, Units::Imperial] {
                let o = generate(&d, units).unwrap();
                let bad = common::replay_problems(&d, &o, units);
                assert!(bad.is_empty(), "{name}/{v} ({units:?}): {bad:?}");
            }
        }
    }
}

#[test]
fn an_inlay_with_its_second_tool_and_a_relief_replay_too() {
    let mut f = shapes::blank_project();
    f.data.loops.push(shapes::new_rect([30.0, 40.0], [50.0, 50.0], 0.0, 0));
    let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
    shapes::add_operation(&mut f.data, &sel, "inlay");
    let o = generate(&f.data, Units::Metric).unwrap();
    assert!(o.moves.iter().any(|m| m.alt), "the inlay uses its flat end mill");
    let bad = common::replay_problems(&f.data, &o, Units::Metric);
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn stops_waits_and_tools_are_steps_with_their_line() {
    let mut d = sample("two_ops_toolchange");
    d.ops[0].p.pause = Some("Flip the stock".into());
    let o = generate(&d, Units::Metric).unwrap();
    let r = replay(&o.lines, &d);
    let stops: Vec<(usize, &str)> = r.info.iter().filter_map(|i| match &i.kind { Kind::Stop(m) => Some((i.line, m.as_str())), _ => None }).collect();
    assert_eq!(stops.len(), 2, "the pause and the tool change: {stops:?}");
    assert_eq!(stops[0].1, "Flip the stock");
    assert!(stops[1].1.starts_with("Change tool to"));
    for (line, _) in &stops { assert!(o.lines[*line].starts_with("M0 "), "{}", o.lines[*line]); }
    assert!(r.info.iter().any(|i| matches!(&i.kind, Kind::Tool(t) if t.contains("flat end mill"))));
    assert!(r.info.iter().any(|i| i.kind == Kind::Spindle(false)));
    // every step knows its line, and a line finds its step again
    for (n, i) in r.info.iter().enumerate() { assert!(r.step_of_line(i.line) <= n); }
}

#[test]
fn lines_that_cannot_be_placed_are_listed_not_guessed() {
    let d = sample("outside_holes");
    let mut o = generate(&d, Units::Metric).unwrap();
    let at = o.lines.len() - 3;
    o.lines.insert(at, "G53 G0 X0 Y200".into());
    o.lines.insert(at, "G91".into());
    let r = replay(&o.lines, &d);
    assert_eq!(r.unknown, vec![at, at + 1]);
}

#[test]
fn a_3d_relief_replays_with_its_roughing_tool() {
    let d = common::pyramid();
    for (v, d) in variants(&d) {
        let o = generate(&d, Units::Metric).unwrap();
        assert!(o.moves.iter().any(|m| m.alt), "the flat end mill roughs");
        let bad = common::replay_problems(&d, &o, Units::Metric);
        assert!(bad.is_empty(), "relief/{v}: {bad:?}");
    }
}

#[test]
fn every_step_ends_at_its_own_time_so_a_stop_can_be_told_from_the_step_after_it() {
    let mut d = sample("two_ops_toolchange");
    d.ops[0].p.pause = Some("Flip the stock".into());
    let o = generate(&d, Units::Metric).unwrap();
    let r = replay(&o.lines, &d);
    let mut t = 0.0;
    let mut stops = 0;
    for (m, i) in r.moves.iter().zip(&r.info) {
        assert!(m.dt > 0.0, "a step takes no time: {:?}", i.kind);
        t += m.dt;
        if matches!(i.kind, Kind::Stop(_)) { stops += 1; }
    }
    assert!(t > 0.0 && stops >= 2, "expected the pause and the tool change to be stops, got {stops}");
}
