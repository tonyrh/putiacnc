//! Random sequences of the things a person does in the window, with checks after every step.
//!
//! The actions are the App's own (draw, delete, duplicate, offset, combine, group, add and remove operations, change the stock, undo,
//! redo...). After each one the project must still make sense (no selection or tab pointing at a shape that is gone, no shape in an
//! operation that does not exist...), the toolpath must still generate, and a frame of the window must draw without panicking and
//! without egui reporting two widgets with the same ID. At the end, undoing everything must give back the first project and redoing
//! everything the last one.
//!
//!   cargo test fuzz                                       (a few sequences)
//!   COUNT=500 SEED=1 cargo test --release fuzz -- --ignored --nocapture
use crate::app::*;
use eframe::egui;
use crate::panels::KINDS;
use putiacnc_core::{history::Kind, model::LoopData, shapes};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn below(&mut self, n: usize) -> usize { (self.next() % n.max(1) as u64) as usize }
    fn f(&mut self, lo: f64, hi: f64) -> f64 { lo + (self.next() % 100_000) as f64 / 100_000.0 * (hi - lo) }
}

fn random_loop(r: &mut Rng, w: f64, h: f64, op: i64) -> Option<LoopData> {
    let (x, y) = (r.f(0.0, w), r.f(0.0, h));
    match r.below(4) {
        0 => Some(shapes::new_rect([x, y], [x + r.f(1.0, 60.0), y + r.f(1.0, 60.0)], if r.below(3) == 0 { r.f(0.0, 5.0) } else { 0.0 }, op)),
        1 => Some(shapes::new_circle([x, y], r.f(1.0, 30.0), op)),
        2 => { let n = 3 + r.below(4); let pts: Vec<[f64; 2]> = (0..n).map(|_| [x + r.f(-40.0, 40.0), y + r.f(-40.0, 40.0)]).collect(); shapes::new_poly(&pts, true, op) }
        _ => { let pts = [[x, y], [x + r.f(-40.0, 40.0), y + r.f(-40.0, 40.0)], [x + r.f(-40.0, 40.0), y + r.f(-40.0, 40.0)]]; shapes::new_poly(&pts, false, op) }
    }
}

/// What must hold after any action.
fn broken(a: &App) -> Vec<String> {
    let d = &a.project.data;
    let mut bad = vec![];
    let n = d.loops.len();
    if let Some(i) = a.sel.iter().find(|&&i| i >= n) { bad.push(format!("the selection holds shape {i} but there are {n}")); }
    let ids: Vec<i64> = d.ops.iter().map(|o| o.id).collect();
    let mut uniq = ids.clone(); uniq.sort_unstable(); uniq.dedup();
    if uniq.len() != ids.len() { bad.push(format!("operation ids repeat: {ids:?}")); }
    if let Some(l) = d.loops.iter().position(|l| l.op != 0 && !ids.contains(&l.op)) { bad.push(format!("shape {l} belongs to operation {} which does not exist", d.loops[l].op)); }
    if let Some(h) = d.hidden_ops.iter().find(|h| !ids.contains(h)) { bad.push(format!("hidden operation {h} does not exist")); }
    if d.act != 0 && !ids.contains(&d.act) { bad.push(format!("the active operation {} does not exist", d.act)); }
    for o in &d.ops {
        if let Some(t) = o.p.tab_pts.as_ref().and_then(|v| v.iter().find(|t| t.l >= n)) { bad.push(format!("operation {} has a tab on shape {} but there are {n}", o.id, t.l)); }
    }
    if let Some(l) = d.loops.iter().position(|l| l.grp.is_some_and(|g| !d.groups.iter().any(|x| x.id == g))) { bad.push(format!("shape {l} is in a group that does not exist")); }
    if let Some(l) = d.loops.iter().position(|l| l.pts.len() % 2 != 0 || l.pts.is_empty()) { bad.push(format!("shape {l} has {} coordinates", d.loops[l].pts.len())); }
    bad
}

/// Waits for the toolpath of the current project, like the main loop does between frames.
pub(crate) fn settle(a: &mut App) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    for _ in 0..30000 {
        a.poll_generation();
        if a.gen_pending && a.gen_rx.is_none() { a.start_generation(); }
        if !a.gen_stale() { break; }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    if t0.elapsed().as_secs() >= 5 {
        let d = &a.project.data;
        eprintln!("SLOW {:?}: stock {}x{}, {} shapes, ops {:?}", t0.elapsed(), d.stock.w, d.stock.h, d.loops.len(), d.ops.iter().map(|o| (o.p.kind.clone(), o.p.depth, o.p.tool.clone())).collect::<Vec<_>>());
        if let (Some(dir), Ok(j)) = (std::env::var_os("FUZZ_DUMP"), putiacnc_core::save_project(&a.project)) { let _ = std::fs::write(std::path::Path::new(&dir).join(format!("slow-{}.json", t0.elapsed().as_secs())), j); }
        for t in &d.tools { eprintln!("  tool {} {} dia {} so {} pd {}", t.id, t.name, t.dia, t.so, t.pd); }
    }
    if a.gen_stale() { return Err("the toolpath never settled".into()); }
    if let Some(o) = &a.output {
        if let Some(l) = o.lines.iter().find(|l| { let t = l.to_ascii_lowercase(); !t.starts_with('(') && (t.contains("nan") || t.contains("inf")) }) { return Err(format!("G-code with a non-number: {l}")); }
        if !o.time_min.is_finite() { return Err("the time is not finite".into()); }
    }
    Ok(())
}

fn text_of(s: &egui::Shape, out: &mut Vec<String>) {
    match s {
        egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
        egui::Shape::Vec(v) => for x in v { text_of(x, out); },
        _ => {}
    }
}

/// One frame of the window; returns what egui complained about.
fn frame(ctx: &egui::Context, a: &mut App, t: &mut f64) -> Vec<String> {
    *t += 0.05;
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1500.0, 900.0))), time: Some(*t), ..Default::default() };
    let out = ctx.run(input, |ctx| a.ui_frame(ctx));
    let mut texts = vec![];
    for c in &out.shapes { text_of(&c.shape, &mut texts); }
    texts.into_iter().filter(|t| t.contains('🔥')).collect()
}

const STEPS: usize = 40;

/// One random sequence. Returns the problems and the actions that led to them.
fn sequence(seed: u64, frames: bool, skip: &[usize]) -> Result<(), String> {
    let mut a = App::new(None);
    a.project.data.loops.clear();
    let ctx = egui::Context::default();
    ctx.options_mut(|o| o.warn_on_id_clash = true);
    let mut t = 0.0;
    let mut log: Vec<String> = vec![];
    let first = serde_json::to_string(&a.project.data).unwrap();
    let mut recorded = 0usize;
    let mut last_json = first.clone();
    for step in 0..STEPS {
        // each step has a random stream of its own, so leaving a step out does not change the others (that is what makes shrinking work)
        let mut r = Rng((seed.wrapping_mul(0x9E3779B97F4A7C15) ^ (step as u64 + 1).wrapping_mul(0xD1B54A32D192ED03)) | 1);
        for _ in 0..4 { r.next(); }
        if skip.contains(&step) { continue; }
        let (w, h) = (a.project.data.stock.w.max(10.0), a.project.data.stock.h.max(10.0));
        let nloops = a.project.data.loops.len();
        let act = r.below(30);
        let mut undoing = false;
        let desc = match act {
            0..=3 => {
                let op = if r.below(2) == 0 { a.project.data.ops.get(r.below(a.project.data.ops.len().max(1))).map_or(0, |o| o.id) } else { 0 };
                match random_loop(&mut r, w, h, op) { Some(l) => { a.add_loop(l); "draw a shape" } None => "draw (nothing)" }
            }
            4 | 5 => {
                let k = 1 + r.below(4);
                a.sel = (0..k).map(|_| r.below(nloops.max(1))).filter(|&i| i < nloops).collect();
                "select some shapes"
            }
            6 => { a.delete_selection(); "delete the selection" }
            7 => { a.duplicate_selection(); "duplicate the selection" }
            8 => { let (dx, dy) = (r.f(-20.0, 20.0), r.f(-20.0, 20.0)); a.nudge(dx, dy); "nudge" }
            9 => { let d = r.f(-8.0, 8.0); let keep = r.below(2) == 0; let _ = a.offset_selection(d, keep); "offset" }
            10 => { let op = [shapes::BoolOp::Union, shapes::BoolOp::Subtract, shapes::BoolOp::Intersect, shapes::BoolOp::Exclude][r.below(4)]; let (k, s) = (r.below(2) == 0, r.below(2) == 0); let _ = a.combine_selection(op, k, s); "combine" }
            11 => { a.group_selection(); "group" }
            12 => { a.ungroup_selection(); "ungroup" }
            13 | 14 => {
                let kinds: Vec<&str> = KINDS.iter().map(|k| k.0).filter(|k| *k != "relief").collect(); // not the 3D relief: it needs a model
                let kind = kinds[r.below(kinds.len())];
                a.hist.record(&a.project.data);
                let sel = a.sel.clone();
                let _ = shapes::add_operation(&mut a.project.data, &sel, kind);
                a.touch();
                "add an operation"
            }
            15 => {
                let d = &mut a.project.data;
                if d.ops.is_empty() { "remove an operation (none)" } else {
                    let i = r.below(d.ops.len());
                    a.hist.record(&a.project.data);
                    let d = &mut a.project.data;
                    let id = d.ops[i].id;
                    d.hidden_ops.retain(|&x| x != id);
                    for l in d.loops.iter_mut() { if l.op == id { l.op = 0; } }
                    d.ops.remove(i);
                    if d.act == id { d.act = d.ops.get(i.saturating_sub(1)).map(|o| o.id).unwrap_or(0); }
                    a.touch();
                    "remove an operation"
                }
            }
            16 => {
                if a.project.data.ops.is_empty() { "hide (none)" } else {
                    a.hist.record(&a.project.data);
                    let d = &mut a.project.data;
                    let id = d.ops[r.below(d.ops.len())].id;
                    if let Some(k) = d.hidden_ops.iter().position(|&x| x == id) { d.hidden_ops.remove(k); } else { d.hidden_ops.push(id); }
                    a.touch();
                    "hide or show an operation"
                }
            }
            17 => {
                a.hist.record(&a.project.data);
                let (nw, nh) = (r.f(20.0, 400.0), r.f(20.0, 300.0));
                a.project.data.stock.w = nw; a.project.data.stock.h = nh;
                a.touch();
                "change the stock size"
            }
            18 => {
                if a.project.data.ops.is_empty() { "use selected shapes (none)" } else {
                    a.hist.record(&a.project.data);
                    let id = a.project.data.ops[r.below(a.project.data.ops.len())].id;
                    let sel = a.sel.clone();
                    shapes::assign_to_op(&mut a.project.data.loops, &sel, id);
                    a.touch();
                    "use selected shapes in an operation"
                }
            }
            19 => { a.hist.record(&a.project.data); let sel = a.sel.clone(); shapes::add_inlay(&mut a.project.data, &sel); a.touch(); "inlay" }
            20 | 21 => { undoing = true; a.do_undo(); "undo" }
            22 => { undoing = true; a.do_redo(); "redo" }
            23 => { undoing = true; if r.below(2) == 0 { a.do_undo_drawing() } else { a.do_redo_drawing() }; "undo or redo in the toolbar" }
            24 => { a.sel = (0..nloops).collect(); "select all" }
            26 => {
                a.array.circular = r.below(2) == 0;
                a.array.cols = 1.0 + r.below(4) as f64; a.array.rows = 1.0 + r.below(3) as f64;
                a.array.count = 2.0 + r.below(6) as f64; a.array.gap_x = r.f(-3.0, 20.0); a.array.gap_y = r.f(-3.0, 20.0);
                a.array.angle = [360.0, 180.0, 90.0, -120.0][r.below(4)]; a.array.rotate = r.below(2) == 0;
                let _ = a.array_selection();
                "array"
            }
            27 => { let k = if r.below(2) == 0 { shapes::CornerKind::Fillet } else { shapes::CornerKind::Chamfer }; let size = r.f(0.2, 12.0); let _ = a.corner_selection(k, size); "round or bevel corners" }
            28 => { let _ = a.clean_drawing(); "clean up the drawing" }
            29 => { let (each, k) = (r.below(2) == 0, r.below(9) as u8); crate::align::align_selection(&mut a, each, k); "align" }
            _ => {
                a.tab = [RightTab::Ops, RightTab::Gcode, RightTab::Info][r.below(3)];
                if r.below(2) == 0 { a.toggle_tool_window([ToolWin::Align, ToolWin::Offset, ToolWin::Array, ToolWin::Corners, ToolWin::Combine, ToolWin::Clean][r.below(6)]); }
                "change tab or open a tool window"
            }
        };
        a.hist.classify(&a.project.data);
        log.push(format!("{step}: {desc} [undo {:?} redo {:?}]", a.hist.undo.iter().map(|e| e.kind.map_or('?', |k| match k { Kind::Drawing => 'D', Kind::Project => 'P', Kind::Both => 'B' })).collect::<String>(), a.hist.redo.iter().map(|e| e.kind.map_or('?', |k| match k { Kind::Drawing => 'D', Kind::Project => 'P', Kind::Both => 'B' })).collect::<String>()));
        let _ = undoing;
        if let Some(b) = broken(&a).first() { return Err(format!("{b}\n  after: {}", log.join(" | "))); }
        let json = serde_json::to_string(&a.project.data).unwrap();
        if !undoing && json != last_json { recorded += 1; }
        last_json = json;
        if frames && step % 3 == 2 {
            let clash = frame(&ctx, &mut a, &mut t);
            if !clash.is_empty() { return Err(format!("egui ID clash: {clash:?}\n  after: {}", log.join(" | "))); }
        }
        if step % 10 == 9 { settle(&mut a).map_err(|e| format!("{e}\n  after: {}", log.join(" | ")))?; }
    }
    settle(&mut a).map_err(|e| format!("{e}\n  after: {}", log.join(" | ")))?;
    // undo everything, then redo everything
    let last = serde_json::to_string(&a.project.data).unwrap();
    let mut n = 0;
    while a.hist.can_undo(Scope::All) && n < 1000 { a.do_undo(); n += 1; }
    let mut back: serde_json::Value = serde_json::from_str(&serde_json::to_string(&a.project.data).unwrap()).unwrap();
    let mut start: serde_json::Value = serde_json::from_str(&first).unwrap();
    // the machine is re-read from the machine list when history moves; it is not what is being compared
    for v in [&mut back, &mut start] { if let Some(o) = v.as_object_mut() { o.remove("machine"); o.remove("job"); } }
    if back != start { return Err(format!("undoing all {n} steps does not give the first project back (differs in: {})\n  after: {}", differs(&back, &start), log.join(" | "))); }
    let mut m = 0;
    // exactly as many as were undone: steps that were already waiting to be redone before are not part of this
    while a.hist.can_redo(Scope::All) && m < n { a.do_redo(); m += 1; }
    let mut again: serde_json::Value = serde_json::from_str(&serde_json::to_string(&a.project.data).unwrap()).unwrap();
    let mut end: serde_json::Value = serde_json::from_str(&last).unwrap();
    for v in [&mut again, &mut end] { if let Some(o) = v.as_object_mut() { o.remove("machine"); o.remove("job"); } }
    if again != end { return Err(format!("undo {n} times then redo {m} times does not give the last project back (differs in: {})\n  after: {}", differs(&again, &end), log.join(" | "))); }
    let _ = recorded;
    if frames { let clash = frame(&ctx, &mut a, &mut t); if !clash.is_empty() { return Err(format!("egui ID clash at the end: {clash:?}")); } }
    Ok(())
}

/// The top-level fields in which two projects differ, with a short look at both values.
fn differs(a: &serde_json::Value, b: &serde_json::Value) -> String {
    let (Some(x), Some(y)) = (a.as_object(), b.as_object()) else { return "?".into() };
    let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
    keys.sort(); keys.dedup();
    keys.into_iter().filter(|k| x.get(*k) != y.get(*k)).map(|k| {
        let cut = |v: Option<&serde_json::Value>| { let s = v.map_or("(none)".to_string(), |v| v.to_string()); s.chars().take(160).collect::<String>() };
        format!("{k}: {} vs {}", cut(x.get(k)), cut(y.get(k)))
    }).collect::<Vec<_>>().join("; ")
}

/// Runs a sequence, turning a panic into a message.
fn attempt(seed: u64, skip: &[usize]) -> Result<(), String> {
    match std::panic::catch_unwind(|| sequence(seed, true, skip)) {
        Ok(r) => r,
        Err(p) => Err(format!("PANIC {}", p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default())),
    }
}

/// Leaves out steps one at a time for as long as the sequence still goes wrong in the same way, so the report is short.
fn shrink(seed: u64, first: &str) -> (Vec<usize>, String) {
    if first.contains("never settled") { return (vec![], first.to_string()); }
    let kind = |e: &str| e.split(['(', '\n', ':']).next().unwrap_or("").chars().filter(|c| !c.is_ascii_digit()).collect::<String>();
    let want = kind(first);
    let mut skip: Vec<usize> = vec![];
    let mut msg = first.to_string();
    for step in 0..STEPS {
        let mut t = skip.clone(); t.push(step);
        if let Err(e) = attempt(seed, &t) { if kind(&e) == want { skip = t; msg = e; } }
    }
    (skip, msg)
}

fn run(seeds: std::ops::Range<u64>) {
    let mut report = vec![];
    for seed in seeds {
        if let Err(e) = attempt(seed, &[]) {
            let (skip, msg) = shrink(seed, &e);
            let steps: Vec<usize> = (0..STEPS).filter(|s| !skip.contains(s)).collect();
            report.push(format!("seed {seed}: {msg}\n  only steps {steps:?} are needed"));
        }
    }
    assert!(report.is_empty(), "{} sequence(s) went wrong:\n{}", report.len(), report.join("\n"));
}

#[test]
fn a_few_random_sequences_of_actions_leave_the_project_sound() { run(0..6); }

#[test]
#[ignore]
fn many_random_sequences_of_actions() {
    let n: u64 = std::env::var("COUNT").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
    let s: u64 = std::env::var("SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(100);
    run(s..s + n);
}
