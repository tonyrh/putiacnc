//! Warnings about the toolpath: the status-bar chip, the list window and the red markers in the 3D preview.
use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Sense, Stroke};
use putiacnc_core::checks::Finding;

pub const RED: Color32 = Color32::from_rgb(0xe0, 0x1b, 0x24);

/// The chip at the right of the status bar: how many warnings, or that they are still being worked out.
pub fn chip(ui: &mut egui::Ui, app: &mut App) {
    if app.output.is_none() { return; }
    if app.checks_rx.is_some() { ui.weak("checking…"); return; }
    if app.checks.is_empty() { return; }
    let n = app.checks.len();
    warn_sign(ui);
    let text = egui::RichText::new(format!("{n} warning{}", if n == 1 { "" } else { "s" })).color(RED).strong();
    if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_text("Things in the toolpath that could hurt the tool, the machine or the work. Click for the list").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        app.checks_open = !app.checks_open;
        // a list that opens fresh: the highlight is only for the warning just sent to the 3D view
        if app.checks_open { app.check_sel = None; }
    }
    ui.separator();
}

/// A small red warning triangle, painted (a glyph may be missing from the system's fonts).
pub fn warn_sign(ui: &mut egui::Ui) { warn_sign_col(ui, RED) }

pub fn warn_sign_col(ui: &mut egui::Ui, col: Color32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 16.0), Sense::hover());
    let p = ui.painter();
    let c = r.center();
    p.add(egui::Shape::convex_polygon(vec![c + egui::vec2(0.0, -7.0), c + egui::vec2(8.0, 7.0), c + egui::vec2(-8.0, 7.0)], col, Stroke::new(1.0_f32, col)));
    p.line_segment([c + egui::vec2(0.0, -2.5), c + egui::vec2(0.0, 2.0)], Stroke::new(1.6_f32, Color32::from_black_alpha(200)));
    p.circle_filled(c + egui::vec2(0.0, 4.6), 1.0, Color32::from_black_alpha(200));
}

/// A warning sign in the 3D preview: a small dot on the exact spot, and a red triangle with a "!" standing over it. The picked
/// warning is larger and says what it is.
pub fn marker(painter: &egui::Painter, at: Pos2, selected: bool, what: &str) { marker_sized(painter, at, selected, what, 1.0); }

/// The same sign at `scale` times its size (the 2D drawing shows it smaller).
pub fn marker_sized(painter: &egui::Painter, at: Pos2, selected: bool, what: &str, scale: f32) {
    let (w, h) = if selected { (30.0_f32 * scale, 26.0_f32 * scale) } else { (21.0_f32 * scale, 18.0_f32 * scale) };
    let base = at + egui::vec2(0.0, -4.0 * scale);
    let top = base + egui::vec2(0.0, -h);
    let (l, r) = (base + egui::vec2(-w / 2.0, 0.0), base + egui::vec2(w / 2.0, 0.0));
    painter.line_segment([at, base], Stroke::new(2.0_f32 * scale, Color32::WHITE));
    painter.circle_filled(at, 3.5 * scale, Color32::WHITE);
    painter.circle_filled(at, 2.2 * scale, RED);
    // a white rim first, so the sign reads on wood, shadow and the dark background alike
    let grow = |p: Pos2, dir: egui::Vec2| p + dir * 2.2 * scale;
    painter.add(egui::Shape::convex_polygon(vec![grow(top, egui::vec2(0.0, -1.0)), grow(r, egui::vec2(1.0, 0.6)), grow(l, egui::vec2(-1.0, 0.6))], Color32::WHITE, Stroke::NONE));
    painter.add(egui::Shape::convex_polygon(vec![top, r, l], RED, Stroke::new(1.0_f32, RED)));
    let c = base + egui::vec2(0.0, -h * 0.38);
    let k = h / 18.0;
    painter.line_segment([c + egui::vec2(0.0, -4.5 * k), c + egui::vec2(0.0, 1.2 * k)], Stroke::new(1.8_f32 * k.max(1.0 * scale), Color32::WHITE));
    painter.circle_filled(c + egui::vec2(0.0, 4.2 * k), 1.1 * k.max(1.0), Color32::WHITE);
    if selected && !what.is_empty() {
        let g = painter.layout_no_wrap(what.to_string(), egui::FontId::proportional(13.0), Color32::WHITE);
        let rect = egui::Rect::from_min_size(Pos2::new(r.x + 8.0, base.y - h * 0.5 - g.size().y / 2.0 - 3.0), g.size() + egui::vec2(12.0, 6.0));
        painter.rect_filled(rect, 5.0, Color32::from_black_alpha(190));
        painter.galley(rect.min + egui::vec2(6.0, 3.0), g, Color32::WHITE);
    }
}

pub fn show_in_3d(app: &mut App, i: usize) {
    let Some(mv) = app.checks[i].mv else { return };
    app.check_sel = Some(i);
    app.v3.part = app.check_parts.get(i).copied().unwrap_or(0);
    app.v3.goto_mv = Some(mv);
    app.show_in_3d();
    let st = &mut app.v3;
    st.playing = false;
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.checks_open { return; }
    let mut close = false;
    let mut show: Option<usize> = None;
    let mut goto: Option<usize> = None;
    egui::Window::new("Toolpath warnings").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, "Toolpath warnings", ctx.screen_rect().center())).show(ctx, |ui| {
        ui.set_width(520.0);
        if crate::csd::dialog_header(ui, "Toolpath warnings", true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
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
                            warn_sign(ui);
                            ui.label(egui::RichText::new(f.kind.title()).strong().size(19.0));
                            if app.check_parts.get(i).is_some_and(|&p| p > 0) { ui.weak("plug piece"); }
                            if f.count > 1 { ui.weak(format!("({} moves)", f.count)); }
                        });
                        ui.add_space(2.0);
                        ui.add(egui::Label::new(&f.msg).wrap(true));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            if f.mv.is_some() && crate::panels::dlg_button(ui, crate::icons::Icon::Fit, "Show in 3D", None, None, false).on_hover_text("Jump the preview to the first move that does it").clicked() { show = Some(i); }
                            if f.op.is_some() && crate::panels::dlg_button(ui, crate::icons::Icon::ChevR, "Go to operation", None, None, false).clicked() { goto = Some(i); }
                        });
                        ui.add_space(4.0);
                    });
                }
            });
        }
        ui.add_space(6.0);
        });
    });
    if let Some(i) = show { show_in_3d(app, i); }
    if let Some(i) = goto {
        if let Some(op) = app.checks[i].op { if let Some(o) = app.project.data.ops.get(op) { app.project.data.act = o.id; app.tab = crate::app::RightTab::Ops; } }
    }
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { app.checks_open = false; }
}

/// A warning line: the painted triangle, then the text wrapped beside it, both in `col`.
pub fn warn_line(ui: &mut egui::Ui, col: Color32, text: &str) -> egui::Response {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        warn_sign_col(ui, col);
        ui.add(egui::Label::new(egui::RichText::new(text).color(col)).wrap(true))
    }).inner
}
