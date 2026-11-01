//! The setup assistant: units, machine and starter tools in a few steps. It opens by itself the first time the app runs
//! (no settings file yet) and from Help → Setup assistant. The first time it replaces the default machine and the tools of the
//! current library; later it only adds a new machine and a new library, so nothing the user made is lost.
use crate::app::App;
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
    pub machine: MachineSetup,
    pub tools: ToolChoice,
}

impl Default for Wizard {
    fn default() -> Self {
        Wizard { open: false, step: 0, first: false, inches: false, preset: 0, machine: MachineSetup::from_preset(&PRESETS[0]), tools: ToolChoice::default() }
    }
}

impl Wizard {
    pub fn start(&mut self, first: bool) {
        *self = Wizard { open: true, first, ..Default::default() };
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
        self.tool_len_in = w.inches;
        self.mach_len_in = w.inches;
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
const LABEL_W: f32 = 110.0;
const VALUE_W: f32 = 110.0;

/// A grid row's label: weak, in its own column.
fn key(ui: &mut egui::Ui, t: &str) { ui.label(egui::RichText::new(t).weak()); }

/// A section title with a hairline under it.
fn section(ui: &mut egui::Ui, t: &str) {
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

/// Value cell of a limit row: a switch and the number when on.
fn limit(ui: &mut egui::Ui, v: &mut Option<f64>, dflt: f64, suffix: &str, tip: &str) {
    let mut on = v.is_some();
    if crate::widgets::check(ui, &mut on, "").on_hover_text(tip).changed() { *v = on.then_some(dflt); }
    if let Some(x) = v { crate::widgets::num(ui, [VALUE_W, 22.0], x, 1.0, 100000.0, 10.0, 0, suffix); } else { ui.weak("not limited"); }
}

/// A large choice button with a title and a second line.
fn card(ui: &mut egui::Ui, w: f32, on: bool, title: &str, sub: &str) -> bool {
    let mut t = egui::text::LayoutJob::default();
    let f = egui::TextStyle::Button.resolve(ui.style());
    t.append(title, 0.0, egui::TextFormat { font_id: f.clone(), color: if on { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() }, ..Default::default() });
    t.append(&format!("\n{sub}"), 0.0, egui::TextFormat { font_id: egui::FontId::proportional(f.size - 2.0), color: if on { ui.visuals().selection.stroke.color } else { ui.visuals().weak_text_color() }, ..Default::default() });
    ui.add_sized([w, 48.0], egui::SelectableLabel::new(on, t)).clicked()
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
    section(ui, "Name and size class");
    egui::Grid::new("setup_name").num_columns(2).spacing([14.0, 8.0]).min_col_width(LABEL_W).show(ui, |ui| {
        key(ui, "Name");
        ui.add(egui::TextEdit::singleline(&mut w.machine.name).desired_width(280.0));
        ui.end_row();
    });
    ui.add_space(6.0);
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
    section(ui, "Travel and speed");
    egui::Grid::new("setup_travel").num_columns(4).spacing([14.0, 8.0]).min_col_width(LABEL_W).show(ui, |ui| {
        for (label, v, lo, hi, suffix) in [("Travel X", &mut w.machine.x, 10.0, 100000.0, " mm"), ("Travel Y", &mut w.machine.y, 10.0, 100000.0, " mm"), ("Travel Z", &mut w.machine.z, 5.0, 10000.0, " mm")] {
            key(ui, label); ui.label(""); crate::widgets::num(ui, [VALUE_W, 22.0], v, lo, hi, 1.0, 1, suffix); ui.end_row();
        }
        key(ui, "Rapid speed"); ui.label(""); crate::widgets::num(ui, [VALUE_W, 22.0], &mut w.machine.rapid, 100.0, 100000.0, 10.0, 0, " mm/min"); ui.end_row();
    });
    section(ui, "Limits (optional)");
    egui::Grid::new("setup_limits").num_columns(4).spacing([14.0, 8.0]).min_col_width(LABEL_W).show(ui, |ui| {
        key(ui, "Max feed"); limit(ui, &mut w.machine.feed_max, 1000.0, " mm/min", "Cut feeds are held at or under this"); ui.end_row();
        key(ui, "Max plunge"); limit(ui, &mut w.machine.plunge_max, 300.0, " mm/min", "Plunge feeds are held at or under this"); ui.end_row();
        key(ui, "Max spindle"); limit(ui, &mut w.machine.rpm_max, 10000.0, " rpm", "Spindle speeds are held at or under this"); ui.end_row();
    });
    section(ui, "Controller");
    egui::Grid::new("setup_ctl").num_columns(2).spacing([14.0, 8.0]).min_col_width(LABEL_W).show(ui, |ui| {
        key(ui, "G-code for");
        egui::ComboBox::from_id_source("setup_post").width(280.0).selected_text(putiacnc_core::model::dialect_name(&w.machine.post)).show_ui(ui, |ui| {
            for (id, name) in DIALECTS { ui.selectable_value(&mut w.machine.post, id.to_string(), name); }
        });
        ui.end_row();
        key(ui, "Rigidity");
        egui::ComboBox::from_id_source("setup_rigid").width(280.0).selected_text(putiacnc_core::feeds::RIGIDITY.iter().find(|r| r.0 == w.machine.rigid).map_or("", |r| r.1)).show_ui(ui, |ui| {
            for (id, name) in putiacnc_core::feeds::RIGIDITY { ui.selectable_value(&mut w.machine.rigid, id.to_string(), name); }
        });
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
    egui::Grid::new("setup_tools").num_columns(2).spacing([14.0, 8.0]).min_col_width(230.0).show(ui, |ui| {
        let t = &mut w.tools;
        crate::widgets::check(ui, &mut t.vbits, "V-bits and engraving bits"); key(ui, "V-carving, lettering"); ui.end_row();
        crate::widgets::check(ui, &mut t.ball, "Ball nose bits"); key(ui, "3D relief carving"); ui.end_row();
        crate::widgets::check(ui, &mut t.surfacing, "Surfacing bit"); key(ui, "Flattening the spoilboard"); ui.end_row();
        crate::widgets::check(ui, &mut t.pens, "Pens and markers"); key(ui, "Plotting"); ui.end_row();
    });
    let n = setup::starter_tools(ToolChoice { inch_sizes: w.inches, ..w.tools }).len();
    ui.add_space(10.0);
    ui.weak(format!("{n} tools will be added."));
}

fn finish_step(ui: &mut egui::Ui, w: &Wizard) {
    ui.label(egui::RichText::new("Ready").size(20.0).strong());
    section(ui, "Summary");
    let m = &w.machine;
    egui::Grid::new("setup_sum").num_columns(2).spacing([14.0, 8.0]).min_col_width(LABEL_W).striped(true).show(ui, |ui| {
        key(ui, "Machine"); ui.label(m.name.trim()); ui.end_row();
        key(ui, "Work area"); ui.label(format!("{:.0} \u{d7} {:.0} \u{d7} {:.0} mm", m.x, m.y, m.z)); ui.end_row();
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
        ui.set_width(540.0);
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
