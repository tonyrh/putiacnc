//! Inlay plugs on a separate piece of stock.
//!
//! The pocket is cut in the main stock; the plug can be cut from another piece (another wood, another thickness). That piece
//! belongs to the plug operation (`OpParams::piece`) and sits beside the main stock in the drawing. The job is cut in
//! parts: the main one, and one per piece, each with its own stock and work zero, so each becomes its own G-code file.
//!
//! The parts are made by splitting the project into ordinary projects and running the ordinary generator on each, so
//! everything the generator checks and writes (zero, safe height, tool limits, notes) works the same for a piece.
use crate::model::{LoopData, Operation, PlugPiece, ProjectData, Units};
use crate::toolpath::{generate, Output};

/// True for an inlay plug that is cut from a separate piece of stock.
pub fn is_piece_op(o: &Operation) -> bool { o.p.kind == "plug" && o.p.piece.is_some() }

/// One separate piece, as a project of its own.
#[derive(Debug, Clone)]
pub struct PieceJob {
    /// Id of the plug operation that uses the piece.
    pub op_id: i64,
    /// Position of that operation in the project's list (0-based).
    pub op_index: usize,
    /// The project to generate: the piece as the stock, only the plug's shapes, moved so the piece's corner is at (0, 0).
    pub data: ProjectData,
}

/// Splits a project into the main job (without the plugs that have a piece) and one job per piece.
/// All operations stay in every part, so operation numbers in messages and moves match the operation list; the parts
/// that are not theirs just have no shapes.
pub fn split(data: &ProjectData) -> (ProjectData, Vec<PieceJob>) {
    let ids: Vec<i64> = data.ops.iter().filter(|o| is_piece_op(o)).map(|o| o.id).collect();
    if ids.is_empty() { return (data.clone(), vec![]); }
    let mut main = data.clone();
    let removed: Vec<usize> = data.loops.iter().enumerate().filter(|(_, l)| ids.contains(&l.op)).map(|(i, _)| i).collect();
    main.loops.retain(|l| !ids.contains(&l.op));
    crate::shapes::tabs_after_delete(&mut main.ops, &removed);
    let mut pieces = Vec::new();
    for (k, o) in data.ops.iter().enumerate() {
        let Some(pc) = (if is_piece_op(o) { o.p.piece.as_ref() } else { None }) else { continue };
        let same = pc.same;
        let pc = &pc.effective(&data.stock);
        let mut d = data.clone();
        d.stock = pc.stock();
        d.loops = data.loops.iter().filter(|l| l.op == o.id).cloned().map(|mut l| { crate::shapes::translate(&mut l, -pc.x, -pc.y); l }).collect::<Vec<LoopData>>();
        // a piece that takes the main stock's place sits among the same clamps; one beside it stands where it is drawn, so the
        // clamps are seen from its own corner
        if !same { for c in d.clamps.iter_mut() { c.x -= pc.x; c.y -= pc.y; } }
        d.relief = None;
        d.groups.clear();
        d.hidden_ops.clear();
        for op in d.ops.iter_mut() { op.p.tab_pts = None; op.p.piece = None; }
        pieces.push(PieceJob { op_id: o.id, op_index: k, data: d });
    }
    (main, pieces)
}

impl PieceJob {
    /// True when this piece takes the main stock's place on the machine.
    pub fn op_index_piece_same(&self, data: &ProjectData) -> bool {
        data.ops.get(self.op_index).and_then(|o| o.p.piece.as_ref()).is_some_and(|p| p.same)
    }
}

/// What the generator made for one part of the job.
#[derive(Debug, Clone)]
pub struct PartOutput {
    /// The piece's job, `None` for the main one.
    pub piece: Option<PieceJob>,
    pub out: Output,
}

/// Generates the main job and, for each separate piece, its own job. The first part is always the main one.
pub fn generate_parts(data: &ProjectData, units: Units) -> Result<Vec<PartOutput>, String> {
    let (main, pieces) = split(data);
    let ids: Vec<usize> = pieces.iter().map(|p| p.op_index).collect();
    let mut out_main = generate(&main, units)?;
    // the plugs that have a piece are not in this part; drop the "no shapes" notes about them
    out_main.notes.retain(|n| !ids.iter().any(|&k| *n == format!("Op {} has no shapes.", k + 1)));
    crate::summary::add_header(&mut out_main, &main, units, false);
    let mut parts = vec![PartOutput { piece: None, out: out_main }];
    for pj in pieces {
        let mut out = generate(&pj.data, units)?;
        // in a piece's job every other operation has no shapes: not worth a note
        out.notes.retain(|n| !(n.starts_with("Op ") && n.ends_with(" has no shapes.") && *n != format!("Op {} has no shapes.", pj.op_index + 1)));
        // the header says what this file is: the plug's piece, not "the stock"
        let replaces = pj.op_index_piece_same(data);
        if let Some(l) = out.lines.iter_mut().find(|l| l.contains("(Stock ")) {
            *l = l.replacen("(Stock ", "(Plug piece ", 1).replacen("of stock", "of the piece", 1);
            if replaces { if let Some(i) = l.find(';') { l.insert_str(i, " [replaces the main stock, same work zero]"); } }
        }
        crate::summary::add_header(&mut out, &pj.data, units, true);
        parts.push(PartOutput { piece: Some(pj), out });
    }
    Ok(parts)
}

/// File-name ending of a part: "" for the main one, "-plug" for the only piece, "-plug-3" (the operation number) for several.
pub fn file_suffix(parts: &[PartOutput], i: usize) -> String {
    match &parts[i].piece {
        None => String::new(),
        Some(pj) if parts.len() == 2 => { let _ = pj; "-plug".into() }
        Some(pj) => format!("-plug-{}", pj.op_index + 1),
    }
}

/// A new piece the size of the main stock, beside it with a gap of 10 mm (the drawing position the plug is placed at). Its look is a
/// contrasting wood, since a plug is usually cut from another one.
pub fn default_piece(main: &crate::model::Stock) -> PlugPiece {
    PlugPiece { x: main.w + 10.0, y: 0.0, w: main.w, h: main.h, t: main.t, mat: if main.mat == "wood" { "darkwood".into() } else if main.mat == "darkwood" { "wood".into() } else { main.mat.clone() }, zr: main.zr.clone(), zm: main.zm.clone(), zh: main.zh, same: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::{add_operation, blank_project, new_rect};

    /// A project with a pocket and its plug on a separate piece; the pocket is a 20 x 10 rectangle at (30, 40).
    fn inlay_with_piece() -> ProjectData {
        let mut f = blank_project();
        let d = &mut f.data;
        d.loops.push(new_rect([30.0, 40.0], [50.0, 50.0], 0.0, 0));
        let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
        assert!(add_operation(d, &sel, "inlay").is_none());
        let plug_id = d.ops[1].id;
        let piece = PlugPiece { t: 8.0, mat: "darkwood".into(), ..default_piece(&d.stock) };
        crate::shapes::put_plug_on_piece(d, d.ops[0].id, plug_id, Some(piece));
        f.data
    }

    #[test]
    fn a_project_without_a_piece_is_one_part_like_before() {
        let mut f = blank_project();
        let d = &mut f.data;
        d.loops.push(new_rect([30.0, 40.0], [50.0, 50.0], 0.0, 0));
        let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
        add_operation(d, &sel, "inlay");
        let parts = generate_parts(d, Units::Metric).unwrap();
        assert_eq!(parts.len(), 1);
        // the same program, under the summary comments
        let plain = generate(d, Units::Metric).unwrap().lines;
        assert!(parts[0].out.lines.ends_with(&plain));
    }

    #[test]
    fn the_plug_goes_to_its_own_job_with_its_own_stock_and_zero() {
        let d = inlay_with_piece();
        let parts = generate_parts(&d, Units::Metric).unwrap();
        assert_eq!(parts.len(), 2, "main and one piece");
        let (main, plug) = (&parts[0].out, &parts[1].out);
        // headers: the main stock and the piece
        assert!(main.lines.iter().any(|l| l.contains("Stock 200 x 150 x 12")), "{:?}", &main.lines[..3]);
        assert!(plug.lines.iter().any(|l| l.contains("Plug piece 200 x 150 x 8")), "{:?}", &plug.lines[..3]);
        // the plug is cut only in its own file
        assert!(main.lines.iter().any(|l| l.contains("inlay pocket")) && !main.lines.iter().any(|l| l.contains("inlay plug")));
        assert!(plug.lines.iter().any(|l| l.contains("inlay plug")) && !plug.lines.iter().any(|l| l.contains("inlay pocket")));
        // the plug's coordinates are on the piece (0..200 x 0..150), at the pocket's place: the plug is 20 mm wide around x 30..50
        let xs: Vec<f64> = plug.moves.iter().flat_map(|m| [m.from[0], m.to[0]]).collect();
        assert!(xs.iter().all(|&x| (-0.01..=200.01).contains(&x)), "outside the piece: {:?}", xs.iter().cloned().fold(f64::NAN, f64::max));
        assert!(xs.iter().any(|&x| (25.0..=55.0).contains(&x)));
        assert_eq!(file_suffix(&parts, 0), "");
        assert_eq!(file_suffix(&parts, 1), "-plug");
        // no note about operations that belong to the other file
        assert!(!main.notes.iter().chain(plug.notes.iter()).any(|n| n.ends_with("has no shapes.")), "{:?} {:?}", main.notes, plug.notes);
    }

    #[test]
    fn a_piece_that_replaces_the_main_stock_has_its_size_zero_and_clamps() {
        let mut d = inlay_with_piece();
        d.stock.zr = "ur".into();
        d.clamps.push(crate::model::Clamp { x: 0.0, y: 0.0, w: 10.0, h: 10.0, height: 20.0 });
        // a piece of its own beside the stock (drawn at x 210) sees the clamp from its own corner: 210 mm to the left
        let (_, jobs) = split(&d);
        assert_eq!(jobs[0].data.clamps.len(), 1);
        assert!((jobs[0].data.clamps[0].x + 210.0).abs() < 1e-9 && jobs[0].data.clamps[0].y.abs() < 1e-9, "{:?}", jobs[0].data.clamps[0]);
        {
            let pc = d.ops[1].p.piece.as_mut().unwrap();
            pc.w = 50.0; pc.h = 40.0; pc.zr = "c".into(); pc.same = true;
        }
        let (_, jobs) = split(&d);
        let st = &jobs[0].data.stock;
        assert_eq!((st.w, st.h, st.t), (200.0, 150.0, 8.0), "main size, own thickness");
        assert_eq!(st.zr, "ur", "the main stock's zero");
        assert_eq!(jobs[0].data.clamps.len(), 1, "it sits among the same clamps");
        let parts = generate_parts(&d, Units::Metric).unwrap();
        assert!(parts[1].out.lines.iter().any(|l| l.contains("Plug piece 200 x 150 x 8") && l.contains("replaces the main stock")), "{:?}", &parts[1].out.lines[..3]);
        // saved and loaded: the flag stays; projects without it load as before
        let mut p = blank_project();
        p.data = d.clone();
        let back = crate::load_project(&crate::save_project(&p).unwrap()).unwrap();
        assert!(back.data.ops[1].p.piece.as_ref().unwrap().same);
        let mut q = blank_project();
        q.data = inlay_with_piece();
        assert!(!crate::save_project(&q).unwrap().contains("\"same\""));
    }

    #[test]
    fn the_piece_has_its_own_work_zero() {
        let mut d = inlay_with_piece();
        d.ops[1].p.piece.as_mut().unwrap().zr = "c".into();
        let parts = generate_parts(&d, Units::Metric).unwrap();
        assert!(parts[1].out.lines.iter().any(|l| l.contains("XY0 = center")), "{:?}", &parts[1].out.lines[..3]);
        assert!(parts[0].out.lines.iter().any(|l| l.contains("XY0 = lower-left")));
        // the plug is at x 30..50 on the piece (moves are in the piece's own frame, from its lower-left corner), so with the zero at
        // its centre (100, 75) the G-code has it at about x -70..-50, while the same plug with the zero in the corner is at +30..+50
        let xs = |p: &PartOutput| -> Vec<f64> { p.out.lines.iter().filter(|l| l.starts_with("G1") || l.starts_with("G0") || l.starts_with("G2") || l.starts_with("G3")).filter_map(|l| l.split_whitespace().find_map(|w| w.strip_prefix('X')).and_then(|v| v.parse().ok())).collect() };
        let c = xs(&parts[1]);
        assert!(!c.is_empty() && c.iter().filter(|&&x| x < 0.0).count() > c.len() / 2, "{c:?}");
        d.ops[1].p.piece.as_mut().unwrap().zr = "ll".into();
        let l = xs(&generate_parts(&d, Units::Metric).unwrap()[1]);
        assert!(l.iter().filter(|&&x| (20.0..60.0).contains(&x)).count() > l.len() / 2, "{l:?}");
    }

    #[test]
    fn piece_stock_and_checks_use_the_piece() {
        let mut d = inlay_with_piece();
        // a piece too thin for the plug's depth is reported in the piece's job only
        d.ops[1].p.piece.as_mut().unwrap().t = 1.0;
        let parts = generate_parts(&d, Units::Metric).unwrap();
        assert!(parts[1].out.notes.iter().any(|n| n.contains("stock thickness")), "{:?}", parts[1].out.notes);
        assert!(!parts[0].out.notes.iter().any(|n| n.contains("stock thickness")), "{:?}", parts[0].out.notes);
    }

    #[test]
    fn the_piece_is_saved_and_old_projects_load_without_one() {
        let d = inlay_with_piece();
        let f = crate::model::ProjectFile { app: "putia-cnc".into(), v: 1, name: "x".into(), data: d.clone() };
        let back = crate::load_project(&crate::save_project(&f).unwrap()).unwrap();
        assert_eq!(back.data.ops[1].p.piece, d.ops[1].p.piece);
        assert!(back.data.ops[1].p.piece.is_some());
        // a project saved before pieces existed has no `piece` key at all
        let j = crate::save_project(&crate::shapes::blank_project()).unwrap();
        assert!(!j.contains("piece"));
    }
}
