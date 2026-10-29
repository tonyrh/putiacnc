//! The G-code text played back as a list of steps, for the 3D preview: what the machine will do, in order, read from the text alone.
//!
//! Each step is a `Move` (the same record the generator keeps, so the simulation and the cutter animation work on either) plus an
//! `Info` saying which line of the text it comes from and what kind of step it is. Besides cuts and rapids there are steps that
//! take no distance: a wait (`G4`), a stop for the operator (`M0`, with its message), a tool in the spindle, the spindle starting or
//! stopping. Arcs (`G2`/`G3`) are cut into short straight pieces.
//!
//! What the text does not say is read from the comments the generator writes: `(Operation k: ...)` for the operation,
//! `(Tool: name)` for the tool in the spindle (an operation's second tool shows as `alt`), `(Pass n)` and `(Pass n, levelling lap)`.
//! Lines it cannot place (machine coordinates with `G53`, relative moves, radius arcs) are listed in `unknown`, not guessed at.
use crate::model::ProjectData;
use crate::toolpath::{stop_message, tool_label, zero_offsets, Move};

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// A rapid or a cut.
    Move,
    /// `G4`: seconds.
    Wait(f64),
    /// `M0`: the program stops until the operator resumes it; the message from the line's comment.
    Stop(String),
    /// A tool is in the spindle from here on (its name as the comment writes it).
    Tool(String),
    /// The spindle starts (`true`) or stops.
    Spindle(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    /// Index into the program's lines.
    pub line: usize,
    pub kind: Kind,
}

#[derive(Debug, Clone, Default)]
pub struct Replay {
    pub moves: Vec<Move>,
    /// One per move, same order.
    pub info: Vec<Info>,
    /// Lines that were not played because they cannot be placed in the work coordinates.
    pub unknown: Vec<usize>,
}

impl Replay {
    /// Index of the first step that comes from this line or a later one (where the preview should be when that line is "current").
    pub fn step_of_line(&self, line: usize) -> usize {
        self.info.partition_point(|i| i.line < line)
    }
    /// The line the step at `i` comes from.
    pub fn line_of_step(&self, i: usize) -> Option<usize> { self.info.get(i).map(|x| x.line) }
}

fn number_prefix(l: &str) -> &str {
    let b = l.as_bytes();
    if b.first() == Some(&b'N') {
        let digits = b[1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && b.get(1 + digits) == Some(&b' ') { return &l[digits + 2..]; }
    }
    l
}

/// Splits a line into its words and its comments (the text inside brackets, or after a semicolon).
fn split(line: &str) -> (Vec<(char, f64)>, Vec<String>) {
    let mut words = Vec::new();
    let mut comments = Vec::new();
    let (mut depth, mut cur) = (0u32, String::new());
    let mut code = String::with_capacity(line.len());
    for (i, c) in line.char_indices() {
        match c {
            '(' => { depth += 1; if depth == 1 { cur.clear(); } else { cur.push(c); } }
            ')' if depth > 0 => { depth -= 1; if depth == 0 { comments.push(std::mem::take(&mut cur)); } else { cur.push(c); } }
            ';' if depth == 0 => { comments.push(line[i + 1..].trim().to_string()); break; }
            _ if depth > 0 => cur.push(c),
            _ => code.push(c),
        }
    }
    let b = code.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_alphabetic() {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.' || b[j] == b'-' || b[j] == b'+') { j += 1; }
            if let Ok(v) = code[i + 1..j].parse::<f64>() { words.push((c.to_ascii_uppercase(), v)); }
            i = j;
        } else { i += 1; }
    }
    (words, comments)
}

/// The tool name in a `(Tool: name)` or `(Tool 2: name)` comment.
fn tool_in(comment: &str) -> Option<String> {
    let r = comment.strip_prefix("Tool")?;
    let r = r.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ');
    Some(r.strip_prefix(':')?.trim().to_string())
}

/// Plays `lines` (the generator's G-code for `data`, in mm or inches, numbered or not).
pub fn replay(lines: &[String], data: &ProjectData) -> Replay {
    let machine = data.machine.clone().unwrap_or_default();
    let (rapid, tsc) = (machine.rapid.max(1.0), machine.tsc);
    let st = &data.stock;
    let (ox, oy) = zero_offsets(st);
    let zt = if st.zm == "top" { 0.0 } else { st.t - st.zh };
    // per operation: the labels of its tool and its second tool
    let labels: Vec<(String, Option<String>)> = data.ops.iter().map(|o| {
        let name = |id: &str| data.tools.iter().find(|t| t.id == id).map(|t| tool_label(&t.name)).unwrap_or_default();
        (name(&o.p.tool), o.p.tool2.as_deref().map(name))
    }).collect();

    let mut r = Replay::default();
    let mut ku = 1.0; // mm per program unit
    let mut pos = [ox, oy, f64::NAN];
    let (mut feed, mut mode) = (0.0f64, 0u8);
    let (mut op, mut pass, mut lap) = (0usize, 0usize, false);
    let mut tool = String::new();
    for (li, raw) in lines.iter().enumerate() {
        let text = number_prefix(raw);
        if text.is_empty() { continue; }
        let (w, comments) = split(text);
        for c in &comments {
            if let Some(rest) = c.strip_prefix("Operation ") {
                if let Some(k) = rest.split(':').next().and_then(|n| n.trim().parse::<usize>().ok()) { op = k.saturating_sub(1); pass = 0; lap = false; }
            } else if let Some(rest) = c.strip_prefix("Pass ") {
                let n: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
                if let Ok(n) = n.parse::<usize>() { pass = n; lap = rest.contains("levelling lap"); }
            } else if let Some(name) = tool_in(c) {
                tool = name.clone();
                r.moves.push(zero(pos, op, pass, 0.0));
                r.info.push(Info { line: li, kind: Kind::Tool(name) });
            }
        }
        if w.is_empty() { continue; }
        let mut g_motion: Option<u8> = None;
        let (mut x, mut y, mut z, mut i_, mut j_, mut f, mut p_) = (None, None, None, None, None, None, None);
        let (mut m_codes, mut unplaceable) = (Vec::new(), false);
        for &(c, v) in &w {
            match c {
                'G' => match v as i32 {
                    0 => g_motion = Some(0), 1 => g_motion = Some(1), 2 => g_motion = Some(2), 3 => g_motion = Some(3),
                    4 => m_codes.push(4),
                    20 => ku = 25.4, 21 => ku = 1.0,
                    17 | 40 | 49 | 54 | 80 | 90 | 94 => {}
                    _ => unplaceable = true, // G53, G91, G92, G28 ...: not placed
                },
                'M' => m_codes.push(v as i32 + 1000),
                'X' => x = Some(v), 'Y' => y = Some(v), 'Z' => z = Some(v),
                'I' => i_ = Some(v), 'J' => j_ = Some(v), 'F' => f = Some(v), 'P' => p_ = Some(v),
                'R' => unplaceable = true,
                _ => {}
            }
        }
        if let Some(v) = f { feed = v * ku; }
        for m in m_codes {
            match m {
                4 => { let secs = p_.unwrap_or(0.0).max(0.0); r.moves.push(zero(pos, op, pass, secs / 60.0)); r.info.push(Info { line: li, kind: Kind::Wait(secs) }); }
                1003 | 1004 => { r.moves.push(zero(pos, op, pass, 0.0)); r.info.push(Info { line: li, kind: Kind::Spindle(true) }); }
                1005 => { r.moves.push(zero(pos, op, pass, 0.0)); r.info.push(Info { line: li, kind: Kind::Spindle(false) }); }
                1000 | 1001 => {
                    let msg = comments.iter().find(|c| !c.is_empty()).cloned().or_else(|| stop_message(text.trim()).map(String::from)).unwrap_or_default();
                    r.moves.push(zero(pos, op, pass, 0.0));
                    r.info.push(Info { line: li, kind: Kind::Stop(msg) });
                }
                _ => {}
            }
        }
        let has_axes = x.is_some() || y.is_some() || z.is_some();
        if let Some(g) = g_motion { mode = g; }
        if !has_axes { if unplaceable { r.unknown.push(li); } continue; }
        if unplaceable { r.unknown.push(li); continue; }
        if text.contains("G53") { r.unknown.push(li); continue; }
        let to = [x.map_or(pos[0], |v| v * ku + ox), y.map_or(pos[1], |v| v * ku + oy), z.map_or(pos[2], |v| v * ku - zt)];
        if pos[2].is_nan() { pos[2] = if to[2].is_nan() { 0.0 } else { to[2] }; }
        let to = [to[0], to[1], if to[2].is_nan() { pos[2] } else { to[2] }];
        // the tool of this move: the second tool of the operation (inlay floor, relief roughing) is `alt`
        let alt = labels.get(op).is_some_and(|(t1, t2)| t2.as_ref().is_some_and(|t2| *t2 == tool && *t1 != tool));
        let cut = mode != 0;
        // a rapid ends the pass: the cuts after it say their pass again
        if !cut { pass = 0; lap = false; }
        let speed = if cut { feed } else { rapid };
        let secs_per_mm = if speed > 0.0 { tsc / speed } else { 0.0 };
        let mut pieces: Vec<[f64; 3]> = Vec::new();
        if (mode == 2 || mode == 3) && (i_.is_some() || j_.is_some()) {
            let (cx, cy) = (pos[0] + i_.unwrap_or(0.0) * ku, pos[1] + j_.unwrap_or(0.0) * ku);
            let rad = ((pos[0] - cx).powi(2) + (pos[1] - cy).powi(2)).sqrt();
            let a0 = (pos[1] - cy).atan2(pos[0] - cx);
            let a1 = (to[1] - cy).atan2(to[0] - cx);
            let tau = std::f64::consts::TAU;
            // G2 turns clockwise (the angle falls), G3 counter-clockwise; the same start and end is a full circle
            let mut sweep = a1 - a0;
            if mode == 2 { while sweep >= 0.0 { sweep -= tau; } if (to[0] - pos[0]).abs() < 1e-9 && (to[1] - pos[1]).abs() < 1e-9 { sweep = -tau; } }
            else { while sweep <= 0.0 { sweep += tau; } if (to[0] - pos[0]).abs() < 1e-9 && (to[1] - pos[1]).abs() < 1e-9 { sweep = tau; } }
            // pieces short enough that the chord stays within 0.01 mm of the curve
            let step = if rad > 0.02 { 2.0 * (1.0 - 0.01 / rad).clamp(-1.0, 1.0).acos() } else { 0.5 }.clamp(0.02, 0.5);
            let n = ((sweep.abs() / step).ceil() as usize).max(1);
            for k in 1..=n {
                let t = k as f64 / n as f64;
                let a = a0 + sweep * t;
                pieces.push(if k == n { to } else { [cx + rad * a.cos(), cy + rad * a.sin(), pos[2] + (to[2] - pos[2]) * t] });
            }
        } else { pieces.push(to); }
        for q in pieces {
            let d = ((q[0] - pos[0]).powi(2) + (q[1] - pos[1]).powi(2) + (q[2] - pos[2]).powi(2)).sqrt();
            if d > 0.0 {
                r.moves.push(Move { from: pos, to: q, cut, dt: d * secs_per_mm, pass, op, alt, lap: lap && cut });
                r.info.push(Info { line: li, kind: Kind::Move });
            }
            pos = q;
        }
    }
    r
}

/// A step that takes no distance: `dt` minutes of waiting, or none.
fn zero(pos: [f64; 3], op: usize, pass: usize, dt: f64) -> Move {
    let at = [pos[0], pos[1], if pos[2].is_nan() { 0.0 } else { pos[2] }];
    Move { from: at, to: at, cut: false, dt, pass, op, alt: false, lap: false }
}
