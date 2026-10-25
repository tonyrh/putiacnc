//! Builds a sample sign (rounded plate, carved name, four holes), saves it and times toolpath generation.
use putiacnc_core::{generate, save_project, shapes, text, Units};
use putiacnc_core::model::Operation;

fn main() {
    let font = std::fs::read("/usr/share/texmf/fonts/opentype/public/tex-gyre/texgyrechorus-mediumitalic.otf").expect("font");
    let mut f = shapes::blank_project();
    f.name = "Sign".into();
    let d = &mut f.data;
    d.stock.w = 160.0; d.stock.h = 100.0; d.stock.t = 12.0;
    let flat = |d: &putiacnc_core::ProjectData, dia: f64| d.tools.iter().find(|t| t.shape == "flat" && (t.dia - dia).abs() < 0.01).or_else(|| d.tools.iter().find(|t| t.shape == "flat")).map(|t| t.id.clone()).unwrap();
    let big = flat(d, 3.175);
    // the strokes of a script font are thinner than a 1/8" cutter, so the text gets its own 1 mm one
    let mut t1 = d.tools.iter().find(|t| t.id == big).unwrap().clone();
    t1.id = "t_1mm".into(); t1.name = "1 mm flat end mill".into(); t1.dia = 1.0; t1.sys = "mm".into(); t1.feed = 500.0; t1.plunge = 150.0; t1.pd = 0.5;
    d.tools.push(t1);
    let small = "t_1mm".to_string();
    let op = |d: &mut putiacnc_core::ProjectData, id: i64, kind: &str, depth: f64, tool: &str| {
        let mut p = shapes::default_op_params(tool);
        p.kind = kind.into(); p.depth = depth;
        d.ops.push(Operation { id, p });
        d.oid = id; d.act = id;
    };
    op(d, 1, "outside", 12.0, &big);
    op(d, 2, "pocket", 6.0, &small);
    op(d, 3, "inside", 12.0, &big);
    d.loops.push(shapes::new_rect([20.0, 10.0], [140.0, 90.0], 10.0, 1));
    let mut t = text::text_loops(&font, 0, "PutiaCNC", text::TextStyle::plain(17.0, 1.0, text::Align::Center), [80.0, 44.0], 2).unwrap();
    d.loops.append(&mut t);
    for [x, y] in [[30.0, 20.0], [130.0, 20.0], [30.0, 80.0], [130.0, 80.0]] { d.loops.push(shapes::new_circle([x, y], 3.0, 3)); }
    std::fs::write("/tmp/sign.project.json", save_project(&f).unwrap()).unwrap();
    for i in 0..3 {
        let t0 = std::time::Instant::now();
        let o = generate(&f.data, Units::Metric).unwrap();
        let t1 = std::time::Instant::now();
        let mut sim = putiacnc_core::sim::Sim::new(&f.data, 800_000);
        sim.advance(&o.moves, o.moves.len());
        println!("  3D replay: {:.0} ms ({}x{} cells)", t1.elapsed().as_secs_f64() * 1000.0, sim.nx, sim.ny);
        println!("run {i}: {} lines, {} moves, {:.0} ms", o.lines.len(), o.moves.len(), t0.elapsed().as_secs_f64() * 1000.0);
    }
}
