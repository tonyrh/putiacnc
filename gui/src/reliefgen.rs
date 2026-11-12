//! Relief from shapes: closed shapes get a height and a profile (flat top, rounded edge, dome, slope) and become a height map, carved by
//! the 3D relief operation like an imported model. Once made it is a relief of its own: change it by making it again.
use putiacnc_core::i18n::{tr, tr_f, tr_n};
use crate::app::{App, Level};
use crate::widgets::Tip;
use eframe::egui;
use putiacnc_core::reliefshapes::{self, Params, Profile};

#[derive(Default)]
pub struct ReliefTool {
    pub open: bool,
    pub params: Params,
}

impl App {
    /// The closed shapes of the selection.
    pub fn closed_selection(&self) -> usize {
        let d = &self.project.data;
        self.sel.iter().filter(|&&i| d.loops.get(i).is_some_and(|l| l.closed() && l.pts.len() >= 6)).count()
    }

    /// Builds the relief from the selected closed shapes and puts it on the stock where the shapes are (replacing a relief the project has).
    pub fn make_relief(&mut self, p: &Params) -> Result<String, String> {
        let d = &self.project.data;
        let shapes: Vec<&putiacnc_core::model::LoopData> = self.sel.iter().filter_map(|&i| d.loops.get(i)).collect();
        let name = tr("Shapes");
        let rel = reliefshapes::from_shapes(&shapes, p, name)?;
        let (w, h) = rel.size();
        let mut note = putiacnc_core::i18n::tr_nf(self.closed_selection() as u64, "Relief made from {n} shape: {w} × {h} mm, {z} mm high.", "Relief made from {n} shapes: {w} × {h} mm, {z} mm high.", &[("w", &format!("{:.1}", w)), ("h", &format!("{:.1}", h)), ("z", &format!("{:.1}", rel.zmax))]);
        if rel.zmax > d.stock.t + 1e-6 { note.push(' '); note += &tr_f("It is taller than the stock ({t} mm): the carving depth is held to the stock.", &[("t", &format!("{:.1}", d.stock.t))]); }
        if d.relief.is_some() { note += tr(" It replaced the relief the project had."); }
        self.hist.record(&self.project.data);
        self.project.data.relief = Some(rel);
        self.touch();
        Ok(note)
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.reliefgen.open { if app.tool_note.as_ref().is_some_and(|(k, _)| *k == "relief") { app.tool_note = None; } return; }
    let n = app.closed_selection();
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Relief from shapes").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Relief from shapes", crate::csd::anchor(app))).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(crate::csd::dialog_w(376.0));
        if crate::csd::dialog_header_help(ui, tr("Relief from shapes"), true, |ui| {
            ui.label(tr("Turns the selected closed shapes into a 3D relief: each one is raised to the height you set, with the profile you choose. A shape inside another one is a hole in it, as in a pocket. Where shapes overlap, the higher one wins."));
            ui.add_space(4.0);
            ui.label(tr("The relief is carved by a 3D relief operation, like an imported model. Make it again to change it."));
        }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if n == 0 { tr("Select one or more closed shapes first.").to_string() } else { tr_n(n as u64, "{n} closed shape selected.", "{n} closed shapes selected.") });
        ui.add_space(6.0);
        let lw = [tr("Profile"), tr("Height"), tr("Edge width"), tr("Background")].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0_f32, f32::max);
        let t = &mut app.reliefgen;
        egui::Grid::new("relief_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            crate::toolmgr::rlabel(ui, lw, tr("Profile"));
            ui.horizontal(|ui| {
                let tips = [tr("A flat top at the full height, with straight sides"), tr("A flat top with a rounded rim"), tr("Round over the whole shape, highest along the middle"), tr("A straight slope up to a ridge along the middle")];
                for ((pr, name), tip) in reliefshapes::PROFILES.iter().zip(tips) {
                    if ui.selectable_label(t.params.profile == *pr, *name).tip(tip).clicked() { t.params.profile = *pr; }
                }
            });
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, tr("Height"));
            ui.horizontal(|ui| { crate::widgets::len(ui, [70.0, 24.0], &mut t.params.height, 0.1, 200.0, 0.1, 2, "").tip(tr("How high the highest point stands above the lowest level")); ui.weak(crate::widgets::len_unit(ui)); });
            ui.end_row();
            if t.params.profile == Profile::Rounded {
                crate::toolmgr::rlabel(ui, lw, tr("Edge width"));
                ui.horizontal(|ui| { crate::widgets::len(ui, [70.0, 24.0], &mut t.params.edge, 0.1, 200.0, 0.1, 2, "").tip(tr("How far in from the edge the rounding reaches")); ui.weak(crate::widgets::len_unit(ui)); });
                ui.end_row();
            }
            crate::toolmgr::rlabel(ui, lw, tr("Background"));
            ui.horizontal(|ui| { crate::widgets::len(ui, [70.0, 24.0], &mut t.params.margin, 0.0, 200.0, 0.5, 2, "").tip(tr("Lowest-level material kept around the shapes, to be carved away")); ui.weak(crate::widgets::len_unit(ui)); });
            ui.end_row();
        });
        let stock_t = app.project.data.stock.t;
        if app.reliefgen.params.height > stock_t + 1e-6 { crate::msg::line(ui, Level::Warning, &tr_f("Taller than the stock ({t}): the carving depth is held to the stock.", &[("t", &crate::widgets::fmt_len(ui, stock_t))])); }
        if app.project.data.relief.is_some() { ui.weak(tr("The project already has a relief: this replaces it.")); }
        if let Some((_, m)) = app.tool_note.as_ref().filter(|(k, _)| *k == "relief") { crate::msg::line(ui, Level::Warning, m); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(n > 0, egui::Button::new(tr("OK"))).on_hover_text(tr("Make the relief and close the window")).clicked() { apply = true; }
            if ui.button(tr("Cancel")).clicked() { close = true; }
        });
        });
    }));
    if apply {
        let p = app.reliefgen.params;
        match app.make_relief(&p) {
            Ok(note) => { app.tool_note = None; app.say(Level::Info, note); app.reliefgen.open = false; }
            Err(e) => app.tool_note = Some(("relief", e)),
        }
    }
    if close { app.reliefgen.open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selected_shapes_become_a_relief_that_can_be_undone() {
        let mut a = App::new(None);
        a.project.data.loops.push(putiacnc_core::shapes::new_circle([50.0, 50.0], 20.0, 0));
        a.sel = [0].into_iter().collect();
        assert_eq!(a.closed_selection(), 1);
        let note = a.make_relief(&Params { height: 6.0, ..Params::default() }).unwrap();
        assert!(note.contains("1 shape") && note.contains("6.0 mm high"), "{note}");
        let r = a.project.data.relief.as_ref().unwrap();
        assert!((r.x - 30.0).abs() < 0.5 && (r.y - 30.0).abs() < 0.5 && (r.zmax - 6.0).abs() < 1e-9);
        // nothing selected: refused, project unchanged
        a.sel.clear();
        a.project.data.relief = None;
        assert!(a.make_relief(&Params::default()).is_err());
        assert!(a.project.data.relief.is_none());
    }
}
