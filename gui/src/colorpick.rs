//! The colour picker: a swatch that opens a small window with standard colours (mid tones that read on a light and a dark background),
//! the colours the user added, and a hex field (#RRGGBB, editable, so a colour can be pasted). "Colour wheel" switches to a hue ring
//! with a saturation/value square, where the colour can be added to the palette.
use eframe::egui::{self, ecolor::Hsva, epaint::Mesh, Color32, Id, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use std::sync::Mutex;

/// The standard colours: eight to a row.
pub const STANDARD: [&str; 16] = [
    "#E53935", "#F4511E", "#FB8C00", "#F9A825", "#7CB342", "#43A047", "#00897B", "#00ACC1",
    "#039BE5", "#1E88E5", "#5C6BC0", "#7E57C2", "#AB47BC", "#D81B60", "#8D6E63", "#757575",
];
const MAX_CUSTOM: usize = 24;
static CUSTOM: Mutex<Vec<[u8; 3]>> = Mutex::new(Vec::new());

/// "#RRGGBB" in capitals.
pub fn hex(c: [u8; 3]) -> String { format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]) }

/// A colour typed or pasted as "#RRGGBB", "RRGGBB" or "#RGB" (blanks around it are ignored).
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if !h.is_ascii() || !h.chars().all(|c| c.is_ascii_hexdigit()) { return None; }
    match h.len() {
        6 => { let v = u32::from_str_radix(h, 16).ok()?; Some([(v >> 16) as u8, (v >> 8) as u8, v as u8]) }
        3 => { let d: Vec<u8> = h.chars().filter_map(|c| c.to_digit(16)).map(|d| (d * 17) as u8).collect(); Some([d[0], d[1], d[2]]) }
        _ => None,
    }
}

fn standard() -> Vec<[u8; 3]> { STANDARD.iter().filter_map(|h| parse_hex(h)).collect() }

/// The colours the user added to the palette, as saved in the settings.
pub fn custom_hex() -> Vec<String> { CUSTOM.lock().map(|v| v.iter().map(|c| hex(*c)).collect()).unwrap_or_default() }
/// Loads the saved colours.
pub fn set_custom(list: &[String]) { if let Ok(mut v) = CUSTOM.lock() { *v = list.iter().filter_map(|h| parse_hex(h)).take(MAX_CUSTOM).collect(); } }
fn custom() -> Vec<[u8; 3]> { CUSTOM.lock().map(|v| v.clone()).unwrap_or_default() }
fn add_custom(c: [u8; 3]) { if let Ok(mut v) = CUSTOM.lock() { if !v.contains(&c) && !standard().contains(&c) { v.push(c); if v.len() > MAX_CUSTOM { v.remove(0); } } } }
fn remove_custom(c: [u8; 3]) { if let Ok(mut v) = CUSTOM.lock() { v.retain(|x| *x != c); } }

fn col32(c: [u8; 3]) -> Color32 { Color32::from_rgb(c[0], c[1], c[2]) }

/// A colour swatch that opens the picker. `salt` tells apart several pickers in one place (the layer's id, say).
/// `tip` is the tooltip of the swatch (not shown while the picker is open: it would cover it).
pub fn button(ui: &mut Ui, salt: impl std::hash::Hash, rgb: &mut [u8; 3], tip: &str) -> Response {
    let id = ui.id().with(("colorpick", salt));
    let mut r = ui.add(egui::Button::new("").fill(col32(*rgb)).min_size(Vec2::new(36.0, ui.spacing().interact_size.y)));
    if r.clicked() { ui.memory_mut(|m| m.toggle_popup(id)); }
    if !ui.memory(|m| m.is_popup_open(id)) { r = r.on_hover_text(tip); }
    let mut changed = false;
    if ui.memory(|m| m.is_popup_open(id)) {
        let area = egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(r.rect.left_bottom() + Vec2::new(0.0, 4.0)).constrain(true).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| { ui.set_width(196.0); changed = panel(ui, id, rgb); });
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) || (area.response.clicked_elsewhere() && !r.clicked()) { ui.memory_mut(|m| m.close_popup()); }
    }
    if changed { r.mark_changed(); }
    r
}

fn swatches(ui: &mut Ui, list: &[[u8; 3]], rgb: &mut [u8; 3], removable: bool) -> bool {
    let mut changed = false;
    let mut remove = None;
    ui.spacing_mut().item_spacing = Vec2::splat(4.0);
    for row in list.chunks(8) {
        ui.horizontal(|ui| {
            for &c in row {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                ui.painter().rect_filled(rect, 4.0, col32(c));
                if *rgb == c { ui.painter().rect_stroke(rect.expand(1.5), 5.0, Stroke::new(2.0_f32, ui.visuals().text_color())); }
                else if resp.hovered() { ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.5_f32, ui.visuals().text_color().gamma_multiply(0.7))); }
                let resp = resp.on_hover_text(if removable { format!("{}. Right-click to remove it from the palette.", hex(c)) } else { hex(c) });
                if resp.clicked() { *rgb = c; changed = true; }
                if removable && resp.secondary_clicked() { remove = Some(c); }
            }
        });
    }
    if let Some(c) = remove { remove_custom(c); }
    changed
}

fn panel(ui: &mut Ui, id: Id, rgb: &mut [u8; 3]) -> bool {
    let mut changed = false;
    let mut other = false; // changed by something other than the hex field
    let wheel_id = id.with("wheel");
    let mut wheel = ui.data(|d| d.get_temp::<bool>(wheel_id)).unwrap_or(false);
    // the two ways to choose, as a segmented control like the app's others
    let pick = ui.vertical_centered(|ui| crate::zoomcard::seg2(ui, ["Palette", "Wheel"], ["The standard colours and yours", "Any colour, on a wheel"], wheel as usize).0).inner;
    if let Some(i) = pick { wheel = i == 1; ui.data_mut(|d| d.insert_temp(wheel_id, wheel)); }
    ui.add_space(6.0);
    if !wheel {
        other |= swatches(ui, &standard(), rgb, false);
        let cu = custom();
        if !cu.is_empty() {
            ui.add_space(4.0);
            ui.weak("Yours");
            other |= swatches(ui, &cu, rgb, true);
        }
    } else {
        let hid = id.with("hsva");
        let mut hsva = ui.data(|d| d.get_temp::<Hsva>(hid)).filter(|h| h.to_srgb() == *rgb).unwrap_or_else(|| Hsva::from_srgb(*rgb));
        if ui.vertical_centered(|ui| wheel_widget(ui, id, &mut hsva)).inner { *rgb = hsva.to_srgb(); other = true; }
        ui.data_mut(|d| d.insert_temp(hid, hsva));
    }
    ui.add_space(6.0);
    // the hex field: editable, takes a pasted colour
    let tid = id.with("hexfield");
    let focused = ui.memory(|m| m.has_focus(tid));
    let mut text = if focused { ui.data(|d| d.get_temp::<String>(tid)).unwrap_or_else(|| hex(*rgb)) } else { hex(*rgb) };
    let valid = parse_hex(&text).is_some();
    ui.horizontal(|ui| {
        // centred: the row is the field, the preview and (on the wheel) the plus button
        let sp = ui.spacing().item_spacing.x;
        let row_w = 84.0 + sp + 24.0 + if wheel { sp + 24.0 } else { 0.0 };
        ui.add_space(((ui.available_width() - row_w) / 2.0).max(0.0));
        let mut te = egui::TextEdit::singleline(&mut text).id(tid).desired_width(84.0).font(egui::TextStyle::Monospace);
        if !valid { te = te.text_color(ui.visuals().error_fg_color); }
        let r = ui.add(te).on_hover_text("The colour as #RRGGBB. Type or paste one here.");
        if r.changed() { if let Some(c) = parse_hex(&text) { *rgb = c; changed = true; } }
        if r.has_focus() || r.changed() { ui.data_mut(|d| d.insert_temp(tid, text.clone())); }
        let (rect, _) = ui.allocate_exact_size(Vec2::new(24.0, 18.0), Sense::hover());
        ui.painter().rect_filled(rect, 3.0, col32(*rgb));
        // on the wheel, a "+" keeps the colour with the standard ones
        if wheel {
            let known = standard().contains(rgb) || custom().contains(rgb);
            if ui.add_enabled(!known, egui::Button::new("+").min_size(Vec2::new(24.0, 18.0))).on_hover_text("Add this colour to the palette").on_disabled_hover_text("This colour is already in the palette").clicked() { add_custom(*rgb); }
        }
    });
    if other { ui.memory_mut(|m| m.surrender_focus(tid)); ui.data_mut(|d| d.remove::<String>(tid)); }
    changed || other
}

/// A hue ring with a saturation/value square inside it. True when the colour changed.
fn wheel_widget(ui: &mut Ui, id: Id, hsva: &mut Hsva) -> bool {
    let size = 176.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let c = rect.center();
    let (ro, ri) = (size / 2.0, size / 2.0 - 16.0);
    let side = (ri - 5.0) * std::f32::consts::SQRT_2;
    let sq = Rect::from_center_size(c, Vec2::splat(side));
    let mode_id = id.with("wmode");
    let mut changed = false;
    let pos = ui.input(|i| i.pointer.interact_pos());
    let pressed = resp.hovered() && ui.input(|i| i.pointer.primary_pressed());
    if pressed { if let Some(p) = pos { let d = (p - c).length(); ui.data_mut(|dd| dd.insert_temp(mode_id, if d > ro + 3.0 { 0u8 } else if d > ri - 2.0 { 1 } else { 2 })); } }
    if pressed || resp.dragged() {
        if let Some(p) = pos {
            match ui.data(|d| d.get_temp::<u8>(mode_id)).unwrap_or(0) {
                1 => { hsva.h = ((p.y - c.y).atan2(p.x - c.x) / std::f32::consts::TAU).rem_euclid(1.0); changed = true; }
                2 => { hsva.s = ((p.x - sq.left()) / side).clamp(0.0, 1.0); hsva.v = 1.0 - ((p.y - sq.top()) / side).clamp(0.0, 1.0); changed = true; }
                _ => {}
            }
        }
    }
    let painter = ui.painter_at(rect);
    // the ring
    let mut ring = Mesh::default();
    let n = 120;
    for i in 0..=n {
        let h = i as f32 / n as f32;
        let (s, co) = (h * std::f32::consts::TAU).sin_cos();
        let col = Color32::from(Hsva::new(h, 1.0, 1.0, 1.0));
        ring.colored_vertex(Pos2::new(c.x + co * ro, c.y + s * ro), col);
        ring.colored_vertex(Pos2::new(c.x + co * ri, c.y + s * ri), col);
        if i > 0 { let k = (i * 2) as u32; ring.add_triangle(k - 2, k - 1, k); ring.add_triangle(k - 1, k + 1, k); }
    }
    painter.add(egui::Shape::mesh(ring));
    // the square: saturation to the right, value upwards, at the current hue
    let g = 12;
    let mut m = Mesh::default();
    for yy in 0..=g { for xx in 0..=g {
        let (fx, fy) = (xx as f32 / g as f32, yy as f32 / g as f32);
        m.colored_vertex(Pos2::new(sq.left() + fx * side, sq.top() + fy * side), Color32::from(Hsva::new(hsva.h, fx, 1.0 - fy, 1.0)));
    } }
    for yy in 0..g { for xx in 0..g {
        let a = (yy * (g + 1) + xx) as u32;
        let w = (g + 1) as u32;
        m.add_triangle(a, a + 1, a + w); m.add_triangle(a + 1, a + w + 1, a + w);
    } }
    painter.add(egui::Shape::mesh(m));
    let knob = |p: Pos2| { painter.circle_stroke(p, 6.0, Stroke::new(3.0_f32, Color32::BLACK)); painter.circle_stroke(p, 6.0, Stroke::new(1.5_f32, Color32::WHITE)); };
    let a = hsva.h * std::f32::consts::TAU;
    let rm = (ro + ri) / 2.0;
    knob(Pos2::new(c.x + a.cos() * rm, c.y + a.sin() * rm));
    knob(Pos2::new(sq.left() + hsva.s * side, sq.top() + (1.0 - hsva.v) * side));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colours_are_read_in_the_usual_ways() {
        assert_eq!(parse_hex("#FFFFFF"), Some([255, 255, 255]));
        assert_eq!(parse_hex("  e53935 "), Some([0xE5, 0x39, 0x35]));
        assert_eq!(parse_hex("#f80"), Some([0xFF, 0x88, 0x00]));
        for bad in ["", "#", "#12345", "#GG0000", "red", "#1234567"] { assert_eq!(parse_hex(bad), None, "{bad}"); }
        assert_eq!(hex([0xE5, 0x39, 0x35]), "#E53935");
    }

    #[test]
    fn the_standard_colours_are_valid_and_distinct() {
        let s = standard();
        assert_eq!(s.len(), STANDARD.len());
        for (i, a) in s.iter().enumerate() { assert!(!s[i + 1..].contains(a)); }
        // none is so light or so dark that it would vanish on a light or a dark background
        for c in &s { let l = 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32; assert!((70.0..=215.0).contains(&l), "{c:?} {l}"); }
    }

    #[test]
    fn added_colours_are_kept_without_duplicates_and_can_be_removed() {
        set_custom(&[]);
        add_custom([1, 2, 3]); add_custom([1, 2, 3]);
        add_custom(standard()[0]);
        assert_eq!(custom_hex(), vec!["#010203".to_string()]);
        set_custom(&custom_hex());
        assert_eq!(custom(), vec![[1, 2, 3]]);
        remove_custom([1, 2, 3]);
        assert!(custom().is_empty());
    }
}
