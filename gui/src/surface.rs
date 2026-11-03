//! Surface dialog: flatten the top of the stock, or the spoilboard, with a wide cutter.
use crate::app::App;
use crate::widgets::Tip;
use eframe::egui;
use putiacnc_core::surface::{self, SurfaceSpec};

pub struct SurfaceTool {
    pub open: bool,
    /// False = the stock's top, true = the spoilboard.
    pub spoil: bool,
    pub spec: Option<SurfaceSpec>,
    /// Make the stock as big as the machine's work area (spoilboard).
    pub fill_area: bool,
}

impl Default for SurfaceTool {
    fn default() -> Self { SurfaceTool { open: false, spoil: false, spec: None, fill_area: true } }
}

impl App {
    /// Adds the surfacing operation, as the dialog set it. With `fill_area` the stock becomes the size of the machine's work area first.
    pub fn add_surface(&mut self, spec: &SurfaceSpec, fill_area: bool) -> Option<String> {
        let mut d = self.project.data.clone();
        if fill_area {
            if let Some(m) = d.machine.clone() { d.stock.w = m.x; d.stock.h = m.y; }
        }
        match surface::add_surfacing(&mut d, spec) {
            Ok(_) => {
                self.hist.record(&self.project.data);
                self.project.data = d;
                self.touch();
                Some(if spec.first { "Surfacing added: it runs first.".into() } else { "Surfacing added: it runs last.".into() })
            }
            Err(e) => Some(e),
        }
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.surface.open { if app.tool_note.as_ref().is_some_and(|(k, _)| *k == "surface") { app.tool_note = None; } return; }
    if app.surface.spec.is_none() {
        let tool = surface::suggest_tool(&app.project.data.tools).cloned();
        app.surface.spec = tool.map(|t| surface::defaults(&t, false));
    }
    let tools = app.project.data.tools.clone();
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Surface").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Surface", crate::csd::anchor(app))).show(ctx, |ui| {
        ui.set_width(376.0);
        if crate::csd::dialog_header_help(ui, "Surface", true, |ui| {
            ui.label("Takes a thin layer off the whole stock with a wide flat cutter, so the top is flat and level with the work zero. Use it on a new stock, or on the spoilboard to level the bed.");
            ui.add_space(4.0);
            ui.label("It adds an ordinary Pocket operation over a rectangle as big as the stock, so you can still change it in the Operations panel.");
        }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        let (c, _) = crate::zoomcard::seg2(ui, ["Stock top", "Spoilboard"], ["Flatten the top of the material", "Level the bed the material sits on"], app.surface.spoil as usize);
        if let Some(i) = c { app.surface.spoil = i == 1; if let Some(s) = app.surface.spec.as_mut() { s.depth = if i == 1 { 1.0 } else { 0.5 }; s.pass = s.pass.min(s.depth); } }
        ui.add_space(6.0);
        let lw = ["Cutter", "Depth", "Pass depth", "Stepover"].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0_f32, f32::max);
        let spoil = app.surface.spoil;
        let area = app.project.data.machine.as_ref().map(|m| (m.x, m.y));
        let note = if spoil { format!("The stock is {} × {}: change it in Stock if the bed is bigger.", crate::widgets::fmt_len(ui, app.project.data.stock.w), crate::widgets::fmt_len(ui, app.project.data.stock.h)) } else { String::new() };
        match app.surface.spec.as_mut() {
            None => { ui.label("Add a flat cutter in the tool library first."); }
            Some(s) => {
                egui::Grid::new("surface_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    crate::toolmgr::rlabel(ui, lw, "Cutter");
                    let cur = tools.iter().find(|t| t.id == s.tool).map(|t| t.name.replace('″', "\"")).unwrap_or_else(|| "none".into());
                    let cur = crate::widgets::combo_elide(ui, &cur, 230.0);
                    let (_, pk) = crate::toolpick::picker(ui, "surface_tool", 230.0, cur, Some(s.tool.as_str()), &[], &tools, |t| t.shape == "flat");
                    match pk {
                        Some(crate::toolpick::Picked::Tool(id)) => { if let Some(t) = tools.iter().find(|t| t.id == id) { let keep = (s.depth, s.first, s.air); *s = surface::defaults(t, spoil); s.depth = keep.0; s.pass = s.pass.min(s.depth); s.first = keep.1; s.air = keep.2; } }
                        Some(crate::toolpick::Picked::Manage) => app.tools_open = true,
                        None => {}
                    }
                    ui.end_row();
                    crate::toolmgr::rlabel(ui, lw, "Depth");
                    ui.horizontal(|ui| { crate::widgets::len(ui, [70.0, 24.0], &mut s.depth, 0.05, 20.0, 0.05, 2, "").tip("How much material to take off in total"); ui.weak(crate::widgets::len_unit(ui)); });
                    ui.end_row();
                    crate::toolmgr::rlabel(ui, lw, "Pass depth");
                    ui.horizontal(|ui| { crate::widgets::len(ui, [70.0, 24.0], &mut s.pass, 0.05, 20.0, 0.05, 2, "").tip("Depth of each pass: a surfacing bit is happiest with light cuts"); ui.weak(crate::widgets::len_unit(ui)); });
                    ui.end_row();
                    crate::toolmgr::rlabel(ui, lw, "Stepover");
                    ui.horizontal(|ui| { crate::widgets::num(ui, [70.0, 24.0], &mut s.stepover, 5.0, 100.0, 1.0, 0, "").tip("Distance between passes, as a percentage of the cutter's diameter"); ui.weak("%"); });
                    ui.end_row();
                });
                ui.add_space(4.0);
                crate::widgets::check(ui, &mut s.first, "Run before the other operations").tip("The later operations then start from a flat top");
                crate::widgets::check(ui, &mut s.air, "Air cut").tip("Run it above the material, to check the path");
                if spoil && area.is_some() { crate::widgets::check(ui, &mut app.surface.fill_area, "Make the stock the size of the work area").tip("The whole bed is surfaced"); }
            }
        }
        if !note.is_empty() && !app.surface.fill_area { ui.weak(note); }
        if let Some((_, m)) = app.tool_note.as_ref().filter(|(k, _)| *k == "surface") { ui.weak(m); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(app.surface.spec.is_some(), egui::Button::new("OK")).on_hover_text("Do it and close the window").clicked() { apply = true; }
            if ui.button("Cancel").clicked() { close = true; }
        });
        });
    });
    if apply {
        if let Some(spec) = app.surface.spec.clone() {
            let fill = app.surface.spoil && app.surface.fill_area;
            if app.run_tool("surface", |a| a.add_surface(&spec, fill)) { app.surface.open = false; app.surface.spec = None; }
        }
    }
    if close { app.surface.open = false; app.surface.spec = None; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surfacing_is_added_and_undone() {
        let mut a = App::new(None);
        a.project.data.tools = putiacnc_core::setup::starter_tools(putiacnc_core::setup::ToolChoice { surfacing: true, ..Default::default() });
        let t = surface::suggest_tool(&a.project.data.tools).unwrap().clone();
        let spec = surface::defaults(&t, false);
        let (n, o) = (a.project.data.loops.len(), a.project.data.ops.len());
        assert!(a.run_tool("surface", |a| a.add_surface(&spec, false)));
        assert_eq!((a.project.data.loops.len(), a.project.data.ops.len()), (n + 1, o + 1));
        a.do_undo();
        assert_eq!((a.project.data.loops.len(), a.project.data.ops.len()), (n, o));
    }

    #[test]
    fn a_missing_cutter_changes_nothing() {
        let mut a = App::new(None);
        let spec = SurfaceSpec { tool: "nope".into(), depth: 0.5, pass: 0.5, stepover: 60.0, first: true, air: false };
        let n = a.project.data.loops.len();
        assert!(!a.run_tool("surface", |a| a.add_surface(&spec, false)));
        assert_eq!(a.project.data.loops.len(), n);
        assert!(a.tool_note.is_some());
    }
}
