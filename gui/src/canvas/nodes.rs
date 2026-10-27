//! Point editing: moving, adding and removing the points of a shape.
use super::*;

// ------------------------------------------------------------------- point editing

/// Switches tool. Leaves point editing; the node tool starts on the selected shape when there is exactly one.
pub(super) fn set_mode(app: &mut App, m: Mode) {
    // one tool at a time: another tool closes the windows of the selection tools, and leaving Text closes its window.
    // Select is how the shapes for those tools are picked, so it leaves them open.
    if m != Mode::Select { app.close_tool_windows(); }
    if m != Mode::Text { app.close_text_window(); }
    app.mode = m;
    app.poly.clear();
    app.tab_edit = None;
    app.node_edit = None;
    app.node_drag = NodeDrag::None;
    // the drawing tools start from a clean slate: nothing stays selected behind the new shape
    if matches!(m, Mode::Line | Mode::Rect | Mode::Ellipse | Mode::Polygon | Mode::Point | Mode::Text | Mode::Trim) { app.sel.clear(); }
    // the text window opens with the tool, so a position can be typed without clicking first
    if m == Mode::Text { app.text.open = true; }
    if m == Mode::Node && app.sel.len() == 1 { let i = *app.sel.iter().next().unwrap(); enter_node_edit(app, i); }
}

pub(super) fn enter_node_edit(app: &mut App, li: usize) {
    if li >= app.project.data.loops.len() || shapes::is_point(&app.project.data.loops[li]) { return; }
    if app.loop_locked(li) { app.mode = Mode::Select; app.node_edit = None; return; } // locked drawings can't be reshaped
    app.mode = Mode::Node;
    app.poly.clear();
    app.node_edit = Some(li);
    app.node_sel.clear();
    app.node_drag = NodeDrag::None;
    app.node_msg.clear();
    app.tab_edit = None;
    app.sel.clear();
    app.sel.insert(li);
}

/// Index of the point under the pointer, if any (8 px reach).
pub(super) fn node_at(app: &App, pts: &[[f64; 2]], w: [f64; 2]) -> Option<usize> {
    let s = app.scale as f64;
    pts.iter().enumerate()
        .map(|(i, p)| (i, (p[0] - w[0]).hypot(p[1] - w[1]) * s))
        .filter(|(_, d)| *d <= 8.0)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(i, _)| i)
}

pub(super) fn node_keys(ctx: &egui::Context, app: &mut App) {
    let Some(li) = app.node_edit else { return };
    let (esc, del, up, down, left, right, shift) = ctx.input(|i| {
        let free = !i.modifiers.command && !i.modifiers.alt;
        (i.key_pressed(Key::Escape), free && (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)),
         i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight), i.modifiers.shift)
    });
    if esc {
        if app.node_sel.is_empty() { app.node_edit = None; } else { app.node_sel.clear(); }
        return;
    }
    // other tool shortcuts leave point editing
    let tool = ctx.input(|i| {
        let free = !i.modifiers.command && !i.modifiers.alt;
        [(Key::V, Mode::Select), (Key::L, Mode::Line), (Key::R, Mode::Rect), (Key::E, Mode::Ellipse), (Key::P, Mode::Polygon), (Key::D, Mode::Point), (Key::T, Mode::Text), (Key::X, Mode::Trim)].into_iter().find(|(k, _)| free && i.key_pressed(*k)).map(|(_, m)| m)
    });
    if let Some(m) = tool { set_mode(app, m); return; }
    if del && !app.node_sel.is_empty() {
        let mut probe = app.project.data.loops[li].clone();
        if shapes::delete_points(&mut probe, &app.node_sel) {
            app.hist.record(&app.project.data);
            app.project.data.loops[li] = probe;
            app.node_sel.clear();
            app.touch();
        }
    }
    let step = if app.snap_on { app.snap_mm } else { 1.0 } * if shift { 10.0 } else { 1.0 };
    let (dx, dy) = ((right as i32 - left as i32) as f64 * step, (up as i32 - down as i32) as f64 * step);
    if (dx != 0.0 || dy != 0.0) && !app.node_sel.is_empty() {
        app.hist.record(&app.project.data);
        shapes::move_points(&mut app.project.data.loops[li], &app.node_sel, dx, dy);
        app.touch();
    }
}

pub(super) fn node_interaction(ctx: &egui::Context, app: &mut App, resp: &egui::Response, view: &View, li: usize, hover_w: Option<[f64; 2]>, pointer_w: Option<[f64; 2]>, tol: f64) {
    let prim = PointerButton::Primary;
    let shift = ctx.input(|i| i.modifiers.shift);
    let pts = shapes::points(&app.project.data.loops[li]);
    app.node_sel.retain(|&i| i < pts.len());

    if resp.drag_started_by(prim) {
        if let Some(w) = ctx.input(|i| i.pointer.press_origin()).map(|p| view.s2w(p)) {
            match node_at(app, &pts, w) {
                Some(k) => {
                    if !app.node_sel.contains(&k) { if !shift { app.node_sel.clear(); } app.node_sel.insert(k); }
                    app.node_drag = NodeDrag::Move { start: w, base: pts[k], applied: [0.0, 0.0] };
                }
                None => app.node_drag = NodeDrag::Box { start: w },
            }
        }
    }
    if resp.dragged_by(prim) {
        if let (NodeDrag::Move { start, base, applied }, Some(cur)) = (&app.node_drag, pointer_w) {
            let (start, base, applied) = (*start, *base, *applied);
            let t = app.snap([base[0] + cur[0] - start[0], base[1] + cur[1] - start[1]]);
            let desired = [t[0] - base[0], t[1] - base[1]];
            let step = [desired[0] - applied[0], desired[1] - applied[1]];
            if step[0].abs() > 1e-9 || step[1].abs() > 1e-9 {
                app.hist.record_once(&app.project.data);
                shapes::move_points(&mut app.project.data.loops[li], &app.node_sel, step[0], step[1]);
                app.node_drag = NodeDrag::Move { start, base, applied: desired };
                app.touch();
            }
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
    }
    if resp.drag_stopped_by(prim) {
        if let (NodeDrag::Box { start }, Some(end)) = (&app.node_drag, pointer_w) {
            let r = [start[0].min(end[0]), start[1].min(end[1]), start[0].max(end[0]), start[1].max(end[1])];
            if !shift { app.node_sel.clear(); }
            for (i, p) in pts.iter().enumerate() { if p[0] >= r[0] && p[0] <= r[2] && p[1] >= r[1] && p[1] <= r[3] { app.node_sel.insert(i); } }
        }
        app.node_drag = NodeDrag::None;
    }
    if resp.clicked_by(prim) {
        if let Some(w) = hover_w {
            if let Some(k) = node_at(app, &pts, w) {
                if shift { if !app.node_sel.remove(&k) { app.node_sel.insert(k); } } else { app.node_sel.clear(); app.node_sel.insert(k); }
            } else {
                let mut l = app.project.data.loops[li].clone();
                if let Some(k) = shapes::insert_point(&mut l, w, tol) {
                    app.hist.record(&app.project.data);
                    app.project.data.loops[li] = l;
                    app.node_sel.clear();
                    app.node_sel.insert(k);
                    app.touch();
                } else if let Some(h) = shapes::pick(&app.project.data.loops, w, tol).filter(|&h| h != li) {
                    enter_node_edit(app, h); // clicking another shape edits that one instead
                } else if !shift { app.node_sel.clear(); }
            }
        }
    }
    if matches!(app.node_drag, NodeDrag::None) {
        if let Some(w) = hover_w {
            if node_at(app, &pts, w).is_some() { ctx.set_cursor_icon(egui::CursorIcon::Grab); }
            else if app.project.data.loops[li].clone().pts.len() >= 4 && shapes::dist_to_loop(&app.project.data.loops[li], w) <= tol { ctx.set_cursor_icon(egui::CursorIcon::Crosshair); }
        }
    }
}

/// Small bar shown while a shape's points are being edited.
pub(super) fn node_bar(ctx: &egui::Context, app: &mut App, rect: Rect) {
    let Some(li) = app.node_edit else { return };
    let Some(l) = app.project.data.loops.get(li) else { return };
    let n = l.pts.len() / 2;
    let plain = l.rc.is_none() && l.cr.is_none() && l.el.is_none();
    let (mut done, mut simplify, mut corner) = (false, false, false);
    let mut tol = app.node_tol;
    let (mut cham, mut csize, nsel) = (app.corners.chamfer, app.corners.size, app.node_sel.len());
    let _bar = egui::Area::new(egui::Id::new("nodebar")).order(egui::Order::Middle).fixed_pos(rect.min + Vec2::new(10.0, 66.0 + app.palette_extra)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(10.0, 6.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Editing points · {n} point{}", if n == 1 { "" } else { "s" }));
                ui.add_space(6.0);
                ui.label("Simplify within");
                ui.add(egui::DragValue::new(&mut tol).clamp_range(0.01..=5.0).speed(0.01).suffix(" mm"));
                if ui.button("Simplify").on_hover_text("Remove points that hardly change the shape").clicked() { simplify = true; }
                ui.add_space(6.0);
                if crate::icons::icon_text_button(ui, Icon::Check, "Done", "Stop editing points (Esc)", None).clicked() { done = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Corners");
                ui.add_space(4.0);
                let (c, _) = crate::zoomcard::seg2(ui, ["Fillet", "Chamfer"], ["Round the selected corners", "Bevel the selected corners"], cham as usize);
                if let Some(i) = c { cham = i == 1; }
                ui.add_space(4.0);
                crate::widgets::num(ui, [64.0, 24.0], &mut csize, 0.01, 1000.0, 0.25, 2, "");
                ui.weak("mm");
                ui.add_space(4.0);
                if ui.add_enabled(nsel > 0, egui::Button::new(if nsel > 0 { format!("Apply to {nsel} selected") } else { "Apply to selected".to_string() })).on_hover_text("Click a point (Shift-click or drag a box for several), then round or bevel just those corners").clicked() { corner = true; }
            });
            if !plain { ui.weak("Changing a point turns this rectangle or ellipse into a plain polygon."); }
            if !app.node_msg.is_empty() { ui.weak(app.node_msg.as_str()); }
        });
    });
    app.bars_bottom = app.bars_bottom.max(_bar.response.rect.bottom());
    app.node_tol = tol;
    app.corners.chamfer = cham;
    app.corners.size = csize;
    if corner {
        let kind = if cham { shapes::CornerKind::Chamfer } else { shapes::CornerKind::Fillet };
        match shapes::round_corners_at(&app.project.data.loops[li], kind, csize, Some(&app.node_sel)) {
            Some((r, n, lim)) => {
                app.hist.record(&app.project.data);
                app.project.data.loops[li] = r;
                app.node_sel.clear();
                app.touch();
                app.node_msg = format!("{} {n} corner{}{}.", if cham { "Chamfered" } else { "Rounded" }, if n == 1 { "" } else { "s" }, if lim > 0 { format!(" ({lim} too tight, made as large as fits)") } else { String::new() });
            }
            None => app.node_msg = "The selected points are not sharp corners.".into(),
        }
    }
    if simplify {
        let mut probe = app.project.data.loops[li].clone();
        let (before, after) = shapes::simplify(&mut probe, tol);
        if after < before {
            app.hist.record(&app.project.data);
            app.project.data.loops[li] = probe;
            app.node_sel.clear();
            app.touch();
            app.node_msg = format!("Simplified: {before} → {after} points.");
        } else { app.node_msg = "Nothing to simplify at this tolerance.".into(); }
    }
    if done { app.node_edit = None; app.mode = Mode::Select; }
}

#[cfg(test)]
mod tool_window_tests {
    use super::*;
    use crate::app::ToolWin;

    fn open_count(a: &App) -> usize {
        [a.align_open, a.combine_open, a.array.open, a.offset_open, a.corners.open, a.clean.open].iter().filter(|o| **o).count()
    }

    #[test]
    fn opening_a_tool_window_closes_the_others() {
        let mut a = App::new(None);
        a.open_tool_window(ToolWin::Combine);
        a.open_tool_window(ToolWin::Array);
        assert!(a.array.open && !a.combine_open && open_count(&a) == 1);
        a.open_tool_window(ToolWin::Clean);
        assert!(a.clean.open && open_count(&a) == 1);
    }

    #[test]
    fn a_drawing_tool_closes_the_selection_tool_windows_but_select_does_not() {
        let mut a = App::new(None);
        a.open_tool_window(ToolWin::Offset);
        set_mode(&mut a, Mode::Select);
        assert!(a.offset_open, "Select is how the shapes are picked");
        set_mode(&mut a, Mode::Rect);
        assert_eq!(open_count(&a), 0);
        assert_eq!(a.mode, Mode::Rect);
    }

    #[test]
    fn text_and_the_selection_tools_exclude_each_other() {
        let mut a = App::new(None);
        set_mode(&mut a, Mode::Text);
        assert!(a.text.open && a.mode == Mode::Text);
        a.open_tool_window(ToolWin::Combine);
        assert!(!a.text.open && a.mode == Mode::Select && a.combine_open);
        set_mode(&mut a, Mode::Text);
        assert!(a.text.open && !a.combine_open);
        set_mode(&mut a, Mode::Line);
        assert!(!a.text.open);
        // leaving Text for Select closes the Text window as well
        set_mode(&mut a, Mode::Text);
        set_mode(&mut a, Mode::Select);
        assert!(!a.text.open);
    }

    #[test]
    fn the_toolbar_buttons_toggle_one_window_at_a_time() {
        let mut a = App::new(None);
        a.toggle_tool_window(ToolWin::Corners);
        a.toggle_tool_window(ToolWin::Offset);
        assert!(a.offset_open && !a.corners.open && open_count(&a) == 1);
        a.toggle_tool_window(ToolWin::Offset);
        assert_eq!(open_count(&a), 0);
    }

    #[test]
    fn any_other_way_of_opening_a_second_window_leaves_only_the_newest() {
        let mut a = App::new(None);
        a.offset_open = true;
        a.one_tool_window();
        // a window opened by a path that does not know the rule
        a.array.open = true;
        a.one_tool_window();
        assert!(a.array.open && !a.offset_open && open_count(&a) == 1);
        a.corners.open = true;
        a.one_tool_window();
        assert!(a.corners.open && open_count(&a) == 1);
    }
}
