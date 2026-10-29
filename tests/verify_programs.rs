//! The independent G-code checker against what the generator really writes: no problems found in any program, and the length and depth
//! it measures from the text agree with what the generator says it wrote (so the moves every other check works on are the moves
//! the machine gets).
use putiacnc_core::model::{Operation, ProjectData, Relief};
use putiacnc_core::verify::trace;
use putiacnc_core::{generate, load_project, shapes, Units};

fn sample(name: &str) -> ProjectData {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/data/{name}.project.json"));
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap().data
}

fn pyramid() -> ProjectData {
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

/// An inlay: the pocket is operation 0 and its plug operation 1.
fn inlay() -> ProjectData {
    let mut f = shapes::blank_project();
    let d = &mut f.data;
    d.loops.push(shapes::new_rect([30.0, 40.0], [50.0, 50.0], 0.0, 0));
    let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
    assert!(shapes::add_operation(d, &sel, "inlay").is_none());
    f.data
}

/// Every setting that changes how the text looks or what it holds.
fn variants(base: &ProjectData) -> Vec<(String, ProjectData)> {
    let mut v = vec![("plain".to_string(), base.clone())];
    let mut d = base.clone(); d.job.post = "mach".into(); v.push(("mach".into(), d));
    let mut d = base.clone(); d.job.numbered = true; v.push(("numbered".into(), d));
    let mut d = base.clone(); d.job.air = true; v.push(("air".into(), d));
    let mut d = base.clone(); for o in d.ops.iter_mut() { o.p.ramp = true; } v.push(("ramp".into(), d));
    let mut d = base.clone(); d.machine.get_or_insert_with(Default::default).arcs = Some(true); v.push(("arcs".into(), d));
    v
}

fn check(name: &str, d: &ProjectData) {
    for units in [Units::Metric, Units::Imperial] {
        let out = generate(d, units).unwrap_or_else(|e| panic!("{name}: {e}"));
        let t = trace(&out.lines);
        assert!(t.hazards.is_empty(), "{name} ({units:?}): {:?}", t.hazards);
        // what the text cuts, against what the generator counted; arcs shorten the program a little (a curve for a chain of short lines)
        let tol = 0.05 + out.cut_len_mm * if d.machine.as_ref().is_some_and(|m| m.arcs == Some(true)) { 0.01 } else { 0.0005 };
        assert!((t.cut_len_mm - out.cut_len_mm).abs() <= tol, "{name} ({units:?}): text cuts {:.3} mm, the generator says {:.3} mm", t.cut_len_mm, out.cut_len_mm);
        // the lowest cut: the program's Z is the drawing's Z plus the stock offset
        let st = &d.stock;
        let zt = if st.zm == "top" { 0.0 } else { st.t - st.zh };
        let low = out.moves.iter().filter(|m| m.cut).map(|m| m.from[2].min(m.to[2])).fold(f64::MAX, f64::min) + zt;
        assert!((t.deepest_cut_mm.unwrap() - low).abs() < 0.01, "{name} ({units:?}): deepest {:?} vs {low}", t.deepest_cut_mm);
    }
}

#[test]
fn the_sample_projects_are_clean_in_every_variant() {
    for name in ["outside_holes", "pocket_island", "vbit_profile", "tabs", "imperial_center", "vcarve_circle", "outside_ramp", "two_ops_toolchange"] {
        for (v, d) in variants(&sample(name)) { check(&format!("{name}/{v}"), &d); }
    }
}

#[test]
fn a_relief_and_the_inlay_operations_are_clean_too() {
    for (v, d) in variants(&pyramid()) { check(&format!("relief/{v}"), &d); }
    for (v, d) in variants(&inlay()) { check(&format!("inlay/{v}"), &d); }
}

#[test]
fn a_plug_on_its_own_piece_is_clean() {
    let mut d = inlay();
    let k = d.ops.iter().position(|o| o.p.kind == "plug").unwrap();
    d.ops[k].p.piece = Some(putiacnc_core::model::PlugPiece { x: 0.0, y: 0.0, w: 100.0, h: 100.0, t: 12.0, mat: "wood".into(), zr: "ll".into(), zm: "top".into(), zh: 0.0, same: false });
    for part in putiacnc_core::pieces::generate_parts(&d, Units::Metric).unwrap() {
        let t = trace(&part.out.lines);
        assert!(t.hazards.is_empty(), "{:?}", t.hazards);
    }
}
