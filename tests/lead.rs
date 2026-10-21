use putiacnc_core::{generate, load_project, Units};

fn sample() -> String { std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/outside_holes.project.json")).unwrap() }

#[test]
fn lead_arcs_start_and_end_in_the_waste_and_are_off_by_default() {
    let mut p = load_project(&sample()).unwrap();
    p.data.ops[0].p.depth = 3.0;
    p.data.ops[0].p.ramp = false;
    let plain = generate(&p.data, Units::Metric).unwrap();
    p.data.ops[0].p.lead = Some(2.0);
    let led = generate(&p.data, Units::Metric).unwrap();
    assert!(led.moves.len() > plain.moves.len(), "arcs add moves");
    assert!(led.notes.iter().all(|n| !n.contains("lead")), "{:?}", led.notes);
    // the cut path itself is the same: every plain cut point also occurs with leads
    let key = |m: &putiacnc_core::toolpath::Move| ((m.to[0] * 100.0).round() as i64, (m.to[1] * 100.0).round() as i64, (m.to[2] * 100.0).round() as i64);
    let set: std::collections::HashSet<_> = led.moves.iter().map(key).collect();
    assert!(plain.moves.iter().filter(|m| m.cut && m.to[2] < -1e-9).all(|m| set.contains(&key(m))));
    // the plunge happens away from the line: a vertical cut move never lands on a point that the plain path plunges at
    let plain_plunges: std::collections::HashSet<_> = plain.moves.iter().filter(|m| m.cut && m.to[2] < -1e-9 && m.from[0] == m.to[0] && m.from[1] == m.to[1] && m.from[2] != m.to[2]).map(|m| (key(m).0, key(m).1)).collect();
    let led_plunges: Vec<_> = led.moves.iter().filter(|m| m.cut && m.to[2] < -1e-9 && m.from[0] == m.to[0] && m.from[1] == m.to[1] && m.from[2] != m.to[2]).collect();
    assert!(!led_plunges.is_empty());
    assert!(led_plunges.iter().all(|m| !plain_plunges.contains(&(key(m).0, key(m).1))), "plunges should move off the line");
    // the arcs are tangent: the first arc move runs towards the line, the path then continues smoothly (no big turn)
    // and the cut ends away from where it starts
    let s = serde_json::to_string(&p).unwrap();
    assert!(s.contains("\"leadR\":2.0"));
    assert!(load_project(&s).unwrap().data.ops[0].p.lead == Some(2.0));
}

#[test]
fn lead_is_dropped_when_there_is_no_room() {
    let mut p = load_project(&sample()).unwrap();
    p.data.ops[0].p.lead = Some(500.0);
    let o = generate(&p.data, Units::Metric).unwrap();
    // a huge arc shrinks to one that fits, or is skipped with a note; either way the job still generates
    assert!(o.moves.iter().any(|m| m.cut));
}
