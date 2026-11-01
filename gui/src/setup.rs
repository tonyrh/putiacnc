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

fn num(ui: &mut egui::Ui, label: &str, v: &mut f64, lo: f64, hi: f64, suffix: &str) {
    ui.horizontal(|ui| {
        crate::toolmgr::rlabel(ui, 140.0, label);
        crate::widgets::num(ui, [110.0, 22.0], v, lo, hi, 1.0, 1, suffix);
    });
}

fn limit(ui: &mut egui::Ui, label: &str, v: &mut Option<f64>, dflt: f64, suffix: &str, tip: &str) {
    ui.horizontal(|ui| {
        crate::toolmgr::rlabel(ui, 140.0, label);
        let mut on = v.is_some();
        if crate::widgets::check(ui, &mut on, "").on_hover_text(tip).changed() { *v = on.then_some(dflt); }
        ui.add_space(24.0);
        if let Some(x) = v { crate::widgets::num(ui, [110.0, 22.0], x, 1.0, 100000.0, 10.0, 0, suffix); } else { ui.weak("not limited"); }
    });
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.setup.open { return; }
    let (mut next, mut back, mut done, mut skip) = (false, false, false, false);
    let title = "Setup assistant";
    egui::Window::new(title).frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, title, ctx.screen_rect().center())).show(ctx, |ui| {
        ui.set_width(520.0);
        if crate::csd::dialog_header(ui, &format!("Setup assistant · step {} of {STEPS}", app.setup.step + 1), true) { skip = true; }
        crate::csd::scroll(ui, 170.0, |ui| {
            let w = &mut app.setup;
            ui.add_space(6.0);
            match w.step {
                0 => {
                    ui.label(egui::RichText::new(if w.first { "Welcome to PutiaCNC" } else { "Set up another machine" }).size(18.0).strong());
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(if w.first { "A minute of setup makes the feeds, the machine area and the tool list match your machine. Everything can be changed later in Libraries → Machines and Tool library." } else { "This adds a new machine and a new tool library. Your existing machines, libraries and projects are not touched." }).wrap(true));
                    ui.add_space(10.0);
                    ui.label("Which units do you work in?");
                    let (c, _) = crate::zoomcard::seg2(ui, ["Millimetres", "Inches"], ["Sizes, tools and G-code in mm", "Sizes, tools and G-code in inches"], w.inches as usize);
                    if let Some(i) = c { w.inches = i == 1; w.machine.inches = w.inches; }
                }
                1 => {
                    ui.label(egui::RichText::new("Your machine").size(18.0).strong());
                    ui.add_space(6.0);
                    ui.horizontal(|ui| { crate::toolmgr::rlabel(ui, 140.0, "Name"); ui.add(egui::TextEdit::singleline(&mut w.machine.name).desired_width(240.0)); });
                    ui.add_space(4.0);
                    ui.label("Closest size class (a starting point; fix the values below):");
                    let mut pick = None;
                    for (i, p) in PRESETS.iter().enumerate() { if ui.radio(w.preset == i, p.label).clicked() { pick = Some(i); } }
                    if let Some(i) = pick { w.pick_preset(i); }
                    ui.add_space(6.0);
                    num(ui, "Travel X", &mut w.machine.x, 10.0, 100000.0, " mm");
                    num(ui, "Travel Y", &mut w.machine.y, 10.0, 100000.0, " mm");
                    num(ui, "Travel Z", &mut w.machine.z, 5.0, 10000.0, " mm");
                    num(ui, "Rapid speed", &mut w.machine.rapid, 100.0, 100000.0, " mm/min");
                    limit(ui, "Max feed", &mut w.machine.feed_max, 1000.0, " mm/min", "Cut feeds are held at or under this");
                    limit(ui, "Max plunge", &mut w.machine.plunge_max, 300.0, " mm/min", "Plunge feeds are held at or under this");
                    limit(ui, "Max spindle", &mut w.machine.rpm_max, 10000.0, " rpm", "Spindle speeds are held at or under this");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        crate::toolmgr::rlabel(ui, 140.0, "Controller");
                        egui::ComboBox::from_id_source("setup_post").selected_text(putiacnc_core::model::dialect_name(&w.machine.post)).show_ui(ui, |ui| {
                            for (id, name) in DIALECTS { ui.selectable_value(&mut w.machine.post, id.to_string(), name); }
                        });
                    });
                    ui.horizontal(|ui| {
                        crate::toolmgr::rlabel(ui, 140.0, "Rigidity");
                        egui::ComboBox::from_id_source("setup_rigid").selected_text(putiacnc_core::feeds::RIGIDITY.iter().find(|r| r.0 == w.machine.rigid).map_or("", |r| r.1)).show_ui(ui, |ui| {
                            for (id, name) in putiacnc_core::feeds::RIGIDITY { ui.selectable_value(&mut w.machine.rigid, id.to_string(), name); }
                        });
                    });
                }
                2 => {
                    ui.label(egui::RichText::new("Your tools").size(18.0).strong());
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(format!("Straight end mills in {} sizes are always included. Add what you own; more can be added any time in the tool library.", if w.inches { "inch" } else { "metric" })).wrap(true));
                    ui.add_space(6.0);
                    crate::widgets::check(ui, &mut w.tools.vbits, "V-bits and engraving bits");
                    crate::widgets::check(ui, &mut w.tools.ball, "Ball nose bits (relief carving)");
                    crate::widgets::check(ui, &mut w.tools.surfacing, "Surfacing bit (flattening the spoilboard)");
                    crate::widgets::check(ui, &mut w.tools.pens, "Pens and markers (plotting)");
                    let n = setup::starter_tools(ToolChoice { inch_sizes: w.inches, ..w.tools }).len();
                    ui.add_space(6.0);
                    ui.weak(format!("{n} tools will be added."));
                }
                _ => {
                    ui.label(egui::RichText::new("Ready").size(18.0).strong());
                    ui.add_space(6.0);
                    let m = &w.machine;
                    ui.label(format!("Machine: {} ({}), {:.0} × {:.0} × {:.0} mm", m.name.trim(), putiacnc_core::model::dialect_name(&m.post), m.x, m.y, m.z));
                    ui.label(format!("Units: {}", if w.inches { "inches" } else { "millimetres" }));
                    ui.label(format!("Tools: {}", setup::starter_tools(ToolChoice { inch_sizes: w.inches, ..w.tools }).len()));
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(if w.first { "The default machine and the tools of the current library are replaced. Your own settings are kept." } else { "A new machine and a new library \"My tools\" are added and selected." }).wrap(true));
                }
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(if app.setup.first { "Skip setup" } else { "Cancel" }).clicked() { skip = true; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if app.setup.step + 1 < STEPS { if ui.button("Next").clicked() { next = true; } } else if ui.button("Finish").clicked() { done = true; }
                if app.setup.step > 0 && ui.button("Back").clicked() { back = true; }
            });
        });
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
