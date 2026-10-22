//! Tool manager: tool libraries (several named sets), tool list and tool editor, like the
//! browser app's "Tool library" dialog. Libraries live in the user's config directory.
use crate::app::App;
use crate::icons::{icon_button, paint_cutter, Icon};
use eframe::egui::{self, Align, Color32, Layout, Sense, Ui};
use putiacnc_core::toollib::{self, has_angle, shape_label, ToolLibs, SHAPES, SPEED_UNITS};
use std::path::PathBuf;

pub fn config_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("PUTIACNC_CONFIG_DIR") { return Some(PathBuf::from(d)); }
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") { return Some(PathBuf::from(x).join("putiacnc")); }
    if cfg!(windows) { return std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("PutiaCNC")); }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    Some(if cfg!(target_os = "macos") { home.join("Library/Application Support/PutiaCNC") } else { home.join(".config/putiacnc") })
}

pub fn libs_path() -> Option<PathBuf> { config_dir().map(|d| d.join("tool-libraries.json")) }

pub fn load_tool_libs() -> ToolLibs {
    libs_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| ToolLibs::from_json(&s).ok()).unwrap_or_default()
}

enum LibAct { Switch(String), New, Duplicate, Delete, Export, Import, Rename(String) }

impl App {
    /// Write the libraries to disk (normalised copy; the live data keeps what the user is typing).
    pub fn save_tool_libs(&mut self) {
        self.libs_dirty = false;
        let Some(p) = libs_path() else { return };
        let mut copy = self.tool_libs.clone();
        copy.repair();
        if let Some(dir) = p.parent() { let _ = std::fs::create_dir_all(dir); }
        if let Err(e) = std::fs::write(&p, copy.to_json()) { self.tool_msg = format!("Could not save the tool libraries: {e}"); }
    }

    /// Make the project's tools match the current library: tools the project brings along are
    /// added to the library, operations whose tool is missing are pointed at the first tool.
    /// Returns how many operations were re-pointed.
    pub fn sync_tools(&mut self) -> usize {
        let lib = self.tool_libs.current_mut();
        toollib::merge_project_tools(lib, &self.project.data.tools);
        let tools = lib.tools.clone();
        let n = toollib::reassign_missing(&mut self.project.data.ops, &tools);
        self.project.data.tools = tools;
        if n > 0 { self.modified = true; }
        self.gen_pending = true;
        self.libs_dirty = true;
        n
    }

    /// Force the project's tool list to the library's (used after undo/redo so the library stays the master).
    pub fn apply_lib_tools(&mut self) {
        let tools = self.tool_libs.current().tools.clone();
        toollib::reassign_missing(&mut self.project.data.ops, &tools);
        self.project.data.tools = tools;
    }

    /// Call after editing tools in the current library.
    fn tools_edited(&mut self) {
        self.project.data.tools = self.tool_libs.current().tools.clone();
        self.gen_pending = true;
        self.libs_dirty = true;
    }

    fn lib_action(&mut self, act: LibAct) {
        match act {
            LibAct::Switch(id) => {
                self.tool_libs.switch(&id);
                self.after_lib_switch(None);
            }
            LibAct::New => { self.tool_libs.new_library(); self.after_lib_switch(Some("Created a library with the common hobby tools.".into())); }
            LibAct::Duplicate => { self.tool_libs.duplicate(); self.after_lib_switch(Some("Saved a copy. You are now editing the copy.".into())); }
            LibAct::Delete => match self.tool_libs.delete_current() {
                Ok(name) => self.after_lib_switch(Some(format!("Deleted \"{name}\"."))),
                Err(e) => self.tool_msg = e,
            },
            LibAct::Rename(name) => { self.tool_libs.rename_current(&name); self.tool_msg = "Renamed.".into(); self.libs_dirty = true; }
            LibAct::Export => {
                let l = self.tool_libs.current();
                let fname = format!("{}-tools.json", l.name.chars().map(|c| if c.is_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect::<String>());
                self.spawn_dialog(crate::app::DlgKind::LibExport, move || rfd::FileDialog::new().add_filter("Tool library", &["json"]).set_file_name(fname).save_file());
            }
            LibAct::Import => {
                self.spawn_dialog(crate::app::DlgKind::LibImport, || rfd::FileDialog::new().add_filter("Tool library", &["json"]).pick_file());
            }
        }
    }

    pub fn finish_lib_export(&mut self, p: std::path::PathBuf) {
        self.tool_msg = match std::fs::write(&p, self.tool_libs.export_current()) {
            Ok(()) => format!("Exported \"{}\".", self.tool_libs.current().name),
            Err(e) => format!("Could not export: {e}"),
        };
    }

    pub fn finish_lib_import(&mut self, p: std::path::PathBuf) {
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|s| self.tool_libs.import(&s)) {
            Ok((name, n)) => self.after_lib_switch(Some(format!("Imported \"{name}\" ({n} tool{}).", if n == 1 { "" } else { "s" }))),
            Err(e) => self.tool_msg = e,
        }
    }

    fn after_lib_switch(&mut self, msg: Option<String>) {
        self.tool_sel = self.tool_libs.current().tools[0].id.clone();
        self.confirm_tool_del = false;
        self.confirm_lib_del = false;
        let before = self.project.data.clone();
        self.project.data.tools = self.tool_libs.current().tools.clone();
        let tools = self.project.data.tools.clone();
        let n = toollib::reassign_missing(&mut self.project.data.ops, &tools);
        if n > 0 {
            self.hist.push(before);
            self.modified = true;
        }
        let first = tools[0].name.clone();
        self.tool_msg = if n > 0 {
            format!("{n} operation{} switched to \"{first}\" because their tool is not in this library.", if n > 1 { "s" } else { "" })
        } else { msg.unwrap_or_else(|| format!("Loaded \"{}\".", self.tool_libs.current().name)) };
        self.gen_pending = true;
        self.libs_dirty = true;
    }
}

/// The window. Call every frame; it draws nothing while closed.
pub fn tool_manager(ctx: &egui::Context, app: &mut App) {
    if !app.tools_open { return; }
    let mut open = true;
    let mut close = false;
    egui::Window::new("Tool library").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).resizable(true)
        .default_size([880.0, 520.0]).min_width(760.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            if crate::csd::dialog_header(ui, "Tool library", true) { close = true; }
            body(ui, app);
        });
    if close { open = false; }
    if !open {
        app.tools_open = false;
        app.tool_libs.repair();
        app.tools_edited();
        app.save_tool_libs();
    }
}

fn unit_k(app: &App) -> f64 { if app.tool_len_in { 25.4 } else { 1.0 } }

fn body(ui: &mut Ui, app: &mut App) {

    // make sure a valid tool is selected
    if !app.tool_libs.current().tools.iter().any(|t| t.id == app.tool_sel) {
        app.tool_sel = app.tool_libs.current().tools[0].id.clone();
    }

    let h = 370.0;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(270.0, h), Layout::top_down(Align::LEFT), |ui| tool_list(ui, app, h));
        // fixed-height divider (ui.separator() would stretch to the whole available height and make the window grow)
        let (sep, _) = ui.allocate_exact_size(egui::vec2(8.0, h), Sense::hover());
        ui.painter().line_segment([egui::pos2(sep.center().x, sep.min.y), egui::pos2(sep.center().x, sep.max.y)], ui.visuals().widgets.noninteractive.bg_stroke);
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width().max(380.0), h), Layout::top_down(Align::LEFT), |ui| tool_form(ui, app));
    });
    ui.add_space(6.0);
    let (ln, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([ln.left_center(), ln.right_center()], ui.visuals().widgets.noninteractive.bg_stroke);
    ui.add_space(8.0);
    lib_row(ui, app);
    ui.add_space(8.0);
    ui.weak(if app.tool_msg.is_empty() { "Libraries are saved automatically." } else { app.tool_msg.as_str() });
}

fn tool_list(ui: &mut Ui, app: &mut App, h: f32) {
    let groups: [(&str, &str); 2] = if app.tool_len_in { [("in", "Imperial"), ("mm", "Metric")] } else { [("mm", "Metric"), ("in", "Imperial")] };
    let dark = app.theme.dark;
    let lib = app.tool_libs.current().clone();
    let mut pick: Option<String> = None;
    egui::ScrollArea::vertical().id_source("toollist").max_height(h - 44.0).auto_shrink([false, false]).show(ui, |ui| {
        for (sys, label) in groups {
            let g: Vec<_> = lib.tools.iter().filter(|t| t.sys == sys).collect();
            if g.is_empty() { continue; }
            ui.label(egui::RichText::new(label).strong().weak());
            for t in g {
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), Sense::click());
                let sel = t.id == app.tool_sel;
                let vis = ui.visuals().clone();
                if sel { ui.painter().rect_filled(rect, 6.0, vis.selection.bg_fill); }
                else if resp.hovered() { ui.painter().rect_filled(rect, 6.0, vis.widgets.hovered.bg_fill); }
                let col = if sel { Color32::WHITE } else { vis.text_color() };
                let ic = egui::Rect::from_center_size(egui::pos2(rect.min.x + 18.0, rect.center().y), egui::vec2(16.0, 24.0));
                paint_cutter(ui.painter(), ic, &t.shape, t.angle, t.tip / (t.dia / 2.0).max(1e-6), (t.arc / (t.dia / 2.0).max(1e-6), t.concave), if sel { Color32::WHITE } else { vis.text_color() });
                let font = egui::TextStyle::Body.resolve(ui.style());
                let dia = format!("Ø{}", toollib::fmt_dia(t.dia, &t.sys, false));
                let gd = ui.painter().layout_no_wrap(dia, font.clone(), col.gamma_multiply(0.7));
                let gn = ui.painter().layout_no_wrap(t.name.replace('″', "\""), font, col);
                let clip = ui.painter().with_clip_rect(egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x - gd.size().x - 14.0, rect.max.y)));
                clip.galley(egui::pos2(rect.min.x + 36.0, rect.center().y - gn.size().y / 2.0), gn, col);
                ui.painter().galley(egui::pos2(rect.max.x - gd.size().x - 8.0, rect.center().y - gd.size().y / 2.0), gd, col);
                if resp.clicked() { pick = Some(t.id.clone()); }
            }
            ui.add_space(4.0);
        }
    });
    let _ = dark;
    ui.add_space(4.0);
    tool_buttons(ui, app);
    if let Some(id) = pick {
        // show lengths in the unit the tool belongs to
        if let Some(t) = lib.tools.iter().find(|t| t.id == id) { app.tool_len_in = t.sys == "in"; }
        app.tool_sel = id; app.confirm_tool_del = false; app.tool_msg.clear(); }
}


fn tool_buttons(ui: &mut Ui, app: &mut App) {
    ui.horizontal(|ui| {
        if app.confirm_tool_del {
            ui.colored_label(Color32::LIGHT_RED, "Delete this tool?");
            if ui.button("Delete").clicked() {
                if app.tool_libs.current().tools.len() < 2 {
                    app.tool_msg = "Keep at least one tool in the library.".into();
                } else {
                    let id = app.tool_sel.clone();
                    app.hist.record(&app.project.data);
                    app.tool_libs.current_mut().tools.retain(|t| t.id != id);
                    let tools = app.tool_libs.current().tools.clone();
                    let n = toollib::reassign_missing(&mut app.project.data.ops, &tools);
                    app.tool_sel = tools[0].id.clone();
                    app.tools_edited();
                    if n > 0 { app.modified = true; app.tool_msg = format!("{n} operation{} now use \"{}\".", if n > 1 { "s" } else { "" }, tools[0].name); }
                }
                app.confirm_tool_del = false;
                return;
            }
            if ui.button("Cancel").clicked() { app.confirm_tool_del = false; }
        } else {
            if icon_button(ui, Icon::Add, false, true, "New tool").clicked() {
                let sel = app.tool_libs.current().tools.iter().find(|t| t.id == app.tool_sel).cloned();
                let sys = if app.tool_len_in { "in" } else { "mm" };
                let mut t = toollib::new_tool_from(sel.as_ref(), false, sys);
                t.sys = sys.into();
                app.tool_sel = t.id.clone();
                app.tool_libs.current_mut().tools.push(t);
                app.tools_edited();
                app.tool_msg.clear();
            }
            if icon_button(ui, Icon::Copy, false, true, "Duplicate tool").clicked() {
                let sel = app.tool_libs.current().tools.iter().find(|t| t.id == app.tool_sel).cloned();
                let sys = if app.tool_len_in { "in" } else { "mm" };
                let t = toollib::new_tool_from(sel.as_ref(), true, sys);
                app.tool_sel = t.id.clone();
                app.tool_libs.current_mut().tools.push(t);
                app.tools_edited();
                app.tool_msg.clear();
            }
            if icon_button(ui, Icon::Trash, false, true, "Delete tool").clicked() {
                if app.tool_libs.current().tools.len() < 2 { app.tool_msg = "Keep at least one tool in the library.".into(); } else { app.confirm_tool_del = true; }
            }
        }
    });
}

fn units_row(ui: &mut Ui, app: &mut App) {
    let uname = if app.tool_len_in { "in" } else { "mm" };
    let sname = SPEED_UNITS[app.tool_speed_idx.min(SPEED_UNITS.len() - 1)].0;
    ui.horizontal(|ui| {
        ui.set_max_width((ui.available_width() - 1.0).max(100.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            egui::ComboBox::from_id_source("tspd").width(90.0).selected_text(sname).show_ui(ui, |ui| {
                for (i, (n, _)) in SPEED_UNITS.iter().enumerate() { ui.selectable_value(&mut app.tool_speed_idx, i, *n); }
            });
            ui.label("Speeds in");
            ui.add_space(8.0);
            egui::ComboBox::from_id_source("tlen").width(70.0).selected_text(uname).show_ui(ui, |ui| {
                ui.selectable_value(&mut app.tool_len_in, false, "mm");
                ui.selectable_value(&mut app.tool_len_in, true, "in");
            });
            ui.label("Show lengths in");
        });
    });
}

fn lib_row(ui: &mut Ui, app: &mut App) {
    // The row is laid out 8 px wider than the window content so the glyph of the last icon (which has
    // padding inside its button) lines up with the field edges; the parent only reserves its real width.
    let avail = ui.available_width();
    let h = 36.0;
    let r = egui::Rect::from_min_size(ui.available_rect_before_wrap().min, egui::vec2(avail + 8.0, h));
    let mut child = ui.child_ui(r, Layout::left_to_right(Align::Center));
    child.set_clip_rect(r.expand(1.0));
    lib_row_inner(&mut child, app);
    ui.allocate_space(egui::vec2(avail, h));
}

fn lib_row_inner(ui: &mut Ui, app: &mut App) {
    let mut act: Option<LibAct> = None;
    ui.horizontal(|ui| {
        ui.label("Library");
        let cur = app.tool_libs.current().clone();
        egui::ComboBox::from_id_source("libsel").width(212.0)
            .selected_text(format!("{} ({} tool{})", cur.name, cur.tools.len(), if cur.tools.len() == 1 { "" } else { "s" }))
            .show_ui(ui, |ui| {
                for l in &app.tool_libs.libs {
                    let txt = format!("{} ({} tool{})", l.name, l.tools.len(), if l.tools.len() == 1 { "" } else { "s" });
                    if ui.selectable_label(l.id == cur.id, txt).clicked() && l.id != cur.id { act = Some(LibAct::Switch(l.id.clone())); }
                }
            });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, Icon::Import, false, true, "Import library from a file").clicked() { act = Some(LibAct::Import); }
            if icon_button(ui, Icon::Export, false, true, "Export library to a file").clicked() { act = Some(LibAct::Export); }
            ui.add_space(6.0);
            if app.confirm_lib_del {
                if ui.button("Cancel").clicked() { app.confirm_lib_del = false; }
                if ui.button("Delete").clicked() { act = Some(LibAct::Delete); }
                ui.colored_label(Color32::LIGHT_RED, "Delete library?");
            } else if icon_button(ui, Icon::Trash, false, true, "Delete library").clicked() {
                if app.tool_libs.libs.len() < 2 { app.tool_msg = "Keep at least one library.".into(); } else { app.confirm_lib_del = true; }
            }
            if icon_button(ui, Icon::Copy, false, true, "Duplicate library (edit the copy)").clicked() { act = Some(LibAct::Duplicate); }
            if icon_button(ui, Icon::Add, false, true, "New library with the common hobby tools").clicked() { act = Some(LibAct::New); }
            ui.add_space(6.0);
            let r = ui.add(egui::TextEdit::singleline(&mut app.lib_name_buf).desired_width(180.0).hint_text("Library name"));
            if !r.has_focus() { app.lib_name_buf = cur.name.clone(); }
            if r.lost_focus() && app.lib_name_buf.trim() != cur.name { act = Some(LibAct::Rename(app.lib_name_buf.clone())); }
        });
    });
    if let Some(a) = act { app.lib_action(a); }
}

fn tool_form(ui: &mut Ui, app: &mut App) {
    let k = unit_k(app);
    let fspd = SPEED_UNITS[app.tool_speed_idx.min(SPEED_UNITS.len() - 1)].1;
    let uname = if app.tool_len_in { "in" } else { "mm" };
    let sname = SPEED_UNITS[app.tool_speed_idx.min(SPEED_UNITS.len() - 1)].0;
    let Some(idx) = app.tool_libs.current().tools.iter().position(|t| t.id == app.tool_sel) else { return };
    let mut t = app.tool_libs.current().tools[idx].clone();
    let mut changed = false;

    // display units
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            let aw = ui.available_width() - 10.0;
            let lw = crate::panels::tw(ui, "Stepover").max(crate::panels::tw(ui, "Diameter")).max(crate::panels::tw(ui, "Max pass")).max(crate::panels::tw(ui, "Tip radius")).max(crate::panels::tw(ui, "Arc radius")) + 2.0;
            let fw = 120.0_f32;
            let sp = 8.0_f32; // label-to-field gap
            let gap = (aw - 2.0 * (lw + fw) - 4.0 * sp).max(8.0); // space between the two column pairs
            egui::Grid::new("tool_form_top").num_columns(2).spacing([sp, 8.0]).show(ui, |ui| {
                rlabel(ui, lw, "Name");
                changed |= ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(aw - lw - sp).char_limit(60)).changed(); ui.end_row();
            });
            ui.add_space(8.0);
            // two columns of label + field pairs
            egui::Grid::new("tool_form").num_columns(5).spacing([sp, 8.0]).show(ui, |ui| {
                // shape block: picture in the right-hand columns of the first row, painted over the rows below
                macro_rules! side { ($first:expr) => {{
                    ui.allocate_exact_size(egui::vec2(gap, 1.0), Sense::hover());
                    if $first {
                        let (cell, _) = ui.allocate_exact_size(egui::vec2(10.0, 22.0), Sense::hover());
                        let rect = egui::Rect::from_min_size(egui::pos2(cell.min.x + (lw + sp + fw) / 2.0 - 30.0, cell.min.y - 2.0), egui::vec2(60.0, if has_angle(&t.shape) { 128.0 } else if t.shape == "arc" { 98.0 } else { 62.0 }));
                        paint_cutter(ui.painter(), rect, &t.shape, t.angle, t.tip / (t.dia / 2.0).max(1e-6), (t.arc / (t.dia / 2.0).max(1e-6), t.concave), ui.visuals().text_color());
                    } else { ui.label(""); }
                    ui.label("");
                    ui.end_row();
                }}; }
                rlabel(ui, lw, "Shape");
                egui::ComboBox::from_id_source("tshape").width(fw - 4.0).selected_text(shape_label(&t.shape)).show_ui(ui, |ui| {
                    for (key, lab) in SHAPES { if ui.selectable_label(t.shape == key, lab).clicked() {
                        t.shape = key.into();
                        // a tapered ball nose has a rounded tip by definition
                        if key == "taper" && t.tip <= 0.0 { t.tip = 0.5; }
                        // an arc profile bit starts as a plain round-over (arc = half the diameter)
                        if key == "arc" && t.arc <= 0.0 { t.arc = t.dia / 2.0; }
                        changed = true;
                    } }
                });
                side!(true);
                if t.shape == "arc" {
                    rlabel(ui, lw, "Arc radius");
                    let mut r = t.arc / k;
                    if crate::widgets::num(ui, [72.0, 22.0], &mut r, if t.concave { t.dia / 2.0 / k } else { 0.1 / k }, 100.0 / k, 0.1 / k, 3, uname).changed() { t.arc = (r * k).max(if t.concave { t.dia / 2.0 } else { 0.1 }); changed = true; }
                    side!(false);
                    rlabel(ui, lw, "Edge");
                    egui::ComboBox::from_id_source("arc_edge").selected_text(if t.concave { "Concave" } else { "Convex" }).show_ui(ui, |ui| {
                        if ui.selectable_label(!t.concave, "Convex").clicked() { t.concave = false; changed = true; }
                        if ui.selectable_label(t.concave, "Concave").clicked() { t.concave = true; changed = true; }
                    });
                    side!(false);
                }
                if has_angle(&t.shape) {
                    rlabel(ui, lw, "Angle");
                    changed |= crate::widgets::num(ui, [60.0, 22.0], &mut t.angle, 10.0, 150.0, 0.5, 3, "°").changed();
                    side!(false);
                    rlabel(ui, lw, "Tip radius");
                    let mut r = t.tip / k;
                    if crate::widgets::num(ui, [72.0, 22.0], &mut r, 0.0, 50.0 / k, 0.05 / k, 3, uname).changed() { t.tip = (r * k).max(0.0); changed = true; }
                    side!(false);
                }
                rlabel(ui, lw, "Flutes");
                let mut fl = t.flutes as f64;
                if crate::widgets::num(ui, [60.0, 22.0], &mut fl, 1.0, 12.0, 0.1, 0, "").changed() { t.flutes = fl.round() as u32; changed = true; }
                side!(false);
                let mut n = 0;
                let (fu, uw) = (60.0_f32, fw - 60.0 - 8.0);
                macro_rules! cell { ($label:expr, $unit:expr, $ui:ident, $body:block) => {{
                    rlabel($ui, lw, $label);
                    $ui.horizontal(|$ui| {
                        $ui.spacing_mut().item_spacing.x = 8.0;
                        $body;
                        let (r, _) = $ui.allocate_exact_size(egui::vec2(uw, 22.0), Sense::hover());
                        $ui.painter().text(egui::pos2(r.min.x, r.center().y), egui::Align2::LEFT_CENTER, $unit, egui::TextStyle::Body.resolve($ui.style()), $ui.visuals().weak_text_color());
                    });
                    n += 1; if n % 2 == 0 { $ui.end_row(); } else { $ui.allocate_exact_size(egui::vec2(gap, 1.0), Sense::hover()); }
                }}; }
                cell!("Diameter", uname, ui, {
                    let mut d = t.dia / k;
                    if crate::widgets::num(ui, [fu, 22.0], &mut d, 0.01 / k, 1000.0 / k, 0.05 / k, 4, "").changed() { t.dia = d * k; t.sys = if app.tool_len_in { "in" } else { "mm" }.into(); changed = true; }
                });
                cell!("Spindle", "rpm", ui, { changed |= crate::widgets::num(ui, [fu, 22.0], &mut t.rpm, 0.0, 100000.0, 50.0, 0, "").changed(); });
                cell!("Feed", sname, ui, {
                    let mut f = t.feed / fspd;
                    if crate::widgets::num(ui, [fu, 22.0], &mut f, 0.001, 100000.0, 1.0 / fspd.max(1.0), 3, "").changed() { t.feed = f * fspd; changed = true; }
                });
                cell!("Plunge", sname, ui, {
                    let mut f = t.plunge / fspd;
                    if crate::widgets::num(ui, [fu, 22.0], &mut f, 0.001, 100000.0, 1.0 / fspd.max(1.0), 3, "").changed() { t.plunge = f * fspd; changed = true; }
                });
                cell!("Stepover", "%", ui, { changed |= crate::widgets::num(ui, [fu, 22.0], &mut t.so, 1.0, 100.0, 0.5, 1, "").changed(); });
                cell!("Max pass", uname, ui, {
                    let mut d = t.pd / k;
                    if crate::widgets::num(ui, [fu, 22.0], &mut d, 0.01 / k, 1000.0 / k, 0.05 / k, 4, "").changed() { t.pd = d * k; changed = true; }
                });
                if n % 2 == 1 { ui.end_row(); }
            });
            ui.add_space(8.0);
            // what these numbers mean for the cut
            egui::Grid::new("tool_form_est").num_columns(2).spacing([sp, 4.0]).show(ui, |ui| {
                use putiacnc_core::toollib::{chip_load, chip_load_range, load_level, removal_rate, Load};
                let (unit_len, per) = if app.tool_len_in { ("in", 25.4) } else { ("mm", 1.0) };
                rlabel(ui, lw, "Chip load");
                match chip_load(&t) {
                    Some(c) => {
                        let (lo, hi) = chip_load_range(&t);
                        let dark = ui.visuals().dark_mode;
                        let (txt, col) = match load_level(&t) {
                            Some(Load::Light) => ("light: the cutter may rub and heat up", if dark { Color32::from_rgb(0xf8, 0xe4, 0x5c) } else { Color32::from_rgb(0xb5, 0x83, 0x00) }),
                            Some(Load::Heavy) => ("heavy: risk of a rough cut or a broken bit", if dark { Color32::from_rgb(0xff, 0x7b, 0x63) } else { Color32::from_rgb(0xc0, 0x1c, 0x28) }),
                            _ => ("in the usual range", if dark { Color32::from_rgb(0x8f, 0xd1, 0x8a) } else { Color32::from_rgb(0x2a, 0x8a, 0x3e) }),
                        };
                        ui.horizontal(|ui| {
                            ui.label(format!("{:.4} {unit_len}/tooth", c / per));
                            ui.label(egui::RichText::new(txt).color(col));
                        }).response.on_hover_text(format!("Feed / (spindle speed x flutes). A usual range for wood and plastics at this size is {:.4} to {:.4} {unit_len}/tooth; adjust it for your material and machine.", lo / per, hi / per));
                    }
                    None => { ui.weak("set the spindle speed to see it"); }
                }
                ui.end_row();
                rlabel(ui, lw, "Removal");
                let r = removal_rate(&t);
                let txt = if app.tool_len_in { format!("{:.2} in³/min", r / 16387.064) } else { format!("{:.1} cm³/min", r / 1000.0) };
                ui.weak(txt).on_hover_text("Material removed per minute by a cut as wide as the stepover and as deep as the maximum pass");
                ui.end_row();
            });
            ui.add_space(8.0);
            egui::Grid::new("tool_form_notes").num_columns(2).spacing([sp, 8.0]).show(ui, |ui| {
                rlabel(ui, lw, "Notes");
                changed |= ui.add(egui::TextEdit::multiline(&mut t.notes).desired_rows(3).desired_width(aw - lw - sp)).changed(); ui.end_row();
            });
        });
    });

    if changed {
        app.tool_libs.current_mut().tools[idx] = t;
        app.tools_edited();
        app.tool_msg.clear();
    }
    ui.add_space((ui.available_height() - 56.0).max(0.0));
    // its own line, so it never runs into the unit selectors however narrow the window is
    let used = app.project.data.ops.iter().filter(|o| o.p.tool == app.tool_sel).count();
    ui.weak(if used > 0 { format!("Used by {used} operation{} in this project.", if used > 1 { "s" } else { "" }) } else { " ".to_string() });
    units_row(ui, app);
}

/// Label right-aligned in a column of width `w`, so it sits next to its field.
pub fn rlabel(ui: &mut Ui, w: f32, text: impl Into<String>) {
    let text: String = text.into();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let g = ui.painter().layout_no_wrap(text, font, ui.visuals().text_color());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 22.0), Sense::hover());
    ui.painter().galley(egui::pos2(rect.max.x - g.size().x, rect.center().y - g.size().y / 2.0), g, ui.visuals().text_color());
}
