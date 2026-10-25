//! Small shared widgets.
use eframe::egui::{self, text::{CCursor, CCursorRange}, Align, Response, Sense, Ui};

fn fmt_val(v: f64, dec: usize) -> String {
    let s = format!("{:.*}", dec, v);
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// A numeric field like egui's `DragValue` (drag to change, click to type) but with the text
/// right-aligned. `speed` is the change per dragged pixel, `dec` the maximum decimals shown.
pub fn num(ui: &mut Ui, size: [f32; 2], v: &mut f64, lo: f64, hi: f64, speed: f64, dec: usize, suffix: &str) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(size[0], size[1]), Sense::click_and_drag());
    let id = resp.id;
    let edit_id = id.with("edit");
    let mut editing: Option<String> = ui.data(|d| d.get_temp(edit_id));

    if let Some(mut text) = editing.take() {
        let first = ui.data(|d| d.get_temp::<bool>(id.with("first"))).unwrap_or(true);
        let te_id = id.with("te");
        let mut child = ui.child_ui(rect, egui::Layout::left_to_right(Align::Center));
        let out = child.add(egui::TextEdit::singleline(&mut text).id(te_id).horizontal_align(Align::RIGHT).margin(egui::vec2(6.0, 2.0)).desired_width((size[0] - 12.0).max(10.0)));
        if first {
            out.request_focus();
            if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), te_id) {
                st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(text.chars().count()))));
                st.store(ui.ctx(), te_id);
            }
            ui.data_mut(|d| d.insert_temp(id.with("first"), false));
        }
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if out.lost_focus() || esc {
            if !esc {
                if let Ok(x) = text.trim().replace(',', ".").parse::<f64>() {
                    let nv = x.clamp(lo, hi);
                    if (nv - *v).abs() > f64::EPSILON { *v = nv; resp.mark_changed(); }
                }
            }
            ui.data_mut(|d| { d.remove::<String>(edit_id); d.remove::<bool>(id.with("first")); });
        } else {
            ui.data_mut(|d| d.insert_temp(edit_id, text));
        }
        return resp;
    }

    if resp.dragged() {
        let nv = (*v + resp.drag_delta().x as f64 * speed).clamp(lo, hi);
        if nv != *v { *v = nv; resp.mark_changed(); }
    }
    if resp.hovered() || resp.dragged() { ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal); }
    if resp.clicked() {
        ui.data_mut(|d| { d.insert_temp(edit_id, fmt_val(*v, dec)); d.insert_temp(id.with("first"), true); });
        ui.ctx().request_repaint();
    }
    let vis = *ui.style().interact(&resp);
    ui.painter().rect(rect, vis.rounding, vis.weak_bg_fill, vis.bg_stroke);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let text = format!("{}{}", fmt_val(*v, dec), suffix);
    let g = ui.painter().layout_no_wrap(text, font, vis.fg_stroke.color);
    ui.painter().galley(egui::pos2(rect.max.x - 8.0 - g.size().x, rect.center().y - g.size().y / 2.0), g, vis.fg_stroke.color);
    resp
}

/// egui closes a combo box popup on any click inside it. A click that does not pick anything (a search box, a heading) should leave it
/// open: call this inside the popup, and when it says so, `reopen_combo` right after the combo box.
pub fn click_inside_popup(ui: &Ui) -> bool {
    ui.input(|i| i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| ui.min_rect().expand(8.0).contains(p)))
}

/// Open the popup of the combo box made with `ComboBox::from_id_source(id)` again.
pub fn reopen_combo(ui: &Ui, id: impl std::hash::Hash) {
    let pid = ui.make_persistent_id(egui::Id::new(id)).with("popup");
    ui.memory_mut(|m| m.open_popup(pid));
}
