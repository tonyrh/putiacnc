//! Import Bitmap: a picture is traced into closed shapes (see `putiacnc_core::bitmap`). The dialog shows the picture
//! with the traced outlines on top, and the shapes are placed on the stock like any other imported drawing.
use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use putiacnc_core::bitmap::{Bitmap, Opts};
use putiacnc_core::svg::Imported;

#[derive(Default)]
pub struct TraceTool {
    pub open: bool,
    pub name: String,
    pub bm: Option<Bitmap>,
    pub opts: Opts,
    pub tex: Option<egui::TextureHandle>,
    pub result: Option<Result<Imported, String>>,
    pub dirty: bool,
    pub err: Option<String>,
}

impl TraceTool {
    pub fn load(&mut self, path: &str, stock: (f64, f64)) {
        match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| Bitmap::load(&b)) {
            Ok(bm) => {
                self.name = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "bitmap".into());
                // a good starting size: as large as fits in 80% of the stock
                let aspect = bm.h as f64 / bm.w as f64;
                let w = (stock.0 * 0.8).min(stock.1 * 0.8 / aspect);
                self.opts = Opts { width_mm: (w * 10.0).round() / 10.0, ..Opts::default() };
                self.bm = Some(bm);
                self.tex = None;
                self.result = None;
                self.dirty = true;
                self.err = None;
                self.open = true;
            }
            Err(e) => self.err = Some(format!("Could not open {path}: {e}")),
        }
    }
    pub fn take_error(&mut self) -> Option<String> { self.err.take() }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if let Some(e) = app.trace.take_error() { app.error = Some(e); }
    if !app.trace.open { return; }
    let (mut place, mut cancel) = (false, false);
    {
        let t = &mut app.trace;
        let Some(bm) = t.bm.as_ref() else { t.open = false; return };
        if t.tex.is_none() {
            let px = bm.luma.iter().map(|&v| Color32::from_gray(v)).collect();
            t.tex = Some(ctx.load_texture("trace_src", egui::ColorImage { size: [bm.w, bm.h], pixels: px }, egui::TextureOptions::LINEAR));
        }
        if t.dirty {
            t.result = Some(bm.trace(&t.opts));
            t.dirty = false;
        }
    }
    egui::Window::new("Import bitmap").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).resizable(false)
        .default_pos([560.0, 172.0]).fixed_size([380.0, 0.0]).show(ctx, |ui| {
        if crate::csd::dialog_header(ui, "Import bitmap", true) { cancel = true; }
        let t = &mut app.trace;
        let (bw, bh) = t.bm.as_ref().map(|b| (b.w as f32, b.h as f32)).unwrap_or((1.0, 1.0));
        // the picture, dimmed, with the traced outlines on top
        let size = Vec2::new(360.0, (360.0 * bh / bw).min(230.0)); 
        let scale = (size.x / bw).min(size.y / bh);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(360.0, size.y), Sense::hover());
        let img = Rect::from_center_size(rect.center(), Vec2::new(bw * scale, bh * scale));
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
        if let Some(tex) = &t.tex { painter.image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::from_white_alpha(110)); }
        if let Some(Ok(imp)) = &t.result {
            let (w, h) = (imp.width_mm as f32, imp.height_mm as f32);
            let k = img.width() / w;
            let to_s = |x: i64, y: i64| Pos2::new(img.min.x + x as f32 / 1000.0 * k, img.max.y - y as f32 / 1000.0 * k);
            let _ = h;
            let st = Stroke::new(1.4_f32, Color32::from_rgb(0xe0, 0x30, 0x30));
            for l in &imp.loops {
                let mut pts: Vec<Pos2> = l.pts.chunks_exact(2).map(|c| to_s(c[0], c[1])).collect();
                if let Some(f) = pts.first().copied() { pts.push(f); }
                painter.add(egui::Shape::line(pts, st));
            }
        }
        ui.add_space(8.0);
        let mut changed = false;
        egui::Grid::new("trace_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label("Threshold");
            let mut th = t.opts.threshold as i32;
            if ui.add(egui::Slider::new(&mut th, 1..=254)).on_hover_text("Pixels darker than this become part of the shape").changed() { t.opts.threshold = th as u8; changed = true; }
            ui.end_row();
            ui.label("Width");
            let (_, hmm) = t.bm.as_ref().map(|b| b.size_mm(&t.opts)).unwrap_or((0.0, 0.0));
            ui.horizontal(|ui| {
                changed |= crate::widgets::num(ui, [66.0, 24.0], &mut t.opts.width_mm, 1.0, 5000.0, 0.5, 2, "").changed();
                ui.add_space(4.0);
                ui.weak(format!("mm   Height {:.2} mm", hmm));
            });
            ui.end_row();
            ui.label("Ignore specks");
            ui.horizontal(|ui| {
                changed |= crate::widgets::num(ui, [66.0, 24.0], &mut t.opts.speck_mm2, 0.0, 10000.0, 0.1, 1, "").changed();
                ui.add_space(4.0);
                ui.weak("mm² or smaller");
            });
            ui.end_row();
            ui.label("");
            ui.horizontal(|ui| {
                changed |= ui.checkbox(&mut t.opts.invert, "Invert").on_hover_text("Trace the light areas instead of the dark ones").changed();
                ui.add_space(10.0);
                changed |= ui.checkbox(&mut t.opts.smooth, "Smooth curves").on_hover_text("Off keeps straight segments that follow the pixels").changed();
            });
            ui.end_row();
        });
        if changed { t.dirty = true; }
        ui.add_space(4.0);
        let count = match &t.result { Some(Ok(i)) => i.loops.len(), _ => 0 };
        match &t.result {
            Some(Ok(i)) if !i.warnings.is_empty() => { ui.colored_label(Color32::from_rgb(0xe0, 0x8a, 0x2a), i.warnings.join(" ")); }
            Some(Err(e)) => { ui.colored_label(Color32::from_rgb(0xe0, 0x30, 0x30), e.as_str()); }
            _ => { ui.weak(format!("{count} outlines (holes included).")); }
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(count > 0, egui::Button::new("Import")).clicked() { place = true; }
            if ui.button("Cancel").clicked() { cancel = true; }
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
    }
    if cancel { app.trace.open = false; app.trace.tex = None; app.trace.bm = None; }
}
