//! Help → About: what the app is, its version, licence and links, and a few things that help when reporting a problem.
use crate::app::App;
use crate::icons::Icon;
use crate::panels::dlg_button;
use eframe::egui;

const REPO_URL: &str = "https://github.com/tonyrh/putiacnc";
const PAGE_URL: &str = "https://tonyrh.github.io/putiacnc/";
const ICON_PNG: &[u8] = include_bytes!("../../assets/icons/putiacnc-256.png");

fn icon_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let mut reader = png::Decoder::new(ICON_PNG).read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight { return None; }
    let img = egui::ColorImage::from_rgba_unmultiplied([info.width as usize, info.height as usize], &buf[..info.buffer_size()]);
    Some(ctx.load_texture("about-icon", img, egui::TextureOptions::LINEAR))
}

/// The operating system as a person would name it ("Fedora Linux 41", "macOS 14.5", "Windows ..."), with the kernel on Linux.
fn os_name() -> String {
    #[cfg(target_os = "linux")]
    {
        let pretty = std::fs::read_to_string("/etc/os-release").ok().and_then(|t| t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string())));
        let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok().map(|k| k.trim().to_string());
        match (pretty, kernel) {
            (Some(p), Some(k)) => format!("{p} (kernel {k})"),
            (Some(p), None) => p,
            (None, Some(k)) => format!("Linux (kernel {k})"),
            _ => "Linux".into(),
        }
    }
    #[cfg(target_os = "macos")]
    { std::process::Command::new("sw_vers").arg("-productVersion").output().ok().map(|o| format!("macOS {}", String::from_utf8_lossy(&o.stdout).trim())).unwrap_or_else(|| "macOS".into()) }
    #[cfg(target_os = "windows")]
    { std::process::Command::new("cmd").args(["/C", "ver"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| "Windows".into()) }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    { std::env::consts::OS.to_string() }
}

/// Installed memory in GiB, where the system says so cheaply.
fn memory_gib() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        let t = std::fs::read_to_string("/proc/meminfo").ok()?;
        let kb: f64 = t.lines().find_map(|l| l.strip_prefix("MemTotal:"))?.trim().trim_end_matches("kB").trim().parse().ok()?;
        Some(kb / 1024.0 / 1024.0)
    }
    #[cfg(not(target_os = "linux"))]
    { None }
}

/// A path with the user's home folder written as "~", so a pasted report does not carry the user name.
fn hide_home(path: &str, home: Option<&str>) -> String {
    let Some(h) = home.map(|h| h.trim_end_matches(['/', '\\'])).filter(|h| !h.is_empty()) else { return path.to_string() };
    match path.strip_prefix(h) {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => format!("~{rest}"),
        _ => path.to_string(),
    }
}

/// Version, system and config folder as plain text, for bug reports. No user or computer name is included.
pub fn version_info(ctx: &egui::Context, gl: &str) -> String {
    let home = std::env::var("HOME").ok().or_else(|| std::env::var("USERPROFILE").ok());
    let dir = crate::toolmgr::config_dir().map(|d| hide_home(&d.display().to_string(), home.as_deref())).unwrap_or_else(|| "unknown".into());
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let mut out = format!("PutiaCNC {} ({} {}, {} build)\n", crate::update::CURRENT, std::env::consts::OS, std::env::consts::ARCH, if cfg!(debug_assertions) { "debug" } else { "release" });
    out += &format!("System: {}\n", os_name());
    let session = [var("XDG_SESSION_TYPE").or_else(|| var("WAYLAND_DISPLAY").map(|_| "wayland".into())), var("XDG_CURRENT_DESKTOP")].into_iter().flatten().collect::<Vec<_>>().join(", ");
    if !session.is_empty() { out += &format!("Session: {session}\n"); }
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    out += &format!("CPU: {cores} threads{}\n", memory_gib().map(|g| format!(", {g:.1} GiB memory")).unwrap_or_default());
    let r = ctx.screen_rect();
    out += &format!("Window: {:.0} x {:.0} points, scale {:.2}\n", r.width(), r.height(), ctx.pixels_per_point());
    if !gl.is_empty() { out += &format!("Graphics: {gl}\n"); }
    if let Some(l) = var("LANG") { out += &format!("Locale: {l}\n"); }
    out += &format!("Config folder: {dir}");
    out
}

/// The OpenGL renderer and version, as the driver names them.
pub fn gl_summary(gl: &eframe::glow::Context) -> String {
    use eframe::glow::HasContext;
    // SAFETY: plain driver queries on the context eframe made current for this thread
    unsafe { format!("{} (OpenGL {})", gl.get_parameter_string(eframe::glow::RENDERER), gl.get_parameter_string(eframe::glow::VERSION)) }
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.about_open { return; }
    if app.about_icon.is_none() { app.about_icon = icon_texture(ctx); }
    let mut close = false;
    egui::Window::new("About PutiaCNC").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, "About PutiaCNC", ctx.screen_rect().center())).show(ctx, |ui| {
        ui.set_width(500.0);
        if crate::csd::dialog_header(ui, "About PutiaCNC", true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.add_space(10.0);
        let head = ui.horizontal(|ui| {
            ui.add_space(6.0);
            if let Some(t) = &app.about_icon { ui.image((t.id(), egui::vec2(64.0, 64.0))); }
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("PutiaCNC").size(22.0).strong());
                ui.label(format!("Version {}", crate::update::CURRENT));
                ui.weak("A CNC toolpath studio for GRBL and Mach3 / LinuxCNC machines.");
            });
        }).response;
        // the whole top block is a link to the project page (the web page; the source code has its own link below)
        let link = ui.interact(head.rect, egui::Id::new("about_head"), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(PAGE_URL);
        if link.clicked() { let _ = open::that(PAGE_URL); }
        ui.add_space(10.0);
        ui.add(egui::Label::new("Draw or import shapes, add operations, preview the cut in 3D and write the G-code. Free software under the GNU General Public License, version 3 or later: you may use, study, share and change it.").wrap(true));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.hyperlink_to("Project page", PAGE_URL);
            ui.label("·");
            ui.hyperlink_to("Source code", REPO_URL);
            ui.label("·");
            ui.hyperlink_to("Report a problem", format!("{REPO_URL}/issues"));
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if dlg_button(ui, Icon::Import, "Check for updates", None, None, false).on_hover_text("Asks GitHub for the latest release; nothing is contacted unless you ask").clicked() {
                app.updater.open = true; app.updater.check(false); close = true;
            }
            if ui.button("Open config folder").on_hover_text("Where your tool libraries, machines and settings are saved").clicked() {
                if let Some(d) = crate::toolmgr::config_dir() { let _ = std::fs::create_dir_all(&d); let _ = open::that(d); }
            }
            if ui.button("Copy version info").on_hover_text("Version, system, display and graphics driver, and config folder, for a bug report (no user or computer name)").clicked() { let t = version_info(ctx, &app.gl_info); ctx.output_mut(|o| o.copied_text = t); }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_info_names_the_system_and_leaves_out_who_and_where() {
        let t = version_info(&egui::Context::default(), "Test GPU (OpenGL 4.6)");
        for want in ["PutiaCNC ", "System: ", "CPU: ", "Window: ", "Graphics: Test GPU (OpenGL 4.6)", "Config folder: "] { assert!(t.contains(want), "missing {want:?} in\n{t}"); }
        let host = std::fs::read_to_string("/etc/hostname").unwrap_or_default();
        if !host.trim().is_empty() { assert!(!t.contains(host.trim()), "the computer's name must not be in the text"); }
        // without a driver name there is no Graphics line
        assert!(!version_info(&egui::Context::default(), "").contains("Graphics:"));
    }
}

#[cfg(test)]
mod home_tests {
    use super::hide_home;

    #[test]
    fn home_becomes_a_tilde() {
        assert_eq!(hide_home("/home/anna/.config/putiacnc", Some("/home/anna")), "~/.config/putiacnc");
        assert_eq!(hide_home("/home/anna/.config/putiacnc", Some("/home/anna/")), "~/.config/putiacnc");
        assert_eq!(hide_home("/home/anna", Some("/home/anna")), "~");
        assert_eq!(hide_home("C:\\Users\\Anna\\AppData\\putiacnc", Some("C:\\Users\\Anna")), "~\\AppData\\putiacnc");
    }

    #[test]
    fn other_paths_are_left_alone() {
        assert_eq!(hide_home("/home/annabel/x", Some("/home/anna")), "/home/annabel/x");
        assert_eq!(hide_home("/etc/putiacnc", Some("/home/anna")), "/etc/putiacnc");
        assert_eq!(hide_home("/etc/putiacnc", None), "/etc/putiacnc");
        assert_eq!(hide_home("/etc/putiacnc", Some("")), "/etc/putiacnc");
    }
}
