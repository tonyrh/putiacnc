//! Tool libraries: several named sets of tools, one of them current. Mirrors the browser app's
//! tool manager (including its `putia-cnc-tools` export format).
use crate::model::{Operation, Tool};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

pub const SHAPES: [(&str, &str); 7] = [
    ("flat", "Flat end mill"), ("ball", "Ball nose"), ("bull", "Bull nose"),
    ("vbit", "V-bit"), ("engrave", "Engraving bit"), ("taper", "Tapered ball nose"), ("arc", "Arc profile bit"),
];

pub fn shape_label(s: &str) -> &'static str {
    SHAPES.iter().find(|(k, _)| *k == s).map(|(_, l)| *l).unwrap_or("Flat end mill")
}

pub fn has_angle(shape: &str) -> bool { matches!(shape, "vbit" | "engrave" | "taper") }

fn base36(mut n: u64) -> String {
    if n == 0 { return "0".into(); }
    let mut v = vec![];
    while n > 0 { v.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(n % 36) as usize]); n /= 36; }
    v.reverse();
    String::from_utf8(v).unwrap()
}

/// Unique-enough id (time + counter), e.g. `tmgx3k2a1`.
pub fn new_id(prefix: &str) -> String {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    format!("{prefix}{}{}", base36(ms), base36(CTR.fetch_add(1, Ordering::Relaxed)))
}

/// Metric or imperial, guessed from the diameter (a fractional-inch size reads as imperial).
pub fn infer_sys(dia: f64) -> &'static str {
    let q = dia / 25.4 * 64.0;
    if (dia - dia.round()).abs() < 1e-6 { "mm" } else if (q - q.round()).abs() < 0.02 { "in" } else { "mm" }
}

/// Clamp every field into its valid range (same rules as the browser app).
pub fn normalize_tool(t: &mut Tool) {
    if t.id.is_empty() { t.id = new_id("t"); }
    t.name = if t.name.trim().is_empty() { "Tool".into() } else { t.name.chars().take(60).collect() };
    if t.sys.is_empty() { t.sys = infer_sys(t.dia).into(); }
    if t.sys != "in" { t.sys = "mm".into(); }
    if !SHAPES.iter().any(|(k, _)| *k == t.shape) { t.shape = "flat".into(); }
    t.flutes = t.flutes.clamp(1, 12);
    t.angle = if t.angle.is_finite() { t.angle.clamp(10.0, 150.0) } else { 60.0 };
    t.dia = if t.dia.is_finite() { t.dia.max(0.01) } else { 3.0 };
    t.rpm = if t.rpm.is_finite() { t.rpm.max(0.0) } else { 0.0 };
    t.feed = if t.feed.is_finite() { t.feed.max(1.0) } else { 800.0 };
    t.plunge = if t.plunge.is_finite() { t.plunge.max(1.0) } else { 250.0 };
    t.so = if t.so.is_finite() { t.so.clamp(1.0, 100.0) } else { 40.0 };
    t.pd = if t.pd.is_finite() { t.pd.max(0.01) } else { 1.0 };
    t.tip = if t.tip.is_finite() { t.tip.max(0.0) } else { 0.0 };
    t.arc = if t.arc.is_finite() { t.arc.max(0.0) } else { 0.0 };
}

#[allow(clippy::too_many_arguments)]
fn tool(name: &str, sys: &str, shape: &str, angle: f64, dia: f64, flutes: u32, rpm: f64, feed: f64, plunge: f64, so: f64, pd: f64, notes: &str) -> Tool {
    Tool { id: new_id("t"), name: name.into(), sys: sys.into(), shape: shape.into(), angle, flutes, dia, rpm, feed, plunge, so, pd, notes: notes.into(), tip: 0.0, arc: 0.0, concave: false }
}

/// The tools most hobby routers use (fresh ids on every call). The speeds are cautious starting points for
/// wood, MDF and plastics on a light machine: adjust them to your machine, spindle and material.
/// The first five are the original starter tools and keep their order.
pub fn seed_tools() -> Vec<Tool> {
    let up = "2-flute upcut: general purpose for wood, MDF and plastics.";
    vec![
        tool("1/8″ flat end mill", "in", "flat", 60.0, 3.175, 2, 12000.0, 800.0, 250.0, 40.0, 1.0, up),
        tool("6 mm flat end mill", "mm", "flat", 60.0, 6.0, 2, 10000.0, 1000.0, 300.0, 40.0, 2.0, up),
        tool("1/4″ flat end mill", "in", "flat", 60.0, 6.35, 2, 10000.0, 1000.0, 300.0, 40.0, 2.0, up),
        tool("60° V-bit (1/4″)", "in", "vbit", 60.0, 6.35, 2, 12000.0, 800.0, 250.0, 40.0, 2.0, "V-carving and engraving; also chamfers."),
        tool("90° V-bit (6 mm)", "mm", "vbit", 90.0, 6.0, 2, 12000.0, 800.0, 250.0, 40.0, 2.0, "V-carving, chamfers and lettering."),
        tool("1 mm flat end mill", "mm", "flat", 60.0, 1.0, 2, 12000.0, 300.0, 100.0, 40.0, 0.5, "Fine detail and small text. Fragile: shallow passes, slow plunge."),
        tool("2 mm flat end mill", "mm", "flat", 60.0, 2.0, 2, 12000.0, 500.0, 150.0, 40.0, 0.8, "Small details and tight corners."),
        tool("3 mm flat end mill", "mm", "flat", 60.0, 3.0, 2, 12000.0, 800.0, 250.0, 40.0, 1.0, "Metric twin of the 1/8″: profiles and pockets."),
        tool("4 mm flat end mill", "mm", "flat", 60.0, 4.0, 2, 12000.0, 900.0, 250.0, 40.0, 1.5, up),
        tool("8 mm flat end mill", "mm", "flat", 60.0, 8.0, 2, 10000.0, 1200.0, 350.0, 40.0, 3.0, "Large pockets and fast roughing; needs a rigid machine."),
        tool("1/8″ downcut end mill", "in", "flat", 60.0, 3.175, 2, 12000.0, 800.0, 250.0, 40.0, 1.0, "Downcut: a clean top edge on plywood and laminates; it packs chips into the cut."),
        tool("1/4″ downcut end mill", "in", "flat", 60.0, 6.35, 2, 10000.0, 1000.0, 300.0, 40.0, 2.0, "Downcut: a clean top edge on plywood and laminates; it packs chips into the cut."),
        tool("1/4″ compression bit", "in", "flat", 60.0, 6.35, 2, 12000.0, 1200.0, 300.0, 40.0, 3.0, "Upcut at the bottom, downcut at the top: clean faces on both sides of plywood. Cut deeper than its downcut section."),
        tool("1/8″ ball nose", "in", "ball", 60.0, 3.175, 2, 12000.0, 700.0, 200.0, 15.0, 1.0, "Relief carving and rounded edges; a small stepover leaves a smoother surface."),
        tool("1/4″ ball nose", "in", "ball", 60.0, 6.35, 2, 10000.0, 900.0, 250.0, 15.0, 2.0, "Relief carving and smooth, rounded pockets."),
        tool("90° V-bit (1/4″)", "in", "vbit", 90.0, 6.35, 2, 12000.0, 800.0, 250.0, 40.0, 2.0, "V-carving, chamfers and lettering."),
        tool("30° engraving bit (1/8″)", "in", "engrave", 30.0, 3.175, 2, 12000.0, 500.0, 150.0, 40.0, 0.5, "Fine lettering and detailed engraving; shallow cuts."),
        tool("1″ surfacing bit", "in", "flat", 60.0, 25.4, 2, 12000.0, 1500.0, 300.0, 80.0, 0.5, "Flattening the spoilboard or a slab: light passes with a wide stepover."),
    ]
}

/// Chip load in mm per tooth: how thick a chip each cutting edge takes (feed / (spindle speed x flutes)).
pub fn chip_load(t: &Tool) -> Option<f64> {
    if t.rpm <= 0.0 || t.flutes == 0 { return None; }
    Some(t.feed / (t.rpm * t.flutes as f64))
}

/// A typical chip-load range (mm per tooth) for wood, MDF and plastics on a light machine. It grows with the
/// diameter; ball and V-shaped bits take less because only part of their edge is cutting.
pub fn chip_load_range(t: &Tool) -> (f64, f64) {
    let k = match t.shape.as_str() { "ball" | "bull" | "taper" | "arc" => 0.8, "vbit" | "engrave" => 0.5, _ => 1.0 };
    let mid = (0.012 * t.dia * k).clamp(0.004, 0.12);
    (0.45 * mid, 1.5 * mid)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Load { Light, Good, Heavy }

pub fn load_level(t: &Tool) -> Option<Load> {
    let c = chip_load(t)?;
    let (lo, hi) = chip_load_range(t);
    Some(if c < lo { Load::Light } else if c > hi { Load::Heavy } else { Load::Good })
}

/// Material removed per minute (mm³) for a cut as wide as the stepover and as deep as the maximum pass.
pub fn removal_rate(t: &Tool) -> f64 { t.feed * t.dia * t.so / 100.0 * t.pd }

/// A new tool based on `base` (a copy, or the first starter tool), like "+ New tool" / "Duplicate".
pub fn new_tool_from(base: Option<&Tool>, copy: bool, default_sys: &str) -> Tool {
    let b = base.cloned().unwrap_or_else(|| seed_tools().remove(0));
    Tool {
        id: new_id("t"),
        name: if copy { format!("{} copy", b.name) } else { "New tool".into() },
        sys: if copy { if b.sys.is_empty() { default_sys.into() } else { b.sys.clone() } } else { default_sys.into() },
        ..b
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library { pub id: String, pub name: String, pub tools: Vec<Tool> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLibs { pub app: String, pub v: u32, pub cur: String, pub libs: Vec<Library> }

impl Default for ToolLibs {
    fn default() -> Self {
        let id = new_id("l");
        ToolLibs { app: "putia-cnc-toollibs".into(), v: 1, cur: id.clone(), libs: vec![Library { id, name: "My tools".into(), tools: seed_tools() }] }
    }
}

impl ToolLibs {
    pub fn from_json(s: &str) -> Result<Self, String> {
        let mut t: ToolLibs = serde_json::from_str(s).map_err(|e| format!("Not a valid tool library store: {e}"))?;
        if t.app != "putia-cnc-toollibs" { return Err("Not a PutiaCNC tool library store.".into()); }
        t.repair();
        Ok(t)
    }

    pub fn to_json(&self) -> String { serde_json::to_string_pretty(self).unwrap_or_default() }

    /// Make the store consistent: at least one library, a valid current one, normalised tools.
    pub fn repair(&mut self) {
        for l in &mut self.libs {
            for t in &mut l.tools { normalize_tool(t); }
            if l.tools.is_empty() { l.tools = seed_tools(); }
            let mut seen = std::collections::HashSet::new();
            for t in &mut l.tools { if !seen.insert(t.id.clone()) { t.id = new_id("t"); } }
        }
        if self.libs.is_empty() { *self = ToolLibs::default(); }
        if !self.libs.iter().any(|l| l.id == self.cur) { self.cur = self.libs[0].id.clone(); }
    }

    pub fn current(&self) -> &Library { self.libs.iter().find(|l| l.id == self.cur).unwrap_or(&self.libs[0]) }
    pub fn current_mut(&mut self) -> &mut Library {
        let i = self.libs.iter().position(|l| l.id == self.cur).unwrap_or(0);
        &mut self.libs[i]
    }

    pub fn unique_name(&self, base: &str, except: Option<&str>) -> String {
        let taken: Vec<&str> = self.libs.iter().filter(|l| Some(l.id.as_str()) != except).map(|l| l.name.as_str()).collect();
        let mut nm = base.to_string();
        let mut n = 2;
        while taken.contains(&nm.as_str()) { nm = format!("{base} ({n})"); n += 1; }
        nm
    }

    /// Create a library with the common hobby tools and make it current.
    pub fn new_library(&mut self) {
        let name = self.unique_name("Hobby CNC", None);
        self.add(name, seed_tools());
    }

    /// Copy the current library (new tool ids are NOT created: projects keep resolving their tools).
    pub fn duplicate(&mut self) {
        let cur = self.current().clone();
        let name = self.unique_name(&format!("{} copy", cur.name), None);
        self.add(name, cur.tools);
    }

    fn add(&mut self, name: String, tools: Vec<Tool>) {
        let id = new_id("l");
        self.libs.push(Library { id: id.clone(), name, tools });
        self.cur = id;
    }

    pub fn rename_current(&mut self, name: &str) {
        let id = self.cur.clone();
        let nm = self.unique_name(if name.trim().is_empty() { "Untitled library" } else { name.trim() }, Some(&id));
        self.current_mut().name = nm;
    }

    /// Delete the current library; the first remaining one becomes current. Returns its name.
    pub fn delete_current(&mut self) -> Result<String, String> {
        if self.libs.len() < 2 { return Err("Keep at least one library.".into()); }
        let i = self.libs.iter().position(|l| l.id == self.cur).unwrap_or(0);
        let removed = self.libs.remove(i);
        self.cur = self.libs[0].id.clone();
        Ok(removed.name)
    }

    pub fn switch(&mut self, id: &str) { if self.libs.iter().any(|l| l.id == id) { self.cur = id.to_string(); } }

    /// JSON in the browser app's `putia-cnc-tools` format.
    pub fn export_current(&self) -> String {
        let l = self.current();
        serde_json::to_string_pretty(&serde_json::json!({ "app": "putia-cnc-tools", "v": 1, "name": l.name, "tools": l.tools })).unwrap_or_default()
    }

    /// Import a `putia-cnc-tools` file as a new library (made current). Returns (name, tool count).
    pub fn import(&mut self, json: &str) -> Result<(String, usize), String> {
        #[derive(Deserialize)]
        struct File { app: String, #[serde(default)] name: String, tools: Vec<Tool> }
        let f: File = serde_json::from_str(json).map_err(|_| "This is not a PutiaCNC tool library file.".to_string())?;
        if f.app != "putia-cnc-tools" || f.tools.is_empty() { return Err("This is not a PutiaCNC tool library file.".into()); }
        let mut tools: Vec<Tool> = f.tools.into_iter().take(500).collect();
        let mut seen = std::collections::HashSet::new();
        for t in &mut tools { normalize_tool(t); if !seen.insert(t.id.clone()) { t.id = new_id("t"); } }
        let n = tools.len();
        let base: String = if f.name.trim().is_empty() { "Imported library".into() } else { f.name.chars().take(60).collect() };
        let name = self.unique_name(&base, None);
        self.add(name.clone(), tools);
        Ok((name, n))
    }
}

/// Point operations whose tool is missing at the first tool. Returns how many were changed.
pub fn reassign_missing(ops: &mut [Operation], tools: &[Tool]) -> usize {
    let Some(first) = tools.first() else { return 0 };
    let mut n = 0;
    for o in ops.iter_mut() {
        if !tools.iter().any(|t| t.id == o.p.tool) { o.p.tool = first.id.clone(); n += 1; }
    }
    n
}

/// Add tools from a project that the library doesn't have (matched by id).
pub fn merge_project_tools(lib: &mut Library, project_tools: &[Tool]) {
    for t in project_tools {
        if !lib.tools.iter().any(|x| x.id == t.id) {
            let mut t = t.clone();
            normalize_tool(&mut t);
            lib.tools.push(t);
        }
    }
}

/// Tool diameter for display: imperial tools in inches as a fraction (1/8, 1 1/4), metric tools in mm.
/// `unit` adds the " mm" / inch-mark suffix.
pub fn fmt_dia(dia_mm: f64, sys: &str, unit: bool) -> String {
    if sys != "in" {
        let s = format!("{:.2}", dia_mm);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        return if unit { format!("{s} mm") } else { s.to_string() };
    }
    let q = if unit { "\"" } else { "" };
    let inch = dia_mm / 25.4;
    for den in [2u32, 4, 8, 16, 32, 64, 128] {
        let n = (inch * den as f64).round();
        if n >= 1.0 && (inch - n / den as f64).abs() < 0.0004 {
            let n = n as u32;
            let g = { let (mut a, mut b) = (n, den); while b != 0 { let r = a % b; a = b; b = r; } a };
            let (n, d) = (n / g, den / g);
            let (whole, rem) = (n / d, n % d);
            return match (whole, rem) {
                (w, 0) => format!("{w}{q}"),
                (0, r) => format!("{r}/{d}{q}"),
                (w, r) => format!("{w} {r}/{d}{q}"),
            };
        }
    }
    let s = format!("{:.3}", inch);
    format!("{}{q}", s.trim_end_matches('0').trim_end_matches('.'))
}

pub const SPEED_UNITS: [(&str, f64); 6] = [("mm/min", 1.0), ("mm/s", 60.0), ("m/min", 1000.0), ("in/min", 25.4), ("in/s", 1524.0), ("ft/min", 304.8)];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dia_format() {
        assert_eq!(fmt_dia(3.175, "in", true), "1/8\"");
        assert_eq!(fmt_dia(6.35, "in", true), "1/4\"");
        assert_eq!(fmt_dia(31.75, "in", true), "1 1/4\"");
        assert_eq!(fmt_dia(25.4, "in", true), "1\"");
        assert_eq!(fmt_dia(6.0, "mm", true), "6 mm");
        assert_eq!(fmt_dia(3.2, "mm", true), "3.2 mm");
    }
    use crate::model::OpParams;

    fn op(id: i64, tool: &str) -> Operation {
        Operation { id, p: OpParams { kind: "outside".into(), dir: "climb".into(), depth: 1.0, ramp: true, tool: tool.into(), tabs: false, air: false, tab_n: None, tab_w: None, tab_h: None, tab_pts: None, pass_depth: None, lead: None, tool2: None, fit: None } }
    }

    #[test]
    fn seed_ids_are_unique_across_calls() {
        let a = seed_tools();
        let b = seed_tools();
        let mut ids: Vec<&str> = a.iter().chain(b.iter()).map(|t| t.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 2 * a.len());
    }

    #[test]
    fn normalisation_clamps_like_the_web_app() {
        let mut t = new_tool_from(None, false, "mm");
        t.name = "x".repeat(100); t.flutes = 99; t.angle = 3.0; t.dia = -1.0; t.so = 500.0; t.feed = 0.0; t.pd = 0.0; t.shape = "weird".into(); t.sys = "".into();
        normalize_tool(&mut t);
        assert_eq!((t.name.len(), t.flutes, t.angle, t.so, t.shape.as_str(), t.sys.as_str()), (60, 12, 10.0, 100.0, "flat", "mm"));
        assert!(t.dia >= 0.01 && t.feed >= 1.0 && t.pd >= 0.01);
    }

    #[test]
    fn system_is_inferred_from_diameter() {
        assert_eq!(infer_sys(6.0), "mm");
        assert_eq!(infer_sys(3.175), "in");
        assert_eq!(infer_sys(6.35), "in");
        assert_eq!(infer_sys(2.4), "mm");
    }

    #[test]
    fn library_management() {
        let mut l = ToolLibs::default();
        assert_eq!(l.current().name, "My tools");
        l.new_library();
        assert_eq!(l.current().name, "Hobby CNC");
        l.new_library();
        assert_eq!(l.current().name, "Hobby CNC (2)");
        l.rename_current("My tools"); // clashes with the first one
        assert_eq!(l.current().name, "My tools (2)");
        l.duplicate();
        assert_eq!(l.current().name, "My tools (2) copy");
        assert_eq!(l.libs.len(), 4);
        assert_eq!(l.delete_current().unwrap(), "My tools (2) copy");
        assert_eq!(l.libs.len(), 3);
        let mut one = ToolLibs::default();
        assert!(one.delete_current().is_err());
    }

    #[test]
    fn export_import_round_trip() {
        let mut l = ToolLibs::default();
        l.current_mut().tools[0].name = "Custom".into();
        let json = l.export_current();
        assert!(json.contains("putia-cnc-tools"));
        let (name, n) = l.import(&json).unwrap();
        assert_eq!((name.as_str(), n), ("My tools (2)", seed_tools().len()));
        assert_eq!(l.current().tools[0].name, "Custom");
        assert!(l.import("{\"app\":\"other\",\"tools\":[]}").is_err());
        assert!(l.import("nonsense").is_err());
    }

    #[test]
    fn store_round_trip_and_repair() {
        let l = ToolLibs::default();
        let back = ToolLibs::from_json(&l.to_json()).unwrap();
        assert_eq!(back.libs.len(), 1);
        let broken = r#"{"app":"putia-cnc-toollibs","v":1,"cur":"nope","libs":[{"id":"a","name":"A","tools":[]}]}"#;
        let fixed = ToolLibs::from_json(broken).unwrap();
        assert_eq!(fixed.cur, "a");
        assert_eq!(fixed.libs[0].tools.len(), seed_tools().len()); // empty library gets the starter tools
    }

    #[test]
    fn project_merge_and_reassign() {
        let mut l = ToolLibs::default();
        let mut extra = seed_tools().remove(1);
        extra.id = "from-project".into();
        let known = l.current().tools[0].clone();
        merge_project_tools(l.current_mut(), &[extra.clone(), known]);
        assert_eq!(l.current().tools.len(), seed_tools().len() + 1); // only the unknown one was added
        let mut ops = vec![op(1, "from-project"), op(2, "gone")];
        let n = reassign_missing(&mut ops, &l.current().tools);
        assert_eq!(n, 1);
        assert_eq!(ops[0].p.tool, "from-project");
        assert_eq!(ops[1].p.tool, l.current().tools[0].id);
    }

    #[test]
    fn hobby_set_is_sound() {
        let t = seed_tools();
        // the original starter tools come first, in their old order
        let first: Vec<&str> = t.iter().take(5).map(|x| x.name.as_str()).collect();
        assert_eq!(first, ["1/8″ flat end mill", "6 mm flat end mill", "1/4″ flat end mill", "60° V-bit (1/4″)", "90° V-bit (6 mm)"]);
        let mut names: Vec<&str> = t.iter().map(|x| x.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), t.len(), "names are unique");
        for x in &t {
            let mut y = x.clone();
            normalize_tool(&mut y);
            assert_eq!((y.dia, y.feed, y.plunge, y.pd, y.so, y.angle), (x.dia, x.feed, x.plunge, x.pd, x.so, x.angle), "{} is within the limits", x.name);
            assert_eq!(infer_sys(x.dia) == "in", x.sys == "in", "{}: the unit matches the size", x.name);
            assert!(x.plunge <= x.feed && !x.notes.is_empty(), "{}", x.name);
        }
    }

    #[test]
    fn chip_load_estimates() {
        let mut t = seed_tools().remove(0); // 1/8" flat, 12000 rpm, 800 mm/min, 2 flutes
        assert!((chip_load(&t).unwrap() - 800.0 / 24000.0).abs() < 1e-12);
        assert_eq!(load_level(&t), Some(Load::Good));
        t.feed = 100.0;
        assert_eq!(load_level(&t), Some(Load::Light));
        t.feed = 3000.0;
        assert_eq!(load_level(&t), Some(Load::Heavy));
        t.rpm = 0.0;
        assert!(chip_load(&t).is_none() && load_level(&t).is_none());
        // 3.175 mm wide, 40 % stepover, 1 mm deep, 800 mm/min
        let t = seed_tools().remove(0);
        assert!((removal_rate(&t) - 800.0 * 3.175 * 0.4 * 1.0).abs() < 1e-9);
        // every tool of the default library is in a sensible range
        for t in seed_tools() { assert_eq!(load_level(&t), Some(Load::Good), "{}", t.name); }
    }
}
