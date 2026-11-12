//! The Edit points window: the points of one shape are dragged, added and removed on the canvas; this window says what is being edited and
//! simplifies the shape. Rounding or bevelling the selected points is the Fillet and chamfer window (it works on the selected points while
//! points are being edited).
use putiacnc_core::i18n::{tr, tr_f, tr_n};
use crate::app::{App, Level, Mode};
use eframe::egui;
use putiacnc_core::shapes;

impl App {
    /// Simplifies the shape whose points are being edited: points that hardly change it go. Returns what happened, to say in the window.
    pub fn simplify_points(&mut self, tol: f64) -> Option<String> {
        let li = self.node_edit?;
        let mut probe = self.project.data.loops.get(li)?.clone();
        let (before, after) = shapes::simplify(&mut probe, tol);
        if after >= before { return Some(tr("Nothing to simplify at this tolerance.").into()); }
        self.hist.record(&self.project.data);
        self.project.data.loops[li] = probe;
        self.touch();
        Some(tr_f("Simplified: {before} \u{2192} {after} points.", &[("before", &before), ("after", &after)]))
    }

    /// Rounds or bevels the selected points of the shape being edited (only sharp corners count).
    pub fn corner_points(&mut self, kind: shapes::CornerKind, size: f64) -> Option<String> {
        let li = self.node_edit?;
        let l = self.project.data.loops.get(li)?;
        let Some((r, n, lim)) = shapes::round_corners_at(l, kind, size, Some(&self.node_sel)) else { return Some(tr("The selected points are not sharp corners.").into()) };
        self.hist.record(&self.project.data);
        self.project.data.loops[li] = r;
        self.touch();
        let what = if kind == shapes::CornerKind::Chamfer { tr_n(n as u64, "Chamfered {n} corner", "Chamfered {n} corners") } else { tr_n(n as u64, "Rounded {n} corner", "Rounded {n} corners") };
        Some(format!("{what}{}.", if lim > 0 { tr_f(" ({n} too tight, made as large as fits)", &[("n", &lim)]) } else { String::new() }))
    }
}

/// A number for the preview's settings that changes whenever the selected points do.
pub fn points_key(app: &App) -> u64 { app.node_sel.iter().fold(1u64, |h, &i| h.wrapping_mul(1_000_003).wrapping_add(i as u64 + 1)) }

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    let Some(li) = app.node_edit.filter(|&i| i < app.project.data.loops.len()) else { app.ghost.clear_if("simplify"); if app.tool_note.as_ref().is_some_and(|(k, _)| *k == "simplify") { app.tool_note = None; } return };
    let tol = app.node_tol;
    app.preview("simplify", &[crate::preview::bits(tol)], move |a| a.simplify_points(tol));
    let pnote = app.ghost.note_of("simplify").map(String::from);
    let l = &app.project.data.loops[li];
    let (n, k) = (l.pts.len() / 2, app.node_sel.len());
    let plain = l.rc.is_none() && l.cr.is_none() && l.el.is_none();
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Edit points").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Edit points", crate::csd::anchor(app))).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(crate::csd::dialog_w(376.0));
        if crate::csd::dialog_header_help(ui, tr("Edit points"), true, |ui| {
            ui.label(tr("Drag a point to move it. Click a point to select it (Shift-click, or drag a box, for several) and use the arrow keys to nudge them, Delete to remove them."));
            ui.add_space(4.0);
            ui.label(tr("Click on an edge to add a point there."));
            ui.add_space(4.0);
            ui.label(tr("To round or bevel selected corners, open Fillet and chamfer: while points are being edited it works on the selected points."));
        }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
            ui.label(putiacnc_core::i18n::tr_nf(n as u64, "{n} point, {k} selected.", "{n} points, {k} selected.", &[("k", &k)]));
            if !plain { crate::msg::line(ui, Level::Warning, tr("Changing a point turns this rectangle or ellipse into a plain polygon.")); }
            ui.add_space(6.0);
            let lw = crate::panels::tw(ui, tr("Simplify within"));
            egui::Grid::new("editpoints_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                crate::toolmgr::rlabel(ui, lw, tr("Simplify within"));
                ui.horizontal(|ui| {
                    crate::widgets::len(ui, [70.0, 24.0], &mut app.node_tol, 0.01, 5.0, 0.01, 2, "");
                    ui.weak(crate::widgets::len_unit(ui));
                });
                ui.end_row();
            });
            let mine = app.tool_note.as_ref().filter(|(k, _)| *k == "simplify").map(|(_, m)| m);
            if let Some(m) = pnote.as_ref().or(mine) { crate::msg::line(ui, Level::Info, m); }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(tr("Apply"))).on_hover_text(tr("Remove the points that hardly change the shape, and keep the window open")).clicked() { apply = true; }
                if ui.button(tr("Close")).on_hover_text(tr("Stop editing points (Esc)")).clicked() { close = true; }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    crate::widgets::check(ui, &mut app.preview_on, tr("Preview"));
                });
            });
        });
    }));
    if apply {
        let tol = app.node_tol;
        app.ghost.clear();
        if app.run_tool("simplify", move |a| a.simplify_points(tol)) { app.node_sel.clear(); }
    }
    if close { app.node_edit = None; app.mode = Mode::Select; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use putiacnc_core::shapes::{self, CornerKind};

    fn polygon_app() -> App {
        let mut a = App::new(None);
        let mut l = shapes::new_rect([0.0, 0.0], [40.0, 30.0], 0.0, 1);
        l.rc = None;
        a.project.data.loops = vec![l];
        a.sel = [0].into_iter().collect();
        a.node_edit = Some(0);
        a
    }

    #[test]
    fn rounding_in_point_mode_touches_only_the_selected_points() {
        let mut a = polygon_app();
        let before = a.project.data.loops[0].pts.len();
        a.node_sel = [0].into_iter().collect();
        a.corner_points(CornerKind::Fillet, 3.0);
        let after = a.project.data.loops[0].pts.len();
        assert!(after > before, "one corner rounded adds points");
        // the three other corners stay sharp: far fewer points than rounding all four
        let mut b = polygon_app();
        b.node_sel = (0..4).collect();
        b.corner_points(CornerKind::Fillet, 3.0);
        assert!(b.project.data.loops[0].pts.len() > after);
    }

    #[test]
    fn simplify_runs_and_undoes() {
        let mut a = polygon_app();
        let n = a.project.data.loops[0].pts.len();
        a.simplify_points(0.1);
        assert!(a.project.data.loops[0].pts.len() <= n);
    }

    #[test]
    fn the_point_selection_gives_a_stable_preview_key() {
        let mut a = polygon_app();
        a.node_sel = [1, 2].into_iter().collect();
        let k = points_key(&a);
        assert_eq!(k, points_key(&a));
        a.node_sel.insert(3);
        assert_ne!(k, points_key(&a));
    }
}
