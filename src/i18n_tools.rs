//! Finding the texts that go through `tr`, writing the template for translators and checking a translation against the code.
//! Used by `putia-i18n` and by the tests.
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// A text found in the code.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The English text (for `tr_n` the "other" form, which is the key).
    pub key: String,
    /// The singular form of a `tr_n` text.
    pub one: Option<String>,
    pub file: String,
    pub line: usize,
}

/// Reads one string literal that starts at `s[i]` (the opening quote); returns its text, unescaped, and where it ends.
fn literal(s: &[u8], mut i: usize) -> Option<(String, usize)> {
    if s.get(i) != Some(&b'"') { return None; }
    i += 1;
    let mut out = Vec::new();
    while i < s.len() {
        match s[i] {
            b'"' => return Some((String::from_utf8_lossy(&out).into_owned(), i + 1)),
            b'\\' => {
                i += 1;
                match s.get(i)? {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'\\' => out.push(b'\\'),
                    b'"' => out.push(b'"'),
                    b'\'' => out.push(b'\''),
                    b'\n' => { while s.get(i + 1).is_some_and(|c| c.is_ascii_whitespace()) { i += 1; } }
                    b'u' => {
                        let end = s[i..].iter().position(|&c| c == b'}')? + i;
                        let hex = std::str::from_utf8(&s[i + 2..end]).ok()?;
                        let ch = char::from_u32(u32::from_str_radix(hex, 16).ok()?)?;
                        let mut b = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                        i = end;
                    }
                    _ => return None,
                }
                i += 1;
            }
            c => { out.push(c); i += 1; }
        }
    }
    None
}

fn skip_ws(s: &[u8], mut i: usize) -> usize { while s.get(i).is_some_and(|c| c.is_ascii_whitespace()) { i += 1; } i }

/// The texts in one source file: calls of `tr("..")`, `tr_f("..", ..)` and `tr_n(n, "..", "..")`.
pub fn scan_source(src: &str, file: &str) -> Vec<Found> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    for (name, plural) in [("tr_n(", true), ("tr_f(", false), ("tr(", false)] {
        let mut from = 0;
        while let Some(p) = src[from..].find(name) {
            let at = from + p;
            from = at + name.len();
            // a whole word: not the end of another name, as in `str(` or `entr(`
            if at > 0 && (b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_') { continue; }
            let line = src[..at].matches('\n').count() + 1;
            if plural {
                // tr_n(n, "one", "other"): the first argument is any expression up to the first comma outside brackets
                let (mut depth, mut i) = (0, from);
                while i < b.len() {
                    match b[i] { b'(' | b'[' | b'{' => depth += 1, b')' | b']' | b'}' => { if depth == 0 { break; } depth -= 1; } b',' if depth == 0 => break, b'"' => { if let Some((_, e)) = literal(b, i) { i = e; continue; } } _ => {} }
                    i += 1;
                }
                let i = skip_ws(b, i + 1);
                if let Some((one, e)) = literal(b, i) {
                    let j = skip_ws(b, e);
                    if b.get(j) == Some(&b',') {
                        if let Some((other, _)) = literal(b, skip_ws(b, j + 1)) { out.push(Found { key: other, one: Some(one), file: file.to_string(), line }); }
                    }
                }
            } else if let Some((key, _)) = literal(b, skip_ws(b, from)) {
                out.push(Found { key, one: None, file: file.to_string(), line });
            }
        }
    }
    out.sort_by_key(|a| a.line);
    out
}

fn walk(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() { walk(&p, files); } else if p.extension().is_some_and(|e| e == "rs") { files.push(p); }
    }
}

/// Every text in the `.rs` files under `dirs`, each once (the first place it is used), in the order of the files.
pub fn scan(root: &Path, dirs: &[&str]) -> Vec<Found> {
    let mut files = Vec::new();
    for d in dirs { walk(&root.join(d), &mut files); }
    let (mut seen, mut out) = (BTreeSet::new(), Vec::new());
    for f in files {
        // the module that tests the scanner itself writes example calls
        if f.ends_with("i18n_tools.rs") || f.ends_with("i18n.rs") { continue; }
        let Ok(src) = std::fs::read_to_string(&f) else { continue };
        let rel = f.strip_prefix(root).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        for x in scan_source(&src, &rel) { if seen.insert(x.key.clone()) { out.push(x); } }
    }
    out
}

/// The template: every text with an empty translation, as JSON in the order of the code, a line per text.
pub fn template_json(found: &[Found]) -> String {
    let mut s = String::from("{\n");
    for (i, f) in found.iter().enumerate() {
        s += &format!("  {}: \"\"{}\n", json_str(&f.key), if i + 1 < found.len() { "," } else { "" });
    }
    s + "}\n"
}

/// A locale file written with the template's order: texts translated so far are kept, new ones are added empty, texts no longer in the code dropped.
pub fn sync_json(found: &[Found], old: &HashMap<String, String>) -> String {
    let mut s = String::from("{\n");
    for (i, f) in found.iter().enumerate() {
        s += &format!("  {}: {}{}\n", json_str(&f.key), json_str(old.get(&f.key).map_or("", |v| v)), if i + 1 < found.len() { "," } else { "" });
    }
    s + "}\n"
}

fn json_str(s: &str) -> String { serde_json::to_string(s).unwrap() }

/// The `{names}` in a text.
pub fn placeholders(s: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = s;
    while let Some(a) = rest.find('{') {
        let Some(b) = rest[a..].find('}') else { break };
        out.insert(rest[a + 1..a + b].to_string());
        rest = &rest[a + b + 1..];
    }
    out
}

/// What is wrong with a translation: texts it has that the code no longer has, placeholders that differ, and the texts it still lacks.
#[derive(Default, Debug)]
pub struct Report { pub stale: Vec<String>, pub bad_placeholders: Vec<String>, pub bad_plurals: Vec<String>, pub missing: Vec<String> }

pub fn check(found: &[Found], locale: &HashMap<String, String>) -> Report {
    let keys: HashMap<&str, &Found> = found.iter().map(|f| (f.key.as_str(), f)).collect();
    let mut r = Report::default();
    let mut names: Vec<&String> = locale.keys().collect();
    names.sort();
    for k in names {
        let v = &locale[k];
        match keys.get(k.as_str()) {
            None => r.stale.push(k.clone()),
            Some(_) if v.is_empty() => {}
            Some(f) => {
                let forms: Vec<&str> = v.split('|').collect();
                if f.one.is_some() && forms.len() != 2 { r.bad_plurals.push(k.clone()); }
                let want = placeholders(&f.key);
                // a form may leave `{n}` out ("una forma") but may not add other names
                if forms.iter().any(|t| !placeholders(t).is_subset(&want)) || (f.one.is_none() && placeholders(v) != want) { r.bad_placeholders.push(k.clone()); }
            }
        }
    }
    for f in found { if locale.get(&f.key).is_none_or(|v| v.is_empty()) { r.missing.push(f.key.clone()); } }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_three_kinds_of_call() {
        let src = "let a = tr(\"Hello\");\nlet b = tr_f(\"{n} of {m}\", &[(\"n\", &1)]);\nlet c = tr_n(n + 1, \"{n} shape\", \"{n} shapes\");\nlet d = str(\"no\");\nlet e = crate::i18n::tr(\"Tab\\there \\u{b7} ok\");";
        let f = scan_source(src, "x.rs");
        let keys: Vec<&str> = f.iter().map(|x| x.key.as_str()).collect();
        assert_eq!(keys, ["Hello", "{n} of {m}", "{n} shapes", "Tab\there \u{b7} ok"]);
        assert_eq!(f[2].one.as_deref(), Some("{n} shape"));
    }

    #[test]
    fn placeholders_are_compared_by_name() {
        let found = vec![Found { key: "{n} of {m}".into(), one: None, file: "x".into(), line: 1 }];
        let mut loc = HashMap::new();
        loc.insert("{n} of {m}".to_string(), "{m} su {n}".to_string());
        assert!(check(&found, &loc).bad_placeholders.is_empty());
        loc.insert("{n} of {m}".to_string(), "{n} su".to_string());
        assert_eq!(check(&found, &loc).bad_placeholders.len(), 1);
        loc.insert("gone".to_string(), "x".to_string());
        assert_eq!(check(&found, &loc).stale, ["gone"]);
    }
}
