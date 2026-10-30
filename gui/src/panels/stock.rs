//! The Stock and Clamps cards of the project panel.
use crate::widgets::Tip;
use super::*;
use putiacnc_core::model::ProjectData;

/// The Stock card. Returns the width it was laid out in and the width of its frame, which the Clamps card matches.
pub(super) fn stock_card(ui: &mut Ui, d: &mut ProjectData, changed: &mut bool) -> (f32, f32) {
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
                *changed |= dv_w(ui, fw, &mut d.stock.w, 1.0, 10000.0, ""); ui.weak("mm");
                col4_x = ui.next_widget_position().x;
                rlabel(ui, l3, "Height");
                *changed |= dv_w(ui, fw, &mut d.stock.h, 1.0, 10000.0, ""); ui.weak("mm"); ui.end_row();
                rlabel(ui, l1, "Thickness");
                *changed |= dv_w(ui, fw, &mut d.stock.t, 0.1, 1000.0, ""); ui.weak("mm");
                ui.end_row();
            });
            ui.add_space(12.0);
            ui.horizontal_top(|ui| {
                rlabel(ui, l1, "XY zero");
                *changed |= xy_picker(ui, &mut d.stock.zr);
                // the Z block lines up with the Height / Safe Z column above
                let gap = col4_x - ui.next_widget_position().x - 3.0 * ui.spacing().item_spacing.x;
                ui.add_space(gap.max(6.0));
                egui::Grid::new("stock_z").num_columns(2).spacing([8.0, 10.0]).show(ui, |ui| {
                    let lz = l3;
                    rlabel(ui, lz, "Z zero");
                    let zm_w = crate::widgets::combo_fit(ui, &ZMODES.iter().map(|(_, l)| *l).collect::<Vec<_>>()) + 10.0;
                    let (c, _) = combo_w2(ui, zm_w, "zm", &mut d.stock.zm, ZMODES, "", "Where Z0 is measured from");
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
                rlabel(ui, l1, "Appearance");
                if !MATERIALS.iter().any(|(k, _)| *k == d.stock.mat) { d.stock.mat = "wood".into(); }
                // as wide as the longest name, so the box does not change size with the choice
                let mat_w = crate::widgets::combo_fit(ui, &MATERIALS.iter().map(|(_, l)| *l).collect::<Vec<_>>()) + 10.0;
                let (c, _) = combo_w2(ui, mat_w, "stock_mat", &mut d.stock.mat, MATERIALS, "", "How the stock looks in the drawing and the 3D preview");
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
            ui.heading("Clamps");
            let g = ui.painter().layout_no_wrap("keep-out zones".to_string(), egui::TextStyle::Body.resolve(ui.style()), ui.visuals().weak_text_color());
            let (r, resp) = ui.allocate_exact_size(egui::vec2(g.size().x, hg), egui::Sense::hover());
            ui.painter().galley(egui::pos2(r.min.x, r.max.y - g.size().y), g, ui.visuals().weak_text_color());
            resp.tip("Rectangles on the table that the tool must not touch. X and Y are measured from the stock's lower-left corner; Width is the size along X, Length along Y, and Rise is how far the clamp stands above the stock top. The toolpath warnings check them");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tip = if *show_clamps { "Clamps are drawn in the drawing and the 3D preview. Click to hide them (the toolpath warnings still check them)" } else { "Clamps are hidden in the drawing and the 3D preview. Click to show them" };
                if small_icon_button(ui, if *show_clamps { Icon::Eye } else { Icon::EyeOff }, true, tip).clicked() { *show_clamps = !*show_clamps; }
            });
        });
        let clamp_inner_w = frame_w - 22.0;
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
                    rlabel(ui, lc1, "X"); *changed |= dv_w(ui, fw, &mut c.x, -10000.0, 10000.0, ""); ui.weak("mm");
                    rlabel(ui, lc2, "Y"); *changed |= dv_w(ui, fw, &mut c.y, -10000.0, 10000.0, ""); ui.weak("mm"); ui.end_row();
                    rlabel(ui, lc1, "Width"); *changed |= dv_w(ui, fw, &mut c.w, 0.1, 10000.0, ""); ui.weak("mm");
                    rlabel(ui, lc2, "Length"); *changed |= dv_w(ui, fw, &mut c.h, 0.1, 10000.0, ""); ui.weak("mm"); ui.end_row();
                    rlabel(ui, lc1, "Rise"); *changed |= dv_w(ui, fw, &mut c.height, 0.1, 10000.0, ""); ui.weak("mm");
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
                if icon_text_button(ui, Icon::Add, "Add clamp", "A clamp on the left edge, 20 mm tall; edit X, Y, width (X size), length (Y size) and rise (how far it stands above the stock top), all in mm", None).clicked() {
                    d.clamps.push(putiacnc_core::model::Clamp { x: -30.0, y: d.stock.h / 2.0 - 15.0, w: 30.0, h: 30.0, height: 20.0 });
                    *changed = true;
                }
            });
        });

}
