//! Previews for the tools that change the drawing: the tool is run for real on a scratch copy, what it would change is kept as a
//! "ghost" drawn over the canvas, and everything is put back. Applying then does exactly what the ghost showed.
use crate::app::App;
use putiacnc_core::model::LoopData;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

/// Drawings with more shapes than this get no preview: comparing before and after would make typing sluggish.
const MAX_SHAPES: usize = 2000;

#[derive(Default)]
pub struct Ghost {
    /// Which tool the ghost belongs to ("" when there is none).
    pub owner: &'static str,
    key: u64,
    /// What the tool would add or change.
    pub loops: Vec<LoopData>,
    /// Shapes of the drawing the tool would replace or remove: drawn faded while the ghost shows.
    pub hide: BTreeSet<usize>,
    /// What the tool says about the result (shapes too small, nothing to do...).
    pub note: Option<String>,
}

impl Ghost {
    pub fn clear(&mut self) { *self = Ghost::default(); }
    pub fn clear_if(&mut self, owner: &str) { if self.owner == owner { self.clear(); } }
    pub fn note_of(&self, owner: &str) -> Option<&str> { if self.owner == owner { self.note.as_deref() } else { None } }
}

impl App {
    /// Shows what `run` (a tool's real apply) would do. `params` are the tool's settings: the preview is redone when they, the
    /// selection or the selected shapes change, and left alone otherwise.
    pub fn preview(&mut self, owner: &'static str, params: &[u64], run: impl FnOnce(&mut App) -> Option<String>) {
        if !self.preview_on { self.ghost.clear_if(owner); return; }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        owner.hash(&mut h);
        params.hash(&mut h);
        self.project.data.loops.len().hash(&mut h);
        for &i in &self.sel {
            i.hash(&mut h);
            if let Some(l) = self.project.data.loops.get(i) { l.pts.hash(&mut h); l.c.hash(&mut h); }
        }
        let key = h.finish();
        if self.ghost.owner == owner && self.ghost.key == key { return; }
        if self.project.data.loops.len() > MAX_SHAPES {
            self.ghost = Ghost { owner, key, note: Some("Too many shapes for a preview.".into()), ..Default::default() };
            return;
        }
        // run the tool on a scratch copy and put everything it touches back afterwards
        let before = self.project.data.clone();
        let (sel, modified, gen_pending, note, error) = (self.sel.clone(), self.modified, self.gen_pending, self.note.clone(), self.error.clone());
        let hist = std::mem::take(&mut self.hist);
        let msg = run(self);
        let after = std::mem::replace(&mut self.project.data, before);
        self.hist = hist;
        self.sel = sel;
        self.modified = modified; self.gen_pending = gen_pending; self.note = note; self.error = error;
        // what changed: shapes only in the result are the ghost, shapes only in the drawing are faded
        let mut used = vec![false; after.loops.len()];
        let mut hide = BTreeSet::new();
        for (i, l) in self.project.data.loops.iter().enumerate() {
            match after.loops.iter().enumerate().position(|(k, a)| !used[k] && a == l) {
                Some(k) => used[k] = true,
                None => { hide.insert(i); }
            }
        }
        let loops: Vec<LoopData> = after.loops.into_iter().zip(used).filter(|(_, u)| !*u).map(|(l, _)| l).collect();
        self.ghost = Ghost { owner, key, loops, hide, note: msg };
    }
}

pub fn bits(v: f64) -> u64 { v.to_bits() }

#[cfg(test)]
mod tests {
    use super::*;
    use putiacnc_core::shapes;

    #[test]
    fn a_preview_shows_the_result_and_leaves_the_project_alone() {
        let mut a = App::new(None);
        a.project.data.loops.clear();
        a.project.data.loops.push(shapes::new_poly(&[[0.0, 0.0], [20.0, 0.0], [20.0, 10.0], [0.0, 10.0]], true, 0).unwrap());
        a.sel = [0].into_iter().collect();
        a.array.cols = 3.0;
        let (before, undo, modified) = (a.project.data.clone(), a.hist.undo.len(), a.modified);
        a.preview("array", &[3], |x| x.array_selection());
        assert_eq!(a.ghost.loops.len(), 2, "two more copies are shown");
        assert!(a.ghost.hide.is_empty(), "an array keeps its originals");
        assert!(a.project.data == before, "the drawing is untouched");
        assert_eq!(a.hist.undo.len(), undo, "nothing to undo");
        assert_eq!(a.modified, modified);
        assert_eq!(a.sel.len(), 1);
        // a tool that replaces the shape fades the original
        a.preview("corners", &[1], |x| x.corner_selection(shapes::CornerKind::Chamfer, 2.0));
        assert_eq!(a.ghost.hide.len(), 1);
        assert_eq!(a.ghost.loops.len(), 1);
        // switched off: no ghost
        a.preview_on = false;
        a.preview("corners", &[2], |x| x.corner_selection(shapes::CornerKind::Chamfer, 2.0));
        assert!(a.ghost.loops.is_empty());
    }
}
