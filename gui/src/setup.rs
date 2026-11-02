//! The setup assistant: units, machine and starter tools in a few steps. It opens by itself the first time the app runs
//! (no settings file yet) and from Help → Setup assistant. The first time it replaces the default machine and the tools of the
//! current library; later it only adds a new machine and a new library, so nothing the user made is lost.
use crate::app::App;
use crate::widgets::Tip;
use eframe::egui::{self, Color32};
use putiacnc_core::model::DIALECTS;
use putiacnc_core::setup::{self, MachineSetup, ToolChoice, PRESETS};

const STEPS: usize = 5;

pub struct Wizard {
    pub open: bool,
    pub step: usize,
    /// First run: replace the defaults instead of adding to them.
    pub first: bool,
    pub inches: bool,
    pub preset: usize,
    /// The maker picked in the list (empty = a machine that is not listed).
    pub maker: String,
    /// The catalogue entry picked (index into `setup::catalog()`).
    pub pick: Option<usize>,
    pub machine: MachineSetup,
    pub tools: ToolChoice,
    /// Words typed in the machine search box.
    pub query: String,
}

impl Default for Wizard {
    fn default() -> Self {
        Wizard { open: false, step: 0, first: false, inches: false, preset: 0, maker: String::new(), pick: None, machine: MachineSetup::from_preset(&PRESETS[0]), tools: ToolChoice::default(), query: String::new() }
    }
}

impl Wizard {
    pub fn start(&mut self, first: bool) {
        *self = Wizard { open: true, first, ..Default::default() };
    }
    /// Fills the machine from a catalogue entry.
    pub fn pick_machine(&mut self, i: usize) {
        let Some(c) = setup::catalog().get(i) else { return };
        self.pick = Some(i);
        self.maker = c.maker.clone();
        self.machine = c.setup();
        self.machine.inches = self.inches;
    }
    fn pick_other(&mut self) {
        self.pick = None;
        self.maker.clear();
        let i = self.preset;
        self.pick_preset(i);
        self.machine.notes.clear();
    }
    fn pick_preset(&mut self, i: usize) {
        self.preset = i;
        let name = self.machine.name.clone();
        let post = self.machine.post.clone();
        self.machine = MachineSetup::from_preset(&PRESETS[i]);
        self.machine.name = name;
        self.machine.post = post;
        self.machine.inches = self.inches;
    }
}

impl App {
    /// Puts the choices to work.
    pub fn finish_setup(&mut self) {
        let w = std::mem::take(&mut self.setup);
        self.apply_setup(w);
        crate::prefs::modify(|s| s.setup_done = Some(true));
    }

    fn apply_setup(&mut self, w: Wizard) {
        let was_modified = self.modified;
        let mut ms = w.machine.clone();
        ms.inches = w.inches;
        let m = ms.into_machine();
        let id = m.id.clone();
        let tools = setup::starter_tools(ToolChoice { inch_sizes: w.inches, ..w.tools });
        if w.first {
            self.machines.list = vec![m];
            self.tool_libs.current_mut().tools = tools;
        } else {
            self.machines.push_unique(m);
            let name = self.tool_libs.unique_name("My tools", None);
            self.tool_libs.new_library();
            self.tool_libs.current_mut().tools = tools;
            self.tool_libs.rename_current(&name);
        }
        self.machines.def = id.clone();
        self.disp_in = w.inches;
        self.imperial = w.inches;
        self.set_machine(&id);
        self.apply_lib_tools();
        self.libs_dirty = true;
        self.machines_dirty = true;
        self.gen_pending = true;
        if w.first { self.modified = was_modified; }
    }

    /// Closing the assistant without finishing keeps everything as it was; the first run does not ask again.
    pub fn skip_setup(&mut self) {
        self.setup.open = false;
        crate::prefs::modify(|s| s.setup_done = Some(true));
    }
}

const NAMES: [&str; STEPS] = ["Units", "Machine", "Details", "Tools", "Finish"];
// the same field sizes and spacing as the machine library
const LABEL_W: f32 = 130.0;
const UNIT_W: f32 = 60.0;
const SP: f32 = 8.0;
const GAP: f32 = 20.0;

/// A grid row's label: right-aligned in a fixed column, like the machine library.
fn key(ui: &mut egui::Ui, t: &str) { crate::toolmgr::rlabel(ui, LABEL_W, t); }

/// A section title with a hairline under it.
pub(crate) fn section(ui: &mut egui::Ui, t: &str) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new(t).strong());
    ui.add_space(2.0);
    ui.separator();
    ui.add_space(4.0);
}

/// A number field followed by its unit; it shows a length or a speed in the chosen unit (`k` mm or mm/min per unit) while the value is kept in mm.
fn field(ui: &mut egui::Ui, v: &mut f64, k: f64, lo: f64, hi: f64, step: f64, unit: &str, tip: &str) {
    let mut x = *v / k;
    if num_unit_w(ui, &mut x, lo / k, hi / k, step / k, unit).tip(tip).changed() { *v = (x * k).clamp(lo, hi); }
}

/// A field and its unit in a cell of a fixed width, so the columns of the separate grids of a step line up whatever their units are.
fn num_unit_w(ui: &mut egui::Ui, x: &mut f64, lo: f64, hi: f64, step: f64, unit: &str) -> egui::Response {
    let r = crate::widgets::num(ui, [crate::widgets::FIELD_W, 22.0], x, lo, hi, step, 3, "");
    // wide enough for "mm/min" and "in/min"
    ui.allocate_ui_with_layout(egui::vec2(UNIT_W, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(UNIT_W); ui.weak(unit); });
    r
}

/// The same for a limit, where 0 means no limit.
fn limit(ui: &mut egui::Ui, v: &mut Option<f64>, k: f64, hi: f64, step: f64, unit: &str) {
    let mut x = v.unwrap_or(0.0) / k;
    if num_unit_w(ui, &mut x, 0.0, hi / k, step / k, unit).tip("0 = no limit").changed() { *v = (x > 0.0).then_some(x * k); }
}

fn spacer(ui: &mut egui::Ui) { ui.allocate_exact_size(egui::vec2(GAP, 1.0), egui::Sense::hover()); }

/// A large choice button with a title and a second line.
fn card(ui: &mut egui::Ui, w: f32, on: bool, title: &str, sub: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 48.0), egui::Sense::click());
    let v = ui.visuals();
    let (fill, stroke) = if on { (v.selection.bg_fill, v.selection.bg_fill) } else if resp.hovered() { (v.widgets.hovered.weak_bg_fill, v.widgets.hovered.bg_stroke.color) } else { (egui::Color32::TRANSPARENT, egui::Color32::TRANSPARENT) };
    ui.painter().rect(rect, 4.0, fill, egui::Stroke::new(1.0_f32, stroke));
    let (c1, c2) = if on { (v.selection.stroke.color, v.selection.stroke.color) } else { (v.text_color(), v.weak_text_color()) };
    let f = egui::TextStyle::Button.resolve(ui.style());
    // both lines centred on the card, whatever their lengths
    let (y1, y2) = if sub.is_empty() { (rect.center().y, rect.center().y) } else { (rect.center().y - 9.0, rect.center().y + 9.0) };
    ui.painter().text(egui::pos2(rect.center().x, y1), egui::Align2::CENTER_CENTER, title, f.clone(), c1);
    ui.painter().text(egui::pos2(rect.center().x, y2), egui::Align2::CENTER_CENTER, sub, egui::FontId::proportional(f.size - 2.0), c2);
    resp.clicked()
}

// ------------------------------------------------------------------------------------------------ look

const BRAND: Color32 = Color32::from_rgb(0x25, 0x63, 0xd6);
/// Width of the blue panel at the left of the window.
const BRAND_W: f32 = 250.0;
/// Width of the whole window.
const WIN_W: f32 = 900.0;

fn polyline(p: &egui::Painter, pts: Vec<egui::Pos2>, st: egui::Stroke) { p.add(egui::Shape::line(pts, st)); }

fn accent(ui: &egui::Ui) -> Color32 { ui.visuals().selection.bg_fill }

/// `s` cut with an ellipsis so that it is at most `w` wide in `font`.
fn clip(p: &egui::Painter, s: &str, font: &egui::FontId, w: f32) -> String {
    let wide = |t: &str| p.layout_no_wrap(t.to_string(), font.clone(), Color32::WHITE).size().x;
    if wide(s) <= w { return s.to_string(); }
    let mut c: Vec<char> = s.chars().collect();
    while !c.is_empty() { c.pop(); let t = format!("{}\u{2026}", c.iter().collect::<String>().trim_end()); if wide(&t) <= w { return t; } }
    String::new()
}

/// A large choice tile with a border, a tint and a check mark when chosen; `draw` paints what is in it (inside the rectangle given).
fn tile(ui: &mut egui::Ui, size: egui::Vec2, on: bool, draw: impl FnOnce(&egui::Painter, egui::Rect, Color32, Color32)) -> bool { tile_at(ui, size, on, draw).0 }

fn tile_at(ui: &mut egui::Ui, size: egui::Vec2, on: bool, draw: impl FnOnce(&egui::Painter, egui::Rect, Color32, Color32)) -> (bool, egui::Rect) {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let (acc, v) = (accent(ui), ui.visuals());
    let fill = if on { acc.gamma_multiply(0.16) } else if resp.hovered() { v.widgets.hovered.weak_bg_fill } else { v.widgets.noninteractive.weak_bg_fill };
    let stroke = if on { egui::Stroke::new(2.5_f32, acc) } else if resp.hovered() { egui::Stroke::new(1.5_f32, acc.gamma_multiply(0.7)) } else { v.widgets.noninteractive.bg_stroke };
    let (ink, weak) = (v.text_color(), v.weak_text_color());
    let p = ui.painter().with_clip_rect(rect.expand(1.0).intersect(ui.clip_rect()));
    p.rect(rect, 10.0, fill, stroke);
    draw(&p, rect.shrink2(egui::vec2(14.0, 12.0)), ink, weak);
    if on {
        let c = egui::pos2(rect.max.x - 18.0, rect.min.y + 18.0);
        p.circle_filled(c, 10.0, acc);
        polyline(&p, vec![c + egui::vec2(-4.5, 0.5), c + egui::vec2(-1.5, 3.5), c + egui::vec2(4.5, -3.5)], egui::Stroke::new(2.0_f32, Color32::WHITE));
    }
    (resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked(), rect)
}

/// The blue panel at the left: what the program is, and where the person is in the setup.
fn brand(ui: &mut egui::Ui, h: f32, step: usize, first: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(BRAND_W, h), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 12.0, BRAND);
    let white = Color32::WHITE;
    let logo = egui::Rect::from_min_size(rect.min + egui::vec2(28.0, 32.0), egui::vec2(54.0, 54.0));
    p.rect_filled(logo, 14.0, white);
    p.text(logo.center(), egui::Align2::CENTER_CENTER, "P", egui::FontId::proportional(32.0), BRAND);
    p.text(rect.min + egui::vec2(28.0, 104.0), egui::Align2::LEFT_TOP, "PutiaCNC", egui::FontId::proportional(26.0), white);
    p.text(rect.min + egui::vec2(28.0, 142.0), egui::Align2::LEFT_TOP, "From drawing to G-code,\nchecked before it cuts.", egui::FontId::proportional(14.0), white.gamma_multiply(0.85));
    for (i, t) in ["Draw or import shapes", "See the toolpath in 3D", "Catch mistakes early"].iter().enumerate() {
        let y = rect.min.y + 232.0 + i as f32 * 34.0;
        let c = egui::pos2(rect.min.x + 38.0, y + 9.0);
        p.circle_stroke(c, 8.0, egui::Stroke::new(1.6_f32, white.gamma_multiply(0.9)));
        polyline(&p, vec![c + egui::vec2(-3.5, 0.0), c + egui::vec2(-1.0, 2.5), c + egui::vec2(3.5, -2.5)], egui::Stroke::new(1.6_f32, white.gamma_multiply(0.9)));
        p.text(egui::pos2(rect.min.x + 56.0, y), egui::Align2::LEFT_TOP, *t, egui::FontId::proportional(14.0), white);
    }
    let _ = first;
    p.text(egui::pos2(rect.min.x + 28.0, rect.max.y - 30.0), egui::Align2::LEFT_TOP, format!("Step {} of {}", step + 1, STEPS), egui::FontId::proportional(13.0), white.gamma_multiply(0.75));
}

/// Dots for the steps, then the name of this one.
fn dots(ui: &mut egui::Ui, step: usize) {
    ui.horizontal(|ui| {
        let acc = accent(ui);
        let off = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(STEPS as f32 * 20.0, 18.0), egui::Sense::hover());
        for i in 0..STEPS {
            let c = egui::pos2(rect.min.x + 6.0 + i as f32 * 20.0, rect.center().y);
            ui.painter().circle_filled(c, if i == step { 5.5 } else { 4.5 }, if i <= step { acc } else { off });
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new(NAMES[step].to_uppercase()).small().weak());
    });
}

fn title(ui: &mut egui::Ui, t: &str) { ui.label(egui::RichText::new(t).size(26.0).strong()); }

fn sub(ui: &mut egui::Ui, t: &str) { ui.add(egui::Label::new(egui::RichText::new(t).weak()).wrap(true)); }

/// A ruler for the unit tiles: ticks every unit, longer every fifth (mm) or at the halves (inches).
fn ruler(p: &egui::Painter, r: egui::Rect, inches: bool) {
    let bar = egui::Rect::from_min_size(r.min, egui::vec2(r.width(), 30.0));
    p.rect(bar, 3.0, Color32::from_rgb(0xf6, 0xe7, 0xbd), egui::Stroke::new(1.0_f32, Color32::from_rgb(0xbe, 0xa0, 0x5a)));
    let n = if inches { 8 } else { 20 };
    let ink = Color32::from_rgb(0x6f, 0x5a, 0x28);
    for k in 1..n {
        let x = bar.min.x + bar.width() * k as f32 / n as f32;
        let long = if inches { k % 4 == 0 } else { k % 5 == 0 };
        let mid = inches && k % 2 == 0;
        p.line_segment([egui::pos2(x, bar.min.y), egui::pos2(x, bar.min.y + if long { 17.0 } else if mid { 13.0 } else { 8.0 })], egui::Stroke::new(1.3_f32, ink));
    }
}

fn units_step(ui: &mut egui::Ui, w: &mut Wizard) {
    title(ui, if w.first { "Welcome" } else { "Set up another machine" });
    ui.add_space(4.0);
    sub(ui, if w.first {
        "A minute of setup matches the work area, the feeds and the tool list to your machine. Everything can be changed later in Libraries \u{2192} Machines and Tool library."
    } else {
        "This adds a new machine and a new tool library. Your existing machines, libraries and projects are not touched."
    });
    ui.add_space(18.0);
    ui.label(egui::RichText::new("How do you measure?").strong().size(16.0));
    ui.add_space(8.0);
    let tw = (ui.available_width() - 14.0) / 2.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        for inch in [false, true] {
            let (name, hint) = if inch { ("Inches", "fractions like 3/16 are fine") } else { ("Millimetres", "sizes, tools and G-code in mm") };
            if tile(ui, egui::vec2(tw, 150.0), w.inches == inch, |p, r, ink, weak| {
                ruler(p, egui::Rect::from_min_size(r.min + egui::vec2(8.0, 6.0), egui::vec2(r.width() - 16.0, 30.0)), inch);
                p.text(egui::pos2(r.center().x, r.min.y + 60.0), egui::Align2::CENTER_TOP, name, egui::FontId::proportional(19.0), ink);
                p.text(egui::pos2(r.center().x, r.min.y + 90.0), egui::Align2::CENTER_TOP, hint, egui::FontId::proportional(13.0), weak);
            }) { w.inches = inch; w.machine.inches = inch; }
        }
    });
    ui.add_space(14.0);
    ui.weak("You can change this any time in View \u{2192} Units.");
}

/// What the machine picker lists: the catalogue entries whose maker and model hold every word typed.
fn matches(q: &str) -> Vec<usize> {
    let words: Vec<String> = q.split_whitespace().map(|x| x.to_lowercase()).collect();
    setup::catalog().iter().enumerate().filter(|(_, c)| { let h = format!("{} {}", c.maker, c.model).to_lowercase(); words.iter().all(|x| h.contains(x)) }).map(|(i, _)| i).collect()
}

fn size_text(x: f64, y: f64, inches: bool) -> String {
    if inches { format!("{:.1} \u{d7} {:.1} in", x / 25.4, y / 25.4) } else { format!("{:.0} \u{d7} {:.0} mm", x, y) }
}

fn machine_step(ui: &mut egui::Ui, w: &mut Wizard) {
    title(ui, "Which machine do you have?");
    ui.add_space(4.0);
    sub(ui, "Pick yours and the sizes, speeds and controller fill in. Not listed? Choose \u{201c}Other\u{201d}.");
    ui.add_space(10.0);
    ui.add(egui::TextEdit::singleline(&mut w.query).hint_text("Search: maker or model").desired_width(ui.available_width()).margin(egui::vec2(10.0, 6.0)));
    ui.add_space(8.0);
    let list = matches(&w.query);
    let cols = 3;
    let tw = (ui.available_width() - 8.0 - (cols as f32 - 1.0) * 10.0) / cols as f32;
    let other = w.pick.is_none();
    let mut pick: Option<Option<usize>> = None;
    {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
        let mut cells: Vec<Option<usize>> = vec![None];
        cells.extend(list.iter().map(|&i| Some(i)));
        for row in cells.chunks(cols) {
            ui.horizontal(|ui| {
                for cell in row {
                    let on = match cell { None => other, Some(i) => w.pick == Some(*i) };
                    let (t1, t2, t3) = match cell {
                        None => ("Other / not listed".to_string(), String::new(), "I will type the sizes".to_string()),
                        Some(i) => { let c = &setup::catalog()[*i]; (c.model.clone(), c.maker.clone(), size_text(c.x, c.y, w.inches)) }
                    };
                    let (clicked, trect) = tile_at(ui, egui::vec2(tw, 72.0), on, |p, r, ink, weak| {
                        let (f1, f2) = (egui::FontId::proportional(14.5), egui::FontId::proportional(12.5));
                        let room = r.width() - 20.0;
                        p.text(r.min, egui::Align2::LEFT_TOP, clip(p, &t1, &f1, room), f1.clone(), ink);
                        if !t2.is_empty() { p.text(r.min + egui::vec2(0.0, 22.0), egui::Align2::LEFT_TOP, clip(p, &t2, &f2, room), f2.clone(), weak); }
                        p.text(r.min + egui::vec2(0.0, if t2.is_empty() { 22.0 } else { 40.0 }), egui::Align2::LEFT_TOP, clip(p, &t3, &f2, room), f2.clone(), weak);
                    });
                    if clicked { pick = Some(*cell); }
                    // a machine chosen before (coming back to this step) is brought into view once
                    if on {
                        let key = egui::Id::new(("setup_seen", cell.unwrap_or(usize::MAX)));
                        if !ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false) { ui.data_mut(|d| d.insert_temp(key, true)); ui.scroll_to_rect(trect, None); }
                    }
                }
            });
        }
        if list.is_empty() { ui.add_space(6.0); ui.weak("Nothing matches. Choose \u{201c}Other\u{201d} to type the sizes."); }
    }
    match pick { Some(Some(i)) => w.pick_machine(i), Some(None) if w.pick.is_some() || !w.machine.notes.is_empty() => w.pick_other(),
        _ => {} }
}

/// The second part of the machine: its name, the figures, and every value to adjust.
fn details_step(ui: &mut egui::Ui, w: &mut Wizard) {
    title(ui, "Name it and check the sizes");
    ui.add_space(4.0);
    sub(ui, if w.pick.is_some() { "These are the maker's figures. Change any that differ on your machine." } else { "Pick the closest size class, then type the real values." });
    ui.add_space(12.0);
    spec_card(ui, w);
    adjust_form(ui, w);
}

/// The chosen machine: its work area drawn to scale, a name to change, and the figures that matter.
fn spec_card(ui: &mut egui::Ui, w: &mut Wizard) {
    let acc = accent(ui);
    egui::Frame::none().fill(ui.visuals().widgets.noninteractive.weak_bg_fill).stroke(ui.visuals().widgets.noninteractive.bg_stroke).rounding(10.0).inner_margin(12.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            // the work area, to scale in a 110 x 76 box
            let (box_rect, _) = ui.allocate_exact_size(egui::vec2(124.0, 84.0), egui::Sense::hover());
            let (mx, my) = (w.machine.x.max(1.0), w.machine.y.max(1.0));
            let k = ((box_rect.width() - 8.0) / mx as f32).min((box_rect.height() - 8.0) / my as f32);
            let r = egui::Rect::from_center_size(box_rect.center(), egui::vec2(mx as f32 * k, my as f32 * k));
            ui.painter().rect(r, 3.0, Color32::from_rgb(0xf6, 0xe7, 0xbd), egui::Stroke::new(1.5_f32, Color32::from_rgb(0xbe, 0xa0, 0x5a)));
            ui.painter().circle_filled(r.left_bottom(), 3.5, acc);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| { ui.label("Name"); ui.add(egui::TextEdit::singleline(&mut w.machine.name).desired_width(ui.available_width().min(300.0))); });
                let m = &w.machine;
                let area = if w.inches { format!("{:.1} \u{d7} {:.1} \u{d7} {:.1} in", m.x / 25.4, m.y / 25.4, m.z / 25.4) } else { format!("{:.0} \u{d7} {:.0} \u{d7} {:.0} mm", m.x, m.y, m.z) };
                let spindle = m.rpm_max.map_or("not set".to_string(), |r| format!("up to {:.0} rpm", r));
                ui.add_space(2.0);
                egui::Grid::new("setup_facts").num_columns(2).spacing([14.0, 3.0]).show(ui, |ui| {
                    ui.weak("Work area"); ui.label(area); ui.end_row();
                    ui.weak("Spindle"); ui.label(spindle); ui.end_row();
                    ui.weak("G-code for"); ui.label(putiacnc_core::model::dialect_name(&m.post)); ui.end_row();
                });
            });
        });
        if let Some(c) = w.pick.and_then(|i| setup::catalog().get(i)) {
            ui.add_space(6.0);
            ui.add(egui::Label::new(egui::RichText::new("Figures published by the maker; check them against your machine's manual.").weak()).wrap(true));
            ui.hyperlink_to("Maker's product page", &c.source);
            if !c.note.is_empty() { ui.add(egui::Label::new(egui::RichText::new(&c.note).weak().small()).wrap(true)); }
        }
    });
}

/// Every value of the machine (for one that is not listed, or whose figures differ).
fn adjust_form(ui: &mut egui::Ui, w: &mut Wizard) {
    if w.pick.is_none() {
        ui.add_space(4.0);
        ui.label("Closest size class:");
        let cw = (ui.available_width() - 8.0) / 2.0;
        let mut pick = None;
        for row in 0..2 {
            ui.horizontal(|ui| {
                for col in 0..2 {
                    let i = row * 2 + col;
                    let (t, s) = PRESETS[i].label.split_once(" (").map_or((PRESETS[i].label, ""), |(a, b)| (a, b.trim_end_matches(')')));
                    if card(ui, cw, w.preset == i, t, s) { pick = Some(i); }
                }
            });
        }
        if let Some(i) = pick { w.pick_preset(i); }
        ui.weak("Only a starting point: set the real values below.");
    }
    let (lu, su) = if w.inches { ("in", "in/min") } else { ("mm", "mm/min") };
    let (k, sk) = if w.inches { (25.4, 25.4) } else { (1.0, 1.0) };
    section(ui, "Travel and speed");
    egui::Grid::new("setup_travel").num_columns(7).spacing([SP, 8.0]).show(ui, |ui| {
        key(ui, "Travel X"); field(ui, &mut w.machine.x, k, 10.0, 100000.0, 1.0, lu, "How far the tool can move in X");
        spacer(ui);
        key(ui, "Travel Y"); field(ui, &mut w.machine.y, k, 10.0, 100000.0, 1.0, lu, "How far the tool can move in Y");
        ui.end_row();
        key(ui, "Travel Z"); field(ui, &mut w.machine.z, k, 5.0, 10000.0, 1.0, lu, "How far the tool can move in Z");
        spacer(ui);
        key(ui, "Rapid rate"); field(ui, &mut w.machine.rapid, sk, 1.0, 1.0e6, 10.0, su, "How fast the machine travels in its rapid moves. Used for the time estimate only.");
        ui.end_row();
    });
    section(ui, "Limits (0 = no limit)");
    egui::Grid::new("setup_limits").num_columns(7).spacing([SP, 8.0]).show(ui, |ui| {
        key(ui, "Spindle max"); limit(ui, &mut w.machine.rpm_max, 1.0, 200000.0, 100.0, "rpm");
        spacer(ui);
        key(ui, "Max feed"); limit(ui, &mut w.machine.feed_max, sk, 1.0e6, 10.0, su);
        ui.end_row();
        key(ui, "Max plunge"); limit(ui, &mut w.machine.plunge_max, sk, 1.0e6, 10.0, su);
        ui.end_row();
    });
    ui.add_space(4.0);
    ui.weak("A tool that asks for more is held to the limit in the G-code, with a warning.");
    section(ui, "Controller");
    let gw = crate::widgets::combo_fit(ui, &["GRBL", "grblHAL", "FluidNC", "Mach3 / LinuxCNC"]).max(100.0);
    egui::Grid::new("setup_ctl").num_columns(2).spacing([SP, 8.0]).show(ui, |ui| {
        key(ui, "G-code output for");
        egui::ComboBox::from_id_source("setup_post").width(gw).selected_text(putiacnc_core::model::dialect_name(&w.machine.post)).show_ui(ui, |ui| {
            for (id, name) in DIALECTS { ui.selectable_value(&mut w.machine.post, id.to_string(), name); }
        });
        ui.end_row();
        key(ui, "Rigidity");
        let cur = putiacnc_core::feeds::RIGIDITY.iter().find(|r| r.0 == w.machine.rigid).map_or("", |r| r.1);
        egui::ComboBox::from_id_source("setup_rigid").selected_text(cur).show_ui(ui, |ui| {
            for (id, name) in putiacnc_core::feeds::RIGIDITY { ui.selectable_value(&mut w.machine.rigid, id.to_string(), name); }
        }).response.on_hover_text("How stiff the machine is: the feeds and speeds suggestion takes heavier chips and deeper cuts from a stiffer one. Start with light unless you know better");
        ui.end_row();
    });
}

fn tools_step(ui: &mut egui::Ui, w: &mut Wizard) {
    title(ui, "Which tools do you have?");
    ui.add_space(4.0);
    sub(ui, "Tick what you own; more can be added any time in the tool library.");
    ui.add_space(14.0);
    ui.label(egui::RichText::new(if w.inches { "Always included: straight end mills, 1/8\u{2033} and 1/4\u{2033}" } else { "Always included: straight end mills, 1 to 8 mm" }).strong());
    ui.add_space(10.0);
    let tw = (ui.available_width() - 12.0) / 2.0;
    let opts: [(&str, &str); 4] = [("V-bits and engraving bits", "V-carving, lettering"), ("Ball nose bits", "3D relief carving"), ("Surfacing bit", "Flattening the spoilboard"), ("Pens and markers", "Plotting on paper")];
    for row in 0..2 {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            for col in 0..2 {
                let i = row * 2 + col;
                let on = match i { 0 => w.tools.vbits, 1 => w.tools.ball, 2 => w.tools.surfacing, _ => w.tools.pens };
                if tile(ui, egui::vec2(tw, 74.0), on, |p, r, ink, weak| {
                    p.text(r.min + egui::vec2(0.0, 4.0), egui::Align2::LEFT_TOP, opts[i].0, egui::FontId::proportional(15.0), ink);
                    p.text(r.min + egui::vec2(0.0, 30.0), egui::Align2::LEFT_TOP, opts[i].1, egui::FontId::proportional(13.0), weak);
                }) { match i { 0 => w.tools.vbits = !on, 1 => w.tools.ball = !on, 2 => w.tools.surfacing = !on, _ => w.tools.pens = !on } }
            }
        });
        ui.add_space(2.0);
    }
    let n = setup::starter_tools(ToolChoice { inch_sizes: w.inches, ..w.tools }).len();
    ui.add_space(12.0);
    ui.weak(format!("{n} tools will be added."));
}

fn finish_step(ui: &mut egui::Ui, w: &Wizard) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
    let green = Color32::from_rgb(0x2e, 0xa0, 0x59);
    ui.painter().circle_filled(rect.center(), 30.0, green.gamma_multiply(0.18));
    ui.painter().circle_filled(rect.center(), 22.0, green);
    polyline(ui.painter(), vec![rect.center() + egui::vec2(-9.0, 1.0), rect.center() + egui::vec2(-2.5, 8.0), rect.center() + egui::vec2(10.0, -7.0)], egui::Stroke::new(3.5_f32, Color32::WHITE));
    ui.add_space(8.0);
    title(ui, "You're set up");
    ui.add_space(4.0);
    sub(ui, if w.first { "The default machine and the tools of the current library are replaced." } else { "A new machine and a new library \u{201c}My tools\u{201d} are added and selected." });
    ui.add_space(16.0);
    let m = &w.machine;
    let area = if w.inches { format!("{:.1} \u{d7} {:.1} \u{d7} {:.1} in", m.x / 25.4, m.y / 25.4, m.z / 25.4) } else { format!("{:.0} \u{d7} {:.0} \u{d7} {:.0} mm", m.x, m.y, m.z) };
    let ntools = setup::starter_tools(ToolChoice { inch_sizes: w.inches, ..w.tools }).len();
    let cards = [("Machine", m.name.trim().to_string(), area), ("Units and G-code", (if w.inches { "Inches" } else { "Millimetres" }).to_string(), putiacnc_core::model::dialect_name(&m.post).to_string()), ("Tools", format!("{ntools} tools"), "end mills and your picks".to_string())];
    let tw = (ui.available_width() - 24.0) / 3.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for (a, b, c) in &cards {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(tw, 92.0), egui::Sense::hover());
            let v = ui.visuals();
            ui.painter().rect(rect, 10.0, v.widgets.noninteractive.weak_bg_fill, v.widgets.noninteractive.bg_stroke);
            let (ink, weak) = (v.text_color(), v.weak_text_color());
            let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
            let (f1, f2, f3) = (egui::FontId::proportional(12.0), egui::FontId::proportional(15.5), egui::FontId::proportional(12.5));
            p.text(rect.min + egui::vec2(12.0, 10.0), egui::Align2::LEFT_TOP, a.to_uppercase(), f1, weak);
            p.text(rect.min + egui::vec2(12.0, 32.0), egui::Align2::LEFT_TOP, clip(&p, b, &f2, tw - 24.0), f2.clone(), ink);
            p.text(rect.min + egui::vec2(12.0, 58.0), egui::Align2::LEFT_TOP, clip(&p, c, &f3, tw - 24.0), f3.clone(), weak);
        }
    });
    ui.add_space(14.0);
    ui.weak("Open this again any time from Help \u{2192} Setup assistant.");
    ui.weak("There is more than one sample project: see Help \u{2192} Try a sample.");
}

/// Dims the program behind the window so that the window is the one thing to look at. It is an area of its own, shown before the
/// window so it lies under it; it takes no clicks.
fn dim(ctx: &egui::Context) {
    egui::Area::new(egui::Id::new("setup_dim")).order(egui::Order::Middle).fixed_pos(egui::Pos2::ZERO).interactable(false).show(ctx, |ui| {
        ui.painter().rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(110));
    });
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.setup.open { return; }
    let (mut next, mut back, mut done, mut skip, mut tour, mut sample) = (false, false, false, false, false, false);
    dim(ctx);
    let name = "Setup assistant";
    let scr = ctx.screen_rect();
    let win_w = WIN_W.min(scr.width() - 40.0).max(640.0);
    // one size for every step, so the window does not jump about
    let body_h = (scr.height() - 130.0).clamp(380.0, 580.0);
    egui::Window::new(name).frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, name, scr.center())).show(ctx, |ui| {
        ui.set_width(win_w);
        if crate::csd::dialog_header(ui, name, true) { skip = true; }
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 20.0;
            brand(ui, body_h, app.setup.step, app.setup.first);
            ui.vertical(|ui| {
                let pane_w = win_w - BRAND_W - 20.0 - 4.0;
                ui.set_width(pane_w);
                ui.set_min_height(body_h);
                dots(ui, app.setup.step);
                ui.add_space(8.0);
                egui::ScrollArea::vertical().id_source("setup_body").max_height(body_h - 80.0).auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_width(pane_w - 14.0);
                    match app.setup.step {
                        0 => units_step(ui, &mut app.setup),
                        1 => machine_step(ui, &mut app.setup),
                        2 => details_step(ui, &mut app.setup),
                        3 => tools_step(ui, &mut app.setup),
                        _ => finish_step(ui, &app.setup),
                    }
                    ui.add_space(6.0);
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if app.setup.step + 1 < STEPS && ui.button(if app.setup.first { "Skip setup" } else { "Cancel" }).clicked() { skip = true; }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let last = app.setup.step + 1 >= STEPS;
                        let label = if last { "  Start  " } else { "  Next \u{2192}  " };
                        let go = egui::Button::new(egui::RichText::new(label).color(Color32::WHITE).strong()).fill(accent(ui)).min_size(egui::vec2(96.0, 28.0));
                        if ui.add(go).clicked() { if last { done = true; } else { next = true; } }
                        if last && ui.add(egui::Button::new("Start with a quick tour").min_size(egui::vec2(72.0, 28.0))).on_hover_text("Finish the setup, then a two-minute look at where things are").clicked() { done = true; tour = true; }
                        if last && ui.add(egui::Button::new("Try a sample").min_size(egui::vec2(72.0, 28.0))).on_hover_text("Finish the setup and open a small name plate made to fit your machine. Help \u{2192} Try a sample has more samples").clicked() { done = true; sample = true; }
                        if app.setup.step > 0 && ui.add(egui::Button::new("\u{2190} Back").min_size(egui::vec2(72.0, 28.0))).clicked() { back = true; }
                    });
                });
            });
        });
        ui.add_space(4.0);
    });
    if next { app.setup.step += 1; }
    if back { app.setup.step = app.setup.step.saturating_sub(1); }
    if done {
        app.finish_setup();
        if tour { app.tour.start(); }
        if sample { app.request(crate::app::Action::Sample(putiacnc_core::sample::Kind::NamePlate)); }
    }
    else if skip || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { app.skip_setup(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wiz(first: bool, inches: bool) -> Wizard {
        let mut w = Wizard::default();
        w.start(first);
        w.inches = inches;
        w.machine.name = "Shop router".into();
        w.machine.x = 800.0;
        w.tools.ball = true;
        w
    }

    #[test]
    fn first_run_replaces_the_defaults_and_leaves_the_project_unmodified() {
        let mut a = App::new(None);
        a.modified = false;
        a.apply_setup(wiz(true, false));
        assert_eq!(a.machines.list.len(), 1);
        assert_eq!(a.machines.resolve(&a.machines.def).name, "Shop router");
        assert_eq!(a.project.data.machine.as_ref().map(|m| m.x), Some(800.0));
        assert!(!a.tool_libs.current().tools.is_empty());
        assert_eq!(a.project.data.tools.len(), a.tool_libs.current().tools.len());
        assert!(!a.modified && !a.imperial);
    }

    #[test]
    fn a_catalogue_machine_ends_up_on_the_project() {
        let mut a = App::new(None);
        let mut w = wiz(true, false);
        let i = putiacnc_core::setup::catalog().iter().position(|c| c.maker == "NymoLabs" && c.model == "NBS-6050 PRO").unwrap();
        w.pick_machine(i);
        a.apply_setup(w);
        let m = a.project.data.machine.as_ref().unwrap();
        assert_eq!(m.name, "NymoLabs NBS-6050 PRO");
        assert_eq!((m.x, m.y, m.z, m.rapid, m.rpm_max), (615.0, 504.0, 117.0, 6221.0, Some(30000.0)));
        assert!(m.notes.contains("nymolabs.com"));
    }

    #[test]
    fn going_back_to_other_restores_a_size_class() {
        let mut w = wiz(true, false);
        w.pick_machine(0);
        assert!(w.pick.is_some() && !w.maker.is_empty());
        w.pick_other();
        assert!(w.pick.is_none() && w.maker.is_empty() && w.machine.notes.is_empty());
        assert_eq!(w.machine.x, PRESETS[w.preset].x);
    }

    #[test]
    fn the_machine_search_takes_words_of_maker_and_model() {
        let all = matches("").len();
        assert_eq!(all, setup::catalog().len());
        let c = &setup::catalog()[0];
        let hits = matches(&format!("{} {}", c.maker.to_uppercase(), c.model));
        assert!(hits.contains(&0));
        assert!(hits.len() < all);
        assert!(matches("zzzz-no-such-machine").is_empty());
        // words in any order
        assert!(matches(&format!("{} {}", c.model, c.maker)).contains(&0));
    }

    #[test]
    fn running_it_again_only_adds() {
        let mut a = App::new(None);
        let (m0, l0) = (a.machines.list.len(), a.tool_libs.libs.len());
        let old_tools = a.tool_libs.current().tools.len();
        a.apply_setup(wiz(false, true));
        assert_eq!(a.machines.list.len(), m0 + 1);
        assert_eq!(a.tool_libs.libs.len(), l0 + 1);
        assert_eq!(a.tool_libs.libs[0].tools.len(), old_tools, "the old library is untouched");
        assert!(a.imperial && a.tool_libs.current().tools.iter().any(|t| t.name.contains('″')));
        assert!(a.modified);
    }
}
