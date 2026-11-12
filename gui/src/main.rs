// no console window behind the app on Windows release builds
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! PutiaCNC desktop GUI: 2D editor, operations, G-code and time estimate.
// Style lints that do not suit this code: geometry loops index several arrays by one counter (clearer than zips), and drawing/toolpath helpers take many plain parameters. Rewriting them would only risk changing the G-code.
// Shapes keep their points flat (x, y, x, y ...), so `chunks_exact(2)` is the natural way to read them; newer clippy suggests `as_chunks`.
#![allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
#![allow(clippy::needless_range_loop, clippy::too_many_arguments, clippy::type_complexity, clippy::field_reassign_with_default, clippy::explicit_counter_loop, clippy::manual_clamp, clippy::while_let_loop, clippy::neg_cmp_op_on_partial_ord)]
mod about;
mod setup;
mod units;
mod align;
mod app;
mod array;
mod msg;
mod nest;
mod canvas;
mod cleanup;
mod combine;
mod boxgen;
mod corners;
mod colorpick;
mod editpoints;
mod fingers;
mod surface;
mod reliefgen;
mod csd;
mod dynin;
mod icons;
mod layers;
mod machmgr;
mod offset;
mod osnap;
mod panels;
mod preview;
mod prefs;
mod text;
mod shortcuts;
mod theme;
mod model3d;
mod trace;
mod toolmgr;
mod toolpick;
mod update;
mod view3d;
mod viewcube;
mod checkui;
mod timeui;
mod exportui;
mod widgets;
mod zoomcard;
#[cfg(test)]
mod fuzz;
#[cfg(test)]
mod perf;
#[cfg(test)]
mod tips;
mod tour;
#[cfg(test)]
mod audit;

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
        // PUTIACNC_SHOW_ERR[=warning|info]: a sample message in the banner (an error unless said otherwise)
        if let Ok(v) = std::env::var("PUTIACNC_SHOW_ERR") {
            match v.as_str() {
                "warning" => app.say(app::Level::Warning, "The drawing (190.5 × 202.4 mm) is larger than the stock; resize it with Width/Height in the Selection panel.".into()),
                "info" => app.say(app::Level::Info, "Imported hill.stl: 120.0 × 80.0 × 25.0 mm. Scaled down to 80% to fit the stock (96.0 × 64.0 mm); change the size with Width in the Imported section.".into()),
                _ => app.say(crate::app::Level::Error, "Save failed: permission denied (os error 13).".into()),
            }
        }
        // PUTIACNC_TAB_EDIT=1: the first operation has manual tabs and is in tab placement (for screenshots)
        if std::env::var_os("PUTIACNC_TAB_EDIT").is_some() {
            if let Some(o) = app.project.data.ops.first_mut() { o.p.tabs = true; o.p.tab_pts = Some(vec![]); app.tab_edit = Some(o.id); app.project.data.act = o.id; }
        }
        // PUTIACNC_OP=n: operation n (by position) is the active one with its shapes selected, and every other operation is folded
        if let Some(n) = std::env::var("PUTIACNC_OP").ok().and_then(|v| v.parse::<usize>().ok()) {
            if let Some(id) = app.project.data.ops.get(n.saturating_sub(1)).map(|o| o.id) {
                app.project.data.act = id;
                app.sel = app.project.data.loops.iter().enumerate().filter(|(_, l)| l.op == id).map(|(i, _)| i).collect();
                app.collapsed = app.project.data.ops.iter().map(|o| o.id).filter(|&o| o != id).collect();
            }
        }
        if let Ok(v) = std::env::var("PUTIACNC_COLLAPSE") { for n in v.split(',').filter_map(|s| s.parse::<usize>().ok()) { if let Some(o) = app.project.data.ops.get(n.saturating_sub(1)) { app.collapsed.insert(o.id); } } }
        // first run: no settings file yet (or an assistant that was never finished); PUTIACNC_SKIP_SETUP keeps tests and screenshots clean
        let first_run = !prefs::exists() || prefs::load().setup_done == Some(false);
        if let Ok(v) = std::env::var("PUTIACNC_SHOW_SETUP") {
            // =again: as run from the Help menu; PUTIACNC_SETUP_STEP / _INCHES / _PICK choose what the picture shows
            app.setup.start(v != "again");
            if std::env::var_os("PUTIACNC_SETUP_INCHES").is_some() { app.setup.inches = true; app.setup.machine.inches = true; }
            if let Some(i) = std::env::var("PUTIACNC_SETUP_PICK").ok().and_then(|v| v.parse().ok()) { app.setup.pick_machine(i); }
            app.setup.step = std::env::var("PUTIACNC_SETUP_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        }
        else if first_run && std::env::var_os("PUTIACNC_SKIP_SETUP").is_none() { app.setup.start(true); }
        // PUTIACNC_SHOW_TOUR=n: the quick tour at step n
        if let Ok(v) = std::env::var("PUTIACNC_SHOW_TOUR") { app.tour.start(); app.tour.step = v.parse().unwrap_or(0); }
        // PUTIACNC_SHOW_SAMPLE: the sample project
        if std::env::var_os("PUTIACNC_SHOW_SAMPLE").is_some() {
            app.load_sample(match std::env::var("PUTIACNC_SHOW_SAMPLE").as_deref() { Ok("box") => putiacnc_core::sample::Kind::Box, Ok("relief") => putiacnc_core::sample::Kind::Relief, Ok("shaped") => putiacnc_core::sample::Kind::ShapedRelief, Ok("oval") => putiacnc_core::sample::Kind::Oval, _ => putiacnc_core::sample::Kind::NamePlate });
            if std::env::var("PUTIACNC_VIEW").as_deref() == Ok("3d") { app.view_tab = view3d::ViewTab::Preview; }
            if std::env::var_os("PUTIACNC_AIR").is_some() { for o in &mut app.project.data.ops { o.p.air = true; } }
            if std::env::var_os("PUTIACNC_NO_PATHS").is_some() { app.show_toolpath = false; }
        }
        // PUTIACNC_SHOW_REST: a pocket with a 6 mm cutter and a 2 mm one for the corners (3D preview when PUTIACNC_VIEW=3d)
        if std::env::var_os("PUTIACNC_SHOW_REST").is_some() {
            let by = |a: &app::App, n: &str| a.project.data.tools.iter().find(|t| t.name == n).map(|t| t.id.clone());
            if let (Some(big), Some(small)) = (by(&app, "6 mm flat end mill"), by(&app, "2 mm flat end mill")) {
                let mut p = putiacnc_core::shapes::default_op_params(&big);
                p.kind = "pocket".into(); p.depth = 4.0; p.rest = Some(small);
                let narrow = std::env::var("PUTIACNC_SHOW_REST").as_deref() == Ok("narrow");
                if narrow { p.rest = None; }
                let d = &mut app.project.data;
                d.stock.w = 100.0; d.stock.h = 80.0; d.stock.t = 10.0;
                d.ops.push(putiacnc_core::model::Operation { id: 1, p }); d.oid = 1; d.act = 1;
                d.loops.push(putiacnc_core::shapes::new_rect([20.0, 20.0], [80.0, if narrow { 24.0 } else { 60.0 }], 0.0, 1));
                if std::env::var("PUTIACNC_VIEW").as_deref() == Ok("3d") { app.view_tab = view3d::ViewTab::Preview; }
            }
        }
        // PUTIACNC_SHOW_SURFACE[=add]: the Surface window (and with =add, the operation already added)
        if let Ok(v) = std::env::var("PUTIACNC_SHOW_SURFACE") {
            if v == "add" {
                if let Some(t) = putiacnc_core::surface::suggest_tool(&app.project.data.tools).cloned() { let _ = putiacnc_core::surface::add_surfacing(&mut app.project.data, &putiacnc_core::surface::defaults(&t, false)); }
                if std::env::var("PUTIACNC_VIEW").as_deref() == Ok("3d") { app.view_tab = view3d::ViewTab::Preview; }
            } else { app.open_tool_window(app::ToolWin::Surface); }
        }
        theme::apply_fonts(&cc.egui_ctx);
        theme::apply_style(&cc.egui_ctx, &app.theme);
        Box::new(app)
    }))
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] { [0.0, 0.0, 0.0, 0.0] }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) { csd::tab_hook(ctx, raw_input); }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.gl_info.is_empty() { if let Some(gl) = frame.gl() { self.gl_info = about::gl_summary(gl); if std::env::var_os("PUTIACNC_PRINT_VERSION_INFO").is_some() { eprintln!("{}", about::version_info(ctx, &self.gl_info)); } } }
        self.ui_frame(ctx);
        csd::focus_ring(ctx);
    }
}

impl App {
    /// One frame of the whole window: everything `update` does except the OpenGL information (so tests can run frames without a window).
    pub(crate) fn ui_frame(&mut self, ctx: &egui::Context) {
        widgets::set_inches(ctx, self.disp_in);
        widgets::set_fractions(ctx, self.frac_in);
        widgets::set_speed_unit(ctx, self.tool_speed());
        self.sync_grid_units();
        csd::tab_regions_reset(ctx);
        self.refit_fingers();
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

        // a window narrower than the panels and the smallest canvas (a tiling window manager can do that) hides the operations panel first,
        // then the project panel, until the window is wide enough again
        let (fit_left, fit_right) = csd::fit_panels(ctx.screen_rect().width(), MIN_CANVAS_W, left_w(), right_w(), self.show_left, self.show_right);
        self.squeezed = (self.show_left && !fit_left, self.show_right && !fit_right);
        egui::TopBottomPanel::top("menu").frame(bar_frame(ctx, self.theme.header()).rounding(csd::corners(ctx, true))).show(ctx, |ui| { ui.set_min_height(csd::HEADER_H); csd::header(ui, self) });
        let r = egui::TopBottomPanel::bottom("status").frame(bar_frame(ctx, self.theme.header()).rounding(csd::corners(ctx, false))).show(ctx, |ui| panels::status_bar(ui, self));
        tour::note(ctx, "status", r.response.rect);
        // the smallest window depends on which panels are showing
        let min_w = MIN_CANVAS_W + if self.show_left { left_w() } else { 0.0 } + if self.show_right { right_w() } else { 0.0 };
        if (self.last_min_w - min_w).abs() > 0.5 {
            self.last_min_w = min_w;
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(min_w.max(640.0), MIN_H)));
        }
        if fit_left {
            let r = egui::SidePanel::left("left").resizable(false).exact_width(left_w()).frame(side_frame(ctx)).show(ctx, |ui| { egui::Frame::none().inner_margin(8.0).show(ui, |ui| panels::left_panel(ui, self)); });
            tour::note(ctx, "left", r.response.rect);
            csd::allow_tab(ctx, r.response.rect);
        }
        if fit_right {
            let r = egui::SidePanel::right("right").resizable(false).exact_width(right_w()).frame(side_frame(ctx)).show(ctx, |ui| { egui::Frame::none().inner_margin(8.0).show(ui, |ui| panels::right_panel(ui, self)); });
            tour::note(ctx, "right", r.response.rect);
            csd::allow_tab(ctx, r.response.rect);
        }
        egui::TopBottomPanel::bottom("toolbar").frame(bar_frame(ctx, self.theme.header())).show(ctx, |ui| panels::tool_bar(ui, self));
        let preview = self.view_tab == view3d::ViewTab::Preview;
        let r = egui::TopBottomPanel::top("viewtabs").frame(bar_frame(ctx, self.theme.header()).inner_margin(egui::Margin { left: 8.0, right: 8.0, top: 8.0, bottom: 4.0 })).show(ctx, |ui| view3d::tab_strip(ui, self));
        tour::note(ctx, "tabs", r.response.rect);
        if self.error.is_some() {
            egui::TopBottomPanel::top("msgbar").frame(bar_frame(ctx, self.theme.header()).inner_margin(egui::Margin::symmetric(10.0, 6.0))).show(ctx, |ui| panels::message_bar(ui, self));
        }
        // a clamp the toolpath runs into is the most serious warning: it has a bar of its own for as long as it is so
        if !self.clamp_hits.is_empty() && self.output.is_some() {
            egui::TopBottomPanel::top("clampbar").frame(bar_frame(ctx, self.theme.header()).inner_margin(egui::Margin::symmetric(10.0, 6.0))).show(ctx, |ui| panels::clamp_bar(ui, self));
        }
        if !checkui::bar_findings(self).is_empty() {
            egui::TopBottomPanel::top("checksbar").frame(bar_frame(ctx, self.theme.header()).inner_margin(egui::Margin::symmetric(10.0, 6.0))).show(ctx, |ui| checkui::bar(ui, self));
        }
        // dialogs may go anywhere in what the panels and bars leave, over the 3D half of the split view as well
        let split = self.view_tab == view3d::ViewTab::Split;
        self.split_rect = None;
        if split {
            let w = ctx.available_rect().width();
            let r = egui::SidePanel::right("split3d").resizable(true).default_width((w * 0.45).max(260.0)).width_range(220.0..=(w - 260.0).max(221.0))
                .frame(egui::Frame::none().fill(self.theme.view())).show(ctx, |ui| {
                    view3d::preview(ui, self);
                });
            self.split_rect = Some(r.response.rect);
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
        self.one_tool_window();
        offset::dialog(ctx, self);
        align::popover(ctx, self);
        cleanup::dialog(ctx, self);
        shortcuts::window(ctx, self);
        about::window(ctx, self);
        setup::window(ctx, self);
        tour::window(ctx, self);
        checkui::window(ctx, self);
        checkui::export_gate(ctx, self);
        timeui::window(ctx, self);
        exportui::window(ctx, self);
        combine::dialog(ctx, self);
        array::dialog(ctx, self);
        nest::dialog(ctx, self);
        editpoints::dialog(ctx, self);
        corners::dialog(ctx, self);
        fingers::dialog(ctx, self);
        surface::dialog(ctx, self);
        reliefgen::dialog(ctx, self);
        boxgen::dialog(ctx, self);
        text::dialog(ctx, self);
        trace::dialog(ctx, self);
        model3d::dialog(ctx, self);
        csd::tab_skip(ctx);
        if self.machines_dirty && !ctx.input(|i| i.pointer.any_down()) { self.save_machines(); }
        if self.libs_dirty && !ctx.input(|i| i.pointer.any_down()) { self.save_tool_libs(); }

        let busy = ctx.input(|i| i.pointer.any_down());
        if !busy && !ctx.wants_keyboard_input() { self.hist.open = false; }
        if !busy { self.hist.classify(&self.project.data); }
        if self.poll_checks() { ctx.request_repaint_after(std::time::Duration::from_millis(100)); }
        if self.poll_generation() { ctx.request_repaint_after(std::time::Duration::from_millis(30)); }
        if self.gen_pending && !busy && self.gen_rx.is_none() {
            self.start_generation();
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
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
/// The panels' widths in the language in use (see `i18n::width_factor`).
fn left_w() -> f32 { (LEFT_W * putiacnc_core::i18n::width_factor()).round() }
fn right_w() -> f32 { (RIGHT_W * putiacnc_core::i18n::width_factor()).round() }
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
