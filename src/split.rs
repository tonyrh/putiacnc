//! One program in several files: one per tool, or one per operation.
//!
//! Each file is made by the ordinary generator on a copy of the project that keeps all the operations in the list (so operation
//! numbers in the messages and the moves stay the project's own) but only the shapes of the operations that go in that file: the
//! others have no shapes and are skipped. So every file is a complete program: summary, spindle start, safe heights, the end.
//! A pause after the last operation of a file is the end of that file: it is not written as `M0`, the file ends with a comment that
//! says it, and the next file says what to do before starting it (change the tool, touch off Z0). A plug on a separate piece is its
//! own file already (see [`crate::pieces`]) and stays out.
use std::collections::BTreeSet;

use crate::model::{ProjectData, Units};
use crate::pieces;
use crate::toolpath::{generate, tool_label, Output};

/// How a program is cut into files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Split {
    /// One file, as always.
    #[default]
    One,
    /// A new file at every tool change.
    PerTool,
    /// One file for each operation.
    PerOperation,
}

impl Split {
    pub fn id(self) -> &'static str { match self { Split::One => "one", Split::PerTool => "tool", Split::PerOperation => "operation" } }
    pub fn from_id(s: &str) -> Split { match s { "tool" => Split::PerTool, "operation" => Split::PerOperation, _ => Split::One } }
}

/// One file of a split program.
#[derive(Debug, Clone)]
pub struct SplitFile {
    /// Positions (0-based, in the project's list) of the operations in this file.
    pub ops: Vec<usize>,
    /// The end of the file name, after the project's name: "-2-pocket", "-1-6-mm-flat-end-mill".
    pub suffix: String,
    /// What the file does, in a few words ("Operation 2: pocket", "6 mm flat end mill").
    pub label: String,
    pub out: Output,
}

/// An operation that takes two tools in one go (an inlay clears with a flat mill and cuts with a V-bit, a relief roughs and finishes):
/// it cannot be cut in two, so it stands as a file of its own whatever the split.
fn two_tools(o: &crate::model::Operation) -> bool { o.p.tool2.is_some() && matches!(o.p.kind.as_str(), "inlay" | "plug" | "relief") }

/// The operations (positions in the project's list) that go in each file, in order. Only operations that cut something count: those with
/// shapes (or a relief), without the plugs that have a piece of their own. `Split::One` is one group.
pub fn groups(data: &ProjectData, mode: Split) -> Vec<Vec<usize>> {
    let (main, _) = pieces::split(data);
    let active: Vec<usize> = (0..main.ops.len()).filter(|&k| {
        let o = &main.ops[k];
        !pieces::is_piece_op(o) && (if o.p.kind == "relief" { main.relief.is_some() } else { main.loops.iter().any(|l| l.op == o.id) })
    }).collect();
    match mode {
        Split::One => if active.is_empty() { vec![] } else { vec![active] },
        Split::PerOperation => active.into_iter().map(|k| vec![k]).collect(),
        Split::PerTool => {
            let mut out: Vec<Vec<usize>> = vec![];
            let mut last: Option<String> = None;
            for k in active {
                let o = &main.ops[k];
                if two_tools(o) { out.push(vec![k]); last = None; continue; }
                match (&last, out.last_mut()) {
                    (Some(t), Some(g)) if *t == o.p.tool => g.push(k),
                    _ => out.push(vec![k]),
                }
                last = Some(o.p.tool.clone());
            }
            out
        }
    }
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in tool_label(s).chars() {
        if c.is_ascii_alphanumeric() { out.push(c.to_ascii_lowercase()); } else if !out.ends_with('-') { out.push('-'); }
    }
    out.trim_matches('-').to_string()
}

/// A text safe inside a G-code comment: no brackets, plain ASCII.
fn comment_text(s: &str) -> String { tool_label(&s.replace(['(', ')'], "")) }

fn tool_name(data: &ProjectData, id: &str) -> String {
    data.tools.iter().find(|t| t.id == id).map(|t| t.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "the tool".into())
}

/// Makes the files of the program for `mode`. For `Split::One` that is the whole main job as one file.
pub fn generate_files(data: &ProjectData, units: Units, mode: Split) -> Result<Vec<SplitFile>, String> {
    let (main, _) = pieces::split(data);
    let gs = groups(data, mode);
    let n = gs.len();
    let width = main.ops.len().to_string().len();
    let mut files: Vec<SplitFile> = vec![];
    let mut prev_tool: Option<String> = None;
    for (gi, g) in gs.iter().enumerate() {
        let keep: BTreeSet<i64> = g.iter().map(|&k| main.ops[k].id).collect();
        let mut d = main.clone();
        // the shapes of the other operations are left out, and so is what they would do (an operation without shapes writes nothing)
        let removed: Vec<usize> = d.loops.iter().enumerate().filter(|(_, l)| !keep.contains(&l.op)).map(|(i, _)| i).collect();
        d.loops.retain(|l| keep.contains(&l.op));
        crate::shapes::tabs_after_delete(&mut d.ops, &removed);
        for o in d.ops.iter_mut().filter(|o| !keep.contains(&o.id)) { o.p.kind = "online".into(); o.p.pause = None; }
        // a pause after the last operation is the end of the file
        let last = *g.last().expect("a group has an operation");
        let after = d.ops[last].p.pause.take().map(|m| if m.trim().is_empty() { format!("Operation {} done: resume when ready", last + 1) } else { m });
        let mut out = generate(&d, units)?;
        out.notes.retain(|x| !(x.starts_with("Op ") && x.ends_with(" has no shapes.")));

        let first = &main.ops[g[0]];
        let tool_first = tool_name(&main, &first.p.tool);
        let (label, suffix) = match mode {
            Split::PerOperation => (format!("Operation {}: {}", g[0] + 1, first.p.kind), format!("-{:0w$}-{}", g[0] + 1, slug(&first.p.kind), w = width)),
            Split::PerTool => {
                let ops: Vec<String> = g.iter().map(|k| (k + 1).to_string()).collect();
                (format!("{} (operation{} {})", tool_first, if g.len() == 1 { "" } else { "s" }, ops.join(", ")), format!("-{:0w$}-{}", gi + 1, slug(&tool_first), w = n.to_string().len()))
            }
            Split::One => ("Everything".to_string(), String::new()),
        };

        // what to do before this file, and what comes after it
        let mut top: Vec<String> = vec![];
        if n > 1 {
            top.push(format!("(File {} of {}: {})", gi + 1, n, comment_text(&label)));
            if gi > 0 {
                let same = prev_tool.as_deref() == Some(first.p.tool.as_str()) && !two_tools(first);
                if same { top.push("(Before you start: the same tool as the previous file; keep the same zero.)".into()); }
                else { top.push(format!("(Before you start: change to {} and touch off Z0 again.)", comment_text(&tool_first))); }
            }
        }
        let mut lines = std::mem::take(&mut out.lines);
        if !top.is_empty() { lines.splice(0..0, top); }
        if let Some(m) = after {
            // before the lift and the stop at the end, so the program still ends with its own last line
            if gi + 1 < n {
                let at = lines.iter().rposition(|x| x.trim_end().ends_with("M5")).unwrap_or(lines.len());
                lines.insert(at, format!("(After this file: {})", comment_text(&m)));
            }
        }
        out.lines = lines;
        crate::summary::add_header(&mut out, &d, units, false);
        prev_tool = Some(main.ops[last].p.tool.clone()).filter(|_| !two_tools(&main.ops[last]));
        files.push(SplitFile { ops: g.clone(), suffix, label, out });
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> ProjectData {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/hardware_m0_aircut.project.json");
        crate::load_project(&std::fs::read_to_string(p).unwrap()).unwrap().data
    }

    #[test]
    fn the_groups_follow_the_mode() {
        let d = project();
        assert_eq!(groups(&d, Split::One), vec![vec![0, 1, 2]]);
        assert_eq!(groups(&d, Split::PerOperation), vec![vec![0], vec![1], vec![2]]);
        // operations 1 and 2 use the same tool, 3 another one
        assert_eq!(groups(&d, Split::PerTool), vec![vec![0, 1], vec![2]]);
    }

    #[test]
    fn the_same_tool_twice_with_another_between_makes_three_files() {
        let mut d = project();
        d.ops[1].p.tool = d.tools[1].id.clone();
        d.ops[2].p.tool = d.tools[0].id.clone();
        assert_eq!(groups(&d, Split::PerTool), vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn operations_without_shapes_are_not_files() {
        let mut d = project();
        d.loops.retain(|l| l.op != d.ops[1].id);
        assert_eq!(groups(&d, Split::PerOperation), vec![vec![0], vec![2]]);
        d.loops.clear();
        assert!(groups(&d, Split::PerOperation).is_empty());
        assert!(groups(&d, Split::One).is_empty());
    }

    #[test]
    fn names_are_plain_and_numbered_by_operation() {
        assert_eq!(slug("1/8\u{2033} flat end mill"), "1-8in-flat-end-mill");
        assert_eq!(slug("  6 mm   flat end mill "), "6-mm-flat-end-mill");
        let f = generate_files(&project(), Units::Metric, Split::PerOperation).unwrap();
        assert_eq!(f.iter().map(|x| x.suffix.as_str()).collect::<Vec<_>>(), ["-1-outside", "-2-pocket", "-3-outside"]);
        let t = generate_files(&project(), Units::Metric, Split::PerTool).unwrap();
        assert_eq!(t.iter().map(|x| x.suffix.as_str()).collect::<Vec<_>>(), ["-1-1-8in-flat-end-mill", "-2-6-mm-flat-end-mill"]);
    }
}
