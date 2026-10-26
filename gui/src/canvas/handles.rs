//! Resize and rotate handles of the selection, and the selection bar.
use super::*;

/// In rotate mode: a small field above the selection to turn it by a typed angle (counter-clockwise is positive).
/// While a corner is being dragged it shows the angle turned so far.
pub(super) fn rotate_popup(ctx: &egui::Context, app: &mut App, rect: Rect) {
    if !app.rotate_mode || app.mode != Mode::Select || app.sel.is_empty() || app.sel_locked() { return; }
    let Some(b) = shapes::bbox_many(app.sel.iter().filter_map(|&i| app.project.data.loops.get(i))) else { return };
    let view = View { scale: app.scale, offset: app.offset, origin: rect.min };
    let top = view.w2s([(b[0] + b[2]) / 2.0, b[3]]);
    let live = if let Drag::Rotate { total, .. } = &app.drag { Some(*total) } else { None };
    let mut v = live.unwrap_or(0.0);
    let mut delta: Option<f64> = None;
    let mut typed = false;
    let pos = Pos2::new((top.x - 62.0).clamp(rect.min.x + RULER + 4.0, (rect.max.x - 130.0).max(rect.min.x + RULER + 4.0)), (top.y - 52.0).max(rect.min.y + RULER + 4.0));
    egui::Area::new(egui::Id::new("rotate_popup")).order(egui::Order::Middle).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label("Rotate");
                ui.add_space(4.0); // as far from its field as the labels in the selection bar
                let r = crate::widgets::num(ui, [54.0, 24.0], &mut v, -360.0, 360.0, 0.5, 1, "")
                    .on_hover_text("Turn the selection by this many degrees about the centre mark (counter-clockwise is positive). Drag a corner handle to turn it with the mouse; Shift snaps to 15°.");
                if r.changed() && live.is_none() && v != 0.0 { delta = Some(v); typed = !r.dragged(); }
                ui.weak("°");
            });
        });
    });
    if let Some(deg) = delta {
        let before = app.project.data.clone();
        let pivot = app.rot_pivot.unwrap_or([(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0]);
        let sel = app.sel.clone();
        let d = &mut app.project.data;
        let tabs0 = shapes::tab_positions(&d.loops, &d.ops, &sel);
        shapes::rotate_selection(&mut d.loops, &mut d.ops, &sel, pivot, deg, &tabs0);
        app.rot_pivot = Some(pivot);
        if !app.hist.open { app.hist.push(before); app.hist.open = true; }
        app.touch();
        // a typed angle finishes the turn (Enter), like a dialog's OK: back to the normal handles. Dragging the number keeps going.
        if typed { app.rotate_mode = false; app.rot_pivot = None; }
    }
}

// ------------------------------------------------------------------- resize handles

/// World position of handle `h` on bounding box `b = [x0, y0, x1, y1]`
/// (0 top-left, 1 top, 2 top-right, 3 right, 4 bottom-right, 5 bottom, 6 bottom-left, 7 left).
pub(super) fn handle_world(b: [f64; 4], h: usize) -> [f64; 2] {
    let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    match h {
        0 => [b[0], b[3]], 1 => [cx, b[3]], 2 => [b[2], b[3]], 3 => [b[2], cy],
        4 => [b[2], b[1]], 5 => [cx, b[1]], 6 => [b[0], b[1]], _ => [b[0], cy],
    }
}

/// Handles are drawn 4 px outside the selection box so they don't cover the shapes.
pub(super) fn handle_screen(view: &View, b: [f64; 4], h: usize) -> Pos2 {
    let p = view.w2s(handle_world(b, h));
    let dx = match h { 0 | 6 | 7 => -4.0, 2..=4 => 4.0, _ => 0.0 };
    let dy = match h { 0..=2 => -4.0, 4..=6 => 4.0, _ => 0.0 };
    p + Vec2::new(dx, dy)
}

/// Resize handles are for real shapes and never for a locked drawing; a selection of only drill points has none (a point keeps its size).
pub(super) fn handles_apply(app: &App) -> bool {
    !app.sel_locked() && app.sel.iter().any(|&i| app.project.data.loops.get(i).is_some_and(|l| !shapes::is_point(l)))
}

pub(super) fn hit_handle(view: &View, b: [f64; 4], pos: Pos2) -> Option<usize> {
    (0..8).find(|&h| handle_screen(view, b, h).distance(pos) <= 9.0)
}

pub(super) fn handle_cursor(h: usize) -> egui::CursorIcon {
    match h {
        0 | 4 => egui::CursorIcon::ResizeNwSe,
        2 | 6 => egui::CursorIcon::ResizeNeSw,
        1 | 5 => egui::CursorIcon::ResizeVertical,
        _ => egui::CursorIcon::ResizeHorizontal,
    }
}

/// New bounding box while dragging handle `handle`. Shift keeps the aspect ratio (corners),
/// Alt scales symmetrically about the centre; the moving edge snaps to the grid.
pub(super) fn resized_bbox(app: &App, handle: usize, sb: [f64; 4], grab: [f64; 2], cur: [f64; 2], shift: bool, alt: bool) -> [f64; 4] {
    let (mx0, mx1) = (matches!(handle, 0 | 6 | 7), matches!(handle, 2..=4));
    let (my0, my1) = (matches!(handle, 4..=6), matches!(handle, 0..=2));
    let p = app.snap([cur[0] + grab[0], cur[1] + grab[1]]);
    let (cx, cy) = ((sb[0] + sb[2]) / 2.0, (sb[1] + sb[3]) / 2.0);
    let (ow, oh) = (sb[2] - sb[0], sb[3] - sb[1]);
    let mut n = sb;
    if mx0 { n[0] = p[0]; }
    if mx1 { n[2] = p[0]; }
    if my0 { n[1] = p[1]; }
    if my1 { n[3] = p[1]; }
    if alt {
        if mx0 || mx1 { let h = (p[0] - cx).abs(); n[0] = cx - h; n[2] = cx + h; }
        if my0 || my1 { let h = (p[1] - cy).abs(); n[1] = cy - h; n[3] = cy + h; }
    }
    if shift && matches!(handle, 0 | 2 | 4 | 6) && ow > 1e-9 && oh > 1e-9 {
        let s = ((n[2] - n[0]) / ow).abs().max(((n[3] - n[1]) / oh).abs());
        let (nw, nh) = (ow * s, oh * s);
        if alt { n = [cx - nw / 2.0, cy - nh / 2.0, cx + nw / 2.0, cy + nh / 2.0]; }
        else {
            let fx = if mx0 { sb[2] } else { sb[0] };
            let fy = if my0 { sb[3] } else { sb[1] };
            n[0] = if mx0 { fx - nw } else { fx }; n[2] = if mx0 { fx } else { fx + nw };
            n[1] = if my0 { fy - nh } else { fy }; n[3] = if my0 { fy } else { fy + nh };
        }
    }
    // a degenerate axis (a straight horizontal or vertical line) can't be scaled
    if ow <= 1e-6 { n[0] = sb[0]; n[2] = sb[2]; }
    if oh <= 1e-6 { n[1] = sb[1]; n[3] = sb[3]; }
    // never flip or collapse
    if ow > 1e-6 && n[2] - n[0] < 0.01 { let m = (n[0] + n[2]) / 2.0; n[0] = m - 0.005; n[2] = m + 0.005; }
    if oh > 1e-6 && n[3] - n[1] < 0.01 { let m = (n[1] + n[3]) / 2.0; n[1] = m - 0.005; n[3] = m + 0.005; }
    n
}

/// Position and size of the selected shapes, as a small bar over the canvas.
pub(super) fn selection_bar(ctx: &egui::Context, app: &mut App, rect: Rect) {
    let n = app.sel.len();
    app.sel.retain(|&i| i < app.project.data.loops.len());
    if app.sel.is_empty() || app.mode != Mode::Select { return; }
    let before = app.project.data.clone();
    let sel = app.sel.clone();
    let d = &mut app.project.data;
    let Some(b) = shapes::bbox_many(sel.iter().map(|&i| &d.loops[i])) else { return };
    // X and Y are relative to the XY zero, like the rulers and the pointer readout
    let (zx, zy) = (match d.stock.zr.as_str() { "lr" | "ur" => d.stock.w, "c" => d.stock.w / 2.0, _ => 0.0 }, match d.stock.zr.as_str() { "ul" | "ur" => d.stock.h, "c" => d.stock.h / 2.0, _ => 0.0 });
    let (mut x, mut y, mut w, mut h) = (b[0] - zx, b[1] - zy, b[2] - b[0], b[3] - b[1]);
    let mut changed = false;
    let mut geom = false;
    let _ = n;
    // shapes of a locked drawing cannot be moved, resized or aligned
    let locked = sel.iter().filter_map(|&i| d.loops[i].grp).any(|g| d.groups.iter().any(|x| x.id == g && x.locked));
    let _bar = egui::Area::new(egui::Id::new("selbar")).order(egui::Order::Middle).fixed_pos(rect.min + Vec2::new(10.0, 66.0 + app.palette_extra)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(8.0, 5.0)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let field = |ui: &mut Ui, label: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
                    ui.label(label);
                    ui.add_space(4.0); // as far from its field as the labels in the Stock panel
                    ui.add_enabled_ui(!locked, |ui| crate::widgets::num(ui, [66.0, 24.0], v, lo, hi, 0.25, 2, "").changed()).inner
                };
                geom |= field(ui, "X", &mut x, -100000.0, 100000.0);
                ui.add_space(10.0);
                geom |= field(ui, "Y", &mut y, -100000.0, 100000.0);
                ui.add_space(10.0);
                geom |= field(ui, "W", &mut w, 0.01, 100000.0);
                ui.add_space(10.0);
                geom |= field(ui, "H", &mut h, 0.01, 100000.0);
                ui.weak("mm");
            });
        });
    });
    app.bars_bottom = app.bars_bottom.max(_bar.response.rect.bottom());
    if locked { geom = false; }
    if geom {
        let mut refs: Vec<&mut putiacnc_core::model::LoopData> = d.loops.iter_mut().enumerate().filter(|(i, _)| sel.contains(i)).map(|(_, l)| l).collect();
        shapes::fit_to_bbox(&mut refs, b, [x + zx, y + zy, x + zx + w.max(0.01), y + zy + h.max(0.01)]);
        changed = true;
    }
    if changed {
        if !app.hist.open {
            app.hist.push(before);
            app.hist.open = true;
        }
        app.touch();
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App { let mut a = App::new(None); a.snap_on = true; a.snap_mm = 1.0; a }
    fn close(a: [f64; 4], b: [f64; 4]) -> bool { a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6) }

    #[test]
    fn a_dragged_selection_snaps_its_key_points_to_other_shapes() {
        let mut a = app();
        a.snap_on = false;
        a.snap_tol_mm = 1.0;
        a.project.data.loops = vec![
            shapes::new_rect([100.0, 100.0], [140.0, 120.0], 0.0, 0),
            shapes::new_rect([10.0, 10.0], [30.0, 20.0], 0.0, 0), // the one being moved
        ];
        a.sel = [1usize].into_iter().collect();
        a.snap_skip = vec![1];
        // drag so that its upper-right corner (30,20) lands 0.4 mm from the other's lower-left corner (100,100)
        let t = move_target(&a, [0.0, 0.0], [10.0, 10.0], [0.0, 0.0], [70.3, 80.4]);
        // the box (20 x 10) must end with its upper right on (100,100): lower left at (80,90)
        assert!((t[0] - 80.0).abs() < 1e-9 && (t[1] - 90.0).abs() < 1e-9, "{t:?}");
        assert_eq!(a.snap_hit.get().map(|h| h.kind), Some(crate::osnap::Kind::Corner));
        // far from everything: plain drag
        let t = move_target(&a, [0.0, 0.0], [10.0, 10.0], [0.0, 0.0], [40.0, 30.0]);
        assert!((t[0] - 50.0).abs() < 1e-9 && (t[1] - 40.0).abs() < 1e-9, "{t:?}");
        assert!(a.snap_hit.get().is_none());
    }

    #[test]
    fn drag_corner_free() {
        let n = resized_bbox(&app(), 4, [0.0, 0.0, 100.0, 50.0], [0.0, 0.0], [150.2, -19.9], false, false);
        assert!(close(n, [0.0, -20.0, 150.0, 50.0]), "{n:?}"); // bottom-right: the bottom edge moves down
    }

    #[test]
    fn drag_corner_proportional() {
        let n = resized_bbox(&app(), 4, [0.0, 0.0, 100.0, 50.0], [0.0, 0.0], [150.0, -30.0], true, false);
        assert!(close(n, [0.0, -30.0, 160.0, 50.0]), "{n:?}"); // ratio 2:1 kept, larger scale wins
        let n = resized_bbox(&app(), 0, [0.0, 0.0, 100.0, 50.0], [0.0, 0.0], [-50.0, 100.0], true, false);
        assert!(close(n, [-100.0, 0.0, 100.0, 100.0]), "{n:?}"); // top-left moves, bottom-right fixed
    }

    #[test]
    fn drag_from_centre() {
        let n = resized_bbox(&app(), 4, [0.0, 0.0, 100.0, 50.0], [0.0, 0.0], [150.0, 70.0], false, true);
        assert!(close(n, [-50.0, -20.0, 150.0, 70.0]), "{n:?}");
    }

    #[test]
    fn edge_handles_move_one_axis_and_never_flip() {
        let n = resized_bbox(&app(), 3, [0.0, 0.0, 100.0, 50.0], [0.0, 0.0], [130.0, 999.0], false, false);
        assert!(close(n, [0.0, 0.0, 130.0, 50.0]), "{n:?}");
        let n = resized_bbox(&app(), 7, [0.0, 0.0, 100.0, 50.0], [0.0, 0.0], [120.0, 0.0], false, false);
        assert!(n[2] - n[0] >= 0.0099 && n[0] < n[2], "{n:?}");
    }

    #[test]
    fn grab_offset_prevents_jump_and_degenerate_axes_stay() {
        // handle sits 4 px outside the box: pressing 3 mm beyond the edge must not shift it
        let n = resized_bbox(&app(), 3, [0.0, 0.0, 100.0, 50.0], [-3.0, 0.0], [103.0, 0.0], false, false);
        assert!(close(n, [0.0, 0.0, 100.0, 50.0]), "{n:?}");
        let n = resized_bbox(&app(), 4, [0.0, 10.0, 100.0, 10.0], [0.0, 0.0], [150.0, 40.0], false, false);
        assert!(close(n, [0.0, 10.0, 150.0, 10.0]), "{n:?}"); // a horizontal line keeps its height
    }
}
