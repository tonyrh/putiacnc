//! File → Preferences: the settings that belong to the person and not to a project: the look (theme, language) and how lengths and
//! speeds are shown. A choice takes effect at once and is remembered, so there is nothing to apply: the window closes with the cross in its header.
use crate::app::App;
use crate::widgets::Tip;
use crate::zoomcard::{seg2, seg3};
use eframe::egui;
use putiacnc_core::i18n::tr;
use putiacnc_core::toollib::SPEED_UNITS;

/// A section title with a thin rule under it, the same in every section.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(title).strong());
    ui.separator();
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.prefs_open { return; }
    let mut close = false;
    let mut theme: Option<&str> = None;
    let mut lang: Option<String> = None;
    egui::Window::new("Preferences").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Preferences", egui::pos2(560.0, 120.0))).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(crate::csd::dialog_w(420.0));
        if crate::csd::dialog_header(ui, tr("Preferences"), true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
            let lw = [tr("Theme"), tr("Language"), tr("Show lengths in"), tr("Speeds in"), tr("Projects"), tr("Imports"), tr("G-code"), tr("Settings")].iter().map(|t| crate::panels::tw(ui, t)).fold(0.0_f32, f32::max);
            // the choices sit in one column, as wide as the widest of them
            let field_w = |ui: &egui::Ui| (ui.available_width() - lw - 10.0).max(120.0);
            section(ui, tr("Appearance"));
            egui::Grid::new("pref_look").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                crate::toolmgr::rlabel(ui, lw, tr("Theme"));
                let sel = match app.theme_mode.as_str() { "light" => 1, "dark" => 2, _ => 0 };
                let (c, _) = seg3(ui, [tr("System"), tr("Light"), tr("Dark")], [tr("Follow the light or dark setting of the desktop"), tr("Always light, whatever the desktop uses"), tr("Always dark, whatever the desktop uses")], sel);
                if let Some(i) = c { theme = Some(["system", "light", "dark"][i]); }
                ui.end_row();
                crate::toolmgr::rlabel(ui, lw, tr("Language"));
                let saved = app.prefs.language.clone().unwrap_or_else(|| "auto".into());
                let name_of = |code: &str| if code == "auto" { tr("Automatic") } else { putiacnc_core::i18n::LANGUAGES.iter().find(|(c, _, _)| *c == code).map(|(_, n, _)| *n).unwrap_or(tr("Automatic")) };
                let w = field_w(ui).min(220.0);
                egui::ComboBox::from_id_source("pref_lang").width(w).selected_text(name_of(&saved)).show_ui(ui, |ui| {
                    ui.horizontal(|ui| { crate::widgets::flag(ui, ""); if ui.selectable_label(saved == "auto", tr("Automatic")).clicked() { lang = Some("auto".into()); } });
                    for (code, name, _) in putiacnc_core::i18n::LANGUAGES { ui.horizontal(|ui| { crate::widgets::flag(ui, code); if ui.selectable_label(saved == *code, *name).clicked() { lang = Some(code.to_string()); } }); }
                    if std::env::var_os("PUTIACNC_PSEUDO").is_some() && ui.selectable_label(saved == putiacnc_core::i18n::PSEUDO, tr("Pseudo (test)")).clicked() { lang = Some(putiacnc_core::i18n::PSEUDO.into()); }
                }).response.tip(tr("The language of the menus and windows. Automatic follows the system."));
                ui.end_row();
            });
            ui.add_space(8.0);
            section(ui, tr("Units"));
            egui::Grid::new("pref_units").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                crate::toolmgr::rlabel(ui, lw, tr("Show lengths in"));
                let (c, _) = seg2(ui, [tr("Millimetres"), tr("Inches")], [tr("Lengths are shown and typed in millimetres"), tr("Lengths are shown and typed in inches")], app.disp_in as usize);
                if let Some(i) = c { app.disp_in = i == 1; }
                ui.end_row();
                ui.label("");
                ui.add_enabled_ui(app.disp_in, |ui| { crate::widgets::check(ui, &mut app.frac_in, tr("Fractions (3/16)")).tip(tr("Show inch values as fractions of an inch, to 1/64, where they are one")); });
                ui.end_row();
                crate::toolmgr::rlabel(ui, lw, tr("Speeds in"));
                let sname = app.tool_speed().0;
                let w = crate::widgets::combo_fit(ui, &SPEED_UNITS.iter().map(|(n, _)| *n).collect::<Vec<_>>()).max(90.0);
                egui::ComboBox::from_id_source("pref_speed").width(w).selected_text(sname).show_ui(ui, |ui| {
                    let slot = app.tool_speed_slot();
                    for (i, (n, _)) in SPEED_UNITS.iter().enumerate() { ui.selectable_value(slot, i, *n); }
                }).response.tip(tr("The unit of feeds in the tool library and the operations. The G-code units are set by the machine."));
                ui.end_row();
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new(tr("The G-code units are set by the machine, not here.")).weak());
            ui.add_space(8.0);
            section(ui, tr("Data"));
            let btn_w = 96.0;
            let rows: [(&str, Option<String>, crate::app::DlgKind); 3] = [
                (tr("Projects"), app.prefs.dir_project.clone(), crate::app::DlgKind::DirProject),
                (tr("Imports"), app.prefs.dir_svg.clone(), crate::app::DlgKind::DirImport),
                (tr("G-code"), app.prefs.dir_gcode.clone(), crate::app::DlgKind::DirGcode),
            ];
            for (name, dir, kind) in rows {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    crate::toolmgr::rlabel(ui, lw, name);
                    let path_w = (ui.available_width() - btn_w - 28.0 - 2.0 * 10.0).max(60.0);
                    let shown = dir.as_deref().map(|d| crate::widgets::elide_text(ui, &crate::about::home_short(d), path_w)).unwrap_or_else(|| tr("Not set yet").to_string());
                    ui.allocate_ui_with_layout(egui::vec2(path_w, ui.spacing().interact_size.y), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_width(path_w); ui.add(egui::Label::new(egui::RichText::new(shown).weak()).truncate(true)).on_hover_text(dir.clone().unwrap_or_default()); });
                    if ui.add_sized([btn_w, ui.spacing().interact_size.y], egui::Button::new(tr("Choose…"))).clicked() {
                        let start = crate::prefs::existing_dir(&dir);
                        app.spawn_dialog(kind, move || { let mut dlg = rfd::FileDialog::new(); if let Some(d) = start { dlg = dlg.set_directory(d); } dlg.pick_folder() });
                    }
                    if crate::icons::small_icon_button(ui, crate::icons::Icon::Close, dir.is_some(), tr("Forget this folder")).clicked() {
                        match kind { crate::app::DlgKind::DirProject => app.prefs.dir_project = None, crate::app::DlgKind::DirImport => app.prefs.dir_svg = None, _ => app.prefs.dir_gcode = None }
                    }
                });
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                crate::toolmgr::rlabel(ui, lw, tr("Settings"));
                if ui.button(tr("Open config folder")).tip(tr("Where your tool libraries, machines and settings are saved")).clicked() {
                    if let Some(d) = crate::toolmgr::config_dir() { let _ = std::fs::create_dir_all(&d); let _ = open::that_detached(d); }
                }
            });
            ui.add_space(8.0);
            section(ui, tr("Updates"));
            egui::Grid::new("pref_updates").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                ui.label("");
                let mut auto = app.updater.auto;
                crate::widgets::check(ui, &mut auto, tr("Check for updates when PutiaCNC starts")).tip(tr("Asks GitHub for the latest release once, when the program starts. Nothing else is sent."));
                if auto != app.updater.auto { app.updater.auto = auto; crate::prefs::modify(|s| s.update_check = auto); }
                ui.end_row();
                ui.label("");
                if ui.button(tr("Check for updates…")).tip(tr("Asks GitHub for the latest release; nothing is contacted unless you ask")).clicked() { app.updater.open = true; app.updater.check(false); }
                ui.end_row();
            });
        });
    }));
    if let Some(m) = theme {
        app.theme_mode = m.to_string();
        app.theme = crate::theme::detect_mode(m);
        crate::theme::apply_style(ctx, &app.theme);
    }
    if let Some(code) = lang {
        putiacnc_core::i18n::set_language(&code);
        crate::prefs::modify(|s| s.language = Some(code.clone()));
        app.prefs.language = Some(code);
    }
    if close { app.prefs_open = false; }
}
