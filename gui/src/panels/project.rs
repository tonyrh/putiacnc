//! The project panel: stock, imports, shapes and operations.
use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum GroupAct { Delete, EditText, ToOutlines }

/// X, Y, width and height of an imported thing. Returns the new (x, y, width, height) when a field was edited.
/// `zero` is the XY zero of the stock: X and Y are shown (and typed) relative to it, like the rulers.
pub(super) fn placement_fields(ui: &mut Ui, key: egui::Id, b: [f64; 4], zero: (f64, f64), aspect_fixed: bool, fit: &mut bool, mut lock: Option<&mut bool>) -> Option<[f64; 4]> {
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
        if !aspect_fixed && crate::widgets::check_if(ui, !locked, &mut keep, "Keep proportions").changed() { ui.data_mut(|d| d.insert_temp(keep_id, keep)); }
        if let Some(l) = lock.as_mut() { crate::widgets::check(ui, l, "Lock").on_hover_text("Stops this drawing being moved, resized, rotated or flipped by accident"); }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().button_padding = egui::vec2(10.0, 4.0);
            if ui.add_enabled(!locked, egui::Button::new("Fit to stock")).on_hover_text("Scale to the stock if it is bigger, and centre it").clicked() { *fit = true; }
        });
    });
    if moved || sized { Some([x + zero.0, y + zero.1, w, h]) } else { None }
}

/// The Spindle / Feed / Plunge / Stepover rows of a custom feeds block. A value that differs from the tool's turns blue (a lighter blue in
/// the dark theme) and shows the tool's value and a reset button; the others keep following the tool. Feeds are shown in the speed unit
/// `spd` (name, mm/min per unit) of the tool library window and stored in mm/min. Returns true when something changed.
fn custom_rows(ui: &mut Ui, id: impl std::hash::Hash, cf: &mut putiacnc_core::model::CustomFeeds, t0: &putiacnc_core::model::Tool, l1: f32, sp: f32, accent: Color32, spd: (&str, f64), so_row: bool) -> bool {
    let mut changed = false;
    let (sname, fk) = (spd.0, spd.1.max(1e-9));
    let shown = |v: f64| { let t = format!("{:.3}", v); t.trim_end_matches('0').trim_end_matches('.').to_string() };
    egui::Grid::new(id).num_columns(5).spacing([sp, 6.0]).show(ui, |ui| {
        // name, unit text, the value, the tool's value, the factor from the shown unit to the stored one
        let rows: [(&str, &str, &mut Option<f64>, f64, f64); 4] = [("Spindle", "rpm", &mut cf.rpm, t0.rpm, 1.0), ("Feed", sname, &mut cf.feed, t0.feed, fk), ("Plunge", sname, &mut cf.plunge, t0.plunge, fk), ("Stepover", "%", &mut cf.so, t0.so, 1.0)];
        // a 3D relief has its own stepover fields, so its block stops after Plunge
        for (name, unit, own, base, k) in rows.into_iter().take(if so_row { 4 } else { 3 }) {
            rlabel(ui, l1, name);
            let ov = own.is_some();
            let mut v = own.unwrap_or(base) / k;
            ui.scope(|ui| {
                if ov { ui.visuals_mut().override_text_color = Some(accent); }
                let hi = if unit == "%" { 100.0 } else { 100000.0 / k };
                let size = [62.0, ui.spacing().interact_size.y];
                let edited = if k == 1.0 { dv_w(ui, 62.0, &mut v, 1.0, hi, "") } else { crate::widgets::num(ui, size, &mut v, 0.001, hi, 1.0 / k.max(1.0), 3, "").changed() };
                if edited { let stored = v * k; *own = if (stored - base).abs() < 1e-6 * base.abs().max(1.0) { None } else { Some(stored) }; changed = true; }
            });
            ui.weak(unit);
            let hint = format!("tool: {}", shown(base / k));
            if ov { ui.label(egui::RichText::new(hint).color(accent)); } else { ui.weak(hint); }
            if ov { if reset_button(ui, "Use the tool's value again") { *own = None; changed = true; } } else { ui.label(""); }
            ui.end_row();
        }
    });
    changed
}

/// The blue of a value that differs from the tool's (a lighter one in the dark theme).
fn custom_accent(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode { let c = ui.visuals().hyperlink_color; let m = |a: u8| a + ((255 - a as u16) / 2) as u8; Color32::from_rgb(m(c.r()), m(c.g()), m(c.b())) } else { ui.visuals().hyperlink_color }
}

/// "Custom feeds and speeds" for one step of a 3D relief (roughing or finishing): the checkbox and, when on, the indented block with
/// Spindle, Feed and Plunge (the stepover has its own field in the step). Returns true when something changed.
fn relief_custom(ui: &mut Ui, id: impl std::hash::Hash + Copy, cf: &mut Option<putiacnc_core::model::CustomFeeds>, t0: Option<&putiacnc_core::model::Tool>, l1: f32, sp: f32, spd: (&str, f64)) -> bool {
    let mut changed = false;
    let mut on = cf.is_some();
    if crate::widgets::check(ui, &mut on, "Custom feeds and speeds").on_hover_text("Use other spindle speed and feeds than the tool's for this step only. Values you leave alone keep following the tool.").changed() {
        *cf = if on { Some(putiacnc_core::model::CustomFeeds::default()) } else { None };
        changed = true;
    }
    if let (Some(c), Some(t0)) = (cf.as_mut(), t0) {
        let accent = custom_accent(ui);
        ui.indent((id, "cfs_opts"), |ui| { changed |= custom_rows(ui, (id, "cfs"), c, t0, l1, sp, accent, spd, false); });
    }
    changed
}

/// The small round-arrow button that puts a field back to its default: a real button with the usual padding.
pub(super) fn reset_button(ui: &mut Ui, tip: &str) -> bool {
    ui.add_sized([26.0, ui.spacing().interact_size.y], egui::Button::new("↺")).on_hover_text(tip).clicked()
}

/// The imported files (drawings and the 3D model) with their position and size, and what can be done with them.
pub(super) fn imports_section(ui: &mut Ui, d: &mut putiacnc_core::model::ProjectData, _sel: &std::collections::BTreeSet<usize>, act: &mut Option<(Option<i64>, GroupAct)>, changed: &mut bool) {
    // forget drawings whose shapes are all gone
    let alive: Vec<i64> = d.groups.iter().filter(|g| d.loops.iter().any(|l| l.grp == Some(g.id))).map(|g| g.id).collect();
    if alive.len() != d.groups.len() { d.groups.retain(|g| alive.contains(&g.id)); }
    if d.groups.is_empty() && d.relief.is_none() { return; }
    ui.separator();
    // a 3D model counts as imported (with only a model and no drawings, "all groups are text" would be true of nothing and say Text)
    let has_text = d.groups.iter().any(|g| g.text.is_some());
    let has_import = d.relief.is_some() || d.groups.iter().any(|g| g.text.is_none());
    ui.heading(match (has_text, has_import) { (true, false) => "Text", (true, true) => "Imported and text", _ => "Imported" });
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
                    crate::widgets::check(ui, &mut lock_now, "Lock").on_hover_text("Stops this text being moved, resized, rotated or flipped by accident");
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

/// One body for both panels: `ops` picks the Operations section (right panel) or the
/// Name / Stock / Selection / Tools sections (left panel), so editing and undo logic is shared.
pub(super) fn project_panel(ui: &mut Ui, app: &mut App, ops: bool) {
    let dark = app.theme.dark;
    let before = app.project.data.clone();
    let mut changed = false;
    let mut name_changed = false;
    let mut open_tools = false;
    // the speed unit the tool library window shows (feeds in the custom feeds block follow it)
    let spd = app.tool_speed();
    // machine picker data (read before the panel borrows the project)
    let mach_names: Vec<(String, String)> = app.machines.list.iter().map(|m| (m.id.clone(), m.name.clone())).collect();
    let (mach_cur, mach_name, mach_area) = match &app.project.data.machine {
        Some(m) => (m.id.clone(), m.name.clone(), format!("Work area {} × {} × {} mm", fmt_num(m.x), fmt_num(m.y), fmt_num(m.z))),
        None => (String::new(), String::new(), String::new()),
    };
    let mut mach_pick: Option<String> = None;
    let mut open_machines = false;
    let mut group_act: Option<(Option<i64>, GroupAct)> = None;
    // plug moved onto or off a separate piece: (pocket id, plug id, the piece)
    let mut piece_act: Option<(i64, i64, Option<putiacnc_core::model::PlugPiece>)> = None;
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
            if !has_model { r.on_disabled_hover_text("Import a 3D model first (File > Import 3D model)"); } else if r.clicked() { add_kind = Some("relief"); }
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
        let (stock_w, frame_w) = stock_card(ui, d, &mut changed);
        clamps_card(ui, d, show_clamps, stock_w, frame_w, &mut changed);
        }
        if ops {
        // ---- operations --------------------------------------------------------------
        // the operation list, its buttons row and the editor frame all share this width
        let lw = ui.available_width();
        if let Some(kind) = add_kind {
            let moved = shapes::add_operation(d, sel, kind);
            changed = true;
            // shapes that were in another operation moved to this one, and the status bar says so
            if let Some(from) = moved.filter(|f| !f.is_empty()) {
                let name = |oid: i64| d.ops.iter().position(|o| o.id == oid).map(|k| format!("{}. {}", k + 1, label_of(KINDS, &d.ops[k].p.kind)));
                let first = from[0];
                let same = from.iter().all(|&o| o == first);
                let from_txt = if same { name(first).unwrap_or_else(|| "another operation".into()) } else { "other operations".into() };
                let msg = format!("{} shape{} moved from {}", from.len(), if from.len() == 1 { "" } else { "s" }, from_txt);
                let now = ui.input(|i| i.time);
                ui.ctx().data_mut(|x| x.insert_temp(egui::Id::new("status_note"), (msg, now)));
            }
        }
        let tools = d.tools.clone();
        // the tools the operations use, in order of first use: the tool pickers offer them first
        let mut job_tools: Vec<String> = Vec::new();
        for o in &d.ops { for t in std::iter::once(&o.p.tool).chain(o.p.tool2.iter()) { if !t.is_empty() && !job_tools.contains(t) { job_tools.push(t.clone()); } } }
        let machine_now = d.machine.clone();
        let mut act_remove: Option<usize> = None;
        let mut act_move: Option<(usize, i32)> = None;
        let mut act_use: Option<i64> = None;
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
            let sy = ui.spacing().item_spacing.y;
            let card_hdr: (egui::Rect, bool, Color32);
            {
                let kind_icon = match o_kind.as_str() { "pocket" => Icon::Pocket, "vcarve" => Icon::VCarve, "drill" => Icon::Drill, "inlay" | "plug" => Icon::Inlay, _ => Icon::Profile };
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(lw, 40.0), egui::Sense::click());
                let selected = hi_ids.contains(&id);
                let resp = resp.on_hover_text("Select this operation's shapes in the drawing");
                let folded = collapsed.contains(&id);
                let base = crate::theme::op_color(i, dark);
                let fill = if selected { base } else if resp.hovered() { base.gamma_multiply(0.82) } else { base.gamma_multiply(0.66) };
                let fill = if in3d && hidden_now.contains(&id) { fill.gamma_multiply(0.55) } else { fill };
                let rnd = if folded { egui::Rounding::same(6.0) } else { egui::Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 } };
                ui.painter().rect_filled(rect, rnd, fill);
                card_hdr = (rect, selected, base);
                // the operations that own the shapes selected in the drawing are brighter and outlined
                if selected {
                    if folded { ui.painter().rect_stroke(rect.expand(1.5), 7.0, egui::Stroke::new(2.5_f32, ui.visuals().text_color())); }
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
            if collapsed.contains(&id) { ui.add_space(2.0); } else { ui.spacing_mut().item_spacing.y = 0.0; }
            if !collapsed.contains(&id) {
            let loops = &d.loops;
            let stock_t = d.stock.t;
            // a relief follows the model's height until a depth is typed in
            if d.ops[i].p.kind == "relief" && !d.ops[i].p.d_man {
                let eff = putiacnc_core::toolpath::relief_depth(d, &d.ops[i].p);
                if (d.ops[i].p.depth - eff).abs() > 1e-9 { d.ops[i].p.depth = eff; changed = true; }
            }
            let model_h = d.relief.as_ref().map(|r| r.zmax * r.scale);
            let prev_id = i.checked_sub(1).map(|j| d.ops[j].id).unwrap_or(0);
            let main_stock = d.stock.clone();
            let o = &mut d.ops[i];
            // one card with the header: the same colour all round, flat where the two meet
            let card_col = if card_hdr.1 { card_hdr.2 } else { card_hdr.2.gamma_multiply(0.66) };
            let body_card = egui::Frame::none().inner_margin(egui::Margin::same(7.5)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = sy;
                ui.set_width(lw - 15.0);
                let p = &mut o.p;
                // an operation without shapes keeps its settings; the selected shapes can be put into it (a relief works on the model)
                if p.kind != "relief" {
                    let mine = loops.iter().filter(|l| l.op == id).count();
                    let to_move = sel.iter().filter(|&&k| loops.get(k).is_some_and(|l| l.op != id)).count();
                    if mine == 0 {
                        ui.horizontal(|ui| {
                            ui.weak("No shapes yet");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add_enabled(to_move > 0, egui::Button::new("Use selected shapes")).on_hover_text("Put the shapes selected in the drawing into this operation").clicked() { act_use = Some(id); }
                            });
                        });
                    }
                }
                let sp = ui.spacing().item_spacing.x;
                let bf = egui::TextStyle::Body.resolve(ui.style());
                let tw_of = |ui: &Ui, s: &str| ui.fonts(|f| f.layout_no_wrap(s.to_string(), bf.clone(), Color32::WHITE).size().x);
                // 2 x 2 grid: Type | Cut  /  Tool | Depth
                let l1 = tw_of(ui, "Type").max(tw_of(ui, "Tool")).max(tw_of(ui, "Depth")).max(tw_of(ui, "Rough")).max(tw_of(ui, "Finish")).max(tw_of(ui, "Spindle")).max(tw_of(ui, "Stepover"));
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
                        if crate::widgets::check(ui, &mut on, egui::RichText::new("Roughing").strong()).on_hover_text("Removes the bulk with a flat end mill, leaving a little for the finishing step").changed() { p.r_off = !on; changed = true; }
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
                            let t0 = p.tool2.as_deref().and_then(|tid| tools_ref.iter().find(|t| t.id == tid));
                            changed |= relief_custom(ui, ("rough_cf", id), &mut p.custom2, t0, lw2, sp, spd);
                        }
                    });
                    // ---- finishing
                    ui.add_space(4.0);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let mut on = !p.f_off;
                        if crate::widgets::check(ui, &mut on, egui::RichText::new("Finishing").strong()).on_hover_text("Follows the surface line by line, usually with a ball nose").changed() { p.f_off = !on; changed = true; }
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
                            let t0 = tools_ref.iter().find(|t| t.id == p.tool);
                            changed |= relief_custom(ui, ("finish_cf", id), &mut p.custom, t0, lw2, sp, spd);
                        }
                    });
                    if p.r_off && p.f_off { ui.add_space(2.0); ui.colored_label(Color32::from_rgb(220, 120, 0), "Both steps are off: nothing will be cut."); }
                } else {
                    // Tool row: the selector takes all the room left of the right-aligned "Ramp in"
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = sp;
                        // an inlay ramps with its flat end mill, so the checkbox of the plug is on the Flat row, and the pocket has none
                        let has_ramp = !matches!(p.kind.as_str(), "vcarve" | "drill" | "relief" | "inlay" | "plug");
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
                            changed |= crate::widgets::check_if(ui, p.lead.is_none() || !["outside", "inside"].contains(&p.kind.as_str()), &mut p.ramp, "Ramp in")
                                .on_hover_text(if p.kind == "pocket" { "Eases the tool down along each ring of the pocket instead of plunging straight down" } else { "Eases the tool down along the first pass (not needed with a lead-in arc)" }).changed();
                        }
                    });
                    if p.kind == "inlay" || p.kind == "plug" || p.kind == "relief" {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = sp;
                            // the same width as the Tool selector above, so the two dropdowns line up (the room for "Ramp in" is kept in both)
                            let ramp_w = tw_of(ui, "Ramp in") + 30.0;
                            let tool_w = (ui.available_width() - l1 - ramp_w - 3.0 * sp - 8.0).max(100.0);
                            rlabel(ui, l1, if p.kind == "relief" { "Rough" } else { "Flat" });
                            let cur = p.tool2.as_ref().and_then(|id| tools.iter().find(|t| &t.id == id)).map(|t| clean(&t.name)).unwrap_or_else(|| "none".into());
                            let cur = elide(ui, &cur, tool_w - 30.0);
                            let (r2, pk2) = crate::toolpick::picker(ui, ("optool2", id), tool_w, cur, p.tool2.as_deref(), &job_tools, &tools, |t| t.shape == "flat");
                            match pk2 { Some(crate::toolpick::Picked::Tool(t)) => { p.tool2 = Some(t); changed = true; } Some(crate::toolpick::Picked::Manage) => open_tools = true, None => {} }
                            r2.on_hover_text(match p.kind.as_str() { "inlay" => "Flat end mill that clears the pocket floor", "relief" => "Flat end mill that removes the bulk before the finishing pass (leave empty to finish only)", _ => "Flat end mill that cuts the plug out of the stock" });
                            if p.kind == "plug" || p.kind == "inlay" {
                                let rest = ui.available_width() - ramp_w;
                                if rest > 0.0 { ui.add_space(rest); }
                                let pocket = p.kind == "inlay";
                                changed |= crate::widgets::check(ui, &mut p.ramp, "Ramp in")
                                    .on_hover_text(if pocket { "Eases the flat end mill down along each ring that clears the pocket floor instead of plunging" } else { "Eases the flat end mill down along the cut-out of the plug instead of plunging" }).changed();
                            }
                        });
                        if p.kind == "plug" {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = sp;
                                rlabel(ui, l1, "Gap");
                                let mut g = p.fit.unwrap_or(0.05);
                                if dv_w(ui, 54.0, &mut g, 0.0, 2.0, "") { p.fit = Some(g); changed = true; }
                                ui.weak("mm").on_hover_text("How much smaller the plug is than the pocket, sideways. 0 is a press fit; add a little for glue.");
                                ui.weak("smaller than the pocket, for glue");
                            });
                            // the plug can be cut from a piece of its own (a second G-code file, its own work zero)
                            ui.add_space(2.0);
                            let mut on = p.piece.is_some();
                            if ui.horizontal(|ui| { ui.add_space(6.0); crate::widgets::check(ui, &mut on, "Cut from a separate piece").on_hover_text("The plug is cut from its own piece of stock, with its own G-code file and work zero. Use it when the plug is another wood than the pocket's.").changed() }).inner {
                                piece_act = Some((prev_id, id, if on { Some(putiacnc_core::model::PlugPiece { same: true, ..putiacnc_core::pieces::default_piece(&main_stock) }) } else { None }));
                            }
                            if let Some(pc) = p.piece.as_mut() {
                                let mut moved = false;
                                ui.add_space(2.0);
                                egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(8.0)).show(ui, |ui| {
                                    // the frame adds 12 px (its margins and outline) to the width set here
                                    ui.set_width(ui.available_width() - 12.0);
                                    ui.spacing_mut().item_spacing.x = sp;
                                    // two columns of label + field + unit that share the width; the left labels line up, and so do the right ones
                                    let (la, lb, mm) = (tw_of(ui, "Thickness").max(tw_of(ui, "Position X")).max(tw_of(ui, "Material")).max(tw_of(ui, "XY zero")), tw_of(ui, "Position Y").max(tw_of(ui, "Height")), tw_of(ui, "mm"));
                                    let fw = ((ui.available_width() - la - lb - 2.0 * mm - 6.0 * sp) / 2.0).clamp(46.0, 110.0);
                                    changed |= crate::widgets::check(ui, &mut pc.same, "Replaces the main stock").on_hover_text("The main stock is taken off the machine and this piece put in its place: same size and same work zero, so only Z may need setting again. Switch it off to give the piece its own size and zero.").changed();
                                    ui.add_space(2.0);
                                    let same = pc.same;
                                    let mut col5_x = 0.0_f32;
                                    egui::Grid::new(("plug_piece", id)).num_columns(6).spacing([sp, 6.0]).show(ui, |ui| {
                                        if !same {
                                            rlabel(ui, la, "Width"); changed |= dv_w(ui, fw, &mut pc.w, 1.0, 10000.0, ""); ui.weak("mm");
                                            rlabel(ui, lb, "Height"); let r = ui.scope(|ui| dv_w(ui, fw, &mut pc.h, 1.0, 10000.0, "")); col5_x = r.response.rect.min.x; changed |= r.inner; ui.weak("mm"); ui.end_row();
                                        }
                                        rlabel(ui, la, "Thickness"); changed |= dv_w(ui, fw, &mut pc.t, 0.1, 1000.0, ""); ui.weak("mm"); ui.end_row();
                                        rlabel(ui, la, "Position X");
                                        moved |= dv_w(ui, fw, &mut pc.x, -100000.0, 100000.0, ""); ui.weak("mm");
                                        rlabel(ui, lb, "Position Y"); moved |= dv_w(ui, fw, &mut pc.y, -100000.0, 100000.0, ""); ui.weak("mm"); ui.end_row();
                                    });
                                    if !same {
                                    ui.add_space(4.0);
                                    // the XY picker with the Z zero beside it, as in the Stock section; the Z labels end where the right-hand labels above end,
                                    // and the Z fields start where the right-hand fields above start
                                    ui.horizontal_top(|ui| {
                                        rlabel(ui, la, "XY zero");
                                        changed |= xy_picker(ui, &mut pc.zr);
                                        let gap = col5_x - sp - lb - ui.next_widget_position().x;
                                        ui.add_space(gap.max(6.0));
                                        ui.vertical(|ui| {
                                            ui.spacing_mut().item_spacing.y = 6.0;
                                            rlabel(ui, lb, "Z zero");
                                            if pc.zm != "top" { rlabel(ui, lb, "Z0"); }
                                        });
                                        ui.vertical(|ui| {
                                            ui.spacing_mut().item_spacing.y = 6.0;
                                            let (c, _) = combo_w2(ui, fw + 10.0, &format!("plug_zm{id}"), &mut pc.zm, PIECE_ZMODES, "", "Where Z0 is measured from on this piece: its top surface, or a height above its bottom");
                                            changed |= c;
                                            if pc.zm != "top" { ui.horizontal(|ui| { changed |= dv_w(ui, fw, &mut pc.zh, 0.0, 1000.0, ""); ui.weak("mm"); }); }
                                        });
                                    });
                                    }
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        rlabel(ui, la, "Material");
                                        if !MATERIALS.iter().any(|(k, _)| *k == pc.mat) { pc.mat = "wood".into(); }
                                        let mat_w = crate::widgets::combo_fit(ui, &MATERIALS.iter().map(|(_, l)| *l).collect::<Vec<_>>()) + 10.0;
                                        let (c, _) = combo_w2(ui, mat_w, &format!("plug_mat{id}"), &mut pc.mat, MATERIALS, "", "How the piece looks in the drawing and the 3D preview");
                                        changed |= c;
                                    });
                                });
                                if moved { piece_act = Some((prev_id, id, Some(pc.clone()))); }
                            }
                        }
                    }
                    // depth, per-operation pass depth (no override = the tool's own value) and the resulting pass count
                    let inlay = p.kind == "inlay" || p.kind == "plug" || p.kind == "relief";
                    // an inlay clears and cuts out with its second tool, so the pass depth is that tool's
                    let tool_pd = if inlay { p.tool2.as_deref().and_then(|id| tool_pd_of(&tools, id)).or_else(|| tool_pd_of(&tools, &p.tool)) } else { tool_pd_of(&tools, &p.tool) };
                    ui.add_space(4.0);
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
                    changed |= ui.horizontal(|ui| { ui.add_space(6.0); crate::widgets::check(ui, &mut p.tabs, "Holding tabs").changed() }).inner;
                    if p.tabs {
                      ui.indent(("tab_opts", id), |ui| {
                        let manual = p.tab_pts.is_some();
                        ui.horizontal(|ui| {
                            ui.label("Placement").on_hover_text("Automatic spaces the tabs evenly; manual lets you put each one where you want it");
                            let (c, _) = crate::zoomcard::seg2(ui, ["Automatic", "Manual"], ["", "Starts from the evenly spaced tabs, which you can then move, add or remove"], manual as usize);
                            if c == Some(0) && manual {
                                p.tab_pts = None;
                                if *tab_edit == Some(id) { *tab_edit = None; }
                                changed = true;
                            }
                            if c == Some(1) && !manual {
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
                                if icon_text_button(ui, if editing { Icon::Check } else { Icon::Add }, label, tip, None).clicked() {
                                    *tab_edit = if editing { None } else { Some(id) };
                                    d.act = id;
                                }
                                ui.weak(format!("{} tab{}", tp.len(), if tp.len() == 1 { "" } else { "s" }));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(6.0);
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
                        ui.add_space(6.0);
                        if crate::widgets::check(ui, &mut on, "Lead-in / out").on_hover_text("Enters and leaves the cut along an arc on the waste side, so the tool does not plunge on the line or leave a mark where it stops").changed() {
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
                changed |= ui.horizontal(|ui| { ui.add_space(6.0); crate::widgets::check(ui, &mut p.air, "Air cut (run without cutting)").on_hover_text("Runs this operation 10 mm above the stock so you can check the movements").changed() }).inner;
                // this operation's own spindle speed, feeds and stepover: one block for the tool, and for an inlay one for each of its two tools
                if p.kind != "relief" {
                    let two = matches!(p.kind.as_str(), "inlay" | "plug");
                    let mut on = p.custom.is_some() || p.custom2.is_some();
                    if ui.horizontal(|ui| { ui.add_space(6.0); crate::widgets::check(ui, &mut on, "Custom feeds and speeds").on_hover_text("Use other spindle speed, feeds and stepover than the tool's for this operation only. Values you leave alone keep following the tool.").changed() }).inner {
                        p.custom = if on { Some(putiacnc_core::model::CustomFeeds::default()) } else { None };
                        p.custom2 = if on && two { Some(putiacnc_core::model::CustomFeeds::default()) } else { None };
                        changed = true;
                    }
                    if on {
                        let accent = if ui.visuals().dark_mode { { let c = ui.visuals().hyperlink_color; let m = |a: u8| a + ((255 - a as u16) / 2) as u8; Color32::from_rgb(m(c.r()), m(c.g()), m(c.b())) } } else { ui.visuals().hyperlink_color };
                        let t1 = tools.iter().find(|t| t.id == p.tool).cloned();
                        let t2 = p.tool2.as_ref().and_then(|tid| tools.iter().find(|t| &t.id == tid)).cloned();
                        ui.indent(("cfs_opts", id), |ui| {
                            if let (Some(cf), Some(t0)) = (p.custom.as_mut(), t1.as_ref()) {
                                if two { ui.label(egui::RichText::new(format!("V-bit: {}", clean(&t0.name))).weak()); }
                                changed |= custom_rows(ui, ("cfs", id), cf, t0, l1, sp, accent, spd, true);
                            }
                            if let (Some(cf), Some(t0)) = (p.custom2.as_mut(), t2.as_ref()) {
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new(format!("Flat end mill: {}", clean(&t0.name))).weak());
                                changed |= custom_rows(ui, ("cfs2", id), cf, t0, l1, sp, accent, spd, true);
                            }
                        });
                    }
                }
                ui.add_space(4.0);
                if let Some(t0) = tools.iter().find(|t| t.id == p.tool) {
                    // what the machine will really run (its limits applied, after this operation's own values), with a warning when that differs
                    let asked = match &p.custom { Some(cf) if !(p.kind == "relief" && p.f_off) => cf.apply(t0), _ => t0.clone() };
                    let (t, warn) = match &machine_now { Some(m) => (m.hold(&asked), m.hold_text(&asked)), None => (asked.clone(), String::new()) };
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
            // the border is painted on the header's own width, so the two always match; the operation that owns the
            // selected shapes gets a stronger one in the text colour in its place
            {
                let card = egui::Rect::from_min_max(card_hdr.0.min, egui::pos2(card_hdr.0.max.x, body_card.response.rect.max.y));
                // the colour border only below the header (above it the header is the border); its corners are concentric
                // with the header's, so nothing pokes out at the top
                let below = ui.painter().with_clip_rect(egui::Rect::from_min_max(egui::pos2(card.min.x - 2.0, card_hdr.0.max.y), card.max + egui::vec2(2.0, 2.0)));
                below.rect_stroke(card.shrink(0.75), egui::Rounding { nw: 0.0, ne: 0.0, sw: 5.25, se: 5.25 }, egui::Stroke::new(1.5_f32, card_col));
                if card_hdr.1 { ui.painter().rect_stroke(card.shrink(1.0), 5.0, egui::Stroke::new(2.0_f32, ui.visuals().text_color())); }
            }
            ui.spacing_mut().item_spacing.y = sy;
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
        if let Some(id) = act_use {
            if shapes::assign_to_op(&mut d.loops, sel, id) > 0 { d.act = id; changed = true; }
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
    if let Some((pocket, plug, pc)) = piece_act {
        app.hist.record(&app.project.data);
        putiacnc_core::shapes::put_plug_on_piece(&mut app.project.data, pocket, plug, pc);
        app.sel.clear();
        app.touch();
    }
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
