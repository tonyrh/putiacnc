//! Side panels, toolbar and status bar.
use crate::app::{fmt_time, Action, App, RightTab};
use eframe::egui::{self, Color32, Ui};
use putiacnc_core::model::Operation;
use crate::icons::{icon_text_button, paint_icon, small_icon_button, Icon};
use crate::toolmgr::rlabel;
use putiacnc_core::shapes;

/// The default font has no ″ glyph; show a plain quote instead.
fn clean(s: &str) -> String { s.replace('″', "\"") }

pub const KINDS: &[(&str, &str)] = &[("outside", "Profile outside"), ("inside", "Profile inside"), ("online", "On line"), ("pocket", "Pocket"), ("vcarve", "V-carve"), ("drill", "Drill"), ("inlay", "Inlay pocket"), ("plug", "Inlay plug"), ("relief", "3D relief")];
const PROFILES: &[(&str, &str)] = &[("outside", "Outside"), ("inside", "Inside"), ("online", "On")];
const DIRS: &[(&str, &str)] = &[("climb", "Climb"), ("conv", "Conventional")];
const ZEROS: &[(&str, &str)] = &[("ll", "Lower-left"), ("lr", "Lower-right"), ("ul", "Upper-left"), ("ur", "Upper-right"), ("c", "Center")];
const MATERIALS: &[(&str, &str)] = &[("wood", "Light wood"), ("darkwood", "Dark wood"), ("metal", "Metal"), ("foam", "Foam")];
const ZMODES: &[(&str, &str)] = &[("top", "Stock top"), ("h", "Height")];

pub fn label_of(opts: &[(&str, &str)], v: &str) -> String {
    opts.iter().find(|(k, _)| *k == v).map(|(_, l)| l.to_string()).unwrap_or_else(|| v.to_string())
}

fn tool_pd_of(tools: &[putiacnc_core::model::Tool], id: &str) -> Option<f64> { tools.iter().find(|t| t.id == id).map(|t| t.pd) }

/// Little up/down arrows (a spin button) for whole-number fields. Returns true when a value changed.
fn spin_arrows(ui: &mut Ui, v: &mut f64, lo: f64, hi: f64) -> bool {
    let h = ui.spacing().interact_size.y;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, h), egui::Sense::hover());
    let mut changed = false;
    for (k, up) in [(0, true), (1, false)] {
        let r = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, k as f32 * h / 2.0), egui::vec2(16.0, h / 2.0));
        let resp = ui.interact(r, ui.id().with(("spin", k)), egui::Sense::click());
        let enabled = if up { *v < hi } else { *v > lo };
        if resp.hovered() && enabled { ui.painter().rect_filled(r.shrink(1.0), 3.0, ui.visuals().widgets.hovered.bg_fill); }
        let col = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color().gamma_multiply(0.5) };
        let c = r.center();
        let d = if up { -1.0_f32 } else { 1.0 };
        ui.painter().add(egui::Shape::convex_polygon(vec![c + egui::vec2(-3.5, -d * 1.5), c + egui::vec2(3.5, -d * 1.5), c + egui::vec2(0.0, d * 2.0)], col, egui::Stroke::NONE));
        if resp.clicked() && enabled { *v = (*v + if up { 1.0 } else { -1.0 }).clamp(lo, hi); changed = true; }
    }
    changed
}

fn dv_w(ui: &mut Ui, w: f32, v: &mut f64, lo: f64, hi: f64, suffix: &str) -> bool {
    crate::widgets::num(ui, [w, ui.spacing().interact_size.y], v, lo, hi, 0.25, 2, suffix).changed()
}

fn combo_w2(ui: &mut Ui, w: f32, id: &str, cur: &mut String, opts: &[(&str, &str)], prefix: &str, tip: &str) -> (bool, egui::Rect) {
    let mut ch = false;
    let r = egui::ComboBox::from_id_source(id).width(w - 10.0).selected_text(format!("{prefix}{}", label_of(opts, cur))).show_ui(ui, |ui| {
        for (k, l) in opts {
            if ui.selectable_label(cur == k, *l).clicked() && cur != k { *cur = k.to_string(); ch = true; }
        }
    });
    if !tip.is_empty() { r.response.clone().on_hover_text(tip); }
    (ch, r.response.rect)
}

fn combo_w(ui: &mut Ui, w: f32, id: &str, cur: &mut String, opts: &[(&str, &str)], prefix: &str, tip: &str) -> bool {
    combo_w2(ui, w, id, cur, opts, prefix, tip).0
}

pub fn menu_bar(ui: &mut Ui, app: &mut App) {
    fn item(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
        let r = ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut));
        if r.clicked() { ui.close_menu(); true } else { false }
    }
    egui::menu::bar(ui, |ui| {
        // roomier, rounded menu buttons like a GNOME header bar (menu::bar sets tight defaults)
        {
            let st = ui.style_mut();
            st.spacing.button_padding = egui::vec2(12.0, 6.0);
            st.spacing.item_spacing.x = 4.0;
            st.spacing.interact_size.y = 32.0;
            let r = egui::Rounding::same(7.0);
            st.visuals.widgets.hovered.rounding = r;
            st.visuals.widgets.active.rounding = r;
            st.visuals.widgets.open.rounding = r;
            st.visuals.widgets.inactive.rounding = r;
        }
        ui.menu_button("File", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            if item(ui, "New project", "Ctrl+N", true) { app.request(Action::New); }
            if item(ui, "Open project...", "Ctrl+O", true) { app.open_dialog(); }
            let recent: Vec<String> = app.prefs.recent.iter().filter(|p| std::path::Path::new(p.as_str()).is_file()).cloned().collect();
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button("Open recent", |ui| {
                    ui.style_mut().wrap = Some(false);
                    for p in &recent {
                        let path = std::path::Path::new(p);
                        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
                        let dir = path.parent().map(|d| d.display().to_string()).unwrap_or_default();
                        if ui.button(name).on_hover_text(dir).clicked() { app.request(Action::Open(p.clone())); ui.close_menu(); }
                    }
                    ui.separator();
                    if ui.button("Clear list").clicked() { app.prefs.recent.clear(); ui.close_menu(); }
                });
            });
            if item(ui, "Import Vectors...", "Ctrl+I", true) { app.import_dialog(); }
            if item(ui, "Import Bitmap (trace)...", "", true) { app.import_bitmap_dialog(); }
            if item(ui, "Import 3D Model (STL, OBJ)...", "", true) { app.import_model_dialog(); }
            ui.separator();
            if item(ui, "Save", "Ctrl+S", true) { app.save(); }
            if item(ui, "Save as...", "Ctrl+Shift+S", true) { app.save_as_dialog(); }
            ui.separator();
            if item(ui, "Export G-code...", "Ctrl+E", app.output.is_some()) { app.export_gcode_dialog(); }
            ui.separator();
            if item(ui, "Quit", "Ctrl+Q", true) { app.request(Action::Quit); }
        });
        ui.menu_button("Edit", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            use crate::app::Scope;
            if item(ui, "Undo", "Ctrl+Z", app.hist.can_undo(Scope::All)) { app.do_undo(); }
            if item(ui, "Redo", "Ctrl+Shift+Z", app.hist.can_redo(Scope::All)) { app.do_redo(); }
            ui.separator();
            if item(ui, "Select all", "Ctrl+A", true) { app.sel = (0..app.project.data.loops.len()).collect(); }
            if item(ui, "Duplicate", "Ctrl+D", !app.sel.is_empty()) { app.duplicate_selection(); }
            if item(ui, "Delete", "Del", !app.sel.is_empty()) { app.delete_selection(); }
            ui.separator();
            if item(ui, "Offset...", "", !app.sel.is_empty()) { app.offset_open = true; }
            if item(ui, "Array and copy...", "", !app.sel.is_empty()) { app.array.open = true; }
            if item(ui, "Fillet and chamfer...", "", app.selection_has_corners()) { app.corners.open = true; }
            if item(ui, "Combine shapes...", "", app.sel.len() >= 2) { app.combine_open = true; }
            if item(ui, "Clean up drawing...", "", !app.project.data.loops.is_empty()) { app.clean.open = true; app.clean.msg.clear(); }
        });
        ui.menu_button("View", |ui| {
            ui.style_mut().wrap = Some(false);
            // wide enough for the longest checkbox, so the shortcut of the action row sits at the real right edge
            ui.set_min_width(236.0);
            if item(ui, "Fit to stock", "F", true) { if app.view_tab == crate::view3d::ViewTab::Preview { app.fit3d_pending = true; } else { app.fit_pending = true; } }
            ui.separator();
            ui.checkbox(&mut app.show_toolpath, "Toolpaths");
            ui.checkbox(&mut app.show_rapids, "Rapid moves");
            ui.checkbox(&mut app.show_badges, "Operation badges on shapes");
            ui.separator();
            ui.checkbox(&mut app.show_machine_area, "Machine work area");
            ui.checkbox(&mut app.show_clamps, "Clamps");
            ui.separator();
            ui.checkbox(&mut app.grid_on, "Grid");
            ui.checkbox(&mut app.show_rulers, "Rulers");
            ui.checkbox(&mut app.show_axes, "Axes");
            ui.checkbox(&mut app.show_open_ends, "Loose ends of open contours");
            ui.separator();
            ui.checkbox(&mut app.show_left, "Project panel");
            ui.checkbox(&mut app.show_right, "Operations / G-code panel");
        });
        ui.menu_button("Libraries", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            if ui.button("Tool library...").clicked() { app.tools_open = true; ui.close_menu(); }
            if ui.button("Machines...").clicked() { app.machines_open = true; ui.close_menu(); }
        });
        ui.menu_button("Help", |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(200.0);
            if ui.add(egui::Button::new("Keyboard shortcuts").shortcut_text("F1")).clicked() { app.shortcuts_open = true; ui.close_menu(); }
            ui.separator();
            if ui.button("About PutiaCNC...").clicked() { app.about_open = true; ui.close_menu(); }
        });
    });
}

/// Modal-style prompt shown when an action would discard unsaved changes.
/// Fixed toolbar under the menu: undo/redo and view options.
pub fn tool_bar(ui: &mut Ui, app: &mut App) {
    if app.view_tab == crate::view3d::ViewTab::Preview {
        ui.horizontal(|ui| {
            // same right-hand spots as in the 2D toolbar; the hint takes the left
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let sz = guides_size(ui);
                ui.allocate_exact_size(sz, egui::Sense::hover()); // keeps the spacing identical to the 2D toolbar
                ui.checkbox(&mut app.show_rapids, "Rapids");
                ui.checkbox(&mut app.show_toolpath, "Toolpaths");
            });
        });
        return;
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut app.grid_on, "Grid");
        crate::widgets::num(ui, [72.0, ui.spacing().interact_size.y], &mut app.snap_mm, 0.01, 500.0, 0.1, 2, " mm").on_hover_text("Grid size (also the snap step)");
        dropup(ui, app, "snap_menu", "Snap", |ui, app| {
            ui.set_width(170.0);
            ui.checkbox(&mut app.snap_on, "To the grid").on_hover_text("The pointer jumps to the grid; the grid size is the snap step");
            ui.checkbox(&mut app.osnap_on, "To shapes").on_hover_text("The pointer jumps to the corners, midpoints, centres and crossings of shapes, and to the stock");
        });
        // view toggles on the right (right-to-left, so listed in reverse)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            guides_menu(ui, app);
            ui.checkbox(&mut app.show_rapids, "Rapids");
            ui.checkbox(&mut app.show_toolpath, "Toolpaths");
        });
    });
}

/// The "Guides" drop-up in the bottom bar: a button with a painted arrow (a text arrow is a box in some fonts) whose panel opens upwards
/// and stays open while the toggles are clicked.
fn dropup_size(ui: &Ui, label: &str) -> egui::Vec2 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let w = ui.painter().layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER).size().x;
    egui::vec2(w + 36.0, ui.spacing().interact_size.y)
}

fn guides_size(ui: &Ui) -> egui::Vec2 { dropup_size(ui, "Guides") }

fn guides_menu(ui: &mut Ui, app: &mut App) {
    dropup(ui, app, "guides_menu", "Guides", |ui, app| {
        ui.set_width(190.0); // a fixed width: the separator would otherwise stretch the menu across the screen
        ui.checkbox(&mut app.show_rulers, "Rulers").on_hover_text("Rulers in mm along the top and left of the drawing area; they mark the pointer and the selection");
        ui.checkbox(&mut app.show_axes, "Axes").on_hover_text("Graduated X and Y axes along the stock's lower and left edges");
        ui.checkbox(&mut app.show_machine_area, "Machine area").on_hover_text("Outline of the machine work area, from the stock's lower-left corner");
    });
}

/// A button with a painted arrow whose panel opens upwards and stays open while its toggles are clicked.
fn dropup(ui: &mut Ui, app: &mut App, name: &str, label: &str, contents: impl FnOnce(&mut Ui, &mut App)) {
    let id = ui.id().with(name);
    let open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
    let size = dropup_size(ui, label);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let v = ui.style().interact_selectable(&resp, open);
    ui.painter().rect(rect.expand(v.expansion), v.rounding, v.bg_fill, v.bg_stroke);
    let col = v.fg_stroke.color;
    ui.painter().galley(egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, col);
    let c = egui::pos2(rect.right() - 12.0, rect.center().y);
    let tri = if open { [c + egui::vec2(-4.0, -2.0), c + egui::vec2(4.0, -2.0), c + egui::vec2(0.0, 3.0)] } else { [c + egui::vec2(-4.0, 3.0), c + egui::vec2(4.0, 3.0), c + egui::vec2(0.0, -2.0)] };
    ui.painter().add(egui::Shape::convex_polygon(tri.to_vec(), col, egui::Stroke::NONE));
    let mut open_now = open;
    if resp.clicked() { open_now = !open; }
    if open_now {
        let area = egui::Area::new(id).order(egui::Order::Foreground).pivot(egui::Align2::RIGHT_BOTTOM).fixed_pos(rect.right_top() - egui::vec2(0.0, 4.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    contents(ui, app);
                });
            });
        let outside = ui.input(|i| i.pointer.any_click()) && !resp.clicked()
            && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p));
        if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) { open_now = false; }
    }
    if open_now != open { ui.data_mut(|d| d.insert_temp(id, open_now)); ui.ctx().request_repaint(); }
}

/// Dialog button with an icon: `fill` None = the normal button look; otherwise a tinted / solid button.
pub fn dlg_button(ui: &mut Ui, icon: Icon, text: &str, fill: Option<Color32>, fg: Option<Color32>, solid: bool) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let (icon_w, gap, padx) = (16.0, 7.0, 14.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER);
    let size = egui::vec2(padx * 2.0 + icon_w + gap + galley.size().x, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let v = *ui.style().interact(&resp);
    let (bg, stroke, col) = match (fill, fg) {
        (Some(f), Some(c)) => {
            let hot = resp.hovered() || resp.is_pointer_button_down_on();
            if solid { (if hot { f.gamma_multiply(0.88) } else { f }, egui::Stroke::NONE, c) }
            else { (f.gamma_multiply(if hot { 0.28 } else { 0.14 }), egui::Stroke::new(1.0_f32, c.gamma_multiply(0.55)), c) }
        }
        _ => (v.bg_fill, v.bg_stroke, v.fg_stroke.color),
    };
    ui.painter().rect(rect, 7.0, bg, stroke);
    let x0 = rect.center().x - (icon_w + gap + galley.size().x) / 2.0;
    paint_icon(ui.painter(), icon, egui::pos2(x0 + icon_w / 2.0, rect.center().y), col, 0.62);
    ui.painter().galley(egui::pos2(x0 + icon_w + gap, rect.center().y - galley.size().y / 2.0), galley, col);
    resp
}

pub fn confirm_dialog(ctx: &egui::Context, app: &mut App) {
    let Some(action) = app.pending.clone() else { return };
    let mut choice = 0; // 1 save, 2 discard, 3 cancel
    let dark = app.theme.dark;
    let accent = app.theme.accent;
    let red = if dark { Color32::from_rgb(0xff, 0x7b, 0x63) } else { Color32::from_rgb(0xc0, 0x1c, 0x28) };
    let amber = if dark { Color32::from_rgb(0xf8, 0xe4, 0x5c) } else { Color32::from_rgb(0xf5, 0xc2, 0x11) };
    let name = app.file_name();
    egui::Window::new("Unsaved changes").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.add_space(10.0);
            ui.horizontal_top(|ui| {
                ui.add_space(6.0);
                // warning badge
                let (r, _) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 22.0, amber.gamma_multiply(0.22));
                ui.painter().circle_filled(r.center(), 15.0, amber);
                let dark_ink = Color32::from_rgb(0x3d, 0x2e, 0x00);
                ui.painter().line_segment([r.center() + egui::vec2(0.0, -7.0), r.center() + egui::vec2(0.0, 2.5)], egui::Stroke::new(2.6_f32, dark_ink));
                ui.painter().circle_filled(r.center() + egui::vec2(0.0, 7.0), 1.8, dark_ink);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.set_width(340.0);
                    ui.label(egui::RichText::new("Save changes?").size(18.0).strong());
                    ui.label(egui::RichText::new(format!("\"{name}\" has unsaved changes. If you don't save them, they will be lost.")).color(ui.visuals().weak_text_color()));
                });
            });
            ui.add_space(18.0);
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 32.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if dlg_button(ui, Icon::Save, "Save", Some(accent), Some(Color32::WHITE), true).clicked() { choice = 1; }
                if dlg_button(ui, Icon::Trash, "Discard", Some(red), Some(red), false).clicked() { choice = 2; }
                if dlg_button(ui, Icon::Close, "Cancel", None, None, false).clicked() { choice = 3; }
            });
            ui.add_space(8.0);
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { choice = 3; }
    match choice {
        1 => {
            if app.path.trim().is_empty() { app.after_save = Some(action); app.pending = None; app.save_as_dialog(); }
            else { app.save(); if !app.modified { app.perform(action); } }
        }
        2 => app.perform(action),
        3 => { app.pending = None; app.quit_ok = false; }
        _ => {}
    }
}

pub fn status_bar(ui: &mut Ui, app: &mut App) {
    ui.horizontal(|ui| {
        crate::checkui::chip(ui, app);
        // a generation that takes a moment shows itself; quick ones (most) do not, so the bar does not flicker
        let slow = app.gen_started.is_some_and(|t| t.elapsed().as_millis() > 200);
        if slow {
            ui.add(egui::Spinner::new().size(12.0));
            ui.label("generating…");
            ui.separator();
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        match &app.output {
            Some(o) => {
                ui.label(format!("Estimated time {}", fmt_time(o.time_min)));
                ui.separator();
                ui.label(format!("cutting {}", fmt_time(o.cut_time_min)));
                ui.separator();
                ui.label(format!("cut length {:.1} mm", o.cut_len_mm));
                ui.separator();
                ui.label(format!("{} lines, generated in {:.0} ms", o.lines.len(), app.gen_ms));
            }
            // the first toolpath of a project is still being worked out: the spinner says so
            None if app.gen_stale() && app.gen_error.is_none() => {}
            None if app.project.data.ops.is_empty() => { ui.label("Add an operation to generate G-code"); }
            None => { ui.label(app.gen_error.clone().unwrap_or_else(|| "No G-code".into())); }
        }
        ui.separator();
        ui.label(format!("{} shapes, {} selected", app.project.data.loops.len(), app.sel.len()));
        if let Some(msg) = app.note.take() { let now = ui.input(|i| i.time); ui.ctx().data_mut(|x| x.insert_temp(egui::Id::new("status_note"), (msg, now))); }
        let note: Option<(String, f64)> = ui.ctx().data(|x| x.get_temp(egui::Id::new("status_note")));
        if let Some((msg, t0)) = note {
            let age = ui.input(|i| i.time) - t0;
            if age < 6.0 {
                ui.separator();
                ui.label(egui::RichText::new(msg).strong());
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            }
        }
    });
}

pub fn right_panel(ui: &mut Ui, app: &mut App) {
    let theme = app.theme;
    let n_warn = app.output.as_ref().map_or(0, |o| o.notes.iter().filter(|n| n.starts_with("⚠ ")).count());
    crate::view3d::tabs_badged(ui, &theme, &mut app.tab, &[(RightTab::Ops, "Operations", 0), (RightTab::Gcode, "G-code", 0), (RightTab::Info, "Notes", n_warn)]);
    ui.separator();
    if app.tab == RightTab::Ops { ops_panel(ui, app); return; }
    if app.tab == RightTab::Info { notes_panel(ui, app); return; }
    if app.tab == RightTab::Gcode {
        ui.horizontal(|ui| {
            if ui.checkbox(&mut app.imperial, "Inches (G20)").on_hover_text("Write the G-code in inches instead of millimetres").changed() { app.gen_pending = true; }
        });
        let (d, n) = (if app.project.data.job.mach() { "Mach3 / LinuxCNC" } else { "GRBL" }, app.project.data.job.numbered);
        ui.add(egui::Label::new(egui::RichText::new(format!("Output for {d}{}. Dialect, line numbers and safe Z are set per machine (Libraries → Machines).", if n { ", numbered lines" } else { "" })).weak()).wrap(true));
        ui.add_space(4.0);
    }
    let mut export = false;
    let mut copy: Option<String> = None;
    match (&app.output, app.tab) {
        (Some(o), RightTab::Gcode) => {
            // Copy / Export sit at the bottom; the G-code takes the rest of the height
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Copy").on_hover_text("Copy the G-code to the clipboard").clicked() { copy = Some(o.gcode()); }
                    if ui.button("Export...").on_hover_text("Save the G-code to a file (Ctrl+E)").clicked() { export = true; }
                });
                ui.add_space(6.0);
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
                    egui::ScrollArea::both().auto_shrink([false, false]).show_rows(ui, row_h, o.lines.len(), |ui, range| {
                        for i in range { ui.add(egui::Label::new(egui::RichText::new(&o.lines[i]).monospace()).wrap(false)); }
                    });
                });
            });
        }
        (_, RightTab::Ops | RightTab::Info) => {}
        (None, _) if app.project.data.ops.is_empty() => { ui.weak("Add an operation to generate G-code."); }
        (None, _) => { ui.label(app.gen_error.clone().unwrap_or_else(|| "Nothing generated yet.".into())); }
    }
    if let Some(t) = copy { ui.ctx().copy_text(t); }
    if export { app.export_gcode_dialog(); }
}

/// The Notes tab: the project's own notes (always editable, saved with the project) and, below, what the
/// toolpath generator wants the user to know.
fn notes_panel(ui: &mut Ui, app: &mut App) {
    let warnings: Vec<String> = app.output.as_ref().map(|o| o.notes.clone()).unwrap_or_default();
    let reserve = if warnings.is_empty() { 0.0 } else { 40.0 + 20.0 * warnings.len().min(6) as f32 };
    ui.label("Project notes");
    let row_h = ui.text_style_height(&egui::TextStyle::Body);
    let rows = (((ui.available_height() - reserve - 12.0) / row_h).floor() as usize).max(4);
    let before = app.project.data.clone();
    let mut notes = std::mem::take(&mut app.project.data.notes);
    let r = ui.add(egui::TextEdit::multiline(&mut notes).desired_width(f32::INFINITY).desired_rows(rows).hint_text("Material, fixtures, jobs to remember..."));
    app.project.data.notes = notes;
    if r.changed() {
        // one undo step per editing session; while the field has focus, Ctrl+Z undoes the typing itself
        if !app.hist.open { app.hist.push(before); app.hist.open = true; }
        app.modified = true;
    }
    if !warnings.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Warnings").strong());
        for n in &warnings {
            match n.strip_prefix("⚠ ") {
                Some(rest) => { crate::checkui::warn_line(ui, if app.theme.dark { Color32::from_rgb(255, 176, 64) } else { Color32::from_rgb(190, 100, 0) }, rest); }
                None => { ui.label(format!("• {n}")); }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum GroupAct { Delete, EditText, ToOutlines }

/// X, Y, width and height of an imported thing. Returns the new (x, y, width, height) when a field was edited.
/// `zero` is the XY zero of the stock: X and Y are shown (and typed) relative to it, like the rulers.
fn placement_fields(ui: &mut Ui, key: egui::Id, b: [f64; 4], zero: (f64, f64), aspect_fixed: bool, fit: &mut bool, mut lock: Option<&mut bool>) -> Option<[f64; 4]> {
    let locked = lock.as_ref().is_some_and(|l| **l);
    let (mut x, mut y, mut w, mut h) = (b[0] - zero.0, b[1] - zero.1, b[2] - b[0], b[3] - b[1]);
    let keep_id = key.with("keep");
    let mut keep = aspect_fixed || ui.data(|d| d.get_temp::<bool>(keep_id)).unwrap_or(true);
    let (mut moved, mut sized) = (false, false);
    let lw = tw(ui, "Width").max(tw(ui, "Height")) + 8.0;
    ui.add_enabled_ui(!locked, |ui| egui::Grid::new(key.with("grid")).num_columns(5).spacing([6.0, 6.0]).show(ui, |ui| {
        rlabel(ui, lw, "X"); moved |= dv_w(ui, 62.0, &mut x, -100000.0, 100000.0, ""); ui.weak("mm");
        rlabel(ui, lw, "Y"); moved |= dv_w(ui, 62.0, &mut y, -100000.0, 100000.0, ""); ui.weak("mm"); ui.end_row();
        rlabel(ui, lw, "Width");
        if dv_w(ui, 62.0, &mut w, 0.1, 100000.0, "") { sized = true; if keep && b[2] - b[0] > 1e-9 { h = w * (b[3] - b[1]) / (b[2] - b[0]); } }
        ui.weak("mm");
        rlabel(ui, lw, "Height");
        if dv_w(ui, 62.0, &mut h, 0.1, 100000.0, "") { sized = true; if keep && b[3] - b[1] > 1e-9 { w = h * (b[2] - b[0]) / (b[3] - b[1]); } }
        ui.weak("mm"); ui.end_row();
    }));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if !aspect_fixed && ui.add_enabled(!locked, egui::Checkbox::new(&mut keep, "Keep proportions")).changed() { ui.data_mut(|d| d.insert_temp(keep_id, keep)); }
        if let Some(l) = lock.as_mut() { ui.checkbox(l, "Lock").on_hover_text("Stops this drawing being moved, resized, rotated or flipped by accident"); }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().button_padding = egui::vec2(10.0, 4.0);
            if ui.add_enabled(!locked, egui::Button::new("Fit to stock")).on_hover_text("Scale to the stock if it is bigger, and centre it").clicked() { *fit = true; }
        });
    });
    if moved || sized { Some([x + zero.0, y + zero.1, w, h]) } else { None }
}

/// The small round-arrow button that puts a field back to its default: a real button with the usual padding.
fn reset_button(ui: &mut Ui, tip: &str) -> bool {
    ui.add_sized([26.0, ui.spacing().interact_size.y], egui::Button::new("↺")).on_hover_text(tip).clicked()
}

/// The imported files (drawings and the 3D model) with their position and size, and what can be done with them.
fn imports_section(ui: &mut Ui, d: &mut putiacnc_core::model::ProjectData, _sel: &std::collections::BTreeSet<usize>, act: &mut Option<(Option<i64>, GroupAct)>, changed: &mut bool) {
    // forget drawings whose shapes are all gone
    let alive: Vec<i64> = d.groups.iter().filter(|g| d.loops.iter().any(|l| l.grp == Some(g.id))).map(|g| g.id).collect();
    if alive.len() != d.groups.len() { d.groups.retain(|g| alive.contains(&g.id)); }
    if d.groups.is_empty() && d.relief.is_none() { return; }
    ui.separator();
    ui.heading(if d.groups.iter().all(|g| g.text.is_some()) { "Text" } else if d.groups.iter().all(|g| g.text.is_none()) { "Imported" } else { "Imported and text" });
    let (sw, sh) = (d.stock.w, d.stock.h);
    let zero = (match d.stock.zr.as_str() { "lr" | "ur" => sw, "c" => sw / 2.0, _ => 0.0 }, match d.stock.zr.as_str() { "ul" | "ur" => sh, "c" => sh / 2.0, _ => 0.0 });
    let groups = d.groups.clone();
    for g in groups {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width() - 4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&g.name).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if small_icon_button(ui, Icon::Trash, true, if g.text.is_some() { "Delete this text" } else { "Delete this drawing (all its shapes)" }).clicked() { *act = Some((Some(g.id), GroupAct::Delete)); }
                });
            });
            if g.text.is_some() {
                // a placed text: its place and size are in the selection bar, turning is the rotate handles; only what is not
                // available elsewhere is here
                let mut lock_now = g.locked;
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(8.0, 3.0);
                    if ui.add_enabled(!lock_now, egui::Button::new("Edit text")).on_hover_text("Change the words, font, size or place (double-click the text does the same)").clicked() { *act = Some((Some(g.id), GroupAct::EditText)); }
                    if ui.add_enabled(!lock_now, egui::Button::new("To outlines")).on_hover_text("The letters become ordinary shapes you can edit point by point").clicked() { *act = Some((Some(g.id), GroupAct::ToOutlines)); }
                    ui.checkbox(&mut lock_now, "Lock").on_hover_text("Stops this text being moved, resized, rotated or flipped by accident");
                });
                if lock_now != g.locked {
                    if let Some(x) = d.groups.iter_mut().find(|x| x.id == g.id) { x.locked = lock_now; *changed = true; }
                }
                return;
            }
            let Some(b) = shapes::bbox_many(d.loops.iter().filter(|l| l.grp == Some(g.id))) else { return };
            let mut fit = false;
            let mut lock_now = g.locked;
            if let Some(nb) = placement_fields(ui, egui::Id::new(("grp", g.id)), b, zero, false, &mut fit, Some(&mut lock_now)) {
                let new = [nb[0], nb[1], nb[0] + nb[2], nb[1] + nb[3]];
                let mut refs: Vec<&mut putiacnc_core::model::LoopData> = d.loops.iter_mut().filter(|l| l.grp == Some(g.id)).collect();
                shapes::fit_to_bbox(&mut refs, b, new);
                *changed = true;
            }
            if fit {
                let (w, h) = (b[2] - b[0], b[3] - b[1]);
                let k = (sw / w).min(sh / h).min(1.0);
                let (nw, nh) = (w * k, h * k);
                let new = [(sw - nw) / 2.0, (sh - nh) / 2.0, (sw + nw) / 2.0, (sh + nh) / 2.0];
                let mut refs: Vec<&mut putiacnc_core::model::LoopData> = d.loops.iter_mut().filter(|l| l.grp == Some(g.id)).collect();
                shapes::fit_to_bbox(&mut refs, b, new);
                *changed = true;
            }
            if lock_now != g.locked {
                if let Some(x) = d.groups.iter_mut().find(|x| x.id == g.id) { x.locked = lock_now; *changed = true; }
            }
            // turn and flip about the drawing's centre
            let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
            let mut turn = 0.0;
            let mut flip = (false, false);
            ui.add_space(4.0);
            ui.add_enabled_ui(!lock_now, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(8.0, 3.0);
                    if ui.button("↺ 90°").on_hover_text("Rotate 90° counter-clockwise").clicked() { turn = 90.0; }
                    if ui.button("↻ 90°").on_hover_text("Rotate 90° clockwise").clicked() { turn = -90.0; }
                    if ui.button("Flip H").on_hover_text("Mirror left to right").clicked() { flip.0 = true; }
                    if ui.button("Flip V").on_hover_text("Mirror top to bottom").clicked() { flip.1 = true; }
                });
            });
            if turn != 0.0 || flip.0 || flip.1 {
                for l in d.loops.iter_mut().filter(|l| l.grp == Some(g.id)) {
                    if turn != 0.0 { shapes::rotate_about(l, cx, cy, turn); }
                    if flip.0 { shapes::mirror_x(l, cx); }
                    if flip.1 { shapes::mirror_y(l, cy); }
                }
                *changed = true;
            }
        });
    }
    if let Some(rel) = d.relief.as_mut() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width() - 4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("3D model: {}", rel.name)).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if small_icon_button(ui, Icon::Trash, true, "Remove the 3D model (its relief operations stop working)").clicked() { *act = Some((None, GroupAct::Delete)); }
                });
            });
            let (w, h) = rel.size();
            let b = [rel.x, rel.y, rel.x + w, rel.y + h];
            let mut fit = false;
            if let Some(nb) = placement_fields(ui, egui::Id::new("relief_box"), b, zero, true, &mut fit, None) {
                rel.x = nb[0]; rel.y = nb[1];
                rel.scale = (nb[2] / (rel.nx as f64 * rel.res)).max(0.001);
                *changed = true;
            }
            ui.weak(format!("The model is {:.1} mm tall at this size; the 3D relief operation cuts that deep unless you set another depth.", rel.zmax * rel.scale));
            if fit {
                let k = (sw / (rel.nx as f64 * rel.res)).min(sh / (rel.ny as f64 * rel.res));
                if rel.scale > k { rel.scale = k; }
                let (w, h) = rel.size();
                rel.x = (sw - w) / 2.0; rel.y = (sh - h) / 2.0;
                *changed = true;
            }
        });
    }
}

pub fn left_panel(ui: &mut Ui, app: &mut App) { project_panel(ui, app, false); }
/// The Operations tab of the right panel.
pub fn ops_panel(ui: &mut Ui, app: &mut App) { project_panel(ui, app, true); }

/// The Inlay button: a pocket operation for the selected shapes and a plug operation for a mirrored copy
/// of them, placed to the right of the selection.
fn add_inlay(d: &mut putiacnc_core::model::ProjectData, sel: &std::collections::BTreeSet<usize>) {
    use putiacnc_core::shapes as sh;
    let is_cone = |t: &&putiacnc_core::model::Tool| ["vbit", "engrave", "taper"].contains(&t.shape.as_str());
    let vbit = d.tools.iter().find(|t| is_cone(t) && t.angle == 60.0).or_else(|| d.tools.iter().find(is_cone)).or(d.tools.first()).map(|t| t.id.clone()).unwrap_or_default();
    let flats: Vec<&putiacnc_core::model::Tool> = d.tools.iter().filter(|t| t.shape == "flat").collect();
    let flat = flats.iter().find(|t| t.dia <= 3.5).or(flats.first()).map(|t| t.id.clone());
    let mut base = sh::default_op_params(&vbit);
    base.kind = "inlay".into();
    base.depth = 3.0;
    base.tool2 = flat;
    let mut plug = base.clone();
    plug.kind = "plug".into();
    plug.fit = Some(0.05);
    let first = d.oid.max(d.ops.iter().map(|o| o.id).max().unwrap_or(0)) + 1;
    d.oid = first + 1;
    d.ops.push(Operation { id: first, p: base });
    d.ops.push(Operation { id: first + 1, p: plug });
    d.act = first;
    let picked: Vec<usize> = sel.iter().copied().filter(|&i| i < d.loops.len()).collect();
    for &i in &picked { d.loops[i].op = first; }
    if let Some(b) = sh::bbox_many(picked.iter().map(|&i| &d.loops[i])) {
        let (cx, w) = ((b[0] + b[2]) / 2.0, b[2] - b[0]);
        let copies: Vec<putiacnc_core::model::LoopData> = picked.iter().filter(|&&i| d.loops[i].closed()).map(|&i| {
            let mut l = d.loops[i].clone();
            sh::mirror_x(&mut l, cx);
            sh::translate(&mut l, w + 10.0, 0.0);
            l.op = first + 1;
            l
        }).collect();
        d.loops.extend(copies);
    }
}

/// One body for both panels: `ops` picks the Operations section (right panel) or the
/// Name / Stock / Selection / Tools sections (left panel), so editing and undo logic is shared.
fn project_panel(ui: &mut Ui, app: &mut App, ops: bool) {
    let dark = app.theme.dark;
    let before = app.project.data.clone();
    let mut changed = false;
    let mut name_changed = false;
    let mut open_tools = false;
    // machine picker data (read before the panel borrows the project)
    let mach_names: Vec<(String, String)> = app.machines.list.iter().map(|m| (m.id.clone(), m.name.clone())).collect();
    let (mach_cur, mach_name, mach_area) = match &app.project.data.machine {
        Some(m) => (m.id.clone(), m.name.clone(), format!("Work area {} × {} × {} mm", fmt_num(m.x), fmt_num(m.y), fmt_num(m.z))),
        None => (String::new(), String::new(), String::new()),
    };
    let mut mach_pick: Option<String> = None;
    let mut open_machines = false;
    let mut group_act: Option<(Option<i64>, GroupAct)> = None;
    // the buttons that add operations stay put above the list; only the list scrolls
    let mut add_kind: Option<&str> = None;
    // the eye only changes the 3D preview, so it is only there while that tab is open
    let in3d = app.view_tab == crate::view3d::ViewTab::Preview;
    if ops {
        let d = &app.project.data;
        let sel = &app.sel;
        let collapsed = &mut app.collapsed;
        // two rows: the plain operations with the collapse button at the right edge, then Inlay (it adds two operations)
        // an operation is made for the shapes that are selected, so the buttons wait for a selection
        let has_sel = sel.iter().any(|&k| k < d.loops.len());
        ui.horizontal(|ui| {
            for (kind, label, icon) in [("outside", "Profile", Icon::Profile), ("pocket", "Pocket", Icon::Pocket), ("vcarve", "V-carve", Icon::VCarve), ("drill", "Drill", Icon::Drill)] {
                let r = ui.add_enabled_ui(has_sel, |ui| icon_text_button(ui, icon, label, &format!("Add a {label} operation for the selected shapes"), None)).inner;
                if !has_sel { r.on_disabled_hover_text("Select the shapes first"); } else if r.clicked() { add_kind = Some(kind); }
            }
            if !d.ops.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let all_folded = d.ops.iter().all(|o| collapsed.contains(&o.id));
                    let (icon, tip) = if all_folded { (Icon::Unfold, "Expand all operations") } else { (Icon::Fold, "Collapse all operations") };
                    if small_icon_button(ui, icon, true, tip).clicked() {
                        if all_folded { collapsed.clear(); } else { *collapsed = d.ops.iter().map(|o| o.id).collect(); }
                    }
                });
            }
        });
        ui.horizontal(|ui| {
            let r = ui.add_enabled_ui(has_sel, |ui| icon_text_button(ui, Icon::Inlay, "Inlay", "Add an inlay: a pocket operation and a matching plug operation (a mirrored copy of the selection beside it)", None)).inner;
            if !has_sel { r.on_disabled_hover_text("Select the shapes first"); } else if r.clicked() { add_kind = Some("inlay"); }
            let has_model = d.relief.is_some();
            let r = ui.add_enabled_ui(has_model, |ui| icon_text_button(ui, Icon::Relief, "3D relief", "Carve the imported 3D model: roughing, then a finishing pass that follows the surface", None)).inner;
            if !has_model { r.on_disabled_hover_text("Import a 3D model first (File > Import 3D Model)"); } else if r.clicked() { add_kind = Some("relief"); }
        });
        ui.separator();
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let App { project, sel, collapsed, tab_edit, show_clamps, .. } = app;
        if !ops {
        ui.horizontal(|ui| {
            ui.heading("Project");
            let w = (ui.available_width() - 11.0).max(80.0);
            name_changed = ui.add(egui::TextEdit::singleline(&mut project.name).font(egui::TextStyle::Heading).desired_width(w)).changed();
        });
        }
        if !ops {
        // ---- machine -----------------------------------------------------------------
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Machine");
            let mw = (ui.available_width() - 6.0).max(80.0);
            egui::ComboBox::from_id_source("project_machine").width(mw).selected_text(elide(ui, &mach_name, mw - 30.0)).show_ui(ui, |ui| {
                for (id, name) in &mach_names {
                    if ui.selectable_label(*id == mach_cur, name).clicked() && *id != mach_cur { mach_pick = Some(id.clone()); }
                }
                ui.separator();
                if ui.selectable_label(false, "Manage machines…").on_hover_text("Create and edit machines").clicked() { open_machines = true; ui.close_menu(); }
            });
        });
        ui.weak(&mach_area);
        }
        let d = &mut project.data;

        if !ops {
        // ---- stock -------------------------------------------------------------------
        ui.separator();
        ui.heading("Stock");
        let sp = ui.spacing().item_spacing.x;
        let font = egui::TextStyle::Body.resolve(ui.style());
        let text_w = |ui: &Ui, s: &str| ui.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x);
        let stock_w = ui.available_width();
        let _ = (&text_w, sp);
        let stock_frame = egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
            ui.set_max_width(stock_w - 34.0);
            let fw = 62.0_f32;
            let l1 = tw(ui, "Thickness").max(tw(ui, "XY zero")).max(tw(ui, "Appearance"));
            let l3 = tw(ui, "Height").max(tw(ui, "Safe Z"));
            let mut col4_x = 0.0_f32;
            egui::Grid::new("stock_grid").num_columns(6).spacing([8.0, 10.0]).show(ui, |ui| {
                rlabel(ui, l1, "Width");
                changed |= dv_w(ui, fw, &mut d.stock.w, 1.0, 10000.0, ""); ui.weak("mm");
                col4_x = ui.next_widget_position().x;
                rlabel(ui, l3, "Height");
                changed |= dv_w(ui, fw, &mut d.stock.h, 1.0, 10000.0, ""); ui.weak("mm"); ui.end_row();
                rlabel(ui, l1, "Thickness");
                changed |= dv_w(ui, fw, &mut d.stock.t, 0.1, 1000.0, ""); ui.weak("mm");
                ui.end_row();
            });
            ui.add_space(12.0);
            ui.horizontal_top(|ui| {
                rlabel(ui, l1, "XY zero");
                changed |= xy_picker(ui, &mut d.stock.zr);
                // the Z block lines up with the Height / Safe Z column above
                let gap = col4_x - ui.next_widget_position().x - 3.0 * ui.spacing().item_spacing.x;
                ui.add_space(gap.max(6.0));
                egui::Grid::new("stock_z").num_columns(2).spacing([8.0, 10.0]).show(ui, |ui| {
                    let lz = l3;
                    rlabel(ui, lz, "Z zero");
                    let (c, _) = combo_w2(ui, 96.0, "zm", &mut d.stock.zm, ZMODES, "", "Where Z0 is measured from");
                    changed |= c; ui.end_row();
                    if d.stock.zm != "top" {
                        rlabel(ui, lz, "Z0");
                        ui.horizontal(|ui| {
                            changed |= dv_w(ui, fw, &mut d.stock.zh, 0.0, 1000.0, "");
                            ui.weak("mm");
                        });
                        ui.end_row();
                    }
                });
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                rlabel(ui, l1, "Appearance");
                if !MATERIALS.iter().any(|(k, _)| *k == d.stock.mat) { d.stock.mat = "wood".into(); }
                let (c, _) = combo_w2(ui, 96.0, "stock_mat", &mut d.stock.mat, MATERIALS, "", "How the stock looks in the drawing and the 3D preview");
                changed |= c;
            });
        });

        // ---- clamps ------------------------------------------------------------------
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            // the small text sits on the heading's baseline instead of floating at its middle
            let hg = ui.fonts(|f| f.row_height(&egui::TextStyle::Heading.resolve(ui.style())));
            ui.heading("Clamps");
            let g = ui.painter().layout_no_wrap("keep-out zones".to_string(), egui::TextStyle::Body.resolve(ui.style()), ui.visuals().weak_text_color());
            let (r, resp) = ui.allocate_exact_size(egui::vec2(g.size().x, hg), egui::Sense::hover());
            ui.painter().galley(egui::pos2(r.min.x, r.max.y - g.size().y), g, ui.visuals().weak_text_color());
            resp.on_hover_text("Rectangles on the table that the tool must not touch. X and Y are measured from the stock's lower-left corner; Width is the size along X, Length along Y, and Rise is how far the clamp stands above the stock top. The toolpath warnings check them");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tip = if *show_clamps { "Clamps are drawn in the drawing and the 3D preview. Click to hide them (the toolpath warnings still check them)" } else { "Clamps are hidden in the drawing and the 3D preview. Click to show them" };
                if small_icon_button(ui, if *show_clamps { Icon::Eye } else { Icon::EyeOff }, true, tip).clicked() { *show_clamps = !*show_clamps; }
            });
        });
        let clamp_inner_w = stock_frame.response.rect.width() - 22.0;
        egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
            ui.set_max_width(stock_w - 34.0);
            ui.set_min_width(clamp_inner_w);
            let mut del = None;
            if d.clamps.is_empty() { ui.weak("None. Add one for each clamp or screw on the table."); }
            let n_clamps = d.clamps.len();
            let fw = 62.0_f32;
            let (lc1, lc2) = (tw(ui, "Width").max(tw(ui, "Rise")).max(tw(ui, "X")), tw(ui, "Length").max(tw(ui, "Y")));
            // which clamps are folded away (a view setting: kept for the session, not in the project)
            let fold_key = egui::Id::new("clamp_folded");
            let mut folded: Vec<bool> = ui.data(|x| x.get_temp(fold_key)).unwrap_or_default();
            folded.resize(n_clamps, false);
            for (i, c) in d.clamps.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    if small_icon_button(ui, if folded[i] { Icon::ChevR } else { Icon::ChevD }, true, if folded[i] { "Show this clamp's settings" } else { "Hide this clamp's settings" }).clicked() { folded[i] = !folded[i]; }
                    ui.label(egui::RichText::new(format!("Clamp {}", i + 1)).strong());
                    if folded[i] { ui.weak(format!("X {} · Y {} · {} × {} · rise {}", fnum(c.x), fnum(c.y), fnum(c.w), fnum(c.h), fnum(c.height))); }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if small_icon_button(ui, Icon::Trash, true, "Remove this clamp").clicked() { del = Some(i); }
                    });
                });
                if !folded[i] {
                egui::Grid::new(("clamp_grid", i)).num_columns(6).spacing([8.0, 8.0]).show(ui, |ui| {
                    rlabel(ui, lc1, "X"); changed |= dv_w(ui, fw, &mut c.x, -10000.0, 10000.0, ""); ui.weak("mm");
                    rlabel(ui, lc2, "Y"); changed |= dv_w(ui, fw, &mut c.y, -10000.0, 10000.0, ""); ui.weak("mm"); ui.end_row();
                    rlabel(ui, lc1, "Width"); changed |= dv_w(ui, fw, &mut c.w, 0.1, 10000.0, ""); ui.weak("mm");
                    rlabel(ui, lc2, "Length"); changed |= dv_w(ui, fw, &mut c.h, 0.1, 10000.0, ""); ui.weak("mm"); ui.end_row();
                    rlabel(ui, lc1, "Rise"); changed |= dv_w(ui, fw, &mut c.height, 0.1, 10000.0, ""); ui.weak("mm");
                    ui.end_row();
                });
                }
                if i + 1 < n_clamps { ui.add_space(4.0); ui.separator(); ui.add_space(4.0); }
            }
            if let Some(i) = del { if i < folded.len() { folded.remove(i); } }
            ui.data_mut(|x| x.insert_temp(fold_key, folded));
            ui.add_space(if n_clamps > 0 { 8.0 } else { 4.0 });
            if let Some(i) = del { d.clamps.remove(i); changed = true; }
            ui.horizontal(|ui| {
                if icon_text_button(ui, Icon::Add, "Add clamp", "A clamp on the left edge, 20 mm tall; edit X, Y, width (X size), length (Y size) and rise (how far it stands above the stock top), all in mm", None).clicked() {
                    d.clamps.push(putiacnc_core::model::Clamp { x: -30.0, y: d.stock.h / 2.0 - 15.0, w: 30.0, h: 30.0, height: 20.0 });
                    changed = true;
                }
            });
        });

        }
        if ops {
        // ---- operations --------------------------------------------------------------
        // the operation list, its buttons row and the editor frame all share this width
        let lw = ui.available_width();
        if let Some(kind) = add_kind {
            if kind == "inlay" {
                add_inlay(d, sel);
                changed = true;
            } else {
                let base = d.ops.last().map(|o| o.p.clone());
                let mut p = base.unwrap_or_else(|| shapes::default_op_params(d.tools.first().map(|t| t.id.as_str()).unwrap_or("")));
                p.kind = match kind {
                    "relief" => "relief".into(),
                    "pocket" => "pocket".into(),
                    "vcarve" => "vcarve".into(),
                    "drill" => "drill".into(),
                    _ => if ["outside", "inside", "online"].contains(&p.kind.as_str()) { p.kind.clone() } else { "outside".into() },
                };
                if kind == "relief" {
                    // a ball nose to finish, a flat end mill to rough, and the model's height as the depth
                    if let Some(t) = d.tools.iter().find(|t| t.shape == "ball") { p.tool = t.id.clone(); }
                    p.tool2 = d.tools.iter().filter(|t| t.shape == "flat").min_by(|a, b| (a.dia - 6.0).abs().partial_cmp(&(b.dia - 6.0).abs()).unwrap_or(std::cmp::Ordering::Equal)).map(|t| t.id.clone());
                    p.pass_depth = Some(2.0);
                    p.tabs = false; p.lead = None;
                }
                if kind == "vcarve" {
                    if let Some(t) = d.tools.iter().find(|t| ["vbit", "engrave", "taper"].contains(&t.shape.as_str())) { p.tool = t.id.clone(); }
                }
                d.oid = d.oid.max(d.ops.iter().map(|o| o.id).max().unwrap_or(0)) + 1;
                let id = d.oid;
                d.ops.push(Operation { id, p });
                if kind != "relief" {
                    // shapes that were in another operation move to this one, and the status bar says so
                    let from: Vec<i64> = sel.iter().filter_map(|&i| d.loops.get(i)).map(|l| l.op).filter(|&o| o != 0 && o != id).collect();
                    if !from.is_empty() {
                        let name = |oid: i64| d.ops.iter().position(|o| o.id == oid).map(|k| format!("{}. {}", k + 1, label_of(KINDS, &d.ops[k].p.kind)));
                        let first = from[0];
                        let same = from.iter().all(|&o| o == first);
                        let from_txt = if same { name(first).unwrap_or_else(|| "another operation".into()) } else { "other operations".into() };
                        let msg = format!("{} shape{} moved from {}", from.len(), if from.len() == 1 { "" } else { "s" }, from_txt);
                        let now = ui.input(|i| i.time);
                        ui.ctx().data_mut(|x| x.insert_temp(egui::Id::new("status_note"), (msg, now)));
                    }
                    for &i in sel.iter() { if let Some(l) = d.loops.get_mut(i) { l.op = id; } }
                }
                d.act = id;
                changed = true;
            }
        }
        let tools = d.tools.clone();
        // the tools the operations use, in order of first use: the tool pickers offer them first
        let mut job_tools: Vec<String> = Vec::new();
        for o in &d.ops { for t in std::iter::once(&o.p.tool).chain(o.p.tool2.iter()) { if !t.is_empty() && !job_tools.contains(t) { job_tools.push(t.clone()); } } }
        let machine_now = d.machine.clone();
        let mut act_remove: Option<usize> = None;
        let mut act_move: Option<(usize, i32)> = None;
        // the eye: Some((id, only)) toggles that operation in the 3D preview; with Alt it is shown alone
        let mut act_eye: Option<(i64, bool)> = None;
        let hidden_now = d.hidden_ops.clone();
        // operations that own a selected shape get an outline; the list scrolls to the first one when the selection changes
        let hi_ids: Vec<i64> = d.ops.iter().filter(|o| sel.iter().any(|&k| d.loops.get(k).is_some_and(|l| l.op == o.id))).map(|o| o.id).collect();
        let hi_key = egui::Id::new("op_highlight_prev");
        let hi_prev: Vec<i64> = ui.data(|x| x.get_temp(hi_key)).unwrap_or_default();
        let mut hi_scroll = hi_ids != hi_prev && !hi_ids.is_empty();
        ui.data_mut(|x| x.insert_temp(hi_key, hi_ids.clone()));
        for i in 0..d.ops.len() {
            let o = &d.ops[i];
            let n = d.loops.iter().filter(|l| l.op == o.id).count();
            let tool = d.tools.iter().find(|t| t.id == o.p.tool).map(|t| clean(&t.name)).unwrap_or_else(|| "no tool".into());
            let text = format!("{}. {}", i + 1, label_of(KINDS, &o.p.kind));
            let sub = if o.p.kind == "relief" { tool.clone() } else { format!("{} · {} shape{}", tool, n, if n == 1 { "" } else { "s" }) };
            let id = o.id;
            let o_kind = o.p.kind.clone();
            let n_ops = d.ops.len();
            {
                let kind_icon = match o_kind.as_str() { "pocket" => Icon::Pocket, "vcarve" => Icon::VCarve, "drill" => Icon::Drill, "inlay" | "plug" => Icon::Inlay, _ => Icon::Profile };
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(lw, 40.0), egui::Sense::click());
                let selected = hi_ids.contains(&id);
                let resp = resp.on_hover_text("Select this operation's shapes in the drawing");
                let folded = collapsed.contains(&id);
                let base = crate::theme::op_color(i, dark);
                let fill = if selected { base } else if resp.hovered() { base.gamma_multiply(0.82) } else { base.gamma_multiply(0.66) };
                let fill = if in3d && hidden_now.contains(&id) { fill.gamma_multiply(0.55) } else { fill };
                ui.painter().rect_filled(rect, 6.0, fill);
                // the operations that own the shapes selected in the drawing are brighter and outlined
                if selected {
                    ui.painter().rect_stroke(rect.expand(1.5), 7.0, egui::Stroke::new(2.5_f32, ui.visuals().text_color()));
                    if hi_scroll { ui.scroll_to_rect(rect.expand(8.0), Some(egui::Align::Center)); hi_scroll = false; }
                }
                // white text on dark colours, near-black on light ones
                let lum = 0.299 * base.r() as f32 + 0.587 * base.g() as f32 + 0.114 * base.b() as f32;
                let ink = if lum > 150.0 { Color32::from_rgb(0x1e, 0x1e, 0x1e) } else { Color32::WHITE };
                let col = ink;
                let sub_col = ink.gamma_multiply(0.8);
                let body = egui::TextStyle::Body.resolve(ui.style());
                let small = egui::TextStyle::Small.resolve(ui.style());
                let chev_rect = egui::Rect::from_min_size(rect.min, egui::vec2(30.0, rect.height()));
                let chev = ui.interact(chev_rect, ui.id().with(("op_fold", id)), egui::Sense::click()).on_hover_text(if folded { "Show the settings" } else { "Hide the settings" });
                if chev.hovered() { ui.painter().rect_filled(chev_rect.shrink2(egui::vec2(3.0, 8.0)), 5.0, ink.gamma_multiply(0.18)); }
                paint_icon(ui.painter(), if folded { Icon::ChevR } else { Icon::ChevD }, egui::pos2(rect.min.x + 15.0, rect.center().y), ink, 0.62);
                paint_icon(ui.painter(), kind_icon, egui::pos2(rect.min.x + 42.0, rect.center().y), ink, 0.6);
                let g1 = ui.painter().layout_no_wrap(text, body, col);
                let g2 = ui.painter().layout_no_wrap(sub, small, sub_col);
                let top = rect.center().y - (g1.size().y + g2.size().y) / 2.0;
                let btn_w = if in3d { 4.0 * 26.0 + 3.0 * 2.0 } else { 3.0 * 26.0 + 2.0 * 2.0 };
                let clip = ui.painter().with_clip_rect(egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x - btn_w - 8.0, rect.max.y)));
                let h1 = g1.size().y;
                clip.galley(egui::pos2(rect.min.x + 60.0, top), g1, col);
                clip.galley(egui::pos2(rect.min.x + 60.0, top + h1), g2, sub_col);
                if chev.clicked() { if folded { collapsed.remove(&id); } else { collapsed.insert(id); } }
                else if resp.clicked() { *sel = d.loops.iter().enumerate().filter(|(_, l)| l.op == id).map(|(k, _)| k).collect(); }
                // up / down / delete sit inside the row, on its right
                let brect = egui::Rect::from_min_size(egui::pos2(rect.max.x - btn_w - 6.0, rect.center().y - 13.0), egui::vec2(btn_w, 26.0));
                ui.allocate_ui_at_rect(brect, |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.horizontal(|ui| {
                        let off = hidden_now.contains(&id);
                        if in3d {
                        let tip = if off { "Hidden in the 3D preview. Click to show it (Alt-click: show only this one)" } else { "Shown in the 3D preview. Click to hide it (Alt-click: show only this one)" };
                        if small_icon_button(ui, if off { Icon::EyeOff } else { Icon::Eye }, true, tip).clicked() { act_eye = Some((id, ui.input(|x| x.modifiers.alt))); }
                        }
                        if small_icon_button(ui, Icon::Up, i > 0, "Move up (runs earlier)").clicked() { act_move = Some((i, -1)); }
                        if small_icon_button(ui, Icon::Down, i + 1 < n_ops, "Move down (runs later)").clicked() { act_move = Some((i, 1)); }
                        if small_icon_button(ui, Icon::Trash, true, "Delete operation").clicked() { act_remove = Some(i); }
                    });
                });
            }
            ui.add_space(2.0);
            if !collapsed.contains(&id) {
            let loops = &d.loops;
            let stock_t = d.stock.t;
            // a relief follows the model's height until a depth is typed in
            if d.ops[i].p.kind == "relief" && !d.ops[i].p.d_man {
                let eff = putiacnc_core::toolpath::relief_depth(d, &d.ops[i].p);
                if (d.ops[i].p.depth - eff).abs() > 1e-9 { d.ops[i].p.depth = eff; changed = true; }
            }
            let model_h = d.relief.as_ref().map(|r| r.zmax * r.scale);
            let o = &mut d.ops[i];
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(lw - 14.0);
                let p = &mut o.p;
                let sp = ui.spacing().item_spacing.x;
                let bf = egui::TextStyle::Body.resolve(ui.style());
                let tw_of = |ui: &Ui, s: &str| ui.fonts(|f| f.layout_no_wrap(s.to_string(), bf.clone(), Color32::WHITE).size().x);
                // 2 x 2 grid: Type | Cut  /  Tool | Depth
                let l1 = tw_of(ui, "Type").max(tw_of(ui, "Tool")).max(tw_of(ui, "Depth")).max(tw_of(ui, "Rough")).max(tw_of(ui, "Finish"));
                let l3 = tw_of(ui, "Cut").max(tw_of(ui, "Pass"));
                // fixed widths: the right column never depends on the selected text, so nothing resizes
                let cw = tw_of(ui, "Conventional") + 44.0;
                let lc = (ui.available_width() - l1 - l3 - cw - 3.0 * sp - 12.0).max(100.0);
                egui::Grid::new(("op_grid", id)).num_columns(4).spacing([sp, 10.0]).show(ui, |ui| {
                    rlabel(ui, l1, "Type");
                    let profile = PROFILES.iter().any(|(k, _)| *k == p.kind);
                    let single: [(&str, &str); 1] = [match p.kind.as_str() { "vcarve" => ("vcarve", "V-carve"), "drill" => ("drill", "Drill"), "inlay" => ("inlay", "Inlay pocket"), "plug" => ("plug", "Inlay plug"), "relief" => ("relief", "3D relief"), _ => ("pocket", "Pocket") }];
                    changed |= combo_w(ui, lc + 10.0, &format!("kind{id}"), &mut p.kind, if profile { PROFILES } else { &single }, "", if profile { "Profile: cut outside, inside or on the line" } else { "Operation type" });
                    rlabel(ui, l3, "Cut");
                    ui.add_enabled_ui(p.kind != "drill" && p.kind != "relief", |ui| {
                        let (c, _) = combo_w2(ui, cw + 10.0, &format!("dir{id}"), &mut p.dir, DIRS, "", "Cut direction: climb or conventional");
                        changed |= c;
                    });
                    ui.end_row();
                    ui.end_row();
                });
                ui.add_space(4.0);
                if p.kind == "relief" {
                    ui.add_space(4.0);
                    // total depth, shared by both steps
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = sp;
                        rlabel(ui, l1, "Depth");
                        if dv_w(ui, 48.0, &mut p.depth, 0.01, 1000.0, "") { p.d_man = true; changed = true; }
                        ui.weak("mm");
                        if p.d_man {
                            if reset_button(ui, "Follow the model's height again") { p.d_man = false; changed = true; }
                            ui.weak("typed in");
                        } else {
                            match model_h {
                                Some(h) if h > stock_t + 1e-6 => { ui.weak(format!("model is {:.1} mm: capped by stock", h)); }
                                _ => { ui.weak("the model's height"); }
                            }
                        }
                    });
                    let tools_ref: &[putiacnc_core::model::Tool] = &tools;
                    // one pick list for both steps; `flat_only` limits it to flat end mills
                    let pick = |ui: &mut Ui, src: (&str, i64), w: f32, cur: &str, flat_only: bool, tip: &str| -> Option<String> {
                        let name = tools_ref.iter().find(|t| t.id == cur).map(|t| clean(&t.name)).unwrap_or_else(|| "none".into());
                        let name = elide(ui, &name, w - 30.0);
                        let mut got = None;
                        let mut keep = false;
                        let combo = egui::ComboBox::from_id_source(src).width(w).height(420.0).selected_text(name).show_ui(ui, |ui| {
                            for (sys, label) in [("mm", "Metric"), ("in", "Imperial")] {
                                if !tools_ref.iter().any(|t| t.sys == sys && (!flat_only || t.shape == "flat")) { continue; }
                                ui.weak(label);
                                for t in tools_ref.iter().filter(|t| t.sys == sys && (!flat_only || t.shape == "flat")) {
                                    let dia = putiacnc_core::toollib::fmt_dia(t.dia, &t.sys, true);
                                    if ui.selectable_label(cur == t.id, format!("{}  ({})", clean(&t.name), dia)).clicked() { got = Some(t.id.clone()); }
                                }
                            }
                            if got.is_some() { ui.close_menu(); } else if crate::widgets::click_inside_popup(ui) { keep = true; } // a click on "Metric" / "Imperial" leaves it open
                        });
                        if keep { crate::widgets::reopen_combo(ui, src); }
                        combo.response.on_hover_text(tip);
                        got
                    };
                    let lw2 = tw_of(ui, "Stepover").max(tw_of(ui, "Pass")).max(tw_of(ui, "Leave")).max(tw_of(ui, "Tool"));
                    let tool_w = (ui.available_width() - lw2 - sp - 28.0).max(100.0);
                    // ---- roughing
                    ui.add_space(4.0);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let mut on = !p.r_off;
                        if ui.checkbox(&mut on, egui::RichText::new("Roughing").strong()).on_hover_text("Removes the bulk with a flat end mill, leaving a little for the finishing step").changed() { p.r_off = !on; changed = true; }
                        if on {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = sp;
                                rlabel(ui, lw2, "Tool");
                                let cur = p.tool2.clone().unwrap_or_default();
                                if let Some(id) = pick(ui, ("optool2", id), tool_w, &cur, true, "Flat end mill that removes the bulk (leave empty to skip)") { p.tool2 = Some(id); changed = true; }
                            });
                            let tool_pd = p.tool2.as_deref().and_then(|id| tool_pd_of(&tools, id));
                            let tool_so = p.tool2.as_deref().and_then(|id| tools_ref.iter().find(|t| t.id == id)).map(|t| t.so);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = sp;
                                rlabel(ui, lw2, "Pass");
                                let mut v = p.pass_depth.or(tool_pd).unwrap_or(1.0);
                                if dv_w(ui, 48.0, &mut v, 0.01, 1000.0, "") { p.pass_depth = Some(v); changed = true; }
                                ui.weak("mm");
                                let n = ((p.depth / p.pass_depth.or(tool_pd).unwrap_or(1.0).max(0.001)) - 1e-9).ceil().max(1.0) as usize;
                                ui.weak(format!("{n} layer{}", if n == 1 { "" } else { "s" }));
                                if p.pass_depth.is_some() && reset_button(ui, "Use the tool's pass depth") { p.pass_depth = None; changed = true; }
                            });
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = sp;
                                rlabel(ui, lw2, "Stepover");
                                let mut v = p.r_so.or(tool_so).unwrap_or(40.0);
                                if dv_w(ui, 48.0, &mut v, 1.0, 100.0, "") { p.r_so = Some(v); changed = true; }
                                ui.weak("%");
                                if p.r_so.is_some() && reset_button(ui, "Use the tool's stepover") { p.r_so = None; changed = true; }
                            });
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = sp;
                                rlabel(ui, lw2, "Leave");
                                let mut v = p.r_leave.unwrap_or(0.3);
                                if dv_w(ui, 48.0, &mut v, 0.0, 5.0, "") { p.r_leave = Some(v); changed = true; }
                                ui.weak("mm");
                                ui.weak("left for the finishing step");
                            });
                        }
                    });
                    // ---- finishing
                    ui.add_space(4.0);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let mut on = !p.f_off;
                        if ui.checkbox(&mut on, egui::RichText::new("Finishing").strong()).on_hover_text("Follows the surface line by line, usually with a ball nose").changed() { p.f_off = !on; changed = true; }
                        if on {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = sp;
                                rlabel(ui, lw2, "Tool");
                                let cur = p.tool.clone();
                                if let Some(id) = pick(ui, ("optool", id), tool_w, &cur, false, "Tool that carves the final surface (a ball nose works best)") { p.tool = id; changed = true; }
                            });
                            let tool_so = tools_ref.iter().find(|t| t.id == p.tool).map(|t| t.so);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = sp;
                                rlabel(ui, lw2, "Stepover");
                                let mut v = p.f_so.or(tool_so).unwrap_or(10.0);
                                if dv_w(ui, 48.0, &mut v, 1.0, 100.0, "") { p.f_so = Some(v); changed = true; }
                                ui.weak("%");
                                ui.weak("of the tool diameter");
                                if p.f_so.is_some() && reset_button(ui, "Use the tool's stepover") { p.f_so = None; changed = true; }
                            });
                        }
                    });
                    if p.r_off && p.f_off { ui.add_space(2.0); ui.colored_label(Color32::from_rgb(220, 120, 0), "Both steps are off: nothing will be cut."); }
                } else {
                    // Tool row: the selector takes all the room left of the right-aligned "Ramp in"
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = sp;
                        let has_ramp = p.kind != "vcarve" && p.kind != "drill" && p.kind != "relief";
                        // the room for "Ramp in" is kept even when it is not shown, so the selector is the same width in every operation
                        let ramp_w = tw_of(ui, "Ramp in") + 30.0;
                        let tool_w = (ui.available_width() - l1 - ramp_w - 3.0 * sp - 8.0).max(100.0);
                        rlabel(ui, l1, if p.kind == "relief" { "Finish" } else { "Tool" });
                        let cur = tools.iter().find(|t| t.id == p.tool).map(|t| clean(&t.name)).unwrap_or_else(|| "none".into());
                        let cur = elide(ui, &cur, tool_w - 30.0);
                        let (_, pk) = crate::toolpick::picker(ui, ("optool", id), tool_w, cur, Some(p.tool.as_str()), &job_tools, &tools, |_| true);
                        match pk { Some(crate::toolpick::Picked::Tool(t)) => { p.tool = t; changed = true; } Some(crate::toolpick::Picked::Manage) => open_tools = true, None => {} }
                        if has_ramp {
                            let rest = ui.available_width() - ramp_w;
                            if rest > 0.0 { ui.add_space(rest); }
                            changed |= ui.add_enabled(p.lead.is_none() || !["outside", "inside"].contains(&p.kind.as_str()), egui::Checkbox::new(&mut p.ramp, "Ramp in")).on_hover_text("Eases the tool down along the first pass (not needed with a lead-in arc)").changed();
                        }
                    });
                    if p.kind == "inlay" || p.kind == "plug" || p.kind == "relief" {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = sp;
                            let tool_w = (ui.available_width() - l1 - sp - 8.0).max(100.0);
                            rlabel(ui, l1, if p.kind == "relief" { "Rough" } else { "Flat" });
                            let cur = p.tool2.as_ref().and_then(|id| tools.iter().find(|t| &t.id == id)).map(|t| clean(&t.name)).unwrap_or_else(|| "none".into());
                            let cur = elide(ui, &cur, tool_w - 30.0);
                            let (r2, pk2) = crate::toolpick::picker(ui, ("optool2", id), tool_w, cur, p.tool2.as_deref(), &job_tools, &tools, |t| t.shape == "flat");
                            match pk2 { Some(crate::toolpick::Picked::Tool(t)) => { p.tool2 = Some(t); changed = true; } Some(crate::toolpick::Picked::Manage) => open_tools = true, None => {} }
                            r2.on_hover_text(match p.kind.as_str() { "inlay" => "Flat end mill that clears the pocket floor", "relief" => "Flat end mill that removes the bulk before the finishing pass (leave empty to finish only)", _ => "Flat end mill that cuts the plug out of the stock" });
                        });
                        if p.kind == "plug" {
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = sp;
                                rlabel(ui, l1, "Gap");
                                let mut g = p.fit.unwrap_or(0.05);
                                if dv_w(ui, 42.0, &mut g, 0.0, 2.0, "") { p.fit = Some(g); changed = true; }
                                ui.weak("mm").on_hover_text("How much smaller the plug is than the pocket, sideways. 0 is a press fit; add a little for glue.");
                                ui.weak("smaller than the pocket, for glue");
                            });
                        }
                    }
                    // depth, per-operation pass depth (no override = the tool's own value) and the resulting pass count
                    let inlay = p.kind == "inlay" || p.kind == "plug" || p.kind == "relief";
                    // an inlay clears and cuts out with its second tool, so the pass depth is that tool's
                    let tool_pd = if inlay { p.tool2.as_deref().and_then(|id| tool_pd_of(&tools, id)).or_else(|| tool_pd_of(&tools, &p.tool)) } else { tool_pd_of(&tools, &p.tool) };
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = sp;
                        // the row fills the panel: both fields share what the fixed parts leave over
                        let (w_mm, w_pass, w_cnt, w_rst) = (tw_of(ui, "mm"), tw_of(ui, "Pass"), tw_of(ui, "00 passes"), 28.0);
                        // short fields; whatever width is left is shared out as gaps between the three groups
                        let fw = 42.0;
                        let fixed = l1 + w_pass + 2.0 * w_mm + 2.0 * fw + w_cnt + w_rst + 7.0 * sp;
                        let gap = ((ui.available_width() - fixed) / 2.0).max(6.0);
                        rlabel(ui, l1, "Depth");
                        changed |= dv_w(ui, fw, &mut p.depth, 0.01, 1000.0, "");
                        ui.weak("mm");
                        ui.add_space(gap);
                        ui.label(if p.kind == "drill" { "Peck" } else { "Pass" });
                        let mut v = p.pass_depth.or(tool_pd).unwrap_or(1.0);
                        if dv_w(ui, fw, &mut v, 0.01, 1000.0, "") {
                            p.pass_depth = if tool_pd.is_some_and(|t| (t - v).abs() < 1e-9) { None } else { Some(v) };
                            changed = true;
                        }
                        ui.weak("mm");
                        ui.add_space(gap - sp.min(gap));
                        ui.allocate_ui_with_layout(egui::vec2(w_rst, ui.spacing().interact_size.y), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            if p.pass_depth.is_some() && reset_button(ui, "Use the tool's pass depth") { p.pass_depth = None; changed = true; }
                        });
                        let pd = p.pass_depth.or(tool_pd).unwrap_or(1.0).max(0.001);
                        let n = ((p.depth / pd) - 1e-9).ceil().max(1.0) as usize;
                        ui.allocate_ui_with_layout(egui::vec2(w_cnt, ui.spacing().interact_size.y), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.weak(if p.kind == "drill" { format!("{n} peck{}", if n == 1 { "" } else { "s" }) } else { format!("{n} pass{}", if n == 1 { "" } else { "es" }) });
                        });
                    });
                }
                if p.depth > stock_t + 1e-6 {
                    ui.add_space(2.0);
                    let col = if ui.visuals().dark_mode { Color32::from_rgb(255, 176, 64) } else { Color32::from_rgb(190, 100, 0) };
                    crate::checkui::warn_line(ui, col, &format!("Depth is more than the stock thickness ({} mm): the cutter would go through into the spoilboard.", stock_t));
                }
                ui.add_space(2.0);
                if ["outside", "inside", "online"].contains(&p.kind.as_str()) {
                    changed |= ui.checkbox(&mut p.tabs, "Holding tabs").changed();
                    if p.tabs {
                      ui.indent(("tab_opts", id), |ui| {
                        let manual = p.tab_pts.is_some();
                        ui.horizontal(|ui| {
                            ui.label("Placement").on_hover_text("Automatic spaces the tabs evenly; manual lets you put each one where you want it");
                            if ui.selectable_label(!manual, "Automatic").clicked() && manual {
                                p.tab_pts = None;
                                if *tab_edit == Some(id) { *tab_edit = None; }
                                changed = true;
                            }
                            if ui.selectable_label(manual, "Manual").on_hover_text("Starts from the evenly spaced tabs, which you can then move, add or remove").clicked() && !manual {
                                p.tab_pts = Some(shapes::auto_tabs(loops, id, p.tab_n.unwrap_or(4.0).round().max(1.0) as usize));
                                changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            let (mut n, mut w, mut h) = (p.tab_n.unwrap_or(4.0), p.tab_w.unwrap_or(5.0), p.tab_h.unwrap_or(2.0));
                            // short fields; the spare width is shared out as gaps between the groups
                            let sp = 5.0;
                            ui.spacing_mut().item_spacing.x = sp;
                            let fw = 42.0;
                            let groups = if manual { 2.0 } else { 3.0 };
                            let labels = tw_of(ui, "Width") + tw_of(ui, "Height") + if manual { 0.0 } else { tw_of(ui, "Count") };
                            let fixed = labels + (groups - if manual { 0.0 } else { 1.0 }) * fw + 30.0 * if manual { 0.0 } else { 1.0 } + 17.0 * if manual { 0.0 } else { 1.0 } + 2.0 * tw_of(ui, "mm") + (3.0 * groups + 0.0) * sp;
                            // with all three fields the spare width is spread between them; with two they sit together
                            let gap = if manual { 24.0 } else { ((ui.available_width() - fixed) / (groups - 1.0)).max(4.0) };
                            if !manual {
                                ui.label("Count").on_hover_text("Number of holding tabs, spread evenly along the outline");
                                if dv_w(ui, 30.0, &mut n, 1.0, 50.0, "") { p.tab_n = Some(n.round()); changed = true; }
                                ui.spacing_mut().item_spacing.x = 1.0;
                                if spin_arrows(ui, &mut n, 1.0, 50.0) { p.tab_n = Some(n.round()); changed = true; }
                                ui.spacing_mut().item_spacing.x = sp;
                                ui.add_space(gap);
                            }
                            ui.label("Width").on_hover_text("Length of each tab along the outline");
                            if dv_w(ui, fw, &mut w, 0.5, 100.0, "") { p.tab_w = Some(w); changed = true; }
                            ui.weak("mm");
                            ui.add_space(gap);
                            ui.label("Height").on_hover_text("How tall each tab is, measured up from the bottom of the cut");
                            if dv_w(ui, fw, &mut h, 0.2, 100.0, "") { p.tab_h = Some(h); changed = true; }
                            ui.weak("mm");
                        });
                        if let Some(tp) = p.tab_pts.as_mut() {
                            ui.horizontal(|ui| {
                                let editing = *tab_edit == Some(id);
                                let (label, tip) = if editing { ("Done", "Stop placing tabs (Esc)") } else { ("Place tabs", "Click on the outline in the drawing to add tabs; drag to move them, right-click to remove") };
                                if icon_text_button(ui, if editing { Icon::Close } else { Icon::Add }, label, tip, None).clicked() {
                                    *tab_edit = if editing { None } else { Some(id) };
                                    d.act = id;
                                }
                                ui.weak(format!("{} tab{}", tp.len(), if tp.len() == 1 { "" } else { "s" }));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if !tp.is_empty() && ui.button("Clear").on_hover_text("Remove all tabs of this operation").clicked() { tp.clear(); changed = true; }
                                });
                            });
                        }
                      });
                    }
                }
                if ["outside", "inside"].contains(&p.kind.as_str()) {
                    ui.horizontal(|ui| {
                        let mut on = p.lead.is_some();
                        if ui.checkbox(&mut on, "Lead-in / out").on_hover_text("Enters and leaves the cut along an arc on the waste side, so the tool does not plunge on the line or leave a mark where it stops").changed() {
                            let dia = tools.iter().find(|t| t.id == p.tool).map(|t| t.dia).unwrap_or(3.0);
                            p.lead = if on { Some(dia.max(1.0)) } else { None };
                            changed = true;
                        }
                        if let Some(r) = p.lead.as_mut() {
                            ui.add_space(10.0);
                            ui.label("Radius");
                            if dv_w(ui, 42.0, r, 0.2, 100.0, "") { changed = true; }
                            ui.weak("mm");
                        }
                    });
                }
                changed |= ui.checkbox(&mut p.air, "Air cut (run without cutting)").on_hover_text("Runs this operation 10 mm above the stock so you can check the movements").changed();
                ui.add_space(4.0);
                if let Some(t0) = tools.iter().find(|t| t.id == p.tool) {
                    // what the machine will really run (its limits applied), with a warning when that differs from the tool
                    let (t, warn) = match &machine_now { Some(m) => (m.hold(t0), m.hold_text(t0)), None => (t0.clone(), String::new()) };
                    let ang = if putiacnc_core::toollib::has_angle(&t.shape) { format!(" {}°", t.angle) } else { String::new() };
                    ui.weak(format!("{}{} · Ø{} · {} flute{} · {} rpm · feed {} · plunge {} mm/min · step {}% · {:.2} mm/pass",
                        putiacnc_core::toollib::shape_label(&t.shape), ang, putiacnc_core::toollib::fmt_dia(t.dia, &t.sys, true), t.flutes, if t.flutes == 1 { "" } else { "s" }, t.rpm, t.feed, t.plunge, t.so, t.pd));
                    if !warn.is_empty() {
                        let col = if ui.visuals().dark_mode { Color32::from_rgb(0xf8, 0xe4, 0x5c) } else { Color32::from_rgb(0xb5, 0x83, 0x00) };
                        crate::checkui::warn_line(ui, col, &format!("Held to the machine's limits: {warn}."))
                            .on_hover_text("The G-code uses the limited values. Change the limits in Libraries → Machines, or the tool's values in the tool library.");
                    }
                }
            });
        }
            ui.add_space(6.0);
        }
        if let Some((i, dir)) = act_move {
            let j = i as i32 + dir;
            if j >= 0 && (j as usize) < d.ops.len() { d.ops.swap(i, j as usize); changed = true; }
        }
        if let Some((id, only)) = act_eye {
            if only {
                let others: Vec<i64> = d.ops.iter().map(|o| o.id).filter(|&x| x != id).collect();
                // Alt on the only visible operation brings everything back
                d.hidden_ops = if others.iter().all(|x| d.hidden_ops.contains(x)) { vec![] } else { others };
            } else if let Some(k) = d.hidden_ops.iter().position(|&x| x == id) { d.hidden_ops.remove(k); } else { d.hidden_ops.push(id); }
            changed = true;
        }
        if let Some(i) = act_remove {
            let id = d.ops[i].id;
            d.hidden_ops.retain(|&x| x != id);
            for l in d.loops.iter_mut() { if l.op == id { l.op = 0; } }
            d.ops.remove(i);
            if d.act == id { d.act = d.ops.get(i.saturating_sub(1)).map(|o| o.id).unwrap_or(0); }
            changed = true;
        }
        if d.ops.is_empty() { ui.weak("Select the shapes, then add an operation for them."); }


        }
        if !ops {
        imports_section(ui, d, sel, &mut group_act, &mut changed);
        }
    });
    if let Some((g, act)) = group_act {
        match (g, act) {
            (Some(id), GroupAct::Delete) => {
                app.sel = app.project.data.loops.iter().enumerate().filter(|(_, l)| l.grp == Some(id)).map(|(i, _)| i).collect();
                app.delete_selection();
                app.project.data.groups.retain(|x| x.id != id);
            }
            (Some(id), GroupAct::EditText) => app.edit_text(id),
            (Some(id), GroupAct::ToOutlines) => app.text_to_outlines(id),
            (None, GroupAct::Delete) => { app.hist.record(&app.project.data); app.project.data.relief = None; app.touch(); }
            (None, _) => {}
        }
    }
    if open_tools { app.tools_open = true; }
    if open_machines { app.machines_open = true; }
    if let Some(id) = mach_pick { app.set_machine(&id); }

    if changed || name_changed {
        if !app.hist.open {
            app.hist.push(before);
            app.hist.open = true;
        }
        app.touch();
    }
}

/// Work-origin picker: a square with a point at each corner and one in the centre.
/// `val` is one of ll, lr, ul, ur, c. Returns true when it changed.
pub fn tw(ui: &Ui, s: &str) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    ui.fonts(|f| f.layout_no_wrap(s.to_string(), font, Color32::WHITE).size().x).ceil()
}

/// Shorten `s` with an ellipsis so it fits in `max_w` pixels.
fn elide(ui: &Ui, s: &str, max_w: f32) -> String {
    if tw(ui, s) <= max_w { return s.to_string(); }
    let mut t: Vec<char> = s.chars().collect();
    while !t.is_empty() {
        t.pop();
        let c: String = t.iter().collect::<String>().trim_end().to_string() + "…";
        if tw(ui, &c) <= max_w { return c; }
    }
    "…".into()
}

fn xy_picker(ui: &mut Ui, val: &mut String) -> bool {
    let size = 76.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let vis = ui.visuals().clone();
    let accent = vis.selection.bg_fill;
    let sq = rect.shrink(8.0);
    let p = ui.painter();
    p.rect(sq, 3.0, vis.extreme_bg_color, egui::Stroke::new(1.5_f32, vis.widgets.inactive.fg_stroke.color));
    let pts = [("ul", sq.left_top()), ("ur", sq.right_top()), ("ll", sq.left_bottom()), ("lr", sq.right_bottom()), ("c", sq.center())];
    let hover = resp.hover_pos();
    let near = hover.and_then(|h| pts.iter().filter(|(_, q)| q.distance(h) < 16.0).min_by(|a, b| a.1.distance(h).total_cmp(&b.1.distance(h))).map(|(k, _)| *k));
    let mut changed = false;
    if resp.clicked() {
        if let Some(k) = near { if *val != k { *val = k.to_string(); changed = true; } }
    }
    for (k, q) in pts {
        let sel = *val == k;
        if sel {
            p.circle_filled(q, 6.5, accent);
            p.circle_stroke(q, 6.5, egui::Stroke::new(1.5_f32, Color32::WHITE));
        } else {
            let hot = near == Some(k);
            p.circle(q, 4.5, if hot { vis.widgets.hovered.bg_fill } else { vis.window_fill }, egui::Stroke::new(1.5_f32, if hot { accent } else { vis.widgets.inactive.fg_stroke.color }));
        }
    }
    if near.is_some() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    let name = ZEROS.iter().find(|(k, _)| *k == val.as_str()).map(|(_, n)| *n).unwrap_or("");
    resp.on_hover_text(format!("Work origin (X0 Y0): {name}"));
    changed
}

pub fn fmt_num(v: f64) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Dismissible message bar under the view tabs (import warnings, save errors...). Wraps at any width.
pub fn message_bar(ui: &mut Ui, app: &mut App) {
    let Some(msg) = app.error.clone() else { return };
    let red = if app.theme.dark { Color32::from_rgb(0xff, 0x7b, 0x63) } else { Color32::from_rgb(0xa5, 0x1d, 0x2d) };
    let mut close = false;
    ui.horizontal_top(|ui| {
        ui.add_space(2.0);
        let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 20.0), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), 8.0, red);
        ui.painter().line_segment([r.center() + egui::vec2(0.0, -3.5), r.center() + egui::vec2(0.0, 1.0)], egui::Stroke::new(1.8_f32, Color32::WHITE));
        ui.painter().circle_filled(r.center() + egui::vec2(0.0, 4.0), 1.1, Color32::WHITE);
        let w = (ui.available_width() - 34.0).max(60.0);
        ui.allocate_ui(egui::vec2(w, 0.0), |ui| { ui.set_width(w); ui.add(egui::Label::new(egui::RichText::new(&msg).color(red)).wrap(true)); });
        if icon_close(ui).clicked() { close = true; }
    });
    if close { app.error = None; }
}

fn icon_close(ui: &mut Ui) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
    if resp.hovered() { ui.painter().rect_filled(r, 5.0, ui.visuals().widgets.hovered.weak_bg_fill); }
    paint_icon(ui.painter(), Icon::Close, r.center(), ui.visuals().text_color(), 0.55);
    resp.on_hover_text("Dismiss")
}

/// A number for a one-line summary: no trailing zeros.
fn fnum(v: f64) -> String { let t = format!("{:.1}", v); t.trim_end_matches('0').trim_end_matches('.').to_string() }

#[cfg(test)]
mod inlay_tests {
    use super::*;
    use putiacnc_core::shapes;

    #[test]
    fn the_inlay_button_makes_a_pocket_and_a_mirrored_plug_beside_it() {
        let mut f = shapes::blank_project();
        let d = &mut f.data;
        d.loops.clear();
        // an arrow-like shape, so a mirror image is not the same shape
        d.loops.push(shapes::new_poly(&[[10.0, 10.0], [30.0, 10.0], [30.0, 20.0], [15.0, 30.0]], true, 0).unwrap());
        d.loops.push(shapes::new_circle([50.0, 20.0], 5.0, 0));
        let sel: std::collections::BTreeSet<usize> = [0, 1].into_iter().collect();
        add_inlay(d, &sel);
        assert_eq!(d.ops.len(), 2);
        assert_eq!((d.ops[0].p.kind.as_str(), d.ops[1].p.kind.as_str()), ("inlay", "plug"));
        assert_eq!(d.ops[0].p.depth, 3.0);
        assert_eq!(d.ops[1].p.fit, Some(0.05));
        // 60 degree V-bit and a small flat end mill
        let vb = d.tools.iter().find(|t| t.id == d.ops[0].p.tool).unwrap();
        assert!(vb.shape == "vbit" && vb.angle == 60.0);
        let fl = d.tools.iter().find(|t| Some(&t.id) == d.ops[0].p.tool2.as_ref()).unwrap();
        assert!(fl.shape == "flat" && fl.dia <= 3.5);
        assert_eq!(d.ops[1].p.tool2, d.ops[0].p.tool2);
        // the originals go to the pocket, the copies to the plug
        assert_eq!(d.loops.len(), 4);
        assert!(d.loops[0].op == d.ops[0].id && d.loops[1].op == d.ops[0].id);
        assert!(d.loops[2].op == d.ops[1].id && d.loops[3].op == d.ops[1].id);
        // selection spans x 10..55: the copies start 10 mm to its right, mirrored
        let b = shapes::bbox_many(d.loops[2..].iter()).unwrap();
        assert!((b[0] - 65.0).abs() < 0.01 && (b[2] - 110.0).abs() < 0.01, "{b:?}");
        // mirrored: the slanted corner (15, 30) was near the left of the selection, now it is near the right (105)
        let p = d.loops[2].path();
        assert!(p.iter().any(|q| q[1] == 30_000 && (q[0] as f64 / 1000.0 - 105.0).abs() < 0.01), "{p:?}");
    }
}
