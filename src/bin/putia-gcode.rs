use putiacnc_core::{generate, load_project, Units};
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("Usage: putia-gcode <project.json> [--imperial] [-o out.nc]\nReads a PutiaCNC project file and writes GRBL G-code.");
        std::process::exit(if args.is_empty() { 2 } else { 0 });
    }
    let mut units = Units::Metric;
    let mut out_path: Option<String> = None;
    let mut input: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--imperial" => units = Units::Imperial,
            "-o" => { i += 1; out_path = args.get(i).cloned(); }
            a => input = Some(a.to_string()),
        }
        i += 1;
    }
    let input = input.unwrap_or_else(|| { eprintln!("No project file given."); std::process::exit(2) });
    let text = std::fs::read_to_string(&input).unwrap_or_else(|e| { eprintln!("Cannot read {input}: {e}"); std::process::exit(1) });
    let proj = load_project(&text).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1) });
    match generate(&proj.data, units) {
        Ok(o) => {
            for n in &o.notes { eprintln!("note: {n}"); }
            eprintln!("{} lines, cutting {:.1} mm, estimated {:.1} min", o.lines.len(), o.cut_len_mm, o.time_min);
            match out_path {
                Some(p) => std::fs::write(&p, o.gcode()).unwrap_or_else(|e| { eprintln!("Cannot write {p}: {e}"); std::process::exit(1) }),
                None => { let _ = std::io::stdout().write_all(o.gcode().as_bytes()); println!(); }
            }
        }
        Err(e) => { eprintln!("error: {e}"); std::process::exit(1) }
    }
}
