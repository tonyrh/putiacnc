//! Keyboard shortcuts and mouse actions, as a reference window (Help menu or F1).
use putiacnc_core::i18n::{n_, tr};
use crate::csd::ShowFollowing;
use crate::app::App;
use eframe::egui::{self, Ui};

type Section = (&'static str, &'static [(&'static str, &'static str)]);

const LEFT: &[Section] = &[
    (n_("Tools"), &[
        ("V", n_("Select and move")),
        ("N", n_("Edit points of a shape")),
        ("L", n_("Line / polyline")),
        ("R", n_("Rectangle (Shift: square)")),
        ("E", n_("Ellipse (Shift: circle)")),
        ("D", n_("Drill point (click to place, stays active)")),
        ("P", n_("Polygon or star (Shift: point straight up)")),
        ("T", n_("Text")),
        ("X", n_("Trim and extend")),
        (n_("Esc or right-click"), n_("Cancel, deselect, back to Select (right-click on a shape: its menu)")),
    ]),
    (n_("Project"), &[
        (n_("Ctrl+N"), n_("New project")),
        (n_("Ctrl+O"), n_("Open project")),
        (n_("Ctrl+S"), n_("Save")),
        (n_("Ctrl+Shift+S"), n_("Save as")),
        (n_("Ctrl+I"), n_("Import vectors (SVG or DXF)")),
        (n_("Ctrl+E"), n_("Export G-code")),
        (n_("Ctrl+Q"), n_("Quit")),
    ]),
    (n_("3D preview"), &[
        (n_("Drag"), n_("Orbit")),
        (n_("Right-drag"), n_("Pan")),
        (n_("Scroll"), n_("Zoom")),
        (n_("Double-click"), n_("Reset the view")),
    ]),
];

const RIGHT: &[Section] = &[
    (n_("Editing"), &[
        (n_("Ctrl+Z"), n_("Undo (any change)")),
        (n_("Ctrl+Shift+Z"), n_("Redo (also Ctrl+Y)")),
        (n_("Ctrl+A"), n_("Select all")),
        (n_("Ctrl+D"), n_("Duplicate")),
        (n_("Ctrl+G"), n_("Group the selection")),
        (n_("Ctrl+Shift+G"), n_("Ungroup")),
        (n_("Del"), n_("Delete the selection")),
        (n_("Arrows"), n_("Nudge by the snap step (Shift: ×10)")),
        ("F", n_("Fit the stock in view")),
        ("F1", n_("This window")),
    ]),
    (n_("Drawing and selecting"), &[
        (n_("Click"), n_("Select a shape")),
        (n_("Shift+Click"), n_("Add to or remove from the selection")),
        (n_("Drag"), n_("Box-select, or move the selection")),
        (n_("Shift+Handle"), n_("Resize keeping proportions")),
        (n_("Alt+Handle"), n_("Resize from the centre")),
        (n_("Double-click"), n_("Edit the points of a shape")),
        (n_("Enter / C"), n_("Line: finish / close the shape")),
        (n_("Right-click"), n_("Line: finish. Tabs: remove one")),
        (n_("Scroll"), n_("Zoom")),
        (n_("Middle / right-drag"), n_("Pan")),
    ]),
    (n_("Editing points"), &[
        (n_("Drag"), n_("Move a point, or box-select several")),
        (n_("Click an edge"), n_("Add a point")),
        (n_("Del"), n_("Remove the selected points")),
        (n_("Arrows"), n_("Nudge the selected points")),
        (n_("Esc"), n_("Finish")),
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
        ui.strong(tr(title));
        ui.add_space(2.0);
        egui::Grid::new((id, si)).num_columns(2).spacing([14.0, 5.0]).show(ui, |ui| {
            for (k, d) in rows.iter() {
                // the keycaps share a fixed-width cell so the descriptions line up across sections
                ui.allocate_ui_with_layout(egui::vec2(kw, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_size(egui::vec2(kw, 22.0));
                    let k = putiacnc_core::i18n::tr_dyn(k);
                    for (i, alt) in k.split(" / ").enumerate() { if i > 0 { ui.weak("/"); } keys(ui, alt); }
                });
                ui.label(tr(d));
                ui.end_row();
            }
        });
    }
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.shortcuts_open { return; }
    let mut close = false;
    egui::Window::new("Keyboard shortcuts").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, "Keyboard shortcuts", ctx.screen_rect().center())).show(ctx, crate::csd::trapped(|ui| {
        if crate::csd::dialog_header(ui, tr("Keyboard shortcuts"), true) { close = true; }
        egui::ScrollArea::vertical().max_height((ctx.screen_rect().height() - 140.0).max(200.0)).auto_shrink([true, true]).show_following(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(egui::vec2(370.0, 0.0), egui::Layout::top_down(egui::Align::LEFT), |ui| column(ui, LEFT, "sc_l", 160.0));
                ui.add_space(24.0);
                ui.allocate_ui_with_layout(egui::vec2(450.0, 0.0), egui::Layout::top_down(egui::Align::LEFT), |ui| column(ui, RIGHT, "sc_r", 185.0));
            });
        });
        ui.add_space(6.0);
    }));
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { app.shortcuts_open = false; }
}
