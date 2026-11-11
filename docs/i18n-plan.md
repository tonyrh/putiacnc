# Translating PutiaCNC: plan

Branch: `i18n`. Decisions (12 Nov 2026):

1. The English text is the key: `tr("Slow down in tight corners")`.
2. G-code comments and the summary in the file stay in English.
3. Italian is the first language.
4. Right-to-left and CJK languages are out of scope for now.

## What there is to translate

- About 1,550 string literals in `gui/` (about 1,280 unique, about 10,000 words), 142 of them hover tips. Biggest sources: `panels/project.rs` (about 250), `toolmgr.rs`, `machmgr.rs`, `panels/menu.rs`, `text.rs`, `setup.rs`, `canvas/palette.rs`.
- About 300 strings in `src/` that reach the user: toolpath notes and warnings, checks, setup sheet text.
- The quick tour (`gui/src/tour.rs`). The README and `docs/features.md` stay in English.

## Design

- An `i18n` module in the core crate (so the toolpath messages can use it too).
- `tr("English text")`, `tr_f("{n} shapes selected", &[("n", n)])`, `tr_n(n, "{n} shape", "{n} shapes")`. A translation can reorder placeholders and has its own plural forms. This replaces the roughly 40 `if n == 1 { "" } else { "s" }` patterns.
- One text file per language (`locales/it.json`: English text to translation), embedded in the program at build time. Missing strings fall back to English.
- A `language` setting: Auto (system locale) or a fixed language. It applies at once.
- A script lists every `tr("...")` in the code and writes `locales/template.json`. Tests fail on a locale string the code no longer uses, a changed placeholder, or a missing plural form. Missing translations are only reported.
- A pseudo-language (accented letters, text about 30 % longer, wrapped in `[! ... !]`) shows what was never converted and which layouts overflow.
- Kept in English: G-code comments (`(Pass 2)`, `(Operation 1: ...)`, the summary header; the 3D replay reads them and Grbl wants ASCII), tool and bit names, font names, units. Notes shown in the app that are also written in the G-code summary get an English version for the file and a translated one for the screen.

## Steps (each a commit; the app works after every one)

0. Infrastructure: the module, the setting, the pseudo-language, the extraction script, tests. English behaves exactly as now. Convert one small window (Array and copy) as the proof.
1. Always-visible text: menus, toolbars, status bar, dialog headers, shared widgets.
2. Panels: operation cards, project, layers, clamps.
3. Tool windows: array, corners, nest, text, box generator, clean-up, fingers, trace, edit points, align, offset and the others.
4. Managers: tool library, machine manager, setup assistant, settings, about.
5. Core messages: toolpath notes and warnings, checks, setup sheet text, sample project names.
6. Tour and help: quick tour, shortcut list, canvas hint line and tooltips.
7. Italian: drafted, then reviewed by Antonio. Screenshots in Italian and in the pseudo-language to fix overflow.
8. Contributor guide: how to add a language (copy the template, translate, run the tests, open a pull request) and a README mention.

## Things that need care

- Layout: label columns are measured from the text, but fixed widths (the 376 px dialogs, compact number fields) can break with longer words.
- About 7 places compare against display-like names ("pocket", "outside"): they must use internal names, not the shown text. Fix these first.
- Names created with defaults ("Layer 1") are saved in project files in the language active at that moment.
- Fonts: system font plus the Hack fallback covers Latin, Greek and Cyrillic.
- When English wording changes, the old translation falls back to English; the report lists such strings.
