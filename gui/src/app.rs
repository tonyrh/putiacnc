//! Application state shared by the panels and the canvas.
use eframe::egui::{Pos2, Vec2};
use putiacnc_core::shapes;
use putiacnc_core::{generate, load_project, save_project, Output, ProjectData, ProjectFile, Units};
use std::collections::BTreeSet;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Mode { Select, Node, Line, Rect, Ellipse, Polygon, Point, Text, Trim }

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

/// A drag while editing points.
pub enum NodeDrag {
    None,
    /// Moving the selected points; `base` is where the grabbed point was, `applied` the shift made so far.
    Move { start: [f64; 2], base: [f64; 2], applied: [f64; 2] },
    Box { start: [f64; 2] },
}

pub enum Drag {
    None,
    Box { start: [f64; 2] },
    Move { start: [f64; 2], anchor: [f64; 2], applied: [f64; 2] },
    Draw { start: [f64; 2] },
    /// Resizing the selection by one of its eight handles (0 = top-left, clockwise).
    Resize { handle: usize, start_bbox: [f64; 4], grab: [f64; 2], orig: Vec<(usize, putiacnc_core::model::LoopData)> },
    /// Turning the selection about `pivot`: `a0` is the pointer's angle (radians) when the drag started.
    Rotate { pivot: [f64; 2], a0: f64, orig: Vec<(usize, putiacnc_core::model::LoopData)>, tabs0: Vec<(usize, usize, [f64; 2])>, total: f64 },
    /// Dragging the centre of rotation.
    Pivot,
    /// Moving clamp `idx`: where the pointer and the clamp's lower-left corner were at the start.
    Clamp { idx: usize, start: [f64; 2], orig: [f64; 2] },
    /// Moving the imported 3D model: where the pointer and the model's lower-left corner were at the start.
    Relief { start: [f64; 2], orig: [f64; 2] },
}

/// Which part of the project an edit touched.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind { Drawing, Project, Both }

/// What an undo or redo command works on: the drawing (the shapes) only, or every change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope { Drawing, All }

impl Kind {
    fn matches(self, s: Scope) -> bool { s == Scope::All || self != Kind::Project }
}

pub struct Entry {
    /// The project as it was before the edit (undo) or after it (redo).
    pub snap: ProjectData,
    /// Worked out once the edit is finished; `None` while it may still be going on.
    pub kind: Option<Kind>,
}

fn rest_eq(a: &ProjectData, b: &ProjectData) -> bool {
    a.stock == b.stock && a.job == b.job && a.machine == b.machine && a.ops == b.ops && a.tools == b.tools
        && a.clamps == b.clamps && a.relief == b.relief && a.notes == b.notes && a.act == b.act && a.oid == b.oid
}

/// Undo/redo as whole-project snapshots (projects are small, this is simple and robust). Each edit is
/// classified as a change to the drawing, to the rest of the project, or both, so that the toolbar can
/// undo drawing edits only while the Edit menu and Ctrl+Z undo every change.
#[derive(Default)]
pub struct History {
    pub undo: Vec<Entry>,
    pub redo: Vec<Entry>,
    /// True while a continuous edit (a drag or a text edit) has already been recorded.
    pub open: bool,
}

impl History {
    pub fn record(&mut self, d: &ProjectData) { self.push(d.clone()); }
    /// Record a snapshot taken before an edit.
    pub fn push(&mut self, before: ProjectData) {
        self.undo.push(Entry { snap: before, kind: None });
        if self.undo.len() > 60 { self.undo.remove(0); }
    }
    /// Record once per continuous edit gesture.
    pub fn record_once(&mut self, d: &ProjectData) {
        if !self.open { self.record(d); self.open = true; }
    }

    /// Work out what each recorded edit changed by comparing it with the state after it. Edits that
    /// changed nothing are dropped, and a new edit forgets the redo steps of the same scope.
    pub fn classify(&mut self, cur: &ProjectData) {
        if self.undo.iter().all(|e| e.kind.is_some()) { return; }
        let n = self.undo.len();
        let mut drop = vec![];
        for i in 0..n {
            if self.undo[i].kind.is_some() { continue; }
            let after = if i + 1 < n { &self.undo[i + 1].snap } else { cur };
            let before = &self.undo[i].snap;
            let (d, p) = (before.loops != after.loops || before.groups != after.groups, !rest_eq(before, after));
            match (d, p) {
                (false, false) => drop.push(i),
                (true, false) => self.undo[i].kind = Some(Kind::Drawing),
                (false, true) => self.undo[i].kind = Some(Kind::Project),
                (true, true) => self.undo[i].kind = Some(Kind::Both),
            }
            if let Some(k) = self.undo[i].kind {
                self.redo.retain(|r| match k { Kind::Both => false, Kind::Drawing => r.kind == Some(Kind::Project), Kind::Project => r.kind == Some(Kind::Drawing) });
            }
        }
        for i in drop.into_iter().rev() { self.undo.remove(i); }
    }

    fn find(list: &[Entry], s: Scope) -> Option<usize> { list.iter().rposition(|e| e.kind.is_none_or(|k| k.matches(s))) }
    pub fn can_undo(&self, s: Scope) -> bool { Self::find(&self.undo, s).is_some() }
    pub fn can_redo(&self, s: Scope) -> bool { Self::find(&self.redo, s).is_some() }
}

/// Put back the part of `snap` that an edit of this kind changed.
fn restore(cur: &mut ProjectData, mut snap: ProjectData, kind: Kind) {
    match kind {
        Kind::Both => *cur = snap,
        Kind::Drawing => { cur.loops = snap.loops; cur.groups = snap.groups; }
        Kind::Project => { snap.loops = std::mem::take(&mut cur.loops); snap.groups = std::mem::take(&mut cur.groups); *cur = snap; }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DlgKind { Open, SaveAs, Import, Model, Bitmap, ExportGcode, LibExport, LibImport, MachExport, MachImport }

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
    /// A message for the status bar, taken (and shown for a few seconds) the next time it is drawn.
    pub note: Option<String>,
    /// Set by a right-click on the drawing; the next frame treats it as the Esc key.
    pub esc_request: bool,
    pub imperial: bool,
    pub sel: BTreeSet<usize>,
    /// Operations whose settings are folded away in the Operations tab (ids; not saved in the project).
    pub collapsed: BTreeSet<i64>,
    /// Operation whose holding tabs are being placed on the canvas, and the tab being dragged.
    pub tab_edit: Option<i64>,
    pub tab_drag: Option<usize>,
    /// Shape whose points are being edited, the selected points, the running drag and the simplify settings.
    pub node_edit: Option<usize>,
    pub node_sel: BTreeSet<usize>,
    pub node_drag: NodeDrag,
    pub node_tol: f64,
    pub node_msg: String,
    /// Offset dialog: open, distance (mm), direction and whether the original shapes stay.
    pub offset_open: bool,
    pub clean: crate::cleanup::CleanTool,
    /// Mark the loose ends of open contours on the drawing.
    pub show_open_ends: bool,
    pub offset_mm: f64,
    pub offset_inward: bool,
    pub offset_keep: bool,
    pub text: crate::text::TextTool,
    pub trace: crate::trace::TraceTool,
    pub model3d: crate::model3d::ModelTool,
    /// Combine dialog: open, how, whether the originals stay and whether the last shape is the base.
    pub combine_open: bool,
    pub array: crate::array::ArrayTool,
    pub corners: crate::corners::CornerTool,
    /// What is remembered between runs: the live values and what is on disk.
    pub prefs: crate::prefs::Settings,
    pub prefs_saved: crate::prefs::Settings,
    pub shortcuts_open: bool,
    pub about_open: bool,
    pub about_icon: Option<eframe::egui::TextureHandle>,
    pub combine_op: shapes::BoolOp,
    pub combine_keep: bool,
    pub combine_swap: bool,
    pub mode: Mode,
    pub hist: History,
    pub modified: bool,
    pub gen_pending: bool,
    /// Warnings about the toolpath (collisions, gouges), computed on a background thread for `checks_ver`.
    pub checks: Vec<putiacnc_core::checks::Finding>,
    pub checks_ver: u64,
    pub checks_rx: Option<std::sync::mpsc::Receiver<(u64, Vec<putiacnc_core::checks::Finding>)>>,
    pub checks_open: bool,
    /// The warning picked in the list: drawn larger in the 3D preview.
    pub check_sel: Option<usize>,
    pub gen_ms: f64,
    pub show_toolpath: bool,
    pub show_rapids: bool,
    pub show_machine_area: bool,
    pub show_rulers: bool,
    pub show_axes: bool,
    pub show_badges: bool,
    pub tab: RightTab,
    pub grid_on: bool,
    pub snap_on: bool,
    pub snap_mm: f64,
    /// Object snap: corners, midpoints, centres and crossings of the drawing (and the stock).
    pub osnap_on: bool,
    /// How close the pointer must be to a snap point, in mm (set from the zoom every frame).
    pub snap_tol_mm: f64,
    /// Shapes the pointer must not snap to (the ones being moved or edited).
    pub snap_skip: Vec<usize>,
    /// Trim tool: false trims, true extends.
    pub trim_extend: bool,
    /// The Align panel under its palette button.
    pub align_open: bool,
    /// How much taller the tool palette is than one row (it wraps in a narrow canvas); the bars below move down by it.
    pub palette_extra: f32,
    /// The palette height the last Fit made room for, and whether the view was moved or zoomed by hand since.
    pub fit_extra: f32,
    pub view_touched: bool,
    /// Right edge of the drawing area on screen, so panels that hang off the palette can stay inside it.
    pub canvas_right: f32,
    pub canvas_left: f32,
    /// The preview of the tool whose dialog is open, and whether previews are wanted.
    pub ghost: crate::preview::Ghost,
    pub preview_on: bool,
    /// Bottom edge of the palette and the option bar under it, on screen: tool dialogs open just below.
    pub bars_bottom: f32,
    pub align_btn: Option<eframe::egui::Rect>,
    /// Ellipse tool: draw circles from the centre (size by radius or diameter) instead of ellipses by their box.
    pub circle_mode: bool,
    pub circle_diam: bool,
    /// Typed values for the shape being drawn (see dynin.rs).
    pub dyn_in: Option<crate::dynin::DynIn>,
    /// While dragging a selection: the snap point it jumped to, for the marker.
    pub snap_hit: std::cell::Cell<Option<crate::osnap::Hit>>,
    pub rect_radius: f64,
    /// Picture of the 3D model for the 2D view, and which model it is for.
    pub relief_tex: Option<(String, eframe::egui::TextureHandle)>,
    /// Polygon tool: number of sides (points for a star), star or plain polygon, and the star's inner radius in percent.
    pub poly_sides: f64,
    pub poly_star: bool,
    /// Point tool: place a ring of holes (bolt circle) instead of single points, and how many.
    pub point_ring: bool,
    /// Point tool: a rectangular grid of points (columns x rows) dragged out as a box.
    pub point_grid: bool,
    pub grid_cols: f64,
    pub grid_rows: f64,
    pub point_count: f64,
    pub point_sweep: f64,
    pub poly_inner: f64,
    pub scale: f32,
    pub offset: Vec2,
    pub fit_pending: bool,
    pub drag: Drag,
    /// The selection shows rotate handles instead of resize handles (click a selected shape again).
    pub rotate_mode: bool,
    /// Centre of rotation while in rotate mode (the selection's centre when entering it).
    pub rot_pivot: Option<[f64; 2]>,
    /// The selection the rotate mode belongs to: it ends when the selection changes.
    pub rot_sel: std::collections::BTreeSet<usize>,
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
    /// Words typed in the tool manager's search box.
    pub tool_query: String,
    pub tool_sel: String,
    pub tool_msg: String,
    pub tool_len_in: bool,
    pub tool_speed_idx: usize,
    pub confirm_tool_del: bool,
    pub confirm_lib_del: bool,
    /// the "add a ready-made library" window of the tool manager is open
    pub ready_open: bool,
    pub lib_name_buf: String,
    pub libs_dirty: bool,
    // machine manager
    pub machines: putiacnc_core::machines::Machines,
    pub machines_open: bool,
    pub updater: crate::update::Updater,
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
            path: String::new(), project: shapes::blank_project(), output: None, out_ver: 0, view_tab: crate::view3d::ViewTab::Design, v3: Default::default(), error: None, gen_error: None, note: None, esc_request: false,
            imperial: false, sel: BTreeSet::new(), collapsed: BTreeSet::new(), tab_edit: None, tab_drag: None, node_edit: None, node_sel: BTreeSet::new(), node_drag: NodeDrag::None, node_tol: 0.05, node_msg: String::new(), offset_open: false, clean: Default::default(), show_open_ends: false, offset_mm: 1.0, offset_inward: false, offset_keep: true, text: Default::default(), trace: Default::default(), model3d: Default::default(), combine_open: false, array: Default::default(), corners: Default::default(), prefs: Default::default(), prefs_saved: Default::default(), shortcuts_open: false, about_open: false, about_icon: None, combine_op: shapes::BoolOp::Union, combine_keep: false, combine_swap: false, mode: Mode::Select, hist: History::default(), modified: false,
            gen_pending: true, checks: vec![], checks_ver: 0, checks_rx: None, checks_open: std::env::var_os("PUTIACNC_SHOW_CHECKS").is_some(), check_sel: None, gen_ms: 0.0, show_toolpath: true, show_rapids: false, show_machine_area: true, show_rulers: true, show_axes: false, show_badges: true, tab: if std::env::var_os("PUTIACNC_TAB_GCODE").is_some() { RightTab::Gcode } else if std::env::var_os("PUTIACNC_TAB_NOTES").is_some() { RightTab::Info } else { RightTab::Ops },
            grid_on: true, snap_on: true, snap_mm: 1.0, osnap_on: true, snap_tol_mm: 0.5, snap_skip: Vec::new(), trim_extend: false, align_open: false, palette_extra: 0.0, fit_extra: 0.0, view_touched: false, canvas_right: 0.0, canvas_left: 0.0, ghost: Default::default(), preview_on: true, bars_bottom: 0.0, align_btn: None, circle_mode: false, circle_diam: false, dyn_in: None, snap_hit: std::cell::Cell::new(None), rect_radius: 0.0, relief_tex: None, poly_sides: 6.0, poly_star: false, point_ring: false, point_grid: false, grid_cols: 4.0, grid_rows: 3.0, point_count: 6.0, point_sweep: 360.0, poly_inner: 50.0,
            scale: 2.0, offset: Vec2::ZERO, fit_pending: true, drag: Drag::None, rotate_mode: false, rot_pivot: None, rot_sel: Default::default(), poly: vec![], last_title: String::new(),
            show_left: true, show_right: true, last_min_w: 0.0, pending: None, dlg: None, after_save: None, quit_ok: false, want_quit: false,
            theme: crate::theme::detect(), theme_checked: std::time::Instant::now(),
            tool_libs: crate::toolmgr::load_tool_libs(), tools_open: false, tool_query: String::new(), tool_sel: String::new(), tool_msg: String::new(),
            tool_len_in: false, tool_speed_idx: 0, confirm_tool_del: false, confirm_lib_del: false, ready_open: false, lib_name_buf: String::new(), libs_dirty: false,
            machines: crate::machmgr::load_machines(), machines_open: false, updater: crate::update::Updater::new(), mach_sel: String::new(), mach_msg: String::new(),
            mach_len_in: false, mach_speed_idx: 0, confirm_mach_del: false, machines_dirty: false,
        };
        a.apply_prefs();
        a.project.data.tools.clear();
        a.sync_tools();
        a.sync_machine();
        a.modified = false;
        if let Ok(h) = std::env::var("PUTIACNC_HIDE") { a.show_left = !h.contains("left"); a.show_right = !h.contains("right"); }
        if std::env::var_os("PUTIACNC_SHOW_CONFIRM").is_some() { a.modified = true; a.pending = Some(Action::Quit); }
        if std::env::var("PUTIACNC_VIEW").is_ok_and(|v| v == "3d") { a.view_tab = crate::view3d::ViewTab::Preview; }
        if let Ok(f) = std::env::var("PUTIACNC_PLAY") { a.v3.init_frac = f.parse().ok(); }
        if std::env::var_os("PUTIACNC_SHOW_TOOLS").is_some() { a.tools_open = true; }
        if let Ok(q) = std::env::var("PUTIACNC_TOOL_QUERY") { a.tool_query = q; }
        if std::env::var_os("PUTIACNC_SHOW_MACHINES").is_some() { a.machines_open = true; }
        if std::env::var_os("PUTIACNC_SHOW_UPDATE").is_some() { a.updater.open = true; a.updater.check(false); }
        else if a.updater.auto { a.updater.check(true); }
        if let Some(p) = arg {
            if App::is_drawing(&p) { a.import_file(&p); }
            else if App::is_model(&p) { a.import_model(&p); }
            else { a.path = p.clone(); a.open(&p); }
        }
        if let Ok(p) = std::env::var("PUTIACNC_IMPORT2") { if App::is_bitmap(&p) { a.import_bitmap(&p); } else if App::is_model(&p) { a.import_model(&p); } else { a.import_file(&p); } }
        if std::env::var_os("PUTIACNC_RELIEF").is_some() && a.project.data.relief.is_some() {
            let d = &mut a.project.data;
            let mut p = shapes::default_op_params(&d.tools[0].id);
            p.kind = "relief".into();
            if let Some(t) = d.tools.iter().find(|t| t.shape == "ball") { p.tool = t.id.clone(); }
            p.tool2 = d.tools.iter().filter(|t| t.shape == "flat").min_by(|x, y| (x.dia - 6.0).abs().partial_cmp(&(y.dia - 6.0).abs()).unwrap()).map(|t| t.id.clone());
            p.depth = d.relief.as_ref().map_or(5.0, |r| (r.zmax * r.scale).round().max(1.0));
            p.pass_depth = Some(2.0);
            d.oid += 1; let id = d.oid;
            d.ops.push(putiacnc_core::model::Operation { id, p });
            d.act = id;
        }
        if std::env::var_os("PUTIACNC_SELECT_ALL").is_some() { a.sel = (0..a.project.data.loops.len()).collect(); }
        if let Ok(v) = std::env::var("PUTIACNC_SELECT") { a.sel = v.split(',').filter_map(|s| s.parse().ok()).collect(); }
        if std::env::var_os("PUTIACNC_ROTATE").is_some() { a.rotate_mode = true; a.rot_sel = a.sel.clone(); }
        if std::env::var_os("PUTIACNC_SHOW_SHORTCUTS").is_some() { a.shortcuts_open = true; }
        if std::env::var_os("PUTIACNC_SHOW_ABOUT").is_some() { a.about_open = true; }
        if std::env::var_os("PUTIACNC_SHOW_ARRAY").is_some() { a.array.open = true; a.sel = (0..a.project.data.loops.len()).collect(); }
        if std::env::var_os("PUTIACNC_SHOW_CORNERS").is_some() { a.corners.open = true; a.sel = (0..a.project.data.loops.len()).collect(); }
        if std::env::var_os("PUTIACNC_SHOW_COMBINE").is_some() { a.combine_open = true; a.sel = (0..a.project.data.loops.len()).collect(); }
        if std::env::var_os("PUTIACNC_SHOW_CLEAN").is_some() { a.clean.open = true; }
        if std::env::var_os("PUTIACNC_SHOW_ALIGN").is_some() { a.align_open = true; a.sel = (0..a.project.data.loops.len()).collect(); }
        if std::env::var_os("PUTIACNC_SHOW_OFFSET").is_some() { a.offset_open = true; a.sel = (0..a.project.data.loops.len()).collect(); }
        if std::env::var_os("PUTIACNC_POLYGON").is_some() {
            a.poly_star = true;
            for l in [shapes::new_polygon([50.0, 75.0], 25.0, std::f64::consts::FRAC_PI_2, 6, None, 0), shapes::new_polygon([130.0, 75.0], 30.0, std::f64::consts::FRAC_PI_2, 5, Some(0.5), 0)].into_iter().flatten() { a.add_loop(l); }
            if std::env::var("PUTIACNC_POLYGON").is_ok_and(|v| v == "ring") {
                a.project.data.loops.extend(shapes::bolt_circle([100.0, 60.0], 30.0, 0.0, 5, 240.0, 0));
                a.mode = Mode::Point; a.point_grid = true; a.project.data.loops.extend(shapes::point_grid([40.0, 30.0], [120.0, 60.0], 5, 3, 0));
            } else if std::env::var("PUTIACNC_POLYGON").is_ok_and(|v| v == "point") {
                for x in [30.0, 60.0, 90.0] { a.project.data.loops.push(shapes::new_point([x, 30.0], 0)); }
                a.mode = Mode::Point;
            } else { a.mode = Mode::Polygon; }
            a.sel.clear();
        }
        if let Ok(s) = std::env::var("PUTIACNC_SHOW_TEXT") {
            a.mode = Mode::Text; a.text.open = true; a.text.pos = Some([10.0, 20.0]); a.text.content = s.replace("\\n", "\n");
        }
        if let Ok(s) = std::env::var("PUTIACNC_PLACE_TEXT") {
            // places a text, turned a little, and optionally opens it for editing (for screenshots)
            a.text.content = s; a.text.sel = a.text.fonts.iter().position(|f| f.label == "Hack").unwrap_or(0); a.text.pos = Some([20.0, 30.0]); a.text.size = 15.0; a.text.angle = 20.0;
            crate::text::place(&mut a);
            if std::env::var_os("PUTIACNC_EDIT_TEXT").is_some() { if let Some(g) = a.project.data.groups.last().map(|g| g.id) { a.edit_text(g); } }
        }
        if let Ok(s) = std::env::var("PUTIACNC_DYN") {
            // "x,y" screen position of the pointer when typing started (for screenshots)
            let v: Vec<f32> = s.split(',').filter_map(|t| t.parse().ok()).collect();
            if v.len() == 2 {
                a.mode = Mode::Ellipse;
                a.dyn_in = Some(crate::dynin::DynIn { kind: crate::dynin::Kind::Box, vals: vec!["2".into(), String::new(), String::new(), String::new()], active: 0, focus: true, diam: false, pos: eframe::egui::pos2(v[0], v[1]) });
            }
        }
        if let Some(i) = std::env::var("PUTIACNC_NODE_EDIT").ok().and_then(|v| v.parse::<usize>().ok()) {
            a.mode = Mode::Node;
            a.node_edit = Some(i);
            a.sel.insert(i);
            a.node_sel.extend([0usize, 1]);
        }
        if std::env::var_os("PUTIACNC_TAB_EDIT").is_some() {
            let loops = a.project.data.loops.clone();
            if let Some(o) = a.project.data.ops.iter_mut().find(|o| o.p.tabs) {
                o.p.tab_pts = Some(putiacnc_core::shapes::auto_tabs(&loops, o.id, 4));
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

    /// Takes the remembered settings from disk (called once at start).
    fn apply_prefs(&mut self) {
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

    fn remember_dir(&mut self, kind: DlgKind, p: &std::path::Path) {
        let d = crate::prefs::dir_of(p);
        if d.is_none() { return; }
        match kind {
            DlgKind::Open | DlgKind::SaveAs => self.prefs.dir_project = d,
            DlgKind::Import | DlgKind::Model | DlgKind::Bitmap => self.prefs.dir_svg = d,
            DlgKind::ExportGcode => self.prefs.dir_gcode = d,
            _ => {}
        }
    }

    pub fn open(&mut self, path: &str) {
        self.error = None;
        match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| load_project(&s)) {
            Ok(p) => { self.project = p; self.reset_after_load(); crate::prefs::push_recent(&mut self.prefs.recent, path); }
            Err(e) => { self.error = Some(format!("Could not open {path}: {e}")); self.prefs.recent.retain(|r| r != path); }
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
        self.remember_dir(kind, &p);
        match kind {
            DlgKind::Open => self.request(Action::Open(p.display().to_string())),
            DlgKind::SaveAs => {
                if p.extension().is_none() { p.set_extension("json"); }
                self.path = p.display().to_string();
                self.write_project();
                if !self.modified { if let Some(a) = self.after_save.take() { self.perform(a); } }
                self.after_save = None;
            }
            DlgKind::Import => self.import_file(&p.display().to_string()),
            DlgKind::Model => self.import_model(&p.display().to_string()),
            DlgKind::Bitmap => self.import_bitmap(&p.display().to_string()),
            DlgKind::ExportGcode => {
                if p.extension().is_none() { p.set_extension("nc"); }
                if let Some(o) = &self.output {
                    match std::fs::write(&p, o.gcode()) { Ok(()) => self.error = None, Err(e) => self.error = Some(format!("Export failed: {e}")) }
                }
            }
            DlgKind::LibExport => self.finish_lib_export(p),
            DlgKind::LibImport => self.finish_lib_import(p),
            DlgKind::MachExport => self.finish_mach_export(p),
            DlgKind::MachImport => self.finish_mach_import(p),
        }
    }

    pub fn open_dialog(&mut self) {
        let dir = crate::prefs::existing_dir(&self.prefs.dir_project);
        self.spawn_dialog(DlgKind::Open, move || {
            let mut dlg = rfd::FileDialog::new().add_filter("PutiaCNC project", &["json"]).add_filter("All files", &["*"]);
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.pick_file()
        });
    }

    pub fn save_as_dialog(&mut self) {
        let name = if self.project.name.trim().is_empty() { "project".to_string() } else { self.project.name.trim().to_string() };
        let dir = if self.path.is_empty() { crate::prefs::existing_dir(&self.prefs.dir_project) } else { std::path::Path::new(&self.path).parent().map(|d| d.to_path_buf()) };
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

    pub fn import_dialog(&mut self) {
        let dir = crate::prefs::existing_dir(&self.prefs.dir_svg).or_else(|| crate::prefs::existing_dir(&self.prefs.dir_project));
        self.spawn_dialog(DlgKind::Import, move || {
            let mut dlg = rfd::FileDialog::new().add_filter("Drawings (SVG, DXF)", &["svg", "svgz", "dxf"]).add_filter("SVG drawing", &["svg", "svgz"]).add_filter("DXF drawing", &["dxf"]).add_filter("All files", &["*"]);
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.pick_file()
        });
    }

    pub fn import_model_dialog(&mut self) {
        let dir = crate::prefs::existing_dir(&self.prefs.dir_svg).or_else(|| crate::prefs::existing_dir(&self.prefs.dir_project));
        self.spawn_dialog(DlgKind::Model, move || {
            let mut dlg = rfd::FileDialog::new().add_filter("3D models (STL, OBJ)", &["stl", "obj"]).add_filter("All files", &["*"]);
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.pick_file()
        });
    }

    pub fn import_bitmap_dialog(&mut self) {
        let dir = crate::prefs::existing_dir(&self.prefs.dir_svg).or_else(|| crate::prefs::existing_dir(&self.prefs.dir_project));
        self.spawn_dialog(DlgKind::Bitmap, move || {
            let mut dlg = rfd::FileDialog::new().add_filter("Pictures (PNG, JPG, BMP)", &["png", "jpg", "jpeg", "bmp"]).add_filter("All files", &["*"]);
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.pick_file()
        });
    }

    pub fn is_bitmap(path: &str) -> bool { let p = path.to_lowercase(); p.ends_with(".png") || p.ends_with(".jpg") || p.ends_with(".jpeg") || p.ends_with(".bmp") }

    /// Open the picture in the trace dialog; the shapes are placed when the user accepts.
    pub fn import_bitmap(&mut self, path: &str) {
        let st = (self.project.data.stock.w, self.project.data.stock.h);
        let saved = self.prefs.trace.clone();
        self.trace.load(path, st, saved);
    }

    pub fn is_model(path: &str) -> bool { let p = path.to_lowercase(); p.ends_with(".stl") || p.ends_with(".obj") }

    /// Import an STL model as a height map for relief operations. It is centred on the stock, and scaled
    /// down (keeping its proportions) when it is bigger than the stock.
    pub fn import_model(&mut self, path: &str) {
        let saved = if path.to_lowercase().ends_with(".obj") { self.prefs.model_orient_obj } else { self.prefs.model_orient_stl };
        self.model3d.load(path, saved);
    }

    /// Put a height map on the stock, centred, and scaled down (keeping its proportions) when it is bigger than the stock.
    pub fn place_relief(&mut self, rel: putiacnc_core::model::Relief) {
        let mut rel = rel;
        self.hist.record(&self.project.data);
        let (mw, mh) = rel.size();
        let mut note = vec![format!("Imported {}: {:.1} x {:.1} x {:.1} mm.", rel.name, mw, mh, rel.zmax)];
        let st = &self.project.data.stock;
        if mw > st.w || mh > st.h {
            rel.scale = (st.w / mw).min(st.h / mh);
            note.push(format!("Scaled down to {:.0}% to fit the stock ({:.1} x {:.1} mm); change the size with Width in the Imported section.", rel.scale * 100.0, rel.size().0, rel.size().1));
        }
        let (mw, mh) = rel.size();
        rel.x = (st.w - mw) / 2.0;
        rel.y = (st.h - mh) / 2.0;
        self.project.data.relief = Some(rel);
        self.error = Some(note.join(" "));
        self.touch();
    }

    /// True for the drawing formats `import_file` understands.
    pub fn is_drawing(path: &str) -> bool {
        let p = path.to_lowercase();
        p.ends_with(".svg") || p.ends_with(".svgz") || p.ends_with(".dxf")
    }

    /// Import an SVG or DXF file into the current project. The drawing is centred on the stock when it
    /// fits, otherwise its lower-left corner is put at the stock origin. New shapes are put
    /// in no operation and become the selection.
    pub fn import_file(&mut self, path: &str) {
        let dxf = path.to_lowercase().ends_with(".dxf");
        let imp = match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| if dxf { putiacnc_core::dxf::import(&b) } else { putiacnc_core::svg::import(&b) }) {
            Ok(i) => i,
            Err(e) => { self.error = Some(format!("Could not import {path}: {e}")); return; }
        };
        let gname = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "drawing".into());
        self.place_import(imp, gname);
    }

    /// Put imported shapes on the stock as one drawing: centred when it fits, otherwise its lower-left corner
    /// at the stock origin. The shapes are in no operation and become the selection.
    pub fn place_import(&mut self, imp: putiacnc_core::svg::Imported, gname: String) {
        if imp.loops.is_empty() { self.error = Some(imp.warnings.join(" ")); return; }
        let Some(b) = shapes::bbox_many(imp.loops.iter()) else { return };
        let (bw, bh) = (b[2] - b[0], b[3] - b[1]);
        let st = &self.project.data.stock;
        let (ox, oy) = if bw <= st.w && bh <= st.h { ((st.w - bw) / 2.0 - b[0], (st.h - bh) / 2.0 - b[1]) } else { (-b[0], -b[1]) };
        let mut note = imp.warnings.clone();
        let open = putiacnc_core::cleanup::open_count(&imp.loops);
        if open > 0 {
            note.push(format!("{open} open contour{} (loose ends are marked): Edit > Clean up drawing can join them.", if open == 1 { "" } else { "s" }));
            self.show_open_ends = true;
        }
        if bw > st.w || bh > st.h {
            note.push(format!("The drawing ({:.1} x {:.1} mm) is larger than the stock; resize it with Width/Height in the Selection panel.", bw, bh));
        }
        self.hist.record(&self.project.data);
        let op = 0;
        let start = self.project.data.loops.len();
        let gid = self.project.data.groups.iter().map(|g| g.id).max().unwrap_or(0) + 1;
        self.project.data.groups.push(putiacnc_core::model::ShapeGroup { id: gid, name: gname, locked: false, text: None });
        for mut l in imp.loops {
            shapes::translate(&mut l, ox, oy);
            l.op = op;
            l.grp = Some(gid);
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
        let dir = crate::prefs::existing_dir(&self.prefs.dir_gcode)
            .or_else(|| if self.path.is_empty() { None } else { std::path::Path::new(&self.path).parent().map(|d| d.to_path_buf()) })
            .or_else(|| crate::prefs::existing_dir(&self.prefs.dir_project));
        self.spawn_dialog(DlgKind::ExportGcode, move || {
            let mut dlg = rfd::FileDialog::new().add_filter("G-code", &["nc", "gcode", "tap", "gc"]).set_file_name(format!("{name}.nc"));
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.save_file()
        });
    }

    fn write_project(&mut self) {
        let mut p = self.project.clone();
        // like the browser app, a project file only carries the tools its operations use
        let used: Vec<String> = p.data.ops.iter().map(|o| o.p.tool.clone()).collect();
        p.data.tools.retain(|t| used.contains(&t.id));
        if let Some(o) = &self.output { p.data.gcode = o.gcode(); }
        match save_project(&p).and_then(|s| std::fs::write(&self.path, s).map_err(|e| e.to_string())) {
            Ok(()) => { self.modified = false; self.error = None; let p = self.path.clone(); crate::prefs::push_recent(&mut self.prefs.recent, &p); }
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
        // the warnings are worked out beside the UI: they replay the whole program
        self.checks.clear(); self.check_sel = None;
        self.checks_rx = None;
        if let Some(o) = self.output.clone() {
            let (tx, rx) = std::sync::mpsc::channel();
            let (data, ver) = (self.project.data.clone(), self.out_ver);
            std::thread::spawn(move || { let f = putiacnc_core::checks::run(&data, &o); if std::env::var_os("PUTIACNC_DEBUG_CHECKS").is_some() { eprintln!("checks: {} findings, tool0 flute {:?}", f.len(), data.tools.first().map(|t| (t.flute, t.shank))); } let _ = tx.send((ver, f)); });
            self.checks_rx = Some(rx);
        }
    }

    /// Pick up finished warnings; true while they are still being worked out.
    pub fn poll_checks(&mut self) -> bool {
        let Some(rx) = &self.checks_rx else { return false };
        match rx.try_recv() {
            Ok((v, f)) => {
                if v == self.out_ver { self.checks = f; self.checks_ver = v; }
                self.checks_rx = None;
                // testing aid: press "Show in 3D" on this warning
                if let Some(i) = std::env::var("PUTIACNC_CHECK_SHOW").ok().and_then(|s| s.parse::<usize>().ok()) { if i < self.checks.len() { crate::checkui::show_in_3d(self, i); } }
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => { self.checks_rx = None; false }
        }
    }

    /// Call after any change to the project.
    pub fn touch(&mut self) {
        self.prune_empty_ops();
        self.modified = true;
        self.gen_pending = true;
    }

    /// An operation that has lost its last shape is removed: nothing can put shapes back into it, so it could only sit there empty.
    /// The status bar says so, and undo brings it back. A 3D relief works on the model, not on shapes, so it stays.
    fn prune_empty_ops(&mut self) {
        let d = &mut self.project.data;
        let gone: Vec<(usize, i64)> = d.ops.iter().enumerate().filter(|(_, o)| o.p.kind != "relief" && !d.loops.iter().any(|l| l.op == o.id)).map(|(i, o)| (i, o.id)).collect();
        if gone.is_empty() { return; }
        let msg = if gone.len() == 1 {
            let (i, _) = gone[0];
            format!("Operation {} ({}) removed: it had no shapes left. Ctrl+Z brings it back", i + 1, crate::panels::label_of(crate::panels::KINDS, &d.ops[i].p.kind))
        } else {
            format!("{} operations removed: they had no shapes left. Ctrl+Z brings them back", gone.len())
        };
        for &(_, id) in gone.iter().rev() {
            if let Some(i) = d.ops.iter().position(|o| o.id == id) { d.ops.remove(i); }
            if d.act == id { d.act = d.ops.last().map(|o| o.id).unwrap_or(0); }
        }
        self.note = Some(msg);
    }

    /// Undo the last change of any kind (Edit menu, Ctrl+Z).
    pub fn do_undo(&mut self) { self.step(Scope::All, true); }
    pub fn do_redo(&mut self) { self.step(Scope::All, false); }
    /// Undo the last change to the drawing, leaving stock, operations, tools and the rest alone (toolbar).
    pub fn do_undo_drawing(&mut self) { self.step(Scope::Drawing, true); }
    pub fn do_redo_drawing(&mut self) { self.step(Scope::Drawing, false); }

    fn step(&mut self, s: Scope, undo: bool) {
        self.hist.classify(&self.project.data);
        let list = if undo { &mut self.hist.undo } else { &mut self.hist.redo };
        let Some(i) = History::find(list, s) else { return };
        let e = list.remove(i);
        let kind = e.kind.unwrap_or(Kind::Both);
        let before = self.project.data.clone();
        // a placed text is selected as a whole, but its letters may be more or fewer after the step: follow the text, not the indices
        let is_text = |d: &ProjectData, g: i64| d.groups.iter().any(|x| x.id == g && x.text.is_some());
        let mut texts: Vec<i64> = self.sel.iter().filter_map(|&i| before.loops.get(i).and_then(|l| l.grp)).filter(|&g| is_text(&before, g)).collect();
        texts.sort_unstable();
        texts.dedup();
        let others: std::collections::BTreeSet<usize> = self.sel.iter().copied().filter(|&i| !before.loops.get(i).and_then(|l| l.grp).is_some_and(|g| texts.contains(&g))).collect();
        restore(&mut self.project.data, e.snap, kind);
        let other = if undo { &mut self.hist.redo } else { &mut self.hist.undo };
        other.push(Entry { snap: before, kind: Some(kind) });
        self.sel = others;
        self.after_history();
        let d = &self.project.data;
        for g in texts.into_iter().filter(|&g| is_text(d, g)) {
            self.sel.extend(d.loops.iter().enumerate().filter(|(_, l)| l.grp == Some(g)).map(|(k, _)| k));
        }
    }

    fn after_history(&mut self) {
        let n = self.project.data.loops.len();
        self.sel.retain(|&i| i < n);
        self.apply_lib_tools();
        let cur = self.machines.def.clone();
        self.project.data.machine = Some(self.machines.resolve(&self.project.data.machine.as_ref().map(|m| m.id.clone()).unwrap_or(cur)).clone());
        self.apply_machine_job();
        self.touch();
    }

    pub fn delete_selection(&mut self) {
        if self.sel.is_empty() { return; }
        self.hist.record(&self.project.data);
        for &i in self.sel.iter().rev() { self.project.data.loops.remove(i); }
        let removed: Vec<usize> = self.sel.iter().copied().collect();
        shapes::tabs_after_delete(&mut self.project.data.ops, &removed);
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
            l.grp = None; // a copy is its own shape, not part of the imported file
            shapes::translate(&mut l, off, -off);
            new.push(l);
        }
        let start = self.project.data.loops.len();
        self.project.data.loops.extend(new);
        let src: Vec<usize> = self.sel.iter().copied().collect();
        for (k, &i) in src.iter().enumerate() { shapes::tabs_copy(&mut self.project.data.ops, i, start + k); }
        self.sel = (start..self.project.data.loops.len()).collect();
        self.touch();
    }

    /// Grows (or shrinks) the selected closed shapes by `dist` mm. With `keep` the originals stay and the
    /// results are added; otherwise a result replaces its original. Returns a note when something was skipped.
    pub fn offset_selection(&mut self, dist: f64, keep: bool) -> Option<String> {
        if self.sel.is_empty() || dist.abs() < 1e-9 { return None; }
        self.hist.record(&self.project.data);
        let (mut skipped, mut vanished) = (0, 0);
        let mut added: Vec<putiacnc_core::model::LoopData> = vec![];
        let mut replaced: Vec<usize> = vec![];
        let mut removed: Vec<usize> = vec![];
        let sel: Vec<usize> = self.sel.iter().copied().collect();
        for i in sel {
            let l = self.project.data.loops[i].clone();
            if !l.closed() { skipped += 1; continue; }
            let res = shapes::offset_loop(&l, dist);
            if res.is_empty() { vanished += 1; continue; }
            if keep { added.extend(res); }
            else if res.len() == 1 { self.project.data.loops[i] = res.into_iter().next().unwrap(); replaced.push(i); }
            else { removed.push(i); added.extend(res); }
        }
        removed.sort_unstable();
        for &i in removed.iter().rev() { self.project.data.loops.remove(i); }
        shapes::tabs_after_delete(&mut self.project.data.ops, &removed);
        let shift = |i: usize| i - removed.iter().filter(|&&r| r < i).count();
        let start = self.project.data.loops.len();
        self.project.data.loops.extend(added);
        self.sel = replaced.iter().map(|&i| shift(i)).chain(start..self.project.data.loops.len()).collect();
        self.touch();
        let mut notes = vec![];
        if skipped > 0 { notes.push(format!("{skipped} open shape{} cannot be offset", if skipped == 1 { "" } else { "s" })); }
        if vanished > 0 { notes.push(format!("{vanished} shape{} too small to shrink by {} mm", if vanished == 1 { " is" } else { "s are" }, dist.abs())); }
        if notes.is_empty() { None } else { Some(notes.join("; ") + ".") }
    }

    /// Replaces the selected closed shapes by their union, difference, intersection or exclusion.
    /// Returns a message when nothing could be done or something was left out.
    pub fn combine_selection(&mut self, op: shapes::BoolOp, keep: bool, swap: bool) -> Option<String> {
        let idx: Vec<usize> = self.sel.iter().copied().filter(|&i| self.project.data.loops.get(i).is_some_and(|l| l.closed())).collect();
        let open = self.sel.len() - idx.len();
        if idx.len() < 2 { return Some("Select at least two closed shapes.".into()); }
        let result = { let refs: Vec<&putiacnc_core::model::LoopData> = idx.iter().map(|&i| &self.project.data.loops[i]).collect(); shapes::combine(&refs, op, swap) };
        if result.is_empty() { return Some("Nothing is left: the shapes do not overlap that way.".into()); }
        self.hist.record(&self.project.data);
        if !keep {
            for &i in idx.iter().rev() { self.project.data.loops.remove(i); }
            shapes::tabs_after_delete(&mut self.project.data.ops, &idx);
        }
        let start = self.project.data.loops.len();
        self.project.data.loops.extend(result);
        self.sel = (start..self.project.data.loops.len()).collect();
        self.touch();
        if open > 0 { Some(format!("{open} open shape{} left out.", if open == 1 { " was" } else { "s were" })) } else { None }
    }

    /// True when a selected shape belongs to a locked imported drawing.
    pub fn sel_locked(&self) -> bool {
        let d = &self.project.data;
        self.sel.iter().filter_map(|&i| d.loops.get(i)).filter_map(|l| l.grp).any(|g| d.groups.iter().any(|x| x.id == g && x.locked))
    }

    /// True when this shape belongs to a locked imported drawing.
    pub fn loop_locked(&self, i: usize) -> bool {
        let d = &self.project.data;
        d.loops.get(i).and_then(|l| l.grp).is_some_and(|g| d.groups.iter().any(|x| x.id == g && x.locked))
    }

    pub fn nudge(&mut self, dx: f64, dy: f64) {
        if self.sel.is_empty() || self.sel_locked() { return; }
        self.rotate_mode = false;
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

    /// The object snap point near `p`, if object snap is on and one is close.
    pub fn object_snap(&self, p: [f64; 2]) -> Option<crate::osnap::Hit> {
        if !self.osnap_on { return None; }
        let st = &self.project.data.stock;
        crate::osnap::find(&self.project.data.loops, &self.snap_skip, (st.w, st.h), p, self.snap_tol_mm)
    }

    pub fn snap(&self, p: [f64; 2]) -> [f64; 2] {
        if let Some(h) = self.object_snap(p) { return h.p; }
        self.grid_snap(p)
    }

    pub fn grid_snap(&self, p: [f64; 2]) -> [f64; 2] {
        if self.snap_on && self.snap_mm > 1e-6 {
            [(p[0] / self.snap_mm).round() * self.snap_mm, (p[1] / self.snap_mm).round() * self.snap_mm]
        } else { p }
    }
}

pub fn fmt_time(min: f64) -> String {
    let s = (min * 60.0).round() as u64;
    format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

#[cfg(test)]
mod offset_tests {
    use super::*;

    fn app_with_shapes() -> App {
        let mut a = App::new(None);
        a.project.data.loops.clear();
        a.project.data.loops.push(shapes::new_rect([0.0, 0.0], [20.0, 10.0], 0.0, 1));
        a.project.data.loops.push(shapes::new_circle([50.0, 5.0], 4.0, 1));
        a.project.data.loops.push(shapes::new_poly(&[[0.0, 30.0], [10.0, 30.0], [10.0, 40.0]], false, 1).unwrap());
        a
    }

    #[test]
    fn an_operation_that_loses_its_last_shape_is_removed_and_undo_brings_it_back() {
        let mut a = App::new(None);
        a.project.data.loops.clear();
        a.project.data.loops.push(shapes::new_rect([0.0, 0.0], [20.0, 10.0], 0.0, 1));
        a.project.data.loops.push(shapes::new_circle([50.0, 5.0], 4.0, 1));
        a.project.data.ops.clear();
        a.project.data.oid = 1;
        a.project.data.ops.push(putiacnc_core::model::Operation { id: 1, p: shapes::default_op_params("") });
        let id = 1;
        a.project.data.loops.iter_mut().for_each(|l| l.op = id);
        a.sel = [0].into_iter().collect();
        a.delete_selection();
        assert_eq!(a.project.data.ops.len(), 1, "one shape is left: the operation stays");
        assert!(a.note.is_none());
        a.sel = [0].into_iter().collect();
        a.delete_selection();
        assert!(a.project.data.ops.is_empty(), "no shape is left: the operation goes");
        assert!(a.note.as_deref().is_some_and(|n| n.contains("removed")));
        a.do_undo();
        assert_eq!(a.project.data.ops.len(), 1);
        assert_eq!(a.project.data.loops.len(), 1);
    }

    #[test]
    fn keep_originals_adds_the_results_and_selects_them() {
        let mut a = app_with_shapes();
        a.sel = [0, 1, 2].into_iter().collect();
        let note = a.offset_selection(2.0, true).unwrap();
        assert!(note.contains("open shape"));
        assert_eq!(a.project.data.loops.len(), 5);
        assert_eq!(a.sel.iter().copied().collect::<Vec<_>>(), vec![3, 4]);
        a.do_undo();
        assert_eq!(a.project.data.loops.len(), 3);
    }

    #[test]
    fn replacing_keeps_indices_and_leaves_vanishing_shapes_alone() {
        let mut a = app_with_shapes();
        a.sel = [0, 1].into_iter().collect();
        // neither the circle (r = 4) nor the rectangle (10 high) can shrink by 5.5: both stay as they were
        let note = a.offset_selection(-5.5, false).unwrap();
        assert!(note.contains("too small"), "{note}");
        assert_eq!(a.project.data.loops.len(), 3);
        assert!(a.project.data.loops[0].rc.is_some(), "untouched original");
        // shrink the rectangle by 1: replaced in place
        a.sel = [0].into_iter().collect();
        assert!(a.offset_selection(-1.0, false).is_none());
        let b = shapes::bbox(&a.project.data.loops[0]).unwrap();
        assert!((b[0] - 1.0).abs() < 0.02 && (b[2] - 19.0).abs() < 0.02);
        assert_eq!(a.sel.iter().copied().collect::<Vec<_>>(), vec![0]);
    }
}

#[cfg(test)]
mod combine_app_tests {
    use super::*;

    #[test]
    fn combining_replaces_the_selection_undoes_and_remaps_tabs() {
        let mut a = App::new(None);
        a.project.data.loops.clear();
        a.project.data.loops.push(shapes::new_rect([0.0, 0.0], [10.0, 10.0], 0.0, 1));
        a.project.data.loops.push(shapes::new_rect([5.0, 0.0], [15.0, 10.0], 0.0, 1));
        a.project.data.loops.push(shapes::new_circle([50.0, 5.0], 4.0, 1));
        a.project.data.loops.push(shapes::new_poly(&[[0.0, 30.0], [10.0, 30.0], [10.0, 40.0]], false, 1).unwrap());
        a.sel = [0, 1, 3].into_iter().collect();
        let note = a.combine_selection(shapes::BoolOp::Union, false, false).unwrap();
        assert!(note.contains("open shape"), "{note}");
        // the circle and the open path stay (indices 0, 1 after the removals), the union is last
        assert_eq!(a.project.data.loops.len(), 3);
        assert_eq!(a.sel.iter().copied().collect::<Vec<_>>(), vec![2]);
        let b = shapes::bbox(&a.project.data.loops[2]).unwrap();
        assert!((b[2] - b[0] - 15.0).abs() < 0.01);
        a.do_undo();
        assert_eq!(a.project.data.loops.len(), 4);
        // a single closed shape or an empty result changes nothing
        a.sel = [0].into_iter().collect();
        assert!(a.combine_selection(shapes::BoolOp::Union, false, false).is_some());
        a.sel = [0, 2].into_iter().collect();
        assert!(a.combine_selection(shapes::BoolOp::Intersect, false, false).unwrap().contains("overlap"));
        assert_eq!(a.project.data.loops.len(), 4);
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;

    #[test]
    fn clamps_are_undoable_project_changes() {
        let mut a = App::new(None);
        a.hist.record(&a.project.data);
        a.project.data.clamps.push(putiacnc_core::model::Clamp { x: 1.0, y: 2.0, w: 3.0, h: 4.0, height: 5.0 });
        a.hist.classify(&a.project.data);
        assert!(a.hist.can_undo(Scope::All));
        assert!(!a.hist.can_undo(Scope::Drawing));
        a.do_undo();
        assert!(a.project.data.clamps.is_empty());
        a.do_redo();
        assert_eq!(a.project.data.clamps.len(), 1);
    }

    #[test]
    fn toolbar_undo_only_touches_the_drawing_and_the_menu_reaches_the_rest() {
        let mut a = App::new(None);
        a.project.data.loops.clear();
        let w0 = a.project.data.stock.w;
        // 1: draw a shape, 2: change the stock, 3: draw another shape
        a.hist.record(&a.project.data);
        a.project.data.loops.push(shapes::new_circle([20.0, 20.0], 4.0, 1));
        a.hist.classify(&a.project.data);
        a.hist.record(&a.project.data);
        a.project.data.stock.w = w0 + 50.0;
        a.hist.classify(&a.project.data);
        a.hist.record(&a.project.data);
        a.project.data.loops.push(shapes::new_circle([60.0, 20.0], 4.0, 1));
        a.hist.classify(&a.project.data);

        assert!(a.hist.can_undo(Scope::Drawing) && a.hist.can_undo(Scope::All));
        // the toolbar steps back over the stock change without touching it
        a.do_undo_drawing();
        assert_eq!(a.project.data.loops.len(), 1);
        assert_eq!(a.project.data.stock.w, w0 + 50.0);
        a.do_undo_drawing();
        assert_eq!(a.project.data.loops.len(), 0);
        assert_eq!(a.project.data.stock.w, w0 + 50.0);
        assert!(!a.hist.can_undo(Scope::Drawing));
        // the menu still has the stock change to undo; the drawing stays as it is now
        assert!(a.hist.can_undo(Scope::All));
        a.do_undo();
        assert_eq!(a.project.data.stock.w, w0);
        assert_eq!(a.project.data.loops.len(), 0);
        assert!(!a.hist.can_undo(Scope::All));
        // the menu redo goes forward through everything, in order
        a.do_redo();
        assert_eq!(a.project.data.stock.w, w0 + 50.0);
        a.do_redo();
        a.do_redo();
        assert_eq!(a.project.data.loops.len(), 2);
        assert!(!a.hist.can_redo(Scope::All));

        // the menu undo alone walks back through all three edits
        a.do_undo(); a.do_undo(); a.do_undo();
        assert_eq!((a.project.data.loops.len(), a.project.data.stock.w), (0, w0));
    }

    #[test]
    fn notes_edits_can_be_undone_from_the_menu_but_not_the_toolbar() {
        let mut a = App::new(None);
        a.hist.record(&a.project.data);
        a.project.data.notes = "clamp at the left".into();
        a.hist.classify(&a.project.data);
        assert!(!a.hist.can_undo(Scope::Drawing));
        a.do_undo();
        assert_eq!(a.project.data.notes, "");
        a.do_redo();
        assert_eq!(a.project.data.notes, "clamp at the left");
    }

    #[test]
    fn a_new_drawing_edit_keeps_the_project_redo() {
        let mut a = App::new(None);
        a.project.data.loops.clear();
        let w0 = a.project.data.stock.w;
        a.hist.record(&a.project.data);
        a.project.data.stock.w = w0 + 10.0;
        a.hist.classify(&a.project.data);
        a.do_undo();
        assert!(a.hist.can_redo(Scope::All));
        a.hist.record(&a.project.data);
        a.project.data.loops.push(shapes::new_circle([20.0, 20.0], 4.0, 1));
        a.hist.classify(&a.project.data);
        // the stock redo survives for the menu; the toolbar redo has nothing
        assert!(a.hist.can_redo(Scope::All) && !a.hist.can_redo(Scope::Drawing));
        a.do_redo();
        assert_eq!(a.project.data.stock.w, w0 + 10.0);
        assert_eq!(a.project.data.loops.len(), 1);
        // an edit that changes nothing leaves no step behind
        let n = a.hist.undo.len();
        a.hist.record(&a.project.data);
        a.hist.classify(&a.project.data);
        assert_eq!(a.hist.undo.len(), n);
    }
}
