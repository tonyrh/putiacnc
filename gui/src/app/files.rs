//! Opening, saving, importing and exporting files.
use putiacnc_core::i18n::{tr, tr_f};
use super::*;

impl App {
    pub fn open(&mut self, path: &str) {
        self.error = None;
        match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| load_project(&s)) {
            Ok(p) => { self.project = p; self.reset_after_load(); crate::prefs::push_recent(&mut self.prefs.recent, path); }
            Err(e) => { self.say(crate::app::Level::Error, tr_f("Could not open {path}: {e}", &[("path", &path), ("e", &e)])); self.prefs.recent.retain(|r| r != path); }
        }
    }

    pub fn file_name(&self) -> String {
        if self.path.is_empty() { "Untitled".into() }
        else { std::path::Path::new(&self.path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| self.path.clone()) }
    }

    /// Run an action, asking first if there are unsaved changes.
    pub fn request(&mut self, a: Action) {
        if self.modified { self.pending = Some(a); } else { self.perform(a); }
    }

    pub fn perform(&mut self, a: Action) {
        self.pending = None;
        match a {
            Action::New => self.new_project(),
            Action::Open(p) => { self.path = p.clone(); self.open(&p); }
            Action::Sample(k) => self.load_sample(k),
            Action::Quit => { self.quit_ok = true; self.want_quit = true; }
        }
    }

    /// Run a blocking file dialog on a worker thread; `poll_dialog` picks up the answer.
    pub fn spawn_dialog(&mut self, kind: DlgKind, f: impl FnOnce() -> Option<std::path::PathBuf> + Send + 'static) {
        if self.dlg.is_some() { return; }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || { let _ = tx.send(f()); });
        self.dlg = Some(Dlg { kind, rx });
    }

    /// Called every frame: finish a dialog that has been answered, keep the window awake while one is open.
    pub fn poll_dialog(&mut self, ctx: &eframe::egui::Context) {
        let Some(d) = &self.dlg else { return };
        let kind = d.kind;
        match d.rx.try_recv() {
            Ok(res) => {
                self.dlg = None;
                match res { Some(p) => self.finish_dialog(kind, p), None => self.after_save = None }
                ctx.request_repaint();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => { self.dlg = None; self.after_save = None; }
        }
    }

    pub(crate) fn finish_dialog(&mut self, kind: DlgKind, mut p: std::path::PathBuf) {
        self.remember_dir(kind, &p);
        match kind {
            DlgKind::Open => self.request(Action::Open(p.display().to_string())),
            DlgKind::SaveAs => {
                if p.extension().is_none() { p.set_extension("json"); }
                self.path = p.display().to_string();
                self.write_project();
                if !self.modified { if let Some(a) = self.after_save.take() { self.perform(a); } }
                self.after_save = None;
            }
            DlgKind::Import => self.import_file(&p.display().to_string()),
            DlgKind::Model => self.import_model(&p.display().to_string()),
            DlgKind::Bitmap => self.import_bitmap(&p.display().to_string()),
            DlgKind::ExportGcode => {
                if p.extension().is_none() { p.set_extension("nc"); }
                // the drawing may have changed while the dialog was open
                self.finish_generation();
                if self.output.is_none() { self.say(crate::app::Level::Error, tr_f("Export failed: {e}", &[("e", &self.gen_error.clone().unwrap_or_else(|| tr("there is no G-code.").into()))])); }
                let mode = if self.can_split() { self.export_split } else { putiacnc_core::split::Split::One };
                if mode != putiacnc_core::split::Split::One {
                    // the files are made on a worker, like the whole program is; the export window picks them up
                    let data = self.project.data.clone();
                    let units = if self.imperial { putiacnc_core::Units::Imperial } else { putiacnc_core::Units::Metric };
                    let (tx, rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || { let _ = tx.send(putiacnc_core::split::generate_files(&data, units, mode)); });
                    self.split_job = Some((rx, p));
                    return;
                }
                let files = self.gcode_files(&p);
                if !files.is_empty() {
                    let mut err = None;
                    for (path, text) in &files { if let Err(e) = std::fs::write(path, text) { err = Some(tr_f("Export failed: {e}", &[("e", &e)])); break; } }
                    match err {
                        Some(e) => self.say(crate::app::Level::Error, e),
                        None if files.len() > 1 => {
                            let names: Vec<String> = files.iter().filter_map(|(p, _)| p.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
                            self.say(Level::Info, tr_f("Saved {n} files: {names}. Each piece has its own work zero.", &[("n", &files.len()), ("names", &names.join(", "))]));
                        }
                        None => self.error = None,
                    }
                }
            }
            DlgKind::SetupSheet => self.write_setup_sheet(p),
            DlgKind::LibExport => self.finish_lib_export(p),
            DlgKind::LibImport => self.finish_lib_import(p),
            DlgKind::MachExport => self.finish_mach_export(p),
            DlgKind::MachImport => self.finish_mach_import(p),
        }
    }

    pub fn open_dialog(&mut self) {
        let dir = crate::prefs::existing_dir(&self.prefs.dir_project);
        self.spawn_dialog(DlgKind::Open, move || {
            let mut dlg = rfd::FileDialog::new().add_filter(tr("PutiaCNC project"), &["json"]).add_filter(tr("All files"), &["*"]);
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.pick_file()
        });
    }

    pub fn save_as_dialog(&mut self) {
        let name = if self.project.name.trim().is_empty() { "project".to_string() } else { self.project.name.trim().to_string() };
        let dir = if self.path.is_empty() { crate::prefs::existing_dir(&self.prefs.dir_project) } else { std::path::Path::new(&self.path).parent().map(|d| d.to_path_buf()) };
        self.spawn_dialog(DlgKind::SaveAs, move || {
            let mut dlg = rfd::FileDialog::new().add_filter(tr("PutiaCNC project"), &["json"]).set_file_name(format!("{name}.json"));
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.save_file()
        });
    }

    /// Save to the current path, asking for one if the project has none yet.
    pub fn save(&mut self) {
        if self.path.trim().is_empty() { self.save_as_dialog(); } else { self.write_project(); }
    }

    pub fn import_dialog(&mut self) {
        let dir = crate::prefs::existing_dir(&self.prefs.dir_svg).or_else(|| crate::prefs::existing_dir(&self.prefs.dir_project));
        self.spawn_dialog(DlgKind::Import, move || {
            let mut dlg = rfd::FileDialog::new().add_filter(tr("Drawings (SVG, DXF)"), &["svg", "svgz", "dxf"]).add_filter(tr("SVG drawing"), &["svg", "svgz"]).add_filter(tr("DXF drawing"), &["dxf"]).add_filter(tr("All files"), &["*"]);
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.pick_file()
        });
    }

    pub fn import_model_dialog(&mut self) {
        let dir = crate::prefs::existing_dir(&self.prefs.dir_svg).or_else(|| crate::prefs::existing_dir(&self.prefs.dir_project));
        self.spawn_dialog(DlgKind::Model, move || {
            let mut dlg = rfd::FileDialog::new().add_filter("3D models (STL, OBJ)", &["stl", "obj"]).add_filter(tr("All files"), &["*"]);
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.pick_file()
        });
    }

    pub fn import_bitmap_dialog(&mut self) {
        let dir = crate::prefs::existing_dir(&self.prefs.dir_svg).or_else(|| crate::prefs::existing_dir(&self.prefs.dir_project));
        self.spawn_dialog(DlgKind::Bitmap, move || {
            let mut dlg = rfd::FileDialog::new().add_filter(tr("Pictures (PNG, JPG, BMP)"), &["png", "jpg", "jpeg", "bmp"]).add_filter(tr("All files"), &["*"]);
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.pick_file()
        });
    }

    pub fn is_bitmap(path: &str) -> bool { let p = path.to_lowercase(); p.ends_with(".png") || p.ends_with(".jpg") || p.ends_with(".jpeg") || p.ends_with(".bmp") }

    /// Open the picture in the trace dialog; the shapes are placed when the user accepts.
    pub fn import_bitmap(&mut self, path: &str) {
        let st = (self.project.data.stock.w, self.project.data.stock.h);
        let saved = self.prefs.trace.clone();
        self.trace.load(path, st, saved);
    }

    pub fn is_model(path: &str) -> bool { let p = path.to_lowercase(); p.ends_with(".stl") || p.ends_with(".obj") }

    /// Import an STL model as a height map for relief operations. It is centred on the stock, and scaled
    /// down (keeping its proportions) when it is bigger than the stock.
    pub fn import_model(&mut self, path: &str) {
        let saved = if path.to_lowercase().ends_with(".obj") { self.prefs.model_orient_obj } else { self.prefs.model_orient_stl };
        self.model3d.load(path, saved);
    }

    /// Put a height map on the stock, centred, and scaled down (keeping its proportions) when it is bigger than the stock.
    pub fn place_relief(&mut self, rel: putiacnc_core::model::Relief) {
        let mut rel = rel;
        self.hist.record(&self.project.data);
        let (mw, mh) = rel.size();
        let mut note = vec![tr_f("Imported {name}: {w} × {h} × {z} mm.", &[("name", &rel.name), ("w", &format!("{:.1}", mw)), ("h", &format!("{:.1}", mh)), ("z", &format!("{:.1}", rel.zmax))])];
        let st = &self.project.data.stock;
        if mw > st.w || mh > st.h {
            rel.scale = (st.w / mw).min(st.h / mh);
            note.push(tr_f("Scaled down to {pct}% to fit the stock ({w} × {h} mm); change the size with Width in the Imported section.", &[("pct", &format!("{:.0}", rel.scale * 100.0)), ("w", &format!("{:.1}", rel.size().0)), ("h", &format!("{:.1}", rel.size().1))]));
        }
        let (mw, mh) = rel.size();
        rel.x = (st.w - mw) / 2.0;
        rel.y = (st.h - mh) / 2.0;
        self.project.data.relief = Some(rel);
        self.say(Level::Info, note.join(" "));
        self.touch();
    }

    /// True for the drawing formats `import_file` understands.
    pub fn is_drawing(path: &str) -> bool {
        let p = path.to_lowercase();
        p.ends_with(".svg") || p.ends_with(".svgz") || p.ends_with(".dxf")
    }

    /// Import an SVG or DXF file into the current project. The drawing is centred on the stock when it
    /// fits, otherwise its lower-left corner is put at the stock origin. New shapes are put
    /// in no operation and become the selection.
    pub fn import_file(&mut self, path: &str) {
        let dxf = path.to_lowercase().ends_with(".dxf");
        let imp = match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| if dxf { putiacnc_core::dxf::import(&b) } else { putiacnc_core::svg::import(&b) }) {
            Ok(i) => i,
            Err(e) => { self.say(crate::app::Level::Error, tr_f("Could not import {path}: {e}", &[("path", &path), ("e", &e)])); return; }
        };
        let gname = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "drawing".into());
        self.place_import(imp, gname);
    }

    /// Put imported shapes on the stock as one drawing: centred when it fits, otherwise its lower-left corner
    /// at the stock origin. The shapes are in no operation and become the selection.
    pub fn place_import(&mut self, imp: putiacnc_core::svg::Imported, gname: String) {
        if imp.loops.is_empty() { self.say(crate::app::Level::Warning, imp.warnings.join(" ")); return; }
        let Some(b) = shapes::bbox_many(imp.loops.iter()) else { return };
        let (bw, bh) = (b[2] - b[0], b[3] - b[1]);
        let st = &self.project.data.stock;
        let (ox, oy) = if bw <= st.w && bh <= st.h { ((st.w - bw) / 2.0 - b[0], (st.h - bh) / 2.0 - b[1]) } else { (-b[0], -b[1]) };
        let mut note = imp.warnings.clone();
        let mut sentences: Vec<(super::NoteUntil, String)> = Vec::new();
        let gid = self.project.data.groups.iter().map(|g| g.id).max().unwrap_or(0) + 1;
        let open = putiacnc_core::cleanup::open_count(&imp.loops);
        if open > 0 {
            note.push(putiacnc_core::i18n::tr_n(open as u64, "{n} open contour (loose ends are marked): Modify > Clean up drawing can join it.", "{n} open contours (loose ends are marked): Modify > Clean up drawing can join them."));
            sentences.push((super::NoteUntil::Closed(gid), note.last().cloned().unwrap_or_default()));
            self.show_open_ends = true;
        }
        if bw > st.w || bh > st.h {
            note.push(tr_f("The drawing ({w} × {h} mm) is larger than the stock; resize it with Width/Height in the Selection panel.", &[("w", &format!("{:.1}", bw)), ("h", &format!("{:.1}", bh))]));
            sentences.push((super::NoteUntil::Fits(gid), note.last().cloned().unwrap_or_default()));
        }
        self.hist.record(&self.project.data);
        let op = 0;
        let start = self.project.data.loops.len();
        self.project.data.groups.push(putiacnc_core::model::ShapeGroup { id: gid, name: gname, locked: false, text: None });
        // the layers of the file: a layer of the project with the same name takes the shapes, otherwise a new one is made
        // (with the file's colour, and hidden or locked as the file has it); shapes on no layer go on the active one
        let ids: Vec<i64> = imp.layers.iter().map(|il| {
            let d = &mut self.project.data;
            if let Some(l) = d.all_layers().into_iter().find(|l| l.name == il.name) { return l.id; }
            let id = d.add_layer(&il.name);
            if let Some(l) = d.layer_mut(id) {
                if let Some(c) = il.color { l.color = putiacnc_core::model::hex_rgb(c); }
                l.hidden = il.hidden;
                l.locked = il.locked;
            }
            id
        }).collect();
        for mut l in imp.loops {
            shapes::translate(&mut l, ox, oy);
            l.op = op;
            l.grp = Some(gid);
            l.lay = match l.lay.and_then(|n| ids.get((n - 1) as usize).copied()) { Some(0) => None, Some(id) => Some(id), None => self.new_shape_layer() };
            self.project.data.loops.push(l);
        }
        self.sel = (start..self.project.data.loops.len()).collect();
        self.mode = Mode::Select;
        if note.is_empty() { self.error = None; } else { self.say(Level::Warning, note.join(" ")); }
        self.notes = sentences;
        self.touch();
    }

    /// The G-code files an export writes: the main piece to `path`, then one per plug piece next to it ("name-plug.nc").
    pub fn gcode_files(&self, path: &std::path::Path) -> Vec<(std::path::PathBuf, String)> {
        let Some(o) = &self.output else { return vec![] };
        let mut v = vec![(path.to_path_buf(), o.gcode())];
        for pp in &self.pieces {
            let suffix = if self.pieces.len() == 1 { "-plug".to_string() } else { format!("-plug-{}", pp.job.op_index + 1) };
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let name = match path.extension() { Some(e) => format!("{stem}{suffix}.{}", e.to_string_lossy()), None => format!("{stem}{suffix}") };
            v.push((path.with_file_name(name), pp.output.gcode()));
        }
        v
    }

    /// Export G-code: with two or more operations that can stand alone the export window asks how to split the program first.
    /// The path of a split file: the chosen name with the file's ending before the extension.
    pub fn split_path(path: &std::path::Path, suffix: &str) -> std::path::PathBuf {
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let name = match path.extension() { Some(e) => format!("{stem}{suffix}.{}", e.to_string_lossy()), None => format!("{stem}{suffix}") };
        path.with_file_name(name)
    }

    /// Picks up the split files when they are ready and writes them, with the plug pieces next to them.
    pub fn poll_split(&mut self) -> bool {
        let Some((rx, _)) = &self.split_job else { return false };
        let res = match rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return true,
            Err(_) => Err(tr("the split stopped without an answer.").to_string()),
        };
        let (_, path) = self.split_job.take().expect("a job");
        match res {
            Err(e) => self.say(crate::app::Level::Error, tr_f("Export failed: {e}", &[("e", &e)])),
            Ok(files) => {
                let mut out: Vec<(std::path::PathBuf, String)> = files.iter().map(|f| (Self::split_path(&path, &f.suffix), f.out.gcode())).collect();
                out.extend(self.gcode_files(&path).into_iter().skip(1));
                let mut err = None;
                for (p, t) in &out { if let Err(e) = std::fs::write(p, t) { err = Some(tr_f("Export failed: {e}", &[("e", &e)])); break; } }
                match err {
                    Some(e) => self.say(crate::app::Level::Error, e),
                    None => {
                        let names: Vec<String> = out.iter().filter_map(|(p, _)| p.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
                        self.error = None;
                        self.say(Level::Info, tr_f("Saved {n} files: {names}. Each file is complete: check the note at its top before you start it.", &[("n", &out.len()), ("names", &names.join(", "))]));
                    }
                }
            }
        }
        false
    }

    /// A number that stands for the current warnings (what they are and where), so "Export anyway" is asked once for the same ones.
    pub fn checks_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for (i, f) in self.checks.iter().enumerate() { (f.kind, f.op, f.count, &f.msg, self.check_parts.get(i)).hash(&mut h); }
        h.finish()
    }

    pub fn export_gcode_dialog(&mut self) {
        self.finish_generation();
        if self.output.is_none() { self.say(crate::app::Level::Warning, tr("Nothing to export: there is no G-code yet.").into()); return; }
        // warnings about the toolpath that the person has not seen the export through yet: ask first (only then)
        if self.checks_ver == self.out_ver && !self.checks.is_empty() && self.checks_key() != self.export_ack { self.export_warn_open = true; return; }
        if self.can_split() { self.export_open = true; } else { self.export_split = putiacnc_core::split::Split::One; self.choose_export_file(); }
    }

    /// File → Setup sheet…: asks where to save the page, then writes it and opens it in the browser to print or save as PDF.
    pub fn setup_sheet_dialog(&mut self) {
        self.finish_generation();
        if self.output.is_none() { self.say(crate::app::Level::Warning, tr("There is no toolpath yet: add an operation first.").into()); return; }
        let name = if self.project.name.trim().is_empty() { "job".to_string() } else { self.project.name.trim().to_string() };
        let dir = crate::prefs::existing_dir(&self.prefs.dir_gcode)
            .or_else(|| if self.path.is_empty() { None } else { std::path::Path::new(&self.path).parent().map(|d| d.to_path_buf()) })
            .or_else(|| crate::prefs::existing_dir(&self.prefs.dir_project));
        self.spawn_dialog(DlgKind::SetupSheet, move || {
            let mut dlg = rfd::FileDialog::new().add_filter(tr("Web page"), &["html"]).set_file_name(format!("{name} setup sheet.html"));
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.save_file()
        });
    }

    fn write_setup_sheet(&mut self, mut p: std::path::PathBuf) {
        if p.extension().is_none() { p.set_extension("html"); }
        self.finish_generation();
        let Some(out) = self.output.clone() else { self.say(crate::app::Level::Warning, tr("There is no toolpath yet.").into()); return };
        let mut parts = vec![putiacnc_core::pieces::PartOutput { piece: None, out }];
        parts.extend(self.pieces.iter().map(|pp| putiacnc_core::pieces::PartOutput { piece: Some(pp.job.clone()), out: pp.output.clone() }));
        let units = if self.imperial { putiacnc_core::Units::Imperial } else { putiacnc_core::Units::Metric };
        let page = putiacnc_core::setupsheet::html(&self.project.data, &parts, units, &putiacnc_core::setupsheet::SheetInfo { version: env!("CARGO_PKG_VERSION").into() }, &self.project.name);
        match std::fs::write(&p, page) {
            Ok(()) => {
                let _ = open::that_detached(&p);
                self.say(Level::Info, tr_f("Saved {file} and opened it in your browser: print it, or save it as PDF from there.", &[("file", &p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())]));
            }
            Err(e) => self.say(crate::app::Level::Error, tr_f("Could not save the setup sheet: {e}", &[("e", &e)])),
        }
    }

    pub fn can_split(&self) -> bool {
        putiacnc_core::split::groups(&self.project.data, putiacnc_core::split::Split::PerOperation).len() >= 2
    }

    /// The file dialog of an export; the program is written when a name is chosen.
    pub fn choose_export_file(&mut self) {
        let name = if self.project.name.trim().is_empty() { "output".to_string() } else { self.project.name.trim().to_string() };
        let dir = crate::prefs::existing_dir(&self.prefs.dir_gcode)
            .or_else(|| if self.path.is_empty() { None } else { std::path::Path::new(&self.path).parent().map(|d| d.to_path_buf()) })
            .or_else(|| crate::prefs::existing_dir(&self.prefs.dir_project));
        self.spawn_dialog(DlgKind::ExportGcode, move || {
            let mut dlg = rfd::FileDialog::new().add_filter("G-code", &["nc", "gcode", "tap", "gc"]).set_file_name(format!("{name}.nc"));
            if let Some(d) = dir { dlg = dlg.set_directory(d); }
            dlg.save_file()
        });
    }

    pub(super) fn write_project(&mut self) {
        self.finish_generation();
        let mut p = self.project.clone();
        // like the browser app, a project file only carries the tools its operations use
        let used: Vec<String> = p.data.ops.iter().map(|o| o.p.tool.clone()).collect();
        p.data.tools.retain(|t| used.contains(&t.id));
        if let Some(o) = &self.output { p.data.gcode = o.gcode(); }
        match save_project(&p).and_then(|s| std::fs::write(&self.path, s).map_err(|e| e.to_string())) {
            Ok(()) => { self.modified = false; self.error = None; let p = self.path.clone(); crate::prefs::push_recent(&mut self.prefs.recent, &p); }
            Err(e) => self.say(crate::app::Level::Error, tr_f("Save failed: {e}", &[("e", &e)])),
        }
    }

}
