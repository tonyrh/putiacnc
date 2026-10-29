//! A pause after an operation: lift, spindle off, `M0` with the message, then the next operation starts the spindle again.
use putiacnc_core::model::ProjectFile;
use putiacnc_core::{generate, load_project, summary, verify, Units};

fn project() -> ProjectFile {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/two_ops_toolchange.project.json");
    let mut f = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
    // both operations with the same tool, so the only stop is the one asked for
    let first = f.data.ops[0].p.tool.clone();
    f.data.ops[1].p.tool = first;
    f
}

#[test]
fn a_pause_lifts_stops_the_spindle_waits_and_starts_again() {
    let mut f = project();
    f.data.ops[0].p.pause = Some("Flip the stock".into());
    let o = generate(&f.data, Units::Metric).unwrap();
    let l = &o.lines;
    let m0 = l.iter().position(|x| x.starts_with("M0 ")).expect("a pause");
    assert_eq!(l[m0], "M0 ; Flip the stock", "GRBL: a ; comment, which gSender shows");
    assert_eq!(l[m0 - 1], "M5");
    assert!(l[m0 - 2].starts_with("G0 Z"), "lifts first: {}", l[m0 - 2]);
    // before the next operation the spindle starts again, with the wait, and only then does it move
    let m3 = m0 + l[m0..].iter().position(|x| x.starts_with("M3 ")).unwrap();
    assert_eq!(l[m3 + 1], "G4 P2");
    assert!(l[m0..m3].iter().all(|x| !x.starts_with("G1")), "nothing cuts between the pause and the spindle start");
    assert!(l[m3..].iter().any(|x| x.starts_with("(Operation 2")));
    assert_eq!(l.iter().filter(|x| x.starts_with("M0 ")).count(), 1, "one pause");
    assert!(verify::hazards(l).is_empty(), "{:?}", verify::hazards(l));
}

#[test]
fn without_a_pause_nothing_changes_and_an_empty_message_gets_a_standard_one() {
    let mut f = project();
    let plain = generate(&f.data, Units::Metric).unwrap();
    assert!(!plain.lines.iter().any(|x| x.starts_with("M0")));
    f.data.ops[0].p.pause = Some("  ".into());
    let o = generate(&f.data, Units::Metric).unwrap();
    assert!(o.lines.iter().any(|x| x == "M0 ; Operation 1 done: resume when ready"));
    assert_eq!(o.lines.len(), plain.lines.len() + 4, "lift, M5, M0 and the new M3 with its wait");
}

#[test]
fn the_message_is_plain_ascii_without_parentheses() {
    let mut f = project();
    f.data.ops[0].p.pause = Some("Flip (the) \"stock\" — côté B".into());
    let o = generate(&f.data, Units::Metric).unwrap();
    let m0 = o.lines.iter().find(|x| x.starts_with("M0 ")).unwrap();
    assert!(m0.is_ascii() && !m0.contains('(') && !m0.contains(')'), "{m0}");
    assert!(m0.contains("Flip -the- instock"), "{m0}");
}

#[test]
fn the_summary_mentions_the_pause_and_every_variant_stays_clean() {
    let mut f = project();
    f.data.ops[0].p.pause = Some("Move the clamp".into());
    for (mach, arcs, numbered) in [(false, false, false), (true, false, false), (false, true, true)] {
        let mut d = f.data.clone();
        d.job.post = if mach { "mach".into() } else { String::new() };
        d.job.numbered = numbered;
        d.machine.get_or_insert_with(Default::default).arcs = Some(arcs);
        for units in [Units::Metric, Units::Imperial] {
            let o = generate(&d, units).unwrap();
            assert!(verify::hazards(&o.lines).is_empty(), "{:?}", verify::hazards(&o.lines));
            let rows = summary::describe(&d, &o, units, false);
            let r = rows.iter().find(|r| r.label == "Pauses").expect("a Pauses row");
            assert!(r.text.contains("Move the clamp") && r.text.contains("1 time"), "{}", r.text);
        }
    }
}

#[test]
fn a_new_operation_does_not_inherit_the_pause() {
    let mut f = project();
    f.data.ops[1].p.pause = Some("Flip".into());
    let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
    putiacnc_core::shapes::add_operation(&mut f.data, &sel, "pocket");
    assert!(f.data.ops.last().unwrap().p.pause.is_none());
}

#[test]
fn the_summary_reads_numbered_programs_too() {
    let mut f = project();
    f.data.job.numbered = true;
    let o = generate(&f.data, Units::Metric).unwrap();
    assert!(o.lines[0].starts_with("N10 "));
    let rows = summary::describe(&f.data, &o, Units::Metric, false);
    assert!(rows.iter().any(|r| r.label == "Start" && r.text.contains("waits 2 s")), "{rows:?}");
    assert!(rows.iter().any(|r| r.label == "End"), "{rows:?}");
}

#[test]
fn mach3_gets_a_bracketed_message_because_it_has_no_semicolon_comments() {
    let mut f = project();
    f.data.job.post = "mach".into();
    f.data.ops[0].p.pause = Some("Flip the stock".into());
    let o = generate(&f.data, Units::Metric).unwrap();
    assert!(o.lines.iter().any(|x| x == "M0 (Flip the stock)"));
    assert!(!o.lines.iter().any(|x| !x.starts_with('(') && x.contains(';')), "no semicolon comments outside the bracketed header");
}
