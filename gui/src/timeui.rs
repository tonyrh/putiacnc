//! The estimated time, split up: where it goes, operation by operation. Opens from the time in the status bar.
use crate::app::{fmt_time, App};
use eframe::egui::{self, Color32, Sense, Vec2};
use putiacnc_core::toolpath::{OpTime, Timing};

/// The time of the main program and of the plug pieces together, per operation (the pieces keep the project's operation numbers).
pub fn total_timing(app: &App) -> Option<Timing> {
    let out = app.output.as_ref()?;
    let n = app.project.data.ops.len();
    let mut t = out.timing(n);
    for pp in &app.pieces {
        let p = pp.output.timing(n);
        if p.ops.len() > t.ops.len() { t.ops.resize(p.ops.len(), OpTime::default()); }
        for (a, b) in t.ops.iter_mut().zip(&p.ops) { a.cut += b.cut; a.travel += b.travel; }
        t.waits += p.waits;
    }
    Some(t)
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.time_open { return; }
    let Some(t) = total_timing(app) else { return };
    let dark = app.theme.dark;
    let mut close = false;
    egui::Window::new("Estimated time").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, "Estimated time", ctx.screen_rect().center())).show(ctx, |ui| {
        ui.set_width(520.0);
        if crate::csd::dialog_header(ui, "Estimated time", true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
            ui.add_space(8.0);
            let sum: f64 = t.ops.iter().map(|o| o.total()).sum::<f64>() + t.waits;
            let widest = t.ops.iter().map(|o| o.total()).fold(t.waits, f64::max).max(1e-9);
            let (name_w, bar_w, time_w, pct_w) = (190.0_f32, 150.0_f32, 62.0_f32, 40.0_f32);
            let weak = ui.visuals().weak_text_color();
            let pct = |v: f64| if sum > 0.0 { let p = v / sum * 100.0; if p > 0.0 && p < 1.0 { "<1%".to_string() } else { format!("{:.0}%", p) } } else { String::new() };
            egui::Grid::new("time_grid").num_columns(4).spacing([10.0, 7.0]).show(ui, |ui| {
                let row = |ui: &mut egui::Ui, dot: Color32, name: String, cut: f64, travel: f64, tip: &str| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let (r, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 4.5, dot);
                        let (r, _) = ui.allocate_exact_size(Vec2::new(name_w - 16.0, 20.0), Sense::hover());
                        let g = ui.painter().layout_no_wrap(name, egui::TextStyle::Body.resolve(ui.style()), ui.visuals().text_color());
                        ui.painter().with_clip_rect(r).galley(egui::pos2(r.min.x, r.center().y - g.size().y / 2.0), g, ui.visuals().text_color());
                    });
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(bar_w, 14.0), Sense::hover());
                    ui.painter().rect_filled(r, 3.0, ui.visuals().faint_bg_color);
                    let (wc, wt) = ((cut / widest) as f32 * bar_w, (travel / widest) as f32 * bar_w);
                    let mid = egui::pos2(r.min.x + wc, r.min.y);
                    if wc > 0.0 { ui.painter().rect_filled(egui::Rect::from_min_max(r.min, egui::pos2(mid.x, r.max.y)), 3.0, dot); }
                    if wt > 0.0 { ui.painter().rect_filled(egui::Rect::from_min_max(mid, egui::pos2((mid.x + wt).min(r.max.x), r.max.y)), 3.0, dot.gamma_multiply(0.45)); }
                    resp.on_hover_text(tip);
                    ui.add_sized([time_w, 20.0], egui::Label::new(egui::RichText::new(fmt_time(cut + travel)).monospace()));
                    ui.allocate_ui_with_layout(Vec2::new(pct_w, 20.0), egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(egui::RichText::new(pct(cut + travel)).color(weak)));
                    ui.end_row();
                };
                for (i, o) in t.ops.iter().enumerate() {
                    let kind = app.project.data.ops.get(i).map_or("", |op| crate::panels::KINDS.iter().find(|(k, _)| *k == op.p.kind).map_or(op.p.kind.as_str(), |(_, n)| *n));
                    let tip = format!("cutting {} · moving between cuts {}", fmt_time(o.cut), fmt_time(o.travel));
                    row(ui, crate::theme::op_color(i, dark), format!("{} · {}", i + 1, kind), o.cut, o.travel, &tip);
                }
                if t.waits > 0.0 {
                    row(ui, Color32::from_gray(if dark { 150 } else { 120 }), "Waiting for the spindle".into(), t.waits, 0.0, "The program waits after the spindle starts or changes speed (machine settings: Spindle wait)");
                }
                // the total in the same columns, so the times line up
                ui.label(egui::RichText::new("Total").strong());
                let (r, _) = ui.allocate_exact_size(Vec2::new(bar_w, 14.0), Sense::hover());
                ui.painter().hline(r.x_range(), r.center().y - 7.0 - 4.0, egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color));
                ui.add_sized([time_w, 20.0], egui::Label::new(egui::RichText::new(fmt_time(sum)).monospace().strong()));
                ui.label("");
                ui.end_row();
            });
            ui.add_space(6.0);
            ui.add(egui::Label::new(egui::RichText::new("Solid: cutting. Faded: moving between cuts (rapids, plunges, retracts). Pauses for you (M0) and tool changes by hand take no time here. Includes the machine's time correction, and the plug pieces.").small().weak()).wrap(true));
            ui.add_space(6.0);
        });
    });
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { app.time_open = false; }
}
