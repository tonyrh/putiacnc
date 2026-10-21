//! Application state shared by the panels and the canvas.
use eframe::egui::{Pos2, Vec2};
use putiacnc_core::shapes;
use putiacnc_core::{generate, load_project, save_project, Output, ProjectData, ProjectFile, Units};
use std::collections::BTreeSet;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Mode { Select, Line, Rect, Ellipse }

#[derive(PartialEq, Clone, Copy)]
pub enum RightTab { Ops, Gcode, Info }

/// Screen <-> world (mm, Y up) mapping.
#[derive(Clone, Copy)]
pub struct View {
    pub scale: f32,
    pub offset: Vec2,
    pub origin: Pos2,
}

impl View {
    pub fn w2s(&self, p: [f64; 2]) -> Pos2 {
        Pos2::new(self.origin.x + self.offset.x + p[0] as f32 * self.scale, self.origin.y + self.offset.y - p[1] as f32 * self.scale)
    }
    pub fn s2w(&self, s: Pos2) -> [f64; 2] {
        [((s.x - self.origin.x - self.offset.x) / self.scale) as f64, ((self.origin.y + self.offset.y - s.y) / self.scale) as f64]
    }
}

/// Things that can discard the current project and therefore need an unsaved-changes check.
#[derive(Clone, Debug)]
pub enum Action { New, Open(String), Quit }

pub enum Drag {
    None,
    Box { start: [f64; 2] },
    Move { start: [f64; 2], anchor: [f64; 2], applied: [f64; 2] },
    Draw { start: [f64; 2] },
    /// Resizing the selection by one of its eight handles (0 = top-left, clockwise).
    Resize { handle: usize, start_bbox: [f64; 4], grab: [f64; 2], orig: Vec<(usize, putiacnc_core::model::LoopData)> },
}

/// Undo/redo as whole-project snapshots (projects are small, this is simple and robust).
#[derive(Default)]
pub struct History {
    pub undo: Vec<ProjectData>,
    pub redo: Vec<ProjectData>,
    /// True while a continuous edit (a drag or a text edit) has already been recorded.
    pub open: bool,
}

impl History {
    pub fn record(&mut self, d: &ProjectData) {
        self.undo.push(d.clone());
        if self.undo.len() > 60 { self.undo.remove(0); }
        self.redo.clear();
    }
    /// Record once per continuous edit gesture.
    pub fn record_once(&mut self, d: &ProjectData) {
        if !self.open { self.record(d); self.open = true; }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DlgKind { Open, SaveAs, ImportSvg, ExportGcode, LibExport, LibImport }

pub struct Dlg { pub kind: DlgKind, rx: std::sync::mpsc::Receiver<Option<std::path::PathBuf>> }

pub struct App {
    pub path: String,
    pub project: ProjectFile,
    pub output: Option<Output>,
    pub out_ver: u64,
    pub view_tab: crate::view3d::ViewTab,
    pub v3: crate::view3d::State,
    pub error: Option<String>,
    pub gen_error: Option<String>,
    pub imperial: bool,
    pub sel: BTreeSet<usize>,
    /// Operations whose settings are folded away in the Operations tab (ids; not saved in the project).
    pub collapsed: BTreeSet<i64>,
    /// Operation whose holding tabs are being placed on the canvas, and the tab being dragged.
    pub tab_edit: Option<i64>,
    pub tab_drag: Option<usize>,
    pub mode: Mode,
    pub hist: History,
    pub modified: bool,
    pub gen_pending: bool,
    pub gen_ms: f64,
    pub show_toolpath: bool,
    pub show_rapids: bool,
    pub show_machine_area: bool,
    pub tab: RightTab,
    pub grid_on: bool,
    pub snap_on: bool,
    pub snap_mm: f64,
    pub rect_radius: f64,
    pub scale: f32,
    pub offset: Vec2,
    pub fit_pending: bool,
    pub drag: Drag,
    pub poly: Vec<[f64; 2]>,
    pub last_title: String,
    pub show_left: bool,
    pub show_right: bool,
    pub last_min_w: f32,
    pub pending: Option<Action>,
    /// A file dialog running on its own thread (so the window keeps answering the compositor).
    pub dlg: Option<Dlg>,
    /// Action to carry on with once a Save As dialog has saved the project.
    pub after_save: Option<Action>,
    pub quit_ok: bool,
    pub want_quit: bool,
    pub theme: crate::theme::Theme,
    pub theme_checked: std::time::Instant,
    // tool manager
    pub tool_libs: putiacnc_core::toollib::ToolLibs,
    pub tools_open: bool,
    pub tool_sel: String,
    pub tool_msg: String,
    pub tool_len_in: bool,
    pub tool_speed_idx: usize,
    pub confirm_tool_del: bool,
    pub confirm_lib_del: bool,
    pub lib_name_buf: String,
    pub libs_dirty: bool,
    // machine manager
    pub machines: putiacnc_core::machines::Machines,
    pub machines_open: bool,
    pub mach_sel: String,
    pub mach_msg: String,
    pub mach_len_in: bool,
    pub mach_speed_idx: usize,
    pub confirm_mach_del: bool,
    pub machines_dirty: bool,
}

impl App {
    pub fn new(arg: Option<String>) -> Self {
        let mut a = App {
            path: String::new(), project: shapes::blank_project(), output: None, out_ver: 0, view_tab: crate::view3d::ViewTab::Design, v3: Default::default(), error: None, gen_error: None,
            imperial: false, sel: BTreeSet::new(), collapsed: BTreeSet::new(), tab_edit: None, tab_drag: None, mode: Mode::Select, hist: History::default(), modified: false,
            gen_pending: true, gen_ms: 0.0, show_toolpath: true, show_rapids: false, show_machine_area: true, tab: if std::env::var_os("PUTIACNC_TAB_GCODE").is_some() { RightTab::Gcode } else { RightTab::Ops },
            grid_on: true, snap_on: true, snap_mm: 1.0, rect_radius: 0.0,
            scale: 2.0, offset: Vec2::ZERO, fit_pending: true, drag: Drag::None, poly: vec![], last_title: String::new(),
            show_left: true, show_right: true, last_min_w: 0.0, pending: None, dlg: None, after_save: None, quit_ok: false, want_quit: false,
            theme: crate::theme::detect(), theme_checked: std::time::Instant::now(),
            tool_libs: crate::toolmgr::load_tool_libs(), tools_open: false, tool_sel: String::new(), tool_msg: String::new(),
            tool_len_in: false, tool_speed_idx: 0, confirm_tool_del: false, confirm_lib_del: false, lib_name_buf: String::new(), libs_dirty: false,
            machines: crate::machmgr::load_machines(), machines_open: false, mach_sel: String::new(), mach_msg: String::new(),
            mach_len_in: false, mach_speed_idx: 0, confirm_mach_del: false, machines_dirty: false,
        };
        a.project.data.tools.clear();
        a.sync_tools();
        a.sync_machine();
        a.modified = false;
        if let Ok(h) = std::env::var("PUTIACNC_HIDE") { a.show_left = !h.contains("left"); a.show_right = !h.contains("right"); }
        if std::env::var_os("PUTIACNC_SHOW_CONFIRM").is_some() { a.modified = true; a.pending = Some(Action::Quit); }
        if std::env::var("PUTIACNC_VIEW").map_or(false, |v| v == "3d") { a.view_tab = crate::view3d::ViewTab::Preview; }
        if let Ok(f) = std::env::var("PUTIACNC_PLAY") { a.v3.init_frac = f.parse().ok(); }
        if std::env::var_os("PUTIACNC_SHOW_TOOLS").is_some() { a.tools_open = true; }
        if std::env::var_os("PUTIACNC_SHOW_MACHINES").is_some() { a.machines_open = true; }
        if let Some(p) = arg {
            if p.to_lowercase().ends_with(".svg") { a.import_svg(&p); }
            else { a.path = p.clone(); a.open(&p); }
        }
        if std::env::var_os("PUTIACNC_SELECT_ALL").is_some() { a.sel = (0..a.project.data.loops.len()).collect(); }
        if std::env::var_os("PUTIACNC_TAB_EDIT").is_some() {
            let loops = a.project.data.loops.clone();
            if let Some(o) = a.project.data.ops.iter_mut().find(|o| o.p.tabs) {
                o.p.tab_pts = Some(putiacnc_core::shapes::auto_tab_points(&loops, o.id, 4));
                a.tab_edit = Some(o.id);
            }
            a.touch();
        }
        a
    }

    pub fn new_project(&mut self) {
        self.project = shapes::blank_project();
        self.project.data.tools.clear(); // a new project uses the current library
        self.path.clear();
        self.reset_after_load();
        self.modified = false;
    }

    fn reset_after_load(&mut self) {
        self.sel.clear();
        self.hist = History::default();
        self.drag = Drag::None;
        self.poly.clear();
        self.modified = false;
        self.fit_pending = true;
        self.gen_pending = true;
        let n = self.sync_tools();
        self.sync_machine();
        self.modified = n > 0;
    }

    pub fn open(&mut self, path: &str) {
        self.error = None;
        match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| load_project(&s)) {
            Ok(p) => { self.project = p; self.reset_after_load(); }
            Err(e) => self.error = Some(format!("Could not open {path}: {e}")),
        }
    }

    pub fn file_name(&self) -> String {
        if self.path.is_empty() { "Untitled".into() }
        else { std::path::Path::new(&self.path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| self.path.clone()) }
    }

    /// Run an action, asking first if there are unsaved changes.
    pub fn request(&mut self, a: Action) {
        if self.modified { self.pending = Some(a); } else { self.perform(a); }
    }

    pub fn perform(&mut self, a: Action) {
        self.pending = None;
        match a {
            Action::New => self.new_project(),
            Action::Open(p) => { self.path = p.clone(); self.open(&p); }
            Action::Quit => { self.quit_ok = true; self.want_quit = true; }
        }
    }

    /// Run a blocking file dialog on a worker thread; `poll_dialog` picks up the answer.
    pub fn spawn_dialog(&mut self, kind: DlgKind, f: impl FnOnce() -> Option<std::path::PathBuf> + Send + 'static) {
        if self.dlg.is_some() { return; }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || { let _ = tx.send(f()); });
        self.dlg = Some(Dlg { kind, rx });
    }

    /// Called every frame: finish a dialog that has been answered, keep the window awake while one is open.
    pub fn poll_dialog(&mut self, ctx: &eframe::egui::Context) {
        let Some(d) = &self.dlg else { return };
        let kind = d.kind;
        match d.rx.try_recv() {
            Ok(res) => {
                self.dlg = None;
                match res { Some(p) => self.finish_dialog(kind, p), None => self.after_save = None }
                ctx.request_repaint();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => { self.dlg = None; self.after_save = None; }
        }
    }

    fn finish_dialog(&mut self, kind: DlgKind, mut p: std::path::PathBuf) {
        match kind {
            DlgKind::Open => self.request(Action::Open(p.display().to_string())),
            DlgKind::SaveAs => {
                if p.extension().is_none() { p.set_extension("json"); }
                self.path = p.display().to_string();
                self.write_project();
                if !self.modified { if let Some(a) = self.after_save.take() { self.perform(a); } }
                self.after_save = None;
            }
            DlgKind::ImportSvg => self.import_svg(&p.display().to_string()),
            DlgKind::ExportGcode => {
                if p.extension().is_none() { p.set_extension("nc"); }
                if let Some(o) = &self.output {
                    match std::fs::write(&p, o.gcode()) { Ok(()) => self.error = None, Err(e) => self.error = Some(format!("Export failed: {e}")) }
                }
            }
            DlgKind::LibExport => self.finish_lib_export(p),
            DlgKind::LibImport => self.finish_lib_import(p),
        }
    }

    pub fn open_dialog(&mut self) {
        self.spawn_dialog(DlgKind::Open, || rfd::FileDialog::new().add_filter("PutiaCNC project", &["json"]).add_filter("All files", &["*"]).pick_file());
    }

    pub fn save_as_dialog(&mut self) {
        let name = if self.project.name.trim().is_empty() { "project".to_string() } else { self.project.name.trim().to_string() };
        let dir = if self.path.is_empty() { None } else { std::path::Path::new(&self.path).parent().map(|d| d.to_path_buf()) };
        self.spawn_dialog(DlgKind::SaveAs, move || {
            let mut dlg = rfd::FileDialog::new().add_filter("PutiaCNC project", &["json"]).set_file_name(format!("{name}.json"));
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.save_file()
        });
    }

    /// Save to the current path, asking for one if the project has none yet.
    pub fn save(&mut self) {
        if self.path.trim().is_empty() { self.save_as_dialog(); } else { self.write_project(); }
    }

    pub fn import_svg_dialog(&mut self) {
        self.spawn_dialog(DlgKind::ImportSvg, || rfd::FileDialog::new().add_filter("SVG drawing", &["svg", "svgz"]).add_filter("All files", &["*"]).pick_file());
    }

    /// Import an SVG file into the current project. The drawing is centred on the stock when it
    /// fits, otherwise its lower-left corner is put at the stock origin. New shapes join the
    /// active operation and become the selection.
    pub fn import_svg(&mut self, path: &str) {
        let imp = match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| putiacnc_core::svg::import(&b)) {
            Ok(i) => i,
            Err(e) => { self.error = Some(format!("Could not import {path}: {e}")); return; }
        };
        if imp.loops.is_empty() { self.error = Some(imp.warnings.join(" ")); return; }
        let Some(b) = shapes::bbox_many(imp.loops.iter()) else { return };
        let (bw, bh) = (b[2] - b[0], b[3] - b[1]);
        let st = &self.project.data.stock;
        let (ox, oy) = if bw <= st.w && bh <= st.h { ((st.w - bw) / 2.0 - b[0], (st.h - bh) / 2.0 - b[1]) } else { (-b[0], -b[1]) };
        let mut note = imp.warnings.clone();
        if bw > st.w || bh > st.h {
            note.push(format!("The drawing ({:.1} x {:.1} mm) is larger than the stock; resize it with Width/Height in the Selection panel.", bw, bh));
        }
        self.hist.record(&self.project.data);
        let op = self.project.data.act;
        let start = self.project.data.loops.len();
        for mut l in imp.loops {
            shapes::translate(&mut l, ox, oy);
            l.op = op;
            self.project.data.loops.push(l);
        }
        self.sel = (start..self.project.data.loops.len()).collect();
        self.mode = Mode::Select;
        self.error = if note.is_empty() { None } else { Some(note.join(" ")) };
        self.touch();
    }

    pub fn export_gcode_dialog(&mut self) {
        if self.output.is_none() { self.error = Some("Nothing to export: there is no G-code yet.".into()); return; }
        let name = if self.project.name.trim().is_empty() { "output".to_string() } else { self.project.name.trim().to_string() };
        self.spawn_dialog(DlgKind::ExportGcode, move || rfd::FileDialog::new().add_filter("G-code", &["nc", "gcode", "tap", "gc"]).set_file_name(format!("{name}.nc")).save_file());
    }

    fn write_project(&mut self) {
        let mut p = self.project.clone();
        // like the browser app, a project file only carries the tools its operations use
        let used: Vec<String> = p.data.ops.iter().map(|o| o.p.tool.clone()).collect();
        p.data.tools.retain(|t| used.contains(&t.id));
        if let Some(o) = &self.output { p.data.gcode = o.gcode(); }
        match save_project(&p).and_then(|s| std::fs::write(&self.path, s).map_err(|e| e.to_string())) {
            Ok(()) => { self.modified = false; self.error = None; }
            Err(e) => self.error = Some(format!("Save failed: {e}")),
        }
    }

    pub fn regenerate(&mut self) {
        self.gen_pending = false;
        let units = if self.imperial { Units::Imperial } else { Units::Metric };
        self.out_ver += 1;
        let t = std::time::Instant::now();
        match generate(&self.project.data, units) {
            Ok(o) => { self.output = Some(o); self.gen_error = None; }
            Err(e) => { self.output = None; self.gen_error = Some(e); }
        }
        self.gen_ms = t.elapsed().as_secs_f64() * 1000.0;
    }

    /// Call after any change to the project.
    pub fn touch(&mut self) { self.modified = true; self.gen_pending = true; }

    pub fn do_undo(&mut self) {
        if let Some(prev) = self.hist.undo.pop() {
            let cur = std::mem::replace(&mut self.project.data, prev);
            self.hist.redo.push(cur);
            self.after_history();
        }
    }

    pub fn do_redo(&mut self) {
        if let Some(next) = self.hist.redo.pop() {
            let cur = std::mem::replace(&mut self.project.data, next);
            self.hist.undo.push(cur);
            self.after_history();
        }
    }

    fn after_history(&mut self) {
        let n = self.project.data.loops.len();
        self.sel.retain(|&i| i < n);
        self.apply_lib_tools();
        let cur = self.machines.def.clone();
        self.project.data.machine = Some(self.machines.resolve(&self.project.data.machine.as_ref().map(|m| m.id.clone()).unwrap_or(cur)).clone());
        self.touch();
    }

    pub fn delete_selection(&mut self) {
        if self.sel.is_empty() { return; }
        self.hist.record(&self.project.data);
        for &i in self.sel.iter().rev() { self.project.data.loops.remove(i); }
        self.sel.clear();
        self.touch();
    }

    pub fn duplicate_selection(&mut self) {
        if self.sel.is_empty() { return; }
        self.hist.record(&self.project.data);
        let off = self.snap_mm.max(5.0);
        let mut new = Vec::new();
        for &i in &self.sel {
            let mut l = self.project.data.loops[i].clone();
            shapes::translate(&mut l, off, -off);
            new.push(l);
        }
        let start = self.project.data.loops.len();
        self.project.data.loops.extend(new);
        self.sel = (start..self.project.data.loops.len()).collect();
        self.touch();
    }

    pub fn nudge(&mut self, dx: f64, dy: f64) {
        if self.sel.is_empty() { return; }
        self.hist.record(&self.project.data);
        for &i in &self.sel { shapes::translate(&mut self.project.data.loops[i], dx, dy); }
        self.touch();
    }

    pub fn add_loop(&mut self, l: putiacnc_core::model::LoopData) {
        self.hist.record(&self.project.data);
        self.project.data.loops.push(l);
        self.sel.clear();
        // drawing a shape hands control back to the select tool
        self.mode = Mode::Select;
        self.poly.clear();
        self.sel.insert(self.project.data.loops.len() - 1);
        self.touch();
    }

    pub fn snap(&self, p: [f64; 2]) -> [f64; 2] {
        if self.snap_on && self.snap_mm > 1e-6 {
            [(p[0] / self.snap_mm).round() * self.snap_mm, (p[1] / self.snap_mm).round() * self.snap_mm]
        } else { p }
    }
}

pub fn fmt_time(min: f64) -> String {
    let s = (min * 60.0).round() as u64;
    format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}
