//! Shared by the tests: does the replay of the G-code text agree with what the generator says it wrote?
#![allow(dead_code)]
use putiacnc_core::model::ProjectData;
use putiacnc_core::replay::{replay, Kind};
use putiacnc_core::model::{Operation, Relief};
use putiacnc_core::{shapes, Output, Units};
use std::collections::BTreeMap;

fn len(m: &putiacnc_core::toolpath::Move) -> f64 { ((m.to[0] - m.from[0]).powi(2) + (m.to[1] - m.from[1]).powi(2) + (m.to[2] - m.from[2]).powi(2)).sqrt() }

pub fn replay_problems(d: &ProjectData, o: &Output, units: Units) -> Vec<String> {
    let mut bad = Vec::new();
    let r = replay(&o.lines, d);
    let arcs = d.machine.as_ref().is_some_and(|m| m.arcs == Some(true));
    if !r.unknown.is_empty() { bad.push(format!("replay: {} line(s) not placed, first {:?}", r.unknown.len(), o.lines.get(r.unknown[0]))); }
    if r.moves.len() != r.info.len() { bad.push("replay: moves and info differ in length".into()); return bad; }
    if r.info.windows(2).any(|w| w[0].line > w[1].line) || r.info.iter().any(|i| i.line >= o.lines.len()) { bad.push("replay: line numbers out of order".into()); }
    // the generator does not keep the closing "G0 X0 Y0" as a move; the replay plays it, so it is set aside for the comparisons
    let end_home = r.moves.iter().zip(&r.info).rposition(|(m, i)| matches!(i.kind, Kind::Move) && m.to[2] == m.from[2] && {
        let l = &o.lines[i.line];
        l.trim_start_matches(|c: char| c == 'N' || c.is_ascii_digit()).trim() == "G0 X0 Y0"
    });
    let kept = |k: usize| end_home != Some(k);
    // what is cut
    let (rc, gc): (f64, f64) = (r.moves.iter().filter(|m| m.cut).map(len).sum(), o.cut_len_mm);
    let n = r.moves.iter().filter(|m| m.cut).count() as f64;
    let tol = 0.05 + gc * if arcs { 0.01 } else { 0.0005 } + if units == Units::Imperial { 0.002 * n } else { 0.0 };
    if (rc - gc).abs() > tol { bad.push(format!("replay cuts {rc:.2} mm, the generator {gc:.2} mm")); }
    // how long it takes (waits included)
    let rt: f64 = r.moves.iter().enumerate().filter(|(k, _)| kept(*k)).map(|(_, m)| m.dt).sum();
    if (rt - o.time_min).abs() > 0.01 + o.time_min * 0.02 { bad.push(format!("replay takes {rt:.3} min, the generator {:.3} min", o.time_min)); }
    // where it ends
    if let (Some(a), Some(b)) = (r.moves.iter().enumerate().filter(|(k, _)| kept(*k)).map(|(_, m)| m).next_back(), o.moves.last()) {
        if (a.to[0] - b.to[0]).abs() > 0.01 || (a.to[1] - b.to[1]).abs() > 0.01 { bad.push(format!("replay ends at {:?}, the generator at {:?}", a.to, b.to)); }
    }
    // per operation: the passes and the lowest Z of each, the second tool, the levelling lap
    let summary = |ms: &[putiacnc_core::toolpath::Move]| {
        let mut passes: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        let (mut alt, mut lap): (BTreeMap<usize, f64>, f64) = (BTreeMap::new(), 0.0);
        for m in ms.iter().filter(|m| m.cut) {
            if m.pass > 0 { let e = passes.entry((m.op, m.pass)).or_insert(f64::INFINITY); *e = e.min(m.from[2]).min(m.to[2]); }
            if m.alt { *alt.entry(m.op).or_default() += len(m); }
            if m.lap { lap += len(m); }
        }
        (passes, alt, lap)
    };
    let ((rp, ra, rl), (gp, ga, gl)) = (summary(&r.moves), summary(&o.moves));
    if rp.keys().ne(gp.keys()) { bad.push(format!("replay passes {:?} against the generator's {:?}", rp.keys().collect::<Vec<_>>(), gp.keys().collect::<Vec<_>>())); }
    for (k, z) in &gp { if let Some(rz) = rp.get(k) { if (rz - z).abs() > 0.01 { bad.push(format!("operation {} pass {}: replay goes down to {rz}, the generator to {z}", k.0 + 1, k.1)); } } }
    if ra.keys().ne(ga.keys()) { bad.push(format!("second tool in operations {:?} against {:?}", ra.keys().collect::<Vec<_>>(), ga.keys().collect::<Vec<_>>())); }
    for (k, g) in &ga { if let Some(rr) = ra.get(k) { if (rr - g).abs() > 0.05 + g * 0.01 { bad.push(format!("operation {}: second tool cuts {rr:.2} against {g:.2} mm", k + 1)); } } }
    if (rl - gl).abs() > 0.05 + gl * 0.01 { bad.push(format!("levelling lap {rl:.2} against {gl:.2} mm")); }
    // every stop, wait and tool in the text is a step
    let count = |f: &dyn Fn(&str) -> bool| o.lines.iter().filter(|l| f(l)).count();
    let steps = |k: &dyn Fn(&Kind) -> bool| r.info.iter().filter(|i| k(&i.kind)).count();
    if count(&|l| l.contains("M0 ")) != steps(&|k| matches!(k, Kind::Stop(_))) { bad.push("replay: the stops do not match the M0 lines".into()); }
    if count(&|l| l.contains("G4 ")) != steps(&|k| matches!(k, Kind::Wait(_))) { bad.push("replay: the waits do not match the G4 lines".into()); }
    bad
}

/// A small pyramid as a 3D relief: roughing with a flat end mill, finishing with a ball nose.
pub fn pyramid() -> ProjectData {
    let (a, b, c, d, top) = ([0.0f32, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 20.0, 0.0], [0.0, 20.0, 0.0], [10.0f32, 10.0, 10.0]);
    let tris = [[a, b, top], [b, c, top], [c, d, top], [d, a, top], [a, c, b], [a, d, c]];
    let mut stl = vec![0u8; 80];
    stl.extend((tris.len() as u32).to_le_bytes());
    for t in tris { stl.extend([0u8; 12]); for v in t { for x in v { stl.extend(x.to_le_bytes()); } } stl.extend([0u8; 2]); }
    let mut f = shapes::blank_project();
    let d = &mut f.data;
    d.stock.w = 40.0; d.stock.h = 40.0; d.stock.t = 12.0;
    let mut rel = Relief::from_stl(&stl, "pyramid", 0.5).unwrap();
    rel.x = 10.0; rel.y = 10.0;
    d.relief = Some(rel);
    let ball = d.tools.iter().find(|t| t.shape == "ball" && t.dia < 4.0).unwrap().id.clone();
    let flat = d.tools.iter().find(|t| t.name.starts_with("3 mm flat")).unwrap().id.clone();
    let mut p = shapes::default_op_params(&ball);
    p.kind = "relief".into(); p.depth = 6.0; p.d_man = true; p.pass_depth = Some(1.5); p.tool2 = Some(flat);
    d.ops.push(Operation { id: 1, p });
    d.oid = 1; d.act = 1;
    f.data
}

