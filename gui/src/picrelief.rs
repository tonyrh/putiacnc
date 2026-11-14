//! The dialog of a Photo V-carve picture: the picture is read as a tone map (light = not cut, dark = cut deep) and shown as the 3D model dialog
//! does; it has the few settings that decide how the tones are read. Importing places the picture and adds a Photo V-carve operation.
use putiacnc_core::i18n::{tr, tr_f};
use crate::widgets::Tip;
use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use putiacnc_core::model::Relief;
use putiacnc_core::relief::{shrink_picture, PicParams};

#[derive(Default)]
pub struct PicRelief {
    pub open: bool,
    name: String,
    bytes: Vec<u8>,
    /// Height divided by width of the picture.
    aspect: f64,
    pub p: PicParams,
    /// The settings were chosen once, so a second picture keeps them (the size is always made to fit the stock).
    used: bool,
    tex: Option<egui::TextureHandle>,
    err: Option<String>,
    dirty: bool,
    /// Pending message for the main window (a file that could not be read).
    pub error: Option<String>,
}

impl PicRelief {
    /// `stock` is the width, height and thickness of the stock, for the first size and height offered.
    pub fn load(&mut self, path: &str, stock: (f64, f64, f64)) {
        let bytes = match std::fs::read(path) { Ok(b) => b, Err(e) => { self.error = Some(tr_f("Could not read {path}: {e}", &[("path", &path), ("e", &e)])); return; } };
        let bytes = match shrink_picture(&bytes, 1500) { Ok(b) => b, Err(e) => { self.error = Some(tr_f("Could not import {path}: {e}", &[("path", &path), ("e", &e)])); return; } };
        let probe = match Relief::from_picture(&bytes, "", &PicParams { width_mm: 100.0, ..Default::default() }, 400) { Ok(r) => r, Err(e) => { self.error = Some(tr_f("Could not import {path}: {e}", &[("path", &path), ("e", &e)])); return; } };
        self.aspect = probe.ny as f64 / probe.nx as f64;
        self.name = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "picture".into());
        self.bytes = bytes;
        if !self.used { self.p = PicParams { smooth_mm: 0.15, zmax: 1.0, ..PicParams::default() }; }
        // as large as the stock allows with 5 mm all round, so the finishing cutter, which runs past the edge of the relief, stays on the stock
        self.p.width_mm = (((stock.0 - 10.0).min((stock.1 - 10.0) / self.aspect).max(1.0)) * 10.0).floor() / 10.0;
        self.tex = None;
        self.err = None;
        self.dirty = true;
        self.open = true;
    }
    fn close(&mut self) { self.open = false; self.bytes = Vec::new(); self.tex = None; }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if let Some(e) = app.picrelief.error.take() { app.say(crate::app::Level::Error, e); }
    if !app.picrelief.open { return; }
    let (mut place, mut cancel) = (false, false);
    {
        let t = &mut app.picrelief;
        if t.dirty {
            t.dirty = false;
            match Relief::from_picture(&t.bytes, &t.name, &t.p, 300) {
                Ok(r) => {
                    let h = r.heights();
                    let mut px = vec![Color32::BLACK; r.nx * r.ny];
                    for y in 0..r.ny { for x in 0..r.nx {
                        let v = (55.0 + 185.0 * h[y * r.nx + x]) as u8;
                        px[(r.ny - 1 - y) * r.nx + x] = Color32::from_gray(v);
                    } }
                    t.tex = Some(ctx.load_texture("picrelief_preview", egui::ColorImage { size: [r.nx, r.ny], pixels: px }, egui::TextureOptions::LINEAR));
                    t.err = None;
                }
                Err(e) => { t.tex = None; t.err = Some(e); }
            }
        }
    }
    egui::Window::new("V-carve picture as photo").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "V-carve picture as photo", egui::pos2(560.0, 150.0))).show(ctx, crate::csd::trapped(|ui| {
        let dw = crate::csd::dialog_w(380.0);
        ui.set_width(dw);
        if crate::csd::dialog_header_help(ui, tr("V-carve picture as photo"), true, |ui| { ui.label(tr("The darker a part of the picture, the wider and deeper the groove. The picture below shows the tones that will be etched.")); }) { cancel = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        let t = &mut app.picrelief;
        // the dialog is wider in a language with longer words: the picture and the slider take the width there is
        let cw = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(cw, 230.0), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
        if let Some(tex) = &t.tex {
            let s = tex.size_vec2();
            let k = (rect.width() / s.x).min(rect.height() / s.y);
            let img = Rect::from_center_size(rect.center(), s * k);
            painter.image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        ui.add_space(8.0);
        let mut changed = false;
        let lw = [tr("Width"), tr("Midtones"), tr("Smoothing")].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0, f32::max);
        egui::Grid::new("picrelief_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            crate::toolmgr::rlabel(ui, lw, tr("Width"));
            ui.horizontal(|ui| {
                let aspect = t.aspect;
                if crate::widgets::len(ui, [66.0, 24.0], &mut t.p.width_mm, 1.0, 5000.0, 0.5, 2, "").tip(tr("How wide the relief is on the stock; its length follows the picture")).changed() { changed = true; }
                ui.weak(crate::widgets::len_unit(ui));
                ui.weak(crate::widgets::localize(ui, &tr_f("× {h} mm", &[("h", &format!("{:.1}", t.p.width_mm * aspect))])));
            });
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, tr("Midtones"));
            ui.spacing_mut().slider_width = (cw - lw - 10.0 - 64.0 - 8.0).max(80.0);
            if ui.add(egui::Slider::new(&mut t.p.gamma, 0.3..=3.0).logarithmic(true)).tip(tr("Below 1 lifts the shadows, above 1 deepens them")).changed() { changed = true; }
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, tr("Smoothing"));
            ui.horizontal(|ui| {
                if crate::widgets::len(ui, [66.0, 24.0], &mut t.p.smooth_mm, 0.0, 10.0, 0.05, 2, "").tip(tr("Blur the picture by this much first: it hides the grain and the noise that a cutter would only follow")).changed() { changed = true; }
                ui.weak(crate::widgets::len_unit(ui));
            });
            ui.end_row();
        });
        {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                changed |= crate::widgets::check(ui, &mut t.p.levels, tr("Stretch tones")).tip(tr("Use the whole height: the darkest and the lightest part of the picture become the lowest and the highest place")).changed();
                ui.add_space(10.0);
                changed |= crate::widgets::check(ui, &mut t.p.invert, tr("Invert")).tip(tr("Dark is high instead of light, as in a lithophane that is thick where it is dark")).changed();
            });
        }
        if changed { t.dirty = true; }
        ui.add_space(4.0);
        if let Some(e) = &t.err { ui.colored_label(Color32::from_rgb(0xe0, 0x30, 0x30), e.as_str()); }
        else { ui.weak(crate::widgets::localize(ui, &tr_f("{w} × {h} mm.", &[("w", &format!("{:.1}", t.p.width_mm)), ("h", &format!("{:.1}", t.p.width_mm * t.aspect))]))); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(t.err.is_none(), egui::Button::new(tr("Import"))).clicked() { place = true; }
            if ui.button(tr("Cancel")).clicked() { cancel = true; }
        });
        });
    }));
    if place {
        let t = &mut app.picrelief;
        let rel = Relief::from_picture(&t.bytes, &t.name, &t.p, 600);
        t.used = true;
        t.close();
        match rel {
            Ok(r) => {
                app.place_relief(r);
                let sel = Default::default();
                app.hist.record(&app.project.data);
                putiacnc_core::shapes::add_operation(&mut app.project.data, &sel, "photo");
                app.touch();
            }
            Err(e) => app.say(crate::app::Level::Error, tr_f("Could not import the picture: {e}", &[("e", &e)])),
        }
    }
    if cancel { app.picrelief.close(); }
}
