//! Warnings about the toolpath: the status-bar chip, the list window and the red markers in the 3D preview.
use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Sense, Stroke};
use putiacnc_core::checks::Finding;

pub const RED: Color32 = Color32::from_rgb(0xe0, 0x1b, 0x24);

/// The chip at the right of the status bar: how many warnings, or that they are still being worked out.
pub fn chip(ui: &mut egui::Ui, app: &mut App) {
    if app.output.is_none() { return; }
    if app.checks_rx.is_some() { ui.weak("checking..."); return; }
    if app.checks.is_empty() { return; }
    let n = app.checks.len();
    let text = egui::RichText::new(format!("\u{26a0} {n} warning{}", if n == 1 { "" } else { "s" })).color(RED).strong();
    if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_text("Things in the toolpath that could hurt the tool, the machine or the work. Click for the list").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        app.checks_open = !app.checks_open;
    }
    ui.separator();
}

/// Red ring in the 3D preview where a warning happens.
pub fn marker(painter: &egui::Painter, at: Pos2, selected: bool) {
    let r = if selected { 13.0 } else { 8.0 };
    painter.circle_stroke(at, r + 1.5, Stroke::new(3.0_f32, Color32::WHITE));
    painter.circle_stroke(at, r, Stroke::new(2.5_f32, RED));
    if selected { painter.circle_filled(at, 3.0, RED); }
}

pub fn show_in_3d(app: &mut App, i: usize) {
    let Some(mv) = app.checks[i].mv else { return };
    app.check_sel = Some(i);
    app.view_tab = crate::view3d::ViewTab::Preview;
    let st = &mut app.v3;
    st.playing = false;
    if let Some(&c) = st.cum.get(mv) { st.t = c; }
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.checks_open { return; }
    let mut close = false;
    let mut show: Option<usize> = None;
    let mut goto: Option<usize> = None;
    egui::Window::new("Toolpath warnings").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        ui.set_width(520.0);
        if crate::csd::dialog_header(ui, "Toolpath warnings", true) { close = true; }
        ui.add_space(8.0);
        if app.checks.is_empty() {
            ui.label("Nothing found: no move goes deeper than the cutting length, through the stock at rapid speed, outside the machine or into a clamp.");
        } else {
            ui.add(egui::Label::new(egui::RichText::new("The G-code is still written. These are things to look at before cutting; the cutting length and shank of each tool are in the tool manager, clamps are under Stock.").weak()).wrap(true));
            ui.add_space(6.0);
            egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                let list: Vec<(usize, Finding)> = app.checks.iter().cloned().enumerate().collect();
                for (i, f) in list {
                    let sel = app.check_sel == Some(i);
                    egui::Frame::none().fill(if sel { ui.visuals().selection.bg_fill.gamma_multiply(0.4) } else { Color32::TRANSPARENT })
                        .inner_margin(egui::Margin::symmetric(6.0, 5.0)).rounding(4.0).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("\u{26a0}").color(RED));
                            ui.label(egui::RichText::new(f.kind.title()).strong());
                            if f.count > 1 { ui.weak(format!("({} moves)", f.count)); }
                        });
                        ui.add(egui::Label::new(&f.msg).wrap(true));
                        ui.horizontal(|ui| {
                            if f.mv.is_some() && ui.small_button("Show in 3D").on_hover_text("Jump the preview to the first move that does it").clicked() { show = Some(i); }
                            if f.op.is_some() && ui.small_button("Go to operation").clicked() { goto = Some(i); }
                        });
                    });
                }
            });
        }
        ui.add_space(6.0);
    });
    if let Some(i) = show { show_in_3d(app, i); }
    if let Some(i) = goto {
        if let Some(op) = app.checks[i].op { if let Some(o) = app.project.data.ops.get(op) { app.project.data.act = o.id; app.tab = crate::app::RightTab::Ops; } }
    }
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { app.checks_open = false; }
}
