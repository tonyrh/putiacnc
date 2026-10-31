//! The program in several files: one per tool, or one per operation.
use putiacnc_core::model::ProjectData;
use putiacnc_core::split::{generate_files, groups, Split};
use putiacnc_core::{generate, load_project, verify, Units};

fn project() -> ProjectData {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/hardware_m0_aircut.project.json");
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap().data
}

fn count(l: &[String], pre: &str) -> usize { l.iter().filter(|x| x.starts_with(pre)).count() }

#[test]
fn every_file_is_a_complete_program() {
    for mode in [Split::PerOperation, Split::PerTool] {
        let files = generate_files(&project(), Units::Metric, mode).unwrap();
        assert!(files.len() >= 2);
        for f in &files {
            let l = &f.out.lines;
            assert!(l[0].starts_with("(File ") || l[0].starts_with("(Summary"), "{:?}", &l[..2]);
            assert!(l.iter().any(|x| x.starts_with("(Summary")), "a summary");
            assert!(l.iter().any(|x| x.starts_with("G21")), "units and mode set");
            assert_eq!(count(l, "M3 "), 1 + count(l, "M0"), "the spindle starts once at the top and again after every stop");
            assert_eq!(l.last().map(String::as_str), Some("M2"), "a safe end: {:?}", &l[l.len() - 3..]);
            assert!(l.iter().rev().take(5).any(|x| x == "M5"), "the spindle stops at the end");
            assert!(verify::hazards(l).is_empty(), "{:?}: {:?}", f.suffix, verify::hazards(l));
            // GRBL drops comments as they arrive, so only the other lines have to fit its 80 characters
            assert!(l.iter().filter(|x| !x.starts_with('(')).all(|x| x.len() <= 80), "{:?}", l.iter().find(|x| !x.starts_with('(') && x.len() > 80));
        }
    }
}

#[test]
fn per_operation_has_no_stops_and_says_what_comes_next() {
    let files = generate_files(&project(), Units::Metric, Split::PerOperation).unwrap();
    assert_eq!(files.len(), 3);
    // the pause after operation 1 and the tool change before operation 3 are the ends of files, not M0
    for f in &files { assert_eq!(count(&f.out.lines, "M0"), 0, "{}", f.suffix); }
    let first = &files[0].out.lines;
    assert!(first.iter().any(|x| x == "(File 1 of 3: Operation 1: outside)"));
    assert!(!first.iter().any(|x| x.starts_with("(Before you start")), "nothing to do before the first file");
    assert!(first.iter().any(|x| x == "(After this file: TEST 1: spindle off, tool up. Press Resume)"), "the pause message is kept");
    let second = &files[1].out.lines;
    assert!(second.iter().any(|x| x == "(File 2 of 3: Operation 2: pocket)"));
    assert!(second.iter().any(|x| x.starts_with("(Before you start: the same tool as the previous file")));
    let third = &files[2].out.lines;
    assert!(third.iter().any(|x| x == "(Before you start: change to 6 mm flat end mill and touch off Z0 again.)"), "{:?}", third.iter().filter(|x| x.contains("Before")).collect::<Vec<_>>());
    assert!(!third.iter().any(|x| x.starts_with("(After this file")), "no message at the end of the last file");
}

#[test]
fn operation_numbers_stay_the_projects_own() {
    let files = generate_files(&project(), Units::Metric, Split::PerOperation).unwrap();
    for (k, f) in files.iter().enumerate() {
        assert!(f.out.lines.iter().any(|x| x.starts_with(&format!("(Operation {}:", k + 1))), "file {} names its operation: {:?}", k + 1, f.out.lines.iter().filter(|x| x.starts_with("(Operation")).collect::<Vec<_>>());
        assert!(!f.out.lines.iter().any(|x| x.starts_with("(Operation") && !x.starts_with(&format!("(Operation {}:", k + 1))), "no other operation in file {}", k + 1);
        assert!(f.out.moves.iter().all(|m| m.op == k), "every move belongs to operation {}", k + 1);
    }
}

#[test]
fn per_tool_keeps_a_pause_inside_a_file_and_drops_the_tool_change() {
    let files = generate_files(&project(), Units::Metric, Split::PerTool).unwrap();
    assert_eq!(files.len(), 2);
    // operations 1 and 2 share a tool: the pause between them stays as M0; the tool change is the boundary of the files
    assert_eq!(count(&files[0].out.lines, "M0"), 1);
    assert!(files[0].out.lines.iter().any(|x| x == "M0 ; TEST 1: spindle off, tool up. Press Resume"));
    assert_eq!(count(&files[1].out.lines, "M0"), 0);
    assert!(files[1].out.lines.iter().any(|x| x.starts_with("(Before you start: change to 6 mm")));
}

#[test]
fn the_files_together_cut_what_the_whole_program_cuts() {
    let whole = generate(&project(), Units::Metric).unwrap();
    for mode in [Split::PerOperation, Split::PerTool] {
        let files = generate_files(&project(), Units::Metric, mode).unwrap();
        let len: f64 = files.iter().map(|f| f.out.cut_len_mm).sum();
        assert!((len - whole.cut_len_mm).abs() < 1e-6, "{mode:?}: {len} vs {}", whole.cut_len_mm);
        let moves: usize = files.iter().map(|f| f.out.moves.iter().filter(|m| m.cut).count()).sum();
        assert_eq!(moves, whole.moves.iter().filter(|m| m.cut).count(), "{mode:?}: the same cutting moves");
        // each file cuts only its own shapes
        let g = groups(&project(), mode);
        for (f, ops) in files.iter().zip(&g) { assert!(f.out.moves.iter().all(|m| ops.contains(&m.op))); }
    }
}

#[test]
fn one_file_is_the_whole_program() {
    let files = generate_files(&project(), Units::Metric, Split::One).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].suffix, "");
    let whole = generate(&project(), Units::Metric).unwrap();
    assert!((files[0].out.cut_len_mm - whole.cut_len_mm).abs() < 1e-6);
    // with one file nothing says "File 1 of 1"
    assert!(!files[0].out.lines.iter().any(|x| x.starts_with("(File ")));
    assert_eq!(count(&files[0].out.lines, "M0"), 2, "both stops stay");
}

#[test]
fn a_pause_without_a_message_gets_the_standard_one_at_the_end_of_its_file() {
    let mut d = project();
    d.ops[0].p.pause = Some("  ".into());
    let files = generate_files(&d, Units::Metric, Split::PerOperation).unwrap();
    assert!(files[0].out.lines.iter().any(|x| x == "(After this file: Operation 1 done: resume when ready)"));
}

#[test]
fn brackets_and_odd_letters_in_a_pause_message_do_not_break_the_comment() {
    let mut d = project();
    d.ops[0].p.pause = Some("Flip (the) stock \u{2013} \u{e8} pronto".into());
    let files = generate_files(&d, Units::Metric, Split::PerOperation).unwrap();
    let c = files[0].out.lines.iter().find(|x| x.starts_with("(After this file")).unwrap();
    assert_eq!(c.matches('(').count(), 1);
    assert_eq!(c.matches(')').count(), 1, "{c}");
    assert!(c.is_ascii(), "{c}");
}

#[test]
fn line_numbers_run_on_in_every_file() {
    let mut d = project();
    d.job.numbered = true;
    let files = generate_files(&d, Units::Metric, Split::PerOperation).unwrap();
    for f in &files {
        let l = &f.out.lines;
        assert!(l.iter().all(|x| x.starts_with('N')), "{:?}", l.iter().find(|x| !x.starts_with('N')));
        assert!(l[0].starts_with("N10 "), "numbers start at 10 in every file: {}", l[0]);
    }
}

#[test]
fn a_plug_on_its_own_piece_stays_its_own_file() {
    use putiacnc_core::model::PlugPiece;
    use putiacnc_core::shapes;
    let mut f = shapes::blank_project();
    let d = &mut f.data;
    d.stock.w = 120.0; d.stock.h = 80.0; d.stock.t = 15.0;
    d.loops.push(shapes::new_rect([25.0, 20.0], [95.0, 60.0], 8.0, 0));
    d.loops.push(shapes::new_circle([60.0, 40.0], 10.0, 0));
    let sel: std::collections::BTreeSet<usize> = [0, 1].into_iter().collect();
    shapes::add_operation(d, &sel, "inlay");
    let (pocket, plug) = (d.ops[0].id, d.ops[1].id);
    let piece = PlugPiece { t: 8.0, mat: "darkwood".into(), ..putiacnc_core::pieces::default_piece(&d.stock) };
    shapes::put_plug_on_piece(d, pocket, plug, Some(piece));
    // the main stock has the pocket only; the plug is cut from its piece
    assert_eq!(groups(&f.data, Split::PerOperation), vec![vec![0]]);
    assert_eq!(groups(&f.data, Split::PerTool), vec![vec![0]]);
}

#[test]
fn the_summary_of_a_file_names_only_its_own_tool_and_operations() {
    let files = generate_files(&project(), Units::Metric, Split::PerOperation).unwrap();
    let text = |f: &putiacnc_core::split::SplitFile| f.out.lines.iter().filter(|x| x.starts_with('(')).cloned().collect::<Vec<_>>().join("\n");
    let (a, c) = (text(&files[0]), text(&files[2]));
    assert!(a.contains("Tools:") && a.contains("1/8in flat end mill") && !a.contains("6 mm flat end mill. "), "{a}");
    assert!(c.contains("6 mm flat end mill") && !c.contains("1/8in flat end mill, "), "{c}");
    assert!(a.contains("Operation 1 runs 10 mm above the stock") && c.contains("Operation 3 runs 10 mm above the stock"), "{a}\n{c}");
}
