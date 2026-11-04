//! Small shared widgets.
use eframe::egui::{self, text::{CCursor, CCursorRange}, Align, Response, Sense, Ui};

fn fmt_val(v: f64, dec: usize) -> String {
    let s = format!("{:.*}", dec, v);
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// Whether lengths are shown in inches (the app's Display units setting). Kept in the egui context so every field can ask for it
/// without being handed the app; stored values are always mm.
pub fn set_inches(ctx: &egui::Context, on: bool) { ctx.data_mut(|d| d.insert_temp(egui::Id::new("display_inches"), on)); }
pub fn inches(ui: &Ui) -> bool { ui.data(|d| d.get_temp::<bool>(egui::Id::new("display_inches"))).unwrap_or(false) }
/// "mm" or "in", for the unit after a length field.
pub fn len_unit(ui: &Ui) -> &'static str { if inches(ui) { "in" } else { "mm" } }
/// The speed unit the app shows (name, mm/min per unit), kept in the context like the length unit.
pub fn set_speed_unit(ctx: &egui::Context, u: (&'static str, f64)) { ctx.data_mut(|d| d.insert_temp(egui::Id::new("display_speed"), u)); }
/// A feed rate given in mm/min, in the shown speed unit with its name ("800 mm/min", "31.5 in/min").
pub fn fmt_speed(ui: &Ui, mm_min: f64) -> String {
    let (n, f) = ui.data(|d| d.get_temp::<(&'static str, f64)>(egui::Id::new("display_speed"))).unwrap_or(("mm/min", 1.0));
    format!("{} {}", fmt_val(mm_min / f.max(1e-9), 2), n)
}

/// Whether inch fields show fractions (see `fmt_inch`).
pub fn set_fractions(ctx: &egui::Context, on: bool) { ctx.data_mut(|d| d.insert_temp(egui::Id::new("display_fractions"), on)); }
fn fractions(ui: &Ui) -> bool { ui.data(|d| d.get_temp::<bool>(egui::Id::new("display_fractions"))).unwrap_or(false) }

/// A message written in mm, with its lengths in the shown unit (see `units::localize`).
pub fn localize(ui: &Ui, text: &str) -> String { crate::units::localize(text, inches(ui), fractions(ui)) }

/// An inch value as an exact fraction of an inch (to 1/64: `3/16`, `1 3/16`), when it is one.
fn exact_fraction(v: f64) -> Option<String> {
    let n = (v.abs() * 64.0).round();
    if n > 0.0 && (v.abs() * 64.0 - n).abs() < 0.03 { Some(fraction_text(v, n as u64)) } else { None }
}

fn fraction_text(v: f64, n: u64) -> String {
    let (whole, mut rest, mut den) = (n / 64, n % 64, 64u64);
    while rest != 0 && rest % 2 == 0 { rest /= 2; den /= 2; }
    let sign = if v < 0.0 { "-" } else { "" };
    if rest == 0 { format!("{sign}{whole}") } else if whole == 0 { format!("{sign}{rest}/{den}") } else { format!("{sign}{whole} {rest}/{den}") }
}

/// An inch value for display: with `frac`, as a fraction of an inch to the nearest 1/64 (`3/16`, `2 1/4`) from 1/16 up, and when it is
/// an exact fraction below that; otherwise (and for the tiny values of tolerances) as a decimal.
pub fn fmt_inch(v: f64, dec: usize, frac: bool) -> String {
    if frac {
        if let Some(t) = exact_fraction(v) { return t; }
        if v.abs() >= 1.0 / 16.0 { return fraction_text(v, (v.abs() * 64.0).round() as u64); }
    }
    fmt_val(v, dec)
}

/// What a field starts with when it is clicked to be typed in: an exact fraction, or else the number to its decimals, so that
/// leaving it as it is changes nothing.
fn fmt_inch_edit(v: f64, dec: usize, frac: bool) -> String {
    if frac { if let Some(t) = exact_fraction(v) { return t; } }
    fmt_val(v, dec)
}

/// mm per shown unit.
pub fn len_k(ui: &Ui) -> f64 { if inches(ui) { 25.4 } else { 1.0 } }

/// A length field: `v` is in mm, the field shows and takes mm or inches as the Display units say. `lo`, `hi`, `speed` and `dec` are
/// given for mm; in inches the field gets two more decimals (a thousandth of an inch is 0.0254 mm).
pub fn len(ui: &mut Ui, size: [f32; 2], v: &mut f64, lo: f64, hi: f64, speed: f64, dec: usize, suffix: &str) -> Response {
    let k = len_k(ui);
    if k == 1.0 { return num_with(ui, size, v, lo, hi, speed, dec, suffix, Some(1.0)); }
    let mut x = *v / k;
    let r = num_with(ui, size, &mut x, lo / k, hi / k, speed / k, dec + 2, suffix, Some(k));
    if r.changed() { *v = (x * k).clamp(lo, hi); }
    r
}

/// A length for a message or a readout, in the shown unit with its name ("12.5 mm", "0.492 in").
pub fn fmt_len(ui: &Ui, mm: f64) -> String { if inches(ui) { format!("{} in", fmt_inch(mm / 25.4, 3, fractions(ui))) } else { format!("{} mm", fmt_val(mm, 2)) } }

/// What a number field takes: a decimal (comma or point), a fraction (`1/8`, `3/16`) or a mixed number (`1 3/16`, `1-3/16`), with an
/// optional minus sign. Spaces round it and a trailing inch mark (`"` or `in`) are ignored.
pub fn parse_number(text: &str) -> Option<f64> { parse_number_unit(text).map(|(x, _)| x) }

/// [`parse_number`] for a length in a unit of `unit_mm` mm: a unit typed after the number (`mm`, `cm`, `m`, `in` or `"`) is converted.
pub fn parse_length(text: &str, unit_mm: f64) -> Option<f64> {
    let (x, u) = parse_number_unit(text)?;
    Some(match u { Some(u) if (u - unit_mm).abs() > 1e-12 => x * u / unit_mm, _ => x })
}

/// The number and, when one is typed after it, the unit as mm per unit.
fn parse_number_unit(text: &str) -> Option<(f64, Option<f64>)> {
    let t = text.trim();
    let low = t.to_lowercase();
    let (body, unit) = if let Some(b) = low.strip_suffix("mm") { (b, Some(1.0)) }
        else if let Some(b) = low.strip_suffix("cm") { (b, Some(10.0)) }
        else if let Some(b) = low.strip_suffix("in") { (b, Some(25.4)) }
        else if let Some(b) = low.strip_suffix('"') { (b, Some(25.4)) }
        else if let Some(b) = low.strip_suffix('m') { (b, Some(1000.0)) }
        else { (low.as_str(), None) };
    parse_plain(body).map(|x| (x, unit))
}

fn parse_plain(text: &str) -> Option<f64> {
    let t = text.trim().replace(',', ".");
    if let Ok(x) = t.parse::<f64>() { return Some(x); }
    let (neg, t) = match t.strip_prefix('-') { Some(r) => (true, r.trim()), None => (false, t.as_str()) };
    let frac = |s: &str| -> Option<f64> {
        let (n, d) = s.split_once('/')?;
        let (n, d) = (n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?);
        (d != 0.0 && n >= 0.0 && d > 0.0).then_some(n / d)
    };
    let v = if let Some(x) = frac(t) { x } else {
        // a whole number, then a fraction, after a space or a hyphen
        let (w, r) = t.split_once([' ', '-'])?;
        w.trim().parse::<f64>().ok().filter(|w| *w >= 0.0)? + frac(r)?
    };
    Some(if neg { -v } else { v })
}

/// Width of a number field in the dialogs.
pub const FIELD_W: f32 = 70.0;

/// The number fields drawn in the last frame and the one before it, in drawing order, with the layer each belongs to: the order Tab follows.
#[derive(Clone, Default)]
struct FieldOrder { frame: u64, now: Vec<(egui::Id, egui::LayerId)>, before: Vec<(egui::Id, egui::LayerId)> }

fn note_field(ui: &Ui, id: egui::Id) {
    let key = egui::Id::new("num_field_order");
    let (frame, layer) = (ui.ctx().frame_nr(), ui.layer_id());
    ui.ctx().data_mut(|d| {
        let o = d.get_temp_mut_or_default::<FieldOrder>(key);
        if o.frame != frame { o.before = std::mem::take(&mut o.now); o.frame = frame; }
        o.now.push((id, layer));
    });
}

/// The number field after (or, with `back`, before) `id` in the same window or panel, going round at the ends. The order is the one of the
/// last complete frame, so a field that has just appeared is not in it yet.
fn neighbour_field(ui: &Ui, id: egui::Id, back: bool) -> Option<egui::Id> {
    let layer = ui.layer_id();
    let o: FieldOrder = ui.ctx().data(|d| d.get_temp(egui::Id::new("num_field_order")))?;
    let list: Vec<egui::Id> = o.before.iter().filter(|(_, l)| *l == layer).map(|(i, _)| *i).collect();
    let at = list.iter().position(|&i| i == id)?;
    let n = list.len();
    Some(list[if back { (at + n - 1) % n } else { (at + 1) % n }])
}

/// A numeric field like egui's `DragValue` (drag to change, click to type) but with the text
/// right-aligned. `speed` is the change per dragged pixel, `dec` the maximum decimals shown.
pub fn num(ui: &mut Ui, size: [f32; 2], v: &mut f64, lo: f64, hi: f64, speed: f64, dec: usize, suffix: &str) -> Response {
    num_with(ui, size, v, lo, hi, speed, dec, suffix, None)
}

/// [`num`] for a length: `unit_mm` is the size of the shown unit in mm (1 or 25.4), so that typed text with a unit (`3 mm`, `1/8"`,
/// `2.5cm`) is converted to it.
#[allow(clippy::too_many_arguments)]
pub fn num_with(ui: &mut Ui, size: [f32; 2], v: &mut f64, lo: f64, hi: f64, speed: f64, dec: usize, suffix: &str, unit_mm: Option<f64>) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(size[0], size[1]), Sense::click_and_drag());
    let id = resp.id;
    let frac = unit_mm.is_some_and(|k| k > 25.0) && fractions(ui);
    let shown = |x: f64| if frac { fmt_inch(x, dec, true) } else { fmt_val(x, dec) };
    let edit_id = id.with("edit");
    let mut editing: Option<String> = ui.data(|d| d.get_temp(edit_id));
    note_field(ui, id);
    let start_edit = |ui: &mut Ui, v: f64| {
        ui.data_mut(|d| { let t0 = if frac { fmt_inch_edit(v, dec, true) } else { fmt_val(v, dec) }; d.insert_temp(id.with("orig"), t0.clone()); d.insert_temp(edit_id, t0); d.insert_temp(id.with("first"), true); });
        ui.ctx().request_repaint();
    };

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
        // Tab and Shift+Tab take the value and open the next or the previous number field of the same window or panel: the field says it
        // wants the Tab key, so egui does not move the focus on its own
        ui.memory_mut(|m| m.set_focus_lock_filter(te_id, egui::EventFilter { tab: true, ..Default::default() }));
        let tab = if out.has_focus() { ui.input(|i| i.key_pressed(egui::Key::Tab).then_some(i.modifiers.shift)) } else { None };
        if let Some(back) = tab { if let Some(to) = neighbour_field(ui, id, back) { ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("num_tab_to"), to)); } }
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if out.lost_focus() || esc || tab.is_some() {
            // text left as it was when the field was clicked changes nothing (the value may have more decimals than the text shows)
            let untouched = ui.data(|d| d.get_temp::<String>(id.with("orig"))).is_some_and(|o| o == text);
            if !esc && !untouched {
                if let Some(x) = match unit_mm { Some(k) => parse_length(&text, k), None => parse_number(&text) } {
                    let nv = x.clamp(lo, hi);
                    if (nv - *v).abs() > f64::EPSILON { *v = nv; resp.mark_changed(); }
                }
            }
            ui.data_mut(|d| { d.remove::<String>(edit_id); d.remove::<String>(id.with("orig")); d.remove::<bool>(id.with("first")); });
        } else {
            ui.data_mut(|d| d.insert_temp(edit_id, text));
        }
        return resp;
    }

    // the field being edited makes a child ui, which uses up one of the parent's automatic widget ids; making one here too keeps the ids of
    // the fields after this one the same whether or not it is being edited (the order Tab follows, and every field's state, depend on them)
    let _same_ids = ui.child_ui(rect, egui::Layout::left_to_right(Align::Center));
    if resp.dragged() {
        let nv = (*v + resp.drag_delta().x as f64 * speed).clamp(lo, hi);
        if nv != *v { *v = nv; resp.mark_changed(); }
    }
    if resp.hovered() || resp.dragged() { ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal); }
    // a click opens the field for typing, and so does the keyboard arriving here (Tab or Shift+Tab from the field before or after: the one
    // being edited is closed by the same key, with its value taken)
    let asked = ui.ctx().data(|d| d.get_temp::<egui::Id>(egui::Id::new("num_tab_to"))) == Some(id);
    if asked { ui.ctx().data_mut(|d| d.remove::<egui::Id>(egui::Id::new("num_tab_to"))); }
    if resp.clicked() || resp.gained_focus() || asked { start_edit(ui, *v); }
    let vis = *ui.style().interact(&resp);
    ui.painter().rect(rect, vis.rounding, vis.weak_bg_fill, vis.bg_stroke);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let text = format!("{}{}", shown(*v), suffix);
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
    elide_text(ui, s, width - (sp.icon_spacing + sp.icon_width + 2.0 * sp.button_padding.x) - 1.0)
}

/// `s` cut to fit `max_w` points in the button font, with an ellipsis when something was left out.
pub fn elide_text(ui: &Ui, s: &str, max_w: f32) -> String {
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

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn with_units<R>(inches: bool, f: impl FnOnce(&mut Ui) -> R) -> R {
        let ctx = egui::Context::default();
        let mut out = None;
        let mut f = Some(f);
        let _ = ctx.run(Default::default(), |ctx| {
            set_inches(ctx, inches);
            egui::CentralPanel::default().show(ctx, |ui| { if let Some(f) = f.take() { out = Some(f(ui)); } });
        });
        out.unwrap()
    }

    #[test]
    fn fields_take_fractions() {
        let p = parse_number;
        assert_eq!(p("1/8"), Some(0.125));
        assert_eq!(p("3/16"), Some(0.1875));
        assert_eq!(p("1 3/16"), Some(1.1875));
        assert_eq!(p("1-1/2\""), Some(1.5));
        assert_eq!(p("-1/4"), Some(-0.25));
        assert_eq!(p(" 2,5 "), Some(2.5));
        assert_eq!(p("0.75 in"), Some(0.75));
        assert_eq!(p("1/0"), None);
        assert_eq!(p("a/b"), None);
        assert_eq!(p(""), None);
    }

    /// Clicks the field at (30, 15), then clicks away from it; returns the value after that.
    fn click_and_leave(v0: f64, dec: usize) -> f64 {
        use egui::{Event, PointerButton, Pos2, RawInput, Modifiers};
        let ctx = egui::Context::default();
        let mut v = v0;
        let frame = |t: f64, events: Vec<Event>, v: &mut f64| {
            let input = RawInput { time: Some(t), events, screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 200.0))), ..Default::default() };
            let _ = ctx.run(input, |ctx| { egui::CentralPanel::default().show(ctx, |ui| { num(ui, [70.0, 22.0], v, 0.0, 100.0, 0.1, dec, ""); }); });
        };
        let click = |p: Pos2| vec![
            Event::PointerMoved(p),
            Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
            Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE },
        ];
        frame(0.0, vec![Event::PointerMoved(Pos2::new(30.0, 15.0))], &mut v);
        frame(0.1, click(Pos2::new(30.0, 15.0)), &mut v);
        for i in 0..3 { frame(0.2 + 0.1 * i as f64, vec![], &mut v); }
        frame(0.6, click(Pos2::new(300.0, 150.0)), &mut v);
        for i in 0..3 { frame(0.7 + 0.1 * i as f64, vec![], &mut v); }
        v
    }

    #[test]
    fn clicking_a_field_and_leaving_it_changes_nothing() {
        for v0 in [0.123456, 1.0, 2.0 / 3.0, 12.7, 0.0394] {
            assert_eq!(click_and_leave(v0, 2), v0, "value {v0}");
        }
    }

    #[test]
    fn inches_as_fractions() {
        assert_eq!(fmt_inch(0.1875, 4, true), "3/16");
        assert_eq!(fmt_inch(1.1875, 4, true), "1 3/16");
        assert_eq!(fmt_inch(0.125, 4, true), "1/8");
        assert_eq!(fmt_inch(2.0, 4, true), "2");
        assert_eq!(fmt_inch(-0.25, 4, true), "-1/4");
        assert_eq!(fmt_inch(0.1181, 4, true), "1/8"); // 3 mm: the nearest 1/64 of an inch
        assert_eq!(fmt_inch(2.25, 4, true), "2 1/4");
        assert_eq!(fmt_inch(2.2, 4, true), "2 13/64");
        assert_eq!(fmt_inch(0.03, 4, true), "0.03"); // too small to be rounded to a 64th
        assert_eq!(fmt_inch(0.1181, 4, false), "0.1181");
        assert_eq!(fmt_inch_edit(0.1181, 4, true), "0.1181"); // clicking a field must not round its value
        assert_eq!(fmt_inch(0.1875, 4, false), "0.1875");
        for t in ["3/16", "1 3/16", "-1/4"] { assert_eq!(fmt_inch(parse_number(t).unwrap(), 4, true), t); }
    }

    #[test]
    fn typed_units_are_converted() {
        assert_eq!(parse_length("3 mm", 25.4).map(|x| (x * 1000.0).round()), Some(118.0));
        assert_eq!(parse_length("1/8\"", 1.0), Some(3.175));
        assert_eq!(parse_length("2.5cm", 1.0), Some(25.0));
        assert_eq!(parse_length("0.5 in", 25.4), Some(0.5));
        assert_eq!(parse_length("1 3/16 in", 25.4), Some(1.1875));
        assert_eq!(parse_length("0.1 m", 1.0), Some(100.0));
        assert_eq!(parse_length("12", 25.4), Some(12.0));
        assert_eq!(parse_length("mm", 1.0), None);
    }

    #[test]
    fn lengths_follow_the_display_units() {
        assert_eq!(with_units(false, |ui| (len_unit(ui), fmt_len(ui, 12.5))), ("mm", "12.5 mm".to_string()));
        assert_eq!(with_units(true, |ui| (len_unit(ui), fmt_len(ui, 25.4))), ("in", "1 in".to_string()));
        assert_eq!(with_units(true, |ui| fmt_len(ui, 3.0)), "0.118 in");
    }

    #[test]
    fn inch_rulers_step_in_round_inches() {
        let k = 25.4;
        // 1 mm = 4 px: labels need 70 px, so 0.5 in (50.8 mm = 203 px) is not needed, 0.2 in (20 px/mm... 5.08 mm = 20 px) is too close; 0.5 in is the first that fits
        let (s, _) = crate::canvas::rulers_tick_step(4.0, k);
        assert!((s * k * 4.0) >= 70.0 && s <= 1.0, "{s}");
        assert_eq!(crate::canvas::rulers_tick_step(4.0, 1.0).0, 20.0);
    }
}

#[cfg(test)]
mod tab_tests {
    use super::*;
    use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect};

    struct Rig { ctx: egui::Context, vals: [f64; 3], ids: Vec<egui::Id>, rects: Vec<Rect>, time: f64 }

    impl Rig {
        fn new() -> Self { Rig { ctx: egui::Context::default(), vals: [1.0, 2.0, 3.0], ids: vec![], rects: vec![], time: 0.0 } }
        fn frame(&mut self, events: Vec<Event>, modifiers: Modifiers) {
            self.time += 0.1;
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))), time: Some(self.time), events, modifiers, ..Default::default() };
            let (vals, ids, rects) = (&mut self.vals, &mut self.ids, &mut self.rects);
            let _ = self.ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ids.clear(); rects.clear();
                    for v in vals.iter_mut() { let r = num(ui, [80.0, 24.0], v, 0.0, 100.0, 1.0, 1, ""); ids.push(r.id); rects.push(r.rect); }
                });
            });
        }
        fn settle(&mut self, m: Modifiers) { for _ in 0..3 { self.frame(vec![], m); } }
        fn editing(&self, i: usize) -> bool { self.ctx.data(|d| d.get_temp::<String>(self.ids[i].with("edit"))).is_some() }
        fn click(&mut self, i: usize) {
            let p = self.rects[i].center();
            self.frame(vec![Event::PointerMoved(p)], Modifiers::NONE);
            self.frame(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], Modifiers::NONE);
            self.frame(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
            self.settle(Modifiers::NONE);
        }
        fn tab(&mut self, shift: bool) {
            let m = if shift { Modifiers::SHIFT } else { Modifiers::NONE };
            self.frame(vec![Event::Key { key: Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: m }], m);
            self.frame(vec![Event::Key { key: Key::Tab, physical_key: None, pressed: false, repeat: false, modifiers: m }], m);
            self.settle(m);
        }
    }

    #[test]
    fn tab_takes_the_value_and_opens_the_next_field_and_shift_tab_the_one_before() {
        let mut r = Rig::new();
        r.settle(Modifiers::NONE);
        assert!(!r.editing(0) && !r.editing(1));
        r.click(0);
        assert!(r.editing(0), "a click opens the field");
        r.frame(vec![Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND }], Modifiers::COMMAND);
        r.frame(vec![Event::Text("42".into())], Modifiers::NONE);
        r.tab(false);
        assert_eq!(r.vals[0], 42.0, "Tab takes what was typed");
        assert!(!r.editing(0) && r.editing(1), "and opens the next field: {} {} {}", r.editing(0), r.editing(1), r.editing(2));
        r.tab(false);
        assert!(r.editing(2) && !r.editing(1));
        r.tab(true);
        assert!(r.editing(1) && !r.editing(2), "Shift+Tab goes back");
        assert_eq!(r.vals[1], 2.0, "a field left untouched keeps its value");
    }
}
