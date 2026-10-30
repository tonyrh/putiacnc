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
    let col = ui.visuals().override_text_color.unwrap_or(vis.fg_stroke.color);
    let g = ui.painter().layout_no_wrap(text, font, col);
    ui.painter().galley(egui::pos2(rect.max.x - 8.0 - g.size().x, rect.center().y - g.size().y / 2.0), g, col);
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

/// The `ComboBox::width` that holds the widest of `labels` (in the button font) with its arrow and padding, so the box
/// keeps one width whichever option is selected.
pub fn combo_fit(ui: &Ui, labels: &[&str]) -> f32 {
    let sp = ui.spacing();
    let extra = sp.icon_spacing + sp.icon_width + 2.0 * sp.button_padding.x;
    let font = egui::TextStyle::Button.resolve(ui.style());
    labels.iter().map(|l| ui.fonts(|f| f.layout_no_wrap(l.to_string(), font.clone(), egui::Color32::WHITE).size().x)).fold(0.0_f32, f32::max).ceil() + extra + 1.0
}

/// `s` shortened with an ellipsis so that, shown in a combo box of `width`, it needs no more room than the box has.
pub fn combo_elide(ui: &Ui, s: &str, width: f32) -> String {
    let sp = ui.spacing();
    let max_w = width - (sp.icon_spacing + sp.icon_width + 2.0 * sp.button_padding.x) - 1.0;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let w = |t: &str| ui.fonts(|f| f.layout_no_wrap(t.to_string(), font.clone(), egui::Color32::WHITE).size().x);
    if w(s) <= max_w { return s.to_string(); }
    let mut t: Vec<char> = s.chars().collect();
    while !t.is_empty() {
        t.pop();
        let c: String = t.iter().collect::<String>().trim_end().to_string() + "…";
        if w(&c) <= max_w { return c; }
    }
    "…".into()
}

/// A checkbox that looks like the rest of the app: a square with softly rounded corners, the accent colour and a white tick when
/// ticked, a plain outline when not. Same use as `crate::widgets::check(ui, &mut flag, "text")`.
pub fn check(ui: &mut Ui, on: &mut bool, label: impl Into<egui::WidgetText>) -> Response {
    let icon = ui.spacing().icon_width;
    let gap = ui.spacing().icon_spacing;
    let wrap = ui.available_width() - icon - gap;
    let label: egui::WidgetText = label.into();
    let galley = if label.is_empty() { None } else { Some(label.into_galley(ui, None, wrap.max(20.0), egui::TextStyle::Button)) };
    let tw = galley.as_ref().map_or(0.0, |g| g.size().x + gap);
    let th = galley.as_ref().map_or(0.0, |g| g.size().y);
    let size = egui::vec2(icon + tw, ui.spacing().interact_size.y.max(th).max(icon));
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() { *on = !*on; resp.mark_changed(); }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *on, galley.as_ref().map_or("", |g| g.text())));
    if ui.is_rect_visible(rect) {
        let v = ui.visuals().clone();
        let enabled = ui.is_enabled();
        let w = v.widgets.style(&resp);
        let bx = egui::Rect::from_min_size(egui::pos2(rect.min.x, rect.center().y - icon / 2.0), egui::vec2(icon, icon));
        let fade = |c: egui::Color32| if enabled { c } else { c.gamma_multiply(0.45) };
        let (fill, stroke) = if *on {
            (v.selection.bg_fill, v.selection.bg_fill)
        } else {
            (v.extreme_bg_color, if resp.hovered() { v.widgets.hovered.fg_stroke.color } else { v.widgets.noninteractive.fg_stroke.color.gamma_multiply(0.55) })
        };
        ui.painter().rect(bx, 4.0, fade(fill), egui::Stroke::new(1.0_f32, fade(stroke)));
        if *on {
            let c = bx.center();
            let tick = egui::Stroke::new(2.0_f32, fade(egui::Color32::WHITE));
            ui.painter().line_segment([egui::pos2(c.x - 4.0, c.y + 0.5), egui::pos2(c.x - 1.2, c.y + 3.5)], tick);
            ui.painter().line_segment([egui::pos2(c.x - 1.2, c.y + 3.5), egui::pos2(c.x + 4.5, c.y - 3.5)], tick);
        }
        if let Some(g) = galley {
            let col = if enabled { w.fg_stroke.color } else { v.widgets.noninteractive.fg_stroke.color.gamma_multiply(0.5) };
            ui.painter().galley(egui::pos2(rect.min.x + icon + gap, rect.center().y - g.size().y / 2.0), g, col);
        }
    }
    resp
}

/// `check` that can be switched off (greyed out and not clickable).
pub fn check_if(ui: &mut Ui, enabled: bool, on: &mut bool, label: impl Into<egui::WidgetText>) -> Response {
    ui.add_enabled_ui(enabled, |ui| check(ui, on, label)).inner
}

/// A label that has been drawn and is waiting for the field beside it, so that the field's hover help can be shown on the label too.
#[derive(Clone)]
struct PendingLabel { resp: Response, frame: u64 }

fn pending_id() -> egui::Id { egui::Id::new("label_waiting_for_its_field") }

/// A right-aligned label for a row of a form; the field that follows (and gets its help through [`Tip::tip`]) shares that help with it.
pub fn label_cell(ui: &mut Ui, w: f32, text: impl Into<String>) -> Response {
    let text: String = text.into();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let g = ui.painter().layout_no_wrap(text.clone(), font, ui.visuals().text_color());
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), Sense::hover());
    ui.painter().galley(egui::pos2(rect.max.x - g.size().x, rect.center().y - g.size().y / 2.0), g, ui.visuals().text_color());
    wait_for_field(ui.ctx(), &resp, &text);
    resp
}

/// Remembers a label until the field next to it asks for its hover help.
pub fn wait_for_field(ctx: &egui::Context, label: &Response, _text: &str) {
    let frame = ctx.frame_nr();
    ctx.data_mut(|d| d.insert_temp(pending_id(), PendingLabel { resp: label.clone(), frame }));
    #[cfg(test)]
    tiplog::label(ctx, label.rect, _text);
}

/// Whether `field` is the control that belongs to `label`: on the same row, right after it.
fn beside(label: egui::Rect, field: egui::Rect) -> bool {
    field.left() >= label.right() - 1.0 && field.left() - label.right() <= 40.0 && label.center().y >= field.top() - 3.0 && label.center().y <= field.bottom() + 3.0
}

/// Hover help for a control and for the label beside it.
pub trait Tip {
    /// Like `on_hover_text`, and the label drawn just before this control in the same row shows the same help.
    fn tip(self, text: impl Into<egui::WidgetText>) -> Response;
}

impl Tip for Response {
    fn tip(self, text: impl Into<egui::WidgetText>) -> Response {
        let text: egui::WidgetText = text.into();
        let ctx = self.ctx.clone();
        let mut paired = None;
        if let Some(p) = ctx.data(|d| d.get_temp::<PendingLabel>(pending_id())) {
            if p.frame == ctx.frame_nr() && p.resp.layer_id == self.layer_id && beside(p.resp.rect, self.rect) {
                ctx.data_mut(|d| d.remove::<PendingLabel>(pending_id()));
                paired = Some(p.resp.rect);
                p.resp.on_hover_text(text.clone());
            }
        }
        #[cfg(test)]
        tiplog::field(&ctx, self.rect, paired, text.text());
        let _ = paired;
        // a greyed-out control keeps its help too, as its label does
        if self.enabled { self.on_hover_text(text) } else { self.on_disabled_hover_text(text) }
    }
}

/// Hover help over a rectangle that is not a widget, such as words painted next to a checkbox.
pub fn tip_rect(ui: &mut Ui, rect: egui::Rect, salt: &str, text: impl Into<egui::WidgetText>) -> Response {
    let r = ui.interact(rect, ui.id().with(("tip_rect", salt)), Sense::hover());
    #[cfg(test)]
    { let t: egui::WidgetText = text.into(); tiplog::painted(ui.ctx(), rect, t.text()); r.on_hover_text(t) }
    #[cfg(not(test))]
    r.on_hover_text(text)
}

/// What the tests need to know: every label, every control with help and which label got that help too.
#[cfg(test)]
pub mod tiplog {
    use eframe::egui::{self, Rect};
    #[derive(Clone, Default, Debug)]
    pub struct Log { pub labels: Vec<(Rect, String)>, pub fields: Vec<(Rect, Option<Rect>, String)>, pub painted: Vec<(Rect, String)> }
    fn id() -> egui::Id { egui::Id::new("tip_log_for_tests") }
    fn with(ctx: &egui::Context, f: impl FnOnce(&mut Log)) { ctx.data_mut(|d| f(d.get_temp_mut_or_default::<Log>(id()))) }
    pub fn label(ctx: &egui::Context, r: Rect, t: &str) { with(ctx, |l| l.labels.push((r, t.to_string()))) }
    pub fn field(ctx: &egui::Context, r: Rect, label: Option<Rect>, t: &str) { with(ctx, |l| l.fields.push((r, label, t.to_string()))) }
    pub fn painted(ctx: &egui::Context, r: Rect, t: &str) { with(ctx, |l| l.painted.push((r, t.to_string()))) }
    pub fn take(ctx: &egui::Context) -> Log { ctx.data_mut(|d| d.remove_temp::<Log>(id())).unwrap_or_default() }
}
