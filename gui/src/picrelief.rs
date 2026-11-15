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
    /// The size (points) the preview was drawn for; a different width of the dialog draws it again.
    view: Vec2,
    /// What the preview shows about the cutter: its description, the lines' distance and the depth.
    info: Option<(String, f64, f64)>,
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
    let tools = app.project.data.tools.clone();
    let tool_name = putiacnc_core::shapes::photo_tool(&tools).map(|t| t.name.clone()).unwrap_or_else(|| tr("a 30° V-bit").to_string());
    {
        let t = &mut app.picrelief;
        if t.dirty {
            t.dirty = false;
            match Relief::from_picture(&t.bytes, &t.name, &t.p, 600) {
                Ok(r) => {
                    // the etching as it will look, with the bit a new Photo V-carve starts with and its default depth and spacing
                    let geo = putiacnc_core::shapes::photo_tool(&tools).map(putiacnc_core::toolpath::ToolGeo::new);
                    let groove = move |d: f64| match geo { Some(g) => g.r_at(d), None => d * (15.0f64).to_radians().tan() };
                    let depth = 1.5;
                    let spacing = 2.0 * groove(depth);
                    let (mw, mh) = r.size();
                    let ppp = ctx.pixels_per_point() as f64;
                    // drawn three times finer than shown and then reduced, so the lines blend into tones as they do on the wood
                    let shown = (t.view.x as f64 * ppp / mw).min(t.view.y as f64 * ppp / mh).max(0.5);
                    let ppm = (shown * 3.0).min((1.5e6 / (mw * mh)).sqrt());
                    let (nx, ny, ink) = putiacnc_core::photo::etch_coverage(&r, depth, spacing, &groove, false, ppm);
                    let (dw, dh) = (((mw * shown).round() as u32).max(1), ((mh * shown).round() as u32).max(1));
                    let small = putiacnc_core::photo::reduce(nx, ny, &ink, dw as usize, dh as usize);
                    let (wood, dark) = ([222.0f32, 184.0, 135.0], [70.0f32, 42.0, 24.0]);
                    let px: Vec<Color32> = small.iter().map(|&k| { let k = k.clamp(0.0, 1.0); let c = |i: usize| (wood[i] + (dark[i] - wood[i]) * k) as u8; Color32::from_rgb(c(0), c(1), c(2)) }).collect();
                    t.tex = Some(ctx.load_texture("picrelief_preview", egui::ColorImage { size: [dw as usize, dh as usize], pixels: px }, egui::TextureOptions::LINEAR));
                    t.info = Some((tool_name.clone(), spacing, depth));
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
        if crate::csd::dialog_header_help(ui, tr("V-carve picture as photo"), true, |ui| { ui.label(tr("The darker a part of the picture, the wider and deeper the groove. The picture below shows how it will look etched in wood.")); }) { cancel = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        let t = &mut app.picrelief;
        // the dialog is wider in a language with longer words: the picture and the slider take the width there is
        let cw = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(cw, 320.0), Sense::hover());
        // a different size of the preview is drawn again at the next frame
        if (rect.size() - t.view).abs().max_elem() > 1.0 { t.view = rect.size(); t.dirty = true; }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, Color32::from_rgb(222, 184, 135));
        if let Some(tex) = &t.tex {
            let s = tex.size_vec2();
            let k = (rect.width() / s.x).min(rect.height() / s.y);
            let img = Rect::from_center_size(rect.center(), s * k);
            painter.image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        ui.add_space(8.0);
        let mut changed = false;
        let lw = [tr("Width"), tr("Midtones"), tr("Contrast"), tr("Brightness"), tr("Lighten background"), tr("Smoothing")].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0, f32::max);
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
            crate::toolmgr::rlabel(ui, lw, tr("Contrast"));
            if ui.add(egui::Slider::new(&mut t.p.contrast, 0.3..=3.0).logarithmic(true)).tip(tr("Above 1 pulls the light and the dark apart, below 1 brings them together")).changed() { changed = true; }
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, tr("Brightness"));
            if ui.add(egui::Slider::new(&mut t.p.brightness, -0.5..=0.5)).tip(tr("Lighter pictures are etched less deeply, darker ones more")).changed() { changed = true; }
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, tr("Lighten background"));
            if ui.add(egui::Slider::new(&mut t.p.lighten_bg, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)).custom_parser(|s| s.trim().trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0)))
                .tip(tr("Makes the plain backdrop behind the subject lighter, so that it is etched less; at 100 % it is not etched at all. The backdrop is found from the top of the picture, so check the preview")).changed() { changed = true; }
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
        if let (None, Some((tool, spacing, depth))) = (&t.err, &t.info) {
            ui.weak(crate::widgets::localize(ui, &tr_f("Shown with {tool}: lines {s} apart, up to {d} deep. Change them in the operation.", &[("tool", tool), ("s", &crate::widgets::fmt_len(ui, *spacing)), ("d", &crate::widgets::fmt_len(ui, *depth))])));
        }
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
