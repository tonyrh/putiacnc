//! Helpers for the drawing tools: shapes from a drag, snapping the end point, move targets.
use super::*;

/// The polygon or star dragged out from its centre: the first corner follows the pointer, Shift points it straight up.
pub(super) fn poly_from_drag(app: &App, centre: [f64; 2], to: [f64; 2], shift: bool, op: i64) -> Option<putiacnc_core::model::LoopData> {
    let (dx, dy) = (to[0] - centre[0], to[1] - centre[1]);
    let r = (dx * dx + dy * dy).sqrt();
    let angle = if shift { std::f64::consts::FRAC_PI_2 } else { dy.atan2(dx) };
    let inner = if app.poly_star { Some(app.poly_inner / 100.0) } else { None };
    shapes::new_polygon(centre, r, angle, app.poly_sides.round() as usize, inner, op)
}

/// The ring of drill points dragged out from its centre (first hole at the pointer, Shift = straight up).
pub(super) fn ring_from_drag(app: &App, centre: [f64; 2], to: [f64; 2], shift: bool, op: i64) -> Vec<putiacnc_core::model::LoopData> {
    if app.point_grid { return shapes::point_grid(centre, to, app.grid_cols.round() as usize, app.grid_rows.round() as usize, op); }
    let (dx, dy) = (to[0] - centre[0], to[1] - centre[1]);
    let r = (dx * dx + dy * dy).sqrt();
    let angle = if shift { std::f64::consts::FRAC_PI_2 } else { dy.atan2(dx) };
    shapes::bolt_circle(centre, r, angle, app.point_count.round() as usize, app.point_sweep, op)
}

/// Second corner of a rectangle/ellipse drag; Shift forces a square (circle).
pub(super) fn draw_end(app: &App, a: [f64; 2], b: [f64; 2], shift: bool) -> [f64; 2] {
    if (app.mode == Mode::Ellipse || app.mode == Mode::Rect) && shift {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let m = dx.abs().max(dy.abs());
        [a[0] + m * if dx < 0.0 { -1.0 } else { 1.0 }, a[1] + m * if dy < 0.0 { -1.0 } else { 1.0 }]
    } else { b }
}

/// Options of the active drawing tool, in a second bar under the palette so the palette itself stays compact.
/// Where the selection's lower-left corner goes while it is dragged: the key points of its box
/// (corners, edge middles, centre) jump to a nearby snap point, otherwise the corner follows the grid.
pub(super) fn move_target(app: &App, start: [f64; 2], anchor: [f64; 2], applied: [f64; 2], cur: [f64; 2]) -> [f64; 2] {
    let delta = [cur[0] - start[0], cur[1] - start[1]];
    let mut best: Option<([f64; 2], f64, crate::osnap::Hit)> = None;
    if app.osnap_on {
        if let Some(b) = shapes::bbox_many(app.sel.iter().filter_map(|&i| app.project.data.loops.get(i))) {
            // the box is already at `applied`; this is where it would be with the full drag
            let (ox, oy) = (delta[0] - applied[0], delta[1] - applied[1]);
            let (mx, my) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
            for q in [[b[0], b[1]], [mx, b[1]], [b[2], b[1]], [b[0], my], [mx, my], [b[2], my], [b[0], b[3]], [mx, b[3]], [b[2], b[3]]] {
                let q = [q[0] + ox, q[1] + oy];
                if let Some(h) = app.object_snap(q) {
                    let c = [h.p[0] - q[0], h.p[1] - q[1]];
                    let d = c[0].hypot(c[1]);
                    if best.is_none_or(|(_, bd, _)| d < bd) { best = Some((c, d, h)); }
                }
            }
        }
    }
    app.snap_hit.set(best.map(|b| b.2));
    match best {
        Some((c, _, _)) => [anchor[0] + delta[0] + c[0], anchor[1] + delta[1] + c[1]],
        None => app.grid_snap([anchor[0] + delta[0], anchor[1] + delta[1]]),
    }
}
