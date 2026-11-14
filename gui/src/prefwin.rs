//! File → Preferences: the settings that belong to the person and not to a project: the look (theme, language) and how lengths and
//! speeds are shown. A choice takes effect at once and is remembered, so there is nothing to apply: the window only closes.
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
            let lw = [tr("Theme"), tr("Language"), tr("Show lengths in"), tr("Speeds in")].iter().map(|t| crate::panels::tw(ui, t)).fold(0.0_f32, f32::max);
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
        });
        ui.add_space(6.0);
        if ui.button(tr("Close")).clicked() { close = true; }
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
