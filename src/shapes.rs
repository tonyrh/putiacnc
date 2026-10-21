//! Shape construction and editing helpers used by the GUI. Everything here is UI-free so it
//! can be unit-tested. Coordinates in the API are millimetres (`[f64; 2]`); the stored
//! loop points are integers in 1/1000 mm.
use crate::model::*;
use std::f64::consts::PI;

pub const S: f64 = 1000.0;

fn to_i(v: f64) -> i64 { (v * S).round() as i64 }

fn loop_from(pts: &[[f64; 2]], closed: bool, op: i64) -> LoopData {
    LoopData {
        c: closed as u8, op, rc: None, cr: None, el: None,
        pts: pts.iter().flat_map(|p| [to_i(p[0]), to_i(p[1])]).collect(),
    }
}

/// Rounded rectangle outline (same tessellation as the browser app). Returns the points and
/// the corner radius that was actually used (clamped to half the shorter side).
pub fn rounded_rect(a: [f64; 2], b: [f64; 2], r: f64) -> (Vec<[f64; 2]>, f64) {
    let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
    let (y0, y1) = (a[1].min(b[1]), a[1].max(b[1]));
    let r = r.max(0.0).min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    if r < 1e-6 { return (vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], 0.0); }
    let n = ((r * 1.5708 / 0.2).ceil() as usize).clamp(4, 64);
    let mut p = Vec::new();
    for (cx, cy, s) in [(x1 - r, y0 + r, -1.5708), (x1 - r, y1 - r, 0.0), (x0 + r, y1 - r, 1.5708), (x0 + r, y0 + r, 3.1416)] {
        for i in 0..=n {
            let t = s + 1.5708 * i as f64 / n as f64;
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
        let t = 6.2832 * i as f64 / n as f64;
        [cx + rx * t.cos(), cy + ry * t.sin()]
    }).collect();
    let mut l = loop_from(&p, true, op);
    l.el = Some(1.0);
    if (rx - ry).abs() <= 1e-6 * rx.max(ry).max(1e-9) { l.cr = Some(1.0); }
    l
}

pub fn new_circle(c: [f64; 2], r: f64, op: i64) -> LoopData {
    new_ellipse([c[0] - r, c[1] - r], [c[0] + r, c[1] + r], op)
}

/// Open or closed polyline. Consecutive duplicate points are dropped; returns None when
/// fewer than two distinct points remain (or fewer than three for a closed shape).
pub fn new_poly(pts: &[[f64; 2]], closed: bool, op: i64) -> Option<LoopData> {
    let mut v: Vec<[f64; 2]> = Vec::new();
    for p in pts { if v.last().map_or(true, |q| q != p) { v.push(*p); } }
    if v.len() < 2 { return None; }
    let closed = closed && v.len() > 2;
    Some(loop_from(&v, closed, op))
}

pub fn is_circle(l: &LoopData) -> bool { l.cr.map_or(false, |v| v != 0.0) }
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

/// Scale about an anchor point (mm). Rectangles are rebuilt from their new bounding box so the
/// corner radius stays circular; a circle scaled non-uniformly becomes a plain ellipse.
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
        if d <= tol && best.map_or(true, |(_, bd)| d < bd) { best = Some((i, d)); }
    }
    if let Some((i, _)) = best { return Some(i); }
    let mut inner: Option<(usize, f64)> = None;
    for (i, l) in loops.iter().enumerate() {
        if point_in_loop(l, p) {
            let b = bbox(l).unwrap();
            let area = (b[2] - b[0]) * (b[3] - b[1]);
            if inner.map_or(true, |(_, a)| area < a) { inner = Some((i, area)); }
        }
    }
    inner.map(|(i, _)| i)
}

/// Does the loop's bounding box lie fully inside the rectangle `[x0, y0, x1, y1]`?
pub fn inside_box(l: &LoopData, r: [f64; 4]) -> bool {
    bbox(l).map_or(false, |b| b[0] >= r[0] && b[1] >= r[1] && b[2] <= r[2] && b[3] <= r[3])
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
        if best.map_or(true, |bb| d < bb.1) { let len = l2.sqrt(); best = Some((q, d, [dx / len, dy / len])); }
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
        if best.map_or(true, |bb| d < bb.1) { best = Some((i, d, q)); }
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
        LoopData { c: 1, op: l.op, rc: None, cr: None, el: None, pts: p.iter().flat_map(|q| [q[0], q[1]]).collect() }
    }).collect()
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
        tabs: false, air: false, tab_n: Some(4.0), tab_w: Some(5.0), tab_h: Some(2.0), tab_pts: None, pass_depth: None,
    }
}

pub fn blank_project() -> ProjectFile {
    ProjectFile {
        app: "putia-cnc".into(), v: 1, name: "Untitled".into(),
        data: ProjectData {
            stock: Stock { w: 200.0, h: 150.0, t: 12.0, mat: "wood".into(), zr: "ll".into(), zm: "top".into(), zh: 0.0 },
            job: Job { safe: 5.0, air: false },
            machine: None, ops: vec![], tools: seed_tools(), loops: vec![], gcode: String::new(), act: 0, oid: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generate, load_project, save_project, Units};

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
        let pts = auto_tab_points(&[l.clone()], 1, 4);
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
    fn circles_and_open_shapes() {
        let c = new_circle([0.0, 0.0], 10.0, 1);
        let g = offset_loop(&c, 1.5);
        assert_eq!(g.len(), 1);
        for p in points(&g[0]) { assert!(((p[0] * p[0] + p[1] * p[1]).sqrt() - 11.5).abs() < 0.03); }
        let open = new_poly(&[[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]], false, 1).unwrap();
        assert!(offset_loop(&open, 1.0).is_empty());
    }
}
