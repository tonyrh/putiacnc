//! Clean up drawing: join the pieces of broken outlines, remove duplicates and specks, and show the open ends.
use putiacnc_core::i18n::{tr, tr_f, tr_n};
use crate::widgets::Tip;
use crate::app::App;
use eframe::egui;
use putiacnc_core::cleanup::{self, Options};
use putiacnc_core::shapes;

pub struct CleanTool { pub open: bool, pub join: bool, pub dups: bool, pub tiny: bool, pub tol: f64, pub simplify: bool, pub simp_tol: f64, pub msg: String }

impl Default for CleanTool { fn default() -> Self { CleanTool { open: false, join: true, dups: true, tiny: true, tol: 0.05, simplify: false, simp_tol: 0.02, msg: String::new() } } }

impl App {
    /// The shapes cleaning works on: the selection, or the whole drawing when nothing is selected, without the shapes of a locked layer
    /// or locked drawing and without those on a hidden layer (what cannot be seen is not tidied).
    pub fn clean_scope(&self) -> std::collections::BTreeSet<usize> {
        let all: Vec<usize> = if self.sel.is_empty() { (0..self.project.data.loops.len()).collect() } else { self.sel.iter().copied().collect() };
        all.into_iter().filter(|&i| i < self.project.data.loops.len() && !self.loop_locked(i) && !self.loop_hidden(i)).collect()
    }

    /// The shapes Simplify may change: the cleaning scope without the letters of placed text (they stay as the font draws them).
    pub fn simplify_scope(&self) -> std::collections::BTreeSet<usize> {
        let d = &self.project.data;
        self.clean_scope().into_iter().filter(|&i| !d.loops[i].grp.is_some_and(|g| d.groups.iter().any(|x| x.id == g && x.text.is_some()))).collect()
    }

    /// Cleans the selection (or the whole drawing when nothing is selected).
    pub fn clean_drawing(&mut self) -> String {
        let t = &self.clean;
        let o = Options { join: t.join, duplicates: t.dups, tiny: t.tiny, tol: t.tol.max(1e-4) };
        let scope = self.clean_scope();
        if scope.is_empty() { return tr("Nothing to clean up: the shapes are locked or hidden.").into(); }
        // simplify first, while the shapes still have their numbers; joining then works on the simplified outlines
        let mut base = self.project.data.loops.clone();
        let simplified = if t.simplify { cleanup::simplify(&mut base, &self.simplify_scope(), t.simp_tol.max(1e-4)) } else { 0 };
        let (res, removed, rep) = cleanup::clean(&base, Some(&scope), o);
        let changed = simplified > 0 || !removed.is_empty() || res.len() != self.project.data.loops.len() || rep.closed > 0 || rep.joined > 0
            || res.iter().zip(&self.project.data.loops).any(|(a, b)| a.pts != b.pts || a.c != b.c);
        if !changed { return tr("Nothing to clean up.").into(); }
        self.hist.record(&self.project.data);
        // the selection follows its shapes through the renumbering
        let sel: std::collections::BTreeSet<usize> = self.sel.iter().filter(|i| !removed.contains(i)).map(|&i| i - removed.iter().filter(|&&r| r < i).count()).collect();
        shapes::tabs_after_delete(&mut self.project.data.ops, &removed);
        self.project.data.loops = res;
        self.sel = sel;
        self.touch();
        let mut parts: Vec<String> = Vec::new();
        if rep.joined > 0 { parts.push(tr_n(rep.joined as u64, "joined {n} piece", "joined {n} pieces")); }
        if rep.closed > 0 { parts.push(tr_n(rep.closed as u64, "closed {n} outline", "closed {n} outlines")); }
        if rep.duplicates > 0 { parts.push(tr_n(rep.duplicates as u64, "removed {n} duplicate", "removed {n} duplicates")); }
        if rep.tiny > 0 { parts.push(tr_n(rep.tiny as u64, "removed {n} empty shape", "removed {n} empty shapes")); }
        if simplified > 0 { parts.push(tr_n(simplified as u64, "simplified outlines ({n} point removed)", "simplified outlines ({n} points removed)")); }
        let mut m = parts.join(", ");
        if let Some(c) = m.chars().next() { m = c.to_uppercase().collect::<String>() + &m[c.len_utf8()..]; }
        m.push('.');
        if rep.open_left > 0 { m.push(' '); m.push_str(&tr_n(rep.open_left as u64, "{n} open contour left.", "{n} open contours left.")); }
        m
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.clean.open { return; }
    let scope: Vec<&putiacnc_core::model::LoopData> = app.clean_scope().into_iter().filter_map(|i| app.project.data.loops.get(i)).collect();
    let owned: Vec<putiacnc_core::model::LoopData> = scope.into_iter().cloned().collect();
    let open = cleanup::open_count(&owned);
    let ends = cleanup::loose_ends(&owned, app.clean.tol);
    let joinable = ends.iter().filter(|e| e.3).count();
    let n = if app.sel.is_empty() { None } else { Some(app.sel.len()) };
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Clean up drawing").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Clean up drawing", crate::csd::anchor(app))).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(crate::csd::dialog_w(376.0));
        if crate::csd::dialog_header_help(ui, tr("Clean up drawing"), true, |ui| { ui.label(tr("Rectangles, circles and ellipses are left as they are. Undo restores everything.")); }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(match n { Some(n) => tr_n(n as u64, "Working on the {n} selected shape.", "Working on the {n} selected shapes."), None => tr("Working on the whole drawing (select shapes to limit it).").to_string() });
        ui.add_space(4.0);
        if open == 0 { ui.label(tr("No open contours.")); }
        else { crate::msg::line(ui, crate::app::Level::Warning, &tr_f("{contours}, {ends} ({joinable} can be joined at this tolerance).", &[("contours", &tr_n(open as u64, "{n} open contour", "{n} open contours")), ("ends", &tr_n(ends.len() as u64, "{n} loose end", "{n} loose ends")), ("joinable", &joinable)])); }
        ui.add_space(6.0);
        let lw = crate::panels::tw(ui, tr("Gap tolerance")).max(crate::panels::tw(ui, tr("Simplify tolerance")));
        egui::Grid::new("clean_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            crate::toolmgr::rlabel(ui, lw, tr("Gap tolerance"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut app.clean.tol).clamp_range(0.001..=5.0).speed(0.005).max_decimals(3));
                ui.weak("mm");
            });
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, tr("Simplify tolerance")).tip(tr("The most an outline may move when Simplify drops points"));
            ui.horizontal(|ui| {
                ui.add_enabled(app.clean.simplify, egui::DragValue::new(&mut app.clean.simp_tol).clamp_range(0.001..=2.0).speed(0.002).max_decimals(3));
                ui.weak("mm");
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        crate::widgets::check(ui, &mut app.clean.join, tr("Join pieces whose ends meet")).tip(tr("Lines and curves drawn as separate pieces become one outline; a ring of pieces closes into a shape"));
        crate::widgets::check(ui, &mut app.clean.dups, tr("Remove duplicate shapes")).tip(tr("The same outline drawn twice would be cut twice"));
        crate::widgets::check(ui, &mut app.clean.tiny, tr("Remove empty and tiny shapes")).tip(tr("Shapes with no length, repeated points"));
        crate::widgets::check(ui, &mut app.clean.simplify, tr("Simplify outlines")).tip(tr("Drops the points that do not change the outline by more than the tolerance: fewer points in imported or traced drawings, a shorter G-code. Corners stay; rectangles, circles and placed text are left alone"));
        if app.clean.simplify {
            let mut copy: Vec<putiacnc_core::model::LoopData> = app.project.data.loops.clone();
            let total: usize = app.simplify_scope().iter().map(|&i| copy[i].pts.len() / 2).sum();
            let gone = cleanup::simplify(&mut copy, &app.simplify_scope(), app.clean.simp_tol.max(1e-4));
            ui.indent("simp_note", |ui| { ui.weak(tr_f("{stay} of {total} points would stay (green on the drawing; red ones go).", &[("stay", &(total - gone)), ("total", &total)])); });
        }
        crate::widgets::check(ui, &mut app.show_open_ends, tr("Show loose ends on the drawing")).tip(tr("Red rings: nothing meets this end. Amber rings: another end is within the tolerance"));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(tr("Clean up")).clicked() { apply = true; }
            if ui.button(tr("Close")).clicked() { close = true; }
        });
        if !app.clean.msg.is_empty() { ui.add_space(2.0); ui.weak(crate::widgets::localize(ui, app.clean.msg.as_str())); }
        });
    }));
    if apply { app.clean.msg = app.clean_drawing(); }
    if close { app.clean.open = false; }
}
