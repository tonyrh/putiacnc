//! The Measure tool: two clicks give the distance, the X and Y differences and the angle between the points. Nothing is saved.
use super::*;
use putiacnc_core::i18n::tr;

/// A length in the unit the app shows.
fn len(app: &App, v: f64) -> String {
    if app.disp_in { format!("{} in", crate::widgets::fmt_inch(v / 25.4, 3, app.frac_in)) } else { format!("{} mm", trim_zeros(v)) }
}
fn trim_zeros(v: f64) -> String { let t = format!("{:.2}", v); t.trim_end_matches('0').trim_end_matches('.').to_string() }

pub(super) fn paint_measure(app: &App, painter: &egui::Painter, view: &View, hover_w: Option<[f64; 2]>, shift: bool) {
    let a = match app.measure.first() { Some(a) => *a, None => return };
    let b = if app.measure.len() >= 2 { app.measure[1] } else {
        let Some(h) = hover_w else { return };
        let mut q = app.snap(h);
        if shift { if (q[0] - a[0]).abs() >= (q[1] - a[1]).abs() { q[1] = a[1]; } else { q[0] = a[0]; } }
        q
    };
    let th = app.theme;
    let col = if th.dark { Color32::from_rgb(255, 211, 74) } else { Color32::from_rgb(200, 110, 0) };
    let st = Stroke::new(2.0_f32, col);
    let faint = Stroke::new(1.0_f32, col.gamma_multiply(0.6));
    let (sa, sb) = (view.w2s(a), view.w2s(b));
    let corner = view.w2s([b[0], a[1]]);
    // the X and Y legs, dashed, then the measured line
    for (p, q) in [(sa, corner), (corner, sb)] {
        painter.extend(egui::Shape::dashed_line(&[p, q], faint, 4.0, 4.0));
    }
    painter.line_segment([sa, sb], st);
    for p in [sa, sb] { painter.rect_stroke(Rect::from_center_size(p, Vec2::splat(10.0)), 0.0, st); }
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let dist = dx.hypot(dy);
    if dist < 1e-9 { return; }
    let ang = dy.atan2(dx).to_degrees().rem_euclid(360.0);
    let lines = [
        format!("{}  {}", tr("Distance"), len(app, dist)),
        format!("\u{394}X {}   \u{394}Y {}", len(app, dx.abs()), len(app, dy.abs())),
        format!("{}  {}\u{b0}", tr("Angle"), { let t = format!("{:.1}", ang); t.trim_end_matches('0').trim_end_matches('.').to_string() }),
    ];
    let font = egui::FontId::proportional(13.0);
    let galleys: Vec<_> = lines.iter().map(|t| painter.layout_no_wrap(t.clone(), font.clone(), th.text())).collect();
    let w = galleys.iter().map(|g| g.size().x).fold(0.0_f32, f32::max) + 16.0;
    let h = galleys.iter().map(|g| g.size().y).sum::<f32>() + 14.0;
    // the card sits beside the second point, on the side with room
    let clip = painter.clip_rect();
    let mut tl = sb + Vec2::new(14.0, 14.0);
    if tl.x + w > clip.right() - 4.0 { tl.x = sb.x - 14.0 - w; }
    if tl.y + h > clip.bottom() - 4.0 { tl.y = sb.y - 14.0 - h; }
    let card = Rect::from_min_size(tl, Vec2::new(w, h));
    painter.rect(card, 5.0, th.popover(), Stroke::new(1.0_f32, col.gamma_multiply(0.8)));
    let mut y = card.top() + 7.0;
    for g in galleys { let hh = g.size().y; painter.galley(Pos2::new(card.left() + 8.0, y), g, th.text()); y += hh; }
}
