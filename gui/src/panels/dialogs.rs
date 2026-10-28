//! Dialog buttons, the unsaved-changes prompt and the status bar.
use super::*;

/// Dialog button with an icon: `fill` None = the normal button look; otherwise a tinted / solid button.
pub fn dlg_button(ui: &mut Ui, icon: Icon, text: &str, fill: Option<Color32>, fg: Option<Color32>, solid: bool) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let (icon_w, gap, padx) = (16.0, 7.0, 14.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER);
    let size = egui::vec2(padx * 2.0 + icon_w + gap + galley.size().x, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let v = *ui.style().interact(&resp);
    let (bg, stroke, col) = match (fill, fg) {
        (Some(f), Some(c)) => {
            let hot = resp.hovered() || resp.is_pointer_button_down_on();
            if solid { (if hot { f.gamma_multiply(0.88) } else { f }, egui::Stroke::NONE, c) }
            else { (f.gamma_multiply(if hot { 0.28 } else { 0.14 }), egui::Stroke::new(1.0_f32, c.gamma_multiply(0.55)), c) }
        }
        _ => (v.bg_fill, v.bg_stroke, v.fg_stroke.color),
    };
    ui.painter().rect(rect, 7.0, bg, stroke);
    let x0 = rect.center().x - (icon_w + gap + galley.size().x) / 2.0;
    paint_icon(ui.painter(), icon, egui::pos2(x0 + icon_w / 2.0, rect.center().y), col, 0.62);
    ui.painter().galley(egui::pos2(x0 + icon_w + gap, rect.center().y - galley.size().y / 2.0), galley, col);
    resp
}

pub fn confirm_dialog(ctx: &egui::Context, app: &mut App) {
    let Some(action) = app.pending.clone() else { return };
    let mut choice = 0; // 1 save, 2 discard, 3 cancel
    let dark = app.theme.dark;
    let accent = app.theme.accent;
    let red = if dark { Color32::from_rgb(0xff, 0x7b, 0x63) } else { Color32::from_rgb(0xc0, 0x1c, 0x28) };
    let amber = if dark { Color32::from_rgb(0xf8, 0xe4, 0x5c) } else { Color32::from_rgb(0xf5, 0xc2, 0x11) };
    let name = app.file_name();
    egui::Window::new("Unsaved changes").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.add_space(10.0);
            ui.horizontal_top(|ui| {
                ui.add_space(6.0);
                // warning badge
                let (r, _) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 22.0, amber.gamma_multiply(0.22));
                ui.painter().circle_filled(r.center(), 15.0, amber);
                let dark_ink = Color32::from_rgb(0x3d, 0x2e, 0x00);
                ui.painter().line_segment([r.center() + egui::vec2(0.0, -7.0), r.center() + egui::vec2(0.0, 2.5)], egui::Stroke::new(2.6_f32, dark_ink));
                ui.painter().circle_filled(r.center() + egui::vec2(0.0, 7.0), 1.8, dark_ink);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.set_width(340.0);
                    ui.label(egui::RichText::new("Save changes?").size(18.0).strong());
                    ui.label(egui::RichText::new(format!("\"{name}\" has unsaved changes. If you don't save them, they will be lost.")).color(ui.visuals().weak_text_color()));
                });
            });
            ui.add_space(18.0);
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 32.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if dlg_button(ui, Icon::Save, "Save", Some(accent), Some(Color32::WHITE), true).clicked() { choice = 1; }
                if dlg_button(ui, Icon::Trash, "Discard", Some(red), Some(red), false).clicked() { choice = 2; }
                if dlg_button(ui, Icon::Close, "Cancel", None, None, false).clicked() { choice = 3; }
            });
            ui.add_space(8.0);
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { choice = 3; }
    match choice {
        1 => {
            if app.path.trim().is_empty() { app.after_save = Some(action); app.pending = None; app.save_as_dialog(); }
            else { app.save(); if !app.modified { app.perform(action); } }
        }
        2 => app.perform(action),
        3 => { app.pending = None; app.quit_ok = false; }
        _ => {}
    }
}

pub fn status_bar(ui: &mut Ui, app: &mut App) {
    ui.horizontal(|ui| {
        crate::checkui::chip(ui, app);
        // a generation that takes a moment shows itself; quick ones (most) do not, so the bar does not flicker
        let slow = app.gen_started.is_some_and(|t| t.elapsed().as_millis() > 200);
        if slow {
            ui.add(egui::Spinner::new().size(12.0));
            ui.label("generating…");
            ui.separator();
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        match &app.output {
            Some(o) => {
                // the plug pieces are cut too: they count towards the totals
                let (mut tm, mut ct, mut cl, mut ln) = (o.time_min, o.cut_time_min, o.cut_len_mm, o.lines.len());
                for pp in &app.pieces { tm += pp.output.time_min; ct += pp.output.cut_time_min; cl += pp.output.cut_len_mm; ln += pp.output.lines.len(); }
                ui.label(format!("Estimated time {}", fmt_time(tm)));
                ui.separator();
                ui.label(format!("cutting {}", fmt_time(ct)));
                ui.separator();
                ui.label(format!("cut length {:.1} mm", cl));
                ui.separator();
                ui.label(format!("{} lines, generated in {:.0} ms", ln, app.gen_ms));
            }
            // the first toolpath of a project is still being worked out: the spinner says so
            None if app.gen_stale() && app.gen_error.is_none() => {}
            None if app.project.data.ops.is_empty() => { ui.label("Add an operation to generate G-code"); }
            None => { ui.label(app.gen_error.clone().unwrap_or_else(|| "No G-code".into())); }
        }
        ui.separator();
        ui.label(format!("{} shapes, {} selected", app.project.data.loops.len(), app.sel.len()));
        if let Some(msg) = app.note.take() { let now = ui.input(|i| i.time); ui.ctx().data_mut(|x| x.insert_temp(egui::Id::new("status_note"), (msg, now))); }
        let note: Option<(String, f64)> = ui.ctx().data(|x| x.get_temp(egui::Id::new("status_note")));
        if let Some((msg, t0)) = note {
            let age = ui.input(|i| i.time) - t0;
            if age < 6.0 {
                ui.separator();
                ui.label(egui::RichText::new(msg).strong());
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            }
        }
    });
}
