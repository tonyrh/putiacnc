//! The tool palette, the options bar under it, and the overlays (zoom card, hint line).
use putiacnc_core::i18n::{tr};
use super::*;

pub(super) fn tool_options_bar(ctx: &egui::Context, app: &mut App, rect: Rect) {
    if !matches!(app.mode, Mode::Point | Mode::Polygon | Mode::Rect | Mode::Ellipse | Mode::Trim) || app.tab_edit.is_some() || app.node_edit.is_some() { return; }
    let _bar = egui::Area::new(egui::Id::new("tool_options")).order(egui::Order::Middle).fixed_pos(rect.min + Vec2::new(10.0, 62.0 + app.palette_extra)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
          ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
            if app.mode == Mode::Point {
                if ui.selectable_label(!app.point_ring && !app.point_grid, tr("Point")).clicked() { app.point_ring = false; app.point_grid = false; }
                if ui.selectable_label(app.point_ring, tr("Bolt circle")).clicked() { app.point_ring = true; app.point_grid = false; }
                if ui.selectable_label(app.point_grid, tr("Grid")).clicked() { app.point_grid = true; app.point_ring = false; }
                if app.point_grid {
                    ui.add_space(6.0);
                    ui.label(tr("Columns"));
                    ui.add_space(6.0);
                    crate::widgets::num(ui, [44.0, 24.0], &mut app.grid_cols, 1.0, 100.0, 0.2, 0, "");
                    ui.add_space(8.0);
                    ui.label(tr("Rows"));
                    ui.add_space(6.0);
                    crate::widgets::num(ui, [44.0, 24.0], &mut app.grid_rows, 1.0, 100.0, 0.2, 0, "");
                }
                if app.point_ring {
                    ui.add_space(6.0);
                    ui.label(tr("Holes"));
                    ui.add_space(6.0);
                    crate::widgets::num(ui, [44.0, 24.0], &mut app.point_count, 1.0, 200.0, 0.2, 0, "");
                    ui.add_space(8.0);
                    ui.label(tr("Sweep"));
                    ui.add_space(6.0);
                    crate::widgets::num(ui, [56.0, 24.0], &mut app.point_sweep, -360.0, 360.0, 1.0, 0, "");
                    ui.add_space(2.0);
                    ui.weak("°");
                }
            }
            if app.mode == Mode::Polygon {
                let (c, _) = crate::zoomcard::seg2(ui, [tr("Polygon"), tr("Star")], ["", ""], app.poly_star as usize);
                if let Some(i) = c {
                    if i == 0 { app.poly_star = false; if app.poly_sides == 5.0 { app.poly_sides = 6.0; } } else { app.poly_star = true; if app.poly_sides == 6.0 { app.poly_sides = 5.0; } }
                }
                ui.add_space(6.0);
                ui.label(if app.poly_star { tr("Points") } else { tr("Sides") });
                ui.add_space(6.0);
                crate::widgets::num(ui, [44.0, 24.0], &mut app.poly_sides, 3.0, 64.0, 0.2, 0, "");
                if app.poly_star {
                    ui.add_space(6.0);
                    ui.label(tr("Inner"));
                    ui.add_space(6.0);
                    crate::widgets::num(ui, [48.0, 24.0], &mut app.poly_inner, 5.0, 95.0, 0.5, 0, "");
                    ui.add_space(2.0);
                    ui.weak("%");
                }
            }
            if app.mode == Mode::Ellipse {
                let (c, _) = crate::zoomcard::seg2(ui, [tr("Ellipse"), tr("Circle")], [tr("Drag a bounding box"), tr("Drag from the centre, or type the centre and size")], app.circle_mode as usize);
                if let Some(i) = c { app.circle_mode = i == 1; app.dyn_in = None; }
                if app.circle_mode {
                    ui.add_space(8.0);
                    let (c, _) = crate::zoomcard::seg2(ui, [tr("Radius"), tr("Diameter")], ["", ""], app.circle_diam as usize);
                    if let Some(i) = c { app.circle_diam = i == 1; app.dyn_in = None; }
                }
            }
            if app.mode == Mode::Trim {
                let (c, _) = crate::zoomcard::seg2(ui, [tr("Trim"), tr("Extend")], [tr("Remove the part of a shape between the nearest crossings"), tr("Run the end of an open line on to the next shape")], app.trim_extend as usize);
                if let Some(i) = c { app.trim_extend = i == 1; }
            }
            if app.mode == Mode::Rect {
                ui.label(tr("Corner radius"));
                ui.add_space(6.0);
                crate::widgets::len(ui, [56.0, 24.0], &mut app.rect_radius, 0.0, 500.0, 0.1, 2, "");
                ui.add_space(2.0);
                ui.weak(crate::widgets::len_unit(ui));
            }
          });
        });
    });
    app.bars_bottom = app.bars_bottom.max(_bar.response.rect.bottom());
}

// ------------------------------------------------------------------- overlays

/// A thin vertical divider as tall as the buttons (a Separator would stretch the whole area).
pub(super) fn palette_divider(ui: &mut Ui) {
    let (sep, _) = ui.allocate_exact_size(Vec2::new(8.0, 36.0), egui::Sense::hover());
    ui.painter().line_segment([sep.center_top() + Vec2::new(0.0, 6.0), sep.center_bottom() - Vec2::new(0.0, 6.0)], ui.visuals().widgets.noninteractive.bg_stroke);
}

/// One group of palette buttons: 0 Select and Edit points, 1 the drawing tools, 2 modify the selection, 3 undo and redo.
pub(super) fn palette_group(ui: &mut Ui, app: &mut App, g: usize, fillet: bool) {
    let tools: &[(Mode, Icon, &str)] = match g {
        0 => &[(Mode::Select, Icon::Select, tr("Select / move (V)")), (Mode::Node, Icon::Node, tr("Edit points (N)"))],
        1 => &[(Mode::Line, Icon::Line, tr("Line / polyline (L)")), (Mode::Rect, Icon::Rect, tr("Rectangle (R, Shift = square)")), (Mode::Ellipse, Icon::Ellipse, tr("Ellipse / circle (E, Shift = circle)")), (Mode::Polygon, Icon::Polygon, tr("Polygon or star (P)")), (Mode::Point, Icon::Point, tr("Drill point (D): click to place, use with a Drill operation")), (Mode::Text, Icon::Text, tr("Text (T)")), (Mode::Trim, Icon::Trim, tr("Trim and extend (X): click the part of a line to remove, or switch to Extend to run a line out to the next shape"))],
        _ => &[],
    };
    for &(m, icon, tip) in tools {
        if icon_button(ui, icon, app.mode == m, true, tip).clicked() { set_mode(app, m); }
    }
    if g == 2 {
        if icon_button(ui, Icon::Combine, app.combine_open, app.sel.len() >= 2, tr("Combine the selected shapes: union, subtract, intersect, exclude")).clicked() { app.toggle_tool_window(crate::app::ToolWin::Combine); }
        crate::align::button(ui, app);
        // only offered when the selection has a corner to round (not for circles and ellipses)
        if icon_button(ui, Icon::Fillet, app.corners.open, fillet, tr("Round or bevel the corners of the selected shapes: fillet and chamfer (select a shape with corners)")).clicked() { app.toggle_tool_window(crate::app::ToolWin::Corners); }
        if icon_button(ui, Icon::Offset, app.offset_open, !app.sel.is_empty(), tr("Offset: a copy of the selection grown or shrunk by a distance")).clicked() { app.toggle_tool_window(crate::app::ToolWin::Offset); }
        if icon_button(ui, Icon::Array, app.array.open, !app.sel.is_empty(), tr("Array and copy: rows and columns, a circle of copies, or a spacing along a line")).clicked() { app.toggle_tool_window(crate::app::ToolWin::Array); }
    }
    if g == 3 {
        if icon_button(ui, Icon::Undo, false, app.hist.can_undo(crate::app::Scope::Drawing), tr("Undo drawing change (Edit menu: undo anything)")).clicked() { app.do_undo_drawing(); }
        if icon_button(ui, Icon::Redo, false, app.hist.can_redo(crate::app::Scope::Drawing), tr("Redo drawing change (Edit menu: redo anything)")).clicked() { app.do_redo_drawing(); }
    }
}

pub(super) fn overlays(ctx: &egui::Context, app: &mut App, rect: Rect, zoom_frac: f32) {
    app.canvas_right = rect.right();
    app.canvas_left = rect.left();
    // tool palette
    let pal = egui::Area::new(egui::Id::new("palette")).order(egui::Order::Middle).fixed_pos(rect.min + Vec2::new(10.0, 10.0)).show(ctx, |ui| {
        // a narrow canvas wraps the palette onto a second row instead of running under the side panel
        ui.set_max_width((rect.width() - 28.0 - crate::zoomcard::CARD_W - 12.0).max(240.0));
        egui::Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
          ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
          // groups: Select and Edit points | the drawing tools | modify the selection | undo and redo. A row breaks only between groups.
          let avail = ui.available_width();
          let fillet = app.selection_has_corners();
          let n_mod = 5.0;
          let widths = [2.0 * 36.0 + 2.0, 7.0 * 36.0 + 12.0, n_mod * 36.0 + (n_mod - 1.0) * 2.0, 2.0 * 30.0 + 2.0];
          let mut rows: Vec<Vec<usize>> = vec![vec![]];
          let mut used = 0.0_f32;
          for (g, w) in widths.iter().enumerate() {
              let first = rows.last().is_some_and(|r| r.is_empty());
              let need = *w + if first { 0.0 } else { 12.0 };
              if !first && used + need > avail { rows.push(vec![]); used = 0.0; }
              used += if rows.last().is_some_and(|r| r.is_empty()) { *w } else { need };
              if let Some(r) = rows.last_mut() { r.push(g); }
          }
          for row in rows {
              ui.horizontal(|ui| {
                  for (i, g) in row.into_iter().enumerate() {
                      if i > 0 { palette_divider(ui); }
                      palette_group(ui, app, g, fillet);
                  }
              });
          }
        });
    });
    // the bars under the palette move down when it wraps (46 px is one row)
    crate::tour::note(ctx, "tools", pal.response.rect);
    app.palette_extra = (pal.response.rect.height() - 46.0).max(0.0);
    // where the tool dialogs open: under the palette and whatever option bar is showing, at the same left edge
    app.bars_bottom = pal.response.rect.bottom();
    // the zoom card never moves: top right, level with the first palette row (the palette leaves room for it)
    let card_y = rect.top() + 10.0;
    let (zact, _) = crate::zoomcard::show(ctx, "zoomcard2d", egui::Order::Middle, Pos2::new(rect.right() - 10.0, card_y), zoom_frac);
    if zact.is_some() { app.zoom_act = zact; }
    tool_options_bar(ctx, app, rect);
    if app.tab_edit.is_some() { tab_bar(ctx, app, rect); } else if app.node_edit.is_some() { /* the Edit points window */ } else { selection_bar(ctx, app, rect); }
    // hint line
    let hint = if app.node_edit.is_some() { tr("Drag a point \u{b7} click an edge to add one \u{b7} Del removes") } else if app.tab_edit.is_some() { tr("Click an outline to add a tab \u{b7} right-click removes one") } else { match app.mode {
        Mode::Select if app.selected_text().is_some() => tr("Double-click: edit the text \u{b7} drag to move \u{b7} Right-drag: pan"),
        Mode::Select => tr("Click or box-select \u{b7} drag to move \u{b7} Right-drag: pan"),
        Mode::Node => tr("Click a shape to edit its points"),
        Mode::Line => tr("Click points \u{b7} Enter ends \u{b7} C closes \u{b7} Esc cancels"),
        Mode::Rect => tr("Drag to draw a rectangle \u{b7} Shift for a square"),
        Mode::Ellipse => if app.circle_mode { tr("Drag from the centre \u{b7} or type centre X, Y and size") } else { tr("Drag a bounding box \u{b7} Shift for a circle") },
        Mode::Point => tr("Click to place a drill point \u{b7} Grid and Bolt circle above"),
        Mode::Polygon => tr("Drag from the centre \u{b7} Shift points it up \u{b7} sides above"),
        Mode::Text => tr("Type the text, then click where it starts"),
        Mode::Trim => if app.trim_extend { tr("Click near an open end to run it on to the next shape") } else { tr("Click the part to remove, up to the nearest crossings") },
    } };
    if app.trace.open || app.clean.open { return; } // a dialog covers that part of the canvas
    let hint = if app.array.open && app.array.picking { tr("Click the line or curve the copies should follow") } else { hint };
    let hint = if app.dyn_in.is_some() { tr("Typing values: Enter confirms \u{b7} Tab next field \u{b7} Esc cancels") } else { hint };
    egui::Area::new(egui::Id::new("hint")).order(egui::Order::Middle).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(rect.left_bottom() + Vec2::new(12.0, -28.0)).interactable(false).show(ctx, |ui| {
        ui.set_max_width((rect.width() - 24.0).max(120.0));
        ui.add(egui::Label::new(egui::RichText::new(hint).weak()).truncate(true));
    });
}

/// The right-click menu of the selection: the commands used most while shapes are selected, the same names as in the menus.
/// Commands that do not apply are left out, not greyed.
/// A row that opens a small menu of choices at its right (a drawn arrow marks it: the glyph is not in the font); returns the choice clicked.
fn submenu(ctx: &egui::Context, ui: &mut Ui, key: &str, label: &str, items: &[&str]) -> Option<usize> {
    let m = ui.add(egui::Button::new(label));
    let (r, c) = (m.rect, ui.visuals().widgets.inactive.fg_stroke.color);
    let (x, y) = (r.right() - 10.0, r.center().y);
    ui.painter().add(egui::Shape::convex_polygon(vec![Pos2::new(x - 3.0, y - 4.0), Pos2::new(x - 3.0, y + 4.0), Pos2::new(x + 2.5, y)], c, Stroke::NONE));
    let sub_id = egui::Id::new(("sub_open", key));
    let mut open = ctx.data(|d| d.get_temp::<bool>(sub_id)).unwrap_or(false) || m.hovered() || m.clicked();
    let mut chosen = None;
    if open {
        let w = items.iter().map(|s| crate::panels::tw(ui, s)).fold(0.0_f32, f32::max) + 28.0;
        let sub = egui::Area::new(egui::Id::new(("sub_area", key))).order(egui::Order::Foreground).fixed_pos(Pos2::new(r.right() - 2.0, r.top() - 6.0)).show(ctx, |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.style_mut().wrap = Some(false);
                { let w = &mut ui.style_mut().visuals.widgets; w.inactive.weak_bg_fill = egui::Color32::TRANSPARENT; w.inactive.bg_stroke = egui::Stroke::NONE; }
                // a justified column takes all the width it is offered: give it just enough
                ui.set_width(w.max(100.0));
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    for (i, t) in items.iter().enumerate() { if ui.button(*t).clicked() { chosen = Some(i); } }
                });
            });
        });
        let at = ctx.input(|i| i.pointer.hover_pos());
        open = at.is_some_and(|p| r.expand(1.0).contains(p) || sub.response.rect.expand(2.0).contains(p));
    }
    ctx.data_mut(|d| d.insert_temp(sub_id, open));
    chosen
}

pub(super) fn selection_menu(ctx: &egui::Context, app: &mut App) {
    let Some(pos) = app.ctx_menu else { return };
    if app.sel.is_empty() || app.mode != Mode::Select { app.ctx_menu = None; return; }
    use crate::app::ToolWin;
    let locked = app.sel_locked();
    let mut close = false;
    let area = egui::Area::new(egui::Id::new("selection_menu")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::menu(ui.style()).show(ui, |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(190.0);
            // plain rows like the menu bar's menus: no frame, the whole row is the button
            { let w = &mut ui.style_mut().visuals.widgets; w.inactive.weak_bg_fill = egui::Color32::TRANSPARENT; w.inactive.bg_stroke = egui::Stroke::NONE; }
            ui.set_width(ui.min_rect().width().max(190.0));
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            let row = |ui: &mut Ui, label: &str, key: &str| ui.add(egui::Button::new(label).shortcut_text(key)).clicked();
            if row(ui, tr("Duplicate"), "Ctrl+D") { app.duplicate_selection(); close = true; }
            if row(ui, tr("Delete"), "Del") { app.delete_selection(); close = true; }
            if putiacnc_core::selection::can_group(&app.project.data, &app.sel) && row(ui, tr("Group"), "Ctrl+G") { app.group_selection(); close = true; }
            if app.sel_in_group() && row(ui, tr("Ungroup"), "Ctrl+Shift+G") { app.ungroup_selection(); close = true; }
            ui.separator();
            if !locked {
                match submenu(ctx, ui, "mirror", tr("Mirror"), &[tr("Left to right"), tr("Top to bottom")]) {
                    Some(0) => { app.mirror_selection(true); close = true; }
                    Some(1) => { app.mirror_selection(false); close = true; }
                    _ => {}
                }
                // with more than one layer: move the selection to one of them
                let layers: Vec<(i64, String)> = app.project.data.all_layers().iter().map(|l| (l.id, l.name.clone())).collect();
                if layers.len() > 1 {
                    let names: Vec<&str> = layers.iter().map(|(_, n)| n.as_str()).collect();
                    if let Some(k) = submenu(ctx, ui, "layer", tr("Move to layer"), &names) { app.move_selection_to_layer(layers[k].0); close = true; }
                }
                if row(ui, tr("Align…"), "") { app.open_tool_window(ToolWin::Align); close = true; }
            }
            if row(ui, tr("Array and copy…"), "") { app.open_tool_window(ToolWin::Array); close = true; }
            ui.separator();
            if row(ui, tr("Offset…"), "") { app.open_tool_window(ToolWin::Offset); close = true; }
            if app.selection_has_corners() && row(ui, tr("Fillet and chamfer…"), "") { app.open_tool_window(ToolWin::Corners); close = true; }
            if app.sel.len() >= 2 && row(ui, tr("Combine shapes…"), "") { app.open_tool_window(ToolWin::Combine); close = true; }
            if app.sel.len() == 1 || app.closed_selection() > 0 { ui.separator(); }
            if app.sel.len() == 1 && row(ui, tr("Edit points"), "N") { app.choose_tool(Mode::Node); close = true; }
            if app.closed_selection() > 0 && row(ui, tr("Relief from shapes…"), "") { app.open_tool_window(ToolWin::Relief); close = true; }
            });
        });
    });
    // a click anywhere else, or Esc, closes it
    let sub_rect = ["mirror", "layer"].iter().filter_map(|k| ctx.memory(|m| m.area_rect(egui::Id::new(("sub_area", *k))))).reduce(|a, b| a.union(b));
    let outside = ctx.input(|i| i.pointer.any_pressed()) && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p) && !sub_rect.is_some_and(|r| r.contains(p)) && p != pos);
    if close || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { app.ctx_menu = None; }
}
