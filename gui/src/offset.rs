//! Offset dialog: grow or shrink the selected shapes by a distance (inlays, clearances).
use crate::app::App;
use eframe::egui;

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.offset_open { app.ghost.clear_if("offset"); return; }
    {
        let d = if app.offset_inward { -app.offset_mm } else { app.offset_mm };
        let keep = app.offset_keep;
        app.preview("offset", &[app.offset_inward as u64, crate::preview::bits(app.offset_mm), keep as u64], move |a| a.offset_selection(d, keep));
    }
    let pnote = app.ghost.note_of("offset").map(String::from);
    let n = app.sel.len();
    let mut apply = false;
    let mut close = false;
    egui::Window::new("Offset").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Offset", crate::csd::anchor(app))).show(ctx, |ui| {
        ui.set_width(376.0);
        if crate::csd::dialog_header(ui, "Offset", true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if n == 0 { "Select one or more shapes first.".to_string() } else { format!("{n} shape{} selected.", if n == 1 { "" } else { "s" }) });
        ui.add_space(6.0);
        egui::Grid::new("offset_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label("Direction");
            ui.horizontal(|ui| {
                if ui.selectable_label(!app.offset_inward, "Outward").on_hover_text("Grow the shapes").clicked() { app.offset_inward = false; }
                if ui.selectable_label(app.offset_inward, "Inward").on_hover_text("Shrink the shapes").clicked() { app.offset_inward = true; }
            });
            ui.end_row();
            ui.label("Distance");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut app.offset_mm).clamp_range(0.01..=500.0).speed(0.05));
                ui.weak("mm");
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.checkbox(&mut app.offset_keep, "Keep the original shapes");
        if let Some(n) = &pnote { ui.weak(n); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(n > 0, egui::Button::new("Apply")).clicked() { apply = true; }
            if ui.button("Close").clicked() { close = true; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut app.preview_on, "Preview").on_hover_text("Show the result in the drawing while you change the values");
            });
        });
        ui.add_space(2.0);
        ui.weak("Open shapes are skipped. Outer corners come out rounded.");
        });
    });
    if apply {
        let d = if app.offset_inward { -app.offset_mm } else { app.offset_mm };
        let keep = app.offset_keep;
        app.ghost.clear();
        if let Some(msg) = app.offset_selection(d, keep) { app.error = Some(msg); }
    }
    if close { app.offset_open = false; }
}
