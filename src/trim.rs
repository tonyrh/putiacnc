//! Trim and extend. Trim removes the stretch of a shape between the places where other shapes (or the shape
//! itself) cross it, the way you click on the part of a line you do not want. Extend lengthens the end of an
//! open path in a straight line until it meets another shape.
use crate::model::LoopData;
use crate::shapes::S;

type P = [f64; 2];

fn pts(l: &LoopData) -> Vec<P> { l.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect() }
fn to_i(v: f64) -> i64 { (v * S).round() as i64 }

/// Parameter along a path: the integer part is the segment, the fraction the way along it.
fn point_at(p: &[P], closed: bool, s: f64) -> P {
    let n = p.len();
    let k = (s.floor() as usize).min(n - 1);
    let t = s - k as f64;
    let a = p[k];
    let b = if closed { p[(k + 1) % n] } else { p[(k + 1).min(n - 1)] };
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn nseg(n: usize, closed: bool) -> usize { if closed { n } else { n.saturating_sub(1) } }

/// Where segment a->b meets c->d: the fractions along each.
fn cross(a: P, b: P, c: P, d: P) -> Option<(f64, f64)> {
    let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-12 { return None; }
    let (qp0, qp1) = (c[0] - a[0], c[1] - a[1]);
    let t = (qp0 * s[1] - qp1 * s[0]) / den;
    let u = (qp0 * r[1] - qp1 * r[0]) / den;
    let e = 1e-9;
    if t >= -e && t <= 1.0 + e && u >= -e && u <= 1.0 + e { Some((t.clamp(0.0, 1.0), u.clamp(0.0, 1.0))) } else { None }
}

fn usable(l: &LoopData) -> bool { l.pt.is_none() && l.pts.len() >= 4 }

/// The places along `target` where something crosses it, as sorted, distinct path parameters.
fn cuts(loops: &[LoopData], target: usize, p: &[P], closed: bool) -> Vec<f64> {
    let n = p.len();
    let ns = nseg(n, closed);
    let mut out: Vec<f64> = Vec::new();
    for k in 0..ns {
        let (a, b) = (p[k], p[(k + 1) % n]);
        for (j, l) in loops.iter().enumerate() {
            if !usable(l) { continue; }
            if j == target {
                // the shape against itself: not the neighbouring segments, and each pair once
                for m in k + 2..ns {
                    if closed && k == 0 && m == ns - 1 { continue; }
                    let (c, d) = (p[m], p[(m + 1) % n]);
                    if let Some((t, u)) = cross(a, b, c, d) {
                        out.push(k as f64 + t);
                        out.push(m as f64 + u);
                    }
                }
                continue;
            }
            let q = pts(l);
            let qn = q.len();
            for m in 0..nseg(qn, l.closed()) {
                if let Some((t, _)) = cross(a, b, q[m], q[(m + 1) % qn]) {
                    // a crossing exactly at the end of a segment belongs to the next one (except at the end of an open path)
                    if t > 1.0 - 1e-9 && (closed || k + 1 < ns) { continue; }
                    out.push(k as f64 + t);
                }
            }
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    // on a closed path the end of the last segment is the start of the first
    if closed && out.len() > 1 && (out[0] + n as f64 - out[out.len() - 1]).abs() < 1e-7 { out.pop(); }
    out
}

fn nearest_s(p: &[P], closed: bool, at: P) -> (f64, f64) {
    let n = p.len();
    let mut best = (0.0, f64::INFINITY);
    for k in 0..nseg(n, closed) {
        let (a, b) = (p[k], p[(k + 1) % n]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 < 1e-18 { 0.0 } else { (((at[0] - a[0]) * dx + (at[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
        let d = (at[0] - (a[0] + dx * t)).hypot(at[1] - (a[1] + dy * t));
        if d < best.1 { best = (k as f64 + t, d); }
    }
    best
}

fn dedupe(v: Vec<P>) -> Vec<P> {
    let mut out: Vec<P> = Vec::with_capacity(v.len());
    for q in v { if out.last().map_or(true, |l| (l[0] - q[0]).hypot(l[1] - q[1]) > 1e-7) { out.push(q); } }
    out
}

fn length(v: &[P]) -> f64 { v.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum() }

/// What a trim click would do: the stretch that goes, and the open pieces that stay.
pub struct TrimPlan { pub remove: Vec<P>, pub keep: Vec<Vec<P>> }

pub fn plan_trim(loops: &[LoopData], target: usize, at: P) -> Option<TrimPlan> {
    let l = loops.get(target).filter(|l| usable(l))?;
    let (p, closed) = (pts(l), l.closed());
    let n = p.len();
    let c = cuts(loops, target, &p, closed);
    let (s, _) = nearest_s(&p, closed, at);
    let nf = n as f64;
    let e = 1e-9;
    if closed {
        if c.len() < 2 { return None; }
        // the cut just before the click and the one just after (wrapping around)
        let a = c.iter().copied().rev().find(|&x| x <= s + e).unwrap_or(*c.last().unwrap());
        let b = c.iter().copied().find(|&x| x > s + e).unwrap_or(c[0]);
        let fwd = |from: f64, to: f64| { let d = to - from; if d < 0.0 { d + nf } else { d } };
        let between = |from: f64, to: f64| -> Vec<P> {
            let span = fwd(from, to);
            let mut v = vec![point_at(&p, true, from)];
            for k in 1..=n {
                let idx = ((from.floor() as usize) + k) % n;
                let d = fwd(from, idx as f64);
                if d > e && d < span - e { v.push(p[idx]); }
            }
            v.push(point_at(&p, true, to));
            dedupe(v)
        };
        let remove = between(a, b);
        let keep = between(b, a);
        if length(&keep) < 1e-4 { return None; }
        Some(TrimPlan { remove, keep: vec![keep] })
    } else {
        let prev = c.iter().copied().rev().find(|&x| x <= s + e);
        let next = c.iter().copied().find(|&x| x > s + e);
        if prev.is_none() && next.is_none() { return None; }
        let end = (n - 1) as f64;
        let part = |from: f64, to: f64| -> Vec<P> {
            let mut v = vec![point_at(&p, false, from)];
            for k in 0..n { let kf = k as f64; if kf > from + e && kf < to - e { v.push(p[k]); } }
            v.push(point_at(&p, false, to));
            dedupe(v)
        };
        let remove = part(prev.unwrap_or(0.0), next.unwrap_or(end));
        let mut keep = Vec::new();
        if let Some(a) = prev { let v = part(0.0, a); if length(&v) > 1e-4 { keep.push(v); } }
        if let Some(b) = next { let v = part(b, end); if length(&v) > 1e-4 { keep.push(v); } }
        Some(TrimPlan { remove, keep })
    }
}

/// The shapes that replace the trimmed one: open paths that keep its settings (operation, drawing).
pub fn trimmed_shapes(l: &LoopData, plan: &TrimPlan) -> Vec<LoopData> {
    plan.keep.iter().map(|v| {
        let mut r = l.clone();
        r.c = 0;
        r.rc = None; r.cr = None; r.el = None;
        r.pts = v.iter().flat_map(|q| [to_i(q[0]), to_i(q[1])]).collect();
        r
    }).collect()
}

/// What an extend click would do: the end moves from `from` to `to`.
pub struct ExtendPlan { pub from: P, pub to: P, pub at_end: bool }

pub fn plan_extend(loops: &[LoopData], target: usize, at: P) -> Option<ExtendPlan> {
    let l = loops.get(target).filter(|l| usable(l) && !l.closed())?;
    let p = pts(l);
    let n = p.len();
    if n < 2 { return None; }
    // the end nearer the click
    let at_end = (at[0] - p[n - 1][0]).hypot(at[1] - p[n - 1][1]) <= (at[0] - p[0][0]).hypot(at[1] - p[0][1]);
    let (tip, mut back) = if at_end { (p[n - 1], n - 2) } else { (p[0], 1) };
    // direction of the last stretch of any length
    loop {
        if (p[back][0] - tip[0]).hypot(p[back][1] - tip[1]) > 1e-9 { break; }
        if at_end { if back == 0 { return None; } back -= 1; } else { if back + 1 >= n { return None; } back += 1; }
    }
    let (dx, dy) = (tip[0] - p[back][0], tip[1] - p[back][1]);
    let dl = dx.hypot(dy);
    let d = [dx / dl, dy / dl];
    let far = [tip[0] + d[0] * 100000.0, tip[1] + d[1] * 100000.0];
    let mut best: Option<f64> = None;
    for (j, m) in loops.iter().enumerate() {
        if !usable(m) { continue; }
        let q = pts(m);
        let qn = q.len();
        for k in 0..nseg(qn, m.closed()) {
            // the segment the tip sits on is not an obstacle to itself
            if j == target && ((at_end && k + 2 >= qn) || (!at_end && k < 1)) { continue; }
            if let Some((t, _)) = cross(tip, far, q[k], q[(k + 1) % qn]) {
                let dist = t * 100000.0;
                if dist > 1e-6 && best.map_or(true, |b| dist < b) { best = Some(dist); }
            }
        }
    }
    let dist = best?;
    Some(ExtendPlan { from: tip, to: [tip[0] + d[0] * dist, tip[1] + d[1] * dist], at_end })
}

pub fn extended_shape(l: &LoopData, plan: &ExtendPlan) -> LoopData {
    let mut r = l.clone();
    r.rc = None; r.cr = None; r.el = None;
    let to = [to_i(plan.to[0]), to_i(plan.to[1])];
    let n = r.pts.len() / 2;
    // the tip moves out along its own direction, so the last segment just gets longer
    let i = if plan.at_end { n - 1 } else { 0 };
    r.pts[i * 2] = to[0];
    r.pts[i * 2 + 1] = to[1];
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::{new_poly, new_rect};

    fn line(a: P, b: P) -> LoopData { new_poly(&[a, b], false, 0).unwrap() }

    #[test]
    fn trimming_the_middle_of_a_line_splits_it() {
        // a horizontal line crossed by two verticals
        let loops = vec![line([0.0, 0.0], [100.0, 0.0]), line([30.0, -10.0], [30.0, 10.0]), line([70.0, -10.0], [70.0, 10.0])];
        let plan = plan_trim(&loops, 0, [50.0, 0.0]).unwrap();
        assert_eq!(plan.remove.len(), 2);
        assert!((plan.remove[0][0] - 30.0).abs() < 1e-6 && (plan.remove[1][0] - 70.0).abs() < 1e-6);
        let out = trimmed_shapes(&loops[0], &plan);
        assert_eq!(out.len(), 2);
        assert_eq!(pts(&out[0]), vec![[0.0, 0.0], [30.0, 0.0]]);
        assert_eq!(pts(&out[1]), vec![[70.0, 0.0], [100.0, 0.0]]);
    }

    #[test]
    fn trimming_an_end_keeps_the_rest() {
        let loops = vec![line([0.0, 0.0], [100.0, 0.0]), line([30.0, -10.0], [30.0, 10.0])];
        let plan = plan_trim(&loops, 0, [10.0, 0.0]).unwrap();
        let out = trimmed_shapes(&loops[0], &plan);
        assert_eq!(out.len(), 1);
        assert_eq!(pts(&out[0]), vec![[30.0, 0.0], [100.0, 0.0]]);
        // nothing crosses a lone line
        assert!(plan_trim(&loops[..1], 0, [10.0, 0.0]).is_none());
    }

    #[test]
    fn trimming_a_closed_shape_opens_it() {
        // a rectangle 0..100 x 0..50 with a vertical line through x = 30 and x = 70
        let loops = vec![new_rect([0.0, 0.0], [100.0, 50.0], 0.0, 0), line([30.0, -10.0], [30.0, 60.0]), line([70.0, -10.0], [70.0, 60.0])];
        // click on the bottom edge between the two lines
        let plan = plan_trim(&loops, 0, [50.0, 0.0]).unwrap();
        assert!((plan.remove[0][0] - 30.0).abs() < 1e-6 || (plan.remove[0][0] - 70.0).abs() < 1e-6);
        let out = trimmed_shapes(&loops[0], &plan);
        assert_eq!(out.len(), 1);
        assert!(!out[0].closed());
        let kept = pts(&out[0]);
        // the long way round: from one cut up the side, across the top, down the other side and back along the bottom
        assert!(length(&kept) > 100.0 + 50.0 * 2.0 + 60.0 - 1.0, "{}", length(&kept));
        assert!(kept.iter().all(|q| !(q[1].abs() < 1e-9 && q[0] > 30.0 + 1e-6 && q[0] < 70.0 - 1e-6)), "{:?}", kept);
        // a single crossing cannot trim a closed shape
        let one = vec![new_rect([0.0, 0.0], [100.0, 50.0], 0.0, 0), line([30.0, -10.0], [30.0, 25.0])];
        assert!(plan_trim(&one, 0, [50.0, 0.0]).is_none());
    }

    #[test]
    fn extending_runs_to_the_next_shape() {
        let loops = vec![line([0.0, 0.0], [40.0, 0.0]), line([100.0, -20.0], [100.0, 20.0]), line([60.0, 5.0], [60.0, 20.0])];
        let plan = plan_extend(&loops, 0, [38.0, 1.0]).unwrap();
        assert!((plan.to[0] - 100.0).abs() < 1e-6 && plan.to[1].abs() < 1e-6, "{:?}", plan.to); // the line at x=60 does not reach y=0
        let r = extended_shape(&loops[0], &plan);
        assert_eq!(pts(&r), vec![[0.0, 0.0], [100.0, 0.0]]);
        // the other end runs the other way and finds nothing
        assert!(plan_extend(&loops, 0, [1.0, 0.0]).is_none());
        // closed shapes do not extend
        assert!(plan_extend(&[new_rect([0.0, 0.0], [10.0, 10.0], 0.0, 0)], 0, [0.0, 0.0]).is_none());
    }

    #[test]
    fn a_shape_crossing_itself_can_be_trimmed() {
        // a loop-the-loop: the path crosses itself at (50, 0)
        let l = new_poly(&[[0.0, 0.0], [100.0, 0.0], [100.0, 50.0], [50.0, 50.0], [50.0, -20.0]], false, 0).unwrap();
        let loops = vec![l];
        let plan = plan_trim(&loops, 0, [100.0, 25.0]).unwrap();
        // the click is on the loop; it goes away leaving the two tails
        assert_eq!(plan.keep.len(), 2);
    }
}
