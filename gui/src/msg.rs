//! The look of a message, in one place: a level (error, warning or news) has one colour and one sign wherever it is shown
//! (the banner, the bars under the view tabs, the notes in dialogs, warnings in the panels and the update window).
use crate::app::Level;
use eframe::egui::{self, Color32, Pos2};

/// The colour of a level's text and sign. Softer than pure red or orange on the dark theme, which is hard to read.
pub fn colour(level: Level, dark: bool) -> Color32 {
    match (level, dark) {
        (Level::Error, true) => Color32::from_rgb(0xff, 0x7b, 0x63),
        (Level::Error, false) => Color32::from_rgb(0xa5, 0x1d, 0x2d),
        (Level::Warning, true) => Color32::from_rgb(0xff, 0x95, 0x2e),
        (Level::Warning, false) => Color32::from_rgb(0xd3, 0x54, 0x00),
        (Level::Info, true) => Color32::from_rgb(0x7c, 0xb7, 0xff),
        (Level::Info, false) => Color32::from_rgb(0x1f, 0x5f, 0xb5),
    }
}

/// Paints the sign of a level centred on `c`: a circle with "!" for an error, a triangle with "!" for a warning, a circle with "i" for news.
pub fn paint_sign(p: &egui::Painter, c: Pos2, level: Level, col: Color32) {
    let stroke = egui::Stroke::new(1.8_f32, Color32::WHITE);
    match level {
        Level::Error => {
            p.circle_filled(c, 8.0, col);
            p.line_segment([c + egui::vec2(0.0, -3.5), c + egui::vec2(0.0, 1.0)], stroke);
            p.circle_filled(c + egui::vec2(0.0, 4.0), 1.1, Color32::WHITE);
        }
        Level::Warning => {
            let tri = vec![c + egui::vec2(0.0, -8.5), c + egui::vec2(9.0, 7.0), c + egui::vec2(-9.0, 7.0)];
            p.add(egui::Shape::convex_polygon(tri, col, egui::Stroke::new(1.6_f32, col)));
            p.line_segment([c + egui::vec2(0.0, -2.5), c + egui::vec2(0.0, 2.0)], stroke);
            p.circle_filled(c + egui::vec2(0.0, 4.6), 1.0, Color32::WHITE);
        }
        Level::Info => {
            p.circle_filled(c, 8.0, col);
            p.circle_filled(c + egui::vec2(0.0, -3.8), 1.1, Color32::WHITE);
            p.line_segment([c + egui::vec2(0.0, -0.8), c + egui::vec2(0.0, 4.5)], stroke);
        }
    }
}

/// The sign as a widget, 18 x 20 points.
pub fn sign(ui: &mut egui::Ui, level: Level) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 20.0), egui::Sense::hover());
    paint_sign(ui.painter(), r.center(), level, colour(level, ui.visuals().dark_mode));
}

/// A message in a window or a panel: the level's sign, then the text in the level's colour, wrapping.
pub fn line(ui: &mut egui::Ui, level: Level, text: &str) -> egui::Response {
    let col = colour(level, ui.visuals().dark_mode);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        sign(ui, level);
        ui.add(egui::Label::new(egui::RichText::new(crate::widgets::localize(ui, text)).color(col)).wrap(true))
    }).inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_level_has_its_own_colour_in_both_themes() {
        for dark in [true, false] {
            let c = [Level::Error, Level::Warning, Level::Info].map(|l| colour(l, dark));
            assert!(c[0] != c[1] && c[1] != c[2] && c[0] != c[2], "{c:?}");
        }
        // and the colours are the ones the banner has always had
        assert_eq!(colour(Level::Warning, true), Color32::from_rgb(0xff, 0x95, 0x2e));
    }

    /// A message is raised with `say` and its level, never by setting the banner's text directly (which shows as an error whatever it is).
    #[test]
    fn nothing_sets_the_banner_text_without_a_level() {
        fn walk(dir: &std::path::Path, bad: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() { walk(&p, bad); continue; }
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                // `say` itself, the preview that puts the banner back as it was, and the model tool's own field are the exceptions
                if !name.ends_with(".rs") || ["msg.rs", "edit.rs", "preview.rs", "model3d.rs"].contains(&name.as_str()) { continue; }
                for (n, l) in std::fs::read_to_string(&p).unwrap().lines().enumerate() {
                    if l.contains("app.error = Some(") || l.contains("self.error = Some(") || l.contains("a.error = Some(") { bad.push(format!("{name}:{}", n + 1)); }
                }
            }
        }
        let mut bad = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut bad);
        assert!(bad.is_empty(), "set with app.say(level, text) instead: {bad:?}");
    }

    #[test]
    fn nothing_to_export_is_a_warning_not_an_error() {
        let mut a = crate::app::App::new(None);
        a.project.data.ops.clear();
        a.export_gcode_dialog();
        assert!(a.error.is_some());
        assert_eq!(a.banner_level_now(), Level::Warning);
    }
}
