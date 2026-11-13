//! The Stock and Clamps cards of the project panel.
use putiacnc_core::i18n::{tr, tr_f};
use crate::widgets::Tip;
use super::*;
use putiacnc_core::model::ProjectData;

/// The Stock card. Returns the width it was laid out in and the width of its frame, which the Clamps card matches.
pub(super) fn stock_card(ui: &mut Ui, d: &mut ProjectData, changed: &mut bool) -> (f32, f32) {
        // ---- stock -------------------------------------------------------------------
        ui.separator();
        ui.heading(tr("Stock"));
        let sp = ui.spacing().item_spacing.x;
        let font = egui::TextStyle::Body.resolve(ui.style());
        let text_w = |ui: &Ui, s: &str| ui.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x);
        let stock_w = ui.available_width();
        let _ = (&text_w, sp);
        let stock_frame = egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
            ui.set_max_width(stock_w - 34.0);
            let l1 = tw(ui, tr("Thickness")).max(tw(ui, tr("XY zero"))).max(tw(ui, tr("Appearance"))).max(tw(ui, tr("Material")));
            let l3 = tw(ui, tr("Height")).max(tw(ui, tr("Z zero")));
            // where the Z block starts when it is not pushed right (kept from the last frame), so the Height label can be widened to line up with it
            let nat_id = egui::Id::new("stock_z_nat");
            let nat: f32 = ui.data(|x| x.get_temp(nat_id)).unwrap_or(0.0);
            let left0 = ui.min_rect().left();
            // the number boxes shrink when the labels are long, so the row never passes the card
            let unit_w = tw(ui, crate::widgets::len_unit(ui)).max(tw(ui, "mm"));
            let fw = ((stock_w - 34.0 - l1 - l3 - 2.0 * unit_w - 5.0 * 8.0 - 8.0) / 2.0).clamp(36.0, 62.0);
            let mut col4_x = 0.0_f32;
            egui::Grid::new("stock_grid").num_columns(6).spacing([8.0, 10.0]).show(ui, |ui| {
                rlabel(ui, l1, tr("Width"));
                *changed |= dl_w(ui, fw, &mut d.stock.w, 1.0, 10000.0, ""); ui.weak(crate::widgets::len_unit(ui));
                col4_x = ui.next_widget_position().x;
                let lh = l3 + (nat - (col4_x - left0)).max(0.0);
                rlabel(ui, lh, tr("Height"));
                *changed |= dl_w(ui, fw, &mut d.stock.h, 1.0, 10000.0, ""); ui.weak(crate::widgets::len_unit(ui)); ui.end_row();
                rlabel(ui, l1, tr("Thickness"));
                *changed |= dl_w(ui, fw, &mut d.stock.t, 0.1, 1000.0, ""); ui.weak(crate::widgets::len_unit(ui));
                ui.end_row();
            });
            ui.add_space(12.0);
            ui.horizontal_top(|ui| {
                rlabel(ui, l1, tr("XY zero"));
                *changed |= xy_picker(ui, &mut d.stock.zr);
                // the Z block lines up with the Height / Safe Z column above
                let gap = col4_x - ui.next_widget_position().x - 3.0 * ui.spacing().item_spacing.x;
                ui.add_space(gap.max(6.0));
                // the natural start of the Z block (relative to the card), for the next frame
                let here = ui.next_widget_position().x - left0 + ui.spacing().item_spacing.x;
                let want = if gap >= 6.0 { 0.0 } else { here };
                if (want - nat).abs() > 0.5 { ui.data_mut(|x| x.insert_temp(nat_id, want)); ui.ctx().request_repaint(); }
                egui::Grid::new("stock_z").num_columns(2).spacing([8.0, 10.0]).show(ui, |ui| {
                    let lz = l3;
                    rlabel(ui, lz, tr("Z zero"));
                    let zm_w = (crate::widgets::combo_fit(ui, &ZMODES.iter().map(|(_, l)| *l).collect::<Vec<_>>()) + 10.0).min((stock_w - 34.0 - (ui.next_widget_position().x - ui.min_rect().left()) - l3 - 24.0).max(60.0));
                    let (c, _) = combo_w2(ui, zm_w, "zm", &mut d.stock.zm, ZMODES, "", tr("Where Z0 is measured from"));
                    *changed |= c; ui.end_row();
                    if d.stock.zm != "top" {
                        rlabel(ui, lz, "Z0");
                        ui.horizontal(|ui| {
                            *changed |= dv_w(ui, fw, &mut d.stock.zh, 0.0, 1000.0, "");
                            ui.weak("mm");
                        });
                        ui.end_row();
                    }
                });
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                rlabel(ui, l1, tr("Appearance"));
                if !MATERIALS.iter().any(|(k, _)| *k == d.stock.mat) { d.stock.mat = "wood".into(); }
                // as wide as the longest name, so the box does not change size with the choice
                let mat_w = crate::widgets::combo_fit(ui, &MATERIALS.iter().map(|(_, l)| *l).collect::<Vec<_>>()) + 10.0;
                let (c, _) = combo_w2(ui, mat_w, "stock_mat", &mut d.stock.mat, MATERIALS, "", tr("How the stock looks in the drawing and the 3D preview"));
                *changed |= c;
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                rlabel(ui, l1, tr("Material"));
                let mut opts: Vec<(&str, &str)> = vec![("", tr("Not set"))];
                opts.extend(putiacnc_core::feeds::MATERIALS.iter().map(|m| (m.id, m.label)));
                if !opts.iter().any(|(k, _)| *k == d.stock.cut) { d.stock.cut.clear(); }
                let w = crate::widgets::combo_fit(ui, &opts.iter().map(|(_, l)| *l).collect::<Vec<_>>()) + 10.0;
                let (c, _) = combo_w2(ui, w, "stock_cut", &mut d.stock.cut, &opts, "", tr("What you are cutting. An operation's Suggest button uses it to fill in spindle speed, feeds, stepover and depth per pass. The stock's look changes to match; change Appearance afterwards if you like"));
                // choosing a material gives the stock the matching look
                if c { if let Some(m) = putiacnc_core::feeds::material(&d.stock.cut) { d.stock.mat = m.look.into(); } }
                *changed |= c;
            });
        });

    (stock_w, stock_frame.response.rect.width())
}

/// The Clamps card: keep-out zones on the table. `stock_w` and `frame_w` come from `stock_card`, so both cards line up.
pub(super) fn clamps_card(ui: &mut Ui, d: &mut ProjectData, show_clamps: &mut bool, stock_w: f32, frame_w: f32, changed: &mut bool) {
        // ---- clamps ------------------------------------------------------------------
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            // the small text sits on the heading's baseline instead of floating at its middle
            let hg = ui.fonts(|f| f.row_height(&egui::TextStyle::Heading.resolve(ui.style())));
            ui.heading(tr("Clamps"));
            let g = ui.painter().layout_no_wrap(tr("keep-out zones").to_string(), egui::TextStyle::Body.resolve(ui.style()), ui.visuals().weak_text_color());
            let (r, resp) = ui.allocate_exact_size(egui::vec2(g.size().x, hg), egui::Sense::hover());
            ui.painter().galley(egui::pos2(r.min.x, r.max.y - g.size().y), g, ui.visuals().weak_text_color());
            resp.tip(tr("Rectangles on the table that the tool must not touch. X and Y are measured from the stock's lower-left corner; Width is the size along X, Length along Y, and Rise is how far the clamp stands above the stock top. The toolpath warnings check them"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tip = if *show_clamps { tr("Clamps are drawn in the drawing and the 3D preview. Click to hide them (the toolpath warnings still check them)") } else { tr("Clamps are hidden in the drawing and the 3D preview. Click to show them") };
                if small_icon_button(ui, if *show_clamps { Icon::Eye } else { Icon::EyeOff }, true, tip).clicked() { *show_clamps = !*show_clamps; }
            });
        });
        let clamp_inner_w = frame_w - 22.0;
        egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
            ui.set_max_width(stock_w - 34.0);
            ui.set_min_width(clamp_inner_w);
            let mut del = None;
            if d.clamps.is_empty() { ui.add(egui::Label::new(egui::RichText::new(tr("None. Add one for each clamp or screw on the table.")).weak()).wrap(true)); }
            let n_clamps = d.clamps.len();
            let fw = 62.0_f32;
            let (lc1, lc2) = (tw(ui, tr("Width")).max(tw(ui, tr("Rise"))).max(tw(ui, "X")), tw(ui, tr("Length")).max(tw(ui, "Y")));
            // which clamps are folded away (a view setting: kept for the session, not in the project)
            let fold_key = egui::Id::new("clamp_folded");
            let mut folded: Vec<bool> = ui.data(|x| x.get_temp(fold_key)).unwrap_or_default();
            folded.resize(n_clamps, false);
            for (i, c) in d.clamps.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    if small_icon_button(ui, if folded[i] { Icon::ChevR } else { Icon::ChevD }, true, if folded[i] { tr("Show this clamp's settings") } else { tr("Hide this clamp's settings") }).clicked() { folded[i] = !folded[i]; }
                    ui.label(egui::RichText::new(tr_f("Clamp {n}", &[("n", &(i + 1))])).strong());
                    if folded[i] { ui.weak(tr_f("X {x} · Y {y} · {w} × {h} · rise {rise}", &[("x", &fnum(c.x)), ("y", &fnum(c.y)), ("w", &fnum(c.w)), ("h", &fnum(c.h)), ("rise", &fnum(c.height))])); }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if small_icon_button(ui, Icon::Trash, true, tr("Remove this clamp")).clicked() { del = Some(i); }
                    });
                });
                if !folded[i] {
                egui::Grid::new(("clamp_grid", i)).num_columns(6).spacing([8.0, 8.0]).show(ui, |ui| {
                    rlabel(ui, lc1, "X"); *changed |= dv_w(ui, fw, &mut c.x, -10000.0, 10000.0, ""); ui.weak("mm");
                    rlabel(ui, lc2, "Y"); *changed |= dv_w(ui, fw, &mut c.y, -10000.0, 10000.0, ""); ui.weak("mm"); ui.end_row();
                    rlabel(ui, lc1, tr("Width")); *changed |= dv_w(ui, fw, &mut c.w, 0.1, 10000.0, ""); ui.weak("mm");
                    rlabel(ui, lc2, tr("Length")); *changed |= dv_w(ui, fw, &mut c.h, 0.1, 10000.0, ""); ui.weak("mm"); ui.end_row();
                    rlabel(ui, lc1, tr("Rise")); *changed |= dv_w(ui, fw, &mut c.height, 0.1, 10000.0, ""); ui.weak("mm");
                    ui.end_row();
                });
                }
                if i + 1 < n_clamps { ui.add_space(4.0); ui.separator(); ui.add_space(4.0); }
            }
            if let Some(i) = del { if i < folded.len() { folded.remove(i); } }
            ui.data_mut(|x| x.insert_temp(fold_key, folded));
            ui.add_space(if n_clamps > 0 { 8.0 } else { 4.0 });
            if let Some(i) = del { d.clamps.remove(i); *changed = true; }
            ui.horizontal(|ui| {
                if icon_text_button(ui, Icon::Add, tr("Add clamp"), tr("A clamp on the left edge, 20 mm tall; edit X, Y, width (X size), length (Y size) and rise (how far it stands above the stock top), all in mm"), None).clicked() {
                    d.clamps.push(putiacnc_core::model::Clamp { x: -30.0, y: d.stock.h / 2.0 - 15.0, w: 30.0, h: 30.0, height: 20.0 });
                    *changed = true;
                }
            });
        });

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_material_has_a_look_in_the_appearance_list_and_the_list_has_no_repeats() {
        for m in putiacnc_core::feeds::MATERIALS.iter() {
            assert!(MATERIALS.iter().any(|(k, _)| *k == m.look), "{} has look {:?}", m.id, m.look);
        }
        for (i, (k, _)) in MATERIALS.iter().enumerate() { assert!(MATERIALS.iter().skip(i + 1).all(|(o, _)| o != k), "{k} twice"); }
        // each look has colours of its own in both themes (an unknown name would read as light wood)
        for dark in [false, true] {
            let th = crate::theme::Theme { dark, accent: eframe::egui::Color32::BLUE, size: 14.0 };
            let wood = th.material("wood").top;
            for (k, _) in MATERIALS.iter().filter(|(k, _)| *k != "wood") { assert_ne!(th.material(k).top, wood, "{k} looks like light wood (dark: {dark})"); }
        }
    }
}
