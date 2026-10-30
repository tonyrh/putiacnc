//! Builds the project for testing the pauses (`M0`) on a real machine, safely: every operation is an air cut (10 mm above the stock).
//! It has a "Pause afterwards" and a tool change, the two things that write an `M0`.
//! Usage: cargo run --release --example m0check -- tests/data/hardware_m0_aircut.project.json
use putiacnc_core::model::ProjectFile;
use putiacnc_core::{save_project, shapes};

fn main() {
    let Some(out) = std::env::args().nth(1) else { eprintln!("Usage: cargo run --release --example m0check -- <out.json>"); std::process::exit(2) };
    let mut f: ProjectFile = shapes::blank_project();
    f.name = "M0 test (air cut)".into();
    let d = &mut f.data;
    d.stock.w = 100.0; d.stock.h = 80.0; d.stock.t = 12.0; d.stock.mat = "wood".into();
    // a second tool, 6 mm: changing to it writes the tool-change M0
    let mut big = d.tools[0].clone();
    big.id = format!("{}b", big.id);
    big.name = "6 mm flat end mill".into();
    big.sys = "mm".into();
    big.dia = 6.0;
    let (small_id, big_id) = (d.tools[0].id.clone(), big.id.clone());
    d.tools.push(big);
    // 1: a rectangle with the small tool, then a pause; 2: a circle with the same tool; 3: a rectangle with the 6 mm tool
    d.loops.push(shapes::new_rect([15.0, 15.0], [45.0, 35.0], 0.0, 0));
    d.loops.push(shapes::new_circle([75.0, 25.0], 8.0, 0));
    d.loops.push(shapes::new_rect([15.0, 50.0], [45.0, 70.0], 0.0, 0));
    for (i, kind, tool) in [(0usize, "outside", &small_id), (1, "pocket", &small_id), (2, "outside", &big_id)] {
        let sel: std::collections::BTreeSet<usize> = [i].into_iter().collect();
        shapes::add_operation(d, &sel, kind);
        let o = d.ops.last_mut().unwrap();
        o.p.tool = tool.clone();
        o.p.depth = 3.0;
        o.p.air = true;
    }
    d.ops[0].p.pause = Some("TEST 1: spindle off, tool up. Press Resume".into());
    std::fs::write(&out, save_project(&f).unwrap()).unwrap();
    println!("wrote {out}");
}
