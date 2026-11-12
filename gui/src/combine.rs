//! Combine dialog: union, subtract, intersect or exclude the selected shapes.
use putiacnc_core::i18n::{tr, tr_n};
use crate::app::App;
use eframe::egui;
use putiacnc_core::shapes::BoolOp;

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.combine_open { app.ghost.clear_if("combine"); if app.tool_note.as_ref().is_some_and(|(k, _)| *k == "combine") { app.tool_note = None; } return; }
    {
        let (op, keep, swap) = (app.combine_op, app.combine_keep, app.combine_swap);
        let code = match op { BoolOp::Union => 0, BoolOp::Subtract => 1, BoolOp::Intersect => 2, BoolOp::Exclude => 3 };
        app.preview("combine", &[code, keep as u64, swap as u64], move |a| a.combine_selection(op, keep, swap));
    }
    let pnote = app.ghost.note_of("combine").filter(|_| app.sel.len() >= 2).map(String::from);
    let n = app.sel.len();
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Combine shapes").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Combine shapes", crate::csd::anchor(app))).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(crate::csd::dialog_w(376.0));
        if crate::csd::dialog_header_help(ui, tr("Combine shapes"), true, |ui| { ui.label(tr("The result takes the operation of the base shape. Open shapes are skipped.")); }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if n < 2 { tr("Select two or more closed shapes first.").to_string() } else { tr_n(n as u64, "{n} shape selected.", "{n} shapes selected.") });
        ui.add_space(6.0);
        for (op, name, tip) in [
            (BoolOp::Union, tr("Union"), tr("Merge the shapes into one outline")),
            (BoolOp::Subtract, tr("Subtract"), tr("Cut the other shapes out of the base shape")),
            (BoolOp::Intersect, tr("Intersect"), tr("Keep only the part all the shapes share")),
            (BoolOp::Exclude, tr("Exclude"), tr("Keep the parts that belong to only one shape (overlaps are removed)")),
        ] {
            if ui.radio(app.combine_op == op, name).on_hover_text(tip).clicked() { app.combine_op = op; }
            // the choice of base belongs to Subtract, so it sits right under it (greyed out while another operation is chosen)
            if op == BoolOp::Subtract {
                ui.indent("combine_sub", |ui| {
                    crate::widgets::check_if(ui, app.combine_op == BoolOp::Subtract, &mut app.combine_swap, tr("Use the shape drawn last as the base"))
                        .on_hover_text(tr("By default the shape drawn first is the one that gets cut by the others"));
                });
            }
        }
        ui.add_space(6.0);
        crate::widgets::check(ui, &mut app.combine_keep, tr("Keep the original shapes"));
        let mine = app.tool_note.as_ref().filter(|(k, _)| *k == "combine").map(|(_, m)| m);
        if let Some(n) = pnote.as_ref().or(mine) { crate::msg::line(ui, crate::app::Level::Warning, n); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(n >= 2, egui::Button::new(tr("OK"))).on_hover_text(tr("Do it and close the window")).clicked() { apply = true; }
            if ui.button(tr("Cancel")).clicked() { close = true; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::widgets::check(ui, &mut app.preview_on, tr("Preview")).on_hover_text(tr("Show the result in the drawing while you change the values"));
            });
        });
        });
    }));
    if apply {
        let (op, keep, swap) = (app.combine_op, app.combine_keep, app.combine_swap);
        app.ghost.clear();
        if app.run_tool("combine", |a| a.combine_selection(op, keep, swap)) { app.combine_open = false; }
    }
    if close { app.combine_open = false; }
}
