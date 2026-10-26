//! The Trim and Extend tool.
use super::*;

pub(super) enum TrimHover { Trim(usize, putiacnc_core::trim::TrimPlan), Extend(usize, putiacnc_core::trim::ExtendPlan), Nothing }

/// What a click at `w` would trim or extend: the shape nearest the pointer (open ones only when extending).
pub(super) fn trim_hover(app: &App, w: [f64; 2], tol: f64) -> Option<TrimHover> {
    let loops = &app.project.data.loops;
    let t = loops.iter().enumerate()
        .filter(|(_, l)| !shapes::is_point(l) && (!app.trim_extend || !l.closed()))
        .map(|(i, l)| (i, shapes::dist_to_loop(l, w)))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(i, _)| i)?;
    Some(if app.trim_extend {
        match putiacnc_core::trim::plan_extend(loops, t, w) { Some(p) => TrimHover::Extend(t, p), None => TrimHover::Nothing }
    } else {
        match putiacnc_core::trim::plan_trim(loops, t, w) { Some(p) => TrimHover::Trim(t, p), None => TrimHover::Nothing }
    })
}

/// After a trim, the pieces whose ends meet (the cut points of two shapes trimmed against each other)
/// become one path, closed when it comes round, and the result is selected, so what you see is what you get.
pub(super) fn join_after_trim(app: &mut App, t: usize, n_before: usize) {
    let n = app.project.data.loops.len();
    // the pieces made now: the one in the shape's place (if the shape is still there) and the appended ones
    let mut made: std::collections::BTreeSet<usize> = (n_before..n).collect();
    if n >= n_before { made.insert(t); }
    made.retain(|&i| i < n);
    let tol = 0.01;
    let loops = &app.project.data.loops;
    let ends = |l: &putiacnc_core::model::LoopData| -> Vec<[f64; 2]> {
        if l.closed() || l.pts.len() < 4 || l.pt.is_some() { return Vec::new(); }
        let k = l.pts.len();
        vec![[l.pts[0] as f64 / shapes::S, l.pts[1] as f64 / shapes::S], [l.pts[k - 2] as f64 / shapes::S, l.pts[k - 1] as f64 / shapes::S]]
    };
    let mine: Vec<[f64; 2]> = made.iter().flat_map(|&i| ends(&loops[i])).collect();
    let mut scope = made.clone();
    for (j, l) in loops.iter().enumerate() {
        if made.contains(&j) { continue; }
        if ends(l).iter().any(|e| mine.iter().any(|m| (m[0] - e[0]).hypot(m[1] - e[1]) <= tol)) { scope.insert(j); }
    }
    let opts = putiacnc_core::cleanup::Options { join: true, duplicates: false, tiny: false, tol };
    let (res, removed, _) = putiacnc_core::cleanup::clean(&app.project.data.loops, Some(&scope), opts);
    let remap = |i: usize| i - removed.iter().filter(|&&r| r < i).count();
    app.sel = made.iter().filter(|i| !removed.contains(i)).map(|&i| remap(i)).collect();
    if res != app.project.data.loops {
        shapes::tabs_after_delete(&mut app.project.data.ops, &removed);
        app.project.data.loops = res;
    }
}

pub(super) fn trim_click(app: &mut App, w: [f64; 2], tol: f64) {
    match trim_hover(app, w, tol) {
        Some(TrimHover::Trim(t, plan)) => {
            let pieces = putiacnc_core::trim::trimmed_shapes(&app.project.data.loops[t], &plan);
            app.hist.record(&app.project.data);
            let n_before = app.project.data.loops.len();
            let mut it = pieces.into_iter();
            match it.next() {
                // the first piece takes the shape's place, so every other shape keeps its number
                Some(first) => { app.project.data.loops[t] = first; app.project.data.loops.extend(it); }
                None => { app.project.data.loops.remove(t); shapes::tabs_after_delete(&mut app.project.data.ops, &[t]); }
            }
            join_after_trim(app, t, n_before);
            app.touch();
        }
        Some(TrimHover::Extend(t, plan)) => {
            let r = putiacnc_core::trim::extended_shape(&app.project.data.loops[t], &plan);
            app.hist.record(&app.project.data);
            app.project.data.loops[t] = r;
            app.sel.clear();
            app.touch();
        }
        _ => {}
    }
}
