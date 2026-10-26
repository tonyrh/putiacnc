//! The zoom card: minus, the zoom percentage (a menu), plus and Fit. The 2D drawing and the 3D preview share it, so zoom looks
//! and works the same in both: 100 % is the "fit" view of that window.
use crate::icons::{paint_icon, Icon};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};

/// How much one click on plus or minus zooms.
pub const STEP: f32 = 1.25;
/// The sizes the percentage menu offers (as fractions of the fit view).
const PRESETS: [f32; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
const BTN: f32 = 34.0;
const PCT_W: f32 = 54.0;
/// The width of the card (buttons, gaps, frame), for deciding where there is room for it.
pub const CARD_W: f32 = 3.0 * BTN + PCT_W + 3.0 * 5.0 + 8.0 + 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    /// Zoom by this factor (more than 1 zooms in).
    By(f32),
    /// Zoom to this fraction of the fit view (1.0 = fit width/height of the stock).
    To(f32),
    /// Frame the stock again.
    Fit,
}

/// "100%": the zoom as a fraction of the fit view, never below 1 % and with a plus sign when it is beyond what is shown.
pub fn pct_text(frac: f32) -> String {
    let p = (frac * 100.0).round().max(1.0);
    if p > 9999.0 { "9999%+".into() } else { format!("{p:.0}%") }
}

/// Zoom `scale` by `factor` (kept between `lo` and `hi`) so that the point `at` of the view, measured from its corner, stays put.
pub fn zoom_about(scale: f32, offset: Vec2, at: Vec2, factor: f32, lo: f32, hi: f32) -> (f32, Vec2) {
    let ns = (scale * factor).clamp(lo, hi);
    (ns, at - (at - offset) * (ns / scale))
}

/// Plus, minus and 0 on the keyboard (not while a text field has the keyboard).
pub fn keys(ctx: &egui::Context) -> Option<Act> {
    if ctx.wants_keyboard_input() { return None; }
    ctx.input(|i| {
        if i.modifiers.command || i.modifiers.alt { return None; }
        if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) { Some(Act::By(STEP)) }
        else if i.key_pressed(egui::Key::Minus) { Some(Act::By(1.0 / STEP)) }
        else if i.key_pressed(egui::Key::Num0) { Some(Act::Fit) }
        else { None }
    })
}

/// A pinch on a trackpad (or Ctrl and the wheel): the factor to zoom by this frame, 1.0 when there is none.
pub fn pinch(ctx: &egui::Context) -> f32 { ctx.input(|i| i.zoom_delta()) }

/// A card button: grey, or in the selection colour when `selected`. `paint` draws its face in the colour it is given.
pub fn button(ui: &mut egui::Ui, size: Vec2, selected: bool, tip: &str, paint: impl FnOnce(&egui::Painter, Rect, Color32)) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let v = *ui.style().interact(&resp);
    let (fill, fg) = if selected { (ui.visuals().selection.bg_fill, Color32::WHITE) } else { (v.bg_fill, v.fg_stroke.color) };
    ui.painter().rect(rect, 6.0, fill, v.bg_stroke);
    paint(ui.painter(), rect, fg);
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

/// Draws the card with its top right corner at `pos` and returns what was asked. `frac` is the current zoom (1.0 = fit view).
/// Returns the card's rectangle too, so the caller can place things under it.
pub fn show(ctx: &egui::Context, id: &str, order: egui::Order, pos: Pos2, frac: f32) -> (Option<Act>, Rect) {
    let mut act = None;
    let r = egui::Area::new(egui::Id::new(id)).order(order).pivot(egui::Align2::RIGHT_TOP).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(5.0);
                let sq = Vec2::splat(BTN);
                if button(ui, sq, false, "Zoom out (-)", |p, r, fg| paint_icon(p, Icon::Minus, r.center(), fg, BTN / 36.0)).clicked() { act = Some(Act::By(1.0 / STEP)); }
                let text = pct_text(frac);
                let pr = button(ui, Vec2::new(PCT_W, BTN), false, "Zoom level: 100% is the stock framed to fit. Click for sizes", |p, r, fg| {
                    p.text(r.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(13.0), fg);
                });
                let pid = egui::Id::new((id, "menu"));
                if pr.clicked() { ui.memory_mut(|m| m.toggle_popup(pid)); }
                egui::popup::popup_below_widget(ui, pid, &pr, |ui| {
                    ui.set_min_width(PCT_W + 20.0);
                    if ui.selectable_label(false, "Fit").clicked() { act = Some(Act::Fit); }
                    ui.separator();
                    for f in PRESETS {
                        if ui.selectable_label((f - frac).abs() < 0.005, pct_text(f)).clicked() { act = Some(Act::To(f)); }
                    }
                });
                if button(ui, sq, false, "Zoom in (+)", |p, r, fg| paint_icon(p, Icon::Add, r.center(), fg, BTN / 36.0)).clicked() { act = Some(Act::By(STEP)); }
                if button(ui, sq, false, "Fit the stock in view (0 or F)", |p, r, fg| paint_icon(p, Icon::Fit, r.center(), fg, BTN / 36.0)).clicked() { act = Some(Act::Fit); }
            });
        });
    });
    (act, r.response.rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_percentage_reads_as_a_whole_number_and_stays_in_range() {
        assert_eq!(pct_text(1.0), "100%");
        assert_eq!(pct_text(1.254), "125%");
        assert_eq!(pct_text(0.0), "1%");
        assert_eq!(pct_text(0.004), "1%");
        assert_eq!(pct_text(250.0), "9999%+");
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer_where_it_was() {
        let (scale, offset, at) = (2.0_f32, Vec2::new(30.0, 400.0), Vec2::new(250.0, 120.0));
        // the drawing point under `at` before and after
        let world = |s: f32, o: Vec2| Vec2::new((at.x - o.x) / s, (o.y - at.y) / s);
        let before = world(scale, offset);
        for f in [1.25_f32, 0.8, 3.0, 0.3] {
            let (s2, o2) = zoom_about(scale, offset, at, f, 0.05, 500.0);
            let after = world(s2, o2);
            assert!((after.x - before.x).abs() < 1e-3, "x moved at factor {f}");
        }
        // the limits hold: no zoom beyond them, and the point still stays put
        let (s2, _) = zoom_about(400.0, offset, at, 10.0, 0.05, 500.0);
        assert_eq!(s2, 500.0);
        let (s3, _) = zoom_about(0.06, offset, at, 0.1, 0.05, 500.0);
        assert_eq!(s3, 0.05);
    }

    #[test]
    fn one_step_in_and_one_out_come_back_to_the_start() {
        let (s, o) = zoom_about(3.0, Vec2::new(10.0, 20.0), Vec2::new(100.0, 100.0), STEP, 0.05, 500.0);
        let (s2, o2) = zoom_about(s, o, Vec2::new(100.0, 100.0), 1.0 / STEP, 0.05, 500.0);
        assert!((s2 - 3.0).abs() < 1e-5 && (o2 - Vec2::new(10.0, 20.0)).length() < 1e-3);
    }
}
