//! The Layers section of the left panel: a name, a colour, an eye and a lock for each layer of the drawing.
//! A layer only organizes the drawing. Hiding one hides its shapes from the canvas (they cannot be picked either), but
//! operations still cut them; locking one keeps its shapes where they are.
use crate::icons::{small_icon_button, small_toggle_button, Icon};
use eframe::egui::{self, Ui};
use putiacnc_core::model::ProjectData;
use std::collections::BTreeSet;

pub const HIDDEN_TIP: &str = "Hides this layer from the drawing. Operations still cut its shapes: the eye changes what you see, never what is cut.";

/// Shapes on each layer, by layer id.
pub fn count(d: &ProjectData, id: i64) -> usize { d.loops.iter().filter(|l| d.layer_of(l) == id).count() }

/// The layer section. `pick` is set to a layer whose shapes the user asked to select.
pub fn section(ui: &mut Ui, d: &mut ProjectData, sel: &BTreeSet<usize>, changed: &mut bool, pick: &mut Option<i64>) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.heading("Layers");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if small_icon_button(ui, Icon::Add, true, "Add a layer; new shapes go on the layer marked with the dot").clicked() {
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
        ui.horizontal(|ui| {
            if ui.radio(d.active_layer == l.id, "").on_hover_text("New shapes go on this layer").clicked() && d.active_layer != l.id { d.active_layer = l.id; *changed = true; }
            if small_toggle_button(ui, if l.hidden { Icon::EyeOff } else { Icon::Eye }, l.hidden, if l.hidden { "Show this layer" } else { HIDDEN_TIP }).clicked() {
                if let Some(x) = d.layer_mut(l.id) { x.hidden = !l.hidden; *changed = true; }
            }
            if small_toggle_button(ui, Icon::Lock, l.locked, if l.locked { "Unlock this layer" } else { "Lock this layer: its shapes cannot be moved or changed" }).clicked() {
                if let Some(x) = d.layer_mut(l.id) { x.locked = !l.locked; *changed = true; }
            }
            let mut c = l.rgb().unwrap_or([0x80, 0x80, 0x80]);
            if ui.color_edit_button_srgb(&mut c).on_hover_text("The colour of this layer's shapes while they are not in an operation").changed() {
                if let Some(x) = d.layer_mut(l.id) { x.color = putiacnc_core::model::hex_rgb(c); *changed = true; }
            }
            let mut name = l.name.clone();
            let w = (ui.available_width() - 62.0).max(40.0);
            if ui.add(egui::TextEdit::singleline(&mut name).desired_width(w)).on_hover_text("The name of the layer").changed() {
                if let Some(x) = d.layer_mut(l.id) { x.name = name; *changed = true; }
            }
            if ui.add(egui::Button::new(n.to_string()).small().frame(false)).on_hover_text("Select the shapes on this layer").clicked() { *pick = Some(l.id); }
            if l.id != 0 && small_icon_button(ui, Icon::Trash, true, "Delete the layer; its shapes move to the first layer").clicked() { remove = Some(l.id); }
        });
    }
    if let Some(id) = remove {
        for l in d.loops.iter_mut().filter(|l| l.lay == Some(id)) { l.lay = None; }
        d.layers.retain(|x| x.id != id);
        if d.active_layer == id { d.active_layer = 0; }
        *changed = true;
    }
    if all.len() > 1 && sel.iter().any(|&i| i < d.loops.len()) {
        ui.horizontal(|ui| {
            ui.label("Move selection to");
            let mut to: Option<i64> = None;
            egui::ComboBox::from_id_source("layer_move").selected_text("…").show_ui(ui, |ui| {
                for l in &all { if ui.selectable_label(false, &l.name).clicked() { to = Some(l.id); } }
            });
            if let Some(id) = to {
                for &i in sel { if let Some(l) = d.loops.get_mut(i) { l.lay = (id != 0).then_some(id); } }
                *changed = true;
            }
        });
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
}
