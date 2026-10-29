//! An independent check of the finished G-code. It reads the text of the program, line by line, the way a controller would, and knows
//! nothing about how the program was made: it keeps its own idea of the tool's position, the spindle and the feed, and reports what
//! would hurt (cutting with the spindle off, a feed move with no feed, a sideways move before the tool height is known, numbers that
//! are not numbers, a spindle left running). `trace` also measures the program (cutting length, deepest cut) so tests can compare it
//! with what the generator thinks it wrote.
//!
//! It reads absolute G-code with G0 to G3 (arcs by I and J), G20 / G21, M3 / M4 / M5, M2 / M30, S and F, with or without line numbers
//! and comments, which is all PutiaCNC writes. Anything else on a line is passed over.

/// One word of a G-code line: a letter and its number ("X-1.5").
fn words(line: &str) -> Vec<(char, f64)> {
    // comments: (...) and everything after ; (most lines have none, and are read as they are)
    let stripped;
    let clean: &str = if line.contains(['(', ';']) {
        let (mut depth, mut s) = (0u32, String::with_capacity(line.len()));
        for c in line.chars() {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                ';' if depth == 0 => break,
                c if depth == 0 => s.push(c),
                _ => {}
            }
        }
        stripped = s;
        &stripped
    } else { line };
    let b = clean.as_bytes();
    let mut out = Vec::with_capacity(8);
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphabetic() {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_digit() || matches!(b[j], b'+' | b'-' | b'.')) { j += 1; }
            if j > i + 1 { if let Ok(v) = clean[i + 1..j].parse::<f64>() { out.push((b[i].to_ascii_uppercase() as char, v)); } }
            i = j.max(i + 1);
        } else { i += 1; }
    }
    out
}

/// What the program does, measured from its text.
#[derive(Debug, Default, Clone)]
pub struct Trace {
    /// Length of everything cut with G1, G2 and G3, in mm (a G1 counts in 3D, like the generator's own figure).
    pub cut_len_mm: f64,
    /// The lowest Z of any cutting move, in mm, as the program writes it (its own zero).
    pub deepest_cut_mm: Option<f64>,
    /// Number of cutting moves.
    pub cut_moves: usize,
    /// Problems, each as a ready sentence starting with a warning sign.
    pub hazards: Vec<String>,
}

#[derive(Default)]
struct Found { first: usize, count: usize }

/// Read the program. `lines` are the lines of the finished G-code, as written to the file.
pub fn trace(lines: &[String]) -> Trace {
    let mut t = Trace::default();
    // one entry per kind of problem: the first line it happens on and how many lines have it
    let mut found: Vec<(&'static str, Found)> = Vec::new();
    let mut flag = |kind: &'static str, line: usize| {
        match found.iter_mut().find(|f| f.0 == kind) { Some(f) => f.1.count += 1, None => found.push((kind, Found { first: line, count: 1 })) }
    };

    let mut k = 1.0; // mm per unit
    let mut pos: [Option<f64>; 3] = [None; 3];
    let mut mode = 0u8; // 0..3 once a G word has set it
    let mut mode_set = false;
    let (mut spindle, mut spindle_s): (bool, Option<f64>) = (false, None);
    let mut feed: Option<f64> = None;
    let mut seen_sideways = false;

    for (n, raw) in lines.iter().enumerate() {
        let line_no = n + 1;
        // (the text of a comment may say anything, so only the part before one is looked at)
        let code = raw.split(['(', ';']).next().unwrap_or("");
        if code.len() >= 3 && (code.contains("NaN") || code.contains("nan") || code.contains("inf") || code.contains("Inf")) { flag("nan", line_no); continue; }
        let w = words(raw);
        let g: Vec<i64> = w.iter().filter(|x| x.0 == 'G').map(|x| x.1.round() as i64).collect();
        // relative moves are not followed: the rest of the program cannot be judged from here
        if g.contains(&91) { return t.finish(found); }
        if g.contains(&20) { k = 25.4; }
        if g.contains(&21) { k = 1.0; }
        for &c in &g { if (0..=3).contains(&c) { mode = c as u8; mode_set = true; } }
        let m: Vec<i64> = w.iter().filter(|x| x.0 == 'M').map(|x| x.1.round() as i64).collect();
        for x in &w {
            if x.0 == 'S' { spindle_s = Some(x.1); }
            if x.0 == 'F' { feed = Some(x.1); }
        }
        if m.contains(&3) || m.contains(&4) {
            spindle = true;
            if !spindle_s.is_some_and(|s| s > 0.0) { flag("s0", line_no); }
        }
        if m.contains(&5) || m.contains(&2) || m.contains(&30) { spindle = false; }

        let has = |c: char| w.iter().find(|x| x.0 == c).map(|x| x.1);
        let (x, y, z) = (has('X'), has('Y'), has('Z'));
        // a line in machine coordinates (G53) says nothing about the work position, which is then not known any more
        if g.contains(&53) {
            if x.is_some() { pos[0] = None; }
            if y.is_some() { pos[1] = None; }
            if z.is_some() { pos[2] = None; }
            continue;
        }
        if !mode_set || (x.is_none() && y.is_none() && z.is_none()) { continue; }
        let cut = mode >= 1;
        if (x.is_some() || y.is_some()) && !seen_sideways {
            seen_sideways = true;
            // nothing set the tool's height before the first sideways move: it goes wherever the tool stands
            if pos[2].is_none() && z.is_none() { flag("nolift", line_no); }
        }
        if cut {
            if !spindle { flag("spindle", line_no); }
            if mode == 1 && !feed.is_some_and(|f| f > 0.0) { flag("feed", line_no); }
        }
        let to = [x.map(|v| v * k).or(pos[0]), y.map(|v| v * k).or(pos[1]), z.map(|v| v * k).or(pos[2])];
        if cut {
            t.cut_moves += 1;
            if let Some(z) = to[2] { t.deepest_cut_mm = Some(t.deepest_cut_mm.map_or(z, |d| d.min(z))); }
            if let (Some(a), Some(b)) = (pos[0].zip(pos[1]), to[0].zip(to[1])) {
                let dz = match (pos[2], to[2]) { (Some(p), Some(q)) => q - p, _ => 0.0 };
                if mode == 1 { t.cut_len_mm += ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2) + dz * dz).sqrt(); }
                else if let (Some(i), Some(j)) = (has('I'), has('J')) {
                    let (i, j) = (i * k, j * k);
                    let (cx, cy) = (a.0 + i, a.1 + j);
                    let r = i.hypot(j);
                    let (a0, a1) = ((a.1 - cy).atan2(a.0 - cx), (b.1 - cy).atan2(b.0 - cx));
                    let tau = std::f64::consts::TAU;
                    let mut sweep = if mode == 3 { a1 - a0 } else { a0 - a1 };
                    while sweep <= 1e-9 { sweep += tau; }
                    while sweep > tau + 1e-9 { sweep -= tau; }
                    if (a.0 - b.0).hypot(a.1 - b.1) < 1e-9 { sweep = tau; }
                    let flat = r * sweep;
                    t.cut_len_mm += (flat * flat + dz * dz).sqrt();
                }
            }
        }
        pos = to;
    }
    if spindle { flag("running", lines.len()); }
    t.finish(found)
}

impl Trace {
    fn finish(mut self, found: Vec<(&'static str, Found)>) -> Trace {
        for (kind, f) in found {
            let more = if f.count > 1 { format!(" ({} more lines like it)", f.count - 1) } else { String::new() };
            let msg = match kind {
                "nan" => format!("⚠ G-code line {}: a value that is not a number{more}. Do not run this program.", f.first),
                "spindle" => format!("⚠ G-code line {}: a cutting move while the spindle is off{more}.", f.first),
                "feed" => format!("⚠ G-code line {}: a cutting move with no feed rate set{more}. Most controllers stop with an error.", f.first),
                "s0" => format!("⚠ G-code line {}: the spindle is started with no speed (S){more}.", f.first),
                "nolift" => format!("⚠ G-code line {}: the first sideways move comes before the program has set the tool's height{more}: raise the tool clear of the stock and clamps before you start.", f.first),
                "running" => "⚠ The program ends with the spindle still running (no M5, M2 or M30 after the last M3).".to_string(),
                _ => continue,
            };
            self.hazards.push(msg);
        }
        self
    }
}

/// The problems of a program, as warnings for the Notes tab.
pub fn hazards(lines: &[String]) -> Vec<String> { trace(lines).hazards }

#[cfg(test)]
mod tests {
    use super::*;
    fn l(s: &str) -> Vec<String> { s.lines().map(String::from).collect() }

    #[test]
    fn a_good_program_has_nothing_to_report_and_is_measured() {
        let t = trace(&l("(demo)\nG21 G90 G17\nG0 Z5\nM3 S12000\nG4 P2\nG0 X10 Y10\nG0 Z0.5\nG1 Z-1 F250\nG1 X20 F800\nG1 Y20\nG0 Z5\nM5\nG0 X0 Y0\nM2"));
        assert!(t.hazards.is_empty(), "{:?}", t.hazards);
        assert_eq!(t.cut_moves, 3);
        // 1.5 down (0.5 -> -1), then 10 and 10 along
        assert!((t.cut_len_mm - 21.5).abs() < 1e-9, "{}", t.cut_len_mm);
        assert_eq!(t.deepest_cut_mm, Some(-1.0));
    }

    #[test]
    fn numbered_lines_comments_and_inches_are_read() {
        let t = trace(&l("N10 G20 G90\nN20 G0 Z0.2\nN30 M3 S10000 (on)\nN40 G0 X1 Y1\nN50 G1 Z-0.1 F10 ; plunge\nN60 G1 X2\nN70 M5\nN80 M30"));
        assert!(t.hazards.is_empty(), "{:?}", t.hazards);
        assert!((t.deepest_cut_mm.unwrap() + 2.54).abs() < 1e-9);
        assert!((t.cut_len_mm - (0.3 * 25.4 + 25.4)).abs() < 1e-9, "{}", t.cut_len_mm);
    }

    #[test]
    fn arcs_are_measured() {
        // a half circle of radius 5 (centre 5 to the right of the start), counter-clockwise, then a full circle clockwise
        let t = trace(&l("G21 G90\nG0 Z5\nM3 S1000\nG0 X0 Y0\nG1 Z-1 F100\nG3 X10 Y0 I5 J0\nG2 X10 Y0 I-5 J0\nM5\nM2"));
        assert!(t.hazards.is_empty(), "{:?}", t.hazards);
        let want = 6.0 + std::f64::consts::PI * 5.0 + std::f64::consts::TAU * 5.0;
        assert!((t.cut_len_mm - want).abs() < 1e-9, "{} vs {}", t.cut_len_mm, want);
    }

    #[test]
    fn each_kind_of_problem_is_found() {
        let h = |s: &str| trace(&l(s)).hazards.join("\n");
        assert!(h("G21 G90\nG0 Z5\nG1 X5 F100\nM2").contains("spindle is off"));
        assert!(h("G21 G90\nG0 Z5\nM3 S1000\nG1 X5\nM5").contains("no feed"));
        assert!(h("G21 G90\nG0 Z5\nM3\nM5").contains("no speed"));
        assert!(h("G21 G90\nM3 S1000\nG0 X5 Y5\nM5").contains("before the program has set the tool's height"));
        assert!(h("G21 G90\nG0 Z5\nM3 S1000\nG0 X5 Y5\n").contains("still running"));
        assert!(h("G21 G90\nG0 Z5\nM3 S1000\nG1 X5 Y NaN F100\nM5").contains("not a number"));
        // a G53 lift counts only as far as it goes: the work height is still not known
        assert!(h("G21 G90\nG53 G0 Z0\nM3 S1000\nG0 X5 Y5\nM5").contains("tool's height"));
    }

    #[test]
    fn the_same_problem_on_many_lines_is_one_note() {
        let h = trace(&l("G21 G90\nG0 Z5\nG1 X5 F100\nG1 X6\nG1 X7\nM2")).hazards;
        assert_eq!(h.len(), 1);
        assert!(h[0].contains("line 3") && h[0].contains("2 more lines"), "{}", h[0]);
    }

    #[test]
    fn relative_programs_are_not_judged() {
        assert!(trace(&l("G21 G91\nG1 X5 F100\nM2")).hazards.is_empty());
    }
}
