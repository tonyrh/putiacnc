//! Builds a sample project with an inlay whose plug is cut from a separate piece of stock (for screenshots and tests).
//! Usage: cargo run --release --example inlaypiece -- <out.json>
use putiacnc_core::model::{PlugPiece, ProjectFile};
use putiacnc_core::{pieces, save_project, shapes};

fn main() {
    let Some(out) = std::env::args().nth(1) else { eprintln!("Usage: cargo run --release --example inlaypiece -- <out.json>"); std::process::exit(2) };
    let mut f: ProjectFile = shapes::blank_project();
    f.name = "Inlay on two pieces".into();
    let d = &mut f.data;
    d.stock.w = 120.0; d.stock.h = 80.0; d.stock.t = 15.0; d.stock.mat = "wood".into();
    d.loops.push(shapes::new_rect([25.0, 20.0], [95.0, 60.0], 8.0, 0));
    d.loops.push(shapes::new_circle([60.0, 40.0], 10.0, 0));
    let sel: std::collections::BTreeSet<usize> = [0, 1].into_iter().collect();
    shapes::add_operation(d, &sel, "inlay");
    let (pocket, plug) = (d.ops[0].id, d.ops[1].id);
    let piece = PlugPiece { t: 8.0, mat: "dark".into(), ..pieces::default_piece(&d.stock) };
    shapes::put_plug_on_piece(d, pocket, plug, Some(piece));
    std::fs::write(&out, save_project(&f).unwrap()).unwrap();
    println!("wrote {out}");
}
