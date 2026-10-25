//! Combine dialog: union, subtract, intersect or exclude the selected shapes.
use crate::app::App;
use eframe::egui;
use putiacnc_core::shapes::BoolOp;

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.combine_open { return; }
    let n = app.sel.len();
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Combine shapes").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Combine shapes", crate::csd::anchor(app))).show(ctx, |ui| {
        ui.set_width(340.0);
        if crate::csd::dialog_header(ui, "Combine shapes", true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if n < 2 { "Select two or more closed shapes first.".to_string() } else { format!("{n} shapes selected.") });
        ui.add_space(6.0);
        for (op, name, tip) in [
            (BoolOp::Union, "Union", "Merge the shapes into one outline"),
            (BoolOp::Subtract, "Subtract", "Cut the other shapes out of the base shape"),
            (BoolOp::Intersect, "Intersect", "Keep only the part all the shapes share"),
            (BoolOp::Exclude, "Exclude", "Keep the parts that belong to only one shape (overlaps are removed)"),
        ] {
            if ui.radio(app.combine_op == op, name).on_hover_text(tip).clicked() { app.combine_op = op; }
        }
        if app.combine_op == BoolOp::Subtract {
            ui.add_space(2.0);
            ui.indent("combine_sub", |ui| {
                ui.checkbox(&mut app.combine_swap, "Use the shape drawn last as the base")
                    .on_hover_text("By default the shape drawn first is the one that gets cut by the others");
            });
        }
        ui.add_space(6.0);
        ui.checkbox(&mut app.combine_keep, "Keep the original shapes");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(n >= 2, egui::Button::new("Apply")).clicked() { apply = true; }
            if ui.button("Close").clicked() { close = true; }
        });
        ui.add_space(2.0);
        ui.weak("The result takes the operation of the base shape. Open shapes are skipped.");
        });
    });
    if apply {
        let (op, keep, swap) = (app.combine_op, app.combine_keep, app.combine_swap);
        if let Some(msg) = app.combine_selection(op, keep, swap) { app.error = Some(msg); }
    }
    if close { app.combine_open = false; }
}
