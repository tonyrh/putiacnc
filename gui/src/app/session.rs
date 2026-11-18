//! Starting a new project, loading one, and the preferences.
use super::*;
use eframe::egui::{self, ViewportCommand};

/// Where the window is put back at start: the saved place, when it is clearly on a screen
/// (a monitor may have been unplugged).
pub fn restore_pos(w: &crate::prefs::Win) -> Option<[f32; 2]> {
    let (x, y) = (w.x?, w.y?);
    ((0.0..=3000.0).contains(&x) && (0.0..=2000.0).contains(&y)).then_some([x, y])
}

/// The saved window state while it is being put back. The viewport builder asks for it when the
/// window is created, but on X11 the window manager can miss that request (maximizing is asked
/// right after the window is mapped, before the manager has taken it) or place the window itself,
/// so once the window is up the same is asked again. Until that has settled the window state is
/// not saved, so a window that came up wrong does not overwrite what was saved.
#[derive(Debug, Clone)]
pub struct WinRestore {
    want: crate::prefs::Win,
    start: Option<f64>,
    asked: bool,
}

/// What the restore wants done this frame.
#[derive(Debug, PartialEq)]
pub enum RestoreStep { Wait, Ask(Vec<ViewportCommand>), Done }

impl WinRestore {
    /// Seconds after the first frame before asking again (the window manager may still be at it).
    const ASK_AFTER: f64 = 0.25;
    /// Seconds after the first frame when it gives up and saves whatever the window is.
    const GIVE_UP: f64 = 1.5;

    pub fn new(want: crate::prefs::Win) -> Self { Self { want, start: None, asked: false } }

    /// One frame: `now` in seconds, whether the window is maximized and where its outer corner is.
    pub fn step(&mut self, now: f64, maximized: bool, outer: Option<egui::Pos2>) -> RestoreStep {
        let start = *self.start.get_or_insert(now);
        let pos = restore_pos(&self.want);
        let placed = |o: egui::Pos2, p: [f32; 2]| (o.x - p[0]).abs() <= 2.0 && (o.y - p[1]).abs() <= 2.0;
        let right = if self.want.maximized { maximized } else {
            match (pos, outer) { (Some(p), Some(o)) => placed(o, p), (None, _) => true, (_, None) => false }
        };
        // the first frames can come before the window manager has placed the window, so it is only
        // judged once the manager has had a moment
        if now - start < Self::ASK_AFTER { return RestoreStep::Wait; }
        if right || now - start > Self::GIVE_UP { return RestoreStep::Done; }
        if self.asked { return RestoreStep::Wait; }
        self.asked = true;
        let mut cmds = vec![];
        if let Some(p) = pos { if !maximized { cmds.push(ViewportCommand::OuterPosition(p.into())); } }
        if self.want.maximized { cmds.push(ViewportCommand::Maximized(true)); }
        RestoreStep::Ask(cmds)
    }
}

impl App {
    pub fn new_project(&mut self) {
        self.project = shapes::blank_project();
        self.project.data.tools.clear(); // a new project uses the current library
        self.path.clear();
        self.reset_after_load();
        self.modified = false;
    }

    /// "Sample projects": a new project made to fit the machine and tools in use (a name plate, or a finger-joint box).
    pub fn load_sample(&mut self, kind: putiacnc_core::sample::Kind) {
        self.new_project();
        let inches = self.disp_in;
        match putiacnc_core::sample::fill_kind(&mut self.project.data, inches, kind) {
            Ok(()) => {
                self.project.name = kind.title().into();
                self.view_tab = crate::view3d::ViewTab::Split;
                self.tab = if std::env::var_os("PUTIACNC_TAB_GCODE").is_some() { RightTab::Gcode } else { RightTab::Info }; // the Notes explain the sample
                self.sel.clear();
                self.gen_pending = true;
                self.fit_pending = true;
                self.modified = true;
            }
            Err(e) => { self.new_project(); self.mach_msg = e; }
        }
    }

    pub(super) fn reset_after_load(&mut self) {
        self.sel.clear();
        self.hist = History::default();
        self.drag = Drag::None;
        self.poly.clear();
        self.modified = false;
        self.fit_pending = true;
        self.gen_pending = true;
        // a toolpath still being worked out belongs to the project that was open before: forget it, and its results
        self.gen_rx = None; self.gen_started = None;
        self.output = None; self.pieces.clear(); self.gen_error = None; self.out_ver += 1;
        self.checks.clear(); self.check_parts.clear(); self.clamp_hits.clear(); self.checks_rx = None; self.check_sel = None;
        let n = self.sync_tools();
        self.sync_machine();
        self.modified = n > 0;
    }

    /// Takes the remembered settings from disk (called once at start).
    pub(super) fn apply_prefs(&mut self) {
        let p = crate::prefs::load();
        putiacnc_core::i18n::set_language(&std::env::var("PUTIACNC_LANG").ok().or(p.language.clone()).unwrap_or_else(|| "auto".into()));
        if let Some(m) = p.theme.as_deref().filter(|m| matches!(*m, "system" | "light" | "dark")) { self.theme_mode = m.to_string(); self.theme = crate::theme::detect_mode(m); }
        if let Some(v) = p.show_left { self.show_left = v; }
        if let Some(v) = p.show_right { self.show_right = v; }
        match p.right_tab.as_deref() { Some("gcode") => self.tab = RightTab::Gcode, Some("notes") => self.tab = RightTab::Info, Some("ops") => self.tab = RightTab::Ops, _ => {} }
        if let Some(v) = p.grid_on { self.grid_on = v; }
        if let Some(v) = p.snap_on { self.snap_on = v; }
        if let Some(v) = p.osnap_on { self.osnap_on = v; }
        if let Some(v) = p.snap_mm.filter(|v| *v > 0.0 && *v <= 1000.0) { self.snap_mm = v; }
        if let Some(v) = p.show_toolpath { self.show_toolpath = v; }
        if let Some(v) = p.show_rapids { self.show_rapids = v; }
        if let Some(v) = p.show_machine_area { self.show_machine_area = v; }
        if let Some(v) = p.show_rulers { self.show_rulers = v; }
        if let Some(v) = p.show_axes { self.show_axes = v; }
        if let Some(v) = p.show_badges { self.show_badges = v; }
        if let Some(v) = p.show_clamps { self.show_clamps = v; }
        if let Some(v) = &p.text_sections { self.text.sections = v.iter().cloned().collect(); }
        if let Some(v) = p.tool_len_in.or(p.mach_len_in) { self.disp_in = v; }
        if let Some(v) = p.display_in { self.disp_in = v; }
        if let Some(v) = p.fractions_in { self.frac_in = v; }
        if let Some(v) = p.inch_decimals { self.inch_dec = v.clamp(2, 5); }
        if let Some(v) = p.mm_decimals { self.mm_dec = v.min(2); }
        self.show_hints = !p.hide_hints;
        self.show_card_notes = !p.hide_card_notes;
        if let Some(v) = p.fraction_denom { if crate::widgets::FRACTION_DENOMS.contains(&v) { self.frac_den = v; } }
        if let Ok(u) = std::env::var("PUTIACNC_FRACTIONS") { self.frac_in = u == "1"; }
        // test hook: the display units ("in" or "mm")
        if let Ok(u) = std::env::var("PUTIACNC_UNITS") { self.disp_in = u == "in"; }
        // older versions kept one unit for both views: an inch unit there is the one for the inch view
        if let Some(v) = p.tool_speed_idx { if v < 3 { self.tool_speed_mm = v; } else { self.tool_speed_in = v; } }
        if let Some(v) = p.tool_speed_idx_in { self.tool_speed_in = v; }
        // test hook: the unit n (and the lengths view it belongs to)
        if let Some(i) = std::env::var("PUTIACNC_SPEED_UNIT").ok().and_then(|v| v.parse::<usize>().ok()) {
            self.disp_in = i >= 3;
            *self.tool_speed_slot() = i;
        }
        if let Some(v) = &p.export_split { self.export_split = putiacnc_core::split::Split::from_id(v); }
        crate::colorpick::set_custom(&p.custom_colors);
        self.prefs_saved = p.clone();
        self.prefs = p;
    }

    /// Called every frame: writes the settings to disk when something changed (not while a mouse button is down).
    pub fn sync_prefs(&mut self, ctx: &eframe::egui::Context) {
        let mut s = self.prefs.clone();
        s.update_check = self.updater.auto;
        s.theme = Some(self.theme_mode.clone());
        s.show_left = Some(self.show_left);
        s.show_right = Some(self.show_right);
        s.right_tab = Some(match self.tab { RightTab::Ops => "ops", RightTab::Gcode => "gcode", RightTab::Info => "notes" }.to_string());
        s.grid_on = Some(self.grid_on);
        s.snap_on = Some(self.snap_on);
        s.osnap_on = Some(self.osnap_on);
        s.snap_mm = Some(self.snap_mm);
        s.show_toolpath = Some(self.show_toolpath);
        s.show_rapids = Some(self.show_rapids);
        s.show_machine_area = Some(self.show_machine_area);
        s.show_rulers = Some(self.show_rulers);
        s.show_axes = Some(self.show_axes);
        s.show_badges = Some(self.show_badges);
        s.show_clamps = Some(self.show_clamps);
        s.text_sections = Some(self.text.sections.iter().cloned().collect());
        s.display_in = Some(self.disp_in);
        s.fractions_in = Some(self.frac_in);
        s.inch_decimals = Some(self.inch_dec);
        s.mm_decimals = Some(self.mm_dec);
        s.hide_hints = !self.show_hints;
        s.hide_card_notes = !self.show_card_notes;
        s.fraction_denom = Some(self.frac_den);
        s.tool_speed_idx = Some(self.tool_speed_mm);
        s.tool_speed_idx_in = Some(self.tool_speed_in);
        s.export_split = Some(self.export_split.id().to_string());
        s.custom_colors = crate::colorpick::custom_hex();
        if let Some(o) = self.trace.remember.clone() { s.trace = Some(o); }
        if let Some(o) = self.model3d.remember_obj { s.model_orient_obj = Some(o); }
        if let Some(o) = self.model3d.remember_stl { s.model_orient_stl = Some(o); }
        let (inner, outer, maximized) = ctx.input(|i| { let v = i.viewport(); (v.inner_rect, v.outer_rect, v.maximized.unwrap_or(false)) });
        let mut settling = false;
        if let Some(r) = &mut self.win_restore {
            match r.step(ctx.input(|i| i.time), maximized, outer.map(|o| o.min)) {
                RestoreStep::Done => self.win_restore = None,
                RestoreStep::Wait => settling = true,
                RestoreStep::Ask(cmds) => { settling = true; for c in cmds { ctx.send_viewport_cmd(c); } }
            }
            if settling { ctx.request_repaint_after(std::time::Duration::from_millis(50)); }
        }
        if let (Some(r), false) = (inner, settling) {
            let mut w = s.window.clone().unwrap_or_default();
            w.maximized = maximized;
            // a maximized window keeps the size it had before
            if !maximized && r.width() > 100.0 && r.height() > 100.0 {
                w.w = r.width();
                w.h = r.height();
                if let Some(o) = outer { w.x = Some(o.min.x); w.y = Some(o.min.y); }
            }
            if w.w > 0.0 { s.window = Some(w); }
        }
        if s != self.prefs_saved && !ctx.input(|i| i.pointer.any_down()) {
            crate::prefs::save(&s);
            self.prefs_saved = s.clone();
        }
        self.prefs = s;
    }

    pub(super) fn remember_dir(&mut self, kind: DlgKind, p: &std::path::Path) {
        let d = crate::prefs::dir_of(p);
        if d.is_none() { return; }
        match kind {
            DlgKind::Open | DlgKind::SaveAs => self.prefs.dir_project = d,
            DlgKind::Import | DlgKind::Model | DlgKind::Bitmap => self.prefs.dir_svg = d,
            DlgKind::ExportGcode => self.prefs.dir_gcode = d,
            _ => {}
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::Win;
    fn win(x: f32, y: f32, maximized: bool) -> Win { Win { w: 1000.0, h: 600.0, x: Some(x), y: Some(y), maximized } }

    #[test]
    fn a_maximized_window_that_came_up_normal_is_maximized_again() {
        let mut r = WinRestore::new(win(7.0, 8.0, true));
        assert_eq!(r.step(0.0, false, Some(egui::pos2(25.0, 25.0))), RestoreStep::Wait);
        assert_eq!(r.step(0.3, false, Some(egui::pos2(25.0, 25.0))),
            RestoreStep::Ask(vec![ViewportCommand::OuterPosition(egui::pos2(7.0, 8.0)), ViewportCommand::Maximized(true)]));
        assert_eq!(r.step(0.4, false, None), RestoreStep::Wait, "asked only once");
        assert_eq!(r.step(0.5, true, None), RestoreStep::Done);
    }

    #[test]
    fn a_window_the_manager_placed_elsewhere_is_moved_back() {
        let mut r = WinRestore::new(win(6.9, 7.7, false));
        assert_eq!(r.step(10.0, false, Some(egui::pos2(24.6, 24.6))), RestoreStep::Wait);
        assert_eq!(r.step(10.3, false, Some(egui::pos2(24.6, 24.6))), RestoreStep::Ask(vec![ViewportCommand::OuterPosition(egui::pos2(6.9, 7.7))]));
        assert_eq!(r.step(10.4, false, Some(egui::pos2(6.9, 7.7))), RestoreStep::Done);
        // moved by the manager after a first frame that was still in the right place
        let mut r = WinRestore::new(win(6.9, 7.7, false));
        assert_eq!(r.step(0.0, false, Some(egui::pos2(6.9, 7.7))), RestoreStep::Wait);
        assert_eq!(r.step(0.3, false, Some(egui::pos2(24.6, 24.6))), RestoreStep::Ask(vec![ViewportCommand::OuterPosition(egui::pos2(6.9, 7.7))]));
    }

    #[test]
    fn a_window_already_in_place_is_left_alone_and_it_gives_up_in_the_end() {
        let settled = |w: Win, max: bool, at: egui::Pos2| { let mut r = WinRestore::new(w); r.step(0.0, max, Some(at)); r.step(0.3, max, Some(at)) };
        assert_eq!(settled(win(100.0, 50.0, false), false, egui::pos2(100.5, 50.0)), RestoreStep::Done);
        assert_eq!(settled(win(100.0, 50.0, true), true, egui::pos2(0.0, 0.0)), RestoreStep::Done);
        // off every screen: the place is not restored, nothing to wait for
        assert_eq!(settled(win(-5000.0, 50.0, false), false, egui::pos2(30.0, 30.0)), RestoreStep::Done);
        let mut r = WinRestore::new(win(100.0, 50.0, true));
        r.step(0.0, false, None);
        r.step(0.3, false, None);
        assert_eq!(r.step(1.6, false, None), RestoreStep::Done);
    }
}
