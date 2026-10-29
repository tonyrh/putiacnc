//! Undo and redo: snapshots of the project, each tagged with the part of it the edit touched, so the toolbar can step
//! back through drawing changes only while the Edit menu steps back through everything.
use crate::model::ProjectData;

/// Which part of the project an edit touched.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind { Drawing, Project, Both }

/// What an undo or redo command works on: the drawing (the shapes) only, or every change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope { Drawing, All }

impl Kind {
    fn matches(self, s: Scope) -> bool { s == Scope::All || self != Kind::Project }
}

pub struct Entry {
    /// The project as it was before the edit (undo) or after it (redo).
    pub snap: ProjectData,
    /// Worked out once the edit is finished; `None` while it may still be going on.
    pub kind: Option<Kind>,
}

fn rest_eq(a: &ProjectData, b: &ProjectData) -> bool {
    a.stock == b.stock && a.job == b.job && a.machine == b.machine && a.ops == b.ops && a.tools == b.tools
        && a.clamps == b.clamps && a.relief == b.relief && a.notes == b.notes && a.act == b.act && a.oid == b.oid
}

/// Undo/redo as whole-project snapshots (projects are small, this is simple and robust). Each edit is
/// classified as a change to the drawing, to the rest of the project, or both, so that the toolbar can
/// undo drawing edits only while the Edit menu and Ctrl+Z undo every change.
#[derive(Default)]
pub struct History {
    pub undo: Vec<Entry>,
    pub redo: Vec<Entry>,
    /// True while a continuous edit (a drag or a text edit) has already been recorded.
    pub open: bool,
}

impl History {
    pub fn record(&mut self, d: &ProjectData) { self.push(d.clone()); }
    /// Record a snapshot taken before an edit.
    pub fn push(&mut self, before: ProjectData) {
        self.undo.push(Entry { snap: before, kind: None });
        if self.undo.len() > 60 { self.undo.remove(0); }
    }
    /// Record once per continuous edit gesture.
    pub fn record_once(&mut self, d: &ProjectData) {
        if !self.open { self.record(d); self.open = true; }
    }

    /// Work out what each recorded edit changed by comparing it with the state after it. Edits that
    /// changed nothing are dropped, and a new edit forgets the redo steps of the same scope.
    pub fn classify(&mut self, cur: &ProjectData) {
        if self.undo.iter().all(|e| e.kind.is_some()) { return; }
        let n = self.undo.len();
        let mut drop = vec![];
        for i in 0..n {
            if self.undo[i].kind.is_some() { continue; }
            let after = if i + 1 < n { &self.undo[i + 1].snap } else { cur };
            let before = &self.undo[i].snap;
            let (d, p) = (before.loops != after.loops || before.groups != after.groups, !rest_eq(before, after));
            match (d, p) {
                (false, false) => drop.push(i),
                (true, false) => self.undo[i].kind = Some(Kind::Drawing),
                (false, true) => self.undo[i].kind = Some(Kind::Project),
                (true, true) => self.undo[i].kind = Some(Kind::Both),
            }
            if let Some(k) = self.undo[i].kind {
                self.redo.retain(|r| match k { Kind::Both => false, Kind::Drawing => r.kind == Some(Kind::Project), Kind::Project => r.kind == Some(Kind::Drawing) });
            }
        }
        for i in drop.into_iter().rev() { self.undo.remove(i); }
    }

    /// One undo (`undo`) or redo step of the given scope on `cur`: puts back the part of the project that the step's edit
    /// touched and records the state it replaced on the opposite list. False when there was nothing to step to.
    pub fn step(&mut self, cur: &mut ProjectData, s: Scope, undo: bool) -> bool {
        self.classify(cur);
        let list = if undo { &mut self.undo } else { &mut self.redo };
        let Some(i) = Self::find(list, s) else { return false };
        let e = list.remove(i);
        let mut kind = e.kind.unwrap_or(Kind::Both);
        // the toolbar leaves the stock, operations and the rest alone: of a step that changed both, it takes back the drawing half, and
        // the other half stays in the list for the Edit menu (taking the whole step would put a stale project back at the next undo)
        if s == Scope::Drawing && kind == Kind::Both {
            list.insert(i, Entry { snap: e.snap.clone(), kind: Some(Kind::Project) });
            kind = Kind::Drawing;
        }
        let before = cur.clone();
        restore(cur, e.snap, kind);
        let other = if undo { &mut self.redo } else { &mut self.undo };
        other.push(Entry { snap: before, kind: Some(kind) });
        true
    }

    pub fn find(list: &[Entry], s: Scope) -> Option<usize> { list.iter().rposition(|e| e.kind.is_none_or(|k| k.matches(s))) }
    pub fn can_undo(&self, s: Scope) -> bool { Self::find(&self.undo, s).is_some() }
    pub fn can_redo(&self, s: Scope) -> bool { Self::find(&self.redo, s).is_some() }
}

/// Put back the part of `snap` that an edit of this kind changed.
pub fn restore(cur: &mut ProjectData, mut snap: ProjectData, kind: Kind) {
    match kind {
        Kind::Both => *cur = snap,
        Kind::Drawing => { cur.loops = snap.loops; cur.groups = snap.groups; }
        Kind::Project => { snap.loops = std::mem::take(&mut cur.loops); snap.groups = std::mem::take(&mut cur.groups); *cur = snap; }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes;

    fn project_with_a_circle() -> ProjectData {
        let mut d = shapes::blank_project().data;
        d.loops.clear();
        d.loops.push(shapes::new_circle([20.0, 20.0], 5.0, 0));
        d
    }

    #[test]
    fn the_drawing_scope_skips_project_changes_and_the_all_scope_does_not() {
        let mut d = project_with_a_circle();
        let mut h = History::default();
        h.record(&d);
        d.loops.push(shapes::new_circle([50.0, 20.0], 5.0, 0)); // a drawing edit
        h.record(&d);
        d.stock.w += 10.0; // then a project edit
        let w = d.stock.w;
        assert!(h.step(&mut d, Scope::Drawing, true));
        assert_eq!(d.loops.len(), 1, "the drawing edit is undone");
        assert_eq!(d.stock.w, w, "the stock change stays");
        assert!(h.step(&mut d, Scope::All, true));
        assert_eq!(d.stock.w, w - 10.0, "the all-scope step goes back through the stock change");
    }

    #[test]
    fn redo_puts_the_undone_edit_back() {
        let mut d = project_with_a_circle();
        let mut h = History::default();
        h.record(&d);
        d.loops.push(shapes::new_circle([50.0, 20.0], 5.0, 0));
        assert!(h.step(&mut d, Scope::All, true));
        assert_eq!(d.loops.len(), 1);
        assert!(h.can_redo(Scope::All));
        assert!(h.step(&mut d, Scope::All, false));
        assert_eq!(d.loops.len(), 2);
        assert!(!h.step(&mut d, Scope::All, false), "nothing left to redo");
    }

    #[test]
    fn an_edit_that_changed_nothing_leaves_no_step() {
        let mut d = project_with_a_circle();
        let mut h = History::default();
        h.record(&d);
        assert!(!h.step(&mut d, Scope::All, true));
        assert!(!h.can_undo(Scope::All));
    }

    #[test]
    fn only_the_last_sixty_steps_are_kept() {
        let mut d = project_with_a_circle();
        let mut h = History::default();
        for _ in 0..70 { h.record(&d); d.stock.w += 1.0; }
        assert_eq!(h.undo.len(), 60);
    }

    #[test]
    fn a_new_drawing_edit_forgets_the_redo_of_drawing_edits_but_keeps_project_redo() {
        let mut d = project_with_a_circle();
        let mut h = History::default();
        h.record(&d); d.stock.w += 5.0; // project edit
        assert!(h.step(&mut d, Scope::All, true)); // undone: a project redo exists
        h.record(&d); d.loops.push(shapes::new_circle([60.0, 20.0], 3.0, 0)); // a new drawing edit
        h.classify(&d);
        assert!(h.can_redo(Scope::All), "the stock redo survives a drawing edit");
        assert!(!h.can_redo(Scope::Drawing));
    }
}
