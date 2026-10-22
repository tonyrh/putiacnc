use putiacnc_core::model::{Operation, Relief};
use putiacnc_core::relief::Field;
use putiacnc_core::sim::Sim;
use putiacnc_core::{generate, shapes, Units};

fn pyramid_stl() -> Vec<u8> {
    let (a, b, c, d, top) = ([0.0f32, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 20.0, 0.0], [0.0, 20.0, 0.0], [10.0f32, 10.0, 10.0]);
    let tris = [[a, b, top], [b, c, top], [c, d, top], [d, a, top], [a, c, b], [a, d, c]];
    let mut out = vec![0u8; 80];
    out.extend((tris.len() as u32).to_le_bytes());
    for t in tris { out.extend([0u8; 12]); for v in t { for x in v { out.extend(x.to_le_bytes()); } } out.extend([0u8; 2]); }
    out
}

fn project(rough: bool) -> putiacnc_core::ProjectData {
    let mut f = shapes::blank_project();
    let d = &mut f.data;
    d.stock.w = 40.0; d.stock.h = 40.0; d.stock.t = 12.0;
    let mut rel = Relief::from_stl(&pyramid_stl(), "pyramid", 0.5).unwrap();
    rel.x = 10.0; rel.y = 10.0;
    d.relief = Some(rel);
    let ball = d.tools.iter().find(|t| t.shape == "ball" && t.dia < 4.0).unwrap().id.clone();
    let flat = d.tools.iter().find(|t| t.name.starts_with("3 mm flat")).unwrap().id.clone();
    let mut p = shapes::default_op_params(&ball);
    p.kind = "relief".into();
    p.depth = 6.0;
    p.pass_depth = Some(1.5);
    if rough { p.tool2 = Some(flat); }
    d.ops.push(Operation { id: 1, p });
    d.oid = 1; d.act = 1;
    f.data
}

#[test]
fn relief_is_carved_to_the_surface_without_gouging() {
    for rough in [false, true] {
        let d = project(rough);
        let out = generate(&d, Units::Metric).unwrap();
        assert!(out.moves.iter().any(|m| m.cut));
        assert_eq!(out.moves.iter().any(|m| m.alt), rough);
        let mut s = Sim::new(&d, 160_000);
        s.advance(&out.moves, out.moves.len());
        let rel = d.relief.as_ref().unwrap();
        let f = Field::new(rel, 6.0, 2.0);
        let (mut worst_gouge, mut sum, mut n) = (0.0f32, 0.0f64, 0);
        for iy in 0..s.ny { for ix in 0..s.nx {
            let (x, y) = ((ix as f64 + 0.5) * s.cell, (iy as f64 + 0.5) * s.cell);
            if x < 12.0 || x > 28.0 || y < 12.0 || y > 28.0 { continue; }
            let (i, j) = (((x - f.ox) / f.g).round() as i64, ((y - f.oy) / f.g).round() as i64);
            let target = f.at(i, j);
            let err = s.at(ix, iy) - target;
            worst_gouge = worst_gouge.min(err);
            sum += err as f64; n += 1;
        } }
        // never deeper than the model by more than the grid's rounding, and on average close to it
        assert!(worst_gouge > -0.25, "rough={rough}: gouged {worst_gouge}");
        assert!(sum / n as f64 > -0.05 && sum / (n as f64) < 0.25, "rough={rough}: mean error {}", sum / n as f64);
    }
}

#[test]
fn relief_needs_a_model() {
    let mut d = project(true);
    d.relief = None;
    assert!(generate(&d, Units::Metric).unwrap_err().contains("3D model"));
}

#[test]
fn relief_steps_can_be_switched_off_and_tuned() {
    let full = generate(&project(true), Units::Metric).unwrap();
    // finishing only: nothing from the second tool
    let mut d = project(true);
    d.ops[0].p.r_off = true;
    let o = generate(&d, Units::Metric).unwrap();
    assert!(!o.moves.iter().any(|m| m.alt) && o.moves.iter().any(|m| m.cut));
    assert!(!o.gcode().contains("roughing"));
    // roughing only: no finishing lines, and the finishing tool is not even needed
    let mut d = project(true);
    d.ops[0].p.f_off = true;
    d.ops[0].p.tool = String::new();
    let o = generate(&d, Units::Metric).unwrap();
    assert!(o.moves.iter().all(|m| !m.cut || m.alt) && o.moves.iter().any(|m| m.alt));
    assert!(!o.gcode().contains("finishing"));
    // both off is an error
    let mut d = project(true);
    d.ops[0].p.r_off = true; d.ops[0].p.f_off = true;
    assert!(generate(&d, Units::Metric).unwrap_err().contains("nothing to cut"));
    // a wider finishing stepover means fewer lines; a bigger leave shows in the comment
    let mut d = project(true);
    d.ops[0].p.f_so = Some(40.0);
    let wide = generate(&d, Units::Metric).unwrap();
    assert!(wide.gcode().lines().count() < full.gcode().lines().count());
    let mut d = project(true);
    d.ops[0].p.r_leave = Some(0.8);
    assert!(generate(&d, Units::Metric).unwrap().gcode().contains("leaving 0.8 mm"));
}

/// Timing and size on a real model: PUTIA_STL=/path/model.stl cargo test --release -- --ignored real_model
#[test]
#[ignore]
fn real_model() {
    let Ok(path) = std::env::var("PUTIA_STL") else { return };
    let bytes = std::fs::read(path).unwrap();
    let t0 = std::time::Instant::now();
    let mut d = project(true);
    let mut rel = Relief::from_stl(&bytes, "model", 0.5).unwrap();
    println!("import {:?}, {}x{} cells, hm {} KB", t0.elapsed(), rel.nx, rel.ny, rel.hm.len() / 1024);
    let (w, h) = rel.size();
    d.stock.w = w + 10.0; d.stock.h = h + 10.0; d.stock.t = rel.zmax + 5.0;
    rel.x = 5.0; rel.y = 5.0;
    let depth = rel.zmax;
    d.relief = Some(rel);
    d.ops[0].p.depth = depth;
    d.ops[0].p.pass_depth = Some(3.0);
    d.tools.iter_mut().for_each(|t| if t.name.starts_with("3 mm flat") { t.dia = 6.0; });
    let t1 = std::time::Instant::now();
    let out = generate(&d, Units::Metric).unwrap();
    println!("generate {:?}: {} lines, {} moves, est. {:.0} min", t1.elapsed(), out.lines.len(), out.moves.len(), out.time_min);
    for n in &out.notes { println!("note: {n}"); }
    let t2 = std::time::Instant::now();
    let mut s = Sim::new(&d, 400_000);
    s.advance(&out.moves, out.moves.len());
    println!("simulate {:?}", t2.elapsed());
}
