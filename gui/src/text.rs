//! Text tool: click where the text starts, type it, pick a font and place it as cuttable outlines.
use crate::app::{App, Mode};
use eframe::egui;
use putiacnc_core::model::LoopData;
use putiacnc_core::text::{text_loops, Align, TextStyle};
use std::sync::{mpsc, Arc};

#[derive(Clone)]
pub struct FontEntry {
    pub label: String,
    /// None for the fonts built into the app (the label tells which one).
    id: Option<fontdb::ID>,
}

pub struct TextTool {
    pub open: bool,
    pub focus: bool,
    /// Where the first line starts (baseline, left), mm.
    pub pos: Option<[f64; 2]>,
    pub content: String,
    pub size: f64,
    pub spacing: f64,
    pub align: Align,
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
    bytes: Option<(Vec<u8>, u32)>,
}

fn builtin(name: &str) -> Option<Vec<u8>> {
    egui::FontDefinitions::default().font_data.get(name).map(|d| d.font.to_vec())
}

impl Default for TextTool {
    fn default() -> Self {
        let fonts = vec![FontEntry { label: "Hack".into(), id: None }, FontEntry { label: "Ubuntu Light".into(), id: None }];
        TextTool {
            open: false, focus: false, pos: None, content: String::new(), size: 10.0, spacing: 0.0, align: Align::Left,
            fonts, sel: 0, picked: false, filter: String::new(), found: None, preview: vec![], err: None, db: None, rx: None, loading: false, key: String::new(), bytes_for: None, bytes: None,
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
                Some(FontEntry { label, id: Some(f.id) })
            }).collect();
            v.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
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
            for b in ["Ubuntu Light", "Hack"] { all.push(FontEntry { label: b.into(), id: None }); }
            all.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
            self.fonts = all;
            self.db = Some(db);
            self.rx = None;
            // keep the chosen font; a fresh tool starts on a plain, sturdy sans if there is one
            let want = if first_time { ["DejaVu Sans", "Liberation Sans", "Arial", "Noto Sans", "Cantarell"].iter().find(|w| self.fonts.iter().any(|f| f.label == **w)).map(|s| s.to_string()).or(current) } else { current };
            self.sel = want.and_then(|w| self.fonts.iter().position(|f| f.label == w)).unwrap_or(0);
            self.key.clear();
            self.bytes_for = None;
        }
    }

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

    fn style(&self) -> TextStyle { TextStyle { size_mm: self.size, spacing_mm: self.spacing, align: self.align } }

    fn refresh(&mut self) {
        let key = format!("{}|{}|{}|{}|{:?}|{:?}", self.content, self.sel, self.size, self.spacing, self.align, self.pos);
        if key == self.key { return; }
        self.key = key;
        self.preview.clear();
        self.err = None;
        let Some(pos) = self.pos else { return };
        if self.content.trim().is_empty() { return; }
        match self.font_bytes() {
            None => self.err = Some("This font could not be loaded.".into()),
            Some((data, idx)) => match text_loops(&data, idx, &self.content, self.style(), pos, 0) {
                Ok(l) => { if l.is_empty() { self.err = Some("This font has none of those letters.".into()); } self.preview = l; }
                Err(e) => self.err = Some(e),
            },
        }
    }
}

/// Adds the previewed text to the drawing, in the active operation, and selects it.
fn place(app: &mut App) {
    app.text.key.clear();
    app.text.refresh();
    let op = app.project.data.act;
    let mut loops = std::mem::take(&mut app.text.preview);
    if loops.is_empty() { return; }
    for l in &mut loops { l.op = op; }
    app.hist.record(&app.project.data);
    let start = app.project.data.loops.len();
    app.project.data.loops.extend(loops);
    app.sel = (start..app.project.data.loops.len()).collect();
    app.touch();
    app.text.open = false;
    app.text.content.clear();
    app.text.key.clear();
    app.mode = Mode::Select;
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if app.mode != Mode::Text { app.text.open = false; }
    if !app.text.open { return; }
    app.text.poll();
    if app.text.db.is_none() { app.text.load(ctx); }
    // text starts at the XY zero until a position is clicked or typed
    if app.text.pos.is_none() {
        let st = &app.project.data.stock;
        app.text.pos = Some([match st.zr.as_str() { "lr" | "ur" => st.w, "c" => st.w / 2.0, _ => 0.0 }, match st.zr.as_str() { "ul" | "ur" => st.h, "c" => st.h / 2.0, _ => 0.0 }]);
    }
    app.text.refresh();
    let stock = app.project.data.stock.clone();
    let (mut do_place, mut cancel) = (false, false);
    egui::Window::new("Text").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).resizable(false)
        .default_pos([700.0, 150.0]).fixed_size([300.0, 0.0]).show(ctx, |ui| {
        if crate::csd::dialog_header(ui, "Text", true) { cancel = true; }
        let t = &mut app.text;
        let te = ui.add(egui::TextEdit::multiline(&mut t.content).desired_rows(3).desired_width(f32::INFINITY).hint_text("Type your text"));
        if t.focus { te.request_focus(); t.focus = false; }
        ui.add_space(6.0);
        let st = &stock;
        let (zx, zy) = (match st.zr.as_str() { "lr" | "ur" => st.w, "c" => st.w / 2.0, _ => 0.0 }, match st.zr.as_str() { "ul" | "ur" => st.h, "c" => st.h / 2.0, _ => 0.0 });
        egui::Grid::new("text_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label("Start");
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
            ui.label("Font");
            let cur = t.fonts.get(t.sel).map(|f| f.label.clone()).unwrap_or_default();
            egui::ComboBox::from_id_source("text_font").width(190.0).height(400.0).selected_text(cur).show_ui(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut t.filter).hint_text("Search fonts").desired_width(f32::INFINITY));
                ui.add_space(2.0);
                let q = t.filter.to_lowercase();
                egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                    let mut any = false;
                    for i in 0..t.fonts.len() {
                        if !q.is_empty() && !t.fonts[i].label.to_lowercase().contains(&q) { continue; }
                        any = true;
                        let label = t.fonts[i].label.clone();
                        if ui.selectable_label(t.sel == i, label).clicked() { t.sel = i; t.picked = true; }
                    }
                    if !any { ui.weak("No font matches."); }
                });
            });
            ui.end_row();
            ui.label("Height");
            ui.horizontal(|ui| { crate::widgets::num(ui, [60.0, 24.0], &mut t.size, 0.5, 500.0, 0.1, 2, ""); ui.weak("mm, capital letters"); });
            ui.end_row();
            ui.label("Spacing");
            ui.horizontal(|ui| { crate::widgets::num(ui, [60.0, 24.0], &mut t.spacing, -5.0, 50.0, 0.05, 2, ""); ui.weak("mm between letters"); });
            ui.end_row();
            ui.label("Align");
            ui.horizontal(|ui| {
                for (a, n) in [(Align::Left, "Left"), (Align::Center, "Centre"), (Align::Right, "Right")] {
                    if ui.selectable_label(t.align == a, n).clicked() { t.align = a; }
                }
            });
            ui.end_row();
        });
        ui.add_space(2.0);
        ui.weak(match t.found {
            None => "Looking for the fonts installed on this computer…".to_string(),
            Some(n) => format!("{n} fonts found on this computer, plus Hack and Ubuntu Light."),
        });
        if let Some(e) = &t.err { ui.add_space(4.0); ui.colored_label(egui::Color32::from_rgb(0xe0, 0x8a, 0x2a), e); }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(!t.preview.is_empty(), egui::Button::new("Place")).clicked() { do_place = true; }
            if ui.button("Cancel").clicked() { cancel = true; }
        });
        ui.add_space(2.0);
        ui.weak("Letters become outlines; the holes in A, O, B… are cut out correctly. Click the drawing, or type X and Y, to move the text.");
    });
    if do_place { place(app); }
    if cancel { app.text.open = false; app.mode = Mode::Select; }
}
