//! Dialog audit: every dialog is drawn for real (in millimetres and in inches, on a roomy and on the smallest window) and checked for
//! the faults that keep coming back: a close button that is not at the right edge of its window, a window that leaves the screen,
//! text that runs out of its window or is cut by it, and texts drawn over each other.
use crate::app::App;
use eframe::egui::{self, Pos2, Rect};
use putiacnc_core::shapes;

/// What a dialog's header says about itself (written by `csd::dialog_header` in test builds).
#[derive(Clone, Debug)]
pub struct HeaderNote { pub title: String, pub layer: egui::Id, pub header: Rect, pub close: Rect }

pub fn note(ctx: &egui::Context, n: HeaderNote) {
    let id = egui::Id::new("audit_headers");
    ctx.data_mut(|d| { let mut v: Vec<HeaderNote> = d.get_temp(id).unwrap_or_default(); v.push(n); d.insert_temp(id, v); });
}

fn take_notes(ctx: &egui::Context) -> Vec<HeaderNote> {
    let id = egui::Id::new("audit_headers");
    ctx.data_mut(|d| { let v: Vec<HeaderNote> = d.get_temp(id).unwrap_or_default(); d.insert_temp(id, Vec::<HeaderNote>::new()); v })
}

fn project() -> App {
    let mut a = App::new(None);
    a.tab = crate::app::RightTab::Ops;
    a.project.data.loops.clear();
    a.project.data.loops.push(shapes::new_rect([10.0, 10.0], [90.0, 60.0], 0.0, 1));
    a.project.data.loops.push(shapes::new_circle([40.0, 40.0], 10.0, 1));
    let sel: std::collections::BTreeSet<usize> = (0..2).collect();
    let _ = shapes::add_operation(&mut a.project.data, &sel, "outside");
    a.touch();
    let _ = crate::fuzz::settle(&mut a);
    a
}

struct T { rect: Rect, text: String, clip: Rect }

/// Boxes that look like fields or buttons (a filled rounded rectangle of control size), with the clip they are drawn in.
fn boxes(shapes: &[egui::epaint::ClippedShape]) -> Vec<(Rect, Rect)> {
    fn walk(s: &egui::Shape, clip: Rect, out: &mut Vec<(Rect, Rect)>) {
        match s {
            egui::Shape::Rect(r) if r.fill != egui::Color32::TRANSPARENT && r.rect.height().between(16.0, 34.0) && r.rect.width().between(24.0, 400.0) => out.push((r.rect, clip)),
            egui::Shape::Vec(v) => for x in v { walk(x, clip, out); },
            _ => {}
        }
    }
    let mut out = vec![];
    for c in shapes { walk(&c.shape, c.clip_rect, &mut out); }
    out
}

fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<T> {
    fn walk(s: &egui::Shape, clip: Rect, out: &mut Vec<T>) {
        match s {
            egui::Shape::Text(t) => out.push(T { rect: t.galley.rect.translate(t.pos.to_vec2()), text: t.galley.text().to_string(), clip }),
            egui::Shape::Vec(v) => for x in v { walk(x, clip, out); },
            _ => {}
        }
    }
    let mut out = vec![];
    for c in shapes { walk(&c.shape, c.clip_rect, &mut out); }
    out
}

/// Draws `a` for a while at `screen` size and returns the problems of every dialog found.
fn audit(label: &str, a: &mut App, screen: egui::Vec2) -> Vec<String> {
    let ctx = egui::Context::default();
    let mut t = 0.0;
    let mut last = None;
    for _ in 0..10 {
        t += 0.1;
        take_notes(&ctx);
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)), time: Some(t), ..Default::default() };
        let out = ctx.run(input, |ctx| a.ui_frame(ctx));
        last = Some(out);
    }
    let out = last.unwrap();
    let notes = take_notes(&ctx);
    let mut bad = vec![];
    if notes.is_empty() { bad.push(format!("{label}: no dialog was drawn")); }
    let scr = Rect::from_min_size(Pos2::ZERO, screen);
    let all = texts(&out.shapes);
    for n in &notes {
        let at = |s: String| format!("{label} / {}: {s}", n.title);
        let Some(area) = ctx.memory(|m| m.area_rect(n.layer)) else { bad.push(at("no window".into())); continue };
        // the close button sits at the right end of the window's header, the header spans the window
        if (n.header.max.x - (area.max.x - 13.0)).abs() > 1.5 { bad.push(at(format!("the header ends at {:.0} but the window's inside ends at {:.0}", n.header.max.x, area.max.x - 13.0))); }
        if (n.header.min.x - (area.min.x + 13.0)).abs() > 1.5 { bad.push(at(format!("the header starts at {:.0} but the window's inside starts at {:.0}", n.header.min.x, area.min.x + 13.0))); }
        if !(area.max.x - n.close.center().x).between(20.0, 36.0) { bad.push(at(format!("the close button is {:.0} from the window's right edge", area.max.x - n.close.center().x))); }
        if area.min.x < scr.min.x - 1.0 || area.min.y < scr.min.y - 1.0 || area.max.x > scr.max.x + 1.0 || area.max.y > scr.max.y + 1.0 { bad.push(at(format!("the window {area:?} leaves the screen"))); }
        // the texts of this window: those clipped to something inside it
        // (the shapes come in paint order: those of the dialog follow its title)
        let from = all.iter().position(|t| t.text == n.title && area.expand(2.0).contains_rect(t.clip)).unwrap_or(all.len());
        let mine: Vec<&T> = all[from..].iter().filter(|t| area.expand(2.0).contains_rect(t.clip) && t.clip.width() > 1.0 && t.rect.intersects(area)).collect();
        for t in &mine {
            if t.text.trim().is_empty() { continue; }
            if t.rect.max.x > t.clip.max.x + 0.75 || t.rect.min.x < t.clip.min.x - 0.75 { bad.push(at(format!("\"{}\" is cut off ({:.0}..{:.0} in {:.0}..{:.0})", t.text, t.rect.min.x, t.rect.max.x, t.clip.min.x, t.clip.max.x))); }
            else if t.rect.max.x > area.max.x - 6.0 { bad.push(at(format!("\"{}\" runs to the window's edge ({:.0} of {:.0})", t.text, t.rect.max.x, area.max.x))); }
        }
        // fields in neighbouring rows whose edges miss each other by a few points (the same column drawn a little off)
        let mut fields: Vec<Rect> = vec![];
        for (r, clip) in boxes(&out.shapes) {
            if area.expand(2.0).contains_rect(clip) && area.contains_rect(r) && r.min.y > area.min.y + 40.0 && !fields.iter().any(|f| f.min.distance(r.min) < 0.5 && f.max.distance(r.max) < 0.5) { fields.push(r); }
        }
        // (the shortcut list is a flow of key caps, not columns of fields)
        if n.title.contains("shortcuts") { fields.clear(); }
        for (i, p) in fields.iter().enumerate() {
            for q in &fields[i + 1..] {
                let (hi, lo) = if p.min.y <= q.min.y { (p, q) } else { (q, p) };
                let gap = lo.min.y - hi.max.y;
                if !(-0.5..=70.0).contains(&gap) { continue; }
                // only boxes of the same width make a column of equal fields; buttons, key caps and wide fields differ in width
                if (hi.width() - lo.width()).abs() > 0.75 { continue; }
                for (a, b, side) in [(hi.min.x, lo.min.x, "left"), (hi.max.x, lo.max.x, "right")] {
                    let d = (a - b).abs();
                    if d > 0.75 && d <= 40.0 { bad.push(at(format!("fields at y {:.0} and {:.0} have their {side} edges {d:.1} apart ({a:.1} / {b:.1})", hi.min.y, lo.min.y))); }
                }
            }
        }
        for (i, p) in mine.iter().enumerate() {
            for q in &mine[i + 1..] {
                if p.text.trim().is_empty() || q.text.trim().is_empty() { continue; }
                let o = p.rect.intersect(q.rect);
                if o.is_positive() && o.width() > 2.0 && o.height() > 4.0 && p.rect.center() != q.rect.center() {
                    bad.push(at(format!("\"{}\" and \"{}\" overlap", p.text, q.text)));
                }
            }
        }
    }
    bad
}

trait Between { fn between(self, lo: f32, hi: f32) -> bool; }
impl Between for f32 { fn between(self, lo: f32, hi: f32) -> bool { self >= lo && self <= hi } }

type Setup = fn(&mut App);

fn dialogs() -> Vec<(&'static str, Setup)> {
    fn all_sel(a: &mut App) { a.sel = (0..a.project.data.loops.len()).collect(); }
    vec![
        ("about", |a| a.about_open = true),
        ("array", |a| { all_sel(a); a.array.open = true; }),
        ("array circle", |a| { all_sel(a); a.array.open = true; a.array.circular = true; }),
        ("box", |a| a.boxgen.open = true),
        ("box dogbones", |a| { a.boxgen.open = true; a.boxgen.dogbone = true; }),
        ("combine", |a| { all_sel(a); a.combine_open = true; }),
        ("clean up", |a| a.clean.open = true),
        ("fillet", |a| { all_sel(a); a.corners.open = true; }),
        ("export", |a| a.export_open = true),
        ("fingers", |a| { all_sel(a); a.fingers.open = true; }),
        ("machines", |a| a.machines_open = true),
        ("offset", |a| { all_sel(a); a.offset_open = true; }),
        ("shortcuts", |a| a.shortcuts_open = true),
        ("time", |a| a.time_open = true),
        ("tool library", |a| a.tools_open = true),
        ("text", |a| { a.mode = crate::app::Mode::Text; a.text.open = true; a.text.pos = Some([10.0, 20.0]); a.text.content = "Hello".into(); }),
        ("warnings", |a| a.checks_open = true),
        ("setup 1", |a| { a.setup.open = true; a.setup.step = 0; }),
        ("setup 2", |a| { a.setup.open = true; a.setup.step = 1; }),
        ("setup 3", |a| { a.setup.open = true; a.setup.step = 2; }),
        ("setup 4", |a| { a.setup.open = true; a.setup.step = 3; }),
    ]
}

#[test]
fn every_dialog_is_laid_out_properly() {
    let mut problems = vec![];
    for (name, setup) in dialogs() {
        for inches in [false, true] {
            for (sw, sh) in [(1500.0, 1000.0), (1354.0, 640.0)] {
                let mut a = project();
                a.disp_in = inches;
                a.frac_in = inches;
                setup(&mut a);
                problems.extend(audit(&format!("{name} ({}, {sw}x{sh})", if inches { "in" } else { "mm" }), &mut a, egui::vec2(sw, sh)));
            }
        }
    }
    problems.sort();
    problems.dedup();
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

/// A very long name (an imported file, a tool, a machine, the project) must be cut short where it is shown, not run out of its panel.
#[test]
fn long_names_do_not_run_out_of_the_panels() {
    let long = "75739279-nature-4-elements-circle-line-logo-sign-water-fire-earth-air-on-dark-background-and-then-some-more-words".to_string();
    let mut problems = vec![];
    for (tab, tname) in [(crate::app::RightTab::Ops, "ops"), (crate::app::RightTab::Gcode, "gcode"), (crate::app::RightTab::Info, "notes")] {
        for (sw, sh) in [(1500.0, 1000.0), (1354.0, 640.0)] {
            let mut a = project();
            a.tab = tab;
            a.project.name = long.clone();
            a.project.data.groups.push(putiacnc_core::model::ShapeGroup { id: 77, name: long.clone(), locked: false, text: None });
            for l in a.project.data.loops.iter_mut() { l.grp = Some(77); }
            for t in a.project.data.tools.iter_mut() { t.name = long.clone(); }
            for m in a.machines.list.iter_mut() { m.name = long.clone(); }
            a.touch();
            let _ = crate::fuzz::settle(&mut a);
            let ctx = egui::Context::default();
            let mut last = None;
            for i in 0..8 {
                let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(sw, sh))), time: Some(0.1 * (i + 1) as f64), ..Default::default() };
                last = Some(ctx.run(input, |ctx| a.ui_frame(ctx)));
            }
            let out = last.unwrap();
            // a long name used to push text out of the left panel (the panel itself has a fixed width, so the text ran over the drawing)
            for t in texts(&out.shapes) {
                if t.rect.min.y >= 70.0 && t.rect.min.x < 390.0 && t.rect.max.x > 396.0 && t.rect.min.y < sh - 60.0 { problems.push(format!("{tname} {sw}x{sh}: \"{}\" at {:.0},{:.0} reaches {:.0}, past the left panel's edge (394)", t.text.chars().take(30).collect::<String>(), t.rect.min.x, t.rect.min.y, t.rect.max.x)); }
                // (the project name field and a ruler's number are clipped by design, and the G-code view scrolls)
                if t.text.trim().is_empty() || t.clip.width() < 1.0 || t.rect.min.y < 70.0 || t.text.starts_with('(') || t.text.parse::<f64>().is_ok() { continue; }
                if t.rect.max.x > t.clip.max.x + 0.75 { problems.push(format!("{tname} {sw}x{sh}: \"{}\" at {:.0},{:.0} runs out of its panel ({:.0} > {:.0})", t.text.chars().take(40).collect::<String>(), t.rect.min.x, t.rect.min.y, t.rect.max.x, t.clip.max.x)); }
            }
        }
    }
    problems.sort(); problems.dedup();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The buttons of the tool windows follow one rule: a window that does its job once has OK and Cancel; one that is used again and again
/// has Apply (does it, stays open) and Close. They sit in one row, the first label first.
#[test]
fn tool_windows_have_the_agreed_buttons() {
    let rule: &[(&str, &[&str])] = &[
        ("box", &["OK", "Cancel"]), ("fingers", &["OK", "Cancel"]), ("combine", &["OK", "Cancel"]),
        ("offset", &["Apply", "Close"]), ("array", &["Apply", "Close"]), ("fillet", &["Apply", "Close"]),
    ];
    let mut problems = vec![];
    for (name, setup) in dialogs() {
        let Some((_, want)) = rule.iter().find(|(n, _)| *n == name) else { continue };
        let mut a = project();
        setup(&mut a);
        let ctx = egui::Context::default();
        let mut last = None;
        for i in 0..8 {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1500.0, 1000.0))), time: Some(0.1 * (i + 1) as f64), ..Default::default() };
            last = Some(ctx.run(input, |ctx| a.ui_frame(ctx)));
        }
        let all = texts(&last.unwrap().shapes);
        let find = |l: &str| all.iter().find(|t| t.text == l && t.rect.min.x > 400.0 && t.rect.min.x < 1060.0 && t.rect.min.y > 100.0);
        let found: Vec<_> = want.iter().map(|l| find(l)).collect();
        for (l, f) in want.iter().zip(&found) { if f.is_none() { problems.push(format!("{name}: no \"{l}\" button")); } }
        if found.iter().all(|f| f.is_some()) {
            let r: Vec<Rect> = found.iter().map(|f| f.unwrap().rect).collect();
            if (r[0].center().y - r[1].center().y).abs() > 3.0 { problems.push(format!("{name}: the buttons are not in one row")); }
            if r[0].min.x >= r[1].min.x { problems.push(format!("{name}: \"{}\" should come before \"{}\"", want[0], want[1])); }
        }
        for other in ["Apply", "OK", "Cancel", "Close", "Add box"] {
            if !want.contains(&other) && find(other).is_some_and(|t| t.rect.min.y > 150.0 && name != "box" || other == "Add box") && other != "Close" { problems.push(format!("{name}: has a \"{other}\" button it should not")); }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
