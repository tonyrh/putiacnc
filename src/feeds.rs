//! Feeds and speeds by material: from what is being cut, the tool and how stiff the machine is, suggest spindle speed,
//! feed, plunge, stepover and depth per pass. A starting point for a test cut, not a promise: the numbers are typical
//! hobby-router values (chip load per tooth by material and diameter), and a stiffer machine may go further.
use crate::model::{Machine, Tool};

/// What is being cut.
pub struct Material {
    pub id: &'static str,
    pub label: &'static str,
    /// Chip-load factor against the generic range in `toollib::chip_load_range` (1 = soft wood).
    chip: f64,
    /// Usual spindle speed window, rpm.
    rpm: (f64, f64),
    /// Plunge rate as a fraction of the feed.
    plunge: f64,
    /// Stepover, percent of the diameter.
    so: f64,
    /// Depth per pass as a fraction of the diameter.
    pass: f64,
    /// Things worth knowing when cutting it.
    notes: &'static [&'static str],
}

pub const MATERIALS: [Material; 7] = [
    Material { id: "softwood", label: "Softwood (pine, spruce)", chip: 1.0, rpm: (10000.0, 24000.0), plunge: 0.4, so: 45.0, pass: 0.5, notes: &[] },
    Material { id: "hardwood", label: "Hardwood (oak, beech, walnut)", chip: 0.75, rpm: (10000.0, 20000.0), plunge: 0.3, so: 35.0, pass: 0.35, notes: &["Hard wood burns when the feed is too slow: do not go below the suggested feed."] },
    Material { id: "plywood", label: "Plywood", chip: 0.85, rpm: (12000.0, 24000.0), plunge: 0.35, so: 40.0, pass: 0.45, notes: &["Glue layers dull a cutter quickly; a downcut or compression bit keeps the top edge clean."] },
    Material { id: "mdf", label: "MDF / chipboard", chip: 1.0, rpm: (12000.0, 24000.0), plunge: 0.4, so: 45.0, pass: 0.5, notes: &["MDF dust is fine and unhealthy: use dust extraction and a mask."] },
    Material { id: "acrylic", label: "Acrylic / plastic", chip: 0.6, rpm: (8000.0, 16000.0), plunge: 0.3, so: 35.0, pass: 0.35, notes: &["Use a single-flute or O-flute cutter and keep it moving: a pause melts the plastic. Blow the chips away."] },
    Material { id: "aluminium", label: "Aluminium", chip: 0.3, rpm: (6000.0, 14000.0), plunge: 0.2, so: 25.0, pass: 0.15, notes: &["Use a cutter made for aluminium (1 to 3 flutes, upcut) with lubricant or an air blast. Light cuts only; try it on scrap first."] },
    Material { id: "foam", label: "Foam / wax", chip: 1.4, rpm: (12000.0, 24000.0), plunge: 0.5, so: 50.0, pass: 1.0, notes: &["Only sharp cutters; keep the chips clear or they clog the flutes."] },
];

pub fn material(id: &str) -> Option<&'static Material> { MATERIALS.iter().find(|m| m.id == id) }

/// How stiff the machine is: a stiffer one takes heavier chips and deeper cuts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Rigidity { #[default] Light, Medium, Heavy }

pub const RIGIDITY: [(&str, &str); 3] = [("light", "Light (hobby router, desktop machine)"), ("medium", "Medium (strong frame, good spindle)"), ("heavy", "Heavy (industrial machine)")];

impl Rigidity {
    /// From a machine's setting; anything unknown is light.
    pub fn parse(s: &str) -> Rigidity { match s { "medium" => Rigidity::Medium, "heavy" => Rigidity::Heavy, _ => Rigidity::Light } }
    fn factor(self) -> f64 { match self { Rigidity::Light => 1.0, Rigidity::Medium => 1.3, Rigidity::Heavy => 1.7 } }
}

/// The values to put into an operation, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub rpm: f64,
    pub feed: f64,
    pub plunge: f64,
    /// Stepover, percent of the diameter.
    pub so: f64,
    /// Depth per pass in mm; `None` for tools whose depth is the design's (V-bits, engraving bits).
    pub pass: Option<f64>,
    /// Chip load these values give, mm per tooth, and the usual range for the material.
    pub chip: f64,
    pub range: (f64, f64),
    pub notes: Vec<String>,
}

/// The usual chip-load range (mm per tooth) of the tool in this material on a machine this stiff.
pub fn chip_range(mat: &str, t: &Tool, rigid: Rigidity) -> Option<(f64, f64)> {
    let m = material(mat)?;
    let (lo, hi) = crate::toollib::chip_load_range(t);
    let f = m.chip * rigid.factor();
    Some((lo * f, hi * f))
}

fn round_to(v: f64, step: f64) -> f64 { (v / step).round() * step }

/// Suggested feeds and speeds for cutting `mat` with `t` (None: no such material, or a tool that does not cut, such as a pen).
/// The machine's spindle and feed limits are respected and mentioned.
pub fn suggest(mat: &str, t: &Tool, rigid: Rigidity, machine: Option<&Machine>) -> Option<Suggestion> {
    let m = material(mat)?;
    if t.shape == "pen" || !(t.dia > 0.0) || t.flutes == 0 { return None; }
    let mut notes: Vec<String> = m.notes.iter().map(|s| s.to_string()).collect();
    // the spindle speed the tool is set for, kept when it suits the material; big cutters turn slower
    let (mut lo, mut hi) = m.rpm;
    if t.dia > 12.0 { hi *= 0.7; }
    if t.dia > 20.0 { hi *= 0.7; }
    lo = lo.min(hi);
    let mut rpm = if t.rpm > 0.0 { t.rpm } else { hi }.clamp(lo, hi);
    if let Some(mc) = machine {
        let (a, b) = (mc.rpm_min.unwrap_or(0.0), mc.rpm_max.unwrap_or(f64::MAX));
        let held = rpm.clamp(a, b.max(a));
        if (held - rpm).abs() > 0.5 { notes.push(format!("The machine's spindle limits hold the speed to {} rpm.", held.round())); }
        rpm = held;
    }
    let (clo, chi) = chip_range(mat, t, rigid)?;
    // the middle of the range is the suggestion
    let chip = (clo + chi) / 2.0;
    let mut feed = rpm * t.flutes as f64 * chip;
    let mut plunge = feed * m.plunge;
    if let Some(mc) = machine {
        if let Some(fm) = mc.feed_max.filter(|v| *v > 0.0) { if feed > fm { notes.push(format!("The machine's top feed of {} mm/min holds the feed down.", fm.round())); feed = fm; } }
        if let Some(pm) = mc.plunge_max.filter(|v| *v > 0.0) { if plunge > pm { plunge = pm; } }
    }
    let feed = round_to(feed, 10.0).max(10.0);
    let plunge = round_to(plunge, 10.0).clamp(10.0, feed);
    if feed < 150.0 { notes.push("This feed is very slow: the cutter may rub instead of cutting. A smaller cutter, or a faster spindle if it allows it, helps.".into()); }
    if t.name.to_lowercase().contains("downcut") && matches!(m.id, "aluminium" | "acrylic") {
        notes.push("A downcut bit packs the chips into the slot in this material: an upcut or O-flute cutter clears them better.".into());
    }
    let free_depth = matches!(t.shape.as_str(), "vbit" | "engrave");
    let pass = if free_depth { None } else {
        let mut p = (m.pass * rigid.factor()).min(1.0) * t.dia;
        // small cutters are thin and break: they take less than their share
        if t.dia <= 4.0 { p *= 0.75; }
        if t.flute > 0.0 { p = p.min(t.flute); }
        Some(((p * 20.0).round() / 20.0).max(0.1))
    };
    notes.push("A starting point: make a test cut on scrap and raise the feed or the depth if the cut is clean and the machine is not struggling.".into());
    Some(Suggestion { rpm: round_to(rpm, 100.0).max(100.0), feed, plunge, so: m.so, pass, chip: feed / (rpm * t.flutes as f64), range: (clo, chi), notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(shape: &str, dia: f64, flutes: u32, rpm: f64) -> Tool {
        Tool { id: "t".into(), name: "t".into(), sys: "mm".into(), shape: shape.into(), angle: 60.0, flutes, dia, rpm, feed: 800.0, plunge: 250.0, so: 40.0, pd: 1.0, notes: String::new(), tip: 0.0, arc: 0.0, concave: false, flute: 0.0, shank: 0.0, stick: 0.0, color: None }
    }

    #[test]
    fn every_material_gives_sane_numbers_for_every_cutter() {
        for m in MATERIALS.iter() {
            for (shape, dia) in [("flat", 3.175), ("flat", 6.0), ("flat", 12.7), ("ball", 6.0), ("vbit", 12.0)] {
                for r in [Rigidity::Light, Rigidity::Medium, Rigidity::Heavy] {
                    let t = tool(shape, dia, 2, 12000.0);
                    let s = suggest(m.id, &t, r, None).unwrap_or_else(|| panic!("{} {shape} {dia}", m.id));
                    assert!(s.rpm >= 1000.0 && s.rpm <= 24000.0, "{} {s:?}", m.id);
                    assert!(s.feed >= 10.0 && s.feed <= 8000.0, "{} {shape} {dia}: {s:?}", m.id);
                    assert!(s.plunge >= 10.0 && s.plunge <= s.feed, "{s:?}");
                    assert!((10.0..=60.0).contains(&s.so));
                    assert!(s.chip >= s.range.0 * 0.85 && s.chip <= s.range.1 * 1.15, "{} {s:?}", m.id);
                    assert_eq!(s.pass.is_none(), shape == "vbit", "{shape}");
                    if let Some(p) = s.pass { assert!(p >= 0.1 && p <= dia + 1e-6, "{p} for {dia}"); }
                    assert!(!s.notes.is_empty());
                }
            }
        }
    }

    #[test]
    fn harder_material_means_lighter_cuts_and_a_stiffer_machine_means_heavier_ones() {
        let t = tool("flat", 6.0, 2, 12000.0);
        let get = |m: &str, r| suggest(m, &t, r, None).unwrap();
        let (soft, hard, alu) = (get("softwood", Rigidity::Light), get("hardwood", Rigidity::Light), get("aluminium", Rigidity::Light));
        assert!(soft.feed > hard.feed && hard.feed > alu.feed, "{} {} {}", soft.feed, hard.feed, alu.feed);
        assert!(soft.so > hard.so && hard.so > alu.so);
        assert!(soft.pass.unwrap() > hard.pass.unwrap() && hard.pass.unwrap() > alu.pass.unwrap());
        assert!(alu.plunge / alu.feed < soft.plunge / soft.feed, "aluminium plunges relatively slower");
        let heavy = get("softwood", Rigidity::Heavy);
        assert!(heavy.feed > soft.feed * 1.5 && heavy.pass.unwrap() > soft.pass.unwrap());
        // a bigger cutter takes a bigger chip, and a ball nose and V-bit a lighter one
        let big = suggest("softwood", &tool("flat", 12.0, 2, 12000.0), Rigidity::Light, None).unwrap();
        assert!(big.chip > soft.chip * 1.5, "{} {}", big.chip, soft.chip);
        let ball = suggest("softwood", &tool("ball", 6.0, 2, 12000.0), Rigidity::Light, None).unwrap();
        assert!(ball.chip < soft.chip);
    }

    #[test]
    fn the_feed_follows_the_spindle_speed_and_the_number_of_flutes() {
        let two = suggest("softwood", &tool("flat", 6.0, 2, 12000.0), Rigidity::Light, None).unwrap();
        let one = suggest("softwood", &tool("flat", 6.0, 1, 12000.0), Rigidity::Light, None).unwrap();
        assert!((two.feed / one.feed - 2.0).abs() < 0.05, "{} {}", two.feed, one.feed);
        // a speed the material does not like is brought into its window: aluminium is not cut at 24000 rpm
        let alu = suggest("aluminium", &tool("flat", 6.0, 2, 24000.0), Rigidity::Light, None).unwrap();
        assert_eq!(alu.rpm, 14000.0);
        // a tool with no speed gets the top of the window
        assert_eq!(suggest("softwood", &tool("flat", 6.0, 2, 0.0), Rigidity::Light, None).unwrap().rpm, 24000.0);
    }

    #[test]
    fn machine_limits_are_respected_and_said() {
        let mut m = Machine::default();
        m.rpm_max = Some(10000.0);
        m.feed_max = Some(500.0);
        m.plunge_max = Some(100.0);
        let s = suggest("softwood", &tool("flat", 6.0, 2, 18000.0), Rigidity::Heavy, Some(&m)).unwrap();
        assert_eq!(s.rpm, 10000.0);
        assert_eq!(s.feed, 500.0);
        assert!(s.plunge <= 100.0);
        assert!(s.notes.iter().any(|n| n.contains("spindle limits")) && s.notes.iter().any(|n| n.contains("top feed")), "{:?}", s.notes);
        // a spindle that cannot go slowly enough for its own limit
        let mut slow = Machine::default();
        slow.rpm_min = Some(8000.0);
        assert_eq!(suggest("aluminium", &tool("flat", 6.0, 2, 6000.0), Rigidity::Light, Some(&slow)).unwrap().rpm, 8000.0);
    }

    #[test]
    fn pens_unknown_materials_and_notes() {
        assert!(suggest("softwood", &tool("pen", 0.5, 1, 0.0), Rigidity::Light, None).is_none());
        assert!(suggest("cheese", &tool("flat", 6.0, 2, 12000.0), Rigidity::Light, None).is_none());
        assert!(suggest("", &tool("flat", 6.0, 2, 12000.0), Rigidity::Light, None).is_none());
        let mut d = tool("flat", 6.0, 2, 12000.0);
        d.name = "6 mm downcut".into();
        assert!(suggest("aluminium", &d, Rigidity::Light, None).unwrap().notes.iter().any(|n| n.contains("downcut")));
        assert!(!suggest("softwood", &d, Rigidity::Light, None).unwrap().notes.iter().any(|n| n.contains("downcut")));
        // the cutting length limits the depth per pass
        let mut short = tool("flat", 6.0, 2, 12000.0);
        short.flute = 2.0;
        assert_eq!(suggest("foam", &short, Rigidity::Light, None).unwrap().pass, Some(2.0));
        assert_eq!(Rigidity::parse("heavy"), Rigidity::Heavy);
        assert_eq!(Rigidity::parse("anything"), Rigidity::Light);
        assert!(chip_range("mdf", &d, Rigidity::Light).is_some() && chip_range("", &d, Rigidity::Light).is_none());
    }
}
