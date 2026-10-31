//! Fillet / chamfer dialog: round off or bevel the corners of the selected shapes.
use crate::widgets::Tip;
use crate::app::App;
use eframe::egui;
use putiacnc_core::shapes::{self, CornerKind};

pub struct CornerTool { pub open: bool, pub chamfer: bool, pub size: f64, pub dog: bool, pub dia: f64, pub outside: bool, pub auto: bool }

impl Default for CornerTool { fn default() -> Self { CornerTool { open: false, chamfer: false, size: 2.0, dog: false, dia: 3.175, outside: false, auto: true } } }

impl App {
    /// True when something in the selection has a corner to round (circles and ellipses do not).
    pub fn selection_has_corners(&self) -> bool {
        self.sel.iter().any(|&i| self.project.data.loops.get(i).is_some_and(shapes::has_corners))
    }

    /// Rounds or bevels every sharp corner of the selected shapes. Returns a message when nothing was done
    /// or some corners had to be made smaller than asked.
    pub fn corner_selection(&mut self, kind: CornerKind, size: f64) -> Option<String> {
        let mut changes: Vec<(usize, putiacnc_core::model::LoopData)> = Vec::new();
        let (mut limited, mut corners) = (0, 0);
        for &i in &self.sel {
            let Some(l) = self.project.data.loops.get(i) else { continue };
            // a plain rectangle keeps being a rectangle: only its corner radius changes
            if kind == CornerKind::Fillet && l.rc.is_some_and(|r| r < 1e-9) {
                let mut r = l.clone();
                shapes::set_corner_radius(&mut r, size);
                if r.rc.is_some_and(|v| v < size - 1e-9) { limited += 4; }
                corners += 4;
                changes.push((i, r));
            } else if let Some((r, n, lim)) = shapes::round_corners(l, kind, size) {
                corners += n; limited += lim;
                changes.push((i, r));
            }
        }
        if changes.is_empty() { return Some("There are no sharp corners in the selection.".into()); }
        self.hist.record(&self.project.data);
        // rounded letters are no longer what the font draws: a changed text becomes plain outlines
        let d = &mut self.project.data;
        let mut texts: Vec<i64> = changes.iter().filter_map(|(i, _)| d.loops[*i].grp).filter(|g| d.groups.iter().any(|x| x.id == *g && x.text.is_some())).collect();
        texts.sort_unstable();
        texts.dedup();
        for (i, r) in changes { d.loops[i] = r; }
        for g in &texts {
            for l in d.loops.iter_mut().filter(|l| l.grp == Some(*g)) { l.grp = None; }
            d.groups.retain(|x| x.id != *g);
        }
        self.touch();
        let _ = corners;
        let mut note = String::new();
        if limited > 0 { note = format!("{limited} corner{} too tight for that size and got the largest that fits.", if limited == 1 { " was" } else { "s were" }); }
        if !texts.is_empty() { if !note.is_empty() { note.push(' '); } note.push_str("The text is now plain outlines, so it can no longer be edited as text."); }
        if note.is_empty() { None } else { Some(note) }
    }
}

/// The diameter reliefs are cut for: the tool's when `auto` and there is one, else the typed one.
pub fn cutter_dia(auto: bool, op_dia: Option<f64>, typed: f64) -> f64 { if auto { op_dia.unwrap_or(typed) } else { typed } }

/// The "Cutter" row of a dialog that makes dogbones: a check to follow the operation's tool, else a typed diameter or one picked from a tool.
pub fn cutter_row(ui: &mut egui::Ui, lw: f32, dia: &mut f64, auto: &mut bool, op_dia: Option<f64>, tools: &[(String, f64)]) {
    crate::toolmgr::rlabel(ui, lw, "Cutter");
    ui.vertical(|ui| {
        crate::widgets::check(ui, auto, "Tool of the operation").tip("Take the diameter from the tool of the operation these shapes are cut in, so the reliefs always match the cutter");
        if *auto {
            match op_dia { Some(d) => { ui.weak(format!("{d} mm")); } None => { ui.weak("Not in an operation yet: using the size below."); } }
        }
        if !*auto || op_dia.is_none() {
            ui.horizontal(|ui| {
                crate::widgets::num(ui, [70.0, 24.0], dia, 0.1, 50.0, 0.25, 3, "");
                ui.weak("mm");
                egui::ComboBox::from_id_source("dog_tool").selected_text("from tool").width(110.0).show_ui(ui, |ui| {
                    for (n, d) in tools { if ui.selectable_label(false, format!("{n} ({d} mm)")).clicked() { *dia = *d; } }
                });
            });
        }
    });
    ui.end_row();
}

impl App {
    fn tool_dia(&self, id: &str) -> Option<f64> {
        self.project.data.tools.iter().find(|t| t.id == id && t.shape != "pen" && t.dia > 0.0).map(|t| t.dia)
    }

    /// The cutter diameter of the operation the selected shapes are in (the first shape that is in one).
    pub fn sel_cutter_dia(&self) -> Option<f64> {
        let d = &self.project.data;
        self.sel.iter().filter_map(|&i| d.loops.get(i)).filter(|l| l.op != 0)
            .find_map(|l| d.ops.iter().find(|o| o.id == l.op).and_then(|o| self.tool_dia(&o.p.tool)))
    }

    /// The cutter diameter a new operation would start with: the last operation's tool, else the first cutter.
    pub fn inherited_cutter_dia(&self) -> Option<f64> {
        let d = &self.project.data;
        d.ops.last().and_then(|o| self.tool_dia(&o.p.tool)).or_else(|| d.tools.iter().find(|t| t.shape != "pen" && t.dia > 0.0).map(|t| t.dia))
    }

    /// The tools of the project as (name, diameter) for a picker.
    pub fn cutter_choices(&self) -> Vec<(String, f64)> {
        self.project.data.tools.iter().filter(|t| t.dia > 0.0 && t.shape != "pen").map(|t| (t.name.clone(), t.dia)).collect()
    }

    /// Adds a dogbone relief to each inside corner of the selected closed shapes, for a cutter of diameter `dia`.
    /// `outside` is for shapes that will be cut around (parts with a notch), otherwise for pockets and cut-outs.
    pub fn dogbone_selection(&mut self, dia: f64, outside: bool) -> Option<String> {
        let idx: Vec<usize> = self.sel.iter().copied().filter(|&i| self.project.data.loops.get(i).is_some_and(|l| l.closed())).collect();
        if idx.is_empty() { return Some("Select one or more closed shapes.".into()); }
        let op = self.project.data.loops[idx[0]].op;
        let cut = if outside { putiacnc_core::joints::Cut::Outside } else { putiacnc_core::joints::Cut::Inside };
        let refs: Vec<&putiacnc_core::model::LoopData> = idx.iter().map(|&i| &self.project.data.loops[i]).collect();
        let Some((result, n)) = putiacnc_core::joints::dogbones(&refs, dia, cut, op) else {
            return Some("There are no sharp inside corners that need a relief for this cutter.".into());
        };
        self.hist.record(&self.project.data);
        let d = &mut self.project.data;
        let mut texts: Vec<i64> = idx.iter().filter_map(|&i| d.loops[i].grp).filter(|g| d.groups.iter().any(|x| x.id == *g && x.text.is_some())).collect();
        texts.sort_unstable();
        texts.dedup();
        for &i in idx.iter().rev() { d.loops.remove(i); }
        shapes::tabs_after_delete(&mut d.ops, &idx);
        for g in &texts {
            for l in d.loops.iter_mut().filter(|l| l.grp == Some(*g)) { l.grp = None; }
            d.groups.retain(|x| x.id != *g);
        }
        let start = d.loops.len();
        d.loops.extend(result);
        self.sel = (start..self.project.data.loops.len()).collect();
        self.touch();
        let mut note = format!("{n} dogbone{} added.", if n == 1 { "" } else { "s" });
        if !texts.is_empty() { note.push_str(" The text is now plain outlines."); }
        Some(note)
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.corners.open { app.ghost.clear_if("corners"); if app.tool_note.as_ref().is_some_and(|(k, _)| *k == "corners") { app.tool_note = None; } return; }
    {
        let kind = if app.corners.chamfer { CornerKind::Chamfer } else { CornerKind::Fillet };
        let size = app.corners.size;
        let (dog, outside) = (app.corners.dog, app.corners.outside);
        let dia = cutter_dia(app.corners.auto, app.sel_cutter_dia(), app.corners.dia);
        app.preview("corners", &[app.corners.chamfer as u64, crate::preview::bits(size), dog as u64, crate::preview::bits(dia), outside as u64],
            move |a| if dog { a.dogbone_selection(dia, outside) } else { a.corner_selection(kind, size) });
    }
    let pnote = app.ghost.note_of("corners").filter(|_| !app.sel.is_empty()).map(String::from);
    let n = app.sel.len();
    let has = app.selection_has_corners() || (app.corners.dog && app.sel.iter().any(|&i| app.project.data.loops.get(i).is_some_and(|l| l.closed())));
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Fillet and chamfer").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Fillet and chamfer", crate::csd::anchor(app))).show(ctx, |ui| {
        ui.set_width(376.0);
        if crate::csd::dialog_header_help(ui, "Fillet and chamfer", true, |ui| {
            ui.label("Smooth curves, circles and ellipses are left alone.");
            ui.add_space(4.0);
            ui.label("A chamfer is measured as a distance along each edge from the corner.");
            ui.add_space(4.0);
            ui.label("A dogbone is a small round notch in each inside corner, as wide as the cutter, so a square corner can fit.");
        }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if n == 0 { "Select one or more shapes first.".to_string() } else { format!("{n} shape{} selected.", if n == 1 { "" } else { "s" }) });
        ui.add_space(6.0);
        let lw = ["Corner", "Distance", "Radius", "Cutter"].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0_f32, f32::max);
        egui::Grid::new("corner_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            crate::toolmgr::rlabel(ui, lw, "Corner");
            ui.horizontal(|ui| {
                let cur = if app.corners.dog { 2 } else { app.corners.chamfer as usize };
                let (c, _) = crate::zoomcard::seg3(ui, ["Fillet", "Chamfer", "Dogbone"], ["Round the corners", "Cut the corners off with a straight bevel", "Cut a small round relief into each inside corner so a square part fits a pocket the round cutter made"], cur);
                if let Some(i) = c { app.corners.dog = i == 2; if i < 2 { app.corners.chamfer = i == 1; } }
            });
            ui.end_row();
            if app.corners.dog {
                let (od, tools) = (app.sel_cutter_dia(), app.cutter_choices());
                let c = &mut app.corners;
                cutter_row(ui, lw, &mut c.dia, &mut c.auto, od, &tools);
                crate::toolmgr::rlabel(ui, lw, "Cut");
                ui.horizontal(|ui| {
                    let (c, _) = crate::zoomcard::seg2(ui, ["Inside", "Around"], ["For pockets and cut-outs: the shape is the hole", "For parts cut out around the outside, with a notch in them"], app.corners.outside as usize);
                    if let Some(i) = c { app.corners.outside = i == 1; }
                });
                ui.end_row();
            } else {
                crate::toolmgr::rlabel(ui, lw, if app.corners.chamfer { "Distance" } else { "Radius" });
                ui.horizontal(|ui| {
                    crate::widgets::num(ui, [70.0, 24.0], &mut app.corners.size, 0.01, 1000.0, 0.25, 2, "");
                    ui.weak("mm");
                });
                ui.end_row();
            }
        });
        let mine = app.tool_note.as_ref().filter(|(k, _)| *k == "corners").map(|(_, m)| m);
        if let Some(n) = pnote.as_ref().or(mine) { ui.weak(n); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(has, egui::Button::new("Apply")).clicked() { apply = true; }
            if ui.button("Cancel").clicked() { close = true; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::widgets::check(ui, &mut app.preview_on, "Preview").tip("Show the result in the drawing while you change the values");
            });
        });
        });
    });
    if apply {
        let kind = if app.corners.chamfer { CornerKind::Chamfer } else { CornerKind::Fillet };
        let size = app.corners.size;
        let (dog, outside) = (app.corners.dog, app.corners.outside);
        let dia = cutter_dia(app.corners.auto, app.sel_cutter_dia(), app.corners.dia);
        app.ghost.clear();
        if app.run_tool("corners", |a| if dog { a.dogbone_selection(dia, outside) } else { a.corner_selection(kind, size) }) { app.corners.open = false; }
    }
    if close { app.corners.open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with(loops: Vec<putiacnc_core::model::LoopData>) -> App {
        let mut a = App::new(None);
        a.project.data.loops = loops;
        a.sel = (0..a.project.data.loops.len()).collect();
        a
    }

    #[test]
    fn button_condition_and_apply() {
        let mut a = app_with(vec![shapes::new_circle([5.0, 5.0], 3.0, 1)]);
        assert!(!a.selection_has_corners());
        assert!(a.corner_selection(CornerKind::Fillet, 1.0).is_some());
        let mut a = app_with(vec![shapes::new_circle([5.0, 5.0], 3.0, 1), shapes::new_rect([0.0, 0.0], [20.0, 10.0], 0.0, 1)]);
        assert!(a.selection_has_corners());
        assert!(a.corner_selection(CornerKind::Fillet, 2.0).is_none());
        // the rectangle stays a rectangle with a corner radius; the circle is untouched
        assert_eq!(a.project.data.loops[1].rc, Some(2.0));
        assert!(shapes::is_circle(&a.project.data.loops[0]));
        // undo restores the sharp corners
        a.do_undo();
        assert_eq!(a.project.data.loops[1].rc, Some(0.0));
        // a chamfer turns the rectangle into a polygon
        assert!(a.corner_selection(CornerKind::Chamfer, 2.0).is_none());
        assert!(a.project.data.loops[1].rc.is_none());
        assert_eq!(a.project.data.loops[1].pts.len() / 2, 8);
    }

    #[test]
    fn dogbones_replace_the_selected_shapes_and_undo_restores_them() {
        let mut a = app_with(vec![shapes::new_rect([0.0, 0.0], [30.0, 20.0], 0.0, 1)]);
        let msg = a.dogbone_selection(3.0, false).unwrap();
        assert!(msg.starts_with("4 dogbones"), "{msg}");
        assert_eq!(a.project.data.loops.len(), 1);
        assert!(a.project.data.loops[0].pts.len() / 2 > 8);
        assert_eq!(a.project.data.loops[0].op, 1);
        a.do_undo();
        assert_eq!(a.project.data.loops[0].rc, Some(0.0));
        let mut a = app_with(vec![shapes::new_circle([5.0, 5.0], 3.0, 1)]);
        assert!(a.dogbone_selection(3.0, false).unwrap().contains("no sharp"));
    }

    #[test]
    fn the_dogbone_cutter_follows_the_tool_of_the_operation() {
        let mut a = app_with(vec![shapes::new_rect([0.0, 0.0], [30.0, 20.0], 0.0, 0)]);
        // not in an operation: the typed size
        assert_eq!(a.sel_cutter_dia(), None);
        assert_eq!(cutter_dia(true, a.sel_cutter_dia(), 3.175), 3.175);
        a.project.data.tools[0].dia = 6.0;
        a.project.data.tools[0].shape = "flat".into();
        let sel = a.sel.clone();
        shapes::add_operation(&mut a.project.data, &sel, "pocket");
        let id = a.project.data.tools[0].id.clone();
        a.project.data.ops[0].p.tool = id;
        assert_eq!(a.sel_cutter_dia(), Some(6.0));
        assert_eq!(cutter_dia(true, a.sel_cutter_dia(), 3.175), 6.0);
        assert_eq!(cutter_dia(false, a.sel_cutter_dia(), 3.175), 3.175);
        // change the operation's tool and the reliefs follow
        a.project.data.tools[0].dia = 4.0;
        assert_eq!(a.sel_cutter_dia(), Some(4.0));
        assert_eq!(a.inherited_cutter_dia(), Some(4.0));
    }

    #[test]
    fn rounding_the_corners_of_text_turns_it_into_outlines() {
        use putiacnc_core::model::{ShapeGroup, TextSpec};
        use putiacnc_core::text::{Align, Flow};
        let mut r = shapes::new_rect([0.0, 0.0], [20.0, 10.0], 0.0, 1);
        r.grp = Some(1);
        let mut a = app_with(vec![r]);
        a.project.data.groups.push(ShapeGroup { id: 1, name: "Text: A".into(), locked: false, text: Some(TextSpec {
            content: "A".into(), font: "Hack".into(), size: 10.0, spacing: 0.0, word_spacing: 0.0, kerning: false, align: Align::Left, flow: Flow::Straight, radius: 0.0, arc_deg: 90.0, arc_in: false, fit_width: 0.0, shadow: [0.0, 0.0] }) });
        let msg = a.corner_selection(CornerKind::Fillet, 1.0).expect("a note about the text");
        assert!(msg.contains("plain outlines"), "{msg}");
        assert!(a.project.data.groups.is_empty() && a.project.data.loops[0].grp.is_none());
    }
}
