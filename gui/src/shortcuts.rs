//! Keyboard shortcuts and mouse actions, as a reference window (Help menu or F1).
use crate::app::App;
use eframe::egui::{self, Ui};

type Section = (&'static str, &'static [(&'static str, &'static str)]);

const LEFT: &[Section] = &[
    ("Tools", &[
        ("V", "Select and move"),
        ("N", "Edit points of a shape"),
        ("L", "Line / polyline"),
        ("R", "Rectangle (Shift: square)"),
        ("E", "Ellipse (Shift: circle)"),
        ("D", "Drill point (click to place, stays active)"),
        ("P", "Polygon or star (Shift: point straight up)"),
        ("T", "Text"),
        ("X", "Trim and extend"),
        ("Esc or right-click", "Cancel, deselect, back to Select"),
    ]),
    ("Project", &[
        ("Ctrl+N", "New project"),
        ("Ctrl+O", "Open project"),
        ("Ctrl+S", "Save"),
        ("Ctrl+Shift+S", "Save as"),
        ("Ctrl+I", "Import vectors (SVG or DXF)"),
        ("Ctrl+E", "Export G-code"),
        ("Ctrl+Q", "Quit"),
    ]),
    ("3D preview", &[
        ("Drag", "Orbit"),
        ("Right-drag", "Pan"),
        ("Scroll", "Zoom"),
        ("Double-click", "Reset the view"),
    ]),
];

const RIGHT: &[Section] = &[
    ("Editing", &[
        ("Ctrl+Z", "Undo (any change)"),
        ("Ctrl+Shift+Z", "Redo (also Ctrl+Y)"),
        ("Ctrl+A", "Select all"),
        ("Ctrl+D", "Duplicate"),
        ("Del", "Delete the selection"),
        ("Arrows", "Nudge by the snap step (Shift: ×10)"),
        ("F", "Fit the stock in view"),
        ("F1", "This window"),
    ]),
    ("Drawing and selecting", &[
        ("Click", "Select a shape"),
        ("Shift+Click", "Add to or remove from the selection"),
        ("Drag", "Box-select, or move the selection"),
        ("Shift+Handle", "Resize keeping proportions"),
        ("Alt+Handle", "Resize from the centre"),
        ("Double-click", "Edit the points of a shape"),
        ("Enter / C", "Line: finish / close the shape"),
        ("Right-click", "Line: finish. Tabs: remove one"),
        ("Scroll", "Zoom"),
        ("Middle / right-drag", "Pan"),
    ]),
    ("Editing points", &[
        ("Drag", "Move a point, or box-select several"),
        ("Click an edge", "Add a point"),
        ("Del", "Remove the selected points"),
        ("Arrows", "Nudge the selected points"),
        ("Esc", "Finish"),
    ]),
];

/// One key combination as a row of small keycaps.
fn keys(ui: &mut Ui, combo: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, part) in combo.split('+').enumerate() {
            if i > 0 { ui.weak("+"); }
            // "Ctrl" is Cmd on a Mac
            let part = if part == "Ctrl" && cfg!(target_os = "macos") { "Cmd" } else { part };
            let fill = ui.visuals().widgets.inactive.bg_fill;
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            egui::Frame::none().fill(fill).stroke(stroke).rounding(4.0).inner_margin(egui::Margin::symmetric(6.0, 1.0)).show(ui, |ui| {
                ui.label(egui::RichText::new(part).monospace().size(12.5));
            });
        }
    });
}

fn column(ui: &mut Ui, sections: &[Section], id: &str, kw: f32) {
    for (si, (title, rows)) in sections.iter().enumerate() {
        if si > 0 { ui.add_space(10.0); }
        ui.strong(*title);
        ui.add_space(2.0);
        egui::Grid::new((id, si)).num_columns(2).spacing([14.0, 5.0]).show(ui, |ui| {
            for (k, d) in rows.iter() {
                // the keycaps share a fixed-width cell so the descriptions line up across sections
                ui.allocate_ui_with_layout(egui::vec2(kw, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_size(egui::vec2(kw, 22.0));
                    for (i, alt) in k.split(" / ").enumerate() { if i > 0 { ui.weak("/"); } keys(ui, alt); }
                });
                ui.label(*d);
                ui.end_row();
            }
        });
    }
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.shortcuts_open { return; }
    let mut close = false;
    egui::Window::new("Keyboard shortcuts").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, "Keyboard shortcuts", ctx.screen_rect().center())).show(ctx, |ui| {
        if crate::csd::dialog_header(ui, "Keyboard shortcuts", true) { close = true; }
        egui::ScrollArea::vertical().max_height((ctx.screen_rect().height() - 140.0).max(200.0)).auto_shrink([true, true]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(egui::vec2(370.0, 0.0), egui::Layout::top_down(egui::Align::LEFT), |ui| column(ui, LEFT, "sc_l", 160.0));
                ui.add_space(24.0);
                ui.allocate_ui_with_layout(egui::vec2(450.0, 0.0), egui::Layout::top_down(egui::Align::LEFT), |ui| column(ui, RIGHT, "sc_r", 185.0));
            });
        });
        ui.add_space(6.0);
    });
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { app.shortcuts_open = false; }
}
