//! Moving, resizing and scaling the backdrop picture in the drawing.
use super::*;
use putiacnc_core::backdrop::Backdrop;
use putiacnc_core::i18n::tr;

/// The screen position of a corner: 0 lower-left, 1 lower-right, 2 upper-right, 3 upper-left.
pub(super) fn bd_corner(view: &View, bd: &Backdrop, k: usize) -> Pos2 {
    let (x, y) = (if k == 1 || k == 2 { bd.x + bd.w } else { bd.x }, if k >= 2 { bd.y + bd.h() } else { bd.y });
    view.w2s([x, y])
}

/// A press in empty space: starts moving or resizing an unlocked backdrop. True when it did.
pub(super) fn bd_press(app: &mut App, view: &View, w: [f64; 2], ps: Pos2, shift: bool) -> bool {
    let Some(bd) = app.project.data.backdrop.as_ref().filter(|b| b.visible && !b.locked) else { return false };
    if shift { return false; }
    if let Some(k) = (0..4).find(|&k| bd_corner(view, bd, k).distance(ps) <= 9.0) {
        let o = (k + 2) % 4;
        let (ax, ay) = (if o == 1 || o == 2 { bd.x + bd.w } else { bd.x }, if o >= 2 { bd.y + bd.h() } else { bd.y });
        app.drag = Drag::BackdropSize { anchor: [ax, ay], sx: if k == 1 || k == 2 { 1.0 } else { -1.0 }, sy: if k >= 2 { 1.0 } else { -1.0 } };
        return true;
    }
    if w[0] >= bd.x && w[0] <= bd.x + bd.w && w[1] >= bd.y && w[1] <= bd.y + bd.h() {
        app.drag = Drag::Backdrop { start: w, orig: [bd.x, bd.y] };
        return true;
    }
    false
}

/// After two points are measured: ask the real distance between them and scale the backdrop to it (the first point stays put).
pub fn scale_dialog(ctx: &egui::Context, app: &mut App) {
    let Some(real) = app.bd_scale else { return };
    if app.project.data.backdrop.is_none() { app.bd_scale = None; return; }
    if app.mode != Mode::Measure { app.bd_scale = None; return; }
    if app.measure.len() < 2 { return; }
    let (a, b) = (app.measure[0], app.measure[1]);
    let dist = (b[0] - a[0]).hypot(b[1] - a[1]);
    if dist < 1e-6 { app.measure.clear(); return; }
    let mut real = if real <= 0.0 { dist } else { real };
    let (mut apply, mut cancel) = (false, false);
    let dw = crate::csd::dialog_w(360.0);
    egui::Window::new("Backdrop scale").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Backdrop scale", egui::pos2(520.0, 200.0))).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(dw);
        if crate::csd::dialog_header(ui, tr("Backdrop scale"), true) { cancel = true; }
        ui.horizontal(|ui| { ui.label(tr("The two points are this far apart now:")); ui.strong(crate::widgets::fmt_len(ui, dist)); });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(tr("Their real distance:"));
            crate::widgets::len(ui, [80.0, 24.0], &mut real, 0.01, 100000.0, 0.5, 2, "");
            ui.weak(crate::widgets::len_unit(ui));
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(real > 0.0, egui::Button::new(tr("Apply"))).clicked() { apply = true; }
            if ui.button(tr("Cancel")).clicked() { cancel = true; }
        });
    }));
    app.bd_scale = Some(real);
    if apply {
        let f = real / dist;
        app.hist.record(&app.project.data);
        if let Some(bd) = app.project.data.backdrop.as_mut() {
            bd.x = a[0] + (bd.x - a[0]) * f;
            bd.y = a[1] + (bd.y - a[1]) * f;
            bd.w *= f;
        }
        app.touch();
    }
    if apply || cancel { app.bd_scale = None; app.measure.clear(); app.mode = Mode::Select; }
}
