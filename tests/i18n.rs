//! The translation files match the code: the template lists every text, no translation has a text the code lacks, placeholders agree.
use putiacnc_core::i18n::{self, tr, tr_f, tr_n};
use putiacnc_core::i18n_tools::{check, scan, template_json};
use std::collections::HashMap;
use std::path::PathBuf;

fn root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

#[test]
fn the_template_lists_every_text_in_the_code() {
    let found = scan(&root(), &["src", "gui/src"]);
    let want = template_json(&found);
    let have = std::fs::read_to_string(root().join("locales/template.json")).unwrap();
    assert_eq!(have, want, "locales/template.json is out of date: run `cargo run --bin putia-i18n -- extract`");
}

#[test]
fn translations_have_no_stale_texts_and_agree_on_placeholders() {
    let found = scan(&root(), &["src", "gui/src"]);
    for (code, _, json) in i18n::LANGUAGES.iter().skip(1) {
        let loc: HashMap<String, String> = serde_json::from_str(json).unwrap();
        let r = check(&found, &loc);
        assert!(r.stale.is_empty(), "{code}: texts no longer in the code (run `putia-i18n sync {code}`): {:?}", r.stale);
        assert!(r.bad_placeholders.is_empty(), "{code}: placeholders differ: {:?}", r.bad_placeholders);
        assert!(r.bad_plurals.is_empty(), "{code}: a count text needs two forms, one|other: {:?}", r.bad_plurals);
    }
}

// the only test that changes the language (the setting is global to the process)
#[test]
fn languages_switch_and_fall_back_to_english() {
    i18n::set_language("en");
    assert_eq!(tr("Copies"), "Copies");
    assert_eq!(tr_n(1, "{n} shape selected.", "{n} shapes selected."), "1 shape selected.");
    assert_eq!(tr_n(3, "{n} shape selected.", "{n} shapes selected."), "3 shapes selected.");
    i18n::set_language("it");
    assert_eq!(tr("Copies"), "Copie");
    assert_eq!(tr_n(1, "{n} shape selected.", "{n} shapes selected."), "1 forma selezionata.");
    assert_eq!(tr_n(3, "{n} shape selected.", "{n} shapes selected."), "3 forme selezionate.");
    assert_eq!(tr_f("shape {n}", &[("n", &4)]), "forma 4");
    // a text with no translation is shown in English
    assert_eq!(tr("A text nobody translated"), "A text nobody translated");
    // the G-code file is the same in every language: its comments and the summary at its top stay English, while the
    // notes and the "What this will do" rows are in the language in use
    let data = |name: &str| putiacnc_core::load_project(&std::fs::read_to_string(root().join(format!("tests/data/{name}.project.json"))).unwrap()).unwrap().data;
    for name in ["imperial_center", "hardware_m0_aircut"] {
        let d = data(name);
        i18n::set_language("en");
        let en = putiacnc_core::pieces::generate_parts(&d, putiacnc_core::Units::Metric).unwrap();
        let rows_en = putiacnc_core::summary::describe(&d, &en[0].out, putiacnc_core::Units::Metric, false);
        i18n::set_language("it");
        let it = putiacnc_core::pieces::generate_parts(&d, putiacnc_core::Units::Metric).unwrap();
        let rows_it = putiacnc_core::summary::describe(&d, &it[0].out, putiacnc_core::Units::Metric, false);
        assert_eq!(en.len(), it.len());
        for (a, b) in en.iter().zip(&it) { assert_eq!(a.out.lines, b.out.lines, "{name}: the G-code depends on the language"); }
        assert!(rows_en.iter().zip(&rows_it).any(|(a, b)| a.text != b.text), "{name}: the summary rows are not translated");
    }
    i18n::set_language("pseudo");
    assert!(tr("Copies").starts_with("[! ") && tr("Copies").ends_with(" !]"));
    assert_eq!(tr_f("shape {n}", &[("n", &4)]).matches('4').count(), 1);
    i18n::set_language("xx");
    assert_eq!(tr("Copies"), "Copies");
    i18n::set_language("en");
}
