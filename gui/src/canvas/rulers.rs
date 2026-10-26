//! Rulers and axes along the edges of the drawing.
use super::*;

pub(super) const RULER: f32 = 20.0;

/// A tick spacing (mm) that keeps labels at least ~70 px apart, and how many minor ticks sit between two labels.
pub(super) fn tick_step(scale: f32) -> (f64, usize) {
    for (s, minor) in [(0.01, 5), (0.02, 4), (0.05, 5), (0.1, 5), (0.2, 4), (0.5, 5), (1.0, 5), (2.0, 4), (5.0, 5), (10.0, 5), (20.0, 4), (50.0, 5), (100.0, 5), (200.0, 4), (500.0, 5)] {
        if s * scale as f64 >= 70.0 { return (s, minor); }
    }
    (1000.0, 5)
}

pub(super) fn tick_label(v: f64, step: f64) -> String {
    let dec = if step >= 1.0 { 0 } else if step >= 0.1 { 1 } else { 2 };
    let s = format!("{:.*}", dec, v);
    if s == "-0" || s == "-0.0" || s == "-0.00" { s[1..].to_string() } else { s }
}

/// Rulers along the top and left edges of the drawing area: millimetres in the same coordinates as the X / Y
/// fields, the pointer position marked on both, and the selection's extent shaded.
pub(super) fn draw_rulers(painter: &egui::Painter, rect: Rect, view: &View, scale: f32, (zx, zy): (f64, f64), hover: Option<[f64; 2]>, sel: Option<[f64; 4]>, accent: Color32, ui: &Ui) {
    let vis = ui.visuals();
    let (bg, ink, line) = (vis.panel_fill, vis.text_color().gamma_multiply(0.75), vis.widgets.noninteractive.bg_stroke.color);
    let font = egui::FontId::monospace(9.5);
    let top = Rect::from_min_max(rect.min + Vec2::new(RULER, 0.0), Pos2::new(rect.max.x, rect.min.y + RULER));
    let left = Rect::from_min_max(rect.min + Vec2::new(0.0, RULER), Pos2::new(rect.min.x + RULER, rect.max.y));
    painter.rect_filled(top, 0.0, bg);
    painter.rect_filled(left, 0.0, bg);
    painter.rect_filled(Rect::from_min_size(rect.min, Vec2::splat(RULER)), 0.0, bg);
    let (step, minor) = tick_step(scale);
    let sub = step / minor as f64;
    let st = Stroke::new(1.0_f32, ink);
    let wtl = view.s2w(rect.left_top());
    let wbr = view.s2w(rect.right_bottom());
    // selection extent
    if let Some(b) = sel {
        let (a, c) = (view.w2s([b[0], b[1]]), view.w2s([b[2], b[3]]));
        let fill = accent.gamma_multiply(0.28);
        painter.rect_filled(Rect::from_min_max(Pos2::new(a.x.max(top.min.x), top.min.y), Pos2::new(c.x.min(top.max.x).max(a.x.max(top.min.x)), top.max.y)), 0.0, fill);
        painter.rect_filled(Rect::from_min_max(Pos2::new(left.min.x, c.y.max(left.min.y)), Pos2::new(left.max.x, a.y.min(left.max.y).max(c.y.max(left.min.y)))), 0.0, fill);
    }
    // horizontal ruler
    let top_p = painter.with_clip_rect(top);
    let mut i = ((wtl[0] - zx) / sub).floor() as i64 - 1;
    while (i as f64) * sub <= wbr[0] - zx + sub {
        let v = i as f64 * sub;
        let x = view.w2s([v + zx, 0.0]).x;
        let major = i.rem_euclid(minor as i64) == 0;
        let h = if major { RULER * 0.7 } else if i.rem_euclid(2) == 0 { RULER * 0.38 } else { RULER * 0.28 };
        top_p.line_segment([Pos2::new(x, top.max.y - h), Pos2::new(x, top.max.y)], st);
        if major { top_p.text(Pos2::new(x + 3.0, top.min.y + 1.0), egui::Align2::LEFT_TOP, tick_label(v, step), font.clone(), ink); }
        i += 1;
    }
    // vertical ruler
    let left_p = painter.with_clip_rect(left);
    let mut j = ((wbr[1] - zy) / sub).floor() as i64 - 1;
    while (j as f64) * sub <= wtl[1] - zy + sub {
        let v = j as f64 * sub;
        let y = view.w2s([0.0, v + zy]).y;
        let major = j.rem_euclid(minor as i64) == 0;
        let h = if major { RULER * 0.7 } else if j.rem_euclid(2) == 0 { RULER * 0.38 } else { RULER * 0.28 };
        left_p.line_segment([Pos2::new(left.max.x - h, y), Pos2::new(left.max.x, y)], st);
        if major {
            let g = painter.layout_no_wrap(tick_label(v, step), font.clone(), ink);
            let mut ts = egui::epaint::TextShape::new(Pos2::new(left.min.x + 1.0, y - 3.0), g, ink);
            ts.angle = -std::f32::consts::FRAC_PI_2;
            left_p.add(ts);
        }
        j += 1;
    }
    // pointer position
    if let Some(h) = hover {
        let p = view.w2s(h);
        let ms = Stroke::new(1.0_f32, accent);
        if p.x >= top.min.x && p.x <= top.max.x { top_p.line_segment([Pos2::new(p.x, top.min.y), Pos2::new(p.x, top.max.y)], ms); }
        if p.y >= left.min.y && p.y <= left.max.y { left_p.line_segment([Pos2::new(left.min.x, p.y), Pos2::new(left.max.x, p.y)], ms); }
    }
    let ls = Stroke::new(1.0_f32, line);
    painter.line_segment([Pos2::new(rect.min.x, top.max.y), Pos2::new(rect.max.x, top.max.y)], ls);
    painter.line_segment([Pos2::new(left.max.x, rect.min.y), Pos2::new(left.max.x, rect.max.y)], ls);
    painter.text(rect.min + Vec2::new(RULER / 2.0, RULER / 2.0), egui::Align2::CENTER_CENTER, "mm", egui::FontId::monospace(8.0), ink);
}

/// Graduated X and Y axes along the stock's lower and left edges, with the numbers outside the stock.
pub(super) fn draw_axes(painter: &egui::Painter, view: &View, scale: f32, (sw, sh): (f64, f64), (zx, zy): (f64, f64), dark: bool, ui: &Ui) {
    let col = if dark { Color32::from_rgb(150, 200, 255) } else { Color32::from_rgb(30, 90, 170) };
    let font = egui::FontId::monospace(10.0);
    let (step, minor) = tick_step(scale);
    let sub = step / minor as f64;
    let st = Stroke::new(1.0_f32, col);
    for i in ((-zx) / sub).ceil() as i64..=((sw - zx) / sub).floor() as i64 {
        let v = i as f64 * sub;
        let p = view.w2s([v + zx, 0.0]);
        let major = i % minor as i64 == 0;
        painter.line_segment([p, p + Vec2::new(0.0, if major { 8.0 } else { 4.0 })], st);
        if major { painter.text(p + Vec2::new(0.0, 9.0), egui::Align2::CENTER_TOP, tick_label(v, step), font.clone(), col); }
    }
    for j in ((-zy) / sub).ceil() as i64..=((sh - zy) / sub).floor() as i64 {
        let v = j as f64 * sub;
        let p = view.w2s([0.0, v + zy]);
        let major = j % minor as i64 == 0;
        painter.line_segment([p, p - Vec2::new(if major { 8.0 } else { 4.0 }, 0.0)], st);
        if major { painter.text(p - Vec2::new(9.0, 0.0), egui::Align2::RIGHT_CENTER, tick_label(v, step), font.clone(), col); }
    }
    let (o, xe, ye) = (view.w2s([0.0, 0.0]), view.w2s([sw, 0.0]), view.w2s([0.0, sh]));
    painter.text(xe + Vec2::new(4.0, 0.0), egui::Align2::LEFT_CENTER, "X", egui::FontId::proportional(13.0), col);
    painter.text(ye - Vec2::new(0.0, 4.0), egui::Align2::CENTER_BOTTOM, "Y", egui::FontId::proportional(13.0), col);
    let _ = (o, ui);
}
