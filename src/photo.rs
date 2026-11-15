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

/// A light gap shorter than this (mm) between two grooves is crossed at the surface instead of lifting the bit: it saves a lift and a plunge.
pub const JOIN_GAP: f64 = 1.0;

/// A groove shorter than this (mm) that is also shallow is a speck of the picture (grain, noise) and is not cut.
pub const SPECK_LEN: f64 = 0.4;

/// Which share of the darkness a pass of lines cuts. With lines in both directions the first direction cuts the picture up to
/// [`FIRST_SHARE`] of black (its grooves still leave wood between them) and the second direction crosses only the darker parts, so the
/// two together give the tone and a light picture is not cut twice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pass { Single, First, Second }

pub const FIRST_SHARE: f64 = 0.75;

/// The share of the width between two lines that a groove covers (0..1) for a darkness (0..1) in a pass; the depth follows from it.
pub fn coverage(dark: f64, pass: Pass) -> f64 {
    let d = dark.clamp(0.0, 1.0);
    match pass {
        Pass::Single => d,
        Pass::First => d.min(FIRST_SHARE),
        // what is still missing once the first direction covered its share: 1 - (1 - c1)(1 - c2) = d
        Pass::Second => if d <= FIRST_SHARE { 0.0 } else { 1.0 - (1.0 - d) / (1.0 - FIRST_SHARE) },
    }
}

/// The lines of one direction (`along_x`: along X, otherwise along Y). Every line is a list of runs; a run is a groove of
/// varying depth, as points (x, y, z) with z below the surface (negative). `depth`: the depth of black; `spacing`: the distance between lines.
pub fn lines(tones: &Tones, depth: f64, spacing: f64, along_x: bool, tol: f32, pass: Pass) -> Vec<Vec<Vec<[f64; 3]>>> {
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
            depth * coverage(dark / 5.0, pass)
        }).collect();
        let at = |k: usize, d: f64| -> [f64; 3] {
            let s = o_along + (k as f64 / m as f64) * along;
            if along_x { [s, c, -d] } else { [c, s, -d] }
        };
        let mut runs: Vec<Vec<[f64; 3]>> = Vec::new();
        let join = ((JOIN_GAP / ds).floor() as usize).max(1);
        let speck = (SPECK_LEN / ds).ceil() as usize;
        let mut k = 0;
        while k <= m {
            if prof[k] < MIN_DEPTH { k += 1; continue; }
            let a = k;
            // a run goes on over a short light gap: the next groove starts within `join` samples
            let mut end = k;
            while k <= m {
                if prof[k] >= MIN_DEPTH { end = k; k += 1; continue; }
                let next = (k..=m.min(k + join)).find(|&q| prof[q] >= MIN_DEPTH);
                match next { Some(q) => k = q, None => break }
            }
            let k_end = end + 1;
            k = k_end;
            // a short, shallow groove is a speck
            let top = prof[a..k_end].iter().cloned().fold(0.0, f64::max);
            if k_end - a < speck && top < 0.25 * depth { continue; }
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

/// What the etching looks like from above: for every pixel of a picture drawn at `ppm` pixels per mm, how much of it the grooves cover (0 wood .. 1 groove),
/// row 0 at the top. `groove_r(depth)` is the radius of the groove at a depth (a V-bit: depth times the tangent of half its angle); `both` crosses the
/// darker parts with lines in the other direction. The picture is as large as the relief; its lower-left corner is the corner of the image.
pub fn etch_coverage(rel: &Relief, depth: f64, spacing: f64, groove_r: &dyn Fn(f64) -> f64, both: bool, ppm: f64) -> (usize, usize, Vec<f32>) {
    let tones = Tones::new(&Relief { x: 0.0, y: 0.0, ..rel.clone() });
    let (w, h) = tones.size();
    let (nx, ny) = (((w * ppm).ceil() as usize).max(1), ((h * ppm).ceil() as usize).max(1));
    let mut ink = vec![0f32; nx * ny];
    let mut paint = |along_x: bool, pass: Pass| {
        for line in lines(&tones, depth, spacing, along_x, 0.01, pass) {
            for run in line {
                for seg in run.windows(2) {
                    let (a, b) = (seg[0], seg[1]);
                    // along the line one pixel column (or row) at a time, the groove as wide as it is at that depth
                    let (u0, u1, c0, c1) = if along_x { (a[0], b[0], a[1], b[1]) } else { (a[1], b[1], a[0], b[0]) };
                    let n = (((u1 - u0).abs() * ppm).ceil() as usize).max(1);
                    for q in 0..=n {
                        let f = q as f64 / n as f64;
                        let (u, c, z) = (u0 + (u1 - u0) * f, c0 + (c1 - c0) * f, a[2] + (b[2] - a[2]) * f);
                        let r = groove_r(-z);
                        let (lo, hi) = ((c - r) * ppm, (c + r) * ppm);
                        let up = (u * ppm).floor() as i64;
                        let span = if along_x { ny } else { nx } as i64;
                        let lim = if along_x { nx } else { ny } as i64;
                        if up < 0 || up >= lim { continue; }
                        let mut v = lo.floor().max(0.0) as i64;
                        while (v as f64) < hi && v < span {
                            let cov = ((v as f64 + 1.0).min(hi) - (v as f64).max(lo)).clamp(0.0, 1.0) as f32;
                            // pixel row 0 is the top of the picture (the largest Y)
                            let (px, py) = if along_x { (up as usize, ny - 1 - v as usize) } else { (v as usize, ny - 1 - up as usize) };
                            let k = py * nx + px;
                            ink[k] = (ink[k] + cov * (1.0 - ink[k])).min(1.0);
                            v += 1;
                        }
                    }
                }
            }
        }
    };
    if both { paint(true, Pass::First); paint(false, Pass::Second); } else { paint(true, Pass::Single); }
    (nx, ny, ink)
}

/// A coverage picture reduced to `dw` x `dh` pixels by the average of the pixels each new pixel takes in, so fine lines turn into tones.
pub fn reduce(nx: usize, ny: usize, ink: &[f32], dw: usize, dh: usize) -> Vec<f32> {
    let mut out = vec![0f32; dw * dh];
    for y in 0..dh {
        let (y0, y1) = (y * ny / dh, (((y + 1) * ny).div_ceil(dh)).min(ny).max(y * ny / dh + 1));
        for x in 0..dw {
            let (x0, x1) = (x * nx / dw, (((x + 1) * nx).div_ceil(dw)).min(nx).max(x * nx / dw + 1));
            let mut sum = 0.0;
            for yy in y0..y1 { for xx in x0..x1 { sum += ink[yy * nx + xx]; } }
            out[y * dw + x] = sum / ((y1 - y0) * (x1 - x0)) as f32;
        }
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
        let ls = lines(&t, 1.2, 1.0, true, 0.01, Pass::Single);
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
        let ls = lines(&t, 1.0, 2.0, false, 0.01, Pass::Single);
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
        let ls = lines(&t, 2.0, 1.0, true, 0.01, Pass::Single);
        let z = ls[3][0][0][2];
        assert!((z + 1.0).abs() < 0.01, "{z}");
    }

    /// A picture of dark and light stripes along the line: short light gaps are crossed, a speck is dropped.
    fn stripes(gap_mm: f64, dark_mm: f64, dark_tone: u16) -> Relief {
        let (nx, ny) = (100usize, 8usize); // 0.2 mm cells: 20 mm wide
        let mut b = Vec::new();
        for _ in 0..ny {
            for i in 0..nx {
                let x = i as f64 * 0.2;
                let period = gap_mm + dark_mm;
                let dark = (x % period) < dark_mm && x > 1.0 && x < 18.0;
                b.extend((if dark { dark_tone } else { 65535u16 }).to_le_bytes());
            }
        }
        Relief { name: "t".into(), nx, ny, res: 0.2, zmax: 1.0, hm: crate::relief::b64_encode(&b), x: 0.0, y: 0.0, scale: 1.0 }
    }

    #[test]
    fn a_short_light_gap_is_crossed_and_a_long_one_lifts() {
        let t = Tones::new(&stripes(0.5, 3.0, 0));
        let ls = lines(&t, 1.0, 0.4, true, 0.01, Pass::Single);
        assert!(ls.iter().all(|l| l.len() == 1), "gaps of 0.5 mm: one groove, got {:?}", ls.iter().map(|l| l.len()).collect::<Vec<_>>());
        let t = Tones::new(&stripes(2.0, 3.0, 0));
        let ls = lines(&t, 1.0, 0.4, true, 0.01, Pass::Single);
        assert!(ls.iter().all(|l| l.len() >= 3), "gaps of 2 mm lift the bit");
    }

    #[test]
    fn a_speck_is_not_cut() {
        // 0.2 mm dark dots in a light picture
        let t = Tones::new(&stripes(8.0, 0.2, 52000));
        let ls = lines(&t, 1.0, 0.4, true, 0.01, Pass::Single);
        assert!(ls.iter().all(|l| l.is_empty()), "short and shallow: not cut");
        // a short but black one is
        let t = Tones::new(&stripes(8.0, 0.4, 0));
        let ls = lines(&t, 1.0, 0.4, true, 0.01, Pass::Single);
        assert!(ls.iter().any(|l| !l.is_empty()));
    }

    #[test]
    fn two_directions_share_the_darkness_and_add_up() {
        for d in [0.0, 0.3, 0.75, 0.9, 1.0] {
            let c1 = coverage(d, Pass::First);
            let c2 = coverage(d, Pass::Second);
            let union = 1.0 - (1.0 - c1) * (1.0 - c2);
            assert!((union - d).abs() < 1e-9, "{d}: {union}");
        }
        assert_eq!(coverage(0.5, Pass::Second), 0.0, "a mid grey is cut in one direction only");
        assert!((coverage(1.0, Pass::Single) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_etch_picture_is_dark_where_the_picture_is_dark() {
        let r = half_and_half();
        let (nx, ny, ink) = etch_coverage(&r, 1.0, 0.5, &|d| d * 0.2679, false, 8.0);
        assert_eq!((nx, ny), (160, 80));
        let mean = |x0: usize, x1: usize| { let mut s = 0.0; let mut n = 0.0; for y in 0..ny { for x in x0..x1 { s += ink[y * nx + x] as f64; n += 1.0; } } s / n };
        assert!(mean(0, 70) < 0.01, "the white half is wood");
        assert!(mean(90, 160) > 0.9, "the black half is groove next to groove: {}", mean(90, 160));
    }

    #[test]
    fn reducing_averages_the_pixels() {
        let ink = vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        let r = reduce(4, 2, &ink, 2, 1);
        assert!((r[0] - 0.5).abs() < 1e-6 && (r[1] - 0.5).abs() < 1e-6);
        let one = reduce(4, 2, &ink, 1, 1);
        assert!((one[0] - 0.5).abs() < 1e-6);
    }
}
