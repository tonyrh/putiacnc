//! Prints how far the tool travels at rapid speed (XY only) and the estimated time for each project given.
//!   cargo run --release --example rapids -- a.project.json b.project.json
use putiacnc_core::{load_project, pieces::generate_parts, Units};
fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).expect("read");
        let p = load_project(&text).expect("load");
        match generate_parts(&p.data, Units::Metric) {
            Ok(parts) => {
                for (i, part) in parts.iter().enumerate() {
                    let rap: f64 = part.out.moves.iter().filter(|m| !m.cut).map(|m| (m.to[0] - m.from[0]).hypot(m.to[1] - m.from[1])).sum();
                    println!("{path} part {i}: rapid XY {:.0} mm, estimated {:.2} min, {} lines", rap, part.out.time_min, part.out.lines.len());
                }
            }
            Err(e) => println!("{path}: {e}"),
        }
    }
}
