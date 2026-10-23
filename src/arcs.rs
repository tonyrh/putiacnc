//! Arc output: runs of short straight cutting moves that lie on a circle are written as one G2/G3 line.
//!
//! The G-code is rewritten after it is generated, line by line, in the output units. Only runs of `G1` moves that change X and Y
//! alone are looked at; everything else (rapids, plunges, ramps, spindle and comment lines) passes through unchanged. A run becomes an
//! arc only when its points sit on one circle within a very small tolerance AND each straight piece is already a fine approximation
//! of that circle, so a coarse polygon (a hexagon, say) is never turned into a circle.

/// Points must lie this close to the fitted circle (mm).
const ON_CIRCLE: f64 = 0.005;
/// Largest gap between a straight piece and the circle it stands for (mm): the app draws circles with short chords, so this lets a
/// drawn circle become an arc while a deliberate polygon stays straight.
const MAX_SAGITTA: f64 = 0.12;
/// What a Grbl-type controller accepts for the difference between the start and end radius (mm); we stay well inside it.
const MAX_RADIUS_GAP: f64 = 0.004;
const MIN_RADIUS: f64 = 0.05;
const MAX_RADIUS: f64 = 1000.0;

fn words(line: &str) -> Vec<(char, &str)> {
    line.split_whitespace().filter_map(|w| { let mut c = w.chars(); c.next().map(|k| (k, c.as_str())) }).collect()
}

/// X/Y of the move on this line, if it is a plain linear or arc move word set.
fn xy(line: &str) -> (Option<f64>, Option<f64>) {
    if line.starts_with('(') { return (None, None); }
    let mut x = None; let mut y = None;
    for (k, v) in words(line) {
        match k { 'X' => x = v.parse().ok(), 'Y' => y = v.parse().ok(), _ => {} }
    }
    (x, y)
}

/// A `G1` line that changes only X and/or Y (an F word is allowed): the point it goes to, and its F text.
fn plain_cut(line: &str, pos: [f64; 2]) -> Option<([f64; 2], Option<String>)> {
    let mut it = line.split_whitespace();
    if it.next()? != "G1" { return None; }
    let (mut x, mut y, mut f) = (pos[0], pos[1], None);
    let mut moved = false;
    for w in it {
        let (k, v) = w.split_at(1);
        match k {
            "X" => { x = v.parse().ok()?; moved = true; }
            "Y" => { y = v.parse().ok()?; moved = true; }
            "F" => f = Some(v.to_string()),
            _ => return None,
        }
    }
    if moved { Some(([x, y], f)) } else { None }
}

fn circle3(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Option<([f64; 2], f64)> {
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    if d.abs() < 1e-12 { return None; }
    let (a2, b2, c2) = (a[0] * a[0] + a[1] * a[1], b[0] * b[0] + b[1] * b[1], c[0] * c[0] + c[1] * c[1]);
    let ux = (a2 * (b[1] - c[1]) + b2 * (c[1] - a[1]) + c2 * (a[1] - b[1])) / d;
    let uy = (a2 * (c[0] - b[0]) + b2 * (a[0] - c[0]) + c2 * (b[0] - a[0])) / d;
    let r = ((a[0] - ux).powi(2) + (a[1] - uy).powi(2)).sqrt();
    Some(([ux, uy], r))
}

/// Does `p[0..=n]` form one arc of at most half a circle? Returns (centre, clockwise).
fn fit(p: &[[f64; 2]], k: f64) -> Option<([f64; 2], bool)> {
    let n = p.len();
    if n < 4 { return None; }
    let (c, r) = circle3(p[0], p[n / 2], p[n - 1])?;
    if r < MIN_RADIUS * k || r > MAX_RADIUS * k { return None; }
    let mut sign = 0.0_f64;
    let mut sweep = 0.0_f64;
    for i in 0..n {
        let d = ((p[i][0] - c[0]).powi(2) + (p[i][1] - c[1]).powi(2)).sqrt();
        if (d - r).abs() > ON_CIRCLE * k { return None; }
        if i + 1 < n {
            let l = ((p[i + 1][0] - p[i][0]).powi(2) + (p[i + 1][1] - p[i][1]).powi(2)).sqrt();
            if l < 1e-9 { return None; }
            let sag = r - (r * r - l * l / 4.0).max(0.0).sqrt();
            if sag > MAX_SAGITTA * k { return None; }
            let a0 = (p[i][1] - c[1]).atan2(p[i][0] - c[0]);
            let a1 = (p[i + 1][1] - c[1]).atan2(p[i + 1][0] - c[0]);
            let mut da = a1 - a0;
            while da > std::f64::consts::PI { da -= 2.0 * std::f64::consts::PI; }
            while da < -std::f64::consts::PI { da += 2.0 * std::f64::consts::PI; }
            if da == 0.0 { return None; }
            if sign == 0.0 { sign = da.signum(); } else if da.signum() != sign { return None; }
            sweep += da;
        }
    }
    if sweep.abs() > std::f64::consts::PI + 1e-9 { return None; }
    Some((c, sign < 0.0))
}

fn fixed(v: f64, dec: usize) -> f64 { let f = 10f64.powi(dec as i32); (v * f).round() / f }
fn txt(v: f64) -> String { let v = if v == 0.0 { 0.0 } else { v }; format!("{}", v) }

/// Rewrite `lines` with arcs. `dec` is the number of decimals of the output, `k` the length of one output unit in mm (1 for mm, 25.4 for
/// inches). Returns the new lines and how many arcs were written.
pub fn fit_arcs(lines: &[String], dec: usize, k: f64) -> (Vec<String>, usize) {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut pos: Option<[f64; 2]> = None;
    let mut arcs = 0;
    let mut i = 0;
    while i < lines.len() {
        // collect a run of plain cutting moves starting here
        let mut run: Vec<([f64; 2], Option<String>)> = vec![];
        if let Some(start) = pos {
            let mut cur = start;
            let mut j = i;
            while j < lines.len() {
                match plain_cut(&lines[j], cur) {
                    // a feed change in the middle of a run starts a new run
                    Some((pt, f)) if !(j > i && f.is_some()) => { run.push((pt, f)); cur = pt; j += 1; }
                    _ => break,
                }
            }
        }
        if run.len() < 3 {
            let l = &lines[i];
            let (x, y) = xy(l);
            let mut p = pos.unwrap_or([f64::NAN, f64::NAN]);
            if x.is_some() || y.is_some() {
                if let Some(x) = x { p[0] = x; }
                if let Some(y) = y { p[1] = y; }
                if p[0].is_finite() && p[1].is_finite() { pos = Some(p); }
            }
            out.push(l.clone());
            i += 1;
            continue;
        }
        let start = pos.unwrap();
        let mut pts: Vec<[f64; 2]> = vec![start];
        pts.extend(run.iter().map(|r| r.0));
        // greedy: the longest arc from point `a`
        let mut a = 0;
        while a + 1 < pts.len() {
            let mut best: Option<(usize, [f64; 2], bool)> = None;
            let mut b = a + 3;
            while b < pts.len() {
                match fit(&pts[a..=b], k) { Some((c, cw)) => { best = Some((b, c, cw)); b += 1; } None => break }
            }
            let mut written = false;
            if let Some((b, c, cw)) = best {
                let (s, e) = (pts[a], pts[b]);
                let (ci, cj) = (fixed(c[0] - s[0], dec), fixed(c[1] - s[1], dec));
                let r0 = (ci * ci + cj * cj).sqrt();
                let r1 = ((e[0] - (s[0] + ci)).powi(2) + (e[1] - (s[1] + cj)).powi(2)).sqrt();
                if (r0 - r1).abs() <= MAX_RADIUS_GAP * k {
                    let mut l = format!("{} X{} Y{} I{} J{}", if cw { "G2" } else { "G3" }, txt(e[0]), txt(e[1]), txt(ci), txt(cj));
                    if let Some(f) = &run[a].1 { l += &format!(" F{f}"); }
                    out.push(l);
                    arcs += 1;
                    a = b;
                    written = true;
                }
            }
            if !written {
                out.push(lines[i + a].clone());
                a += 1;
            }
        }
        i += run.len();
        pos = Some(*pts.last().unwrap());
    }
    (out, arcs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle_lines(cx: f64, cy: f64, r: f64, n: usize) -> Vec<String> {
        let mut v = vec![format!("G0 X{} Y{}", txt(fixed(cx + r, 3)), txt(fixed(cy, 3))), "G1 Z-1 F200".to_string()];
        for i in 1..=n {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            v.push(format!("G1 X{} Y{}{}", txt(fixed(cx + r * a.cos(), 3)), txt(fixed(cy + r * a.sin(), 3)), if i == 1 { " F800" } else { "" }));
        }
        v.push("G0 Z5".into());
        v
    }

    #[test]
    fn a_drawn_circle_becomes_arcs() {
        let l = circle_lines(50.0, 40.0, 12.0, 120);
        let (o, n) = fit_arcs(&l, 3, 1.0);
        assert!(n >= 2 && n <= 6, "{n} arcs");
        assert!(o.len() < 20, "{} lines", o.len());
        assert!(o.iter().any(|l| l.starts_with("G3 ")), "counter-clockwise");
        assert!(o.first().unwrap().starts_with("G0 ") && o.last().unwrap() == "G0 Z5" && o[1] == "G1 Z-1 F200");
        // the first arc keeps the feed that was on the first line of the run
        assert!(o.iter().find(|l| l.starts_with("G3 ")).unwrap().contains("F800"));
        // every arc is consistent: start and end sit on the same circle
        let mut p = [62.0, 40.0];
        for l in &o {
            if l.starts_with("G3 ") || l.starts_with("G2 ") {
                let w = words(l);
                let g = |c: char| w.iter().find(|x| x.0 == c).unwrap().1.parse::<f64>().unwrap();
                let (c, e) = ([p[0] + g('I'), p[1] + g('J')], [g('X'), g('Y')]);
                let (r0, r1) = ((p[0] - c[0]).hypot(p[1] - c[1]), (e[0] - c[0]).hypot(e[1] - c[1]));
                assert!((r0 - 12.0).abs() < 0.01 && (r1 - r0).abs() < 0.004);
                p = e;
            }
        }
        assert!((p[0] - 62.0).abs() < 1e-3 && (p[1] - 40.0).abs() < 1e-3, "ends where it started");
    }

    #[test]
    fn clockwise_is_g2() {
        let mut l = circle_lines(0.0, 0.0, 10.0, 100);
        let first = l[0].clone();
        let mut pts: Vec<String> = l.drain(2..l.len() - 1).collect();
        pts.reverse();
        // walk the circle the other way: start at the same point
        let mut rev = vec![first, "G1 Z-1 F200".to_string()];
        rev.extend(pts.into_iter().skip(1));
        rev.push("G1 X10 Y0".into());
        let (o, _) = fit_arcs(&rev, 3, 1.0);
        assert!(o.iter().any(|l| l.starts_with("G2 ")) && !o.iter().any(|l| l.starts_with("G3 ")));
    }

    #[test]
    fn polygons_and_lines_stay_straight() {
        for sides in [6usize, 8, 12] {
            let (o, n) = fit_arcs(&circle_lines(0.0, 0.0, 10.0, sides), 3, 1.0);
            assert_eq!(n, 0, "{sides}-gon");
            assert_eq!(o.len(), sides + 3);
        }
        let l: Vec<String> = ["G0 X0 Y0", "G1 X1 Y0 F500", "G1 X2 Y0", "G1 X3 Y0", "G1 X4 Y0", "G1 X5 Y0"].iter().map(|s| s.to_string()).collect();
        let (o, n) = fit_arcs(&l, 3, 1.0);
        assert_eq!((n, o.len()), (0, 6));
    }

    #[test]
    fn plunges_and_comments_pass_through() {
        let l: Vec<String> = ["(Operation 1: X5 Y5)", "G21 G90 G17", "M3 S12000", "G0 X10 Y10", "G1 Z-1 F200", "G1 X20 Y10 Z-1.1"].iter().map(|s| s.to_string()).collect();
        let (o, n) = fit_arcs(&l, 3, 1.0);
        assert_eq!((n, &o), (0, &l));
    }
}
