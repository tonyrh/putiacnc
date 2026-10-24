//! Experiment: how do extra OS windows behave on this desktop (Hyprland, GNOME, KDE, Windows...)?
//! Run: `cargo run -p putiacnc-gui --example viewport_test`
//! Open each variant and note: does it float or get tiled, does the main window still respond,
//! can you drag/resize it, does it stay in front, does it flicker when resized.
//! All dialog windows share the app id "putiacnc-dialog", so a Hyprland rule like
//!   windowrulev2 = float, class:^(putiacnc-dialog)$
//! can be tried against them.
use eframe::egui::{self, ViewportBuilder, ViewportCommand, ViewportId};

#[derive(Clone, Copy, PartialEq)]
enum Kind { Plain, NoFrame, OnTop, Dialog }

impl Kind {
    const ALL: [(Kind, &'static str, &'static str); 4] = [
        (Kind::Plain, "Plain window", "ordinary window with the system title bar"),
        (Kind::NoFrame, "No system frame", "no title bar; has our own header that drags the window"),
        (Kind::OnTop, "Always on top", "system frame, asks to stay above other windows"),
        (Kind::Dialog, "Dialog type", "asks to be a dialog (honoured on X11 only)"),
    ];
    fn builder(self, label: &str) -> ViewportBuilder {
        let b = ViewportBuilder::default().with_title(format!("PutiaCNC test: {label}")).with_inner_size([420.0, 300.0]).with_min_inner_size([300.0, 200.0]).with_app_id("putiacnc-dialog");
        match self {
            Kind::Plain => b,
            Kind::NoFrame => b.with_decorations(false),
            Kind::OnTop => b.with_always_on_top(),
            Kind::Dialog => b.with_window_type(egui::X11WindowType::Dialog),
        }
    }
}

#[derive(Default)]
struct Test { open: [bool; 4], clicks: u32, text: String }

impl eframe::App for Test {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Extra OS window experiment");
            ui.label("Open a window below, then try to click the counter here while it is open.");
            ui.add_space(6.0);
            for (i, (_, label, note)) in Kind::ALL.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.button(format!("Open: {label}")).clicked() { self.open[i] = true; }
                    ui.weak(*note);
                });
            }
            ui.add_space(10.0);
            if ui.button(format!("Main window counter: {}", self.clicks)).clicked() { self.clicks += 1; }
            ui.label("Text shared with the extra windows:");
            ui.text_edit_singleline(&mut self.text);
            ui.add_space(6.0);
            let (focused, inner, outer) = ctx.input(|i| { let v = i.viewport(); (v.focused, v.inner_rect, v.outer_rect) });
            ui.weak(format!("main: focused {:?}, inner {:?}, outer {:?}", focused, inner.map(|r| r.size()), outer.map(|r| r.min)));
        });
        for (i, (kind, label, _)) in Kind::ALL.iter().enumerate() {
            if !self.open[i] { continue; }
            let id = ViewportId::from_hash_of(("putiacnc_test", i));
            let (clicks, text) = (&mut self.clicks, &mut self.text);
            let mut close = false;
            ctx.show_viewport_immediate(id, kind.builder(label), |ctx, class| {
                if class == egui::ViewportClass::Embedded { eprintln!("viewports unsupported: embedded"); }
                egui::CentralPanel::default().show(ctx, |ui| {
                    if *kind == Kind::NoFrame {
                        let r = ui.horizontal(|ui| { ui.heading("Our own header (drag me)"); });
                        if ui.interact(r.response.rect, ui.id().with("drag"), egui::Sense::drag()).drag_started() { ctx.send_viewport_cmd(ViewportCommand::StartDrag); }
                        ui.separator();
                    }
                    ui.label(format!("Variant: {label}"));
                    if ui.button(format!("Shared counter: {}", clicks)).clicked() { *clicks += 1; }
                    ui.text_edit_singleline(text);
                    ui.add_space(6.0);
                    let (focused, inner, outer) = ctx.input(|i| { let v = i.viewport(); (v.focused, v.inner_rect, v.outer_rect) });
                    ui.weak(format!("focused {:?}\ninner {:?}\nouter pos {:?}", focused, inner.map(|r| r.size()), outer.map(|r| r.min)));
                    ui.add_space(6.0);
                    if ui.button("Close").clicked() { close = true; }
                });
                if ctx.input(|i| i.viewport().close_requested()) { close = true; }
            });
            if close { self.open[i] = false; }
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions { viewport: ViewportBuilder::default().with_inner_size([620.0, 340.0]).with_title("PutiaCNC viewport test").with_app_id("putiacnc-test-main"), ..Default::default() };
    eframe::run_native("PutiaCNC viewport test", options, Box::new(|_| Box::<Test>::default()))
}
