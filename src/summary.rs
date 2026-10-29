//! "What will this do?": a few plain lines about a generated program, read from the G-code itself and the project, for the person
//! standing at the machine: where the zero is, what the first moves do, which tools it asks for, how deep and how long, how it ends.
use crate::model::ProjectData;
use crate::toolpath::Output;
use crate::Units;

/// One line of the summary. `warn` marks something to do or look at before starting.
#[derive(Debug, Clone, PartialEq)]
pub struct Row { pub label: &'static str, pub text: String, pub warn: bool }

fn row(label: &'static str, text: impl Into<String>, warn: bool) -> Row { Row { label, text: text.into(), warn } }

/// The value after a letter on a G-code line ("Z-1.5" -> -1.5), when the line has it.
fn word(line: &str, letter: char) -> Option<f64> {
    line.split_whitespace().find_map(|t| t.strip_prefix(letter).and_then(|v| v.parse::<f64>().ok()))
}

fn is_move(line: &str) -> bool { line.starts_with("G0 ") || line.starts_with("G1 ") || line.starts_with("G0\t") }

fn strip_number(l: &str) -> &str {
    let b = l.as_bytes();
    if b.first() == Some(&b'N') {
        let digits = b[1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && b.get(1 + digits) == Some(&b' ') { return &l[digits + 2..]; }
    }
    l
}

/// "2 min 8 s", "45 s", "1 h 05 min".
pub fn duration(min: f64) -> String {
    let s = (min * 60.0).round().max(0.0) as u64;
    if s >= 3600 { format!("{} h {:02} min", s / 3600, (s % 3600) / 60) }
    else if s >= 60 { format!("{} min {} s", s / 60, s % 60) }
    else { format!("{} s", s) }
}

/// `piece`: the program is for a separate plug piece, so what is read from the project (tool names, air cut) is left out.
pub fn describe(data: &ProjectData, out: &Output, units: Units, piece: bool) -> Vec<Row> {
    let un = units.name();
    // numbered programs ("N120 G0 Z5") are read without their numbers
    let plain: Vec<String> = out.lines.iter().map(|x| strip_number(x).to_string()).collect();
    let l = &plain;
    let mut rows = Vec::new();

    // zero: the header comment says it, for the main piece and for a plug piece alike
    if let Some(h) = l.iter().find(|x| x.starts_with("(Stock ")) {
        if let Some((_, z)) = h.trim_end_matches(')').split_once("; ") { rows.push(row("Zero", z.to_string(), false)); }
    }

    // the first sideways move with no lift before it: the tool goes wherever it stands
    let first_xy = l.iter().position(|x| is_move(x) && (word(x, 'X').is_some() || word(x, 'Y').is_some()));
    // a lift counts when it is a rapid Z move, also in machine coordinates (G53) as a machine's own start code may do it
    let lifted = first_xy.is_some_and(|i| l[..i].iter().any(|x| (x.starts_with("G0 ") || x.contains("G53")) && word(x, 'Z').is_some()));
    if first_xy.is_some() && !lifted {
        rows.push(row("Before you start", "Raise the tool clear of the stock and clamps. The program does not lift first: its first move travels sideways at the height the tool is at.", true));
    }

    // start: spindle, dwell, first move
    if let Some(i) = l.iter().position(|x| x.starts_with("M3 ")) {
        let rpm = word(&l[i], 'S').map(|v| format!("{}", v as i64));
        let wait = l.get(i + 1).filter(|x| x.starts_with("G4 ")).and_then(|x| word(x, 'P'));
        let mut t = format!("Starts the spindle{}", rpm.map(|r| format!(" at {r} rpm")).unwrap_or_default());
        if let Some(w) = wait { t += &format!(" and waits {} s", w); }
        if let Some(f) = first_xy { t += &format!(", then moves to {}", l[f].split_once(' ').map_or("", |x| x.1).split(" F").next().unwrap_or("")); }
        rows.push(row("Start", t + ".", false));
    }

    // tools: the names from the operations, and where the program stops for a change
    let mut names: Vec<String> = Vec::new();
    if !piece {
        let mut add = |id: &str| { if let Some(t) = data.tools.iter().find(|t| t.id == id) { if !names.contains(&t.name) { names.push(t.name.clone()); } } };
        for o in &data.ops {
            if o.p.kind == "relief" {
                if !o.p.r_off { if let Some(t) = &o.p.tool2 { add(t); } }
                if !o.p.f_off { add(&o.p.tool); }
            } else {
                add(&o.p.tool);
                if matches!(o.p.kind.as_str(), "inlay" | "plug") { if let Some(t) = &o.p.tool2 { add(t); } }
            }
        }
    }
    let stops: Vec<&str> = l.iter().filter_map(|x| crate::toolpath::stop_message(x)).filter_map(|m| m.strip_prefix("Change tool to ")).map(|m| m.split(", then").next().unwrap_or("")).collect();
    let m6 = l.iter().filter(|x| x.contains(" M6 ")).count().saturating_sub(1);
    let changes = stops.len().max(m6);
    let mut t = if names.is_empty() { String::new() } else { names.join(", ") };
    if changes == 0 { if !t.is_empty() { t += ". No tool change."; } }
    else if !stops.is_empty() {
        if !t.is_empty() { t += ". "; }
        t += &format!("The program pauses {} time{} for you to change the tool (to {}).", changes, if changes == 1 { "" } else { "s" }, stops.join(", then "));
    } else {
        if !t.is_empty() { t += ". "; }
        t += &format!("{} tool change{} (M6).", changes, if changes == 1 { "" } else { "s" });
    }
    if !t.is_empty() { rows.push(row("Tools", t, false)); }
    if !stops.is_empty() {
        rows.push(row("At a tool change", "Touch off Z0 again with the new tool before you resume: the controller does not know how long each tool is.", true));
    }

    // pauses the operator asked for (a tool change has its own row)
    let pauses: Vec<String> = l.iter().filter_map(|x| crate::toolpath::stop_message(x)).filter(|m| !m.starts_with("Change tool to "))
        .map(|m| format!("\u{201c}{m}\u{201d}")).collect();
    if !pauses.is_empty() {
        rows.push(row("Pauses", format!("The program stops {} time{} with the spindle off: {}. Resume it when you are ready; the spindle starts again by itself.", pauses.len(), if pauses.len() == 1 { "" } else { "s" }, pauses.join(", ")), false));
    }

    // how deep, how much, how long
    let deepest = l.iter().filter(|x| x.starts_with("G1 ")).filter_map(|x| word(x, 'Z')).fold(f64::MAX, f64::min);
    let k = units.factor();
    let mut t = String::new();
    if deepest < f64::MAX { t += &format!("Down to Z {} {}. ", crate::toolpath::num(deepest), un); }
    t += &format!("{} {} of cutting, about {} in all.", crate::toolpath::num(crate::toolpath::to_fixed(out.cut_len_mm / k, if k == 1.0 { 0 } else { 1 })), un, duration(out.time_min));
    rows.push(row("Cutting", t, false));

    // the end
    let tail = &l[l.len().saturating_sub(6)..];
    let lift = tail.iter().rev().find(|x| x.starts_with("G0 ")).and_then(|x| word(x, 'Z')).or_else(|| tail.iter().rev().filter(|x| x.starts_with("G0 ")).find_map(|x| word(x, 'Z')));
    let mut t = Vec::new();
    if let Some(z) = lift { t.push(format!("lifts to Z {} {}", crate::toolpath::num(z), un)); }
    if tail.iter().any(|x| x.starts_with("M5")) { t.push("stops the spindle".into()); }
    if let Some(x) = tail.iter().rev().find(|x| x.starts_with("G0 X")) { t.push(format!("goes to {}", x.trim_start_matches("G0 "))); }
    if !t.is_empty() { let mut s = t.join(", "); if let Some(c) = s.get_mut(0..1) { c.make_ascii_uppercase() } rows.push(row("End", s + ".", false)); }

    // air cut: nothing is cut
    if !piece {
        let air: Vec<String> = data.ops.iter().enumerate().filter(|(_, o)| o.p.air || data.job.air).map(|(i, _)| (i + 1).to_string()).collect();
        if !air.is_empty() { rows.push(row("Air cut", format!("Operation{} {} run{} 10 mm above the stock: nothing is cut.", if air.len() == 1 { "" } else { "s" }, air.join(", "), if air.len() == 1 { "s" } else { "" }), true)); }
    }

    let warns = out.notes.iter().filter(|n| n.starts_with("⚠ ")).count();
    if warns > 0 { rows.push(row("Warnings", format!("{} warning{}: see the Notes tab.", warns, if warns == 1 { "" } else { "s" }), true)); }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generate, load_project};

    fn rows(name: &str, units: Units) -> Vec<Row> {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/data/{name}.project.json"));
        let f = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
        let out = generate(&f.data, units).unwrap();
        describe(&f.data, &out, units, false)
    }
    fn get<'a>(r: &'a [Row], l: &str) -> Option<&'a Row> { r.iter().find(|x| x.label == l) }

    #[test]
    fn a_single_tool_job_is_summarised() {
        let r = rows("outside_holes", Units::Metric);
        assert!(get(&r, "Zero").unwrap().text.contains("lower-left corner of stock"));
        assert!(get(&r, "Start").unwrap().text.contains("rpm") && get(&r, "Start").unwrap().text.contains("waits 2 s"));
        assert!(get(&r, "Tools").unwrap().text.contains("No tool change"));
        let c = &get(&r, "Cutting").unwrap().text;
        assert!(c.contains("Down to Z -") && c.contains("mm of cutting") && c.contains("in all"), "{c}");
        assert!(get(&r, "End").unwrap().text.contains("stops the spindle"));
        // the program lifts to safe Z before its first sideways move, so there is nothing to warn about
        assert!(get(&r, "Before you start").is_none());
        assert!(get(&r, "Air cut").is_none());
    }

    #[test]
    fn a_program_without_a_lift_is_warned_about() {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/outside_holes.project.json");
        let f = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
        let mut out = generate(&f.data, Units::Metric).unwrap();
        let at = out.lines.iter().position(|x| x.starts_with("G0 Z")).unwrap();
        out.lines.remove(at);
        let r = describe(&f.data, &out, Units::Metric, false);
        assert!(get(&r, "Before you start").unwrap().warn);
    }

    #[test]
    fn a_tool_change_is_announced() {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/outside_holes.project.json");
        let mut f = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
        // the second operation takes the last shape and a tool of another diameter
        let mut t2 = f.data.tools.iter().find(|t| t.id == f.data.ops[0].p.tool).unwrap().clone();
        t2.id = "t2".into(); t2.name = "6 mm flat end mill".into(); t2.dia = 6.0;
        f.data.tools.push(t2);
        let other = "t2".to_string();
        let mut o2 = f.data.ops[0].clone();
        o2.id = 99; o2.p.tool = other;
        f.data.ops.push(o2);
        f.data.loops.last_mut().unwrap().op = 99;
        let out = generate(&f.data, Units::Metric).unwrap();
        let t = describe(&f.data, &out, Units::Metric, false).into_iter().find(|x| x.label == "Tools").unwrap().text;
        assert!(t.contains("pauses 1 time") && t.contains("change the tool"), "{t}");
    }

    #[test]
    fn an_air_cut_is_flagged_and_inches_are_used() {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/outside_holes.project.json");
        let mut f = load_project(&std::fs::read_to_string(p).unwrap()).unwrap();
        f.data.ops[0].p.air = true;
        let out = generate(&f.data, Units::Imperial).unwrap();
        let r = describe(&f.data, &out, Units::Imperial, false);
        let air = get(&r, "Air cut").unwrap();
        assert!(air.warn && air.text.contains("Operation 1 runs"));
        assert!(get(&r, "Cutting").unwrap().text.contains(" in of cutting"));
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(duration(0.5), "30 s");
        assert_eq!(duration(2.0 + 8.0 / 60.0), "2 min 8 s");
        assert_eq!(duration(65.0), "1 h 05 min");
    }
}
