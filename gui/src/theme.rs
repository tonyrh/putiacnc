//! GNOME / libadwaita look: system UI font and size, dark/light preference and accent colour
//! are read from GNOME's settings (gsettings) and fontconfig. Everywhere else, or when a
//! setting can't be read, sensible Adwaita-like defaults are used.
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, TextStyle, Vec2, Visuals};
use std::process::Command;

#[derive(Clone, Copy, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub accent: Color32,
    /// UI font size in logical pixels.
    pub size: f32,
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() { return None; }
    let s = String::from_utf8_lossy(&out.stdout).trim().trim_matches('\'').to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn gsetting(key: &str) -> Option<String> { run("gsettings", &["get", "org.gnome.desktop.interface", key]) }

/// "Cantarell 11" -> ("Cantarell", 11.0)
fn parse_font(s: &str) -> (String, f32) {
    let mut parts: Vec<&str> = s.split_whitespace().collect();
    let size = parts.last().and_then(|p| p.parse::<f32>().ok());
    if size.is_some() { parts.pop(); }
    (parts.join(" "), size.unwrap_or(11.0))
}

pub fn detect() -> Theme {
    // PUTIACNC_THEME=light|dark overrides the desktop setting (handy for testing)
    let forced = std::env::var("PUTIACNC_THEME").ok();
    let dark = match forced.as_deref().or(gsetting("color-scheme").as_deref()) {
        Some(s) if s.contains("dark") => true,
        Some(s) if s.contains("light") || s == "default" => false,
        _ => true,
    };
    let accent = match gsetting("accent-color").as_deref().unwrap_or("blue") {
        "teal" => Color32::from_rgb(0x21, 0x90, 0xa4),
        "green" => Color32::from_rgb(0x3a, 0x94, 0x4a),
        "yellow" => Color32::from_rgb(0xc8, 0x88, 0x00),
        "orange" => Color32::from_rgb(0xed, 0x5b, 0x00),
        "red" => Color32::from_rgb(0xe6, 0x2d, 0x42),
        "pink" => Color32::from_rgb(0xd5, 0x61, 0x99),
        "purple" => Color32::from_rgb(0x91, 0x41, 0xac),
        "slate" => Color32::from_rgb(0x6f, 0x83, 0x96),
        _ => Color32::from_rgb(0x35, 0x84, 0xe4),
    };
    let (_, pt) = parse_font(&gsetting("font-name").unwrap_or_default());
    let scale = gsetting("text-scaling-factor").and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);
    Theme { dark, accent, size: pt * scale * 96.0 / 72.0 }
}

fn load_font(name: &str) -> Option<Vec<u8>> {
    let file = run("fc-match", &["-f", "%{file}", name])?;
    std::fs::read(file).ok()
}

/// Use the desktop's UI and monospace fonts, keeping egui's bundled fonts as a glyph fallback.
pub fn apply_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let (ui_name, _) = parse_font(&gsetting("font-name").unwrap_or_else(|| "Cantarell 11".into()));
    let (mono_name, _) = parse_font(&gsetting("monospace-font-name").unwrap_or_else(|| "Monospace 11".into()));
    if let Some(bytes) = load_font(&ui_name) {
        fonts.font_data.insert("system-ui".into(), FontData::from_owned(bytes));
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "system-ui".into());
    }
    if let Some(bytes) = load_font(&mono_name) {
        fonts.font_data.insert("system-mono".into(), FontData::from_owned(bytes));
        fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "system-mono".into());
    }
    ctx.set_fonts(fonts);
}

impl Theme {
    pub fn window(&self) -> Color32 { if self.dark { rgb(0x24, 0x24, 0x24) } else { rgb(0xfa, 0xfa, 0xfa) } }
    pub fn header(&self) -> Color32 { if self.dark { rgb(0x30, 0x30, 0x30) } else { rgb(0xeb, 0xeb, 0xeb) } }
    pub fn view(&self) -> Color32 { if self.dark { rgb(0x1e, 0x1e, 0x1e) } else { rgb(0xff, 0xff, 0xff) } }
    pub fn popover(&self) -> Color32 { if self.dark { rgb(0x38, 0x38, 0x38) } else { rgb(0xff, 0xff, 0xff) } }
    pub fn border(&self) -> Color32 { if self.dark { rgb(0x44, 0x44, 0x44) } else { rgb(0xd5, 0xd5, 0xd5) } }
    pub fn text(&self) -> Color32 { if self.dark { rgb(0xff, 0xff, 0xff) } else { rgb(0x2e, 0x2e, 0x32) } }
    pub fn grid(&self) -> Color32 { if self.dark { rgb(0x2a, 0x2a, 0x2a) } else { rgb(0xec, 0xec, 0xec) } }
    pub fn stock_fill(&self) -> Color32 { if self.dark { rgb(0x3a, 0x30, 0x24) } else { rgb(0xe9, 0xdb, 0xc3) } }
    pub fn stock_stroke(&self) -> Color32 { if self.dark { rgb(0xa0, 0x84, 0x5c) } else { rgb(0xa0, 0x7c, 0x45) } }
    pub fn shape_idle(&self) -> Color32 { if self.dark { Color32::from_gray(150) } else { Color32::from_gray(100) } }
    pub fn shape_selected(&self) -> Color32 { if self.dark { Color32::WHITE } else { rgb(0x10, 0x10, 0x10) } }
    pub fn rapid(&self) -> Color32 { if self.dark { Color32::from_gray(110) } else { Color32::from_gray(170) } }
    pub fn hud(&self) -> Color32 { self.text() }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 { Color32::from_rgb(r, g, b) }

/// Per-operation colours (Adwaita palette; lighter tints on dark backgrounds).
pub fn op_color(i: usize, dark: bool) -> Color32 {
    const L: [Color32; 6] = [rgb(0x1c, 0x71, 0xd8), rgb(0xe6, 0x61, 0x00), rgb(0x26, 0xa2, 0x69), rgb(0x91, 0x41, 0xac), rgb(0xc8, 0x88, 0x00), rgb(0x21, 0x90, 0xa4)];
    const D: [Color32; 6] = [rgb(0x62, 0xa0, 0xea), rgb(0xff, 0xa3, 0x48), rgb(0x57, 0xe3, 0x89), rgb(0xdc, 0x8a, 0xdd), rgb(0xf8, 0xe4, 0x5c), rgb(0x5b, 0xc8, 0xaf)];
    if dark { D[i % 6] } else { L[i % 6] }
}

pub fn apply_style(ctx: &egui::Context, t: &Theme) {
    let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
    let (btn, btn_h, btn_a) = if t.dark {
        (rgb(0x3b, 0x3b, 0x3b), rgb(0x45, 0x45, 0x45), rgb(0x50, 0x50, 0x50))
    } else {
        (rgb(0xe6, 0xe6, 0xe6), rgb(0xdc, 0xdc, 0xdc), rgb(0xd0, 0xd0, 0xd0))
    };
    let r = Rounding::same(6.0);
    let edge = if t.dark { rgb(0x4e, 0x4e, 0x4e) } else { rgb(0xc4, 0xc4, 0xc4) };
    let text = Stroke::new(1.0_f32, t.text());
    v.panel_fill = t.window();
    v.window_fill = t.popover();
    v.extreme_bg_color = t.view();
    v.faint_bg_color = if t.dark { rgb(0x2a, 0x2a, 0x2a) } else { rgb(0xf2, 0xf2, 0xf2) };
    v.window_stroke = Stroke::new(1.0_f32, t.border());
    v.window_rounding = Rounding::same(12.0);
    v.menu_rounding = Rounding::same(9.0);
    v.selection.bg_fill = t.accent;
    v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.hyperlink_color = t.accent;
    v.widgets.noninteractive.bg_fill = t.window();
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, t.border());
    v.widgets.noninteractive.fg_stroke = text;
    v.widgets.noninteractive.rounding = r;
    for (w, bg) in [(&mut v.widgets.inactive, btn), (&mut v.widgets.hovered, btn_h), (&mut v.widgets.active, btn_a), (&mut v.widgets.open, btn_a)] {
        w.bg_fill = bg;
        w.weak_bg_fill = bg;
        w.bg_stroke = Stroke::new(1.0_f32, edge);
        w.fg_stroke = text;
        w.rounding = r;
        w.expansion = 0.0;
    }
    v.widgets.hovered.fg_stroke = text;
    v.widgets.active.fg_stroke = text;
    ctx.set_visuals(v);

    let mut s = (*ctx.style()).clone();
    let size = t.size.clamp(10.0, 24.0);
    s.text_styles = [
        (TextStyle::Small, FontId::new(size * 0.85, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(size, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(size, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(size * 1.15, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(size * 0.95, FontFamily::Monospace)),
    ].into();
    s.spacing.item_spacing = Vec2::new(8.0, 6.0);
    s.spacing.button_padding = Vec2::new(10.0, 4.0);
    s.spacing.interact_size = Vec2::new(36.0, 28.0);
    s.spacing.window_margin = egui::Margin::same(14.0);
    s.spacing.menu_margin = egui::Margin::same(6.0);
    s.spacing.indent = 16.0;
    ctx.set_style(s);
}
