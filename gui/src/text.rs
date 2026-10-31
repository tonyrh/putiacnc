//! Text tool: click where the text starts, type it, pick a font and place it as cuttable outlines.
use crate::widgets::Tip;
use crate::app::{App, Mode};
use eframe::egui;
use putiacnc_core::model::LoopData;
use putiacnc_core::shapes;
use putiacnc_core::linetext::line_text_loops;
use putiacnc_core::text::{apply_affine, fit_affine, text_loops, Affine, Align, Flow, TextStyle};
use std::sync::{mpsc, Arc};

#[derive(Clone)]
pub struct FontEntry {
    pub label: String,
    /// None for the fonts built into the app (the label tells which one).
    id: Option<fontdb::ID>,
    /// A single-line (stroke) font built into the app: its id in `putiacnc_core::linetext`.
    line: Option<&'static str>,
}

/// The single-line fonts, first in the list.
fn line_entries() -> Vec<FontEntry> {
    putiacnc_core::linetext::fonts().into_iter().map(|(id, label)| FontEntry { label: label.into(), id: None, line: Some(id) }).collect()
}

pub struct TextTool {
    pub open: bool,
    pub focus: bool,
    /// Where the first line starts (baseline, left), mm.
    pub pos: Option<[f64; 2]>,
    pub content: String,
    pub size: f64,
    /// Space between letters and extra space at word gaps, as a percentage of the height; and whether the font's kerning pairs apply.
    pub letter_pct: f64,
    pub word_pct: f64,
    pub kerning: bool,
    pub align: Align,
    pub flow: Flow,
    /// Radius of the circle for the arc layouts, mm.
    pub radius: f64,
    pub arc_deg: f64,
    pub arc_in: bool,
    /// Fit the text to a width: the height follows from it.
    pub fit_on: bool,
    pub fit_w: f64,
    fit_key: String,
    /// Offset of the shadow copy, mm; 0 and 0 means no shadow.
    pub shadow: [f64; 2],
    /// Which sections of the dialog are open (kept in the settings).
    pub sections: std::collections::BTreeSet<String>,
    /// The turn of the text, degrees counter-clockwise about its start point.
    pub angle: f64,
    /// What remains of the transform of text being edited after the turn and the size (a stretch or a flip); the identity for new text.
    pub rest: [f64; 4],
    /// The text group being edited (its letters are replaced when the text is updated), and the one waiting for the fonts to be listed.
    pub edit: Option<i64>,
    pub edit_req: Option<i64>,
    pub fonts: Vec<FontEntry>,
    pub sel: usize,
    /// The font was picked by the user, so a finished font search must not change it.
    picked: bool,
    filter: String,
    /// How many fonts were found on the computer (None while still looking).
    pub found: Option<usize>,
    pub preview: Vec<LoopData>,
    pub err: Option<String>,
    db: Option<Arc<fontdb::Database>>,
    rx: Option<mpsc::Receiver<(Arc<fontdb::Database>, Vec<FontEntry>)>>,
    loading: bool,
    key: String,
    bytes_for: Option<usize>,
    /// Each font's name drawn in its own letters, made when its row first scrolls into view (None: the font cannot draw its name).
    previews: std::collections::HashMap<usize, Option<egui::TextureHandle>>,
    bytes: Option<(Vec<u8>, u32)>,
}

/// Width (points) a font name may take in the Font menu; longer names are drawn smaller to fit.
const NAME_W: f64 = 280.0;

fn builtin(name: &str) -> Option<Vec<u8>> {
    egui::FontDefinitions::default().font_data.get(name).map(|d| d.font.to_vec())
}

impl Default for TextTool {
    fn default() -> Self {
        let mut fonts = line_entries();
        let first_outline = fonts.len();
        fonts.extend([FontEntry { label: "Hack".into(), id: None, line: None }, FontEntry { label: "Ubuntu Light".into(), id: None, line: None }]);
        TextTool {
            open: false, focus: false, pos: None, content: String::new(), size: 10.0, letter_pct: 0.0, word_pct: 0.0, kerning: true, align: Align::Left, flow: Flow::Straight, radius: 60.0, arc_deg: 90.0, arc_in: false, fit_on: false, fit_w: 100.0, fit_key: String::new(), shadow: [0.0, 0.0], sections: Default::default(), angle: 0.0, rest: [1.0, 0.0, 0.0, 1.0], edit: None, edit_req: None,
            fonts, sel: first_outline, picked: false, filter: String::new(), found: None, preview: vec![], err: None, db: None, rx: None, loading: false, key: String::new(), bytes_for: None, bytes: None, previews: Default::default(),
        }
    }
}

impl TextTool {
    /// Starts listing the fonts installed on the computer (on a thread; it takes a moment).
    fn load(&mut self, ctx: &egui::Context) {
        if self.loading { return; }
        self.loading = true;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            let mut v: Vec<FontEntry> = db.faces().filter_map(|f| {
                let fam = f.families.first()?.0.clone();
                let mut label = fam;
                match f.weight.0 {
                    0..=299 => label += " Light",
                    500..=599 => label += " Medium",
                    600..=699 => label += " SemiBold",
                    700..=799 => label += " Bold",
                    800..=u16::MAX => label += " Black",
                    _ => {}
                }
                if f.stretch != fontdb::Stretch::Normal { label += " Condensed"; }
                if f.style != fontdb::Style::Normal { label += " Italic"; }
                Some(FontEntry { label, id: Some(f.id), line: None })
            }).collect();
            v.sort_by_key(|a| a.label.to_lowercase());
            v.dedup_by(|a, b| a.label == b.label);
            let _ = tx.send((Arc::new(db), v));
            ctx.request_repaint();
        });
    }

    fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        if let Ok((db, list)) = rx.try_recv() {
            let current = self.fonts.get(self.sel).map(|f| f.label.clone());
            let first_time = !self.picked;
            self.found = Some(list.len());
            let mut all = list;
            for b in ["Ubuntu Light", "Hack"] { all.push(FontEntry { label: b.into(), id: None, line: None }); }
            all.sort_by_key(|a| a.label.to_lowercase());
            let mut fonts = line_entries();
            let first_outline = fonts.len();
            fonts.extend(all);
            self.fonts = fonts;
            self.db = Some(db);
            self.rx = None;
            // keep the chosen font; a fresh tool starts on a plain, sturdy sans if there is one
            let want = if first_time { ["DejaVu Sans", "Liberation Sans", "Arial", "Noto Sans", "Cantarell"].iter().find(|w| self.fonts.iter().any(|f| f.label == **w)).map(|s| s.to_string()).or(current) } else { current };
            self.sel = want.and_then(|w| self.fonts.iter().position(|f| f.label == w)).unwrap_or(first_outline);
            self.key.clear();
            self.bytes_for = None;
            self.previews.clear();
        }
    }

    /// The font's name in its own letters as a white-on-clear texture, made once per font.
    fn name_preview(&mut self, ctx: &egui::Context, i: usize) -> Option<egui::TextureHandle> {
        if let Some(p) = self.previews.get(&i) { return p.clone(); }
        let e = self.fonts.get(i)?.clone();
        // the stroke fonts have no outlines to draw their names with
        if e.line.is_some() { return None; }
        let ppp = ctx.pixels_per_point() as f64;
        let draw = |data: &[u8], idx: u32| putiacnc_core::text::name_bitmap(data, idx, &e.label, 17.0 * ppp, 11.0 * ppp, (NAME_W * ppp) as usize, (30.0 * ppp) as usize);
        let bmp = match e.id {
            Some(id) => self.db.as_ref().and_then(|d| d.with_face_data(id, |data, idx| draw(data, idx)).flatten()),
            None => builtin(if e.label == "Hack" { "Hack" } else { "Ubuntu-Light" }).and_then(|b| draw(&b, 0)),
        };
        let tex = bmp.map(|(w, h, a)| {
            // egui blends in gamma space, which makes thin strokes look heavy; thin the coverage a little to compensate
            let rgba: Vec<u8> = a.iter().flat_map(|&v| [255, 255, 255, (255.0 * (v as f32 / 255.0).powf(1.5)) as u8]).collect();
            ctx.load_texture(format!("font_name_{i}"), egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba), egui::TextureOptions::LINEAR)
        });
        // fonts still loading must not be remembered as failures
        if self.db.is_some() { self.previews.insert(i, tex.clone()); }
        tex
    }

    /// The letters of `text` in the chosen font: None when the font cannot be loaded.
    fn loops(&mut self, text: &str, style: TextStyle) -> Option<Result<Vec<LoopData>, String>> {
        if let Some(id) = self.fonts.get(self.sel).and_then(|f| f.line) { return Some(line_text_loops(id, text, style, [0.0, 0.0], 0)); }
        let (data, idx) = self.font_bytes()?;
        Some(text_loops(&data, idx, text, style, [0.0, 0.0], 0))
    }

    /// Chooses the font with this label, if the list has it.
    pub fn select_font(&mut self, label: &str) -> bool {
        let Some(i) = self.fonts.iter().position(|f| f.label == label) else { return false };
        self.sel = i;
        self.picked = true;
        true
    }

    /// Whether the chosen font draws single strokes (for a pen) rather than outlines.
    pub fn is_line_font(&self) -> bool { self.fonts.get(self.sel).is_some_and(|f| f.line.is_some()) }

    fn font_bytes(&mut self) -> Option<(Vec<u8>, u32)> {
        if self.bytes_for != Some(self.sel) {
            let e = self.fonts.get(self.sel)?.clone();
            self.bytes = match e.id {
                Some(id) => self.db.as_ref().and_then(|d| d.with_face_data(id, |data, idx| (data.to_vec(), idx))),
                None => builtin(if e.label == "Hack" { "Hack" } else { "Ubuntu-Light" }).map(|b| (b, 0)),
            };
            self.bytes_for = Some(self.sel);
        }
        self.bytes.clone()
    }

    fn style(&self) -> TextStyle { TextStyle { size_mm: self.size, spacing_mm: self.size * self.letter_pct / 100.0, word_spacing_mm: self.size * self.word_pct / 100.0, kerning: self.kerning, align: self.align, flow: self.flow, radius_mm: self.radius, arc_deg: self.arc_deg, arc_in: self.arc_in, shadow: self.shadow } }

    /// With fit to width on, sets the height so the widest line is as wide as asked (width grows in step with the height).
    fn fit(&mut self) {
        if !self.fit_on || self.flow != Flow::Straight || self.content.trim().is_empty() { return; }
        let key = format!("{}|{}|{}|{}|{}|{}|{:?}", self.content, self.sel, self.letter_pct, self.word_pct, self.kerning, self.fit_w, self.align);
        if key == self.fit_key { return; }
        self.fit_key = key;
        let probe = TextStyle { size_mm: 10.0, spacing_mm: 10.0 * self.letter_pct / 100.0, word_spacing_mm: 10.0 * self.word_pct / 100.0, flow: Flow::Straight, align: Align::Left, shadow: [0.0, 0.0], ..self.style() };
        let content = self.content.clone();
        if let Some(b) = self.loops(&content, probe).and_then(|r| r.ok()).and_then(|l| shapes::bbox_many(l.iter())) {
            let w = b[2] - b[0];
            if w > 1e-6 && self.fit_w > 0.0 { self.size = (self.fit_w * 10.0 / w).clamp(0.5, 500.0); }
        }
    }

    fn refresh(&mut self) {
        self.fit();
        let key = format!("{}|{}|{}|{}|{}|{}|{:?}|{:?}|{}|{:?}|{:?}|{}|{:?}|{}|{}|{}", self.content, self.sel, self.size, self.letter_pct, self.word_pct, self.kerning, self.align, self.pos, self.angle, self.rest, self.flow, self.radius, self.shadow, self.arc_deg, self.arc_in, self.fit_on);
        if key == self.key { return; }
        self.key = key;
        self.preview.clear();
        self.err = None;
        let Some(pos) = self.pos else { return };
        if self.content.trim().is_empty() { return; }
        let (content, style) = (self.content.clone(), self.style());
        match self.loops(&content, style) {
            None => self.err = Some("This font could not be loaded.".into()),
            Some(r) => match r {
                Ok(mut l) => {
                    if l.is_empty() { self.err = Some("This font has none of those letters.".into()); }
                    // set at the origin, then turned and moved: the same steps the editor takes back later
                    apply_affine(&mut l, &Affine::compose(self.angle.to_radians(), self.rest, pos));
                    self.preview = l;
                    // text longer than the circle would run into its own start
                    if self.flow == Flow::Arc && self.err.is_none() {
                        let flat = TextStyle { flow: Flow::Straight, align: Align::Left, shadow: [0.0, 0.0], ..self.style() };
                        if let Some(b) = self.loops(&content, flat).and_then(|r| r.ok()).and_then(|f| shapes::bbox_many(f.iter())) {
                            let circle = std::f64::consts::TAU * self.radius.max(self.size);
                            if b[2] - b[0] > circle { self.err = Some(format!("The text is {:.0} mm long but the circle is only {:.0} mm round: the letters will overlap. Use a bigger radius or less text.", b[2] - b[0], circle)); }
                        }
                    }
                }
                Err(e) => self.err = Some(e),
            },
        }
    }
}

/// Adds the previewed text to the drawing, in no operation, and selects it. The shapes form a group that
/// remembers the settings, so the text can be edited later; when a text is being edited its letters are replaced.
pub fn place(app: &mut App) {
    app.text.key.clear();
    app.text.refresh();
    let mut loops = std::mem::take(&mut app.text.preview);
    if loops.is_empty() { return; }
    let t = &app.text;
    let spec = putiacnc_core::model::TextSpec {
        content: t.content.clone(), font: t.fonts.get(t.sel).map(|f| f.label.clone()).unwrap_or_default(), size: t.size, spacing: t.size * t.letter_pct / 100.0, word_spacing: t.size * t.word_pct / 100.0, kerning: t.kerning, align: t.align, flow: t.flow, radius: t.radius, arc_deg: t.arc_deg, arc_in: t.arc_in, fit_width: if t.fit_on && t.flow == Flow::Straight { t.fit_w } else { 0.0 }, shadow: t.shadow,
    };
    let first = t.content.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().chars().take(24).collect::<String>();
    let name = format!("Text: {first}");
    app.hist.record(&app.project.data);
    if let Some(gid) = app.text.edit {
        let d = &mut app.project.data;
        let old: Vec<usize> = d.loops.iter().enumerate().filter(|(_, l)| l.grp == Some(gid)).map(|(i, _)| i).collect();
        let first_op = old.first().map(|&i| d.loops[i].op).unwrap_or(0);
        let same = old.len() == loops.len();
        for (k, l) in loops.iter_mut().enumerate() {
            l.grp = Some(gid);
            l.op = if same { d.loops[old[k]].op } else { first_op };
        }
        if same {
            // the same number of letters: they take each other's place, so tabs and order stay as they are
            for (k, l) in loops.into_iter().enumerate() { d.loops[old[k]] = l; }
            app.sel = old.into_iter().collect();
        } else {
            shapes::tabs_after_delete(&mut d.ops, &old);
            for &i in old.iter().rev() { d.loops.remove(i); }
            let start = d.loops.len();
            d.loops.extend(loops);
            app.sel = (start..d.loops.len()).collect();
        }
        if let Some(g) = d.groups.iter_mut().find(|g| g.id == gid) { g.text = Some(spec); g.name = name; }
    } else {
        let op = 0;
        let gid = app.project.data.groups.iter().map(|g| g.id).max().unwrap_or(0) + 1;
        app.project.data.groups.push(putiacnc_core::model::ShapeGroup { id: gid, name, locked: false, text: Some(spec) });
        for l in &mut loops { l.op = op; l.grp = Some(gid); }
        let start = app.project.data.loops.len();
        app.project.data.loops.extend(loops);
        app.sel = (start..app.project.data.loops.len()).collect();
    }
    app.touch();
    app.text.open = false;
    app.text.content.clear();
    app.text.key.clear();
    app.text.edit = None;
    app.text.angle = 0.0;
    app.text.rest = [1.0, 0.0, 0.0, 1.0];
    app.mode = Mode::Select;
}

impl App {
    /// Opens the Text window on a placed text, to change its words, font, size or place. The window fills in
    /// once the fonts are listed.
    pub fn edit_text(&mut self, gid: i64) {
        if self.project.data.groups.iter().all(|g| g.id != gid || g.text.is_none()) { return; }
        self.mode = Mode::Text;
        self.text.open = true;
        self.text.edit_req = Some(gid);
        self.text.edit = None;
        self.text.preview.clear();
        self.text.key.clear();
    }

    /// Whether shape `i` is drawn faded because a preview stands in for it: a tool's ghost replaces it, or it is a letter of the text being edited
    /// while the preview of the new settings shows.
    pub fn dimmed_by_preview(&self, i: usize) -> bool {
        if self.ghost.hide.contains(&i) { return true; }
        match (self.text.open, self.text.edit, self.project.data.loops.get(i)) {
            (true, Some(gid), Some(l)) => l.grp == Some(gid) && !self.text.preview.is_empty(),
            _ => false,
        }
    }

    /// Drops the link to the settings: the letters stay as they are, as plain outlines.
    pub fn text_to_outlines(&mut self, gid: i64) {
        self.hist.record(&self.project.data);
        for l in self.project.data.loops.iter_mut().filter(|l| l.grp == Some(gid)) { l.grp = None; }
        self.project.data.groups.retain(|g| g.id != gid);
        self.touch();
    }

    /// Everything that is selected together with loop `i`: all the letters of a placed text, and all the shapes of a group
    /// made with Edit > Group (and so on, when a text is part of a group or a group holds a text). Just `i` for any other shape.
    pub fn text_siblings(&self, i: usize) -> Vec<usize> { self.unit_of(&[i]) }

    /// The shapes of `seed` plus everything that belongs with them (see `text_siblings`).
    pub fn unit_of(&self, seed: &[usize]) -> Vec<usize> { putiacnc_core::selection::unit_of(&self.project.data, seed) }

    /// Edit > Group: the selected shapes (a placed text counts as one) become one group.
    pub fn group_selection(&mut self) {
        if !putiacnc_core::selection::can_group(&self.project.data, &self.sel) { return; }
        self.hist.record(&self.project.data);
        if let Some(sel) = putiacnc_core::selection::group(&mut self.project.data, &self.sel) { self.sel = sel; }
        self.touch();
    }

    /// Edit > Ungroup: the groups in the selection fall apart; the shapes stay selected.
    pub fn ungroup_selection(&mut self) {
        if !self.sel_in_group() { return; }
        self.hist.record(&self.project.data);
        putiacnc_core::selection::ungroup(&mut self.project.data, &self.sel);
        self.touch();
    }

    /// The selection is exactly one group already, so grouping it again would change nothing.
    pub fn group_is_whole_selection(&self) -> bool { putiacnc_core::selection::is_whole_group(&self.project.data, &self.sel) }

    pub fn sel_in_group(&self) -> bool { putiacnc_core::selection::in_group(&self.project.data, &self.sel) }

    pub fn expand_text_selection(&mut self) {
        let all: Vec<usize> = self.sel.iter().flat_map(|&i| self.text_siblings(i)).collect();
        self.sel.extend(all);
    }

    /// The group of the letters when the selection is one placed text (or part of it).
    pub fn selected_text(&self) -> Option<i64> {
        let d = &self.project.data;
        let mut gid = None;
        for &i in &self.sel {
            let g = d.loops.get(i)?.grp?;
            if *gid.get_or_insert(g) != g { return None; }
        }
        let g = gid?;
        d.groups.iter().find(|x| x.id == g).and_then(|x| x.text.as_ref()).map(|_| g)
    }
}

/// Fills the window from a placed text: its settings, with the move, turn and size it has had since.
/// `false` while the fonts are still being listed.
fn begin_edit(app: &mut App, gid: i64) -> bool {
    let Some(spec) = app.project.data.groups.iter().find(|g| g.id == gid).and_then(|g| g.text.clone()) else { return true };
    let Some(idx) = app.text.fonts.iter().position(|f| f.label == spec.font) else {
        if app.text.found.is_none() { return false; }
        let msg = format!("The font \"{}\" is not on this computer, so this text cannot be edited here. You can still convert it to outlines.", spec.font);
        app.notes.push((crate::app::NoteUntil::NotText(gid), msg.clone()));
        app.say(crate::app::Level::Warning, msg);
        app.text.open = false;
        app.mode = Mode::Select;
        return true;
    };
    app.text.sel = idx;
    app.text.picked = true;
    app.text.bytes_for = None;
    let style = TextStyle { size_mm: spec.size, spacing_mm: spec.spacing, word_spacing_mm: spec.word_spacing, kerning: spec.kerning, align: spec.align, flow: spec.flow, radius_mm: spec.radius, arc_deg: spec.arc_deg, arc_in: spec.arc_in, shadow: spec.shadow };
    let g0 = app.text.loops(&spec.content, style).and_then(|r| r.ok()).unwrap_or_default();
    let now: Vec<&LoopData> = app.project.data.loops.iter().filter(|l| l.grp == Some(gid)).collect();
    let fit = fit_affine(&g0, &now).filter(|(_, res)| *res < 0.02);
    let Some((af, _)) = fit else {
        // the letters were edited point by point (or the font draws them differently now): they stay as outlines
        app.text_to_outlines(gid);
        app.say(crate::app::Level::Info, "These letters were edited as shapes, so they can no longer be edited as text. They are plain outlines now.".into());
        app.text.open = false;
        app.mode = Mode::Select;
        return true;
    };
    let (s, turn, rest) = af.decompose();
    let t = &mut app.text;
    t.content = spec.content;
    t.size = spec.size * s;
    t.letter_pct = if spec.size > 0.0 { spec.spacing / spec.size * 100.0 } else { 0.0 };
    t.word_pct = if spec.size > 0.0 { spec.word_spacing / spec.size * 100.0 } else { 0.0 };
    t.kerning = spec.kerning;
    t.align = spec.align;
    // the two old circle choices are just arcs at the top and at the bottom
    (t.flow, t.arc_deg, t.arc_in) = match spec.flow { Flow::ArcTop => (Flow::Arc, 90.0, false), Flow::ArcBottom => (Flow::Arc, 270.0, true), f => (f, spec.arc_deg, spec.arc_in) };
    t.radius = spec.radius * s;
    t.fit_on = spec.fit_width > 0.0 && spec.flow == Flow::Straight;
    if t.fit_on { t.fit_w = spec.fit_width * s; }
    t.fit_key.clear();
    t.shadow = [spec.shadow[0] * s, spec.shadow[1] * s];
    t.angle = turn.to_degrees();
    t.rest = rest;
    t.pos = Some(af.t);
    t.edit = Some(gid);
    t.key.clear();
    true
}

/// A folding section: a header with a chevron, the title and, in grey at the right, what it is set to. `open` holds the names of the
/// sections left open (kept between sessions in the settings).
fn section(ui: &mut egui::Ui, open: &mut std::collections::BTreeSet<String>, key: &str, title: &str, summary: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(2.0);
    ui.separator();
    let is_open = open.contains(key);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.hovered() { ui.painter().rect_filled(rect.shrink2(egui::vec2(0.0, 2.0)), 5.0, ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.5)); }
    let ink = ui.visuals().text_color();
    crate::icons::paint_icon(ui.painter(), if is_open { crate::icons::Icon::ChevD } else { crate::icons::Icon::ChevR }, egui::pos2(rect.min.x + 12.0, rect.center().y), ink, 0.55);
    let g = ui.painter().layout_no_wrap(title.to_string(), egui::TextStyle::Body.resolve(ui.style()), ink);
    let tw = g.size().x;
    ui.painter().galley(egui::pos2(rect.min.x + 26.0, rect.center().y - g.size().y / 2.0), g, ink);
    // the summary is shown only if there is room beside the title
    let room = rect.width() - 26.0 - tw - 16.0;
    let gs = ui.painter().layout_no_wrap(summary.to_string(), egui::TextStyle::Small.resolve(ui.style()), ui.visuals().weak_text_color());
    if gs.size().x <= room { ui.painter().galley(egui::pos2(rect.max.x - 8.0 - gs.size().x, rect.center().y - gs.size().y / 2.0), gs, ui.visuals().weak_text_color()); }
    if resp.clicked() { if is_open { open.remove(key); } else { open.insert(key.to_string()); } }
    if is_open { ui.add_space(4.0); body(ui); ui.add_space(4.0); }
}

/// What the "?" in the header says: what the tool does, and which fonts it found.
fn help_text(ui: &mut egui::Ui, editing: bool, found: Option<usize>) {
    if editing {
        ui.label("The letters are set again from these settings, where the text is now.");
    } else {
        ui.label("Letters become outlines (holes in A, O, B… are cut out). The text stays editable: double-click it later.");
        ui.add_space(4.0);
        ui.label("Click the drawing, or open Position and type X and Y, to move it.");
    }
    ui.add_space(4.0);
    ui.weak(match found {
        None => "Looking for the fonts installed on this computer…".to_string(),
        Some(n) => format!("{n} fonts found on this computer, plus Hack and Ubuntu Light."),
    });
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if app.mode != Mode::Text { app.text.open = false; }
    if !app.text.open { return; }
    app.text.poll();
    if app.text.db.is_none() { app.text.load(ctx); }
    if let Some(gid) = app.text.edit_req {
        if begin_edit(app, gid) {
            app.text.edit_req = None;
            // PUTIACNC_EDIT_SIZE=mm: the height is changed as editing starts, so the preview differs from the text (for screenshots)
            if let Some(h) = std::env::var("PUTIACNC_EDIT_SIZE").ok().and_then(|v| v.parse::<f64>().ok()) { app.text.size = h; }
        } else { ctx.request_repaint_after(std::time::Duration::from_millis(100)); return; }
        if !app.text.open { return; }
    }
    // text starts at the XY zero until a position is clicked or typed
    if app.text.pos.is_none() {
        let st = &app.project.data.stock;
        app.text.pos = Some([match st.zr.as_str() { "lr" | "ur" => st.w, "c" => st.w / 2.0, _ => 0.0 }, match st.zr.as_str() { "ul" | "ur" => st.h, "c" => st.h / 2.0, _ => 0.0 }]);
    }
    app.text.refresh();
    let stock = app.project.data.stock.clone();
    let (mut do_place, mut cancel) = (false, false);
    let editing = app.text.edit.is_some();
    egui::Window::new("Text").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .current_pos(crate::csd::spot(ctx, "Text", crate::csd::anchor(app))).show(ctx, |ui| {
        ui.set_width(376.0);
        let found = app.text.found;
        if crate::csd::dialog_header_help(ui, if editing { "Edit text" } else { "Text" }, true, |ui| help_text(ui, editing, found)) { cancel = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
        let t = &mut app.text;
        let te = ui.add(egui::TextEdit::multiline(&mut t.content).desired_rows(3).desired_width(f32::INFINITY).hint_text("Type your text"));
        if t.focus { te.request_focus(); t.focus = false; }
        ui.add_space(6.0);
        let st = &stock;
        let (zx, zy) = (match st.zr.as_str() { "lr" | "ur" => st.w, "c" => st.w / 2.0, _ => 0.0 }, match st.zr.as_str() { "ul" | "ur" => st.h, "c" => st.h / 2.0, _ => 0.0 });
        // one label column width for the whole dialog, so every field lines up whichever section it is in
        let lw = ["Font", "Height", "Align", "Letters", "Words", "Layout", "Width", "Radius", "Position", "Start", "Angle", "Shadow"].iter().map(|s| crate::panels::tw(ui, s)).fold(0.0_f32, f32::max);
        // the things set every time stay in view: font, height, alignment
        egui::Grid::new("text_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            crate::toolmgr::rlabel(ui, lw, "Font");
            let cur = t.fonts.get(t.sel).map(|f| f.label.clone()).unwrap_or_default();
            // a long name is shortened, so the box keeps its width
            let cur = crate::widgets::combo_elide(ui, &cur, 190.0);
            let mut keep = false;
            // PUTIACNC_FONT_LIST[=text]: the font list stays open, filtered by the text (for screenshots)
            if let Ok(q) = std::env::var("PUTIACNC_FONT_LIST") { if t.filter.is_empty() && q != "1" { t.filter = q; } crate::widgets::reopen_combo(ui, "text_font"); }
            egui::ComboBox::from_id_source("text_font").width(190.0).height(400.0).selected_text(cur).show_ui(ui, |ui| {
                let mut picked = false;
                ui.set_min_width(NAME_W as f32 + 40.0);
                ui.add(egui::TextEdit::singleline(&mut t.filter).hint_text("Search fonts").desired_width(f32::INFINITY));
                ui.add_space(2.0);
                let q = t.filter.to_lowercase();
                egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                    let mut any = false;
                    for i in 0..t.fonts.len() {
                        if !q.is_empty() && !t.fonts[i].label.to_lowercase().contains(&q) { continue; }
                        any = true;
                        let label = t.fonts[i].label.clone();
                        // a row as tall as the preview: the name in the font's own letters, or in the plain one when it cannot draw it
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::click());
                        if ui.is_rect_visible(rect) {
                            let v = ui.style().interact_selectable(&resp, t.sel == i);
                            if t.sel == i || resp.hovered() { ui.painter().rect(rect, v.rounding, if t.sel == i { ui.visuals().selection.bg_fill } else { v.weak_bg_fill }, egui::Stroke::NONE); }
                            let col = if t.sel == i { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
                            // whatever is drawn stays inside the row, beside the scroll bar
                            let row = rect.shrink2(egui::vec2(0.0, 0.0)).with_max_x(rect.max.x - 14.0);
                            let painter = ui.painter().with_clip_rect(row.intersect(ui.clip_rect()));
                            match t.name_preview(ui.ctx(), i) {
                                Some(tex) => {
                                    let ppp = ui.ctx().pixels_per_point();
                                    let sz = tex.size_vec2() / ppp;
                                    let r = egui::Rect::from_min_size(egui::pos2(rect.min.x + 6.0, rect.center().y - sz.y / 2.0), sz);
                                    painter.image(tex.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), col);
                                }
                                None => { painter.text(egui::pos2(rect.min.x + 6.0, rect.center().y), egui::Align2::LEFT_CENTER, crate::widgets::elide_text(ui, &label, row.width() - 12.0), egui::TextStyle::Button.resolve(ui.style()), col); }
                            }
                        }
                        if resp.clicked() { t.sel = i; t.picked = true; picked = true; }
                        resp.tip(&label);
                    }
                    if !any { ui.weak("No font matches."); }
                });
                if picked { ui.close_menu(); }
                // a click on the search box or beside the list leaves the menu open
                else if crate::widgets::click_inside_popup(ui) { keep = true; }
            });
            if keep { crate::widgets::reopen_combo(ui, "text_font"); }
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, "Height");
            ui.horizontal(|ui| {
                let fitted = t.fit_on && t.flow == Flow::Straight;
                ui.add_enabled_ui(!fitted, |ui| { crate::widgets::num(ui, [60.0, 24.0], &mut t.size, 0.5, 500.0, 0.1, 2, ""); });
                ui.weak(if fitted { "mm, from the width" } else { "mm, capital letters" });
            });
            ui.end_row();
            crate::toolmgr::rlabel(ui, lw, "Align");
            {
                const ALIGNS: [Align; 3] = [Align::Left, Align::Center, Align::Right];
                let cur = ALIGNS.iter().position(|a| *a == t.align).unwrap_or(0);
                let (c, _) = crate::zoomcard::seg3(ui, ["Left", "Centre", "Right"], ["Lines start at the left edge", "Lines are centred", "Lines end at the right edge"], cur);
                if let Some(i) = c { t.align = ALIGNS[i]; }
            }
            ui.end_row();
        });

        // the rest folds away; each header says in grey what it is set to, and stays open or shut as the user left it
        let grid = |id: &'static str| egui::Grid::new(id).num_columns(2).spacing([10.0, 8.0]);
        let line_font = t.is_line_font();
        let sum_spacing = if line_font { format!("letters {} · words {}", crate::panels::fmt_num(t.letter_pct), crate::panels::fmt_num(t.word_pct)) }
        else { format!("kerning {} · letters {} · words {}", if t.kerning { "on" } else { "off" }, crate::panels::fmt_num(t.letter_pct), crate::panels::fmt_num(t.word_pct)) };
        section(ui, &mut t.sections, "spacing", "Spacing", &sum_spacing, |ui| {
            ui.add_enabled_ui(!line_font, |ui| crate::widgets::check(ui, &mut t.kerning, "Kerning")).inner.tip("Move pairs of letters like AV, To or Ty together or apart the way the font says (not every font has the information)");
            ui.add_space(4.0);
            grid("text_spacing").show(ui, |ui| {
                crate::toolmgr::rlabel(ui, lw, "Letters");
                ui.horizontal(|ui| { crate::widgets::num(ui, [60.0, 24.0], &mut t.letter_pct, -30.0, 300.0, 0.5, 1, ""); ui.weak("% of the height"); });
                ui.end_row();
                crate::toolmgr::rlabel(ui, lw, "Words");
                ui.horizontal(|ui| { crate::widgets::num(ui, [60.0, 24.0], &mut t.word_pct, -100.0, 500.0, 1.0, 1, ""); ui.weak("% of the height, at the gaps"); });
                ui.end_row();
            });
        });
        let sum_layout = match t.flow {
            Flow::Straight if t.fit_on => format!("straight · {} mm wide", crate::panels::fmt_num(t.fit_w)),
            Flow::Straight => "straight".to_string(),
            Flow::Vertical => "stacked".to_string(),
            _ => format!("arc, r {} · {}°", crate::panels::fmt_num(t.radius), crate::panels::fmt_num(t.arc_deg)),
        };
        section(ui, &mut t.sections, "layout", "Layout", &sum_layout, |ui| {
            grid("text_layout").show(ui, |ui| {
                crate::toolmgr::rlabel(ui, lw, "Layout");
                let flow_resp = egui::ComboBox::from_id_source("text_flow").width(crate::widgets::combo_fit(ui, &["Straight", "Along an arc", "Stacked, top to bottom"]).max(190.0)).selected_text(match t.flow {
                    Flow::Straight => "Straight", Flow::Vertical => "Stacked, top to bottom", _ => "Along an arc",
                }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.flow, Flow::Straight, "Straight");
                    ui.selectable_value(&mut t.flow, Flow::Arc, "Along an arc");
                    ui.selectable_value(&mut t.flow, Flow::Vertical, "Stacked, top to bottom");
                }).response;
                if !ui.memory(|m| m.any_popup_open()) { flow_resp.tip("Along an arc: Start is where the text touches the circle, and Align says which part of the text sits there"); }
                ui.end_row();
                if t.flow == Flow::Straight {
                    crate::toolmgr::rlabel(ui, lw, "Width");
                    ui.horizontal(|ui| {
                        crate::widgets::check(ui, &mut t.fit_on, "Fit to");
                        ui.add_enabled_ui(t.fit_on, |ui| { crate::widgets::num(ui, [60.0, 24.0], &mut t.fit_w, 1.0, 5000.0, 1.0, 1, ""); });
                        ui.weak("mm");
                    }).response.tip("Sets the height so the longest line is exactly this wide");
                    ui.end_row();
                }
                if t.flow == Flow::Arc {
                    crate::toolmgr::rlabel(ui, lw, "Radius");
                    ui.horizontal(|ui| { crate::widgets::num(ui, [60.0, 24.0], &mut t.radius, 5.0, 5000.0, 0.5, 2, ""); ui.weak("mm, of the baseline"); });
                    ui.end_row();
                    crate::toolmgr::rlabel(ui, lw, "Position");
                    ui.horizontal(|ui| {
                        crate::widgets::num(ui, [60.0, 24.0], &mut t.arc_deg, -360.0, 720.0, 1.0, 1, "");
                        ui.weak("° (90 top, 270 bottom)");
                    }).response.tip("Where the text sits round the circle: 0 is the right, 90 the top, 180 the left, 270 the bottom");
                    ui.end_row();
                    crate::toolmgr::rlabel(ui, lw, "Letters");
                    ui.horizontal(|ui| {
                        let (c, _) = crate::zoomcard::seg2(ui, ["Face outwards", "Face the centre"], ["", ""], t.arc_in as usize);
                        if let Some(i) = c { t.arc_in = i == 1; }
                    });
                    ui.end_row();
                }
            });
        });
        let p0 = t.pos.unwrap_or([zx, zy]);
        let sum_pos = format!("X {} · Y {} · {}°", crate::panels::fmt_num(p0[0] - zx), crate::panels::fmt_num(p0[1] - zy), crate::panels::fmt_num(t.angle));
        section(ui, &mut t.sections, "position", "Position", &sum_pos, |ui| {
            grid("text_position").show(ui, |ui| {
                crate::toolmgr::rlabel(ui, lw, "Start");
                ui.horizontal(|ui| {
                    // where the first line begins (left end of its baseline), from the XY zero like the rulers
                    let p = t.pos.unwrap_or([zx, zy]);
                    let (mut x, mut y) = (p[0] - zx, p[1] - zy);
                    ui.label("X");
                    let cx = crate::widgets::num(ui, [64.0, 24.0], &mut x, -100000.0, 100000.0, 0.5, 2, "").changed();
                    ui.add_space(4.0);
                    ui.label("Y");
                    let cy = crate::widgets::num(ui, [64.0, 24.0], &mut y, -100000.0, 100000.0, 0.5, 2, "").changed();
                    ui.weak("mm");
                    if cx || cy { t.pos = Some([x + zx, y + zy]); }
                });
                ui.end_row();
                crate::toolmgr::rlabel(ui, lw, "Angle");
                ui.horizontal(|ui| { crate::widgets::num(ui, [60.0, 24.0], &mut t.angle, -360.0, 360.0, 1.0, 1, ""); ui.weak("degrees, counter-clockwise"); });
                ui.end_row();
            });
        });
        let sum_shadow = if t.shadow == [0.0, 0.0] { "none".to_string() } else { format!("X {} · Y {} mm", crate::panels::fmt_num(t.shadow[0]), crate::panels::fmt_num(t.shadow[1])) };
        section(ui, &mut t.sections, "shadow", "Shadow", &sum_shadow, |ui| {
            grid("text_shadow").show(ui, |ui| {
                crate::toolmgr::rlabel(ui, lw, "Shadow");
                ui.horizontal(|ui| {
                    ui.label("X");
                    crate::widgets::num(ui, [50.0, 24.0], &mut t.shadow[0], -100.0, 100.0, 0.1, 2, "");
                    ui.add_space(4.0);
                    ui.label("Y");
                    crate::widgets::num(ui, [50.0, 24.0], &mut t.shadow[1], -100.0, 100.0, 0.1, 2, "");
                    ui.weak("mm; 0 and 0 = none");
                }).response.tip("A second copy of the letters, moved by this much. Give it an operation of its own (for example a shallower V-carve) to cut a drop shadow");
                ui.end_row();
            });
        });
        if let Some(e) = &t.err { ui.add_space(4.0); ui.colored_label(egui::Color32::from_rgb(0xe0, 0x8a, 0x2a), e); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(!t.preview.is_empty(), egui::Button::new(if editing { "Update" } else { "Place" })).clicked() { do_place = true; }
            if ui.button("Cancel").clicked() { cancel = true; }
        });
        });
    });
    if do_place { place(app); }
    if cancel { app.text.open = false; app.text.edit = None; app.text.angle = 0.0; app.text.rest = [1.0, 0.0, 0.0, 1.0]; app.mode = Mode::Select; }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(content: &str) -> App {
        let mut a = App::new(None);
        a.text.content = content.into();
        a.text.sel = a.text.fonts.iter().position(|f| f.label == "Hack").unwrap();
        a.text.pos = Some([10.0, 20.0]);
        a.text.size = 12.0;
        place(&mut a);
        a
    }

    fn bbox(a: &App, gid: i64) -> [f64; 4] { shapes::bbox_many(a.project.data.loops.iter().filter(|l| l.grp == Some(gid))).unwrap() }

    #[test]
    fn the_text_being_edited_is_faded_while_its_preview_shows() {
        let mut a = placed("Hi");
        a.project.data.loops.push(shapes::new_circle([100.0, 100.0], 5.0, 0));
        let gid = a.project.data.groups[0].id;
        let n = a.project.data.loops.len();
        let letters: Vec<usize> = (0..n).filter(|&i| a.project.data.loops[i].grp == Some(gid)).collect();
        let other = n - 1;
        assert!(!letters.is_empty() && !letters.contains(&other));
        // nothing is faded while the text is just selected
        assert!((0..n).all(|i| !a.dimmed_by_preview(i)));
        a.edit_text(gid);
        assert!(begin_edit(&mut a, gid));
        a.text.refresh();
        assert!(!a.text.preview.is_empty());
        // editing: the letters of the text are faded, the other shapes are not
        assert!(letters.iter().all(|&i| a.dimmed_by_preview(i)));
        assert!(!a.dimmed_by_preview(other));
        // no preview yet (nothing to compare with): nothing is faded
        let keep = std::mem::take(&mut a.text.preview);
        assert!(letters.iter().all(|&i| !a.dimmed_by_preview(i)));
        a.text.preview = keep;
        // another text placed fresh is not the one being edited
        a.text.edit = None;
        assert!(letters.iter().all(|&i| !a.dimmed_by_preview(i)), "a new text has no original to fade");
        // closed again (Update or Cancel): back to normal
        a.text.edit = Some(gid);
        a.text.open = false;
        assert!(letters.iter().all(|&i| !a.dimmed_by_preview(i)));
        // a tool's own ghost still fades what it replaces
        a.ghost.hide.insert(other);
        assert!(a.dimmed_by_preview(other));
    }

    #[test]
    fn placed_text_is_a_group_that_remembers_its_settings() {
        let a = placed("Hi");
        let g = &a.project.data.groups[0];
        assert_eq!(g.text.as_ref().unwrap().content, "Hi");
        assert_eq!(g.name, "Text: Hi");
        assert!(!a.project.data.loops.is_empty() && a.project.data.loops.iter().all(|l| l.grp == Some(g.id)));
        assert_eq!(a.selected_text(), Some(g.id));
    }

    #[test]
    fn text_stays_editable_after_it_is_moved_scaled_and_turned() {
        let mut a = placed("Hi");
        let gid = a.project.data.groups[0].id;
        let b = bbox(&a, gid);
        // scale to twice the size, keeping the shape, then turn by 30 degrees about the centre, then move
        {
            let mut refs: Vec<&mut LoopData> = a.project.data.loops.iter_mut().collect();
            shapes::fit_to_bbox(&mut refs, b, [b[0] + 5.0, b[1] + 5.0, b[0] + 5.0 + (b[2] - b[0]) * 2.0, b[1] + 5.0 + (b[3] - b[1]) * 2.0]);
        }
        let b2 = bbox(&a, gid);
        let (cx, cy) = ((b2[0] + b2[2]) / 2.0, (b2[1] + b2[3]) / 2.0);
        for l in a.project.data.loops.iter_mut() { shapes::rotate_about(l, cx, cy, 30.0); shapes::translate(l, 7.0, -3.0); }
        let before = bbox(&a, gid);
        a.edit_text(gid);
        assert!(begin_edit(&mut a, gid));
        assert_eq!(a.text.edit, Some(gid));
        assert!((a.text.size - 24.0).abs() < 0.05, "size {}", a.text.size);
        assert!((a.text.angle - 30.0).abs() < 0.05, "angle {}", a.text.angle);
        // setting the same text again lands on the same place
        a.text.refresh();
        let again = shapes::bbox_many(a.text.preview.iter()).unwrap();
        for i in 0..4 { assert!((again[i] - before[i]).abs() < 0.3, "{again:?} vs {before:?}"); }
        // change the words and update: the group keeps its id and the new letters keep the size and the turn
        a.text.content = "Hello".into();
        place(&mut a);
        assert_eq!(a.project.data.groups.len(), 1);
        assert_eq!(a.project.data.groups[0].text.as_ref().unwrap().content, "Hello");
        assert!((a.project.data.groups[0].text.as_ref().unwrap().size - 24.0).abs() < 0.05);
        let after = bbox(&a, gid);
        assert!(after[2] - after[0] > before[2] - before[0], "Hello is wider than Hi");
        // and it can be edited again
        a.edit_text(gid);
        assert!(begin_edit(&mut a, gid));
        assert!((a.text.angle - 30.0).abs() < 0.05);
    }

    #[test]
    fn text_edited_as_shapes_becomes_plain_outlines() {
        let mut a = placed("Hi");
        let gid = a.project.data.groups[0].id;
        a.project.data.loops[0].pts[0] += 3000; // one point pulled away
        a.edit_text(gid);
        assert!(begin_edit(&mut a, gid));
        assert!(a.project.data.groups.is_empty());
        assert!(a.project.data.loops.iter().all(|l| l.grp.is_none()));
        assert!(a.error.is_some());
    }

    #[test]
    fn to_outlines_keeps_the_letters() {
        let mut a = placed("Hi");
        let gid = a.project.data.groups[0].id;
        let n = a.project.data.loops.len();
        a.text_to_outlines(gid);
        assert_eq!(a.project.data.loops.len(), n);
        assert!(a.project.data.groups.is_empty());
    }

    #[test]
    fn clicking_a_letter_selects_the_whole_text() {
        let a = placed("Hi");
        assert_eq!(a.text_siblings(0).len(), a.project.data.loops.len());
    }

    #[test]
    fn undo_and_redo_of_a_text_edit_keep_the_whole_text_selected() {
        let mut a = placed("Hi");
        let gid = a.project.data.groups[0].id;
        let letters = |a: &App| a.project.data.loops.iter().filter(|l| l.grp == Some(gid)).count();
        let short = letters(&a);
        // edit it to a longer text
        assert!(begin_edit(&mut a, gid));
        a.text.content = "Hello there".into();
        place(&mut a);
        let long = letters(&a);
        assert!(long > short);
        assert_eq!(a.sel.len(), long);
        a.do_undo();
        assert_eq!(letters(&a), short);
        assert_eq!(a.sel.len(), short, "the short text is selected as a whole");
        a.do_redo();
        assert_eq!(letters(&a), long);
        assert_eq!(a.sel.len(), long, "and so is the long one again");
    }

    #[test]
    fn fit_to_width_sets_the_height_and_is_remembered() {
        let mut a = App::new(None);
        a.text.content = "Hello".into();
        a.text.pos = Some([10.0, 20.0]);
        a.text.fit_on = true;
        a.text.fit_w = 80.0;
        a.text.refresh();
        let b = shapes::bbox_many(a.text.preview.iter()).unwrap();
        assert!((b[2] - b[0] - 80.0).abs() < 0.3, "{}", b[2] - b[0]);
        // longer text, same width: smaller letters
        let h = a.text.size;
        a.text.content = "Hello there".into();
        a.text.refresh();
        let b = shapes::bbox_many(a.text.preview.iter()).unwrap();
        assert!((b[2] - b[0] - 80.0).abs() < 0.3 && a.text.size < h);
        place(&mut a);
        let gid = a.project.data.groups[0].id;
        assert!((a.project.data.groups[0].text.as_ref().unwrap().fit_width - 80.0).abs() < 1e-9);
        a.edit_text(gid);
        assert!(begin_edit(&mut a, gid));
        assert!(a.text.fit_on && (a.text.fit_w - 80.0).abs() < 1e-6);
    }

    #[test]
    fn arc_text_warns_when_longer_than_the_circle_and_follows_the_angle() {
        let mut a = App::new(None);
        a.text.content = "HELLO WORLD".into();
        a.text.pos = Some([50.0, 50.0]);
        a.text.size = 10.0;
        a.text.flow = Flow::Arc;
        a.text.radius = 10.0;
        a.text.refresh();
        assert!(a.text.err.as_deref().is_some_and(|e| e.contains("overlap")), "{:?}", a.text.err);
        a.text.radius = 80.0;
        a.text.refresh();
        assert!(a.text.err.is_none());
        let top = shapes::bbox_many(a.text.preview.iter()).unwrap();
        a.text.arc_deg = 270.0;
        a.text.arc_in = true;
        a.text.refresh();
        let bottom = shapes::bbox_many(a.text.preview.iter()).unwrap();
        assert!(top != bottom, "moving the text round the circle changes the preview");
    }

    #[test]
    fn single_line_fonts_are_listed_first_and_make_open_strokes_that_stay_editable() {
        let mut a = App::new(None);
        assert!(!a.text.is_line_font(), "a fresh tool starts on an outline font");
        assert!(a.text.fonts[0].label.starts_with("Single line"));
        a.text.sel = a.text.fonts.iter().position(|f| f.label == "Single line · Roman").unwrap();
        assert!(a.text.is_line_font());
        a.text.content = "Ciao è".into();
        a.text.pos = Some([10.0, 20.0]);
        a.text.size = 10.0;
        place(&mut a);
        let ls: Vec<&LoopData> = a.project.data.loops.iter().collect();
        assert!(!ls.is_empty() && ls.iter().any(|l| l.c == 0), "open strokes");
        assert_eq!(a.project.data.groups[0].text.as_ref().unwrap().font, "Single line · Roman");
        let gid = a.project.data.groups[0].id;
        a.edit_text(gid);
        assert!(begin_edit(&mut a, gid));
        assert_eq!(a.text.edit, Some(gid), "still text, not outlines");
        assert!(a.text.is_line_font());
        assert!(a.text.name_preview(&egui::Context::default(), a.text.sel).is_none());
    }

    #[test]
    fn kerning_and_spacing_are_remembered_and_the_text_stays_editable() {
        let mut a = App::new(None);
        a.text.content = "AV To".into();
        a.text.sel = a.text.fonts.iter().position(|f| f.label == "Ubuntu Light").unwrap();
        a.text.pos = Some([10.0, 20.0]);
        a.text.size = 12.0;
        a.text.kerning = true;
        a.text.letter_pct = 5.0;
        a.text.word_pct = 40.0;
        place(&mut a);
        let spec = a.project.data.groups[0].text.clone().unwrap();
        assert!(spec.kerning && (spec.spacing - 0.6).abs() < 1e-9 && (spec.word_spacing - 4.8).abs() < 1e-9);
        let gid = a.project.data.groups[0].id;
        a.edit_text(gid);
        assert!(begin_edit(&mut a, gid));
        assert_eq!(a.text.edit, Some(gid), "still text, not outlines");
        assert!(a.text.kerning && (a.text.letter_pct - 5.0).abs() < 1e-6 && (a.text.word_pct - 40.0).abs() < 1e-6);
        // the same text without kerning is wider (AV and To are pulled together with it)
        let w_on = { a.text.refresh(); let b = shapes::bbox_many(a.text.preview.iter()).unwrap(); b[2] - b[0] };
        a.text.kerning = false;
        a.text.refresh();
        let w_off = { let b = shapes::bbox_many(a.text.preview.iter()).unwrap(); b[2] - b[0] };
        assert!(w_on < w_off, "kerning tightens the text: {w_on} vs {w_off}");
    }
}
