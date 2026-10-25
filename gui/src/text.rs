//! Text tool: click where the text starts, type it, pick a font and place it as cuttable outlines.
use crate::app::{App, Mode};
use eframe::egui;
use putiacnc_core::model::LoopData;
use putiacnc_core::shapes;
use putiacnc_core::text::{apply_affine, fit_affine, text_loops, Affine, Align, Flow, TextStyle};
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
    pub flow: Flow,
    /// Radius of the circle for the arc layouts, mm.
    pub radius: f64,
    /// Offset of the shadow copy, mm; 0 and 0 means no shadow.
    pub shadow: [f64; 2],
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
    bytes: Option<(Vec<u8>, u32)>,
}

fn builtin(name: &str) -> Option<Vec<u8>> {
    egui::FontDefinitions::default().font_data.get(name).map(|d| d.font.to_vec())
}

impl Default for TextTool {
    fn default() -> Self {
        let fonts = vec![FontEntry { label: "Hack".into(), id: None }, FontEntry { label: "Ubuntu Light".into(), id: None }];
        TextTool {
            open: false, focus: false, pos: None, content: String::new(), size: 10.0, spacing: 0.0, align: Align::Left, flow: Flow::Straight, radius: 60.0, shadow: [0.0, 0.0], angle: 0.0, rest: [1.0, 0.0, 0.0, 1.0], edit: None, edit_req: None,
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
            for b in ["Ubuntu Light", "Hack"] { all.push(FontEntry { label: b.into(), id: None }); }
            all.sort_by_key(|a| a.label.to_lowercase());
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

    fn style(&self) -> TextStyle { TextStyle { size_mm: self.size, spacing_mm: self.spacing, align: self.align, flow: self.flow, radius_mm: self.radius, shadow: self.shadow } }

    fn refresh(&mut self) {
        let key = format!("{}|{}|{}|{}|{:?}|{:?}|{}|{:?}|{:?}|{}|{:?}", self.content, self.sel, self.size, self.spacing, self.align, self.pos, self.angle, self.rest, self.flow, self.radius, self.shadow);
        if key == self.key { return; }
        self.key = key;
        self.preview.clear();
        self.err = None;
        let Some(pos) = self.pos else { return };
        if self.content.trim().is_empty() { return; }
        match self.font_bytes() {
            None => self.err = Some("This font could not be loaded.".into()),
            Some((data, idx)) => match text_loops(&data, idx, &self.content, self.style(), [0.0, 0.0], 0) {
                Ok(mut l) => {
                    if l.is_empty() { self.err = Some("This font has none of those letters.".into()); }
                    // set at the origin, then turned and moved: the same steps the editor takes back later
                    apply_affine(&mut l, &Affine::compose(self.angle.to_radians(), self.rest, pos));
                    self.preview = l;
                }
                Err(e) => self.err = Some(e),
            },
        }
    }
}

/// Adds the previewed text to the drawing, in the active operation, and selects it. The shapes form a group that
/// remembers the settings, so the text can be edited later; when a text is being edited its letters are replaced.
pub fn place(app: &mut App) {
    app.text.key.clear();
    app.text.refresh();
    let mut loops = std::mem::take(&mut app.text.preview);
    if loops.is_empty() { return; }
    let t = &app.text;
    let spec = putiacnc_core::model::TextSpec {
        content: t.content.clone(), font: t.fonts.get(t.sel).map(|f| f.label.clone()).unwrap_or_default(), size: t.size, spacing: t.spacing, align: t.align, flow: t.flow, radius: t.radius, shadow: t.shadow,
    };
    let first = t.content.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().chars().take(24).collect::<String>();
    let name = format!("Text: {first}");
    app.hist.record(&app.project.data);
    if let Some(gid) = app.text.edit {
        let d = &mut app.project.data;
        let old: Vec<usize> = d.loops.iter().enumerate().filter(|(_, l)| l.grp == Some(gid)).map(|(i, _)| i).collect();
        let first_op = old.first().map(|&i| d.loops[i].op).unwrap_or(d.act);
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
        let op = app.project.data.act;
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

    /// Drops the link to the settings: the letters stay as they are, as plain outlines.
    pub fn text_to_outlines(&mut self, gid: i64) {
        self.hist.record(&self.project.data);
        for l in self.project.data.loops.iter_mut().filter(|l| l.grp == Some(gid)) { l.grp = None; }
        self.project.data.groups.retain(|g| g.id != gid);
        self.touch();
    }

    /// All the letters of the placed text that loop `i` belongs to, or just `i` for any other shape.
    pub fn text_siblings(&self, i: usize) -> Vec<usize> {
        let d = &self.project.data;
        match d.loops.get(i).and_then(|l| l.grp).filter(|g| d.groups.iter().any(|x| x.id == *g && x.text.is_some())) {
            Some(g) => d.loops.iter().enumerate().filter(|(_, l)| l.grp == Some(g)).map(|(k, _)| k).collect(),
            None => vec![i],
        }
    }

    /// A placed text is selected as a whole: any letter in the selection brings in the rest.
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
        app.error = Some(format!("The font \"{}\" is not on this computer, so this text cannot be edited here. You can still convert it to outlines.", spec.font));
        app.text.open = false;
        app.mode = Mode::Select;
        return true;
    };
    app.text.sel = idx;
    app.text.picked = true;
    app.text.bytes_for = None;
    let style = TextStyle { size_mm: spec.size, spacing_mm: spec.spacing, align: spec.align, flow: spec.flow, radius_mm: spec.radius, shadow: spec.shadow };
    let g0 = app.text.font_bytes().and_then(|(data, face)| text_loops(&data, face, &spec.content, style, [0.0, 0.0], 0).ok()).unwrap_or_default();
    let now: Vec<&LoopData> = app.project.data.loops.iter().filter(|l| l.grp == Some(gid)).collect();
    let fit = fit_affine(&g0, &now).filter(|(_, res)| *res < 0.02);
    let Some((af, _)) = fit else {
        // the letters were edited point by point (or the font draws them differently now): they stay as outlines
        app.text_to_outlines(gid);
        app.error = Some("These letters were edited as shapes, so they can no longer be edited as text. They are plain outlines now.".into());
        app.text.open = false;
        app.mode = Mode::Select;
        return true;
    };
    let (s, turn, rest) = af.decompose();
    let t = &mut app.text;
    t.content = spec.content;
    t.size = spec.size * s;
    t.spacing = spec.spacing * s;
    t.align = spec.align;
    t.flow = spec.flow;
    t.radius = spec.radius * s;
    t.shadow = [spec.shadow[0] * s, spec.shadow[1] * s];
    t.angle = turn.to_degrees();
    t.rest = rest;
    t.pos = Some(af.t);
    t.edit = Some(gid);
    t.key.clear();
    true
}

pub fn dialog(ctx: &egui::Context, app: &mut App) {
    if app.mode != Mode::Text { app.text.open = false; }
    if !app.text.open { return; }
    app.text.poll();
    if app.text.db.is_none() { app.text.load(ctx); }
    if let Some(gid) = app.text.edit_req {
        if begin_edit(app, gid) { app.text.edit_req = None; } else { ctx.request_repaint_after(std::time::Duration::from_millis(100)); return; }
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
        .current_pos(crate::csd::spot(ctx, "Text", egui::pos2(700.0, 150.0))).show(ctx, |ui| {
        ui.set_width(376.0);
        if crate::csd::dialog_header(ui, if editing { "Edit text" } else { "Text" }, true) { cancel = true; }
        crate::csd::scroll(ui, 90.0, |ui| {
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
            ui.label("Angle");
            ui.horizontal(|ui| { crate::widgets::num(ui, [60.0, 24.0], &mut t.angle, -360.0, 360.0, 1.0, 1, ""); ui.weak("degrees, counter-clockwise"); });
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
            ui.label("Layout");
            egui::ComboBox::from_id_source("text_flow").width(190.0).selected_text(match t.flow {
                Flow::Straight => "Straight", Flow::ArcTop => "Round a circle, on top", Flow::ArcBottom => "Round a circle, underneath", Flow::Vertical => "Stacked, top to bottom",
            }).show_ui(ui, |ui| {
                ui.selectable_value(&mut t.flow, Flow::Straight, "Straight");
                ui.selectable_value(&mut t.flow, Flow::ArcTop, "Round a circle, on top");
                ui.selectable_value(&mut t.flow, Flow::ArcBottom, "Round a circle, underneath");
                ui.selectable_value(&mut t.flow, Flow::Vertical, "Stacked, top to bottom");
            }).response.on_hover_text("Round a circle: Start is where the baseline touches the circle (its top, or its bottom), and Align says which part of the text sits there");
            ui.end_row();
            if matches!(t.flow, Flow::ArcTop | Flow::ArcBottom) {
                ui.label("Radius");
                ui.horizontal(|ui| { crate::widgets::num(ui, [60.0, 24.0], &mut t.radius, 5.0, 5000.0, 0.5, 2, ""); ui.weak("mm, of the baseline"); });
                ui.end_row();
            }
            ui.label("Shadow");
            ui.horizontal(|ui| {
                ui.label("X");
                crate::widgets::num(ui, [50.0, 24.0], &mut t.shadow[0], -100.0, 100.0, 0.1, 2, "");
                ui.add_space(4.0);
                ui.label("Y");
                crate::widgets::num(ui, [50.0, 24.0], &mut t.shadow[1], -100.0, 100.0, 0.1, 2, "");
                ui.weak("mm; 0 and 0 = none");
            }).response.on_hover_text("A second copy of the letters, moved by this much. Give it an operation of its own (for example a shallower V-carve) to cut a drop shadow");
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
            if ui.add_enabled(!t.preview.is_empty(), egui::Button::new(if editing { "Update" } else { "Place" })).clicked() { do_place = true; }
            if ui.button("Cancel").clicked() { cancel = true; }
        });
        ui.add_space(2.0);
        ui.weak(if editing { "The letters are set again from these settings, where the text is now." } else { "Letters become outlines (holes in A, O, B… are cut out); the text stays editable: double-click it later. Click the drawing, or type X and Y, to move it." });
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
}
