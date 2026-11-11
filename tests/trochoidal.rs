//! Trochoidal clearing: the pocket is still cleared completely, and the tool never leaves the pocket.
use putiacnc_core::shapes;
use putiacnc_core::{generate, load_project, Units};
use std::collections::HashMap;
use std::path::PathBuf;

fn dist_seg(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dy * t)
}
fn inside(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut c = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        if (poly[i][1] > p[1]) != (poly[j][1] > p[1]) && p[0] < (poly[j][0] - poly[i][0]) * (p[1] - poly[i][1]) / (poly[j][1] - poly[i][1]) + poly[i][0] { c = !c; }
        j = i;
    }
    c
}
fn edge_dist(poly: &[[f64; 2]], p: [f64; 2]) -> f64 {
    (0..poly.len()).map(|i| dist_seg(p, poly[i], poly[(i + 1) % poly.len()])).fold(f64::INFINITY, f64::min)
}

fn cut_positions(lines: &[String]) -> Vec<[f64; 2]> {
    let (mut x, mut y, mut out) = (0.0_f64, 0.0_f64, vec![]);
    let mut cutting = false;
    for l in lines {
        let g1 = l.starts_with("G1 ");
        let arc = l.starts_with("G2 ") || l.starts_with("G3 ");
        let w = |c: char| l.split(' ').find(|t| t.starts_with(c)).and_then(|t| t[1..].parse::<f64>().ok());
        let (nx, ny) = (w('X').unwrap_or(x), w('Y').unwrap_or(y));
        if arc {
            // sample the arc
            let (cx, cy) = (x + w('I').unwrap_or(0.0), y + w('J').unwrap_or(0.0));
            let r = (x - cx).hypot(y - cy);
            let (a0, mut a1) = ((y - cy).atan2(x - cx), (ny - cy).atan2(nx - cx));
            let ccw = l.starts_with("G3 ");
            if ccw { while a1 <= a0 + 1e-9 { a1 += std::f64::consts::TAU; } } else { while a1 >= a0 - 1e-9 { a1 -= std::f64::consts::TAU; } }
            let n = ((a1 - a0).abs() * r / 0.2).ceil().max(1.0) as usize;
            if cutting { for k in 1..=n { let a = a0 + (a1 - a0) * k as f64 / n as f64; out.push([cx + r * a.cos(), cy + r * a.sin()]); } }
            x = nx; y = ny;
        } else if l.starts_with("G0 ") || g1 {
            if g1 && cutting {
                let n = (((nx - x).hypot(ny - y)) / 0.2).ceil().max(1.0) as usize;
                for k in 1..=n { let t = k as f64 / n as f64; out.push([x + (nx - x) * t, y + (ny - y) * t]); }
            }
            if l.starts_with("G0 Z") { cutting = false; }
            if g1 && (w('Z').is_some_and(|z| z < 0.0) || cutting) { cutting = true; }
            x = nx; y = ny;
        }
    }
    out
}

fn missed_points(pos: &[[f64; 2]], polys: &[Vec<[f64; 2]>]) -> (usize, usize) {
    let radius = 3.0;
    let mut cells: HashMap<(i64, i64), Vec<[f64; 2]>> = HashMap::new();
    for p in pos { cells.entry(((p[0] / 3.0).floor() as i64, (p[1] / 3.0).floor() as i64)).or_default().push(*p); }
    let (mut missed, mut total) = (0, 0);
    let mut y = 20.5;
    while y < 69.5 {
        let mut x = 20.5;
        while x < 89.5 {
            let q = [x, y];
            if polys.iter().filter(|pl| inside(pl, q)).count() == 1 {
                total += 1;
                let (cx, cy) = ((x / 3.0).floor() as i64, (y / 3.0).floor() as i64);
                let near = (-2..=2).any(|dx| (-2..=2).any(|dy| cells.get(&(cx + dx, cy + dy)).is_some_and(|v| v.iter().any(|p| (p[0] - x).hypot(p[1] - y) <= radius + 0.05))));
                if !near { missed += 1; if std::env::var_os("TROCH_DEBUG").is_some() { eprintln!("MISS {x} {y}"); } }
            }
            x += 1.0;
        }
        y += 1.0;
    }
    (missed, total)
}

fn run(troch: bool) -> (Vec<String>, Vec<Vec<[f64; 2]>>) {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/pocket_island.project.json");
    let mut proj = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
    for o in proj.data.ops.iter_mut() { o.p.troch = troch; o.p.ramp = false; o.p.depth = 1.5; o.p.pass_depth = Some(1.5); }
    let polys: Vec<Vec<[f64; 2]>> = proj.data.loops.iter().map(shapes::points).collect();
    (generate(&proj.data, Units::Metric).unwrap().lines, polys)
}

#[test]
fn the_pocket_is_cleared_completely_and_the_tool_stays_inside() {
    let (lines, polys) = run(true);
    assert!(lines.iter().filter(|l| l.starts_with("G1 X")).count() > 300, "the loops are cut");
    let pos = cut_positions(&lines);
    let radius = 3.0;
    // the tool stays inside the pocket outline and out of the island
    for p in pos.iter().step_by(7) {
        let ok = polys.iter().all(|pl| edge_dist(pl, *p) >= radius - 0.02) && polys.iter().filter(|pl| inside(pl, *p)).count() == 1;
        assert!(ok, "tool centre {p:?} is too close to a wall");
    }
    let (missed, total) = missed_points(&pos, &polys);
    let (plain_pos, _) = { let (l, p) = run(false); (cut_positions(&l), p) };
    let (plain_missed, _) = missed_points(&plain_pos, &polys);
    // the corners of the outline cannot be reached by a round tool, whatever the path: plain rings leave the same
    assert!(total > 1000 && missed <= plain_missed, "{missed} points of {total} not reached, plain rings leave {plain_missed}");
}

#[test]
fn plain_rings_are_unchanged_when_the_option_is_off() {
    let (a, _) = run(false);
    assert!(!a.is_empty());
    assert!(a.iter().filter(|l| l.starts_with("G1 X")).count() < 1500);
}

#[test]
fn a_narrow_slot_is_cleared_by_loops_alone() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/pocket_island.project.json");
    let mut proj = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
    let op = proj.data.loops[0].op;
    // 9 mm wide for a 6 mm tool: the tool can move only 3 mm sideways
    proj.data.loops = vec![shapes::new_rect([20.0, 20.0], [29.0, 60.0], 0.0, op)];
    for o in proj.data.ops.iter_mut() { o.p.troch = true; o.p.ramp = false; o.p.depth = 1.5; o.p.pass_depth = Some(1.5); }
    let polys: Vec<Vec<[f64; 2]>> = proj.data.loops.iter().map(shapes::points).collect();
    let lines = generate(&proj.data, Units::Metric).unwrap().lines;
    let pos = cut_positions(&lines);
    assert!(lines.iter().filter(|l| l.starts_with("G3 ")).count() > 100, "the slot is cut with loops");
    let (missed, total) = missed_points(&pos, &polys);
    // the four corners of the slot are out of reach of a round tool, as with plain rings
    assert!(total > 300 && missed <= 12, "{missed} of {total} not reached");
    assert!(pos.iter().all(|p| polys.iter().all(|pl| edge_dist(pl, *p) >= 2.98)), "the tool stays inside the slot");
}
