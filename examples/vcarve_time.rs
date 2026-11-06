//! Times V-carve generation on a big text sign (a profiling aid, not a test).
use putiacnc_core::{generate, shapes, text, Units};
fn main() {
    let font = std::fs::read("/usr/share/texmf/fonts/opentype/public/tex-gyre/texgyreheros-bold.otf").or_else(|_| std::fs::read("/usr/share/texmf/fonts/opentype/public/tex-gyre/texgyrechorus-mediumitalic.otf")).expect("font");
    for (word, size, depth, flat) in [("PUTIA", 60.0, 8.0, None), ("PUTIA CNC", 120.0, 15.0, None), ("PUTIA CNC", 120.0, 15.0, Some(3.0)), ("PUTIA", 250.0, 30.0, None)] {
        let mut d = shapes::blank_project().data;
        d.stock.w = 1400.0; d.stock.h = 400.0; d.stock.t = 40.0;
        let mut v = d.tools.iter().find(|t| t.shape == "vbit").unwrap().clone();
        v.id = "v90".into(); v.angle = 90.0; v.dia = 40.0;
        d.tools.push(v);
        let mut p = shapes::default_op_params("v90");
        p.kind = "vcarve".into(); p.depth = depth; p.flat = flat;
        if flat.is_some() { p.tool2 = d.tools.iter().find(|t| t.shape == "flat" && t.dia < 3.5).map(|t| t.id.clone()); }
        d.ops.push(putiacnc_core::model::Operation { id: 1, p });
        d.oid = 2; d.act = 1;
        let mut t = text::text_loops(&font, 0, word, text::TextStyle::plain(size, 1.0, text::Align::Left), [20.0, 100.0], 1).unwrap();
        d.loops.append(&mut t);
        let t0 = std::time::Instant::now();
        let out = generate(&d, Units::Metric).unwrap();
        let t1 = std::time::Instant::now();
        let mut sim = putiacnc_core::sim::Sim::new(&d, 800_000);
        sim.advance(&out.moves, out.moves.len());
        let sim_s = t1.elapsed().as_secs_f64();
        let sum: f64 = sim.z.iter().map(|v| *v as f64).sum();
        println!("  3D replay {sim_s:.2}s, surface sum {sum:.4}");
        println!("{word} {size} depth {depth} flat {flat:?}: {:.2}s, {} moves", t0.elapsed().as_secs_f64(), out.moves.len());
    }
}
