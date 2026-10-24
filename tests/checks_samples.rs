//! Runs the checks over project files named in PUTIA_SAMPLES (colon-separated) and prints what they find.
//! `PUTIA_SAMPLES=a.json:b.json cargo test --test checks_samples -- --ignored --nocapture`
use putiacnc_core::{checks, generate, load_project, Units};

#[test]
#[ignore]
fn samples() {
    let Ok(list) = std::env::var("PUTIA_SAMPLES") else { return };
    for path in list.split(':').filter(|s| !s.is_empty()) {
        let Ok(text) = std::fs::read_to_string(path) else { eprintln!("{path}: unreadable"); continue };
        let Ok(p) = load_project(&text) else { eprintln!("{path}: not a project"); continue };
        let Ok(o) = generate(&p.data, Units::Metric) else { eprintln!("{path}: no toolpath"); continue };
        let t = std::time::Instant::now();
        let f = checks::run(&p.data, &o);
        eprintln!("{path}: {} moves, {} findings in {:.0} ms", o.moves.len(), f.len(), t.elapsed().as_secs_f64() * 1000.0);
        for x in f { eprintln!("    {:?} (x{}): {}", x.kind, x.count, x.msg); }
    }
}
