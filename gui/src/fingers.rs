//! Finger joint dialog: cut tabs or slots into the edges of a selected rectangle, for plates that slot together.
use crate::app::App;
use crate::widgets::Tip;
use eframe::egui;
use putiacnc_core::joints::{self, Edge, Fingers};
use putiacnc_core::shapes;

pub struct FingerTool {
    pub open: bool,
    /// Bottom, right, top, left.
    pub edges: [Edge; 4],
    pub width: f64,
    pub clearance: f64,
    pub dogbone: bool,
    pub dia: f64,
    pub auto: bool,
}

impl Default for FingerTool {
    fn default() -> Self { FingerTool { open: false, edges: [Edge::Tabs, Edge::Flat, Edge::Flat, Edge::Flat], width: 10.0, clearance: 0.05, dogbone: false, dia: 3.175, auto: true } }
}

const NAMES: [&str; 4] = ["Bottom", "Right", "Top", "Left"];

impl App {
    /// True when the selection holds a rectangle the fingers can be cut into.
    pub fn selection_has_rect(&self) -> bool {
        self.sel.iter().any(|&i| self.project.data.loops.get(i).is_some_and(|l| l.rc.is_some() && l.closed()))
    }

    /// Cuts fingers into the edges of every selected rectangle. The thickness of the material is the stock's.
    pub fn finger_selection(&mut self, edges: [Edge; 4], width: f64, clearance: f64, dogbone: Option<f64>) -> Option<String> {
        let depth = self.project.data.stock.t;
        let idx: Vec<usize> = self.sel.iter().copied().filter(|&i| self.project.data.loops.get(i).is_some_and(|l| l.rc.is_some() && l.closed())).collect();
        if idx.is_empty() { return Some("Select a rectangle first.".into()); }
        if edges.iter().all(|e| *e == Edge::Flat) { return Some("Choose Tabs or Slots for at least one edge.".into()); }
        let f = Fingers { width, depth, clearance };
        let mut made: Vec<(usize, Vec<putiacnc_core::model::LoopData>)> = Vec::new();
        let mut dogs = 0;
        for &i in &idx {
            let l = &self.project.data.loops[i];
            let Some(bx) = shapes::bbox(l) else { continue };
            let Some(pts) = joints::finger_outline(bx, edges, f) else {
                return Some(format!("The fingers do not fit: the rectangle must be more than twice the material thickness ({depth} mm) in both directions."));
            };
            let Some(poly) = shapes::new_poly(&pts, true, l.op) else { continue };
            match dogbone.and_then(|d| joints::dogbones(&[&poly], d, joints::Cut::Outside, l.op)) {
                Some((ls, n)) => { dogs += n; made.push((i, ls)); }
                None => made.push((i, vec![poly])),
            }
        }
        if made.is_empty() { return None; }
        self.hist.record(&self.project.data);
        let d = &mut self.project.data;
        let removed: Vec<usize> = made.iter().map(|(i, _)| *i).collect();
        let new: Vec<_> = made.into_iter().flat_map(|(_, ls)| ls).collect();
        for &i in removed.iter().rev() { d.loops.remove(i); }
        shapes::tabs_after_delete(&mut d.ops, &removed);
        let start = d.loops.len();
        d.loops.extend(new);
        self.sel = (start..self.project.data.loops.len()).collect();
        self.touch();
        Some(if dogs > 0 { format!("Fingers cut, with {dogs} dogbone{}.", if dogs == 1 { "" } else { "s" }) } else { "Fingers cut.".into() })
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.fingers.open { app.ghost.clear_if("fingers"); if app.tool_note.as_ref().is_some_and(|(k, _)| *k == "fingers") { app.tool_note = None; } return; }
    let ft = &app.fingers;
    let dia = crate::corners::cutter_dia(ft.auto, app.sel_cutter_dia(), ft.dia);
    let (edges, width, clearance, dog) = (ft.edges, ft.width, ft.clearance, ft.dogbone.then_some(dia));
    let key: Vec<u64> = edges.iter().map(|e| *e as u64).chain([crate::preview::bits(width), crate::preview::bits(clearance), crate::preview::bits(dog.unwrap_or(0.0))]).collect();
    app.preview("fingers", &key, move |a| a.finger_selection(edges, width, clearance, dog));
    let pnote = app.ghost.note_of("fingers").filter(|_| !app.sel.is_empty()).map(String::from);
    let has = app.selection_has_rect();
    let thick = app.project.data.stock.t;
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Finger joint").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Finger joint", crate::csd::anchor(app))).show(ctx, |ui| {
        ui.set_width(376.0);
        if crate::csd::dialog_header_help(ui, "Finger joint", true, |ui| {
            ui.label("Cuts tabs or slots into the edges of the selected rectangle. Give one plate Tabs and the plate it meets Slots on edges of the same length, and they slot together.");
            ui.add_space(4.0);
            ui.label("The fingers are as deep as the stock is thick. The number of fingers is odd, so both ends of an edge match.");
            ui.add_space(4.0);
            ui.label("Dogbones make room in the slot corners for a round cutter.");
        }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if app.sel.is_empty() { "Select a rectangle first.".to_string() } else if has { format!("Depth {thick} mm: the thickness of the stock.") } else { "The selection has no rectangle.".to_string() });
        ui.add_space(6.0);
        let lw = ["Bottom", "Finger width", "Clearance", "Cutter"].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0_f32, f32::max);
        egui::Grid::new("finger_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            for (k, name) in NAMES.iter().enumerate() {
                crate::toolmgr::rlabel(ui, lw, *name);
                let (c, _) = crate::zoomcard::seg3(ui, ["Flat", "Tabs", "Slots"], ["Leave the edge straight", "Fingers stand out of the edge", "Notches go into the edge"], app.fingers.edges[k] as usize);
                if let Some(i) = c { app.fingers.edges[k] = [Edge::Flat, Edge::Tabs, Edge::Slots][i]; }
                ui.end_row();
            }
            crate::toolmgr::rlabel(ui, lw, "Finger width");
            ui.horizontal(|ui| { crate::widgets::num(ui, [70.0, 24.0], &mut app.fingers.width, 0.5, 500.0, 0.5, 2, ""); ui.weak("mm"); });
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, "Clearance");
            ui.horizontal(|ui| { crate::widgets::num(ui, [70.0, 24.0], &mut app.fingers.clearance, 0.0, 2.0, 0.01, 2, "").tip("How much wider a slot is than its tab, for the cutting kerf and a sliding fit"); ui.weak("mm"); });
            ui.end_row();
            if app.fingers.dogbone {
                let (od, tools) = (app.sel_cutter_dia(), app.cutter_choices());
                let c = &mut app.fingers;
                crate::corners::cutter_row(ui, lw, &mut c.dia, &mut c.auto, od, &tools);
            }
        });
        crate::widgets::check(ui, &mut app.fingers.dogbone, "Dogbones in the slots").tip("A small round relief in each slot corner so a round cutter leaves room for the square tab");
        let mine = app.tool_note.as_ref().filter(|(k, _)| *k == "fingers").map(|(_, m)| m);
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
        let ft = &app.fingers;
        let dia = crate::corners::cutter_dia(ft.auto, app.sel_cutter_dia(), ft.dia);
        let (edges, width, clearance, dog) = (ft.edges, ft.width, ft.clearance, ft.dogbone.then_some(dia));
        app.ghost.clear();
        if app.run_tool("fingers", |a| a.finger_selection(edges, width, clearance, dog)) { app.fingers.open = false; }
    }
    if close { app.fingers.open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingers_replace_a_rectangle_and_undo_restores_it() {
        let mut a = App::new(None);
        a.project.data.stock.t = 3.0;
        a.project.data.loops = vec![shapes::new_rect([0.0, 0.0], [60.0, 40.0], 0.0, 1)];
        a.sel = [0].into_iter().collect();
        assert!(a.selection_has_rect());
        let n0 = a.project.data.loops[0].pts.len();
        let msg = a.finger_selection([Edge::Tabs, Edge::Flat, Edge::Slots, Edge::Flat], 10.0, 0.05, None).unwrap();
        assert!(msg.contains("Fingers cut"), "{msg}");
        assert!(a.project.data.loops[0].pts.len() > n0);
        assert_eq!(a.project.data.loops[0].op, 1);
        a.do_undo();
        assert_eq!(a.project.data.loops[0].pts.len(), n0);
        // slots with dogbones add reliefs
        let msg = a.finger_selection([Edge::Slots, Edge::Flat, Edge::Flat, Edge::Flat], 10.0, 0.05, Some(3.0)).unwrap();
        assert!(msg.contains("dogbone"), "{msg}");
    }

    #[test]
    fn nothing_to_cut_or_no_room_says_so() {
        let mut a = App::new(None);
        a.project.data.stock.t = 3.0;
        a.project.data.loops = vec![shapes::new_rect([0.0, 0.0], [60.0, 5.0], 0.0, 0), shapes::new_circle([5.0, 5.0], 3.0, 0)];
        a.sel = [1].into_iter().collect();
        assert!(!a.selection_has_rect());
        assert!(a.finger_selection([Edge::Tabs; 4], 10.0, 0.0, None).unwrap().contains("Select a rectangle"));
        a.sel = [0].into_iter().collect();
        assert!(a.finger_selection([Edge::Flat; 4], 10.0, 0.0, None).unwrap().contains("at least one edge"));
        assert!(a.finger_selection([Edge::Tabs, Edge::Flat, Edge::Flat, Edge::Flat], 10.0, 0.0, None).unwrap().contains("do not fit"));
    }
}
