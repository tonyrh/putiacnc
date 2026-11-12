//! Translation of the text the program shows to the user.
//!
//! The English text is the key: `tr("Copies")` gives "Copies" in English and, say, "Copie" in Italian. A text with no translation (or an
//! empty one) is shown in English. Translations live in `locales/<code>.json` (English text to translation), embedded in the program.
//!
//! * `tr("text")` a fixed text;
//! * `tr_f("{n} shapes; limit {max}", &[("n", &n), ("max", &max)])` a text with values put in by name, so a translation may reorder them;
//! * `tr_n(n, "{n} shape", "{n} shapes")` a text that depends on a count: a translation gives its forms as "one|other" (the key is the
//!   English "other" form).
//!
//! The G-code comments and the summary written into a G-code file stay in English: the 3D replay reads them.
//! The extraction and checking of the texts is in `i18n_tools`.
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// The languages: code, name in that language, and the translations (empty for English, the language of the texts in the code).
pub const LANGUAGES: &[(&str, &str, &str)] = &[
    ("en", "English", ""),
    ("it", "Italiano", include_str!("../locales/it.json")),
];

/// The test language: every letter accented, the text longer, wrapped in `[! !]`. It shows at a glance what was never put through `tr`
/// and which layouts break with long words.
pub const PSEUDO: &str = "pseudo";
const PSEUDO_IDX: usize = usize::MAX;

static CURRENT: AtomicUsize = AtomicUsize::new(0);

type Table = HashMap<&'static str, &'static str>;

fn tables() -> &'static Vec<Table> {
    static T: OnceLock<Vec<Table>> = OnceLock::new();
    T.get_or_init(|| LANGUAGES.iter().map(|(code, _, json)| {
        let parsed: HashMap<String, String> = if json.is_empty() { HashMap::new() } else { serde_json::from_str(json).unwrap_or_else(|e| panic!("locales/{code}.json: {e}")) };
        // leaked once, so that a text can be handed out as a `&'static str`
        parsed.into_iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| (&*Box::leak(k.into_boxed_str()), &*Box::leak(v.into_boxed_str()))).collect()
    }).collect())
}

/// Switch to a language by its code ("en", "it", ...), "pseudo" for the test language, or "auto" for the system's. Unknown codes mean English.
pub fn set_language(code: &str) {
    let code = if code == "auto" { system_language() } else { code.to_string() };
    let idx = if code == PSEUDO { PSEUDO_IDX } else { LANGUAGES.iter().position(|(c, _, _)| *c == code).unwrap_or(0) };
    CURRENT.store(idx, Ordering::Relaxed);
}

/// The code of the language in use.
pub fn language() -> &'static str {
    match CURRENT.load(Ordering::Relaxed) { PSEUDO_IDX => PSEUDO, i => LANGUAGES.get(i).map_or("en", |l| l.0) }
}

/// The language of the system if there is a translation for it, else English.
pub fn system_language() -> String {
    let loc = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    let base = loc.split(['-', '_', '.']).next().unwrap_or("");
    if LANGUAGES.iter().any(|(c, _, _)| *c == base) { base.to_string() } else { "en".to_string() }
}

fn pseudo(s: &'static str) -> &'static str {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, &'static str>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    if let Some(p) = cache.get(s) { return p; }
    let out = pseudo_text(s);
    let leaked: &'static str = Box::leak(out.into_boxed_str());
    cache.insert(s, leaked);
    leaked
}

/// `text` in the test language; what is in braces (the values put in later) is left alone.
pub fn pseudo_text(text: &str) -> String {
    let mut out = String::from("[! ");
    let (mut depth, mut letters) = (0, 0);
    for ch in text.chars() {
        match ch {
            '{' => { depth += 1; out.push(ch); }
            '}' => { depth = (depth - 1).max(0); out.push(ch); }
            _ if depth > 0 => out.push(ch),
            _ => {
                if ch.is_alphabetic() { letters += 1; }
                out.push(match ch {
                    'a' => 'á', 'e' => 'é', 'i' => 'í', 'o' => 'ö', 'u' => 'ü', 'c' => 'ç', 'n' => 'ñ', 'y' => 'ý', 's' => 'š', 't' => 'ţ',
                    'A' => 'Å', 'E' => 'É', 'I' => 'Î', 'O' => 'Ö', 'U' => 'Ü', 'C' => 'Ç', 'N' => 'Ñ', 'S' => 'Š', 'T' => 'Ţ',
                    c => c,
                });
            }
        }
    }
    // about a third longer, like the longest languages
    out.extend(std::iter::repeat_n('~', letters / 3));
    out.push_str(" !]");
    out
}

fn lookup(s: &'static str) -> Option<&'static str> {
    match CURRENT.load(Ordering::Relaxed) {
        0 => None,
        PSEUDO_IDX => Some(pseudo(s)),
        i => tables().get(i).and_then(|t| t.get(s).copied()),
    }
}

/// How much wider than in English the side panels should be for the language in use: texts there are measured from
/// fixed layouts, and longer words (Italian is about a fifth longer) would otherwise be cut off at the panel's edge.
pub fn width_factor() -> f32 {
    match language() { "en" => 1.0, PSEUDO => 1.4, "it" => 1.25, _ => 1.25 }
}

/// Marks a text in a `const` for extraction; translate it with `tr` where it is shown.
pub const fn n_(s: &'static str) -> &'static str { s }

/// A text known only at run time (from a `n_` list): translated when it is in the table, else as it is.
pub fn tr_dyn(s: &str) -> String {
    match CURRENT.load(Ordering::Relaxed) {
        0 => s.to_string(),
        PSEUDO_IDX => pseudo_text(s),
        i => tables().get(i).and_then(|t| t.get(s).copied()).unwrap_or(s).to_string(),
    }
}

/// A fixed text in the language in use.
pub fn tr(s: &'static str) -> &'static str { lookup(s).unwrap_or(s) }

fn fill(t: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut out = t.to_string();
    for (k, v) in args { out = out.replace(&format!("{{{k}}}"), &v.to_string()); }
    out
}

/// A text with values put in: each `{name}` is replaced by the value of that name.
pub fn tr_f(s: &'static str, args: &[(&str, &dyn std::fmt::Display)]) -> String { fill(tr(s), args) }

/// Which form of a text to use for a count: 0 = the singular.
fn plural_form(_lang: &str, n: u64) -> usize { if n == 1 { 0 } else { 1 } }

/// A text that depends on a count `n` (`{n}` is replaced by it): `one` for 1, `other` for the rest; a translation may have its own forms.
pub fn tr_n(n: u64, one: &'static str, other: &'static str) -> String {
    let lang = language();
    let chosen = match lookup(other) {
        Some(t) if lang == PSEUDO => t,
        Some(t) => t.split('|').nth(plural_form(lang, n)).unwrap_or(if n == 1 { one } else { other }),
        None => if n == 1 { one } else { other },
    };
    fill(chosen, &[("n", &n)])
}
