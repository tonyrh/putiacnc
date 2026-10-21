//! Material-removal simulation on a height field: the stock top is z = 0, the bottom z = -thickness.
//! Each cutting move is stamped into the grid with the tool's radial profile, so the result shows
//! what the finished part looks like (flat pockets, ball and V-bit grooves, through cuts).
use crate::model::{ProjectData, Tool};
use crate::toolpath::{Move, ShapeKind, ToolGeo};

/// Height of the cutter surface above its tip at radial distance `rho` (infinity outside the tool).
fn rise(g: &ToolGeo, rho: f64) -> f64 {
    if rho > g.r { return f64::INFINITY; }
    match g.shape {
        ShapeKind::VBit | ShapeKind::Engrave | ShapeKind::Taper if g.cone => rho / g.tn,
        ShapeKind::Ball => g.r - (g.r * g.r - rho * rho).max(0.0).sqrt(),
        _ => 0.0,
    }
}

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
    tools: Vec<(f64, Vec<f32>)>, // per operation: radius, rise table with step = cell / 2
    pub tip: [f64; 3],
}

impl Sim {
    /// `max_cells` bounds the grid size (each side gets about sqrt of it at most).
    pub fn new(data: &ProjectData, max_cells: usize) -> Sim {
        let st = &data.stock;
        let area = (st.w * st.h).max(1.0);
        let cell = (area / max_cells.max(1000) as f64).sqrt().max(0.05);
        let nx = ((st.w / cell).ceil() as usize).max(2) + 1;
        let ny = ((st.h / cell).ceil() as usize).max(2) + 1;
        let tools = data.ops.iter().map(|o| {
            let t: Option<&Tool> = data.tools.iter().find(|t| t.id == o.p.tool);
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
        Sim { nx, ny, cell, w: st.w, h: st.h, t: st.t, z: vec![0.0; nx * ny], done: 0, tools, tip: [0.0, 0.0, 10.0] }
    }

    pub fn reset(&mut self) { self.z.iter_mut().for_each(|v| *v = 0.0); self.done = 0; }

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
    pub fn advance(&mut self, moves: &[Move], upto: usize) {
        let upto = upto.min(moves.len());
        if upto < self.done { self.reset(); }
        while self.done < upto {
            let m = &moves[self.done];
            if m.cut {
                let d = ((m.to[0] - m.from[0]).powi(2) + (m.to[1] - m.from[1]).powi(2) + (m.to[2] - m.from[2]).powi(2)).sqrt();
                let n = ((d / (self.cell * 0.7)).ceil() as usize).max(1);
                for i in 1..=n {
                    let f = i as f64 / n as f64;
                    // skip samples clearly above the stock
                    let z = m.from[2] + (m.to[2] - m.from[2]) * f;
                    if z > 0.0 { continue; }
                    self.stamp(m.op, m.from[0] + (m.to[0] - m.from[0]) * f, m.from[1] + (m.to[1] - m.from[1]) * f, z);
                }
            }
            self.tip = m.to;
            self.done += 1;
        }
        if upto == 0 { self.tip = [0.0, 0.0, 10.0]; }
    }

    pub fn at(&self, ix: usize, iy: usize) -> f32 { self.z[iy * self.nx + ix] }
    pub fn through(&self, ix: usize, iy: usize) -> bool { self.at(ix, iy) <= -self.t as f32 + 1e-4 }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let g = ToolGeo { r: 3.0, shape: ShapeKind::VBit, cone: true, tn: 1.0, z_full: 3.0 };
        assert_eq!(rise(&g, 0.0), 0.0);
        assert!((rise(&g, 2.0) - 2.0).abs() < 1e-9);
        assert!(rise(&g, 3.5).is_infinite());
    }
}
