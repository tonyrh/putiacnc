//! The setup assistant: units, machine and starter tools in a few steps. It opens by itself the first time the app runs
//! (no settings file yet) and from Help → Setup assistant. The first time it replaces the default machine and the tools of the
//! current library; later it only adds a new machine and a new library, so nothing the user made is lost.
use crate::app::App;
use crate::widgets::Tip;
use eframe::egui;
use putiacnc_core::model::DIALECTS;
use putiacnc_core::setup::{self, MachineSetup, ToolChoice, PRESETS};

const STEPS: usize = 4;

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
}

impl Default for Wizard {
    fn default() -> Self {
        Wizard { open: false, step: 0, first: false, inches: false, preset: 0, maker: String::new(), pick: None, machine: MachineSetup::from_preset(&PRESETS[0]), tools: ToolChoice::default() }
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
            self.machines.list.push(m);
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

const NAMES: [&str; STEPS] = ["Units", "Machine", "Tools", "Finish"];
// the same field sizes and spacing as the machine library
const LABEL_W: f32 = 130.0;
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

/// The row of steps under the title: done and current ones are bold, the rest dim.
fn progress(ui: &mut egui::Ui, step: usize) {
    ui.horizontal(|ui| {
        for (i, n) in NAMES.iter().enumerate() {
            let t = format!("{}  {n}", i + 1);
            let r = egui::RichText::new(t);
            ui.label(if i == step { r.strong().color(ui.visuals().hyperlink_color) } else if i < step { r } else { r.weak() });
            if i + 1 < STEPS { ui.label(egui::RichText::new("›").weak()); }
        }
    });
    ui.add_space(4.0);
}

/// A number field followed by its unit; it shows a length or a speed in the chosen unit (`k` mm or mm/min per unit) while the value is kept in mm.
fn field(ui: &mut egui::Ui, v: &mut f64, k: f64, lo: f64, hi: f64, step: f64, unit: &str, tip: &str) {
    let mut x = *v / k;
    if crate::widgets::num_unit(ui, &mut x, lo / k, hi / k, step / k, 3, unit).tip(tip).changed() { *v = (x * k).clamp(lo, hi); }
}

/// The same for a limit, where 0 means no limit.
fn limit(ui: &mut egui::Ui, v: &mut Option<f64>, k: f64, hi: f64, step: f64, unit: &str) {
    let mut x = v.unwrap_or(0.0) / k;
    if crate::widgets::num_unit(ui, &mut x, 0.0, hi / k, step / k, 3, unit).tip("0 = no limit").changed() { *v = (x > 0.0).then_some(x * k); }
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

fn units_step(ui: &mut egui::Ui, w: &mut Wizard) {
    ui.label(egui::RichText::new(if w.first { "Welcome to PutiaCNC" } else { "Set up another machine" }).size(20.0).strong());
    ui.add_space(4.0);
    ui.add(egui::Label::new(egui::RichText::new(if w.first {
        "A minute of setup makes the machine area, the feeds and the tool list match your machine. Everything can be changed later in Libraries \u{2192} Machines and Tool library."
    } else {
        "This adds a new machine and a new tool library. Your existing machines, libraries and projects are not touched."
    }).weak()).wrap(true));
    section(ui, "Units");
    let half = (ui.available_width() - 8.0) / 2.0;
    ui.horizontal(|ui| {
        if card(ui, half, !w.inches, "Millimetres", "Sizes, tools and G-code in mm") { w.inches = false; w.machine.inches = false; }
        if card(ui, half, w.inches, "Inches", "Sizes, tools and G-code in inches") { w.inches = true; w.machine.inches = true; }
    });
}

fn machine_step(ui: &mut egui::Ui, w: &mut Wizard) {
    ui.label(egui::RichText::new("Your machine").size(20.0).strong());
    section(ui, "Machine");
    egui::Grid::new("setup_name").num_columns(2).spacing([SP, 8.0]).show(ui, |ui| {
        key(ui, "Maker");
        let mut chosen = None;
        egui::ComboBox::from_id_source("setup_maker").width(260.0).selected_text(if w.maker.is_empty() { "Other / not listed" } else { w.maker.as_str() }).show_ui(ui, |ui| {
            if ui.selectable_label(w.maker.is_empty(), "Other / not listed").clicked() { chosen = Some(None); }
            for m in setup::makers() { if ui.selectable_label(w.maker == m, m).clicked() && w.maker != m { chosen = Some(setup::catalog().iter().position(|c| c.maker == m)); } }
        });
        ui.end_row();
        match chosen { Some(Some(i)) => w.pick_machine(i), Some(None) => w.pick_other(), None => {} }
        if !w.maker.is_empty() {
            key(ui, "Model");
            let cur = w.pick.and_then(|i| setup::catalog().get(i)).map_or("", |c| c.model.as_str());
            let mut chosen = None;
            egui::ComboBox::from_id_source("setup_model").width(260.0).selected_text(cur).show_ui(ui, |ui| {
                for (i, c) in setup::catalog().iter().enumerate().filter(|(_, c)| c.maker == w.maker) { if ui.selectable_label(w.pick == Some(i), &c.model).clicked() { chosen = Some(i); } }
            });
            ui.end_row();
            if let Some(i) = chosen { w.pick_machine(i); }
        }
        key(ui, "Name");
        ui.add(egui::TextEdit::singleline(&mut w.machine.name).desired_width(260.0));
        ui.end_row();
    });
    if let Some(c) = w.pick.and_then(|i| setup::catalog().get(i)) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.weak("Figures published by the maker:");
            ui.hyperlink_to("product page", &c.source);
        });
        if !c.note.is_empty() { ui.add(egui::Label::new(egui::RichText::new(&c.note).weak().small()).wrap(true)); }
        ui.weak("Check them against your machine's manual; every value below can be changed.");
    } else {
        ui.add_space(6.0);
        ui.label("Closest size class:");
        let cw = (ui.available_width() - 8.0) / 2.0;
        let mut pick = None;
        for row in 0..2 {
            ui.horizontal(|ui| {
                for col in 0..2 {
                    let i = row * 2 + col;
                    let (title, sub) = PRESETS[i].label.split_once(" (").map_or((PRESETS[i].label, ""), |(a, b)| (a, b.trim_end_matches(')')));
                    if card(ui, cw, w.preset == i, title, sub) { pick = Some(i); }
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
    ui.label(egui::RichText::new("Your tools").size(20.0).strong());
    ui.add_space(4.0);
    ui.add(egui::Label::new(egui::RichText::new("Add what you own; more can be added any time in the tool library.").weak()).wrap(true));
    section(ui, "Always included");
    ui.label(if w.inches { "Straight end mills, 1/8\u{2033} and 1/4\u{2033}" } else { "Straight end mills, 1 to 8 mm" });
    section(ui, "Also add");
    egui::Grid::new("setup_tools").num_columns(2).spacing([SP + 6.0, 8.0]).show(ui, |ui| {
        let t = &mut w.tools;
        crate::widgets::check(ui, &mut t.vbits, "V-bits and engraving bits"); ui.weak("V-carving, lettering"); ui.end_row();
        crate::widgets::check(ui, &mut t.ball, "Ball nose bits"); ui.weak("3D relief carving"); ui.end_row();
        crate::widgets::check(ui, &mut t.surfacing, "Surfacing bit"); ui.weak("Flattening the spoilboard"); ui.end_row();
        crate::widgets::check(ui, &mut t.pens, "Pens and markers"); ui.weak("Plotting"); ui.end_row();
    });
    let n = setup::starter_tools(ToolChoice { inch_sizes: w.inches, ..w.tools }).len();
    ui.add_space(10.0);
    ui.weak(format!("{n} tools will be added."));
}

fn finish_step(ui: &mut egui::Ui, w: &Wizard) {
    ui.label(egui::RichText::new("Ready").size(20.0).strong());
    section(ui, "Summary");
    let m = &w.machine;
    egui::Grid::new("setup_sum").num_columns(2).spacing([SP, 8.0]).show(ui, |ui| {
        key(ui, "Machine"); ui.label(m.name.trim()); ui.end_row();
        key(ui, "Work area");
        ui.label(if w.inches { format!("{:.1} \u{d7} {:.1} \u{d7} {:.1} in", m.x / 25.4, m.y / 25.4, m.z / 25.4) } else { format!("{:.0} \u{d7} {:.0} \u{d7} {:.0} mm", m.x, m.y, m.z) });
        ui.end_row();
        key(ui, "G-code for"); ui.label(putiacnc_core::model::dialect_name(&m.post)); ui.end_row();
        key(ui, "Units"); ui.label(if w.inches { "Inches" } else { "Millimetres" }); ui.end_row();
        key(ui, "Tools"); ui.label(setup::starter_tools(ToolChoice { inch_sizes: w.inches, ..w.tools }).len().to_string()); ui.end_row();
    });
    ui.add_space(10.0);
    ui.add(egui::Label::new(egui::RichText::new(if w.first { "The default machine and the tools of the current library are replaced." } else { "A new machine and a new library \"My tools\" are added and selected." }).weak()).wrap(true));
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.setup.open { return; }
    let (mut next, mut back, mut done, mut skip) = (false, false, false, false);
    let title = "Setup assistant";
    egui::Window::new(title).frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, title, ctx.screen_rect().center())).show(ctx, |ui| {
        ui.set_width(600.0);
        if crate::csd::dialog_header(ui, title, true) { skip = true; }
        progress(ui, app.setup.step);
        crate::csd::scroll(ui, 190.0, |ui| {
            ui.add_space(4.0);
            match app.setup.step {
                0 => units_step(ui, &mut app.setup),
                1 => machine_step(ui, &mut app.setup),
                2 => tools_step(ui, &mut app.setup),
                _ => finish_step(ui, &app.setup),
            }
            ui.add_space(6.0);
        });
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(if app.setup.first { "Skip setup" } else { "Cancel" }).clicked() { skip = true; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if app.setup.step + 1 < STEPS { if ui.button("  Next  ").clicked() { next = true; } } else if ui.button("  Finish  ").clicked() { done = true; }
                if app.setup.step > 0 && ui.button("Back").clicked() { back = true; }
            });
        });
        ui.add_space(4.0);
    });
    if next { app.setup.step += 1; }
    if back { app.setup.step = app.setup.step.saturating_sub(1); }
    if done { app.finish_setup(); }
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
