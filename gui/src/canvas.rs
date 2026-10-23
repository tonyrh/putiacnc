//! The 2D drawing canvas: view control, drawing tools, selection and keyboard shortcuts.
use crate::app::{Action, App, Drag, Mode, NodeDrag, View};
use crate::icons::{icon_button, Icon};
use crate::theme::op_color;
use eframe::egui::{self, Color32, Key, Modifiers, PointerButton, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use putiacnc_core::shapes;

fn finish_poly(app: &mut App, close: bool) {
    let op = app.project.data.act;
    let pts = std::mem::take(&mut app.poly);
    if let Some(l) = shapes::new_poly(&pts, close, op) { app.add_loop(l); }
}

pub fn handle_keys(ctx: &egui::Context, app: &mut App) {
    if ctx.wants_keyboard_input() { return; }
    let (save_as, save, redo, redo2, undo, all, dup, new, open, export, quit, import) = ctx.input_mut(|i| {
        let save_as = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S);
        let save = i.consume_key(Modifiers::COMMAND, Key::S);
        let redo = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        let redo2 = i.consume_key(Modifiers::COMMAND, Key::Y);
        let undo = i.consume_key(Modifiers::COMMAND, Key::Z);
        let all = i.consume_key(Modifiers::COMMAND, Key::A);
        let dup = i.consume_key(Modifiers::COMMAND, Key::D);
        let new = i.consume_key(Modifiers::COMMAND, Key::N);
        let open = i.consume_key(Modifiers::COMMAND, Key::O);
        let export = i.consume_key(Modifiers::COMMAND, Key::E);
        let quit = i.consume_key(Modifiers::COMMAND, Key::Q);
        let import = i.consume_key(Modifiers::COMMAND, Key::I);
        (save_as, save, redo, redo2, undo, all, dup, new, open, export, quit, import)
    });
    if save_as { app.save_as_dialog(); }
    if new { app.request(Action::New); }
    if open { app.open_dialog(); }
    if import { app.import_dialog(); }
    if export { app.export_gcode_dialog(); }
    if quit { app.request(Action::Quit); }
    if save { app.save(); }
    if undo { app.do_undo(); }
    if redo || redo2 { app.do_redo(); }
    if ctx.input(|i| i.key_pressed(Key::F1)) { app.shortcuts_open = !app.shortcuts_open; }
    if all { app.sel = (0..app.project.data.loops.len()).collect(); }
    if dup { app.duplicate_selection(); }

    // while tabs are being placed the other shortcuts (and Del) are off; Esc ends the mode
    if app.tab_edit.is_some() {
        if ctx.input(|i| i.key_pressed(Key::Escape)) { app.tab_edit = None; }
        return;
    }
    if app.node_edit.is_some() { node_keys(ctx, app); return; }
    // N: node tool (edits the points of the selected shape, or of the next one you click)
    if ctx.input(|i| i.key_pressed(Key::N) && !i.modifiers.command && !i.modifiers.alt) {
        set_mode(app, Mode::Node);
        return;
    }
    if ctx.input(|i| i.key_pressed(Key::T) && !i.modifiers.command && !i.modifiers.alt) {
        set_mode(app, Mode::Text);
        // the key that opened the window must not be typed into it
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Text(_))));
        app.text.focus = true;
        return;
    }
    if ctx.input(|i| i.key_pressed(Key::X) && !i.modifiers.command && !i.modifiers.alt) {
        set_mode(app, Mode::Trim);
        return;
    }
    let (v, l, r, e, pg, dp, del, esc, enter, c, up, down, left, right, shift) = ctx.input(|i| {
        let free = !i.modifiers.command && !i.modifiers.alt;
        (
            free && i.key_pressed(Key::V), free && i.key_pressed(Key::L), free && i.key_pressed(Key::R), free && i.key_pressed(Key::E), free && i.key_pressed(Key::P), free && i.key_pressed(Key::D),
            free && (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)),
            i.key_pressed(Key::Escape), i.key_pressed(Key::Enter), free && i.key_pressed(Key::C),
            i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight),
            i.modifiers.shift,
        )
    });
    for (hit, m) in [(v, Mode::Select), (l, Mode::Line), (r, Mode::Rect), (e, Mode::Ellipse), (pg, Mode::Polygon), (dp, Mode::Point)] {
        if hit { set_mode(app, m); }
    }
    if app.mode == Mode::Line && !app.poly.is_empty() {
        if enter { finish_poly(app, false); }
        else if c { finish_poly(app, true); }
    }
    if esc {
        if !app.poly.is_empty() { app.poly.clear(); }
        else if !matches!(app.drag, Drag::None) { app.drag = Drag::None; }
        else if app.rotate_mode { app.rotate_mode = false; app.rot_pivot = None; } // leaves rotate handles, keeps the selection
        else if !app.sel.is_empty() { app.sel.clear(); }
        else { app.mode = Mode::Select; }
    }
    if del && app.poly.is_empty() { app.delete_selection(); }
    let step = if app.snap_on { app.snap_mm } else { 1.0 } * if shift { 10.0 } else { 1.0 };
    let (dx, dy) = ((right as i32 - left as i32) as f64 * step, (up as i32 - down as i32) as f64 * step);
    if dx != 0.0 || dy != 0.0 { app.nudge(dx, dy); }
}

pub fn canvas(ui: &mut Ui, app: &mut App) {
    let ctx = ui.ctx().clone();
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
let (sw, sh) = (app.project.data.stock.w, app.project.data.stock.h);
    let zr = app.project.data.stock.zr.clone();
    let zero = (match zr.as_str() { "lr" | "ur" => sw, "c" => sw / 2.0, _ => 0.0 }, match zr.as_str() { "ul" | "ur" => sh, "c" => sh / 2.0, _ => 0.0 });

    if app.fit_pending || (!ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::F))) {
        // frame the stock, plus the machine work area when it is shown
        let (mut bw, mut bh) = (sw, sh);
        if app.show_machine_area { if let Some(m) = &app.project.data.machine { bw = bw.max(m.x); bh = bh.max(m.y); } }
        let sx = rect.width() / (bw as f32 * 1.12);
        let sy = rect.height() / (bh as f32 * 1.12);
        app.scale = sx.min(sy).max(0.01);
        app.offset = Vec2::new(rect.width() / 2.0 - bw as f32 * app.scale / 2.0, rect.height() / 2.0 + bh as f32 * app.scale / 2.0);
        app.fit_pending = false;
    }
    if resp.dragged_by(PointerButton::Middle) || resp.dragged_by(PointerButton::Secondary) { app.offset += resp.drag_delta(); }
    if resp.hovered() {
        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let ns = (app.scale * (scroll * 0.0015).exp()).clamp(0.05, 500.0);
            if let Some(m) = resp.hover_pos() {
                let local = m - rect.min;
                app.offset = local - (local - app.offset) * (ns / app.scale);
            }
            app.scale = ns;
        }
    }
    // the rotate handles belong to one selection; a different selection ends them (Esc is handled with the other keys)
    if app.sel != app.rot_sel { app.rotate_mode = false; app.rot_pivot = None; app.rot_sel = app.sel.clone(); }
    if app.mode != Mode::Select { app.rotate_mode = false; }
    let view = View { scale: app.scale, offset: app.offset, origin: rect.min };
    let pointer_w = ctx.input(|i| i.pointer.latest_pos()).map(|p| view.s2w(p));
    let hover_w = resp.hover_pos().map(|p| view.s2w(p));
    let shift = ctx.input(|i| i.modifiers.shift);
    let tol = 6.0 / app.scale as f64;
    let prim = PointerButton::Primary;
    // object snap: ten pixels of reach, never to the shapes that are being moved or edited
    app.snap_tol_mm = 10.0 / app.scale as f64;
    app.snap_hit.set(None);
    app.snap_skip = match (&app.drag, app.node_edit) {
        (Drag::Move { .. } | Drag::Resize { .. } | Drag::Rotate { .. }, _) => app.sel.iter().copied().collect(),
        (_, Some(li)) => vec![li],
        _ => Vec::new(),
    };

    // ---------------------------------------------------------------- interaction
    // tab placement only makes sense for a manual-tab operation that still exists
    let editing_tabs = app.tab_edit.filter(|id| app.project.data.ops.iter().any(|o| o.id == *id && o.p.tabs && o.p.tab_pts.is_some()));
    if editing_tabs.is_none() { app.tab_edit = None; }
    if editing_tabs.is_some() { app.node_edit = None; }
    let editing_nodes = app.node_edit.filter(|&i| i < app.project.data.loops.len());
    if editing_nodes.is_none() { app.node_edit = None; app.node_drag = NodeDrag::None; }
    let interaction_mode = if editing_tabs.is_some() || editing_nodes.is_some() { None } else { Some(app.mode) };
    if let Some(op_id) = editing_tabs { tab_interaction(&ctx, app, &resp, &view, op_id, hover_w); }
    if let Some(li) = editing_nodes { node_interaction(&ctx, app, &resp, &view, li, hover_w, pointer_w, tol); }
    match interaction_mode {
        None => {}
        Some(Mode::Select) => {
            if resp.drag_started_by(prim) {
                let sel_bb = shapes::bbox_many(app.sel.iter().filter_map(|&i| app.project.data.loops.get(i))).filter(|_| handles_apply(app));
                let locked_sel = app.sel_locked();
                if let (Some(ps), Some(b), false, true) = (ctx.input(|i| i.pointer.press_origin()), sel_bb, locked_sel, app.rotate_mode) {
                    let pivot = app.rot_pivot.unwrap_or([(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0]);
                    let w = view.s2w(ps);
                    if view.w2s(pivot).distance(ps) <= 10.0 {
                        app.drag = Drag::Pivot;
                    } else if let Some(h) = hit_handle(&view, b, ps).filter(|h| h % 2 == 0) {
                        let _ = h;
                        app.drag = Drag::Rotate {
                            pivot, a0: (w[1] - pivot[1]).atan2(w[0] - pivot[0]),
                            orig: app.sel.iter().map(|&i| (i, app.project.data.loops[i].clone())).collect(),
                            tabs0: shapes::tab_positions(&app.project.data.loops, &app.project.data.ops, &app.sel),
                            total: 0.0,
                        };
                    }
                }
                if let (Some(ps), Some(b), false, false) = (ctx.input(|i| i.pointer.press_origin()), sel_bb, locked_sel, app.rotate_mode) {
                    if let Some(h) = hit_handle(&view, b, ps) {
                        let w = view.s2w(ps);
                        let hw = handle_world(b, h);
                        app.drag = Drag::Resize {
                            handle: h, start_bbox: b, grab: [hw[0] - w[0], hw[1] - w[1]],
                            orig: app.sel.iter().map(|&i| (i, app.project.data.loops[i].clone())).collect(),
                        };
                    }
                }
                if let (Some(w), false) = (ctx.input(|i| i.pointer.press_origin()).map(|p| view.s2w(p)), matches!(app.drag, Drag::Resize { .. } | Drag::Rotate { .. } | Drag::Pivot)) {
                    let loops = &app.project.data.loops;
                    let edge = loops.iter().enumerate()
                        .map(|(i, l)| (i, shapes::dist_to_loop(l, w)))
                        .filter(|(_, d)| *d <= tol)
                        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                        .map(|(i, _)| i);
                    let hit = edge.or_else(|| app.sel.iter().copied().find(|&i| shapes::point_in_loop(&loops[i], w)));
                    match hit {
                        Some(h) => {
                            if !app.sel.contains(&h) { if !shift { app.sel.clear(); } app.sel.insert(h); }
                            let b = shapes::bbox_many(app.sel.iter().map(|&i| &app.project.data.loops[i])).unwrap();
                            // a locked drawing is selected but stays where it is
                            app.drag = if app.sel_locked() { Drag::None } else { Drag::Move { start: w, anchor: [b[0], b[1]], applied: [0.0, 0.0] } };
                        }
                        // empty space over the 3D model drags the model (Shift box-selects instead)
                        None => {
                            let over_model = app.project.data.relief.as_ref().filter(|_| !shift).filter(|r| { let (mw, mh) = r.size(); w[0] >= r.x && w[0] <= r.x + mw && w[1] >= r.y && w[1] <= r.y + mh }).map(|r| [r.x, r.y]);
                            app.drag = match over_model { Some(orig) => Drag::Relief { start: w, orig }, None => Drag::Box { start: w } };
                        }
                    }
                }
            }
            if resp.dragged_by(prim) {
                if let (Drag::Resize { handle, start_bbox, grab, orig }, Some(cur)) = (&app.drag, pointer_w) {
                    let alt = ctx.input(|i| i.modifiers.alt);
                    let nb = resized_bbox(&app, *handle, *start_bbox, *grab, cur, shift, alt);
                    let (sb, orig) = (*start_bbox, orig.clone());
                    app.hist.record_once(&app.project.data);
                    for (i, l) in &orig { app.project.data.loops[*i] = l.clone(); }
                    let sel = &app.sel;
                    let mut refs: Vec<&mut putiacnc_core::model::LoopData> =
                        app.project.data.loops.iter_mut().enumerate().filter(|(i, _)| sel.contains(i)).map(|(_, l)| l).collect();
                    shapes::fit_to_bbox(&mut refs, sb, nb);
                    app.touch();
                }
                if let (Drag::Rotate { pivot, a0, orig, tabs0, .. }, Some(cur)) = (&app.drag, pointer_w) {
                    let (pivot, a0, orig, tabs0) = (*pivot, *a0, orig.clone(), tabs0.clone());
                    let mut deg = ((cur[1] - pivot[1]).atan2(cur[0] - pivot[0]) - a0).to_degrees();
                    // keep the turn within -180..180 so it does not jump at the half turn
                    while deg > 180.0 { deg -= 360.0; }
                    while deg < -180.0 { deg += 360.0; }
                    if shift { deg = (deg / 15.0).round() * 15.0; }
                    app.hist.record_once(&app.project.data);
                    for (i, l) in &orig { app.project.data.loops[*i] = l.clone(); }
                    let sel = app.sel.clone();
                    let d = &mut app.project.data;
                    shapes::rotate_selection(&mut d.loops, &mut d.ops, &sel, pivot, deg, &tabs0);
                    app.drag = Drag::Rotate { pivot, a0, orig, tabs0, total: deg };
                    app.touch();
                }
                if let (Drag::Relief { start, orig }, Some(cur)) = (&app.drag, pointer_w) {
                    let t = app.snap([orig[0] + cur[0] - start[0], orig[1] + cur[1] - start[1]]);
                    let moved = app.project.data.relief.as_ref().map_or(false, |r| (r.x - t[0]).abs() > 1e-9 || (r.y - t[1]).abs() > 1e-9);
                    if moved {
                        app.hist.record_once(&app.project.data);
                        if let Some(r) = app.project.data.relief.as_mut() { r.x = t[0]; r.y = t[1]; }
                        app.touch();
                    }
                }
                if let (Drag::Pivot, Some(cur)) = (&app.drag, pointer_w) { app.rot_pivot = Some(app.snap(cur)); }
                if let (Drag::Move { start, anchor, applied }, Some(cur)) = (&app.drag, pointer_w) {
                    let (start, anchor, applied) = (*start, *anchor, *applied);
                    let t = move_target(app, start, anchor, applied, cur);
                    let desired = [t[0] - anchor[0], t[1] - anchor[1]];
                    let step = [desired[0] - applied[0], desired[1] - applied[1]];
                    if step[0].abs() > 1e-9 || step[1].abs() > 1e-9 {
                        app.hist.record_once(&app.project.data);
                        for &i in &app.sel { shapes::translate(&mut app.project.data.loops[i], step[0], step[1]); }
                        if let Some(p) = app.rot_pivot.as_mut() { p[0] += step[0]; p[1] += step[1]; }
                        app.drag = Drag::Move { start, anchor, applied: desired };
                        app.touch();
                    }
                }
            }
            if resp.drag_stopped_by(prim) {
                if let (Drag::Box { start }, Some(end)) = (&app.drag, pointer_w) {
                    let r = [start[0].min(end[0]), start[1].min(end[1]), start[0].max(end[0]), start[1].max(end[1])];
                    if !shift { app.sel.clear(); }
                    for (i, l) in app.project.data.loops.iter().enumerate() {
                        if shapes::inside_box(l, r) { app.sel.insert(i); }
                    }
                    app.expand_text_selection();
                }
                app.drag = Drag::None;
            }
            if resp.double_clicked_by(prim) {
                if let Some(h) = hover_w.and_then(|w| shapes::pick(&app.project.data.loops, w, tol)) {
                    // placed text opens in the Text window; everything else in the node editor
                    let text = app.project.data.loops[h].grp.filter(|g| app.project.data.groups.iter().any(|x| x.id == *g && x.text.is_some()));
                    match text { Some(g) => { app.sel = app.text_siblings(h).into_iter().collect(); app.edit_text(g); } None => enter_node_edit(app, h) }
                }
            }
            if resp.clicked_by(prim) && !resp.double_clicked() {
                if let Some(w) = hover_w {
                    let hit = shapes::pick(&app.project.data.loops, w, tol);
                    match hit {
                        Some(h) => {
                            if shift {
                                let sib = app.text_siblings(h);
                                if app.sel.contains(&h) { for i in sib { app.sel.remove(&i); } } else { app.sel.extend(sib); }
                            }
                            else if app.sel.contains(&h) {
                                // clicking a selected shape again swaps the resize handles for rotate handles and back
                                if handles_apply(app) && !app.sel_locked() {
                                    app.rotate_mode = !app.rotate_mode;
                                    app.rot_pivot = if app.rotate_mode {
                                        shapes::bbox_many(app.sel.iter().filter_map(|&i| app.project.data.loops.get(i))).map(|b| [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0])
                                    } else { None };
                                }
                            }
                            else { app.sel = app.text_siblings(h).into_iter().collect(); }
                        }
                        None => if !shift { app.sel.clear(); },
                    }
                }
            }
        }
        Some(Mode::Node) => {
            // nothing is being edited yet: clicking a shape starts editing its points
            if resp.clicked_by(prim) {
                if let Some(h) = hover_w.and_then(|w| shapes::pick(&app.project.data.loops, w, tol)) { enter_node_edit(app, h); }
            }
            if hover_w.map_or(false, |w| shapes::pick(&app.project.data.loops, w, tol).is_some()) { ctx.set_cursor_icon(egui::CursorIcon::PointingHand); }
        }
        Some(Mode::Trim) => {
            if resp.clicked_by(prim) {
                if let Some(w) = hover_w { trim_click(app, w, 8.0 / app.scale as f64); }
            }
        }
        Some(Mode::Text) => {
            if resp.clicked_by(prim) {
                if let Some(w) = hover_w { app.text.pos = Some(app.snap(w)); app.text.open = true; app.text.focus = true; }
            }
        }
        Some(Mode::Point) => {
            if app.point_ring || app.point_grid {
                // bolt circle: drag from the centre; the first hole follows the pointer. Grid: drag a box.
                if resp.drag_started_by(prim) {
                    if let Some(w) = ctx.input(|i| i.pointer.press_origin()).map(|p| view.s2w(p)) {
                        app.drag = Drag::Draw { start: app.snap(w) };
                    }
                }
                if resp.drag_stopped_by(prim) {
                    if let (Drag::Draw { start }, Some(cur)) = (&app.drag, pointer_w) {
                        let op = app.project.data.act;
                        let v = ring_from_drag(app, *start, app.snap(cur), shift, op);
                        if !v.is_empty() {
                            app.hist.record(&app.project.data);
                            app.project.data.loops.extend(v);
                            app.sel.clear();
                            app.touch();
                        }
                    }
                    app.drag = Drag::None;
                }
            } else if resp.clicked_by(prim) {
                if let Some(w) = hover_w {
                    // points stay in the tool, so a row of holes is a row of clicks
                    let p = app.snap(w);
                    let op = app.project.data.act;
                    app.hist.record(&app.project.data);
                    app.project.data.loops.push(shapes::new_point(p, op));
                    app.sel.clear();
                    app.touch();
                }
            }
        }
        Some(Mode::Polygon) => {
            if resp.drag_started_by(prim) {
                if let Some(w) = ctx.input(|i| i.pointer.press_origin()).map(|p| view.s2w(p)) {
                    app.drag = Drag::Draw { start: app.snap(w) };
                }
            }
            if resp.drag_stopped_by(prim) {
                if let (Drag::Draw { start }, Some(cur)) = (&app.drag, pointer_w) {
                    let op = app.project.data.act;
                    if let Some(l) = poly_from_drag(app, *start, app.snap(cur), shift, op) { app.add_loop(l); }
                }
                app.drag = Drag::None;
            }
        }
        Some(Mode::Rect) | Some(Mode::Ellipse) => {
            if resp.drag_started_by(prim) {
                if let Some(w) = ctx.input(|i| i.pointer.press_origin()).map(|p| view.s2w(p)) {
                    app.drag = Drag::Draw { start: app.snap(w) };
                }
            }
            if resp.drag_stopped_by(prim) {
                if let (Drag::Draw { start }, Some(cur)) = (&app.drag, pointer_w) {
                    let (a, b) = (*start, draw_end(app, *start, app.snap(cur), shift));
                    if app.mode == Mode::Ellipse && app.circle_mode {
                        // a circle: the drag starts at its centre
                        let c = app.snap(cur);
                        let r = (c[0] - a[0]).hypot(c[1] - a[1]);
                        if r > 0.01 { let op = app.project.data.act; app.add_loop(shapes::new_circle(a, r, op)); }
                    } else if (b[0] - a[0]).abs() > 0.01 && (b[1] - a[1]).abs() > 0.01 {
                        let op = app.project.data.act;
                        let l = if app.mode == Mode::Rect { shapes::new_rect(a, b, app.rect_radius, op) } else { shapes::new_ellipse(a, b, op) };
                        app.add_loop(l);
                    }
                }
                app.drag = Drag::None;
            }
        }
        Some(Mode::Line) => {
            if resp.clicked_by(prim) {
                if let Some(w) = hover_w {
                    let p = app.snap(w);
                    let close = app.poly.len() >= 3 && {
                        let f = view.w2s(app.poly[0]);
                        f.distance(view.w2s(p)) < 10.0
                    };
                    if close { finish_poly(app, true); }
                    else {
                        app.poly.push(p);
                        if resp.double_clicked() { finish_poly(app, false); }
                    }
                }
            }
            if resp.clicked_by(PointerButton::Secondary) {
                if app.poly.len() >= 2 { finish_poly(app, false); } else { app.poly.clear(); }
            }
        }
    }

    // typed values: a click on the drawing drops them, a number typed over it opens the panel
    if resp.clicked() || resp.drag_started() { app.dyn_in = None; }
    let typed_at = pointer_w.map(|p| app.snap(p));
    crate::dynin::drive(&ctx, app, resp.hovered(), typed_at, zero, rect);

    if app.mode == Mode::Select && editing_tabs.is_none() && editing_nodes.is_none() {
        if let Drag::Resize { handle, .. } = &app.drag { ctx.set_cursor_icon(handle_cursor(*handle)); }
        else if matches!(app.drag, Drag::Rotate { .. } | Drag::Pivot | Drag::Relief { .. }) { ctx.set_cursor_icon(egui::CursorIcon::Grabbing); }
        else if matches!(app.drag, Drag::None) {
            let sel_bb = shapes::bbox_many(app.sel.iter().filter_map(|&i| app.project.data.loops.get(i))).filter(|_| handles_apply(app));
            if let (Some(ps), Some(b)) = (ctx.input(|i| i.pointer.latest_pos()), sel_bb) {
                if app.rotate_mode {
                    let pivot = app.rot_pivot.unwrap_or([(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0]);
                    if view.w2s(pivot).distance(ps) <= 10.0 || hit_handle(&view, b, ps).map_or(false, |h| h % 2 == 0) { ctx.set_cursor_icon(egui::CursorIcon::Grab); }
                } else if let Some(h) = hit_handle(&view, b, ps) { ctx.set_cursor_icon(handle_cursor(h)); }
            }
        }
    }

    // ---------------------------------------------------------------- painting
    let th = app.theme;
    let dark = th.dark;
    let d = &app.project.data;
    let to_s = |p: [f64; 2]| view.w2s(p);
    if app.grid_on {
        let step_mm = [1.0, 5.0, 10.0, 50.0, 100.0, 500.0].into_iter().find(|s| s * app.scale as f64 >= 14.0).unwrap_or(1000.0);
        let tl = view.s2w(rect.left_top());
        let br = view.s2w(rect.right_bottom());
        let (x0, x1, y0, y1) = (tl[0], br[0], br[1], tl[1]);
        let gs = Stroke::new(1.0_f32, th.grid());
        let mut gx = (x0 / step_mm).floor() * step_mm;
        while gx <= x1 { painter.line_segment([to_s([gx, y0]), to_s([gx, y1])], gs); gx += step_mm; }
        let mut gy = (y0 / step_mm).floor() * step_mm;
        while gy <= y1 { painter.line_segment([to_s([x0, gy]), to_s([x1, gy])], gs); gy += step_mm; }
    }
    // machine work area, anchored at the stock's lower-left corner
    if app.show_machine_area {
        if let Some(m) = &d.machine {
            let (a, b) = (to_s([0.0, 0.0]), to_s([m.x, m.y]));
            let col = if dark { Color32::from_rgb(240, 170, 80) } else { Color32::from_rgb(200, 110, 20) };
            let pts = [a, Pos2::new(b.x, a.y), b, Pos2::new(a.x, b.y), a];
            painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.5_f32, col), 8.0, 5.0));
            painter.text(Pos2::new(b.x - 6.0, b.y + 4.0), egui::Align2::RIGHT_TOP,
                format!("{}: {} × {} mm", m.name, m.x, m.y), egui::TextStyle::Small.resolve(ui.style()), col);
        }
    }
    let sr = Rect::from_two_pos(to_s([0.0, 0.0]), to_s([sw, sh]));
    let mat = th.material(&d.stock.mat);
    painter.rect_filled(sr, 0.0, mat.fill);
    painter.rect_stroke(sr, 0.0, Stroke::new(1.5_f32, mat.stroke));
    // the imported 3D model, as a shaded height map on the stock
    if let Some(rel) = &d.relief {
        let key = format!("{}|{}|{}|{}|{}", rel.name, rel.nx, rel.ny, rel.hm.len(), rel.hm.get(..48).unwrap_or("") );
        if app.relief_tex.as_ref().map_or(true, |(k, _)| *k != key) {
            let h = rel.heights();
            let step = (rel.nx.max(rel.ny) as f32 / 256.0).ceil().max(1.0) as usize;
            let (w, hh) = (rel.nx / step, rel.ny / step);
            let mut px = vec![Color32::TRANSPARENT; w * hh];
            for y in 0..hh { for x in 0..w {
                let mut m = 0f32;
                for dy in 0..step { for dx in 0..step { m = m.max(h[(y * step + dy) * rel.nx + x * step + dx]); } }
                let v = (55.0 + 185.0 * m) as u8;
                // image rows run top to bottom, the height map bottom to top
                px[(hh - 1 - y) * w + x] = Color32::from_rgba_unmultiplied(v, v, v, 215);
            } }
            let img = egui::ColorImage { size: [w, hh], pixels: px };
            app.relief_tex = Some((key, ctx.load_texture("relief", img, egui::TextureOptions::LINEAR)));
        }
        let (mw, mh) = rel.size();
        let r = Rect::from_two_pos(to_s([rel.x, rel.y]), to_s([rel.x + mw, rel.y + mh]));
        if let Some((_, tex)) = &app.relief_tex {
            painter.image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, mat.stroke));
    }
    let (zx, zy) = match d.stock.zr.as_str() { "lr" => (sw, 0.0), "ul" => (0.0, sh), "ur" => (sw, sh), "c" => (sw / 2.0, sh / 2.0), _ => (0.0, 0.0) };
    let z = to_s([zx, zy]);
    painter.line_segment([z - Vec2::new(8.0, 0.0), z + Vec2::new(8.0, 0.0)], Stroke::new(2.0_f32, Color32::RED));
    painter.line_segment([z - Vec2::new(0.0, 8.0), z + Vec2::new(0.0, 8.0)], Stroke::new(2.0_f32, Color32::GREEN));

    let act_idx = d.ops.iter().position(|o| o.id == d.act);
    for (i, l) in d.loops.iter().enumerate() {
        let opi = d.ops.iter().position(|o| o.id == l.op);
        let selected = app.sel.contains(&i);
        let col = if selected { th.shape_selected() } else { opi.map(|k| op_color(k, dark)).unwrap_or(th.shape_idle()) };
        if shapes::is_point(l) {
            if let Some(b) = shapes::bbox(l) {
                let s = to_s([(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0]);
                let st = Stroke::new(if selected { 2.5_f32 } else { 1.8_f32 }, col);
                painter.circle_stroke(s, 4.0, st);
                painter.line_segment([s - Vec2::new(8.0, 0.0), s + Vec2::new(8.0, 0.0)], st);
                painter.line_segment([s - Vec2::new(0.0, 8.0), s + Vec2::new(0.0, 8.0)], st);
            }
            continue;
        }
        let mut pts: Vec<Pos2> = l.pts.chunks_exact(2).map(|c| to_s([c[0] as f64 / shapes::S, c[1] as f64 / shapes::S])).collect();
        if l.closed() { if let Some(f) = pts.first().copied() { pts.push(f); } }
        painter.add(egui::Shape::line(pts, Stroke::new(if editing_nodes == Some(i) { 3.0_f32 } else if selected { 2.5_f32 } else { 1.8_f32 }, if editing_nodes == Some(i) { th.accent } else { col })));
    }
    if let Some(b) = shapes::bbox_many(app.sel.iter().filter_map(|&i| d.loops.get(i))).filter(|_| editing_nodes.is_none()) {
        let r = Rect::from_two_pos(to_s([b[0], b[1]]), to_s([b[2], b[3]])).expand(4.0);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, th.shape_selected().gamma_multiply(0.3)));
        if app.mode == Mode::Select && handles_apply(app) && app.rotate_mode {
            // rotate handles: a round handle with a turning arrow on each corner, and the centre of rotation
            let acc = Stroke::new(1.5_f32, th.accent);
            for h in [0usize, 2, 4, 6] {
                let c = handle_screen(&view, b, h);
                painter.circle_filled(c, 6.5, th.view());
                painter.circle_stroke(c, 6.5, acc);
                // an arc of about 270 degrees with an arrow head
                let pts: Vec<Pos2> = (0..=14).map(|k| { let t = 0.6 + k as f32 * 0.32; c + Vec2::new(t.cos(), -t.sin()) * 3.2 }).collect();
                painter.add(egui::Shape::line(pts.clone(), acc));
                if let (Some(&e), Some(&p)) = (pts.last(), pts.get(pts.len() - 2)) {
                    let dir = (e - p).normalized();
                    let n = Vec2::new(-dir.y, dir.x);
                    painter.add(egui::Shape::convex_polygon(vec![e + dir * 2.6, e - dir * 0.6 + n * 2.2, e - dir * 0.6 - n * 2.2], th.accent, Stroke::NONE));
                }
            }
            let pv = view.w2s(app.rot_pivot.unwrap_or([(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0]));
            painter.circle_filled(pv, 5.0, th.view());
            painter.circle_stroke(pv, 5.0, acc);
            painter.line_segment([pv - Vec2::new(8.0, 0.0), pv + Vec2::new(8.0, 0.0)], acc);
            painter.line_segment([pv - Vec2::new(0.0, 8.0), pv + Vec2::new(0.0, 8.0)], acc);
        } else if app.mode == Mode::Select && handles_apply(app) {
            for h in 0..8 {
                let hr = Rect::from_center_size(handle_screen(&view, b, h), Vec2::splat(9.0));
                painter.rect_filled(hr, 2.0, th.view());
                painter.rect_stroke(hr, 2.0, Stroke::new(1.5_f32, th.accent));
            }
        }
    }

    // points of the shape being edited
    if let Some(li) = editing_nodes {
        let pts = shapes::points(&d.loops[li]);
        let hov = hover_w.and_then(|w| node_at(app, &pts, w));
        if let NodeDrag::Box { start } = &app.node_drag {
            if let Some(cur) = pointer_w {
                let r = Rect::from_two_pos(to_s(*start), to_s(cur));
                painter.rect_filled(r, 0.0, th.accent.gamma_multiply(0.12));
                painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, th.accent));
            }
        }
        // where a click would add a point
        if hov.is_none() && matches!(app.node_drag, NodeDrag::None) {
            if let Some(w) = hover_w {
                let mut probe = d.loops[li].clone();
                if let Some(i) = shapes::insert_point(&mut probe, w, tol) { let q = shapes::points(&probe)[i]; painter.circle(to_s(q), 4.5, th.accent.gamma_multiply(0.35), Stroke::new(1.0_f32, th.accent)); }
            }
        }
        // very dense shapes (a flattened circle): small dots instead of handles that would merge into a blob
        let per: f32 = pts.windows(2).map(|w| to_s(w[0]).distance(to_s(w[1]))).sum();
        let dense = per / (pts.len().max(1) as f32) < 8.0;
        for (k, p) in pts.iter().enumerate() {
            let s = to_s(*p);
            let (sel, hot) = (app.node_sel.contains(&k), hov == Some(k));
            if dense && !sel && !hot { painter.circle_filled(s, 1.6, th.accent); continue; }
            let r = Rect::from_center_size(s, Vec2::splat(if hot { 9.0 } else { 7.0 }));
            painter.rect_filled(r, 1.5, if sel { th.accent } else { Color32::WHITE });
            painter.rect_stroke(r, 1.5, Stroke::new(1.5_f32, th.accent));
        }
    }

    // holding tabs (automatic and manual)
    {
        let hov_tab = editing_tabs.and_then(|id| hover_w.and_then(|w| hovered_tab(app, id, w)));
        for (k, o) in d.ops.iter().enumerate() {
            if !o.p.tabs || !["outside", "inside", "online"].contains(&o.p.kind.as_str()) { continue; }
            // manual tabs follow their shapes; automatic ones sit at their evenly spaced positions
            let items: Vec<Option<([f64; 2], [f64; 2])>> = match o.p.tab_pts.as_ref() {
                Some(v) => v.iter().map(|t| shapes::tab_resolve(&d.loops, o.id, t)).collect(),
                None => {
                    // where the generated toolpath really leaves the tabs; an estimate while it is being regenerated
                    let spots: Vec<[f64; 2]> = match app.output.as_ref().filter(|_| !app.gen_pending) {
                        Some(out) => out.tab_marks.iter().filter(|m| m.op == k).map(|m| m.mid).collect(),
                        None => shapes::auto_tab_points(&d.loops, o.id, o.p.tab_n.unwrap_or(4.0).round().max(1.0) as usize),
                    };
                    spots.into_iter().map(|t| {
                        d.loops.iter().filter(|l| l.op == o.id).filter_map(|l| shapes::nearest_on_loop(l, t)).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|n| (n.0, n.2))
                    }).collect()
                }
            };
            let editing = editing_tabs == Some(o.id);
            let col = op_color(k, dark);
            let half = o.p.tab_w.unwrap_or(5.0) / 2.0;
            for (ti, item) in items.iter().enumerate() {
                let Some((t, tang)) = item else { continue };
                let (a, b) = (to_s([t[0] - tang[0] * half, t[1] - tang[1] * half]), to_s([t[0] + tang[0] * half, t[1] + tang[1] * half]));
                let hot = editing && hov_tab == Some(ti);
                painter.line_segment([a, b], Stroke::new(if editing { 10.0_f32 } else { 9.0 }, Color32::WHITE.gamma_multiply(0.95)));
                painter.line_segment([a, b], Stroke::new(if editing { 6.0_f32 } else { 5.5 }, if hot { th.accent } else { col }));
                if editing { painter.circle(to_s(*t), 4.0, Color32::WHITE, Stroke::new(1.5_f32, if hot { th.accent } else { col })); }
            }
        }
        if let (Some(id), Some(w)) = (editing_tabs, hover_w) {
            if hov_tab.is_none() && app.tab_drag.is_none() {
                if let Some((_, q, dist, tang)) = nearest_outline(app, id, w) {
                    if dist * app.scale as f64 <= TAB_REACH_PX {
                        let half = d.ops.iter().find(|o| o.id == id).and_then(|o| o.p.tab_w).unwrap_or(5.0) / 2.0;
                        let (a, b) = (to_s([q[0] - tang[0] * half, q[1] - tang[1] * half]), to_s([q[0] + tang[0] * half, q[1] + tang[1] * half]));
                        painter.line_segment([a, b], Stroke::new(6.0_f32, th.accent.gamma_multiply(0.45)));
                    }
                }
            }
        }
    }

    if app.show_toolpath && !app.gen_pending {
        if let Some(o) = &app.output {
            for m in &o.moves {
                if !m.cut && !app.show_rapids { continue; }
                if m.from[0] == m.to[0] && m.from[1] == m.to[1] { continue; }
                let col = if m.cut { op_color(m.op, dark).gamma_multiply(0.85) } else { th.rapid() };
                painter.line_segment([to_s([m.from[0], m.from[1]]), to_s([m.to[0], m.to[1]])], Stroke::new(1.0_f32, col));
            }
        }
    }

    // previews
    let prev_col = act_idx.map(|k| op_color(k, dark)).unwrap_or(th.accent);
    let mut hud: Option<String> = None;
    if let Drag::Rotate { total, .. } = &app.drag { hud = Some(format!("{:.1}°", total)); }
    if let (Drag::Draw { start }, Some(cur)) = (&app.drag, pointer_w) {
        let a = *start;
        let b = draw_end(app, a, app.snap(cur), shift);
        if app.mode == Mode::Point {
            let v = ring_from_drag(app, a, app.snap(cur), shift, 0);
            for l in &v {
                if let Some(bb) = shapes::bbox(l) {
                    let s = to_s([(bb[0] + bb[2]) / 2.0, (bb[1] + bb[3]) / 2.0]);
                    let st = Stroke::new(2.0_f32, prev_col);
                    painter.circle_stroke(s, 4.0, st);
                    painter.line_segment([s - Vec2::new(8.0, 0.0), s + Vec2::new(8.0, 0.0)], st);
                    painter.line_segment([s - Vec2::new(0.0, 8.0), s + Vec2::new(0.0, 8.0)], st);
                }
            }
            if app.point_grid {
                painter.rect_stroke(Rect::from_two_pos(to_s(a), to_s(app.snap(cur))), 0.0, Stroke::new(1.0_f32, prev_col.gamma_multiply(0.4)));
                let to = app.snap(cur);
                hud = Some(format!("{} holes, {:.2} x {:.2} mm", v.len(), (to[0] - a[0]).abs(), (to[1] - a[1]).abs()));
            } else {
                painter.circle_stroke(to_s(a), ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt() as f32 * app.scale, Stroke::new(1.0_f32, prev_col.gamma_multiply(0.4)));
                hud = Some(format!("{} holes, radius {:.2} mm", v.len(), ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()));
            }
        } else if app.mode == Mode::Polygon {
            if let Some(l) = poly_from_drag(app, a, app.snap(cur), shift, 0) {
                let mut pts: Vec<Pos2> = l.pts.chunks_exact(2).map(|c| to_s([c[0] as f64 / shapes::S, c[1] as f64 / shapes::S])).collect();
                if let Some(f) = pts.first().copied() { pts.push(f); }
                painter.add(egui::Shape::line(pts, Stroke::new(2.0_f32, prev_col)));
                hud = Some(format!("radius {:.2} mm", ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()));
            }
        } else {
        let circle = app.mode == Mode::Ellipse && app.circle_mode;
        let cr = { let e = app.snap(cur); (e[0] - a[0]).hypot(e[1] - a[1]) };
        let l = if circle { shapes::new_circle(a, cr.max(1e-6), 0) } else if app.mode == Mode::Rect { shapes::new_rect(a, b, app.rect_radius, 0) } else { shapes::new_ellipse(a, b, 0) };
        let mut pts: Vec<Pos2> = l.pts.chunks_exact(2).map(|c| to_s([c[0] as f64 / shapes::S, c[1] as f64 / shapes::S])).collect();
        if let Some(f) = pts.first().copied() { pts.push(f); }
        painter.add(egui::Shape::line(pts, Stroke::new(2.0_f32, prev_col)));
        hud = Some(if circle {
            if app.circle_diam { format!("diameter {:.2} mm", cr * 2.0) } else { format!("radius {:.2} mm", cr) }
        } else { format!("{:.2} x {:.2} mm", (b[0] - a[0]).abs(), (b[1] - a[1]).abs()) });
        if circle { let s = to_s(a); let st = Stroke::new(1.0_f32, prev_col); painter.line_segment([s - Vec2::new(6.0, 0.0), s + Vec2::new(6.0, 0.0)], st); painter.line_segment([s - Vec2::new(0.0, 6.0), s + Vec2::new(0.0, 6.0)], st); }
        }
    }
    if app.text.open {
        for l in &app.text.preview {
            let mut pts: Vec<Pos2> = l.pts.chunks_exact(2).map(|c| to_s([c[0] as f64 / shapes::S, c[1] as f64 / shapes::S])).collect();
            if let Some(f) = pts.first().copied() { pts.push(f); }
            painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, prev_col)));
        }
        if let Some(p) = app.text.pos {
            let c = to_s(p);
            painter.line_segment([c - Vec2::new(6.0, 0.0), c + Vec2::new(6.0, 0.0)], Stroke::new(1.0_f32, prev_col));
            painter.line_segment([c - Vec2::new(0.0, 6.0), c + Vec2::new(0.0, 6.0)], Stroke::new(1.0_f32, prev_col));
        }
    }
    if let Drag::Box { start } = &app.drag {
        if let Some(cur) = pointer_w {
            let r = Rect::from_two_pos(to_s(*start), to_s(cur));
            painter.rect_filled(r, 0.0, th.accent.gamma_multiply(0.15));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, th.accent));
        }
    }
    if let Some(p) = pointer_w {
        if let Some(all) = crate::dynin::preview(app, app.snap(p), zero) {
            for v in all { painter.add(egui::Shape::line(v.iter().map(|q| to_s(*q)).collect(), Stroke::new(2.0_f32, prev_col))); }
        }
    }
    if app.mode == Mode::Line && !app.poly.is_empty() {
        let mut pts: Vec<Pos2> = app.poly.iter().map(|p| to_s(*p)).collect();
        if let (Some(h), true) = (hover_w, app.dyn_in.is_none()) {
            let c = app.snap(h);
            let last = *app.poly.last().unwrap();
            hud = Some(format!("{:.2} mm", (c[0] - last[0]).hypot(c[1] - last[1])));
            pts.push(to_s(c));
        }
        painter.add(egui::Shape::line(pts, Stroke::new(2.0_f32, prev_col)));
        for p in &app.poly { painter.circle_filled(to_s(*p), 3.0, prev_col); }
    }
    // loose ends of open contours: red where nothing meets them, amber where another end is within the cleanup tolerance
    if app.show_open_ends || app.clean.open {
        let ends = putiacnc_core::cleanup::loose_ends(&app.project.data.loops, app.clean.tol);
        for (_, _, p, joinable) in ends.into_iter().take(4000) {
            let col = if joinable { Color32::from_rgb(235, 150, 20) } else { Color32::from_rgb(220, 50, 47) };
            painter.circle_stroke(to_s(p), 7.0, Stroke::new(2.0_f32, col));
        }
    }
    // the snap marker: what the pointer (or the dragged shape) is locked onto
    let marker = match &app.drag {
        Drag::Move { .. } => app.snap_hit.get(),
        Drag::Resize { .. } => hover_w.and_then(|h| app.object_snap(h)),
        Drag::None | Drag::Draw { .. } if !matches!(app.mode, Mode::Select) || app.node_edit.is_some() => hover_w.and_then(|h| app.object_snap(h)),
        _ => None,
    };
    if let Some(m) = marker {
        let c = to_s(m.p);
        let st = Stroke::new(2.0_f32, th.shape_selected());
        match m.kind {
            crate::osnap::Kind::Corner | crate::osnap::Kind::Stock => { painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(11.0)), 0.0, st); }
            crate::osnap::Kind::Midpoint => { painter.add(egui::Shape::closed_line(vec![c + Vec2::new(0.0, -6.0), c + Vec2::new(6.0, 5.0), c + Vec2::new(-6.0, 5.0)], st)); }
            crate::osnap::Kind::Centre => { painter.circle_stroke(c, 6.0, st); painter.circle_filled(c, 1.5, th.shape_selected()); }
            crate::osnap::Kind::Crossing => {
                painter.line_segment([c + Vec2::new(-6.0, -6.0), c + Vec2::new(6.0, 6.0)], st);
                painter.line_segment([c + Vec2::new(-6.0, 6.0), c + Vec2::new(6.0, -6.0)], st);
            }
        }
        painter.text(c + Vec2::new(10.0, 10.0), egui::Align2::LEFT_TOP, m.kind.label(), egui::FontId::proportional(11.0), th.shape_selected());
    }
    if app.mode == Mode::Trim {
        if let Some(w) = hover_w {
            match trim_hover(app, w, 8.0 / app.scale as f64) {
                Some(TrimHover::Trim(_, plan)) => {
                    painter.add(egui::Shape::line(plan.remove.iter().map(|q| to_s(*q)).collect(), Stroke::new(4.0_f32, Color32::from_rgb(220, 50, 47))));
                }
                Some(TrimHover::Extend(_, plan)) => {
                    painter.line_segment([to_s(plan.from), to_s(plan.to)], Stroke::new(3.0_f32, Color32::from_rgb(40, 160, 90)));
                    painter.circle_filled(to_s(plan.to), 4.0, Color32::from_rgb(40, 160, 90));
                }
                Some(TrimHover::Nothing) => {
                    let m = if app.trim_extend { "nothing to run on to" } else { "nothing crosses here" };
                    painter.text(to_s(w) + Vec2::new(12.0, 14.0), egui::Align2::LEFT_TOP, m, egui::FontId::proportional(12.0), th.hud());
                }
                None => {}
            }
        }
    }
    if let Some(h) = hover_w {
        let c = app.snap(h);
        if app.mode != Mode::Select && app.mode != Mode::Node && marker.is_none() { painter.circle_stroke(to_s(c), 5.0, Stroke::new(1.0_f32, th.shape_selected())); }
        if let Some(t) = hud {
            painter.text(to_s(c) + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, t, egui::FontId::monospace(12.0), th.hud());
        }
        painter.text(rect.right_bottom() + Vec2::new(-10.0, -6.0), egui::Align2::RIGHT_BOTTOM,
            format!("X {:.2}  Y {:.2} mm", c[0] - zero.0, c[1] - zero.1), egui::FontId::monospace(12.0), th.hud().gamma_multiply(0.6));
    }
    if app.show_axes { draw_axes(&painter, &view, app.scale, (sw, sh), zero, th.dark, ui); }
    let mut orect = rect;
    if app.show_rulers {
        let sel_bb = shapes::bbox_many(app.sel.iter().filter_map(|&i| app.project.data.loops.get(i)));
        draw_rulers(&painter, rect, &view, app.scale, zero, hover_w, sel_bb, th.accent, ui);
        orect = Rect::from_min_max(rect.min + Vec2::splat(RULER), rect.max);
    }
    if !app.tools_open && !app.machines_open { overlays(&ctx, app, orect); rotate_popup(&ctx, app, rect); }
}

/// In rotate mode: a small field above the selection to turn it by a typed angle (counter-clockwise is positive).
/// While a corner is being dragged it shows the angle turned so far.
fn rotate_popup(ctx: &egui::Context, app: &mut App, rect: Rect) {
    if !app.rotate_mode || app.mode != Mode::Select || app.sel.is_empty() || app.sel_locked() { return; }
    let Some(b) = shapes::bbox_many(app.sel.iter().filter_map(|&i| app.project.data.loops.get(i))) else { return };
    let view = View { scale: app.scale, offset: app.offset, origin: rect.min };
    let top = view.w2s([(b[0] + b[2]) / 2.0, b[3]]);
    let live = if let Drag::Rotate { total, .. } = &app.drag { Some(*total) } else { None };
    let mut v = live.unwrap_or(0.0);
    let mut delta: Option<f64> = None;
    let pos = Pos2::new((top.x - 62.0).clamp(rect.min.x + RULER + 4.0, (rect.max.x - 130.0).max(rect.min.x + RULER + 4.0)), (top.y - 52.0).max(rect.min.y + RULER + 4.0));
    egui::Area::new(egui::Id::new("rotate_popup")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label("Rotate");
                ui.add_space(4.0); // as far from its field as the labels in the selection bar
                let r = crate::widgets::num(ui, [54.0, 24.0], &mut v, -360.0, 360.0, 0.5, 1, "")
                    .on_hover_text("Turn the selection by this many degrees about the centre mark (counter-clockwise is positive). Drag a corner handle to turn it with the mouse; Shift snaps to 15°.");
                if r.changed() && live.is_none() && v != 0.0 { delta = Some(v); }
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
    }
}

const RULER: f32 = 20.0;

/// A tick spacing (mm) that keeps labels at least ~70 px apart, and how many minor ticks sit between two labels.
fn tick_step(scale: f32) -> (f64, usize) {
    for (s, minor) in [(0.01, 5), (0.02, 4), (0.05, 5), (0.1, 5), (0.2, 4), (0.5, 5), (1.0, 5), (2.0, 4), (5.0, 5), (10.0, 5), (20.0, 4), (50.0, 5), (100.0, 5), (200.0, 4), (500.0, 5)] {
        if s * scale as f64 >= 70.0 { return (s, minor); }
    }
    (1000.0, 5)
}

fn tick_label(v: f64, step: f64) -> String {
    let dec = if step >= 1.0 { 0 } else if step >= 0.1 { 1 } else { 2 };
    let s = format!("{:.*}", dec, v);
    if s == "-0" || s == "-0.0" || s == "-0.00" { s[1..].to_string() } else { s }
}

/// Rulers along the top and left edges of the drawing area: millimetres in the same coordinates as the X / Y
/// fields, the pointer position marked on both, and the selection's extent shaded.
fn draw_rulers(painter: &egui::Painter, rect: Rect, view: &View, scale: f32, (zx, zy): (f64, f64), hover: Option<[f64; 2]>, sel: Option<[f64; 4]>, accent: Color32, ui: &Ui) {
    let vis = ui.visuals();
    let (bg, ink, line) = (vis.panel_fill, vis.text_color().gamma_multiply(0.75), vis.widgets.noninteractive.bg_stroke.color);
    let font = egui::FontId::monospace(9.5);
    let top = Rect::from_min_max(rect.min + Vec2::new(RULER, 0.0), Pos2::new(rect.max.x, rect.min.y + RULER));
    let left = Rect::from_min_max(rect.min + Vec2::new(0.0, RULER), Pos2::new(rect.min.x + RULER, rect.max.y));
    painter.rect_filled(top, 0.0, bg);
    painter.rect_filled(left, 0.0, bg);
    painter.rect_filled(Rect::from_min_size(rect.min, Vec2::splat(RULER)), 0.0, bg);
    let (step, minor) = tick_step(scale);
    let sub = step / minor as f64;
    let st = Stroke::new(1.0_f32, ink);
    let wtl = view.s2w(rect.left_top());
    let wbr = view.s2w(rect.right_bottom());
    // selection extent
    if let Some(b) = sel {
        let (a, c) = (view.w2s([b[0], b[1]]), view.w2s([b[2], b[3]]));
        let fill = accent.gamma_multiply(0.28);
        painter.rect_filled(Rect::from_min_max(Pos2::new(a.x.max(top.min.x), top.min.y), Pos2::new(c.x.min(top.max.x).max(a.x.max(top.min.x)), top.max.y)), 0.0, fill);
        painter.rect_filled(Rect::from_min_max(Pos2::new(left.min.x, c.y.max(left.min.y)), Pos2::new(left.max.x, a.y.min(left.max.y).max(c.y.max(left.min.y)))), 0.0, fill);
    }
    // horizontal ruler
    let top_p = painter.with_clip_rect(top);
    let mut i = ((wtl[0] - zx) / sub).floor() as i64 - 1;
    while (i as f64) * sub <= wbr[0] - zx + sub {
        let v = i as f64 * sub;
        let x = view.w2s([v + zx, 0.0]).x;
        let major = i.rem_euclid(minor as i64) == 0;
        let h = if major { RULER * 0.7 } else if i.rem_euclid(2) == 0 { RULER * 0.38 } else { RULER * 0.28 };
        top_p.line_segment([Pos2::new(x, top.max.y - h), Pos2::new(x, top.max.y)], st);
        if major { top_p.text(Pos2::new(x + 3.0, top.min.y + 1.0), egui::Align2::LEFT_TOP, tick_label(v, step), font.clone(), ink); }
        i += 1;
    }
    // vertical ruler
    let left_p = painter.with_clip_rect(left);
    let mut j = ((wbr[1] - zy) / sub).floor() as i64 - 1;
    while (j as f64) * sub <= wtl[1] - zy + sub {
        let v = j as f64 * sub;
        let y = view.w2s([0.0, v + zy]).y;
        let major = j.rem_euclid(minor as i64) == 0;
        let h = if major { RULER * 0.7 } else if j.rem_euclid(2) == 0 { RULER * 0.38 } else { RULER * 0.28 };
        left_p.line_segment([Pos2::new(left.max.x - h, y), Pos2::new(left.max.x, y)], st);
        if major {
            let g = painter.layout_no_wrap(tick_label(v, step), font.clone(), ink);
            let mut ts = egui::epaint::TextShape::new(Pos2::new(left.min.x + 1.0, y - 3.0), g, ink);
            ts.angle = -std::f32::consts::FRAC_PI_2;
            left_p.add(ts);
        }
        j += 1;
    }
    // pointer position
    if let Some(h) = hover {
        let p = view.w2s(h);
        let ms = Stroke::new(1.0_f32, accent);
        if p.x >= top.min.x && p.x <= top.max.x { top_p.line_segment([Pos2::new(p.x, top.min.y), Pos2::new(p.x, top.max.y)], ms); }
        if p.y >= left.min.y && p.y <= left.max.y { left_p.line_segment([Pos2::new(left.min.x, p.y), Pos2::new(left.max.x, p.y)], ms); }
    }
    let ls = Stroke::new(1.0_f32, line);
    painter.line_segment([Pos2::new(rect.min.x, top.max.y), Pos2::new(rect.max.x, top.max.y)], ls);
    painter.line_segment([Pos2::new(left.max.x, rect.min.y), Pos2::new(left.max.x, rect.max.y)], ls);
    painter.text(rect.min + Vec2::new(RULER / 2.0, RULER / 2.0), egui::Align2::CENTER_CENTER, "mm", egui::FontId::monospace(8.0), ink);
}

/// Graduated X and Y axes along the stock's lower and left edges, with the numbers outside the stock.
fn draw_axes(painter: &egui::Painter, view: &View, scale: f32, (sw, sh): (f64, f64), (zx, zy): (f64, f64), dark: bool, ui: &Ui) {
    let col = if dark { Color32::from_rgb(150, 200, 255) } else { Color32::from_rgb(30, 90, 170) };
    let font = egui::FontId::monospace(10.0);
    let (step, minor) = tick_step(scale);
    let sub = step / minor as f64;
    let st = Stroke::new(1.0_f32, col);
    for i in ((-zx) / sub).ceil() as i64..=((sw - zx) / sub).floor() as i64 {
        let v = i as f64 * sub;
        let p = view.w2s([v + zx, 0.0]);
        let major = i % minor as i64 == 0;
        painter.line_segment([p, p + Vec2::new(0.0, if major { 8.0 } else { 4.0 })], st);
        if major { painter.text(p + Vec2::new(0.0, 9.0), egui::Align2::CENTER_TOP, tick_label(v, step), font.clone(), col); }
    }
    for j in ((-zy) / sub).ceil() as i64..=((sh - zy) / sub).floor() as i64 {
        let v = j as f64 * sub;
        let p = view.w2s([0.0, v + zy]);
        let major = j % minor as i64 == 0;
        painter.line_segment([p, p - Vec2::new(if major { 8.0 } else { 4.0 }, 0.0)], st);
        if major { painter.text(p - Vec2::new(9.0, 0.0), egui::Align2::RIGHT_CENTER, tick_label(v, step), font.clone(), col); }
    }
    let (o, xe, ye) = (view.w2s([0.0, 0.0]), view.w2s([sw, 0.0]), view.w2s([0.0, sh]));
    painter.text(xe + Vec2::new(4.0, 0.0), egui::Align2::LEFT_CENTER, "X", egui::FontId::proportional(13.0), col);
    painter.text(ye - Vec2::new(0.0, 4.0), egui::Align2::CENTER_BOTTOM, "Y", egui::FontId::proportional(13.0), col);
    let _ = (o, ui);
}

/// The polygon or star dragged out from its centre: the first corner follows the pointer, Shift points it straight up.
fn poly_from_drag(app: &App, centre: [f64; 2], to: [f64; 2], shift: bool, op: i64) -> Option<putiacnc_core::model::LoopData> {
    let (dx, dy) = (to[0] - centre[0], to[1] - centre[1]);
    let r = (dx * dx + dy * dy).sqrt();
    let angle = if shift { std::f64::consts::FRAC_PI_2 } else { dy.atan2(dx) };
    let inner = if app.poly_star { Some(app.poly_inner / 100.0) } else { None };
    shapes::new_polygon(centre, r, angle, app.poly_sides.round() as usize, inner, op)
}

/// The ring of drill points dragged out from its centre (first hole at the pointer, Shift = straight up).
fn ring_from_drag(app: &App, centre: [f64; 2], to: [f64; 2], shift: bool, op: i64) -> Vec<putiacnc_core::model::LoopData> {
    if app.point_grid { return shapes::point_grid(centre, to, app.grid_cols.round() as usize, app.grid_rows.round() as usize, op); }
    let (dx, dy) = (to[0] - centre[0], to[1] - centre[1]);
    let r = (dx * dx + dy * dy).sqrt();
    let angle = if shift { std::f64::consts::FRAC_PI_2 } else { dy.atan2(dx) };
    shapes::bolt_circle(centre, r, angle, app.point_count.round() as usize, app.point_sweep, op)
}

/// Second corner of a rectangle/ellipse drag; Shift forces a square (circle).
fn draw_end(app: &App, a: [f64; 2], b: [f64; 2], shift: bool) -> [f64; 2] {
    if (app.mode == Mode::Ellipse || app.mode == Mode::Rect) && shift {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let m = dx.abs().max(dy.abs());
        [a[0] + m * if dx < 0.0 { -1.0 } else { 1.0 }, a[1] + m * if dy < 0.0 { -1.0 } else { 1.0 }]
    } else { b }
}

/// Options of the active drawing tool, in a second bar under the palette so the palette itself stays compact.
/// Where the selection's lower-left corner goes while it is dragged: the key points of its box
/// (corners, edge middles, centre) jump to a nearby snap point, otherwise the corner follows the grid.
fn move_target(app: &App, start: [f64; 2], anchor: [f64; 2], applied: [f64; 2], cur: [f64; 2]) -> [f64; 2] {
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
                    if best.map_or(true, |(_, bd, _)| d < bd) { best = Some((c, d, h)); }
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

enum TrimHover { Trim(usize, putiacnc_core::trim::TrimPlan), Extend(usize, putiacnc_core::trim::ExtendPlan), Nothing }

/// What a click at `w` would trim or extend: the shape nearest the pointer (open ones only when extending).
fn trim_hover(app: &App, w: [f64; 2], tol: f64) -> Option<TrimHover> {
    let loops = &app.project.data.loops;
    let t = loops.iter().enumerate()
        .filter(|(_, l)| !shapes::is_point(l) && (!app.trim_extend || !l.closed()))
        .map(|(i, l)| (i, shapes::dist_to_loop(l, w)))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(i, _)| i)?;
    Some(if app.trim_extend {
        match putiacnc_core::trim::plan_extend(loops, t, w) { Some(p) => TrimHover::Extend(t, p), None => TrimHover::Nothing }
    } else {
        match putiacnc_core::trim::plan_trim(loops, t, w) { Some(p) => TrimHover::Trim(t, p), None => TrimHover::Nothing }
    })
}

/// After a trim, the pieces whose ends meet (the cut points of two shapes trimmed against each other)
/// become one path, closed when it comes round, and the result is selected, so what you see is what you get.
fn join_after_trim(app: &mut App, t: usize, n_before: usize) {
    let n = app.project.data.loops.len();
    // the pieces made now: the one in the shape's place (if the shape is still there) and the appended ones
    let mut made: std::collections::BTreeSet<usize> = (n_before..n).collect();
    if n >= n_before { made.insert(t); }
    made.retain(|&i| i < n);
    let tol = 0.01;
    let loops = &app.project.data.loops;
    let ends = |l: &putiacnc_core::model::LoopData| -> Vec<[f64; 2]> {
        if l.closed() || l.pts.len() < 4 || l.pt.is_some() { return Vec::new(); }
        let k = l.pts.len();
        vec![[l.pts[0] as f64 / shapes::S, l.pts[1] as f64 / shapes::S], [l.pts[k - 2] as f64 / shapes::S, l.pts[k - 1] as f64 / shapes::S]]
    };
    let mine: Vec<[f64; 2]> = made.iter().flat_map(|&i| ends(&loops[i])).collect();
    let mut scope = made.clone();
    for (j, l) in loops.iter().enumerate() {
        if made.contains(&j) { continue; }
        if ends(l).iter().any(|e| mine.iter().any(|m| (m[0] - e[0]).hypot(m[1] - e[1]) <= tol)) { scope.insert(j); }
    }
    let opts = putiacnc_core::cleanup::Options { join: true, duplicates: false, tiny: false, tol };
    let (res, removed, _) = putiacnc_core::cleanup::clean(&app.project.data.loops, Some(&scope), opts);
    let remap = |i: usize| i - removed.iter().filter(|&&r| r < i).count();
    app.sel = made.iter().filter(|i| !removed.contains(i)).map(|&i| remap(i)).collect();
    if res != app.project.data.loops {
        shapes::tabs_after_delete(&mut app.project.data.ops, &removed);
        app.project.data.loops = res;
    }
}

fn trim_click(app: &mut App, w: [f64; 2], tol: f64) {
    match trim_hover(app, w, tol) {
        Some(TrimHover::Trim(t, plan)) => {
            let pieces = putiacnc_core::trim::trimmed_shapes(&app.project.data.loops[t], &plan);
            app.hist.record(&app.project.data);
            let n_before = app.project.data.loops.len();
            let mut it = pieces.into_iter();
            match it.next() {
                // the first piece takes the shape's place, so every other shape keeps its number
                Some(first) => { app.project.data.loops[t] = first; app.project.data.loops.extend(it); }
                None => { app.project.data.loops.remove(t); shapes::tabs_after_delete(&mut app.project.data.ops, &[t]); }
            }
            join_after_trim(app, t, n_before);
            app.touch();
        }
        Some(TrimHover::Extend(t, plan)) => {
            let r = putiacnc_core::trim::extended_shape(&app.project.data.loops[t], &plan);
            app.hist.record(&app.project.data);
            app.project.data.loops[t] = r;
            app.sel.clear();
            app.touch();
        }
        _ => {}
    }
}

fn tool_options_bar(ctx: &egui::Context, app: &mut App, rect: Rect) {
    if !matches!(app.mode, Mode::Point | Mode::Polygon | Mode::Rect | Mode::Ellipse | Mode::Trim) || app.tab_edit.is_some() || app.node_edit.is_some() { return; }
    egui::Area::new(egui::Id::new("tool_options")).order(egui::Order::Foreground).fixed_pos(rect.min + Vec2::new(10.0, 62.0 + app.palette_extra)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
          ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
            if app.mode == Mode::Point {
                if ui.selectable_label(!app.point_ring && !app.point_grid, "Point").clicked() { app.point_ring = false; app.point_grid = false; }
                if ui.selectable_label(app.point_ring, "Bolt circle").clicked() { app.point_ring = true; app.point_grid = false; }
                if ui.selectable_label(app.point_grid, "Grid").clicked() { app.point_grid = true; app.point_ring = false; }
                if app.point_grid {
                    ui.add_space(6.0);
                    ui.label("Columns");
                    ui.add_space(6.0);
                    crate::widgets::num(ui, [44.0, 24.0], &mut app.grid_cols, 1.0, 100.0, 0.2, 0, "");
                    ui.add_space(8.0);
                    ui.label("Rows");
                    ui.add_space(6.0);
                    crate::widgets::num(ui, [44.0, 24.0], &mut app.grid_rows, 1.0, 100.0, 0.2, 0, "");
                }
                if app.point_ring {
                    ui.add_space(6.0);
                    ui.label("Holes");
                    ui.add_space(6.0);
                    crate::widgets::num(ui, [44.0, 24.0], &mut app.point_count, 1.0, 200.0, 0.2, 0, "");
                    ui.add_space(8.0);
                    ui.label("Sweep");
                    ui.add_space(6.0);
                    crate::widgets::num(ui, [56.0, 24.0], &mut app.point_sweep, -360.0, 360.0, 1.0, 0, "");
                    ui.add_space(2.0);
                    ui.weak("°");
                }
            }
            if app.mode == Mode::Polygon {
                if ui.selectable_label(!app.poly_star, "Polygon").clicked() { app.poly_star = false; if app.poly_sides == 5.0 { app.poly_sides = 6.0; } }
                if ui.selectable_label(app.poly_star, "Star").clicked() { app.poly_star = true; if app.poly_sides == 6.0 { app.poly_sides = 5.0; } }
                ui.add_space(6.0);
                ui.label(if app.poly_star { "Points" } else { "Sides" });
                ui.add_space(6.0);
                crate::widgets::num(ui, [44.0, 24.0], &mut app.poly_sides, 3.0, 64.0, 0.2, 0, "");
                if app.poly_star {
                    ui.add_space(6.0);
                    ui.label("Inner");
                    ui.add_space(6.0);
                    crate::widgets::num(ui, [48.0, 24.0], &mut app.poly_inner, 5.0, 95.0, 0.5, 0, "");
                    ui.add_space(2.0);
                    ui.weak("%");
                }
            }
            if app.mode == Mode::Ellipse {
                if ui.selectable_label(!app.circle_mode, "Ellipse").on_hover_text("Drag a bounding box").clicked() { app.circle_mode = false; app.dyn_in = None; }
                if ui.selectable_label(app.circle_mode, "Circle").on_hover_text("Drag from the centre, or type the centre and size").clicked() { app.circle_mode = true; app.dyn_in = None; }
                if app.circle_mode {
                    ui.add_space(8.0);
                    if ui.selectable_label(!app.circle_diam, "Radius").clicked() { app.circle_diam = false; app.dyn_in = None; }
                    if ui.selectable_label(app.circle_diam, "Diameter").clicked() { app.circle_diam = true; app.dyn_in = None; }
                }
            }
            if app.mode == Mode::Trim {
                if ui.selectable_label(!app.trim_extend, "Trim").on_hover_text("Remove the part of a shape between the nearest crossings").clicked() { app.trim_extend = false; }
                if ui.selectable_label(app.trim_extend, "Extend").on_hover_text("Run the end of an open line on to the next shape").clicked() { app.trim_extend = true; }
            }
            if app.mode == Mode::Rect {
                ui.label("Radius");
                ui.add_space(6.0);
                crate::widgets::num(ui, [56.0, 24.0], &mut app.rect_radius, 0.0, 500.0, 0.1, 2, "");
                ui.add_space(2.0);
                ui.weak("mm");
            }
          });
        });
    });
}

// ------------------------------------------------------------------- overlays

fn overlays(ctx: &egui::Context, app: &mut App, rect: Rect) {
    app.canvas_right = rect.right();
    // tool palette
    let pal = egui::Area::new(egui::Id::new("palette")).order(egui::Order::Foreground).fixed_pos(rect.min + Vec2::new(10.0, 10.0)).show(ctx, |ui| {
        // a narrow canvas wraps the palette onto a second row instead of running under the side panel
        ui.set_max_width((rect.width() - 28.0).max(240.0));
        egui::Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
          ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
            for (m, icon, tip) in [
                (Mode::Select, Icon::Select, "Select / move (V)"), (Mode::Node, Icon::Node, "Edit points (N)"), (Mode::Line, Icon::Line, "Line / polyline (L)"),
                (Mode::Rect, Icon::Rect, "Rectangle (R, Shift = square)"), (Mode::Ellipse, Icon::Ellipse, "Ellipse / circle (E, Shift = circle)"), (Mode::Polygon, Icon::Polygon, "Polygon or star (P)"), (Mode::Point, Icon::Point, "Drill point (D): click to place, use with a Drill operation"), (Mode::Text, Icon::Text, "Text (T)"), (Mode::Trim, Icon::Trim, "Trim and extend (X): click the part of a line to remove, or switch to Extend to run a line out to the next shape"),
            ] {
                if icon_button(ui, icon, app.mode == m, true, tip).clicked() { set_mode(app, m); }
            }
            if icon_button(ui, Icon::Combine, app.combine_open, app.sel.len() >= 2, "Combine the selected shapes: union, subtract, intersect, exclude").clicked() { app.combine_open = !app.combine_open; }
            crate::align::button(ui, app);
            // only offered when the selection has a corner to round (not for circles and ellipses)
            if app.selection_has_corners() {
                if icon_button(ui, Icon::Fillet, app.corners.open, true, "Round or bevel the corners of the selected shapes: fillet and chamfer").clicked() { app.corners.open = !app.corners.open; }
            }
            // thin vertical dividers as tall as the buttons (a Separator would stretch the whole area)
            let divider = |ui: &mut Ui| {
                let (sep, _) = ui.allocate_exact_size(Vec2::new(8.0, 36.0), egui::Sense::hover());
                ui.painter().line_segment([sep.center_top() + Vec2::new(0.0, 6.0), sep.center_bottom() - Vec2::new(0.0, 6.0)], ui.visuals().widgets.noninteractive.bg_stroke);
            };
            divider(ui);
            if icon_button(ui, Icon::Undo, false, app.hist.can_undo(crate::app::Scope::Drawing), "Undo drawing change (Edit menu: undo anything)").clicked() { app.do_undo_drawing(); }
            if icon_button(ui, Icon::Redo, false, app.hist.can_redo(crate::app::Scope::Drawing), "Redo drawing change (Edit menu: redo anything)").clicked() { app.do_redo_drawing(); }
            divider(ui);
            if icon_button(ui, Icon::Fit, false, true, "Fit the stock and work area in view (F)").clicked() { app.fit_pending = true; }
          });
        });
    });
    // the bars under the palette move down when it wraps (46 px is one row)
    app.palette_extra = (pal.response.rect.height() - 46.0).max(0.0);
    tool_options_bar(ctx, app, rect);
    if app.tab_edit.is_some() { tab_bar(ctx, app, rect); } else if app.node_edit.is_some() { node_bar(ctx, app, rect); } else { selection_bar(ctx, app, rect); }
    // hint line
    let hint = if app.node_edit.is_some() { "Drag a point to move it, click an edge to add one, drag a box to select several. Del removes the selected points, arrows nudge them. Esc or Done finishes." } else if app.tab_edit.is_some() { "Click on an outline to add a tab. Drag a tab to move it, right-click it to remove. Esc or Done finishes." } else { match app.mode {
        Mode::Select => "Click or box-select, drag to move, drag the handles to resize (Shift keeps proportions, Alt scales from the centre). Click a selected shape again to rotate it. Double-click or N edits points. Del deletes. Drag the 3D model to move it. Middle/right-drag pans.",
        Mode::Node => "Click a shape to edit its points.",
        Mode::Line => "Click points. Double-click, Enter or right-click ends. Click the first point or press C to close. Esc cancels.",
        Mode::Rect => "Drag to draw a rectangle. Hold Shift for a square.",
        Mode::Ellipse => if app.circle_mode { "Drag from the centre: the distance is the radius. Or just type: centre X, Y and the size." } else { "Drag a bounding box. Hold Shift for a circle." },
        Mode::Point => "Click to place a drill point, or choose Grid and drag a box for columns x rows of holes, or Bolt circle and drag from the centre: the distance sets the radius, the first hole follows the pointer (Shift = straight up). Sweep below 360° makes an arc of holes (negative = clockwise). Or type the position (grid: corner and size; bolt circle: centre, radius, angle). Assign the points to a Drill operation. Esc finishes.",
        Mode::Polygon => "Drag from the centre: the first corner follows the pointer, the distance sets the size. Hold Shift to point it straight up. Sides, star and inner radius are set in the bar above. Or type: centre X, Y, radius and angle.",
        Mode::Text => "Type the text in the Text window. Click the drawing where it should start, or type X and Y there.",
        Mode::Trim => if app.trim_extend { "Click near the end of an open line: it runs on in a straight line to the first shape it meets." } else { "Click the part of a shape you want to remove: it goes up to the nearest crossing on each side (shown in red). A closed shape needs two crossings." },
    } };
    if app.trace.open || app.clean.open { return; } // a dialog covers that part of the canvas
    let hint = if app.dyn_in.is_some() { "Typing values: Enter confirms, Tab moves to the next field, Esc cancels. Empty fields follow the pointer." } else { hint };
    egui::Area::new(egui::Id::new("hint")).order(egui::Order::Foreground).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(rect.left_bottom() + Vec2::new(12.0, -28.0)).interactable(false).show(ctx, |ui| {
        ui.set_max_width((rect.width() - 24.0).max(120.0));
        ui.label(egui::RichText::new(hint).weak());
    });
}

// ------------------------------------------------------------------- resize handles

/// World position of handle `h` on bounding box `b = [x0, y0, x1, y1]`
/// (0 top-left, 1 top, 2 top-right, 3 right, 4 bottom-right, 5 bottom, 6 bottom-left, 7 left).
fn handle_world(b: [f64; 4], h: usize) -> [f64; 2] {
    let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    match h {
        0 => [b[0], b[3]], 1 => [cx, b[3]], 2 => [b[2], b[3]], 3 => [b[2], cy],
        4 => [b[2], b[1]], 5 => [cx, b[1]], 6 => [b[0], b[1]], _ => [b[0], cy],
    }
}

/// Handles are drawn 4 px outside the selection box so they don't cover the shapes.
fn handle_screen(view: &View, b: [f64; 4], h: usize) -> Pos2 {
    let p = view.w2s(handle_world(b, h));
    let dx = match h { 0 | 6 | 7 => -4.0, 2 | 3 | 4 => 4.0, _ => 0.0 };
    let dy = match h { 0 | 1 | 2 => -4.0, 4 | 5 | 6 => 4.0, _ => 0.0 };
    p + Vec2::new(dx, dy)
}

/// Resize handles are for real shapes and never for a locked drawing; a selection of only drill points has none (a point keeps its size).
fn handles_apply(app: &App) -> bool {
    !app.sel_locked() && app.sel.iter().any(|&i| app.project.data.loops.get(i).map_or(false, |l| !shapes::is_point(l)))
}

fn hit_handle(view: &View, b: [f64; 4], pos: Pos2) -> Option<usize> {
    (0..8).find(|&h| handle_screen(view, b, h).distance(pos) <= 9.0)
}

fn handle_cursor(h: usize) -> egui::CursorIcon {
    match h {
        0 | 4 => egui::CursorIcon::ResizeNwSe,
        2 | 6 => egui::CursorIcon::ResizeNeSw,
        1 | 5 => egui::CursorIcon::ResizeVertical,
        _ => egui::CursorIcon::ResizeHorizontal,
    }
}

/// New bounding box while dragging handle `handle`. Shift keeps the aspect ratio (corners),
/// Alt scales symmetrically about the centre; the moving edge snaps to the grid.
fn resized_bbox(app: &App, handle: usize, sb: [f64; 4], grab: [f64; 2], cur: [f64; 2], shift: bool, alt: bool) -> [f64; 4] {
    let (mx0, mx1) = (matches!(handle, 0 | 6 | 7), matches!(handle, 2 | 3 | 4));
    let (my0, my1) = (matches!(handle, 4 | 5 | 6), matches!(handle, 0 | 1 | 2));
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

/// Position, size, corner radius and operation of the selected shapes, as a small bar over the canvas.
fn selection_bar(ctx: &egui::Context, app: &mut App, rect: Rect) {
    let n = app.sel.len();
    app.sel.retain(|&i| i < app.project.data.loops.len());
    if app.sel.is_empty() || app.mode != Mode::Select { return; }
    let before = app.project.data.clone();
    let sel = app.sel.clone();
    let text_gid = app.selected_text();
    let mut text_act = 0u8; // 1 edit, 2 convert to outlines
    let d = &mut app.project.data;
    let Some(b) = shapes::bbox_many(sel.iter().map(|&i| &d.loops[i])) else { return };
    // X and Y are relative to the XY zero, like the rulers and the pointer readout
    let (zx, zy) = (match d.stock.zr.as_str() { "lr" | "ur" => d.stock.w, "c" => d.stock.w / 2.0, _ => 0.0 }, match d.stock.zr.as_str() { "ul" | "ur" => d.stock.h, "c" => d.stock.h / 2.0, _ => 0.0 });
    let (mut x, mut y, mut w, mut h) = (b[0] - zx, b[1] - zy, b[2] - b[0], b[3] - b[1]);
    let mut changed = false;
    let mut geom = false;
    let single_rect = if sel.len() == 1 { d.loops[*sel.iter().next().unwrap()].rc } else { None };
    let mut radius = single_rect.unwrap_or(0.0);
    let mut radius_changed = false;
    let ops: Vec<(i64, String)> = d.ops.iter().enumerate().map(|(i, o)| (o.id, format!("{}. {}", i + 1, crate::panels::label_of(crate::panels::KINDS, &o.p.kind)))).collect();
    let first_op = d.loops[*sel.iter().next().unwrap()].op;
    let same = sel.iter().all(|&i| d.loops[i].op == first_op);
    let cur = if !same { "Mixed".to_string() } else { ops.iter().find(|(id, _)| *id == first_op).map(|(_, s)| s.clone()).unwrap_or_else(|| "No operation".into()) };
    let mut pick: Option<i64> = None;
    let _ = n;
    // shapes of a locked drawing can be assigned to operations but not moved, resized or aligned
    let locked = sel.iter().filter_map(|&i| d.loops[i].grp).any(|g| d.groups.iter().any(|x| x.id == g && x.locked));
    egui::Area::new(egui::Id::new("selbar")).order(egui::Order::Foreground).fixed_pos(rect.min + Vec2::new(10.0, 66.0 + app.palette_extra)).show(ctx, |ui| {
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
                if single_rect.is_some() {
                    ui.add_space(6.0);
                    radius_changed |= field(ui, "Radius", &mut radius, 0.0, 100000.0);
                }
                ui.add_space(6.0);
                egui::ComboBox::from_id_source("selop").width(130.0).selected_text(cur.clone()).show_ui(ui, |ui| {
                    for (id, s) in &ops { if ui.selectable_label(same && first_op == *id, s).clicked() { pick = Some(*id); } }
                    if ui.selectable_label(same && first_op == 0, "No operation").clicked() { pick = Some(0); }
                }).response.on_hover_text("Operation applied to the selected shapes");
            });
            if text_gid.is_some() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label("Text");
                    if ui.button("Edit text").on_hover_text("Change the words, font, size or place of this text (double-click does the same)").clicked() { text_act = 1; }
                    if ui.button("To outlines").on_hover_text("Stop treating the letters as text: they become ordinary shapes you can edit point by point").clicked() { text_act = 2; }
                });
            }
        });
    });
    if locked { geom = false; radius_changed = false; }
    if geom {
        let mut refs: Vec<&mut putiacnc_core::model::LoopData> = d.loops.iter_mut().enumerate().filter(|(i, _)| sel.contains(i)).map(|(_, l)| l).collect();
        shapes::fit_to_bbox(&mut refs, b, [x + zx, y + zy, x + zx + w.max(0.01), y + zy + h.max(0.01)]);
        changed = true;
    }
    if radius_changed {
        if let Some(&i) = sel.iter().next() { shapes::set_corner_radius(&mut d.loops[i], radius); changed = true; }
    }
    if let Some(id) = pick { for &i in sel.iter() { d.loops[i].op = id; } changed = true; }
    if changed {
        if !app.hist.open {
            app.hist.push(before);
            app.hist.open = true;
        }
        app.touch();
    }
    match (text_act, text_gid) {
        (1, Some(g)) => app.edit_text(g),
        (2, Some(g)) => app.text_to_outlines(g),
        _ => {}
    }
}


// ------------------------------------------------------------------- manual holding tabs

/// How close (screen pixels) the pointer must be to an outline to add a tab there.
const TAB_REACH_PX: f64 = 16.0;

/// Closest point on the closed shapes of operation `op_id`: (shape index, point, distance, direction).
fn nearest_outline(app: &App, op_id: i64, w: [f64; 2]) -> Option<(usize, [f64; 2], f64, [f64; 2])> {
    app.project.data.loops.iter().enumerate().filter(|(_, l)| l.op == op_id && l.closed())
        .filter_map(|(i, l)| shapes::nearest_on_loop(l, w).map(|(q, d, t)| (i, q, d, t)))
        .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
}

/// Index of the placed tab under the pointer, if any.
fn hovered_tab(app: &App, op_id: i64, w: [f64; 2]) -> Option<usize> {
    let d = &app.project.data;
    let tp = d.ops.iter().find(|o| o.id == op_id)?.p.tab_pts.as_ref()?;
    let s = app.scale as f64;
    tp.iter().enumerate()
        .filter_map(|(i, t)| shapes::tab_resolve(&d.loops, op_id, t).map(|(q, _)| (i, (q[0] - w[0]).hypot(q[1] - w[1]) * s)))
        .filter(|(_, d)| *d < 11.0)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(i, _)| i)
}

fn tab_interaction(ctx: &egui::Context, app: &mut App, resp: &egui::Response, view: &View, op_id: i64, hover_w: Option<[f64; 2]>) {
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
        }
    }
    if app.tab_drag.is_none() {
        if let Some(w) = hover_w {
            if hovered_tab(app, op_id, w).is_some() { ctx.set_cursor_icon(egui::CursorIcon::Grab); }
            else if nearest_outline(app, op_id, w).map_or(false, |n| n.2 * app.scale as f64 <= TAB_REACH_PX) { ctx.set_cursor_icon(egui::CursorIcon::Crosshair); }
        }
    }
}

/// Small bar shown while tabs are being placed.
fn tab_bar(ctx: &egui::Context, app: &mut App, rect: Rect) {
    let Some(id) = app.tab_edit else { return };
    let n = app.project.data.ops.iter().find(|o| o.id == id).and_then(|o| o.p.tab_pts.as_ref()).map_or(0, |v| v.len());
    let k = app.project.data.ops.iter().position(|o| o.id == id).map_or(0, |k| k + 1);
    let mut done = false;
    egui::Area::new(egui::Id::new("tabbar")).order(egui::Order::Foreground).fixed_pos(rect.min + Vec2::new(10.0, 66.0 + app.palette_extra)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(10.0, 6.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Placing tabs for operation {k} · {n} tab{}", if n == 1 { "" } else { "s" }));
                ui.add_space(6.0);
                if crate::icons::icon_text_button(ui, Icon::Close, "Done", "Stop placing tabs (Esc)", None).clicked() { done = true; }
            });
        });
    });
    if done { app.tab_edit = None; }
}

// ------------------------------------------------------------------- point editing

/// Switches tool. Leaves point editing; the node tool starts on the selected shape when there is exactly one.
fn set_mode(app: &mut App, m: Mode) {
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

fn enter_node_edit(app: &mut App, li: usize) {
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
fn node_at(app: &App, pts: &[[f64; 2]], w: [f64; 2]) -> Option<usize> {
    let s = app.scale as f64;
    pts.iter().enumerate()
        .map(|(i, p)| (i, (p[0] - w[0]).hypot(p[1] - w[1]) * s))
        .filter(|(_, d)| *d <= 8.0)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(i, _)| i)
}

fn node_keys(ctx: &egui::Context, app: &mut App) {
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

fn node_interaction(ctx: &egui::Context, app: &mut App, resp: &egui::Response, view: &View, li: usize, hover_w: Option<[f64; 2]>, pointer_w: Option<[f64; 2]>, tol: f64) {
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
fn node_bar(ctx: &egui::Context, app: &mut App, rect: Rect) {
    let Some(li) = app.node_edit else { return };
    let Some(l) = app.project.data.loops.get(li) else { return };
    let n = l.pts.len() / 2;
    let plain = l.rc.is_none() && l.cr.is_none() && l.el.is_none();
    let (mut done, mut simplify, mut corner) = (false, false, false);
    let mut tol = app.node_tol;
    let (mut cham, mut csize, nsel) = (app.corners.chamfer, app.corners.size, app.node_sel.len());
    egui::Area::new(egui::Id::new("nodebar")).order(egui::Order::Foreground).fixed_pos(rect.min + Vec2::new(10.0, 66.0 + app.palette_extra)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(10.0, 6.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Editing points · {n} point{}", if n == 1 { "" } else { "s" }));
                ui.add_space(6.0);
                ui.label("Simplify within");
                ui.add(egui::DragValue::new(&mut tol).clamp_range(0.01..=5.0).speed(0.01).suffix(" mm"));
                if ui.button("Simplify").on_hover_text("Remove points that hardly change the shape").clicked() { simplify = true; }
                ui.add_space(6.0);
                if crate::icons::icon_text_button(ui, Icon::Close, "Done", "Stop editing points (Esc)", None).clicked() { done = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Corners");
                ui.add_space(4.0);
                if ui.selectable_label(!cham, "Fillet").on_hover_text("Round the selected corners").clicked() { cham = false; }
                if ui.selectable_label(cham, "Chamfer").on_hover_text("Bevel the selected corners").clicked() { cham = true; }
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
