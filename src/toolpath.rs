//! Toolpath and G-code generation. This is a direct port of the browser app's `build()`
//! so the two produce the same output for the same project.
use crate::clip::{self, Path};
use crate::model::*;

const S: f64 = 1000.0;

#[derive(Debug, Clone)]
pub struct Move {
    pub from: [f64; 3],
    pub to: [f64; 3],
    pub cut: bool,
    /// Minutes, including the machine's time correction.
    pub dt: f64,
    /// 1-based pass number inside the operation (0 = approach).
    pub pass: usize,
    pub op: usize,
    /// Made with the operation's second tool (inlay operations use a V-bit and a flat end mill).
    pub alt: bool,
    /// The flat lap that levels the floor after the ramped passes: part of the last pass, not a pass of its own.
    pub lap: bool,
}

/// Where the generated toolpath leaves a holding tab uncut: the middle of the tab on the toolpath
/// and the direction of the path there. Lets the drawing show the tabs where they really are.
#[derive(Debug, Clone)]
pub struct TabMark {
    /// Index of the operation.
    pub op: usize,
    pub mid: [f64; 2],
    pub dir: [f64; 2],
}

#[derive(Debug, Clone)]
pub struct Output {
    pub lines: Vec<String>,
    pub moves: Vec<Move>,
    pub time_min: f64,
    pub cut_time_min: f64,
    pub cut_len_mm: f64,
    pub notes: Vec<String>,
    pub tab_marks: Vec<TabMark>,
}

impl Output {
    pub fn gcode(&self) -> String { self.lines.join("\n") }
}

// ---------- number formatting identical to JavaScript's +n.toFixed(d) and String(n) ----------
pub fn to_fixed(v: f64, dec: usize) -> f64 {
    if !v.is_finite() { return v; }
    let neg = v < 0.0;
    let s = format!("{:.40}", v.abs());
    let (ip, fp) = s.split_once('.').unwrap();
    let mut digits = String::from(ip);
    digits.push_str(&fp[..dec]);
    let next = fp.as_bytes()[dec];
    let mut n: u128 = digits.parse().unwrap();
    if next >= b'5' { n += 1; }
    let r = n as f64 / 10f64.powi(dec as i32);
    if neg { -r } else { r }
}
pub fn num(v: f64) -> String { if v == 0.0 { "0".into() } else { format!("{}", v) } }

// ---------- tool geometry ----------
#[derive(Debug, Clone, Copy)]
pub struct ToolGeo {
    pub r: f64, pub shape: ShapeKind, pub cone: bool, pub tn: f64,
    /// Depth at which the cutting edge reaches the full diameter (V-bits, tapers, ball noses).
    pub z_full: f64,
    /// Tip radius of a cone-shaped tool (0 = sharp).
    pub rt: f64,
    /// Arc profile bit: radius of the arc that forms the cutting edge, and whether it is concave.
    pub arc: f64,
    pub concave: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind { Flat, Ball, Bull, VBit, Engrave, Taper, Arc }

impl ShapeKind {
    pub fn parse(s: &str) -> Self {
        match s { "ball" => Self::Ball, "bull" => Self::Bull, "vbit" => Self::VBit, "engrave" => Self::Engrave, "taper" => Self::Taper, "arc" => Self::Arc, _ => Self::Flat }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Flat => "Flat end mill", Self::Ball => "Ball nose", Self::Bull => "Bull nose", Self::VBit => "V-bit", Self::Engrave => "Engraving bit", Self::Taper => "Tapered ball nose", Self::Arc => "Arc profile bit" }
    }
}

impl ToolGeo {
    pub fn new(t: &Tool) -> Self {
        let r = t.dia / 2.0;
        let shape = ShapeKind::parse(&t.shape);
        let cone = matches!(shape, ShapeKind::VBit | ShapeKind::Engrave | ShapeKind::Taper);
        let ang = if t.angle != 0.0 { t.angle } else { 60.0 };
        let half = ang.clamp(10.0, 150.0) * std::f64::consts::PI / 360.0;
        let tn = half.tan();
        // the tip sphere has to fit inside the diameter
        let rt = if cone && t.tip.is_finite() { t.tip.clamp(0.0, r / half.cos()) } else { 0.0 };
        // the arc cannot be tighter than the bit is wide
        // a concave arc cannot be tighter than the bit is wide; a convex one smaller than the bit radius leaves a flat bottom
        let arc = if shape == ShapeKind::Arc {
            let a = if t.arc.is_finite() { t.arc } else { r };
            if t.concave { a.max(r) } else { a.max(0.05) }
        } else { 0.0 };
        let mut g = ToolGeo { r, shape, cone, tn, z_full: 0.0, rt, arc, concave: shape == ShapeKind::Arc && t.concave };
        g.z_full = if cone || matches!(shape, ShapeKind::Ball | ShapeKind::Arc) { g.rise(r) } else { 0.0 };
        g
    }
    /// Height of the cutting edge above the tip at distance `rho` from the axis (infinity beyond the tool).
    /// A cone with a tip radius is a cone ending in a sphere that touches it.
    pub fn rise(&self, rho: f64) -> f64 {
        if rho > self.r + 1e-12 { return f64::INFINITY; }
        let rho = rho.max(0.0);
        if self.cone {
            if self.rt <= 0.0 { return rho / self.tn; }
            let (sin_a, cos_a) = { let a = self.tn.atan(); (a.sin(), a.cos()) };
            let (rho_t, z_t) = (self.rt * cos_a, self.rt * (1.0 - sin_a));
            if rho <= rho_t { return self.rt - (self.rt * self.rt - rho * rho).max(0.0).sqrt(); }
            return z_t + (rho - rho_t) / self.tn;
        }
        if self.shape == ShapeKind::Ball { return self.r - (self.r * self.r - rho * rho).max(0.0).sqrt(); }
        if self.shape == ShapeKind::Arc {
            let rs = self.arc;
            let sag = |u: f64| rs - (rs * rs - u * u).max(0.0).sqrt();
            // convex: part of a sphere; concave: the same arc turned over, flaring out towards the rim
            if self.concave { return sag(self.r) - sag(self.r - rho); }
            // flat bottom of radius r - rs when the arc is tighter than the bit
            let flat = (self.r - rs).max(0.0);
            return sag((rho - flat).max(0.0));
        }
        0.0
    }
    /// Radius of the cutting edge at the surface when the tip is `d` below it.
    pub fn r_at(&self, d: f64) -> f64 {
        let d = d.max(0.0);
        if self.cone {
            if self.rt <= 0.0 { return self.r.min(d * self.tn); }
            let (sin_a, cos_a) = { let a = self.tn.atan(); (a.sin(), a.cos()) };
            let (rho_t, z_t) = (self.rt * cos_a, self.rt * (1.0 - sin_a));
            let r = if d <= z_t { (2.0 * self.rt * d - d * d).max(0.0).sqrt() } else { rho_t + (d - z_t) * self.tn };
            return r.min(self.r);
        }
        if self.shape == ShapeKind::Ball { return if d >= self.r { self.r } else { (2.0 * self.r * d - d * d).max(0.0).sqrt() }; }
        if self.shape == ShapeKind::Arc {
            // the profile only rises, so find where it reaches height d
            if d >= self.rise(self.r) { return self.r; }
            let (mut lo, mut hi) = (0.0, self.r);
            for _ in 0..50 { let mid = (lo + hi) / 2.0; if self.rise(mid) < d { lo = mid; } else { hi = mid; } }
            return (lo + hi) / 2.0;
        }
        self.r
    }
}

// ---------- emitter ----------
struct Em {
    g: Vec<String>, cur: [f64; 3], lf: Option<f64>, time: f64, tc: f64, pn: usize, oi: usize, lap: bool,
    moves: Vec<Move>, alt: bool, marks: Vec<TabMark>, lead_skipped: usize, ox: f64, oy: f64, zt: f64, ku: f64, imp: bool, rap: f64, tsc: f64,
}

impl Em {
    fn dec(&self) -> usize { if self.imp { 4 } else { 3 } }
    fn go(&mut self, c: &str, x: Option<f64>, y: Option<f64>, z: Option<f64>, fd: f64) {
        let x = x.unwrap_or(self.cur[0]);
        let y = y.unwrap_or(self.cur[1]);
        let z = z.unwrap_or(self.cur[2]);
        let mut s = c.to_string();
        if x != self.cur[0] { s += &format!(" X{}", num(to_fixed((x - self.ox) / self.ku, self.dec()))); }
        if y != self.cur[1] { s += &format!(" Y{}", num(to_fixed((y - self.oy) / self.ku, self.dec()))); }
        if z != self.cur[2] { s += &format!(" Z{}", num(to_fixed((z + self.zt) / self.ku, self.dec()))); }
        let is_cut = c == "G1";
        if is_cut && Some(fd) != self.lf {
            s += &format!(" F{}", num(to_fixed(fd / self.ku, if self.imp { 2 } else { 1 })));
            self.lf = Some(fd);
        }
        if s == c { return; }
        self.g.push(s);
        let dist = ((x - self.cur[0]).powi(2) + (y - self.cur[1]).powi(2) + (z - self.cur[2]).powi(2)).sqrt();
        let dt = dist / (if is_cut { fd } else { self.rap }) * self.tsc;
        self.moves.push(Move { from: self.cur, to: [x, y, z], cut: is_cut, dt, pass: self.pn, op: self.oi, alt: self.alt, lap: self.lap });
        self.time += dt;
        if is_cut { self.tc += dt; }
        self.cur = [x, y, z];
    }
}

// ---------- geometry helpers ----------
/// Offset without cleaning. Repeated offsets feed this uncleaned result into the next step, which keeps
/// cleaning error from adding up and avoids Clipper's very slow case of one huge inward offset that collapses a shape.
fn off_raw(region: &[Path], d: f64) -> Vec<Path> { clip::offset(region, d, S * 0.01) }
fn clean_paths(o: &[Path]) -> Vec<Path> { clip::clean(o, S * 0.02).into_iter().filter(|p| p.len() > 2).collect() }
fn off(region: &[Path], d: f64) -> Vec<Path> { clean_paths(&off_raw(region, d)) }

struct Job { p: Path, closed: bool, pocket: bool, iv: Option<Option<Vec<[f64; 2]>>> }

struct OpCtx<'a> {
    kind: &'a str, dia: f64, so: f64, plunge: f64, feed: f64, ramp: bool, rev: bool,
    z0: f64, sz: f64, zs: Vec<f64>,
    tabs_on: bool, ztab: f64, tab_w: f64, tab_n: f64,
    /// Lead-in / lead-out arc radius in mm (0 = none).
    lead: f64,
}

fn mk_jobs(c: &OpCtx, region: &[Path], closed: &[Path], open: &[Path], rm: f64) -> Vec<Job> {
    let mut jb = Vec::new();
    let r = rm * S;
    match c.kind {
        "online" => {
            for p in closed { jb.push(Job { p: p.clone(), closed: true, pocket: false, iv: None }); }
            for p in open { jb.push(Job { p: p.clone(), closed: false, pocket: false, iv: None }); }
        }
        "pocket" => {
            let st2 = c.dia * c.so / 100.0 * S;
            let mut cur = off_raw(region, -r);
            for _ in 0..500 {
                if cur.is_empty() { break; }
                let q = clean_paths(&cur);
                if q.is_empty() { break; }
                for p in q { jb.push(Job { p, closed: true, pocket: true, iv: None }); }
                cur = off_raw(&cur, -st2);
            }
        }
        _ => {
            let mut q = off(region, if c.kind == "outside" { r } else { -r });
            // Holes first, then ascending size, so a part is never freed before its holes are cut.
            q.sort_by(|a, b| {
                let (aa, ab) = (clip::area(a), clip::area(b));
                let (ka, kb) = (if aa < 0.0 { 0 } else { 1 }, if ab < 0.0 { 0 } else { 1 });
                ka.cmp(&kb).then(aa.abs().partial_cmp(&ab.abs()).unwrap_or(std::cmp::Ordering::Equal))
            });
            if c.kind == "inside" {
                q.sort_by(|a, b| clip::area(a).abs().partial_cmp(&clip::area(b).abs()).unwrap_or(std::cmp::Ordering::Equal));
            }
            for p in q { jb.push(Job { p, closed: true, pocket: false, iv: None }); }
        }
    }
    jb
}

fn path_f(p: &Path, rev: bool, closed: bool) -> Vec<[f64; 2]> {
    let mut v: Vec<[f64; 2]> = p.iter().map(|q| [q[0] as f64 / S, q[1] as f64 / S]).collect();
    if rev && closed { v.reverse(); }
    if closed { let a = v[0]; v.push(a); }
    v
}

fn tab_iv(e: &mut Em, c: &OpCtx, job: &mut Job, pts: &[[f64; 2]]) -> Option<Vec<[f64; 2]>> {
    if let Some(iv) = &job.iv { return iv.clone(); }
    let mut lp = 0.0;
    for i in 1..pts.len() { lp += (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]); }
    let len = c.tab_w + c.dia;
    let n = (c.tab_n.round().max(1.0)).min((lp / (2.2 * len)).floor());
    if n < 1.0 { job.iv = Some(None); return None; }
    let mut iv = Vec::new();
    for q in 0..(n as usize) { let c0 = (q as f64 + 0.5) * lp / n; iv.push([c0 - len / 2.0, c0 + len / 2.0]); }
    for [a, b] in &iv { if let Some((mid, dir)) = point_at(pts, (a + b) / 2.0) { e.marks.push(TabMark { op: e.oi, mid, dir }); } }
    job.iv = Some(Some(iv.clone()));
    Some(iv)
}

/// Point and direction at distance `s` along a polyline.
fn point_at(pts: &[[f64; 2]], s: f64) -> Option<([f64; 2], [f64; 2])> {
    let mut cum = 0.0;
    for i in 1..pts.len() {
        let (a, b) = (pts[i - 1], pts[i]);
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        if l > 0.0 && cum + l >= s - 1e-9 {
            let u = ((s - cum) / l).clamp(0.0, 1.0);
            return Some(([a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u], [(b[0] - a[0]) / l, (b[1] - a[1]) / l]));
        }
        cum += l;
    }
    None
}

fn tab_pass(e: &mut Em, c: &OpCtx, pts: &[[f64; 2]], z: f64, iv: &[[f64; 2]]) {
    let mut cum = vec![0.0];
    for i in 1..pts.len() { let l = cum[i - 1] + (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]); cum.push(l); }
    let mut bps: Vec<(f64, bool)> = Vec::new();
    for [a, b] in iv { bps.push((*a, true)); bps.push((*b, false)); }
    let (mut bi, mut cur) = (0usize, false);
    e.go("G1", None, None, Some(c.z0 + z), c.plunge);
    for i in 1..pts.len() {
        while bi < bps.len() && bps[bi].0 <= cum[i] + 1e-9 {
            let (s, t) = bps[bi]; bi += 1;
            let d = cum[i] - cum[i - 1];
            let u = if d != 0.0 { (s - cum[i - 1]) / d } else { 0.0 };
            let x = pts[i - 1][0] + (pts[i][0] - pts[i - 1][0]) * u;
            let y = pts[i - 1][1] + (pts[i][1] - pts[i - 1][1]) * u;
            e.go("G1", Some(x), Some(y), Some(c.z0 + if cur { c.ztab } else { z }), c.feed);
            cur = t;
            e.go("G1", None, None, Some(c.z0 + if cur { c.ztab } else { z }), c.plunge);
        }
        e.go("G1", Some(pts[i][0]), Some(pts[i][1]), Some(c.z0 + if cur { c.ztab } else { z }), c.feed);
    }
}

/// Manual tabs: put each tab on the closest closed toolpath and turn it into a length interval along that path.
fn assign_manual_tabs(c: &OpCtx, jobs: &mut [Job], tabs: &[[f64; 2]], notes: &mut Vec<String>, pre: &str) {
    let polys: Vec<Vec<[f64; 2]>> = jobs.iter().map(|j| path_f(&j.p, c.rev, j.closed)).collect();
    let mut per_job: Vec<Vec<f64>> = vec![vec![]; jobs.len()];
    let mut skipped = 0;
    for t in tabs {
        let mut best: Option<(usize, f64, f64)> = None; // job, distance, position along the path
        for (ji, pts) in polys.iter().enumerate() {
            if !jobs[ji].closed { continue; }
            let mut cum = 0.0;
            for i in 1..pts.len() {
                let (a, b) = (pts[i - 1], pts[i]);
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l = dx.hypot(dy);
                if l > 0.0 {
                    let u = (((t[0] - a[0]) * dx + (t[1] - a[1]) * dy) / (l * l)).clamp(0.0, 1.0);
                    let d = (t[0] - a[0] - u * dx).hypot(t[1] - a[1] - u * dy);
                    if best.is_none_or(|bb| d < bb.1) { best = Some((ji, d, cum + u * l)); }
                }
                cum += l;
            }
        }
        match best { Some((ji, d, s)) if d <= c.dia + 1.0 => per_job[ji].push(s), _ => skipped += 1 }
    }
    for (ji, ss) in per_job.into_iter().enumerate() {
        if ss.is_empty() { jobs[ji].iv = Some(None); continue; }
        let pts = &polys[ji];
        let lp: f64 = (1..pts.len()).map(|i| (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1])).sum();
        let len = (c.tab_w + c.dia).min(lp * 0.9);
        let mut iv: Vec<[f64; 2]> = Vec::new();
        for s in ss {
            let (a, b) = (s - len / 2.0, s + len / 2.0);
            if a < 0.0 { iv.push([0.0, b]); iv.push([lp + a, lp]); }
            else if b > lp { iv.push([a, lp]); iv.push([0.0, b - lp]); }
            else { iv.push([a, b]); }
        }
        iv.sort_by(|p, q| p[0].partial_cmp(&q[0]).unwrap());
        let mut merged: Vec<[f64; 2]> = Vec::new();
        for v in iv { match merged.last_mut() { Some(m) if v[0] <= m[1] => m[1] = m[1].max(v[1]), _ => merged.push(v) } }
        jobs[ji].iv = Some(Some(merged));
    }
    if skipped > 0 { notes.push(format!("{}{} manual tab{} not on the outline, skipped.", pre, skipped, if skipped == 1 { " is" } else { "s are" })); }
}

/// Even-odd test of a point (mm) against a set of closed paths.
pub(crate) fn inside_eo(paths: &[Path], x: f64, y: f64) -> bool {
    let (xi, yi) = (x * S, y * S);
    let mut c = false;
    for p in paths {
        let n = p.len();
        if n < 3 { continue; }
        let mut j = n - 1;
        for i in 0..n {
            let (a, b) = ([p[i][0] as f64, p[i][1] as f64], [p[j][0] as f64, p[j][1] as f64]);
            if (a[1] > yi) != (b[1] > yi) && xi < a[0] + (yi - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) { c = !c; }
            j = i;
        }
    }
    c
}

/// A tangent arc to enter the cut and one to leave it, on the waste side of the toolpath.
struct Lead { s: [f64; 2], arc_in: Vec<[f64; 2]>, arc_out: Vec<[f64; 2]> }

/// `guard` is the set of toolpath outlines of the operation: the tool centre is on the waste side of them
/// where the even-odd test gives `waste_inside`. The radius shrinks until the arcs fit; None if even a small one does not.
fn make_lead(pts: &[[f64; 2]], guard: &[Path], waste_inside: bool, r_max: f64) -> Option<Lead> {
    let n = pts.len();
    if n < 4 { return None; }
    let p0 = pts[0];
    let dir = |a: [f64; 2], b: [f64; 2]| -> Option<[f64; 2]> {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy);
        if l < 1e-6 { None } else { Some([dx / l, dy / l]) }
    };
    let mut t_out = (1..n).find_map(|i| dir(p0, pts[i]))?;
    let mut t_in = (0..n - 1).rev().find_map(|i| dir(pts[i], p0))?;
    // on a smooth curve (a polygon with short edges) both arcs follow the true tangent; at a real corner each follows its own edge
    if t_in[0] * t_out[0] + t_in[1] * t_out[1] > 0.9 {
        let m = [t_in[0] + t_out[0], t_in[1] + t_out[1]];
        let l = m[0].hypot(m[1]);
        t_out = [m[0] / l, m[1] / l];
        t_in = t_out;
    }
    let ok = |q: [f64; 2]| inside_eo(guard, q[0], q[1]) == waste_inside;
    // the normal that points into the waste
    let side = |t: [f64; 2]| -> [f64; 2] {
        let l = [-t[1], t[0]];
        if ok([p0[0] + l[0] * 0.2, p0[1] + l[1] * 0.2]) { l } else { [-l[0], -l[1]] }
    };
    let (n_in, n_out) = (side(t_out), side(t_in));
    let mut r = r_max;
    while r >= 0.3 {
        let c_in = [p0[0] + n_in[0] * r, p0[1] + n_in[1] * r];
        let c_out = [p0[0] + n_out[0] * r, p0[1] + n_out[1] * r];
        let steps = 12;
        let mut arc_in = Vec::new();
        let mut arc_out = Vec::new();
        for i in 0..=steps {
            let a = std::f64::consts::FRAC_PI_2 * i as f64 / steps as f64;
            let (ca, sa) = (a.cos(), a.sin());
            arc_in.push([c_in[0] - t_out[0] * r * ca - n_in[0] * r * sa, c_in[1] - t_out[1] * r * ca - n_in[1] * r * sa]);
            arc_out.push([c_out[0] - n_out[0] * r * ca + t_in[0] * r * sa, c_out[1] - n_out[1] * r * ca + t_in[1] * r * sa]);
        }
        let (s, e) = (arc_in[0], arc_out[steps]);
        // right at the join the arcs hug the outline, within the polygon's own error, so those points are not judged
        let far = |q: &[f64; 2]| (q[0] - p0[0]).hypot(q[1] - p0[1]) > 0.3;
        let fits = arc_in[..steps].iter().chain(arc_out[1..].iter()).filter(|q| far(q)).all(|&q| ok(q))
            && (1..4).all(|i| { let u = i as f64 / 4.0; ok([e[0] + (s[0] - e[0]) * u, e[1] + (s[1] - e[1]) * u]) });
        if fits {
            arc_out.remove(0);
            arc_in.remove(0);
            return Some(Lead { s, arc_in, arc_out });
        }
        r /= 2.0;
    }
    None
}

fn cut(e: &mut Em, c: &OpCtx, job: &mut Job, zl: &[f64], guard: &[Path]) {
    let pts = path_f(&job.p, c.rev, job.closed);
    let lead = if c.lead > 0.0 && job.closed && !job.pocket {
        let l = make_lead(&pts, guard, c.kind == "inside", c.lead);
        if l.is_none() { e.lead_skipped += 1; }
        l
    } else { None };
    let a = lead.as_ref().map(|l| l.s).unwrap_or(pts[0]);
    e.go("G0", None, None, Some(c.sz), 0.0);
    e.go("G0", Some(a[0]), Some(a[1]), None, 0.0);
    e.pn = 0;
    let mut pz = 0.0;
    e.go("G0", None, None, Some(c.z0 + 0.5), 0.0);
    e.go("G1", None, None, Some(c.z0), c.plunge);
    for &z in zl {
        e.pn = c.zs.iter().position(|&q| q == z).map(|i| i + 1).unwrap_or(0);
        let tiv = if c.tabs_on && job.closed && z < c.ztab - 1e-9 { tab_iv(e, c, job, &pts) } else { None };
        if let Some(l) = &lead {
            // back to the start of the arc (it is in the waste, so this is free air), down, and onto the line
            e.go("G1", Some(l.s[0]), Some(l.s[1]), Some(c.z0 + pz), c.feed);
            e.go("G1", None, None, Some(c.z0 + z), c.plunge);
            for q in &l.arc_in { e.go("G1", Some(q[0]), Some(q[1]), Some(c.z0 + z), c.feed); }
        }
        if let Some(iv) = tiv {
            tab_pass(e, c, &pts, z, &iv);
        } else if c.ramp && lead.is_none() && job.closed && !job.pocket && pts.len() > 2 {
            let mut lc = vec![0.0];
            let mut t = 0.0;
            for i in 1..pts.len() { t += (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]); lc.push(t); }
            for (i, q) in pts.iter().enumerate() { e.go("G1", Some(q[0]), Some(q[1]), Some(c.z0 + pz + (z - pz) * lc[i] / t), c.feed); }
            // the sloped lap leaves a sloped floor, but the next lap ramps down from there and cuts deeper everywhere, so
            // only the last depth needs a flat lap to level the floor: 3 depth passes are 3 ramps and one flat lap, not 6 laps
            if zl.last() == Some(&z) {
                e.lap = true;
                for q in &pts { e.go("G1", Some(q[0]), Some(q[1]), Some(c.z0 + z), c.feed); }
                e.lap = false;
            }
        } else {
            e.go("G1", None, None, Some(c.z0 + z), c.plunge);
            for q in &pts { e.go("G1", Some(q[0]), Some(q[1]), Some(c.z0 + z), c.feed); }
        }
        if let Some(l) = &lead { for q in &l.arc_out { e.go("G1", Some(q[0]), Some(q[1]), Some(c.z0 + z), c.feed); } }
        pz = z;
    }
    e.go("G0", None, None, Some(c.sz), 0.0);
}

fn zero_offsets(st: &Stock) -> (f64, f64) {
    let ox = match st.zr.as_str() { "lr" | "ur" => st.w, "c" => st.w / 2.0, _ => 0.0 };
    let oy = match st.zr.as_str() { "ul" | "ur" => st.h, "c" => st.h / 2.0, _ => 0.0 };
    (ox, oy)
}
fn zero_label(zr: &str) -> &'static str {
    match zr { "lr" => "lower-right corner", "ul" => "upper-left corner", "ur" => "upper-right corner", "c" => "center", _ => "lower-left corner" }
}

/// Prefix every line with a line number: N10, N20, ...
pub fn number_lines(lines: Vec<String>) -> Vec<String> {
    lines.into_iter().enumerate().map(|(i, l)| format!("N{} {}", (i + 1) * 10, l)).collect()
}

/// Which tool is in the spindle, and the Mach3 / LinuxCNC tool numbers (in order of first use).
#[derive(Default)]
struct ToolSt { d: Option<f64>, r: Option<f64>, t: Option<String>, ids: Vec<String> }

/// Emit what it takes to start cutting with `tool`: spindle start, or a tool change when another tool was in.
fn select_tool(e: &mut Em, st: &mut ToolSt, mach: bool, tool: &Tool, sz: f64, dia_txt: &str, un: &str) {
    if !st.ids.contains(&tool.id) { st.ids.push(tool.id.clone()); }
    let tno = st.ids.iter().position(|i| *i == tool.id).unwrap_or(0) + 1;
    // Mach3 and some LinuxCNC setups choke on anything but plain ASCII inside a comment
    let tname: String = tool.name.replace(['″', '"'], "in").replace(['(', ')'], "-").chars().filter(|c| c.is_ascii() && !c.is_ascii_control()).collect();
    let m6 = format!("T{} M6 (Tool {}: {})", tno, tno, tname);
    let rpm = tool.rpm;
    match st.d {
        None => {
            if mach { e.g.push(m6); }
            e.g.push(format!("M3 S{}", num(rpm))); e.g.push("G4 P2".into());
        }
        Some(ld) if if mach { st.t.as_deref() != Some(tool.id.as_str()) } else { ld != tool.dia } => {
            e.go("G0", None, None, Some(sz), 0.0);
            e.g.push("M5".into());
            if mach { e.g.push(m6); }
            else { e.g.push(format!("M0 (Change tool to {} {}, then resume)", dia_txt, un)); }
            e.g.push(format!("M3 S{}", num(rpm)));
            e.g.push("G4 P2".into());
        }
        Some(_) if st.r != Some(rpm) => e.g.push(format!("M3 S{}", num(rpm))),
        _ => {}
    }
    st.d = Some(tool.dia); st.r = Some(rpm); st.t = Some(tool.id.clone()); e.lf = None;
}

/// Trace closed rings with a V-bit, each at its own depth (the tip's Z).
fn emit_rings(e: &mut Em, rings: &[(Path, f64)], rev: bool, z0: f64, sz: f64, hop: f64, plunge: f64, feed: f64) {
    for (idx, (p, z)) in rings.iter().enumerate() {
        let pts = path_f(p, rev, true);
        let a = pts[0];
        e.pn = idx + 1;
        e.go("G0", None, None, Some(if idx > 0 { z0 + hop } else { sz }), 0.0);
        e.go("G0", Some(a[0]), Some(a[1]), None, 0.0);
        e.go("G1", None, None, Some(z0 + z), plunge);
        for q in pts { e.go("G1", Some(q[0]), Some(q[1]), Some(z0 + z), feed); }
    }
    e.go("G0", None, None, Some(sz), 0.0);
}

/// Stock left for the finishing pass after roughing a relief, mm.
const RELIEF_LEAVE: f64 = 0.3;

/// A 3D relief: the roughing tool clears the bulk in layers, the finishing tool follows the surface.
/// How deep a relief operation cuts: what was typed in, or else the model's height at its current size
/// (never more than the stock is thick).
pub fn relief_depth(data: &ProjectData, p: &crate::model::OpParams) -> f64 {
    match (&data.relief, p.d_man) {
        (Some(r), false) => ((r.zmax * r.scale).max(0.1)).min(data.stock.t),
        _ => p.depth,
    }
}

fn relief_op(e: &mut Em, ts: &mut ToolSt, notes: &mut Vec<String>, data: &ProjectData, o: &Operation, k: usize, mach: bool, sz_init: f64) -> Result<(), String> {
    use crate::relief::{finish_lines, Field, Footprint, Rough};
    let pre = format!("Op {}: ", k + 1);
    let un = if e.imp { "in" } else { "mm" };
    let ku = e.ku;
    let Some(rel) = &data.relief else { return Err(format!("Operation {}: import a 3D model first (File menu, Import 3D Model)", k + 1)); };
    let depth = relief_depth(data, &o.p);
    let tool_opt = data.tools.iter().find(|t| t.id == o.p.tool);
    let rough = if o.p.r_off { None } else { o.p.tool2.as_deref().and_then(|id| data.tools.iter().find(|t| t.id == id)) };
    if o.p.r_off && o.p.f_off { return Err(format!("Operation {}: roughing and finishing are both switched off, so there is nothing to cut", k + 1)); }
    if !(depth > 0.0) { return Err(format!("Operation {}: check the depth", k + 1)); }
    // the finishing tool is only needed when finishing; without it the roughing tool sets the margins
    let tool = match (tool_opt, o.p.f_off) {
        (Some(t), _) => t,
        (None, true) => rough.ok_or_else(|| format!("Operation {}: choose a roughing tool", k + 1))?,
        (None, false) => return Err(format!("Operation {}: choose a finishing tool (Manage… to create one)", k + 1)),
    };
    if !o.p.f_off && !(tool.dia > 0.0 && tool.so > 0.0 && tool.feed > 0.0 && tool.plunge > 0.0) { return Err(format!("Operation {}: check the finishing tool's values", k + 1)); }
    let leave = o.p.r_leave.unwrap_or(RELIEF_LEAVE).clamp(0.0, 5.0);
    let z0 = if o.p.air || data.job.air { 10.0 } else { 0.0 };
    let sz = data.job.safe + z0;
    let hop = data.job.safe.min(1.0);
    let st = &data.stock;
    if depth > st.t + 1e-6 { notes.push(format!("⚠ {}depth exceeds stock thickness.", pre)); }
    let (mw, mh) = rel.size();
    if rel.x < -1e-6 || rel.y < -1e-6 || rel.x + mw > st.w + 1e-6 || rel.y + mh > st.h + 1e-6 {
        notes.push(format!("⚠ {}the model ({:.0} x {:.0} mm) extends beyond the stock.", pre, mw, mh));
    }
    let fgeo = ToolGeo::new(tool);
    let rmax = tool.dia / 2.0 + 0.2;
    let rmax = rough.map_or(rmax, |r| rmax.max(r.dia / 2.0 + 0.2));
    let f = Field::new(rel, depth, rmax + 1.0);
    let g = f.g;
    let dec = if e.imp { 4 } else { 3 };
    let fnum = move |v: f64| num(to_fixed(v / ku, dec));

    // 1. roughing with the second tool
    let mut layers = 0usize;
    if let Some(rt) = rough {
        if !(rt.dia > 0.0 && rt.so > 0.0 && rt.feed > 0.0 && rt.plunge > 0.0 && rt.pd > 0.0) { return Err(format!("Operation {}: check the roughing tool's values", k + 1)); }
        let rg = ToolGeo::new(rt);
        let kc = ((rt.dia / 6.0 / g).round() as usize).max(1);
        let margin = (((rt.dia / 2.0 + 0.2) / g).ceil() as usize).min(f.pad);
        let rp = Rough::new(&f, kc, rt.dia / 2.0, leave as f32, |rho| rg.rise(rho), margin);
        let range = -(rp.min_t as f64);
        if range > 1e-6 {
            let pdr = o.p.pass_depth.filter(|v| *v > 0.0).unwrap_or(rt.pd);
            let n = ((range / pdr) - 1e-9).ceil().max(1.0) as usize;
            let dz = range / n as f64;
            let rstep = ((rt.dia * o.p.r_so.filter(|v| *v > 0.0).unwrap_or(rt.so) / 100.0 / rp.gc).round() as usize).max(1);
            select_tool(e, ts, mach, rt, sz, &fnum(rt.dia), un);
            e.alt = true;
            e.g.push(format!("(Operation {}: relief, roughing in {} layers, {} {} {}, leaving {} {})", k + 1, n, rg.shape.label(), fnum(rt.dia), un, fnum(leave), un));
            let mut first = true;
            for li in 0..n {
                let z = -dz * (li as f64 + 1.0);
                e.pn = li + 1;
                // rows, zigzag: left to right, then right to left
                let mut segs: Vec<([f64; 2], [f64; 2])> = Vec::new();
                let mut ltr = true;
                let mut cj = 0;
                while cj < rp.ny {
                    let runs = rp.runs(cj, z as f32);
                    let y = rp.y(cj);
                    let mut row: Vec<([f64; 2], [f64; 2])> = runs.iter().map(|&(a, b)| ([rp.x(a), y], [rp.x(b), y])).collect();
                    if !ltr { row.reverse(); row = row.into_iter().map(|(a, b)| (b, a)).collect(); }
                    if !row.is_empty() { ltr = !ltr; }
                    segs.extend(row);
                    cj += rstep;
                }
                let mut prev: Option<[f64; 2]> = None;
                for (a, b) in segs {
                    let linked = prev.is_some_and(|p| (p[0] - a[0]).hypot(p[1] - a[1]) < 3.0 * rt.dia && rp.open_line(p, a, z as f32));
                    if linked {
                        e.go("G1", Some(a[0]), Some(a[1]), Some(z0 + z), rt.feed);
                    } else {
                        e.go("G0", None, None, Some(if first { sz } else { z0 + hop }), 0.0);
                        e.go("G0", Some(a[0]), Some(a[1]), None, 0.0);
                        e.go("G0", None, None, Some(z0 + (z + dz + 0.3).min(hop)), 0.0);
                        e.go("G1", None, None, Some(z0 + z), rt.plunge);
                    }
                    first = false;
                    e.go("G1", Some(b[0]), Some(b[1]), Some(z0 + z), rt.feed);
                    prev = Some(b);
                }
            }
            e.go("G0", None, None, Some(sz), 0.0);
            e.alt = false;
            layers = n;
        } else {
            notes.push(format!("{}nothing to rough: the model is flat.", pre));
        }
    } else if !o.p.r_off {
        notes.push(format!("{}no roughing tool chosen: the finishing tool cuts everything (slow, and hard on the tool). Pick a flat end mill as the roughing tool.", pre));
    }

    if !o.p.f_off {
        // 2. finishing with the main tool, lines along X following the surface
        let fp = Footprint::new(g, tool.dia / 2.0, |rho| fgeo.rise(rho));
        let fso = o.p.f_so.filter(|v| *v > 0.0).unwrap_or(tool.so);
        let step = ((tool.dia * fso / 100.0 / g).round() as usize).max(1);
        if step as f64 * g > tool.dia * fso / 100.0 * 1.6 { notes.push(format!("{}the model's resolution ({} {}) limits the finishing line spacing; re-import it finer for a smoother surface.", pre, fnum(g), un)); }
        let margin = (((tool.dia / 2.0 + 0.2) / g).ceil() as usize).min(f.pad);
        let lines = finish_lines(&f, &fp, margin, step, 0.01);
        select_tool(e, ts, mach, tool, sz, &fnum(tool.dia), un);
        e.g.push(format!("(Operation {}: relief, finishing, {} {} {}, lines {} {} apart)", k + 1, fgeo.shape.label(), fnum(tool.dia), un, fnum(step as f64 * g), un));
        for (li, line) in lines.iter().enumerate() {
            let pts: Vec<[f64; 3]> = if li % 2 == 1 { line.iter().rev().copied().collect() } else { line.clone() };
            e.pn = layers + li + 1;
            let a = pts[0];
            if li == 0 {
                e.go("G0", None, None, Some(sz), 0.0);
                e.go("G0", Some(a[0]), Some(a[1]), None, 0.0);
                e.go("G0", None, None, Some(z0 + hop), 0.0);
                e.go("G1", None, None, Some(z0 + a[2]), tool.plunge);
            } else {
                e.go("G1", Some(a[0]), Some(a[1]), Some(z0 + a[2]), tool.feed);
            }
            for p in &pts[1..] { e.go("G1", Some(p[0]), Some(p[1]), Some(z0 + p[2]), tool.feed); }
        }
        e.go("G0", None, None, Some(sz), 0.0);
        if tool.shape == "flat" { notes.push(format!("{}a flat end mill as the finishing tool leaves a stepped surface; use a ball nose.", pre)); }
    }
    let _ = sz_init;
    Ok(())
}

pub fn generate(data: &ProjectData, units: Units) -> Result<Output, String> {
    let ku = units.factor();
    let un = units.name();
    let imp = units == Units::Imperial;
    let fl = |v: f64| to_fixed(v / ku, if imp { 4 } else { 3 });
    let st = &data.stock;
    let machine = data.machine.clone().unwrap_or_default();
    let mut limit_notes: Vec<String> = Vec::new();
    let limited;
    let data = if machine.rpm_min.is_some() || machine.rpm_max.is_some() || machine.feed_max.is_some() || machine.plunge_max.is_some() {
        let mut d = data.clone();
        let used = serde_json::to_string(&d.ops).unwrap_or_default();
        for t in d.tools.iter_mut() {
            if !used.contains(&format!("\"{}\"", t.id)) { continue; }
            let (r0, f0, p0) = (t.rpm, t.feed, t.plunge);
            let h = machine.hold(t);
            let mut what: Vec<String> = vec![];
            if h.rpm != r0 { what.push(format!("spindle {} rpm (tool: {})", num(h.rpm), num(r0))); }
            if h.feed != f0 { what.push(format!("feed {} {}/min (tool: {})", num(fl(h.feed)), un, num(fl(f0)))); }
            if h.plunge != p0 { what.push(format!("plunge {} {}/min (tool: {})", num(fl(h.plunge)), un, num(fl(p0)))); }
            if !what.is_empty() { limit_notes.push(format!("⚠ {}: held to the machine's limits, {}.", if t.name.is_empty() { "Tool" } else { &t.name }, what.join(", "))); }
        }
        for t in d.tools.iter_mut() { *t = machine.hold(t); }
        limited = d;
        &limited
    } else { data };
    // air cut is per operation; an old project-level flag still applies to every operation
    let sz_init = data.job.safe + if data.job.air { 10.0 } else { 0.0 };
    let (ox, oy) = zero_offsets(st);
    let zt = if st.zm == "top" { 0.0 } else { st.t - st.zh };
    let mut e = Em { g: vec![], cur: [0.0, 0.0, sz_init], lf: None, time: 0.0, tc: 0.0, pn: 0, oi: 0, lap: false, moves: vec![], alt: false, marks: vec![], lead_skipped: 0, ox, oy, zt, ku, imp, rap: machine.rapid, tsc: machine.tsc };
    let mut notes: Vec<String> = limit_notes;

    if data.loops.iter().any(|l| l.pts.chunks_exact(2).any(|p| (p[0] as f64) < 0.0 || (p[1] as f64) < 0.0 || (p[0] as f64) > st.w * S || (p[1] as f64) > st.h * S)) {
        notes.push("⚠ Shapes extend beyond the stock.".into());
    }
    if data.loops.iter().any(|l| !data.ops.iter().any(|o| o.id == l.op)) { notes.push("Some shapes have no operation (not machined).".into()); }
    if st.w > machine.x + 1e-6 || st.h > machine.y + 1e-6 || st.t > machine.z + 1e-6 {
        notes.push(format!("⚠ The stock is larger than the machine work area ({} × {} × {} {}).", num(fl(machine.x)), num(fl(machine.y)), num(fl(machine.z)), un));
    }
    let mach = data.job.mach();
    e.g.push(format!("(PutiaCNC - {})", if mach { "Mach3 / LinuxCNC" } else { "GRBL" }));
    e.g.push(format!("(Stock {} x {} x {} {}; XY0 = {} of stock, Z0 = {})", num(fl(st.w)), num(fl(st.h)), num(fl(st.t)), un, zero_label(&st.zr),
        if st.zm == "top" { "top surface".to_string() } else { format!("stock bottom + {} {}", num(fl(st.zh)), un) }));
    if mach {
        // metric or imperial, absolute, XY plane, feed per minute; cancel cutter compensation, tool length and canned cycles
        e.g.push(format!("{} G90 G17 G94", if imp { "G20" } else { "G21" }));
        e.g.push("G40 G49 G80".into());
        e.g.push("G54".into());
    } else {
        e.g.push(format!("{} G90 G17", if imp { "G20" } else { "G21" }));
    }

    let custom = |s: &Option<String>| -> Vec<String> { s.as_deref().unwrap_or("").lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect() };
    let (start_code, end_code) = (custom(&machine.start_code), custom(&machine.end_code));
    // the machine's own start lines go in last, below, so arc fitting never sees them; remember where
    let start_at = e.g.len();

    let mut ts = ToolSt::default();
    let mut any = false;

    for (k, o) in data.ops.iter().enumerate() {
        e.oi = k;
        let pre = format!("Op {}: ", k + 1);
        if o.p.kind == "relief" {
            relief_op(&mut e, &mut ts, &mut notes, data, o, k, mach, sz_init)?;
            any = true;
            continue;
        }
        let mut mine: Vec<&LoopData> = data.loops.iter().filter(|l| l.op == o.id).collect();
        if o.p.kind != "drill" {
            let n = mine.len();
            mine.retain(|l| !crate::shapes::is_point(l));
            if mine.len() < n { notes.push(format!("{}{} drill point(s) skipped: only Drill operations use points.", pre, n - mine.len())); }
        }
        if mine.is_empty() { notes.push(format!("Op {} has no shapes.", k + 1)); continue; }
        let tool = data.tools.iter().find(|t| t.id == o.p.tool)
            .ok_or_else(|| format!("Operation {}: choose a tool (Manage… to create one)", k + 1))?;
        let (dia, so, pd, feed, plunge) = (tool.dia, tool.so, tool.pd, tool.feed, tool.plunge);
        let depth = o.p.depth;
        let z0 = if o.p.air || data.job.air { 10.0 } else { 0.0 };
        let sz = data.job.safe + z0;
        if !(dia > 0.0 && depth > 0.0 && pd > 0.0 && so > 0.0 && feed > 0.0 && plunge > 0.0) {
            return Err(format!("Operation {}: check tool, depth and feed values", k + 1));
        }
        let kind = o.p.kind.as_str();
        let geo = ToolGeo::new(tool);
        let rr = dia / 2.0;
        let closed: Vec<Path> = mine.iter().filter(|l| l.closed() && l.pts.len() / 2 > 2).map(|l| l.path()).collect();
        let open: Vec<Path> = mine.iter().filter(|l| !l.closed()).map(|l| l.path()).collect();
        if depth > st.t { notes.push(format!("⚠ {}depth exceeds stock thickness.", pre)); }
        let region = clip::simplify_even_odd(&closed);
        if kind != "online" && kind != "drill" && !open.is_empty() { notes.push(format!("{}{} open path(s) skipped.", pre, open.len())); }

        let pd = o.p.pass_depth.filter(|v| *v > 0.0).unwrap_or(pd);
        let nz = (depth / pd - 1e-9).ceil() as usize;
        let zs: Vec<f64> = (0..nz).map(|i| -(depth / nz as f64) * (i as f64 + 1.0)).collect();
        let tab_h = o.p.tab_h.unwrap_or(2.0);
        let tabs_req = o.p.tabs && (kind == "outside" || kind == "inside");
        let tabs_on = tabs_req && tab_h > 0.0 && tab_h < depth - 1e-6;
        let mut ctx = OpCtx { kind, dia, so, plunge, feed, ramp: o.p.ramp, rev: false, z0, sz, zs: zs.clone(),
            tabs_on, ztab: -(depth - tab_h), tab_w: o.p.tab_w.unwrap_or(5.0), tab_n: o.p.tab_n.unwrap_or(4.0),
            lead: if kind == "outside" || kind == "inside" { o.p.lead.filter(|r| *r > 0.0).unwrap_or(0.0) } else { 0.0 } };

        if kind == "inlay" || kind == "plug" {
            let female = kind == "inlay";
            let label = if female { "inlay pocket" } else { "inlay plug" };
            if !geo.cone { return Err(format!("Operation {}: an inlay needs a V-bit, engraving or tapered tool; choose one in Manage…", k + 1)); }
            let Some(ft) = o.p.tool2.as_deref().and_then(|id| data.tools.iter().find(|t| t.id == id)) else {
                return Err(format!("Operation {}: choose the flat end mill that clears the pocket and cuts the plug out", k + 1));
            };
            if ToolGeo::new(ft).cone { return Err(format!("Operation {}: the clearing tool of an inlay must be a flat end mill", k + 1)); }
            if !(ft.dia > 0.0 && ft.pd > 0.0 && ft.so > 0.0 && ft.feed > 0.0 && ft.plunge > 0.0) {
                return Err(format!("Operation {}: check the clearing tool's values", k + 1));
            }
            if region.is_empty() { notes.push(format!("{}no closed shapes to cut.", pre)); continue; }
            if geo.rt > 1e-9 { notes.push(format!("{}the inlay assumes a sharp V-bit tip; with a tip radius the plug will not fit exactly.", pre)); }
            let tn = geo.tn;
            let pdepth = depth.min(geo.z_full);
            if depth > geo.z_full + 1e-6 { notes.push(format!("{}depth limited to {} {} (the V-bit diameter is reached).", pre, num(fl(pdepth)), un)); }
            // how far the walls slope sideways over the whole depth
            let total = pdepth * tn;
            let fit = o.p.fit.unwrap_or(0.05).max(0.0);
            let rf = ft.dia / 2.0;
            let hop = data.job.safe.min(1.0);
            let sv = (dia * so / 100.0 * 0.25).min(0.3).max(0.05);
            let fpd = o.p.pass_depth.filter(|v| *v > 0.0).unwrap_or(ft.pd);
            let conv = o.p.dir == "conv";
            let mkzs = |d: f64| -> Vec<f64> { let n = ((d / fpd) - 1e-9).ceil().max(1.0) as usize; (0..n).map(|i| -(d / n as f64) * (i as f64 + 1.0)).collect() };
            let fctx = |kind: &'static str, zs: Vec<f64>, rev: bool| OpCtx {
                kind, dia: ft.dia, so: ft.so, plunge: ft.plunge, feed: ft.feed, ramp: o.p.ramp, rev, z0, sz, zs,
                tabs_on: false, ztab: 0.0, tab_w: 0.0, tab_n: 0.0, lead: 0.0,
            };
            let ring_rev = !conv;
            let fdia = num(fl(ft.dia));
            let vtxt = format!("{} deg V-bit {} {}", num(if tool.angle != 0.0 { tool.angle } else { 60.0 }), num(fl(dia)), un);
            let ftxt = format!("flat {} {}", fdia, un);
            // V-bit rings: tip depth and the sideways position of the tool go together along the slope
            let mut rings: Vec<(Path, f64)> = Vec::new();
            if female {
                // the tool at offset d inside the outline has its tip d / tn deep, so the wall slopes at the cone's angle
                let mut cur = region.clone();
                let mut d = 0.0;
                loop {
                    let next = (d + sv).min(total);
                    if next - d < 1e-9 { break; }
                    cur = off_raw(&cur, -(next - d) * S);
                    d = next;
                    let q = clean_paths(&cur);
                    if q.is_empty() { break; }
                    let z = (d / tn).min(pdepth);
                    for p in q { rings.push((p, -z)); }
                    if d >= total - 1e-9 { break; }
                }
            } else {
                // plug: the V-bit runs along the wall of the boss, deepest at the base of the boss
                let base = if fit > 0.0 { off(&region, -fit * S) } else { region.clone() };
                let mut cur = base;
                let mut d = 0.0;
                loop {
                    let z = pdepth - d / tn;
                    if z < 0.02 || cur.is_empty() { break; }
                    let q = clean_paths(&cur);
                    if q.is_empty() { break; }
                    for p in q { rings.push((p, -z)); }
                    let step = sv.min(total - d);
                    if step < 1e-6 { break; }
                    cur = off_raw(&cur, -step * S);
                    d += step;
                }
                // shallow rings first
                rings.reverse();
            }
            if rings.is_empty() { notes.push(format!("{}no V-bit toolpath: the shapes are too thin for this depth.", pre)); }

            if female {
                // 1. flat end mill clears the floor, up to where the walls begin
                let floor = off(&region, -total * S);
                let zsf = mkzs(pdepth);
                let c = fctx("pocket", zsf.clone(), !conv);
                let mut jobs = mk_jobs(&c, &floor, &[], &[], rf);
                if jobs.is_empty() {
                    notes.push(format!("{}the floor is too narrow for the flat end mill; the V-bit cuts all of it.", pre));
                } else {
                    select_tool(&mut e, &mut ts, mach, ft, sz, &fdia, un);
                    e.alt = true;
                    e.g.push(format!("(Operation {}: {}, clear the floor, {})", k + 1, label, ftxt));
                    for &z in &zsf { for j in jobs.iter_mut() { cut(&mut e, &c, j, &[z], &[]); } }
                    e.alt = false;
                }
                // 2. V-bit cuts the sloped walls
                select_tool(&mut e, &mut ts, mach, tool, sz, &num(fl(dia)), un);
                e.g.push(format!("(Operation {}: {}, sloped walls, {})", k + 1, label, vtxt));
                emit_rings(&mut e, &rings, ring_rev, z0, sz, hop, plunge, feed);
            } else {
                // 1. V-bit carves the sloped sides of the plug
                select_tool(&mut e, &mut ts, mach, tool, sz, &num(fl(dia)), un);
                e.g.push(format!("(Operation {}: {}, sloped sides, {})", k + 1, label, vtxt));
                emit_rings(&mut e, &rings, ring_rev, z0, sz, hop, plunge, feed);
                // 2. flat end mill cuts the plug out, through the stock
                let base = if fit > 0.0 { off(&region, -fit * S) } else { region.clone() };
                let zso = mkzs(st.t);
                let c = fctx("outside", zso.clone(), conv);
                let mut jobs = mk_jobs(&c, &base, &[], &[], rf);
                if jobs.is_empty() { notes.push(format!("{}nothing to cut out.", pre)); }
                else {
                    select_tool(&mut e, &mut ts, mach, ft, sz, &fdia, un);
                    e.alt = true;
                    e.g.push(format!("(Operation {}: {}, cut out through the stock, {})", k + 1, label, ftxt));
                    let guard: Vec<Path> = jobs.iter().map(|j| j.p.clone()).collect();
                    for j in jobs.iter_mut() { cut(&mut e, &c, j, &zso, &guard); }
                    e.alt = false;
                    notes.push(format!("{}the plug is cut free of the stock: hold it down, and it is sanded flush after gluing.", pre));
                }
            }
            any = true;
            continue;
        }

        let mut jobs: Vec<Job> = Vec::new();
        let mut vr: Vec<(Path, f64)> = Vec::new();
        let mut by_pass = false;
        let mut holes: Vec<[f64; 2]> = Vec::new();
        if kind == "drill" {
            let (mut wide, mut odd) = (0, 0);
            for l in mine.iter().filter(|l| l.closed() && l.pts.len() / 2 > 2) {
                let Some(b) = crate::shapes::bbox(l) else { continue };
                holes.push([(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0]);
                if (b[2] - b[0]).max(b[3] - b[1]) > dia * 1.1 + 1e-6 { wide += 1; }
                if !crate::shapes::is_circle(l) && !crate::shapes::is_point(l) { odd += 1; }
            }
            if wide > 0 { notes.push(format!("{}{} hole(s) are wider than the tool, so they will come out the tool's size (use Pocket or Profile inside to cut them to size).", pre, wide)); }
            if odd > 0 { notes.push(format!("{}{} shape(s) are not circles; drilled at their centre.", pre, odd)); }
            if !open.is_empty() { notes.push(format!("{}{} open path(s) skipped.", pre, open.len())); }
            // visit the holes in a short order: always the nearest one next, starting from where the tool is
            let mut from = [e.cur[0], e.cur[1]];
            let mut left = std::mem::take(&mut holes);
            while !left.is_empty() {
                let k = (0..left.len()).min_by(|&a, &b| {
                    let (da, db) = ((left[a][0] - from[0]).hypot(left[a][1] - from[1]), (left[b][0] - from[0]).hypot(left[b][1] - from[1]));
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                }).unwrap();
                from = left.remove(k);
                holes.push(from);
            }
        } else if kind == "vcarve" {
            if !(geo.cone || matches!(geo.shape, ShapeKind::Ball | ShapeKind::Arc)) { return Err(format!("Operation {}: V-carve needs a V-bit, engraving, tapered, ball nose or arc profile tool (choose one in Manage…)", k + 1)); }
            let zmax = depth.min(geo.z_full);
            if depth > geo.z_full + 1e-6 { notes.push(format!("{}V-carve depth limited to {} {} (the tool diameter is reached).", pre, num(fl(zmax)), un)); }
            let sv = (dia * so / 100.0 * 0.25).min(0.3).max(0.05);
            let mut cur = region.clone();
            let mut d = 0.0;
            for _ in 1..3000 {
                let next = off_raw(&cur, -sv * S);
                let q = clean_paths(&next);
                if q.is_empty() { break; }
                cur = next;
                d += sv;
                let z = -geo.rise(d).min(zmax);
                for p in q { vr.push((p, z)); }
            }
            // the last full step overshoots the centre line: finish with smaller steps so that narrow parts
            // are carved right to their middle, not left with a ridge
            let mut step = sv;
            for _ in 0..40 {
                step /= 2.0;
                if step < 0.01 { break; }
                let next = off_raw(&cur, -step * S);
                let q = clean_paths(&next);
                if q.is_empty() { continue; }
                cur = next;
                d += step;
                let z = -geo.rise(d).min(zmax);
                for p in q { vr.push((p, z)); }
                step *= 2.0; // it fitted: try the same step again
            }
        } else if geo.cone && kind != "online" {
            by_pass = true;
            notes.push(format!("{}{}: walls are sloped, with the top edge on the line.", pre, geo.shape.label()));
            if kind == "pocket" { notes.push(format!("{}a pocket cut with a cone tool has a ridged floor (try V-carve or a flat tool).", pre)); }
        } else {
            let rm = if matches!(geo.shape, ShapeKind::Ball | ShapeKind::Arc) && depth < geo.z_full { geo.r_at(depth) } else { rr };
            if rm < rr - 1e-6 && kind != "online" { notes.push(format!("{}depth is below the ball radius, so the cut is narrower than the tool ({} {}).", pre, num(fl(rm * 2.0)), un)); }
            jobs = mk_jobs(&ctx, &region, &closed, &open, rm);
        }
        if if kind == "drill" { holes.is_empty() } else if kind == "vcarve" { vr.is_empty() } else { !by_pass && jobs.is_empty() } {
            notes.push(format!("{}no toolpath (tool too large?).", pre));
            continue;
        }
        if tabs_req && !tabs_on { notes.push(format!("{}tabs skipped: the tab height must be smaller than the cut depth.", pre)); }
        ctx.rev = (kind == "inside" || kind == "pocket" || kind == "vcarve") != (o.p.dir == "conv");
        if tabs_on {
            if let Some(tp) = &o.p.tab_pts {
                let pts: Vec<[f64; 2]> = tp.iter().filter_map(|t| crate::shapes::tab_resolve(&data.loops, o.id, t).map(|r| r.0)).collect();
                assign_manual_tabs(&ctx, &mut jobs, &pts, &mut notes, &pre);
            }
        }

        select_tool(&mut e, &mut ts, mach, tool, sz, &num(fl(dia)), un);
        e.g.push(format!("(Operation {}: {}, tool {} {}{})", k + 1, kind, num(fl(dia)), un,
            if geo.cone { format!(", {} deg", num(if tool.angle != 0.0 { tool.angle } else { 60.0 })) } else { String::new() }));

        if kind == "drill" {
            let hop = data.job.safe.min(1.0);
            for (idx, h) in holes.iter().enumerate() {
                e.pn = 0;
                e.go("G0", None, None, Some(if idx > 0 { z0 + hop } else { sz }), 0.0);
                e.go("G0", Some(h[0]), Some(h[1]), None, 0.0);
                e.go("G0", None, None, Some(z0 + 1.0), 0.0);
                for (i, &z) in zs.iter().enumerate() {
                    e.pn = i + 1;
                    e.go("G1", None, None, Some(z0 + z), plunge);
                    if i + 1 < zs.len() {
                        // peck: out of the hole to clear the chips, then back down to just above the bottom
                        e.go("G0", None, None, Some(z0 + 1.0), 0.0);
                        e.go("G0", None, None, Some(z0 + z + 0.3), 0.0);
                    }
                }
                e.go("G0", None, None, Some(z0 + hop), 0.0);
            }
            e.go("G0", None, None, Some(sz), 0.0);
        } else if kind == "vcarve" {
            let hop = data.job.safe.min(1.0);
            for (idx, (p, z)) in vr.iter().enumerate() {
                let mut pts = path_f(p, ctx.rev, true);
                let a = pts[0];
                e.pn = idx + 1;
                e.go("G0", None, None, Some(if idx > 0 { z0 + hop } else { sz }), 0.0);
                e.go("G0", Some(a[0]), Some(a[1]), None, 0.0);
                e.go("G1", None, None, Some(z0 + z), plunge);
                for q in pts.drain(..) { e.go("G1", Some(q[0]), Some(q[1]), Some(z0 + z), feed); }
            }
            e.go("G0", None, None, Some(sz), 0.0);
        } else if by_pass {
            for &z in &zs {
                let mut jb = mk_jobs(&ctx, &region, &closed, &open, geo.r_at(-z));
                let guard: Vec<Path> = jb.iter().map(|j| j.p.clone()).collect();
                for j in jb.iter_mut() { cut(&mut e, &ctx, j, &[z], &guard); }
            }
        } else if kind == "pocket" {
            for &z in &zs { for j in jobs.iter_mut() { cut(&mut e, &ctx, j, &[z], &[]); } }
        } else {
            let guard: Vec<Path> = jobs.iter().map(|j| j.p.clone()).collect();
            for j in jobs.iter_mut() { cut(&mut e, &ctx, j, &zs, &guard); }
        }
        if e.lead_skipped > 0 {
            notes.push(format!("{}lead-in/out skipped on {} path(s): not enough room for the arc.", pre, e.lead_skipped));
            e.lead_skipped = 0;
        }
        any = true;
    }
    if !any { return Err(format!("No toolpath: attach closed shapes to an operation (or the tool is too large). {}", notes.join(" "))); }
    e.g.push("M5".into()); e.g.push("G0 X0 Y0".into()); e.g.push(if mach { "M30" } else { "M2" }.into());
    let cut_len: f64 = e.moves.iter().filter(|m| m.cut).map(|m| ((m.to[0] - m.from[0]).powi(2) + (m.to[1] - m.from[1]).powi(2) + (m.to[2] - m.from[2]).powi(2)).sqrt()).sum();
    if machine.arcs == Some(true) {
        let (lines, n) = crate::arcs::fit_arcs(&e.g, e.dec(), ku);
        if n > 0 { e.g = lines; notes.push(format!("{} arc{} written as G2/G3: shorter file; run an air cut first if this machine has not done arcs before.", n, if n == 1 { "" } else { "s" })); }
    }
    if !start_code.is_empty() || !end_code.is_empty() {
        let last = e.g.len().saturating_sub(1);
        for (k, l) in end_code.iter().enumerate() { e.g.insert(last + k, l.clone()); }
        for (k, l) in start_code.iter().enumerate() { e.g.insert(start_at + k, l.clone()); }
        notes.push(format!("{}: {} start line{} and {} end line{} of your own G-code added; the time estimate does not include any moves in them.", machine.name,
            start_code.len(), if start_code.len() == 1 { "" } else { "s" }, end_code.len(), if end_code.len() == 1 { "" } else { "s" }));
    }
    if data.job.numbered { e.g = number_lines(e.g); }
    Ok(Output { lines: e.g, moves: e.moves, time_min: e.time, cut_time_min: e.tc, cut_len_mm: cut_len, notes, tab_marks: e.marks })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn to_fixed_matches_javascript() {
        assert_eq!(to_fixed(0.0625, 3), 0.063); // exact tie rounds up, like JS
        assert_eq!(to_fixed(2.5, 0), 3.0);
        assert_eq!(to_fixed(-0.0005, 3), -0.001);
        assert_eq!(to_fixed(1.005, 2), 1.0); // 1.005 is really 1.00499999999999989...
        assert_eq!(to_fixed(60.3535, 3), 60.353); // the double is 60.35349999..., so JS also gives 60.353
        assert_eq!(to_fixed(8.345, 2), 8.35); // 8.3450000000000006394..., rounds up
        assert_eq!(num(to_fixed(-0.0001, 3)), "0");
        assert_eq!(num(5.0), "5");
        assert_eq!(num(0.5), "0.5");
    }
    fn drill_project(depth: f64, pass: Option<f64>) -> ProjectData {
        let mut d = crate::shapes::blank_project().data;
        d.loops.clear();
        d.loops.push(crate::shapes::new_circle([100.0, 20.0], 1.5, 1));
        d.loops.push(crate::shapes::new_circle([10.0, 10.0], 1.5, 1));
        d.loops.push(crate::shapes::new_circle([12.0, 30.0], 1.5, 1));
        d.ops.clear();
        let mut p = crate::shapes::default_op_params(&d.tools[0].id);
        p.kind = "drill".into();
        p.depth = depth;
        p.pass_depth = pass;
        d.ops.push(Operation { id: 1, p });
        d
    }

    #[test]
    fn drill_visits_each_hole_nearest_first_and_pecks() {
        let d = drill_project(6.0, Some(2.0));
        let o = generate(&d, Units::Metric).unwrap();
        // the nearest hole to the origin first, then the next closest, then the far one
        let plunges: Vec<[f64; 2]> = o.moves.iter().filter(|m| m.cut && m.to[2] < m.from[2]).map(|m| [m.to[0], m.to[1]]).collect();
        let mut order: Vec<[f64; 2]> = vec![];
        for p in plunges { if order.last() != Some(&p) { order.push(p); } }
        assert_eq!(order, vec![[10.0, 10.0], [12.0, 30.0], [100.0, 20.0]]);
        // 6 mm in 2 mm pecks = 3 plunges per hole, never any sideways cutting
        assert_eq!(o.moves.iter().filter(|m| m.cut).count(), 9);
        assert!(o.moves.iter().filter(|m| m.cut).all(|m| m.from[0] == m.to[0] && m.from[1] == m.to[1]));
        let bottom = o.moves.iter().map(|m| m.to[2]).fold(f64::MAX, f64::min);
        assert!((bottom + 6.0).abs() < 1e-9);
        assert!(o.gcode().contains("(Operation 1: drill"));
    }

    #[test]
    fn drill_points_are_drilled_where_they_are_and_other_operations_skip_them() {
        let mut d = drill_project(3.0, None);
        d.loops.clear();
        d.loops.push(crate::shapes::new_point([20.0, 20.0], 1));
        d.loops.push(crate::shapes::new_point([40.0, 25.0], 1));
        let o = generate(&d, Units::Metric).unwrap();
        let mut plunges: Vec<[f64; 2]> = o.moves.iter().filter(|m| m.cut && m.to[2] < m.from[2]).map(|m| [m.to[0], m.to[1]]).collect();
        plunges.dedup();
        assert_eq!(plunges.len(), 2);
        assert!((plunges[0][0] - 20.0).abs() < 1e-6 && (plunges[1][0] - 40.0).abs() < 1e-6);
        assert!(!o.notes.iter().any(|n| n.contains("not circles")), "{:?}", o.notes);
        // the same points under a pocket are ignored, with a note
        d.ops[0].p.kind = "pocket".into();
        let r = generate(&d, Units::Metric);
        if let Ok(o) = r { assert!(o.notes.iter().any(|n| n.contains("drill point")), "{:?}", o.notes); }
    }

    #[test]
    fn drill_in_one_go_when_the_peck_is_deep_enough() {
        let d = drill_project(3.0, Some(5.0));
        let o = generate(&d, Units::Metric).unwrap();
        assert_eq!(o.moves.iter().filter(|m| m.cut).count(), 3);
    }

    #[test]
    fn drill_warns_about_holes_wider_than_the_tool() {
        let mut d = drill_project(3.0, None);
        d.loops.push(crate::shapes::new_circle([50.0, 50.0], 10.0, 1));
        let o = generate(&d, Units::Metric).unwrap();
        assert!(o.notes.iter().any(|n| n.contains("wider than the tool")));
    }

    #[test]
    fn line_numbers_are_optional() {
        let mut d = drill_project(3.0, None);
        let plain = generate(&d, Units::Metric).unwrap();
        assert!(!plain.lines[0].starts_with('N'));
        d.job.numbered = true;
        let n = generate(&d, Units::Metric).unwrap();
        assert_eq!(n.lines.len(), plain.lines.len());
        assert_eq!(n.lines[0], format!("N10 {}", plain.lines[0]));
        assert!(n.lines[n.lines.len() - 1].starts_with(&format!("N{} M2", n.lines.len() * 10)));
        // same moves and time either way
        assert_eq!(n.moves.len(), plain.moves.len());
        // the setting survives a save and load
        let j = serde_json::to_string(&d).unwrap();
        assert!(serde_json::from_str::<ProjectData>(&j).unwrap().job.numbered);
    }

    #[test]
    fn the_machines_own_start_and_end_code_is_added() {
        let mut d = drill_project(3.0, None);
        let plain = generate(&d, Units::Metric).unwrap();
        let mut m = d.machine.clone().unwrap_or_default();
        m.start_code = Some("M3 S8000\n\n  G4 P2  \n".into());
        m.end_code = Some("G53 G0 X0 Y200".into());
        d.machine = Some(m);
        let o = generate(&d, Units::Metric).unwrap();
        assert_eq!(o.lines.len(), plain.lines.len() + 3);
        // after the setup lines, before the first tool; the end block just before the final M2
        let i = o.lines.iter().position(|l| l == "M3 S8000").unwrap();
        assert_eq!(o.lines[i + 1], "G4 P2");
        assert_eq!(&o.lines[..i], &plain.lines[..i]);
        let n = o.lines.len();
        assert_eq!(&o.lines[n - 3..], &["G0 X0 Y0".to_string(), "G53 G0 X0 Y200".to_string(), "M2".to_string()][..]);
        // the moves and the time do not change, and the notes say so
        assert_eq!(o.moves.len(), plain.moves.len());
        assert!(o.notes.iter().any(|x| x.contains("2 start lines") && x.contains("1 end line")));
        // line numbers cover them too
        d.job.numbered = true;
        let nn = generate(&d, Units::Metric).unwrap();
        assert!(nn.lines[i].ends_with("M3 S8000") && nn.lines[i].starts_with('N'));
        // an empty box adds nothing
        d.job.numbered = false;
        d.machine.as_mut().unwrap().start_code = Some("  \n".into());
        d.machine.as_mut().unwrap().end_code = None;
        assert_eq!(generate(&d, Units::Metric).unwrap().lines, plain.lines);
        // saved with the machine
        let j = serde_json::to_string(d.machine.as_ref().unwrap()).unwrap();
        assert!(!j.contains("endCode") && j.contains("startCode"));
    }

    #[test]
    fn mach_dialect_has_its_own_header_tool_changes_and_end() {
        let mut d = drill_project(3.0, None);
        // a second operation with a different tool
        let t2 = d.tools[1].id.clone();
        let mut p = crate::shapes::default_op_params(&t2);
        p.kind = "drill".into();
        p.depth = 3.0;
        d.ops.push(Operation { id: 2, p });
        d.loops.push(crate::shapes::new_circle([80.0, 40.0], 1.0, 2));
        let grbl = generate(&d, Units::Metric).unwrap();
        assert!(grbl.gcode().contains("M0 (Change tool") && grbl.lines.last().unwrap() == "M2" && !grbl.gcode().contains("M6"));

        d.job.post = "mach".into();
        let o = generate(&d, Units::Metric).unwrap();
        let g = o.gcode();
        assert!(o.lines[0].contains("Mach3"));
        assert!(g.contains("G21 G90 G17 G94") && g.contains("G40 G49 G80") && g.contains("\nG54"));
        let m6: Vec<&String> = o.lines.iter().filter(|l| l.contains("M6")).collect();
        assert_eq!(m6.len(), 2, "{m6:?}");
        assert!(m6[0].starts_with("T1 M6") && m6[1].starts_with("T2 M6"));
        assert!(m6.iter().all(|l| l.is_ascii()), "{m6:?}");
        assert!(!g.contains("M0"));
        // the spindle starts after the first tool is in
        let (i6, i3) = (o.lines.iter().position(|l| l.starts_with("T1")).unwrap(), o.lines.iter().position(|l| l.starts_with("M3")).unwrap());
        assert!(i6 < i3);
        assert_eq!(o.lines.last().unwrap(), "M30");
        // the same moves as the GRBL output
        assert_eq!(o.moves.len(), grbl.moves.len());
        let j = serde_json::to_string(&d).unwrap();
        assert!(serde_json::from_str::<ProjectData>(&j).unwrap().job.mach());
    }

    #[test]
    fn cone_radius_grows_with_depth() {
        let t = Tool { id: "t".into(), name: "v".into(), sys: "mm".into(), shape: "vbit".into(), angle: 90.0, flutes: 2, dia: 6.0, rpm: 1.0, feed: 1.0, plunge: 1.0, so: 40.0, pd: 1.0, notes: String::new(), tip: 0.0, arc: 0.0, concave: false, flute: 0.0, shank: 0.0 };
        let g = ToolGeo::new(&t);
        assert!((g.r_at(1.5) - 1.5).abs() < 1e-9);
        assert!((g.r_at(10.0) - 3.0).abs() < 1e-9);
        assert!((g.z_full - 3.0).abs() < 1e-9);
    }
}
