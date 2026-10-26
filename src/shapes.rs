//! Shape construction and editing helpers used by the GUI. Everything here is UI-free so it
//! can be unit-tested. Coordinates in the API are millimetres (`[f64; 2]`); the stored
//! loop points are integers in 1/1000 mm.
use crate::model::*;
use std::f64::consts::PI;

pub const S: f64 = 1000.0;

fn to_i(v: f64) -> i64 { (v * S).round() as i64 }

fn loop_from(pts: &[[f64; 2]], closed: bool, op: i64) -> LoopData {
    LoopData {
        c: closed as u8, op, rc: None, cr: None, el: None, pt: None, grp: None, ug: None,
        pts: pts.iter().flat_map(|p| [to_i(p[0]), to_i(p[1])]).collect(),
    }
}

/// Rounded rectangle outline (same tessellation as the browser app). Returns the points and
/// the corner radius that was actually used (clamped to half the shorter side).
use std::f64::consts::{FRAC_PI_2, TAU};

pub fn rounded_rect(a: [f64; 2], b: [f64; 2], r: f64) -> (Vec<[f64; 2]>, f64) {
    let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
    let (y0, y1) = (a[1].min(b[1]), a[1].max(b[1]));
    let r = r.max(0.0).min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    if r < 1e-6 { return (vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], 0.0); }
    let n = ((r * std::f64::consts::FRAC_PI_2 / 0.2).ceil() as usize).clamp(4, 64);
    let mut p = Vec::new();
    for (cx, cy, s) in [(x1 - r, y0 + r, -FRAC_PI_2), (x1 - r, y1 - r, 0.0), (x0 + r, y1 - r, FRAC_PI_2), (x0 + r, y0 + r, PI)] {
        for i in 0..=n {
            let t = s + FRAC_PI_2 * i as f64 / n as f64;
            p.push([cx + r * t.cos(), cy + r * t.sin()]);
        }
    }
    (p, r)
}

pub fn new_rect(a: [f64; 2], b: [f64; 2], r: f64, op: i64) -> LoopData {
    let (p, used) = rounded_rect(a, b, r);
    let mut l = loop_from(&p, true, op);
    l.rc = Some(used);
    l
}

pub fn new_ellipse(a: [f64; 2], b: [f64; 2], op: i64) -> LoopData {
    let (cx, cy) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
    let (rx, ry) = ((b[0] - a[0]).abs() / 2.0, (b[1] - a[1]).abs() / 2.0);
    let per = PI * (3.0 * (rx + ry) - ((3.0 * rx + ry) * (rx + 3.0 * ry)).sqrt());
    let n = ((per / 0.2).ceil() as usize).clamp(32, 2000);
    let p: Vec<[f64; 2]> = (0..n).map(|i| {
        let t = TAU * i as f64 / n as f64;
        [cx + rx * t.cos(), cy + ry * t.sin()]
    }).collect();
    let mut l = loop_from(&p, true, op);
    l.el = Some(1.0);
    if (rx - ry).abs() <= 1e-6 * rx.max(ry).max(1e-9) { l.cr = Some(1.0); }
    l
}

/// A regular polygon (`inner` = None) or a star (`inner` = Some(ratio of the inner to the outer radius)).
/// `c` is the centre, `r` the outer radius and `angle` the direction of the first corner in radians.
pub fn new_polygon(c: [f64; 2], r: f64, angle: f64, sides: usize, inner: Option<f64>, op: i64) -> Option<LoopData> {
    let n = sides.clamp(3, 200);
    if r < 1e-6 { return None; }
    let mut p: Vec<[f64; 2]> = Vec::new();
    for k in 0..n {
        let a = angle + 2.0 * PI * k as f64 / n as f64;
        p.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
        if let Some(q) = inner {
            let b = a + PI / n as f64;
            let ri = r * q.clamp(0.05, 0.98);
            p.push([c[0] + ri * b.cos(), c[1] + ri * b.sin()]);
        }
    }
    Some(loop_from(&p, true, op))
}

/// A drill point: a tiny octagon (0.4 mm across) flagged as a point. Moves, copies and arrays like any shape.
pub fn new_point(c: [f64; 2], op: i64) -> LoopData {
    let p: Vec<[f64; 2]> = (0..8).map(|k| { let a = 2.0 * PI * k as f64 / 8.0; [c[0] + 0.2 * a.cos(), c[1] + 0.2 * a.sin()] }).collect();
    let mut l = loop_from(&p, true, op);
    l.pt = Some(1.0);
    l
}

/// `n` drill points evenly spaced on a circle of radius `r` around `c`; the first at `angle` (radians).
/// `sweep` is how far round the circle they go, in degrees (counter-clockwise; negative = clockwise):
/// 360 or more is the full circle, anything less puts the last hole at the end of the arc.
pub fn bolt_circle(c: [f64; 2], r: f64, angle: f64, n: usize, sweep: f64, op: i64) -> Vec<LoopData> {
    let n = n.clamp(1, 200);
    if r < 1e-6 { return vec![]; }
    let full = sweep.abs() >= 360.0 - 1e-9;
    let sw = if full { 2.0 * PI * if sweep < 0.0 { -1.0 } else { 1.0 } } else { sweep.to_radians() };
    let steps = if full || n < 2 { n as f64 } else { (n - 1) as f64 };
    (0..n).map(|k| {
        let a = angle + sw * k as f64 / steps;
        new_point([c[0] + r * a.cos(), c[1] + r * a.sin()], op)
    }).collect()
}

/// A grid of drill points filling the box between two corners, `cols` x `rows`, edges included.
/// A single column or row sits on the first corner's line.
pub fn point_grid(a: [f64; 2], b: [f64; 2], cols: usize, rows: usize, op: i64) -> Vec<LoopData> {
    let (cols, rows) = (cols.clamp(1, 100), rows.clamp(1, 100));
    let step = |from: f64, to: f64, n: usize, k: usize| if n < 2 { from } else { from + (to - from) * k as f64 / (n - 1) as f64 };
    let mut v = Vec::new();
    for j in 0..rows { for i in 0..cols {
        v.push(new_point([step(a[0], b[0], cols, i), step(a[1], b[1], rows, j)], op));
    } }
    v
}

pub fn is_point(l: &LoopData) -> bool { l.pt.is_some() }

pub fn new_circle(c: [f64; 2], r: f64, op: i64) -> LoopData {
    new_ellipse([c[0] - r, c[1] - r], [c[0] + r, c[1] + r], op)
}

/// Open or closed polyline. Consecutive duplicate points are dropped; returns None when
/// fewer than two distinct points remain (or fewer than three for a closed shape).
pub fn new_poly(pts: &[[f64; 2]], closed: bool, op: i64) -> Option<LoopData> {
    let mut v: Vec<[f64; 2]> = Vec::new();
    for p in pts { if v.last() != Some(p) { v.push(*p); } }
    if v.len() < 2 { return None; }
    let closed = closed && v.len() > 2;
    Some(loop_from(&v, closed, op))
}

pub fn is_circle(l: &LoopData) -> bool { l.cr.is_some_and(|v| v != 0.0) }
pub fn is_rect(l: &LoopData) -> bool { l.rc.is_some() }

/// Bounding box `[x0, y0, x1, y1]` in mm.
pub fn bbox(l: &LoopData) -> Option<[f64; 4]> {
    if l.pts.len() < 2 { return None; }
    let (mut x0, mut y0, mut x1, mut y1) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
    for c in l.pts.chunks_exact(2) { x0 = x0.min(c[0]); x1 = x1.max(c[0]); y0 = y0.min(c[1]); y1 = y1.max(c[1]); }
    Some([x0 as f64 / S, y0 as f64 / S, x1 as f64 / S, y1 as f64 / S])
}

pub fn bbox_many<'a>(ls: impl IntoIterator<Item = &'a LoopData>) -> Option<[f64; 4]> {
    let mut r: Option<[f64; 4]> = None;
    for l in ls {
        if let Some(b) = bbox(l) {
            r = Some(match r { None => b, Some(a) => [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])] });
        }
    }
    r
}

pub fn translate(l: &mut LoopData, dx: f64, dy: f64) {
    let (ix, iy) = (to_i(dx), to_i(dy));
    for c in l.pts.chunks_exact_mut(2) { c[0] += ix; c[1] += iy; }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CornerKind { Fillet, Chamfer }

/// Corners sharper than this (interior angle) count as corners; the vertices of a flattened curve do not.
const SHARP: f64 = 150.0 * PI / 180.0;

fn pts_mm(l: &LoopData) -> Vec<[f64; 2]> {
    let mut p: Vec<[f64; 2]> = l.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect();
    p.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6);
    if l.closed() && p.len() > 1 && (p[0][0] - p[p.len() - 1][0]).hypot(p[0][1] - p[p.len() - 1][1]) < 1e-6 { p.pop(); }
    p
}

/// Interior angle at vertex `i` and the unit vectors towards its neighbours, if it is a corner.
fn corner_at(p: &[[f64; 2]], i: usize, closed: bool) -> Option<(f64, [f64; 2], [f64; 2], f64, f64)> {
    let n = p.len();
    if n < 3 || (!closed && (i == 0 || i + 1 == n)) { return None; }
    let (a, v, b) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
    let (da, db) = ((a[0] - v[0]).hypot(a[1] - v[1]), (b[0] - v[0]).hypot(b[1] - v[1]));
    if da < 1e-6 || db < 1e-6 { return None; }
    let (ua, ub) = ([(a[0] - v[0]) / da, (a[1] - v[1]) / da], [(b[0] - v[0]) / db, (b[1] - v[1]) / db]);
    let phi = (ua[0] * ub[0] + ua[1] * ub[1]).clamp(-1.0, 1.0).acos();
    if !(0.05..=SHARP).contains(&phi) { return None; }
    Some((phi, ua, ub, da, db))
}

/// Number of sharp corners a shape has (circles and ellipses have none).
pub fn corner_count(l: &LoopData) -> usize {
    if l.cr.is_some() || l.el.is_some() { return 0; }
    let p = pts_mm(l);
    (0..p.len()).filter(|&i| corner_at(&p, i, l.closed()).is_some()).count()
}

pub fn has_corners(l: &LoopData) -> bool { corner_count(l) > 0 }

/// Rounds (fillet, `size` = radius) or bevels (chamfer, `size` = distance along each edge) every sharp corner.
/// A corner too tight for the size gets the largest one that fits. Returns the new shape, the number of
/// corners done and how many of them had to be made smaller; `None` when there is no corner.
pub fn round_corners(l: &LoopData, kind: CornerKind, size: f64) -> Option<(LoopData, usize, usize)> {
    round_corners_at(l, kind, size, None)
}

/// Like `round_corners`, but only the corners at the listed point numbers (`None` = all of them).
pub fn round_corners_at(l: &LoopData, kind: CornerKind, size: f64, only: Option<&std::collections::BTreeSet<usize>>) -> Option<(LoopData, usize, usize)> {
    if l.cr.is_some() || l.el.is_some() || size <= 1e-6 { return None; }
    let closed = l.closed();
    let p = pts_mm(l);
    let n = p.len();
    let chosen = |i: usize| only.is_none_or(|s| s.contains(&i));
    let sharp: Vec<bool> = (0..n).map(|i| chosen(i) && corner_at(&p, i, closed).is_some()).collect();
    let (mut done, mut limited) = (0, 0);
    let mut out: Vec<[f64; 2]> = Vec::new();
    for i in 0..n {
        let Some((phi, ua, ub, da, db)) = corner_at(&p, i, closed).filter(|_| chosen(i)) else { out.push(p[i]); continue };
        // a neighbour that is a corner too may use half of the shared edge, the other half is its own
        let (ia, ib) = ((i + n - 1) % n, (i + 1) % n);
        let (la, lb) = (da * if sharp[ia] { 0.5 } else { 1.0 }, db * if sharp[ib] { 0.5 } else { 1.0 });
        let want = if kind == CornerKind::Fillet { size / (phi / 2.0).tan() } else { size };
        let t = want.min(la).min(lb);
        if t < 1e-4 { out.push(p[i]); continue; }
        if t < want - 1e-9 { limited += 1; }
        done += 1;
        let v = p[i];
        let (t1, t2) = ([v[0] + ua[0] * t, v[1] + ua[1] * t], [v[0] + ub[0] * t, v[1] + ub[1] * t]);
        if kind == CornerKind::Chamfer { out.push(t1); out.push(t2); continue; }
        let r = t * (phi / 2.0).tan();
        let bis = [ua[0] + ub[0], ua[1] + ub[1]];
        let bl = bis[0].hypot(bis[1]);
        let k = r / (phi / 2.0).sin() / bl;
        let c = [v[0] + bis[0] * k, v[1] + bis[1] * k];
        let (s0, e0) = ([t1[0] - c[0], t1[1] - c[1]], [t2[0] - c[0], t2[1] - c[1]]);
        let sweep = PI - phi;
        let dir = if s0[0] * e0[1] - s0[1] * e0[0] >= 0.0 { 1.0 } else { -1.0 };
        let step = if r > 0.02 { 2.0 * (1.0 - 0.02 / r).acos() } else { sweep };
        let steps = ((sweep / step.max(1e-3)).ceil() as usize).clamp(2, 64);
        for j in 0..=steps {
            let a = dir * sweep * j as f64 / steps as f64;
            let (sn, cs) = a.sin_cos();
            out.push([c[0] + s0[0] * cs - s0[1] * sn, c[1] + s0[0] * sn + s0[1] * cs]);
        }
    }
    if done == 0 { return None; }
    let mut res = l.clone();
    res.pts = out.iter().flat_map(|q| [to_i(q[0]), to_i(q[1])]).collect();
    res.rc = None;
    Some((res, done, limited))
}

/// Rotate about a point (mm), counter-clockwise in degrees. A rotated rectangle or ellipse is no longer
/// axis-aligned, so it becomes a plain polygon; a circle stays a circle.
pub fn rotate_about(l: &mut LoopData, cx: f64, cy: f64, deg: f64) {
    let (sn, cs) = deg.to_radians().sin_cos();
    for c in l.pts.chunks_exact_mut(2) {
        let (x, y) = (c[0] as f64 / S - cx, c[1] as f64 / S - cy);
        c[0] = to_i(cx + x * cs - y * sn);
        c[1] = to_i(cy + x * sn + y * cs);
    }
    l.rc = None;
    l.el = None;
}

/// Scale about an anchor point (mm). Rectangles are rebuilt from their new bounding box so the
/// corner radius stays circular; a circle scaled non-uniformly becomes a plain ellipse.
/// Mirror a shape left to right about the vertical line x = `cx` (mm). Keeps the shape's kind.
pub fn mirror_x(l: &mut LoopData, cx: f64) {
    for c in l.pts.chunks_exact_mut(2) { c[0] = to_i(2.0 * cx - c[0] as f64 / S); }
    // a mirrored loop runs the other way round; keep the original order of the points
    let n = l.pts.len() / 2;
    let mut v: Vec<[i64; 2]> = l.pts.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
    if n > 2 && l.closed() { v.reverse(); }
    l.pts = v.into_iter().flatten().collect();
}

/// Mirror a shape top to bottom about the horizontal line y = `cy` (mm).
pub fn mirror_y(l: &mut LoopData, cy: f64) {
    for c in l.pts.chunks_exact_mut(2) { c[1] = to_i(2.0 * cy - c[1] as f64 / S); }
    let n = l.pts.len() / 2;
    let mut v: Vec<[i64; 2]> = l.pts.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
    if n > 2 && l.closed() { v.reverse(); }
    l.pts = v.into_iter().flatten().collect();
}

pub fn scale_about(l: &mut LoopData, ax: f64, ay: f64, sx: f64, sy: f64) {
    for c in l.pts.chunks_exact_mut(2) {
        c[0] = to_i(ax + (c[0] as f64 / S - ax) * sx);
        c[1] = to_i(ay + (c[1] as f64 / S - ay) * sy);
    }
    if let Some(rc) = l.rc {
        if let Some(b) = bbox(l) {
            let (p, used) = rounded_rect([b[0], b[1]], [b[2], b[3]], rc * sx.abs().min(sy.abs()));
            l.pts = p.iter().flat_map(|q| [to_i(q[0]), to_i(q[1])]).collect();
            l.rc = Some(used);
        }
    }
    if l.cr.is_some() && (sx - sy).abs() > 1e-9 { l.cr = None; }
    // a point keeps its size: it only follows where its centre went
    if l.pt.is_some() {
        if let Some(b) = bbox(&l.clone()) {
            let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
            let op = l.op;
            *l = new_point([cx, cy], op);
        }
    }
}

/// Change a rectangle's corner radius, keeping its bounding box.
pub fn set_corner_radius(l: &mut LoopData, r: f64) {
    if l.rc.is_none() { return; }
    if let Some(b) = bbox(l) {
        let (p, used) = rounded_rect([b[0], b[1]], [b[2], b[3]], r);
        l.pts = p.iter().flat_map(|q| [to_i(q[0]), to_i(q[1])]).collect();
        l.rc = Some(used);
    }
}

/// Resize/move a set of loops so their combined bounding box becomes `new`.
pub fn fit_to_bbox(ls: &mut [&mut LoopData], old: [f64; 4], new: [f64; 4]) {
    let (ow, oh) = (old[2] - old[0], old[3] - old[1]);
    let sx = if ow > 1e-9 { (new[2] - new[0]) / ow } else { 1.0 };
    let sy = if oh > 1e-9 { (new[3] - new[1]) / oh } else { 1.0 };
    if sx <= 0.0 || sy <= 0.0 { return; }
    for l in ls.iter_mut() {
        scale_about(l, old[0], old[1], sx, sy);
        translate(l, new[0] - old[0], new[1] - old[1]);
    }
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-18 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

/// Distance in mm from a point to the outline.
pub fn dist_to_loop(l: &LoopData, p: [f64; 2]) -> f64 {
    let v: Vec<[f64; 2]> = l.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect();
    if v.is_empty() { return f64::INFINITY; }
    if v.len() == 1 { return (p[0] - v[0][0]).hypot(p[1] - v[0][1]); }
    let mut d = f64::INFINITY;
    for w in v.windows(2) { d = d.min(seg_dist(p, w[0], w[1])); }
    if l.closed() { d = d.min(seg_dist(p, *v.last().unwrap(), v[0])); }
    d
}

pub fn point_in_loop(l: &LoopData, p: [f64; 2]) -> bool {
    if !l.closed() { return false; }
    let v: Vec<[f64; 2]> = l.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect();
    let mut inside = false;
    let mut j = v.len().wrapping_sub(1);
    for i in 0..v.len() {
        let (a, b) = (v[i], v[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] { inside = !inside; }
        j = i;
    }
    inside
}

/// Index of the loop under `p`: nearest outline within `tol` mm, otherwise the smallest closed
/// shape that contains the point.
pub fn pick(loops: &[LoopData], p: [f64; 2], tol: f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, l) in loops.iter().enumerate() {
        let d = dist_to_loop(l, p);
        if d <= tol && best.is_none_or(|(_, bd)| d < bd) { best = Some((i, d)); }
    }
    if let Some((i, _)) = best { return Some(i); }
    let mut inner: Option<(usize, f64)> = None;
    for (i, l) in loops.iter().enumerate() {
        if point_in_loop(l, p) {
            let b = bbox(l).unwrap();
            let area = (b[2] - b[0]) * (b[3] - b[1]);
            if inner.is_none_or(|(_, a)| area < a) { inner = Some((i, area)); }
        }
    }
    inner.map(|(i, _)| i)
}

/// Does the loop's bounding box lie fully inside the rectangle `[x0, y0, x1, y1]`?
pub fn inside_box(l: &LoopData, r: [f64; 4]) -> bool {
    bbox(l).is_some_and(|b| b[0] >= r[0] && b[1] >= r[1] && b[2] <= r[2] && b[3] <= r[3])
}

pub fn seed_tools() -> Vec<Tool> { crate::toollib::seed_tools() }

/// Closest point on a loop's outline: (point, distance, unit tangent), all in mm.
pub fn nearest_on_loop(l: &LoopData, p: [f64; 2]) -> Option<([f64; 2], f64, [f64; 2])> {
    let pts: Vec<[f64; 2]> = l.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect();
    if pts.len() < 2 { return None; }
    let n = if l.closed() { pts.len() } else { pts.len() - 1 };
    let mut best: Option<([f64; 2], f64, [f64; 2])> = None;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        if l2 < 1e-18 { continue; }
        let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
        let q = [a[0] + t * dx, a[1] + t * dy];
        let d = (p[0] - q[0]).hypot(p[1] - q[1]);
        if best.is_none_or(|bb| d < bb.1) { let len = l2.sqrt(); best = Some((q, d, [dx / len, dy / len])); }
    }
    best
}

// ------------------------------------------------------------------- point editing

/// The shape's points in mm.
pub fn points(l: &LoopData) -> Vec<[f64; 2]> {
    l.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect()
}

/// Forget that this was a rectangle or an ellipse: after a point is edited it is a plain polygon.
pub fn make_polygon(l: &mut LoopData) { l.rc = None; l.cr = None; l.el = None; }

/// Fewest points a shape can keep.
pub fn min_points(l: &LoopData) -> usize { if l.closed() { 3 } else { 2 } }

/// Moves the points with these indices by (dx, dy) mm.
pub fn move_points(l: &mut LoopData, idx: &std::collections::BTreeSet<usize>, dx: f64, dy: f64) {
    let (ix, iy) = (to_i(dx), to_i(dy));
    for &i in idx {
        if let Some(c) = l.pts.chunks_exact_mut(2).nth(i) { c[0] += ix; c[1] += iy; }
    }
    make_polygon(l);
}

/// Adds a point on the edge nearest to `p`, if `p` is within `tol` mm of it. Returns the new point's index.
pub fn insert_point(l: &mut LoopData, p: [f64; 2], tol: f64) -> Option<usize> {
    let pts = points(l);
    let n = pts.len();
    if n < 2 { return None; }
    let edges = if l.closed() { n } else { n - 1 };
    let mut best: Option<(usize, f64, [f64; 2])> = None;
    for i in 0..edges {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        if l2 < 1e-12 { continue; }
        let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
        let q = [a[0] + t * dx, a[1] + t * dy];
        let d = (p[0] - q[0]).hypot(p[1] - q[1]);
        if best.is_none_or(|bb| d < bb.1) { best = Some((i, d, q)); }
    }
    let (i, d, q) = best?;
    if d > tol { return None; }
    let at = (i + 1) * 2;
    l.pts.splice(at..at, [to_i(q[0]), to_i(q[1])]);
    make_polygon(l);
    Some(i + 1)
}

/// Deletes the points with these indices, unless that would leave too few. Returns whether it did.
pub fn delete_points(l: &mut LoopData, idx: &std::collections::BTreeSet<usize>) -> bool {
    let n = l.pts.len() / 2;
    let del = idx.iter().filter(|&&i| i < n).count();
    if del == 0 || n - del < min_points(l) { return false; }
    let mut k = 0;
    let mut keep = Vec::with_capacity(l.pts.len());
    for c in l.pts.chunks_exact(2) {
        if !idx.contains(&k) { keep.extend_from_slice(c); }
        k += 1;
    }
    l.pts = keep;
    make_polygon(l);
    true
}

fn dp_keep(pts: &[[f64; 2]], tol: f64, keep: &mut [bool]) {
    // Douglas-Peucker on pts[0..], first and last point already kept
    let n = pts.len();
    if n < 3 { return; }
    let (a, b) = (pts[0], pts[n - 1]);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let mut worst = (0usize, -1.0f64);
    for (i, p) in pts.iter().enumerate().take(n - 1).skip(1) {
        let d = if l2 < 1e-18 { (p[0] - a[0]).hypot(p[1] - a[1]) }
        else { let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0); (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy) };
        if d > worst.1 { worst = (i, d); }
    }
    if worst.1 > tol {
        keep[worst.0] = true;
        dp_keep(&pts[..=worst.0], tol, &mut keep[..=worst.0]);
        dp_keep(&pts[worst.0..], tol, &mut keep[worst.0..]);
    }
}

/// Removes points that lie within `tol` mm of the line through their neighbours (Douglas-Peucker).
/// Returns (points before, points after). A shape that changes becomes a plain polygon.
pub fn simplify(l: &mut LoopData, tol: f64) -> (usize, usize) {
    let pts = points(l);
    let n = pts.len();
    if n <= min_points(l) { return (n, n); }
    let mut keep = vec![false; n];
    if l.closed() {
        // split the ring at the point farthest from point 0 and simplify both halves
        let far = (1..n).max_by(|&a, &b| {
            let d = |i: usize| (pts[i][0] - pts[0][0]).hypot(pts[i][1] - pts[0][1]);
            d(a).partial_cmp(&d(b)).unwrap()
        }).unwrap();
        keep[0] = true; keep[far] = true;
        dp_keep(&pts[..=far], tol, &mut keep[..=far]);
        let mut second: Vec<[f64; 2]> = pts[far..].to_vec();
        second.push(pts[0]);
        let mut k2 = vec![false; second.len()];
        k2[0] = true; *k2.last_mut().unwrap() = true;
        dp_keep(&second, tol, &mut k2);
        for (j, &k) in k2.iter().enumerate().take(second.len() - 1) { if k { keep[far + j] = true; } }
    } else {
        keep[0] = true; keep[n - 1] = true;
        dp_keep(&pts, tol, &mut keep);
    }
    let kept: Vec<[f64; 2]> = pts.iter().zip(&keep).filter(|(_, &k)| k).map(|(p, _)| *p).collect();
    if kept.len() < min_points(l) || kept.len() == n { return (n, n); }
    l.pts = kept.iter().flat_map(|p| [to_i(p[0]), to_i(p[1])]).collect();
    make_polygon(l);
    (n, kept.len())
}

/// A closed shape grown (`delta_mm` > 0) or shrunk (< 0) by a distance, with rounded outer corners.
/// May give several shapes (a shape pinched in two) or none (shrunk out of existence). Open shapes give none.
pub fn offset_loop(l: &LoopData, delta_mm: f64) -> Vec<LoopData> {
    if !l.closed() || l.pts.len() / 2 < 3 { return vec![]; }
    let region = crate::clip::simplify_even_odd(&[l.path()]);
    let out = crate::clip::offset(&region, delta_mm * S, S * 0.01);
    crate::clip::clean(&out, S * 0.02).into_iter().filter(|p| p.len() > 2).map(|p| {
        LoopData { c: 1, op: l.op, rc: None, cr: None, el: None, pt: None, grp: None, ug: None, pts: p.iter().flat_map(|q| [q[0], q[1]]).collect() }
    }).collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoolOp { Union, Subtract, Intersect, Exclude }

/// Combines closed shapes (in drawing order) into new ones: the union of all of them, the first one with the others cut
/// out (`swap` makes the last one the base instead), the part they all share, or the parts that belong to only one.
/// The result takes the operation of the base shape. Holes come out as shapes inside the outline.
/// Open shapes are ignored; fewer than two closed shapes give nothing.
pub fn combine(shapes: &[&LoopData], op: BoolOp, swap: bool) -> Vec<LoopData> {
    use crate::clip::{boolean, simplify_even_odd, BoolKind};
    let mut list: Vec<&LoopData> = shapes.iter().copied().filter(|l| l.closed() && l.pts.len() / 2 >= 3).collect();
    if list.len() < 2 { return vec![]; }
    if swap { list.reverse(); }
    let regions: Vec<Vec<crate::clip::Path>> = list.iter().map(|l| simplify_even_odd(&[l.path()])).collect();
    let fold = |kind: BoolKind, rs: &[Vec<crate::clip::Path>]| -> Vec<crate::clip::Path> {
        let mut acc = rs[0].clone();
        for r in &rs[1..] { acc = boolean(kind, &acc, r); }
        acc
    };
    let out = match op {
        BoolOp::Union => fold(BoolKind::Union, &regions),
        BoolOp::Intersect => fold(BoolKind::Intersect, &regions),
        BoolOp::Exclude => fold(BoolKind::Xor, &regions),
        BoolOp::Subtract => {
            let cutters = fold(BoolKind::Union, &regions[1..]);
            boolean(BoolKind::Difference, &regions[0], &cutters)
        }
    };
    let opid = list[0].op;
    out.into_iter().filter(|p| p.len() > 2).map(|p| LoopData { c: 1, op: opid, rc: None, cr: None, el: None, pt: None, grp: None, ug: None, pts: p.iter().flat_map(|q| [q[0], q[1]]).collect() }).collect()
}

/// Anchors a point on shape `li` as a tab (position relative to the shape's bounding box).
pub fn tab_anchor(loops: &[LoopData], li: usize, p: [f64; 2]) -> Option<TabPt> {
    let b = bbox(loops.get(li)?)?;
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    Some(TabPt { l: li, u: if w > 1e-9 { (p[0] - b[0]) / w } else { 0.5 }, v: if h > 1e-9 { (p[1] - b[1]) / h } else { 0.5 } })
}

/// Where a tab sits now: the point on its shape's outline (and the outline direction there).
/// `None` when the shape is gone, is not a closed shape of operation `op`, or has no outline.
pub fn tab_resolve(loops: &[LoopData], op: i64, t: &TabPt) -> Option<([f64; 2], [f64; 2])> {
    let l = loops.get(t.l).filter(|l| l.op == op && l.closed())?;
    let b = bbox(l)?;
    let target = [b[0] + t.u * (b[2] - b[0]), b[1] + t.v * (b[3] - b[1])];
    nearest_on_loop(l, target).map(|(q, _, tang)| (q, tang))
}

/// Where the manual tabs of the shapes in `sel` sit now: (operation index, tab index, point).
/// Take this before rotating, so the tabs can follow the shapes.
pub fn tab_positions(loops: &[LoopData], ops: &[Operation], sel: &std::collections::BTreeSet<usize>) -> Vec<(usize, usize, [f64; 2])> {
    let mut v = vec![];
    for (oi, o) in ops.iter().enumerate() {
        let Some(tp) = &o.p.tab_pts else { continue };
        for (ti, t) in tp.iter().enumerate() {
            if !sel.contains(&t.l) { continue; }
            if let Some((p, _)) = tab_resolve(loops, o.id, t) { v.push((oi, ti, p)); }
        }
    }
    v
}

/// Rotate the shapes in `sel` about `pivot` (mm, counter-clockwise degrees) and move their manual tabs along
/// (`tabs0` comes from `tab_positions`, taken before the first rotation).
pub fn rotate_selection(loops: &mut [LoopData], ops: &mut [Operation], sel: &std::collections::BTreeSet<usize>, pivot: [f64; 2], deg: f64, tabs0: &[(usize, usize, [f64; 2])]) {
    for &i in sel { if let Some(l) = loops.get_mut(i) { rotate_about(l, pivot[0], pivot[1], deg); } }
    let (sn, cs) = deg.to_radians().sin_cos();
    for &(oi, ti, p) in tabs0 {
        let (x, y) = (p[0] - pivot[0], p[1] - pivot[1]);
        let q = [pivot[0] + x * cs - y * sn, pivot[1] + x * sn + y * cs];
        if let Some(t) = ops.get_mut(oi).and_then(|o| o.p.tab_pts.as_mut()).and_then(|v| v.get_mut(ti)) {
            if let Some(a) = tab_anchor(loops, t.l, q) { *t = a; }
        }
    }
}

/// Keeps tab anchors right after shapes were deleted (`removed` = their indices, any order).
pub fn tabs_after_delete(ops: &mut [Operation], removed: &[usize]) {
    for o in ops.iter_mut() {
        if let Some(v) = o.p.tab_pts.as_mut() {
            v.retain(|t| !removed.contains(&t.l));
            for t in v.iter_mut() { t.l -= removed.iter().filter(|&&r| r < t.l).count(); }
        }
    }
}

/// Copies the tabs of shape `from` to its duplicate `to`.
pub fn tabs_copy(ops: &mut [Operation], from: usize, to: usize) {
    for o in ops.iter_mut() {
        if let Some(v) = o.p.tab_pts.as_mut() {
            let extra: Vec<TabPt> = v.iter().filter(|t| t.l == from).map(|t| TabPt { l: to, ..*t }).collect();
            v.extend(extra);
        }
    }
}

/// Starting tabs for manual placement, anchored to their shapes: like `auto_tab_points`.
pub fn auto_tabs(loops: &[LoopData], op: i64, n: usize) -> Vec<TabPt> {
    let mut out = Vec::new();
    for (li, l) in loops.iter().enumerate().filter(|(_, l)| l.op == op && l.closed() && l.pts.len() / 2 > 2) {
        for p in auto_tab_points(std::slice::from_ref(l), op, n) { if let Some(t) = tab_anchor(loops, li, p) { out.push(t); } }
    }
    out
}

/// Starting tabs for manual placement: `n` evenly spaced points along every closed outline of operation `op`.
pub fn auto_tab_points(loops: &[LoopData], op: i64, n: usize) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for l in loops.iter().filter(|l| l.op == op && l.closed() && l.pts.len() / 2 > 2) {
        let mut pts: Vec<[f64; 2]> = l.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect();
        pts.push(pts[0]);
        let mut cum = vec![0.0];
        for i in 1..pts.len() { let d = (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]); cum.push(cum[i - 1] + d); }
        let total = *cum.last().unwrap();
        if total <= 0.0 || n == 0 { continue; }
        for q in 0..n {
            let s = (q as f64 + 0.5) * total / n as f64;
            let i = cum.partition_point(|&c| c < s).clamp(1, pts.len() - 1);
            let f = if cum[i] > cum[i - 1] { (s - cum[i - 1]) / (cum[i] - cum[i - 1]) } else { 0.0 };
            out.push([pts[i - 1][0] + (pts[i][0] - pts[i - 1][0]) * f, pts[i - 1][1] + (pts[i][1] - pts[i - 1][1]) * f]);
        }
    }
    out
}

pub fn default_op_params(tool: &str) -> OpParams {
    OpParams {
        kind: "outside".into(), dir: "climb".into(), depth: 3.0, ramp: true, tool: tool.into(),
        tabs: false, air: false, tab_n: Some(4.0), tab_w: Some(5.0), tab_h: Some(2.0), tab_pts: None, pass_depth: None, lead: None, tool2: None, fit: None, d_man: false, r_off: false, f_off: false, r_leave: None, r_so: None, f_so: None,
    }
}

pub fn blank_project() -> ProjectFile {
    ProjectFile {
        app: "putia-cnc".into(), v: 1, name: "Untitled".into(),
        data: ProjectData {
            stock: Stock { w: 200.0, h: 150.0, t: 12.0, mat: "wood".into(), zr: "ll".into(), zm: "top".into(), zh: 0.0 },
            job: Job { safe: 5.0, air: false, numbered: false, post: String::new() },
            machine: None, ops: vec![], tools: seed_tools(), loops: vec![], groups: vec![], clamps: vec![], relief: None, gcode: String::new(), notes: String::new(), act: 0, oid: 0, hidden_ops: vec![],
        },
    }
}

// ---------------------------------------------------------------- adding operations

/// The Profile, Pocket, V-carve, Drill, Inlay and 3D relief buttons: a new operation (inheriting the settings of the
/// last one) that takes over the selected shapes. Returns the operations those shapes were in before (one entry per
/// shape that moved), so the caller can say so.
pub fn add_operation(d: &mut ProjectData, sel: &std::collections::BTreeSet<usize>, kind: &str) -> Option<Vec<i64>> {
    if kind == "inlay" { add_inlay(d, sel); return None; }
    let base = d.ops.last().map(|o| o.p.clone());
    let mut p = base.unwrap_or_else(|| default_op_params(d.tools.first().map(|t| t.id.as_str()).unwrap_or("")));
    p.kind = match kind {
        "relief" => "relief".into(),
        "pocket" => "pocket".into(),
        "vcarve" => "vcarve".into(),
        "drill" => "drill".into(),
        _ => if ["outside", "inside", "online"].contains(&p.kind.as_str()) { p.kind.clone() } else { "outside".into() },
    };
    if kind == "relief" {
        // a ball nose to finish, a flat end mill to rough, and the model's height as the depth
        if let Some(t) = d.tools.iter().find(|t| t.shape == "ball") { p.tool = t.id.clone(); }
        p.tool2 = d.tools.iter().filter(|t| t.shape == "flat").min_by(|a, b| (a.dia - 6.0).abs().partial_cmp(&(b.dia - 6.0).abs()).unwrap_or(std::cmp::Ordering::Equal)).map(|t| t.id.clone());
        p.pass_depth = Some(2.0);
        p.tabs = false; p.lead = None;
    }
    if kind == "vcarve" {
        if let Some(t) = d.tools.iter().find(|t| ["vbit", "engrave", "taper"].contains(&t.shape.as_str())) { p.tool = t.id.clone(); }
    }
    d.oid = d.oid.max(d.ops.iter().map(|o| o.id).max().unwrap_or(0)) + 1;
    let id = d.oid;
    d.ops.push(Operation { id, p });
    let mut from = Vec::new();
    if kind != "relief" {
        from = sel.iter().filter_map(|&i| d.loops.get(i)).map(|l| l.op).filter(|&o| o != 0 && o != id).collect();
        for &i in sel.iter() { if let Some(l) = d.loops.get_mut(i) { l.op = id; } }
    }
    d.act = id;
    Some(from)
}

/// The Inlay button: a pocket operation for the selected shapes and a plug operation for a mirrored copy
/// of them, placed to the right of the selection.
pub fn add_inlay(d: &mut ProjectData, sel: &std::collections::BTreeSet<usize>) {
    let is_cone = |t: &&Tool| ["vbit", "engrave", "taper"].contains(&t.shape.as_str());
    let vbit = d.tools.iter().find(|t| is_cone(t) && t.angle == 60.0).or_else(|| d.tools.iter().find(is_cone)).or(d.tools.first()).map(|t| t.id.clone()).unwrap_or_default();
    let flats: Vec<&Tool> = d.tools.iter().filter(|t| t.shape == "flat").collect();
    let flat = flats.iter().find(|t| t.dia <= 3.5).or(flats.first()).map(|t| t.id.clone());
    let mut base = default_op_params(&vbit);
    base.kind = "inlay".into();
    base.depth = 3.0;
    base.tool2 = flat;
    let mut plug = base.clone();
    plug.kind = "plug".into();
    plug.fit = Some(0.05);
    let first = d.oid.max(d.ops.iter().map(|o| o.id).max().unwrap_or(0)) + 1;
    d.oid = first + 1;
    d.ops.push(Operation { id: first, p: base });
    d.ops.push(Operation { id: first + 1, p: plug });
    d.act = first;
    let picked: Vec<usize> = sel.iter().copied().filter(|&i| i < d.loops.len()).collect();
    for &i in &picked { d.loops[i].op = first; }
    if let Some(b) = bbox_many(picked.iter().map(|&i| &d.loops[i])) {
        let (cx, w) = ((b[0] + b[2]) / 2.0, b[2] - b[0]);
        let copies: Vec<LoopData> = picked.iter().filter(|&&i| d.loops[i].closed()).map(|&i| {
            let mut l = d.loops[i].clone();
            mirror_x(&mut l, cx);
            translate(&mut l, w + 10.0, 0.0);
            l.op = first + 1;
            l
        }).collect();
        d.loops.extend(copies);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generate, load_project, save_project, Units};

    #[test]
    fn mirror_y_flips_top_to_bottom() {
        let mut l = new_poly(&[[0.0, 0.0], [10.0, 0.0], [10.0, 4.0]], true, 0).unwrap();
        mirror_y(&mut l, 5.0);
        let b = bbox(&l).unwrap();
        assert!((b[1] - 6.0).abs() < 1e-6 && (b[3] - 10.0).abs() < 1e-6);
        assert_eq!(l.pts.len(), 6);
    }

    #[test]
    fn notes_are_saved_with_the_project() {
        let mut f = blank_project();
        f.data.notes = "Cherry, vacuum table".into();
        let back = load_project(&save_project(&f).unwrap()).unwrap();
        assert_eq!(back.data.notes, "Cherry, vacuum table");
    }

    fn area_mm2(l: &LoopData) -> f64 { crate::clip::area(&l.path()).abs() / (S * S) }

    #[test]
    fn only_the_chosen_corners_are_rounded() {
        let sq = new_poly(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]], true, 1).unwrap();
        let mut only = std::collections::BTreeSet::new();
        only.insert(2usize);
        let (c, n, _) = round_corners_at(&sq, CornerKind::Chamfer, 2.0, Some(&only)).unwrap();
        assert_eq!(n, 1);
        assert_eq!(c.pts.len() / 2, 5); // three corners stay, one becomes two points
        // an empty choice does nothing
        assert!(round_corners_at(&sq, CornerKind::Chamfer, 2.0, Some(&std::collections::BTreeSet::new())).is_none());
    }

    #[test]
    fn corners_are_rounded_and_bevelled() {
        let sq = new_rect([0.0, 0.0], [20.0, 20.0], 0.0, 1);
        assert_eq!(corner_count(&sq), 4);
        assert_eq!(corner_count(&new_circle([5.0, 5.0], 3.0, 1)), 0);
        assert_eq!(corner_count(&new_ellipse([0.0, 0.0], [10.0, 6.0], 1)), 0);
        let (f, n, lim) = round_corners(&sq, CornerKind::Fillet, 3.0).unwrap();
        assert_eq!((n, lim), (4, 0));
        assert!((area_mm2(&f) - (400.0 - (4.0 - PI) * 9.0)).abs() < 0.5, "{}", area_mm2(&f)); // the arc is a polygon within 0.02 mm of the true one
        let b = bbox(&f).unwrap();
        assert!(b[0] >= -1e-6 && b[1] >= -1e-6 && b[2] <= 20.0 + 1e-6 && b[3] <= 20.0 + 1e-6);
        let (c, n, _) = round_corners(&sq, CornerKind::Chamfer, 2.0).unwrap();
        assert_eq!(n, 4);
        assert!((area_mm2(&c) - 392.0).abs() < 1e-3, "{} {:?}", area_mm2(&c), c.pts);
        // too big for the sides: every corner shrinks to what fits (a full semicircle-ish round)
        let (big, _, lim) = round_corners(&sq, CornerKind::Fillet, 15.0).unwrap();
        assert_eq!(lim, 4);
        assert!((area_mm2(&big) - (400.0 - (4.0 - PI) * 100.0)).abs() < 3.0);
        // an open L: only the inner vertex is a corner
        let l = new_poly(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], false, 1).unwrap();
        assert_eq!(corner_count(&l), 1);
        let (lr, n, _) = round_corners(&l, CornerKind::Chamfer, 2.0).unwrap();
        assert_eq!(n, 1);
        assert_eq!(lr.pts.len() / 2, 4);
        assert!(!lr.closed());
        // smooth outlines have no corners
        assert!(round_corners(&sq_to_circleish(), CornerKind::Fillet, 1.0).is_none());
    }

    fn sq_to_circleish() -> LoopData {
        let mut l = new_ellipse([0.0, 0.0], [10.0, 6.0], 1);
        l.el = None; // a plain polygon, as after node editing
        l
    }

    #[test]
    fn rotation_turns_a_shape_about_a_point() {
        let mut l = new_rect([10.0, 0.0], [20.0, 4.0], 0.0, 1);
        rotate_about(&mut l, 0.0, 0.0, 90.0);
        let b = bbox(&l).unwrap();
        assert!((b[0] + 4.0).abs() < 1e-3 && (b[1] - 10.0).abs() < 1e-3 && b[2].abs() < 1e-3 && (b[3] - 20.0).abs() < 1e-3, "{b:?}");
        assert!(l.rc.is_none());
        let mut c = new_circle([5.0, 0.0], 2.0, 1);
        rotate_about(&mut c, 0.0, 0.0, 180.0);
        assert!(is_circle(&c));
        let bc = bbox(&c).unwrap();
        assert!((bc[0] + 7.0).abs() < 1e-3, "{bc:?}");
    }

    #[test]
    fn rect_bbox_and_radius() {
        let l = new_rect([10.0, 20.0], [50.0, 40.0], 5.0, 1);
        assert_eq!(bbox(&l).unwrap(), [10.0, 20.0, 50.0, 40.0]);
        assert_eq!(l.rc, Some(5.0));
        let l2 = new_rect([0.0, 0.0], [10.0, 4.0], 99.0, 1);
        assert_eq!(l2.rc, Some(2.0)); // clamped
        assert_eq!(new_rect([0.0, 0.0], [10.0, 4.0], 0.0, 1).pts.len(), 8);
    }

    #[test]
    fn polygons_and_stars() {
        let hex = new_polygon([0.0, 0.0], 10.0, 0.0, 6, None, 1).unwrap();
        assert_eq!(hex.pts.len(), 12);
        assert_eq!(&hex.pts[0..2], &[10000, 0]);
        let star = new_polygon([5.0, 5.0], 10.0, std::f64::consts::FRAC_PI_2, 5, Some(0.5), 1).unwrap();
        assert_eq!(star.pts.len(), 20);
        assert_eq!(&star.pts[0..2], &[5000, 15000]); // the first point is straight up
        assert!(new_polygon([0.0, 0.0], 0.0, 0.0, 6, None, 1).is_none());
        assert_eq!(new_polygon([0.0, 0.0], 5.0, 0.0, 1, None, 1).unwrap().pts.len(), 6); // at least a triangle
    }

    #[test]
    fn points_are_shapes_that_keep_their_size() {
        let mut p = new_point([10.0, 20.0], 3);
        assert!(is_point(&p) && p.op == 3 && p.closed());
        translate(&mut p, 5.0, 0.0);
        let b = bbox(&p).unwrap();
        assert!(((b[0] + b[2]) / 2.0 - 15.0).abs() < 1e-6);
        scale_about(&mut p, 0.0, 0.0, 4.0, 4.0);
        let b = bbox(&p).unwrap();
        assert!(is_point(&p) && (b[2] - b[0]) < 0.5 && ((b[0] + b[2]) / 2.0 - 60.0).abs() < 1e-6);
    }

    #[test]
    fn bolt_circle_spaces_points_evenly() {
        let v = bolt_circle([50.0, 50.0], 20.0, 0.0, 4, 360.0, 2);
        assert_eq!(v.len(), 4);
        assert!(v.iter().all(|l| is_point(l) && l.op == 2));
        let cs: Vec<[f64; 2]> = v.iter().map(|l| { let b = bbox(l).unwrap(); [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0] }).collect();
        assert!((cs[0][0] - 70.0).abs() < 1e-3 && (cs[0][1] - 50.0).abs() < 1e-3);
        assert!((cs[1][0] - 50.0).abs() < 1e-3 && (cs[1][1] - 70.0).abs() < 1e-3);
        assert!(bolt_circle([0.0, 0.0], 0.0, 0.0, 4, 360.0, 0).is_empty());
        // a quarter arc: the last hole sits at the end of the arc, 90 degrees round
        let q = bolt_circle([0.0, 0.0], 10.0, 0.0, 3, 90.0, 0);
        let c = |l: &LoopData| { let b = bbox(l).unwrap(); [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0] };
        assert!((c(&q[2])[0]).abs() < 1e-3 && (c(&q[2])[1] - 10.0).abs() < 1e-3);
        assert!((c(&q[1])[0] - 7.071).abs() < 1e-2);
        // negative sweeps go clockwise
        let cw = bolt_circle([0.0, 0.0], 10.0, 0.0, 2, -90.0, 0);
        assert!((c(&cw[1])[1] + 10.0).abs() < 1e-3);
    }

    #[test]
    fn point_grid_fills_the_box() {
        let g = point_grid([10.0, 10.0], [30.0, 20.0], 3, 2, 1);
        assert_eq!(g.len(), 6);
        let c = |l: &LoopData| { let b = bbox(l).unwrap(); [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0] };
        assert!((c(&g[0])[0] - 10.0).abs() < 1e-3 && (c(&g[1])[0] - 20.0).abs() < 1e-3 && (c(&g[2])[0] - 30.0).abs() < 1e-3);
        assert!((c(&g[3])[1] - 20.0).abs() < 1e-3);
        // dragging towards the lower left works the same way
        let r = point_grid([30.0, 20.0], [10.0, 10.0], 2, 1, 0);
        assert!((c(&r[1])[0] - 10.0).abs() < 1e-3 && (c(&r[1])[1] - 20.0).abs() < 1e-3);
    }

    #[test]
    fn circle_flags() {
        let c = new_circle([5.0, 5.0], 3.0, 0);
        assert!(is_circle(&c) && c.el.is_some());
        let b = bbox(&c).unwrap();
        assert!((b[2] - b[0] - 6.0).abs() < 0.01);
        assert!(!is_circle(&new_ellipse([0.0, 0.0], [10.0, 4.0], 0)));
    }

    #[test]
    fn scaling_rebuilds_rect_and_moves() {
        let mut l = new_rect([0.0, 0.0], [20.0, 10.0], 4.0, 1);
        let old = bbox(&l).unwrap();
        fit_to_bbox(&mut [&mut l], old, [5.0, 5.0, 65.0, 35.0]);
        let b = bbox(&l).unwrap();
        for (x, y) in b.iter().zip([5.0, 5.0, 65.0, 35.0]) { assert!((x - y).abs() < 1e-3, "{b:?}"); }
        assert!((l.rc.unwrap() - 12.0).abs() < 1e-3);
        let mut c = new_circle([0.0, 0.0], 5.0, 0);
        let ob = bbox(&c).unwrap();
        fit_to_bbox(&mut [&mut c], ob, [0.0, 0.0, 20.0, 10.0]);
        assert!(!is_circle(&c));
    }

    #[test]
    fn picking() {
        let ls = vec![new_rect([0.0, 0.0], [100.0, 100.0], 0.0, 0), new_rect([40.0, 40.0], [60.0, 60.0], 0.0, 0)];
        assert_eq!(pick(&ls, [0.2, 50.0], 1.0), Some(0)); // on the outer edge
        assert_eq!(pick(&ls, [50.0, 50.0], 1.0), Some(1)); // inside both -> smallest
        assert_eq!(pick(&ls, [20.0, 20.0], 1.0), Some(0));
        assert_eq!(pick(&ls, [200.0, 200.0], 1.0), None);
    }

    #[test]
    fn poly_rules() {
        assert!(new_poly(&[[0.0, 0.0], [0.0, 0.0]], false, 0).is_none());
        let l = new_poly(&[[0.0, 0.0], [5.0, 0.0]], true, 0).unwrap();
        assert!(!l.closed()); // two points can't close
        assert!(new_poly(&[[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]], true, 0).unwrap().closed());
    }

    #[test]
    fn save_load_generate_roundtrip() {
        let mut p = blank_project();
        p.data.oid = 1; p.data.act = 1;
        p.data.ops.push(Operation { id: 1, p: default_op_params(&p.data.tools[1].id.clone()) });
        p.data.loops.push(new_rect([20.0, 20.0], [80.0, 60.0], 5.0, 1));
        p.data.loops.push(new_circle([100.0, 40.0], 10.0, 1));
        let s = save_project(&p).unwrap();
        assert!(!s.contains("null"), "no nulls expected for the browser app: {s}");
        let q = load_project(&s).unwrap();
        assert_eq!(q.data.loops.len(), 2);
        let out = generate(&q.data, Units::Metric).unwrap();
        assert!(out.lines.iter().any(|l| l.starts_with("G1")));
        assert!(out.cut_len_mm > 100.0);
    }
}

#[cfg(test)]
mod tab_tests {
    use super::*;

    #[test]
    fn auto_points_lie_on_the_outline_and_are_spread() {
        let l = new_rect([0.0, 0.0], [100.0, 50.0], 0.0, 1);
        let pts = auto_tab_points(std::slice::from_ref(&l), 1, 4);
        assert_eq!(pts.len(), 4);
        for p in &pts { assert!(nearest_on_loop(&l, *p).unwrap().1 < 1e-6); }
        assert!((pts[0][0] - pts[1][0]).abs() + (pts[0][1] - pts[1][1]).abs() > 10.0);
    }

    #[test]
    fn nearest_point_and_tangent() {
        let l = new_rect([0.0, 0.0], [100.0, 50.0], 0.0, 1);
        let (q, d, t) = nearest_on_loop(&l, [40.0, -3.0]).unwrap();
        assert!((q[1]).abs() < 1e-9 && (q[0] - 40.0).abs() < 1e-9 && (d - 3.0).abs() < 1e-9);
        assert!(t[1].abs() < 1e-9);
    }
}

#[cfg(test)]
mod point_tests {
    use super::*;
    use std::collections::BTreeSet;

    fn set(v: &[usize]) -> BTreeSet<usize> { v.iter().copied().collect() }

    #[test]
    fn move_insert_delete() {
        let mut l = new_rect([0.0, 0.0], [10.0, 10.0], 0.0, 1);
        assert_eq!(points(&l).len(), 4);
        move_points(&mut l, &set(&[2]), 3.0, -1.0);
        assert!(l.rc.is_none(), "editing a point makes the rectangle a polygon");
        let p = points(&l);
        assert!((p[2][0] - 13.0).abs() < 1e-9 && (p[2][1] - 9.0).abs() < 1e-9);
        // add a point on the first edge (0,0)-(10,0)
        let i = insert_point(&mut l, [4.0, 0.2], 0.5).unwrap();
        assert_eq!(i, 1);
        let p = points(&l);
        assert_eq!(p.len(), 5);
        assert!((p[1][0] - 4.0).abs() < 1e-9 && p[1][1].abs() < 1e-9);
        assert!(insert_point(&mut l, [4.0, 3.0], 0.5).is_none(), "too far from any edge");
        assert!(delete_points(&mut l, &set(&[1])));
        assert_eq!(points(&l).len(), 4);
        // a closed shape keeps at least three points
        assert!(delete_points(&mut l, &set(&[0])));
        assert!(!delete_points(&mut l, &set(&[0])));
        assert_eq!(points(&l).len(), 3);
    }

    #[test]
    fn insert_on_the_closing_edge() {
        let mut l = new_rect([0.0, 0.0], [10.0, 10.0], 0.0, 1);
        let i = insert_point(&mut l, [0.1, 5.0], 0.5).unwrap(); // edge (0,10)-(0,0)
        assert_eq!(i, 4);
        assert_eq!(points(&l).len(), 5);
    }

    #[test]
    fn simplify_removes_collinear_points_and_keeps_corners() {
        let mut pts = vec![];
        for i in 0..=10 { pts.push([i as f64, 0.0]); }
        for i in 1..=10 { pts.push([10.0, i as f64]); }
        for i in (0..10).rev() { pts.push([i as f64, 10.0]); }
        for i in (1..10).rev() { pts.push([0.0, i as f64]); }
        let mut l = new_poly(&pts, true, 1).unwrap();
        let (before, after) = simplify(&mut l, 0.01);
        assert_eq!(before, 40);
        assert_eq!(after, 4);
        let b = bbox(&l).unwrap();
        assert_eq!(b, [0.0, 0.0, 10.0, 10.0]);
        // open polyline
        let mut o = new_poly(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [2.0, 2.0]], false, 1).unwrap();
        assert_eq!(simplify(&mut o, 0.01), (4, 3));
    }

    #[test]
    fn simplify_a_circle_keeps_its_shape() {
        let mut l = new_circle([0.0, 0.0], 20.0, 1);
        let (before, after) = simplify(&mut l, 0.05);
        assert!(after < before && after >= 8, "{before} -> {after}");
        for p in points(&l) { assert!(((p[0] * p[0] + p[1] * p[1]).sqrt() - 20.0).abs() < 0.05); }
    }
}

#[cfg(test)]
mod combine_tests {
    use super::*;
    use crate::clip;
    fn rect(x0: f64, y0: f64, x1: f64, y1: f64, op: i64) -> LoopData { new_rect([x0, y0], [x1, y1], 0.0, op) }
    fn net_area(v: &[LoopData]) -> f64 { v.iter().map(|l| clip::area(&l.path())).sum::<f64>().abs() / (S * S) }
    #[test]
    fn two_overlapping_squares() {
        let (a, b) = (rect(0.0, 0.0, 10.0, 10.0, 3), rect(5.0, 0.0, 15.0, 10.0, 4));
        let l = [&a, &b];
        let u = combine(&l, BoolOp::Union, false);
        assert_eq!(u.len(), 1);
        assert!((net_area(&u) - 150.0).abs() < 0.01);
        assert_eq!(u[0].op, 3);
        assert!((net_area(&combine(&l, BoolOp::Intersect, false)) - 50.0).abs() < 0.01);
        let s = combine(&l, BoolOp::Subtract, false);
        assert!((net_area(&s) - 50.0).abs() < 0.01 && shapes_bbox(&s)[2] < 5.01);
        let sw = combine(&l, BoolOp::Subtract, true);
        assert!((net_area(&sw) - 50.0).abs() < 0.01 && shapes_bbox(&sw)[0] > 9.99);
        assert_eq!(sw[0].op, 4, "the result belongs to the base shape's operation");
        let x = combine(&l, BoolOp::Exclude, false);
        assert_eq!(x.len(), 2);
        assert!((x.iter().map(|l| clip::area(&l.path()).abs()).sum::<f64>() / (S * S) - 100.0).abs() < 0.01);
    }
    fn shapes_bbox(v: &[LoopData]) -> [f64; 4] { bbox_many(v.iter()).unwrap() }
    #[test]
    fn a_hole_stays_a_hole_and_separate_shapes_give_nothing() {
        let (a, b) = (rect(0.0, 0.0, 10.0, 10.0, 1), rect(3.0, 3.0, 6.0, 6.0, 1));
        let s = combine(&[&a, &b], BoolOp::Subtract, false);
        assert_eq!(s.len(), 2, "outline and hole");
        assert!((net_area(&s) - 91.0).abs() < 0.01);
        let far = rect(50.0, 50.0, 60.0, 60.0, 1);
        assert!(combine(&[&a, &far], BoolOp::Intersect, false).is_empty());
        assert_eq!(combine(&[&a, &far], BoolOp::Union, false).len(), 2);
        assert!(combine(&[&a], BoolOp::Union, false).is_empty());
        // three shapes: the base minus two cutters
        let (c1, c2) = (rect(0.0, 0.0, 4.0, 10.0, 1), rect(6.0, 0.0, 10.0, 10.0, 1));
        assert!((net_area(&combine(&[&a, &c1, &c2], BoolOp::Subtract, false)) - 20.0).abs() < 0.01);
    }
}

#[cfg(test)]
mod offset_tests {
    use super::*;

    fn area(l: &LoopData) -> f64 { crate::clip::area(&l.path()).abs() / (S * S) }

    #[test]
    fn grow_and_shrink_a_rectangle() {
        let r = new_rect([0.0, 0.0], [20.0, 10.0], 0.0, 3);
        let g = offset_loop(&r, 2.0);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].op, 3);
        let b = bbox(&g[0]).unwrap();
        assert!((b[0] + 2.0).abs() < 0.02 && (b[1] + 2.0).abs() < 0.02 && (b[2] - 22.0).abs() < 0.02 && (b[3] - 12.0).abs() < 0.02, "{b:?}");
        // rounded corners: area = w*h + perimeter*d + pi*d^2
        let expect = 200.0 + 60.0 * 2.0 + std::f64::consts::PI * 4.0;
        assert!((area(&g[0]) - expect).abs() < 0.5, "{} vs {expect}", area(&g[0]));
        let s = offset_loop(&r, -2.0);
        assert_eq!(s.len(), 1);
        let b = bbox(&s[0]).unwrap();
        assert!((b[0] - 2.0).abs() < 0.02 && (b[3] - 8.0).abs() < 0.02, "{b:?}");
        assert!((area(&s[0]) - 16.0 * 6.0).abs() < 0.2);
        // too far inward: nothing is left
        assert!(offset_loop(&r, -6.0).is_empty());
        assert!(!is_rect(&g[0]) && g[0].cr.is_none());
    }

    #[test]
    fn rotating_a_selection_takes_its_manual_tabs_along() {
        use std::collections::BTreeSet;
        let mut d = blank_project().data;
        let tool = d.tools[0].id.clone();
        let mut p = default_op_params(&tool);
        p.tabs = true;
        d.oid = 1;
        d.loops.push(new_rect([0.0, 0.0], [40.0, 20.0], 0.0, 1));
        // one tab on the right edge of the rectangle
        let tab = tab_anchor(&d.loops, 0, [40.0, 10.0]).unwrap();
        p.tab_pts = Some(vec![tab]);
        d.ops.push(Operation { id: 1, p });
        let sel: BTreeSet<usize> = [0].into_iter().collect();
        let t0 = tab_positions(&d.loops, &d.ops, &sel);
        assert_eq!(t0.len(), 1);
        assert!((t0[0].2[0] - 40.0).abs() < 1e-6 && (t0[0].2[1] - 10.0).abs() < 1e-6);
        // 90 degrees about the centre (20, 10): the right edge becomes the top edge, the tab at (20, 30)
        rotate_selection(&mut d.loops, &mut d.ops, &sel, [20.0, 10.0], 90.0, &t0);
        let b = bbox(&d.loops[0]).unwrap();
        assert!((b[2] - b[0] - 20.0).abs() < 1e-3 && (b[3] - b[1] - 40.0).abs() < 1e-3);
        let t1 = tab_positions(&d.loops, &d.ops, &sel);
        assert_eq!(t1.len(), 1);
        assert!((t1[0].2[0] - 20.0).abs() < 1e-3 && (t1[0].2[1] - 30.0).abs() < 1e-3, "{:?}", t1[0].2);
    }

    #[test]
    fn circles_and_open_shapes() {
        let c = new_circle([0.0, 0.0], 10.0, 1);
        let g = offset_loop(&c, 1.5);
        assert_eq!(g.len(), 1);
        for p in points(&g[0]) { assert!(((p[0] * p[0] + p[1] * p[1]).sqrt() - 11.5).abs() < 0.03); }
        let open = new_poly(&[[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]], false, 1).unwrap();
        assert!(offset_loop(&open, 1.0).is_empty());
    }
}


/// Puts the shapes in `sel` into operation `op` (the shapes already there stay). Returns how many moved.
pub fn assign_to_op(loops: &mut [LoopData], sel: &std::collections::BTreeSet<usize>, op: i64) -> usize {
    let mut n = 0;
    for &k in sel { if let Some(l) = loops.get_mut(k) { if l.op != op { l.op = op; n += 1; } } }
    n
}

/// Hands out group numbers for copies of grouped shapes: all copies made from one group in one go share a new group,
/// so a copy of a group is a group of its own and never joins the original.
pub struct GroupCopies { next: i64, map: std::collections::HashMap<i64, i64> }

impl GroupCopies {
    pub fn new(loops: &[LoopData]) -> Self { GroupCopies { next: loops.iter().filter_map(|l| l.ug).max().unwrap_or(0) + 1, map: Default::default() } }
    /// A group number nobody uses yet.
    pub fn fresh(&mut self) -> i64 { let n = self.next; self.next += 1; n }
    /// Call before each batch of copies (each step of an array).
    pub fn next_batch(&mut self) { self.map.clear(); }
    pub fn apply(&mut self, l: &mut LoopData) {
        if let Some(g) = l.ug {
            let next = &mut self.next;
            l.ug = Some(*self.map.entry(g).or_insert_with(|| { let n = *next; *next += 1; n }));
        }
    }
}

#[cfg(test)]
mod group_copy_tests {
    use super::*;
    #[test]
    fn selected_shapes_move_into_an_operation() {
        let mut a = new_rect([0.0, 0.0], [5.0, 5.0], 0.0, 4);
        let b = new_rect([10.0, 0.0], [15.0, 5.0], 0.0, 0);
        a.op = 4;
        let mut v = vec![a, b];
        let sel: std::collections::BTreeSet<usize> = [0, 1].into_iter().collect();
        assert_eq!(assign_to_op(&mut v, &sel, 7), 2);
        assert!(v.iter().all(|l| l.op == 7));
        assert_eq!(assign_to_op(&mut v, &sel, 7), 0, "nothing left to move");
        assert_eq!(assign_to_op(&mut v, &[9].into_iter().collect(), 7), 0, "an index past the end is ignored");
    }

    #[test]
    fn copies_of_a_group_make_a_new_group_each_time() {
        let mut a = new_rect([0.0, 0.0], [5.0, 5.0], 0.0, 0); a.ug = Some(3);
        let mut b = new_rect([10.0, 0.0], [5.0, 5.0], 0.0, 0); b.ug = Some(3);
        let loose = new_rect([20.0, 0.0], [5.0, 5.0], 0.0, 0);
        let all = vec![a.clone(), b.clone(), loose.clone()];
        let mut gc = GroupCopies::new(&all);
        let (mut a1, mut b1, mut l1) = (a.clone(), b.clone(), loose.clone());
        gc.apply(&mut a1); gc.apply(&mut b1); gc.apply(&mut l1);
        assert_eq!(a1.ug, b1.ug);
        assert!(a1.ug.is_some() && a1.ug != Some(3), "the copy is a group of its own");
        assert!(l1.ug.is_none());
        gc.next_batch();
        let mut a2 = a.clone(); gc.apply(&mut a2);
        assert!(a2.ug != a1.ug && a2.ug != Some(3), "the next copy is another group");
    }
}

#[cfg(test)]
mod add_operation_tests {
    use super::*;

    fn two_shapes() -> ProjectFile {
        let mut f = blank_project();
        f.data.loops.clear();
        f.data.loops.push(new_poly(&[[10.0, 10.0], [30.0, 10.0], [30.0, 20.0], [15.0, 30.0]], true, 0).unwrap());
        f.data.loops.push(new_circle([50.0, 20.0], 5.0, 0));
        f
    }

    #[test]
    fn a_new_operation_takes_over_the_selected_shapes() {
        let mut f = two_shapes();
        let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
        let from = add_operation(&mut f.data, &sel, "pocket").unwrap();
        assert!(from.is_empty()); // the shape had no operation before
        let op = f.data.ops.last().unwrap();
        assert_eq!(op.p.kind, "pocket");
        assert_eq!(f.data.loops[0].op, op.id);
        assert_eq!(f.data.loops[1].op, 0); // the other shape is untouched
        assert_eq!(f.data.act, op.id);
    }

    #[test]
    fn shapes_that_leave_an_operation_are_reported() {
        let mut f = two_shapes();
        let all: std::collections::BTreeSet<usize> = [0, 1].into_iter().collect();
        add_operation(&mut f.data, &all, "outside");
        let first = f.data.ops[0].id;
        let from = add_operation(&mut f.data, &all, "pocket").unwrap();
        assert_eq!(from, vec![first, first]);
        let second = f.data.ops[1].id;
        assert_ne!(first, second);
        assert!(f.data.loops.iter().all(|l| l.op == second));
    }

    #[test]
    fn operation_ids_never_repeat() {
        let mut f = two_shapes();
        let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
        for _ in 0..3 { add_operation(&mut f.data, &sel, "outside"); }
        let mut ids: Vec<i64> = f.data.ops.iter().map(|o| o.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), f.data.ops.len());
    }

    #[test]
    fn a_relief_operation_leaves_the_shapes_alone_and_picks_a_ball_nose() {
        let mut f = two_shapes();
        let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
        let from = add_operation(&mut f.data, &sel, "relief").unwrap();
        assert!(from.is_empty());
        assert_eq!(f.data.loops[0].op, 0);
        let op = f.data.ops.last().unwrap();
        assert_eq!(op.p.kind, "relief");
        assert!(f.data.tools.iter().find(|t| t.id == op.p.tool).is_some_and(|t| t.shape == "ball") || !f.data.tools.iter().any(|t| t.shape == "ball"));
        assert_eq!(op.p.pass_depth, Some(2.0));
    }

    #[test]
    fn a_vcarve_operation_picks_a_cone_tool() {
        let mut f = two_shapes();
        let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
        add_operation(&mut f.data, &sel, "vcarve");
        let op = f.data.ops.last().unwrap();
        assert!(f.data.tools.iter().find(|t| t.id == op.p.tool).is_some_and(|t| ["vbit", "engrave", "taper"].contains(&t.shape.as_str())));
    }

    #[test]
    fn the_inlay_button_makes_a_pocket_and_a_mirrored_plug_beside_it() {
        let mut f = two_shapes();
        let d = &mut f.data;
        let sel: std::collections::BTreeSet<usize> = [0, 1].into_iter().collect();
        assert!(add_operation(d, &sel, "inlay").is_none());
        assert_eq!(d.ops.len(), 2);
        assert_eq!((d.ops[0].p.kind.as_str(), d.ops[1].p.kind.as_str()), ("inlay", "plug"));
        assert_eq!(d.ops[0].p.depth, 3.0);
        assert_eq!(d.ops[1].p.fit, Some(0.05));
        let vb = d.tools.iter().find(|t| t.id == d.ops[0].p.tool).unwrap();
        assert!(vb.shape == "vbit" && vb.angle == 60.0);
        let fl = d.tools.iter().find(|t| Some(&t.id) == d.ops[0].p.tool2.as_ref()).unwrap();
        assert!(fl.shape == "flat" && fl.dia <= 3.5);
        assert_eq!(d.ops[1].p.tool2, d.ops[0].p.tool2);
        assert_eq!(d.loops.len(), 4);
        assert!(d.loops[0].op == d.ops[0].id && d.loops[1].op == d.ops[0].id);
        assert!(d.loops[2].op == d.ops[1].id && d.loops[3].op == d.ops[1].id);
        let b = bbox_many(d.loops[2..].iter()).unwrap();
        assert!((b[0] - 65.0).abs() < 0.01 && (b[2] - 110.0).abs() < 0.01, "{b:?}");
        let p = d.loops[2].path();
        assert!(p.iter().any(|q| q[1] == 30_000 && (q[0] as f64 / 1000.0 - 105.0).abs() < 0.01), "{p:?}");
    }
}
