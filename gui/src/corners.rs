//! Fillet / chamfer dialog: round off or bevel the corners of the selected shapes.
use crate::app::App;
use eframe::egui;
use putiacnc_core::shapes::{self, CornerKind};

pub struct CornerTool { pub open: bool, pub chamfer: bool, pub size: f64 }

impl Default for CornerTool { fn default() -> Self { CornerTool { open: false, chamfer: false, size: 2.0 } } }

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

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.corners.open { app.ghost.clear_if("corners"); return; }
    {
        let kind = if app.corners.chamfer { CornerKind::Chamfer } else { CornerKind::Fillet };
        let size = app.corners.size;
        app.preview("corners", &[app.corners.chamfer as u64, crate::preview::bits(size)], move |a| a.corner_selection(kind, size));
    }
    let pnote = app.ghost.note_of("corners").map(String::from);
    let n = app.sel.len();
    let has = app.selection_has_corners();
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Fillet and chamfer").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Fillet and chamfer", crate::csd::anchor(app))).show(ctx, |ui| {
        ui.set_width(320.0);
        if crate::csd::dialog_header(ui, "Fillet and chamfer", true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if n == 0 { "Select one or more shapes first.".to_string() } else { format!("{n} shape{} selected.", if n == 1 { "" } else { "s" }) });
        ui.add_space(6.0);
        egui::Grid::new("corner_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label("Corner");
            ui.horizontal(|ui| {
                if ui.selectable_label(!app.corners.chamfer, "Fillet").on_hover_text("Round the corners").clicked() { app.corners.chamfer = false; }
                if ui.selectable_label(app.corners.chamfer, "Chamfer").on_hover_text("Cut the corners off with a straight bevel").clicked() { app.corners.chamfer = true; }
            });
            ui.end_row();
            ui.label(if app.corners.chamfer { "Distance" } else { "Radius" });
            ui.horizontal(|ui| {
                crate::widgets::num(ui, [70.0, 24.0], &mut app.corners.size, 0.01, 1000.0, 0.25, 2, "");
                ui.weak("mm");
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.checkbox(&mut app.preview_on, "Preview").on_hover_text("Show the result in the drawing while you change the values");
        if let Some(n) = &pnote { ui.weak(n); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(has, egui::Button::new("Apply")).clicked() { apply = true; }
            if ui.button("Close").clicked() { close = true; }
        });
        ui.add_space(2.0);
        ui.weak(if app.corners.chamfer { "The distance is measured along each edge from the corner." } else { "Smooth curves, circles and ellipses are left alone." });
        });
    });
    if apply {
        let kind = if app.corners.chamfer { CornerKind::Chamfer } else { CornerKind::Fillet };
        let size = app.corners.size;
        app.ghost.clear();
        if let Some(msg) = app.corner_selection(kind, size) { app.error = Some(msg); }
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
    fn rounding_the_corners_of_text_turns_it_into_outlines() {
        use putiacnc_core::model::{ShapeGroup, TextSpec};
        use putiacnc_core::text::{Align, Flow};
        let mut r = shapes::new_rect([0.0, 0.0], [20.0, 10.0], 0.0, 1);
        r.grp = Some(1);
        let mut a = app_with(vec![r]);
        a.project.data.groups.push(ShapeGroup { id: 1, name: "Text: A".into(), locked: false, text: Some(TextSpec {
            content: "A".into(), font: "Hack".into(), size: 10.0, spacing: 0.0, align: Align::Left, flow: Flow::Straight, radius: 0.0, shadow: [0.0, 0.0] }) });
        let msg = a.corner_selection(CornerKind::Fillet, 1.0).expect("a note about the text");
        assert!(msg.contains("plain outlines"), "{msg}");
        assert!(a.project.data.groups.is_empty() && a.project.data.loops[0].grp.is_none());
    }
}
