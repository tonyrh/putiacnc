# Translating PutiaCNC

PutiaCNC is written in English and can be shown in other languages. Italian is the first one. This page says how to add a language or improve one.

You do not need to know Rust to translate. You edit one file, `locales/<code>.json`, where `<code>` is the language code (`it` for Italian, `de` for German, `pt` for Portuguese). To try it in the app you need to build the app once (see the README, "Build and test").

## How it works

- Every text the program shows is written in the code in English, inside `tr("...")`. The English text is also the key.
- `locales/<code>.json` gives the translation of each English text. A text with an empty translation is shown in English, so a half-finished language still works.
- `locales/template.json` lists every text, with empty translations. It is written by a tool; do not edit it by hand.
- The language is chosen in View → Language, or on the first page of the setup assistant. "Automatic" follows the system.

## What is not translated

- The G-code, its comments and the summary at the top of the file stay English in every language. A test checks that the file is the same whatever the language.
- Names kept in your own files: tool names and notes, layer names such as "Layer 1", the names of fonts, file names.
- The system information for bug reports (Help → About → Copy system info).

## Adding a language

1. Make the file, with every text empty:
   ```sh
   cargo run -p putiacnc-core --bin putia-i18n -- sync de
   ```
   This writes `locales/de.json`.
2. Add the language to the list in `src/i18n.rs`, with its code and its name in that language:
   ```rust
   ("de", "Deutsch", include_str!("../locales/de.json")),
   ```
   If its words are much longer or shorter than English, give it a width in `width_factor` in the same file (Italian has 1.25): the side panels and the windows are made that much wider.
3. Translate. Either edit `locales/de.json` directly, or use a spreadsheet (next section).
4. Check it:
   ```sh
   cargo run -p putiacnc-core --bin putia-i18n -- check de
   cargo test -p putiacnc-core --test i18n
   ```
5. Look at it in the app:
   ```sh
   PUTIACNC_LANG=de cargo run -p putiacnc-gui
   ```
6. Open a pull request with `locales/de.json` and the line in `src/i18n.rs`.

## Translating in a spreadsheet

For a whole language, a table is easier to read than the JSON file.

```sh
cargo run -p putiacnc-core --bin putia-i18n -- review de de-review.csv
```

This writes a CSV file with the columns Where (the source file the text is in), English, English (for one), Translation, Correction and Note. Open it in any spreadsheet program, type in the Correction column only where you want a change, save it as CSV again, and put the corrections back:

```sh
cargo run -p putiacnc-core --bin putia-i18n -- apply de de-review.csv
cargo run -p putiacnc-core --bin putia-i18n -- check de
```

## Rules for a translation

- **Words in braces** such as `{n}`, `{name}` or `{unit}` are filled in by the program. Keep them exactly as they are. You may move them within the sentence. `check` reports a translation whose braces differ from the English.
- **Counts.** A text shown with a number, such as `{n} shapes selected.`, takes two forms in the translation, separated by `|`: first the one for 1, then the one for the rest.
  ```json
  "{n} shapes selected.": "{n} forma selezionata.|{n} forme selezionate."
  ```
  The English key is the form for several; the form for one is in the code. Languages with more than two forms need a rule in `plural_form` in `src/i18n.rs`.
- **Spaces and signs at the ends count.** A text that starts with a space or a comma (`, hidden`) is joined to another one: keep the space or the comma. Keep `⚠` at the start of a warning, and the line breaks (`\n`) of the long texts.
- **Keys in the keyboard shortcuts** are translated like any text (Shift → Maiusc in Italian).
- **One English text, one translation.** The same English text used in two places shares its translation. If the two places need different words in your language, say so in the pull request: the code can give them separate texts.
- **Technical words.** Use the words people in your country use at the machine. Where the English word is the one in use (dogbone, T-bone, G-code, offset), keep it.
- Keep the tone of the English: short, plain sentences, said to one person.

## When the English changes

After any change to the texts in the code, run:

```sh
cargo run -p putiacnc-core --bin putia-i18n -- extract
cargo run -p putiacnc-core --bin putia-i18n -- sync it
```

`extract` writes `locales/template.json` anew. `sync` keeps what is translated, adds the new texts empty and drops the ones no longer in the code. A changed English text is a new text: its old translation is dropped and it is shown in English until someone translates it. The test `tests/i18n.rs` fails when the template is out of date, when a translation has texts the code no longer has, or when braces or count forms are wrong.

## For those who write code

- A fixed text: `tr("Add clamp")`.
- A text with values: `tr_f("Clamp {n}", &[("n", &(i + 1))])`. Never build a sentence from pieces with `format!`: word order differs between languages.
- A text with a count: `tr_n(n, "{n} shape", "{n} shapes")`, or `tr_nf` when it has other values too.
- A text in a `const`: mark it with `n_("...")` and translate it where it is shown, with `tr(...)` (or `tr_dyn` when it is not `'static`).
- Text that goes into a G-code file is made inside `i18n::english(|| ...)`.
- Do not recognise a message by its English words (`starts_with("Op ")`): ask the function that makes it.
- Ids, file names, names saved in a project and anything compared in the code stay plain strings.
- `PUTIACNC_LANG=pseudo` shows the test language: every text accented, 40% longer and wrapped in `[! !]`. What is still plain English on screen was never put through `tr`; what is cut off needs more room. `PUTIACNC_PSEUDO=1` adds it to View → Language.
