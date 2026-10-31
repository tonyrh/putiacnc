//! Prints the files of a split program (head and tail of each): cargo run --release --example splitshow -- <project.json> <tool|operation>
use putiacnc_core::split::{generate_files, Split};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = putiacnc_core::load_project(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    for x in generate_files(&f.data, putiacnc_core::Units::Metric, Split::from_id(&a[2])).unwrap() {
        println!("=== {}{}  ({} lines, {:.1} min)", f.name, x.suffix, x.out.lines.len(), x.out.time_min);
        for l in x.out.lines.iter().take(24) { println!("{l}"); }
        println!("   ...");
        for l in x.out.lines.iter().rev().take(6).rev() { println!("{l}"); }
    }
}
