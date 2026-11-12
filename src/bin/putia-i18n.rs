//! Translation helper.
//!   putia-i18n extract          write locales/template.json: every text that goes through `tr`
//!   putia-i18n sync <code>      bring locales/<code>.json in line with the code: keep what is translated, add new texts empty, drop old ones
//!   putia-i18n check [<code>]   report what is missing, stale or wrong in a translation (all of them without a code); exit 1 on mistakes
//!   putia-i18n review <code> [file.csv]   write a table to read a translation in a spreadsheet: where each text is, the English, the translation,
//!                                         and an empty Correction column (default file: <code>-review.csv)
//!   putia-i18n apply <code> <file.csv>    put the filled Correction cells of such a table into locales/<code>.json
use putiacnc_core::i18n_tools::{check, scan, sync_json, template_json};
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let found = scan(&root, &["src", "gui/src"]);
    let read = |code: &str| -> HashMap<String, String> {
        let p = root.join("locales").join(format!("{code}.json"));
        std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    };
    match args.first().map(String::as_str) {
        Some("extract") => {
            std::fs::write(root.join("locales/template.json"), template_json(&found)).unwrap();
            println!("{} texts written to locales/template.json", found.len());
        }
        Some("sync") => {
            let Some(code) = args.get(1) else { eprintln!("sync needs a language code, like `sync it`"); std::process::exit(2) };
            std::fs::write(root.join("locales").join(format!("{code}.json")), sync_json(&found, &read(code))).unwrap();
            let r = check(&found, &read(code));
            println!("locales/{code}.json: {} of {} texts still to translate", r.missing.len(), found.len());
        }
        Some("check") => {
            let codes: Vec<String> = match args.get(1) { Some(c) => vec![c.clone()], None => putiacnc_core::i18n::LANGUAGES.iter().skip(1).map(|l| l.0.to_string()).collect() };
            let mut bad = false;
            for code in codes {
                let r = check(&found, &read(&code));
                println!("{code}: {} of {} translated; {} stale, {} wrong placeholders, {} wrong plurals", found.len() - r.missing.len(), found.len(), r.stale.len(), r.bad_placeholders.len(), r.bad_plurals.len());
                for (what, list) in [("stale", &r.stale), ("placeholders differ", &r.bad_placeholders), ("needs two forms (one|other)", &r.bad_plurals)] {
                    for k in list { println!("  {what}: {k}"); bad = true; }
                }
            }
            if bad { std::process::exit(1); }
        }
        Some("review") => {
            let Some(code) = args.get(1) else { eprintln!("review needs a language code, like `review it`"); std::process::exit(2) };
            let out = args.get(2).cloned().unwrap_or_else(|| format!("{code}-review.csv"));
            let tr = read(code);
            let mut seen = std::collections::HashSet::new();
            let mut rows: Vec<(String, &str, String, String)> = Vec::new();
            for f in &found {
                if !seen.insert(f.key.as_str()) { continue; }
                rows.push((f.file.clone(), f.key.as_str(), f.one.clone().unwrap_or_default(), tr.get(&f.key).cloned().unwrap_or_default()));
            }
            rows.sort_by(|a, b| a.0.cmp(&b.0)); // by file, in the order of the code within one
            let mut csv = String::from("Where,English,English (for one),Translation,Correction,Note\n");
            for (file, key, one, t) in &rows { csv += &format!("{},{},{},{},,\n", cell(file), cell(key), cell(one), cell(t)); }
            std::fs::write(&out, csv).unwrap();
            println!("{} texts written to {out}", rows.len());
        }
        Some("apply") => {
            let (Some(code), Some(file)) = (args.get(1), args.get(2)) else { eprintln!("apply needs a language code and a file, like `apply it it-review.csv`"); std::process::exit(2) };
            let table = parse_csv(&std::fs::read_to_string(file).unwrap_or_else(|e| { eprintln!("cannot read {file}: {e}"); std::process::exit(2) }));
            let head = table.first().cloned().unwrap_or_default();
            let col = |name: &str| head.iter().position(|h| h.trim() == name);
            let (Some(en), Some(fix)) = (col("English"), col("Correction")) else { eprintln!("{file} has no English and Correction columns"); std::process::exit(2) };
            let mut tr = read(code);
            let (mut changed, mut unknown) = (0, 0);
            for row in table.iter().skip(1) {
                let (Some(k), Some(v)) = (row.get(en), row.get(fix)) else { continue };
                if v.trim().is_empty() { continue; }
                if !found.iter().any(|f| &f.key == k) { unknown += 1; println!("  not a text of the program (skipped): {k}"); continue; }
                if tr.get(k).map(String::as_str) != Some(v.as_str()) { tr.insert(k.clone(), v.clone()); changed += 1; }
            }
            std::fs::write(root.join("locales").join(format!("{code}.json")), sync_json(&found, &tr)).unwrap();
            println!("locales/{code}.json: {changed} texts changed, {unknown} skipped; run `putia-i18n check {code}` next");
        }
        _ => { eprintln!("usage: putia-i18n extract | sync <code> | check [<code>] | review <code> [file.csv] | apply <code> <file.csv>"); std::process::exit(2); }
    }
}

/// One cell of a CSV file: in quotation marks when it needs them, and with an apostrophe first where a spreadsheet would
/// otherwise read the text as a formula or a number.
fn cell(s: &str) -> String {
    let guarded = if s.starts_with(['=', '+', '-', '@', '\'']) || (!s.is_empty() && s.parse::<f64>().is_ok()) { format!("'{s}") } else { s.to_string() };
    if guarded.contains([',', '"', '\n', '\r']) || guarded.starts_with(' ') || guarded.ends_with(' ') { format!("\"{}\"", guarded.replace('"', "\"\"")) } else { guarded }
}

/// The rows of a CSV file (commas, quotation marks doubled inside a quoted cell, line ends inside one kept). The apostrophe
/// that `cell` puts first is taken off again.
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let (mut rows, mut row, mut cur, mut quoted) = (Vec::new(), Vec::new(), String::new(), false);
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' if quoted && it.peek() == Some(&'"') => { cur.push('"'); it.next(); }
            '"' => quoted = !quoted,
            ',' if !quoted => row.push(std::mem::take(&mut cur)),
            '\r' if !quoted => {}
            '\n' if !quoted => { row.push(std::mem::take(&mut cur)); rows.push(std::mem::take(&mut row)); }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() || !row.is_empty() { row.push(cur); rows.push(row); }
    for r in rows.iter_mut() { for c in r.iter_mut() { if let Some(rest) = c.strip_prefix('\'') { if rest.starts_with(['=', '+', '-', '@', '\'']) || rest.parse::<f64>().is_ok() { *c = rest.to_string(); } } } }
    rows
}
