//! Warnings about the toolpath: the status-bar chip, the list window and the red markers in the 3D preview.
use putiacnc_core::i18n::{tr, tr_f};
use crate::csd::ShowFollowing;
use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Stroke};
use putiacnc_core::checks::Finding;

pub const RED: Color32 = Color32::from_rgb(0xe0, 0x1b, 0x24);

/// "checking…" at the right of the status bar while the warnings are being worked out (the warnings themselves are in `bar`).
pub fn chip(ui: &mut egui::Ui, app: &mut App) {
    if app.output.is_some() && app.checks_rx.is_some() { ui.weak(tr("checking…")); ui.separator(); }
}

/// The findings the bar under the view tabs speaks for: all but those about clamps, which have a bar of their own.
pub fn bar_findings(app: &App) -> Vec<usize> {
    if app.output.is_none() { return vec![]; }
    (0..app.checks.len()).filter(|&i| !app.checks[i].kind.about_clamps()).collect()
}

/// The bar under the view tabs for the toolpath warnings: the first one in words, how many more there are, and the way to the list
/// and to the place in the 3D preview. It cannot be dismissed; it is there while the warnings are.
pub fn bar(ui: &mut egui::Ui, app: &mut App) {
    let col = crate::msg::colour(crate::app::Level::Warning, app.theme.dark);
    let ids = bar_findings(app);
    let Some(&first) = ids.first() else { return };
    let n = app.checks.len();
    let f = &app.checks[first];
    let text = if ids.len() == 1 { f.msg.clone() } else { tr_f("{msg} (and {n} more)", &[("msg", &f.msg), ("n", &(ids.len() - 1))]) };
    let go = f.mv.is_some();
    let (mut show, mut list) = (false, false);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.button(tr_f("List ({n})", &[("n", &n)])).on_hover_text(tr("All the warnings about the toolpath")).clicked() { list = true; }
        if ui.add_enabled(go, egui::Button::new(tr("Show in 3D"))).on_hover_text(tr("Jump the 3D preview to the move")).clicked() { show = true; }
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_space(2.0);
            crate::msg::sign(ui, crate::app::Level::Warning);
            ui.add(egui::Label::new(egui::RichText::new(crate::widgets::localize(ui, &text)).color(col)).wrap(true));
        });
    });
    if show { show_in_3d(app, first); }
    if list { app.checks_open = !app.checks_open; if app.checks_open { app.check_sel = None; } }
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
    egui::Window::new("Toolpath warnings").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, "Toolpath warnings", ctx.screen_rect().center())).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(520.0);
        if crate::csd::dialog_header(ui, tr("Toolpath warnings"), true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.add_space(8.0);
        if app.checks.is_empty() {
            ui.label(tr("Nothing found: no move goes deeper than the cutting length, through the stock at rapid speed, outside the machine or into a clamp."));
        } else {
            ui.add(egui::Label::new(egui::RichText::new(tr("The G-code is still written. These are things to look at before cutting; the cutting length and shank of each tool are in the tool manager, clamps are under Stock.")).weak()).wrap(true));
            ui.add_space(6.0);
            egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show_following(ui, |ui| {
                let list: Vec<(usize, Finding)> = app.checks.iter().cloned().enumerate().collect();
                for (i, f) in list {
                    let sel = app.check_sel == Some(i);
                    egui::Frame::none().fill(if sel { ui.visuals().selection.bg_fill.gamma_multiply(0.4) } else { Color32::TRANSPARENT })
                        .inner_margin(egui::Margin::symmetric(6.0, 5.0)).rounding(4.0).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            // a collision with a clamp is an error, as in its bar; the other findings are warnings
                            crate::msg::sign(ui, if f.kind.about_clamps() { crate::app::Level::Error } else { crate::app::Level::Warning });
                            ui.label(egui::RichText::new(f.kind.title()).strong().size(19.0));
                            if app.check_parts.get(i).is_some_and(|&p| p > 0) { ui.weak(tr("plug piece")); }
                            if f.count > 1 { ui.weak(tr_f("({count} moves)", &[("count", &f.count)])); }
                        });
                        ui.add_space(2.0);
                        ui.add(egui::Label::new(crate::widgets::localize(ui, &f.msg)).wrap(true));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            if f.mv.is_some() && crate::panels::dlg_button(ui, crate::icons::Icon::Fit, tr("Show in 3D"), None, None, false).on_hover_text(tr("Jump the preview to the first move that does it")).clicked() { show = Some(i); }
                            if f.op.is_some() && crate::panels::dlg_button(ui, crate::icons::Icon::ChevR, "Go to operation", None, None, false).clicked() { goto = Some(i); }
                        });
                        ui.add_space(4.0);
                    });
                }
            });
        }
        ui.add_space(6.0);
        });
    }));
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
        crate::msg::sign(ui, crate::app::Level::Warning);
        ui.add(egui::Label::new(egui::RichText::new(crate::widgets::localize(ui, text)).color(col)).wrap(true))
    }).inner
}

/// "Before you export": the warnings about the toolpath, asked once (only while there are some). Export anyway goes on to the export
/// window and remembers these warnings; Review opens the list; Cancel goes back.
pub fn export_gate(ctx: &egui::Context, app: &mut App) {
    if !app.export_warn_open { return; }
    let (mut close, mut review, mut go) = (false, false, false);
    egui::Window::new("Before you export").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, "Before you export", ctx.screen_rect().center())).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(crate::csd::dialog_w(480.0));
        if crate::csd::dialog_header(ui, tr("Before you export"), true) { close = true; }
        ui.add_space(8.0);
        let n = app.checks.len();
        ui.add(egui::Label::new(if n == 1 { tr("The toolpath has a warning:").to_string() } else { tr_f("The toolpath has {n} warnings:", &[("n", &n)]) }).wrap(true));
        ui.add_space(6.0);
        for f in app.checks.iter().take(4) {
            ui.horizontal_top(|ui| {
                crate::msg::sign(ui, if f.kind.about_clamps() { crate::app::Level::Error } else { crate::app::Level::Warning });
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(f.kind.title()).strong());
                    ui.add(egui::Label::new(egui::RichText::new(crate::widgets::localize(ui, &f.msg)).weak()).wrap(true));
                });
            });
            ui.add_space(4.0);
        }
        if n > 4 { ui.weak(tr_f("…and {n} more", &[("n", &(n - 4))])); }
        ui.add_space(6.0);
        ui.add(egui::Label::new(egui::RichText::new(tr("The G-code can still be written. You will not be asked again until the warnings change.")).weak()).wrap(true));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(tr("Export anyway")).fill(ui.visuals().selection.bg_fill)).clicked() { go = true; }
                if ui.button(tr("Review warnings")).clicked() { review = true; }
                if ui.button(tr("Cancel")).clicked() { close = true; }
            });
        });
        ui.add_space(4.0);
    }));
    if go { app.export_ack = app.checks_key(); app.export_warn_open = false; app.export_gcode_dialog(); }
    else if review { app.export_warn_open = false; app.checks_open = true; app.check_sel = None; }
    else if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { app.export_warn_open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use putiacnc_core::checks::{Finding, Kind};

    fn app() -> App {
        let mut a = App::new(None);
        a.open(&format!("{}/../tests/data/outside_holes.project.json", env!("CARGO_MANIFEST_DIR")));
        a.start_generation();
        a.finish_generation();
        assert!(a.output.is_some(), "{:?}", a.error);
        a
    }

    fn finding(op: usize, msg: &str) -> Finding {
        Finding { kind: Kind::OutOfArea, op: Some(op), mv: Some(0), at: [0.0; 3], count: 1, msg: msg.into(), clamps: vec![] }
    }

    #[test]
    fn exporting_asks_only_while_there_are_warnings_and_only_once_for_the_same_ones() {
        let mut a = app();
        // nothing found: straight to the file chooser or the split window, never the question
        a.checks.clear(); a.checks_ver = a.out_ver;
        a.export_gcode_dialog();
        assert!(!a.export_warn_open);
        a.export_open = false;
        // a warning: the question comes first and nothing is exported yet
        a.checks = vec![finding(0, "Goes past the edge of the machine.")]; a.check_parts = vec![0]; a.checks_ver = a.out_ver;
        a.export_gcode_dialog();
        assert!(a.export_warn_open && !a.export_open);
        // "Export anyway": remembered, and the same warning does not ask again
        a.export_ack = a.checks_key(); a.export_warn_open = false;
        a.export_gcode_dialog();
        assert!(!a.export_warn_open);
        a.export_open = false;
        // a different warning asks again
        a.checks = vec![finding(0, "Goes past the edge of the machine."), finding(0, "Another one.")]; a.check_parts = vec![0, 0];
        a.export_gcode_dialog();
        assert!(a.export_warn_open);
        // warnings from an older toolpath do not count
        a.export_warn_open = false;
        a.checks_ver = a.out_ver.wrapping_sub(1);
        a.export_gcode_dialog();
        assert!(!a.export_warn_open);
    }

    fn settled(a: &mut App) {
        a.start_generation();
        a.finish_generation();
        for _ in 0..600 { if !a.poll_checks() { break; } std::thread::sleep(std::time::Duration::from_millis(50)); }
    }

    #[test]
    fn the_generators_warnings_join_the_list_and_a_repeat_of_a_check_is_left_out() {
        let mut a = app();
        let stock_t = a.project.data.stock.t;
        a.project.data.ops[0].p.depth = stock_t + 4.0;
        settled(&mut a);
        let out = a.output.clone().unwrap();
        assert!(out.notes.iter().any(|n| n.starts_with("⚠ ") && n.contains("depth exceeds stock thickness")), "the generator says it: {:?}", out.notes);
        let deep: Vec<_> = a.checks.iter().filter(|f| f.op == Some(0) && matches!(f.kind, Kind::Spoilboard | Kind::TooDeep)).collect();
        assert_eq!(deep.len(), 1, "the toolpath check says it once: {:?}", a.checks.iter().map(|f| (f.kind, &f.msg)).collect::<Vec<_>>());
        assert!(!a.checks.iter().any(|f| f.kind == Kind::Setting && f.msg.contains("depth exceeds")), "no second warning for the same thing");
        // a generator warning with no check of its own is in the list: shapes beyond the stock
        a.project.data.ops[0].p.depth = 3.0;
        a.project.data.stock.w = 20.0;
        settled(&mut a);
        let s = a.checks.iter().find(|f| f.kind == Kind::Setting).expect("the shapes beyond the stock are in the list");
        assert!(s.msg.contains("beyond the stock"), "{}", s.msg);
        assert!(a.check_parts.len() == a.checks.len());
    }
}
