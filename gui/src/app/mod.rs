//! Application state shared by the panels and the canvas.
use eframe::egui::{Pos2, Vec2};
use putiacnc_core::shapes;
use putiacnc_core::{load_project, save_project, Output, ProjectData, ProjectFile, Units};
mod edit;
mod files;
mod generate;
mod session;
use std::collections::BTreeSet;
pub use putiacnc_core::history::{History, Scope};

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DlgKind { Open, SaveAs, Import, Model, Bitmap, ExportGcode, LibExport, LibImport, MachExport, MachImport }

pub struct Dlg { pub kind: DlgKind, rx: std::sync::mpsc::Receiver<Option<std::path::PathBuf>> }

/// A separate piece of stock of the job (see `putiacnc_core::pieces`): its own project, in the piece's own frame, and its toolpath.
#[derive(Clone)]
pub struct PiecePart { pub job: putiacnc_core::pieces::PieceJob, pub output: Output }

/// How serious a banner message is: an error (something failed), a warning (something is wrong and can be fixed) or news.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Level { Error, Warning, Info }

/// The windows of the tools that work on the selection. Only one tool is active at a time: starting another closes the first.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ToolWin { Align, Offset, Array, Corners, Combine, Clean }

/// What has to happen to the shapes of a group (by id) for a banner sentence about it to be dropped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoteUntil {
    /// the group fits the stock
    Fits(i64),
    /// the group has no open contour left
    Closed(i64),
    /// the group is no longer a placed text (converted to outlines) or is gone
    NotText(i64),
}

pub struct App {
    pub path: String,
    pub project: ProjectFile,
    pub output: Option<Output>,
    /// The plugs that are cut from a separate piece of stock, each with its own job and toolpath (the main one is `output`).
    pub pieces: Vec<PiecePart>,
    pub out_ver: u64,
    /// The "What this will do" lines of the G-code tab, with the output version and piece they were made for.
    pub summary: Option<((u64, usize), Vec<putiacnc_core::summary::Row>)>,
    pub view_tab: crate::view3d::ViewTab,
    pub v3: crate::view3d::State,
    pub error: Option<String>,
    /// The level of the banner message; it counts only while `error` still holds exactly that text, otherwise the message is an error.
    pub banner_level: Option<(Level, String)>,
    pub gen_error: Option<String>,
    /// A message for the status bar, taken (and shown for a few seconds) the next time it is drawn.
    pub note: Option<String>,
    /// Set by a right-click on the drawing; the next frame treats it as the Esc key.
    pub esc_request: bool,
    pub imperial: bool,
    /// Which G-code the G-code tab shows: 0 the main piece, 1.. the plug pieces.
    pub gcode_part: usize,
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
    /// Sentences of the banner that describe the state of a drawing, with the condition that makes each obsolete: they leave the
    /// banner as soon as the problem is fixed.
    pub notes: Vec<(NoteUntil, String)>,
    /// What the last Apply of a selection tool had to say when it changed nothing: shown in the tool's own window, not in the banner.
    pub tool_note: Option<(&'static str, String)>,
    /// The selection tool windows that were open at the end of the last frame (see `one_tool_window`).
    pub tool_wins_seen: Vec<ToolWin>,
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
    /// The OpenGL renderer and version, read once from the first frame (for Help > About > Copy system info).
    pub gl_info: String,
    /// What the zoom card asked for this frame, applied by the drawing or the 3D view.
    pub zoom_act: Option<crate::zoomcard::Act>,
    /// The operations that had no shapes after the last change (so the status bar mentions each one only once).
    pub empty_ops: Vec<i64>,
    pub about_icon: Option<eframe::egui::TextureHandle>,
    pub combine_op: shapes::BoolOp,
    pub combine_keep: bool,
    pub combine_swap: bool,
    pub mode: Mode,
    pub hist: History,
    pub modified: bool,
    pub gen_pending: bool,
    /// The toolpath being worked out on a worker thread (at most one at a time; changes made meanwhile set `gen_pending` and are picked up
    /// when it finishes), with the version it was started for and when.
    pub gen_rx: Option<std::sync::mpsc::Receiver<(Result<Vec<putiacnc_core::pieces::PartOutput>, String>, f64)>>,
    pub gen_started: Option<std::time::Instant>,
    /// Warnings about the toolpath (collisions, gouges), computed on a background thread for `checks_ver`.
    pub checks: Vec<putiacnc_core::checks::Finding>,
    pub checks_ver: u64,
    /// Which part each warning belongs to (0 the main stock, 1.. a plug piece), parallel to `checks`.
    pub check_parts: Vec<usize>,
    /// The clamps the toolpath runs into, from the last finished check, with the operations that hit them. It stays on screen while a
    /// newer toolpath is being worked out, so the red highlights do not blink off and on with every change.
    pub clamp_hits: Vec<(usize, Vec<usize>)>,
    pub checks_rx: Option<std::sync::mpsc::Receiver<(u64, Vec<(usize, putiacnc_core::checks::Finding)>)>>,
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
    pub show_clamps: bool,
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
    /// Fit the 3D view (menu or F key while the 3D preview is open).
    pub fit3d_pending: bool,
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
    /// Panels hidden for now because the window is too narrow for them (left, right); `show_left` / `show_right` keep what the user chose.
    pub squeezed: (bool, bool),
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
    /// Speed unit (index into `SPEED_UNITS`) of the tool library window and the custom feeds block: one for each view of the lengths (mm, in).
    pub tool_speed_mm: usize,
    pub tool_speed_in: usize,
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
            path: String::new(), project: shapes::blank_project(), output: None, pieces: vec![], out_ver: 0, summary: None, view_tab: crate::view3d::ViewTab::Design, v3: Default::default(), error: None, gen_error: None, note: None, esc_request: false,
            imperial: false, gcode_part: 0, sel: BTreeSet::new(), collapsed: BTreeSet::new(), tab_edit: None, tab_drag: None, node_edit: None, node_sel: BTreeSet::new(), node_drag: NodeDrag::None, node_tol: 0.05, node_msg: String::new(), offset_open: false, clean: Default::default(), show_open_ends: false, notes: Vec::new(), tool_note: None, banner_level: None, tool_wins_seen: Vec::new(), offset_mm: 1.0, offset_inward: false, offset_keep: true, text: Default::default(), trace: Default::default(), model3d: Default::default(), combine_open: false, array: Default::default(), corners: Default::default(), prefs: Default::default(), prefs_saved: Default::default(), shortcuts_open: false, about_open: false, gl_info: String::new(), zoom_act: None, empty_ops: vec![], about_icon: None, combine_op: shapes::BoolOp::Union, combine_keep: false, combine_swap: false, mode: Mode::Select, hist: History::default(), modified: false,
            gen_pending: true, gen_rx: None, gen_started: None, checks: vec![], checks_ver: 0, check_parts: vec![], clamp_hits: vec![], checks_rx: None, checks_open: std::env::var_os("PUTIACNC_SHOW_CHECKS").is_some(), check_sel: None, gen_ms: 0.0, show_toolpath: true, show_rapids: false, show_machine_area: true, show_rulers: true, show_axes: false, show_badges: true, show_clamps: true, tab: if std::env::var_os("PUTIACNC_TAB_GCODE").is_some() { RightTab::Gcode } else if std::env::var_os("PUTIACNC_TAB_NOTES").is_some() { RightTab::Info } else { RightTab::Ops },
            grid_on: true, snap_on: true, snap_mm: 1.0, osnap_on: true, snap_tol_mm: 0.5, snap_skip: Vec::new(), trim_extend: false, align_open: false, palette_extra: 0.0, fit_extra: 0.0, view_touched: false, canvas_right: 0.0, canvas_left: 0.0, ghost: Default::default(), preview_on: true, bars_bottom: 0.0, align_btn: None, circle_mode: false, circle_diam: false, dyn_in: None, snap_hit: std::cell::Cell::new(None), rect_radius: 0.0, relief_tex: None, poly_sides: 6.0, poly_star: false, point_ring: false, point_grid: false, grid_cols: 4.0, grid_rows: 3.0, point_count: 6.0, point_sweep: 360.0, poly_inner: 50.0,
            scale: 2.0, offset: Vec2::ZERO, fit_pending: true, fit3d_pending: false, drag: Drag::None, rotate_mode: false, rot_pivot: None, rot_sel: Default::default(), poly: vec![], last_title: String::new(),
            show_left: true, show_right: true, squeezed: (false, false), last_min_w: 0.0, pending: None, dlg: None, after_save: None, quit_ok: false, want_quit: false,
            theme: crate::theme::detect(), theme_checked: std::time::Instant::now(),
            tool_libs: crate::toolmgr::load_tool_libs(), tools_open: false, tool_query: String::new(), tool_sel: String::new(), tool_msg: String::new(),
            tool_len_in: false, tool_speed_mm: 0, tool_speed_in: 3, confirm_tool_del: false, confirm_lib_del: false, ready_open: false, lib_name_buf: String::new(), libs_dirty: false,
            machines: crate::machmgr::load_machines(), machines_open: false, updater: crate::update::Updater::new(), mach_sel: String::new(), mach_msg: String::new(),
            mach_len_in: false, mach_speed_idx: 0, confirm_mach_del: false, machines_dirty: false,
        };
        a.apply_prefs();
        a.project.data.tools.clear();
        a.sync_tools();
        a.sync_machine();
        a.modified = false;
        if let Ok(h) = std::env::var("PUTIACNC_HIDE") { a.show_left = !h.contains("left"); a.show_right = !h.contains("right"); a.show_machine_area = !h.contains("machine"); if h.contains("toolpaths") { a.show_toolpath = false; } }
        if std::env::var_os("PUTIACNC_SHOW_CONFIRM").is_some() { a.modified = true; a.pending = Some(Action::Quit); }
        if std::env::var("PUTIACNC_VIEW").is_ok_and(|v| v == "3d") { a.view_tab = crate::view3d::ViewTab::Preview; }
        // PUTIACNC_PART=n: the 3D view shows a separate piece of stock (1 = the first), 0 = the main stock
        if let Some(n) = std::env::var("PUTIACNC_PART").ok().and_then(|v| v.parse::<usize>().ok()) { a.v3.part = n; }
        // PUTIACNC_GCODE_PART=n: the G-code tab shows that piece (1 = the first plug piece)
        if let Some(n) = std::env::var("PUTIACNC_GCODE_PART").ok().and_then(|v| v.parse::<usize>().ok()) { a.gcode_part = n; }
        if let Ok(f) = std::env::var("PUTIACNC_PLAY") { a.v3.init_frac = f.parse().ok(); }
        if let Some(i) = std::env::var("PUTIACNC_SPEED").ok().and_then(|v| v.parse::<usize>().ok()) { a.v3.speed_idx = i.min(crate::view3d::SPEEDS.len() - 1); }
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
        if std::env::var("PUTIACNC_TRACE_MODE").is_ok_and(|v| v == "centre") { a.trace.opts.mode = putiacnc_core::bitmap::Mode::Centerline; }
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
        // PUTIACNC_DESELECT: nothing is selected, whichever tool window the lines above opened
        if std::env::var_os("PUTIACNC_DESELECT").is_some() { a.sel.clear(); }
        // an Apply that changes nothing, to see where its message goes: array | combine | corners
        if let Ok(which) = std::env::var("PUTIACNC_APPLY_NOTHING") {
            a.project.data.loops.clear();
            match which.as_str() {
                "combine" => {
                    a.project.data.loops.push(shapes::new_rect([20.0, 20.0], [60.0, 60.0], 0.0, 0));
                    a.project.data.loops.push(shapes::new_rect([100.0, 20.0], [140.0, 60.0], 0.0, 0));
                    a.combine_open = true; a.combine_op = shapes::BoolOp::Intersect;
                }
                "corners" => { a.project.data.loops.push(shapes::new_circle([60.0, 60.0], 30.0, 0)); a.corners.open = true; }
                _ => { a.project.data.loops.push(shapes::new_rect([20.0, 20.0], [60.0, 60.0], 0.0, 0)); a.array.open = true; a.array.cols = 1.0; a.array.rows = 1.0; }
            }
            a.sel = (0..a.project.data.loops.len()).collect();
            a.preview_on = false;
            match which.as_str() {
                "combine" => { let (op, keep, swap) = (a.combine_op, a.combine_keep, a.combine_swap); a.run_tool("combine", |a| a.combine_selection(op, keep, swap)); }
                "corners" => { let (kind, size) = (if a.corners.chamfer { putiacnc_core::shapes::CornerKind::Chamfer } else { putiacnc_core::shapes::CornerKind::Fillet }, a.corners.size); a.run_tool("corners", |a| a.corner_selection(kind, size)); }
                _ => { a.run_tool("array", |a| a.array_selection()); }
            }
        }
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
    fn an_operation_that_loses_its_last_shape_stays_empty_and_undo_brings_the_shape_back() {
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
        assert_eq!(a.project.data.ops.len(), 1, "no shape is left: the operation stays, with its settings");
        assert!(a.note.as_deref().is_some_and(|n| n.contains("keeps its settings")));
        a.do_undo();
        assert_eq!(a.project.data.ops.len(), 1);
        assert_eq!(a.project.data.loops.len(), 1);
        assert!(a.empty_ops.is_empty(), "it has a shape again");
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

#[cfg(test)]
mod gen_tests {
    use super::*;

    /// A project with something to cut: a rectangle in a profile operation.
    fn app_with_job() -> App {
        let mut a = App::new(None);
        let d = &mut a.project.data;
        let tool = d.tools[0].id.clone();
        let mut p = shapes::default_op_params(&tool);
        p.kind = "outside".into(); p.depth = 3.0;
        d.ops.push(putiacnc_core::model::Operation { id: 1, p });
        d.loops.push(shapes::new_rect([10.0, 10.0], [60.0, 40.0], 0.0, 1));
        a.touch();
        a
    }

    /// What the main loop does every frame, until the toolpath matches the drawing.
    fn settle(a: &mut App) {
        for _ in 0..2000 {
            a.poll_generation();
            if a.gen_pending && a.gen_rx.is_none() { a.start_generation(); }
            if !a.gen_stale() { return; }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("the toolpath never settled");
    }

    #[test]
    fn an_operation_keeps_its_settings_when_its_last_shape_is_deleted() {
        let mut a = app_with_job();
        a.project.data.ops[0].p.depth = 7.5;
        a.sel = [0].into_iter().collect();
        a.delete_selection();
        assert_eq!(a.project.data.ops.len(), 1, "the operation stays");
        assert_eq!(a.project.data.ops[0].p.depth, 7.5);
        assert!(a.note.as_deref().is_some_and(|n| n.contains("keeps its settings")), "{:?}", a.note);
        a.note = None;
        a.touch();
        assert!(a.note.is_none(), "it is mentioned once, not on every change");
        // draw the shape again and put it back into the operation
        a.project.data.loops.push(shapes::new_rect([20.0, 20.0], [70.0, 50.0], 0.0, 0));
        let id = a.project.data.ops[0].id;
        assert_eq!(shapes::assign_to_op(&mut a.project.data.loops, &[0].into_iter().collect(), id), 1);
        a.touch();
        settle(&mut a);
        assert!(a.output.as_ref().unwrap().moves.iter().any(|m| m.cut), "the operation cuts again");
        assert_eq!(a.project.data.ops[0].p.depth, 7.5);
        // and an operation emptied again is mentioned again
        a.sel = [0].into_iter().collect();
        a.delete_selection();
        assert!(a.note.is_some());
    }

    #[test]
    fn a_plug_on_a_separate_piece_gives_two_files_and_old_projects_give_one() {
        let mut a = app_with_job();
        shapes::add_inlay(&mut a.project.data, &[0].into_iter().collect());
        a.touch();
        settle(&mut a);
        assert!(a.pieces.is_empty());
        assert_eq!(a.gcode_files(std::path::Path::new("/x/job.nc")).len(), 1, "no piece, one file");
        let (pocket, plug) = { let o = &a.project.data.ops; (o[o.len() - 2].id, o[o.len() - 1].id) };
        let pc = putiacnc_core::pieces::default_piece(&a.project.data.stock);
        shapes::put_plug_on_piece(&mut a.project.data, pocket, plug, Some(pc));
        a.touch();
        settle(&mut a);
        assert_eq!(a.pieces.len(), 1);
        let files = a.gcode_files(std::path::Path::new("/x/job.nc"));
        let names: Vec<String> = files.iter().map(|(p, _)| p.display().to_string()).collect();
        assert_eq!(names, ["/x/job.nc", "/x/job-plug.nc"]);
        assert!(files[1].1.contains("Plug piece"), "the piece's file says what it is");
        // and back onto the main stock: one file again
        shapes::put_plug_on_piece(&mut a.project.data, pocket, plug, None);
        a.touch();
        settle(&mut a);
        assert!(a.pieces.is_empty());
    }

    #[test]
    fn a_plug_piece_is_checked_on_its_own() {
        let mut a = app_with_job();
        shapes::add_inlay(&mut a.project.data, &[0].into_iter().collect());
        let (pocket, plug) = { let o = &a.project.data.ops; (o[o.len() - 2].id, o[o.len() - 1].id) };
        // a piece thinner than the plug is deep: the plug cuts into the table, the main stock is fine
        let mut pc = putiacnc_core::pieces::default_piece(&a.project.data.stock);
        pc.t = 1.0;
        shapes::put_plug_on_piece(&mut a.project.data, pocket, plug, Some(pc));
        a.touch();
        settle(&mut a);
        for _ in 0..3000 { if !a.poll_checks() { break; } std::thread::sleep(std::time::Duration::from_millis(5)); }
        assert_eq!(a.checks.len(), a.check_parts.len());
        assert!(a.checks.iter().zip(&a.check_parts).any(|(f, &p)| p == 1 && f.kind == putiacnc_core::checks::Kind::Spoilboard), "{:?}", a.checks.iter().map(|f| f.kind).collect::<Vec<_>>());
        assert!(a.check_parts.iter().all(|&p| p == 1), "the main stock has nothing to report");
    }

    #[test]
    fn a_clamp_under_a_plug_piece_beside_the_stock_is_hit_by_the_piece_only() {
        let mut a = app_with_job();
        shapes::add_inlay(&mut a.project.data, &[0].into_iter().collect());
        let (pocket, plug) = { let o = &a.project.data.ops; (o[o.len() - 2].id, o[o.len() - 1].id) };
        let pc = putiacnc_core::pieces::default_piece(&a.project.data.stock);
        let x = pc.x;
        shapes::put_plug_on_piece(&mut a.project.data, pocket, plug, Some(pc));
        // a clamp on the table where the piece is drawn, at the corner of the plug's outline (10, 10) on the piece
        a.project.data.clamps.push(putiacnc_core::model::Clamp { x: x + 8.0, y: 8.0, w: 6.0, h: 6.0, height: 20.0 });
        a.touch();
        settle(&mut a);
        for _ in 0..3000 { if !a.poll_checks() { break; } std::thread::sleep(std::time::Duration::from_millis(5)); }
        let hits: Vec<usize> = a.checks.iter().zip(&a.check_parts).filter(|(f, _)| f.kind == putiacnc_core::checks::Kind::Clamp).map(|(_, &p)| p).collect();
        assert_eq!(hits, [1], "only the piece runs into it: {:?}", a.checks.iter().map(|f| f.kind).collect::<Vec<_>>());
        assert!(!a.clamp_hits.is_empty());
    }

    #[test]
    fn the_worker_gives_the_same_toolpath_as_generating_here() {
        let mut a = app_with_job();
        settle(&mut a);
        let bg = a.output.as_ref().unwrap().lines.clone();
        a.regenerate();
        assert_eq!(bg, a.output.as_ref().unwrap().lines);
        assert!(!bg.is_empty());
    }

    #[test]
    fn only_one_generation_runs_and_the_latest_change_wins() {
        let mut a = app_with_job();
        a.start_generation();
        assert!(a.gen_rx.is_some());
        // a change while it runs: no second worker, the change waits
        a.project.data.ops[0].p.depth = 5.0;
        a.touch();
        a.start_generation();
        assert!(a.gen_pending, "the change is still waiting while the first worker runs");
        settle(&mut a);
        let deepest = a.output.as_ref().unwrap().moves.iter().map(|m| m.to[2]).fold(0.0_f64, f64::min);
        assert!((deepest + 5.0).abs() < 1e-6, "the toolpath is for the latest depth: {deepest}");
    }

    #[test]
    fn saving_and_exporting_never_use_an_old_toolpath() {
        let mut a = app_with_job();
        settle(&mut a);
        // change the drawing and, before the main loop gets to it, ask for the current toolpath (what save and export do)
        a.project.data.ops[0].p.depth = 7.0;
        a.touch();
        a.start_generation();
        a.project.data.ops[0].p.depth = 8.0;
        a.touch();
        a.finish_generation();
        assert!(!a.gen_stale());
        let deepest = a.output.as_ref().unwrap().moves.iter().map(|m| m.to[2]).fold(0.0_f64, f64::min);
        assert!((deepest + 8.0).abs() < 1e-6, "{deepest}");
    }

    #[test]
    fn opening_another_project_drops_the_old_toolpath() {
        let mut a = app_with_job();
        a.start_generation();
        a.project = shapes::blank_project();
        a.reset_after_load();
        assert!(a.gen_rx.is_none() && a.output.is_none(), "nothing of the old project is kept or picked up later");
        settle(&mut a);
        assert!(a.output.as_ref().is_none_or(|o| o.moves.is_empty()), "the blank project has nothing to cut");
    }

    #[test]
    fn a_worker_that_dies_is_reported_not_waited_for() {
        let mut a = app_with_job();
        let (tx, rx) = std::sync::mpsc::channel();
        drop(tx);
        a.gen_pending = false;
        a.gen_rx = Some(rx);
        assert!(!a.poll_generation());
        assert!(a.gen_rx.is_none() && a.output.is_none());
        assert!(a.gen_error.as_deref().is_some_and(|e| e.contains("internal error")));
    }
}

impl App {
    /// The speed unit shown for tools: the one chosen for the current view of the lengths (name, mm/min per unit).
    pub fn tool_speed(&self) -> (&'static str, f64) {
        let i = if self.tool_len_in { self.tool_speed_in } else { self.tool_speed_mm };
        putiacnc_core::toollib::SPEED_UNITS[i.min(putiacnc_core::toollib::SPEED_UNITS.len() - 1)]
    }
    /// The setting `tool_speed` reads, for the "Speeds in" box to change.
    pub fn tool_speed_slot(&mut self) -> &mut usize { if self.tool_len_in { &mut self.tool_speed_in } else { &mut self.tool_speed_mm } }
}
