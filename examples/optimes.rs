//! Prints when (minutes into the job) each operation starts and ends, for choosing a stretch of the 3D replay.
use putiacnc_core::{generate, load_project, Units};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let proj = load_project(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let out = generate(&proj.data, Units::Metric).unwrap();
    let mut t = 0.0;
    let mut spans: Vec<(usize, f64, f64)> = vec![];
    for m in &out.moves {
        if spans.last().map(|s| s.0) != Some(m.op) { spans.push((m.op, t, t)); }
        t += m.dt;
        spans.last_mut().unwrap().2 = t;
    }
    for (op, s, e) in spans { println!("op {} from {:.3} to {:.3} min ({:.0} s)", op + 1, s, e, (e - s) * 60.0); }
    println!("total {:.3} min", t);
}
