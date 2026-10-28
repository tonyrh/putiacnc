use putiacnc_core::pieces::{file_suffix, generate_parts};
use putiacnc_core::{load_project, Units};
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("Usage: putia-gcode <project.json> [--imperial] [-o out.nc]\nReads a PutiaCNC project file and writes GRBL G-code.\nA project with inlay plugs on a separate piece of stock writes one file per piece (out.nc, out-plug.nc).");
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
    match generate_parts(&proj.data, units) {
        Ok(parts) => {
            if parts.len() > 1 && out_path.is_none() {
                eprintln!("error: this project has inlay plugs on a separate piece of stock, so it makes {} files; give -o out.nc (the others get -plug before .nc)", parts.len());
                std::process::exit(2);
            }
            for (i, part) in parts.iter().enumerate() {
                let o = &part.out;
                for n in &o.notes { eprintln!("note: {n}"); }
                eprintln!("{} lines, cutting {:.1} mm, estimated {:.1} min", o.lines.len(), o.cut_len_mm, o.time_min);
                match &out_path {
                    Some(p) => {
                        let suffix = file_suffix(&parts, i);
                        let path = match p.strip_suffix(".nc") { Some(stem) => format!("{stem}{suffix}.nc"), None => format!("{p}{suffix}") };
                        std::fs::write(&path, o.gcode()).unwrap_or_else(|e| { eprintln!("Cannot write {path}: {e}"); std::process::exit(1) });
                        if parts.len() > 1 { eprintln!("wrote {path}"); }
                    }
                    None => { let _ = std::io::stdout().write_all(o.gcode().as_bytes()); println!(); }
                }
            }
        }
        Err(e) => { eprintln!("error: {e}"); std::process::exit(1) }
    }
}
