//! Data model. The structs mirror the JSON that the browser app exports, so existing
//! project files (`"app": "putia-cnc"`) load unchanged. All lengths are millimetres,
//! feeds are mm/min, and loop points are integers in 1/1000 mm.
use serde::{Deserialize, Serialize};

fn d_true() -> bool { true }
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
    /// Imported drawings, listed in the left panel (shapes point to them with `grp`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub groups: Vec<ShapeGroup>,
    /// Clamps and other things on the table that the tool must keep away from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub clamps: Vec<Clamp>,
    /// A 3D model (height map) for relief operations.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub relief: Option<Relief>,
    #[serde(default)] pub gcode: String,
    /// Free-form notes about the job.
    #[serde(default, skip_serializing_if = "String::is_empty")] pub notes: String,
    /// Active operation id and last-used operation id (browser app bookkeeping).
    #[serde(default)] pub act: i64,
    #[serde(default)] pub oid: i64,
    /// Operations (by id) left out of the 3D preview; the G-code still has them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub hidden_ops: Vec<i64>,
    /// Layers of the drawing. The first layer (id 0) always exists even when it is not in the list; the list holds
    /// it once it has been renamed, hidden, locked or recoloured. A hidden layer is only hidden from the drawing:
    /// operations still cut its shapes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub layers: Vec<Layer>,
    /// The layer new shapes go on.
    #[serde(default, skip_serializing_if = "is_zero_i64")] pub active_layer: i64,
}

/// A layer of the drawing: a name, a colour, and whether it is shown and whether its shapes can be changed.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Layer {
    pub id: i64,
    pub name: String,
    /// "#rrggbb"; empty = the drawing's usual colours.
    #[serde(default, skip_serializing_if = "String::is_empty")] pub color: String,
    #[serde(default, skip_serializing_if = "is_false")] pub hidden: bool,
    #[serde(default, skip_serializing_if = "is_false")] pub locked: bool,
}

impl Layer {
    /// The layer's colour as red, green and blue, if it has a valid one.
    pub fn rgb(&self) -> Option<[u8; 3]> {
        let h = self.color.strip_prefix('#')?;
        if h.len() != 6 || !h.is_ascii() { return None; }
        let v = u32::from_str_radix(h, 16).ok()?;
        Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
    }
}

fn is_zero_i64(v: &i64) -> bool { *v == 0 }

impl ProjectData {
    /// How many things the person sees in these shapes: a group (placed text, or Ctrl+G) is one, whatever number of outlines it is made of
    /// (the letters of "WOOD" are 7 outlines but one line of text).
    pub fn object_count<'a>(loops: impl IntoIterator<Item = &'a LoopData>) -> usize {
        let mut groups = std::collections::BTreeSet::new();
        let mut single = 0;
        for l in loops { match l.grp { Some(g) => { groups.insert(g); } None => single += 1 } }
        single + groups.len()
    }

    /// Every layer, the first one included, in order.
    pub fn all_layers(&self) -> Vec<Layer> {
        let mut v = self.layers.clone();
        if !v.iter().any(|l| l.id == 0) { v.insert(0, Layer { id: 0, name: "Layer 1".into(), color: String::new(), hidden: false, locked: false }); }
        v
    }
    /// The layer a shape is on: a number that is no longer in the list counts as the first layer.
    pub fn layer_of(&self, l: &LoopData) -> i64 {
        let id = l.lay.unwrap_or(0);
        if id == 0 || self.layers.iter().any(|x| x.id == id) { id } else { 0 }
    }
    pub fn layer_hidden(&self, id: i64) -> bool { self.layers.iter().any(|l| l.id == id && l.hidden) }
    pub fn layer_locked(&self, id: i64) -> bool { self.layers.iter().any(|l| l.id == id && l.locked) }
    pub fn shape_hidden(&self, l: &LoopData) -> bool { !self.layers.is_empty() && self.layer_hidden(self.layer_of(l)) }
    pub fn shape_locked(&self, l: &LoopData) -> bool { !self.layers.is_empty() && self.layer_locked(self.layer_of(l)) }
    /// The layer to change, put in the list first if it is the first layer and not there yet.
    pub fn layer_mut(&mut self, id: i64) -> Option<&mut Layer> {
        if id == 0 && !self.layers.iter().any(|l| l.id == 0) { let first = self.all_layers().remove(0); self.layers.insert(0, first); }
        self.layers.iter_mut().find(|l| l.id == id)
    }
    /// A new layer called `name`, returned by id.
    pub fn add_layer(&mut self, name: &str) -> i64 {
        if !self.layers.iter().any(|l| l.id == 0) { let first = self.all_layers().remove(0); self.layers.insert(0, first); }
        let id = self.layers.iter().map(|l| l.id).max().unwrap_or(0) + 1;
        self.layers.push(Layer { id, name: name.to_string(), color: String::new(), hidden: false, locked: false });
        id
    }
}

/// A rectangle on the table, in mm from the stock's lower-left corner, standing `height` mm above the stock top.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Clamp {
    pub x: f64, pub y: f64, pub w: f64, pub h: f64,
    #[serde(default = "d_clamp_h")] pub height: f64,
}
fn d_clamp_h() -> f64 { 20.0 }

/// A file of shapes imported as one unit.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ShapeGroup {
    pub id: i64,
    pub name: String,
    /// Locked: it cannot be moved, resized, rotated or flipped by accident.
    #[serde(default, skip_serializing_if = "is_false")] pub locked: bool,
    /// Set when the shapes are letters placed with the Text tool: what it takes to set the same text again,
    /// so the text can be edited later (see `text::fit_affine`).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub text: Option<TextSpec>,
}

/// The settings a text was set with. The shapes of its group are these settings turned into outlines at the
/// origin, then moved, scaled, turned or flipped like any other shapes.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TextSpec {
    pub content: String,
    /// The font as the Text tool names it ("DejaVu Sans Bold", "Hack"...).
    pub font: String,
    /// Height of capital letters and extra space between letters, in mm.
    pub size: f64,
    pub spacing: f64,
    /// Extra space after each space character, mm; and whether the font's pair kerning is applied (texts made before it existed have it off).
    #[serde(default)] pub word_spacing: f64,
    #[serde(default)] pub kerning: bool,
    pub align: crate::text::Align,
    #[serde(default)] pub flow: crate::text::Flow,
    #[serde(default)] pub radius: f64,
    #[serde(default = "ninety")] pub arc_deg: f64,
    #[serde(default)] pub arc_in: bool,
    /// Width (mm) the text was fitted to, 0 when its height was set by hand.
    #[serde(default)] pub fit_width: f64,
    #[serde(default)] pub shadow: [f64; 2],
    /// Flow::Shape: the outline (mm) of the shape the text follows, as it was when the text was set; `radius` is then the
    /// distance from its edge to the letters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub path: Vec<[f64; 2]>,
}

/// A 3D model reduced to a height map (seen from above), carved by relief operations.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Relief {
    pub name: String,
    pub nx: usize,
    pub ny: usize,
    /// Size of one height-map cell in mm at scale 1.
    pub res: f64,
    /// Height of the model in mm at scale 1; the highest cell holds 65535.
    pub zmax: f64,
    /// Heights, row by row from the bottom (y up), as little-endian u16 in base64.
    pub hm: String,
    /// Lower-left corner on the stock, mm.
    pub x: f64,
    pub y: f64,
    /// Scale applied to the model's X and Y size.
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Stock {
    pub w: f64, pub h: f64, pub t: f64,
    #[serde(default = "d_wood")] pub mat: String,
    /// What is being cut, for the feeds and speeds suggestion (an id of `feeds::MATERIALS`; empty = not set). Not the look of the stock: that is `mat`.
    #[serde(default, skip_serializing_if = "String::is_empty")] pub cut: String,
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
    /// G-code dialect: "grbl" (default) or "mach" (Mach3 / LinuxCNC).
    #[serde(default, skip_serializing_if = "String::is_empty")] pub post: String,
    /// The work coordinate system the program runs in: 0 = G54 (the default) to 5 = G59, then G59.1 to G59.3 (Mach3 / LinuxCNC only).
    #[serde(default, skip_serializing_if = "is_zero_u8")] pub wcs: u8,
}

fn is_zero_u8(v: &u8) -> bool { *v == 0 }

/// The work coordinate systems in the order of `Job::wcs`.
pub const WCS: [&str; 9] = ["G54", "G55", "G56", "G57", "G58", "G59", "G59.1", "G59.2", "G59.3"];

impl Job {
    /// How many systems this controller has: GRBL knows G54 to G59, Mach3 / LinuxCNC three more.
    pub fn wcs_count(&self) -> usize { if self.mach() { 9 } else if self.marlin() { 1 } else { 6 } }
    /// The G-code word of the chosen system (G54 when the choice is not one this controller has).
    pub fn wcs_name(&self) -> &'static str { let i = self.wcs as usize; if i < self.wcs_count() { WCS[i] } else { WCS[0] } }

    /// True for Marlin on a machine whose board does not run the spindle: the program asks the operator to start and stop it.
    pub fn marlin(&self) -> bool { self.post == "marlin" }
    /// True for the Mach3 / LinuxCNC dialect.
    pub fn mach(&self) -> bool { self.post == "mach" }
    /// The dialect as people name it, for the G-code's first comment and the G-code tab.
    pub fn dialect_name(&self) -> &'static str { dialect_name(&self.post) }
}

/// The controllers a machine can be set to: "" = GRBL, "grblhal", "fluidnc" (both write what GRBL gets today), "mach", "marlin" (motion only: the operator runs the spindle by hand).
pub const DIALECTS: [(&str, &str); 5] = [("", "GRBL"), ("grblhal", "grblHAL"), ("fluidnc", "FluidNC"), ("mach", "Mach3 / LinuxCNC"), ("marlin", "Marlin (no spindle control)")];

pub fn dialect_name(post: &str) -> &'static str { DIALECTS.iter().find(|d| d.0 == post).map_or("GRBL", |d| d.1) }

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Machine {
    #[serde(default)] pub id: String,
    #[serde(default)] pub name: String,
    pub rapid: f64,
    pub tsc: f64,
    pub x: f64, pub y: f64, pub z: f64,
    #[serde(default)] pub notes: String,
    /// G-code dialect of the machine: "" = GRBL, "grblhal", "fluidnc" or "mach" = Mach3 / LinuxCNC. `None` in machine lists saved before the setting moved here:
    /// the first project that uses the machine then fills it in from its own settings.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub post: Option<String>,
    /// Number the G-code lines (N10, N20, ...) for controllers that want them.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub numbered: Option<bool>,
    /// Safe Z: the height above the stock where the tool travels, in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub safe: Option<f64>,
    /// Spindle speed limits in rpm: a tool outside them is held to the limit in the G-code (`None` = no limit).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rpm_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rpm_max: Option<f64>,
    /// Fastest cutting feed and plunge rate the machine can do, in mm/min (`None` = no limit).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub feed_max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub plunge_max: Option<f64>,
    /// Seconds to wait after the spindle starts or changes speed before the tool touches anything (`None` = 2 s, 0 = no wait).
    #[serde(default, rename = "spinUp", skip_serializing_if = "Option::is_none")] pub spin_up: Option<f64>,
    /// Write the G-code in inches (G20) when a project uses this machine (`None` = leave the current choice).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub inches: Option<bool>,
    /// Write circular runs of cutting moves as G2/G3 arcs (shorter files; the controller must support them, as Grbl does).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub arcs: Option<bool>,
    /// Lines of G-code written after the setup lines (units, absolute mode...), before the first tool is chosen.
    #[serde(default, rename = "startCode", skip_serializing_if = "Option::is_none")] pub start_code: Option<String>,
    /// Lines of G-code written at the end, after the spindle stops and the tool returns to X0 Y0, just before the program ends.
    #[serde(default, rename = "endCode", skip_serializing_if = "Option::is_none")] pub end_code: Option<String>,
    /// Diameter in mm of the tool holder (the collet nut or the spindle nose, whatever hangs lowest). `None` = not known (the holder is not checked).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub holder: Option<f64>,
    /// Check and draw the tool holder (switched off by default; the diameter above is kept while it is off).
    #[serde(default, rename = "holderOn", skip_serializing_if = "is_false")] pub holder_on: bool,
    /// Acceleration in mm/s² (Grbl's `$120`): when set, the time estimate and the playback let the machine speed up, slow down and take corners
    /// the way it really does (`None` = no acceleration model: time is length / feed times the correction).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub accel: Option<f64>,
    /// The Y and Z accelerations (Grbl's `$121` and `$122`) where they differ from `accel` (`None` = the same as `accel`).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub accel_y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub accel_z: Option<f64>,
    /// The top speed of each axis in mm/min (Grbl's `$110` to `$112`): no move is run faster than the slowest axis in it allows. `None` = no limit; Y and Z
    /// follow X where they are not set.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rate_y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rate_z: Option<f64>,
    /// Junction deviation in mm (Grbl's `$11`; `None` = 0.01): how fast the machine takes corners when an acceleration is set.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub junction: Option<f64>,
    /// How stiff the machine is, for the feeds and speeds suggestion: "light" (also when empty), "medium" or "heavy" (`feeds::RIGIDITY`).
    #[serde(default, skip_serializing_if = "String::is_empty")] pub rigid: String,
}

impl Machine {
    /// The wait after the spindle starts, in seconds.
    pub fn spin_up_s(&self) -> f64 { self.spin_up.unwrap_or(2.0).clamp(0.0, 60.0) }
    /// The tool as this machine can run it: spindle speed, feed and plunge held to the machine's limits.
    pub fn hold(&self, t: &Tool) -> Tool {
        let mut h = t.clone();
        if let Some(m) = self.rpm_max { if h.rpm > m { h.rpm = m; } }
        if let Some(m) = self.rpm_min { if h.rpm < m { h.rpm = m; } }
        if let Some(m) = self.feed_max { if h.feed > m { h.feed = m; } }
        if let Some(m) = self.plunge_max { if h.plunge > m { h.plunge = m; } }
        h
    }
    /// What `hold` changes, as words ("spindle 11000 rpm (tool: 12000), feed ..."); empty when nothing changes. Feeds are in mm/min.
    pub fn hold_text(&self, t: &Tool) -> String {
        let h = self.hold(t);
        let n = |v: f64| format!("{}", (v * 100.0).round() / 100.0);
        let mut w: Vec<String> = vec![];
        if h.rpm != t.rpm { w.push(format!("spindle {} rpm (tool: {})", n(h.rpm), n(t.rpm))); }
        if h.feed != t.feed { w.push(format!("feed {} mm/min (tool: {})", n(h.feed), n(t.feed))); }
        if h.plunge != t.plunge { w.push(format!("plunge {} mm/min (tool: {})", n(h.plunge), n(t.plunge))); }
        w.join(", ")
    }
}

impl Default for Machine {
    fn default() -> Self {
        Machine { id: String::new(), name: "My CNC machine".into(), rapid: 2000.0, tsc: 1.2, x: 300.0, y: 200.0, z: 60.0, notes: String::new(),
                  post: Some(String::new()), numbered: Some(false), safe: Some(5.0), rpm_min: None, rpm_max: None, feed_max: None, plunge_max: None, inches: Some(false), arcs: Some(false), spin_up: Some(2.0), start_code: None, end_code: None, holder: None, holder_on: false, rigid: String::new(), accel: None, accel_y: None, accel_z: None, rate: None, rate_y: None, rate_z: None, junction: None }
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
    /// Tip radius in mm of a V-bit, engraving bit or tapered ball nose (0 = sharp tip).
    #[serde(default, skip_serializing_if = "is_zero")] pub tip: f64,
    /// Arc profile bit: radius in mm of the arc that forms the cutting edge. Convex: below half the diameter leaves a flat bottom (a bull nose); above it is a shallow dome. Concave: never less than half the diameter.
    #[serde(default, skip_serializing_if = "is_zero")] pub arc: f64,
    /// Arc profile bit: the edge is a concave arc (flared, like a cove) instead of a convex one (rounded, like a ball).
    #[serde(default, skip_serializing_if = "is_false")] pub concave: bool,
    /// Cutting length (flute length) in mm: how deep the cutter can go before the shank rubs. 0 = not known.
    #[serde(default, skip_serializing_if = "is_zero")] pub flute: f64,
    /// Shank diameter in mm. 0 = the same as the cutter.
    #[serde(default, skip_serializing_if = "is_zero")] pub shank: f64,
    /// Stick-out in mm: how far the tool tip is below the bottom of the holder (collet nut). 0 = not known (the holder is not checked).
    #[serde(default, skip_serializing_if = "is_zero")] pub stick: f64,
    /// Pen: the colour of its ink as "#rrggbb" (shown in the 3D preview and the tool lists); `None` = the usual dark ink.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub color: Option<String>,
}

impl Tool {
    /// The ink colour of a pen as red, green and blue, if it has a valid one.
    pub fn ink_rgb(&self) -> Option<[u8; 3]> {
        let h = self.color.as_deref()?.strip_prefix('#')?;
        if h.len() != 6 || !h.is_ascii() { return None; }
        let v = u32::from_str_radix(h, 16).ok()?;
        Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
    }
}

/// "#rrggbb" for a colour.
pub fn hex_rgb(c: [u8; 3]) -> String { format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]) }

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Operation { pub id: i64, pub p: OpParams }

impl OpParams {
    /// The operation's second tool: the flat end mill of an inlay or relief (`tool2`), or the cleanup cutter of a pocket (`rest`).
    pub fn second_tool(&self) -> Option<&str> { if self.kind == "pocket" { self.rest.as_deref() } else { self.tool2.as_deref() } }
    /// The flat end mill of a V-carve with a flat depth ("Clear with"): it cuts the floor, so it is a second tool of the operation.
    pub fn flat_mill(&self) -> Option<&str> { if self.kind == "vcarve" && self.flat.is_some_and(|f| f > 1e-6) { self.tool2.as_deref() } else { None } }
    /// The operation's third tool: the small ball nose that finishes the detail of a relief (`rest`).
    pub fn detail_tool(&self) -> Option<&str> { if self.kind == "relief" { self.rest.as_deref() } else { None } }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct OpParams {
    /// outside, inside, online, pocket, vcarve, drill, inlay (pocket), plug (inlay plug), relief or draw (a pen).
    #[serde(rename = "type", default = "d_outside")] pub kind: String,
    #[serde(default = "d_climb")] pub dir: String,
    pub depth: f64,
    #[serde(default = "d_true")] pub ramp: bool,
    #[serde(default)] pub tool: String,
    #[serde(default)] pub tabs: bool,
    /// Air cut: run this operation 10 mm above the stock, without cutting.
    #[serde(default, skip_serializing_if = "is_false")] pub air: bool,
    /// A surfacing pocket (made by the Surface tool): its rectangle is a cutter radius larger than the stock on purpose, so it is not warned about.
    #[serde(default, skip_serializing_if = "is_false")] pub surf: bool,
    /// Rest machining (pockets): a smaller flat cutter (a tool id) that clears the corners and narrow parts the main tool could not reach.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rest: Option<String>,
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
    /// V-carve: the depth of a flat floor. The V-bit carves only the sloped band along the edges down to it, and the flat end mill in `tool2`
    /// clears the floor inside at that depth (without `tool2` this is just a cap on the depth).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub flat: Option<f64>,
    /// Second tool of an inlay operation: the flat end mill that clears the pocket floor or cuts the plug out.
    #[serde(default, rename = "tool2", skip_serializing_if = "Option::is_none")] pub tool2: Option<String>,
    /// Inlay plug: how much smaller the plug is than the pocket, measured sideways, in mm (the glue gap).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub fit: Option<f64>,
    /// Inlay plug: cut from this separate piece of stock instead of the main one (its own G-code file, its own zero).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub piece: Option<PlugPiece>,
    /// 3D relief: the depth was typed in; otherwise it follows the model's height (capped at the stock thickness).
    #[serde(default, rename = "dMan", skip_serializing_if = "is_false")] pub d_man: bool,
    /// 3D relief: skip the roughing step.
    #[serde(default, rename = "rOff", skip_serializing_if = "is_false")] pub r_off: bool,
    /// 3D relief: skip the finishing step.
    #[serde(default, rename = "fOff", skip_serializing_if = "is_false")] pub f_off: bool,
    /// 3D relief roughing: material left on the surface for the finishing pass, in mm (default 0.3).
    #[serde(default, rename = "rLeave", skip_serializing_if = "Option::is_none")] pub r_leave: Option<f64>,
    /// 3D relief roughing: stepover in % of the tool diameter; `None` = the roughing tool's own.
    #[serde(default, rename = "rSo", skip_serializing_if = "Option::is_none")] pub r_so: Option<f64>,
    /// 3D relief finishing: stepover in % of the tool diameter; `None` = the finishing tool's own.
    #[serde(default, rename = "fSo", skip_serializing_if = "Option::is_none")] pub f_so: Option<f64>,
    /// Profile and pocket: leave this much (mm) on the walls in the depth passes and cut them to size in a finishing pass; `None` = no finishing pass.
    /// Enter a pocket's first ring or a profile with a helix (a spiral down the waste side) instead of ramping along the path.
    #[serde(default, skip_serializing_if = "is_false")] pub helix: bool,
    /// The helix circle the tool centre follows, as a percentage of the tool diameter (`None` = 100), and the steepest angle of the spiral in degrees (`None` = 3).
    #[serde(default, rename = "helixD", skip_serializing_if = "Option::is_none")] pub helix_d: Option<f64>,
    #[serde(default, rename = "helixA", skip_serializing_if = "Option::is_none")] pub helix_a: Option<f64>,
    /// Pocket: adaptive clearing: a light stepover (`adaptive_so` % of the tool, `None` = 12) and the corners of the inner rings rounded, so the tool
    /// meets about the same amount of material all along the path and can cut at full flute depth.
    #[serde(default, skip_serializing_if = "is_false")] pub adaptive: bool,
    #[serde(default, rename = "adaptiveSo", skip_serializing_if = "Option::is_none")] pub adaptive_so: Option<f64>,
    #[serde(default, rename = "wLeave", skip_serializing_if = "Option::is_none")] pub w_leave: Option<f64>,
    /// 3D relief finishing: the direction of the lines, "x" (default), "y", or "xy" (along X, then along Y over the same surface).
    #[serde(default, rename = "fDir", skip_serializing_if = "Option::is_none")] pub f_dir: Option<String>,
    /// 3D relief detail tool: stepover in % of the tool diameter; `None` = the detail tool's own.
    #[serde(default, rename = "dSo", skip_serializing_if = "Option::is_none")] pub d_so: Option<f64>,
    /// Stop the program after this operation: the tool lifts, the spindle stops, and the program waits (`M0`) with this message until you resume
    /// (flip the stock, move a clamp, clean up chips). `Some("")` = a pause with a standard message; `None` = no pause.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub pause: Option<String>,
    /// Custom feeds and speeds for this operation; `None` = the tool's own. Only the values that are set differ from the tool.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub custom: Option<CustomFeeds>,
    /// The same for the second tool (the flat end mill of an inlay); `None` = the tool's own.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub custom2: Option<CustomFeeds>,
    /// Draw (pen): how far above the paper the pen travels between strokes, in mm; `None` = 2 mm (never above the safe height).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub lift: Option<f64>,
    /// Draw (pen): fill the closed shapes with lines instead of drawing only their outlines; `None` = outlines only.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub hatch: Option<Hatch>,
    /// Draw (pen): when the next stroke starts closer than this (mm) to where the last one ended, the pen stays down and draws a line
    /// across instead of lifting; `None` = always lift. (The lines of a fill are joined on their own.)
    #[serde(default, skip_serializing_if = "Option::is_none")] pub join: Option<f64>,
    /// Draw (pen): how many times the whole drawing is gone over (a darker, more even line); `None` = once.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub repeat: Option<u32>,
    /// Draw (pen): seconds the pen rests on the paper after going down, before the stroke starts (ink flow, dots, a spring holder settling); `None` = none.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub dwell: Option<f64>,
}

/// How a Draw operation fills closed shapes: parallel lines (and a second set across them when `cross`).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Hatch {
    /// Direction of the lines, degrees counter-clockwise from the X axis.
    #[serde(default = "d_hatch_angle")] pub angle: f64,
    /// Distance between the lines in mm; `None` = the pen's line width (the ink just touches).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub gap: Option<f64>,
    /// A second set of lines at right angles.
    #[serde(default)] pub cross: bool,
    /// Also draw the outline of every shape.
    #[serde(default = "d_true")] pub outline: bool,
}

fn d_hatch_angle() -> f64 { 45.0 }

impl Default for Hatch { fn default() -> Self { Hatch { angle: 45.0, gap: None, cross: false, outline: true } } }

/// Spindle speed (rpm), cutting feed, plunge feed (mm/min) and stepover (% of the diameter) of one operation that differ from its tool's.
/// `None` = follow the tool, so changing the tool's value later still reaches the operation.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct CustomFeeds {
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rpm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub feed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub plunge: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub so: Option<f64>,
}

impl CustomFeeds {
    /// The tool with this operation's values in place of its own (values that are not positive numbers are ignored).
    pub fn apply(&self, t: &Tool) -> Tool {
        let ok = |v: Option<f64>| v.filter(|x| x.is_finite() && *x > 0.0);
        let mut o = t.clone();
        if let Some(v) = ok(self.rpm) { o.rpm = v; }
        if let Some(v) = ok(self.feed) { o.feed = v; }
        if let Some(v) = ok(self.plunge) { o.plunge = v; }
        if let Some(v) = ok(self.so) { o.so = v.min(100.0); }
        o
    }
}

/// A separate piece of stock that an inlay plug is cut from (the pocket is cut in the main stock). The piece sits beside
/// the main stock in the drawing: `x`, `y` are the drawing position of its lower-left corner, in mm. The rest is
/// the same as `Stock`; the G-code of the piece uses its own work zero.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PlugPiece {
    pub x: f64, pub y: f64, pub w: f64, pub h: f64, pub t: f64,
    #[serde(default = "d_wood")] pub mat: String,
    /// Work zero: ll, lr, ul, ur or c.
    #[serde(default = "d_ll")] pub zr: String,
    /// Z zero: "top" or "h" (height above the piece's bottom, see `zh`).
    #[serde(default = "d_top")] pub zm: String,
    #[serde(default)] pub zh: f64,
    /// The piece takes the main stock's place on the machine (the main stock is swapped for it): it has the main stock's
    /// size and work zero, so the machine needs no new XY zero. Only its thickness and material are its own.
    #[serde(default, skip_serializing_if = "is_false")] pub same: bool,
}

impl PlugPiece {
    /// The piece as it is cut: a piece that replaces the main stock has its size and zero.
    pub fn effective(&self, main: &Stock) -> PlugPiece {
        if !self.same { return self.clone(); }
        PlugPiece { w: main.w, h: main.h, zr: main.zr.clone(), zm: main.zm.clone(), zh: main.zh, ..self.clone() }
    }

    /// The piece as a stock of its own, with its lower-left corner at (0, 0).
    pub fn stock(&self) -> Stock { Stock { w: self.w, h: self.h, t: self.t, mat: self.mat.clone(), cut: String::new(), zr: self.zr.clone(), zm: self.zm.clone(), zh: self.zh } }
}

/// A manually placed holding tab, anchored to a shape: `l` is the index of the shape in the project's
/// loop list, `u`/`v` the tab's position inside that shape's bounding box (0..1, y up from the bottom).
/// The tab sits on the point of the outline nearest to that position, so moving or resizing the shape
/// moves the tab with it.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct TabPt { pub l: usize, pub u: f64, pub v: f64 }

/// The fingers cut into a rectangle (`joints::make_fingers`): shapes made by one cut share an `id`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FingerSpec {
    pub id: i64,
    /// Bottom, right, top, left: 0 flat, 1 tabs, 2 slots.
    pub edges: [u8; 4],
    pub width: f64,
    pub clearance: f64,
    /// The thickness (mm) the fingers were cut for.
    pub depth: f64,
    /// Cutter diameter of the dogbone reliefs, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub dog: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct LoopData {
    /// 1 = closed.
    #[serde(default)] pub c: u8,
    /// Operation id this shape belongs to (0 = none).
    #[serde(default)] pub op: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rc: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub cr: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub el: Option<f64>,
    /// A drill point: a tiny shape that only the Drill operation uses (it drills at its centre).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub pt: Option<f64>,
    /// The imported file this shape came from (see `ProjectData::groups`), so it can be sized and moved as one.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub grp: Option<i64>,
    /// The group the user made with Edit > Group: shapes with the same number are selected, moved, scaled and turned together.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub ug: Option<i64>,
    /// The layer this shape is on (`ProjectData::layers`); none = the first layer.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub lay: Option<i64>,
    /// How the fingers of this shape were cut, so they can be cut again when the stock thickness changes.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub fj: Option<FingerSpec>,
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


fn is_false(b: &bool) -> bool { !*b }
fn is_zero(v: &f64) -> bool { *v == 0.0 }

fn ninety() -> f64 { 90.0 }
