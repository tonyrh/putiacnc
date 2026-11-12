//! Offset dialog: grow or shrink the selected shapes by a distance (inlays, clearances).
use putiacnc_core::i18n::{tr, tr_n};
use crate::widgets::Tip;
use crate::app::App;
use eframe::egui;

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.offset_open { app.ghost.clear_if("offset"); if app.tool_note.as_ref().is_some_and(|(k, _)| *k == "offset") { app.tool_note = None; } return; }
    {
        let d = if app.offset_inward { -app.offset_mm } else { app.offset_mm };
        let keep = app.offset_keep;
        app.preview("offset", &[app.offset_inward as u64, crate::preview::bits(app.offset_mm), keep as u64], move |a| a.offset_selection(d, keep));
    }
    let pnote = app.ghost.note_of("offset").filter(|_| !app.sel.is_empty()).map(String::from);
    let n = app.sel.len();
    let mut apply = false;
    let mut close = false;
    egui::Window::new("Offset").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Offset", crate::csd::anchor(app))).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(crate::csd::dialog_w(376.0));
        if crate::csd::dialog_header_help(ui, tr("Offset"), true, |ui| { ui.label(tr("Open shapes are skipped. Outer corners come out rounded.")); }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if n == 0 { tr("Select one or more shapes first.").to_string() } else { tr_n(n as u64, "{n} shape selected.", "{n} shapes selected.") });
        ui.add_space(6.0);
        let lw = crate::panels::tw(ui, tr("Direction")).max(crate::panels::tw(ui, tr("Distance")));
        egui::Grid::new("offset_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            crate::toolmgr::rlabel(ui, lw, tr("Direction"));
            ui.horizontal(|ui| {
                let (c, _) = crate::zoomcard::seg2(ui, [tr("Outward"), tr("Inward")], [tr("Grow the shapes"), tr("Shrink the shapes")], app.offset_inward as usize);
                if let Some(i) = c { app.offset_inward = i == 1; }
            });
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, tr("Distance"));
            ui.horizontal(|ui| {
                crate::widgets::len(ui, [70.0, 24.0], &mut app.offset_mm, 0.01, 500.0, 0.05, 2, "");
                ui.weak(crate::widgets::len_unit(ui));
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        crate::widgets::check(ui, &mut app.offset_keep, tr("Keep the original shapes"));
        let mine = app.tool_note.as_ref().filter(|(k, _)| *k == "offset").map(|(_, m)| m);
        if let Some(n) = pnote.as_ref().or(mine) { crate::msg::line(ui, crate::app::Level::Warning, n); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(n > 0, egui::Button::new(tr("Apply"))).on_hover_text(tr("Do it and keep the window open, to go again")).clicked() { apply = true; }
            if ui.button(tr("Close")).clicked() { close = true; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::widgets::check(ui, &mut app.preview_on, tr("Preview")).tip(tr("Show the result in the drawing while you change the values"));
            });
        });
        });
    }));
    if apply {
        let d = if app.offset_inward { -app.offset_mm } else { app.offset_mm };
        let keep = app.offset_keep;
        app.ghost.clear();
        app.run_tool("offset", |a| a.offset_selection(d, keep));
    }
    if close { app.offset_open = false; }
}
