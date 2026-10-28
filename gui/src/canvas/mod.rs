//! The 2D drawing canvas: view control, drawing tools, selection and keyboard shortcuts.
use crate::app::{Action, App, Drag, Mode, NodeDrag, View};
use crate::icons::{icon_button, Icon};
use crate::theme::op_color;
use eframe::egui::{self, Color32, Key, Modifiers, PointerButton, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use putiacnc_core::shapes;
mod draw;
mod handles;
mod nodes;
mod palette;
mod rulers;
mod tabs;
mod trim;
use self::draw::*;
use self::handles::*;
use self::nodes::*;
use self::palette::*;
use self::rulers::*;
use self::tabs::*;
use self::trim::*;

pub(super) fn finish_poly(app: &mut App, close: bool) {
    let op = 0;
    let pts = std::mem::take(&mut app.poly);
    if let Some(l) = shapes::new_poly(&pts, close, op) { app.add_loop(l); }
}

pub fn handle_keys(ctx: &egui::Context, app: &mut App) {
    // a right-click on the drawing counts as Esc: it is fed in as the key, so every place that listens for Esc answers it
    if std::mem::take(&mut app.esc_request) {
        ctx.input_mut(|i| i.events.push(egui::Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE }));
    }
    if ctx.wants_keyboard_input() { return; }
    let (save_as, save, redo, redo2, undo, all, dup, new, open, export, quit, import, group, ungroup) = ctx.input_mut(|i| {
        let save_as = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S);
        let save = i.consume_key(Modifiers::COMMAND, Key::S);
        let redo = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        let redo2 = i.consume_key(Modifiers::COMMAND, Key::Y);
        let undo = i.consume_key(Modifiers::COMMAND, Key::Z);
        let all = i.consume_key(Modifiers::COMMAND, Key::A);
        let dup = i.consume_key(Modifiers::COMMAND, Key::D);
        let ungroup = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::G);
        let group = i.consume_key(Modifiers::COMMAND, Key::G);
        let new = i.consume_key(Modifiers::COMMAND, Key::N);
        let open = i.consume_key(Modifiers::COMMAND, Key::O);
        let export = i.consume_key(Modifiers::COMMAND, Key::E);
        let quit = i.consume_key(Modifiers::COMMAND, Key::Q);
        let import = i.consume_key(Modifiers::COMMAND, Key::I);
        (save_as, save, redo, redo2, undo, all, dup, new, open, export, quit, import, group, ungroup)
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
    if ungroup { app.ungroup_selection(); } else if group { app.group_selection(); }

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
        // an open tool dialog (fillet, offset, array, combine, clean up) is closed first; the selection stays for the next press
        if app.corners.open || app.offset_open || app.combine_open || app.array.open || app.clean.open {
            app.corners.open = false; app.offset_open = false; app.combine_open = false; app.array.open = false; app.clean.open = false;
        }
        else if !app.poly.is_empty() { app.poly.clear(); }
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

/// The clamp under `w`, unless a drawing shape is there (shapes win: their edge, or the inside of a selected one).
pub(super) fn clamp_at(app: &App, w: [f64; 2], tol: f64) -> Option<usize> {
    let d = &app.project.data;
    let on_shape = d.loops.iter().any(|l| shapes::dist_to_loop(l, w) <= tol) || app.sel.iter().any(|&i| d.loops.get(i).is_some_and(|l| shapes::point_in_loop(l, w)));
    if on_shape { return None; }
    if !app.show_clamps { return None; }
    d.clamps.iter().rposition(|c| w[0] >= c.x && w[0] <= c.x + c.w && w[1] >= c.y && w[1] <= c.y + c.h)
}

/// The scale and offset that frame the stock, plus the machine work area when it is shown, in `rect` (the free band between the
/// palette at the top and the hint at the bottom). This is the 100 % of the zoom card.
pub(super) fn fit_view(app: &App, rect: Rect) -> (f32, Vec2) {
    let (sw, sh) = (app.project.data.stock.w, app.project.data.stock.h);
    let (mut bw, mut bh) = (sw, sh);
    if app.show_machine_area { if let Some(m) = &app.project.data.machine { bw = bw.max(m.x); bh = bh.max(m.y); } }
    // the palette floats over the top of the canvas and the hint over the bottom: frame the stock in the free band between them
    let (top, bottom) = (62.0 + app.palette_extra, 50.0);
    let avail = rect.height() - top - bottom;
    let (band, top) = if avail >= rect.height() * 0.4 { (avail, top) } else { (rect.height(), 0.0) };
    let sx = rect.width() / (bw as f32 * 1.12);
    let sy = band / (bh as f32 * 1.12);
    let scale = sx.min(sy).max(0.01);
    (scale, Vec2::new(rect.width() / 2.0 - bw as f32 * scale / 2.0, top + band / 2.0 + bh as f32 * scale / 2.0))
}

/// Zoom the drawing by `f` about the middle of the canvas (the buttons and keys; the wheel and pinch zoom about the pointer).
pub(super) fn zoom_centre(app: &mut App, rect: Rect, f: f32) {
    let (s, o) = crate::zoomcard::zoom_about(app.scale, app.offset, rect.size() / 2.0, f, 0.05, 500.0);
    app.scale = s; app.offset = o; app.view_touched = true;
}

pub fn canvas(ui: &mut Ui, app: &mut App) {
    let ctx = ui.ctx().clone();
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
let (sw, sh) = (app.project.data.stock.w, app.project.data.stock.h);
    let zr = app.project.data.stock.zr.clone();
    let zero = (match zr.as_str() { "lr" | "ur" => sw, "c" => sw / 2.0, _ => 0.0 }, match zr.as_str() { "ul" | "ur" => sh, "c" => sh / 2.0, _ => 0.0 });

    // the tool palette wrapped to another row (or back): frame the stock again below it, unless the view was moved by hand
    if !app.view_touched && (app.palette_extra - app.fit_extra).abs() > 0.5 { app.fit_pending = true; }
    let key_act = crate::zoomcard::keys(&ctx);
    if app.fit_pending || key_act == Some(crate::zoomcard::Act::Fit) || (!ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::F))) {
        let (scale, offset) = fit_view(app, rect);
        app.scale = scale; app.offset = offset;
        app.fit_pending = false; app.fit_extra = app.palette_extra; app.view_touched = false;
    }
    if let Some(crate::zoomcard::Act::By(f)) = key_act { zoom_centre(app, rect, f); }
    if resp.dragged_by(PointerButton::Middle) || resp.dragged_by(PointerButton::Secondary) { app.offset += resp.drag_delta(); app.view_touched = true; }
    // the closed hand from the moment the button goes down, so the pan is easy to find and to recognise
    if (resp.hovered() || resp.dragged()) && ctx.input(|i| i.pointer.button_down(PointerButton::Secondary) || i.pointer.button_down(PointerButton::Middle)) { ctx.set_cursor_icon(egui::CursorIcon::Grabbing); }
    if resp.hovered() {
        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let ns = (app.scale * (scroll * 0.0015).exp()).clamp(0.05, 500.0);
            if let Some(m) = resp.hover_pos() {
                let local = m - rect.min;
                app.offset = local - (local - app.offset) * (ns / app.scale);
            }
            app.scale = ns; app.view_touched = true;
        }
        // a pinch on a trackpad, or Ctrl and the wheel
        let pinch = crate::zoomcard::pinch(&ctx);
        if (pinch - 1.0).abs() > 1e-4 {
            if let Some(m) = resp.hover_pos() {
                let (ns, no) = crate::zoomcard::zoom_about(app.scale, app.offset, m - rect.min, pinch, 0.05, 500.0);
                app.scale = ns; app.offset = no; app.view_touched = true;
            }
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
    let mut clamp_hot: Option<usize> = None;
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
    // a right-click (not a right-drag, which pans) is Esc; the line tool and tab placement have their own use of it, handled where they are
    if resp.clicked_by(PointerButton::Secondary) && editing_tabs.is_none() && app.mode != Mode::Line { app.esc_request = true; }
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
                            if !app.sel.contains(&h) { if !shift { app.sel.clear(); } let sib = app.text_siblings(h); app.sel.extend(sib); }
                            let b = shapes::bbox_many(app.sel.iter().map(|&i| &app.project.data.loops[i])).unwrap();
                            // a locked drawing is selected but stays where it is
                            app.drag = if app.sel_locked() { Drag::None } else { Drag::Move { start: w, anchor: [b[0], b[1]], applied: [0.0, 0.0] } };
                        }
                        // empty space over the 3D model drags the model (Shift box-selects instead)
                        None if !shift && clamp_at(app, w, tol).is_some() => {
                            let idx = clamp_at(app, w, tol).unwrap_or(0);
                            let cl = &app.project.data.clamps[idx];
                            app.drag = Drag::Clamp { idx, start: w, orig: [cl.x, cl.y] };
                        }
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
                    let nb = resized_bbox(app, *handle, *start_bbox, *grab, cur, shift, alt);
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
                    let moved = app.project.data.relief.as_ref().is_some_and(|r| (r.x - t[0]).abs() > 1e-9 || (r.y - t[1]).abs() > 1e-9);
                    if moved {
                        app.hist.record_once(&app.project.data);
                        if let Some(r) = app.project.data.relief.as_mut() { r.x = t[0]; r.y = t[1]; }
                        app.touch();
                    }
                }
                if let (Drag::Clamp { idx, start, orig }, Some(cur)) = (&app.drag, pointer_w) {
                    let (idx, t) = (*idx, app.snap([orig[0] + cur[0] - start[0], orig[1] + cur[1] - start[1]]));
                    let moved = app.project.data.clamps.get(idx).is_some_and(|c| (c.x - t[0]).abs() > 1e-9 || (c.y - t[1]).abs() > 1e-9);
                    if moved {
                        app.hist.record_once(&app.project.data);
                        if let Some(c) = app.project.data.clamps.get_mut(idx) { c.x = t[0]; c.y = t[1]; }
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
            if hover_w.is_some_and(|w| shapes::pick(&app.project.data.loops, w, tol).is_some()) { ctx.set_cursor_icon(egui::CursorIcon::PointingHand); }
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
                        let op = 0;
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
                    let op = 0;
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
                    let op = 0;
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
                        if r > 0.01 { let op = 0; app.add_loop(shapes::new_circle(a, r, op)); }
                    } else if (b[0] - a[0]).abs() > 0.01 && (b[1] - a[1]).abs() > 0.01 {
                        let op = 0;
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
                if app.poly.len() >= 2 { finish_poly(app, false); } else if !app.poly.is_empty() { app.poly.clear(); } else { app.esc_request = true; }
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
        else if matches!(app.drag, Drag::Move { .. }) { ctx.set_cursor_icon(egui::CursorIcon::AllScroll); }
        else if let Drag::Clamp { idx, .. } = &app.drag { clamp_hot = Some(*idx); ctx.set_cursor_icon(egui::CursorIcon::Grabbing); }
        else if matches!(app.drag, Drag::None) && resp.hovered() && hover_w.and_then(|w| clamp_at(app, w, tol)).is_some() {
            clamp_hot = hover_w.and_then(|w| clamp_at(app, w, tol));
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
        else if matches!(app.drag, Drag::None) {
            let sel_bb = shapes::bbox_many(app.sel.iter().filter_map(|&i| app.project.data.loops.get(i))).filter(|_| handles_apply(app));
            let mut on_handle = false;
            if let (Some(ps), Some(b)) = (ctx.input(|i| i.pointer.latest_pos()), sel_bb) {
                if app.rotate_mode {
                    let pivot = app.rot_pivot.unwrap_or([(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0]);
                    if view.w2s(pivot).distance(ps) <= 10.0 || hit_handle(&view, b, ps).is_some_and(|h| h % 2 == 0) { ctx.set_cursor_icon(egui::CursorIcon::Grab); }
                } else if let Some(h) = hit_handle(&view, b, ps) { ctx.set_cursor_icon(handle_cursor(h)); on_handle = true; }
                if app.rotate_mode && (view.w2s(app.rot_pivot.unwrap_or([(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0])).distance(ps) <= 10.0 || hit_handle(&view, b, ps).is_some_and(|h| h % 2 == 0)) { on_handle = true; }
            }
            // over a selected shape (its outline, or its inside): the four-way move cursor; unselected shapes keep the arrow
            if !on_handle && resp.hovered() {
                if let Some(w) = hover_w {
                    let loops = &app.project.data.loops;
                    let edge = loops.iter().enumerate().map(|(i, l)| (i, shapes::dist_to_loop(l, w))).filter(|(_, d)| *d <= tol)
                        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|(i, _)| i);
                    let hit = edge.or_else(|| app.sel.iter().copied().find(|&i| shapes::point_in_loop(&loops[i], w)));
                    if hit.is_some_and(|h| app.sel.contains(&h) && !app.loop_locked(h)) { ctx.set_cursor_icon(egui::CursorIcon::AllScroll); }
                }
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
    // separate pieces of stock (inlay plugs), beside the main one
    for (k, o) in d.ops.iter().enumerate() {
        let Some(pc) = o.p.piece.as_ref().filter(|_| o.p.kind == "plug") else { continue };
        let pm = th.material(&pc.mat);
        let r = Rect::from_two_pos(to_s([pc.x, pc.y]), to_s([pc.x + pc.w, pc.y + pc.h]));
        painter.rect_filled(r, 0.0, pm.fill);
        painter.rect_stroke(r, 0.0, Stroke::new(1.5_f32, pm.stroke));
        painter.text(r.left_top() + egui::vec2(0.0, -3.0), egui::Align2::LEFT_BOTTOM, format!("Plug piece, operation {}: {} × {} × {} mm", k + 1, pc.w, pc.h, pc.t), egui::TextStyle::Small.resolve(ui.style()), th.shape_idle());
    }
    // clamps: keep-out rectangles on the table
    for (i, c) in d.clamps.iter().enumerate().filter(|_| app.show_clamps) {
        let r = Rect::from_two_pos(to_s([c.x, c.y]), to_s([c.x + c.w, c.y + c.h]));
        // a clamp the toolpath runs into (from the background checks; the last result stays up while a new one is worked out)
        let hit_by: Vec<usize> = app.clamp_hits.iter().find(|h| h.0 == i).map(|h| h.1.clone()).unwrap_or_default();
        let hit = !hit_by.is_empty();
        painter.rect_filled(r, 2.0, Color32::from_rgba_unmultiplied(0xe0, 0x1b, 0x24, if hit { 130 } else { 55 }));
        painter.rect_stroke(r, 2.0, Stroke::new(if clamp_hot == Some(i) || hit { 3.0_f32 } else { 1.5_f32 }, Color32::from_rgb(0xe0, 0x1b, 0x24)));
        if hit {
            let ops: Vec<String> = hit_by.iter().map(|o| (o + 1).to_string()).collect();
            let tip = format!("Operation {} runs into this clamp", ops.join(", "));
            // the same warning sign as in the 3D preview, standing in the clamp's top right corner
            let c0 = r.right_top() + egui::vec2(-11.0, 23.0);
            crate::checkui::marker_sized(&painter, c0, false, "", 0.85);
            if hover_w.is_some() && resp.hovered() && ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| (p.x - c0.x).abs() < 11.0 && p.y > c0.y - 21.0 && p.y < c0.y + 3.0) && !ui.memory(|m| m.any_popup_open()) {
                egui::show_tooltip_text(ui.ctx(), egui::Id::new(("clamp_hit", i)), tip);
            }
        }
        painter.line_segment([r.left_top(), r.right_bottom()], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0xe0, 0x1b, 0x24, 120)));
        painter.line_segment([r.right_top(), r.left_bottom()], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0xe0, 0x1b, 0x24, 120)));
        painter.text(r.center_top() + egui::vec2(0.0, -3.0), egui::Align2::CENTER_BOTTOM, format!("Clamp {}", i + 1), egui::TextStyle::Small.resolve(ui.style()), Color32::from_rgb(0xe0, 0x1b, 0x24));
    }
    // the imported 3D model, as a shaded height map on the stock
    if let Some(rel) = &d.relief {
        let key = format!("{}|{}|{}|{}|{}", rel.name, rel.nx, rel.ny, rel.hm.len(), rel.hm.get(..48).unwrap_or("") );
        if app.relief_tex.as_ref().is_none_or(|(k, _)| *k != key) {
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
        let col = if app.ghost.hide.contains(&i) { col.gamma_multiply(0.3) } else { col };
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
    // the number of the operation on the shapes that belong to one, so the link does not rest on colour alone. Shapes of one operation
    // that lie close together (the letters of a text, a hole in its outline) share a single badge; huge drawings get none
    if app.show_badges && d.loops.len() <= 600 {
        let boxes: Vec<Option<(usize, Rect)>> = d.loops.iter().map(|l| {
            let k = d.ops.iter().position(|o| o.id == l.op)?;
            let b = shapes::bbox(l)?;
            Some((k, Rect::from_two_pos(to_s([b[0], b[1]]), to_s([b[2], b[3]]))))
        }).collect();
        let mut root: Vec<usize> = (0..boxes.len()).collect();
        fn find(r: &mut [usize], i: usize) -> usize { let mut i = i; while r[i] != i { r[i] = r[r[i]]; i = r[i]; } i }
        for i in 0..boxes.len() {
            let Some((ki, ri)) = &boxes[i] else { continue };
            for j in 0..i {
                let Some((kj, rj)) = &boxes[j] else { continue };
                // the letters of one text always share a badge; other shapes of an operation do when they lie close together compared with
                // their own size (so the grouping does not fall apart when the view is zoomed in)
                let same_group = d.loops[i].grp.is_some() && d.loops[i].grp == d.loops[j].grp;
                let reach = (0.4 * ri.width().max(ri.height()).min(rj.width().max(rj.height()))).max(10.0);
                if ki == kj && (same_group || ri.expand(reach).intersects(*rj)) { let (a, b) = (find(&mut root, i), find(&mut root, j)); root[a] = b; }
            }
        }
        let mut clusters: std::collections::BTreeMap<usize, (usize, Rect)> = Default::default();
        for i in 0..boxes.len() {
            let Some((k, r)) = &boxes[i] else { continue };
            let rt = find(&mut root, i);
            clusters.entry(rt).and_modify(|(_, u)| *u = u.union(*r)).or_insert((*k, *r));
        }
        for (k, r) in clusters.values() {
            // only specks too small to tell apart go without; the badge sits clear of the shape, so it does not hide a small one
            if !painter.clip_rect().intersects(*r) || (r.width() < 6.0 && r.height() < 6.0) { continue; }
            let col = op_color(*k, dark);
            let lum = 0.299 * col.r() as f32 + 0.587 * col.g() as f32 + 0.114 * col.b() as f32;
            let ink = if lum > 150.0 { Color32::from_rgb(0x1e, 0x1e, 0x1e) } else { Color32::WHITE };
            let c = r.left_top() + Vec2::new(-12.0, -12.0);
            painter.circle_filled(c, 8.5, th.view());
            painter.circle_filled(c, 7.0, col);
            painter.text(c, egui::Align2::CENTER_CENTER, format!("{}", k + 1), egui::FontId::proportional(10.0), ink);
        }
    }
    // the preview of the tool whose dialog is open: what it would add or change
    for l in &app.ghost.loops {
        if shapes::is_point(l) { continue; }
        let mut pts: Vec<Pos2> = l.pts.chunks_exact(2).map(|c| to_s([c[0] as f64 / shapes::S, c[1] as f64 / shapes::S])).collect();
        if l.closed() { if let Some(f) = pts.first().copied() { pts.push(f); } }
        painter.add(egui::Shape::line(pts, Stroke::new(2.0_f32, th.accent)));
    }
    // a dashed box round each selected group, so it is plain which shapes belong together (when the group is the whole selection, the selection box says it)
    if editing_nodes.is_none() && !app.group_is_whole_selection() {
        let mut ids: Vec<i64> = app.sel.iter().filter_map(|&i| d.loops.get(i).and_then(|l| l.ug)).collect();
        ids.sort_unstable();
        ids.dedup();
        for g in ids {
            if let Some(b) = shapes::bbox_many(d.loops.iter().filter(|l| l.ug == Some(g))) {
                let r = Rect::from_two_pos(to_s([b[0], b[1]]), to_s([b[2], b[3]])).expand(2.0);
                let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.0_f32, th.shape_selected().gamma_multiply(0.7)), 5.0, 4.0));
            }
        }
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
                    let spots: Vec<[f64; 2]> = match app.output.as_ref().filter(|_| !app.gen_stale()) {
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

    if app.show_toolpath {
        // the toolpath on hand stays drawn while a new one is worked out, so it does not blink off with every change; only when the wait
        // gets long does it fade, to say it is out of date
        let fade = if app.gen_started.is_some_and(|t| t.elapsed().as_secs_f32() > 0.4) { 0.35 } else { 1.0 };
        if let Some(o) = &app.output {
            for m in &o.moves {
                if !m.cut && !app.show_rapids { continue; }
                if m.from[0] == m.to[0] && m.from[1] == m.to[1] { continue; }
                let col = if m.cut { op_color(m.op, dark).gamma_multiply(0.85 * fade) } else { th.rapid().gamma_multiply(fade) };
                painter.line_segment([to_s([m.from[0], m.from[1]]), to_s([m.to[0], m.to[1]])], Stroke::new(1.0_f32, col));
            }
        }
        // the toolpath of each separate piece, in the piece's own frame: drawn where the piece sits in the drawing
        for pp in &app.pieces {
            let Some(pc) = app.project.data.ops.get(pp.job.op_index).and_then(|o| o.p.piece.as_ref()) else { continue };
            for m in &pp.output.moves {
                if !m.cut && !app.show_rapids { continue; }
                if m.from[0] == m.to[0] && m.from[1] == m.to[1] { continue; }
                let col = if m.cut { op_color(m.op, dark).gamma_multiply(0.85 * fade) } else { th.rapid().gamma_multiply(fade) };
                painter.line_segment([to_s([m.from[0] + pc.x, m.from[1] + pc.y]), to_s([m.to[0] + pc.x, m.to[1] + pc.y])], Stroke::new(1.0_f32, col));
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
                hud = Some(format!("{} holes, {:.2} × {:.2} mm", v.len(), (to[0] - a[0]).abs(), (to[1] - a[1]).abs()));
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
        } else { format!("{:.2} × {:.2} mm", (b[0] - a[0]).abs(), (b[1] - a[1]).abs()) });
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
    if !app.tools_open && !app.machines_open {
        let frac = app.scale / fit_view(app, rect).0.max(1e-6);
        overlays(&ctx, app, orect, frac);
        rotate_popup(&ctx, app, rect);
        // what the zoom card asked for takes effect on the next frame
        match app.zoom_act.take() {
            Some(crate::zoomcard::Act::By(f)) => { zoom_centre(app, rect, f); ctx.request_repaint(); }
            Some(crate::zoomcard::Act::To(t)) => { let f = t / frac.max(1e-6); zoom_centre(app, rect, f); ctx.request_repaint(); }
            Some(crate::zoomcard::Act::Fit) => { app.fit_pending = true; ctx.request_repaint(); }
            None => {}
        }
    }
}
