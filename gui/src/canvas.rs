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
    if import { app.import_svg_dialog(); }
    if export { app.export_gcode_dialog(); }
    if quit { app.request(Action::Quit); }
    if save { app.save(); }
    if undo { app.do_undo(); }
    if redo || redo2 { app.do_redo(); }
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
    let (v, l, r, e, del, esc, enter, c, up, down, left, right, shift) = ctx.input(|i| {
        let free = !i.modifiers.command && !i.modifiers.alt;
        (
            free && i.key_pressed(Key::V), free && i.key_pressed(Key::L), free && i.key_pressed(Key::R), free && i.key_pressed(Key::E),
            free && (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)),
            i.key_pressed(Key::Escape), i.key_pressed(Key::Enter), free && i.key_pressed(Key::C),
            i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight),
            i.modifiers.shift,
        )
    });
    for (hit, m) in [(v, Mode::Select), (l, Mode::Line), (r, Mode::Rect), (e, Mode::Ellipse)] {
        if hit { set_mode(app, m); }
    }
    if app.mode == Mode::Line && !app.poly.is_empty() {
        if enter { finish_poly(app, false); }
        else if c { finish_poly(app, true); }
    }
    if esc {
        if !app.poly.is_empty() { app.poly.clear(); }
        else if !matches!(app.drag, Drag::None) { app.drag = Drag::None; }
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
    let view = View { scale: app.scale, offset: app.offset, origin: rect.min };
    let pointer_w = ctx.input(|i| i.pointer.latest_pos()).map(|p| view.s2w(p));
    let hover_w = resp.hover_pos().map(|p| view.s2w(p));
    let shift = ctx.input(|i| i.modifiers.shift);
    let tol = 6.0 / app.scale as f64;
    let prim = PointerButton::Primary;

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
                let sel_bb = shapes::bbox_many(app.sel.iter().filter_map(|&i| app.project.data.loops.get(i)));
                if let (Some(ps), Some(b)) = (ctx.input(|i| i.pointer.press_origin()), sel_bb) {
                    if let Some(h) = hit_handle(&view, b, ps) {
                        let w = view.s2w(ps);
                        let hw = handle_world(b, h);
                        app.drag = Drag::Resize {
                            handle: h, start_bbox: b, grab: [hw[0] - w[0], hw[1] - w[1]],
                            orig: app.sel.iter().map(|&i| (i, app.project.data.loops[i].clone())).collect(),
                        };
                    }
                }
                if let (Some(w), false) = (ctx.input(|i| i.pointer.press_origin()).map(|p| view.s2w(p)), matches!(app.drag, Drag::Resize { .. })) {
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
                            app.drag = Drag::Move { start: w, anchor: [b[0], b[1]], applied: [0.0, 0.0] };
                        }
                        None => app.drag = Drag::Box { start: w },
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
                if let (Drag::Move { start, anchor, applied }, Some(cur)) = (&app.drag, pointer_w) {
                    let (start, anchor, applied) = (*start, *anchor, *applied);
                    let t = app.snap([anchor[0] + cur[0] - start[0], anchor[1] + cur[1] - start[1]]);
                    let desired = [t[0] - anchor[0], t[1] - anchor[1]];
                    let step = [desired[0] - applied[0], desired[1] - applied[1]];
                    if step[0].abs() > 1e-9 || step[1].abs() > 1e-9 {
                        app.hist.record_once(&app.project.data);
                        for &i in &app.sel { shapes::translate(&mut app.project.data.loops[i], step[0], step[1]); }
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
                }
                app.drag = Drag::None;
            }
            if resp.double_clicked_by(prim) {
                if let Some(h) = hover_w.and_then(|w| shapes::pick(&app.project.data.loops, w, tol)) { enter_node_edit(app, h); }
            }
            if resp.clicked_by(prim) && !resp.double_clicked() {
                if let Some(w) = hover_w {
                    let hit = shapes::pick(&app.project.data.loops, w, tol);
                    match hit {
                        Some(h) => {
                            if shift { if !app.sel.remove(&h) { app.sel.insert(h); } }
                            else { app.sel.clear(); app.sel.insert(h); }
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
        Some(Mode::Rect) | Some(Mode::Ellipse) => {
            if resp.drag_started_by(prim) {
                if let Some(w) = ctx.input(|i| i.pointer.press_origin()).map(|p| view.s2w(p)) {
                    app.drag = Drag::Draw { start: app.snap(w) };
                }
            }
            if resp.drag_stopped_by(prim) {
                if let (Drag::Draw { start }, Some(cur)) = (&app.drag, pointer_w) {
                    let (a, b) = (*start, draw_end(app, *start, app.snap(cur), shift));
                    if (b[0] - a[0]).abs() > 0.01 && (b[1] - a[1]).abs() > 0.01 {
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

    if app.mode == Mode::Select && editing_tabs.is_none() && editing_nodes.is_none() {
        if let Drag::Resize { handle, .. } = &app.drag { ctx.set_cursor_icon(handle_cursor(*handle)); }
        else if matches!(app.drag, Drag::None) {
            let sel_bb = shapes::bbox_many(app.sel.iter().filter_map(|&i| app.project.data.loops.get(i)));
            if let (Some(ps), Some(b)) = (ctx.input(|i| i.pointer.latest_pos()), sel_bb) {
                if let Some(h) = hit_handle(&view, b, ps) { ctx.set_cursor_icon(handle_cursor(h)); }
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
    painter.rect_filled(sr, 0.0, th.stock_fill());
    painter.rect_stroke(sr, 0.0, Stroke::new(1.5_f32, th.stock_stroke()));
    let (zx, zy) = match d.stock.zr.as_str() { "lr" => (sw, 0.0), "ul" => (0.0, sh), "ur" => (sw, sh), "c" => (sw / 2.0, sh / 2.0), _ => (0.0, 0.0) };
    let z = to_s([zx, zy]);
    painter.line_segment([z - Vec2::new(8.0, 0.0), z + Vec2::new(8.0, 0.0)], Stroke::new(2.0_f32, Color32::RED));
    painter.line_segment([z - Vec2::new(0.0, 8.0), z + Vec2::new(0.0, 8.0)], Stroke::new(2.0_f32, Color32::GREEN));

    let act_idx = d.ops.iter().position(|o| o.id == d.act);
    for (i, l) in d.loops.iter().enumerate() {
        let opi = d.ops.iter().position(|o| o.id == l.op);
        let selected = app.sel.contains(&i);
        let col = if selected { th.shape_selected() } else { opi.map(|k| op_color(k, dark)).unwrap_or(th.shape_idle()) };
        let mut pts: Vec<Pos2> = l.pts.chunks_exact(2).map(|c| to_s([c[0] as f64 / shapes::S, c[1] as f64 / shapes::S])).collect();
        if l.closed() { if let Some(f) = pts.first().copied() { pts.push(f); } }
        painter.add(egui::Shape::line(pts, Stroke::new(if editing_nodes == Some(i) { 3.0_f32 } else if selected { 2.5_f32 } else { 1.8_f32 }, if editing_nodes == Some(i) { th.accent } else { col })));
    }
    if let Some(b) = shapes::bbox_many(app.sel.iter().filter_map(|&i| d.loops.get(i))).filter(|_| editing_nodes.is_none()) {
        let r = Rect::from_two_pos(to_s([b[0], b[1]]), to_s([b[2], b[3]])).expand(4.0);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, th.shape_selected().gamma_multiply(0.3)));
        if app.mode == Mode::Select {
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
    if let (Drag::Draw { start }, Some(cur)) = (&app.drag, pointer_w) {
        let a = *start;
        let b = draw_end(app, a, app.snap(cur), shift);
        let l = if app.mode == Mode::Rect { shapes::new_rect(a, b, app.rect_radius, 0) } else { shapes::new_ellipse(a, b, 0) };
        let mut pts: Vec<Pos2> = l.pts.chunks_exact(2).map(|c| to_s([c[0] as f64 / shapes::S, c[1] as f64 / shapes::S])).collect();
        if let Some(f) = pts.first().copied() { pts.push(f); }
        painter.add(egui::Shape::line(pts, Stroke::new(2.0_f32, prev_col)));
        hud = Some(format!("{:.2} x {:.2} mm", (b[0] - a[0]).abs(), (b[1] - a[1]).abs()));
    }
    if let Drag::Box { start } = &app.drag {
        if let Some(cur) = pointer_w {
            let r = Rect::from_two_pos(to_s(*start), to_s(cur));
            painter.rect_filled(r, 0.0, th.accent.gamma_multiply(0.15));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, th.accent));
        }
    }
    if app.mode == Mode::Line && !app.poly.is_empty() {
        let mut pts: Vec<Pos2> = app.poly.iter().map(|p| to_s(*p)).collect();
        if let Some(h) = hover_w {
            let c = app.snap(h);
            let last = *app.poly.last().unwrap();
            hud = Some(format!("{:.2} mm", (c[0] - last[0]).hypot(c[1] - last[1])));
            pts.push(to_s(c));
        }
        painter.add(egui::Shape::line(pts, Stroke::new(2.0_f32, prev_col)));
        for p in &app.poly { painter.circle_filled(to_s(*p), 3.0, prev_col); }
    }
    if let Some(h) = hover_w {
        let c = app.snap(h);
        if app.mode != Mode::Select && app.mode != Mode::Node { painter.circle_stroke(to_s(c), 5.0, Stroke::new(1.0_f32, th.shape_selected())); }
        if let Some(t) = hud {
            painter.text(to_s(c) + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, t, egui::FontId::monospace(12.0), th.hud());
        }
        painter.text(rect.right_bottom() + Vec2::new(-10.0, -8.0), egui::Align2::RIGHT_BOTTOM,
            format!("X {:.2}  Y {:.2} mm", c[0], c[1]), egui::FontId::monospace(12.0), th.hud().gamma_multiply(0.6));
    }
    if !app.tools_open && !app.machines_open { overlays(&ctx, app, rect); }
}

/// Second corner of a rectangle/ellipse drag; Shift forces a square (circle) for ellipses.
fn draw_end(app: &App, a: [f64; 2], b: [f64; 2], shift: bool) -> [f64; 2] {
    if app.mode == Mode::Ellipse && shift {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let m = dx.abs().max(dy.abs());
        [a[0] + m * if dx < 0.0 { -1.0 } else { 1.0 }, a[1] + m * if dy < 0.0 { -1.0 } else { 1.0 }]
    } else { b }
}

// ------------------------------------------------------------------- overlays

fn overlays(ctx: &egui::Context, app: &mut App, rect: Rect) {
    // tool palette
    egui::Area::new(egui::Id::new("palette")).order(egui::Order::Foreground).fixed_pos(rect.min + Vec2::new(10.0, 10.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
          ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
            for (m, icon, tip) in [
                (Mode::Select, Icon::Select, "Select / move (V)"), (Mode::Node, Icon::Node, "Edit points (N)"), (Mode::Line, Icon::Line, "Line / polyline (L)"),
                (Mode::Rect, Icon::Rect, "Rectangle (R)"), (Mode::Ellipse, Icon::Ellipse, "Ellipse / circle (E, Shift = circle)"),
            ] {
                if icon_button(ui, icon, app.mode == m, true, tip).clicked() { set_mode(app, m); }
            }
            // thin vertical dividers as tall as the buttons (a Separator would stretch the whole area)
            let divider = |ui: &mut Ui| {
                let (sep, _) = ui.allocate_exact_size(Vec2::new(8.0, 36.0), egui::Sense::hover());
                ui.painter().line_segment([sep.center_top() + Vec2::new(0.0, 6.0), sep.center_bottom() - Vec2::new(0.0, 6.0)], ui.visuals().widgets.noninteractive.bg_stroke);
            };
            divider(ui);
            if icon_button(ui, Icon::Undo, false, !app.hist.undo.is_empty(), "Undo (Ctrl+Z)").clicked() { app.do_undo(); }
            if icon_button(ui, Icon::Redo, false, !app.hist.redo.is_empty(), "Redo (Ctrl+Shift+Z)").clicked() { app.do_redo(); }
            divider(ui);
            if icon_button(ui, Icon::Fit, false, true, "Fit the stock and work area in view (F)").clicked() { app.fit_pending = true; }
            if app.mode == Mode::Rect {
                divider(ui);
                ui.label("Radius");
                crate::widgets::num(ui, [48.0, 24.0], &mut app.rect_radius, 0.0, 500.0, 0.1, 3, "");
            }
          });
        });
    });
    if app.tab_edit.is_some() { tab_bar(ctx, app, rect); } else if app.node_edit.is_some() { node_bar(ctx, app, rect); } else { selection_bar(ctx, app, rect); }
    // hint line
    let hint = if app.node_edit.is_some() { "Drag a point to move it, click an edge to add one, drag a box to select several. Del removes the selected points, arrows nudge them. Esc or Done finishes." } else if app.tab_edit.is_some() { "Click on an outline to add a tab. Drag a tab to move it, right-click it to remove. Esc or Done finishes." } else { match app.mode {
        Mode::Select => "Click or box-select, drag to move, drag the handles to resize (Shift keeps proportions, Alt scales from the centre). Double-click or N edits points. Del deletes. Middle/right-drag pans.",
        Mode::Node => "Click a shape to edit its points.",
        Mode::Line => "Click points. Double-click, Enter or right-click ends. Click the first point or press C to close. Esc cancels.",
        Mode::Rect => "Drag to draw a rectangle.",
        Mode::Ellipse => "Drag a bounding box. Hold Shift for a circle.",
    } };
    egui::Area::new(egui::Id::new("hint")).order(egui::Order::Foreground).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(rect.left_bottom() + Vec2::new(12.0, -10.0)).interactable(false).show(ctx, |ui| {
        ui.set_max_width((rect.width() - 220.0).max(120.0));
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
    let d = &mut app.project.data;
    let Some(b) = shapes::bbox_many(sel.iter().map(|&i| &d.loops[i])) else { return };
    let (mut x, mut y, mut w, mut h) = (b[0], b[1], b[2] - b[0], b[3] - b[1]);
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
    let mut align: Option<u8> = None;
    let mut align_each: Option<u8> = None;
    let _ = n;
    egui::Area::new(egui::Id::new("selbar")).order(egui::Order::Foreground).fixed_pos(rect.min + Vec2::new(10.0, 66.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(8.0, 5.0)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let field = |ui: &mut Ui, label: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
                    ui.label(label);
                    crate::widgets::num(ui, [66.0, 24.0], v, lo, hi, 0.25, 3, "").changed()
                };
                geom |= field(ui, "X", &mut x, -100000.0, 100000.0);
                ui.add_space(4.0);
                geom |= field(ui, "Y", &mut y, -100000.0, 100000.0);
                ui.add_space(4.0);
                geom |= field(ui, "W", &mut w, 0.01, 100000.0);
                ui.add_space(4.0);
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
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label("Align to stock");
                ui.add_space(4.0);
                for (k, icon, tip) in [
                    (0, Icon::AlignL, "Align to the left edge of the stock"), (1, Icon::AlignHC, "Centre horizontally on the stock"), (2, Icon::AlignR, "Align to the right edge of the stock"),
                    (3, Icon::AlignT, "Align to the top edge of the stock"), (4, Icon::AlignVC, "Centre vertically on the stock"), (5, Icon::AlignB, "Align to the bottom edge of the stock"),
                    (6, Icon::CenterBoth, "Centre on the stock"),
                ] {
                    if k == 3 || k == 6 { ui.add_space(6.0); }
                    if icon_button(ui, icon, false, true, tip).clicked() { align = Some(k); }
                }
            });
            if sel.len() >= 2 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let lw = crate::panels::tw(ui, "Align to stock");
                    ui.add_sized([lw, 24.0], egui::Label::new("Align shapes"));
                    ui.add_space(4.0);
                    for (k, icon, tip) in [
                        (0, Icon::AlignL, "Align the shapes' left edges"), (1, Icon::AlignHC, "Centre the shapes horizontally on each other"), (2, Icon::AlignR, "Align the shapes' right edges"),
                        (3, Icon::AlignT, "Align the shapes' top edges"), (4, Icon::AlignVC, "Centre the shapes vertically on each other"), (5, Icon::AlignB, "Align the shapes' bottom edges"),
                        (6, Icon::DistH, "Distribute horizontally (equal gaps, needs 3 or more)"), (7, Icon::DistV, "Distribute vertically (equal gaps, needs 3 or more)"),
                    ] {
                        if k == 3 || k == 6 { ui.add_space(6.0); }
                        let on = k < 6 || sel.len() >= 3;
                        if icon_button(ui, icon, false, on, tip).clicked() { align_each = Some(k); }
                    }
                });
            }
        });
    });
    if let Some(k) = align {
        let (sw, sh) = (d.stock.w, d.stock.h);
        let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
        let dx = match k { 0 => -b[0], 1 | 6 => sw / 2.0 - cx, 2 => sw - b[2], _ => 0.0 };
        let dy = match k { 3 => sh - b[3], 4 | 6 => sh / 2.0 - cy, 5 => -b[1], _ => 0.0 };
        for &i in sel.iter() { shapes::translate(&mut d.loops[i], dx, dy); }
        changed = true;
    }
    if geom {
        let mut refs: Vec<&mut putiacnc_core::model::LoopData> = d.loops.iter_mut().enumerate().filter(|(i, _)| sel.contains(i)).map(|(_, l)| l).collect();
        shapes::fit_to_bbox(&mut refs, b, [x, y, x + w.max(0.01), y + h.max(0.01)]);
        changed = true;
    }
    if radius_changed {
        if let Some(&i) = sel.iter().next() { shapes::set_corner_radius(&mut d.loops[i], radius); changed = true; }
    }
    if let Some(k) = align_each {
        let ids: Vec<usize> = sel.iter().copied().collect();
        if k < 6 {
            for &i in &ids {
                let Some(lb) = shapes::bbox(&d.loops[i]) else { continue };
                let dx = match k { 0 => b[0] - lb[0], 1 => (b[0] + b[2]) / 2.0 - (lb[0] + lb[2]) / 2.0, 2 => b[2] - lb[2], _ => 0.0 };
                let dy = match k { 3 => b[3] - lb[3], 4 => (b[1] + b[3]) / 2.0 - (lb[1] + lb[3]) / 2.0, 5 => b[1] - lb[1], _ => 0.0 };
                shapes::translate(&mut d.loops[i], dx, dy);
            }
        } else {
            // equal gaps between the shapes, keeping the outermost two where they are
            let horiz = k == 6;
            let ax = |bb: [f64; 4]| if horiz { (bb[0], bb[2]) } else { (bb[1], bb[3]) };
            let mut items: Vec<(usize, f64, f64)> = ids.iter().filter_map(|&i| shapes::bbox(&d.loops[i]).map(|bb| { let (lo, hi) = ax(bb); (i, lo, hi) })).collect();
            items.sort_by(|p, q| (p.1 + p.2).partial_cmp(&(q.1 + q.2)).unwrap());
            if items.len() >= 3 {
                let (lo, hi) = (b_axis(b, horiz).0, b_axis(b, horiz).1);
                let total: f64 = items.iter().map(|t| t.2 - t.1).sum();
                let gap = (hi - lo - total) / (items.len() - 1) as f64;
                let mut pos = lo;
                for (i, l0, l1) in items {
                    let delta = pos - l0;
                    if horiz { shapes::translate(&mut d.loops[i], delta, 0.0); } else { shapes::translate(&mut d.loops[i], 0.0, delta); }
                    pos += (l1 - l0) + gap;
                }
            }
        }
        changed = true;
    }
    if let Some(id) = pick { for &i in sel.iter() { d.loops[i].op = id; } changed = true; }
    if changed {
        if !app.hist.open {
            app.hist.undo.push(before);
            if app.hist.undo.len() > 60 { app.hist.undo.remove(0); }
            app.hist.redo.clear();
            app.hist.open = true;
        }
        app.touch();
    }
}

fn b_axis(b: [f64; 4], horiz: bool) -> (f64, f64) { if horiz { (b[0], b[2]) } else { (b[1], b[3]) } }

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
    egui::Area::new(egui::Id::new("tabbar")).order(egui::Order::Foreground).fixed_pos(rect.min + Vec2::new(10.0, 66.0)).show(ctx, |ui| {
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
    if m == Mode::Node && app.sel.len() == 1 { let i = *app.sel.iter().next().unwrap(); enter_node_edit(app, i); }
}

fn enter_node_edit(app: &mut App, li: usize) {
    if li >= app.project.data.loops.len() { return; }
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
        [(Key::V, Mode::Select), (Key::L, Mode::Line), (Key::R, Mode::Rect), (Key::E, Mode::Ellipse)].into_iter().find(|(k, _)| free && i.key_pressed(*k)).map(|(_, m)| m)
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
    let (mut done, mut simplify) = (false, false);
    let mut tol = app.node_tol;
    egui::Area::new(egui::Id::new("nodebar")).order(egui::Order::Foreground).fixed_pos(rect.min + Vec2::new(10.0, 66.0)).show(ctx, |ui| {
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
            if !plain { ui.weak("Changing a point turns this rectangle or ellipse into a plain polygon."); }
            if !app.node_msg.is_empty() { ui.weak(app.node_msg.as_str()); }
        });
    });
    app.node_tol = tol;
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
