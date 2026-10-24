//! Help → About: what the app is, its version, licence and links, and a few things that help when reporting a problem.
use crate::app::App;
use crate::icons::Icon;
use crate::panels::dlg_button;
use eframe::egui;

const REPO_URL: &str = "https://github.com/tonyrh/putiacnc";
const ICON_PNG: &[u8] = include_bytes!("../../assets/icons/putiacnc-256.png");

fn icon_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let mut reader = png::Decoder::new(ICON_PNG).read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight { return None; }
    let img = egui::ColorImage::from_rgba_unmultiplied([info.width as usize, info.height as usize], &buf[..info.buffer_size()]);
    Some(ctx.load_texture("about-icon", img, egui::TextureOptions::LINEAR))
}

/// Version, system and config folder as plain text, for bug reports.
pub fn version_info() -> String {
    let dir = crate::toolmgr::config_dir().map(|d| d.display().to_string()).unwrap_or_else(|| "unknown".into());
    format!("PutiaCNC {} ({} {})\nConfig folder: {}", crate::update::CURRENT, std::env::consts::OS, std::env::consts::ARCH, dir)
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.about_open { return; }
    if app.about_icon.is_none() { app.about_icon = icon_texture(ctx); }
    let mut close = false;
    let accent = app.theme.accent;
    egui::Window::new("About PutiaCNC").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, "About PutiaCNC", ctx.screen_rect().center())).show(ctx, |ui| {
        ui.set_width(460.0);
        if crate::csd::dialog_header(ui, "About PutiaCNC", true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            if let Some(t) = &app.about_icon { ui.image((t.id(), egui::vec2(64.0, 64.0))); }
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("PutiaCNC").size(22.0).strong());
                ui.label(format!("Version {}", crate::update::CURRENT));
                ui.weak("A CNC toolpath studio for GRBL and Mach3 / LinuxCNC machines.");
            });
        });
        ui.add_space(10.0);
        ui.add(egui::Label::new("Draw or import shapes, add operations, preview the cut in 3D and write the G-code. Free software under the GNU General Public License, version 3 or later: you may use, study, share and change it.").wrap(true));
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            if dlg_button(ui, Icon::Export, "Source and releases", Some(accent), Some(egui::Color32::WHITE), true).on_hover_text(REPO_URL).clicked() { let _ = open::that(REPO_URL); }
            if dlg_button(ui, Icon::Import, "Check for updates", None, None, false).on_hover_text("Asks GitHub for the latest release; nothing is contacted unless you ask").clicked() {
                app.updater.open = true; app.updater.check(false); close = true;
            }
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Open config folder").on_hover_text("Where your tool libraries, machines and settings are saved").clicked() {
                if let Some(d) = crate::toolmgr::config_dir() { let _ = std::fs::create_dir_all(&d); let _ = open::that(d); }
            }
            if ui.button("Copy version info").on_hover_text("Version, system and config folder, for a bug report").clicked() { ctx.output_mut(|o| o.copied_text = version_info()); }
        });
        ui.add_space(8.0);
        egui::CollapsingHeader::new(egui::RichText::new("Notices").small().weak()).default_open(false).show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new("Amana Tool, Bosch, CMT, Diablo, Freud and Whiteside are trademarks of their owners. PutiaCNC is not affiliated with or endorsed by them; the names only say which bits the ready-made libraries match. Dimensions come from the makers' published catalogs; feeds, speeds and notes are conservative starting values, not the makers' recommendations.").small().weak()).wrap(true));
            ui.add_space(4.0);
            ui.add(egui::Label::new(egui::RichText::new("Built with Rust and the egui toolkit; the fallback fonts are the ones bundled with egui.").small().weak()).wrap(true));
        });
        ui.add_space(6.0);
        });
    });
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { app.about_open = false; }
}
