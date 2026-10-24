//! Drawing cleanup for imported files: join paths that were drawn as separate pieces, drop duplicate
//! and degenerate shapes, and find the open contours that would stop a pocket from being cut.
use crate::model::LoopData;
use crate::shapes::S;

type P = [f64; 2];

fn pts(l: &LoopData) -> Vec<P> { l.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect() }
fn dist(a: P, b: P) -> f64 { (a[0] - b[0]).hypot(a[1] - b[1]) }

/// A plain polyline that cleanup may change: not a rectangle, circle, ellipse or drill point.
fn plain(l: &LoopData) -> bool { l.pt.is_none() && l.rc.is_none() && l.cr.is_none() && l.el.is_none() }

/// Open paths (with at least two points) that are not drill points.
fn is_open_path(l: &LoopData) -> bool { l.pt.is_none() && !l.closed() && l.pts.len() >= 4 }

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Report {
    /// Open pieces that were merged into longer paths (counting each piece that disappeared).
    pub joined: usize,
    /// Paths that ended up closed.
    pub closed: usize,
    pub duplicates: usize,
    pub tiny: usize,
    /// Open contours left afterwards.
    pub open_left: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Options { pub join: bool, pub duplicates: bool, pub tiny: bool, pub tol: f64 }

/// The loose ends of the open paths among `loops`: `(shape index, first or last point, position, joinable)`,
/// where `joinable` means another open end lies within `tol` of it (so Join would connect them).
pub fn loose_ends(loops: &[LoopData], tol: f64) -> Vec<(usize, bool, P, bool)> {
    let mut ends: Vec<(usize, bool, P)> = Vec::new();
    for (i, l) in loops.iter().enumerate() {
        if !is_open_path(l) { continue; }
        let v = pts(l);
        ends.push((i, false, v[0]));
        ends.push((i, true, *v.last().unwrap()));
    }
    ends.iter().enumerate().map(|(k, &(i, last, p))| {
        let join = ends.iter().enumerate().any(|(m, &(j, jl, q))| m != k && !(i == j && last == jl) && dist(p, q) <= tol);
        (i, last, p, join)
    }).collect()
}

/// Number of open contours among `loops`.
pub fn open_count(loops: &[LoopData]) -> usize { loops.iter().filter(|l| is_open_path(l)).count() }

fn key(l: &LoopData) -> Vec<i64> {
    let n = l.pts.len() / 2;
    let p: Vec<[i64; 2]> = l.pts.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
    let flat = |v: &[[i64; 2]]| v.iter().flat_map(|q| [q[0], q[1]]).collect::<Vec<i64>>();
    let rev: Vec<[i64; 2]> = p.iter().rev().copied().collect();
    if l.closed() && n > 0 {
        // the same outline whichever point it starts at and whichever way it runs
        let norm = |v: &[[i64; 2]]| {
            let k = (0..v.len()).min_by_key(|&i| v[i]).unwrap();
            let r: Vec<[i64; 2]> = v[k..].iter().chain(v[..k].iter()).copied().collect();
            flat(&r)
        };
        let (a, b) = (norm(&p), norm(&rev));
        let mut k = vec![1];
        k.extend(if a <= b { a } else { b });
        k
    } else {
        let (a, b) = (flat(&p), flat(&rev));
        let mut k = vec![0];
        k.extend(if a <= b { a } else { b });
        k
    }
}

fn outline_length(l: &LoopData) -> f64 {
    let v = pts(l);
    let mut t: f64 = v.windows(2).map(|w| dist(w[0], w[1])).sum();
    if l.closed() && v.len() > 1 { t += dist(v[v.len() - 1], v[0]); }
    t
}

/// Cleans the shapes whose indices are in `scope` (all of them when `None`). Returns the new list of shapes and the
/// indices (in the old list) of the shapes that are gone, for fixing up references such as holding tabs.
pub fn clean(loops: &[LoopData], scope: Option<&std::collections::BTreeSet<usize>>, o: Options) -> (Vec<LoopData>, Vec<usize>, Report) {
    let n = loops.len();
    let in_scope = |i: usize| scope.is_none_or(|s| s.contains(&i));
    let mut out: Vec<Option<LoopData>> = loops.iter().cloned().map(Some).collect();
    let mut report = Report::default();

    // degenerate shapes: no length at all (or shorter than the tolerance), and repeated points
    if o.tiny {
        for i in 0..n {
            if !in_scope(i) { continue; }
            let Some(l) = out[i].as_mut() else { continue };
            if l.pt.is_some() { continue; }
            if plain(l) {
                // drop points that sit on top of the previous one
                let v = pts(l);
                let mut keep: Vec<P> = Vec::with_capacity(v.len());
                for q in v { if keep.last().is_none_or(|p| dist(*p, q) > 1e-6) { keep.push(q); } }
                if l.closed() && keep.len() > 1 && dist(keep[0], *keep.last().unwrap()) <= 1e-6 { keep.pop(); }
                if keep.len() * 2 != l.pts.len() { l.pts = keep.iter().flat_map(|q| [(q[0] * S).round() as i64, (q[1] * S).round() as i64]).collect(); }
            }
            let degenerate = l.pts.len() < 4 || (l.closed() && l.pts.len() < 6) || outline_length(l) < o.tol.max(1e-6);
            if degenerate { out[i] = None; report.tiny += 1; }
        }
    }

    // the same shape drawn twice: the later copy goes
    if o.duplicates {
        let mut seen: std::collections::HashSet<Vec<i64>> = std::collections::HashSet::new();
        for i in 0..n {
            let Some(l) = out[i].as_ref() else { continue };
            if l.pt.is_some() || l.pts.is_empty() { continue; }
            let k = key(l);
            if !seen.insert(k) && in_scope(i) { out[i] = None; report.duplicates += 1; }
        }
    }

    // join open paths whose ends meet
    if o.join {
        let ids: Vec<usize> = (0..n).filter(|&i| in_scope(i) && out[i].as_ref().is_some_and(|l| plain(l) && is_open_path(l))).collect();
        let mut paths: Vec<Option<(Vec<P>, usize)>> = ids.iter().map(|&i| Some((pts(out[i].as_ref().unwrap()), i))).collect();
        let m = paths.len();
        let mut closed_set: Vec<usize> = Vec::new();
        for a in 0..m {
            loop {
                let Some((pa, ia)) = paths[a].clone() else { break };
                if closed_set.contains(&ia) { break; }
                let tail = *pa.last().unwrap();
                // a path whose two ends meet is closed
                if pa.len() >= 3 && dist(pa[0], tail) <= o.tol {
                    closed_set.push(ia);
                    break;
                }
                // otherwise the nearest end of another path within the tolerance continues it
                let mut best: Option<(usize, bool, f64)> = None; // path, true when its last point is the match
                for b in 0..m {
                    if b == a { continue; }
                    let Some((pb, jb)) = &paths[b] else { continue };
                    if closed_set.contains(jb) { continue; }
                    let (d0, d1) = (dist(tail, pb[0]), dist(tail, *pb.last().unwrap()));
                    let (d, rev) = if d0 <= d1 { (d0, false) } else { (d1, true) };
                    if d <= o.tol && best.is_none_or(|(_, _, bd)| d < bd) { best = Some((b, rev, d)); }
                }
                // or its own start continues it (the pieces are in the other order)
                let Some((b, rev, _)) = best else { break };
                let (mut pb, _) = paths[b].take().unwrap();
                if rev { pb.reverse(); }
                let mut merged = pa;
                // the shared point is kept once
                // the two ends are one point: the first piece's stays
                merged.extend(pb.into_iter().skip(1));
                paths[a] = Some((merged, ia));
                report.joined += 1;
            }
        }
        // a path can also continue at its start: run the same joining on reversed paths once more
        for a in 0..m {
            let Some((mut pa, ia)) = paths[a].clone() else { continue };
            if closed_set.contains(&ia) { continue; }
            pa.reverse();
            paths[a] = Some((pa, ia));
        }
        for a in 0..m {
            loop {
                let Some((pa, ia)) = paths[a].clone() else { break };
                if closed_set.contains(&ia) { break; }
                let tail = *pa.last().unwrap();
                if pa.len() >= 3 && dist(pa[0], tail) <= o.tol { closed_set.push(ia); break; }
                let mut best: Option<(usize, bool, f64)> = None;
                for b in 0..m {
                    if b == a { continue; }
                    let Some((pb, jb)) = &paths[b] else { continue };
                    if closed_set.contains(jb) { continue; }
                    let (d0, d1) = (dist(tail, pb[0]), dist(tail, *pb.last().unwrap()));
                    let (d, rev) = if d0 <= d1 { (d0, false) } else { (d1, true) };
                    if d <= o.tol && best.is_none_or(|(_, _, bd)| d < bd) { best = Some((b, rev, d)); }
                }
                let Some((b, rev, _)) = best else { break };
                let (mut pb, _) = paths[b].take().unwrap();
                if rev { pb.reverse(); }
                let mut merged = pa;
                // the two ends are one point: the first piece's stays
                merged.extend(pb.into_iter().skip(1));
                paths[a] = Some((merged, ia));
                report.joined += 1;
            }
        }
        // write the results back: the first piece's slot keeps the merged path, the others are gone
        for (k, &i) in ids.iter().enumerate() {
            match paths[k].take() {
                None => { out[i] = None; }
                Some((mut v, _)) => {
                    let l = out[i].as_mut().unwrap();
                    let close = v.len() >= 3 && dist(v[0], *v.last().unwrap()) <= o.tol;
                    if close { v.pop(); l.c = 1; report.closed += 1; }
                    l.pts = v.iter().flat_map(|q| [(q[0] * S).round() as i64, (q[1] * S).round() as i64]).collect();
                }
            }
        }
    }

    let removed: Vec<usize> = (0..n).filter(|&i| out[i].is_none()).collect();
    let res: Vec<LoopData> = out.into_iter().flatten().collect();
    report.open_left = res.iter().filter(|l| is_open_path(l)).count();
    (res, removed, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::{new_poly, new_rect};

    fn line(a: P, b: P) -> LoopData { new_poly(&[a, b], false, 0).unwrap() }
    fn opts(tol: f64) -> Options { Options { join: true, duplicates: true, tiny: true, tol } }

    #[test]
    fn four_lines_become_a_closed_rectangle() {
        // drawn in a jumble, with 0.02 mm gaps and one piece reversed
        let l = vec![
            line([0.0, 0.0], [50.0, 0.0]),
            line([50.02, 0.0], [50.0, 30.0]),
            line([0.0, 30.0], [50.0, 30.01]),
            line([0.01, 30.0], [0.0, 0.0]),
        ];
        assert_eq!(open_count(&l), 4);
        let (r, removed, rep) = clean(&l, None, opts(0.1));
        assert_eq!(r.len(), 1, "{:?}", rep);
        assert!(r[0].closed());
        assert_eq!(removed.len(), 3);
        assert_eq!((rep.joined, rep.closed, rep.open_left), (3, 1, 0));
        // 4 corners
        assert_eq!(r[0].pts.len() / 2, 4, "{:?}", pts(&r[0]));
    }

    #[test]
    fn gaps_wider_than_the_tolerance_stay_open() {
        let l = vec![line([0.0, 0.0], [10.0, 0.0]), line([10.5, 0.0], [20.0, 0.0])];
        let (r, _, rep) = clean(&l, None, opts(0.1));
        assert_eq!(r.len(), 2);
        assert_eq!(rep.open_left, 2);
        // with a wider tolerance they join into one path
        let (r, _, rep) = clean(&l, None, opts(1.0));
        assert_eq!(r.len(), 1);
        assert_eq!(rep.open_left, 1);
        assert_eq!(r[0].pts.len() / 2, 3); // the two ends within the tolerance are one point
    }

    #[test]
    fn duplicates_and_degenerate_shapes_go() {
        let rect = new_rect([0.0, 0.0], [10.0, 10.0], 0.0, 0);
        let mut again = rect.clone();
        // same outline, started elsewhere and run backwards
        let mut v = pts(&again); v.rotate_left(2); v.reverse();
        again.pts = v.iter().flat_map(|q| [(q[0] * S).round() as i64, (q[1] * S).round() as i64]).collect();
        let mut dot = line([5.0, 5.0], [6.0, 5.0]);
        dot.pts = vec![5000, 5000, 5000, 5000];
        let l = vec![rect, again, dot, line([0.0, 20.0], [10.0, 20.0]), line([10.0, 20.0], [0.0, 20.0])];
        let (r, removed, rep) = clean(&l, None, opts(0.05));
        assert_eq!((rep.duplicates, rep.tiny), (2, 1), "{:?}", rep);
        assert_eq!(r.len(), 2);
        assert_eq!(removed, vec![1, 2, 4]);
    }

    #[test]
    fn loose_ends_tell_joinable_from_lost() {
        let l = vec![line([0.0, 0.0], [10.0, 0.0]), line([10.05, 0.0], [10.0, 10.0]), new_rect([50.0, 50.0], [60.0, 60.0], 0.0, 0)];
        let e = loose_ends(&l, 0.1);
        assert_eq!(e.len(), 4); // closed shapes have no ends
        let joinable = e.iter().filter(|x| x.3).count();
        assert_eq!(joinable, 2);
    }

    #[test]
    fn scope_limits_the_cleanup() {
        let l = vec![line([0.0, 0.0], [10.0, 0.0]), line([10.0, 0.0], [10.0, 10.0]), line([100.0, 0.0], [110.0, 0.0]), line([110.0, 0.0], [110.0, 10.0])];
        let scope: std::collections::BTreeSet<usize> = [0usize, 1].into_iter().collect();
        let (r, removed, _) = clean(&l, Some(&scope), opts(0.1));
        assert_eq!(r.len(), 3);
        assert_eq!(removed, vec![1]);
    }
}
