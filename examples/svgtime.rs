//! Times SVG import: `cargo run --release --example svgtime -- file.svg`
fn main() {
    let p = std::env::args().nth(1).expect("an svg file");
    let b = std::fs::read(&p).unwrap();
    let t = std::time::Instant::now();
    let mut o = usvg::Options::default();
    o.fontdb_mut().load_system_fonts();
    println!("load system fonts: {:?} ({} faces)", t.elapsed(), o.fontdb.faces().count());
    let t = std::time::Instant::now();
    let i = putiacnc_core::svg::import(&b).unwrap();
    println!("whole import: {:?}, {} loops", t.elapsed(), i.loops.len());
}
