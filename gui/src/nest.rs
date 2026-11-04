//! Nest dialog: packs the selected shapes onto the stock in rows (see `putiacnc_core::nest`), a group or a placed text as one part.
use crate::app::App;
use eframe::egui;
use putiacnc_core::shapes;

pub struct NestTool {
    pub open: bool,
    /// Space between neighbouring parts (edge to edge), mm: leave at least the width of the cutter.
    pub gap: f64,
    /// Space kept free along the edges of the stock, mm.
    pub margin: f64,
    /// Allow a quarter turn.
    pub turn: bool,
}

impl Default for NestTool {
    fn default() -> Self { NestTool { open: false, gap: 6.0, margin: 3.0, turn: true } }
}

impl App {
    /// Moves (and, where it helps, turns by 90 degrees) the selected parts so they sit in rows on the stock.
    /// Parts that do not fit stay where they are. Returns a message when something is worth saying.
    pub fn nest_selection(&mut self) -> Option<String> {
        if self.sel.is_empty() { return Some("Select the shapes to nest first.".into()); }
        if self.sel_locked() { return Some("This drawing is locked.".into()); }
        let units = crate::align::units(self);
        let boxes: Vec<[f64; 4]> = units.iter().filter_map(|u| shapes::bbox_many(u.iter().map(|&i| &self.project.data.loops[i]))).collect();
        if boxes.len() != units.len() { return Some("Nothing to nest.".into()); }
        let (sw, sh) = (self.project.data.stock.w, self.project.data.stock.h);
        let m = self.nest.margin.max(0.0);
        let sheet = [m, m, sw - m, sh - m];
        if sheet[2] <= sheet[0] || sheet[3] <= sheet[1] { return Some("The margin leaves no room on the stock.".into()); }
        let dims: Vec<(f64, f64)> = boxes.iter().map(|b| (b[2] - b[0], b[3] - b[1])).collect();
        let r = putiacnc_core::nest::nest(&dims, sheet, self.nest.gap, self.nest.turn);
        if r.placed.is_empty() { return Some("None of the shapes fits on the stock. Try a smaller gap or margin, or allow turning.".into()); }
        self.hist.record(&self.project.data);
        for p in &r.placed {
            let unit: std::collections::BTreeSet<usize> = units[p.item].iter().copied().collect();
            let b = boxes[p.item];
            let d = &mut self.project.data;
            if p.turned {
                let tabs0 = shapes::tab_positions(&d.loops, &d.ops, &unit);
                shapes::rotate_selection(&mut d.loops, &mut d.ops, &unit, [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0], 90.0, &tabs0);
            }
            let Some(nb) = shapes::bbox_many(unit.iter().map(|&i| &d.loops[i])) else { continue };
            for &i in &unit { shapes::translate(&mut d.loops[i], p.x - nb[0], p.y - nb[1]); }
        }
        self.touch();
        let k = r.left_out.len();
        (k > 0).then(|| format!("{k} shape{} did not fit and {} left where {} {}: allow turning, or use a smaller gap or margin.", if k == 1 { "" } else { "s" }, if k == 1 { "was" } else { "were" }, if k == 1 { "it" } else { "they" }, if k == 1 { "was" } else { "were" }))
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.nest.open { if app.tool_note.as_ref().is_some_and(|(k, _)| *k == "nest") { app.tool_note = None; } return; }
    let n = crate::align::units(app).len();
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Nest on the stock").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Nest on the stock", crate::csd::anchor(app))).show(ctx, |ui| {
        ui.set_width(376.0);
        if crate::csd::dialog_header_help(ui, "Nest on the stock", true, |ui| {
            ui.label("Packs the selected shapes onto the stock in rows, tallest first from the lower left corner, to use less material. A group or a placed text moves as one part.");
            ui.add_space(4.0);
            ui.label("Leave a gap at least as wide as the cutter. Parts that do not fit stay where they are. It works from each part's bounding box, so odd shapes are not interlocked.");
        }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if n == 0 { "Select the shapes to nest first.".to_string() } else { format!("{n} part{} selected.", if n == 1 { "" } else { "s" }) });
        ui.add_space(6.0);
        egui::Grid::new("nest_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            let lw = ["Gap", "Margin"].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0_f32, f32::max);
            crate::toolmgr::rlabel(ui, lw, "Gap");
            ui.horizontal(|ui| { crate::widgets::len(ui, [70.0, 24.0], &mut app.nest.gap, 0.0, 10000.0, 0.25, 2, ""); ui.weak(crate::widgets::len_unit(ui)); });
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, "Margin");
            ui.horizontal(|ui| { crate::widgets::len(ui, [70.0, 24.0], &mut app.nest.margin, 0.0, 10000.0, 0.25, 2, ""); ui.weak(crate::widgets::len_unit(ui)); });
            ui.end_row();
        });
        ui.add_space(4.0);
        crate::widgets::check(ui, &mut app.nest.turn, "Allow turning by 90°");
        let mine = app.tool_note.as_ref().filter(|(k, _)| *k == "nest").map(|(_, m)| m);
        if let Some(m) = mine { crate::msg::line(ui, crate::app::Level::Warning, m); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(n > 0, egui::Button::new("Nest")).on_hover_text("Do it and keep the window open, to go again").clicked() { apply = true; }
            if ui.button("Close").clicked() { close = true; }
        });
        });
    });
    if apply { app.run_tool("nest", |a| a.nest_selection()); }
    if close { app.nest.open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_parts() -> App {
        let mut a = App::new(None);
        a.project.data.stock.w = 100.0;
        a.project.data.stock.h = 60.0;
        a.project.data.loops = vec![
            shapes::new_rect([70.0, 40.0], [90.0, 55.0], 0.0, 0),
            shapes::new_rect([10.0, 10.0], [40.0, 20.0], 0.0, 0),
            shapes::new_rect([50.0, 5.0], [60.0, 50.0], 0.0, 0),
        ];
        a.sel = (0..3).collect();
        a
    }

    fn boxes(a: &App) -> Vec<[f64; 4]> { a.project.data.loops.iter().map(|l| shapes::bbox(l).unwrap()).collect() }

    #[test]
    fn nested_parts_sit_inside_the_margin_apart_by_the_gap() {
        let mut a = app_with_parts();
        a.nest.gap = 4.0; a.nest.margin = 2.0; a.nest.turn = false;
        assert!(a.nest_selection().is_none());
        let b = boxes(&a);
        assert!(b.iter().all(|r| r[0] >= 2.0 - 1e-6 && r[1] >= 2.0 - 1e-6 && r[2] <= 98.0 + 1e-6 && r[3] <= 58.0 + 1e-6), "{b:?}");
        for i in 0..3 { for j in i + 1..3 {
            let (p, q) = (b[i], b[j]);
            let apart = p[2] + 4.0 <= q[0] + 1e-6 || q[2] + 4.0 <= p[0] + 1e-6 || p[3] + 4.0 <= q[1] + 1e-6 || q[3] + 4.0 <= p[1] + 1e-6;
            assert!(apart, "{i} and {j}: {p:?} {q:?}");
        } }
        // and undo puts them back
        a.do_undo();
        assert_eq!(boxes(&a)[0], [70.0, 40.0, 90.0, 55.0]);
    }

    #[test]
    fn a_part_too_big_for_the_stock_stays_put_and_is_reported() {
        let mut a = app_with_parts();
        a.project.data.loops.push(shapes::new_rect([0.0, 0.0], [300.0, 10.0], 0.0, 0));
        a.sel = (0..4).collect();
        a.nest.turn = false;
        let msg = a.nest_selection().expect("a note");
        assert!(msg.contains("did not fit"), "{msg}");
        assert_eq!(boxes(&a)[3], [0.0, 0.0, 300.0, 10.0]);
    }

    #[test]
    fn turning_lets_a_tall_part_lie_down() {
        let mut a = App::new(None);
        a.project.data.stock.w = 100.0;
        a.project.data.stock.h = 30.0;
        a.project.data.loops = vec![shapes::new_rect([0.0, 0.0], [10.0, 80.0], 0.0, 0)];
        a.sel = [0].into_iter().collect();
        a.nest.margin = 0.0; a.nest.turn = true;
        assert!(a.nest_selection().is_none());
        let b = boxes(&a)[0];
        assert!(((b[2] - b[0]) - 80.0).abs() < 1e-6 && ((b[3] - b[1]) - 10.0).abs() < 1e-6, "{b:?}");
        assert!(b[0] >= -1e-6 && b[1] >= -1e-6 && b[2] <= 100.0 + 1e-6);
    }

    #[test]
    fn nothing_selected_is_refused() {
        let mut a = app_with_parts();
        a.sel.clear();
        assert!(a.nest_selection().is_some());
    }
}
