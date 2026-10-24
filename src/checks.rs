//! Checks on a finished toolpath: things that would hurt the machine, the tool or the work. Some read the
//! moves and the settings alone (too deep for the tool, outside the machine); the others replay the moves on
//! the height-field simulation and look at what the tool touches (a rapid through material, a shank rubbing
//! the walls, a clamp in the way).
use crate::model::{ProjectData, Tool};
use crate::sim::Sim;
use crate::clip::{self, Path};
use crate::shapes::S;
use crate::toolpath::{inside_eo, Output};

/// How far (mm) the cutter may stray from the line before it counts as cutting past the design.
const TOL: f64 = 0.2;

/// Where a flat cutter's centre may not be, for one operation.
enum Guard {
    /// Outside profile: the centre must stay out of every part (own and other operations') grown by the cutter radius.
    Keep(Vec<Vec<Path>>),
    /// Inside profile or pocket: the centre must stay within the shape shrunk by the cutter radius.
    Stay(Vec<Path>),
}

/// Even-odd test against closed paths, looking only at the paths whose bounding box holds the point.
fn inside_zone(z: &[Path], x: f64, y: f64) -> bool {
    let (xi, yi) = ((x * S) as i64, (y * S) as i64);
    let near: Vec<Path> = z.iter().filter(|p| {
        let (mut x0, mut x1, mut y0, mut y1) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
        for q in p.iter() { x0 = x0.min(q[0]); x1 = x1.max(q[0]); y0 = y0.min(q[1]); y1 = y1.max(q[1]); }
        xi >= x0 && xi <= x1 && yi >= y0 && yi <= y1
    }).cloned().collect();
    inside_eo(&near, x, y)
}

impl Guard {
    fn violates(&self, x: f64, y: f64) -> bool {
        match self {
            Guard::Keep(zones) => zones.iter().any(|z| inside_zone(z, x, y)),
            Guard::Stay(safe) => !inside_zone(safe, x, y),
        }
    }
}

fn region_of(data: &ProjectData, op: usize) -> Vec<Path> {
    let Some(o) = data.ops.get(op) else { return vec![] };
    let closed: Vec<Path> = data.loops.iter().filter(|l| l.op == o.id && l.closed() && l.pts.len() / 2 > 2 && !crate::shapes::is_point(l)).map(|l| l.path()).collect();
    clip::simplify_even_odd(&closed)
}

fn flat_radius(data: &ProjectData, op: usize) -> Option<f64> {
    let t = tool_of(data, op, false)?;
    if t.shape == "flat" && t.dia > 0.0 { Some(t.dia / 2.0) } else { None }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind { TooDeep, Spoilboard, OutOfArea, ZTravel, RapidCut, Shank, Clamp, TooSmall, Overcut }

impl Kind {
    pub fn title(self) -> &'static str {
        match self {
            Kind::TooDeep => "Deeper than the cutting length",
            Kind::Spoilboard => "Cuts into the table",
            Kind::OutOfArea => "Outside the machine",
            Kind::ZTravel => "Not enough Z travel",
            Kind::RapidCut => "Rapid move through material",
            Kind::Shank => "Shank rubbing the walls",
            Kind::Clamp => "Tool hits a clamp",
            Kind::TooSmall => "Shape smaller than the tool",
            Kind::Overcut => "Tool cuts past the design",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub kind: Kind,
    /// Index of the operation, when it belongs to one.
    pub op: Option<usize>,
    /// The first move that does it, and where the tool tip is then (mm, stock coordinates, z up from the top).
    pub mv: Option<usize>,
    pub at: [f64; 3],
    /// How many moves do it (1 for settings that are not about a move).
    pub count: usize,
    pub msg: String,
}

fn tool_of(d: &ProjectData, op: usize, alt: bool) -> Option<&Tool> {
    let o = d.ops.get(op)?;
    let id = if alt { o.p.tool2.as_deref()? } else { o.p.tool.as_str() };
    d.tools.iter().find(|t| t.id == id)
}

fn op_name(d: &ProjectData, op: usize) -> String {
    let kind = d.ops.get(op).map(|o| match o.p.kind.as_str() { "outside" => "Profile outside", "inside" => "Profile inside", "online" => "Profile on the line", "pocket" => "Pocket", "vcarve" => "V-carve", "drill" => "Drill", "inlay" => "Inlay pocket", "plug" => "Inlay plug", "relief" => "3D relief", k => k }).unwrap_or("Operation");
    format!("Operation {} ({kind})", op + 1)
}

/// Does the segment from a to b come within r of the rectangle?
fn seg_near_rect(a: [f64; 2], b: [f64; 2], c: &crate::model::Clamp, r: f64) -> bool {
    let (x0, y0, x1, y1) = (c.x - r, c.y - r, c.x + c.w + r, c.y + c.h + r);
    // clip the segment to the grown rectangle (Liang-Barsky); the corners are cut square, a little cautious
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    for (p, q) in [(-dx, a[0] - x0), (dx, x1 - a[0]), (-dy, a[1] - y0), (dy, y1 - a[1])] {
        if p == 0.0 { if q < 0.0 { return false; } } else {
            let t = q / p;
            if p < 0.0 { if t > t1 { return false; } t0 = t0.max(t); } else { if t < t0 { return false; } t1 = t1.min(t); }
        }
    }
    t0 <= t1
}

pub fn run(data: &ProjectData, out: &Output) -> Vec<Finding> {
    let mut found: Vec<Finding> = Vec::new();
    let t_stock = data.stock.t;

    // ---- from the settings and the moves alone ----
    // how deep each operation goes with its cutting moves
    let mut deepest: Vec<Option<(f64, usize)>> = vec![None; data.ops.len()];
    let mut lowest: Option<(f64, usize)> = None;
    for (i, m) in out.moves.iter().enumerate() {
        if !m.cut { continue; }
        let z = m.from[2].min(m.to[2]);
        if let Some(d) = deepest.get_mut(m.op) { if d.is_none_or(|(v, _)| z < v) { *d = Some((z, i)); } }
        if lowest.is_none_or(|(v, _)| z < v) { lowest = Some((z, i)); }
    }
    for (op, d) in deepest.iter().enumerate() {
        let Some((z, i)) = *d else { continue };
        let Some(t) = tool_of(data, op, false) else { continue };
        if t.flute > 0.0 && -z > t.flute + 0.01 {
            let m = &out.moves[i];
            found.push(Finding { kind: Kind::TooDeep, op: Some(op), mv: Some(i), at: m.to, count: 1,
                msg: format!("{} cuts {:.1} mm deep, but the {} has a cutting length of {:.1} mm: the shank would rub the work.", op_name(data, op), -z, if t.name.is_empty() { "tool" } else { &t.name }, t.flute) });
        }
    }
    if let Some((z, i)) = lowest {
        if -z > t_stock + 0.5 {
            let m = &out.moves[i];
            found.push(Finding { kind: Kind::Spoilboard, op: Some(m.op), mv: Some(i), at: m.to, count: 1,
                msg: format!("{} goes {:.1} mm below the bottom of the stock: that cuts into the table or spoilboard (a through cut needs 0.1 to 0.3 mm).", op_name(data, m.op), -z - t_stock) });
        }
    }
    if let Some(mc) = &data.machine {
        // the stock's lower-left corner is the machine's origin, as the drawn work area shows
        let bad: Vec<usize> = out.moves.iter().enumerate().filter(|(_, m)| [m.from, m.to].iter().any(|p| p[0] < -0.01 || p[1] < -0.01 || p[0] > mc.x + 0.01 || p[1] > mc.y + 0.01)).map(|(i, _)| i).collect();
        if let Some(&i) = bad.first() {
            let m = &out.moves[i];
            found.push(Finding { kind: Kind::OutOfArea, op: Some(m.op), mv: Some(i), at: m.to, count: bad.len(),
                msg: format!("{} moves outside the machine's work area ({:.0} × {:.0} mm).", op_name(data, m.op), mc.x, mc.y) });
        }
        if mc.z > 0.0 && t_stock + data.job.safe > mc.z + 1e-6 {
            found.push(Finding { kind: Kind::ZTravel, op: None, mv: None, at: [0.0, 0.0, 0.0], count: 1,
                msg: format!("The stock ({t_stock:.1} mm) plus the safe height ({:.1} mm) is more than the machine's Z travel ({:.1} mm).", data.job.safe, mc.z) });
        }
    }

    // ---- against the drawing ----
    let kind_of = |op: usize| data.ops.get(op).map(|o| o.p.kind.as_str()).unwrap_or("");
    let mut guards: Vec<Option<Guard>> = (0..data.ops.len()).map(|_| None).collect();
    let parts: Vec<(usize, Vec<Path>)> = (0..data.ops.len()).filter(|&j| kind_of(j) == "outside").map(|j| (j, region_of(data, j))).collect();
    for op in 0..data.ops.len() {
        let kind = kind_of(op);
        let Some(r) = flat_radius(data, op) else { continue };
        match kind {
            "outside" => {
                let zones: Vec<Vec<Path>> = parts.iter().filter(|(_, p)| !p.is_empty()).map(|(_, p)| clip::offset(p, (r - TOL) * S, S * 0.01)).collect();
                guards[op] = Some(Guard::Keep(zones));
            }
            "inside" | "pocket" => {
                let region = region_of(data, op);
                if region.is_empty() { continue; }
                guards[op] = Some(Guard::Stay(clip::offset(&region, -(r - TOL) * S, S * 0.01)));
                // shapes narrower than the cutter give no toolpath at all
                let id = data.ops[op].id;
                let loops: Vec<Path> = data.loops.iter().filter(|l| l.op == id && l.closed() && l.pts.len() / 2 > 2 && !crate::shapes::is_point(l)).map(|l| l.path()).collect();
                let small = loops.iter().enumerate().filter(|(i, p)| {
                    let others: Vec<Path> = loops.iter().enumerate().filter(|(j, _)| j != i).map(|(_, q)| q.clone()).collect();
                    let nested = inside_eo(&others, p[0][0] as f64 / S, p[0][1] as f64 / S);
                    !nested && clip::offset(&clip::simplify_even_odd(&[(*p).clone()]), -r * S, S * 0.01).is_empty()
                }).count();
                if small > 0 {
                    found.push(Finding { kind: Kind::TooSmall, op: Some(op), mv: None, at: [0.0; 3], count: small,
                        msg: format!("{}: {} shape{} narrower than the {:.2} mm tool, so {} not cut at all. Use a smaller tool or remove {}.", op_name(data, op), small, if small == 1 { " is" } else { "s are" }, r * 2.0, if small == 1 { "it is" } else { "they are" }, if small == 1 { "it" } else { "them" }) });
                }
            }
            _ => {}
        }
    }
    let mut over_done = vec![false; data.ops.len()];

    // ---- by replaying the moves ----
    let mut sim = Sim::new(data, 250_000);
    let step = sim.cell.max(0.4);
    let mut seen: std::collections::HashMap<(Kind, usize), usize> = std::collections::HashMap::new();
    let mut note = |found: &mut Vec<Finding>, kind: Kind, i: usize, at: [f64; 3], msg: String| {
        let op = out.moves[i].op;
        match seen.get(&(kind, op)) {
            Some(&k) => found[k].count += 1,
            None => { seen.insert((kind, op), found.len()); found.push(Finding { kind, op: Some(op), mv: Some(i), at, count: 1, msg }); }
        }
    };
    for (i, m) in out.moves.iter().enumerate() {
        let slot = m.op * 2 + m.alt as usize;
        let tool = tool_of(data, m.op, m.alt);
        let r = sim.tool_radius(slot);
        let len = ((m.to[0] - m.from[0]).powi(2) + (m.to[1] - m.from[1]).powi(2) + (m.to[2] - m.from[2]).powi(2)).sqrt();
        let n = ((len / step).ceil() as usize).max(1);
        let at = |k: usize| { let f = k as f64 / n as f64; [m.from[0] + (m.to[0] - m.from[0]) * f, m.from[1] + (m.to[1] - m.from[1]) * f, m.from[2] + (m.to[2] - m.from[2]) * f] };

        for c in &data.clamps {
            if m.from[2].min(m.to[2]) <= c.height && seg_near_rect([m.from[0], m.from[1]], [m.to[0], m.to[1]], c, r) {
                note(&mut found, Kind::Clamp, i, m.to, format!("{} runs into a clamp at {:.0}, {:.0} mm.", op_name(data, m.op), c.x, c.y));
            }
        }
        if m.cut && !m.alt && !over_done.get(m.op).copied().unwrap_or(true) && m.from[2].min(m.to[2]) < -0.01 {
            if let Some(g) = guards.get(m.op).and_then(|g| g.as_ref()) {
                for k in 0..=n {
                    let p = at(k);
                    if p[2] >= -0.01 || !g.violates(p[0], p[1]) { continue; }
                    over_done[m.op] = true;
                    let why = if matches!(g, Guard::Keep(_)) { "cuts into a part: shapes closer together than the tool's width, or overlapping" } else { "cuts outside its shape" };
                    note(&mut found, Kind::Overcut, i, p, format!("{} {why} (by more than {TOL} mm), first at {:.1}, {:.1} mm.", op_name(data, m.op), p[0], p[1]));
                    break;
                }
            }
        }
        if !m.cut {
            if m.from[2].min(m.to[2]) < -0.02 && r > 0.0 {
                for k in 0..=n {
                    let p = at(k);
                    if p[2] >= -0.02 { continue; }
                    let ex = sim.inside_cutter(slot, p[0], p[1], p[2]);
                    if ex > 0.05 {
                        note(&mut found, Kind::RapidCut, i, p, format!("{} has a rapid move through material ({ex:.1} mm into the stock): the tool would crash. Check the safe height and the retracts.", op_name(data, m.op)));
                        break;
                    }
                }
            }
        } else if let Some(t) = tool {
            if t.flute > 0.0 && t.shank > t.dia + 0.01 && m.from[2].min(m.to[2]) + t.flute < 0.0 {
                for k in 0..=n {
                    let p = at(k);
                    let top = p[2] + t.flute;
                    if top >= 0.0 { continue; }
                    let ex = sim.above_in_ring(p[0], p[1], r + sim.cell * 1.5 + 0.05, t.shank / 2.0, top);
                    if ex > 0.05 {
                        note(&mut found, Kind::Shank, i, p, format!("{}: the {:.1} mm shank of the {} would rub the walls ({ex:.1} mm of material above its cutting length of {:.1} mm).", op_name(data, m.op), t.shank, if t.name.is_empty() { "tool" } else { &t.name }, t.flute));
                        break;
                    }
                }
            }
        }
        sim.advance(&out.moves, i + 1);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generate, shapes, Units};

    fn proj(kind: &str, depth: f64) -> ProjectData {
        let mut f = shapes::blank_project();
        let d = &mut f.data;
        d.stock.w = 60.0; d.stock.h = 40.0; d.stock.t = 12.0;
        d.ops.push(crate::model::Operation { id: 1, p: shapes::default_op_params(&d.tools[0].id) });
        d.ops[0].p.kind = kind.into();
        d.ops[0].p.depth = depth;
        d.oid = 1; d.act = 1;
        d.loops.push(shapes::new_rect([15.0, 10.0], [45.0, 30.0], 0.0, 1));
        f.data
    }

    fn kinds(d: &ProjectData) -> Vec<Kind> { let o = generate(d, Units::Metric).unwrap(); run(d, &o).iter().map(|f| f.kind).collect() }

    #[test]
    fn ordinary_jobs_raise_nothing() {
        for (k, depth) in [("pocket", 3.0), ("outside", 12.2), ("inside", 5.0), ("online", 2.0)] {
            let mut d = proj(k, depth);
            d.machine = None;
            assert_eq!(kinds(&d), vec![], "{k}");
        }
    }

    #[test]
    fn a_tool_that_is_too_short_for_the_depth_is_found() {
        let mut d = proj("pocket", 8.0);
        d.tools[0].flute = 5.0;
        assert!(kinds(&d).contains(&Kind::TooDeep));
        d.tools[0].flute = 12.0;
        assert!(!kinds(&d).contains(&Kind::TooDeep));
    }

    #[test]
    fn cutting_far_below_the_stock_is_found() {
        let d = proj("outside", 14.0);
        assert!(kinds(&d).contains(&Kind::Spoilboard));
    }

    #[test]
    fn work_outside_the_machine_area_is_found() {
        let mut d = proj("pocket", 3.0);
        let mut m = d.machine.clone().unwrap_or_else(|| crate::machines::Machines::from_json("{}").map(|l| l.resolve("").clone()).unwrap_or_else(|_| panic!("no machine")));
        m.x = 40.0; m.y = 100.0;
        d.machine = Some(m);
        assert!(kinds(&d).contains(&Kind::OutOfArea));
    }

    #[test]
    fn a_clamp_in_the_way_is_found_and_one_far_away_is_not() {
        let mut d = proj("pocket", 3.0);
        d.clamps.push(crate::model::Clamp { x: 40.0, y: 5.0, w: 10.0, h: 10.0, height: 20.0 });
        assert!(kinds(&d).contains(&Kind::Clamp));
        d.clamps[0].x = 200.0;
        assert!(!kinds(&d).contains(&Kind::Clamp));
        // a clamp lower than the tool's path is passed over
        d.clamps[0].x = 40.0; d.clamps[0].height = -50.0;
        assert!(!kinds(&d).contains(&Kind::Clamp));
    }

    #[test]
    fn a_rapid_through_the_stock_is_found() {
        let d = proj("pocket", 3.0);
        let mut o = generate(&d, Units::Metric).unwrap();
        // make a sideways rapid at 2 mm depth across the uncut stock
        let i = o.moves.iter().position(|m| m.cut).unwrap();
        let mut bad = o.moves[i].clone();
        bad.cut = false; bad.from = [2.0, 2.0, -2.0]; bad.to = [8.0, 2.0, -2.0];
        o.moves.insert(0, bad);
        let f = run(&d, &o);
        assert!(f.iter().any(|f| f.kind == Kind::RapidCut), "{f:?}");
    }

    #[test]
    fn a_narrow_neck_rubbing_the_walls_is_found() {
        let mut d = proj("pocket", 8.0);
        // a 3 mm cutter with a 12 mm shank, 4 mm of cutting length, in a pocket 8 mm deep
        d.tools[0].dia = 3.0; d.tools[0].flute = 4.0; d.tools[0].shank = 12.0;
        assert!(kinds(&d).contains(&Kind::Shank));
        d.tools[0].shank = 0.0;
        assert!(!kinds(&d).contains(&Kind::Shank));
    }

    #[test]
    fn parts_too_close_for_a_second_operation_are_a_gouge() {
        let mut d = proj("outside", 3.0);
        d.machine = None;
        // a second part 1 mm from the first, cut by its own operation with the same 3 mm tool
        d.ops.push(crate::model::Operation { id: 2, p: d.ops[0].p.clone() });
        d.oid = 2;
        d.loops.push(shapes::new_rect([46.0, 10.0], [56.0, 30.0], 0.0, 2));
        assert!(kinds(&d).contains(&Kind::Overcut));
        // a clear gap is fine
        d.loops.pop();
        d.loops.push(shapes::new_rect([55.0, 10.0], [58.0, 30.0], 0.0, 2));
        assert!(!kinds(&d).contains(&Kind::Overcut));
    }

    #[test]
    fn a_shape_narrower_than_the_tool_is_reported() {
        let mut d = proj("pocket", 2.0);
        d.machine = None;
        d.loops.push(shapes::new_rect([5.0, 5.0], [7.0, 7.0], 0.0, 1));
        let o = generate(&d, Units::Metric).unwrap();
        let f = run(&d, &o);
        let small = f.iter().find(|f| f.kind == Kind::TooSmall).expect("reported");
        assert_eq!(small.count, 1);
        // islands inside a pocket are not "too small"
        d.loops.pop();
        d.loops.push(shapes::new_rect([25.0, 15.0], [27.0, 17.0], 0.0, 1));
        assert!(!kinds(&d).contains(&Kind::TooSmall));
    }
}
