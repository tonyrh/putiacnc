//! Nest dialog: packs the selected shapes onto the stock in rows (see `putiacnc_core::nest`), a group or a placed text as one part.
use putiacnc_core::i18n::{tr, tr_n};
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
    /// Fit the parts by their outlines instead of their boxes.
    pub shape: bool,
    /// Turn in steps of this many degrees (with `turn`), when fitting by outline.
    pub step: f64,
    /// Let other parts go into the holes cut right through a part.
    pub holes: bool,
}

impl Default for NestTool {
    fn default() -> Self { NestTool { open: false, gap: 6.0, margin: 3.0, turn: true, shape: true, step: 90.0, holes: true } }
}

impl App {
    /// Moves (and, where it helps, turns by 90 degrees) the selected parts so they sit in rows on the stock.
    /// Parts keep the gap's distance from the clamps. Parts that do not fit stay where they are. Returns a message when something is worth saying.
    pub fn nest_selection(&mut self) -> Option<String> {
        if self.sel.is_empty() { return Some(tr("Select the shapes to nest first.").into()); }
        if self.sel_locked() { return Some(tr("This drawing is locked.").into()); }
        let units = crate::align::units(self);
        let boxes: Vec<[f64; 4]> = units.iter().filter_map(|u| shapes::bbox_many(u.iter().map(|&i| &self.project.data.loops[i]))).collect();
        if boxes.len() != units.len() { return Some(tr("Nothing to nest.").into()); }
        let (sw, sh) = (self.project.data.stock.w, self.project.data.stock.h);
        let m = self.nest.margin.max(0.0);
        let sheet = [m, m, sw - m, sh - m];
        if sheet[2] <= sheet[0] || sheet[3] <= sheet[1] { return Some(tr("The margin leaves no room on the stock.").into()); }
        let dims: Vec<(f64, f64)> = boxes.iter().map(|b| (b[2] - b[0], b[3] - b[1])).collect();
        let left_out: Vec<usize>;
        let clamps: Vec<[f64; 4]> = self.project.data.clamps.iter().map(|c| [c.x, c.y, c.x + c.w, c.y + c.h]).collect();
        // each part is turned (if it turns) and put so the lower left corner of its box lands on (x, y)
        let moves: Vec<(usize, f64, f64, f64)> = if self.nest.shape {
            let d = &self.project.data;
            let parts: Vec<putiacnc_core::nest::Part> = units.iter().map(|u| {
                let ls: Vec<&putiacnc_core::model::LoopData> = u.iter().map(|&i| &d.loops[i]).collect();
                let closed: Vec<usize> = (0..ls.len()).filter(|&k| ls[k].closed() && ls[k].pts.len() >= 6).collect();
                let mut part = putiacnc_core::nest::Part::default();
                for (k, l) in ls.iter().enumerate() {
                    if !closed.contains(&k) {
                        let b = shapes::bbox(l).unwrap_or([0.0; 4]);
                        part.outline.push(vec![[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]);
                        continue;
                    }
                    // how many other closed shapes of the part contain this one: even = an outline, odd = a hole (or what stands in one)
                    let p0 = shapes::points(l)[0];
                    let depth = closed.iter().filter(|&&j| j != k && shapes::point_in_loop(ls[j], p0)).count();
                    let poly = shapes::points(l);
                    if depth == 0 { part.outline.push(poly); }
                    else if depth % 2 == 1 {
                        // only a hole cut right through is room for another part: an inside profile as deep as the stock is thick
                        let through = self.nest.holes && d.ops.iter().any(|o| o.id == l.op && o.p.kind == "inside" && o.p.depth >= d.stock.t - 1e-6);
                        if through { part.holes.push(poly); }
                    } else if self.nest.holes { part.islands.push(poly); }
                }
                part
            }).collect();
            let step = self.nest.step.clamp(5.0, 180.0);
            let angles: Vec<f64> = if self.nest.turn { (0..(360.0 / step).floor() as usize).map(|k| k as f64 * step).collect() } else { vec![0.0] };
            // the parts are turned about the middle of their box, so turn them there for the fit as well
            let centred: Vec<putiacnc_core::nest::Part> = parts.iter().zip(&boxes).map(|(pt, b)| {
                let c = [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
                let mv = |ps: &Vec<putiacnc_core::nest::Poly>| -> Vec<putiacnc_core::nest::Poly> { ps.iter().map(|p| p.iter().map(|q| [q[0] - c[0], q[1] - c[1]]).collect()).collect() };
                putiacnc_core::nest::Part { outline: mv(&pt.outline), holes: mv(&pt.holes), islands: mv(&pt.islands) }
            }).collect();
            let r = putiacnc_core::nest::nest_shapes(&centred, sheet, self.nest.gap, &angles, &clamps);
            left_out = r.left_out;
            r.placed.iter().map(|p| (p.item, p.angle, p.x, p.y)).collect()
        } else {
            let r = putiacnc_core::nest::nest(&dims, sheet, self.nest.gap, self.nest.turn, &clamps);
            left_out = r.left_out;
            r.placed.iter().map(|p| (p.item, if p.turned { 90.0 } else { 0.0 }, p.x, p.y)).collect()
        };
        if moves.is_empty() { return Some(tr("None of the shapes fits on the stock. Try a smaller gap or margin, or allow turning.").into()); }
        self.hist.record(&self.project.data);
        for &(item, angle, x, y) in &moves {
            let unit: std::collections::BTreeSet<usize> = units[item].iter().copied().collect();
            let b = boxes[item];
            let d = &mut self.project.data;
            if angle != 0.0 {
                let tabs0 = shapes::tab_positions(&d.loops, &d.ops, &unit);
                shapes::rotate_selection(&mut d.loops, &mut d.ops, &unit, [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0], angle, &tabs0);
            }
            let Some(nb) = shapes::bbox_many(unit.iter().map(|&i| &d.loops[i])) else { continue };
            for &i in &unit { shapes::translate(&mut d.loops[i], x - nb[0], y - nb[1]); }
        }
        self.touch();
        let k = left_out.len();
        (k > 0).then(|| tr_n(k as u64, "{n} shape did not fit and was left where it was: allow turning, or use a smaller gap or margin.", "{n} shapes did not fit and were left where they were: allow turning, or use a smaller gap or margin."))
    }
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if !app.nest.open { app.ghost.clear_if("nest"); if app.tool_note.as_ref().is_some_and(|(k, _)| *k == "nest") { app.tool_note = None; } return; }
    {
        let t = &app.nest;
        let params = [t.gap.to_bits(), t.margin.to_bits(), t.turn as u64, t.shape as u64, t.holes as u64, t.step.to_bits()];
        app.preview("nest", &params, |x| x.nest_selection());
    }
    // the window already says what to select; the preview's own message would repeat it
    let pnote = app.ghost.note_of("nest").filter(|_| !app.sel.is_empty()).map(String::from);
    let n = crate::align::units(app).len();
    let (mut apply, mut close) = (false, false);
    egui::Window::new("Nest on the stock").frame(crate::csd::dialog_frame(ctx)).constrain_to(crate::csd::dialog_bounds(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Nest on the stock", crate::csd::anchor(app))).show(ctx, crate::csd::trapped(|ui| {
        ui.set_width(crate::csd::dialog_w(376.0));
        if crate::csd::dialog_header_help(ui, tr("Nest on the stock"), true, |ui| {
            ui.label(tr("Packs the selected shapes onto the stock to use less material, starting from the lower left corner: by their outlines (largest first, a part may go into the space beside or inside another), or in rows of boxes, tallest first. A group or a placed text moves as one part."));
            ui.add_space(4.0);
            ui.label(tr("Leave a gap at least as wide as the cutter. Parts keep the gap's distance from the clamps. Parts that do not fit stay where they are. Finer turning steps fit better and take longer. Holes cut right through a part (an Inside profile as deep as the stock) can take other parts."));
        }) { close = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        ui.label(if n == 0 { tr("Select the shapes to nest first.").to_string() } else { tr_n(n as u64, "{n} part selected.", "{n} parts selected.") });
        ui.add_space(6.0);
        egui::Grid::new("nest_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            let lw = [tr("Gap"), tr("Margin")].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0_f32, f32::max);
            crate::toolmgr::rlabel(ui, lw, tr("Gap"));
            ui.horizontal(|ui| { crate::widgets::len(ui, [70.0, 24.0], &mut app.nest.gap, 0.0, 10000.0, 0.25, 2, ""); ui.weak(crate::widgets::len_unit(ui)); });
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, tr("Margin"));
            ui.horizontal(|ui| { crate::widgets::len(ui, [70.0, 24.0], &mut app.nest.margin, 0.0, 10000.0, 0.25, 2, ""); ui.weak(crate::widgets::len_unit(ui)); });
            ui.end_row();
        });
        ui.add_space(4.0);
        crate::widgets::check(ui, &mut app.nest.shape, tr("Follow the outlines (tighter fit)")).on_hover_text(tr("Fit parts by their real shape, so a small part can go beside a curve or into the corner of an L. Off: rows of boxes."));
        if app.nest.shape { crate::widgets::check(ui, &mut app.nest.holes, tr("Use the holes in parts")).on_hover_text(tr("Other parts may go into a hole that is cut right through a part (an Inside profile as deep as the stock is thick). Holes that are only pocketed or engraved are never used.")); }
        ui.horizontal(|ui| {
            crate::widgets::check(ui, &mut app.nest.turn, if app.nest.shape { tr("Allow turning, in steps of") } else { tr("Allow turning by 90°") });
            if app.nest.shape && app.nest.turn {
                egui::ComboBox::from_id_source("nest_step").width(60.0).selected_text(format!("{}°", app.nest.step)).show_ui(ui, |ui| {
                    for v in [90.0, 45.0, 15.0] { ui.selectable_value(&mut app.nest.step, v, format!("{v}°")); }
                });
            }
        });
        let mine = app.tool_note.as_ref().filter(|(k, _)| *k == "nest").map(|(_, m)| m);
        if let Some(m) = pnote.as_ref().or(mine) { crate::msg::line(ui, crate::app::Level::Warning, m); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(n > 0, egui::Button::new(tr("Apply"))).on_hover_text(tr("Do it and keep the window open, to go again")).clicked() { apply = true; }
            if ui.button(tr("Close")).clicked() { close = true; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::widgets::Tip::tip(crate::widgets::check(ui, &mut app.preview_on, tr("Preview")), tr("Show the result in the drawing while you change the values"));
            });
        });
        });
    }));
    if apply { app.ghost.clear(); app.run_tool("nest", |a| a.nest_selection()); }
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
    fn nested_parts_keep_the_gap_from_a_clamp() {
        let mut a = app_with_parts();
        a.nest.gap = 4.0; a.nest.margin = 2.0; a.nest.turn = false;
        a.project.data.stock.h = 100.0;
        a.project.data.clamps = vec![putiacnc_core::model::Clamp { x: 0.0, y: 0.0, w: 40.0, h: 25.0, height: 20.0 }];
        assert!(a.nest_selection().is_none(), "all three still fit");
        for b in boxes(&a) {
            let on = b[0] < 44.0 - 1e-6 && b[2] > -4.0 && b[1] < 29.0 - 1e-6 && b[3] > -4.0;
            assert!(!on, "{b:?} is on the clamp");
        }
    }

    #[test]
    fn rows_mode_still_packs_boxes() {
        let mut a = app_with_parts();
        a.nest.shape = false; a.nest.gap = 4.0; a.nest.margin = 2.0; a.nest.turn = false;
        assert!(a.nest_selection().is_none());
        let b = boxes(&a);
        // tallest first in the lower left corner of the margin
        assert_eq!([b[2][0], b[2][1]], [2.0, 2.0]);
    }

    #[test]
    fn by_outline_two_l_shapes_interlock_and_keep_the_gap() {
        let mut a = App::new(None);
        a.project.data.stock.w = 100.0;
        a.project.data.stock.h = 100.0;
        let ell = |ox: f64| shapes::new_poly(&[[ox, 0.0], [ox + 60.0, 0.0], [ox + 60.0, 20.0], [ox + 20.0, 20.0], [ox + 20.0, 60.0], [ox, 60.0]], true, 0).unwrap();
        a.project.data.loops = vec![ell(200.0), ell(300.0)];
        a.sel = [0, 1].into_iter().collect();
        a.nest.gap = 4.0; a.nest.margin = 0.0; a.nest.turn = true; a.nest.step = 90.0;
        assert!(a.nest_selection().is_none(), "both fit");
        let to = |l: &putiacnc_core::model::LoopData| -> Vec<[i64; 2]> { let mut v = l.path(); if putiacnc_core::clip::area(&v) < 0.0 { v.reverse(); } v };
        let (p, q) = (to(&a.project.data.loops[0]), to(&a.project.data.loops[1]));
        let wide = putiacnc_core::clip::offset(std::slice::from_ref(&p), 3900.0, 50.0);
        assert!(putiacnc_core::clip::boolean(putiacnc_core::clip::BoolKind::Intersect, &wide, &[q]).is_empty(), "closer than the gap");
        for b in boxes(&a) { assert!(b[0] >= -1e-6 && b[1] >= -1e-6 && b[2] <= 100.0 + 1e-6 && b[3] <= 100.0 + 1e-6, "{b:?}"); }
    }

    #[test]
    fn a_part_goes_into_a_hole_cut_right_through_another() {
        let mut a = App::new(None);
        a.project.data.stock.w = 100.0; a.project.data.stock.h = 100.0; a.project.data.stock.t = 6.0;
        // a plate with a hole (grouped as one part), and a small square far away
        a.project.data.loops = vec![
            shapes::new_rect([200.0, 200.0], [290.0, 290.0], 0.0, 0),
            shapes::new_rect([220.0, 220.0], [270.0, 270.0], 0.0, 0),
            shapes::new_rect([400.0, 400.0], [430.0, 430.0], 0.0, 0),
        ];
        for l in a.project.data.loops.iter_mut().take(2) { l.ug = Some(1); }
        shapes::add_operation(&mut a.project.data, &[1].into_iter().collect(), "inside");
        let op = a.project.data.ops.last_mut().expect("an operation");
        op.p.kind = "inside".into(); op.p.depth = 6.0;
        let id = op.id;
        a.project.data.loops[1].op = id;
        a.sel = (0..3).collect();
        a.nest.gap = 4.0; a.nest.margin = 0.0; a.nest.turn = false;
        assert!(a.nest_selection().is_none(), "all fit: the square goes into the hole");
        let b = boxes(&a);
        assert!(b[2][0] >= b[1][0] + 4.0 - 1e-6 && b[2][2] <= b[1][2] - 4.0 + 1e-6 && b[2][1] >= b[1][1] + 4.0 - 1e-6 && b[2][3] <= b[1][3] - 4.0 + 1e-6, "{b:?}");
        // with the tick off, or a hole that is not cut through, it does not fit
        a.do_undo();
        a.nest.holes = false;
        let msg = a.nest_selection().expect("a part is left out");
        assert!(msg.contains("did not fit"), "{msg}");
    }

    #[test]
    fn nothing_selected_is_refused() {
        let mut a = app_with_parts();
        a.sel.clear();
        assert!(a.nest_selection().is_some());
    }

    #[test]
    fn the_preview_shows_the_nesting_and_leaves_the_drawing_alone() {
        let mut a = app_with_parts();
        a.project.data.loops.push(shapes::new_rect([0.0, 0.0], [300.0, 10.0], 0.0, 0));
        a.sel = (0..4).collect();
        let (before, undo) = (a.project.data.clone(), a.hist.undo.len());
        a.preview("nest", &[1], |x| x.nest_selection());
        assert!(a.project.data == before, "the drawing is untouched");
        assert_eq!(a.hist.undo.len(), undo);
        assert!(!a.ghost.loops.is_empty(), "the moved parts are shown");
        assert!(a.ghost.note_of("nest").is_some_and(|m| m.contains("did not fit")), "the preview says what will not fit");
        // the preview shows what apply then does
        let shown: Vec<_> = a.ghost.loops.iter().map(|l| shapes::bbox(l).unwrap()).collect();
        a.ghost.clear();
        a.nest_selection();
        for r in shown { assert!(boxes(&a).contains(&r), "{r:?}"); }
    }
}
