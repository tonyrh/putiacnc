//! Plotter mode: a Draw operation moves a pen along the outlines by Z alone, and nothing about it spins or cuts.
use putiacnc_core::model::ProjectFile;
use putiacnc_core::{checks, generate, load_project, shapes, sim::Sim, split, verify, Units};

fn pen_project() -> ProjectFile {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/hardware_pen_test.project.json");
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn z_of(l: &str) -> Option<f64> { l.split_whitespace().find(|w| w.starts_with('Z')).and_then(|w| w[1..].parse().ok()) }

#[test]
fn a_drawing_never_starts_the_spindle() {
    let l = generate(&pen_project().data, Units::Metric).unwrap().lines;
    assert!(!l.iter().any(|x| x.starts_with("M3") || x.starts_with("M4") || x.starts_with("M5") || x.starts_with("G4")), "no spindle lines, no spin-up wait");
    assert_eq!(l.last().unwrap(), "M2");
    assert!(l.iter().any(|x| x.starts_with("(Pen:")), "the program says it is a pen");
    assert!(verify::hazards(&l).is_empty(), "{:?}", verify::hazards(&l));
    // the same text read with its pen marker removed is a cutting program with the spindle off, which the checker has to find
    let bare: Vec<String> = l.iter().filter(|x| !x.starts_with("(Pen:")).cloned().collect();
    assert!(verify::hazards(&bare).iter().any(|h| h.contains("spindle is off")), "{:?}", verify::hazards(&bare));
}

#[test]
fn the_pen_goes_down_at_z0_and_up_by_the_lift_between_strokes() {
    let f = pen_project();
    let l = generate(&f.data, Units::Metric).unwrap().lines;
    // the second operation is the real one (the first is in the air, 10 mm higher)
    let at = l.iter().position(|x| x.starts_with("M0")).unwrap();
    let real = &l[at..];
    let downs: Vec<f64> = real.iter().filter(|x| x.starts_with("G1 Z")).filter_map(|x| z_of(x)).collect();
    assert_eq!(downs.len(), 3, "three strokes: {downs:?}");
    assert!(downs.iter().all(|z| z.abs() < 1e-9), "pen down at Z0: {downs:?}");
    let ups: Vec<f64> = real.iter().filter(|x| x.starts_with("G0 Z")).filter_map(|x| z_of(x)).collect();
    assert!(ups.iter().all(|z| (*z - 2.0).abs() < 1e-9 || (*z - 5.0).abs() < 1e-9), "up at the lift (2) or the safe height (5): {ups:?}");
    assert!(real.iter().filter(|x| x.starts_with("G1")).all(|x| z_of(x).is_none_or(|z| z.abs() < 1e-9)), "nothing goes below Z0");
    // sideways moves between strokes are rapids with the pen up
    let mut z = 5.0;
    for x in real.iter().filter(|x| x.starts_with('G')) {
        if let Some(v) = z_of(x) { z = v; }
        if x.starts_with("G0 X") || x.starts_with("G0 Y") { assert!(z >= 2.0 - 1e-9, "a rapid with the pen down: {x}"); }
    }
}

#[test]
fn the_air_run_is_ten_millimetres_above_the_paper() {
    let l = generate(&pen_project().data, Units::Metric).unwrap().lines;
    let at = l.iter().position(|x| x.starts_with("M0")).unwrap();
    let low = l[..at].iter().filter(|x| x.starts_with("G1")).filter_map(|x| z_of(x)).fold(f64::MAX, f64::min);
    assert!((low - 10.0).abs() < 1e-9, "the lowest the pen goes in the air run: {low}");
}

#[test]
fn a_spring_pen_can_press_below_z0() {
    let mut f = pen_project();
    f.data.ops[1].p.depth = 1.5;
    f.data.ops[1].p.lift = Some(3.0);
    let l = generate(&f.data, Units::Metric).unwrap().lines;
    let at = l.iter().position(|x| x.starts_with("M0")).unwrap();
    let low = l[at..].iter().filter(|x| x.starts_with("G1")).filter_map(|x| z_of(x)).fold(f64::MAX, f64::min);
    assert!((low + 1.5).abs() < 1e-9, "pen down 1.5 mm below Z0: {low}");
    let hi = l[at..].iter().filter(|x| x.starts_with("G0 Z")).filter_map(|x| z_of(x)).fold(f64::MAX, f64::min);
    assert!((hi - 3.0).abs() < 1e-9, "lifted 3 mm: {hi}");
}

#[test]
fn outlines_are_closed_and_open_paths_stay_open() {
    let mut f = pen_project();
    f.data.ops.truncate(1);
    f.data.ops[0].p.air = false;
    f.data.ops[0].p.pause = None;
    f.data.loops.retain(|l| l.op == f.data.ops[0].id);
    let out = generate(&f.data, Units::Metric).unwrap();
    // split the drawn moves into strokes at each pen-down
    let mut strokes: Vec<Vec<(f64, f64)>> = vec![];
    let (mut x, mut y) = (0.0, 0.0);
    for l in &out.lines {
        for w in l.split_whitespace() { if let Some(v) = w.strip_prefix('X') { x = v.parse().unwrap_or(x); } else if let Some(v) = w.strip_prefix('Y') { y = v.parse().unwrap_or(y); } }
        if l.starts_with("G1 Z") { strokes.push(vec![(x, y)]); }
        else if l.starts_with("G1") { if let Some(s) = strokes.last_mut() { s.push((x, y)); } }
    }
    assert_eq!(strokes.len(), 3);
    let closed = strokes.iter().filter(|s| s.first() == s.last()).count();
    assert_eq!(closed, 2, "the rectangle and the circle come back to where they started, the zigzag does not");
    let rect = strokes.iter().find(|s| s.len() == 5).expect("the rectangle has four sides");
    assert!(rect.iter().all(|p| [15.0, 45.0].contains(&p.0) || [15.0, 35.0].contains(&p.1)));
}

#[test]
fn shapes_are_drawn_nearest_first() {
    let mut f = pen_project();
    f.data.ops.truncate(1);
    f.data.ops[0].p.pause = None;
    f.data.loops.retain(|l| l.op == f.data.ops[0].id);
    f.data.loops.clear();
    let id = f.data.ops[0].id;
    // three squares in a row, given out of order: the pen starts by the origin and walks along them
    for x in [60.0, 10.0, 35.0] { f.data.loops.push(shapes::new_rect([x, 10.0], [x + 10.0, 20.0], 0.0, id)); }
    let l = generate(&f.data, Units::Metric).unwrap().lines;
    let starts: Vec<f64> = l.iter().filter(|x| x.starts_with("G0 X") && !x.starts_with("G0 X0 Y0")).filter_map(|x| x.split_whitespace().find(|t| t.starts_with('X')).and_then(|t| t[1..].parse().ok())).collect();
    assert_eq!(starts.len(), 3, "{l:?}");
    assert!(starts[0] <= 20.0 && starts[1] > starts[0] && starts[2] > starts[1], "{starts:?}");
}

#[test]
fn a_pen_and_a_cutter_do_not_mix_in_one_operation() {
    let mut f = pen_project();
    let mut cutter = shapes::blank_project().data.tools[0].clone();
    cutter.id = "tcut".into();
    f.data.tools.push(cutter);
    // draw with a cutter
    f.data.ops[0].p.tool = "tcut".into();
    let e = generate(&f.data, Units::Metric).unwrap_err();
    assert!(e.contains("Operation 1") && e.contains("pen"), "{e}");
    // cut with a pen
    let mut g = pen_project();
    g.data.ops[1].p.kind = "outside".into();
    g.data.ops[1].p.depth = 1.0;
    let e = generate(&g.data, Units::Metric).unwrap_err();
    assert!(e.contains("Operation 2") && e.contains("Draw"), "{e}");
}

#[test]
fn a_cutting_job_followed_by_a_drawing_stops_the_spindle_and_asks_for_the_pen() {
    let mut f = load_project(&std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/outside_holes.project.json")).unwrap()).unwrap();
    let pen = pen_project().data.tools[0].clone();
    f.data.tools.push(pen.clone());
    f.data.loops.push(shapes::new_rect([5.0, 5.0], [15.0, 15.0], 0.0, 0));
    let n = f.data.loops.len();
    let sel: std::collections::BTreeSet<usize> = [n - 1].into_iter().collect();
    shapes::add_operation(&mut f.data, &sel, "draw");
    let o = f.data.ops.last().unwrap();
    assert_eq!(o.p.kind, "draw");
    assert_eq!(o.p.tool, pen.id, "a new Draw operation takes a pen");
    assert_eq!(o.p.depth, 0.0);
    let l = generate(&f.data, Units::Metric).unwrap().lines;
    let stop = l.iter().position(|x| x.starts_with("M0 ; Change tool to the pen")).expect("a stop to change to the pen");
    assert_eq!(l[stop - 1], "M5", "{:?}", &l[stop - 3..=stop]);
    assert!(l[stop..].iter().all(|x| !x.starts_with("M3")), "the spindle does not start again for the pen");
    assert!(verify::hazards(&l).is_empty(), "{:?}", verify::hazards(&l));
    // and a cutting operation added after a drawing starts with a cutter, at a depth
    f.data.loops.push(shapes::new_rect([30.0, 5.0], [40.0, 15.0], 0.0, 0));
    let n = f.data.loops.len();
    let sel: std::collections::BTreeSet<usize> = [n - 1].into_iter().collect();
    shapes::add_operation(&mut f.data, &sel, "outside");
    let o = f.data.ops.last().unwrap();
    assert!(f.data.tools.iter().any(|t| t.id == o.p.tool && t.shape != "pen"), "a cutter");
    assert!(o.p.depth > 0.0);
    assert!(generate(&f.data, Units::Metric).is_ok());
}

#[test]
fn a_drawing_takes_nothing_off_the_stock_and_raises_no_warning() {
    let mut f = pen_project();
    f.data.ops[1].p.depth = 1.0; // pressing 1 mm into a 1 mm "stock" would otherwise be a cut
    let out = generate(&f.data, Units::Metric).unwrap();
    let mut sim = Sim::new(&f.data, 40_000);
    sim.advance(&out.moves, out.moves.len());
    let cut: usize = (0..sim.nx).flat_map(|i| (0..sim.ny).map(move |j| (i, j))).filter(|&(i, j)| sim.at(i, j) < -1e-4).count();
    assert_eq!(cut, 0, "cells cut by a pen");
    let found = checks::run(&f.data, &out);
    assert!(found.is_empty(), "{:?}", found.iter().map(|x| &x.msg).collect::<Vec<_>>());
}

#[test]
fn imperial_and_numbered_output_work_too() {
    let mut f = pen_project();
    f.data.job.numbered = true;
    let l = generate(&f.data, Units::Imperial).unwrap().lines;
    assert!(l.iter().any(|x| x.contains("G20")));
    assert!(!l.iter().any(|x| x.contains("M3 ") || x.contains("M5")));
    assert!(verify::hazards(&l).is_empty(), "{:?}", verify::hazards(&l));
}

#[test]
fn the_pen_test_project_is_all_air_then_paper() {
    let f = pen_project();
    assert_eq!(f.data.ops.len(), 2);
    assert!(f.data.ops.iter().all(|o| o.p.kind == "draw"));
    assert!(f.data.ops[0].p.air && !f.data.ops[1].p.air);
    assert!(f.data.ops[0].p.pause.as_deref().is_some_and(|m| m.len() < 78), "the pause message fits a GRBL line");
    let l = generate(&f.data, Units::Metric).unwrap().lines;
    assert!(l.iter().filter(|x| !x.starts_with('(')).all(|x| x.len() <= 80), "{:?}", l.iter().find(|x| x.len() > 80));
    assert_eq!(l.iter().filter(|x| x.starts_with("M0")).count(), 1);
}

#[test]
fn a_split_of_drawings_gives_a_file_per_operation_and_no_spindle_anywhere() {
    let f = pen_project();
    let files = split::generate_files(&f.data, Units::Metric, split::Split::PerOperation).unwrap();
    assert_eq!(files.len(), 2);
    for x in &files {
        let l = &x.out.lines;
        assert!(!l.iter().any(|y| y.starts_with("M3") || y.starts_with("M5")), "{}", x.suffix);
        assert!(l.iter().any(|y| y.starts_with("(Pen:")));
        assert!(!l.iter().any(|y| y.starts_with("M0")), "the pause is the end of the first file");
        assert!(verify::hazards(l).is_empty(), "{:?}", verify::hazards(l));
        assert!(x.out.gcode().contains("Spindle:"), "the summary says the spindle stays off");
    }
    let per_tool = split::generate_files(&f.data, Units::Metric, split::Split::PerTool).unwrap();
    assert_eq!(per_tool.len(), 1, "both operations use the same pen, so they stay one file");
}

#[test]
fn every_pen_of_the_pens_and_markers_library_draws() {
    let (name, json) = putiacnc_core::toollib::BUNDLED.iter().find(|(n, _)| *n == "Pens and markers").expect("the library is built in");
    let mut libs = putiacnc_core::toollib::ToolLibs::default();
    libs.import(json).unwrap();
    let lib = libs.libs.iter().find(|l| l.name == *name).unwrap();
    assert!(lib.tools.len() >= 16);
    for t in &lib.tools {
        assert_eq!(t.shape, "pen", "{}", t.name);
        assert!(t.dia > 0.0 && t.dia <= 3.0 && t.rpm == 0.0 && t.feed >= 1000.0 && t.plunge > 0.0, "{}", t.name);
        assert!(t.name.chars().count() <= 24, "{}: a name that fits the tool list", t.name);
        let mut f = pen_project();
        f.data.tools = vec![t.clone()];
        for o in f.data.ops.iter_mut() { o.p.tool = t.id.clone(); }
        let l = generate(&f.data, Units::Metric).unwrap_or_else(|e| panic!("{}: {e}", t.name)).lines;
        assert!(!l.iter().any(|x| x.starts_with("M3") || x.starts_with("M5")), "{}", t.name);
        assert!(verify::hazards(&l).is_empty(), "{}: {:?}", t.name, verify::hazards(&l));
        assert!(l.iter().any(|x| x.contains(&format!("F{}", t.feed))), "{} draws at its own speed", t.name);
    }
}

#[test]
fn single_line_text_is_followed_by_the_pen_stroke_by_stroke() {
    let mut f = pen_project();
    f.data.loops = putiacnc_core::linetext::line_text_loops("futural", "Ciao, è così!", putiacnc_core::text::TextStyle::plain(8.0, 0.0, putiacnc_core::text::Align::Left), [10.0, 10.0], 0).unwrap();
    assert!(f.data.loops.iter().all(|l| l.pts.len() >= 4));
    let sel: std::collections::BTreeSet<usize> = (0..f.data.loops.len()).collect();
    f.data.ops.clear();
    shapes::add_operation(&mut f.data, &sel, "draw");
    let l = generate(&f.data, Units::Metric).unwrap().lines;
    assert!(!l.iter().any(|x| x.starts_with("M3") || x.starts_with("M5")));
    let downs = l.iter().filter(|x| x.starts_with("G1 Z")).count();
    assert_eq!(downs, f.data.loops.len(), "one pen-down per stroke");
    assert!(verify::hazards(&l).is_empty(), "{:?}", verify::hazards(&l));
}

fn hatch_job(shapes_: Vec<putiacnc_core::model::LoopData>, h: putiacnc_core::model::Hatch) -> Vec<String> {
    let mut f = pen_project();
    f.data.loops = shapes_;
    let sel: std::collections::BTreeSet<usize> = (0..f.data.loops.len()).collect();
    f.data.ops.clear();
    shapes::add_operation(&mut f.data, &sel, "draw");
    f.data.ops[0].p.hatch = Some(h);
    generate(&f.data, Units::Metric).unwrap().lines
}

/// Total length of the moves made with the pen down.
fn ink(l: &[String]) -> f64 {
    let (mut x, mut y, mut z, mut len) = (0.0, 0.0, 5.0, 0.0);
    for s in l.iter().filter(|s| s.starts_with("G0") || s.starts_with("G1")) {
        let v = |c: char| s.split_whitespace().find(|w| w.starts_with(c)).and_then(|w| w[1..].parse::<f64>().ok());
        let (nx, ny, nz) = (v('X').unwrap_or(x), v('Y').unwrap_or(y), v('Z').unwrap_or(z));
        if s.starts_with("G1") && nz.abs() < 1e-9 && z.abs() < 1e-9 { len += (nx - x).hypot(ny - y); }
        (x, y, z) = (nx, ny, nz);
    }
    len
}

#[test]
fn a_hatch_fills_a_rectangle_with_lines_one_pen_width_apart() {
    use putiacnc_core::model::Hatch;
    let r = shapes::new_rect([10.0, 10.0], [30.0, 20.0], 0.0, 0);
    let pen = pen_project().data.tools[0].dia;
    let plain = ink(&hatch_job(vec![r.clone()], Hatch { angle: 0.0, gap: Some(1.0), cross: false, outline: false }));
    // 10 lines of 20 mm, and the 9 short joins between neighbouring ends
    assert!((plain - 209.0).abs() < 1.0, "{plain}");
    let with_edge = ink(&hatch_job(vec![r.clone()], Hatch { angle: 0.0, gap: Some(1.0), cross: false, outline: true }));
    assert!((with_edge - plain - 60.0).abs() < 1.0, "{with_edge}");
    let crossed = ink(&hatch_job(vec![r.clone()], Hatch { angle: 30.0, gap: Some(1.0), cross: true, outline: false }));
    let one = ink(&hatch_job(vec![r.clone()], Hatch { angle: 30.0, gap: Some(1.0), cross: false, outline: false }));
    assert!(crossed > one * 1.8 && one > 190.0 && one < 220.0, "{one} {crossed}");
    // without a gap the lines are one pen width apart
    let dflt = ink(&hatch_job(vec![r], Hatch { angle: 0.0, gap: None, cross: false, outline: false }));
    assert!((dflt - 200.0 / pen).abs() < 20.0 / pen + 20.0, "{dflt} for pen {pen}");
}

#[test]
fn a_shape_inside_another_is_a_hole_in_the_fill() {
    use putiacnc_core::model::Hatch;
    let outer = shapes::new_rect([0.0, 0.0], [20.0, 20.0], 0.0, 0);
    let inner = shapes::new_rect([5.0, 5.0], [15.0, 15.0], 0.0, 0);
    let h = Hatch { angle: 0.0, gap: Some(1.0), cross: false, outline: false };
    let full = ink(&hatch_job(vec![outer.clone()], h.clone()));
    let ring = ink(&hatch_job(vec![outer, inner], h));
    // the 10 x 10 hole takes 10 lines of 10 mm away (the short joins along the edges differ a little)
    assert!((full - ring - 100.0).abs() < 12.0, "{full} {ring}");
}

#[test]
fn open_lines_are_still_drawn_and_a_hatch_never_starts_the_spindle() {
    use putiacnc_core::model::Hatch;
    let line = shapes::new_poly(&[[0.0, 0.0], [10.0, 0.0]], false, 0).unwrap();
    let l = hatch_job(vec![line], Hatch::default());
    assert!((ink(&l) - 10.0).abs() < 1e-6);
    assert!(!l.iter().any(|x| x.starts_with("M3") || x.starts_with("M5")));
    assert!(verify::hazards(&l).is_empty());
}

#[test]
fn the_fill_settings_survive_saving_and_old_projects_have_none() {
    use putiacnc_core::model::Hatch;
    let mut f = pen_project();
    assert!(f.data.ops.iter().all(|o| o.p.hatch.is_none()), "the pen test has no fill");
    let json = serde_json::to_string(&f).unwrap();
    assert!(!json.contains("hatch"), "no fill, nothing saved");
    f.data.ops[0].p.hatch = Some(Hatch { angle: 30.0, gap: Some(0.8), cross: true, outline: false });
    let back = load_project(&serde_json::to_string(&f).unwrap()).unwrap();
    assert_eq!(back.data.ops[0].p.hatch, f.data.ops[0].p.hatch);
    // a fill saved with only some of its settings gets the usual ones for the rest
    let part: Hatch = serde_json::from_str("{\"cross\":true}").unwrap();
    assert!(part.outline && part.angle == 45.0 && part.gap.is_none() && part.cross);
}

fn plunges(l: &[String]) -> usize { l.iter().filter(|x| x.starts_with("G1 Z")).count() }

#[test]
fn the_lines_of_a_fill_are_joined_into_one_zigzag_without_lifting() {
    use putiacnc_core::model::Hatch;
    let r = shapes::new_rect([10.0, 10.0], [30.0, 20.0], 0.0, 0);
    let l = hatch_job(vec![r.clone()], Hatch { angle: 0.0, gap: Some(1.0), cross: false, outline: false });
    assert_eq!(plunges(&l), 1, "one zigzag, one pen-down");
    // joins are drawn with the pen down: no lift between rows
    let lifts = l.iter().filter(|x| x.starts_with("G0 Z")).count();
    assert!(lifts <= 3, "{lifts}");
}

#[test]
fn lines_with_a_hole_between_them_are_not_joined_across_it() {
    use putiacnc_core::model::Hatch;
    let outer = shapes::new_rect([0.0, 0.0], [20.0, 20.0], 0.0, 0);
    let inner = shapes::new_rect([5.0, 5.0], [15.0, 15.0], 0.0, 0);
    let l = hatch_job(vec![outer, inner], Hatch { angle: 0.0, gap: Some(1.0), cross: false, outline: false });
    // 10 full rows of 20 mm and 10 rows of two 5 mm pieces make 300 mm of lines, plus at most a 1 mm join between neighbouring
    // rows; a join across the 10 mm hole would add 10 mm each
    let total = ink(&l);
    assert!(total > 299.0 && total < 335.0, "{total}");
}

#[test]
fn a_join_distance_keeps_the_pen_down_between_nearby_strokes() {
    let a = shapes::new_poly(&[[0.0, 0.0], [10.0, 0.0]], false, 0).unwrap();
    let b = shapes::new_poly(&[[10.4, 0.0], [20.0, 0.0]], false, 0).unwrap();
    let c = shapes::new_poly(&[[40.0, 0.0], [50.0, 0.0]], false, 0).unwrap();
    let run = |join: Option<f64>| {
        let mut f = pen_project();
        f.data.loops = vec![a.clone(), b.clone(), c.clone()];
        let sel: std::collections::BTreeSet<usize> = (0..3).collect();
        f.data.ops.clear();
        shapes::add_operation(&mut f.data, &sel, "draw");
        f.data.ops[0].p.join = join;
        generate(&f.data, Units::Metric).unwrap().lines
    };
    assert_eq!(plunges(&run(None)), 3);
    assert_eq!(plunges(&run(Some(1.0))), 2, "the first two are one line, the far one is not joined");
    assert!((ink(&run(Some(1.0))) - ink(&run(None)) - 0.4).abs() < 1e-6, "the join is drawn");
    let back = load_project(&serde_json::to_string(&{ let mut f = pen_project(); f.data.ops[0].p.join = Some(0.7); f }).unwrap()).unwrap();
    assert_eq!(back.data.ops[0].p.join, Some(0.7));
}

#[test]
fn an_edge_two_shapes_share_is_drawn_once() {
    // two squares side by side share the edge x = 10
    let l = shapes::new_rect([0.0, 0.0], [10.0, 10.0], 0.0, 0);
    let r = shapes::new_rect([10.0, 0.0], [20.0, 10.0], 0.0, 0);
    let mut f = pen_project();
    f.data.loops = vec![l, r];
    let sel: std::collections::BTreeSet<usize> = (0..2).collect();
    f.data.ops.clear();
    shapes::add_operation(&mut f.data, &sel, "draw");
    let lines = generate(&f.data, Units::Metric).unwrap().lines;
    // 2 squares of 40 mm round, minus the shared 10 mm
    assert!((ink(&lines) - 70.0).abs() < 1e-6, "{}", ink(&lines));
    // shapes that share nothing stay as they were: two separate loops
    let mut g = pen_project();
    g.data.loops = vec![shapes::new_rect([0.0, 0.0], [10.0, 10.0], 0.0, 0), shapes::new_rect([20.0, 0.0], [30.0, 10.0], 0.0, 0)];
    g.data.ops.clear();
    shapes::add_operation(&mut g.data, &sel, "draw");
    assert!((ink(&generate(&g.data, Units::Metric).unwrap().lines) - 80.0).abs() < 1e-6);
}

#[test]
fn a_pens_ink_colour_is_saved_read_back_and_does_not_change_the_gcode() {
    use putiacnc_core::model::hex_rgb;
    let mut f = pen_project();
    assert_eq!(f.data.tools[0].ink_rgb(), None, "no colour unless set");
    let before = generate(&f.data, Units::Metric).unwrap().lines;
    assert!(!serde_json::to_string(&f).unwrap().contains("color"), "nothing saved without a colour");
    f.data.tools[0].color = Some("#d62828".into());
    assert_eq!(f.data.tools[0].ink_rgb(), Some([0xd6, 0x28, 0x28]));
    assert_eq!(hex_rgb([0xd6, 0x28, 0x28]), "#d62828");
    let back = load_project(&serde_json::to_string(&f).unwrap()).unwrap();
    assert_eq!(back.data.tools[0].ink_rgb(), Some([0xd6, 0x28, 0x28]));
    assert_eq!(generate(&f.data, Units::Metric).unwrap().lines, before, "colour is for the screen only");
    // a duplicate keeps it
    assert_eq!(putiacnc_core::toollib::new_tool_from(Some(&f.data.tools[0]), true, "mm").color, f.data.tools[0].color);
    // anything that is not #rrggbb is no colour
    for bad in ["red", "#12345", "#12345g", "d62828", "#d6282", "#é62828"] {
        f.data.tools[0].color = Some(bad.into());
        assert_eq!(f.data.tools[0].ink_rgb(), None, "{bad}");
    }
}
