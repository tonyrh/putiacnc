//! Data model. The structs mirror the JSON that the browser app exports, so existing
//! project files (`"app": "putia-cnc"`) load unchanged. All lengths are millimetres,
//! feeds are mm/min, and loop points are integers in 1/1000 mm.
use serde::{Deserialize, Serialize};

fn d_true() -> bool { true }
fn d_one() -> f64 { 1.0 }
fn d_flat() -> String { "flat".into() }
fn d_two() -> u32 { 2 }
fn d_60() -> f64 { 60.0 }
fn d_ll() -> String { "ll".into() }
fn d_top() -> String { "top".into() }
fn d_wood() -> String { "wood".into() }
fn d_climb() -> String { "climb".into() }
fn d_outside() -> String { "outside".into() }

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ProjectFile {
    pub app: String,
    #[serde(default)] pub v: u32,
    #[serde(default)] pub name: String,
    pub data: ProjectData,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ProjectData {
    pub stock: Stock,
    #[serde(default)] pub job: Job,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub machine: Option<Machine>,
    #[serde(default)] pub ops: Vec<Operation>,
    #[serde(default)] pub tools: Vec<Tool>,
    #[serde(default)] pub loops: Vec<LoopData>,
    #[serde(default)] pub gcode: String,
    /// Free-form notes about the job.
    #[serde(default, skip_serializing_if = "String::is_empty")] pub notes: String,
    /// Active operation id and last-used operation id (browser app bookkeeping).
    #[serde(default)] pub act: i64,
    #[serde(default)] pub oid: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Stock {
    pub w: f64, pub h: f64, pub t: f64,
    #[serde(default = "d_wood")] pub mat: String,
    /// Work zero: ll, lr, ul, ur or c.
    #[serde(default = "d_ll")] pub zr: String,
    /// Z zero: "top" or "h" (height above the stock bottom, see `zh`).
    #[serde(default = "d_top")] pub zm: String,
    #[serde(default)] pub zh: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Job {
    #[serde(default)] pub safe: f64,
    #[serde(default)] pub air: bool,
    /// Number the G-code lines (N10, N20, ...).
    #[serde(default, skip_serializing_if = "is_false")] pub numbered: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Machine {
    #[serde(default)] pub id: String,
    #[serde(default)] pub name: String,
    pub rapid: f64,
    pub tsc: f64,
    pub x: f64, pub y: f64, pub z: f64,
    #[serde(default)] pub notes: String,
}

impl Default for Machine {
    fn default() -> Self {
        Machine { id: String::new(), name: "My CNC machine".into(), rapid: 2000.0, tsc: 1.2, x: 300.0, y: 200.0, z: 60.0, notes: String::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Tool {
    pub id: String,
    #[serde(default)] pub name: String,
    #[serde(default)] pub sys: String,
    #[serde(default = "d_flat")] pub shape: String,
    #[serde(default = "d_60")] pub angle: f64,
    #[serde(default = "d_two")] pub flutes: u32,
    pub dia: f64, pub rpm: f64, pub feed: f64, pub plunge: f64,
    /// Stepover, percent of the diameter.
    pub so: f64,
    /// Maximum depth per pass.
    pub pd: f64,
    #[serde(default)] pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Operation { pub id: i64, pub p: OpParams }

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct OpParams {
    /// outside, inside, online, pocket, vcarve or drill.
    #[serde(rename = "type", default = "d_outside")] pub kind: String,
    #[serde(default = "d_climb")] pub dir: String,
    pub depth: f64,
    #[serde(default = "d_true")] pub ramp: bool,
    #[serde(default)] pub tool: String,
    #[serde(default)] pub tabs: bool,
    /// Air cut: run this operation 10 mm above the stock, without cutting.
    #[serde(default, skip_serializing_if = "is_false")] pub air: bool,
    /// Depth per pass in mm; `None` = the tool's own pass depth.
    #[serde(default, rename = "passD", skip_serializing_if = "Option::is_none")] pub pass_depth: Option<f64>,
    /// Radius (mm) of the arc the tool uses to enter and leave a profile cut; `None` = plunge straight onto the line.
    #[serde(default, rename = "leadR", skip_serializing_if = "Option::is_none")] pub lead: Option<f64>,
    #[serde(default, rename = "tabN", skip_serializing_if = "Option::is_none")] pub tab_n: Option<f64>,
    #[serde(default, rename = "tabW", skip_serializing_if = "Option::is_none")] pub tab_w: Option<f64>,
    #[serde(default, rename = "tabH", skip_serializing_if = "Option::is_none")] pub tab_h: Option<f64>,
    /// Manually placed holding tabs. `Some` = manual placement, `None` = evenly spaced tabs (`tabN`).
    /// Each tab is anchored to a shape, so it follows when the shape is moved or resized.
    #[serde(default, rename = "tabPts", skip_serializing_if = "Option::is_none")] pub tab_pts: Option<Vec<TabPt>>,
}

/// A manually placed holding tab, anchored to a shape: `l` is the index of the shape in the project's
/// loop list, `u`/`v` the tab's position inside that shape's bounding box (0..1, y up from the bottom).
/// The tab sits on the point of the outline nearest to that position, so moving or resizing the shape
/// moves the tab with it.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct TabPt { pub l: usize, pub u: f64, pub v: f64 }

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct LoopData {
    /// 1 = closed.
    #[serde(default)] pub c: u8,
    /// Operation id this shape belongs to (0 = none).
    #[serde(default)] pub op: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rc: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub cr: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub el: Option<f64>,
    /// Flat list: X0, Y0, X1, Y1, ... in 1/1000 mm.
    pub pts: Vec<i64>,
}

impl LoopData {
    pub fn path(&self) -> Vec<[i64; 2]> { self.pts.chunks_exact(2).map(|c| [c[0], c[1]]).collect() }
    pub fn closed(&self) -> bool { self.c != 0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Units { Metric, Imperial }

impl Units {
    pub fn factor(self) -> f64 { if self == Units::Imperial { 25.4 } else { 1.0 } }
    pub fn name(self) -> &'static str { if self == Units::Imperial { "in" } else { "mm" } }
}

pub const _UNUSED: f64 = 1.0;
#[allow(dead_code)] fn _silence() -> f64 { d_one() }

fn is_false(b: &bool) -> bool { !*b }
