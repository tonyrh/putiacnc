//! The tool palette, the options bar under it, and the overlays (zoom card, hint line).
use super::*;

pub(super) fn tool_options_bar(ctx: &egui::Context, app: &mut App, rect: Rect) {
    if !matches!(app.mode, Mode::Point | Mode::Polygon | Mode::Rect | Mode::Ellipse | Mode::Trim) || app.tab_edit.is_some() || app.node_edit.is_some() { return; }
    let _bar = egui::Area::new(egui::Id::new("tool_options")).order(egui::Order::Middle).fixed_pos(rect.min + Vec2::new(10.0, 62.0 + app.palette_extra)).show(ctx, |ui| {
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
                ui.label("Corner radius");
                ui.add_space(6.0);
                crate::widgets::num(ui, [56.0, 24.0], &mut app.rect_radius, 0.0, 500.0, 0.1, 2, "");
                ui.add_space(2.0);
                ui.weak("mm");
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
        0 => &[(Mode::Select, Icon::Select, "Select / move (V)"), (Mode::Node, Icon::Node, "Edit points (N)")],
        1 => &[(Mode::Line, Icon::Line, "Line / polyline (L)"), (Mode::Rect, Icon::Rect, "Rectangle (R, Shift = square)"), (Mode::Ellipse, Icon::Ellipse, "Ellipse / circle (E, Shift = circle)"), (Mode::Polygon, Icon::Polygon, "Polygon or star (P)"), (Mode::Point, Icon::Point, "Drill point (D): click to place, use with a Drill operation"), (Mode::Text, Icon::Text, "Text (T)"), (Mode::Trim, Icon::Trim, "Trim and extend (X): click the part of a line to remove, or switch to Extend to run a line out to the next shape")],
        _ => &[],
    };
    for &(m, icon, tip) in tools {
        if icon_button(ui, icon, app.mode == m, true, tip).clicked() { set_mode(app, m); }
    }
    if g == 2 {
        if icon_button(ui, Icon::Combine, app.combine_open, app.sel.len() >= 2, "Combine the selected shapes: union, subtract, intersect, exclude").clicked() { app.combine_open = !app.combine_open; }
        crate::align::button(ui, app);
        // only offered when the selection has a corner to round (not for circles and ellipses)
        if fillet && icon_button(ui, Icon::Fillet, app.corners.open, true, "Round or bevel the corners of the selected shapes: fillet and chamfer").clicked() { app.corners.open = !app.corners.open; }
        if icon_button(ui, Icon::Offset, app.offset_open, !app.sel.is_empty(), "Offset: a copy of the selection grown or shrunk by a distance").clicked() { app.offset_open = !app.offset_open; }
        if icon_button(ui, Icon::Array, app.array.open, !app.sel.is_empty(), "Array and copy: rows and columns, a circle of copies, or a spacing along a line").clicked() { app.array.open = !app.array.open; }
    }
    if g == 3 {
        if icon_button(ui, Icon::Undo, false, app.hist.can_undo(crate::app::Scope::Drawing), "Undo drawing change (Edit menu: undo anything)").clicked() { app.do_undo_drawing(); }
        if icon_button(ui, Icon::Redo, false, app.hist.can_redo(crate::app::Scope::Drawing), "Redo drawing change (Edit menu: redo anything)").clicked() { app.do_redo_drawing(); }
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
          let n_mod = if fillet { 5.0 } else { 4.0 };
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
    app.palette_extra = (pal.response.rect.height() - 46.0).max(0.0);
    // where the tool dialogs open: under the palette and whatever option bar is showing, at the same left edge
    app.bars_bottom = pal.response.rect.bottom();
    // the zoom card never moves: top right, level with the first palette row (the palette leaves room for it)
    let card_y = rect.top() + 10.0;
    let (zact, _) = crate::zoomcard::show(ctx, "zoomcard2d", egui::Order::Middle, Pos2::new(rect.right() - 10.0, card_y), zoom_frac);
    if zact.is_some() { app.zoom_act = zact; }
    tool_options_bar(ctx, app, rect);
    if app.tab_edit.is_some() { tab_bar(ctx, app, rect); } else if app.node_edit.is_some() { node_bar(ctx, app, rect); } else { selection_bar(ctx, app, rect); }
    // hint line
    let hint = if app.node_edit.is_some() { "Drag a point \u{b7} click an edge to add one \u{b7} Del removes" } else if app.tab_edit.is_some() { "Click an outline to add a tab \u{b7} right-click removes one" } else { match app.mode {
        Mode::Select if app.selected_text().is_some() => "Double-click: edit the text \u{b7} drag to move \u{b7} handles resize",
        Mode::Select => "Click or box-select \u{b7} drag to move \u{b7} handles resize",
        Mode::Node => "Click a shape to edit its points",
        Mode::Line => "Click points \u{b7} Enter ends \u{b7} C closes \u{b7} Esc cancels",
        Mode::Rect => "Drag to draw a rectangle \u{b7} Shift for a square",
        Mode::Ellipse => if app.circle_mode { "Drag from the centre \u{b7} or type centre X, Y and size" } else { "Drag a bounding box \u{b7} Shift for a circle" },
        Mode::Point => "Click to place a drill point \u{b7} Grid and Bolt circle above",
        Mode::Polygon => "Drag from the centre \u{b7} Shift points it up \u{b7} sides above",
        Mode::Text => "Type the text, then click where it starts",
        Mode::Trim => if app.trim_extend { "Click near an open end to run it on to the next shape" } else { "Click the part to remove, up to the nearest crossings" },
    } };
    if app.trace.open || app.clean.open { return; } // a dialog covers that part of the canvas
    let hint = if app.dyn_in.is_some() { "Typing values: Enter confirms \u{b7} Tab next field \u{b7} Esc cancels" } else { hint };
    egui::Area::new(egui::Id::new("hint")).order(egui::Order::Middle).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(rect.left_bottom() + Vec2::new(12.0, -28.0)).interactable(false).show(ctx, |ui| {
        ui.set_max_width((rect.width() - 24.0).max(120.0));
        ui.add(egui::Label::new(egui::RichText::new(hint).weak()).truncate(true));
    });
}
