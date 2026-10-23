// no console window behind the app on Windows release builds
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! PutiaCNC desktop GUI: 2D editor, operations, G-code and time estimate.
mod about;
mod align;
mod app;
mod array;
mod canvas;
mod cleanup;
mod combine;
mod corners;
mod csd;
mod dynin;
mod icons;
mod machmgr;
mod offset;
mod osnap;
mod panels;
mod prefs;
mod text;
mod shortcuts;
mod theme;
mod model3d;
mod trace;
mod toolmgr;
mod update;
mod view3d;
mod widgets;

use app::App;
use eframe::egui;

/// Window icon (taskbar and window switcher), decoded from the bundled PNG.
fn window_icon() -> Option<egui::IconData> {
    let mut reader = png::Decoder::new(&include_bytes!("../../assets/icons/putiacnc-256.png")[..]).read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight { return None; }
    buf.truncate(info.buffer_size());
    Some(egui::IconData { rgba: buf, width: info.width, height: info.height })
}

fn main() -> eframe::Result<()> {
    let arg = std::env::args().nth(1);
    let saved = prefs::load().window;
    let mut viewport = egui::ViewportBuilder::default().with_inner_size(start_size(&saved)).with_min_inner_size([MIN_W, MIN_H]).with_title("PutiaCNC")
        .with_app_id("putiacnc").with_decorations(false).with_transparent(true);
    if std::env::var_os("PUTIACNC_WIN").is_none() {
        if let Some(w) = &saved {
            if w.maximized { viewport = viewport.with_maximized(true); }
            // the place is only restored when it is clearly on a screen (a monitor may have been unplugged)
            if let (Some(x), Some(y)) = (w.x, w.y) { if (0.0..=3000.0).contains(&x) && (0.0..=2000.0).contains(&y) { viewport = viewport.with_position([x, y]); } }
        }
    }
    if let Some(icon) = window_icon() { viewport = viewport.with_icon(icon); }
    let opts = eframe::NativeOptions {
        viewport,
        // Wayland compositors stop sending frame callbacks to windows on other workspaces; with vsync
        // the buffer swap then blocks the whole event loop, the ping goes unanswered and GNOME shows
        // "Application not responding". Without vsync the swap never blocks.
        vsync: false,
        ..Default::default()
    };
    eframe::run_native("PutiaCNC", opts, Box::new(move |cc| {
        let mut app = App::new(arg);
        if std::env::var_os("PUTIACNC_SHOW_ERR").is_some() { app.error = Some("The imported drawing (420 x 310 mm) is larger than the machine work area (300 x 200 mm).".into()); }
        if let Ok(v) = std::env::var("PUTIACNC_COLLAPSE") { for n in v.split(',').filter_map(|s| s.parse::<usize>().ok()) { if let Some(o) = app.project.data.ops.get(n.saturating_sub(1)) { app.collapsed.insert(o.id); } } }
        theme::apply_fonts(&cc.egui_ctx);
        theme::apply_style(&cc.egui_ctx, &app.theme);
        Box::new(app)
    }))
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] { [0.0, 0.0, 0.0, 0.0] }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dropped: Option<String> = ctx.input(|i| i.raw.dropped_files.first().and_then(|f| f.path.as_ref().map(|p| p.display().to_string())));
        if let Some(p) = dropped {
            if App::is_drawing(&p) { self.import_file(&p); }
            else if App::is_model(&p) { self.import_model(&p); }
            else if App::is_bitmap(&p) { self.import_bitmap(&p); }
            else { self.request(app::Action::Open(p)); }
        }

        // window close button: ask first when there are unsaved changes
        if ctx.input(|i| i.viewport().close_requested()) && self.modified && !self.quit_ok {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(app::Action::Quit);
        }

        // follow GNOME's dark/light, accent and font-size settings while running
        if self.theme_checked.elapsed().as_secs() >= 3 {
            self.theme_checked = std::time::Instant::now();
            let t = theme::detect();
            if t != self.theme { self.theme = t; theme::apply_style(ctx, &t); }
        }
        // slow idle wake-ups (theme check); even slower while the window is in the background
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        ctx.request_repaint_after(std::time::Duration::from_secs(if focused { 3 } else { 30 }));

        self.poll_dialog(ctx);
        if self.pending.is_none() { canvas::handle_keys(ctx, self); }

        egui::TopBottomPanel::top("menu").frame(bar_frame(ctx, self.theme.header()).rounding(csd::corners(ctx, true))).show(ctx, |ui| { ui.set_min_height(csd::HEADER_H); csd::header(ui, self) });
        egui::TopBottomPanel::bottom("status").frame(bar_frame(ctx, self.theme.header()).rounding(csd::corners(ctx, false))).show(ctx, |ui| panels::status_bar(ui, self));
        // the smallest window depends on which panels are showing
        let min_w = MIN_CANVAS_W + if self.show_left { LEFT_W } else { 0.0 } + if self.show_right { RIGHT_W } else { 0.0 };
        if (self.last_min_w - min_w).abs() > 0.5 {
            self.last_min_w = min_w;
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(min_w.max(640.0), MIN_H)));
        }
        if self.show_left {
            egui::SidePanel::left("left").resizable(false).exact_width(LEFT_W).frame(side_frame(ctx)).show(ctx, |ui| { egui::Frame::none().inner_margin(8.0).show(ui, |ui| panels::left_panel(ui, self)); });
        }
        if self.show_right {
            egui::SidePanel::right("right").resizable(false).exact_width(RIGHT_W).frame(side_frame(ctx)).show(ctx, |ui| { egui::Frame::none().inner_margin(8.0).show(ui, |ui| panels::right_panel(ui, self)); });
        }
        egui::TopBottomPanel::bottom("toolbar").frame(bar_frame(ctx, self.theme.header())).show(ctx, |ui| panels::tool_bar(ui, self));
        let preview = self.view_tab == view3d::ViewTab::Preview;
        if preview {
            egui::TopBottomPanel::bottom("playbar").frame(bar_frame(ctx, self.theme.header())).show(ctx, |ui| view3d::play_bar(ui, self));
        }
        egui::TopBottomPanel::top("viewtabs").frame(bar_frame(ctx, self.theme.header()).inner_margin(egui::Margin { left: 8.0, right: 8.0, top: 8.0, bottom: 4.0 })).show(ctx, |ui| view3d::tab_strip(ui, self));
        if self.error.is_some() {
            egui::TopBottomPanel::top("msgbar").frame(bar_frame(ctx, self.theme.header()).inner_margin(egui::Margin::symmetric(10.0, 6.0))).show(ctx, |ui| panels::message_bar(ui, self));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(self.theme.view()))
            .show(ctx, |ui| if preview { view3d::preview(ui, self) } else { canvas::canvas(ui, self) });
        csd::edges(ctx);
        panels::confirm_dialog(ctx, self);
        toolmgr::tool_manager(ctx, self);
        machmgr::machine_manager(ctx, self);
        self.updater.poll(ctx);
        self.sync_prefs(ctx);
        update::window(ctx, self);
        offset::dialog(ctx, self);
        align::popover(ctx, self);
        cleanup::dialog(ctx, self);
        shortcuts::window(ctx, self);
        about::window(ctx, self);
        combine::dialog(ctx, self);
        array::dialog(ctx, self);
        corners::dialog(ctx, self);
        text::dialog(ctx, self);
        trace::dialog(ctx, self);
        model3d::dialog(ctx, self);
        if self.machines_dirty && !ctx.input(|i| i.pointer.any_down()) { self.save_machines(); }
        if self.libs_dirty && !ctx.input(|i| i.pointer.any_down()) { self.save_tool_libs(); }

        let busy = ctx.input(|i| i.pointer.any_down());
        if !busy && !ctx.wants_keyboard_input() { self.hist.open = false; }
        if !busy { self.hist.classify(&self.project.data); }
        if self.gen_pending && !busy {
            self.regenerate();
            ctx.request_repaint();
        }

        let title = format!("{}{} - PutiaCNC", self.project.name, if self.modified { " *" } else { "" });
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        if self.want_quit {
            self.want_quit = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn bar_frame(ctx: &egui::Context, fill: egui::Color32) -> egui::Frame {
    egui::Frame::none().fill(fill).inner_margin(egui::Margin::symmetric(8.0, 4.0))
        .stroke(egui::Stroke::new(1.0_f32, ctx.style().visuals.widgets.noninteractive.bg_stroke.color))
}

/// Fixed side panel widths (content-driven: the left panel holds the stock grid, the right one the operation editor).
const LEFT_W: f32 = 394.0;
const RIGHT_W: f32 = 440.0;
/// Smallest canvas that still fits the bottom toolbar (view toggles left and right) and the floating palette.
const MIN_CANVAS_W: f32 = 520.0;
/// Smallest window: both panels plus the canvas.
const MIN_W: f32 = LEFT_W + RIGHT_W + MIN_CANVAS_W;
const MIN_H: f32 = 640.0;

fn start_size(saved: &Option<prefs::Win>) -> [f32; 2] {
    // PUTIACNC_WIN=1354x620 lets us test the smallest layout
    if let Ok(v) = std::env::var("PUTIACNC_WIN") {
        if let Some((w, h)) = v.split_once('x') {
            if let (Ok(w), Ok(h)) = (w.parse::<f32>(), h.parse::<f32>()) { return [w, h]; }
        }
    }
    // the size of the last session, kept within sensible limits
    if let Some(w) = saved { if w.w >= MIN_W && w.h >= MIN_H && w.w <= 8000.0 && w.h <= 6000.0 { return [w.w, w.h]; } }
    [1440.0, 860.0]
}

/// Side panel frame without its own margin (the content adds it), so `exact_width` is the real outer width.
fn side_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::side_top_panel(&ctx.style()).inner_margin(0.0)
}

#[cfg(test)]
mod icon_tests {
    #[test]
    fn window_icon_decodes() {
        let i = super::window_icon().expect("icon");
        assert_eq!((i.width, i.height), (256, 256));
        assert_eq!(i.rgba.len(), 256 * 256 * 4);
    }
}
