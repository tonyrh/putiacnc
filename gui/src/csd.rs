//! Client-side decorations: our own header bar (menus, title, window buttons), window dragging
//! and edge resizing, so the window looks like a GNOME/libadwaita app on Wayland and X11 alike.
use crate::app::App;
use eframe::egui::{self, Color32, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, ViewportCommand};
use egui::viewport::ResizeDirection;

pub const RADIUS: f32 = 12.0;
/// Header bar content height (a bit taller than a plain menu bar, like a GNOME header bar).
pub const HEIGHT: f32 = 34.0;
/// Height of the header bar content.
pub const HEADER_H: f32 = 36.0;

pub fn is_maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false))
}

/// Corner rounding for the outer bars (none when maximized).
pub fn corners(ctx: &egui::Context, top: bool) -> egui::Rounding {
    if is_maximized(ctx) { return egui::Rounding::ZERO; }
    if top { egui::Rounding { nw: RADIUS, ne: RADIUS, sw: 0.0, se: 0.0 } } else { egui::Rounding { nw: 0.0, ne: 0.0, sw: RADIUS, se: RADIUS } }
}

/// The header bar contents: menus on the left, title in the middle, window buttons on the right.
pub fn header(ui: &mut Ui, app: &mut App) {
    let ctx = ui.ctx().clone();
    ui.set_min_height(HEIGHT);
    let rect = ui.max_rect();
    let maximized = is_maximized(&ctx);

    // empty header space drags the window; double click maximizes
    let drag = ui.interact(rect, egui::Id::new("header_drag"), Sense::click_and_drag());
    if drag.drag_started_by(egui::PointerButton::Primary) { ctx.send_viewport_cmd(ViewportCommand::StartDrag); }
    if drag.double_clicked() { ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized)); }

    // title, centred
    let name = if app.project.name.trim().is_empty() { "Untitled" } else { app.project.name.trim() };
    let title = format!("{}{} - PutiaCNC", name, if app.modified { " •" } else { "" });
    let font = egui::TextStyle::Body.resolve(ui.style());
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, title, font, ui.visuals().strong_text_color());

    // window buttons
    let bw = 30.0;
    let ctl = Rect::from_min_max(Pos2::new(rect.max.x - 3.0 * bw - 8.0 - 2.0 * 30.0 - 14.0, rect.min.y), Pos2::new(rect.max.x, rect.min.y + HEADER_H));
    ui.allocate_ui_at_rect(ctl, |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if win_button(ui, WinIcon::Close, "Close") { app.request(crate::app::Action::Quit); }
            if win_button(ui, if maximized { WinIcon::Restore } else { WinIcon::Maximize }, if maximized { "Restore" } else { "Maximize" }) {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
            if win_button(ui, WinIcon::Minimize, "Minimize") { ctx.send_viewport_cmd(ViewportCommand::Minimized(true)); }
            ui.add_space(10.0);
            // panel toggles (listed right-to-left: they read left panel, right panel)
            // a panel hidden because the window is too narrow shows as off; clicking it shows it in place of the other one
            let (sq_l, sq_r) = app.squeezed;
            if panel_toggle(ui, false, app.show_right && !sq_r, if sq_r { "Hidden: the window is too narrow. Click to show it instead of the project panel" } else { "Show / hide the operations panel" }) {
                if sq_r { app.show_left = false; } else { app.show_right = !app.show_right; }
            }
            if panel_toggle(ui, true, app.show_left && !sq_l, if sq_l { "Hidden: the window is too narrow. Click to show it instead of the operations panel" } else { "Show / hide the project panel" }) {
                if sq_l { app.show_right = false; } else { app.show_left = !app.show_left; }
            }
        });
    });

    // menus, left
    let menus = Rect::from_min_max(rect.min, Pos2::new(ctl.min.x, rect.max.y));
    ui.allocate_ui_at_rect(menus, |ui| {
        ui.set_min_height(HEADER_H);
        crate::panels::menu_bar(ui, app);
    });
}

/// A small window glyph with the left or right strip filled while that panel is visible.
/// One of the two panel buttons of the header; true when it was clicked.
fn panel_toggle(ui: &mut Ui, left: bool, on: bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 28.0), Sense::click());
    let vis = ui.visuals().clone();
    if resp.hovered() { ui.painter().rect_filled(rect.shrink2(egui::vec2(1.0, 1.0)), 7.0, vis.widgets.hovered.bg_fill); }
    let col = if on { vis.text_color() } else { vis.text_color().gamma_multiply(0.65) };
    let st = Stroke::new(1.5_f32, col);
    let body = Rect::from_center_size(rect.center(), egui::vec2(18.0, 14.0));
    let strip_w = 6.0;
    let strip = if left { Rect::from_min_size(body.min, egui::vec2(strip_w, body.height())) }
                else { Rect::from_min_size(Pos2::new(body.max.x - strip_w, body.min.y), egui::vec2(strip_w, body.height())) };
    if on { ui.painter().rect_filled(strip.shrink(1.0), 1.0, col.gamma_multiply(0.8)); }
    ui.painter().rect_stroke(body, 3.0, st);
    let x = if left { strip.max.x } else { strip.min.x };
    ui.painter().line_segment([Pos2::new(x, body.min.y), Pos2::new(x, body.max.y)], Stroke::new(1.2_f32, col));
    resp.on_hover_text(tip).clicked()
}

/// Which side panels fit in a window `w` wide next to a canvas of at least `canvas`: the operations panel (right) is the first to go,
/// then the project panel (left). Returns (left, right) as shown.
pub fn fit_panels(w: f32, canvas: f32, left_w: f32, right_w: f32, left: bool, right: bool) -> (bool, bool) {
    let right = right && w >= canvas + right_w + if left { left_w } else { 0.0 };
    let left = left && w >= canvas + left_w + if right { right_w } else { 0.0 };
    (left, right)
}

#[cfg(test)]
mod fit_tests {
    use super::fit_panels;
    #[test]
    fn the_operations_panel_goes_first_then_the_project_panel() {
        let f = |w: f32, l: bool, r: bool| fit_panels(w, 520.0, 394.0, 440.0, l, r);
        assert_eq!(f(1354.0, true, true), (true, true), "wide enough for both");
        assert_eq!(f(1353.0, true, true), (true, false), "one pixel short: the right one goes");
        assert_eq!(f(1000.0, true, true), (true, false));
        assert_eq!(f(900.0, true, true), (false, false), "not even the left one fits");
        assert_eq!(f(1000.0, false, true), (false, true), "the right one alone fits");
        assert_eq!(f(900.0, false, true), (false, false));
        assert_eq!(f(1000.0, true, false), (true, false));
        assert_eq!(f(1000.0, false, false), (false, false), "panels the user hid stay hidden");
    }
}


#[derive(Clone, Copy, PartialEq)]
enum WinIcon { Minimize, Maximize, Restore, Close }

fn win_button(ui: &mut Ui, icon: WinIcon, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::click());
    let vis = ui.visuals().clone();
    if resp.hovered() || resp.is_pointer_button_down_on() {
        let c = if resp.is_pointer_button_down_on() { vis.widgets.active.bg_fill } else { vis.widgets.hovered.bg_fill };
        ui.painter().circle_filled(rect.center(), 12.0, c);
    }
    let st = Stroke::new(1.6_f32, vis.text_color());
    let c = rect.center();
    let p = ui.painter();
    match icon {
        WinIcon::Minimize => { p.line_segment([c + egui::vec2(-5.0, 3.0), c + egui::vec2(5.0, 3.0)], st); }
        WinIcon::Maximize => { p.rect_stroke(Rect::from_center_size(c, egui::vec2(10.0, 10.0)), 1.5, st); }
        WinIcon::Restore => {
            p.rect_stroke(Rect::from_center_size(c + egui::vec2(-1.0, 1.0), egui::vec2(8.0, 8.0)), 1.5, st);
            p.line_segment([c + egui::vec2(-1.0, -5.0), c + egui::vec2(5.0, -5.0)], st);
            p.line_segment([c + egui::vec2(5.0, -5.0), c + egui::vec2(5.0, 1.0)], st);
        }
        WinIcon::Close => {
            p.line_segment([c + egui::vec2(-4.5, -4.5), c + egui::vec2(4.5, 4.5)], st);
            p.line_segment([c + egui::vec2(-4.5, 4.5), c + egui::vec2(4.5, -4.5)], st);
        }
    }
    resp.on_hover_text(tip).clicked()
}

/// Resize from the window edges and corners, and draw the thin window outline.
pub fn edges(ctx: &egui::Context) {
    if is_maximized(ctx) { return; }
    let screen = ctx.screen_rect();
    // outline
    ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("csd_outline")))
        .rect_stroke(screen.shrink(0.5), egui::Rounding::same(RADIUS), Stroke::new(1.0_f32, Color32::from_black_alpha(70)));

    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let (m, cm) = (5.0_f32, 14.0_f32);
    let left = pos.x - screen.min.x < m;
    let right = screen.max.x - pos.x < m;
    let top = pos.y - screen.min.y < m;
    let bottom = screen.max.y - pos.y < m;
    let near_x = |edge: f32| (pos.x - edge).abs() < cm;
    let near_y = |edge: f32| (pos.y - edge).abs() < cm;
    use ResizeDirection as D;
    let dir = if (left || top) && near_x(screen.min.x) && near_y(screen.min.y) && (left || top) { Some(D::NorthWest) }
        else if (right || top) && near_x(screen.max.x) && near_y(screen.min.y) && (right || top) { Some(D::NorthEast) }
        else if (left || bottom) && near_x(screen.min.x) && near_y(screen.max.y) && (left || bottom) { Some(D::SouthWest) }
        else if (right || bottom) && near_x(screen.max.x) && near_y(screen.max.y) && (right || bottom) { Some(D::SouthEast) }
        else if left { Some(D::West) } else if right { Some(D::East) } else if top { Some(D::North) } else if bottom { Some(D::South) } else { None };
    if let Some(d) = dir {
        ctx.set_cursor_icon(match d {
            D::North => CursorIcon::ResizeNorth, D::South => CursorIcon::ResizeSouth,
            D::East => CursorIcon::ResizeEast, D::West => CursorIcon::ResizeWest,
            D::NorthEast => CursorIcon::ResizeNorthEast, D::NorthWest => CursorIcon::ResizeNorthWest,
            D::SouthEast => CursorIcon::ResizeSouthEast, D::SouthWest => CursorIcon::ResizeSouthWest,
        });
        if ctx.input(|i| i.pointer.primary_pressed()) { ctx.send_viewport_cmd(ViewportCommand::BeginResize(d)); }
    }
}

/// Header row for dialogs: the title centred on the row's middle line and, when `closable`, a round
/// close button on the same line, followed by a thin divider. Returns true when close was clicked.
/// (egui's own window title bar can't centre its title and button in a custom-sized bar.)
pub fn dialog_header(ui: &mut Ui, title: &str, closable: bool) -> bool {
    dialog_header_inner(ui, title, closable, None::<fn(&mut Ui)>)
}

/// The same header with a round "?" at its left end: hover it for `help`, the explanations that do not need to
/// stay on screen. It mirrors the close button at the right end.
pub fn dialog_header_help(ui: &mut Ui, title: &str, closable: bool, help: impl FnOnce(&mut Ui)) -> bool {
    dialog_header_inner(ui, title, closable, Some(help))
}

fn dialog_header_inner(ui: &mut Ui, title: &str, closable: bool, help: Option<impl FnOnce(&mut Ui)>) -> bool {
    let h = 34.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::hover());
    // the header is the handle that moves the dialog
    let key = ui.layer_id();
    let grab = ui.interact(rect, ui.id().with(("dialog_drag", title)), Sense::drag()).on_hover_cursor(egui::CursorIcon::Grab);
    if grab.dragged() {
        let off = ui.ctx().data(|d| d.get_temp::<egui::Vec2>(dialog_key(key))).unwrap_or_default() + grab.drag_delta();
        ui.ctx().data_mut(|d| d.insert_temp(dialog_key(key), off));
        ui.ctx().request_repaint();
    }
    let vis = ui.visuals().clone();
    let font = egui::FontId::proportional(14.5);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, title, font, vis.strong_text_color());
    let mut clicked = false;
    if closable {
        let b = Rect::from_center_size(Pos2::new(rect.max.x - 15.0, rect.center().y), egui::vec2(28.0, 28.0));
        let resp = ui.interact(b, ui.id().with(("dialog_close", title)), Sense::click());
        if resp.hovered() || resp.is_pointer_button_down_on() {
            let c = if resp.is_pointer_button_down_on() { vis.widgets.active.bg_fill } else { vis.widgets.hovered.bg_fill };
            ui.painter().circle_filled(b.center(), 12.0, c);
        }
        let st = Stroke::new(1.6_f32, vis.text_color());
        let c = b.center();
        ui.painter().line_segment([c + egui::vec2(-4.5, -4.5), c + egui::vec2(4.5, 4.5)], st);
        ui.painter().line_segment([c + egui::vec2(-4.5, 4.5), c + egui::vec2(4.5, -4.5)], st);
        clicked = resp.on_hover_text("Close").clicked();
    }
    if let Some(help) = help {
        let b = Rect::from_center_size(Pos2::new(rect.min.x + 15.0, rect.center().y), egui::vec2(28.0, 28.0));
        let resp = ui.interact(b, ui.id().with(("dialog_help", title)), Sense::hover());
        if resp.hovered() { ui.painter().circle_filled(b.center(), 12.0, vis.widgets.hovered.bg_fill); }
        let col = if resp.hovered() { vis.text_color() } else { vis.weak_text_color() };
        ui.painter().circle_stroke(b.center(), 8.5, Stroke::new(1.3_f32, col));
        ui.painter().text(b.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(12.0), col);
        resp.on_hover_ui(|ui| { ui.set_max_width(300.0); help(ui); });
    }
    // divider spanning the full dialog width (through the window's padding)
    let y = rect.max.y + 2.0;
    ui.painter().line_segment([Pos2::new(rect.min.x - 12.0, y), Pos2::new(rect.max.x + 12.0, y)], vis.widgets.noninteractive.bg_stroke);
    ui.add_space(8.0);
    clicked
}

pub fn dialog_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::window(&ctx.style()).inner_margin(egui::Margin::symmetric(12.0, 6.0))
}

/// The body of a dialog below its pinned header: it scrolls when the window is too short to show all of it.
/// `reserve` is the height taken by everything else in the dialog (header, padding, any pinned footer).
pub fn scroll<R>(ui: &mut Ui, reserve: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let h = (ui.ctx().screen_rect().height() - reserve).max(160.0);
    // A window starts out 420 points tall and only grows to what its content asked for, and a scroll area asks for no more
    // than it is offered, so it would stay stuck at 420. Giving it its own room of `h` lets it take exactly what the content
    // needs, up to the window's height, and scroll only beyond that.
    ui.allocate_ui(egui::vec2(ui.available_width(), h), |ui| {
        egui::ScrollArea::vertical().max_height(h).auto_shrink([false, true]).show(ui, add).inner
    }).inner
}

/// Like `scroll`, but fills the dialog: the dialog's own height (which the user can drag) decides
/// how much is shown. `footer` is the height of what is pinned below it.
pub fn scroll_fill<R>(ui: &mut Ui, footer: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let h = (ui.available_height() - footer).clamp(120.0, (ui.ctx().screen_rect().height() - 140.0).max(160.0));
    egui::ScrollArea::vertical().max_height(h).min_scrolled_height(h).auto_shrink([false, false]).show(ui, add).inner
}

fn dialog_key(layer: egui::LayerId) -> egui::Id { egui::Id::new(("dialog_offset", layer)) }

/// Where the tool dialogs (array, combine, text, fillet, offset, clean up) open: at the top left of the drawing, under the palette and
/// the option bar that is showing, where the tools' own options appear.
pub fn anchor(app: &crate::app::App) -> Pos2 {
    if app.bars_bottom <= 0.0 { return Pos2::new(650.0, 300.0); }
    Pos2::new(app.canvas_left + 10.0, app.bars_bottom + 8.0)
}

/// Where a dialog goes: its starting spot plus however far the user has dragged it by the header
/// (kept so the dialog's anchor point stays inside the window).
pub fn spot(ctx: &egui::Context, title: &str, base: Pos2) -> Pos2 {
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new(title));
    let off = ctx.data(|d| d.get_temp::<egui::Vec2>(dialog_key(layer))).unwrap_or_default();
    let s = ctx.screen_rect().shrink(30.0);
    let p = base + off;
    let p = Pos2::new(p.x.clamp(s.min.x, s.max.x.max(s.min.x)), p.y.clamp(s.min.y, s.max.y.max(s.min.y)));
    ctx.data_mut(|d| d.insert_temp(dialog_key(layer), p - base));
    p
}
