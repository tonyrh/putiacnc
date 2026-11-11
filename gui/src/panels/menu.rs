//! The menu bar.
use putiacnc_core::i18n::tr;
use super::*;
use crate::app::ToolWin;

pub fn menu_bar(ui: &mut Ui, app: &mut App) {
    fn item(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
        let r = ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut));
        if r.clicked() { ui.close_menu(); true } else { false }
    }
    // a menu row that says why it is greyed out
    fn item_why(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool, why: &str) -> bool {
        let r = ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut)).on_disabled_hover_text(why);
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
            if item(ui, "Open project…", "Ctrl+O", true) { app.open_dialog(); }
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
            ui.separator();
            if item(ui, "Import vectors…", "Ctrl+I", true) { app.import_dialog(); }
            if item(ui, "Import bitmap…", "", true) { app.import_bitmap_dialog(); }
            if item(ui, "Import 3D model…", "", true) { app.import_model_dialog(); }
            ui.separator();
            if item(ui, "Save", "Ctrl+S", true) { app.save(); }
            if item(ui, "Save as…", "Ctrl+Shift+S", true) { app.save_as_dialog(); }
            ui.separator();
            if item(ui, "Export G-code…", "Ctrl+E", app.output.is_some()) { app.export_gcode_dialog(); }
            if item(ui, "Setup sheet…", "", app.output.is_some()) { app.setup_sheet_dialog(); }
            ui.separator();
            if item(ui, "Tool library…", "", true) { app.tools_open = true; }
            if item(ui, "Machines…", "", true) { app.machines_open = true; }
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
            if item(ui, "Select all", "Ctrl+A", true) { app.sel = (0..app.project.data.loops.len()).filter(|&i| !app.loop_hidden(i)).collect(); }
            if item(ui, "Duplicate", "Ctrl+D", !app.sel.is_empty()) { app.duplicate_selection(); }
            if item(ui, "Delete", "Del", !app.sel.is_empty()) { app.delete_selection(); }
            ui.separator();
            let can_group = putiacnc_core::selection::can_group(&app.project.data, &app.sel);
            if item(ui, "Group", "Ctrl+G", can_group) { app.group_selection(); }
            if item(ui, "Ungroup", "Ctrl+Shift+G", app.sel_in_group()) { app.ungroup_selection(); }
        });
        ui.menu_button("Draw", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(250.0);
            use crate::app::Mode;
            for (m, label, key) in [(Mode::Select, "Select / move", "V"), (Mode::Node, "Edit points", "N")] {
                if item(ui, label, key, true) { app.choose_tool(m); }
            }
            ui.separator();
            for (m, label, key) in [(Mode::Line, "Line / polyline", "L"), (Mode::Rect, "Rectangle", "R"), (Mode::Ellipse, "Ellipse / circle", "E"), (Mode::Polygon, "Polygon / star", "P"), (Mode::Point, "Drill point, bolt circle, grid", "D"), (Mode::Text, "Text", "T")] {
                if item(ui, label, key, true) { app.choose_tool(m); }
            }
            ui.separator();
            if item(ui, "Trim / extend", "X", true) { app.choose_tool(Mode::Trim); }
        });
        ui.menu_button("Modify", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            let some = !app.sel.is_empty();
            let free = some && !app.sel_locked();
            let pick = "Select a shape first";
            // move and copy
            ui.add_enabled_ui(free, |ui| {
                ui.menu_button("Mirror", |ui| {
                    ui.style_mut().wrap = Some(false);
                    if item(ui, "Left to right", "", true) { app.mirror_selection(true); }
                    if item(ui, "Top to bottom", "", true) { app.mirror_selection(false); }
                }).response.on_disabled_hover_text(if some { "This drawing is locked" } else { pick });
            });
            if item_why(ui, "Align…", "", free, if some { "This drawing is locked" } else { pick }) { app.open_tool_window(ToolWin::Align); }
            if item_why(ui, "Array and copy…", "", some, pick) { app.open_tool_window(ToolWin::Array); }
            if item_why(ui, "Nest on the stock…", "", some, pick) { app.open_tool_window(ToolWin::Nest); }
            ui.separator();
            // reshape
            if item_why(ui, "Offset…", "", some, pick) { app.open_tool_window(ToolWin::Offset); }
            if item_why(ui, "Fillet and chamfer…", "", app.selection_has_corners(), "Select a shape with corners (not a circle or an ellipse)") { app.open_tool_window(ToolWin::Corners); }
            if item_why(ui, "Combine shapes…", "", app.sel.len() >= 2, "Select two or more shapes") { app.open_tool_window(ToolWin::Combine); }
            ui.separator();
            // joinery and tidy-up
            if item_why(ui, "Finger joint…", "", app.selection_has_rect(), "Select a rectangle") { app.open_tool_window(ToolWin::Fingers); }
            if item_why(ui, "Clean up drawing…", "", !app.project.data.loops.is_empty(), "The drawing is empty") { app.open_tool_window(ToolWin::Clean); }
        });
        ui.menu_button("Generate", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            if item(ui, "Box with finger joints…", "", true) { app.open_tool_window(ToolWin::Box); }
            if item_why(ui, "Relief from shapes…", "", app.closed_selection() > 0, "Select one or more closed shapes") { app.open_tool_window(ToolWin::Relief); }
            if item(ui, "Surface the stock…", "", true) { app.open_tool_window(ToolWin::Surface); }
            ui.separator();
            ui.menu_button("Sample projects", |ui| {
                ui.style_mut().wrap = Some(false);
                if ui.button("Name plate").on_hover_text("A plate with a recess and engraved lettering, made to fit your machine and tools; the Notes tab explains it").clicked() { app.request(Action::Sample(putiacnc_core::sample::Kind::NamePlate)); ui.close_menu(); }
                if ui.button("Finger-joint box").on_hover_text("The five plates of a small box with finger joints, laid out on your stock; the Notes tab explains it").clicked() { app.request(Action::Sample(putiacnc_core::sample::Kind::Box)); ui.close_menu(); }
                if ui.button("3D relief").on_hover_text("A six-petal rosette carved with roughing, finishing and detail tools, made to fit your machine and tools; the Notes tab explains it").clicked() { app.request(Action::Sample(putiacnc_core::sample::Kind::Relief)); ui.close_menu(); }
                if ui.button("Relief from shapes").on_hover_text("A cushion with a groove and a button, made from three closed shapes with Generate > Relief from shapes and carved as a 3D relief; the Notes tab explains it").clicked() { app.request(Action::Sample(putiacnc_core::sample::Kind::ShapedRelief)); ui.close_menu(); }
                if ui.button("Oval sign").on_hover_text("An oval plate with its name V-carved in the middle, text following the oval, a fine border and four holding tabs; needs a work area of about 260 x 170 mm; the Notes tab explains it").clicked() { app.request(Action::Sample(putiacnc_core::sample::Kind::Oval)); ui.close_menu(); }
            });
        });
        ui.menu_button("View", |ui| {
            ui.style_mut().wrap = Some(false);
            // wide enough for the longest checkbox, so the shortcut of the action row sits at the real right edge
            ui.set_min_width(236.0);
            if item(ui, "Fit to stock", "F", true) { match app.view_tab { crate::view3d::ViewTab::Preview => app.fit3d_pending = true, crate::view3d::ViewTab::Split => { app.fit3d_pending = true; app.fit_pending = true; } _ => app.fit_pending = true } }
            ui.separator();
            crate::widgets::check(ui, &mut app.show_toolpath, "Toolpaths");
            crate::widgets::check(ui, &mut app.show_rapids, "Rapid moves");
            crate::widgets::check(ui, &mut app.show_badges, "Operation badges on shapes");
            ui.separator();
            crate::widgets::check(ui, &mut app.show_machine_area, "Machine work area");
            crate::widgets::check(ui, &mut app.show_clamps, "Clamps");
            ui.separator();
            crate::widgets::check(ui, &mut app.grid_on, "Grid");
            crate::widgets::check(ui, &mut app.show_rulers, "Rulers");
            crate::widgets::check(ui, &mut app.show_axes, "Axes");
            crate::widgets::check(ui, &mut app.show_open_ends, "Loose ends of open contours");
            ui.separator();
            ui.menu_button("Units", |ui| {
                ui.style_mut().wrap = Some(false);
                if ui.radio(!app.disp_in, "Millimetres").clicked() { app.disp_in = false; ui.close_menu(); }
                if ui.radio(app.disp_in, "Inches").clicked() { app.disp_in = true; ui.close_menu(); }
                ui.add_enabled_ui(app.disp_in, |ui| { crate::widgets::check(ui, &mut app.frac_in, "Fractions (3/16)").on_hover_text("Show inch values as fractions of an inch, to 1/64, where they are one"); });
            }).response.on_hover_text("How lengths are shown and typed. The G-code units are set by the machine.");
            ui.menu_button(tr("Language"), |ui| {
                ui.style_mut().wrap = Some(false);
                let saved = crate::prefs::load().language.unwrap_or_else(|| "auto".into());
                let mut pick: Option<String> = None;
                if ui.radio(saved == "auto", tr("Automatic")).clicked() { pick = Some("auto".into()); }
                for (code, name, _) in putiacnc_core::i18n::LANGUAGES { if ui.radio(saved == *code, *name).clicked() { pick = Some(code.to_string()); } }
                if std::env::var_os("PUTIACNC_PSEUDO").is_some() && ui.radio(saved == putiacnc_core::i18n::PSEUDO, "Pseudo (test)").clicked() { pick = Some(putiacnc_core::i18n::PSEUDO.into()); }
                if let Some(code) = pick {
                    putiacnc_core::i18n::set_language(&code);
                    crate::prefs::modify(|s| s.language = Some(code));
                    ui.close_menu();
                }
            });
            ui.separator();
            crate::widgets::check(ui, &mut app.show_left, "Project panel");
            crate::widgets::check(ui, &mut app.show_right, "Operations / G-code panel");
        });
        ui.menu_button("Help", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            if ui.button("Quick tour").on_hover_text("A two-minute look at where things are").clicked() { app.tour.start(); ui.close_menu(); }
            if ui.button("Setup assistant…").on_hover_text("Set up a machine and a starter set of tools; nothing you have is deleted").clicked() { app.setup.start(false); ui.close_menu(); }
            ui.separator();
            if ui.add(egui::Button::new("Keyboard shortcuts").shortcut_text("F1")).clicked() { app.shortcuts_open = true; ui.close_menu(); }
            ui.separator();
            if ui.button("Check for updates…").on_hover_text("Asks GitHub for the latest release; nothing is contacted unless you ask").clicked() { app.updater.open = true; app.updater.check(false); ui.close_menu(); }
            if ui.button("Open config folder").on_hover_text("Where your tool libraries, machines and settings are saved").clicked() {
                if let Some(d) = crate::toolmgr::config_dir() { let _ = std::fs::create_dir_all(&d); let _ = open::that_detached(d); }
                ui.close_menu();
            }
            ui.separator();
            if ui.button("About PutiaCNC…").clicked() { app.about_open = true; ui.close_menu(); }
        });
    });
}
