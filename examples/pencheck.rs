//! Builds the project for trying a pen on a real machine, safely: first everything is drawn in the air (10 mm above the paper), then
//! a pause to put the paper under the pen, then the same drawing for real.
//! Usage: cargo run --release --example pencheck -- tests/data/hardware_pen_test.project.json
use putiacnc_core::model::{ProjectFile, Tool};
use putiacnc_core::{save_project, shapes};

fn main() {
    let Some(out) = std::env::args().nth(1) else { eprintln!("Usage: cargo run --release --example pencheck -- <out.json>"); std::process::exit(2) };
    let mut f: ProjectFile = shapes::blank_project();
    f.name = "Pen test".into();
    let d = &mut f.data;
    d.stock.w = 100.0; d.stock.h = 80.0; d.stock.t = 1.0; d.stock.mat = "other".into();
    d.tools = vec![Tool {
        id: "tpentest1".into(), name: "Fine pen 0.5 mm".into(), sys: "mm".into(), shape: "pen".into(), angle: 60.0, flutes: 1, dia: 0.5, rpm: 0.0,
        feed: 1500.0, plunge: 300.0, so: 100.0, pd: 1.0, notes: "A pen held by the router clamp: it moves up and down by Z alone.".into(),
        tip: 0.0, arc: 0.0, concave: false, flute: 0.0, shank: 0.0, stick: 0.0, color: None,
    }];
    let tool = d.tools[0].id.clone();
    // the same drawing twice: a rectangle, a circle and a zigzag line (an open path)
    for i in 0..2usize {
        d.loops.push(shapes::new_rect([15.0, 15.0], [45.0, 35.0], 0.0, 0));
        d.loops.push(shapes::new_circle([75.0, 25.0], 8.0, 0));
        d.loops.push(shapes::new_poly(&[[15.0, 55.0], [30.0, 70.0], [45.0, 55.0], [60.0, 70.0], [75.0, 55.0]], false, 0).expect("a zigzag"));
        let n = d.loops.len();
        let sel: std::collections::BTreeSet<usize> = (n - 3..n).collect();
        shapes::add_operation(d, &sel, "draw");
        let o = d.ops.last_mut().unwrap();
        o.p.tool = tool.clone();
        o.p.depth = 0.0;
        o.p.lift = Some(2.0);
        o.p.air = i == 0;
    }
    d.ops[0].p.pause = Some("TEST 1 done in the air. Put paper under the pen, set Z0 on it. Resume".into());
    std::fs::write(&out, save_project(&f).unwrap()).unwrap();
    println!("wrote {out}");
}
