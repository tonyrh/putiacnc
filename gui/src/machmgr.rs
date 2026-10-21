//! Machine manager: the list of machines (name, rapid rate, time correction, work area) and the
//! dialog to edit them, like the browser app's "Machines" dialog.
use crate::app::App;
use crate::icons::{icon_button, Icon};
use crate::toolmgr::{config_dir, rlabel};
use eframe::egui::{self, Align, Color32, Layout, Sense, Ui};
use putiacnc_core::machines::Machines;
use putiacnc_core::toollib::SPEED_UNITS;
use std::path::PathBuf;

pub fn machines_path() -> Option<PathBuf> { config_dir().map(|d| d.join("machines.json")) }

pub fn load_machines() -> Machines {
    machines_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| Machines::from_json(&s).ok()).unwrap_or_default()
}

impl App {
    pub fn save_machines(&mut self) {
        self.machines_dirty = false;
        let Some(p) = machines_path() else { return };
        if let Some(dir) = p.parent() { let _ = std::fs::create_dir_all(dir); }
        let mut copy = self.machines.clone();
        copy.repair();
        if let Err(e) = std::fs::write(&p, copy.to_json()) { self.mach_msg = format!("Could not save the machines: {e}"); }
    }

    /// Make the project's machine match the list: a machine the project brings along is added,
    /// otherwise the project uses the default machine.
    pub fn sync_machine(&mut self) {
        if let Some(m) = self.project.data.machine.clone() { self.machines.adopt(&m); }
        let id = self.project.data.machine.as_ref().map(|m| m.id.clone()).filter(|id| self.machines.get(id).is_some())
            .unwrap_or_else(|| self.machines.def.clone());
        self.project.data.machine = Some(self.machines.resolve(&id).clone());
        self.gen_pending = true;
        self.machines_dirty = true;
    }

    /// The user picked another machine for this project.
    pub fn set_machine(&mut self, id: &str) {
        if self.machines.get(id).is_none() { return; }
        self.machines.def = id.to_string();
        self.project.data.machine = Some(self.machines.resolve(id).clone());
        self.modified = true;
        self.gen_pending = true;
        self.machines_dirty = true;
    }

    /// Call after editing a machine in the manager.
    fn machine_edited(&mut self) {
        let cur = self.project.data.machine.as_ref().map(|m| m.id.clone()).unwrap_or_default();
        if let Some(m) = self.machines.get(&cur) {
            if self.project.data.machine.as_ref().map(|p| (p.rapid, p.tsc, p.x, p.y, p.z, &p.name, &p.notes)) != Some((m.rapid, m.tsc, m.x, m.y, m.z, &m.name, &m.notes)) {
                self.project.data.machine = Some(m.clone());
                self.modified = true;
                self.gen_pending = true;
            }
        }
        self.machines_dirty = true;
    }
}

pub fn machine_manager(ctx: &egui::Context, app: &mut App) {
    if !app.machines_open { return; }
    let mut open = true;
    let mut close = false;
    egui::Window::new("Machines").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).resizable(true)
        .default_size([760.0, 430.0]).min_width(700.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            if crate::csd::dialog_header(ui, "Machines", true) { close = true; }
            body(ui, app);
        });
    if close { open = false; }
    if !open {
        app.machines_open = false;
        app.machines.repair();
        app.sync_machine();
        app.save_machines();
    }
}

fn body(ui: &mut Ui, app: &mut App) {
    if app.machines.get(&app.mach_sel).is_none() { app.mach_sel = app.project.data.machine.as_ref().map(|m| m.id.clone()).unwrap_or_else(|| app.machines.def.clone()); }
    let h = 300.0;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(200.0, h), Layout::top_down(Align::LEFT), |ui| list(ui, app, h));
        let (sep, _) = ui.allocate_exact_size(egui::vec2(8.0, h), Sense::hover());
        ui.painter().line_segment([egui::pos2(sep.center().x, sep.min.y), egui::pos2(sep.center().x, sep.max.y)], ui.visuals().widgets.noninteractive.bg_stroke);
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width().max(380.0), h), Layout::top_down(Align::LEFT), |ui| form(ui, app));
    });
    ui.add_space(6.0);
    let (ln, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([ln.left_center(), ln.right_center()], ui.visuals().widgets.noninteractive.bg_stroke);
    ui.add_space(8.0);
    footer(ui, app);
    ui.add_space(8.0);
    let used = app.project.data.machine.as_ref().map(|p| p.id == app.mach_sel).unwrap_or(false);
    ui.weak(if !app.mach_msg.is_empty() { app.mach_msg.as_str() } else if used { "Used by the current project. Machines are saved automatically." } else { "Machines are saved automatically." });
}

fn footer(ui: &mut Ui, app: &mut App) {
    let sname = SPEED_UNITS[app.mach_speed_idx.min(SPEED_UNITS.len() - 1)].0;
    let uname = if app.mach_len_in { "in" } else { "mm" };
    ui.horizontal(|ui| {
        if app.confirm_mach_del {
            ui.colored_label(Color32::LIGHT_RED, "Delete this machine?");
            if ui.button("Delete").clicked() {
                let id = app.mach_sel.clone();
                match app.machines.delete(&id) {
                    Ok(next) => {
                        app.mach_sel = next;
                        // a project that used it moves to the default machine
                        if app.project.data.machine.as_ref().map(|p| p.id == id).unwrap_or(false) {
                            app.sync_machine();
                            app.modified = true;
                            app.mach_msg = "The project now uses another machine.".into();
                        } else { app.mach_msg.clear(); }
                        app.machines_dirty = true;
                    }
                    Err(e) => app.mach_msg = e,
                }
                app.confirm_mach_del = false;
                return;
            }
            if ui.button("Cancel").clicked() { app.confirm_mach_del = false; }
        } else {
            if icon_button(ui, Icon::Add, false, true, "New machine").clicked() {
                let id = app.machines.add(None, false);
                app.mach_sel = id;
                app.machine_edited();
            }
            if icon_button(ui, Icon::Copy, false, true, "Duplicate machine").clicked() {
                let sel = app.mach_sel.clone();
                let id = app.machines.add(Some(&sel), true);
                app.mach_sel = id;
                app.machine_edited();
            }
            if icon_button(ui, Icon::Trash, false, true, "Delete machine").clicked() {
                if app.machines.list.len() < 2 { app.mach_msg = "Keep at least one machine.".into(); } else { app.confirm_mach_del = true; }
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            egui::ComboBox::from_id_source("mspd").width(90.0).selected_text(sname).show_ui(ui, |ui| {
                for (i, (n, _)) in SPEED_UNITS.iter().enumerate() { ui.selectable_value(&mut app.mach_speed_idx, i, *n); }
            });
            ui.label("Speeds in");
            ui.add_space(8.0);
            egui::ComboBox::from_id_source("mlen").width(70.0).selected_text(uname).show_ui(ui, |ui| {
                ui.selectable_value(&mut app.mach_len_in, false, "mm");
                ui.selectable_value(&mut app.mach_len_in, true, "in");
            });
            ui.label("Lengths in");
        });
    });

}

fn list(ui: &mut Ui, app: &mut App, h: f32) {
    let used = app.project.data.machine.as_ref().map(|m| m.id.clone()).unwrap_or_default();
    let mut pick: Option<String> = None;
    egui::ScrollArea::vertical().id_source("machlist").max_height(h).auto_shrink([false, false]).show(ui, |ui| {
        for m in &app.machines.list {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), Sense::click());
            let sel = m.id == app.mach_sel;
            let vis = ui.visuals().clone();
            if sel { ui.painter().rect_filled(rect, 6.0, vis.selection.bg_fill); }
            else if resp.hovered() { ui.painter().rect_filled(rect, 6.0, vis.widgets.hovered.bg_fill); }
            let col = if sel { Color32::WHITE } else { vis.text_color() };
            let font = egui::TextStyle::Body.resolve(ui.style());
            let tag = if m.id == used { "in use" } else { "" };
            let gt = ui.painter().layout_no_wrap(tag.to_string(), font.clone(), col.gamma_multiply(0.7));
            let gn = ui.painter().layout_no_wrap(m.name.clone(), font, col);
            let clip = ui.painter().with_clip_rect(egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x - gt.size().x - 12.0, rect.max.y)));
            clip.galley(egui::pos2(rect.min.x + 10.0, rect.center().y - gn.size().y / 2.0), gn, col);
            ui.painter().galley(egui::pos2(rect.max.x - gt.size().x - 8.0, rect.center().y - gt.size().y / 2.0), gt, col);
            if resp.clicked() { pick = Some(m.id.clone()); }
        }
    });
    if let Some(id) = pick { app.mach_sel = id; app.confirm_mach_del = false; app.mach_msg.clear(); }
}

fn form(ui: &mut Ui, app: &mut App) {
    let k = if app.mach_len_in { 25.4 } else { 1.0 };
    let (sname, sfac) = SPEED_UNITS[app.mach_speed_idx.min(SPEED_UNITS.len() - 1)];
    let uname = if app.mach_len_in { "in" } else { "mm" };
    let Some(mut m) = app.machines.get(&app.mach_sel).cloned() else { return };
    let mut changed = false;

    // compact fixed layout: two label + field pairs side by side
    let (lw, fw) = (135.0_f32, 100.0_f32);
    let sp = 8.0_f32;
    let gap = 20.0_f32;
    let aw = 2.0 * (lw + fw) + gap + 4.0 * sp;
    let spacer = |ui: &mut Ui| { ui.allocate_exact_size(egui::vec2(gap, 1.0), Sense::hover()); };

    egui::Grid::new("mach_name").num_columns(2).spacing([sp, 8.0]).show(ui, |ui| {
        rlabel(ui, lw, "Name");
        changed |= ui.add(egui::TextEdit::singleline(&mut m.name).desired_width(aw - lw - sp + 8.0).char_limit(60)).changed(); ui.end_row();
    });
    ui.add_space(8.0);
    egui::Grid::new("mach_perf").num_columns(5).spacing([sp, 8.0]).show(ui, |ui| {
        rlabel(ui, lw, format!("Rapid rate ({sname})"));
        let mut r = m.rapid / sfac;
        if crate::widgets::num(ui, [fw, 22.0], &mut r, 0.01, 1.0e6, 10.0 / sfac, 3, "").changed() { m.rapid = (r * sfac).max(1.0); changed = true; }
        spacer(ui);
        rlabel(ui, lw, "Time correction ×");
        changed |= crate::widgets::num(ui, [fw, 22.0], &mut m.tsc, 0.1, 100.0, 0.01, 2, "")
            .on_hover_text("Multiplies the estimated time to account for acceleration and corners").changed();
        ui.end_row();
    });
    ui.add_space(10.0);
    ui.weak("Max size of operation (work area)");
    ui.add_space(4.0);
    egui::Grid::new("mach_area").num_columns(5).spacing([sp, 8.0]).show(ui, |ui| {
        for (i, (lab, v)) in [("X", &mut m.x), ("Y", &mut m.y), ("Z", &mut m.z)].into_iter().enumerate() {
            rlabel(ui, lw, format!("{lab} ({uname})"));
            let mut d = *v / k;
            if crate::widgets::num(ui, [fw, 22.0], &mut d, 0.1 / k, 100000.0 / k, 1.0 / k, 3, "").changed() { *v = (d * k).max(1.0); changed = true; }
            if i % 2 == 0 { spacer(ui); } else { ui.end_row(); }
        }
    });
    ui.add_space(8.0);
    egui::Grid::new("mach_notes").num_columns(2).spacing([sp, 8.0]).show(ui, |ui| {
        rlabel(ui, lw, "Notes");
        changed |= ui.add(egui::TextEdit::multiline(&mut m.notes).desired_rows(3).desired_width(aw - lw - sp + 8.0)).changed(); ui.end_row();
    });
    if changed {
        if let Some(slot) = app.machines.get_mut(&app.mach_sel) { *slot = m; }
        app.mach_msg.clear();
        app.machine_edited();
    }
}
