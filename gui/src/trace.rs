//! Import Bitmap: a picture is traced into closed shapes (see `putiacnc_core::bitmap`). The dialog shows the picture
//! with the traced outlines on top, and the shapes are placed on the stock like any other imported drawing.
use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use putiacnc_core::bitmap::{Bitmap, Mode, Opts};
use putiacnc_core::svg::Imported;
use std::sync::{mpsc, Arc};

type Traced = Result<Imported, String>;

#[derive(Default)]
pub struct TraceTool {
    pub open: bool,
    pub name: String,
    pub bm: Option<Arc<Bitmap>>,
    pub opts: Opts,
    pub tex: Option<egui::TextureHandle>,
    /// The latest finished trace. While a newer one is being worked out it is still drawn, faded.
    pub result: Option<Traced>,
    /// The settings changed since the running trace (or the shown one) was started.
    pub dirty: bool,
    /// The trace running on a worker thread (one at a time); its answer arrives here.
    pub rx: Option<mpsc::Receiver<Traced>>,
    /// When the running trace started: the busy sign over the preview only shows once the wait is long enough to notice.
    pub started: Option<std::time::Instant>,
    pub err: Option<String>,
    /// The settings to remember for next time (see `Settings::trace`).
    pub remember: Option<Opts>,
}

impl TraceTool {
    pub fn load(&mut self, path: &str, stock: (f64, f64), saved: Option<Opts>) {
        match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| Bitmap::load(&b)) {
            Ok(bm) => {
                self.name = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "bitmap".into());
                // a good starting size: as large as fits in 80% of the stock
                let aspect = bm.h as f64 / bm.w as f64;
                let w = (stock.0 * 0.8).min(stock.1 * 0.8 / aspect);
                // the settings of last time, but the size is fitted to this picture
                self.opts = Opts { width_mm: (w * 10.0).round() / 10.0, ..saved.unwrap_or_default() };
                self.bm = Some(Arc::new(bm));
                self.tex = None;
                self.result = None;
                self.rx = None; self.started = None; // an answer still on its way for the previous picture is dropped
                self.dirty = true;
                self.err = None;
                self.open = true;
            }
            Err(e) => self.err = Some(format!("Could not open {path}: {e}")),
        }
    }
    pub fn take_error(&mut self) -> Option<String> { self.err.take() }

    /// A trace is running or waiting to start: what is shown does not match the settings yet.
    pub fn working(&self) -> bool { self.dirty || self.rx.is_some() }

    /// Collect a finished trace and start the next one when the settings changed meanwhile. The window never waits for a trace.
    fn pump(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(r) => { self.result = Some(r); self.rx = None; self.started = None; }
                Err(mpsc::TryRecvError::Disconnected) => { self.rx = None; self.started = None; }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.dirty && self.rx.is_none() {
            if let Some(bm) = self.bm.clone() {
                let (tx, rx) = mpsc::channel();
                let (opts, ctx2) = (self.opts.clone(), ctx.clone());
                self.rx = Some(rx);
                self.started = Some(std::time::Instant::now());
                self.dirty = false;
                std::thread::spawn(move || { let _ = tx.send(bm.trace(&opts)); ctx2.request_repaint(); });
            }
        }
        if self.rx.is_some() { ctx.request_repaint_after(std::time::Duration::from_millis(100)); }
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if let Some(e) = app.trace.take_error() { app.error = Some(e); }
    if !app.trace.open { return; }
    let (mut place, mut cancel) = (false, false);
    {
        let t = &mut app.trace;
        let Some(bm) = t.bm.clone() else { t.open = false; return };
        if t.tex.is_none() {
            let px = bm.luma.iter().map(|&v| Color32::from_gray(v)).collect();
            t.tex = Some(ctx.load_texture("trace_src", egui::ColorImage { size: [bm.w, bm.h], pixels: px }, egui::TextureOptions::LINEAR));
        }
        t.pump(ctx);
    }
    // as large as the window allows: the picture is what the settings are judged by
    let (dw, ph) = { let r = ctx.screen_rect(); ((r.width() - 140.0).clamp(380.0, 660.0), (r.height() - 90.0 - 300.0).clamp(200.0, 460.0)) };
    egui::Window::new("Import bitmap").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Import bitmap", egui::pos2(560.0, 172.0))).show(ctx, |ui| {
        ui.set_width(dw);
        if crate::csd::dialog_header(ui, "Import bitmap", true) { cancel = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        let t = &mut app.trace;
        let (bw, bh) = t.bm.as_ref().map(|b| (b.w as f32, b.h as f32)).unwrap_or((1.0, 1.0));
        // the picture, dimmed, with the traced outlines on top
        let size = Vec2::new(dw, (dw * bh / bw).min(ph));
        let scale = (size.x / bw).min(size.y / bh);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(dw, size.y), Sense::hover());
        let img = Rect::from_center_size(rect.center(), Vec2::new(bw * scale, bh * scale));
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
        if let Some(tex) = &t.tex { painter.image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::from_white_alpha(110)); }
        // the part that is traced; what is cropped or trimmed away is darkened
        let (x0, y0, x1, y1) = t.bm.as_ref().map(|b| b.region(&t.opts)).unwrap_or((0, 0, bw as usize, bh as usize));
        let reg = Rect::from_min_max(Pos2::new(img.min.x + x0 as f32 * scale, img.min.y + y0 as f32 * scale), Pos2::new(img.min.x + x1 as f32 * scale, img.min.y + y1 as f32 * scale));
        if reg != img {
            let shade = Color32::from_black_alpha(120);
            for r in [
                Rect::from_min_max(img.min, Pos2::new(img.max.x, reg.min.y)), Rect::from_min_max(Pos2::new(img.min.x, reg.max.y), img.max),
                Rect::from_min_max(Pos2::new(img.min.x, reg.min.y), Pos2::new(reg.min.x, reg.max.y)), Rect::from_min_max(Pos2::new(reg.max.x, reg.min.y), Pos2::new(img.max.x, reg.max.y)),
            ] { painter.rect_filled(r, 0.0, shade); }
            painter.rect_stroke(reg, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(0x30, 0x90, 0xe0)));
        }
        if let Some(Ok(imp)) = &t.result {
            let k = reg.width() / imp.width_mm as f32;
            let to_s = |x: i64, y: i64| Pos2::new(reg.min.x + x as f32 / 1000.0 * k, reg.max.y - y as f32 / 1000.0 * k);
            // faded while a newer trace is on its way: it no longer matches the settings
            let st = Stroke::new(1.4_f32, if t.working() { Color32::from_rgba_unmultiplied(0xe0, 0x30, 0x30, 90) } else { Color32::from_rgb(0xe0, 0x30, 0x30) });
            for l in &imp.loops {
                let mut pts: Vec<Pos2> = l.pts.chunks_exact(2).map(|c| to_s(c[0], c[1])).collect();
                if l.closed() { if let Some(f) = pts.first().copied() { pts.push(f); } }
                painter.add(egui::Shape::line(pts, st));
            }
        }
        // a trace that takes a moment: the preview is dimmed and says so (quick ones show nothing)
        if t.working() && t.started.is_some_and(|s| s.elapsed().as_secs_f32() > 0.3) {
            let dark = ui.visuals().dark_mode;
            painter.rect_filled(rect, 4.0, Color32::from_black_alpha(if dark { 125 } else { 70 }));
            let pill = Rect::from_center_size(rect.center(), Vec2::new(170.0, 44.0));
            painter.rect(pill, 12.0, ui.visuals().window_fill().gamma_multiply(0.95), ui.visuals().widgets.noninteractive.bg_stroke);
            egui::Spinner::new().size(18.0).paint_at(ui, Rect::from_center_size(Pos2::new(pill.left() + 32.0, pill.center().y), Vec2::splat(18.0)));
            painter.text(Pos2::new(pill.left() + 55.0, pill.center().y), egui::Align2::LEFT_CENTER, "Tracing\u{2026}", egui::TextStyle::Body.resolve(ui.style()), ui.visuals().text_color());
        }
        ui.add_space(8.0);
        let mut changed = false;
        // the label columns are as wide in both grids, so the labels line up
        let col1 = ["Trace", "Threshold", "Width", "Edge trim"].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0_f32, f32::max);
        let col3 = crate::panels::tw(ui, "Ignore specks").max(crate::panels::tw(ui, "Height"));
        egui::Grid::new("trace_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            crate::toolmgr::rlabel(ui, col1, "Trace");
            ui.horizontal(|ui| {
                let (c, _) = crate::zoomcard::seg2(ui, ["Outlines", "Centre line"], ["Closed outlines of the shapes, holes included: for Profile, Pocket, V-carve", "One open line along the middle of each stroke: for engraving lines and writing"], (t.opts.mode == Mode::Centerline) as usize);
                if let Some(i) = c { let m = if i == 0 { Mode::Outline } else { Mode::Centerline }; if t.opts.mode != m { t.opts.mode = m; changed = true; } }
            });
            ui.end_row();
            crate::toolmgr::rlabel(ui, col1, "Threshold");
            let mut th = t.opts.threshold as i32;
            // the slider takes the rest of the row, up to the number box
            ui.spacing_mut().slider_width = (ui.available_width() - 64.0).max(80.0);
            if ui.add(egui::Slider::new(&mut th, 1..=254)).on_hover_text("Pixels darker than this become part of the shape").changed() { t.opts.threshold = th as u8; changed = true; }
            ui.end_row();
        });
        ui.add_space(8.0);
        // number fields in pairs: label, field, label, field
        egui::Grid::new("trace_grid2").num_columns(4).spacing([10.0, 8.0]).show(ui, |ui| {
            let (_, hmm) = t.bm.as_ref().map(|b| b.size_mm(&t.opts)).unwrap_or((0.0, 0.0));
            crate::toolmgr::rlabel(ui, col1, "Width");
            ui.horizontal(|ui| { changed |= crate::widgets::num(ui, [66.0, 24.0], &mut t.opts.width_mm, 1.0, 5000.0, 0.5, 2, "").changed(); ui.weak("mm"); });
            crate::toolmgr::rlabel(ui, col3, "Height");
            ui.weak(format!("{hmm:.2} mm"));
            ui.end_row();
            crate::toolmgr::rlabel(ui, col1, "Edge trim");
            ui.horizontal(|ui| { changed |= crate::widgets::num(ui, [66.0, 24.0], &mut t.opts.trim_pct, 0.0, 45.0, 0.1, 1, "").on_hover_text("Ignore this part of every edge, for a scanner border or a frame").changed(); ui.weak("%"); });
            crate::toolmgr::rlabel(ui, col3, "Ignore specks");
            ui.horizontal(|ui| { changed |= crate::widgets::num(ui, [66.0, 24.0], &mut t.opts.speck_mm2, 0.0, 10000.0, 0.1, 1, "").on_hover_text("Shapes this small (or smaller) are left out").changed(); ui.weak("mm\u{b2}"); });
            ui.end_row();
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut t.opts.crop, "Crop to content").on_hover_text("Cut the picture down to the area that has something in it").changed();
            ui.add_space(8.0);
            changed |= ui.checkbox(&mut t.opts.invert, "Invert").on_hover_text("Trace the light areas instead of the dark ones").changed();
            ui.add_space(8.0);
            changed |= ui.checkbox(&mut t.opts.smooth, "Smooth curves").on_hover_text("Off keeps straight segments that follow the pixels").changed();
            // what the trace found, at the right end of the row
            let count = match &t.result { Some(Ok(i)) => i.loops.len(), _ => 0 };
            let (txt, tip) = if t.opts.mode == Mode::Outline { (format!("{count} outlines"), "Holes included") } else { (format!("{count} lines"), "Centre lines found") };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.weak(txt).on_hover_text(tip); });
        });
        if changed {
            t.dirty = true;
            t.remember = Some(Opts { width_mm: 100.0, ..t.opts.clone() });
        }
        let count = match &t.result { Some(Ok(i)) => i.loops.len(), _ => 0 };
        match &t.result {
            Some(Ok(i)) if !i.warnings.is_empty() => { ui.add_space(4.0); ui.colored_label(Color32::from_rgb(0xe0, 0x8a, 0x2a), i.warnings.join(" ")); }
            Some(Err(e)) => { ui.add_space(4.0); ui.colored_label(Color32::from_rgb(0xe0, 0x30, 0x30), e.as_str()); }
            _ => {}
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(count > 0 && !app.trace.working(), egui::Button::new("Import")).clicked() { place = true; }
            if ui.button("Cancel").clicked() { cancel = true; }
        });
        });
    });
    if place {
        if let Some(Ok(imp)) = app.trace.result.take() {
            let name = app.trace.name.clone();
            app.place_import(imp, name);
        }
        app.trace.open = false;
        app.trace.tex = None;
        app.trace.bm = None;
        app.trace.rx = None;
    }
    if cancel { app.trace.open = false; app.trace.tex = None; app.trace.bm = None; app.trace.rx = None; }
}
