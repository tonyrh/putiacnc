//! Joints for CNC parts: dogbone corner reliefs (and, below, finger joints).
//!
//! A round cutter cannot reach into a sharp inside corner: it leaves a radius, and a square corner of a mating part will not go
//! in. A dogbone cuts a little further into the walls at the corner, with the cutter's centre going out to a point one radius
//! from the corner, so the corner itself is reached.
use crate::clip::{self, BoolKind, Path};
use crate::model::LoopData;
use crate::shapes::S;

/// Which corners get a relief.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    /// The cutter works inside the shapes (pockets, slots, holes for square parts): their sharp corners are the inside corners.
    Inside,
    /// The cutter works around the shapes (the outline of a part with notches): the notches' corners are the inside corners.
    Outside,
}

/// Corners opener than this (the angle between the two walls, degrees) are left alone: the cutter nearly reaches them, and the
/// vertices of a flattened curve are never touched.
const MAX_ANGLE: f64 = 140.0;
/// Corners sharper than this are spikes, not corners.
const MIN_ANGLE: f64 = 5.0;

/// The polygon of two discs of radius `r` at `a` and `b` and everything between them (a slot with round ends).
fn capsule(a: [f64; 2], b: [f64; 2], r: f64) -> Path {
    let steps = ((std::f64::consts::PI / (1.0 - 0.01 / r.max(0.02)).clamp(-1.0, 1.0).acos()).ceil() as usize).clamp(12, 72);
    let dir = [b[0] - a[0], b[1] - a[1]];
    let ang = dir[1].atan2(dir[0]);
    let mut pts: Vec<[f64; 2]> = Vec::new();
    // half a circle round `b`, from the left of the direction to the right of it, then half round `a`
    for k in 0..=steps { let t = ang + std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * k as f64 / steps as f64; pts.push([b[0] + r * t.cos(), b[1] + r * t.sin()]); }
    for k in 0..=steps { let t = ang - std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * k as f64 / steps as f64; pts.push([a[0] + r * t.cos(), a[1] + r * t.sin()]); }
    // the walk above goes clockwise: turn it round so the capsule is counter-clockwise like a region
    pts.reverse();
    pts.iter().map(|p| [(p[0] * S).round() as i64, (p[1] * S).round() as i64]).collect()
}

fn closed_paths(loops: &[&LoopData]) -> Vec<Path> {
    loops.iter().filter(|l| l.closed() && l.pts.len() >= 6).map(|l| {
        let mut p: Path = l.pts.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
        if p.len() > 1 && p[0] == p[p.len() - 1] { p.pop(); }
        p
    }).collect()
}

/// The shapes with a dogbone relief at each inside corner, for a cutter of diameter `tool_dia` (mm). The shapes are joined as one
/// region first (a shape inside another is a hole, as everywhere in the app), so a result may have fewer or more shapes than the
/// input; all take operation `op`. Returns the new shapes and the number of corners done, or `None` when there is no such corner.
pub fn dogbones(loops: &[&LoopData], tool_dia: f64, cut: Cut, op: i64) -> Option<(Vec<LoopData>, usize)> {
    reliefs(loops, tool_dia, cut, op, Relief::Dogbone)
}

/// The shape of a corner relief.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Relief {
    /// Along the bisector of the corner, into both walls.
    #[default]
    Dogbone,
    /// Straight out into one wall (the longer one, so the short wall keeps its length and the joint keeps its look); corners far from a right angle get a dogbone.
    TBone,
}

/// Like [`dogbones`], with a choice of relief.
pub fn reliefs(loops: &[&LoopData], tool_dia: f64, cut: Cut, op: i64, style: Relief) -> Option<(Vec<LoopData>, usize)> {
    if !(tool_dia > 1e-3) { return None; }
    let r = tool_dia / 2.0;
    let region = clip::simplify_even_odd(&closed_paths(loops));
    if region.is_empty() { return None; }
    // which side of its path the region lies on: the largest path is an outer one
    let big = region.iter().map(|p| clip::area(p)).fold(0.0_f64, |m, a| if a.abs() > m.abs() { a } else { m });
    let left_is_region = big > 0.0;
    let mut caps: Vec<Path> = Vec::new();
    for path in &region {
        let n = path.len();
        if n < 3 { continue; }
        let pt = |i: usize| { let q = path[i % n]; [q[0] as f64 / S, q[1] as f64 / S] };
        for i in 0..n {
            let (a, p, b) = (pt(i + n - 1), pt(i), pt(i + 1));
            let (va, vb) = ([a[0] - p[0], a[1] - p[1]], [b[0] - p[0], b[1] - p[1]]);
            let (da, db) = (va[0].hypot(va[1]), vb[0].hypot(vb[1]));
            if da < 0.05 || db < 0.05 { continue; }
            let (ua, ub) = ([va[0] / da, va[1] / da], [vb[0] / db, vb[1] / db]);
            let psi = (ua[0] * ub[0] + ua[1] * ub[1]).clamp(-1.0, 1.0).acos().to_degrees();
            if !(MIN_ANGLE..=MAX_ANGLE).contains(&psi) { continue; }
            // a turn towards the region's side is a corner of the region itself (an inside corner for a cutter working inside it)
            let turn = (p[0] - a[0]) * (b[1] - p[1]) - (p[1] - a[1]) * (b[0] - p[0]);
            let towards_region = (turn > 0.0) == left_is_region;
            if (cut == Cut::Inside) != towards_region { continue; }
            // the bisector of the two walls points into the narrow side, where the cutter is
            let bis = [ua[0] + ub[0], ua[1] + ub[1]];
            let bl = bis[0].hypot(bis[1]);
            if bl < 1e-9 { continue; }
            let u = [bis[0] / bl, bis[1] / bl];
            // the cutter's centre runs along the bisector as far as r/sin(angle/2) from the corner; going on to a distance r reaches it
            let far = r / (psi.to_radians() / 2.0).sin();
            // a hair wider than the cutter (0.03 mm): at exactly the cutter's width an offset by the cutter radius collapses to nothing and the toolpath never enters the relief
            let c0 = [p[0] + u[0] * r, p[1] + u[1] * r];
            if style == Relief::TBone && (70.0..=110.0).contains(&psi) {
                // the cutter backs out along the shorter wall, so the relief goes into the longer one: its centre ends a radius from the corner
                let e = if da >= db { ub } else { ua };
                let cf = [p[0] + u[0] * far, p[1] + u[1] * far];
                caps.push(capsule(cf, [cf[0] - e[0] * r, cf[1] - e[1] * r], r + 0.03));
            } else {
                caps.push(capsule(c0, [p[0] + u[0] * far, p[1] + u[1] * far], r + 0.03));
            }
        }
    }
    if caps.is_empty() { return None; }
    let count = caps.len();
    let caps = clip::simplify_even_odd(&caps.to_vec());
    // the capsules were joined with the even-odd rule only to merge overlaps: orient them as regions again
    let caps: Vec<Path> = caps.into_iter().collect();
    let res = clip::boolean(if cut == Cut::Inside { BoolKind::Union } else { BoolKind::Difference }, &region, &caps);
    let out: Vec<LoopData> = res.into_iter().filter(|p| p.len() >= 3).map(|p| LoopData {
        c: 1, op, rc: None, cr: None, el: None, pt: None, grp: None, ug: None, pts: p.iter().flat_map(|q| [q[0], q[1]]).collect(),
    }).collect();
    if out.is_empty() { None } else { Some((out, count)) }
}

/// What an edge of a plate does at a finger joint.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Edge {
    /// Left straight.
    #[default]
    Flat,
    /// Fingers stand out of the edge (the first and last are at the corners).
    Tabs,
    /// Notches go into the edge, in the places where the other plate's tabs are.
    Slots,
}

/// How the fingers are cut.
#[derive(Clone, Copy, Debug)]
pub struct Fingers {
    /// Wanted width of a finger, mm. The edge gets an odd number of equal fingers as close to this as fits.
    pub width: f64,
    /// How far tabs stand out and slots go in: the thickness of the material, mm.
    pub depth: f64,
    /// Extra room in the slots, mm: each slot is this much wider than its tab.
    pub clearance: f64,
}

/// The number of fingers an edge of `len` mm gets for a wanted width: odd, at least 3, and the fingers as close to `width` as possible.
pub fn finger_count(len: f64, width: f64) -> usize {
    let n = (len / width.max(1e-6)).round().max(1.0) as usize;
    let n = if n.is_multiple_of(2) { n + 1 } else { n };
    // of the two odd counts either side of the ideal, the one that gives the width nearer to the wanted
    let alt = n.saturating_sub(2).max(3);
    let best = if (len / alt as f64 - width).abs() < (len / n as f64 - width).abs() { alt } else { n };
    best.max(3)
}

/// Outline of the rectangle `bx` = [min x, min y, max x, max y] with finger joint edges, counter-clockwise from the bottom left corner
/// (edges in the order bottom, right, top, left). `None` when the rectangle or the fingers are degenerate.
pub fn finger_outline(bx: [f64; 4], edges: [Edge; 4], f: Fingers) -> Option<Vec<[f64; 2]>> {
    let (w, h) = (bx[2] - bx[0], bx[3] - bx[1]);
    if !(w > 0.0 && h > 0.0 && f.width > 0.0 && f.depth > 0.0 && f.clearance >= 0.0) { return None; }
    let corners = [[bx[0], bx[1]], [bx[2], bx[1]], [bx[2], bx[3]], [bx[0], bx[3]]];
    let mut out: Vec<[f64; 2]> = Vec::new();
    for e in 0..4 {
        let (p, q) = (corners[e], corners[(e + 1) % 4]);
        let len = (q[0] - p[0]).hypot(q[1] - p[1]);
        let (ux, uy) = ((q[0] - p[0]) / len, (q[1] - p[1]) / len);
        // outward is to the right of the direction of travel for a counter-clockwise outline
        let (nx, ny) = (uy, -ux);
        let at = |s: f64, o: f64| [p[0] + ux * s + nx * o, p[1] + uy * s + ny * o];
        out.push(p);
        if edges[e] == Edge::Flat { continue; }
        if f.depth * 2.0 >= (if e % 2 == 0 { h } else { w }) { return None; }
        let n = finger_count(len, f.width);
        let fw = len / n as f64;
        for k in (0..n).step_by(2) {
            let (a, b) = (fw * k as f64, fw * (k + 1) as f64);
            match edges[e] {
                Edge::Tabs => { out.push(at(a, 0.0)); out.push(at(a, f.depth)); out.push(at(b, f.depth)); out.push(at(b, 0.0)); }
                _ => {
                    let (a, b) = ((a - f.clearance / 2.0).max(0.0), (b + f.clearance / 2.0).min(len));
                    out.push(at(a, 0.0)); out.push(at(a, -f.depth)); out.push(at(b, -f.depth)); out.push(at(b, 0.0));
                }
            }
        }
    }
    // a corner that lies in a slot or at the foot of a tab is repeated
    out.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9);
    if out.len() > 1 && (out[0][0] - out[out.len() - 1][0]).abs() < 1e-9 && (out[0][1] - out[out.len() - 1][1]).abs() < 1e-9 { out.pop(); }
    Some(out)
}

/// The six plates of a box.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side { Bottom, Top, Front, Back, Left, Right, Divider }

impl Side {
    pub fn name(self) -> &'static str {
        match self { Side::Bottom => "Bottom", Side::Top => "Top", Side::Front => "Front", Side::Back => "Back", Side::Left => "Left", Side::Right => "Right", Side::Divider => "Divider" }
    }
    /// The plate that keeps a contested place wins: bottom and top over front and back over left and right.
    fn rank(self) -> u8 { match self { Side::Bottom | Side::Top => 0, Side::Front | Side::Back => 1, _ => 2 } }
}

/// A box made of finger-jointed plates. Sizes are outside sizes in mm.
#[derive(Clone, Copy, Debug)]
pub struct BoxSpec {
    pub w: f64, pub d: f64, pub h: f64,
    /// Thickness of the material.
    pub t: f64,
    pub finger: f64,
    pub clearance: f64,
    /// A top plate as well; without it the box is open.
    pub lid: bool,
    /// Plates standing inside the box, parallel to the left and right plates (splitting the width), evenly spaced, held in slots of the plates they meet.
    pub div_w: usize,
    /// Plates parallel to the front and back, splitting the depth. Where they cross the others each is slotted halfway (a half-lap).
    pub div_d: usize,
}

/// One plate: its size and its outline in its own frame, counter-clockwise, with the lower left corner of its box at (0, 0).
#[derive(Clone, Debug)]
pub struct Panel { pub side: Side, pub w: f64, pub h: f64, pub outline: Vec<[f64; 2]>, /// Slots cut right through the plate, for the tabs of dividers.
    pub holes: Vec<Vec<[f64; 2]>>,
    /// For a divider: (true when it splits the width, where it starts along that axis in mm from the outside).
    pub pos: Option<(bool, f64)> }

fn rect_path(x0: f64, y0: f64, x1: f64, y1: f64) -> Path { poly_path(&[[x0, y0], [x1, y0], [x1, y1], [x0, y1]]) }
fn poly_path(p: &[[f64; 2]]) -> Path { p.iter().map(|q| [(q[0] * S).round() as i64, (q[1] * S).round() as i64]).collect() }

/// Where each of `n` dividers starts along an axis of outside length `span`, mm from the outside.
pub fn divider_positions(span: f64, n: usize, t: f64) -> Vec<f64> {
    let nf = n as f64;
    let cw = (span - 2.0 * t - nf * t) / (nf + 1.0);
    (1..=n).map(|j| t + j as f64 * cw + (j as f64 - 1.0) * t).collect()
}

/// `spans` without the parts that lie in the `cut` intervals (leftovers under half a millimetre are dropped).
fn spans_minus(spans: Vec<(f64, f64)>, cut: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for sp in spans {
        let mut cur = vec![sp];
        for &(e0, e1) in cut {
            cur = cur.into_iter().flat_map(|(a, b)| if e1 <= a || e0 >= b { vec![(a, b)] } else {
                let mut v = Vec::new();
                if e0 > a { v.push((a, e0)); }
                if e1 < b { v.push((e1, b)); }
                v
            }).collect();
        }
        out.extend(cur.into_iter().filter(|(a, b)| b - a > 0.5));
    }
    out
}

/// The even-numbered finger spans (start, end) of an odd number of equal fingers fitted into [from, to].
fn even_spans(from: f64, to: f64, width: f64) -> Vec<(f64, f64)> {
    let n = finger_count(to - from, width);
    let fw = (to - from) / n as f64;
    (0..n).step_by(2).map(|k| (from + fw * k as f64, from + fw * (k + 1) as f64)).collect()
}

fn rect_pts(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> { vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]] }

/// The plates of a box that fit together. Every joint along a box edge alternates between the two plates that meet there, an odd
/// number of fingers from one end to the other, and each of the eight corners belongs to one plate only, so that nothing
/// overlaps and nothing is missing (except the clearance).
pub fn box_panels(b: BoxSpec) -> Result<Vec<Panel>, String> {
    let t = b.t;
    if !(t > 0.0 && b.finger > 0.0 && b.clearance >= 0.0) { return Err("The thickness and the finger width must be above zero.".into()); }
    if b.w < 4.0 * t || b.d < 4.0 * t || b.h < 4.0 * t { return Err(format!("The box must be at least {} mm in every direction for {} mm material.", 4.0 * t, t)); }
    for (n, la, lb) in [(b.div_w, b.d, b.w), (b.div_d, b.w, b.d)] {
        if n == 0 { continue; }
        if b.h < 6.0 * t { return Err(format!("Dividers need a box at least {} mm high for {} mm material.", 6.0 * t, t)); }
        if la < 6.0 * t { return Err("The box is too short for dividers.".into()); }
        let cw = (lb - 2.0 * t - n as f64 * t) / (n as f64 + 1.0);
        if cw < 2.0 * t { return Err("The dividers are too close together: the compartments must be at least twice the material thickness.".into()); }
    }
    let (xs_list, ys_list) = (divider_positions(b.w, b.div_w, t), divider_positions(b.d, b.div_d, t));
    let sides: Vec<Side> = if b.lid { vec![Side::Bottom, Side::Top, Side::Front, Side::Back, Side::Left, Side::Right] } else { vec![Side::Bottom, Side::Front, Side::Back, Side::Left, Side::Right] };
    // the plate that owns the corner cube where three plates (one of each pair of axes) would meet
    let corner_owner = |z: Option<Side>, y: Side, x: Side| -> Side { [z, Some(y), Some(x)].into_iter().flatten().min_by_key(|s| s.rank()).unwrap() };
    let mut out = Vec::new();
    for &side in &sides {
        // size, and for each local edge (bottom, right, top, left) the neighbouring plate, and for each local corner
        // (lower left, lower right, upper right, upper left) the owner of the corner cube
        let (w, h, nb, owners): (f64, f64, [Option<Side>; 4], [Side; 4]) = {
            let has = |s: Side| sides.contains(&s);
            let zs = |bottom: bool| if bottom { Some(Side::Bottom) } else if has(Side::Top) { Some(Side::Top) } else { None };
            match side {
                Side::Front | Side::Back => {
                    let (l, r) = (Side::Left, Side::Right);
                    (b.w, b.h, [Some(Side::Bottom), Some(r), has(Side::Top).then_some(Side::Top), Some(l)],
                        [corner_owner(zs(true), side, l), corner_owner(zs(true), side, r), corner_owner(zs(false), side, r), corner_owner(zs(false), side, l)])
                }
                Side::Left | Side::Right => {
                    let (f, k) = (Side::Front, Side::Back);
                    (b.d, b.h, [Some(Side::Bottom), Some(k), has(Side::Top).then_some(Side::Top), Some(f)],
                        [corner_owner(zs(true), f, side), corner_owner(zs(true), k, side), corner_owner(zs(false), k, side), corner_owner(zs(false), f, side)])
                }
                Side::Bottom | Side::Top | Side::Divider => {
                    let z = Some(side);
                    (b.w, b.d, [Some(Side::Front), Some(Side::Right), Some(Side::Back), Some(Side::Left)],
                        [corner_owner(z, Side::Front, Side::Left), corner_owner(z, Side::Front, Side::Right), corner_owner(z, Side::Back, Side::Right), corner_owner(z, Side::Back, Side::Left)])
                }
            }
        };
        let mut notches: Vec<Path> = Vec::new();
        // corner cubes that belong to another plate
        let cs = [(0.0, 0.0), (w - t, 0.0), (w - t, h - t), (0.0, h - t)];
        for (k, (x, y)) in cs.iter().enumerate() { if owners[k] != side { notches.push(rect_path(*x, *y, x + t, y + t)); } }
        for e in 0..4 {
            let Some(n) = nb[e] else { continue };
            let len = if e % 2 == 0 { w } else { h };
            let inner = len - 2.0 * t;
            if inner <= 0.0 { return Err("The box is too small for this material.".into()); }
            let cnt = finger_count(inner, b.finger);
            let fw = inner / cnt as f64;
            let owns_even = side.rank() < n.rank();
            for k in 0..cnt {
                if (k % 2 == 0) == owns_even { continue; }
                let a = (t + fw * k as f64 - b.clearance / 4.0).max(t * 0.0);
                let z = (t + fw * (k + 1) as f64 + b.clearance / 4.0).min(len);
                notches.push(match e { 0 => rect_path(a, 0.0, z, t), 1 => rect_path(w - t, a, w, z), 2 => rect_path(a, h - t, z, h), _ => rect_path(0.0, a, t, z) });
            }
        }
        let base = vec![rect_path(0.0, 0.0, w, h)];
        let cut = if notches.is_empty() { base } else {
            // all the notches run the same way round, so where they overlap (clearance) they add up rather than cancel
            clip::boolean(BoolKind::Difference, &base, &notches)
        };
        if cut.len() != 1 { return Err("A plate came out in pieces: try a different finger width.".into()); }
        let pts = cut[0].iter().map(|q| [q[0] as f64 / S, q[1] as f64 / S]).collect();
        // slots for the tabs of the dividers
        let mut holes = Vec::new();
        let c = b.clearance / 2.0;
        let zs = even_spans(2.0 * t, b.h - 2.0 * t, b.finger);
        let x_foot: Vec<(f64, f64)> = xs_list.iter().map(|x| (x - 2.0 * c, x + t + 2.0 * c)).collect();
        for &xs in &xs_list {
            match side {
                Side::Front | Side::Back => for (z0, z1) in &zs { holes.push(rect_pts(xs - c, z0 - c, xs + t + c, z1 + c)); },
                Side::Bottom => for (y0, y1) in even_spans(2.0 * t, b.d - 2.0 * t, b.finger) { holes.push(rect_pts(xs - c, y0 - c, xs + t + c, y1 + c)); },
                _ => {}
            }
        }
        for &ys in &ys_list {
            match side {
                Side::Left | Side::Right => for (z0, z1) in &zs { holes.push(rect_pts(ys - c, z0 - c, ys + t + c, z1 + c)); },
                // the tabs of these dividers stop short of the ones they cross
                Side::Bottom => for (x0, x1) in spans_minus(even_spans(2.0 * t, b.w - 2.0 * t, b.finger), &x_foot) { holes.push(rect_pts(x0 - c, ys - c, x1 + c, ys + t + c)); },
                _ => {}
            }
        }
        out.push(Panel { side, w, h, outline: pts, holes, pos: None });
    }
    // the dividers: a body between the plates it meets, with tabs into their slots, and half-lap slots where they cross
    let hd = b.h - t;
    let zmid = t + (hd - t) / 2.0;
    let c = b.clearance / 2.0;
    let empty: Vec<Path> = Vec::new();
    for (split_w, la, here, across) in [(true, b.d, &xs_list, &ys_list), (false, b.w, &ys_list, &xs_list)] {
        let tabs = if split_w { even_spans(2.0 * t, la - 2.0 * t, b.finger) } else {
            let foot: Vec<(f64, f64)> = across.iter().map(|x| (x - 2.0 * c, x + t + 2.0 * c)).collect();
            spans_minus(even_spans(2.0 * t, la - 2.0 * t, b.finger), &foot)
        };
        let mut parts: Vec<Path> = vec![rect_path(t, t, la - t, hd)];
        for (y0, y1) in tabs { parts.push(rect_path(y0, 0.0, y1, t)); }
        for (z0, z1) in even_spans(2.0 * t, b.h - 2.0 * t, b.finger) { parts.push(rect_path(0.0, z0, t, z1)); parts.push(rect_path(la - t, z0, la, z1)); }
        let mut body = clip::boolean(BoolKind::Union, &parts, &empty);
        if !across.is_empty() {
            // the dividers splitting the width keep the lower half where they cross, the others the upper half
            let slots: Vec<Path> = across.iter().map(|&a| if split_w { rect_path(a - c, zmid - c / 2.0, a + t + c, hd + 1.0) } else { rect_path(a - c, t - 1.0, a + t + c, zmid + c / 2.0) }).collect();
            body = clip::boolean(BoolKind::Difference, &body, &slots);
        }
        if body.len() != 1 { return Err("A divider came out in pieces: try a different finger width.".into()); }
        let pts: Vec<[f64; 2]> = body[0].iter().map(|q| [q[0] as f64 / S, q[1] as f64 / S]).collect();
        for &st in here.iter() { out.push(Panel { side: Side::Divider, w: la, h: hd, outline: pts.clone(), holes: Vec::new(), pos: Some((split_w, st)) }); }
    }
    Ok(out)
}

/// Places plates in rows no wider than `max_w`, `gap` apart: the lower left corner of each, relative to the first row's start.
pub fn layout(panels: &[Panel], gap: f64, max_w: f64) -> Vec<[f64; 2]> {
    let (mut x, mut y, mut row_h) = (0.0, 0.0, 0.0_f64);
    let mut out = Vec::new();
    for p in panels {
        if x > 0.0 && x + p.w > max_w { x = 0.0; y += row_h + gap; row_h = 0.0; }
        out.push([x, y]);
        x += p.w + gap;
        row_h = row_h.max(p.h);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::{new_rect, bbox};

    fn area_mm2(ls: &[LoopData]) -> f64 {
        // signed sum: outers add, holes subtract (the result is oriented as a region)
        ls.iter().map(|l| { let p: Path = l.pts.chunks_exact(2).map(|c| [c[0], c[1]]).collect(); clip::area(&p) }).sum::<f64>().abs() / (S * S)
    }

    #[test]
    fn a_square_pocket_gets_four_reliefs_that_reach_the_corners() {
        let sq = new_rect([10.0, 10.0], [30.0, 30.0], 0.0, 0);
        let (out, n) = dogbones(&[&sq], 3.0, Cut::Inside, 0).unwrap();
        assert_eq!(n, 4);
        assert_eq!(out.len(), 1);
        // each relief adds a little to the area, and the shape stays inside a box a cutter radius larger on each side
        let a = area_mm2(&out);
        assert!(a > 400.0 + 4.0 * 0.3 && a < 400.0 + 4.0 * 5.0, "{a}");
        let b = bbox(&out[0]).unwrap();
        assert!(b[0] < 10.0 && b[0] > 10.0 - 1.5 && b[3] > 30.0 && b[3] < 30.0 + 1.5, "{b:?}");
        // the cutter centre goes to 1.5 mm from the corner: the relief circle of radius 1.5 there passes through the corner
        let pts: Vec<[f64; 2]> = out[0].pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect();
        let c = [10.0 + 1.5 * 0.5_f64.sqrt(), 10.0 + 1.5 * 0.5_f64.sqrt()];
        // some vertex of the result lies a radius from that centre and beyond the walls
        assert!(pts.iter().any(|p| ((p[0] - c[0]).hypot(p[1] - c[1]) - 1.5).abs() < 0.05 && p[0] < 10.0), "overcut into the wall");
    }

    #[test]
    fn a_part_with_a_notch_gets_reliefs_only_in_the_notch_corners_when_cut_around() {
        // a 40 x 30 plate with a 10 x 5 notch in the top edge
        let plate = crate::shapes::new_poly(&[[0.0, 0.0], [40.0, 0.0], [40.0, 30.0], [25.0, 30.0], [25.0, 25.0], [15.0, 25.0], [15.0, 30.0], [0.0, 30.0]], true, 0).unwrap();
        let (out, n) = dogbones(&[&plate], 3.0, Cut::Outside, 0).unwrap();
        assert_eq!(n, 2, "the two corners at the bottom of the notch");
        // material is taken out of the plate
        assert!(area_mm2(&out) < 40.0 * 30.0 - 50.0 && area_mm2(&out) > 40.0 * 30.0 - 50.0 - 4.0 * 3.0, "{}", area_mm2(&out));
        // cut inside (a pocket of this shape) the same corners are the outer ones: six corners of the outline are convex
        let (_, m) = dogbones(&[&plate], 3.0, Cut::Inside, 0).unwrap();
        assert_eq!(m, 6);
    }

    #[test]
    fn holes_inside_a_pocket_count_from_the_region_not_the_shape() {
        let outer = new_rect([0.0, 0.0], [40.0, 40.0], 0.0, 0);
        let island = new_rect([15.0, 15.0], [25.0, 25.0], 0.0, 0);
        // an island in a pocket: its corners point into the pocket, so only the four outer corners are inside corners
        let (out, n) = dogbones(&[&outer, &island], 3.0, Cut::Inside, 0).unwrap();
        assert_eq!(n, 4);
        assert_eq!(out.len(), 2, "the pocket and the island");
        // a hole in a part, cut around: the hole's corners are the part's reflex corners
        let (_, m) = dogbones(&[&outer, &island], 3.0, Cut::Outside, 0).unwrap();
        assert_eq!(m, 4);
    }

    #[test]
    fn round_shapes_obtuse_corners_and_bad_sizes_get_nothing() {
        let circle = crate::shapes::new_ellipse([10.0, 10.0], [30.0, 30.0], 0);
        assert!(dogbones(&[&circle], 3.0, Cut::Inside, 0).is_none());
        // a hexagon's 120 degree corners still need a relief for a nut to go in; the 150 degree corners of a 12-sided shape do not
        let hex = crate::shapes::new_polygon([0.0, 0.0], 10.0, 0.0, 6, None, 0).unwrap();
        assert_eq!(dogbones(&[&hex], 3.0, Cut::Inside, 0).unwrap().1, 6);
        let dodecagon = crate::shapes::new_polygon([0.0, 0.0], 10.0, 0.0, 12, None, 0).unwrap();
        assert!(dogbones(&[&dodecagon], 3.0, Cut::Inside, 0).is_none());
        let sq = new_rect([0.0, 0.0], [10.0, 10.0], 0.0, 0);
        assert!(dogbones(&[&sq], 0.0, Cut::Inside, 0).is_none());
        assert!(dogbones(&[], 3.0, Cut::Inside, 0).is_none());
        let line = crate::shapes::new_poly(&[[0.0, 0.0], [5.0, 5.0]], false, 0).unwrap();
        assert!(dogbones(&[&line], 3.0, Cut::Inside, 0).is_none());
    }

    #[test]
    fn finger_counts_are_odd_and_near_the_wanted_width() {
        assert_eq!(finger_count(100.0, 10.0), 11);
        assert_eq!(finger_count(100.0, 12.0), 9);
        assert_eq!(finger_count(100.0, 9.0), 11);
        assert_eq!(finger_count(5.0, 10.0), 3);
        for len in [20.0, 37.5, 80.0, 123.4] { for w in [3.0, 6.0, 10.0] { assert_eq!(finger_count(len, w) % 2, 1); } }
    }

    fn poly(pts: &[[f64; 2]]) -> Path { pts.iter().map(|p| [(p[0] * S).round() as i64, (p[1] * S).round() as i64]).collect() }

    #[test]
    fn tabs_fit_the_slots_of_the_plate_they_meet_with_the_clearance_to_spare() {
        let f = Fingers { width: 10.0, depth: 3.0, clearance: 0.1 };
        // plate A (above the line y = 0) has tabs on its bottom edge, standing out below it; plate B (below the line) has slots in its top edge
        let a = finger_outline([0.0, 0.0, 90.0, 40.0], [Edge::Tabs, Edge::Flat, Edge::Flat, Edge::Flat], f).unwrap();
        let b = finger_outline([0.0, -40.0, 90.0, 0.0], [Edge::Flat, Edge::Flat, Edge::Slots, Edge::Flat], f).unwrap();
        // 9 fingers: 5 tabs, 5 slots
        assert_eq!(a.len(), 4 + 5 * 4 - 2, "{}", a.len());
        let (pa, pb) = (poly(&a), poly(&b));
        let ra = clip::simplify_even_odd(std::slice::from_ref(&pa));
        let rb = clip::simplify_even_odd(std::slice::from_ref(&pb));
        let both = clip::boolean(BoolKind::Intersect, &ra, &rb);
        let overlap: f64 = both.iter().map(|p| clip::area(p)).sum::<f64>().abs() / (S * S);
        assert!(overlap < 1e-6, "the plates overlap by {overlap} mm2");
        // what is left of the slots round the tabs is the clearance: 0.05 mm each side of a tab (end slots touch one wall only at the corners)
        let area = |p: &Path| clip::area(p).abs() / (S * S);
        let (aa, ab) = (area(&pa), area(&pb));
        assert!((aa - (90.0 * 40.0 + 5.0 * 10.0 * 3.0)).abs() < 1e-3, "{aa}");
        assert!((ab - (90.0 * 40.0 - (5.0 * 10.0 * 3.0 + 3.0 * (4.0 * 0.1) + 2.0 * 0.05 * 3.0 * 0.0 + 0.0))).abs() < 1.0, "{ab}");
        let gap = 90.0 * 40.0 - ab - 5.0 * 10.0 * 3.0;
        assert!(gap > 0.0 && gap < 5.0 * 0.1 * 3.0 + 1e-6, "clearance area {gap}");
    }

    #[test]
    fn a_plate_with_all_four_edges_cut_is_a_simple_closed_outline() {
        let f = Fingers { width: 8.0, depth: 3.0, clearance: 0.0 };
        let o = finger_outline([0.0, 0.0, 60.0, 40.0], [Edge::Tabs, Edge::Slots, Edge::Tabs, Edge::Slots], f).unwrap();
        let p = poly(&o);
        let r = clip::simplify_even_odd(std::slice::from_ref(&p));
        assert_eq!(r.len(), 1, "the outline crosses itself");
        assert!(clip::area(&p) > 0.0, "counter-clockwise");
        // too thin to cut
        assert!(finger_outline([0.0, 0.0, 60.0, 5.0], [Edge::Tabs, Edge::Flat, Edge::Tabs, Edge::Flat], f).is_none());
        assert!(finger_outline([0.0, 0.0, 60.0, 40.0], [Edge::Flat; 4], f).unwrap().len() == 4);
    }

    fn inside(poly: &[[f64; 2]], x: f64, y: f64) -> bool {
        let mut c = false;
        let mut j = poly.len() - 1;
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[j]);
            if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] { c = !c; }
            j = i;
        }
        c
    }

    /// Every cell of the box's walls is filled by exactly one plate, and no cell elsewhere is.
    fn check_box(b: BoxSpec) {
        let ps = box_panels(b).unwrap();
        let t = b.t;
        let (xl, yl) = (divider_positions(b.w, b.div_w, t), divider_positions(b.d, b.div_d, t));
        let wall = |x: f64, y: f64, z: f64| {
            let in_x = x < t || x > b.w - t; let in_y = y < t || y > b.d - t;
            let bottom = z < t; let top = b.lid && z > b.h - t;
            let div = z > t && z < b.h - t && (xl.iter().any(|&d| x > d && x < d + t && y > t && y < b.d - t) || yl.iter().any(|&d| y > d && y < d + t && x > t && x < b.w - t));
            in_x || in_y || bottom || top || div
        };
        let (mut bad, mut cells) = (0, 0);
        let step = 1.0;
        let (nx, ny, nz) = ((b.w / step) as usize, (b.d / step) as usize, (b.h / step) as usize);
        for i in 0..nx { for j in 0..ny { for k in 0..nz {
            let (x, y, z) = ((i as f64 + 0.5) * step, (j as f64 + 0.5) * step, (k as f64 + 0.5) * step);
            let hit = |p: &Panel, u: f64, v: f64| inside(&p.outline, u, v) && !p.holes.iter().any(|h| inside(h, u, v));
            let n = ps.iter().filter(|p| match p.side {
                Side::Front => y < t && hit(p, x, z),
                Side::Back => y > b.d - t && hit(p, x, z),
                Side::Left => x < t && hit(p, y, z),
                Side::Right => x > b.w - t && hit(p, y, z),
                Side::Bottom => z < t && hit(p, x, y),
                Side::Top => z > b.h - t && hit(p, x, y),
                Side::Divider => match p.pos { Some((true, d)) => x > d && x < d + t && hit(p, y, z), Some((false, d)) => y > d && y < d + t && hit(p, x, z), None => false },
            }).count();
            let want = if wall(x, y, z) { 1 } else { 0 };
            let n = if n > 0 && !wall(x, y, z) { 99 } else { n };
            cells += 1;
            if n != want { bad += 1; }
        }}}
        assert_eq!(bad, 0, "{bad} of {cells} cells are overlapped or empty");
    }

    #[test]
    fn the_plates_of_a_closed_box_fill_its_walls_exactly() {
        check_box(BoxSpec { w: 96.0, d: 56.0, h: 36.0, t: 3.0, finger: 10.0, clearance: 0.0, lid: true, div_w: 0, div_d: 0 });
    }

    #[test]
    fn the_plates_of_an_open_box_fill_its_walls_exactly() {
        check_box(BoxSpec { w: 96.0, d: 56.0, h: 33.0, t: 3.0, finger: 10.0, clearance: 0.0, lid: false, div_w: 0, div_d: 0 });
        // sizes that do not divide evenly into fingers still have no overlap or gap
        check_box(BoxSpec { w: 100.0, d: 61.0, h: 40.0, t: 3.0, finger: 9.0, clearance: 0.0, lid: true, div_w: 0, div_d: 0 });
    }

    #[test]
    fn a_box_too_small_for_its_material_is_refused() {
        assert!(box_panels(BoxSpec { w: 10.0, d: 56.0, h: 30.0, t: 3.0, finger: 10.0, clearance: 0.0, lid: true, div_w: 0, div_d: 0 }).is_err());
        assert!(box_panels(BoxSpec { w: 50.0, d: 50.0, h: 30.0, t: 0.0, finger: 10.0, clearance: 0.0, lid: true, div_w: 0, div_d: 0 }).is_err());
    }

    #[test]
    fn layout_wraps_rows() {
        let ps = box_panels(BoxSpec { w: 60.0, d: 40.0, h: 30.0, t: 3.0, finger: 10.0, clearance: 0.1, lid: true, div_w: 0, div_d: 0 }).unwrap();
        assert_eq!(ps.len(), 6);
        let pos = layout(&ps, 5.0, 150.0);
        assert!(pos.iter().all(|p| p[0] + 1.0 > 0.0));
        assert!(pos.iter().any(|p| p[1] > 0.0), "narrow stock needs a second row");
        let one = layout(&ps, 5.0, 10_000.0);
        assert!(one.iter().all(|p| p[1] == 0.0));
    }

    #[test]
    fn a_tbone_goes_into_the_longer_wall_only() {
        let r = new_rect([10.0, 10.0], [40.0, 30.0], 0.0, 0);
        let (out, n) = reliefs(&[&r], 3.0, Cut::Inside, 0, Relief::TBone).unwrap();
        assert_eq!(n, 4);
        let b = bbox(&out[0]).unwrap();
        // the long walls (bottom and top) are cut into by about a radius; the short ones are not
        assert!((b[0] - 10.0).abs() < 0.05 && (b[2] - 40.0).abs() < 0.05, "{b:?}");
        assert!((b[1] - (10.0 - 1.53)).abs() < 0.02 && (b[3] - (30.0 + 1.53)).abs() < 0.02, "{b:?}");
        // a hexagon's corners are not right angles: it gets dogbones instead
        let hex = crate::shapes::new_polygon([0.0, 0.0], 10.0, 0.0, 6, None, 0).unwrap();
        assert_eq!(reliefs(&[&hex], 3.0, Cut::Inside, 0, Relief::TBone).unwrap().1, 6);
    }

    #[test]
    fn dividers_sit_in_slots_and_fill_their_place_exactly() {
        // two dividers splitting the width, with a lid; and one splitting the depth, open
        check_box(BoxSpec { w: 99.0, d: 66.0, h: 36.0, t: 3.0, finger: 10.0, clearance: 0.0, lid: true, div_w: 1, div_d: 0 });
        check_box(BoxSpec { w: 99.0, d: 63.0, h: 39.0, t: 3.0, finger: 10.0, clearance: 0.0, lid: false, div_w: 0, div_d: 1 });
        let ps = box_panels(BoxSpec { w: 120.0, d: 80.0, h: 40.0, t: 3.0, finger: 10.0, clearance: 0.05, lid: false, div_w: 2, div_d: 0 }).unwrap();
        assert_eq!(ps.iter().filter(|p| p.side == Side::Divider).count(), 2);
        assert!(ps.iter().find(|p| p.side == Side::Front).unwrap().holes.len() >= 2);
        // too crowded or too low
        assert!(box_panels(BoxSpec { w: 40.0, d: 80.0, h: 40.0, t: 3.0, finger: 10.0, clearance: 0.0, lid: false, div_w: 4, div_d: 0 }).is_err());
        assert!(box_panels(BoxSpec { w: 120.0, d: 80.0, h: 15.0, t: 3.0, finger: 10.0, clearance: 0.0, lid: false, div_w: 1, div_d: 0 }).is_err());
    }

    #[test]
    fn dividers_in_both_directions_cross_in_half_laps() {
        // one each way, closed and open; two by one, with clearance zero so every cell can be checked
        check_box(BoxSpec { w: 99.0, d: 63.0, h: 36.0, t: 3.0, finger: 10.0, clearance: 0.0, lid: true, div_w: 1, div_d: 1 });
        check_box(BoxSpec { w: 129.0, d: 63.0, h: 39.0, t: 3.0, finger: 10.0, clearance: 0.0, lid: false, div_w: 2, div_d: 1 });
        // with clearance the plates still come out whole
        let ps = box_panels(BoxSpec { w: 120.0, d: 80.0, h: 40.0, t: 3.0, finger: 10.0, clearance: 0.05, lid: false, div_w: 2, div_d: 2 }).unwrap();
        assert_eq!(ps.iter().filter(|p| p.side == Side::Divider).count(), 4);
        // a crossing divider has a slot cut in from one edge: its outline is not just a rectangle with tabs
        let (a, b) = (ps.iter().find(|p| p.pos.is_some_and(|q| q.0)).unwrap(), ps.iter().find(|p| p.pos.is_some_and(|q| !q.0)).unwrap());
        assert!(a.outline.len() > 20 && b.outline.len() > 20);
    }
}
