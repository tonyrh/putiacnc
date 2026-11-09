//! The Layers section of the left panel: a name, a colour, an eye and a lock for each layer of the drawing.
//! A layer only organizes the drawing. Hiding one hides its shapes from the canvas (they cannot be picked either), but
//! operations still cut them; locking one keeps its shapes where they are.
use crate::icons::{small_icon_button, small_toggle_button, Icon};
use eframe::egui::{self, Ui};
use putiacnc_core::model::ProjectData;

pub const HIDDEN_TIP: &str = "Hides this layer from the drawing. Operations still cut its shapes: the eye changes what you see, never what is cut.";

/// What is on a layer, by layer id: a line of text or a group counts once.
pub fn count(d: &ProjectData, id: i64) -> usize { ProjectData::object_count(d.loops.iter().filter(|l| d.layer_of(l) == id)) }

impl crate::app::App {
    /// Puts the selected shapes on a layer (one undo step).
    pub fn move_selection_to_layer(&mut self, id: i64) {
        self.hist.record(&self.project.data);
        for &i in &self.sel { if let Some(l) = self.project.data.loops.get_mut(i) { l.lay = (id != 0).then_some(id); } }
        self.touch();
    }
}

/// The layer section. `pick` is set to a layer whose shapes the user asked to select.
pub fn section(ui: &mut Ui, d: &mut ProjectData, changed: &mut bool, pick: &mut Option<i64>) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.heading("Layers");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if small_icon_button(ui, Icon::Add, true, "Add a layer; new shapes go on the layer highlighted").clicked() {
                let n = d.layers.len().max(1) + 1;
                let id = d.add_layer(&format!("Layer {n}"));
                d.active_layer = id;
                *changed = true;
            }
        });
    });
    let all = d.all_layers();
    let mut remove: Option<i64> = None;
    for l in &all {
        let n = count(d, l.id);
        let active = d.active_layer == l.id;
        // the layer new shapes go on has a tinted row; clicking a name makes a layer the active one
        let fill = if active { ui.visuals().selection.bg_fill.gamma_multiply(0.5) } else { egui::Color32::TRANSPARENT };
        egui::Frame::none().fill(fill).rounding(4.0).inner_margin(egui::Margin::symmetric(3.0, 2.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut c = l.rgb().unwrap_or([0x80, 0x80, 0x80]);
                if ui.color_edit_button_srgb(&mut c).on_hover_text("The colour of this layer's shapes while they are not in an operation").changed() {
                    if let Some(x) = d.layer_mut(l.id) { x.color = putiacnc_core::model::hex_rgb(c); *changed = true; }
                }
                // right side, in the order eye, lock, delete (laid out from the right)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if small_icon_button(ui, Icon::Trash, l.id != 0, if l.id != 0 { "Delete the layer; its shapes move to the first layer" } else { "The first layer cannot be deleted" }).clicked() && l.id != 0 { remove = Some(l.id); }
                    if small_toggle_button(ui, Icon::Lock, l.locked, if l.locked { "Unlock this layer" } else { "Lock this layer: its shapes cannot be moved or changed" }).clicked() {
                        if let Some(x) = d.layer_mut(l.id) { x.locked = !l.locked; *changed = true; }
                    }
                    if small_toggle_button(ui, if l.hidden { Icon::EyeOff } else { Icon::Eye }, l.hidden, if l.hidden { "Show this layer" } else { HIDDEN_TIP }).clicked() {
                        if let Some(x) = d.layer_mut(l.id) { x.hidden = !l.hidden; *changed = true; }
                    }
                    if ui.add(egui::Button::new(n.to_string()).small().frame(false)).on_hover_text("Select the shapes on this layer").clicked() { *pick = Some(l.id); }
                    // the name is plain text: a click makes this the active layer, a double click renames it
                    let w = ui.available_width().max(40.0);
                    let edit_id = egui::Id::new("layer_rename");
                    let editing = ui.data(|d| d.get_temp::<i64>(edit_id)) == Some(l.id);
                    if editing {
                        let mut name = l.name.clone();
                        let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(w));
                        if !r.has_focus() && !r.gained_focus() { r.request_focus(); }
                        if r.changed() { if let Some(x) = d.layer_mut(l.id) { x.name = name; *changed = true; } }
                        // a click anywhere else ends the rename (a plain click does not take the focus away by itself)
                        let away = ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !r.rect.contains(p)));
                        if r.lost_focus() || away || ui.input(|i| i.key_pressed(egui::Key::Escape)) { ui.data_mut(|d| d.remove::<i64>(edit_id)); }
                    } else {
                        let r = ui.allocate_response(egui::vec2(w, ui.spacing().interact_size.y), egui::Sense::click());
                        if r.hovered() { ui.painter().rect_filled(r.rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill); }
                        let col = ui.visuals().text_color();
                        let font = egui::TextStyle::Body.resolve(ui.style());
                        let galley = ui.painter().layout(l.name.clone(), font, col, w - 8.0);
                        ui.painter().with_clip_rect(r.rect).galley(egui::pos2(r.rect.left() + 4.0, r.rect.center().y - galley.size().y / 2.0), galley, col);
                        if r.clicked() && d.active_layer != l.id { d.active_layer = l.id; *changed = true; }
                        if r.double_clicked() { ui.data_mut(|d| d.insert_temp(edit_id, l.id)); }
                        r.on_hover_text(if active { "The name of the layer. New shapes go on this layer. Double-click to rename." } else { "Click to put new shapes on this layer. Double-click to rename." });
                    }
                });
            });
        });
    }
    if let Some(id) = remove {
        for l in d.loops.iter_mut().filter(|l| l.lay == Some(id)) { l.lay = None; }
        d.layers.retain(|x| x.id != id);
        if d.active_layer == id { d.active_layer = 0; }
        *changed = true;
    }
    if d.layers.iter().any(|l| l.hidden) { ui.add(egui::Label::new(egui::RichText::new("A hidden layer is still cut by the operations that use its shapes.").weak().small()).wrap(true)); }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use putiacnc_core::shapes;

    fn app() -> App {
        let mut a = App::new(None);
        a.project.data.loops = vec![shapes::new_rect([10.0, 10.0], [30.0, 30.0], 0.0, 0), shapes::new_rect([50.0, 10.0], [70.0, 30.0], 0.0, 0)];
        a
    }

    #[test]
    fn a_hidden_layer_cannot_be_picked_and_a_locked_one_cannot_move() {
        let mut a = app();
        let b = a.project.data.add_layer("Box");
        a.project.data.loops[1].lay = Some(b);
        assert_eq!(a.pick_shape([50.0, 20.0], 1.0), Some(1));
        a.project.data.layer_mut(b).unwrap().hidden = true;
        assert!(a.loop_hidden(1) && !a.loop_hidden(0));
        assert_eq!(a.pick_shape([50.0, 20.0], 1.0), None, "not picked while hidden");
        assert_eq!(a.pick_shape([20.0, 20.0], 1.0), Some(0), "the other layer still is");
        a.project.data.layer_mut(b).unwrap().hidden = false;
        a.project.data.layer_mut(b).unwrap().locked = true;
        a.sel = [1].into_iter().collect();
        assert!(a.sel_locked(), "a shape on a locked layer stays where it is");
        a.sel = [0].into_iter().collect();
        assert!(!a.sel_locked());
    }

    #[test]
    fn the_selection_moves_to_a_layer_and_back_with_undo() {
        let mut a = app();
        let id = a.project.data.add_layer("Two");
        a.sel = [1].into_iter().collect();
        a.move_selection_to_layer(id);
        assert_eq!(a.project.data.layer_of(&a.project.data.loops[1]), id);
        assert_ne!(a.project.data.layer_of(&a.project.data.loops[0]), id);
        a.do_undo();
        assert_ne!(a.project.data.layer_of(&a.project.data.loops[1]), id);
    }

    #[test]
    fn new_shapes_go_on_the_active_layer() {
        let mut a = app();
        assert_eq!(a.new_shape_layer(), None);
        let b = a.project.data.add_layer("Box");
        a.project.data.active_layer = b;
        a.add_loop(shapes::new_rect([0.0, 0.0], [5.0, 5.0], 0.0, 0));
        assert_eq!(a.project.data.loops.last().unwrap().lay, Some(b));
        // a layer that was deleted is not used any more
        a.project.data.layers.retain(|l| l.id != b);
        assert_eq!(a.new_shape_layer(), None);
    }

    #[test]
    fn copies_and_results_keep_the_layer_of_their_source() {
        let mut a = app();
        let b = a.project.data.add_layer("Box");
        a.project.data.loops[0].lay = Some(b);
        a.sel = [0].into_iter().collect();
        a.array.cols = 2.0;
        a.array_selection();
        let on_b = a.project.data.loops.iter().filter(|l| l.lay == Some(b)).count();
        assert_eq!(on_b, 2, "the copy stays on its source's layer");
        let c = shapes::combine(&[&a.project.data.loops[0], &a.project.data.loops[1]], putiacnc_core::shapes::BoolOp::Union, false);
        assert!(c.iter().all(|l| l.lay == Some(b)));
    }

    #[test]
    fn the_tools_leave_the_shapes_of_a_locked_layer_alone() {
        use putiacnc_core::shapes::CornerKind;
        let mut a = app();
        let b = a.project.data.add_layer("Box");
        a.project.data.loops[1].lay = Some(b);
        a.project.data.layer_mut(b).unwrap().locked = true;
        a.sel = [1].into_iter().collect();
        let before = a.project.data.loops.clone();
        assert!(!a.run_tool("corners", |a| a.corner_selection(CornerKind::Fillet, 2.0)), "no fillet on a locked shape");
        assert!(!a.run_tool("offset", |a| a.offset_selection(2.0, false)));
        assert!(a.project.data.loops == before);
        assert!(a.tool_note.as_ref().is_some_and(|(k, m)| *k == "corners" || m.contains("locked")));
        // a preview shows nothing either
        a.preview("corners", &[1], |x| x.corner_selection(CornerKind::Fillet, 2.0));
        assert!(a.ghost.loops.is_empty());
        // nudging does nothing; the unlocked shape still can be changed
        a.nudge(5.0, 0.0);
        assert!(a.project.data.loops == before);
        // Delete keeps a locked shape too, and deletes the unlocked ones of a mixed selection
        a.sel = [1].into_iter().collect();
        a.delete_selection();
        assert!(a.project.data.loops == before, "a locked shape is not deleted");
        a.sel = [0, 1].into_iter().collect();
        a.delete_selection();
        assert_eq!(a.project.data.loops.len(), 1);
        assert_eq!(a.project.data.loops[0].lay, Some(b), "only the locked one is left");
        a.sel = [0].into_iter().collect();
        a.project.data.loops.insert(0, shapes::new_rect([0.0, 0.0], [5.0, 5.0], 0.0, 0));
        assert!(a.run_tool("corners", |a| a.corner_selection(CornerKind::Fillet, 2.0)));
    }

    #[test]
    fn clean_up_leaves_locked_and_hidden_shapes_alone() {
        let mut a = app();
        // two identical shapes: a duplicate to remove, on the first layer; a third copy on a locked layer
        a.project.data.loops = vec![shapes::new_rect([10.0, 10.0], [30.0, 30.0], 0.0, 0), shapes::new_rect([10.0, 10.0], [30.0, 30.0], 0.0, 0), shapes::new_rect([10.0, 10.0], [30.0, 30.0], 0.0, 0)];
        let b = a.project.data.add_layer("Box");
        a.project.data.loops[2].lay = Some(b);
        a.project.data.layer_mut(b).unwrap().locked = true;
        a.sel.clear();
        a.clean.dups = true;
        a.clean_drawing();
        assert_eq!(a.project.data.loops.len(), 2, "one duplicate went, the locked copy stays");
        assert_eq!(a.project.data.loops.iter().filter(|l| l.lay == Some(b)).count(), 1);
        // a hidden layer is left alone too
        a.project.data.layer_mut(b).unwrap().locked = false;
        a.project.data.layer_mut(b).unwrap().hidden = true;
        let n = a.project.data.loops.len();
        a.clean_drawing();
        assert_eq!(a.project.data.loops.len(), n);
    }

    #[test]
    fn simplify_in_clean_up_drops_points_of_a_noisy_outline_and_undo_restores_them() {
        let mut a = app();
        // a closed outline: a 40 x 20 rectangle with extra points along the sides, plus a plain rectangle
        let mut noisy = shapes::new_rect([0.0, 0.0], [40.0, 20.0], 0.0, 0);
        noisy.pt = None; noisy.rc = None;
        let mut pts = Vec::new();
        for i in 0..40 { pts.push([i as f64, 0.0]); }
        for j in 0..20 { pts.push([40.0, j as f64]); }
        for i in 0..40 { pts.push([40.0 - i as f64, 20.0]); }
        for j in 0..20 { pts.push([0.0, 20.0 - j as f64]); }
        noisy.pts = pts.iter().flat_map(|p| [(p[0] * 1000.0) as i64, (p[1] * 1000.0) as i64]).collect();
        let rect = shapes::new_rect([100.0, 0.0], [120.0, 20.0], 0.0, 0);
        let before = a.project.data.loops.clone();
        a.project.data.loops = vec![noisy, rect];
        a.sel.clear();
        a.clean.dups = false; a.clean.join = false; a.clean.tiny = false;
        a.clean.simplify = true; a.clean.simp_tol = 0.02;
        let msg = a.clean_drawing();
        assert!(msg.to_lowercase().contains("simplified"), "{msg}");
        assert_eq!(a.project.data.loops[0].pts.len() / 2, 4, "the noisy outline is down to its corners");
        assert_eq!(a.project.data.loops[1].pts.len() / 2, 4);
        a.do_undo();
        assert_eq!(a.project.data.loops[0].pts.len() / 2, 120, "undo brings the points back");
        let _ = before;
    }

    fn dxf_two_layers() -> Vec<u8> {
        let circle = |layer: &str, x: f64| format!("0\nCIRCLE\n8\n{layer}\n10\n{x}\n20\n10\n40\n3\n");
        format!("0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n0\nLAYER\n2\nCut\n70\n0\n62\n1\n0\nLAYER\n2\nGuide\n70\n1\n62\n5\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{}{}{}0\nENDSEC\n0\nEOF\n", circle("Cut", 10.0), circle("Guide", 30.0), circle("Cut", 50.0)).into_bytes()
    }

    #[test]
    fn an_imported_dxf_brings_its_layers_and_a_later_one_reuses_them() {
        let mut a = App::new(None);
        a.project.data.loops.clear();
        let imp = putiacnc_core::dxf::import(&dxf_two_layers()).unwrap();
        a.place_import(imp, "parts".into());
        let d = &a.project.data;
        let names: Vec<&str> = d.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Layer 1", "Cut", "Guide"]);
        let cut = d.layers.iter().find(|l| l.name == "Cut").unwrap();
        assert_eq!(cut.rgb(), Some([255, 0, 0]));
        let guide = d.layers.iter().find(|l| l.name == "Guide").unwrap();
        assert!(guide.hidden, "frozen in the file");
        assert_eq!(d.loops.len(), 3);
        assert_eq!(d.loops.iter().filter(|l| l.lay == Some(cut.id)).count(), 2);
        assert_eq!(d.loops.iter().filter(|l| l.lay == Some(guide.id)).count(), 1);
        // a second import joins the layers that are there, whatever the user did to them meanwhile
        a.project.data.layer_mut(guide.id).unwrap().hidden = false;
        let imp = putiacnc_core::dxf::import(&dxf_two_layers()).unwrap();
        a.place_import(imp, "again".into());
        let d = &a.project.data;
        assert_eq!(d.layers.len(), 3, "no duplicates");
        assert!(!d.layers.iter().find(|l| l.name == "Guide").unwrap().hidden, "the layer keeps its own state");
        assert_eq!(d.loops.len(), 6);
    }

    #[test]
    fn shapes_on_no_layer_go_on_the_active_layer() {
        let mut a = App::new(None);
        a.project.data.loops.clear();
        let b = a.project.data.add_layer("Mine");
        a.project.data.active_layer = b;
        let imp = putiacnc_core::dxf::import(b"0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\n5\n20\n5\n40\n2\n0\nENDSEC\n0\nEOF\n").unwrap();
        a.place_import(imp, "c".into());
        assert_eq!(a.project.data.loops[0].lay, Some(b));
    }
}
