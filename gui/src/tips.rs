//! Hover help: a label shows the same help as the field beside it.
//!
//! The windows and the operation cards are drawn for real, every label and every control with help is found through the log that
//! `widgets::Tip` keeps in test builds, and then the pointer is put over each of them in turn: the help text has to come up for the
//! label and for the field, and for nothing else.
use crate::app::App;
use crate::widgets::tiplog;
use eframe::egui::{self, Pos2, Rect};
use putiacnc_core::shapes;

const SCREEN: egui::Vec2 = egui::vec2(1500.0, 1700.0);

struct Rig { ctx: egui::Context, t: f64, pointer: Option<Pos2> }

impl Rig {
    fn new() -> Self { Rig { ctx: egui::Context::default(), t: 0.0, pointer: None } }

    /// One frame; the pointer is where `pointer` says. Returns the texts drawn.
    fn frame(&mut self, a: &mut App, dt: f64) -> Vec<String> {
        self.t += dt;
        let events = vec![match self.pointer { Some(p) => egui::Event::PointerMoved(p), None => egui::Event::PointerGone }];
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(self.t), events, ..Default::default() };
        let out = self.ctx.run(input, |ctx| a.ui_frame(ctx));
        let mut texts = vec![];
        for c in &out.shapes { text_of(&c.shape, &mut texts); }
        texts
    }

    fn settle(&mut self, a: &mut App) { for _ in 0..8 { self.frame(a, 0.1); } }

    /// The log of one frame.
    fn log(&mut self, a: &mut App) -> tiplog::Log {
        tiplog::take(&self.ctx);
        self.frame(a, 0.1);
        tiplog::take(&self.ctx)
    }

    /// Everything drawn after the pointer has rested on `p` for a while.
    fn hover(&mut self, a: &mut App, p: Pos2) -> Vec<String> {
        self.pointer = Some(p);
        let mut last = vec![];
        for _ in 0..5 { last = self.frame(a, 0.2); }
        last
    }

    fn away(&mut self, a: &mut App) { self.pointer = None; for _ in 0..3 { self.frame(a, 0.2); } }
}

/// Where each text was drawn (the middle of it), for finding words that no widget logs.
fn spots(s: &egui::Shape, out: &mut Vec<(Pos2, String)>) {
    match s {
        egui::Shape::Text(t) => out.push((t.pos + t.galley.rect.center().to_vec2(), t.galley.text().to_string())),
        egui::Shape::Vec(v) => for x in v { spots(x, out); },
        _ => {}
    }
}

fn text_of(s: &egui::Shape, out: &mut Vec<String>) {
    match s {
        egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
        egui::Shape::Vec(v) => for x in v { text_of(x, out); },
        _ => {}
    }
}

fn project() -> App {
    let mut a = App::new(None);
    a.tab = crate::app::RightTab::Ops;
    a.project.data.loops.clear();
    a.project.data.loops.push(shapes::new_circle([40.0, 40.0], 10.0, 1));
    a.project.data.loops.push(shapes::new_circle([80.0, 40.0], 10.0, 1));
    a
}

/// Makes the last operation the active one, with its shapes selected and the others folded: its card is open.
fn open_last_op(a: &mut App) {
    a.touch();
    if let Some(id) = a.project.data.ops.last().map(|o| o.id) {
        a.project.data.act = id;
        a.sel = a.project.data.loops.iter().enumerate().filter(|(_, l)| l.op == id).map(|(i, _)| i).collect();
        a.collapsed = a.project.data.ops.iter().map(|o| o.id).filter(|&o| o != id).collect();
    }
}

fn show(texts: &[String], tip: &str) -> bool {
    // the whole text of the tooltip; a part of it would also match a sentence on the page
    texts.iter().any(|t| t == tip)
}

/// A tip may be wrapped over lines; this is its first words.
fn short(tip: &str) -> String { tip.chars().take(60).collect() }

/// Checks one scene. Returns (pairs checked, labels without help, controls with help that no label was paired with).
fn check_scene(name: &str, a: &mut App, strict: bool) -> (usize, Vec<String>, Vec<String>) {
    let mut rig = Rig::new();
    rig.settle(a);
    let log = rig.log(a);
    let screen = Rect::from_min_size(Pos2::ZERO, SCREEN);
    if std::env::var_os("TIPS_DUMP").is_some() {
        for (f, l, t) in &log.fields { eprintln!("  field {:?} label {:?} \"{}\"", f.center(), l.map(|l| l.center()), short(t)); }
        for (l, t) in &log.labels { eprintln!("  label {:?} {t}", l.center()); }
    }
    let mut problems: Vec<String> = vec![];
    let mut checked = 0;
    let mut hidden = 0;
    let mut paired_labels: Vec<Rect> = vec![];

    for (field, label, tip) in &log.fields {
        if !screen.contains_rect(*field) { continue; }
        // a field the pointer cannot reach (clipped by a scroll area, hidden under another window) has nothing to check
        rig.away(a);
        let on_field = rig.hover(a, field.center());
        if !show(&on_field, tip) {
            // a control scrolled out of a window's view cannot be reached by the pointer: then its label cannot either, and that is all right
            let label_shows = label.is_some_and(|l| { rig.away(a); let t = rig.hover(a, l.center()); show(&t, tip) });
            if label_shows { problems.push(format!("[{name}] the label shows \"{}\" and the control at {:?} does not", short(tip), field.center())); }
            else if label.is_none() { problems.push(format!("[{name}] hovering the control at {:?} did not show \"{}\"", field.center(), short(tip))); }
            else { hidden += 1; }
            continue;
        }
        if let Some(l) = label {
            paired_labels.push(*l);
            rig.away(a);
            let on_label = rig.hover(a, l.center());
            if !show(&on_label, tip) { problems.push(format!("[{name}] hovering the label at {:?} did not show \"{}\"", l.center(), short(tip))); }
            // the label must show its own help and not that of its neighbours
            for (_, _, other) in log.fields.iter().filter(|(_, ol, _)| *ol != Some(*l)) {
                if other != tip && show(&on_label, other) { problems.push(format!("[{name}] the label at {:?} shows another control's help: \"{}\"", l.center(), short(other))); }
            }
            checked += 1;
        }
    }
    for (r, text) in &log.painted {
        rig.away(a);
        let t = rig.hover(a, r.center());
        if !log.fields.iter().any(|(_, _, tip)| show(&t, tip)) { problems.push(format!("[{name}] the painted text \"{text}\" shows no help")); }
        checked += 1;
    }
    rig.away(a);
    let nothing = rig.hover(a, Pos2::new(3.0, 3.0));
    for (_, _, tip) in &log.fields { if show(&nothing, tip) { problems.push(format!("[{name}] help shows with the pointer in a corner: \"{}\"", short(tip))); } }

    // controls with help that sit right after a label which did not get it: the pairing missed a row
    let mut unpaired_fields = vec![];
    for (field, label, tip) in &log.fields {
        if label.is_some() { continue; }
        let near = log.labels.iter().find(|(l, _)| !paired_labels.contains(l) && l.right() <= field.left() + 1.0 && field.left() - l.right() <= 60.0 && l.center().y >= field.top() - 3.0 && l.center().y <= field.bottom() + 3.0);
        if let Some((l, text)) = near {
            let m = format!("[{name}] the control with \"{}\" has the label \"{text}\" at {:?} beside it, and the label did not get the help", short(tip), l.center());
            if strict { problems.push(m); } else { unpaired_fields.push(m); }
        }
    }
    let without: Vec<String> = log.labels.iter().filter(|(l, _)| !paired_labels.contains(l) && screen.contains_rect(*l)).map(|(_, t)| format!("[{name}] {t}")).collect();
    if std::env::var_os("TIPS_REPORT").is_some() {
        eprintln!("{name}: {} labels, {} controls with help, {} pairs checked, {} out of reach (scrolled away)", log.labels.len(), log.fields.len(), checked, hidden);
        for w in &without { eprintln!("   no help: {w}"); }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    (checked, without, unpaired_fields)
}

// ---- the helper on its own -------------------------------------------------------------------------------------------------

#[test]
fn a_label_takes_the_help_of_the_field_right_after_it() {
    use crate::widgets::{label_cell, Tip};
    let ctx = egui::Context::default();
    let mut t = 0.0;
    let mut v = 1.0_f64;
    let mut pos: Option<Pos2>;
    let mut seen: Vec<String> = vec![];
    let mut rects: Vec<Rect> = vec![];
    let mut run = |pos: Option<Pos2>, t: &mut f64, rects: &mut Vec<Rect>| -> Vec<String> {
        *t += 0.2;
        let ev = vec![match pos { Some(p) => egui::Event::PointerMoved(p), None => egui::Event::PointerGone }];
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 400.0))), time: Some(*t), events: ev, ..Default::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::Grid::new("g").num_columns(2).show(ui, |ui| {
                    let l = label_cell(ui, 100.0, "One"); rects.clear(); rects.push(l.rect);
                    crate::widgets::num(ui, [60.0, 22.0], &mut v, 0.0, 10.0, 0.1, 1, "").tip("help one"); ui.end_row();
                    let l = label_cell(ui, 100.0, "Two"); rects.push(l.rect);
                    crate::widgets::num(ui, [60.0, 22.0], &mut v, 0.0, 10.0, 0.1, 1, ""); ui.end_row();
                    let l = label_cell(ui, 100.0, "Three"); rects.push(l.rect);
                    ui.add_space(0.0);
                    crate::widgets::num(ui, [60.0, 22.0], &mut v, 0.0, 10.0, 0.1, 1, "").tip("help three"); ui.end_row();
                });
            });
        });
        let mut texts = vec![];
        for c in &out.shapes { text_of(&c.shape, &mut texts); }
        texts
    };
    for _ in 0..3 { run(None, &mut t, &mut rects); }
    let (one, two, three) = (rects[0], rects[1], rects[2]);
    pos = Some(one.center());
    for _ in 0..4 { seen = run(pos, &mut t, &mut rects); }
    assert!(seen.iter().any(|s| s == "help one"), "label One: {seen:?}");
    pos = Some(two.center());
    for _ in 0..4 { seen = run(pos, &mut t, &mut rects); }
    assert!(!seen.iter().any(|s| s.starts_with("help")), "label Two has a field without help, so none: {seen:?}");
    pos = Some(three.center());
    for _ in 0..4 { seen = run(pos, &mut t, &mut rects); }
    assert!(seen.iter().any(|s| s == "help three") && !seen.iter().any(|s| s == "help one"), "label Three: {seen:?}");
    let _ = pos;
}

#[test]
fn two_pairs_on_one_row_keep_their_own_help() {
    use crate::widgets::{label_cell, Tip};
    let ctx = egui::Context::default();
    let mut v = 1.0_f64;
    let mut t = 0.0;
    let mut labels: Vec<Rect> = vec![];
    let mut fields: Vec<Rect> = vec![];
    let mut go = |p: Option<Pos2>, t: &mut f64, labels: &mut Vec<Rect>, fields: &mut Vec<Rect>| -> Vec<String> {
        *t += 0.2;
        let ev = vec![match p { Some(p) => egui::Event::PointerMoved(p), None => egui::Event::PointerGone }];
        let out = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(700.0, 300.0))), time: Some(*t), events: ev, ..Default::default() }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    labels.clear(); fields.clear();
                    labels.push(label_cell(ui, 80.0, "Columns").rect);
                    fields.push(crate::widgets::num(ui, [50.0, 22.0], &mut v, 0.0, 10.0, 0.1, 1, "").tip("columns help").rect);
                    labels.push(label_cell(ui, 80.0, "Rows").rect);
                    fields.push(crate::widgets::num(ui, [50.0, 22.0], &mut v, 0.0, 10.0, 0.1, 1, "").tip("rows help").rect);
                });
            });
        });
        let mut texts = vec![];
        for c in &out.shapes { text_of(&c.shape, &mut texts); }
        texts
    };
    for _ in 0..3 { go(None, &mut t, &mut labels, &mut fields); }
    let (l0, l1) = (labels[0], labels[1]);
    let mut seen = vec![];
    for _ in 0..4 { seen = go(Some(l1.center()), &mut t, &mut labels, &mut fields); }
    assert!(seen.iter().any(|s| s == "rows help") && !seen.iter().any(|s| s == "columns help"), "{seen:?}");
    for _ in 0..4 { seen = go(Some(l0.center()), &mut t, &mut labels, &mut fields); }
    assert!(seen.iter().any(|s| s == "columns help") && !seen.iter().any(|s| s == "rows help"), "{seen:?}");
}

#[test]
fn a_label_in_an_earlier_frame_or_on_another_row_is_never_paired() {
    use crate::widgets::{label_cell, Tip};
    let ctx = egui::Context::default();
    let mut v = 1.0_f64;
    // the label is drawn in one frame, the control in the next: no pair
    let mut lrect = Rect::NOTHING;
    let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))), time: Some(0.1), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| { lrect = label_cell(ui, 80.0, "Alone").rect; });
    });
    tiplog::take(&ctx);
    let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))), time: Some(0.2), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| { ui.add_space(lrect.width() + 4.0); crate::widgets::num(ui, [50.0, 22.0], &mut v, 0.0, 10.0, 0.1, 1, "").tip("stray help"); });
        });
    });
    let log = tiplog::take(&ctx);
    assert!(log.fields.iter().all(|(_, l, _)| l.is_none()), "{log:?}");
    // a label on one row and a control on the next row
    let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))), time: Some(0.3), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            label_cell(ui, 80.0, "Row one");
            ui.end_row();
            ui.add_space(60.0);
            crate::widgets::num(ui, [50.0, 22.0], &mut v, 0.0, 10.0, 0.1, 1, "").tip("row two help");
        });
    });
    let log = tiplog::take(&ctx);
    assert!(log.fields.iter().all(|(_, l, _)| l.is_none()), "{log:?}");
}

// ---- the program's own windows ---------------------------------------------------------------------------------------------

#[test]
fn the_machines_window_shows_help_on_labels_and_fields() {
    let mut a = project();
    a.machines_open = true;
    let r = check_scene("machines", &mut a, true);
    assert!(r.0 >= 5, "only {} pairs found", r.0);
}

/// The three texts of the machines window that are not drawn by the shared label: they show the help of their control.
#[test]
fn the_texts_next_to_the_machines_checkboxes_show_their_help() {
    let mut a = project();
    a.machines_open = true;
    let id = a.project.data.machine.as_ref().map(|m| m.id.clone()).unwrap_or_else(|| a.machines.def.clone());
    if let Some(m) = a.machines.list.iter_mut().find(|m| m.id == id) { m.holder_on = true; m.holder = Some(30.0); }
    if let Some(m) = a.project.data.machine.as_mut() { m.holder_on = true; m.holder = Some(30.0); }
    let mut rig = Rig::new();
    rig.settle(&mut a);
    rig.pointer = None;
    let out = rig.ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(9.0), ..Default::default() }, |ctx| a.ui_frame(ctx));
    let mut at = vec![];
    for c in &out.shapes { spots(&c.shape, &mut at); }
    for (word, tip) in [
        ("Time correction ×", "Multiplies the estimated time to account for acceleration and corners"),
        ("Line numbers", "Number the G-code lines (N10, N20…), for controllers that want them"),
        ("Check it against the clamps", "Off by default. When on, give the holder's diameter here and each tool's stick-out in the tool manager: the warnings and the 3D preview then check the holder against the clamps and draw it."),
    ] {
        let p = at.iter().find(|(_, t)| t == word).unwrap_or_else(|| panic!("\"{word}\" is not drawn")).0;
        rig.away(&mut a);
        let shown = rig.hover(&mut a, p);
        assert!(show(&shown, tip), "hovering \"{word}\" at {p:?} shows no help");
    }
}

#[test]
fn the_machines_window_with_the_holder_on_shows_its_rows_too() {
    let mut a = project();
    a.machines_open = true;
    let id = a.project.data.machine.as_ref().map(|m| m.id.clone()).unwrap_or_else(|| a.machines.def.clone());
    if let Some(m) = a.machines.list.iter_mut().find(|m| m.id == id) { m.holder_on = true; m.holder = Some(30.0); }
    if let Some(m) = a.project.data.machine.as_mut() { m.holder_on = true; m.holder = Some(30.0); }
    let r = check_scene("machines-holder", &mut a, true);
    assert!(r.0 >= 6, "only {} pairs found", r.0);
}

#[test]
fn the_tool_window_shows_help_on_labels_and_fields() {
    let mut a = project();
    a.tools_open = true;
    let r = check_scene("tools", &mut a, true);
    assert!(r.0 >= 3, "only {} pairs found", r.0);
}

#[test]
fn every_tool_shape_in_the_tool_window() {
    let ids: Vec<String> = project().project.data.tools.iter().map(|t| t.id.clone()).collect();
    assert!(!ids.is_empty());
    for (i, id) in ids.iter().enumerate() {
        let mut a = project();
        a.tools_open = true;
        a.tool_sel = id.clone();
        check_scene(&format!("tool-{i}"), &mut a, true);
    }
}

#[test]
fn the_small_dialogs_show_help_on_labels_and_fields() {
    for (name, setup) in [
        ("array", (|a: &mut App| { a.array.open = true; a.sel = (0..a.project.data.loops.len()).collect(); }) as fn(&mut App)),
        ("corners", |a: &mut App| { a.corners.open = true; a.sel = (0..a.project.data.loops.len()).collect(); }),
        ("clean", |a: &mut App| { a.clean.open = true; }),
        ("offset", |a: &mut App| { a.offset_open = true; a.sel = (0..a.project.data.loops.len()).collect(); }),
    ] {
        let mut a = project();
        setup(&mut a);
        check_scene(name, &mut a, true);
    }
}

#[test]
fn every_operation_card_shows_help_on_labels_and_fields() {
    for kind in ["outside", "inside", "online", "pocket", "vcarve", "drill", "inlay", "plug"] {
        let mut a = project();
        a.sel = (0..a.project.data.loops.len()).collect();
        let sel = a.sel.clone();
        let _ = shapes::add_operation(&mut a.project.data, &sel, kind);
        open_last_op(&mut a);
        check_scene(&format!("op-{kind}"), &mut a, true);
    }
}

#[test]
fn the_operation_card_with_its_options_open() {
    for kind in ["pocket", "outside", "plug"] {
        let mut a = project();
        a.sel = (0..a.project.data.loops.len()).collect();
        let sel = a.sel.clone();
        let _ = shapes::add_operation(&mut a.project.data, &sel, kind);
        if let Some(o) = a.project.data.ops.last_mut() {
            o.p.pause = Some("1".into());
            o.p.air = true;
            o.p.lead = Some(2.0);
            o.p.tabs = true;
        }
        open_last_op(&mut a);
        check_scene(&format!("op-open-{kind}"), &mut a, true);
    }
}

#[test]
fn the_text_and_trace_windows_show_help_on_labels_and_fields() {
    let mut a = project();
    a.mode = crate::app::Mode::Text;
    a.text.open = true;
    a.text.pos = Some([10.0, 20.0]);
    a.text.content = "Hello".into();
    check_scene("text", &mut a, true);
    let mut a = project();
    a.trace.open = true;
    check_scene("trace", &mut a, true);
}

/// The pass depth of an operation explains itself on its label and on its field (a peck for a drill).
#[test]
fn the_pass_depth_has_help_on_its_label_and_its_field() {
    for (kind, word, tip) in [
        ("pocket", "Pass", "Maximum depth of a single pass: the cut goes down in passes of this depth until the full depth is reached"),
        ("outside", "Pass", "Maximum depth of a single pass: the cut goes down in passes of this depth until the full depth is reached"),
        ("drill", "Peck", "Depth of one peck: the drill goes this deep, lifts to clear the chips and goes in again until the full depth is reached"),
    ] {
        let mut a = project();
        a.sel = (0..a.project.data.loops.len()).collect();
        let sel = a.sel.clone();
        let _ = shapes::add_operation(&mut a.project.data, &sel, kind);
        open_last_op(&mut a);
        let mut rig = Rig::new();
        rig.settle(&mut a);
        tiplog::take(&rig.ctx);
        rig.pointer = None;
        let out = rig.ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(9.0), ..Default::default() }, |ctx| a.ui_frame(ctx));
        let log = tiplog::take(&rig.ctx);
        let mut at = vec![];
        for c in &out.shapes { spots(&c.shape, &mut at); }
        let label = at.iter().find(|(_, t)| t == word).unwrap_or_else(|| panic!("[{kind}] \"{word}\" is not drawn")).0;
        let field = log.fields.iter().find(|(_, _, t)| t == tip).unwrap_or_else(|| panic!("[{kind}] no control has the pass depth help")).0;
        for (what, p) in [("label", label), ("field", field.center())] {
            rig.away(&mut a);
            let shown = rig.hover(&mut a, p);
            assert!(show(&shown, tip), "[{kind}] hovering the {what} at {p:?} shows no help");
        }
    }
}

/// A font name too long for its row in the font list is cut with an ellipsis and fits; a short one is left alone.
#[test]
fn a_long_name_is_cut_to_fit_its_row() {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 300.0))), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let font = egui::TextStyle::Button.resolve(ui.style());
            let width = |s: &str| ui.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), egui::Color32::WHITE).size().x);
            for name in ["Noto Sans Tamil Condensed Medium Condensed", "Noto Sans Tamil ExtraCondensed SemiBold Condensed", "A", "", "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW"] {
                for max in [60.0_f32, 150.0, 260.0] {
                    let cut = crate::widgets::elide_text(ui, name, max);
                    assert!(width(&cut) <= max + 0.01, "\"{cut}\" is {} wide, room for {max}", width(&cut));
                    if width(name) <= max { assert_eq!(cut, name); } else { assert!(cut.ends_with('…') && cut.len() > 1 || cut == "…", "{cut:?}"); }
                }
            }
        });
    });
}
