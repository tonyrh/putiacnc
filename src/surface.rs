//! Surfacing: take a thin layer off the whole stock (or the spoilboard) with a wide cutter so the top is flat and level with the work zero.
//! It is made of the things the program already has: a rectangle as big as the stock, and a Pocket operation over it with the wide cutter,
//! a light cut and a wide stepover. The rectangle is a cutter radius larger than the stock on every side, so the cutter's edge (not its
//! centre) reaches the stock's edge and the whole surface is cleared.
use crate::i18n::{tr};
use crate::model::{Operation, ProjectData, Tool};
use crate::shapes;

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceSpec {
    /// The cutter (an id of a tool of the project).
    pub tool: String,
    /// Total depth taken off, mm.
    pub depth: f64,
    /// Depth of one pass, mm.
    pub pass: f64,
    /// Stepover, percent of the cutter's diameter.
    pub stepover: f64,
    /// Run it before the other operations (the usual place: the later ones then start from a flat top).
    pub first: bool,
    /// Run it as an air cut.
    pub air: bool,
}

/// The widest flat cutter, below 50 mm: the best one for surfacing (a surfacing bit when there is one).
pub fn suggest_tool(tools: &[Tool]) -> Option<&Tool> {
    tools.iter().filter(|t| t.shape == "flat" && t.dia > 0.0 && t.dia <= 50.0).max_by(|a, b| a.dia.total_cmp(&b.dia))
}

/// Sensible values for a cutter: a stepover of 70 % at most (80 % for a surfacing bit, which is made for it) and a pass no deeper than the cutter's own.
pub fn defaults(t: &Tool, spoilboard: bool) -> SurfaceSpec {
    let depth = if spoilboard { 1.0 } else { 0.5 };
    SurfaceSpec { tool: t.id.clone(), depth, pass: depth.min(if t.pd > 0.0 { t.pd } else { depth }), stepover: if t.so > 0.0 { t.so.min(80.0) } else { 60.0 }, first: true, air: false }
}

/// Adds the surfacing operation and its rectangle. Returns the operation's id.
pub fn add_surfacing(d: &mut ProjectData, s: &SurfaceSpec) -> Result<i64, String> {
    let tool = d.tools.iter().find(|t| t.id == s.tool).ok_or(tr("Choose a cutter for the surfacing."))?.clone();
    if tool.shape != "flat" { return Err(tr("Surfacing needs a flat cutter.").into()); }
    if !(s.depth > 0.0 && s.pass > 0.0 && s.stepover > 0.0 && s.stepover <= 100.0) { return Err(tr("Check the depth, the pass depth and the stepover.").into()); }
    if d.stock.w <= 0.0 || d.stock.h <= 0.0 { return Err(tr("The stock has no size.").into()); }
    let r = tool.dia / 2.0;
    let id = d.ops.iter().map(|o| o.id).max().unwrap_or(0).max(d.oid) + 1;
    let mut p = shapes::default_op_params(&tool.id);
    p.kind = "pocket".into();
    p.depth = s.depth;
    p.pass_depth = Some(s.pass.min(s.depth));
    p.air = s.air;
    // a cut in the air above the stock, then straight down: no ramp, no tabs
    p.ramp = false; p.tabs = false; p.surf = true;
    p.custom = Some(crate::model::CustomFeeds { so: Some(s.stepover), ..Default::default() });
    let op = Operation { id, p };
    if s.first { d.ops.insert(0, op); } else { d.ops.push(op); }
    d.loops.push(shapes::new_rect([-r, -r], [d.stock.w + r, d.stock.h + r], 0.0, id));
    d.oid = id;
    d.act = id;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generate, Units};

    fn project() -> ProjectData {
        let mut f = shapes::blank_project();
        f.data.machine = Some(crate::model::Machine::default());
        f.data.tools = crate::setup::starter_tools(crate::setup::ToolChoice { surfacing: true, ..Default::default() });
        f.data
    }

    #[test]
    fn the_widest_flat_cutter_is_suggested() {
        let d = project();
        let t = suggest_tool(&d.tools).unwrap();
        assert!(t.name.contains("surfacing") && (t.dia - 25.4).abs() < 1e-9, "{}", t.name);
        assert!(defaults(t, false).stepover <= 80.0);
    }

    #[test]
    fn surfacing_covers_the_whole_stock_with_the_cutters_edge() {
        let mut d = project();
        let t = suggest_tool(&d.tools).unwrap().clone();
        let spec = defaults(&t, false);
        add_surfacing(&mut d, &spec).unwrap();
        // the rectangle is a radius larger than the stock on every side
        let b = shapes::bbox_many(d.loops.iter()).unwrap();
        let r = t.dia / 2.0;
        assert!((b[0] + r).abs() < 1e-6 && (b[1] + r).abs() < 1e-6 && (b[2] - d.stock.w - r).abs() < 1e-6 && (b[3] - d.stock.h - r).abs() < 1e-6, "{b:?}");
        let o = generate(&d, Units::Metric).unwrap();
        assert!(!o.notes.iter().any(|n| n.starts_with('\u{26a0}')), "{:?}", o.notes);
        // the tool centre stays over the stock; its edge reaches (but does not pass) the expanded rectangle
        let mut lowest = f64::MAX;
        let (mut minx, mut maxx, mut miny, mut maxy) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for l in o.lines.iter().filter(|l| l.starts_with("G1")) {
            for w in l.split_whitespace() {
                let v = |p: &str| w.strip_prefix(p).and_then(|x| x.parse::<f64>().ok());
                if let Some(z) = v("Z") { lowest = lowest.min(z); }
                if let Some(x) = v("X") { minx = minx.min(x); maxx = maxx.max(x); }
                if let Some(y) = v("Y") { miny = miny.min(y); maxy = maxy.max(y); }
            }
        }
        assert!((lowest + 0.5).abs() < 1e-6, "lowest Z {lowest}");
        assert!(minx >= -1e-6 && miny >= -1e-6 && maxx <= d.stock.w + 1e-6 && maxy <= d.stock.h + 1e-6, "centre path leaves the stock: x {minx}..{maxx} y {miny}..{maxy}");
        assert!(minx < 1.0 && miny < 1.0 && maxx > d.stock.w - 1.0 && maxy > d.stock.h - 1.0, "centre path does not reach the stock's edges: x {minx}..{maxx} y {miny}..{maxy}");
    }

    #[test]
    fn it_runs_first_and_other_operations_keep_their_shapes() {
        let mut d = project();
        crate::sample::fill_kind(&mut d, false, crate::sample::Kind::NamePlate).unwrap();
        let before: Vec<(i64, usize)> = d.ops.iter().map(|o| (o.id, d.loops.iter().filter(|l| l.op == o.id).count())).collect();
        let t = suggest_tool(&d.tools).unwrap().clone();
        let id = add_surfacing(&mut d, &defaults(&t, false)).unwrap();
        assert_eq!(d.ops[0].id, id, "surfacing is not the first operation");
        for (oid, n) in before { assert_eq!(d.loops.iter().filter(|l| l.op == oid).count(), n); }
        assert!(generate(&d, Units::Metric).is_ok());
        // the other way: last
        let mut d2 = project();
        crate::sample::fill_kind(&mut d2, false, crate::sample::Kind::NamePlate).unwrap();
        let t = suggest_tool(&d2.tools).unwrap().clone();
        let mut s = defaults(&t, false); s.first = false;
        let id2 = add_surfacing(&mut d2, &s).unwrap();
        assert_eq!(d2.ops.last().unwrap().id, id2);
    }

    #[test]
    fn bad_values_and_tools_are_refused() {
        let mut d = project();
        let t = suggest_tool(&d.tools).unwrap().clone();
        let mut s = defaults(&t, false);
        s.depth = 0.0;
        assert!(add_surfacing(&mut d, &s).is_err());
        let mut s = defaults(&t, false);
        s.tool = "nope".into();
        assert!(add_surfacing(&mut d, &s).is_err());
        let vb = d.tools.iter().find(|t| t.shape == "vbit").unwrap().id.clone();
        let mut s = defaults(&t, false);
        s.tool = vb;
        assert!(add_surfacing(&mut d, &s).is_err());
        assert!(d.ops.is_empty() && d.loops.is_empty());
    }
}
