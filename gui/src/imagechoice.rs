//! The question asked when a picture is imported: trace its outlines into shapes, show it behind the drawing as a
//! backdrop, or both (the traced shapes then land exactly on the backdrop).
use crate::app::App;
use crate::widgets::Tip;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use putiacnc_core::backdrop::Backdrop;
use putiacnc_core::i18n::tr;

#[derive(Default)]
pub struct ImageChoice {
    pub open: bool,
    pub path: String,
    pub size: (usize, usize),
    pixels: Option<Vec<u8>>,
    tex: Option<egui::TextureHandle>,
    err: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Pick { Trace, Backdrop, Both }

impl ImageChoice {
    pub fn load(&mut self, path: &str) {
        let r = std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| Backdrop::preview(&b, 640));
        match r {
            Ok((w, h, px)) => { self.path = path.to_string(); self.size = (w, h); self.pixels = Some(px); self.tex = None; self.err = None; self.open = true; }
            Err(e) => { self.err = Some(putiacnc_core::i18n::tr_f("Could not open {path}: {e}", &[("path", &path), ("e", &e)])); }
        }
    }
    pub fn take_error(&mut self) -> Option<String> { self.err.take() }
    fn close(&mut self) { self.open = false; self.tex = None; self.pixels = None; }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if let Some(e) = app.pic.take_error() { app.say(crate::app::Level::Error, e); }
    if !app.pic.open { return; }
    if app.pic.tex.is_none() {
        if let Some(px) = &app.pic.pixels {
            let (w, h) = app.pic.size;
            let img = egui::ColorImage::from_rgba_unmultiplied([w, h], px);
            app.pic.tex = Some(ctx.load_texture("pic_choice", img, egui::TextureOptions::LINEAR));
        }
    }
    let mut pick: Option<Pick> = None;
    let mut cancel = false;
    let dw = crate::csd::dialog_w(440.0);
    egui::Window::new("Import picture").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Import picture", egui::pos2(600.0, 150.0))).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(dw);
        if crate::csd::dialog_header(ui, tr("Import picture"), true) { cancel = true; }
        let (bw, bh) = (app.pic.size.0.max(1) as f32, app.pic.size.1.max(1) as f32);
        let size = Vec2::new(dw, (dw * bh / bw).min(230.0));
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
        let k = (size.x / bw).min(size.y / bh);
        let img = Rect::from_center_size(rect.center(), Vec2::new(bw * k, bh * k));
        if let Some(t) = &app.pic.tex { painter.image(t.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
        ui.add_space(8.0);
        ui.label(tr("What is this picture for?"));
        ui.add_space(4.0);
        for (p, name, text, tip) in [
            (Pick::Trace, tr("Trace outlines"), tr("Turns the dark parts into shapes you can cut."), tr("Opens the trace settings; the picture itself is not kept")),
            (Pick::Backdrop, tr("Use as backdrop"), tr("Shows it behind the drawing, to draw over it. It is never cut."), tr("The picture is kept in the project; set its size and see-through level in the left panel")),
            (Pick::Both, tr("Both"), tr("The backdrop first, then the traced shapes land exactly on it."), tr("For copying a drawing or a photo of a piece you will recreate")),
        ] {
            let w = dw - 4.0;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 50.0), Sense::click());
            let resp = resp.tip(tip);
            let v = ui.visuals();
            let fill = if resp.hovered() { v.widgets.hovered.bg_fill } else { v.widgets.inactive.bg_fill };
            ui.painter().rect(rect, 6.0, fill, if resp.hovered() { v.widgets.hovered.bg_stroke } else { v.widgets.inactive.bg_stroke });
            ui.painter().text(Pos2::new(rect.left() + 12.0, rect.top() + 9.0), egui::Align2::LEFT_TOP, name, egui::TextStyle::Button.resolve(ui.style()), v.strong_text_color());
            let galley = ui.painter().layout(text.to_string(), egui::TextStyle::Small.resolve(ui.style()), v.weak_text_color(), w - 24.0);
            ui.painter().galley(Pos2::new(rect.left() + 12.0, rect.top() + 28.0), galley, v.weak_text_color());
            if resp.clicked() { pick = Some(p); }
            ui.add_space(2.0);
        }
        ui.add_space(4.0);
        if ui.button(tr("Cancel")).clicked() { cancel = true; }
    }));
    if let Some(p) = pick {
        let path = app.pic.path.clone();
        app.pic.close();
        match p {
            Pick::Trace => app.import_trace(&path, None),
            Pick::Backdrop => { app.place_backdrop(&path); }
            Pick::Both => { let at = app.place_backdrop(&path); if at.is_some() { app.import_trace(&path, at); } }
        }
    } else if cancel { app.pic.close(); }
}
