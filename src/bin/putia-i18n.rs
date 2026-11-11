//! Translation helper.
//!   putia-i18n extract          write locales/template.json: every text that goes through `tr`
//!   putia-i18n sync <code>      bring locales/<code>.json in line with the code: keep what is translated, add new texts empty, drop old ones
//!   putia-i18n check [<code>]   report what is missing, stale or wrong in a translation (all of them without a code); exit 1 on mistakes
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
        _ => { eprintln!("usage: putia-i18n extract | sync <code> | check [<code>]"); std::process::exit(2); }
    }
}
