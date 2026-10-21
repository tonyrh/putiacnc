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
}

#[derive(Debug, Clone)]
pub struct Output {
    pub lines: Vec<String>,
    pub moves: Vec<Move>,
    pub time_min: f64,
    pub cut_time_min: f64,
    pub cut_len_mm: f64,
    pub notes: Vec<String>,
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
pub struct ToolGeo { pub r: f64, pub shape: ShapeKind, pub cone: bool, pub tn: f64, pub z_full: f64 }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind { Flat, Ball, Bull, VBit, Engrave, Taper }

impl ShapeKind {
    pub fn parse(s: &str) -> Self {
        match s { "ball" => Self::Ball, "bull" => Self::Bull, "vbit" => Self::VBit, "engrave" => Self::Engrave, "taper" => Self::Taper, _ => Self::Flat }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Flat => "Flat end mill", Self::Ball => "Ball nose", Self::Bull => "Bull nose", Self::VBit => "V-bit", Self::Engrave => "Engraving bit", Self::Taper => "Tapered ball nose" }
    }
}

impl ToolGeo {
    pub fn new(t: &Tool) -> Self {
        let r = t.dia / 2.0;
        let shape = ShapeKind::parse(&t.shape);
        let cone = matches!(shape, ShapeKind::VBit | ShapeKind::Engrave | ShapeKind::Taper);
        let ang = if t.angle != 0.0 { t.angle } else { 60.0 };
        let tn = (ang.clamp(10.0, 150.0) * std::f64::consts::PI / 360.0).tan();
        ToolGeo { r, shape, cone, tn, z_full: if cone { r / tn } else { 0.0 } }
    }
    /// Radius of the cutting edge at the surface when the tip is `d` below it.
    pub fn r_at(&self, d: f64) -> f64 {
        if self.cone { return self.r.min(d.max(0.0) * self.tn); }
        if self.shape == ShapeKind::Ball { return if d >= self.r { self.r } else { (2.0 * self.r * d - d * d).max(0.0).sqrt() }; }
        self.r
    }
}

// ---------- emitter ----------
struct Em {
    g: Vec<String>, cur: [f64; 3], lf: Option<f64>, time: f64, tc: f64, pn: usize, oi: usize,
    moves: Vec<Move>, ox: f64, oy: f64, zt: f64, ku: f64, imp: bool, rap: f64, tsc: f64,
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
        self.moves.push(Move { from: self.cur, to: [x, y, z], cut: is_cut, dt, pass: self.pn, op: self.oi });
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

fn tab_iv(c: &OpCtx, job: &mut Job, pts: &[[f64; 2]]) -> Option<Vec<[f64; 2]>> {
    if let Some(iv) = &job.iv { return iv.clone(); }
    let mut lp = 0.0;
    for i in 1..pts.len() { lp += (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]); }
    let len = c.tab_w + c.dia;
    let n = (c.tab_n.round().max(1.0)).min((lp / (2.2 * len)).floor());
    if n < 1.0 { job.iv = Some(None); return None; }
    let mut iv = Vec::new();
    for q in 0..(n as usize) { let c0 = (q as f64 + 0.5) * lp / n; iv.push([c0 - len / 2.0, c0 + len / 2.0]); }
    job.iv = Some(Some(iv.clone()));
    Some(iv)
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
                    if best.map_or(true, |bb| d < bb.1) { best = Some((ji, d, cum + u * l)); }
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

fn cut(e: &mut Em, c: &OpCtx, job: &mut Job, zl: &[f64]) {
    let pts = path_f(&job.p, c.rev, job.closed);
    let a = pts[0];
    e.go("G0", None, None, Some(c.sz), 0.0);
    e.go("G0", Some(a[0]), Some(a[1]), None, 0.0);
    e.pn = 0;
    let mut pz = 0.0;
    e.go("G0", None, None, Some(c.z0 + 0.5), 0.0);
    e.go("G1", None, None, Some(c.z0), c.plunge);
    for &z in zl {
        e.pn = c.zs.iter().position(|&q| q == z).map(|i| i + 1).unwrap_or(0);
        let tiv = if c.tabs_on && job.closed && z < c.ztab - 1e-9 { tab_iv(c, job, &pts) } else { None };
        if let Some(iv) = tiv {
            tab_pass(e, c, &pts, z, &iv);
        } else if c.ramp && job.closed && !job.pocket && pts.len() > 2 {
            let mut lc = vec![0.0];
            let mut t = 0.0;
            for i in 1..pts.len() { t += (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]); lc.push(t); }
            for (i, q) in pts.iter().enumerate() { e.go("G1", Some(q[0]), Some(q[1]), Some(c.z0 + pz + (z - pz) * lc[i] / t), c.feed); }
            for q in &pts { e.go("G1", Some(q[0]), Some(q[1]), Some(c.z0 + z), c.feed); }
        } else {
            e.go("G1", None, None, Some(c.z0 + z), c.plunge);
            for q in &pts { e.go("G1", Some(q[0]), Some(q[1]), Some(c.z0 + z), c.feed); }
        }
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

pub fn generate(data: &ProjectData, units: Units) -> Result<Output, String> {
    let ku = units.factor();
    let un = units.name();
    let imp = units == Units::Imperial;
    let fl = |v: f64| to_fixed(v / ku, if imp { 4 } else { 3 });
    let st = &data.stock;
    let machine = data.machine.clone().unwrap_or_default();
    // air cut is per operation; an old project-level flag still applies to every operation
    let sz_init = data.job.safe + if data.job.air { 10.0 } else { 0.0 };
    let (ox, oy) = zero_offsets(st);
    let zt = if st.zm == "top" { 0.0 } else { st.t - st.zh };
    let mut e = Em { g: vec![], cur: [0.0, 0.0, sz_init], lf: None, time: 0.0, tc: 0.0, pn: 0, oi: 0, moves: vec![], ox, oy, zt, ku, imp, rap: machine.rapid, tsc: machine.tsc };
    let mut notes: Vec<String> = Vec::new();

    if data.loops.iter().any(|l| l.pts.chunks_exact(2).any(|p| (p[0] as f64) < 0.0 || (p[1] as f64) < 0.0 || (p[0] as f64) > st.w * S || (p[1] as f64) > st.h * S)) {
        notes.push("⚠ Shapes extend beyond the stock.".into());
    }
    if data.loops.iter().any(|l| !data.ops.iter().any(|o| o.id == l.op)) { notes.push("Some shapes have no operation (not machined).".into()); }
    if st.w > machine.x + 1e-6 || st.h > machine.y + 1e-6 || st.t > machine.z + 1e-6 {
        notes.push(format!("⚠ The stock is larger than the machine work area ({} × {} × {} {}).", num(fl(machine.x)), num(fl(machine.y)), num(fl(machine.z)), un));
    }
    e.g.push("(PutiaCNC - GRBL)".into());
    e.g.push(format!("(Stock {} x {} x {} {}; XY0 = {} of stock, Z0 = {})", num(fl(st.w)), num(fl(st.h)), num(fl(st.t)), un, zero_label(&st.zr),
        if st.zm == "top" { "top surface".to_string() } else { format!("stock bottom + {} {}", num(fl(st.zh)), un) }));
    e.g.push(format!("{} G90 G17", if imp { "G20" } else { "G21" }));

    let mut last_d: Option<f64> = None;
    let mut last_r: Option<f64> = None;
    let mut any = false;

    for (k, o) in data.ops.iter().enumerate() {
        e.oi = k;
        let pre = format!("Op {}: ", k + 1);
        let mine: Vec<&LoopData> = data.loops.iter().filter(|l| l.op == o.id).collect();
        if mine.is_empty() { notes.push(format!("Op {} has no shapes.", k + 1)); continue; }
        let tool = data.tools.iter().find(|t| t.id == o.p.tool)
            .ok_or_else(|| format!("Operation {}: choose a tool (Manage… to create one)", k + 1))?;
        let (dia, so, pd, feed, plunge, rpm) = (tool.dia, tool.so, tool.pd, tool.feed, tool.plunge, tool.rpm);
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
        if kind != "online" && !open.is_empty() { notes.push(format!("{}{} open path(s) skipped.", pre, open.len())); }

        let pd = o.p.pass_depth.filter(|v| *v > 0.0).unwrap_or(pd);
        let nz = (depth / pd - 1e-9).ceil() as usize;
        let zs: Vec<f64> = (0..nz).map(|i| -(depth / nz as f64) * (i as f64 + 1.0)).collect();
        let tab_h = o.p.tab_h.unwrap_or(2.0);
        let tabs_req = o.p.tabs && (kind == "outside" || kind == "inside");
        let tabs_on = tabs_req && tab_h > 0.0 && tab_h < depth - 1e-6;
        let mut ctx = OpCtx { kind, dia, so, plunge, feed, ramp: o.p.ramp, rev: false, z0, sz, zs: zs.clone(),
            tabs_on, ztab: -(depth - tab_h), tab_w: o.p.tab_w.unwrap_or(5.0), tab_n: o.p.tab_n.unwrap_or(4.0) };

        let mut jobs: Vec<Job> = Vec::new();
        let mut vr: Vec<(Path, f64)> = Vec::new();
        let mut by_pass = false;
        if kind == "vcarve" {
            if !geo.cone { return Err(format!("Operation {}: V-carve needs a V-bit, engraving or tapered tool (choose one in Manage…)", k + 1)); }
            let zmax = depth.min(geo.z_full);
            if depth > geo.z_full + 1e-6 { notes.push(format!("{}V-carve depth limited to {} {} (the tool diameter is reached).", pre, num(fl(zmax)), un)); }
            let sv = (dia * so / 100.0 * 0.25).min(0.3).max(0.05);
            let mut cur = region.clone();
            for j in 1..3000 {
                cur = off_raw(&cur, -sv * S);
                let d = j as f64 * sv;
                let q = clean_paths(&cur);
                if q.is_empty() { break; }
                let z = -(d / geo.tn).min(zmax);
                for p in q { vr.push((p, z)); }
            }
        } else if geo.cone && kind != "online" {
            by_pass = true;
            notes.push(format!("{}{}: walls are sloped, with the top edge on the line.", pre, geo.shape.label()));
            if kind == "pocket" { notes.push(format!("{}a pocket cut with a cone tool has a ridged floor (try V-carve or a flat tool).", pre)); }
        } else {
            let rm = if geo.shape == ShapeKind::Ball && depth < rr { geo.r_at(depth) } else { rr };
            if rm < rr - 1e-6 && kind != "online" { notes.push(format!("{}depth is below the ball radius, so the cut is narrower than the tool ({} {}).", pre, num(fl(rm * 2.0)), un)); }
            jobs = mk_jobs(&ctx, &region, &closed, &open, rm);
        }
        if if kind == "vcarve" { vr.is_empty() } else { !by_pass && jobs.is_empty() } {
            notes.push(format!("{}no toolpath (tool too large?).", pre));
            continue;
        }
        if tabs_req && !tabs_on { notes.push(format!("{}tabs skipped: the tab height must be smaller than the cut depth.", pre)); }
        ctx.rev = (kind == "inside" || kind == "pocket" || kind == "vcarve") != (o.p.dir == "conv");
        if tabs_on { if let Some(tp) = &o.p.tab_pts { assign_manual_tabs(&ctx, &mut jobs, tp, &mut notes, &pre); } }

        match last_d {
            None => { e.g.push(format!("M3 S{}", num(rpm))); e.g.push("G4 P2".into()); }
            Some(ld) if ld != dia => {
                e.go("G0", None, None, Some(sz), 0.0);
                e.g.push("M5".into());
                e.g.push(format!("M0 (Change tool to {} {}, then resume)", num(fl(dia)), un));
                e.g.push(format!("M3 S{}", num(rpm)));
                e.g.push("G4 P2".into());
            }
            Some(_) if last_r != Some(rpm) => e.g.push(format!("M3 S{}", num(rpm))),
            _ => {}
        }
        last_d = Some(dia); last_r = Some(rpm); e.lf = None;
        e.g.push(format!("(Operation {}: {}, tool {} {}{})", k + 1, kind, num(fl(dia)), un,
            if geo.cone { format!(", {} deg", num(if tool.angle != 0.0 { tool.angle } else { 60.0 })) } else { String::new() }));

        if kind == "vcarve" {
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
                for j in jb.iter_mut() { cut(&mut e, &ctx, j, &[z]); }
            }
        } else if kind == "pocket" {
            for &z in &zs { for j in jobs.iter_mut() { cut(&mut e, &ctx, j, &[z]); } }
        } else {
            for j in jobs.iter_mut() { cut(&mut e, &ctx, j, &zs); }
        }
        any = true;
    }
    if !any { return Err(format!("No toolpath: attach closed shapes to an operation (or the tool is too large). {}", notes.join(" "))); }
    e.g.push("M5".into()); e.g.push("G0 X0 Y0".into()); e.g.push("M2".into());
    let cut_len: f64 = e.moves.iter().filter(|m| m.cut).map(|m| ((m.to[0] - m.from[0]).powi(2) + (m.to[1] - m.from[1]).powi(2) + (m.to[2] - m.from[2]).powi(2)).sqrt()).sum();
    Ok(Output { lines: e.g, moves: e.moves, time_min: e.time, cut_time_min: e.tc, cut_len_mm: cut_len, notes })
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
    #[test]
    fn cone_radius_grows_with_depth() {
        let t = Tool { id: "t".into(), name: "v".into(), sys: "mm".into(), shape: "vbit".into(), angle: 90.0, flutes: 2, dia: 6.0, rpm: 1.0, feed: 1.0, plunge: 1.0, so: 40.0, pd: 1.0, notes: String::new() };
        let g = ToolGeo::new(&t);
        assert!((g.r_at(1.5) - 1.5).abs() < 1e-9);
        assert!((g.r_at(10.0) - 3.0).abs() < 1e-9);
        assert!((g.z_full - 3.0).abs() < 1e-9);
    }
}
