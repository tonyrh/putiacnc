//! The view cube of the 3D preview: a small cube in the corner that shows which way you are looking.
//! Click a face for that view, an edge for the 45 degree view between two faces, a corner for an
//! isometric view; drag it to orbit. The camera glides to the new view.
use crate::app::App;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use std::f32::consts::{FRAC_PI_2, PI};

type V3 = [f32; 3];
fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn norm(a: V3) -> V3 { let l = dot(a, a).sqrt().max(1e-9); [a[0] / l, a[1] / l, a[2] / l] }

/// The camera's right, up and forward vectors for a yaw and pitch.
fn basis(yaw: f32, pitch: f32) -> (V3, V3, V3, V3) {
    let e = [pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin()];
    let f = [-e[0], -e[1], -e[2]];
    let r = norm(cross(f, [0.0, 0.0, 1.0]));
    let u = cross(r, f);
    (r, u, f, e)
}

/// A glide of the camera from one view to another.
#[derive(Clone, Debug)]
pub struct Anim { pub from: (f32, f32, f32, V3), pub to: (f32, f32, f32, V3), pub t: f32 }

/// Yaw, pitch for looking at the work from the direction `d` (the way the eye sits, in the drawing's axes:
/// +X right, +Y up the page, +Z up from the stock).
pub fn angles_for(d: [i32; 3]) -> (f32, f32) {
    let v = [d[0] as f32, d[1] as f32, d[2] as f32];
    let l = dot(v, v).sqrt().max(1e-9);
    let pitch = (v[2] / l).asin().clamp(0.0, 1.57);
    // straight down: keep the drawing's top at the top of the screen
    let yaw = if d[0] == 0 && d[1] == 0 { -FRAC_PI_2 } else { v[1].atan2(v[0]) };
    (yaw, pitch)
}

/// How far the camera has to sit for the stock to fill about 72% of the view from this angle.
pub fn fit_dist(yaw: f32, pitch: f32, stock: &putiacnc_core::model::Stock, aspect: f32, ortho: bool) -> f32 {
    let (r, u, f, _) = basis(yaw, pitch);
    let (w, h, t) = (stock.w as f32, stock.h as f32, stock.t as f32);
    let c = [w / 2.0, h / 2.0, -t / 2.0];
    let (mut hx, mut hy, mut hz) = (0.0_f32, 0.0_f32, 0.0_f32);
    for &x in &[0.0, w] { for &y in &[0.0, h] { for &z in &[-t, 0.0] {
        let d = [x - c[0], y - c[1], z - c[2]];
        hx = hx.max(dot(d, r).abs()); hy = hy.max(dot(d, u).abs()); hz = hz.max(dot(d, f).abs());
    } } }
    let k = 2.0 * (35.0_f32.to_radians() * 0.5).tan() * 0.72;
    let need = (2.0 * hy).max(2.0 * hx / aspect.max(0.2));
    // in perspective the near face is closer than the middle of the stock, so it looks bigger: step back by half the depth
    (need / k + if ortho { 0.0 } else { hz }).clamp(5.0, 20000.0)
}

fn wrap(a: f32) -> f32 { (a + PI).rem_euclid(2.0 * PI) - PI }

/// Moves the glide along; call once per frame before the camera is built.
pub fn step(st: &mut crate::view3d::State, ctx: &egui::Context) {
    let Some(a) = st.anim.as_mut() else { return };
    a.t += ctx.input(|i| i.stable_dt).min(0.05) / 0.38;
    let t = a.t.min(1.0);
    let e = t * t * (3.0 - 2.0 * t);
    st.yaw = a.from.0 + wrap(a.to.0 - a.from.0) * e;
    st.pitch = a.from.1 + (a.to.1 - a.from.1) * e;
    st.dist = (a.from.2.ln() + (a.to.2.ln() - a.from.2.ln()) * e).exp();
    for i in 0..3 { st.target[i] = a.from.3[i] + (a.to.3[i] - a.from.3[i]) * e; }
    if a.t >= 1.0 { st.yaw = a.to.0; st.pitch = a.to.1; st.dist = a.to.2; st.target = a.to.3; st.anim = None; } else { ctx.request_repaint(); }
}

const SIZE: f32 = 108.0;
// the cell borders along a face: edge, middle, edge
const CUT: [f32; 4] = [-1.0, -0.42, 0.42, 1.0];
const FACES: [(usize, f32, &str); 6] = [(2, 1.0, "TOP"), (1, -1.0, "FRONT"), (0, 1.0, "RIGHT"), (1, 1.0, "BACK"), (0, -1.0, "LEFT"), (2, -1.0, "BOTTOM")];

struct Cell { quad: [Pos2; 4], dir: [i32; 3], face: usize, centre: bool, shade: f32 }

/// The cells of the faces that point at the eye, with their corners on the screen.
fn cells(centre: Pos2, scale: f32, yaw: f32, pitch: f32) -> Vec<Cell> {
    let (r, u, _, e) = basis(yaw, pitch);
    let proj = |p: V3| Pos2::new(centre.x + dot(p, r) * scale, centre.y - dot(p, u) * scale);
    let mut out = Vec::new();
    for (fi, (axis, sign, _)) in FACES.iter().enumerate() {
        let mut n = [0.0_f32; 3]; n[*axis] = *sign;
        let facing = dot(n, e);
        if facing <= 0.02 { continue; }
        let (a1, a2) = ((axis + 1) % 3, (axis + 2) % 3);
        let light = norm([0.35, -0.5, 0.8]);
        let shade = 0.80 + 0.20 * dot(n, light).max(0.0);
        for i in 0..3 { for j in 0..3 {
            let pt = |a: f32, b: f32| { let mut p = n; p[a1] = a; p[a2] = b; proj(p) };
            let quad = [pt(CUT[i], CUT[j]), pt(CUT[i + 1], CUT[j]), pt(CUT[i + 1], CUT[j + 1]), pt(CUT[i], CUT[j + 1])];
            let mut dir = [0_i32; 3];
            dir[*axis] = *sign as i32; dir[a1] = i as i32 - 1; dir[a2] = j as i32 - 1;
            out.push(Cell { quad, dir, face: fi, centre: i == 1 && j == 1, shade });
        } }
    }
    out
}

fn inside(q: &[Pos2; 4], p: Pos2) -> bool {
    let mut sign = 0.0_f32;
    for k in 0..4 {
        let (a, b) = (q[k], q[(k + 1) % 4]);
        let c = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if c != 0.0 { if sign != 0.0 && c.signum() != sign { return false; } sign = c.signum(); }
    }
    true
}

/// Draws the cube and its two small buttons in the top right corner of `rect`, and acts on clicks and drags.
pub fn show(ctx: &egui::Context, app: &mut App, rect: Rect) {
    let accent = app.theme.accent;
    let text = app.theme.text();
    // what the pointer is over, explained to the left of the cube (a tooltip would run over the right panel)
    let mut tip: Option<&'static str> = None;
    let shown = egui::Area::new(egui::Id::new("viewcube")).order(egui::Order::Foreground).pivot(egui::Align2::RIGHT_TOP).fixed_pos(rect.right_top() + Vec2::new(-10.0, 10.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(6.0, 6.0)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            let (area, resp) = ui.allocate_exact_size(Vec2::new(SIZE, SIZE), Sense::click_and_drag());
            let (yaw, pitch) = (app.v3.yaw, app.v3.pitch);
            let list = cells(area.center(), SIZE * 0.5 / 1.85, yaw, pitch);
            let hover = resp.hover_pos().and_then(|p| list.iter().filter(|c| c.dir[2] >= 0).find(|c| inside(&c.quad, p)).map(|c| c.dir));
            let dragging = resp.dragged_by(egui::PointerButton::Primary);
            let base = { let (b, t) = (ui.visuals().widgets.inactive.bg_fill, text); Color32::from_rgb(((b.r() as f32) * 0.78 + (t.r() as f32) * 0.22) as u8, ((b.g() as f32) * 0.78 + (t.g() as f32) * 0.22) as u8, ((b.b() as f32) * 0.78 + (t.b() as f32) * 0.22) as u8) };
            let line = Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color.gamma_multiply(1.6));
            let mix = |c: Color32, k: f32| Color32::from_rgb((c.r() as f32 * k).min(255.0) as u8, (c.g() as f32 * k).min(255.0) as u8, (c.b() as f32 * k).min(255.0) as u8);
            for c in &list {
                let hot = !dragging && hover == Some(c.dir) && c.dir[2] >= 0;
                let fill = if hot { accent } else if c.centre { mix(base, c.shade * 1.15) } else { mix(base, c.shade * 0.88) };
                ui.painter().add(Shape::convex_polygon(c.quad.to_vec(), fill, line));
            }
            for c in list.iter().filter(|c| c.centre) {
                let mid = Pos2::new(c.quad.iter().map(|p| p.x).sum::<f32>() / 4.0, c.quad.iter().map(|p| p.y).sum::<f32>() / 4.0);
                let hot = !dragging && hover == Some(c.dir);
                ui.painter().text(mid, egui::Align2::CENTER_CENTER, FACES[c.face].2, FontId::proportional(9.5), if hot { Color32::WHITE } else { text });
            }
            if hover.is_some() || dragging { ui.ctx().set_cursor_icon(if dragging { egui::CursorIcon::Grabbing } else { egui::CursorIcon::PointingHand }); }
            if dragging {
                let d = resp.drag_delta();
                app.v3.anim = None;
                app.v3.yaw -= d.x * 0.008;
                app.v3.pitch = (app.v3.pitch + d.y * 0.008).clamp(0.0, 1.57);
            } else if resp.clicked() {
                if let Some(d) = hover {
                    let (ty, tp) = angles_for(d);
                    let stk = &app.project.data.stock;
                    let asp = rect.width() / rect.height().max(1.0);
                    let dist = fit_dist(ty, tp, stk, asp, app.v3.ortho);
                    let tgt = [stk.w as f32 / 2.0, stk.h as f32 / 2.0, -(stk.t as f32) / 2.0];
                    app.v3.anim = Some(Anim { from: (app.v3.yaw, app.v3.pitch, app.v3.dist, app.v3.target), to: (ty, tp, dist, tgt), t: 0.0 });
                    app.v3.back = (stk.w * stk.w + stk.h * stk.h + stk.t * stk.t).sqrt() as f32 * 3.0 + 100.0;
                    ctx.request_repaint();
                }
            }
            if resp.hovered() && !dragging { tip = Some("Click a face, an edge or a corner to look from there. Drag the cube to orbit."); }
            ui.horizontal(|ui| {
                let label = if app.v3.ortho { "Ortho" } else { "Persp." };
                let r = ui.selectable_label(app.v3.ortho, label);
                if r.hovered() { tip = Some("Perspective or orthographic: in orthographic, parallel lines stay parallel and sizes do not shrink with distance."); }
                if r.clicked() { app.v3.ortho = !app.v3.ortho; }
                let r = ui.button("Fit");
                if r.hovered() { tip = Some("Zoom to fit the stock from this angle (F)."); }
                let key_fit = ui.ctx().input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.command && !i.modifiers.alt) && !ui.ctx().wants_keyboard_input();
                if r.clicked() || std::mem::take(&mut app.fit3d_pending) || key_fit {
                    let stk = &app.project.data.stock;
                    let asp = rect.width() / rect.height().max(1.0);
                    let dist = fit_dist(app.v3.yaw, app.v3.pitch, stk, asp, app.v3.ortho);
                    let tgt = [stk.w as f32 / 2.0, stk.h as f32 / 2.0, -(stk.t as f32) / 2.0];
                    app.v3.anim = Some(Anim { from: (app.v3.yaw, app.v3.pitch, app.v3.dist, app.v3.target), to: (app.v3.yaw, app.v3.pitch, dist, tgt), t: 0.0 });
                }
            });
        });
    });
    if let Some(t) = tip {
        egui::Area::new(egui::Id::new("viewcube_tip")).order(egui::Order::Tooltip).pivot(egui::Align2::RIGHT_TOP).fixed_pos(shown.response.rect.left_top() + Vec2::new(-8.0, 0.0)).interactable(false).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(10.0, 7.0)).show(ui, |ui| {
                ui.set_max_width(210.0);
                ui.label(t);
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_give_the_classic_views() {
        let (y, p) = angles_for([0, 0, 1]); assert!((y + FRAC_PI_2).abs() < 1e-6 && (p - 1.57).abs() < 1e-6);
        let (y, p) = angles_for([0, -1, 0]); assert!((y + FRAC_PI_2).abs() < 1e-6 && p.abs() < 1e-6);
        let (y, p) = angles_for([1, 0, 0]); assert!(y.abs() < 1e-6 && p.abs() < 1e-6);
        let (y, p) = angles_for([1, -1, 1]); assert!((y + PI / 4.0).abs() < 1e-6 && (p - 0.6155).abs() < 1e-3);
        // below the work is not reachable: it stays level
        assert_eq!(angles_for([0, -1, -1]).1, 0.0);
    }

    #[test]
    fn the_glide_takes_the_short_way_round() {
        assert!((wrap(3.0 * PI / 2.0 + 0.0) + FRAC_PI_2).abs() < 1e-5);
        assert!((wrap(-3.0) - (-3.0)).abs() < 1e-5);
    }

    #[test]
    fn a_face_towards_the_eye_has_nine_cells_and_the_back_ones_are_hidden() {
        let c = cells(Pos2::new(0.0, 0.0), 30.0, -FRAC_PI_2, 1.57);
        assert_eq!(c.iter().filter(|c| c.centre).count(), 1, "only the top face looks at the eye");
        let c = cells(Pos2::new(0.0, 0.0), 30.0, -std::f32::consts::FRAC_PI_4, 0.6155);
        assert_eq!(c.iter().filter(|c| c.centre).count(), 3, "top, front and right in the isometric view");
        assert_eq!(c.len(), 27);
    }

    #[test]
    fn fitting_matches_the_old_top_view_rule() {
        let stk = putiacnc_core::model::Stock { w: 200.0, h: 150.0, t: 12.0, mat: String::new(), zr: "ll".into(), zm: "top".into(), zh: 0.0 };
        let k = 2.0 * (35.0_f32.to_radians() * 0.5).tan() * 0.72;
        let d = fit_dist(-FRAC_PI_2, 1.57, &stk, 1.5, true);
        assert!((d - (200.0_f32 / 1.5).max(150.0) / k).abs() / d < 0.01, "{d}");
        // perspective steps back by half the depth
        assert!(fit_dist(0.0, 0.0, &stk, 1.5, false) > fit_dist(0.0, 0.0, &stk, 1.5, true) + 90.0);
    }
}
