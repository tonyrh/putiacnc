//! The Align tool: a button in the palette that opens a small panel to align the selection to the stock,
//! align shapes to each other, or distribute them with equal gaps.
use crate::app::App;
use crate::icons::{icon_button, Icon};
use eframe::egui::{self, Vec2};
use putiacnc_core::shapes;

/// Aligns the selection. `each` = false: to the stock (0 left, 1 centre horizontally, 2 right, 3 top, 4 centre
/// vertically, 5 bottom, 6 centre on both). `each` = true: to each other (0..5 the same edges), 6 and 7 distribute
/// horizontally and vertically (three shapes or more).
pub fn align_selection(app: &mut App, each: bool, k: u8) {
    if app.sel.is_empty() || app.sel_locked() { return; }
    let ids: Vec<usize> = app.sel.iter().copied().filter(|&i| i < app.project.data.loops.len()).collect();
    let Some(b) = shapes::bbox_many(ids.iter().map(|&i| &app.project.data.loops[i])) else { return };
    if each && ids.len() < 2 { return; }
    if each && k >= 6 && ids.len() < 3 { return; }
    app.hist.record(&app.project.data);
    let (sw, sh) = (app.project.data.stock.w, app.project.data.stock.h);
    let d = &mut app.project.data;
    if !each {
        let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
        let dx = match k { 0 => -b[0], 1 | 6 => sw / 2.0 - cx, 2 => sw - b[2], _ => 0.0 };
        let dy = match k { 3 => sh - b[3], 4 | 6 => sh / 2.0 - cy, 5 => -b[1], _ => 0.0 };
        for &i in &ids { shapes::translate(&mut d.loops[i], dx, dy); }
    } else if k < 6 {
        for &i in &ids {
            let Some(lb) = shapes::bbox(&d.loops[i]) else { continue };
            let dx = match k { 0 => b[0] - lb[0], 1 => (b[0] + b[2]) / 2.0 - (lb[0] + lb[2]) / 2.0, 2 => b[2] - lb[2], _ => 0.0 };
            let dy = match k { 3 => b[3] - lb[3], 4 => (b[1] + b[3]) / 2.0 - (lb[1] + lb[3]) / 2.0, 5 => b[1] - lb[1], _ => 0.0 };
            shapes::translate(&mut d.loops[i], dx, dy);
        }
    } else {
        // equal gaps between the shapes, keeping the outermost two where they are
        let horiz = k == 6;
        let ax = |bb: [f64; 4]| if horiz { (bb[0], bb[2]) } else { (bb[1], bb[3]) };
        let mut items: Vec<(usize, f64, f64)> = ids.iter().filter_map(|&i| shapes::bbox(&d.loops[i]).map(|bb| { let (lo, hi) = ax(bb); (i, lo, hi) })).collect();
        items.sort_by(|p, q| (p.1 + p.2).partial_cmp(&(q.1 + q.2)).unwrap());
        let (lo, hi) = ax(b);
        let total: f64 = items.iter().map(|t| t.2 - t.1).sum();
        let gap = (hi - lo - total) / (items.len() - 1) as f64;
        let mut pos = lo;
        for (i, l0, l1) in items {
            let delta = pos - l0;
            if horiz { shapes::translate(&mut d.loops[i], delta, 0.0); } else { shapes::translate(&mut d.loops[i], 0.0, delta); }
            pos += (l1 - l0) + gap;
        }
    }
    app.touch();
}

/// The palette button. Greyed out until something is selected (and for a locked drawing).
pub fn button(ui: &mut egui::Ui, app: &mut App) {
    let locked = app.sel_locked();
    let tip = if locked { "Align: this drawing is locked" } else { "Align the selection to the stock or to each other, or distribute shapes evenly" };
    let r = icon_button(ui, Icon::AlignMenu, app.align_open, !app.sel.is_empty() && !locked, tip);
    app.align_btn = Some(r.rect);
    if r.clicked() { app.align_open = !app.align_open; }
}

/// The panel under the button. It stays open so several alignments can follow each other.
pub fn popover(ctx: &egui::Context, app: &mut App) {
    if app.sel.is_empty() || app.sel_locked() { app.align_open = false; }
    if !app.align_open { return; }
    let Some(btn) = app.align_btn else { return };
    let n = app.sel.len();
    let mut pick: Option<(bool, u8)> = None;
    let resp = egui::Area::new(egui::Id::new("align_pop")).order(egui::Order::Foreground).fixed_pos(egui::pos2(btn.left().min(app.canvas_right - 445.0).max(4.0), btn.bottom() + 4.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(8.0, 6.0)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 4.0);
            let row = |ui: &mut egui::Ui, title: &str, items: &[(u8, Icon, &str, bool)], each: bool, pick: &mut Option<(bool, u8)>| {
                ui.horizontal(|ui| {
                    let lw = 120.0;
                    // labels flush right in one column, so the three rows line up
                    let (r, _) = ui.allocate_exact_size(Vec2::new(lw, 36.0), egui::Sense::hover());
                    ui.painter().text(r.right_center(), egui::Align2::RIGHT_CENTER, title, egui::TextStyle::Body.resolve(ui.style()), ui.visuals().text_color());
                    ui.add_space(14.0);
                    for (pos, (k, icon, tip, on)) in items.iter().enumerate() {
                        if pos == 3 || pos == 6 { ui.add_space(6.0); }
                        if icon_button(ui, *icon, false, *on, tip).clicked() { *pick = Some((each, *k)); }
                    }
                });
            };
            row(ui, "To the stock", &[
                (0, Icon::AlignL, "Align to the left edge of the stock", true), (1, Icon::AlignHC, "Centre horizontally on the stock", true), (2, Icon::AlignR, "Align to the right edge of the stock", true),
                (3, Icon::AlignT, "Align to the top edge of the stock", true), (4, Icon::AlignVC, "Centre vertically on the stock", true), (5, Icon::AlignB, "Align to the bottom edge of the stock", true),
                (6, Icon::CenterBoth, "Centre on the stock", true),
            ], false, &mut pick);
            row(ui, if n >= 2 { "To each other" } else { "To each other (2+)" }, &[
                (0, Icon::AlignL, "Align the shapes' left edges", n >= 2), (1, Icon::AlignHC, "Centre the shapes horizontally on each other", n >= 2), (2, Icon::AlignR, "Align the shapes' right edges", n >= 2),
                (3, Icon::AlignT, "Align the shapes' top edges", n >= 2), (4, Icon::AlignVC, "Centre the shapes vertically on each other", n >= 2), (5, Icon::AlignB, "Align the shapes' bottom edges", n >= 2),
            ], true, &mut pick);
            row(ui, if n >= 3 { "Distribute" } else { "Distribute (3+)" }, &[
                (6, Icon::DistH, "Equal gaps between the shapes, left to right", n >= 3), (7, Icon::DistV, "Equal gaps between the shapes, bottom to top", n >= 3),
            ], true, &mut pick);
        });
    });
    if let Some((each, k)) = pick { align_selection(app, each, k); }
    // Esc, or a click outside the panel and its button, closes it
    let (esc, clicked, pos) = ctx.input(|i| (i.key_pressed(egui::Key::Escape), i.pointer.any_pressed(), i.pointer.interact_pos()));
    let outside = pos.map_or(false, |p| !resp.response.rect.contains(p) && !btn.contains(p));
    if (clicked && outside) || (esc && !ctx.wants_keyboard_input()) { app.align_open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app3() -> App {
        let mut a = App::new(None);
        a.project.data.stock.w = 200.0;
        a.project.data.stock.h = 100.0;
        a.project.data.loops = vec![
            shapes::new_rect([10.0, 10.0], [30.0, 20.0], 0.0, 0),
            shapes::new_rect([50.0, 40.0], [60.0, 80.0], 0.0, 0),
            shapes::new_rect([150.0, 5.0], [190.0, 15.0], 0.0, 0),
        ];
        a.sel = (0..3).collect();
        a
    }

    fn boxes(a: &App) -> Vec<[f64; 4]> { a.project.data.loops.iter().map(|l| shapes::bbox(l).unwrap()).collect() }

    #[test]
    fn aligning_to_the_stock_moves_the_selection_as_one() {
        let mut a = app3();
        align_selection(&mut a, false, 6); // centre on the stock
        let b = shapes::bbox_many(a.project.data.loops.iter()).unwrap();
        assert!(((b[0] + b[2]) / 2.0 - 100.0).abs() < 1e-6 && ((b[1] + b[3]) / 2.0 - 50.0).abs() < 1e-6, "{b:?}");
        a.do_undo();
        assert_eq!(boxes(&a)[0], [10.0, 10.0, 30.0, 20.0]);
        align_selection(&mut a, false, 0); // left edge
        assert!((shapes::bbox_many(a.project.data.loops.iter()).unwrap()[0]).abs() < 1e-6);
    }

    #[test]
    fn aligning_to_each_other_and_distributing() {
        let mut a = app3();
        align_selection(&mut a, true, 5); // bottoms
        assert!(boxes(&a).iter().all(|b| (b[1] - 5.0).abs() < 1e-6));
        let mut a = app3();
        align_selection(&mut a, true, 6); // equal gaps horizontally
        let b = boxes(&a);
        let (g1, g2) = (b[1][0] - b[0][2], b[2][0] - b[1][2]);
        assert!((g1 - g2).abs() < 1e-6, "{g1} {g2}");
        // two shapes cannot be distributed, and nothing selected does nothing
        let mut a = app3();
        a.sel = [0usize, 1].into_iter().collect();
        align_selection(&mut a, true, 6);
        assert_eq!(boxes(&a)[0], [10.0, 10.0, 30.0, 20.0]);
        a.sel.clear();
        align_selection(&mut a, false, 6);
        assert_eq!(boxes(&a)[0], [10.0, 10.0, 30.0, 20.0]);
    }
}
