//! 3D preview: the simulated stock (height field) seen through an orbit camera, the toolpath and a
//! playback bar that replays the job move by move.
use crate::app::App;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use eframe::glow::{self, HasContext};
use putiacnc_core::sim::Sim;
use putiacnc_core::toolpath::Move;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewTab { Design, Preview }

pub const SPEEDS: [(f64, &str); 6] = [(1.0, "1×"), (5.0, "5×"), (20.0, "20×"), (60.0, "60×"), (200.0, "200×"), (1000.0, "1000×")];
const MAX_CELLS: usize = 800_000;

type V3 = [f32; 3];
fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn norm(a: V3) -> V3 { let l = dot(a, a).sqrt().max(1e-9); [a[0] / l, a[1] / l, a[2] / l] }

pub struct State {
    pub sim: Option<Sim>,
    pub ver: u64,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub target: V3,
    /// Orthographic projection instead of perspective.
    pub ortho: bool,
    /// How far behind the target the orthographic eye sits (mm): beyond the whole stock.
    pub back: f32,
    pub framed: bool,
    /// The camera is gliding to a view picked on the view cube.
    pub anim: Option<crate::viewcube::Anim>,
    /// End time (minutes) of every move, and the total.
    pub cum: Vec<f64>,
    pub total: f64,
    pub t: f64,
    pub playing: bool,
    pub speed_idx: usize,
    /// Fraction to jump to after the first build (testing aid).
    pub init_frac: Option<f64>,
    pub gen: u64,
    pub uploaded: Option<(u64, usize)>,
    /// The simulation is still replaying the moves up to the play position.
    pub catching_up: bool,
    pub last_upload: Option<std::time::Instant>,
    pub glres: std::sync::Arc<std::sync::Mutex<GlRes>>,
    pub pending: std::sync::Arc<std::sync::Mutex<Option<(usize, usize, Vec<f32>)>>>,
}

impl Default for State {
    fn default() -> Self {
        State { sim: None, ver: u64::MAX, yaw: -1.15, pitch: 0.62, dist: 300.0, target: [0.0; 3], ortho: false, back: 1000.0, framed: false, anim: None, cum: vec![], total: 0.0, t: 0.0,
            playing: false, speed_idx: 2, init_frac: None, gen: 0, uploaded: None, catching_up: false, last_upload: None, glres: Default::default(), pending: Default::default() }
    }
}

struct Cam { eye: V3, f: V3, r: V3, u: V3, focal: f32, c: Pos2, ortho: bool, target: V3, half: f32 }

impl Cam {
    fn new(s: &State, rect: Rect) -> Cam {
        let (cy, sy) = (s.yaw.cos(), s.yaw.sin());
        let (cp, sp) = (s.pitch.cos(), s.pitch.sin());
        let eye = [s.target[0] + s.dist * cp * cy, s.target[1] + s.dist * cp * sy, s.target[2] + s.dist * sp];
        let f = norm(sub(s.target, eye));
        let r = norm(cross(f, [0.0, 0.0, 1.0]));
        let u = cross(r, f);
        let tanh = (35.0_f32.to_radians() * 0.5).tan();
        let focal = rect.height() * 0.5 / tanh;
        // orthographic: the eye moves back past the whole stock and the picture keeps the size it has at the target
        let eye = if s.ortho { [s.target[0] - f[0] * s.back, s.target[1] - f[1] * s.back, s.target[2] - f[2] * s.back] } else { eye };
        Cam { eye, f, r, u, focal, c: rect.center(), ortho: s.ortho, target: s.target, half: s.dist * tanh }
    }
    fn p(&self, q: V3) -> Option<Pos2> {
        if self.ortho {
            let d = sub(q, self.target);
            let k = self.focal / (self.half / (35.0_f32.to_radians() * 0.5).tan());
            return Some(Pos2::new(self.c.x + dot(d, self.r) * k, self.c.y - dot(d, self.u) * k));
        }
        let d = sub(q, self.eye);
        let z = dot(d, self.f);
        if z < 1.0 { return None; }
        Some(Pos2::new(self.c.x + dot(d, self.r) / z * self.focal, self.c.y - dot(d, self.u) / z * self.focal))
    }
}

fn fmt_time(min: f64) -> String {
    let s = (min * 60.0).round().max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Keep the simulation in step with the toolpath and the playback position.
fn sync(app: &mut App, ctx: &egui::Context) {
    let moves: &[Move] = app.output.as_ref().map(|o| &o.moves[..]).unwrap_or(&[]);
    let st = &mut app.v3;
    if st.sim.is_none() || st.ver != app.out_ver {
        let was_end = st.t >= st.total - 1e-9;
        let stk = &app.project.data.stock;
        // the height texture is at most 4096 texels wide
        let cap = (stk.w * stk.h / (stk.w.max(stk.h) / 4000.0).powi(2)).min(MAX_CELLS as f64) as usize;
        st.sim = Some(Sim::new(&app.project.data, cap));
        st.gen += 1;
        let mut acc = 0.0;
        st.cum = moves.iter().map(|m| { acc += m.dt; acc }).collect();
        st.total = acc;
        st.t = if was_end { st.total } else { st.t.min(st.total) };
        st.ver = app.out_ver;
        if st.total > 0.0 { if let Some(f) = st.init_frac.take() { st.t = st.total * f; } }
        if !st.framed { frame(st, &app.project.data.stock); }
    }
    if st.playing {
        let dt = ctx.input(|i| i.stable_dt).min(0.1) as f64;
        st.t += dt * SPEEDS[st.speed_idx].0 / 60.0;
        if st.t >= st.total { st.t = st.total; st.playing = false; }
        ctx.request_repaint();
    }
    let idx = st.cum.partition_point(|&c| c <= st.t + 1e-12);
    // the simulation catches up in slices, so the window keeps answering while a long program is replayed
    if let Some(sim) = st.sim.as_mut() {
        // a slow frame (software drawing, big window) gets a longer slice, so the replay does not crawl
        let slice = (ctx.input(|i| i.stable_dt) * 0.6).clamp(0.010, 0.05);
        st.catching_up = !sim.advance_for(moves, idx, Some(std::time::Duration::from_secs_f32(slice)));
        if st.catching_up { ctx.request_repaint(); }
        else if let Some(m) = moves.get(sim.done) {
            // the move in progress is cut as far as the cutter has got
            let start = if sim.done > 0 { st.cum.get(sim.done - 1).copied().unwrap_or(0.0) } else { 0.0 };
            let f = if m.dt > 1e-12 { ((st.t - start) / m.dt).clamp(0.0, 1.0) } else { 1.0 };
            sim.advance_into(moves, f);
        }
    }
}

pub(crate) fn frame(st: &mut State, stock: &putiacnc_core::model::Stock) {
    st.target = [stock.w as f32 / 2.0, stock.h as f32 / 2.0, -(stock.t as f32) / 2.0];
    let diag = ((stock.w * stock.w + stock.h * stock.h + stock.t * stock.t) as f32).sqrt();
    st.dist = diag * 1.9;
    st.back = diag * 3.0 + 100.0;
    if let Some(z) = std::env::var("PUTIACNC_ZOOM").ok().and_then(|s| s.parse::<f32>().ok()) { st.dist *= z; st.target = [stock.w as f32 * 0.35, stock.h as f32 * 0.55, 0.0]; }
    st.framed = true;
}

/// Underlined tab row, shared by the view tabs and the right panel's tabs.
pub fn tabs<T: PartialEq + Copy>(ui: &mut Ui, theme: &crate::theme::Theme, cur: &mut T, items: &[(T, &str)]) {
    let plain: Vec<(T, &str, usize)> = items.iter().map(|(t, l)| (*t, *l, 0)).collect();
    tabs_badged(ui, theme, cur, &plain);
}

/// Like `tabs`, with a small amber count after the label of any tab whose number is not zero (warnings waiting there).
pub fn tabs_badged<T: PartialEq + Copy>(ui: &mut Ui, theme: &crate::theme::Theme, cur: &mut T, items: &[(T, &str, usize)]) {
    let (accent, strong) = (theme.accent, theme.text());
    let weak = ui.visuals().weak_text_color();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (tab, label, badge) in items {
            let sel = *cur == *tab;
            let font = egui::TextStyle::Body.resolve(ui.style());
            let galley = ui.painter().layout_no_wrap(label.to_string(), font, if sel { strong } else { weak });
            let badge_w = if *badge > 0 { 22.0 } else { 0.0 };
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 28.0 + badge_w, 26.0), Sense::click());
            let resp = if *badge > 0 { resp.on_hover_text(format!("{badge} warning{} in this tab", if *badge == 1 { "" } else { "s" })) } else { resp };
            if resp.hovered() && !sel { ui.painter().rect_filled(rect.shrink2(Vec2::new(0.0, 3.0)), 6.0, ui.visuals().widgets.hovered.weak_bg_fill); }
            let col = if sel || resp.hovered() { strong } else { weak };
            let gw = galley.size().x;
            let lx = rect.center().x - (gw + badge_w) / 2.0;
            ui.painter().galley(Pos2::new(lx, rect.center().y - galley.size().y / 2.0), galley, col);
            if *badge > 0 {
                let amber = if theme.dark { Color32::from_rgb(255, 176, 64) } else { Color32::from_rgb(210, 110, 0) };
                let c = Pos2::new(lx + gw + 4.0 + 9.0, rect.center().y);
                ui.painter().circle_filled(c, 9.0, amber);
                let t = if *badge > 9 { "9+".to_string() } else { badge.to_string() };
                ui.painter().text(c, egui::Align2::CENTER_CENTER, t, egui::FontId::proportional(11.0), Color32::from_rgb(30, 20, 0));
            }
            if sel { ui.painter().rect_filled(Rect::from_min_max(Pos2::new(rect.left() + 8.0, rect.bottom() - 2.0), Pos2::new(rect.right() - 8.0, rect.bottom())), 1.0, accent); }
            if resp.clicked() { *cur = *tab; }
        }
    });
}

pub fn tab_strip(ui: &mut Ui, app: &mut App) {
    let theme = app.theme;
    tabs(ui, &theme, &mut app.view_tab, &[(ViewTab::Design, "2D design"), (ViewTab::Preview, "3D preview")]);
}

/// Painted play / pause / restart glyphs.
fn glyph_button(ui: &mut Ui, kind: u8, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    let resp = resp.on_hover_text(tip);
    if resp.hovered() { ui.painter().rect_filled(rect, 5.0, ui.visuals().widgets.hovered.weak_bg_fill); }
    let col = ui.visuals().text_color();
    let c = rect.center();
    let p = ui.painter();
    match kind {
        0 => { p.add(Shape::convex_polygon(vec![c + Vec2::new(-4.5, -6.0), c + Vec2::new(-4.5, 6.0), c + Vec2::new(6.0, 0.0)], col, Stroke::NONE)); }
        1 => { p.rect_filled(Rect::from_center_size(c + Vec2::new(-3.5, 0.0), Vec2::new(3.5, 12.0)), 1.0, col); p.rect_filled(Rect::from_center_size(c + Vec2::new(3.5, 0.0), Vec2::new(3.5, 12.0)), 1.0, col); }
        _ => { p.rect_filled(Rect::from_center_size(c + Vec2::new(-5.0, 0.0), Vec2::new(2.5, 12.0)), 1.0, col);
               p.add(Shape::convex_polygon(vec![c + Vec2::new(6.0, -6.0), c + Vec2::new(6.0, 6.0), c + Vec2::new(-3.0, 0.0)], col, Stroke::NONE)); }
    }
    resp
}

pub fn play_bar(ui: &mut Ui, app: &mut App) {
    let have = app.v3.total > 0.0;
    ui.horizontal(|ui| {
        ui.add_enabled_ui(have, |ui| {
            if glyph_button(ui, 2, "Restart from the beginning").clicked() { app.v3.t = 0.0; app.v3.playing = true; }
            let playing = app.v3.playing;
            if glyph_button(ui, if playing { 1 } else { 0 }, if playing { "Pause" } else { "Play" }).clicked() {
                if playing { app.v3.playing = false; } else { if app.v3.t >= app.v3.total - 1e-9 { app.v3.t = 0.0; } app.v3.playing = true; }
            }
        });
        let label = format!("{} / {}", fmt_time(app.v3.t), fmt_time(app.v3.total));
        let lw = crate::panels::tw(ui, "00:00 / 00:00") + 8.0;
        let speed_w = 74.0;
        let avail = ui.available_width();
        // the speed box is its set width plus the arrow and padding; the slider takes everything else
        let combo_total = (speed_w - 24.0) + ui.spacing().button_padding.x * 2.0 + ui.spacing().icon_width;
        ui.spacing_mut().slider_width = (avail - lw - combo_total - 1.0 * ui.spacing().item_spacing.x - 2.0).max(60.0);
        let total = app.v3.total.max(1e-6);
        let mut t = app.v3.t;
        let r = ui.add_enabled(have, egui::Slider::new(&mut t, 0.0..=total).show_value(false));
        if r.changed() { app.v3.t = t; app.v3.playing = false; }
        ui.allocate_ui_with_layout(Vec2::new(lw, ui.spacing().interact_size.y), egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(label); });
        ui.add_enabled_ui(have, |ui| {
            egui::ComboBox::from_id_source("speed").width(speed_w - 24.0).selected_text(SPEEDS[app.v3.speed_idx].1).show_ui(ui, |ui| {
                for (i, (_, n)) in SPEEDS.iter().enumerate() { ui.selectable_value(&mut app.v3.speed_idx, i, *n); }
            }).response.on_hover_text("Playback speed relative to the machine's real time");
        });
    });
}

pub fn preview(ui: &mut Ui, app: &mut App) {
    let ctx = ui.ctx().clone();
    sync(app, &ctx);
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
    let painter = painter.with_clip_rect(rect);
    let dark = app.theme.dark;
    // hint line, as wide as the view
    egui::Area::new(egui::Id::new("hint3d")).order(egui::Order::Foreground).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(rect.left_bottom() + egui::Vec2::new(12.0, -10.0)).interactable(false).show(&ctx, |ui| {
        ui.set_max_width((rect.width() - 24.0).max(120.0));
        ui.label(egui::RichText::new("Drag: orbit  ·  Right-drag or Shift-drag: pan  ·  Scroll: zoom  ·  Double-click: reset  ·  Click the cube to change view").weak());
    });

    // camera controls
    crate::viewcube::step(&mut app.v3, &ctx);
    {
        let st = &mut app.v3;
        if resp.dragged() || (resp.hovered() && ctx.input(|i| i.smooth_scroll_delta.y != 0.0)) { st.anim = None; }
        let pan = resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Secondary)
            || (resp.dragged_by(egui::PointerButton::Primary) && ctx.input(|i| i.modifiers.shift));
        if pan {
            let cam = Cam::new(st, rect);
            let d = resp.drag_delta();
            let k = st.dist / cam.focal;
            for i in 0..3 { st.target[i] += (-cam.r[i] * d.x + cam.u[i] * d.y) * k; }
        } else if resp.dragged_by(egui::PointerButton::Primary) {
            let d = resp.drag_delta();
            st.yaw -= d.x * 0.008;
            st.pitch = (st.pitch + d.y * 0.008).clamp(0.0, 1.57);
        }
        if resp.hovered() {
            let s = ctx.input(|i| i.smooth_scroll_delta.y);
            if s != 0.0 { st.dist = (st.dist * (-s * 0.0015).exp()).clamp(5.0, 20000.0); }
        }
        if resp.double_clicked() { st.framed = false; st.yaw = -1.15; st.pitch = 0.62; frame(st, &app.project.data.stock); }
    }

    // the view cube, top right
    crate::viewcube::show(&ctx, app, rect);

    let cam = Cam::new(&app.v3, rect);
    let Some(sim) = app.v3.sim.as_ref() else { return };
    let (w, h, t) = (sim.w as f32, sim.h as f32, sim.t as f32);
    let cell = sim.cell as f32;
    // hand the new heights to the GPU only when the simulation changed
    let stamp = (app.v3.gen, sim.rev as usize);
    // while catching up the copy for the GPU is made a few times a second only
    let due = !app.v3.catching_up || app.v3.last_upload.is_none_or(|t| t.elapsed().as_millis() > 150);
    if app.v3.uploaded != Some(stamp) && due {
        *app.v3.pending.lock().unwrap() = Some((sim.nx, sim.ny, sim.z.clone()));
        app.v3.uploaded = Some(stamp);
        app.v3.last_upload = Some(std::time::Instant::now());
    }
    let col = |c: Color32| [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0];
    let u = Uniforms {
        eye: cam.eye, f: cam.f, r: cam.r, u: cam.u, tanh: (35.0_f32.to_radians() * 0.5).tan(), ortho: if cam.ortho { cam.half } else { 0.0 },
        bx: [w, h, t], cell, n: [sim.nx as i32, sim.ny as i32],
        wood: col(app.theme.material(&app.project.data.stock.mat).top),
        floor: col(if dark { Color32::from_rgb(70, 70, 76) } else { Color32::from_rgb(120, 120, 128) }),
        side: col(app.theme.material(&app.project.data.stock.mat).side),
        grain: app.theme.material(&app.project.data.stock.mat).grain,
        rings: app.theme.material(&app.project.data.stock.mat).rings,
        brush: app.theme.material(&app.project.data.stock.mat).brush,
    };
    let (glres, pending) = (app.v3.glres.clone(), app.v3.pending.clone());
    painter.add(egui::PaintCallback {
        rect,
        callback: std::sync::Arc::new(eframe::egui_glow::CallbackFn::new(move |info, p| draw_gl(&glres, &pending, &u, &info, p))),
    });
    let sim = app.v3.sim.as_ref().unwrap();

    // clamps: translucent red boxes from the table to their top
    for c in &app.project.data.clamps {
        let (z0, z1) = (-(app.project.data.stock.t as f32), c.height as f32);
        let (x0, y0, x1, y1) = (c.x as f32, c.y as f32, (c.x + c.w) as f32, (c.y + c.h) as f32);
        let corner = |i: usize, z: f32| cam.p([if i == 1 || i == 2 { x1 } else { x0 }, if i >= 2 { y1 } else { y0 }, z]);
        let fill = Color32::from_rgba_unmultiplied(0xe0, 0x1b, 0x24, 70);
        let edge = Stroke::new(1.5_f32, Color32::from_rgb(0xe0, 0x1b, 0x24));
        for i in 0..4 {
            let j = (i + 1) % 4;
            if let (Some(a), Some(b), Some(c2), Some(d)) = (corner(i, z0), corner(j, z0), corner(j, z1), corner(i, z1)) {
                painter.add(Shape::convex_polygon(vec![a, b, c2, d], fill, Stroke::NONE));
            }
        }
        let top: Vec<Pos2> = (0..4).filter_map(|i| corner(i, z1)).collect();
        if top.len() == 4 { painter.add(Shape::convex_polygon(top.clone(), fill, edge)); }
        for i in 0..4 {
            if let (Some(a), Some(b)) = (corner(i, z0), corner(i, z1)) { painter.line_segment([a, b], edge); }
            if let (Some(a), Some(b)) = (corner(i, z0), corner((i + 1) % 4, z0)) { painter.line_segment([a, b], edge); }
        }
    }

    // toolpath
    if let Some(o) = &app.output {
        let done = sim.done;
        let mut run: Vec<Pos2> = Vec::new();
        let mut run_key: Option<(usize, bool)> = None;
        let flush = |run: &mut Vec<Pos2>, key: Option<(usize, bool)>| {
            if run.len() >= 2 {
                if let Some((op, executed)) = key {
                    let c = crate::theme::op_color(op, dark);
                    let (c, wd) = if executed { (c, 1.6_f32) } else { (c.gamma_multiply(0.35), 1.0_f32) };
                    painter.add(Shape::line(std::mem::take(run), Stroke::new(wd, c)));
                }
            }
            run.clear();
        };
        for (i, m) in o.moves.iter().enumerate() {
            let executed = i < done;
            if !m.cut {
                flush(&mut run, run_key); run_key = None;
                if app.show_rapids {
                    if let (Some(a), Some(b)) = (cam.p([m.from[0] as f32, m.from[1] as f32, m.from[2] as f32]), cam.p([m.to[0] as f32, m.to[1] as f32, m.to[2] as f32])) {
                        painter.line_segment([a, b], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(160, 160, 160, if executed { 150 } else { 50 })));
                    }
                }
                continue;
            }
            if !app.show_toolpath { flush(&mut run, run_key); run_key = None; continue; }
            let key = (m.op, executed);
            if run_key != Some(key) { flush(&mut run, run_key); run_key = Some(key); }
            let (Some(a), Some(b)) = (cam.p([m.from[0] as f32, m.from[1] as f32, m.from[2] as f32]), cam.p([m.to[0] as f32, m.to[1] as f32, m.to[2] as f32])) else { flush(&mut run, run_key); continue };
            if run.is_empty() { run.push(a); }
            run.push(b);
        }
        flush(&mut run, run_key);
        for (i, f) in app.checks.iter().enumerate() {
            if f.mv.is_none() { continue; }
            if let Some(a) = cam.p([f.at[0] as f32, f.at[1] as f32, f.at[2] as f32]) { crate::checkui::marker(&painter, a, app.check_sel == Some(i)); }
        }
        // the cutter: it glides along the move in progress, so rapids and plunges are not shown as jumps
        let st = &app.v3;
        if (done > 0 || st.t > 0.0) && done < o.moves.len() {
            let m = &o.moves[done];
            let start = if done > 0 { st.cum.get(done - 1).copied().unwrap_or(0.0) } else { 0.0 };
            let f = if m.dt > 1e-12 { ((st.t - start) / m.dt).clamp(0.0, 1.0) } else { 1.0 };
            let tip: Vec<f32> = (0..3).map(|i| (m.from[i] + (m.to[i] - m.from[i]) * f) as f32).collect();
            if let (Some(a), Some(b)) = (cam.p([tip[0], tip[1], tip[2]]), cam.p([tip[0], tip[1], tip[2] + 30.0])) {
                let c = crate::theme::op_color(m.op, dark);
                // lighter while rapiding: the tool is not cutting
                let c = if m.cut { c } else { c.gamma_multiply(0.55) };
                // a dark edge, a white halo and the operation colour: readable on wood, metal, foam and the toolpath lines
                painter.line_segment([a, b], Stroke::new(9.0_f32, Color32::from_black_alpha(if m.cut { 130 } else { 80 })));
                painter.line_segment([a, b], Stroke::new(7.0_f32, Color32::WHITE));
                painter.line_segment([a, b], Stroke::new(4.0_f32, c));
                painter.circle_filled(a, 7.0, Color32::from_black_alpha(if m.cut { 130 } else { 80 }));
                painter.circle_filled(a, 6.0, Color32::WHITE);
                painter.circle_filled(a, 4.0, c);
            }
        }
    }

    // which operation and pass the replay is on. Retracts and rapids carry no pass, so the last real pass is kept; and at high
    // speed the label changes at most a few times a second and its box keeps one width, so it does not flicker
    if let (Some(o), Some(sim)) = (&app.output, app.v3.sim.as_ref()) {
        let cur = sim.done.saturating_sub(1).min(o.moves.len().saturating_sub(1));
        if let Some(m) = o.moves.get(cur) {
            let pass = o.moves[..=cur].iter().rev().take_while(|x| x.op == m.op).find(|x| x.pass > 0).map_or(0, |x| x.pass);
            let passes = o.moves.iter().filter(|x| x.op == m.op).map(|x| x.pass).max().unwrap_or(0);
            let name = app.project.data.ops.get(m.op).map(|op| crate::panels::KINDS.iter().find(|(k, _)| *k == op.p.kind).map_or(op.p.kind.as_str(), |(_, n)| *n)).unwrap_or("");
            // the Z the pass cuts down to, as the G-code writes it (a ramp reaches it by the end of the pass)
            let imp = app.imperial;
            let zfmt = |z: f64| if imp { format!("{:.4} in", z / 25.4) } else { format!("{:.3} mm", z) };
            // lowest Z of each pass of this operation, worked out once per output
            let cid = egui::Id::new(("v3_pass_z", m.op, app.v3.gen));
            let lows: std::sync::Arc<Vec<f64>> = ui.data(|d| d.get_temp(cid)).unwrap_or_else(|| {
                let mut v = vec![f64::INFINITY; passes + 1];
                for x in o.moves.iter().filter(|x| x.op == m.op && x.cut && x.pass > 0 && x.pass <= passes) { v[x.pass] = v[x.pass].min(x.from[2]).min(x.to[2]); }
                let v = std::sync::Arc::new(v);
                ui.data_mut(|d| d.insert_temp(cid, v.clone()));
                v
            });
            let zof = |_op: usize, pass: usize| -> Option<f64> { lows.get(pass).copied().filter(|z| z.is_finite()) };
            let fmt = |op: usize, pass: usize| if pass > 0 && passes > 0 {
                match zof(op, pass) { Some(z) => format!("Operation {} · {}  ·  Pass {} of {}  ·  to Z {}", op + 1, name, pass, passes, zfmt(z)), None => format!("Operation {} · {}  ·  Pass {} of {}", op + 1, name, pass, passes) }
            } else { format!("Operation {} · {}", op + 1, name) };
            let hid = egui::Id::new("v3_label");
            let now = ui.input(|i| i.time);
            let (mut shown, since): ((usize, usize), f64) = ui.data(|d| d.get_temp(hid)).unwrap_or(((m.op, pass), -1.0));
            let mut since = since;
            if shown != (m.op, pass) {
                if now - since > 0.3 || shown.0 != m.op && now - since > 0.1 { shown = (m.op, pass); since = now; }
                else { ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)); }
            }
            ui.data_mut(|d| d.insert_temp(hid, (shown, since)));
            let widest = {
                let low = (1..=passes).filter_map(|p| zof(m.op, p)).fold(f64::INFINITY, f64::min);
                let zs = if low.is_finite() { format!("  ·  to Z {}", zfmt(low).replace(|c: char| c.is_ascii_digit(), "8")) } else { String::new() };
                format!("Operation {} · {}  ·  Pass {} of {}{}", m.op + 1, name, "8".repeat(passes.max(1).to_string().len()), passes, zs)
            };
            let wg = painter.layout_no_wrap(widest, FontId::proportional(13.0), Color32::WHITE);
            let g = painter.layout_no_wrap(fmt(shown.0, shown.1), FontId::proportional(13.0), Color32::WHITE);
            let r = Rect::from_min_size(rect.left_top() + Vec2::new(10.0, 10.0), Vec2::new(wg.size().x.max(g.size().x), g.size().y) + Vec2::new(18.0, 10.0));
            painter.rect_filled(r, 6.0, Color32::from_black_alpha(150));
            painter.galley(r.min + Vec2::new(9.0, 5.0), g, Color32::WHITE);
        }
    }

    if app.v3.catching_up {
        let total = app.output.as_ref().map_or(1, |o| o.moves.len()).max(1);
        let pct = (app.v3.sim.as_ref().map_or(0, |s| s.done) * 100 / total).min(99);
        let r = Rect::from_center_size(rect.center(), Vec2::new(240.0, 44.0));
        painter.rect_filled(r, 8.0, Color32::from_black_alpha(170));
        let c = r.left_center() + Vec2::new(26.0, 0.0);
        let a = ui.input(|i| i.time) as f32 * 6.0;
        let pts: Vec<Pos2> = (0..=18).map(|k| { let t = a + k as f32 * 0.2; c + Vec2::new(t.cos(), t.sin()) * 9.0 }).collect();
        painter.add(Shape::line(pts, Stroke::new(2.5_f32, Color32::WHITE)));
        painter.text(c + Vec2::new(24.0, 0.0), egui::Align2::LEFT_CENTER, format!("Preparing 3D preview... {pct}%"), FontId::proportional(13.0), Color32::WHITE);
    }
    if let Some(e) = &app.gen_error {
        let col = Color32::from_rgb(0xe0, 0x1b, 0x24);
        let g = painter.layout(e.clone(), FontId::proportional(13.0), col, (rect.width() - 24.0).max(100.0));
        painter.galley(rect.left_top() + Vec2::new(12.0, 10.0), g, col);
    } else if app.output.as_ref().is_none_or(|o| o.moves.is_empty()) {
        painter.text(rect.center_top() + Vec2::new(0.0, 14.0), egui::Align2::CENTER_TOP, "No toolpaths yet: add an operation to preview the cut", FontId::proportional(13.0), ui.visuals().weak_text_color());
    }
}

// ---------------------------------------------------------------------------------------------
// GPU renderer: a full-screen pass that ray-marches the height texture, so walls and floors are
// shaded per pixel at the simulation's resolution instead of through a coarse mesh.

#[derive(Default)]
pub struct GlRes {
    prog: Option<glow::Program>,
    vao: Option<glow::VertexArray>,
    tex: Option<glow::Texture>,
    dims: (usize, usize),
    tried: bool,
    pub ok: bool,
}

#[derive(Clone, Copy)]
pub struct Uniforms {
    eye: V3, f: V3, r: V3, u: V3, tanh: f32, ortho: f32, bx: V3, cell: f32, n: [i32; 2], wood: V3, floor: V3, side: V3, grain: f32, rings: f32, brush: f32,
}

const VS: &str = "void main(){ vec2 p = vec2(float(gl_VertexID & 1) * 2.0 - 1.0, float(gl_VertexID >> 1) * 2.0 - 1.0); gl_Position = vec4(p, 0.0, 1.0); }";

const FS: &str = r#"
uniform sampler2D u_h;
uniform vec3 u_eye, u_f, u_r, u_u;
uniform vec2 u_org, u_res;
uniform float u_tanh, u_cell, u_ortho;
uniform vec3 u_box, u_wood, u_floor, u_side;
uniform float u_grain, u_rings, u_brush;
uniform ivec2 u_n;
out vec4 o;

float Hc(int i, int j) { return texelFetch(u_h, ivec2(clamp(i, 0, u_n.x - 1), clamp(j, 0, u_n.y - 1)), 0).r; }
float H(vec2 p) {
    vec2 g = p / u_cell; vec2 f = floor(g); vec2 a = g - f; int i = int(f.x), j = int(f.y);
    return mix(mix(Hc(i, j), Hc(i + 1, j), a.x), mix(Hc(i, j + 1), Hc(i + 1, j + 1), a.x), a.y);
}
const vec3 L = vec3(-0.45, -0.55, 0.75) / 1.0;

float hash3(vec3 q) { q = fract(q * 0.3183099 + vec3(0.71, 0.113, 0.419)); q *= 17.0; return fract(q.x * q.y * q.z * (q.x + q.y + q.z)); }
// speckle for foam: pores of about half a millimetre plus coarser blotches
float grain(vec3 p) { return 0.65 * (hash3(floor(p / 0.5)) - 0.5) + 0.35 * (hash3(floor(p / 2.0) + 7.0) - 0.5); }

// stylised wood: soft wavy growth lines running along X, plus long thin fibres
float wood(vec3 p) {
    float w = p.y * 0.55 + 2.2 * sin(p.x * 0.021 + p.y * 0.07) + 0.8 * sin(p.x * 0.067 - p.y * 0.11);
    float lines = sin(w * 2.1 + 0.9 * sin(p.x * 0.05 + w));
    float fibre = hash3(vec3(floor(p.x / 9.0), floor(p.y * 2.2), floor(p.z * 2.2))) - 0.5;
    return 0.65 * lines + 0.35 * fibre;
}

// brushed metal: very fine streaks along X, some of them longer than others
float brushed(vec3 p) {
    float fine = hash3(vec3(floor(p.x / 14.0), floor(p.y * 14.0), floor(p.z * 14.0))) - 0.5;
    float broad = hash3(vec3(floor(p.x / 60.0) + 3.0, floor(p.y * 3.0), floor(p.z * 3.0))) - 0.5;
    return 0.7 * fine + 0.3 * broad;
}

// returns colour in rgb, coverage in a
vec4 shade(vec2 pix) {
    vec2 ndc = pix / u_res * 2.0 - 1.0;
    vec3 d = normalize(u_f + u_r * (ndc.x * u_tanh * u_res.x / u_res.y) + u_u * (ndc.y * u_tanh));
    vec3 ro = u_eye;
    if (u_ortho > 0.0) { d = u_f; ro = u_eye + u_r * (ndc.x * u_ortho * u_res.x / u_res.y) + u_u * (ndc.y * u_ortho); }
    vec3 bmin = vec3(0.0, 0.0, -u_box.z), bmax = vec3(u_box.x, u_box.y, 0.0);
    vec3 inv = 1.0 / (d + vec3(1e-9));
    vec3 a = (bmin - ro) * inv, b = (bmax - ro) * inv;
    vec3 tn = min(a, b), tf = max(a, b);
    float tin = max(max(tn.x, tn.y), tn.z), tout = min(min(tf.x, tf.y), tf.z);
    if (tout <= max(tin, 0.0)) return vec4(0.0);
    vec3 fn = vec3(0.0);
    if (tin == tn.x) fn = vec3(-sign(d.x), 0.0, 0.0); else if (tin == tn.y) fn = vec3(0.0, -sign(d.y), 0.0); else fn = vec3(0.0, 0.0, -sign(d.z));
    float t = tin; vec3 p = ro + d * t;
    bool hit = false, face = false;
    if (p.z <= H(p.xy) + 1e-4) { hit = true; face = fn.z == 0.0; }
    else {
        float prev = t;
        for (int i = 0; i < 1200; i++) {
            float st = u_cell * 0.6 * (1.0 + (t - tin) * 0.004);
            t += st;
            bool last = false;
            if (t > tout) { t = tout; last = true; }
            p = ro + d * t;
            if (p.z <= H(p.xy)) {
                float lo = prev, hi = t;
                for (int k = 0; k < 7; k++) { float m = 0.5 * (lo + hi); vec3 q = ro + d * m; if (q.z <= H(q.xy)) hi = m; else lo = m; }
                t = hi; p = ro + d * t; hit = true; break;
            }
            if (last) break;
            prev = t;
        }
    }
    if (!hit) return vec4(0.0);
    vec3 Ln = normalize(L);
    float flat_ = 0.45 + 0.55 * Ln.z;
    vec3 c;
    if (face) {
        c = u_side * (0.55 + 0.45 * max(dot(fn, Ln), 0.0));
        // darker towards the bottom edge, like a real cross-section
        c *= 1.0 - 0.18 * clamp(-p.z / u_box.z, 0.0, 1.0);
    } else {
        float e = u_cell * 2.2;
        float gx = (H(p.xy + vec2(e, 0.0)) - H(p.xy - vec2(e, 0.0))) / (2.0 * e);
        float gy = (H(p.xy + vec2(0.0, e)) - H(p.xy - vec2(0.0, e))) / (2.0 * e);
        vec3 n = normalize(vec3(-gx, -gy, 1.0));
        float k = (0.45 + 0.55 * max(dot(n, Ln), 0.0)) / flat_;
        k *= 1.0 - 0.32 * clamp(-p.z / u_box.z, 0.0, 1.0);
        bool through = H(p.xy) <= -u_box.z + 1e-3;
        c = (through ? u_floor : u_wood) * k;
    }
    if (u_grain > 0.0) c *= 1.0 + u_grain * 0.28 * grain(p);
    if (u_brush > 0.0 && (face || H(p.xy) > -u_box.z + 1e-3)) c *= 1.0 + u_brush * 0.05 * brushed(face ? vec3(p.x, p.y + p.z * 1.4, p.z) : p);
    if (u_rings > 0.0 && (face || H(p.xy) > -u_box.z + 1e-3)) c *= 1.0 + u_rings * 0.075 * wood(face ? vec3(p.x, p.y + p.z * 1.4, p.z) : p);
    return vec4(c, 1.0);
}

void main() {
    vec2 pix = gl_FragCoord.xy - u_org;
    vec2 offs[4] = vec2[4](vec2(-0.125, -0.375), vec2(0.375, -0.125), vec2(0.125, 0.375), vec2(-0.375, 0.125));
    vec4 acc = vec4(0.0);
    for (int s = 0; s < 4; s++) {
        vec4 r = shade(pix + offs[s]);
        acc += vec4(r.rgb * r.a, r.a);
    }
    acc *= 0.25;
    if (acc.a <= 0.0) discard;
    o = vec4(min(acc.rgb, vec3(1.0)), acc.a);
}
"#;

fn build(gl: &glow::Context, sv: eframe::egui_glow::ShaderVersion) -> Result<(glow::Program, glow::VertexArray), String> {
    unsafe {
        let head = format!("{}{}", sv.version_declaration(), if sv.is_embedded() { "precision highp float;\nprecision highp int;\nprecision highp sampler2D;\n" } else { "" });
        let mk = |kind: u32, src: &str| -> Result<glow::Shader, String> {
            let s = gl.create_shader(kind)?;
            gl.shader_source(s, &format!("{head}{src}"));
            gl.compile_shader(s);
            if !gl.get_shader_compile_status(s) { return Err(gl.get_shader_info_log(s)); }
            Ok(s)
        };
        let vs = mk(glow::VERTEX_SHADER, VS)?;
        let fs = mk(glow::FRAGMENT_SHADER, FS)?;
        let p = gl.create_program()?;
        gl.attach_shader(p, vs); gl.attach_shader(p, fs);
        gl.link_program(p);
        if !gl.get_program_link_status(p) { return Err(gl.get_program_info_log(p)); }
        gl.delete_shader(vs); gl.delete_shader(fs);
        let vao = gl.create_vertex_array()?;
        Ok((p, vao))
    }
}

fn draw_gl(res: &std::sync::Mutex<GlRes>, pending: &std::sync::Mutex<Option<(usize, usize, Vec<f32>)>>, u: &Uniforms, info: &egui::PaintCallbackInfo, painter: &eframe::egui_glow::Painter) {
    let gl: &glow::Context = painter.gl();
    let mut r = res.lock().unwrap();
    if !r.tried {
        r.tried = true;
        let sv = eframe::egui_glow::ShaderVersion::get(gl);
        if sv.is_new_shader_interface() {
            match build(gl, sv) {
                Ok((p, v)) => { r.prog = Some(p); r.vao = Some(v); r.ok = true; }
                Err(e) => eprintln!("3D preview shader failed: {e}"),
            }
        } else { eprintln!("3D preview needs OpenGL 3.1 or newer"); }
    }
    if !r.ok { return; }
    unsafe {
        gl.active_texture(glow::TEXTURE1);
        if r.tex.is_none() { r.tex = gl.create_texture().ok(); }
        gl.bind_texture(glow::TEXTURE_2D, r.tex);
        if let Some((nx, ny, z)) = pending.lock().unwrap().take() {
            let bytes = std::slice::from_raw_parts(z.as_ptr() as *const u8, z.len() * 4);
            if r.dims != (nx, ny) {
                gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::R32F as i32, nx as i32, ny as i32, 0, glow::RED, glow::FLOAT, Some(bytes));
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
                r.dims = (nx, ny);
            } else {
                gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, nx as i32, ny as i32, glow::RED, glow::FLOAT, glow::PixelUnpackData::Slice(bytes));
            }
        }
        if r.dims == (0, 0) { gl.active_texture(glow::TEXTURE0); return; }
        let vp = info.viewport_in_pixels();
        let prog = r.prog.unwrap();
        gl.use_program(Some(prog));
        let loc = |n: &str| gl.get_uniform_location(prog, n);
        gl.uniform_1_i32(loc("u_h").as_ref(), 1);
        gl.uniform_3_f32_slice(loc("u_eye").as_ref(), &u.eye);
        gl.uniform_3_f32_slice(loc("u_f").as_ref(), &u.f);
        gl.uniform_3_f32_slice(loc("u_r").as_ref(), &u.r);
        gl.uniform_3_f32_slice(loc("u_u").as_ref(), &u.u);
        gl.uniform_2_f32(loc("u_org").as_ref(), vp.left_px as f32, vp.from_bottom_px as f32);
        gl.uniform_2_f32(loc("u_res").as_ref(), vp.width_px as f32, vp.height_px as f32);
        gl.uniform_1_f32(loc("u_tanh").as_ref(), u.tanh);
        gl.uniform_1_f32(loc("u_ortho").as_ref(), u.ortho);
        gl.uniform_1_f32(loc("u_cell").as_ref(), u.cell);
        gl.uniform_3_f32_slice(loc("u_box").as_ref(), &u.bx);
        gl.uniform_3_f32_slice(loc("u_wood").as_ref(), &u.wood);
        gl.uniform_3_f32_slice(loc("u_floor").as_ref(), &u.floor);
        gl.uniform_3_f32_slice(loc("u_side").as_ref(), &u.side);
        gl.uniform_1_f32(loc("u_grain").as_ref(), u.grain);
        gl.uniform_1_f32(loc("u_rings").as_ref(), u.rings);
        gl.uniform_1_f32(loc("u_brush").as_ref(), u.brush);
        gl.uniform_2_i32(loc("u_n").as_ref(), u.n[0], u.n[1]);
        gl.bind_vertex_array(r.vao);
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        gl.bind_vertex_array(None);
        gl.use_program(None);
        gl.active_texture(glow::TEXTURE0);
    }
}
