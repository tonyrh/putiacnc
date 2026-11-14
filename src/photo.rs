//! Photo V-carve: a picture etched into a flat surface with a V-bit. The bit runs along parallel lines and goes deeper where the
//! picture is darker, so the grooves are wider there and the wood left between them is narrower: the eye blends them into tones.
//! (Unlike a 3D relief the surface stays flat; the shading comes from the width of the grooves, not from heights.)
//!
//! The picture is the project's height map read as a tone map: 1 is white (no groove), 0 is black (the full depth).

use crate::model::Relief;

/// The tone of the picture (0 black .. 1 white) at any point, by bilinear interpolation; outside the picture it is white.
pub struct Tones {
    t: Vec<f32>,
    nx: usize,
    ny: usize,
    g: f64,
    x0: f64,
    y0: f64,
}

impl Tones {
    pub fn new(rel: &Relief) -> Tones { Tones { t: rel.heights(), nx: rel.nx, ny: rel.ny, g: rel.res * rel.scale, x0: rel.x, y0: rel.y } }

    pub fn at(&self, x: f64, y: f64) -> f32 {
        let (u, v) = ((x - self.x0) / self.g - 0.5, (y - self.y0) / self.g - 0.5);
        if u < -1.0 || v < -1.0 || u > self.nx as f64 || v > self.ny as f64 { return 1.0; }
        let (u, v) = (u.clamp(0.0, (self.nx - 1) as f64), v.clamp(0.0, (self.ny - 1) as f64));
        let (i, j) = (u.floor() as usize, v.floor() as usize);
        let (i1, j1) = ((i + 1).min(self.nx - 1), (j + 1).min(self.ny - 1));
        let (fu, fv) = ((u - i as f64) as f32, (v - j as f64) as f32);
        let p = |a: usize, b: usize| self.t[b * self.nx + a];
        let top = p(i, j) * (1.0 - fu) + p(i1, j) * fu;
        let bot = p(i, j1) * (1.0 - fu) + p(i1, j1) * fu;
        top * (1.0 - fv) + bot * fv
    }

    pub fn size(&self) -> (f64, f64) { (self.nx as f64 * self.g, self.ny as f64 * self.g) }
}

/// Grooves shallower than this (mm) are not cut: the bit lifts over the light parts of the picture.
pub const MIN_DEPTH: f64 = 0.03;

/// The lines of one direction (`along_x`: along X, otherwise along Y). Every line is a list of runs; a run is a groove of
/// varying depth, as points (x, y, z) with z below the surface (negative). `depth`: the depth of black; `spacing`: the distance between lines.
pub fn lines(tones: &Tones, depth: f64, spacing: f64, along_x: bool, tol: f32) -> Vec<Vec<Vec<[f64; 3]>>> {
    let (w, h) = tones.size();
    let (across, along) = if along_x { (h, w) } else { (w, h) };
    let n = ((across / spacing).ceil() as usize).max(1);
    // the lines are spread evenly over the picture, the first and the last half a spacing from its edges
    let step = across / n as f64;
    let ds = (spacing / 4.0).clamp(0.05, 0.3);
    let m = ((along / ds).ceil() as usize).max(1);
    let (o_across, o_along) = if along_x { (tones.y0, tones.x0) } else { (tones.x0, tones.y0) };
    let mut out = Vec::with_capacity(n);
    for li in 0..n {
        let c = o_across + (li as f64 + 0.5) * step;
        // the depth along the line, from the tone averaged across the strip the line stands for
        let prof: Vec<f64> = (0..=m).map(|k| {
            let s = o_along + (k as f64 / m as f64) * along;
            let mut dark = 0.0;
            for q in [-0.4, -0.2, 0.0, 0.2, 0.4] {
                let a = c + q * step;
                let t = if along_x { tones.at(s, a) } else { tones.at(a, s) };
                dark += 1.0 - t as f64;
            }
            depth * dark / 5.0
        }).collect();
        let at = |k: usize, d: f64| -> [f64; 3] {
            let s = o_along + (k as f64 / m as f64) * along;
            if along_x { [s, c, -d] } else { [c, s, -d] }
        };
        let mut runs: Vec<Vec<[f64; 3]>> = Vec::new();
        let mut k = 0;
        while k <= m {
            if prof[k] < MIN_DEPTH { k += 1; continue; }
            let a = k;
            while k <= m && prof[k] >= MIN_DEPTH { k += 1; }
            let seg: Vec<(f64, f32)> = (a..k).map(|q| (q as f64 * ds, prof[q] as f32)).collect();
            let keep = crate::relief::simplify(&seg, tol);
            let run: Vec<[f64; 3]> = keep.iter().map(|&q| at(a + q, prof[a + q])).collect();
            if run.len() >= 2 { runs.push(run); }
        }
        // a zigzag: every other line is cut the other way
        if li % 2 == 1 { runs.reverse(); for r in runs.iter_mut() { r.reverse(); } }
        out.push(runs);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture whose left half is white and whose right half is black.
    fn half_and_half() -> Relief {
        let (nx, ny) = (40usize, 20usize);
        let mut b = Vec::new();
        for _ in 0..ny { for i in 0..nx { b.extend((if i < nx / 2 { 65535u16 } else { 0u16 }).to_le_bytes()); } }
        Relief { name: "t".into(), nx, ny, res: 0.5, zmax: 3.0, hm: crate::relief::b64_encode(&b), x: 10.0, y: 5.0, scale: 1.0 }
    }

    #[test]
    fn black_is_cut_at_full_depth_and_white_is_left_alone() {
        let r = half_and_half();
        let t = Tones::new(&r);
        let ls = lines(&t, 1.2, 1.0, true, 0.01);
        assert_eq!(ls.len(), 10, "20 mm high at 1 mm apart");
        for runs in &ls {
            assert_eq!(runs.len(), 1, "one groove: the white half is not cut");
            let run = &runs[0];
            let xs: Vec<f64> = run.iter().map(|p| p[0]).collect();
            let (lo, hi) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max));
            assert!(lo > 19.5 && hi <= 20.0 + 10.0 + 1e-6, "{lo} {hi}");
            assert!(run.iter().all(|p| (p[2] + 1.2).abs() < 0.1 || p[2] > -1.2), "depth up to 1.2");
            let deepest = run.iter().map(|p| p[2]).fold(0.0, f64::min);
            assert!((deepest + 1.2).abs() < 1e-6, "{deepest}");
        }
    }

    #[test]
    fn the_lines_follow_the_other_axis_too_and_zigzag() {
        let r = half_and_half();
        let t = Tones::new(&r);
        let ls = lines(&t, 1.0, 2.0, false, 0.01);
        // along Y: 20 mm wide at 2 mm apart; the first ten are on the white half, which is not cut
        assert_eq!(ls.len(), 10);
        assert!(ls[..5].iter().all(|l| l.is_empty()));
        let (a, b) = (&ls[5][0], &ls[6][0]);
        assert!(a[0][1] > a[a.len() - 1][1], "line 5 comes down");
        assert!(b[0][1] < b[b.len() - 1][1], "line 6 goes up");
    }

    #[test]
    fn a_grey_gets_half_the_depth() {
        let mut r = half_and_half();
        let mut b = Vec::new();
        for _ in 0..r.ny { for _ in 0..r.nx { b.extend(32768u16.to_le_bytes()); } }
        r.hm = crate::relief::b64_encode(&b);
        let t = Tones::new(&r);
        let ls = lines(&t, 2.0, 1.0, true, 0.01);
        let z = ls[3][0][0][2];
        assert!((z + 1.0).abs() < 0.01, "{z}");
    }
}
