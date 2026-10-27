//! Placing holding tabs by hand on an operation's outline.
use super::*;

// ------------------------------------------------------------------- manual holding tabs

/// How close (screen pixels) the pointer must be to an outline to add a tab there.
pub(super) const TAB_REACH_PX: f64 = 16.0;

/// Closest point on the closed shapes of operation `op_id`: (shape index, point, distance, direction).
pub(super) fn nearest_outline(app: &App, op_id: i64, w: [f64; 2]) -> Option<(usize, [f64; 2], f64, [f64; 2])> {
    app.project.data.loops.iter().enumerate().filter(|(_, l)| l.op == op_id && l.closed())
        .filter_map(|(i, l)| shapes::nearest_on_loop(l, w).map(|(q, d, t)| (i, q, d, t)))
        .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
}

/// Index of the placed tab under the pointer, if any.
pub(super) fn hovered_tab(app: &App, op_id: i64, w: [f64; 2]) -> Option<usize> {
    let d = &app.project.data;
    let tp = d.ops.iter().find(|o| o.id == op_id)?.p.tab_pts.as_ref()?;
    let s = app.scale as f64;
    tp.iter().enumerate()
        .filter_map(|(i, t)| shapes::tab_resolve(&d.loops, op_id, t).map(|(q, _)| (i, (q[0] - w[0]).hypot(q[1] - w[1]) * s)))
        .filter(|(_, d)| *d < 11.0)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(i, _)| i)
}

pub(super) fn tab_interaction(ctx: &egui::Context, app: &mut App, resp: &egui::Response, view: &View, op_id: i64, hover_w: Option<[f64; 2]>) {
    let prim = PointerButton::Primary;
    let pointer_w = ctx.input(|i| i.pointer.latest_pos()).map(|p| view.s2w(p));
    if resp.drag_started_by(prim) {
        if let Some(w) = ctx.input(|i| i.pointer.press_origin()).map(|p| view.s2w(p)) { app.tab_drag = hovered_tab(app, op_id, w); }
    }
    if resp.dragged_by(prim) {
        if let (Some(i), Some(cur)) = (app.tab_drag, pointer_w) {
            if let Some((li, q, _, _)) = nearest_outline(app, op_id, cur) {
                if let Some(anchor) = shapes::tab_anchor(&app.project.data.loops, li, q) {
                    app.hist.record_once(&app.project.data);
                    if let Some(t) = app.project.data.ops.iter_mut().find(|o| o.id == op_id).and_then(|o| o.p.tab_pts.as_mut()).and_then(|v| v.get_mut(i)) { *t = anchor; }
                    app.touch();
                }
            }
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
    }
    if resp.drag_stopped_by(prim) { app.tab_drag = None; }
    if resp.clicked_by(prim) {
        if let Some(w) = hover_w {
            if hovered_tab(app, op_id, w).is_none() {
                if let Some((li, q, dist, _)) = nearest_outline(app, op_id, w) {
                    if dist * app.scale as f64 <= TAB_REACH_PX {
                        if let Some(anchor) = shapes::tab_anchor(&app.project.data.loops, li, q) {
                            app.hist.record(&app.project.data);
                            if let Some(v) = app.project.data.ops.iter_mut().find(|o| o.id == op_id).and_then(|o| o.p.tab_pts.as_mut()) { v.push(anchor); }
                            app.touch();
                        }
                    }
                }
            }
        }
    }
    if resp.clicked_by(PointerButton::Secondary) {
        if let Some(i) = hover_w.and_then(|w| hovered_tab(app, op_id, w)) {
            app.hist.record(&app.project.data);
            if let Some(v) = app.project.data.ops.iter_mut().find(|o| o.id == op_id).and_then(|o| o.p.tab_pts.as_mut()) { if i < v.len() { v.remove(i); } }
            app.touch();
        } else {
            app.esc_request = true; // not on a tab: leave the mode, as Esc does
        }
    }
    if app.tab_drag.is_none() {
        if let Some(w) = hover_w {
            if hovered_tab(app, op_id, w).is_some() { ctx.set_cursor_icon(egui::CursorIcon::Grab); }
            else if nearest_outline(app, op_id, w).is_some_and(|n| n.2 * app.scale as f64 <= TAB_REACH_PX) { ctx.set_cursor_icon(egui::CursorIcon::Crosshair); }
        }
    }
}

/// Small bar shown while tabs are being placed.
pub(super) fn tab_bar(ctx: &egui::Context, app: &mut App, rect: Rect) {
    let Some(id) = app.tab_edit else { return };
    let n = app.project.data.ops.iter().find(|o| o.id == id).and_then(|o| o.p.tab_pts.as_ref()).map_or(0, |v| v.len());
    let k = app.project.data.ops.iter().position(|o| o.id == id).map_or(0, |k| k + 1);
    let mut done = false;
    let _bar = egui::Area::new(egui::Id::new("tabbar")).order(egui::Order::Middle).fixed_pos(rect.min + Vec2::new(10.0, 66.0 + app.palette_extra)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(10.0, 6.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Placing tabs for operation {k} · {n} tab{}", if n == 1 { "" } else { "s" }));
                ui.add_space(6.0);
                if crate::icons::icon_text_button(ui, Icon::Check, "Done", "Stop placing tabs (Esc)", None).clicked() { done = true; }
            });
        });
    });
    app.bars_bottom = app.bars_bottom.max(_bar.response.rect.bottom());
    if done { app.tab_edit = None; }
}
