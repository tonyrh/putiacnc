//! Box dialog: the plates of a finger-jointed box, laid out ready to cut.
use crate::app::App;
use crate::widgets::Tip;
use eframe::egui;
use putiacnc_core::joints::{self, BoxSpec};
use putiacnc_core::shapes;

pub struct BoxTool { pub open: bool, pub w: f64, pub d: f64, pub h: f64, pub finger: f64, pub clearance: f64, pub lid: bool, pub dogbone: bool, pub dia: f64, pub cut: bool }

impl Default for BoxTool {
    fn default() -> Self { BoxTool { open: false, w: 120.0, d: 80.0, h: 50.0, finger: 10.0, clearance: 0.05, lid: false, dogbone: false, dia: 3.175, cut: true } }
}

impl App {
    /// Adds the plates of a box to the drawing, to the right of what is there, in rows no wider than the stock.
    pub fn add_box(&mut self, b: BoxSpec, dogbone: Option<f64>, cut: bool) -> Option<String> {
        let panels = match joints::box_panels(b) { Ok(p) => p, Err(e) => return Some(e) };
        let d = &self.project.data;
        let origin = shapes::bbox_many(d.loops.iter()).map(|bx| [bx[2] + 10.0, bx[1]]).unwrap_or([0.0, 0.0]);
        let max_w = if d.stock.w > 0.0 { d.stock.w.max(b.w.max(b.d) + 1.0) } else { 600.0 };
        let pos = joints::layout(&panels, 6.0, max_w);
        let mut made = Vec::new();
        let mut dogs = 0;
        for (p, o) in panels.iter().zip(&pos) {
            let pts: Vec<[f64; 2]> = p.outline.iter().map(|q| [q[0] + o[0] + origin[0], q[1] + o[1] + origin[1]]).collect();
            let Some(poly) = shapes::new_poly(&pts, true, 0) else { continue };
            match dogbone.and_then(|dia| joints::dogbones(&[&poly], dia, joints::Cut::Outside, 0)) {
                Some((ls, n)) => { dogs += n; made.extend(ls); }
                None => made.push(poly),
            }
        }
        if made.is_empty() { return Some("Nothing to add.".into()); }
        self.hist.record(&self.project.data);
        let start = self.project.data.loops.len();
        self.project.data.loops.extend(made);
        self.sel = (start..self.project.data.loops.len()).collect();
        let mut note = format!("{} plates added{}", panels.len(), if dogs > 0 { format!(", with {dogs} dogbones") } else { String::new() });
        if cut {
            // one operation round the outside of every plate, as deep as the material is thick and a little more so it cuts through
            let sel = self.sel.clone();
            shapes::add_operation(&mut self.project.data, &sel, "outside");
            if let Some(o) = self.project.data.ops.last_mut() {
                o.p.kind = "outside".into();
                o.p.depth = b.t + 0.2;
            }
            note.push_str(&format!(" and an outside cut {} mm deep", b.t + 0.2));
        }
        self.touch();
        Some(note + ".")
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.boxgen.open { app.ghost.clear_if("box"); if app.tool_note.as_ref().is_some_and(|(k, _)| *k == "box") { app.tool_note = None; } return; }
    let spec = |a: &App| BoxSpec { w: a.boxgen.w, d: a.boxgen.d, h: a.boxgen.h, t: a.project.data.stock.t, finger: a.boxgen.finger, clearance: a.boxgen.clearance, lid: a.boxgen.lid };
    let (s, dog, cut) = (spec(app), app.boxgen.dogbone.then_some(app.boxgen.dia), app.boxgen.cut);
    let key: Vec<u64> = [s.w, s.d, s.h, s.t, s.finger, s.clearance, dog.unwrap_or(0.0)].iter().map(|v| crate::preview::bits(*v)).chain([s.lid as u64, cut as u64]).collect();
    app.preview("box", &key, move |a| a.add_box(s, dog, cut));
    let pnote = app.ghost.note_of("box").map(String::from);
    let thick = app.project.data.stock.t;
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Box").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Box", crate::csd::anchor(app))).show(ctx, |ui| {
        ui.set_width(376.0);
        if crate::csd::dialog_header_help(ui, "Box", true, |ui| {
            ui.label("Adds the plates of a box with finger joints to the drawing, side by side, ready to cut. The sizes are the outside sizes.");
            ui.add_space(4.0);
            ui.label("The material thickness is the stock's. Fingers alternate from plate to plate; each corner belongs to one plate.");
        }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(format!("Material {thick} mm thick (the stock)."));
        ui.add_space(6.0);
        let lw = ["Width", "Depth", "Height", "Finger width", "Clearance", "Cutter"].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0_f32, f32::max);
        egui::Grid::new("box_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            for (name, v, lo, hi, dec) in [("Width", 0usize, 1.0, 2000.0, 1usize), ("Depth", 1, 1.0, 2000.0, 1), ("Height", 2, 1.0, 2000.0, 1), ("Finger width", 3, 0.5, 500.0, 2), ("Clearance", 4, 0.0, 2.0, 2)] {
                crate::toolmgr::rlabel(ui, lw, name);
                ui.horizontal(|ui| {
                    let b = &mut app.boxgen;
                    let r = match v { 0 => &mut b.w, 1 => &mut b.d, 2 => &mut b.h, 3 => &mut b.finger, _ => &mut b.clearance };
                    crate::widgets::num(ui, [70.0, 24.0], r, lo, hi, if v < 3 { 1.0 } else { 0.05 }, dec, "");
                    ui.weak("mm");
                });
                ui.end_row();
            }
            if app.boxgen.dogbone {
                crate::toolmgr::rlabel(ui, lw, "Cutter");
                ui.horizontal(|ui| { crate::widgets::num(ui, [70.0, 24.0], &mut app.boxgen.dia, 0.1, 50.0, 0.25, 3, ""); ui.weak("mm"); });
                ui.end_row();
            }
        });
        ui.add_space(4.0);
        crate::widgets::check(ui, &mut app.boxgen.lid, "Lid (a top plate)").tip("Without it the box is open at the top");
        crate::widgets::check(ui, &mut app.boxgen.dogbone, "Dogbones in the slots").tip("A small round relief in each inside corner so a round cutter leaves room for the square corners");
        crate::widgets::check(ui, &mut app.boxgen.cut, "Create the outside cut").tip("Adds an operation that cuts round the outside of every plate, right through the material. Choose its tool and tabs in the project panel.");
        if let Some(n) = pnote.as_ref().or(app.tool_note.as_ref().filter(|(k, _)| *k == "box").map(|(_, m)| m)) { ui.weak(n); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Add box").clicked() { apply = true; }
            if ui.button("Cancel").clicked() { close = true; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::widgets::check(ui, &mut app.preview_on, "Preview").tip("Show the plates in the drawing while you change the values");
            });
        });
        });
    });
    if apply {
        let (s, dog, cut) = (spec(app), app.boxgen.dogbone.then_some(app.boxgen.dia), app.boxgen.cut);
        app.ghost.clear();
        if app.run_tool("box", |a| a.add_box(s, dog, cut)) { app.boxgen.open = false; }
    }
    if close { app.boxgen.open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_adds_its_plates_beside_the_drawing_and_undo_removes_them() {
        let mut a = App::new(None);
        a.project.data.stock.t = 3.0;
        a.project.data.loops = vec![shapes::new_rect([0.0, 0.0], [20.0, 20.0], 0.0, 0)];
        let b = BoxSpec { w: 90.0, d: 60.0, h: 40.0, t: 3.0, finger: 10.0, clearance: 0.05, lid: false };
        let msg = a.add_box(b, None, false).unwrap();
        assert!(msg.starts_with("5 plates"), "{msg}");
        assert_eq!(a.project.data.loops.len(), 6);
        let bx = shapes::bbox_many(a.project.data.loops[1..].iter()).unwrap();
        assert!(bx[0] >= 29.9, "to the right of the drawing: {bx:?}");
        a.do_undo();
        assert_eq!(a.project.data.loops.len(), 1);
        let msg = a.add_box(BoxSpec { w: 8.0, ..b }, None, false).unwrap();
        assert!(msg.contains("at least"), "{msg}");
        assert_eq!(a.project.data.loops.len(), 1);
        assert!(a.add_box(b, Some(3.0), false).unwrap().contains("dogbones"));
    }

    #[test]
    fn the_outside_cut_takes_every_plate_and_goes_through_the_material() {
        let mut a = App::new(None);
        a.project.data.stock.t = 3.0;
        let n_ops = a.project.data.ops.len();
        let b = BoxSpec { w: 90.0, d: 60.0, h: 40.0, t: 3.0, finger: 10.0, clearance: 0.05, lid: true };
        assert!(a.add_box(b, None, true).unwrap().contains("outside cut"));
        let d = &a.project.data;
        assert_eq!(d.ops.len(), n_ops + 1);
        let o = d.ops.last().unwrap();
        assert_eq!(o.p.kind, "outside");
        assert!((o.p.depth - 3.2).abs() < 1e-9);
        assert_eq!(d.loops.len(), 6);
        assert!(d.loops.iter().all(|l| l.op == o.id));
        // and it makes G-code
        assert!(putiacnc_core::generate(d, putiacnc_core::Units::Metric).unwrap().lines.iter().any(|l| l.starts_with("G1")));
        a.do_undo();
        assert_eq!(a.project.data.ops.len(), n_ops);
        assert!(a.project.data.loops.is_empty());
    }
}
