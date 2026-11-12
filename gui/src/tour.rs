//! The quick tour: a few steps that dim the window, outline one part of it and say what it is for. It runs from the end of the setup
//! assistant and from Help → Quick tour. It changes nothing in the project: it only shows panels that are hidden and the drawing tab.
use putiacnc_core::i18n::{n_, tr, tr_f};
use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::collections::HashMap;

#[derive(Default)]
pub struct Tour {
    pub open: bool,
    pub step: usize,
}

impl Tour {
    pub fn start(&mut self) { self.open = true; self.step = 0; }
}

struct Step { target: Option<&'static str>, title: &'static str, body: &'static str }

const STEPS: [Step; 8] = [
    Step { target: None, title: n_("Welcome to PutiaCNC"), body: n_("A two-minute look at where things are. Use Next, or press Esc to leave at any time. Help \u{2192} Quick tour shows it again.") },
    Step { target: Some("tools"), title: n_("Draw or import"), body: n_("Draw lines, rectangles, circles, polygons and text here, or bring in an SVG, DXF, bitmap or 3D model from the File menu. Select shapes to move, resize or edit their points, and right-click a selection for Mirror, Move to layer, Fillet and more. The Draw, Modify and Generate menus hold the rest (arrays, offsets, boxes, finger joints).") },
    Step { target: Some("left"), title: n_("Your material"), body: n_("Set the size and thickness of the stock, where zero is, and add clamps for the places the tool must keep out of. Layers (hide, lock, colour) and the imported drawings and text are listed below.") },
    Step { target: Some("right"), title: n_("Make operations"), body: n_("Select shapes in the drawing, then add an operation: Profile cuts along or round them, Pocket clears the inside, V-carve engraves, Drill makes holes, and 3D relief carves a model or shapes you raised to a height. Each operation has its own tool, depth and options.") },
    Step { target: Some("tabs"), title: n_("2D, 3D and split"), body: n_("Switch between the drawing, a 3D preview and a split view. The preview replays the real G-code, so you see what the machine will do before it does it.") },
    Step { target: Some("status"), title: n_("Time and checks"), body: n_("The estimated time and the cut length are here. Warnings about the toolpath (too deep, a clamp in the way, a part too small for the tool) appear in a bar under the view tabs.") },
    Step { target: Some("right"), title: n_("G-code"), body: n_("The G-code tab shows the program. File \u{2192} Export G-code\u{2026} saves it, split per tool or per operation if you like. Do an air cut first.") },
    Step { target: None, title: n_("That's it"), body: n_("Help \u{2192} Keyboard shortcuts (F1) lists the keys. The setup assistant and this tour are both in the Help menu.") },
];

fn key() -> egui::Id { egui::Id::new("tour_rects") }

/// Called where a part of the window is drawn, so the tour can outline it.
pub fn note(ctx: &egui::Context, name: &'static str, rect: Rect) {
    ctx.data_mut(|d| { let mut m: HashMap<&'static str, Rect> = d.get_temp(key()).unwrap_or_default(); m.insert(name, rect); d.insert_temp(key(), m); });
}

fn rect_of(ctx: &egui::Context, name: &str) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<HashMap<&'static str, Rect>>(key())).and_then(|m| m.get(name).copied())
}

pub const CARD_W: f32 = 340.0;
pub fn card_id() -> egui::Id { egui::Id::new("tour_card") }

/// Where the card goes: beside the outlined part on the side with room, on the screen.
fn place(screen: Rect, target: Option<Rect>, size: Vec2) -> Pos2 {
    let gap = 18.0;
    let clamp = |p: Pos2| Pos2::new(p.x.clamp(screen.min.x + 12.0, (screen.max.x - size.x - 12.0).max(screen.min.x + 12.0)), p.y.clamp(screen.min.y + 12.0, (screen.max.y - size.y - 12.0).max(screen.min.y + 12.0)));
    let Some(t) = target else { return clamp(screen.center() - size / 2.0) };
    // beside it if there is room, else under or over it
    if t.max.x + gap + size.x <= screen.max.x - 12.0 { return clamp(Pos2::new(t.max.x + gap, t.center().y - size.y / 2.0)); }
    if t.min.x - gap - size.x >= screen.min.x + 12.0 { return clamp(Pos2::new(t.min.x - gap - size.x, t.center().y - size.y / 2.0)); }
    if t.max.y + gap + size.y <= screen.max.y - 12.0 { return clamp(Pos2::new(t.center().x - size.x / 2.0, t.max.y + gap)); }
    clamp(Pos2::new(t.center().x - size.x / 2.0, t.min.y - gap - size.y))
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    if !app.tour.open || app.setup.open { return; }
    let screen = ctx.screen_rect();
    let step = app.tour.step.min(STEPS.len() - 1);
    let s = &STEPS[step];
    // what the step points at must be on screen
    if step == 0 { app.show_left = true; app.show_right = true; app.tab = crate::app::RightTab::Ops; app.view_tab = crate::view3d::ViewTab::Design; }
    if s.target == Some("tools") { app.view_tab = crate::view3d::ViewTab::Design; }
    if s.target == Some("right") { app.tab = if step == 6 { crate::app::RightTab::Gcode } else { crate::app::RightTab::Ops }; }
    let target = s.target.and_then(|n| rect_of(ctx, n)).map(|r| r.intersect(screen));
    let acc = app.theme.accent;
    // the dimming: one area over everything, taking the clicks, with the outlined part left clear
    egui::Area::new(egui::Id::new("tour_dim")).order(egui::Order::Foreground).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
        ui.allocate_rect(screen, egui::Sense::click_and_drag());
        let p = ui.painter();
        let dim = Color32::from_black_alpha(140);
        match target {
            Some(t) => {
                let t = t.expand(6.0);
                p.rect_filled(Rect::from_min_max(screen.min, Pos2::new(screen.max.x, t.min.y)), 0.0, dim);
                p.rect_filled(Rect::from_min_max(Pos2::new(screen.min.x, t.max.y), screen.max), 0.0, dim);
                p.rect_filled(Rect::from_min_max(Pos2::new(screen.min.x, t.min.y), Pos2::new(t.min.x, t.max.y)), 0.0, dim);
                p.rect_filled(Rect::from_min_max(Pos2::new(t.max.x, t.min.y), Pos2::new(screen.max.x, t.max.y)), 0.0, dim);
                p.rect_stroke(t, 8.0, egui::Stroke::new(2.5_f32, acc));
            }
            None => { p.rect_filled(screen, 0.0, dim); }
        }
    });
    let (mut next, mut back, mut close) = (false, false, false);
    let size_hint = ctx.memory(|m| m.area_rect(card_id())).map_or(Vec2::new(CARD_W, 200.0), |r| r.size());
    egui::Area::new(card_id()).order(egui::Order::Foreground).fixed_pos(place(screen, target, size_hint)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(16.0).rounding(12.0).show(ui, |ui| {
            ui.set_width(CARD_W);
            ui.label(egui::RichText::new(tr_f("{n} of {total}", &[("n", &(step + 1)), ("total", &STEPS.len())])).small().weak());
            ui.add_space(2.0);
            ui.label(egui::RichText::new(tr(s.title)).size(19.0).strong());
            ui.add_space(6.0);
            ui.add(egui::Label::new(tr(s.body)).wrap(true));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if step + 1 < STEPS.len() && ui.button(tr("Skip tour")).clicked() { close = true; }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let last = step + 1 >= STEPS.len();
                    let go = egui::Button::new(egui::RichText::new(if last { tr("  Done  ") } else { tr("  Next \u{2192}  ") }).color(Color32::WHITE).strong()).fill(acc).min_size(Vec2::new(86.0, 28.0));
                    if ui.add(go).clicked() { if last { close = true; } else { next = true; } }
                    if step > 0 && ui.add(egui::Button::new(tr("\u{2190} Back")).min_size(Vec2::new(64.0, 28.0))).clicked() { back = true; }
                });
            });
        });
    });
    // the card must stay above the dimming layer, which comes to the front whenever it is clicked
    ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, card_id()));
    let keys = ctx.input(|i| (i.key_pressed(egui::Key::Escape), i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::ArrowLeft)));
    if keys.0 { close = true; }
    if keys.1 { next = true; }
    if keys.2 { back = true; }
    if next { if step + 1 >= STEPS.len() { close = true; } else { app.tour.step = step + 1; } }
    if back { app.tour.step = step.saturating_sub(1); }
    if close { app.tour.open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every step of the tour finds the part it points at, and its card is on the screen and not over that part.
    #[test]
    fn every_step_points_at_something_and_the_card_fits() {
        let mut problems = vec![];
        for (sw, sh) in [(1500.0_f32, 1000.0_f32), (1354.0, 640.0)] {
            for step in 0..STEPS.len() {
                let mut a = crate::audit::project_for_tests();
                a.tour.start();
                a.tour.step = step;
                let ctx = egui::Context::default();
                for i in 0..10 {
                    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(sw, sh))), time: Some(0.1 * (i + 1) as f64), ..Default::default() };
                    let _ = ctx.run(input, |ctx| a.ui_frame(ctx));
                    a.tour.step = step; // Enter / arrows are not pressed, but keep it fixed
                }
                let scr = Rect::from_min_size(Pos2::ZERO, egui::vec2(sw, sh));
                let card = ctx.memory(|m| m.area_rect(card_id()));
                let Some(card) = card else { problems.push(format!("{sw}x{sh} step {step}: no card")); continue };
                if !scr.contains_rect(card) { problems.push(format!("{sw}x{sh} step {step}: the card {card:?} leaves the screen")); }
                if let Some(name) = STEPS[step].target {
                    match rect_of(&ctx, name) {
                        None => problems.push(format!("{sw}x{sh} step {step}: nothing was noted for \"{name}\"")),
                        Some(t) => if t.width() < 20.0 || t.height() < 10.0 { problems.push(format!("{sw}x{sh} step {step}: \"{name}\" is {t:?}")); } else if card.intersects(t.shrink(2.0)) && t.width() < sw * 0.6 { problems.push(format!("{sw}x{sh} step {step}: the card covers \"{name}\"")); },
                    }
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// The card's buttons can really be clicked (the dimming layer must not swallow the clicks).
    #[test]
    fn card_buttons_can_be_clicked() {
        let scr = Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 860.0));
        let mut a = crate::audit::project_for_tests();
        a.tour.start();
        let ctx = egui::Context::default();
        let mut t = 0.0;
        let mut frame = |a: &mut App, events: Vec<egui::Event>| {
            t += 0.1;
            let input = egui::RawInput { screen_rect: Some(scr), time: Some(t), events, ..Default::default() };
            let _ = ctx.run(input, |ctx| a.ui_frame(ctx));
        };
        for _ in 0..6 { frame(&mut a, vec![]); }
        for want in 1..3 {
            let card = ctx.memory(|m| m.area_rect(card_id())).expect("card");
            let pos = Pos2::new(card.max.x - 50.0, card.max.y - 30.0); // the Next button
            let click = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            frame(&mut a, vec![egui::Event::PointerMoved(pos)]);
            frame(&mut a, vec![click(true)]);
            frame(&mut a, vec![click(false)]);
            for _ in 0..4 { frame(&mut a, vec![]); }
            assert_eq!(a.tour.step, want, "clicking Next did not advance the tour");
        }
    }
}
