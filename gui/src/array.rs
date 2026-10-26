//! Array / copy dialog: repeat the selected shapes in a grid or around a circle.
use crate::app::App;
use eframe::egui;
use putiacnc_core::shapes;

/// Most copies one array may make (a guard against typing a huge count).
const MAX_COPIES: usize = 500;

pub struct ArrayTool {
    pub open: bool,
    pub circular: bool,
    pub cols: f64,
    pub rows: f64,
    /// Space between neighbouring copies (edge to edge), mm.
    pub gap_x: f64,
    pub gap_y: f64,
    /// Number of shapes in the ring, the original included.
    pub count: f64,
    pub cx: f64,
    pub cy: f64,
    pub centre_set: bool,
    pub angle: f64,
    pub rotate: bool,
}

impl Default for ArrayTool {
    fn default() -> Self {
        ArrayTool { open: false, circular: false, cols: 3.0, rows: 1.0, gap_x: 5.0, gap_y: 5.0, count: 6.0, cx: 0.0, cy: 0.0, centre_set: false, angle: 360.0, rotate: true }
    }
}

impl App {
    /// Adds copies of the selected shapes, keeping the originals. Returns a message when nothing was made.
    pub fn array_selection(&mut self) -> Option<String> {
        if self.sel.is_empty() { return Some("Select one or more shapes first.".into()); }
        let a = &self.array;
        let src: Vec<usize> = self.sel.iter().copied().filter(|&i| i < self.project.data.loops.len()).collect();
        let Some(b) = shapes::bbox_many(src.iter().map(|&i| &self.project.data.loops[i])) else { return Some("Nothing to copy.".into()) };
        // every copy as (shift or turn) of the whole selection
        enum Step { Shift(f64, f64), Turn(f64) }
        let mut steps: Vec<Step> = Vec::new();
        if a.circular {
            let n = (a.count.round() as i64).max(2) as usize;
            let full = a.angle.abs() >= 359.999;
            let each = if full { a.angle / n as f64 } else { a.angle / (n - 1) as f64 };
            let c0 = [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
            for k in 1..n {
                let th = each * k as f64;
                if a.rotate { steps.push(Step::Turn(th)); }
                else {
                    let (sn, cs) = th.to_radians().sin_cos();
                    let (x, y) = (c0[0] - a.cx, c0[1] - a.cy);
                    steps.push(Step::Shift(a.cx + x * cs - y * sn - c0[0], a.cy + x * sn + y * cs - c0[1]));
                }
            }
        } else {
            let (cols, rows) = ((a.cols.round() as i64).max(1) as usize, (a.rows.round() as i64).max(1) as usize);
            let (sx, sy) = (b[2] - b[0] + a.gap_x, b[3] - b[1] + a.gap_y);
            for r in 0..rows { for c in 0..cols { if r + c > 0 { steps.push(Step::Shift(c as f64 * sx, r as f64 * sy)); } } }
        }
        if steps.is_empty() { return Some("Nothing to copy: use more than one column, row or copy.".into()); }
        if steps.len() * src.len() > MAX_COPIES { return Some(format!("That would make {} shapes; the limit is {MAX_COPIES}.", steps.len() * src.len())); }
        let (cx, cy) = (a.cx, a.cy);
        self.hist.record(&self.project.data);
        let first_new = self.project.data.loops.len();
        let mut gc = putiacnc_core::shapes::GroupCopies::new(&self.project.data.loops);
        for st in &steps {
            gc.next_batch();
            let start = self.project.data.loops.len();
            for &i in &src {
                let mut l = self.project.data.loops[i].clone();
                gc.apply(&mut l);
                match st { Step::Shift(dx, dy) => shapes::translate(&mut l, *dx, *dy), Step::Turn(th) => shapes::rotate_about(&mut l, cx, cy, *th) }
                self.project.data.loops.push(l);
            }
            for (k, &i) in src.iter().enumerate() { shapes::tabs_copy(&mut self.project.data.ops, i, start + k); }
        }
        self.sel = src.iter().copied().chain(first_new..self.project.data.loops.len()).collect();
        self.touch();
        None
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.array.open { app.ghost.clear_if("array"); return; }
    if !app.array.centre_set {
        let s = &app.project.data.stock;
        (app.array.cx, app.array.cy, app.array.centre_set) = (s.w / 2.0, s.h / 2.0, true);
    }
    {
        let a = &app.array;
        let params = [a.circular as u64, a.cols.to_bits(), a.rows.to_bits(), a.gap_x.to_bits(), a.gap_y.to_bits(), a.count.to_bits(), a.cx.to_bits(), a.cy.to_bits(), a.angle.to_bits(), a.rotate as u64];
        app.preview("array", &params, |x| x.array_selection());
    }
    let pnote = app.ghost.note_of("array").map(String::from);
    let n = app.sel.len();
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Array and copy").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Array and copy", crate::csd::anchor(app))).show(ctx, |ui| {
        ui.set_width(376.0);
        if crate::csd::dialog_header(ui, "Array and copy", true) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if n == 0 { "Select one or more shapes first.".to_string() } else { format!("{n} shape{} selected.", if n == 1 { "" } else { "s" }) });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.selectable_label(!app.array.circular, "Grid").on_hover_text("Rows and columns").clicked() { app.array.circular = false; }
            if ui.selectable_label(app.array.circular, "Circle").on_hover_text("Around a centre point").clicked() { app.array.circular = true; }
        });
        ui.add_space(6.0);
        let a = &mut app.array;
        let field = |ui: &mut egui::Ui, v: &mut f64, lo: f64, hi: f64, dec: usize, unit: &str| {
            crate::widgets::num(ui, [70.0, 24.0], v, lo, hi, if dec == 0 { 0.1 } else { 0.25 }, dec, "");
            ui.weak(unit);
        };
        egui::Grid::new("array_grid").num_columns(4).spacing([10.0, 8.0]).show(ui, |ui| {
            if !a.circular {
                ui.label("Columns"); field(ui, &mut a.cols, 1.0, 100.0, 0, ""); ui.label("Rows"); field(ui, &mut a.rows, 1.0, 100.0, 0, ""); ui.end_row();
                ui.label("Gap X"); field(ui, &mut a.gap_x, -10000.0, 10000.0, 2, "mm"); ui.label("Gap Y"); field(ui, &mut a.gap_y, -10000.0, 10000.0, 2, "mm"); ui.end_row();
            } else {
                ui.label("Copies"); field(ui, &mut a.count, 2.0, 200.0, 0, ""); ui.label("Angle"); field(ui, &mut a.angle, -360.0, 360.0, 1, "°"); ui.end_row();
                ui.label("Centre X"); field(ui, &mut a.cx, -100000.0, 100000.0, 2, "mm"); ui.label("Centre Y"); field(ui, &mut a.cy, -100000.0, 100000.0, 2, "mm"); ui.end_row();
            }
        });
        if a.circular {
            ui.add_space(4.0);
            ui.checkbox(&mut a.rotate, "Turn the copies").on_hover_text("Off: the copies keep their orientation and only move round the circle");
        }
        if let Some(n) = &pnote { ui.weak(n); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(n > 0, egui::Button::new("Apply")).clicked() { apply = true; }
            if ui.button("Close").clicked() { close = true; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut app.preview_on, "Preview").on_hover_text("Show the result in the drawing while you change the values");
            });
        });
        ui.add_space(2.0);
        ui.weak(if app.array.circular { "Copies includes the original. With 360° they are spaced evenly all the way round." }
                else { "The gap is measured between the edges of neighbouring copies." });
        });
    });
    if apply { app.ghost.clear(); if let Some(msg) = app.array_selection() { app.error = Some(msg); } }
    if close { app.array.open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_rect() -> App {
        let mut a = App::new(None);
        a.project.data.loops.clear();
        a.project.data.loops.push(shapes::new_rect([0.0, 0.0], [10.0, 4.0], 0.0, 0));
        a.sel = [0].into_iter().collect();
        a
    }

    #[test]
    fn grid_makes_rows_and_columns() {
        let mut a = app_with_rect();
        a.array.cols = 3.0; a.array.rows = 2.0; a.array.gap_x = 5.0; a.array.gap_y = 2.0;
        assert!(a.array_selection().is_none());
        assert_eq!(a.project.data.loops.len(), 6);
        assert_eq!(a.sel.len(), 6);
        let b = shapes::bbox_many(a.project.data.loops.iter()).unwrap();
        assert!((b[2] - (3.0 * 10.0 + 2.0 * 5.0)).abs() < 1e-3 && (b[3] - (2.0 * 4.0 + 2.0)).abs() < 1e-3, "{b:?}");
    }

    #[test]
    fn circle_spaces_copies_evenly() {
        let mut a = app_with_rect();
        a.array.circular = true; a.array.count = 4.0; a.array.angle = 360.0; a.array.cx = 5.0; a.array.cy = -20.0; a.array.centre_set = true;
        assert!(a.array_selection().is_none());
        assert_eq!(a.project.data.loops.len(), 4);
        // turned copies are polygons, not rectangles
        assert!(a.project.data.loops[1].rc.is_none());
        // the copy 180 degrees round sits on the other side of the centre
        let b = shapes::bbox(&a.project.data.loops[2]).unwrap();
        assert!(((b[0] + b[2]) / 2.0 - 5.0).abs() < 1e-2 && ((b[1] + b[3]) / 2.0 - (-42.0)).abs() < 1e-2, "{b:?}");
    }

    #[test]
    fn nothing_selected_or_too_many_is_refused() {
        let mut a = app_with_rect();
        a.sel.clear();
        assert!(a.array_selection().is_some());
        a.sel = [0].into_iter().collect();
        a.array.cols = 100.0; a.array.rows = 100.0;
        assert!(a.array_selection().unwrap().contains("limit"));
        assert_eq!(a.project.data.loops.len(), 1);
    }
}
