//! The toolbar at the bottom of the window and its drop-up menus.
use super::*;

/// Modal-style prompt shown when an action would discard unsaved changes.
/// Fixed toolbar under the menu: undo/redo and view options.
pub fn tool_bar(ui: &mut Ui, app: &mut App) {
    if app.view_tab == crate::view3d::ViewTab::Preview {
        // playback on the left; on the right the same spots as in the 2D bar, with the speed box where Guides is
        let sp = ui.spacing();
        let (gap, extra) = (sp.item_spacing.x, sp.icon_width + sp.icon_spacing + 2.0 * sp.button_padding.x);
        let slot = guides_size(ui).x;
        let right_w = slot + 2.0 * extra + tw(ui, "Rapids") + tw(ui, "Toolpaths") + 3.0 * gap;
        let left_w = ui.available_width() - right_w - gap - 2.0;
        let h = ui.spacing().interact_size.y;
        if left_w >= 300.0 {
            // the toggles take their own width from the right; playback gets exactly what is left, so nothing is wasted between them
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    crate::view3d::speed_box(ui, app, slot);
                    ui.checkbox(&mut app.show_rapids, "Rapids");
                    ui.checkbox(&mut app.show_toolpath, "Toolpaths");
                    let w = ui.available_width().max(0.0);
                    ui.allocate_ui_with_layout(egui::vec2(w, h), egui::Layout::left_to_right(egui::Align::Center), |ui| crate::view3d::play_bar(ui, app));
                });
            });
        } else {
            // too narrow for the slider to stay useful: playback and speed on one row, the toggles under it
            ui.horizontal(|ui| {
                let w = (ui.available_width() - slot - gap).max(120.0);
                ui.allocate_ui_with_layout(egui::vec2(w, h), egui::Layout::left_to_right(egui::Align::Center), |ui| crate::view3d::play_bar(ui, app));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| crate::view3d::speed_box(ui, app, slot));
            });
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.allocate_exact_size(egui::vec2(slot, h), egui::Sense::hover());
                    ui.checkbox(&mut app.show_rapids, "Rapids");
                    ui.checkbox(&mut app.show_toolpath, "Toolpaths");
                });
            });
        }
        return;
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut app.grid_on, "Grid");
        crate::widgets::num(ui, [72.0, ui.spacing().interact_size.y], &mut app.snap_mm, 0.01, 500.0, 0.1, 2, " mm").on_hover_text("Grid size (also the snap step)");
        dropup(ui, app, "snap_menu", "Snap", |ui, app| {
            ui.set_width(checks_width(ui, &["To the grid", "To shapes"]));
            ui.checkbox(&mut app.snap_on, "To the grid").on_hover_text("The pointer jumps to the grid; the grid size is the snap step");
            ui.checkbox(&mut app.osnap_on, "To shapes").on_hover_text("The pointer jumps to the corners, midpoints, centres and crossings of shapes, and to the stock");
        });
        // view toggles on the right (right-to-left, so listed in reverse)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            guides_menu(ui, app);
            ui.checkbox(&mut app.show_rapids, "Rapids");
            ui.checkbox(&mut app.show_toolpath, "Toolpaths");
        });
    });
}

/// The "Guides" drop-up in the bottom bar: a button with a painted arrow (a text arrow is a box in some fonts) whose panel opens upwards
/// and stays open while the toggles are clicked.
pub(super) fn dropup_size(ui: &Ui, label: &str) -> egui::Vec2 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let w = ui.painter().layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER).size().x;
    egui::vec2(w + 36.0, ui.spacing().interact_size.y)
}

pub(super) fn guides_size(ui: &Ui) -> egui::Vec2 { dropup_size(ui, "Guides") }

pub(super) fn guides_menu(ui: &mut Ui, app: &mut App) {
    dropup(ui, app, "guides_menu", "Guides", |ui, app| {
        ui.set_width(checks_width(ui, &["Rulers", "Axes", "Machine area"])); // a set width: a separator would otherwise stretch the menu across the screen
        ui.checkbox(&mut app.show_rulers, "Rulers").on_hover_text("Rulers in mm along the top and left of the drawing area; they mark the pointer and the selection");
        ui.checkbox(&mut app.show_axes, "Axes").on_hover_text("Graduated X and Y axes along the stock's lower and left edges");
        ui.checkbox(&mut app.show_machine_area, "Machine area").on_hover_text("Outline of the machine work area, from the stock's lower-left corner");
    });
}

/// The width a menu of checkboxes needs: the longest label and the box in front of it.
fn checks_width(ui: &Ui, labels: &[&str]) -> f32 {
    let sp = ui.spacing();
    labels.iter().map(|l| tw(ui, l)).fold(0.0_f32, f32::max) + sp.icon_width + sp.icon_spacing + 2.0
}

/// A button with a painted arrow whose panel opens upwards and stays open while its toggles are clicked.
pub(super) fn dropup(ui: &mut Ui, app: &mut App, name: &str, label: &str, contents: impl FnOnce(&mut Ui, &mut App)) {
    let id = ui.id().with(name);
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if std::env::var("PUTIACNC_DROPUP").is_ok_and(|v| v == name) { open = true; } // for screenshots
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
    let size = dropup_size(ui, label);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let v = ui.style().interact_selectable(&resp, open);
    ui.painter().rect(rect.expand(v.expansion), v.rounding, v.bg_fill, v.bg_stroke);
    let col = v.fg_stroke.color;
    ui.painter().galley(egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, col);
    let c = egui::pos2(rect.right() - 12.0, rect.center().y);
    let tri = if open { [c + egui::vec2(-4.0, -2.0), c + egui::vec2(4.0, -2.0), c + egui::vec2(0.0, 3.0)] } else { [c + egui::vec2(-4.0, 3.0), c + egui::vec2(4.0, 3.0), c + egui::vec2(0.0, -2.0)] };
    ui.painter().add(egui::Shape::convex_polygon(tri.to_vec(), col, egui::Stroke::NONE));
    let mut open_now = open;
    if resp.clicked() { open_now = !open; }
    if open_now {
        let area = egui::Area::new(id).order(egui::Order::Foreground).pivot(egui::Align2::RIGHT_BOTTOM).fixed_pos(rect.right_top() - egui::vec2(0.0, 4.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    contents(ui, app);
                });
            });
        let outside = ui.input(|i| i.pointer.any_click()) && !resp.clicked()
            && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p));
        if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) { open_now = false; }
    }
    if open_now != open { ui.data_mut(|d| d.insert_temp(id, open_now)); ui.ctx().request_repaint(); }
}
