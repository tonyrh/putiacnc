//! Side panels, toolbar and status bar.
use putiacnc_core::i18n::{n_, tr, tr_f};
use crate::csd::ShowFollowing;
use crate::app::{fmt_time, Action, App, Level, RightTab};
use eframe::egui::{self, Color32, Ui};
use crate::icons::{icon_text_button, paint_icon, small_icon_button, Icon};
use crate::toolmgr::rlabel;
use putiacnc_core::shapes;
mod menu;
mod bottom;
mod dialogs;
mod project;
mod stock;
pub use self::menu::*;
pub use self::bottom::*;
pub use self::dialogs::*;
pub use self::project::*;
use self::stock::*;

/// The default font has no ″ glyph; show a plain quote instead.
pub(super) fn clean(s: &str) -> String { s.replace('″', "\"") }

pub const KINDS: &[(&str, &str)] = &[("outside", n_("Profile outside")), ("inside", n_("Profile inside")), ("online", n_("On line")), ("pocket", n_("Pocket")), ("vcarve", n_("V-carve")), ("drill", n_("Drill")), ("inlay", n_("Inlay pocket")), ("plug", n_("Inlay plug")), ("relief", n_("3D relief")), ("draw", n_("Draw (pen)"))];
pub(super) const PROFILES: &[(&str, &str)] = &[("outside", n_("Outside")), ("inside", n_("Inside")), ("online", n_("On"))];
pub(super) const DIRS: &[(&str, &str)] = &[("climb", n_("Climb")), ("conv", n_("Conventional"))];
pub(super) const ZEROS: &[(&str, &str)] = &[("ll", n_("Lower-left")), ("lr", n_("Lower-right")), ("ul", n_("Upper-left")), ("ur", n_("Upper-right")), ("c", n_("Center"))];
pub(super) const MATERIALS: &[(&str, &str)] = &[("wood", n_("Light wood")), ("darkwood", n_("Dark wood")), ("plywood", n_("Plywood")), ("mdf", "MDF"), ("acrylic", n_("Acrylic")), ("metal", n_("Metal")), ("foam", n_("Foam")), ("paper", n_("Paper"))];
/// The same two choices for a plug piece, with a shorter first name so that the box is no wider than the fields above it.
pub(super) const PIECE_ZMODES: &[(&str, &str)] = &[("top", n_("Top")), ("h", n_("Height"))];
pub(super) const ZMODES: &[(&str, &str)] = &[("top", n_("Stock top")), ("h", n_("Height"))];

pub fn label_of(opts: &[(&str, &str)], v: &str) -> String {
    opts.iter().find(|(k, _)| *k == v).map(|(_, l)| putiacnc_core::i18n::tr_dyn(l)).unwrap_or_else(|| v.to_string())
}

pub(super) fn tool_pd_of(tools: &[putiacnc_core::model::Tool], id: &str) -> Option<f64> { tools.iter().find(|t| t.id == id).map(|t| t.pd) }

/// Little up/down arrows (a spin button) for whole-number fields. Returns true when a value changed.
pub(super) fn spin_arrows(ui: &mut Ui, v: &mut f64, lo: f64, hi: f64) -> bool {
    let h = ui.spacing().interact_size.y;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, h), egui::Sense::hover());
    let mut changed = false;
    for (k, up) in [(0, true), (1, false)] {
        let r = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, k as f32 * h / 2.0), egui::vec2(16.0, h / 2.0));
        let resp = ui.interact(r, ui.id().with(("spin", k)), egui::Sense::click());
        let enabled = if up { *v < hi } else { *v > lo };
        if resp.hovered() && enabled { ui.painter().rect_filled(r.shrink(1.0), 3.0, ui.visuals().widgets.hovered.bg_fill); }
        let col = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color().gamma_multiply(0.5) };
        let c = r.center();
        let d = if up { -1.0_f32 } else { 1.0 };
        ui.painter().add(egui::Shape::convex_polygon(vec![c + egui::vec2(-3.5, -d * 1.5), c + egui::vec2(3.5, -d * 1.5), c + egui::vec2(0.0, d * 2.0)], col, egui::Stroke::NONE));
        if resp.clicked() && enabled { *v = (*v + if up { 1.0 } else { -1.0 }).clamp(lo, hi); changed = true; }
    }
    changed
}

pub(super) fn dv_w(ui: &mut Ui, w: f32, v: &mut f64, lo: f64, hi: f64, suffix: &str) -> bool {
    dv_r(ui, w, v, lo, hi, suffix).changed()
}

/// Like [`dv_w`], returning the field's response (to give it help).
pub(super) fn dv_r(ui: &mut Ui, w: f32, v: &mut f64, lo: f64, hi: f64, suffix: &str) -> egui::Response {
    crate::widgets::num(ui, [w, ui.spacing().interact_size.y], v, lo, hi, 0.25, 2, suffix)
}

/// Length versions of `dv_w` / `dv_r`: the value is in mm, the field shows the Display units.
pub(super) fn dl_w(ui: &mut Ui, w: f32, v: &mut f64, lo: f64, hi: f64, suffix: &str) -> bool { dl_r(ui, w, v, lo, hi, suffix).changed() }
pub(super) fn dl_r(ui: &mut Ui, w: f32, v: &mut f64, lo: f64, hi: f64, suffix: &str) -> egui::Response {
    crate::widgets::len(ui, [w, ui.spacing().interact_size.y], v, lo, hi, 0.25, 2, suffix)
}

pub(super) fn combo_w2(ui: &mut Ui, w: f32, id: &str, cur: &mut String, opts: &[(&str, &str)], prefix: &str, tip: &str) -> (bool, egui::Rect) {
    let mut ch = false;
    // a long name is cut short to the box, rather than the box growing past the card
    let shown = elide(ui, &format!("{prefix}{}", label_of(opts, cur)), (w - 30.0).max(40.0));
    let r = egui::ComboBox::from_id_source(id).width(w - 10.0).selected_text(shown).show_ui(ui, |ui| {
        for (k, l) in opts {
            if ui.selectable_label(cur == k, putiacnc_core::i18n::tr_dyn(l)).clicked() && cur != k { *cur = k.to_string(); ch = true; }
        }
    });
    if !tip.is_empty() { { use crate::widgets::Tip; r.response.clone().tip(tip); } }
    (ch, r.response.rect)
}

pub(super) fn combo_w(ui: &mut Ui, w: f32, id: &str, cur: &mut String, opts: &[(&str, &str)], prefix: &str, tip: &str) -> bool {
    combo_w2(ui, w, id, cur, opts, prefix, tip).0
}

pub fn right_panel(ui: &mut Ui, app: &mut App) {
    let theme = app.theme;
    crate::view3d::tabs_badged(ui, &theme, &mut app.tab, &[(RightTab::Ops, tr("Operations"), 0), (RightTab::Gcode, tr("G-code"), 0), (RightTab::Info, tr("Notes"), 0)]);
    ui.separator();
    if app.tab == RightTab::Ops { ops_panel(ui, app); return; }
    if app.tab == RightTab::Info { notes_panel(ui, app); return; }
    if app.tab == RightTab::Gcode {
        let (d, n) = (app.project.data.job.dialect_name(), app.project.data.job.numbered);
        ui.add(egui::Label::new(egui::RichText::new(tr_f("Output for {dialect}{numbered}. Dialect, units, line numbers and safe Z are set per machine (File → Machines).", &[("dialect", &d), ("numbered", &if n { tr(", numbered lines") } else { "" })])).weak()).wrap(true));
        ui.add_space(4.0);
        crate::exportui::wcs_pick(ui, app);
        ui.add_space(4.0);
        // plug pieces have a file of their own: choose which one to read
        if app.gcode_part > app.pieces.len() && !app.gen_stale() { app.gcode_part = 0; }
        if !app.pieces.is_empty() {
            if app.pieces.len() == 1 {
                let (c, _) = crate::zoomcard::seg2(ui, [tr("Main piece"), tr("Plug piece")], [tr("The G-code for the main stock"), tr("The G-code for the separate piece the plug is cut from (its own file, its own work zero)")], app.gcode_part.min(1));
                if let Some(i) = c { app.gcode_part = i; }
            } else {
                let names: Vec<String> = app.pieces.iter().map(|pp| tr_f("Plug piece (operation {n})", &[("n", &(pp.job.op_index + 1))])).collect();
                let cur = if app.gcode_part == 0 { tr("Main piece").to_string() } else { names[app.gcode_part - 1].clone() };
                egui::ComboBox::from_id_source("gcode_part_pick").selected_text(cur).show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.gcode_part, 0, tr("Main piece"));
                    for (i, n) in names.iter().enumerate() { ui.selectable_value(&mut app.gcode_part, i + 1, n.as_str()); }
                });
            }
            ui.add_space(4.0);
        }
    }
    let mut export = false;
    let mut copy: Option<String> = None;
    // the line the 3D preview is at is highlighted; a click on a line takes the preview there
    let cur = if app.v3.part == app.gcode_part { app.current_gcode_line() } else { None };
    let scroll_to = if cur != app.gcode_seen { app.gcode_seen = cur; cur } else { None };
    let mut clicked = None;
    match (&app.output, app.tab) {
        (Some(main), RightTab::Gcode) => {
            let o = app.gcode_part.checked_sub(1).and_then(|i| app.pieces.get(i)).map_or(main, |pp| &pp.output);
            let several = !app.pieces.is_empty();
            // what the program will do, in plain words; made again only when the G-code was
            let key = (app.out_ver, app.gcode_part);
            if app.summary.as_ref().map(|s| s.0) != Some(key) {
                let units = if app.imperial { putiacnc_core::Units::Imperial } else { putiacnc_core::Units::Metric };
                app.summary = Some((key, putiacnc_core::summary::describe(&app.project.data, o, units, app.gcode_part > 0)));
            }
            if let Some((_, rows)) = &app.summary {
                let warn_col = if ui.visuals().dark_mode { Color32::from_rgb(0xf8, 0xe4, 0x5c) } else { Color32::from_rgb(0xb5, 0x83, 0x00) };
                // on a short window the summary scrolls, so the G-code list and the buttons below it always keep their room
                let cap = (ui.available_height() - 210.0).max(90.0);
                egui::ScrollArea::vertical().id_source("gcode_summary_scroll").max_height(cap).auto_shrink([false, true]).show_following(ui, |ui| {
                egui::CollapsingHeader::new(egui::RichText::new(tr("What this will do")).strong()).id_source("gcode_summary").default_open(true).show(ui, |ui| {
                    let lw = rows.iter().map(|r| tw(ui, r.label)).fold(0.0_f32, f32::max);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    for r in rows {
                        // the label stays level with the first line of a text that wraps
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            ui.allocate_ui_with_layout(egui::vec2(lw, 0.0), egui::Layout::top_down(egui::Align::Max), |ui| { ui.set_width(lw); ui.label(r.label); });
                            let t = egui::RichText::new(&r.text);
                            ui.add(egui::Label::new(if r.warn { t.color(warn_col) } else { t }).wrap(true));
                        });
                    }
                });
                // what the generator chose or skipped, apart from the summary above (its warnings are in the warnings list)
                let infos: Vec<&String> = o.notes.iter().filter(|n| !n.starts_with("⚠ ")).collect();
                if !infos.is_empty() {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(2.0);
                    egui::CollapsingHeader::new(egui::RichText::new(tr("Notes on the toolpath")).strong()).id_source("gcode_notes").default_open(true).show(ui, |ui| {
                        for n in infos { crate::msg::line(ui, Level::Info, n); }
                    });
                }
                });
                ui.add_space(2.0);
                ui.separator();
                ui.label(egui::RichText::new(tr_f("G-code ({n} lines)", &[("n", &o.lines.len())])).strong());
                ui.add_space(2.0);
            }
            // Copy / Export sit at the bottom; the G-code takes the rest of the height
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr("Copy")).on_hover_text(if several { tr("Copy the G-code shown to the clipboard") } else { tr("Copy the G-code to the clipboard") }).clicked() { copy = Some(o.gcode()); }
                    if ui.button(tr("Export G-code…")).on_hover_text(if several { tr("Save the G-code to files: one for the main piece and one for each plug piece (Ctrl+E)") } else { tr("Save the G-code to a file (Ctrl+E)") }).clicked() { export = true; }
                });
                ui.add_space(6.0);
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
                    let mut area = egui::ScrollArea::both().auto_shrink([false, false]);
                    if let Some(c) = scroll_to {
                        let step = row_h + ui.spacing().item_spacing.y;
                        area = area.vertical_scroll_offset((c as f32 * step - 100.0).max(0.0));
                    }
                    let (hi, hover) = (ui.visuals().selection.bg_fill, ui.visuals().widgets.hovered.weak_bg_fill);
                    let col = ui.visuals().text_color();
                    area.show_rows(ui, row_h, o.lines.len(), |ui, range| {
                        for i in range {
                            let g = ui.painter().layout_no_wrap(o.lines[i].clone(), egui::TextStyle::Monospace.resolve(ui.style()), col);
                            let w = g.size().x.max(ui.available_width());
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(w, row_h), egui::Sense::click());
                            if cur == Some(i) { ui.painter().rect_filled(r, 2.0, hi); } else if resp.hovered() { ui.painter().rect_filled(r, 2.0, hover); }
                            ui.painter().galley(r.min, g, col);
                            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() { clicked = Some(i); }
                        }
                    });

                });
            });
        }
        (_, RightTab::Ops | RightTab::Info) => {}
        (None, _) if app.project.data.ops.is_empty() => { ui.weak(tr("Add an operation to generate G-code.")); }
        (None, _) => { ui.label(crate::widgets::localize(ui, &app.gen_error.clone().unwrap_or_else(|| "Nothing generated yet.".into()))); }
    }
    if let Some(t) = copy { ui.ctx().copy_text(t); }
    if export { app.export_gcode_dialog(); }
    if let Some(i) = clicked {
        app.v3.part = app.gcode_part;
        app.v3.goto_line = Some(i);
        app.show_in_3d();
    }
}

/// The Notes tab: the project's own notes (always editable, saved with the project). What the toolpath generator
/// has to say is in the G-code tab, under "Notes on the toolpath".
pub(super) fn notes_panel(ui: &mut Ui, app: &mut App) {
    ui.label(tr("Project notes"));
    let row_h = ui.text_style_height(&egui::TextStyle::Body);
    let rows = (((ui.available_height() - 12.0) / row_h).floor() as usize).max(4);
    let before = app.project.data.clone();
    let mut notes = std::mem::take(&mut app.project.data.notes);
    let r = ui.add(egui::TextEdit::multiline(&mut notes).desired_width(f32::INFINITY).desired_rows(rows).hint_text(tr("Material, fixtures, jobs to remember…")));
    app.project.data.notes = notes;
    if r.changed() {
        // one undo step per editing session; while the field has focus, Ctrl+Z undoes the typing itself
        if !app.hist.open { app.hist.push(before); app.hist.open = true; }
        app.modified = true;
    }
}

/// Work-origin picker: a square with a point at each corner and one in the centre.
/// `val` is one of ll, lr, ul, ur, c. Returns true when it changed.
pub fn tw(ui: &Ui, s: &str) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    ui.fonts(|f| f.layout_no_wrap(s.to_string(), font, Color32::WHITE).size().x).ceil()
}

/// Shorten `s` with an ellipsis so it fits in `max_w` pixels.
pub(super) fn elide(ui: &Ui, s: &str, max_w: f32) -> String {
    if tw(ui, s) <= max_w { return s.to_string(); }
    let mut t: Vec<char> = s.chars().collect();
    while !t.is_empty() {
        t.pop();
        let c: String = t.iter().collect::<String>().trim_end().to_string() + "…";
        if tw(ui, &c) <= max_w { return c; }
    }
    "…".into()
}

pub(super) fn xy_picker(ui: &mut Ui, val: &mut String) -> bool {
    let size = 76.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let vis = ui.visuals().clone();
    let accent = vis.selection.bg_fill;
    let sq = rect.shrink(8.0);
    let p = ui.painter();
    p.rect(sq, 3.0, vis.extreme_bg_color, egui::Stroke::new(1.5_f32, vis.widgets.inactive.fg_stroke.color));
    let pts = [("ul", sq.left_top()), ("ur", sq.right_top()), ("ll", sq.left_bottom()), ("lr", sq.right_bottom()), ("c", sq.center())];
    let hover = resp.hover_pos();
    let near = hover.and_then(|h| pts.iter().filter(|(_, q)| q.distance(h) < 16.0).min_by(|a, b| a.1.distance(h).total_cmp(&b.1.distance(h))).map(|(k, _)| *k));
    let mut changed = false;
    if resp.clicked() {
        if let Some(k) = near { if *val != k { *val = k.to_string(); changed = true; } }
    }
    for (k, q) in pts {
        let sel = *val == k;
        if sel {
            p.circle_filled(q, 6.5, accent);
            p.circle_stroke(q, 6.5, egui::Stroke::new(1.5_f32, Color32::WHITE));
        } else {
            let hot = near == Some(k);
            p.circle(q, 4.5, if hot { vis.widgets.hovered.bg_fill } else { vis.window_fill }, egui::Stroke::new(1.5_f32, if hot { accent } else { vis.widgets.inactive.fg_stroke.color }));
        }
    }
    if near.is_some() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    let name = ZEROS.iter().find(|(k, _)| *k == val.as_str()).map(|(_, n)| *n).unwrap_or("");
    resp.on_hover_text(tr_f("Work origin (X0 Y0): {name}", &[("name", &putiacnc_core::i18n::tr_dyn(name))]));
    changed
}

pub fn fmt_num(v: f64) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Dismissible message bar under the view tabs (import warnings, save errors...). Wraps at any width.
pub fn message_bar(ui: &mut Ui, app: &mut App) {
    let Some(msg) = app.error.clone() else { return };
    let level = app.banner_level_now();
    let dark = app.theme.dark;
    let col = crate::msg::colour(level, dark);
    let mut close = false;
    ui.horizontal_top(|ui| {
        ui.add_space(2.0);
        crate::msg::sign(ui, level);
        let w = (ui.available_width() - 34.0).max(60.0);
        ui.allocate_ui(egui::vec2(w, 0.0), |ui| { ui.set_width(w); ui.add(egui::Label::new(egui::RichText::new(crate::widgets::localize(ui, &msg)).color(col)).wrap(true)); });
        if icon_close(ui).clicked() { close = true; }
    });
    if close { app.error = None; }
}

/// The bar for clamp collisions: which operations run into which clamps, and a way to see where. It cannot be dismissed; it is there
/// while the toolpath hits a clamp. (The warnings bar's count and list still include these.)
pub fn clamp_bar(ui: &mut Ui, app: &mut App) {
    let col = crate::msg::colour(Level::Error, app.theme.dark);
    let mut ops: Vec<usize> = app.clamp_hits.iter().flat_map(|h| h.1.iter().copied()).collect();
    ops.sort_unstable(); ops.dedup();
    let mut clamps: Vec<usize> = app.clamp_hits.iter().map(|h| h.0).collect();
    clamps.sort_unstable(); clamps.dedup();
    let list = |v: &[usize]| -> String {
        let n: Vec<String> = v.iter().map(|x| (x + 1).to_string()).collect();
        match n.split_last() { None => String::new(), Some((last, [])) => last.clone(), Some((last, rest)) => tr_f("{list} and {last}", &[("list", &rest.join(", ")), ("last", last)]) }
    };
    let text = match (ops.len() == 1, clamps.len() == 1) {
        (true, true) => tr_f("Operation {ops} runs into clamp {clamps}. Move the clamp or change the drawing.", &[("ops", &list(&ops)), ("clamps", &list(&clamps))]),
        (true, false) => tr_f("Operation {ops} runs into clamps {clamps}. Move the clamp or change the drawing.", &[("ops", &list(&ops)), ("clamps", &list(&clamps))]),
        (false, true) => tr_f("Operations {ops} run into clamp {clamps}. Move the clamp or change the drawing.", &[("ops", &list(&ops)), ("clamps", &list(&clamps))]),
        (false, false) => tr_f("Operations {ops} run into clamps {clamps}. Move the clamp or change the drawing.", &[("ops", &list(&ops)), ("clamps", &list(&clamps))]),
    };
    // the first finding about a clamp, once the checks have listed them
    let first = app.checks.iter().position(|f| f.kind.about_clamps() && f.mv.is_some());
    let mut show = false;
    // the button at the right, the sign and the text in what is left; everything centred on the row's middle
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.add_enabled(first.is_some(), egui::Button::new(tr("Show in 3D"))).on_hover_text(tr("Jump the 3D preview to the first move that hits the clamp")).clicked() { show = true; }
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_space(2.0);
            crate::msg::sign(ui, Level::Error);
            ui.add(egui::Label::new(egui::RichText::new(&text).color(col)).wrap(true));
        });
    });
    if show { if let Some(i) = first { crate::checkui::show_in_3d(app, i); } }
}

pub(super) fn icon_close(ui: &mut Ui) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
    if resp.hovered() { ui.painter().rect_filled(r, 5.0, ui.visuals().widgets.hovered.weak_bg_fill); }
    paint_icon(ui.painter(), Icon::Close, r.center(), ui.visuals().text_color(), 0.55);
    resp.on_hover_text(tr("Dismiss"))
}

/// A number for a one-line summary: no trailing zeros.
pub(super) fn fnum(v: f64) -> String { let t = format!("{:.1}", v); t.trim_end_matches('0').trim_end_matches('.').to_string() }
