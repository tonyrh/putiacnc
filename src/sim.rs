//! Material-removal simulation on a height field: the stock top is z = 0, the bottom z = -thickness.
//! Each cutting move is stamped into the grid with the tool's radial profile, so the result shows
//! what the finished part looks like (flat pockets, ball and V-bit grooves, through cuts).
use crate::model::{ProjectData, Tool};
use crate::toolpath::{Move, ToolGeo};

/// Height of the cutter surface above its tip at radial distance `rho` (infinity outside the tool).
fn rise(g: &ToolGeo, rho: f64) -> f64 { g.rise(rho) }

pub struct Sim {
    pub nx: usize,
    pub ny: usize,
    pub cell: f64,
    /// Stock size in mm and thickness.
    pub w: f64,
    pub h: f64,
    pub t: f64,
    /// Surface height per cell (row-major, y up), between -t and 0.
    pub z: Vec<f32>,
    /// Number of moves applied so far.
    pub done: usize,
    tools: Vec<(f64, Vec<f32>)>, // two per operation (tool, second tool): radius, rise table with step = cell / 2
    pub tip: [f64; 3],
    /// Bumped whenever the surface changes (the GUI uploads it again when this moves).
    pub rev: u64,
    /// How far into the move at `done` the cutter has been stamped (0..1), see `advance_into`.
    part: f64,
    /// Per operation: its moves are left out of the replay.
    pub skip: Vec<bool>,
}

impl Sim {
    /// `max_cells` bounds the grid size (each side gets about sqrt of it at most).
    pub fn new(data: &ProjectData, max_cells: usize) -> Sim {
        let st = &data.stock;
        let area = (st.w * st.h).max(1.0);
        let cell = (area / max_cells.max(1000) as f64).sqrt().max(0.05);
        let nx = ((st.w / cell).ceil() as usize).max(2) + 1;
        let ny = ((st.h / cell).ceil() as usize).max(2) + 1;
        // two slots per operation: its tool and its second tool (inlays)
        let tools = data.ops.iter().flat_map(|o| [Some(o.p.tool.as_str()), o.p.tool2.as_deref()]).map(|id| {
            let t: Option<&Tool> = id.and_then(|id| data.tools.iter().find(|t| t.id == id));
            match t {
                Some(t) => {
                    let g = ToolGeo::new(t);
                    let step = cell / 2.0;
                    // The wall is a thin ramp of fixed width (1.5 cells) instead of a one-cell step: a cell
                    // beside the wall then stores where the edge really is inside it, so edges come out
                    // smooth while staying within ~0.1 mm of vertical at any depth.
                    let n = (g.r / step).ceil() as usize + 1;
                    let tab: Vec<f32> = (0..=n).map(|k| rise(&g, (k as f64 * step).min(g.r)) as f32).collect();
                    (g.r, tab)
                }
                None => (0.0, vec![0.0]),
            }
        }).collect();
        Sim { nx, ny, cell, w: st.w, h: st.h, t: st.t, z: vec![0.0; nx * ny], done: 0, tools, tip: [0.0, 0.0, 10.0], rev: 0, part: 0.0, skip: vec![] }
    }

    pub fn reset(&mut self) { self.z.iter_mut().for_each(|v| *v = 0.0); self.done = 0; self.part = 0.0; self.rev += 1; }

    fn stamp(&mut self, op: usize, x: f64, y: f64, z: f64) {
        let Some((r, tab)) = self.tools.get(op) else { return };
        let r0 = *tab.last().unwrap_or(&0.0) as f64;
        let wall = self.cell * 1.5;
        let (r, ext) = (*r, *r + wall);
        if r <= 0.0 { return; }
        let bottom = -self.t as f32;
        let (ix0, ix1) = ((((x - ext) / self.cell).floor().max(0.0)) as usize, (((x + ext) / self.cell).ceil().max(0.0) as usize).min(self.nx - 1));
        let (iy0, iy1) = ((((y - ext) / self.cell).floor().max(0.0)) as usize, (((y + ext) / self.cell).ceil().max(0.0) as usize).min(self.ny - 1));
        let step = self.cell / 2.0;
        for iy in iy0..=iy1 {
            let dy = iy as f64 * self.cell - y;
            for ix in ix0..=ix1 {
                let dx = ix as f64 * self.cell - x;
                let rho = (dx * dx + dy * dy).sqrt();
                if rho > ext { continue; }
                let v = if rho <= r {
                    let k = ((rho / step).round() as usize).min(tab.len() - 1);
                    (z + tab[k] as f64) as f32
                } else {
                    // thin wall ramp: climbs from the cut height to the stock top (0) over `wall`
                    let b = z + r0;
                    if b >= 0.0 { continue; }
                    (b * (1.0 - (rho - r) / wall)) as f32
                };
                let c = &mut self.z[iy * self.nx + ix];
                if v < *c { *c = v.max(bottom); }
            }
        }
    }

    /// Apply moves up to (not including) index `upto`; moving backwards restarts from the top.
    pub fn advance(&mut self, moves: &[Move], upto: usize) { self.advance_for(moves, upto, None); }

    /// Like `advance`, but gives up after `budget` so a window can keep answering; returns true once `upto` is reached.
    pub fn advance_for(&mut self, moves: &[Move], upto: usize, budget: Option<std::time::Duration>) -> bool {
        let upto = upto.min(moves.len());
        if upto < self.done { self.reset(); }
        let start = std::time::Instant::now();
        while self.done < upto {
            if let Some(b) = budget { if start.elapsed() >= b { return false; } }
            let m = &moves[self.done];
            self.stamp_move(m, 0.0, 1.0);
            self.tip = m.to;
            self.done += 1;
            self.part = 0.0;
            self.rev += 1;
        }
        if upto == 0 { self.tip = [0.0, 0.0, 10.0]; }
        true
    }

    /// Cuts the part of a move between fractions `f0` and `f1` of its length.
    /// Cuts a level stretch from `a` to `b` at height `z` in one sweep: every cell takes the cutter's height at its distance from the
    /// stretch. The same as stamping the cutter all along it, because the cutter (and its wall ramp) never gets lower further out.
    fn stamp_level(&mut self, slot: usize, a: [f64; 2], b: [f64; 2], z: f64) {
        let Some((r, tab)) = self.tools.get(slot) else { return };
        let r0 = *tab.last().unwrap_or(&0.0) as f64;
        let wall = self.cell * 1.5;
        let (r, ext) = (*r, *r + wall);
        if r <= 0.0 { return; }
        let bottom = -self.t as f32;
        let (x0, x1, y0, y1) = (a[0].min(b[0]) - ext, a[0].max(b[0]) + ext, a[1].min(b[1]) - ext, a[1].max(b[1]) + ext);
        let (ix0, ix1) = (((x0 / self.cell).floor().max(0.0)) as usize, ((x1 / self.cell).ceil().max(0.0) as usize).min(self.nx - 1));
        let (iy0, iy1) = (((y0 / self.cell).floor().max(0.0)) as usize, ((y1 / self.cell).ceil().max(0.0) as usize).min(self.ny - 1));
        let (ux, uy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = ux * ux + uy * uy;
        let step = self.cell / 2.0;
        for iy in iy0..=iy1 {
            let py = iy as f64 * self.cell;
            for ix in ix0..=ix1 {
                let px = ix as f64 * self.cell;
                let t = if l2 > 0.0 { (((px - a[0]) * ux + (py - a[1]) * uy) / l2).clamp(0.0, 1.0) } else { 0.0 };
                let rho = (px - a[0] - ux * t).hypot(py - a[1] - uy * t);
                if rho > ext { continue; }
                let v = if rho <= r {
                    let k = ((rho / step).round() as usize).min(tab.len() - 1);
                    (z + tab[k] as f64) as f32
                } else {
                    let b = z + r0;
                    if b >= 0.0 { continue; }
                    (b * (1.0 - (rho - r) / wall)) as f32
                };
                let c = &mut self.z[iy * self.nx + ix];
                if v < *c { *c = v.max(bottom); }
            }
        }
    }

    fn stamp_move(&mut self, m: &Move, f0: f64, f1: f64) {
        if !m.cut || f1 <= f0 || self.skip.get(m.op).copied().unwrap_or(false) { return; }
        let slot = m.op * 2 + m.alt as usize;
        // when the cutter never gets lower further out (almost every tool), the move is swept in level pieces: one piece for a level move,
        // pieces at most 0.01 mm apart in height for a ramp (each cut at its lower end, so a ramp is never left too high), and one piece at
        // the bottom for a plunge. Much faster than stamping the cutter every 0.7 cell, above all for big cutters.
        if self.tools.get(slot).is_some_and(|(_, tab)| tab.windows(2).all(|w| w[1] >= w[0])) {
            let at = |f: f64| [m.from[0] + (m.to[0] - m.from[0]) * f, m.from[1] + (m.to[1] - m.from[1]) * f, m.from[2] + (m.to[2] - m.from[2]) * f];
            let horiz = (m.to[0] - m.from[0]).hypot(m.to[1] - m.from[1]) * (f1 - f0);
            let dz = (m.to[2] - m.from[2]).abs() * (f1 - f0);
            let n = ((dz / 0.01).ceil().min((horiz / (self.cell * 0.7)).ceil()) as usize).max(1);
            for i in 0..n {
                let (a, b) = (at(f0 + (f1 - f0) * i as f64 / n as f64), at(f0 + (f1 - f0) * (i + 1) as f64 / n as f64));
                let z = a[2].min(b[2]);
                if z > 0.0 { continue; }
                self.stamp_level(slot, [a[0], a[1]], [b[0], b[1]], z);
            }
            return;
        }
        let d = ((m.to[0] - m.from[0]).powi(2) + (m.to[1] - m.from[1]).powi(2) + (m.to[2] - m.from[2]).powi(2)).sqrt() * (f1 - f0);
        let n = ((d / (self.cell * 0.7)).ceil() as usize).max(1);
        for i in 0..=n {
            let f = f0 + (f1 - f0) * i as f64 / n as f64;
            if i == 0 && f0 > 0.0 { continue; }
            // skip samples clearly above the stock
            let z = m.from[2] + (m.to[2] - m.from[2]) * f;
            if z > 0.0 { continue; }
            self.stamp(m.op * 2 + m.alt as usize, m.from[0] + (m.to[0] - m.from[0]) * f, m.from[1] + (m.to[1] - m.from[1]) * f, z);
        }
    }

    /// Cuts the move at `done` up to fraction `f` of its length, so the material goes away under the
    /// cutter instead of a whole move at a time. Call after `advance` has reached `done`.
    /// Going back within the move restarts the replay (the caller's next `advance` rebuilds the surface).
    pub fn advance_into(&mut self, moves: &[Move], f: f64) {
        let Some(m) = moves.get(self.done) else { return };
        let f = f.clamp(0.0, 1.0);
        if f < self.part - 1e-9 { self.reset(); return; }
        if f > self.part + 1e-9 {
            self.stamp_move(m, self.part, f);
            self.part = f;
            self.rev += 1;
        }
    }

    /// Radius of the cutter in this tool slot (two per operation), 0 if there is none.
    pub fn tool_radius(&self, slot: usize) -> f64 { self.tools.get(slot).map_or(0.0, |t| t.0) }

    /// How far (mm) the stock stands inside the cutter when its tip is at (x, y, z): the largest height of the
    /// surface above the cutter's own surface, over the cutter's footprint. The outer 1.5 cells and 0.05 mm are
    /// left out, because the walls of earlier cuts are a thin slope that reaches that far.
    pub fn inside_cutter(&self, slot: usize, x: f64, y: f64, z: f64) -> f64 {
        let Some((r, tab)) = self.tools.get(slot) else { return 0.0 };
        // below the stock's bottom there is the table, not stock: the surface cannot be lower than that
        let z = z.max(-self.t);
        let r_in = (*r - self.cell * 1.5 - 0.05).max(0.0);
        let step = self.cell / 2.0;
        let mut worst = 0.0_f64;
        self.for_cells(x, y, r_in, |rho, s| {
            let k = ((rho / step).round() as usize).min(tab.len() - 1);
            worst = worst.max(s as f64 - (z + tab[k] as f64));
        });
        worst
    }

    /// The largest height (mm) the surface has above `top` in the ring between radius `r_in` and `r_out` around (x, y).
    pub fn above_in_ring(&self, x: f64, y: f64, r_in: f64, r_out: f64, top: f64) -> f64 {
        let mut worst = f64::NEG_INFINITY;
        self.for_cells(x, y, r_out, |rho, s| { if rho > r_in { worst = worst.max(s as f64 - top); } });
        worst
    }

    /// Calls `f(distance, surface height)` for every grid point within `r` of (x, y).
    fn for_cells(&self, x: f64, y: f64, r: f64, mut f: impl FnMut(f64, f32)) {
        let (ix0, ix1) = ((((x - r) / self.cell).floor().max(0.0)) as usize, (((x + r) / self.cell).ceil().max(0.0) as usize).min(self.nx - 1));
        let (iy0, iy1) = ((((y - r) / self.cell).floor().max(0.0)) as usize, (((y + r) / self.cell).ceil().max(0.0) as usize).min(self.ny - 1));
        for iy in iy0..=iy1 {
            let dy = iy as f64 * self.cell - y;
            for ix in ix0..=ix1 {
                let dx = ix as f64 * self.cell - x;
                let rho = (dx * dx + dy * dy).sqrt();
                if rho <= r { f(rho, self.z[iy * self.nx + ix]); }
            }
        }
    }

    pub fn at(&self, ix: usize, iy: usize) -> f32 { self.z[iy * self.nx + ix] }
    pub fn through(&self, ix: usize, iy: usize) -> bool { self.at(ix, iy) <= -self.t as f32 + 1e-4 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::ShapeKind;
    use crate::{generate, shapes, Units};

    fn proj() -> ProjectData {
        let mut f = shapes::blank_project();
        let d = &mut f.data;
        d.stock.w = 40.0; d.stock.h = 30.0; d.stock.t = 10.0;
        d.ops.push(crate::model::Operation { id: 1, p: shapes::default_op_params(&d.tools[0].id) });
        d.ops[0].p.kind = "pocket".into();
        d.ops[0].p.depth = 3.0;
        d.oid = 1; d.act = 1;
        d.loops.push(shapes::new_rect([10.0, 10.0], [30.0, 20.0], 0.0, 1));
        f.data
    }

    #[test]
    fn pocket_is_cut_to_depth_only_inside() {
        let d = proj();
        let out = generate(&d, Units::Metric).unwrap();
        let mut s = Sim::new(&d, 40_000);
        s.advance(&out.moves, out.moves.len());
        let ix = (20.0 / s.cell) as usize; let iy = (15.0 / s.cell) as usize;
        assert!((s.at(ix, iy) + 3.0).abs() < 0.02, "floor {}", s.at(ix, iy));
        assert_eq!(s.at(1, 1), 0.0);
        assert_eq!(s.at(s.nx - 2, s.ny - 2), 0.0);
        assert!(!s.through(ix, iy));
    }

    #[test]
    fn scrubbing_back_restores_material() {
        let d = proj();
        let out = generate(&d, Units::Metric).unwrap();
        let mut s = Sim::new(&d, 40_000);
        s.advance(&out.moves, out.moves.len());
        s.advance(&out.moves, 0);
        assert!(s.z.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn vbit_profile_rises_with_radius() {
        let g = ToolGeo { r: 3.0, shape: ShapeKind::VBit, cone: true, tn: 1.0, z_full: 3.0, rt: 0.0, arc: 0.0, concave: false };
        assert_eq!(rise(&g, 0.0), 0.0);
        assert!((rise(&g, 2.0) - 2.0).abs() < 1e-9);
        assert!(rise(&g, 3.5).is_infinite());
    }

    #[test]
    fn a_move_is_cut_progressively() {
        let d = proj();
        let out = generate(&d, Units::Metric).unwrap();
        // the longest cutting move
        let k = (0..out.moves.len()).filter(|&i| out.moves[i].cut).max_by(|&a, &b| {
            let len = |m: &Move| ((m.to[0] - m.from[0]).powi(2) + (m.to[1] - m.from[1]).powi(2)).sqrt();
            len(&out.moves[a]).partial_cmp(&len(&out.moves[b])).unwrap()
        }).unwrap();
        let cut = |s: &Sim| s.z.iter().map(|&v| (-v * 100.0) as i64).sum::<i64>();
        let mut s = Sim::new(&d, 100_000);
        s.advance(&out.moves, k);
        let c0 = cut(&s);
        s.advance_into(&out.moves, 0.5);
        let c1 = cut(&s);
        s.advance_into(&out.moves, 1.0);
        let c2 = cut(&s);
        assert!(c0 < c1 && c1 < c2, "{c0} {c1} {c2}");
        // and the result matches cutting the whole move at once
        let mut t = Sim::new(&d, 100_000);
        t.advance(&out.moves, k + 1);
        assert_eq!(cut(&t), c2);
        // going back within the move restarts the replay
        s.advance_into(&out.moves, 0.2);
        assert_eq!(s.done, 0);
    }

    #[test]
    fn inlay_pocket_and_plug_have_matching_slopes() {
        let mut f = shapes::blank_project();
        let d = &mut f.data;
        d.stock.w = 80.0; d.stock.h = 40.0; d.stock.t = 12.0;
        let vbit = d.tools.iter().find(|t| t.shape == "vbit" && t.angle == 60.0).unwrap().id.clone();
        let flat = d.tools.iter().find(|t| t.shape == "flat" && t.dia < 3.5).unwrap().id.clone();
        for (id, kind) in [(1, "inlay"), (2, "plug")] {
            let mut p = shapes::default_op_params(&vbit);
            p.kind = kind.into();
            p.depth = 3.0;
            p.tool2 = Some(flat.clone());
            d.ops.push(crate::model::Operation { id, p });
        }
        d.oid = 2; d.act = 1;
        d.loops.push(shapes::new_rect([10.0, 10.0], [30.0, 30.0], 0.0, 1));
        d.loops.push(shapes::new_rect([50.0, 10.0], [70.0, 30.0], 0.0, 2));
        let out = generate(&f.data, Units::Metric).unwrap();
        assert!(out.moves.iter().any(|m| m.alt), "the flat end mill takes part");
        let mut s = Sim::new(&f.data, 200_000);
        s.advance(&out.moves, out.moves.len());
        let h = |x: f64, y: f64| s.at((x / s.cell).round() as usize, (y / s.cell).round() as usize) as f64;
        let tn = (30.0_f64).to_radians().tan();
        // pocket: flat floor 3 mm deep, walls sloping at the V-bit's angle
        assert!((h(20.0, 20.0) + 3.0).abs() < 0.05, "floor {}", h(20.0, 20.0));
        assert!((h(11.0, 20.0) + 1.0 / tn).abs() < 0.2, "wall {} vs {}", h(11.0, 20.0), -1.0 / tn);
        assert!(h(5.0, 20.0).abs() < 1e-6, "outside the pocket is untouched");
        // plug: top of the boss is the original surface, walls slope the same way, 0.05 mm smaller sideways
        assert!(h(60.0, 20.0).abs() < 0.05, "boss top {}", h(60.0, 20.0));
        let want = (1.0 - 0.05) / tn - 3.0;
        assert!((h(51.0, 20.0) - want).abs() < 0.2, "boss wall {} vs {}", h(51.0, 20.0), want);
        // the plug is cut free of the stock around its base
        assert!(s.through((49.0 / s.cell).round() as usize, (20.0 / s.cell).round() as usize));
        assert!(h(40.0, 20.0).abs() < 1e-6, "between the two is untouched");
    }

    /// A V-carve of a slot `w` mm wide with the given tool; returns the depth reached on its centre line.
    fn carve_depth(tool: crate::model::Tool, w: f64) -> f64 {
        let mut f = shapes::blank_project();
        let d = &mut f.data;
        d.stock.w = 40.0; d.stock.h = 30.0; d.stock.t = 10.0;
        let id = tool.id.clone();
        d.tools.push(tool);
        let mut p = shapes::default_op_params(&id);
        p.kind = "vcarve".into();
        p.depth = 5.0;
        d.ops.push(crate::model::Operation { id: 1, p });
        d.oid = 1; d.act = 1;
        d.loops.push(shapes::new_rect([5.0, 15.0 - w / 2.0], [35.0, 15.0 + w / 2.0], 0.0, 1));
        let out = generate(&f.data, Units::Metric).unwrap();
        let mut s = Sim::new(&f.data, 400_000);
        s.advance(&out.moves, out.moves.len());
        -(s.at((20.0 / s.cell).round() as usize, (15.0 / s.cell).round() as usize) as f64)
    }

    fn tool(shape: &str, angle: f64, dia: f64, tip: f64) -> crate::model::Tool {
        let mut t = shapes::seed_tools().remove(0);
        t.id = "curved".into(); t.shape = shape.into(); t.angle = angle; t.dia = dia; t.tip = tip;
        if shape == "arc" { t.arc = angle; } // for arc bits `angle` carries the arc radius in these tests
        t
    }

    #[test]
    fn a_tip_radius_makes_vcarve_troughs_rounder_and_shallower() {
        // slot 1 mm wide, 60 degree V-bit: sharp tip reaches 0.5 / tan(30) = 0.866 mm
        let sharp = carve_depth(tool("vbit", 60.0, 6.0, 0.0), 1.0);
        assert!((sharp - 0.866).abs() < 0.06, "sharp {sharp}");
        // with a 0.5 mm tip radius the same width is only 0.366 mm deep
        let round = carve_depth(tool("vbit", 60.0, 6.0, 0.5), 1.0);
        assert!((round - 0.366).abs() < 0.06, "rounded {round}");
        // a ball nose carves a round trough: 4 mm slot with a 6 mm ball: 3 - sqrt(9 - 4) = 0.764
        let ball = carve_depth(tool("ball", 60.0, 6.0, 0.0), 4.0);
        assert!((ball - 0.764).abs() < 0.06, "ball {ball}");
    }

    #[test]
    fn arc_profile_bits_carve_round_and_flared_troughs() {
        // 6 mm bit, 10 mm arc, convex: a 4 mm slot is only 10 - sqrt(100 - 4) = 0.202 mm deep
        let convex = carve_depth(tool("arc", 10.0, 6.0, 0.0), 4.0);
        assert!((convex - 0.202).abs() < 0.06, "convex {convex}");
        // concave with the arc as tight as the bit allows (3 mm): at 2 mm out the edge is 3 - (3 - sqrt(9 - 1)) = 2.83 mm up
        let mut t = tool("arc", 3.0, 6.0, 0.0);
        t.concave = true;
        let concave = carve_depth(t, 4.0);
        assert!((concave - 2.828).abs() < 0.08, "concave {concave}");
    }

    #[test]
    fn a_tight_convex_arc_leaves_a_flat_bottom_like_a_bull_nose() {
        // 12 mm bit with a 2 mm corner arc: flat bottom 8 mm across, quarter round beyond that
        let g = ToolGeo::new(&tool("arc", 2.0, 12.0, 0.0));
        assert!(g.rise(3.9).abs() < 1e-9 && (g.rise(6.0) - 2.0).abs() < 1e-9 && (g.z_full - 2.0).abs() < 1e-9);
        assert!((g.rise(5.0) - (2.0 - 3.0f64.sqrt())).abs() < 1e-9);
    }

    #[test]
    fn tool_profile_and_its_inverse_agree() {
        let mut cc = tool("arc", 5.0, 6.0, 0.0);
        cc.concave = true;
        for t in [tool("arc", 10.0, 6.0, 0.0), tool("arc", 1.0, 6.0, 0.0), cc, tool("vbit", 60.0, 6.0, 0.0), tool("vbit", 60.0, 6.0, 0.8), tool("taper", 12.0, 3.0, 0.4), tool("ball", 60.0, 6.0, 0.0)] {
            let g = ToolGeo::new(&t);
            for k in 1..20 {
                let rho = g.r * k as f64 / 20.0;
                let z = g.rise(rho);
                if z < 1e-9 { continue; } // a flat bottom has no single radius for height 0
                assert!((g.r_at(z) - rho).abs() < 1e-6, "{} {rho}: {z} -> {}", t.shape, g.r_at(z));
            }
            assert!(g.z_full > 0.0 && (g.r_at(g.z_full) - g.r).abs() < 1e-6);
        }
    }
}

#[cfg(test)]
mod skip_tests {
    use super::*;
    use crate::{generate, shapes, Units};

    #[test]
    fn a_skipped_operation_leaves_the_stock_uncut() {
        let mut f = shapes::blank_project();
        let d = &mut f.data;
        d.stock.w = 60.0; d.stock.h = 40.0; d.stock.t = 12.0;
        d.ops.push(crate::model::Operation { id: 1, p: shapes::default_op_params(&d.tools[0].id) });
        d.ops[0].p.kind = "pocket".into();
        d.ops[0].p.depth = 3.0;
        d.oid = 1; d.act = 1;
        d.loops.push(shapes::new_rect([15.0, 10.0], [45.0, 30.0], 0.0, 1));
        let o = generate(d, Units::Metric).unwrap();
        let mut a = Sim::new(d, 20000);
        a.advance(&o.moves, o.moves.len());
        assert!(a.z.iter().any(|&z| z < -1.0), "the pocket is cut");
        let mut b = Sim::new(d, 20000);
        b.skip = vec![true];
        b.advance(&o.moves, o.moves.len());
        assert!(b.z.iter().all(|&z| z == 0.0), "a hidden operation cuts nothing");
    }
}
