//! First-run setup: a machine and a starter set of tools from a few choices.
//!
//! Nothing here knows about windows. The setup assistant of the app collects a `MachineSetup` and a `ToolChoice` and applies them;
//! these functions make the machine and the tool list, and are tested on their own.
use crate::model::{Machine, Tool};
use crate::toollib;

/// A typical machine of a size class: travel, rapid rate and limits to start from. Real machines differ: every value can be changed.
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    /// Travel in X, Y and Z, mm.
    pub x: f64, pub y: f64, pub z: f64,
    pub rapid: f64,
    pub rpm_max: Option<f64>,
    pub feed_max: Option<f64>,
    pub plunge_max: Option<f64>,
    /// "light", "medium" or "heavy" (`feeds::RIGIDITY`).
    pub rigid: &'static str,
}

/// Size classes, not products: the figures are round numbers for the class; check the manual of the machine for its own.
pub const PRESETS: [Preset; 4] = [
    Preset { id: "small", label: "Small desktop (about 300 × 180 mm, like a 3018)", x: 300.0, y: 180.0, z: 45.0, rapid: 2000.0, rpm_max: Some(10000.0), feed_max: Some(1000.0), plunge_max: Some(300.0), rigid: "light" },
    Preset { id: "medium", label: "Medium desktop (about 600 × 400 mm, like a 6040)", x: 600.0, y: 400.0, z: 80.0, rapid: 3000.0, rpm_max: None, feed_max: Some(2000.0), plunge_max: Some(600.0), rigid: "light" },
    Preset { id: "large", label: "Large format (about 1200 × 800 mm)", x: 1200.0, y: 800.0, z: 100.0, rapid: 5000.0, rpm_max: None, feed_max: Some(4000.0), plunge_max: Some(1000.0), rigid: "medium" },
    Preset { id: "custom", label: "Other: I will type the sizes", x: 300.0, y: 200.0, z: 60.0, rapid: 2000.0, rpm_max: None, feed_max: None, plunge_max: None, rigid: "light" },
];

/// What the assistant asks about the machine.
#[derive(Clone, Debug, PartialEq)]
pub struct MachineSetup {
    pub name: String,
    pub x: f64, pub y: f64, pub z: f64,
    pub rapid: f64,
    /// The G-code dialect (`model::DIALECTS`): "" = GRBL.
    pub post: String,
    pub rpm_max: Option<f64>,
    pub feed_max: Option<f64>,
    pub plunge_max: Option<f64>,
    pub rigid: String,
    /// Write the G-code in inches.
    pub inches: bool,
}

impl MachineSetup {
    pub fn from_preset(p: &Preset) -> MachineSetup {
        MachineSetup { name: "My CNC machine".into(), x: p.x, y: p.y, z: p.z, rapid: p.rapid, post: String::new(), rpm_max: p.rpm_max, feed_max: p.feed_max, plunge_max: p.plunge_max, rigid: p.rigid.into(), inches: false }
    }

    /// The machine, with a new id and every value in range.
    pub fn into_machine(&self) -> Machine {
        let mut m = Machine::default();
        m.id = String::new();
        m.name = self.name.clone();
        m.x = self.x; m.y = self.y; m.z = self.z;
        m.rapid = self.rapid;
        m.post = Some(self.post.clone());
        m.safe = Some(5.0);
        m.rpm_max = self.rpm_max;
        m.feed_max = self.feed_max;
        m.plunge_max = self.plunge_max;
        m.rigid = self.rigid.clone();
        m.inches = Some(self.inches);
        crate::machines::normalize(&mut m);
        m
    }
}

/// Which starter tools to put in the tool list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolChoice {
    /// The straight bits come in inch sizes (1/8″, 1/4″) instead of metric ones.
    pub inch_sizes: bool,
    /// V-bits and engraving bits.
    pub vbits: bool,
    /// Ball nose bits, for relief carving.
    pub ball: bool,
    /// A 1″ surfacing bit.
    pub surfacing: bool,
    /// The pens and markers of the plotter library.
    pub pens: bool,
}

impl Default for ToolChoice {
    fn default() -> Self { ToolChoice { inch_sizes: false, vbits: true, ball: false, surfacing: false, pens: false } }
}

fn pens() -> Vec<Tool> {
    #[derive(serde::Deserialize)]
    struct File { tools: Vec<Tool> }
    toollib::BUNDLED.iter().find(|(n, _)| *n == "Pens and markers")
        .and_then(|(_, j)| serde_json::from_str::<File>(j).ok()).map(|f| f.tools).unwrap_or_default()
}

/// The starter tools, cutters first (a new operation takes the first tool, so a pen must never be first), each with a fresh id.
pub fn starter_tools(c: ToolChoice) -> Vec<Tool> {
    let all = toollib::seed_tools();
    let pick = |names: &[&str]| -> Vec<Tool> { names.iter().filter_map(|n| all.iter().find(|t| t.name == *n).cloned()).collect() };
    let mut v = if c.inch_sizes {
        pick(&["1/8″ flat end mill", "1/4″ flat end mill", "1/8″ downcut end mill", "1/4″ downcut end mill"])
    } else {
        pick(&["1 mm flat end mill", "2 mm flat end mill", "3 mm flat end mill", "4 mm flat end mill", "6 mm flat end mill", "8 mm flat end mill"])
    };
    if c.vbits {
        v.extend(pick(if c.inch_sizes { &["60° V-bit (1/4″)", "90° V-bit (1/4″)", "30° engraving bit (1/8″)"] } else { &["90° V-bit (6 mm)", "30° engraving bit (1/8″)"] }));
    }
    if c.ball { v.extend(pick(&["1/8″ ball nose", "1/4″ ball nose"])); }
    if c.surfacing { v.extend(pick(&["1″ surfacing bit"])); }
    if c.pens { v.extend(pens()); }
    for t in &mut v { t.id = toollib::new_id("t"); }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_makes_a_machine_in_range() {
        for p in &PRESETS {
            let m = MachineSetup::from_preset(p).into_machine();
            assert!(!m.id.is_empty() && m.x > 0.0 && m.y > 0.0 && m.z > 0.0 && m.rapid > 0.0, "{}", p.id);
            assert_eq!(m.post.as_deref(), Some(""));
            assert_eq!(m.inches, Some(false));
            assert!(crate::feeds::RIGIDITY.iter().any(|r| r.0 == m.rigid), "{}", p.id);
        }
    }

    #[test]
    fn what_was_typed_ends_up_on_the_machine_and_nonsense_is_repaired() {
        let mut s = MachineSetup::from_preset(&PRESETS[0]);
        s.name = "  Workshop router ".into();
        s.post = "fluidnc".into();
        s.x = 1250.0; s.rpm_max = Some(18000.0); s.inches = true;
        let m = s.into_machine();
        assert_eq!(m.name, "Workshop router");
        assert_eq!((m.x, m.rpm_max, m.inches, m.post.as_deref()), (1250.0, Some(18000.0), Some(true), Some("fluidnc")));
        s.x = -5.0; s.name = String::new(); s.post = "nonsense".into(); s.rpm_max = Some(0.0);
        let m = s.into_machine();
        assert!(m.x > 0.0 && !m.name.is_empty() && m.post.as_deref() == Some("") && m.rpm_max.is_none());
    }

    #[test]
    fn starter_tools_put_cutters_first_and_every_choice_has_something() {
        for inch in [false, true] {
            let t = starter_tools(ToolChoice { inch_sizes: inch, ..Default::default() });
            assert!(t.len() >= 4, "{inch}: {}", t.len());
            assert!(t.iter().all(|x| x.shape != "pen"));
            assert!(t[0].shape == "flat");
            let ids: std::collections::HashSet<_> = t.iter().map(|x| x.id.clone()).collect();
            assert_eq!(ids.len(), t.len(), "ids are unique");
        }
        let all = starter_tools(ToolChoice { inch_sizes: false, vbits: true, ball: true, surfacing: true, pens: true });
        assert!(all.iter().any(|t| t.shape == "pen") && all.iter().any(|t| t.shape == "vbit") && all.iter().any(|t| t.shape == "ball") && all.iter().any(|t| t.dia > 25.0));
        assert_ne!(all[0].shape, "pen", "the first tool, which a new operation takes, is a cutter");
        // the sizes follow the choice
        assert!(starter_tools(ToolChoice { inch_sizes: true, ..Default::default() }).iter().any(|t| t.name == "1/4″ flat end mill"));
        assert!(starter_tools(ToolChoice::default()).iter().any(|t| t.name == "6 mm flat end mill"));
    }

    #[test]
    fn the_starter_tools_make_a_usable_library() {
        let mut libs = toollib::ToolLibs::default();
        libs.libs[0].tools = starter_tools(ToolChoice::default());
        let before = libs.libs[0].tools.len();
        libs.repair();
        assert_eq!(libs.libs[0].tools.len(), before);
        assert!(libs.current().tools.iter().all(|t| t.dia > 0.0 && t.feed > 0.0));
    }
}
