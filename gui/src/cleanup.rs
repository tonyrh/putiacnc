//! Clean up drawing: join the pieces of broken outlines, remove duplicates and specks, and show the open ends.
use crate::app::App;
use eframe::egui;
use putiacnc_core::cleanup::{self, Options};
use putiacnc_core::shapes;

pub struct CleanTool { pub open: bool, pub join: bool, pub dups: bool, pub tiny: bool, pub tol: f64, pub msg: String }

impl Default for CleanTool { fn default() -> Self { CleanTool { open: false, join: true, dups: true, tiny: true, tol: 0.05, msg: String::new() } } }

impl App {
    /// Cleans the selection (or the whole drawing when nothing is selected).
    pub fn clean_drawing(&mut self) -> String {
        let t = &self.clean;
        let o = Options { join: t.join, duplicates: t.dups, tiny: t.tiny, tol: t.tol.max(1e-4) };
        let scope = if self.sel.is_empty() { None } else { Some(self.sel.clone()) };
        let (res, removed, rep) = cleanup::clean(&self.project.data.loops, scope.as_ref(), o);
        let changed = !removed.is_empty() || res.len() != self.project.data.loops.len() || rep.closed > 0 || rep.joined > 0
            || res.iter().zip(&self.project.data.loops).any(|(a, b)| a.pts != b.pts || a.c != b.c);
        if !changed { return "Nothing to clean up.".into(); }
        self.hist.record(&self.project.data);
        // the selection follows its shapes through the renumbering
        let sel: std::collections::BTreeSet<usize> = self.sel.iter().filter(|i| !removed.contains(i)).map(|&i| i - removed.iter().filter(|&&r| r < i).count()).collect();
        shapes::tabs_after_delete(&mut self.project.data.ops, &removed);
        self.project.data.loops = res;
        self.sel = sel;
        self.touch();
        let mut parts: Vec<String> = Vec::new();
        if rep.joined > 0 { parts.push(format!("joined {} piece{}", rep.joined, if rep.joined == 1 { "" } else { "s" })); }
        if rep.closed > 0 { parts.push(format!("closed {} outline{}", rep.closed, if rep.closed == 1 { "" } else { "s" })); }
        if rep.duplicates > 0 { parts.push(format!("removed {} duplicate{}", rep.duplicates, if rep.duplicates == 1 { "" } else { "s" })); }
        if rep.tiny > 0 { parts.push(format!("removed {} empty shape{}", rep.tiny, if rep.tiny == 1 { "" } else { "s" })); }
        let mut m = parts.join(", ");
        if let Some(c) = m.get_mut(0..1) { c.make_ascii_uppercase(); }
        m.push('.');
        if rep.open_left > 0 { m.push_str(&format!(" {} open contour{} left.", rep.open_left, if rep.open_left == 1 { "" } else { "s" })); }
        m
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.clean.open { return; }
    let scope: Vec<&putiacnc_core::model::LoopData> = if app.sel.is_empty() { app.project.data.loops.iter().collect() } else { app.sel.iter().filter_map(|&i| app.project.data.loops.get(i)).collect() };
    let owned: Vec<putiacnc_core::model::LoopData> = scope.into_iter().cloned().collect();
    let open = cleanup::open_count(&owned);
    let ends = cleanup::loose_ends(&owned, app.clean.tol);
    let joinable = ends.iter().filter(|e| e.3).count();
    let n = if app.sel.is_empty() { None } else { Some(app.sel.len()) };
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Clean up drawing").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Clean up drawing", egui::pos2(650.0, 170.0))).show(ctx, |ui| {
        ui.set_width(340.0);
        if crate::csd::dialog_header(ui, "Clean up drawing", true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(match n { Some(n) => format!("Working on the {n} selected shape{}.", if n == 1 { "" } else { "s" }), None => "Working on the whole drawing (select shapes to limit it).".to_string() });
        ui.add_space(4.0);
        if open == 0 { ui.label("No open contours."); }
        else { ui.colored_label(egui::Color32::from_rgb(200, 90, 30), format!("{open} open contour{}, {} loose end{} ({joinable} can be joined at this tolerance).", if open == 1 { "" } else { "s" }, ends.len(), if ends.len() == 1 { "" } else { "s" })); }
        ui.add_space(6.0);
        egui::Grid::new("clean_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Gap tolerance");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut app.clean.tol).clamp_range(0.001..=5.0).speed(0.005).max_decimals(3));
                ui.weak("mm");
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.checkbox(&mut app.clean.join, "Join pieces whose ends meet").on_hover_text("Lines and curves drawn as separate pieces become one outline; a ring of pieces closes into a shape");
        ui.checkbox(&mut app.clean.dups, "Remove duplicate shapes").on_hover_text("The same outline drawn twice would be cut twice");
        ui.checkbox(&mut app.clean.tiny, "Remove empty and tiny shapes").on_hover_text("Shapes with no length, repeated points");
        ui.checkbox(&mut app.show_open_ends, "Show loose ends on the drawing").on_hover_text("Red rings: nothing meets this end. Amber rings: another end is within the tolerance");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Clean up").clicked() { apply = true; }
            if ui.button("Close").clicked() { close = true; }
        });
        if !app.clean.msg.is_empty() { ui.add_space(2.0); ui.weak(app.clean.msg.as_str()); }
        ui.add_space(2.0);
        ui.weak("Rectangles, circles and ellipses are left as they are. Undo restores everything.");
        });
    });
    if apply { app.clean.msg = app.clean_drawing(); }
    if close { app.clean.open = false; }
}
