//! The menu bar.
use super::*;

pub fn menu_bar(ui: &mut Ui, app: &mut App) {
    fn item(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
        let r = ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut));
        if r.clicked() { ui.close_menu(); true } else { false }
    }
    egui::menu::bar(ui, |ui| {
        // roomier, rounded menu buttons like a GNOME header bar (menu::bar sets tight defaults)
        {
            let st = ui.style_mut();
            st.spacing.button_padding = egui::vec2(12.0, 6.0);
            st.spacing.item_spacing.x = 4.0;
            st.spacing.interact_size.y = 32.0;
            let r = egui::Rounding::same(7.0);
            st.visuals.widgets.hovered.rounding = r;
            st.visuals.widgets.active.rounding = r;
            st.visuals.widgets.open.rounding = r;
            st.visuals.widgets.inactive.rounding = r;
        }
        ui.menu_button("File", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            if item(ui, "New project", "Ctrl+N", true) { app.request(Action::New); }
            if item(ui, "Open project...", "Ctrl+O", true) { app.open_dialog(); }
            let recent: Vec<String> = app.prefs.recent.iter().filter(|p| std::path::Path::new(p.as_str()).is_file()).cloned().collect();
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button("Open recent", |ui| {
                    ui.style_mut().wrap = Some(false);
                    for p in &recent {
                        let path = std::path::Path::new(p);
                        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
                        let dir = path.parent().map(|d| d.display().to_string()).unwrap_or_default();
                        if ui.button(name).on_hover_text(dir).clicked() { app.request(Action::Open(p.clone())); ui.close_menu(); }
                    }
                    ui.separator();
                    if ui.button("Clear list").clicked() { app.prefs.recent.clear(); ui.close_menu(); }
                });
            });
            if item(ui, "Import Vectors...", "Ctrl+I", true) { app.import_dialog(); }
            if item(ui, "Import Bitmap...", "", true) { app.import_bitmap_dialog(); }
            if item(ui, "Import 3D Model...", "", true) { app.import_model_dialog(); }
            ui.separator();
            if item(ui, "Save", "Ctrl+S", true) { app.save(); }
            if item(ui, "Save as...", "Ctrl+Shift+S", true) { app.save_as_dialog(); }
            ui.separator();
            if item(ui, "Export G-code...", "Ctrl+E", app.output.is_some()) { app.export_gcode_dialog(); }
            ui.separator();
            if item(ui, "Quit", "Ctrl+Q", true) { app.request(Action::Quit); }
        });
        ui.menu_button("Edit", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            use crate::app::Scope;
            if item(ui, "Undo", "Ctrl+Z", app.hist.can_undo(Scope::All)) { app.do_undo(); }
            if item(ui, "Redo", "Ctrl+Shift+Z", app.hist.can_redo(Scope::All)) { app.do_redo(); }
            ui.separator();
            if item(ui, "Select all", "Ctrl+A", true) { app.sel = (0..app.project.data.loops.len()).collect(); }
            if item(ui, "Duplicate", "Ctrl+D", !app.sel.is_empty()) { app.duplicate_selection(); }
            if item(ui, "Delete", "Del", !app.sel.is_empty()) { app.delete_selection(); }
            ui.separator();
            let can_group = putiacnc_core::selection::can_group(&app.project.data, &app.sel);
            if item(ui, "Group", "Ctrl+G", can_group) { app.group_selection(); }
            if item(ui, "Ungroup", "Ctrl+Shift+G", app.sel_in_group()) { app.ungroup_selection(); }
            ui.separator();
            if item(ui, "Offset...", "", !app.sel.is_empty()) { app.offset_open = true; }
            if item(ui, "Array and copy...", "", !app.sel.is_empty()) { app.array.open = true; }
            if item(ui, "Fillet and chamfer...", "", app.selection_has_corners()) { app.corners.open = true; }
            if item(ui, "Combine shapes...", "", app.sel.len() >= 2) { app.combine_open = true; }
            if item(ui, "Clean up drawing...", "", !app.project.data.loops.is_empty()) { app.clean.open = true; app.clean.msg.clear(); }
        });
        ui.menu_button("View", |ui| {
            ui.style_mut().wrap = Some(false);
            // wide enough for the longest checkbox, so the shortcut of the action row sits at the real right edge
            ui.set_min_width(236.0);
            if item(ui, "Fit to stock", "F", true) { if app.view_tab == crate::view3d::ViewTab::Preview { app.fit3d_pending = true; } else { app.fit_pending = true; } }
            ui.separator();
            ui.checkbox(&mut app.show_toolpath, "Toolpaths");
            ui.checkbox(&mut app.show_rapids, "Rapid moves");
            ui.checkbox(&mut app.show_badges, "Operation badges on shapes");
            ui.separator();
            ui.checkbox(&mut app.show_machine_area, "Machine work area");
            ui.checkbox(&mut app.show_clamps, "Clamps");
            ui.separator();
            ui.checkbox(&mut app.grid_on, "Grid");
            ui.checkbox(&mut app.show_rulers, "Rulers");
            ui.checkbox(&mut app.show_axes, "Axes");
            ui.checkbox(&mut app.show_open_ends, "Loose ends of open contours");
            ui.separator();
            ui.checkbox(&mut app.show_left, "Project panel");
            ui.checkbox(&mut app.show_right, "Operations / G-code panel");
        });
        ui.menu_button("Libraries", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            if ui.button("Tool library...").clicked() { app.tools_open = true; ui.close_menu(); }
            if ui.button("Machines...").clicked() { app.machines_open = true; ui.close_menu(); }
        });
        ui.menu_button("Help", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            if ui.add(egui::Button::new("Keyboard shortcuts").shortcut_text("F1")).clicked() { app.shortcuts_open = true; ui.close_menu(); }
            ui.separator();
            if ui.button("About PutiaCNC...").clicked() { app.about_open = true; ui.close_menu(); }
        });
    });
}
