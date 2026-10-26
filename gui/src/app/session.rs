//! Starting a new project, loading one, and the preferences.
use super::*;

impl App {
    pub fn new_project(&mut self) {
        self.project = shapes::blank_project();
        self.project.data.tools.clear(); // a new project uses the current library
        self.path.clear();
        self.reset_after_load();
        self.modified = false;
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
        self.output = None; self.gen_error = None; self.out_ver += 1;
        self.checks.clear(); self.checks_rx = None; self.check_sel = None;
        let n = self.sync_tools();
        self.sync_machine();
        self.modified = n > 0;
    }

    /// Takes the remembered settings from disk (called once at start).
    pub(super) fn apply_prefs(&mut self) {
        let p = crate::prefs::load();
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
        if let Some(v) = p.tool_len_in { self.tool_len_in = v; }
        if let Some(v) = p.tool_speed_idx { self.tool_speed_idx = v; }
        if let Some(v) = p.mach_len_in { self.mach_len_in = v; }
        if let Some(v) = p.mach_speed_idx { self.mach_speed_idx = v; }
        self.prefs_saved = p.clone();
        self.prefs = p;
    }

    /// Called every frame: writes the settings to disk when something changed (not while a mouse button is down).
    pub fn sync_prefs(&mut self, ctx: &eframe::egui::Context) {
        let mut s = self.prefs.clone();
        s.update_check = self.updater.auto;
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
        s.tool_len_in = Some(self.tool_len_in);
        s.tool_speed_idx = Some(self.tool_speed_idx);
        s.mach_len_in = Some(self.mach_len_in);
        s.mach_speed_idx = Some(self.mach_speed_idx);
        if let Some(o) = self.trace.remember.clone() { s.trace = Some(o); }
        if let Some(o) = self.model3d.remember_obj { s.model_orient_obj = Some(o); }
        if let Some(o) = self.model3d.remember_stl { s.model_orient_stl = Some(o); }
        let (inner, outer, maximized) = ctx.input(|i| { let v = i.viewport(); (v.inner_rect, v.outer_rect, v.maximized.unwrap_or(false)) });
        if let Some(r) = inner {
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
