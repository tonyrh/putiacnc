//! Small vector icons drawn in code (no icon font or image files needed), styled like
//! Adwaita symbolic icons: 1.8 px strokes in the current text colour.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy)]
pub enum Icon { Inlay, Drill, Fillet, Combine, Select, Node, Line, Rect, Ellipse, Text, Undo, Redo, Profile, Pocket, VCarve, Up, Down, Trash, Add, Fit, Copy, Export, Import, Save, Close, AlignL, AlignHC, AlignR, AlignT, AlignVC, AlignB, CenterBoth, DistH, DistV, ChevR, ChevD, Fold, Unfold }

/// Paint an icon centred on `c`. `s` scales it: 1.0 fits a 36 px button.
pub fn paint_icon(painter: &egui::Painter, icon: Icon, c: Pos2, fg: Color32, s: f32) {
    let st = Stroke::new((1.8_f32 * s.max(0.7)).max(1.4), fg);
    let p = |x: f32, y: f32| Pos2::new(c.x + x * s, c.y + y * s);
    let line = |pts: Vec<Pos2>| { painter.add(egui::Shape::line(pts, st)); };
    let closed = |pts: Vec<Pos2>| { painter.add(egui::Shape::closed_line(pts, st)); };
    match icon {
        Icon::Select => closed(vec![p(-6.0, -9.0), p(-6.0, 8.0), p(-2.0, 4.0), p(1.0, 10.0), p(4.0, 8.5), p(1.5, 3.0), p(7.0, 3.0)]),
        Icon::Node => {
            // a polyline with square points, one of them selected
            let (a, b, d) = (p(-8.0, 7.0), p(-1.0, -6.0), p(8.0, 3.0));
            painter.line_segment([a, b], st);
            painter.line_segment([b, d], st);
            for (q, filled) in [(a, false), (b, true), (d, false)] {
                let r = Rect::from_center_size(q, Vec2::splat(5.0 * s));
                if filled { painter.rect_filled(r, 0.5, fg); } else { painter.rect_stroke(r, 0.5, st); }
            }
        }
        Icon::Fillet => {
            // a corner with its tip rounded off
            line(vec![p(9.0, -9.0), p(-1.0, -9.0), p(-5.0, -8.0), p(-8.0, -5.0), p(-9.0, -1.0), p(-9.0, 9.0)]);
            painter.line_segment([p(-9.0, -9.0), p(-5.5, -9.0)], Stroke::new(1.0_f32, fg.gamma_multiply(0.4)));
            painter.line_segment([p(-9.0, -9.0), p(-9.0, -5.5)], Stroke::new(1.0_f32, fg.gamma_multiply(0.4)));
        }
        Icon::Combine => {
            // two overlapping squares, the shared part filled
            let (a, b) = (Rect::from_min_max(p(-9.0, -8.0), p(3.0, 4.0)), Rect::from_min_max(p(-3.0, -4.0), p(9.0, 8.0)));
            painter.rect_filled(a.intersect(b), 0.0, fg.gamma_multiply(0.55));
            painter.rect_stroke(a, 0.5, st);
            painter.rect_stroke(b, 0.5, st);
        }
        Icon::Text => {
            painter.line_segment([p(-8.0, -8.0), p(8.0, -8.0)], st);
            painter.line_segment([p(0.0, -8.0), p(0.0, 8.0)], st);
            painter.line_segment([p(-3.0, 8.0), p(3.0, 8.0)], st);
            painter.line_segment([p(-8.0, -8.0), p(-8.0, -4.0)], st);
            painter.line_segment([p(8.0, -8.0), p(8.0, -4.0)], st);
        }
        Icon::Line => {
            painter.line_segment([p(-9.0, 8.0), p(9.0, -8.0)], st);
            painter.circle_filled(p(-9.0, 8.0), 2.5 * s, fg);
            painter.circle_filled(p(9.0, -8.0), 2.5 * s, fg);
        }
        Icon::Undo | Icon::Redo => {
            let flip = if matches!(icon, Icon::Redo) { -1.0 } else { 1.0 };
            let (r, a0, a1) = (7.0_f32, -2.8_f32, 1.2_f32);
            let q = |t: f32| { let a = a0 + (a1 - a0) * t; p(flip * r * a.cos(), 2.0 + r * a.sin()) };
            line((0..=16).map(|i| q(i as f32 / 16.0)).collect());
            // arrow head at the start of the arc
            let tip = q(0.0);
            let travel = Vec2::new(flip * -a0.sin(), a0.cos());
            for ang in [0.6_f32, -0.6] {
                let (sn, cs) = ang.sin_cos();
                let d = Vec2::new(travel.x * cs - travel.y * sn, travel.x * sn + travel.y * cs) * 5.5 * s;
                painter.line_segment([tip, tip + d], st);
            }
        }
        Icon::Rect => { painter.rect_stroke(Rect::from_center_size(c, Vec2::new(20.0, 15.0) * s), 1.5 * s, st); }
        Icon::Ellipse => line((0..=40).map(|i| { let t = i as f32 / 40.0 * std::f32::consts::TAU; p(10.5 * t.cos(), 8.0 * t.sin()) }).collect()),
        Icon::Profile => {
            // part outline with the tool path running around it
            painter.rect_stroke(Rect::from_center_size(c, Vec2::new(10.0, 8.0) * s), 1.0 * s, st);
            painter.rect_stroke(Rect::from_center_size(c, Vec2::new(22.0, 18.0) * s), 3.0 * s, Stroke::new(st.width * 0.75, fg.gamma_multiply(0.7)));
        }
        Icon::Pocket => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::new(22.0, 18.0) * s), 2.0 * s, st);
            painter.rect_filled(Rect::from_center_size(c, Vec2::new(13.0, 9.0) * s), 1.0 * s, fg.gamma_multiply(0.45));
        }
        Icon::VCarve => {
            closed(vec![p(-10.0, -8.0), p(10.0, -8.0), p(0.0, 9.0)]);
        }
        Icon::Inlay => {
            // a pocket with a plug beside it
            painter.rect_stroke(Rect::from_center_size(p(-6.0, 0.0), Vec2::new(11.0, 16.0) * s), 1.5 * s, st);
            painter.rect_filled(Rect::from_center_size(p(7.0, 0.0), Vec2::new(10.0, 13.0) * s), 1.5 * s, fg.gamma_multiply(0.55));
        }
        Icon::Drill => {
            // a hole seen from above, with the bit going in
            painter.circle_stroke(c, 8.0 * s, st);
            painter.circle_filled(c, 3.0 * s, fg.gamma_multiply(0.6));
        }
        Icon::Up => line(vec![p(-7.0, 3.5), p(0.0, -3.5), p(7.0, 3.5)]),
        Icon::Down => line(vec![p(-7.0, -3.5), p(0.0, 3.5), p(7.0, -3.5)]),
        Icon::Fit => {
            // four corner brackets: "frame everything"
            for (sx, sy) in [(-1.0_f32, -1.0_f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                line(vec![p(sx * 9.0, sy * 4.0), p(sx * 9.0, sy * 9.0), p(sx * 4.0, sy * 9.0)]);
            }
            painter.rect_filled(Rect::from_center_size(c, Vec2::splat(6.0 * s)), 1.0, fg.gamma_multiply(0.5));
        }
        Icon::Copy => {
            painter.rect_stroke(Rect::from_min_max(p(-8.0, -4.0), p(3.0, 9.0)), 1.5 * s, st);
            line(vec![p(-3.0, -4.0), p(-3.0, -9.0), p(8.0, -9.0), p(8.0, 4.0), p(3.0, 4.0)]);
        }
        Icon::Export | Icon::Import => {
            // tray with an arrow leaving (export) or entering (import)
            line(vec![p(-8.0, 2.0), p(-8.0, 9.0), p(8.0, 9.0), p(8.0, 2.0)]);
            let (y0, y1) = if matches!(icon, Icon::Export) { (5.0, -9.0) } else { (-9.0, 5.0) };
            painter.line_segment([p(0.0, y0), p(0.0, y1)], st);
            let d = if y1 < y0 { 1.0 } else { -1.0 };
            line(vec![p(-4.5, y1 + d * 4.5), p(0.0, y1), p(4.5, y1 + d * 4.5)]);
        }
        Icon::Add => {
            painter.line_segment([p(-7.0, 0.0), p(7.0, 0.0)], st);
            painter.line_segment([p(0.0, -7.0), p(0.0, 7.0)], st);
        }
        Icon::AlignL | Icon::AlignHC | Icon::AlignR | Icon::AlignT | Icon::AlignVC | Icon::AlignB => {
            let r = |x0: f32, y0: f32, x1: f32, y1: f32| { closed(vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]); };
            match icon {
                Icon::AlignL => { painter.line_segment([p(-9.0, -9.0), p(-9.0, 9.0)], st); r(-6.0, -6.0, 8.0, -1.5); r(-6.0, 2.0, 3.0, 6.5); }
                Icon::AlignR => { painter.line_segment([p(9.0, -9.0), p(9.0, 9.0)], st); r(-8.0, -6.0, 6.0, -1.5); r(-3.0, 2.0, 6.0, 6.5); }
                Icon::AlignHC => { painter.line_segment([p(0.0, -9.0), p(0.0, 9.0)], st); r(-7.0, -6.0, 7.0, -1.5); r(-4.0, 2.0, 4.0, 6.5); }
                Icon::AlignT => { painter.line_segment([p(-9.0, -9.0), p(9.0, -9.0)], st); r(-6.0, -6.0, -1.5, 8.0); r(2.0, -6.0, 6.5, 3.0); }
                Icon::AlignB => { painter.line_segment([p(-9.0, 9.0), p(9.0, 9.0)], st); r(-6.0, -8.0, -1.5, 6.0); r(2.0, -3.0, 6.5, 6.0); }
                _ => { painter.line_segment([p(-9.0, 0.0), p(9.0, 0.0)], st); r(-6.0, -7.0, -1.5, 7.0); r(2.0, -4.0, 6.5, 4.0); }
            }
        }
        Icon::ChevR => line(vec![p(-3.0, -6.0), p(3.5, 0.0), p(-3.0, 6.0)]),
        Icon::ChevD => line(vec![p(-6.0, -3.0), p(0.0, 3.5), p(6.0, -3.0)]),
        Icon::Fold => {
            line(vec![p(-6.0, -8.0), p(0.0, -2.5), p(6.0, -8.0)]);
            line(vec![p(-6.0, 8.0), p(0.0, 2.5), p(6.0, 8.0)]);
        }
        Icon::Unfold => {
            line(vec![p(-6.0, -2.5), p(0.0, -8.0), p(6.0, -2.5)]);
            line(vec![p(-6.0, 2.5), p(0.0, 8.0), p(6.0, 2.5)]);
        }
        Icon::DistH => {
            painter.line_segment([p(-9.0, -9.0), p(-9.0, 9.0)], st);
            painter.line_segment([p(9.0, -9.0), p(9.0, 9.0)], st);
            closed(vec![p(-3.0, -5.0), p(3.0, -5.0), p(3.0, 5.0), p(-3.0, 5.0)]);
        }
        Icon::DistV => {
            painter.line_segment([p(-9.0, -9.0), p(9.0, -9.0)], st);
            painter.line_segment([p(-9.0, 9.0), p(9.0, 9.0)], st);
            closed(vec![p(-5.0, -3.0), p(5.0, -3.0), p(5.0, 3.0), p(-5.0, 3.0)]);
        }
        Icon::CenterBoth => {
            closed(vec![p(-8.0, -8.0), p(8.0, -8.0), p(8.0, 8.0), p(-8.0, 8.0)]);
            painter.line_segment([p(-3.5, 0.0), p(3.5, 0.0)], st);
            painter.line_segment([p(0.0, -3.5), p(0.0, 3.5)], st);
        }
        Icon::Save => {
            closed(vec![p(-8.0, -8.0), p(5.0, -8.0), p(8.0, -5.0), p(8.0, 8.0), p(-8.0, 8.0)]);
            line(vec![p(-4.0, -8.0), p(-4.0, -3.0), p(4.0, -3.0), p(4.0, -8.0)]);
            line(vec![p(-5.0, 8.0), p(-5.0, 2.0), p(5.0, 2.0), p(5.0, 8.0)]);
        }
        Icon::Close => {
            painter.line_segment([p(-6.0, -6.0), p(6.0, 6.0)], st);
            painter.line_segment([p(-6.0, 6.0), p(6.0, -6.0)], st);
        }
        Icon::Trash => {
            painter.line_segment([p(-9.0, -5.0), p(9.0, -5.0)], st);
            line(vec![p(-3.0, -5.0), p(-3.0, -8.5), p(3.0, -8.5), p(3.0, -5.0)]);
            line(vec![p(-7.0, -5.0), p(-6.0, 9.0), p(6.0, 9.0), p(7.0, -5.0)]);
            painter.line_segment([p(-2.5, -1.0), p(-2.5, 6.0)], st);
            painter.line_segment([p(2.5, -1.0), p(2.5, 6.0)], st);
        }
    }
}

fn button_impl(ui: &mut Ui, icon: Icon, size: f32, selected: bool, enabled: bool, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), if enabled { Sense::click() } else { Sense::hover() });
    let vis = ui.style().visuals.clone();
    let bg = if selected { vis.selection.bg_fill } else if enabled && resp.hovered() { vis.widgets.hovered.bg_fill } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 6.0, bg);
    let mut fg = if selected { Color32::WHITE } else { vis.widgets.inactive.fg_stroke.color };
    if !enabled { fg = fg.gamma_multiply(0.35); }
    paint_icon(ui.painter(), icon, rect.center(), fg, size / 36.0);
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

/// Tool-palette sized (36 px) icon button; undo/redo use a slightly smaller one.
pub fn icon_button(ui: &mut Ui, icon: Icon, selected: bool, enabled: bool, tip: &str) -> egui::Response {
    let size = if matches!(icon, Icon::Undo | Icon::Redo) { 30.0 } else { 36.0 };
    button_impl(ui, icon, size, selected, enabled, tip)
}

/// Compact (26 px) icon button for list rows.
pub fn small_icon_button(ui: &mut Ui, icon: Icon, enabled: bool, tip: &str) -> egui::Response {
    button_impl(ui, icon, 26.0, false, enabled, tip)
}

/// A regular-looking button with an icon in front of its text.
pub fn icon_text_button(ui: &mut Ui, icon: Icon, text: &str, tip: &str, width: Option<f32>) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, ui.visuals().text_color());
    let pad = ui.spacing().button_padding;
    let icon_w = 20.0;
    let content_w = icon_w + 6.0 + galley.size().x;
    let size = Vec2::new(width.unwrap_or(0.0).max(pad.x * 2.0 + content_w), ui.spacing().interact_size.y.max(galley.size().y + 2.0 * pad.y));
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let v = ui.style().interact(&resp).clone();
    ui.painter().rect(rect, v.rounding, v.bg_fill, v.bg_stroke);
    paint_icon(ui.painter(), icon, Pos2::new(rect.center().x - content_w / 2.0 + icon_w / 2.0, rect.center().y), v.fg_stroke.color, 0.6);
    ui.painter().galley(Pos2::new(rect.center().x - content_w / 2.0 + icon_w + 6.0, rect.center().y - galley.size().y / 2.0), galley, v.fg_stroke.color);
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

/// Side view of a cutter (shank + flutes), drawn like the browser app's tool picture.
/// `rect` is the drawing box; the artwork is laid out on a 48 x 72 grid.
///
/// `tip` is the tip radius as a fraction of the bit's radius (0 = sharp); it rounds the point of V-bits and tapers.
pub fn paint_cutter(painter: &egui::Painter, rect: Rect, shape: &str, angle: f64, tip: f64, col: Color32) {
    let s = (rect.width() / 48.0).min(rect.height() / 72.0);
    let ox = rect.center().x - 24.0 * s;
    let oy = rect.center().y - 36.0 * s;
    let p = |x: f32, y: f32| Pos2::new(ox + x * s, oy + y * s);
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| -> Vec<Pos2> {
        (0..=12).map(|i| { let a = a0 + (a1 - a0) * i as f32 / 12.0; p(cx + r * a.cos(), cy + r * a.sin()) }).collect()
    };
    let pi = std::f32::consts::PI;
    let t = (angle.clamp(10.0, 150.0) as f32 * pi / 360.0).tan();
    let mut sk = 7.0_f32;
    let body: Vec<Pos2> = match shape {
        "ball" => { let mut v = vec![p(14.0, 22.0), p(14.0, 46.0)]; v.extend(arc(24.0, 46.0, 10.0, pi, 0.0)); v.push(p(34.0, 22.0)); v }
        "bull" => vec![p(14.0, 22.0), p(14.0, 52.0), p(15.2, 55.0), p(18.0, 56.0), p(30.0, 56.0), p(32.8, 55.0), p(34.0, 52.0), p(34.0, 22.0)],
        "vbit" => {
            let hw = (40.0 * t).min(10.0);
            let (ca, sa) = (1.0 / (1.0 + t * t).sqrt(), t / (1.0 + t * t).sqrt());
            // rounded point: a circle touching both sides, kept small enough to stay inside the picture
            let mut rg = (tip.max(0.0) as f32 * hw).min(hw * 0.9);
            if sa < 0.999 { rg = rg.min(4.0 / (1.0 / sa - 1.0)); }
            sk = sk.min(hw * 0.8);
            if rg < 0.4 {
                let h = hw / t; let y0 = 66.0 - h;
                vec![p(24.0 - hw, 22.0), p(24.0 - hw, y0), p(24.0, 66.0), p(24.0 + hw, y0), p(24.0 + hw, 22.0)]
            } else {
                let av = 66.0 + rg / sa - rg; // where the sides would meet
                let (h, cy) = (hw / t, av - rg / sa);
                let y0 = av - h;
                let lim = pi / 2.0 - sa.asin();
                let mut v = vec![p(24.0 - hw, 22.0), p(24.0 - hw, y0)];
                v.extend((0..=12).map(|k| { let f = -lim + 2.0 * lim * k as f32 / 12.0; p(24.0 + rg * f.sin(), cy + rg * f.cos()) }));
                v.push(p(24.0 + hw, y0)); v.push(p(24.0 + hw, 22.0)); let _ = ca; v
            }
        }
        "engrave" => {
            let hw = (36.0 * t + 3.0).clamp(3.6, 10.0); let h = (hw - 3.0) / t; let y0 = 62.0 - h; sk = sk.min(hw * 0.8);
            vec![p(24.0 - hw, 22.0), p(24.0 - hw, y0), p(21.0, 62.0), p(27.0, 62.0), p(24.0 + hw, y0), p(24.0 + hw, 22.0)]
        }
        "taper" => {
            let hw = (34.0 * t + 3.0).clamp(3.6, 10.0);
            // the ball at the tip grows with the tip radius
            let rg = (tip.max(0.0) as f32 * hw).clamp(1.5, hw * 0.9);
            let h = (hw - rg) / t; let yb = (22.0 + h).min(58.0); sk = sk.min(hw * 0.8);
            let mut v = vec![p(24.0 - hw, 22.0), p(24.0 - rg, yb)]; v.extend(arc(24.0, yb, rg, pi, 0.0)); v.push(p(24.0 + hw, 22.0)); v
        }
        _ => vec![p(14.0, 22.0), p(14.0, 56.0), p(34.0, 56.0), p(34.0, 22.0)],
    };
    let st = Stroke::new((1.6 * s).max(1.0), col);
    painter.rect(Rect::from_min_max(p(24.0 - sk, 2.0), p(24.0 + sk, 22.0)), 2.0 * s, col.gamma_multiply(0.22), st);
    painter.add(egui::Shape::convex_polygon(body.clone(), col.gamma_multiply(0.14), st));
}
