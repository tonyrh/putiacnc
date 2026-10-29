//! Editing the drawing: history steps, delete, duplicate, offset, combine, nudge, snapping.
use super::*;

impl App {
    /// Call after any change to the project.
    pub fn touch(&mut self) {
        self.clear_fixed_notes();
        self.note_empty_ops();
        self.modified = true;
        self.gen_pending = true;
    }

    /// Drops the banner sentences whose problem has been fixed (the drawing fits, the contours are joined, the text is outlines).
    fn clear_fixed_notes(&mut self) {
        if self.notes.is_empty() { return; }
        if self.error.is_none() { self.notes.clear(); return; }
        let d = &self.project.data;
        let done: Vec<String> = self.notes.iter().filter(|(until, _)| match *until {
            NoteUntil::Fits(g) => match shapes::bbox_many(d.loops.iter().filter(|l| l.grp == Some(g))) {
                Some(b) => (b[2] - b[0]) <= d.stock.w + 1e-6 && (b[3] - b[1]) <= d.stock.h + 1e-6,
                None => true,
            },
            NoteUntil::Closed(g) => !d.loops.iter().any(|l| l.grp == Some(g) && putiacnc_core::cleanup::is_open_path(l)),
            NoteUntil::NotText(g) => !d.groups.iter().any(|x| x.id == g && x.text.is_some()),
        }).map(|(_, s)| s.clone()).collect();
        if done.is_empty() { return; }
        self.notes.retain(|(_, s)| !done.contains(s));
        if let Some(mut e) = self.error.take() {
            let level = self.banner_level_with(&e);
            for s in &done { e = e.replacen(s.as_str(), "", 1); }
            let e = e.split_whitespace().collect::<Vec<_>>().join(" ");
            if !e.is_empty() { self.banner_level = level.map(|l| (l, e.clone())); self.error = Some(e); }
        }
    }

    /// An operation that has lost its last shape stays, with all its settings, so the shapes can be redrawn and put back with
    /// "Use selected shapes" in the operation. The status bar says so once, when it happens; the trash icon removes the operation.
    /// A 3D relief works on the model, not on shapes, so it is never counted.
    pub(super) fn note_empty_ops(&mut self) {
        let d = &self.project.data;
        let empty: Vec<i64> = d.ops.iter().filter(|o| o.p.kind != "relief" && !d.loops.iter().any(|l| l.op == o.id)).map(|o| o.id).collect();
        let fresh: Vec<i64> = empty.iter().copied().filter(|id| !self.empty_ops.contains(id)).collect();
        if !fresh.is_empty() {
            let name = |id: i64| d.ops.iter().position(|o| o.id == id).map(|k| format!("{} ({})", k + 1, crate::panels::label_of(crate::panels::KINDS, &d.ops[k].p.kind))).unwrap_or_default();
            self.note = Some(if fresh.len() == 1 {
                format!("Operation {} has no shapes now but keeps its settings: select shapes and press \"Use selected shapes\" in it", name(fresh[0]))
            } else {
                format!("{} operations have no shapes now but keep their settings", fresh.len())
            });
        }
        self.empty_ops = empty;
    }

    /// Undo the last change of any kind (Edit menu, Ctrl+Z).
    pub fn do_undo(&mut self) { self.step(Scope::All, true); }
    pub fn do_redo(&mut self) { self.step(Scope::All, false); }
    /// Undo the last change to the drawing, leaving stock, operations, tools and the rest alone (toolbar).
    pub fn do_undo_drawing(&mut self) { self.step(Scope::Drawing, true); }
    pub fn do_redo_drawing(&mut self) { self.step(Scope::Drawing, false); }

    pub(super) fn step(&mut self, s: Scope, undo: bool) {
        let before = self.project.data.clone();
        if !self.hist.step(&mut self.project.data, s, undo) { return; }
        // a placed text is selected as a whole, but its letters may be more or fewer after the step: follow the text, not the indices
        let is_text = |d: &ProjectData, g: i64| d.groups.iter().any(|x| x.id == g && x.text.is_some());
        let mut texts: Vec<i64> = self.sel.iter().filter_map(|&i| before.loops.get(i).and_then(|l| l.grp)).filter(|&g| is_text(&before, g)).collect();
        texts.sort_unstable();
        texts.dedup();
        // the same for the groups made with Edit > Group
        let mut ugs: Vec<i64> = self.sel.iter().filter_map(|&i| before.loops.get(i).and_then(|l| l.ug)).collect();
        ugs.sort_unstable();
        ugs.dedup();
        let others: std::collections::BTreeSet<usize> = self.sel.iter().copied().filter(|&i| {
            let l = before.loops.get(i);
            !l.and_then(|l| l.grp).is_some_and(|g| texts.contains(&g)) && !l.and_then(|l| l.ug).is_some_and(|g| ugs.contains(&g))
        }).collect();
        self.sel = others;
        self.after_history();
        let d = &self.project.data;
        for g in texts.into_iter().filter(|&g| is_text(d, g)) {
            self.sel.extend(d.loops.iter().enumerate().filter(|(_, l)| l.grp == Some(g)).map(|(k, _)| k));
        }
        for g in ugs { self.sel.extend(d.loops.iter().enumerate().filter(|(_, l)| l.ug == Some(g)).map(|(k, _)| k)); }
    }

    pub(super) fn after_history(&mut self) {
        shapes::repair_refs(&mut self.project.data);
        let n = self.project.data.loops.len();
        self.sel.retain(|&i| i < n);
        self.apply_lib_tools();
        let cur = self.machines.def.clone();
        self.project.data.machine = Some(self.machines.resolve(&self.project.data.machine.as_ref().map(|m| m.id.clone()).unwrap_or(cur)).clone());
        self.apply_machine_job();
        self.touch();
    }

    pub fn delete_selection(&mut self) {
        if self.sel.is_empty() { return; }
        self.hist.record(&self.project.data);
        for &i in self.sel.iter().rev() { self.project.data.loops.remove(i); }
        let removed: Vec<usize> = self.sel.iter().copied().collect();
        shapes::tabs_after_delete(&mut self.project.data.ops, &removed);
        self.sel.clear();
        // the tools that work on the selection have nothing left to work on
        self.close_tool_windows();
        self.touch();
    }

    /// Closes the windows of the tools that work on the selection.
    pub fn close_tool_windows(&mut self) {
        self.align_open = false; self.combine_open = false; self.array.open = false; self.offset_open = false; self.corners.open = false; self.clean.open = false;
    }

    /// Shows `msg` in the banner at the given level.
    pub fn say(&mut self, level: Level, msg: String) {
        self.banner_level = Some((level, msg.clone()));
        self.error = Some(msg);
    }

    fn banner_level_with(&self, msg: &str) -> Option<Level> {
        self.banner_level.as_ref().filter(|(_, m)| m == msg).map(|(l, _)| *l)
    }

    /// The level of the message now in the banner.
    pub fn banner_level_now(&self) -> Level {
        match (&self.banner_level, &self.error) {
            (Some((l, m)), Some(e)) if m == e => *l,
            _ => Level::Error,
        }
    }

    /// The Text window with whatever it was doing: a text being edited goes back as it was.
    pub fn close_text_window(&mut self) {
        if !self.text.open && self.text.edit.is_none() { return; }
        self.text.open = false; self.text.edit = None; self.text.angle = 0.0; self.text.rest = [1.0, 0.0, 0.0, 1.0];
    }

    pub fn tool_window_open(&self, w: ToolWin) -> bool {
        match w {
            ToolWin::Align => self.align_open,
            ToolWin::Offset => self.offset_open,
            ToolWin::Array => self.array.open,
            ToolWin::Corners => self.corners.open,
            ToolWin::Combine => self.combine_open,
            ToolWin::Clean => self.clean.open,
        }
    }

    /// The button of a tool: closes its window when it is open, opens it (and closes the others) when it is not.
    pub fn toggle_tool_window(&mut self, w: ToolWin) {
        if self.tool_window_open(w) { self.close_tool_windows(); } else { self.open_tool_window(w); }
    }

    /// Safety net for any way of opening a tool window: when more than one is open, the one opened last stays.
    pub fn one_tool_window(&mut self) {
        const ALL: [ToolWin; 6] = [ToolWin::Align, ToolWin::Offset, ToolWin::Array, ToolWin::Corners, ToolWin::Combine, ToolWin::Clean];
        let open: Vec<ToolWin> = ALL.into_iter().filter(|w| self.tool_window_open(*w)).collect();
        if open.len() > 1 {
            let newest = open.iter().copied().find(|w| !self.tool_wins_seen.contains(w)).unwrap_or(open[open.len() - 1]);
            self.open_tool_window(newest);
        }
        self.tool_wins_seen = ALL.into_iter().filter(|w| self.tool_window_open(*w)).collect();
    }

    /// Opens the window of a selection tool. Only one tool is active at a time, so every other tool window closes, the Text tool too.
    pub fn open_tool_window(&mut self, w: ToolWin) {
        self.close_tool_windows();
        if self.mode == Mode::Text { self.mode = Mode::Select; }
        self.close_text_window();
        match w {
            ToolWin::Align => self.align_open = true,
            ToolWin::Offset => self.offset_open = true,
            ToolWin::Array => self.array.open = true,
            ToolWin::Corners => self.corners.open = true,
            ToolWin::Combine => self.combine_open = true,
            ToolWin::Clean => { self.clean.open = true; self.clean.msg.clear(); }
        }
    }

    /// Runs a tool that changes the selected shapes and shows its message, if any. True when the drawing changed, so the
    /// tool's window can close; when nothing happened it stays open for new values.
    pub fn run_tool(&mut self, tool: &'static str, f: impl FnOnce(&mut App) -> Option<String>) -> bool {
        let before = self.project.data.loops.clone();
        self.tool_note = None;
        let msg = f(self);
        let changed = self.project.data.loops != before;
        // a result that only matters while the window is open stays in the window; a note about what was done goes to the banner
        if let Some(m) = msg { if changed { self.say(Level::Info, m); } else { self.tool_note = Some((tool, m)); } }
        changed
    }

    pub fn duplicate_selection(&mut self) {
        if self.sel.is_empty() { return; }
        self.hist.record(&self.project.data);
        let off = self.snap_mm.max(5.0);
        let mut new = Vec::new();
        let mut gc = shapes::GroupCopies::new(&self.project.data.loops);
        let mut text_copies: std::collections::HashMap<i64, Option<i64>> = Default::default();
        for &i in &self.sel {
            let mut l = self.project.data.loops[i].clone();
            gc.apply(&mut l); // a copy of a group is a group of its own
            // a copy of a placed text is a text of its own (listed in the panel, editable); a copy of an imported drawing's shape is loose
            l.grp = l.grp.and_then(|g| *text_copies.entry(g).or_insert_with(|| shapes::copy_text_group(&mut self.project.data, g)));
            shapes::translate(&mut l, off, -off);
            new.push(l);
        }
        let start = self.project.data.loops.len();
        self.project.data.loops.extend(new);
        let src: Vec<usize> = self.sel.iter().copied().collect();
        for (k, &i) in src.iter().enumerate() { shapes::tabs_copy(&mut self.project.data.ops, i, start + k); }
        self.sel = (start..self.project.data.loops.len()).collect();
        self.touch();
    }

    /// Grows (or shrinks) the selected closed shapes by `dist` mm. With `keep` the originals stay and the
    /// results are added; otherwise a result replaces its original. Returns a note when something was skipped.
    pub fn offset_selection(&mut self, dist: f64, keep: bool) -> Option<String> {
        if self.sel.is_empty() || dist.abs() < 1e-9 { return None; }
        self.hist.record(&self.project.data);
        let (mut skipped, mut vanished) = (0, 0);
        let mut added: Vec<putiacnc_core::model::LoopData> = vec![];
        let mut replaced: Vec<usize> = vec![];
        let mut removed: Vec<usize> = vec![];
        let sel: Vec<usize> = self.sel.iter().copied().collect();
        for i in sel {
            let l = self.project.data.loops[i].clone();
            if !l.closed() { skipped += 1; continue; }
            let res = shapes::offset_loop(&l, dist);
            if res.is_empty() { vanished += 1; continue; }
            if keep { added.extend(res); }
            else if res.len() == 1 { self.project.data.loops[i] = res.into_iter().next().unwrap(); replaced.push(i); }
            else { removed.push(i); added.extend(res); }
        }
        removed.sort_unstable();
        for &i in removed.iter().rev() { self.project.data.loops.remove(i); }
        shapes::tabs_after_delete(&mut self.project.data.ops, &removed);
        let shift = |i: usize| i - removed.iter().filter(|&&r| r < i).count();
        let start = self.project.data.loops.len();
        self.project.data.loops.extend(added);
        self.sel = replaced.iter().map(|&i| shift(i)).chain(start..self.project.data.loops.len()).collect();
        self.touch();
        let mut notes = vec![];
        if skipped > 0 { notes.push(format!("{skipped} open shape{} cannot be offset", if skipped == 1 { "" } else { "s" })); }
        if vanished > 0 { notes.push(format!("{vanished} shape{} too small to shrink by {} mm", if vanished == 1 { " is" } else { "s are" }, dist.abs())); }
        if notes.is_empty() { None } else { Some(notes.join("; ") + ".") }
    }

    /// Replaces the selected closed shapes by their union, difference, intersection or exclusion.
    /// Returns a message when nothing could be done or something was left out.
    pub fn combine_selection(&mut self, op: shapes::BoolOp, keep: bool, swap: bool) -> Option<String> {
        let idx: Vec<usize> = self.sel.iter().copied().filter(|&i| self.project.data.loops.get(i).is_some_and(|l| l.closed())).collect();
        let open = self.sel.len() - idx.len();
        if idx.len() < 2 { return Some("Select at least two closed shapes.".into()); }
        let result = { let refs: Vec<&putiacnc_core::model::LoopData> = idx.iter().map(|&i| &self.project.data.loops[i]).collect(); shapes::combine(&refs, op, swap) };
        if result.is_empty() { return Some("Nothing is left: the shapes do not overlap that way.".into()); }
        self.hist.record(&self.project.data);
        if !keep {
            for &i in idx.iter().rev() { self.project.data.loops.remove(i); }
            shapes::tabs_after_delete(&mut self.project.data.ops, &idx);
        }
        let start = self.project.data.loops.len();
        self.project.data.loops.extend(result);
        self.sel = (start..self.project.data.loops.len()).collect();
        self.touch();
        if open > 0 { Some(format!("{open} open shape{} left out.", if open == 1 { " was" } else { "s were" })) } else { None }
    }

    /// True when a selected shape belongs to a locked imported drawing.
    pub fn sel_locked(&self) -> bool {
        let d = &self.project.data;
        self.sel.iter().filter_map(|&i| d.loops.get(i)).filter_map(|l| l.grp).any(|g| d.groups.iter().any(|x| x.id == g && x.locked))
    }

    /// True when this shape belongs to a locked imported drawing.
    pub fn loop_locked(&self, i: usize) -> bool {
        let d = &self.project.data;
        d.loops.get(i).and_then(|l| l.grp).is_some_and(|g| d.groups.iter().any(|x| x.id == g && x.locked))
    }

    pub fn nudge(&mut self, dx: f64, dy: f64) {
        if self.sel.is_empty() || self.sel_locked() { return; }
        self.rotate_mode = false;
        self.hist.record(&self.project.data);
        for &i in &self.sel { shapes::translate(&mut self.project.data.loops[i], dx, dy); }
        self.touch();
    }

    pub fn add_loop(&mut self, l: putiacnc_core::model::LoopData) {
        self.hist.record(&self.project.data);
        self.project.data.loops.push(l);
        self.sel.clear();
        // drawing a shape hands control back to the select tool
        self.mode = Mode::Select;
        self.poly.clear();
        self.sel.insert(self.project.data.loops.len() - 1);
        self.touch();
    }

    /// The object snap point near `p`, if object snap is on and one is close.
    pub fn object_snap(&self, p: [f64; 2]) -> Option<crate::osnap::Hit> {
        if !self.osnap_on { return None; }
        let st = &self.project.data.stock;
        crate::osnap::find(&self.project.data.loops, &self.snap_skip, (st.w, st.h), p, self.snap_tol_mm)
    }

    pub fn snap(&self, p: [f64; 2]) -> [f64; 2] {
        if let Some(h) = self.object_snap(p) { return h.p; }
        self.grid_snap(p)
    }

    pub fn grid_snap(&self, p: [f64; 2]) -> [f64; 2] {
        if self.snap_on && self.snap_mm > 1e-6 {
            [(p[0] / self.snap_mm).round() * self.snap_mm, (p[1] / self.snap_mm).round() * self.snap_mm]
        } else { p }
    }
}
