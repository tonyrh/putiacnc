//! Generating the toolpath in the background and running the checks.
use super::*;

impl App {
    /// True while the toolpath on hand does not match the drawing yet (a change waits, or the new toolpath is being worked out).
    pub fn gen_stale(&self) -> bool { self.gen_pending || self.gen_rx.is_some() }

    /// Starts working out the toolpath on a worker thread, so the window keeps answering while a big job is generated.
    /// Does nothing while one is already running: the change is picked up as soon as it finishes.
    pub fn start_generation(&mut self) {
        if self.gen_rx.is_some() { return; }
        self.gen_pending = false;
        let units = if self.imperial { Units::Imperial } else { Units::Metric };
        let data = self.project.data.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let t = std::time::Instant::now();
            let r = putiacnc_core::pieces::generate_parts(&data, units);
            let _ = tx.send((r, t.elapsed().as_secs_f64() * 1000.0));
        });
        self.gen_rx = Some(rx);
        self.gen_started = Some(std::time::Instant::now());
    }

    /// Picks up a finished toolpath; true while one is still being worked out.
    pub fn poll_generation(&mut self) -> bool {
        let Some(rx) = &self.gen_rx else { return false };
        match rx.try_recv() {
            Ok((r, ms)) => { self.gen_rx = None; self.gen_started = None; self.apply_generated(r, ms); false }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // the worker stopped without an answer (a bug in the generator): say so instead of waiting for ever
                self.gen_rx = None; self.gen_started = None;
                self.apply_generated(Err("The toolpath could not be generated (internal error). Please report it with the project file.".into()), 0.0);
                false
            }
        }
    }

    /// Makes sure the toolpath matches the drawing right now, waiting for or redoing the generation if needed. Saving and exporting call
    /// it, so the file never carries a toolpath of an earlier version of the drawing.
    pub fn finish_generation(&mut self) {
        if let Some(rx) = self.gen_rx.take() {
            self.gen_started = None;
            match rx.recv() {
                Ok((r, ms)) => self.apply_generated(r, ms),
                Err(_) => self.apply_generated(Err("The toolpath could not be generated (internal error). Please report it with the project file.".into()), 0.0),
            }
        }
        if self.gen_pending { self.regenerate(); }
    }

    /// Works out the toolpath here and now (tests, and saving or exporting right after a change).
    pub fn regenerate(&mut self) {
        self.gen_pending = false;
        let units = if self.imperial { Units::Imperial } else { Units::Metric };
        let t = std::time::Instant::now();
        let r = putiacnc_core::pieces::generate_parts(&self.project.data, units);
        self.apply_generated(r, t.elapsed().as_secs_f64() * 1000.0);
    }

    pub(super) fn apply_generated(&mut self, r: Result<Vec<putiacnc_core::pieces::PartOutput>, String>, ms: f64) {
        self.out_ver += 1;
        self.pieces.clear();
        match r {
            Ok(parts) => {
                let mut parts = parts.into_iter();
                self.output = parts.next().map(|p| p.out);
                self.pieces = parts.filter_map(|p| p.piece.map(|job| PiecePart { job, output: p.out })).collect();
                self.gen_error = None;
            }
            Err(e) => { self.output = None; self.gen_error = Some(e); }
        }
        self.gen_ms = ms;
        // the warnings are worked out beside the UI: they replay the whole program
        self.checks.clear(); self.check_parts.clear(); self.check_sel = None;
        self.checks_rx = None;
        // a newer change is already waiting: its toolpath gets the checks, not this one
        if self.gen_pending { return; }
        if let Some(o) = self.output.clone() {
            let (tx, rx) = std::sync::mpsc::channel();
            let (data, ver) = (putiacnc_core::pieces::split(&self.project.data).0, self.out_ver);
            // each plug piece is checked on its own, in its own coordinates
            let pieces: Vec<(putiacnc_core::model::ProjectData, putiacnc_core::toolpath::Output)> = self.pieces.iter().map(|p| (p.job.data.clone(), p.output.clone())).collect();
            std::thread::spawn(move || {
                let mut all: Vec<(usize, putiacnc_core::checks::Finding)> = putiacnc_core::checks::run(&data, &o).into_iter().map(|f| (0, f)).collect();
                if std::env::var_os("PUTIACNC_DEBUG_CHECKS").is_some() { eprintln!("checks: {} findings, tool0 flute {:?}", all.len(), data.tools.first().map(|t| (t.flute, t.shank))); }
                for (i, (d, out)) in pieces.iter().enumerate() { all.extend(putiacnc_core::checks::run(d, out).into_iter().map(|f| (i + 1, f))); }
                let _ = tx.send((ver, all));
            });
            self.checks_rx = Some(rx);
        }
    }

    /// Pick up finished warnings; true while they are still being worked out.
    pub fn poll_checks(&mut self) -> bool {
        let Some(rx) = &self.checks_rx else { return false };
        match rx.try_recv() {
            Ok((v, f)) => {
                if v == self.out_ver {
                    let mut hits: Vec<(usize, Vec<usize>)> = vec![];
                    for x in f.iter().filter(|(p, x)| *p == 0 && x.kind == putiacnc_core::checks::Kind::Clamp).map(|(_, x)| x) {
                        for &c in &x.clamps { match hits.iter_mut().find(|h| h.0 == c) { Some(h) => h.1.extend(x.op), None => hits.push((c, x.op.into_iter().collect())) } }
                    }
                    self.clamp_hits = hits;
                    self.check_parts = f.iter().map(|(p, _)| *p).collect();
                    self.checks = f.into_iter().map(|(_, x)| x).collect();
                    self.checks_ver = v;
                }
                self.checks_rx = None;
                // testing aid: press "Show in 3D" on this warning
                if let Some(i) = std::env::var("PUTIACNC_CHECK_SHOW").ok().and_then(|s| s.parse::<usize>().ok()) { if i < self.checks.len() { crate::checkui::show_in_3d(self, i); } }
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => { self.checks_rx = None; false }
        }
    }

}
