//! Import 3D model: STL and OBJ files do not say which way is up, so the dialog shows the model as a height map
//! seen from above and lets the user pick the axis that points up (and turn the model upside down).
use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use putiacnc_core::relief::{Mesh, Orient, Up};

#[derive(Default)]
pub struct ModelTool {
    pub open: bool,
    pub name: String,
    obj: bool,
    mesh: Option<Mesh>,
    pub orient: Orient,
    tex: Option<egui::TextureHandle>,
    err: Option<String>,
    dirty: bool,
    /// Pending message for the main window (a file that could not be read).
    pub error: Option<String>,
    /// The choice to remember for next time, per file type (see `Settings`).
    pub remember_obj: Option<Orient>,
    pub remember_stl: Option<Orient>,
}

impl ModelTool {
    pub fn load(&mut self, path: &str, saved: Option<Orient>) {
        let bytes = match std::fs::read(path) { Ok(b) => b, Err(e) => { self.error = Some(format!("Could not read {path}: {e}")); return; } };
        let mesh = match Mesh::parse(&bytes) { Ok(m) => m, Err(e) => { self.error = Some(format!("Could not import {path}: {e}")); return; } };
        self.obj = path.to_lowercase().ends_with(".obj");
        self.name = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "model".into());
        // an OBJ is most often Y up (the usual export), an STL Z up (CAD and printing)
        self.orient = saved.unwrap_or(Orient { up: if self.obj { Up::Y } else { Up::Z }, flip: false });
        self.mesh = Some(mesh);
        self.tex = None;
        self.err = None;
        self.dirty = true;
        self.open = true;
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if let Some(e) = app.model3d.error.take() { app.error = Some(e); }
    if !app.model3d.open { return; }
    let (mut place, mut cancel) = (false, false);
    {
        let t = &mut app.model3d;
        if t.dirty {
            t.dirty = false;
            if let Some(m) = &t.mesh {
                match m.relief(&t.name, 40_000, t.orient) {
                    Ok(r) => {
                        let h = r.heights();
                        let mut px = vec![Color32::BLACK; r.nx * r.ny];
                        for y in 0..r.ny { for x in 0..r.nx {
                            let v = (55.0 + 185.0 * h[y * r.nx + x]) as u8;
                            px[(r.ny - 1 - y) * r.nx + x] = Color32::from_gray(v);
                        } }
                        t.tex = Some(ctx.load_texture("model_preview", egui::ColorImage { size: [r.nx, r.ny], pixels: px }, egui::TextureOptions::LINEAR));
                        t.err = None;
                    }
                    Err(e) => { t.tex = None; t.err = Some(e); }
                }
            }
        }
    }
    egui::Window::new("Import 3D model").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Import 3D model", egui::pos2(560.0, 172.0))).show(ctx, |ui| {
        ui.set_width(380.0);
        if crate::csd::dialog_header_help(ui, "Import 3D model", true, |ui| { ui.label("The picture of the model is a height map: light is high, dark is low."); }) { cancel = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        let t = &mut app.model3d;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(360.0, 230.0), Sense::hover());
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
        let lw = crate::panels::tw(ui, "Up axis");
        egui::Grid::new("model_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            crate::toolmgr::rlabel(ui, lw, "Up axis");
            ui.horizontal(|ui| {
                for (u, n) in [(Up::Z, "Z"), (Up::Y, "Y"), (Up::X, "X")] {
                    if ui.selectable_label(t.orient.up == u, n).on_hover_text("The axis of the model that points towards the cutter").clicked() { t.orient.up = u; changed = true; }
                }
                ui.add_space(10.0);
                changed |= crate::widgets::check(ui, &mut t.orient.flip, "Upside down").on_hover_text("Look at the model from the other side").changed();
            });
            ui.end_row();
        });
        if changed {
            t.dirty = true;
            if t.obj { t.remember_obj = Some(t.orient); } else { t.remember_stl = Some(t.orient); }
        }
        ui.add_space(4.0);
        match (&t.err, &t.mesh) {
            (Some(e), _) => { ui.colored_label(Color32::from_rgb(0xe0, 0x30, 0x30), e.as_str()); }
            (None, Some(m)) => { let s = m.size(t.orient); ui.weak(format!("{:.1} × {:.1} mm, {:.1} mm high.", s[0], s[1], s[2])); }
            _ => {}
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(t.err.is_none(), egui::Button::new("Import")).clicked() { place = true; }
            if ui.button("Cancel").clicked() { cancel = true; }
        });
        });
    });
    if place {
        let t = &mut app.model3d;
        let rel = t.mesh.as_ref().map(|m| m.relief(&t.name, 600_000, t.orient));
        t.open = false;
        t.mesh = None;
        t.tex = None;
        match rel {
            Some(Ok(r)) => app.place_relief(r),
            Some(Err(e)) => app.error = Some(format!("Could not import the model: {e}")),
            None => {}
        }
    }
    if cancel { app.model3d.open = false; app.model3d.mesh = None; app.model3d.tex = None; }
}
