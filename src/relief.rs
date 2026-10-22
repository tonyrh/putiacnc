//! 3D relief carving: an STL model reduced to a height map, and the planning of the roughing and
//! finishing passes over it. The G-code itself is written by `toolpath`.
use crate::model::Relief;

// ------------------------------------------------------------------ base64 (heights are stored in the project file)

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(d: &[u8]) -> String {
    let mut s = String::with_capacity(d.len() / 3 * 4 + 4);
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        s.push(B64[(n >> 18) as usize & 63] as char);
        s.push(B64[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    s
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut buf = 0u32;
    let mut bits = 0;
    for ch in s.bytes() {
        if ch == b'=' || ch == b'\n' || ch == b'\r' { continue; }
        let v = B64.iter().position(|&b| b == ch)? as u32;
        buf = buf << 6 | v;
        bits += 6;
        if bits >= 8 { bits -= 8; out.push((buf >> bits) as u8); buf &= (1 << bits) - 1; }
    }
    Some(out)
}

// ------------------------------------------------------------------ STL

type Tri = [[f32; 3]; 3];

fn parse_stl(b: &[u8]) -> Result<Vec<Tri>, String> {
    if b.len() >= 84 {
        let n = u32::from_le_bytes([b[80], b[81], b[82], b[83]]) as usize;
        if 84 + n * 50 == b.len() {
            let f = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
            return Ok((0..n).map(|i| {
                let o = 84 + i * 50 + 12;
                let v = |k: usize| [f(o + k * 12), f(o + k * 12 + 4), f(o + k * 12 + 8)];
                [v(0), v(1), v(2)]
            }).collect());
        }
    }
    let text = String::from_utf8_lossy(b);
    if !text.trim_start().starts_with("solid") { return Err("Not an STL file.".into()); }
    let mut vs: Vec<[f32; 3]> = Vec::new();
    for line in text.lines() {
        let mut w = line.split_whitespace();
        if w.next() == Some("vertex") {
            let n: Vec<f32> = w.filter_map(|x| x.parse().ok()).collect();
            if n.len() == 3 { vs.push([n[0], n[1], n[2]]); }
        }
    }
    if vs.len() < 3 { return Err("The STL file has no triangles.".into()); }
    Ok(vs.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect())
}

impl Relief {
    /// Reduce an STL model (binary or ASCII) to a height map seen from above, `res` mm per cell.
    /// The lowest point of the model is height 0.
    pub fn from_stl(bytes: &[u8], name: &str, res: f64) -> Result<Relief, String> {
        let tris = parse_stl(bytes)?;
        if tris.is_empty() { return Err("The STL file has no triangles.".into()); }
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for t in &tris { for v in t { for k in 0..3 {
            if !v[k].is_finite() { return Err("The STL file contains invalid numbers.".into()); }
            lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]);
        } } }
        let (w, h, zmax) = ((hi[0] - lo[0]) as f64, (hi[1] - lo[1]) as f64, (hi[2] - lo[2]) as f64);
        if w < 1e-3 || h < 1e-3 || zmax < 1e-3 { return Err("The model is flat in one direction; it needs width, depth and height.".into()); }
        let res = res.max(0.05);
        let (nx, ny) = (((w / res).ceil() as usize).max(2), ((h / res).ceil() as usize).max(2));
        if nx * ny > 6_000_000 { return Err("The height map would be too large; use a coarser resolution.".into()); }
        let mut g = vec![0f32; nx * ny];
        let (rx, ry) = (res as f32, res as f32);
        for t in &tris {
            let p: Vec<[f32; 3]> = t.iter().map(|v| [(v[0] - lo[0]) / rx, (v[1] - lo[1]) / ry, v[2] - lo[2]]).collect();
            let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
            if area.abs() < 1e-12 { continue; }
            let (x0, x1) = (p.iter().map(|v| v[0]).fold(f32::MAX, f32::min), p.iter().map(|v| v[0]).fold(f32::MIN, f32::max));
            let (y0, y1) = (p.iter().map(|v| v[1]).fold(f32::MAX, f32::min), p.iter().map(|v| v[1]).fold(f32::MIN, f32::max));
            let (i0, i1) = ((x0 - 0.5).floor().max(0.0) as usize, ((x1 - 0.5).ceil().max(0.0) as usize).min(nx - 1));
            let (j0, j1) = ((y0 - 0.5).floor().max(0.0) as usize, ((y1 - 0.5).ceil().max(0.0) as usize).min(ny - 1));
            for j in j0..=j1 { for i in i0..=i1 {
                let (cx, cy) = (i as f32 + 0.5, j as f32 + 0.5);
                let w0 = ((p[1][0] - cx) * (p[2][1] - cy) - (p[2][0] - cx) * (p[1][1] - cy)) / area;
                let w1 = ((p[2][0] - cx) * (p[0][1] - cy) - (p[0][0] - cx) * (p[2][1] - cy)) / area;
                let w2 = 1.0 - w0 - w1;
                let e = -1e-4;
                if w0 >= e && w1 >= e && w2 >= e {
                    let z = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
                    let c = &mut g[j * nx + i];
                    if z > *c { *c = z; }
                }
            } }
        }
        let bytes: Vec<u8> = g.iter().flat_map(|&z| ((z as f64 / zmax * 65535.0).round().clamp(0.0, 65535.0) as u16).to_le_bytes()).collect();
        Ok(Relief { name: name.into(), nx, ny, res, zmax, hm: b64_encode(&bytes), x: 0.0, y: 0.0, scale: 1.0 })
    }

    /// Like `from_stl`, with the resolution chosen so the height map has about `max_cells` cells (never finer than 0.25 mm).
    pub fn from_stl_auto(bytes: &[u8], name: &str, max_cells: usize) -> Result<Relief, String> {
        let tris = parse_stl(bytes)?;
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for t in &tris { for v in t { for k in 0..2 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } } }
        let area = ((hi[0] - lo[0]) as f64 * (hi[1] - lo[1]) as f64).max(1e-6);
        let res = (area / max_cells.max(1000) as f64).sqrt().max(0.25);
        Relief::from_stl(bytes, name, (res * 100.0).round() / 100.0)
    }

    /// Model size on the stock, mm.
    pub fn size(&self) -> (f64, f64) { (self.nx as f64 * self.res * self.scale, self.ny as f64 * self.res * self.scale) }

    /// Heights 0..1 (1 = the top of the model), row by row from the bottom.
    pub fn heights(&self) -> Vec<f32> {
        let b = b64_decode(&self.hm).unwrap_or_default();
        let mut v: Vec<f32> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]) as f32 / 65535.0).collect();
        v.resize(self.nx * self.ny, 0.0);
        v
    }
}

// ------------------------------------------------------------------ the surface and the tool paths over it

/// The surface to carve as heights in mm (0 = stock top, -depth = the base of the model), on a grid with a margin around the model.
pub struct Field {
    /// World position of the centre of cell (0, 0), and the cell size.
    pub ox: f64, pub oy: f64, pub g: f64,
    pub nx: usize, pub ny: usize,
    /// Cells of margin around the model.
    pub pad: usize,
    pub z: Vec<f32>,
    pub base: f32,
}

impl Field {
    /// `depth`: how deep the model's highest point to its lowest is carved, i.e. the model base sits at `-depth`.
    pub fn new(rel: &Relief, depth: f64, margin_mm: f64) -> Field {
        let g = rel.res * rel.scale;
        let pad = (margin_mm / g).ceil() as usize + 2;
        let (nx, ny) = (rel.nx + 2 * pad, rel.ny + 2 * pad);
        let base = -depth as f32;
        let mut z = vec![base; nx * ny];
        let h = rel.heights();
        for j in 0..rel.ny { for i in 0..rel.nx {
            z[(j + pad) * nx + i + pad] = base + h[j * rel.nx + i] * depth as f32;
        } }
        Field { ox: rel.x + 0.5 * g - pad as f64 * g, oy: rel.y + 0.5 * g - pad as f64 * g, g, nx, ny, pad, z, base }
    }
    pub fn at(&self, i: i64, j: i64) -> f32 {
        if i < 0 || j < 0 || i >= self.nx as i64 || j >= self.ny as i64 { self.base } else { self.z[j as usize * self.nx + i as usize] }
    }
    pub fn x(&self, i: i64) -> f64 { self.ox + i as f64 * self.g }
    pub fn y(&self, j: i64) -> f64 { self.oy + j as f64 * self.g }
}

/// The cells under a tool and how high its edge is above its tip there.
pub struct Footprint(pub Vec<(i32, i32, f32)>);

impl Footprint {
    pub fn new(g: f64, r: f64, rise: impl Fn(f64) -> f64) -> Footprint {
        let rc = (r / g).floor() as i32;
        let mut v = Vec::new();
        for dj in -rc..=rc { for di in -rc..=rc {
            let rho = g * ((di * di + dj * dj) as f64).sqrt();
            if rho <= r + 1e-9 { v.push((di, dj, rise(rho.min(r)) as f32)); }
        } }
        if v.is_empty() { v.push((0, 0, 0.0)); }
        Footprint(v)
    }
}

impl Field {
    /// Lowest height of the tool tip at cell (i, j) that keeps the whole tool on or above the surface, plus `allow`.
    pub fn tip(&self, i: i64, j: i64, fp: &Footprint, allow: f32) -> f32 {
        let mut t = f32::MIN;
        for &(di, dj, rise) in &fp.0 {
            let v = self.at(i + di as i64, j + dj as i64) + allow - rise;
            if v > t { t = v; }
        }
        t
    }
}

/// Simplify a profile (x, z) so that no point is further than `tol` (in z) from the straight line.
fn simplify(pts: &[(f64, f32)], tol: f32) -> Vec<usize> {
    let n = pts.len();
    if n < 3 { return (0..n).collect(); }
    let mut keep = vec![false; n];
    keep[0] = true; keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 { continue; }
        let (xa, za, xb, zb) = (pts[a].0, pts[a].1, pts[b].0, pts[b].1);
        let (mut worst, mut wi) = (0f32, a);
        for k in a + 1..b {
            let t = if (xb - xa).abs() < 1e-12 { 0.0 } else { ((pts[k].0 - xa) / (xb - xa)) as f32 };
            let d = (pts[k].1 - (za + (zb - za) * t)).abs();
            if d > worst { worst = d; wi = k; }
        }
        if worst > tol { keep[wi] = true; stack.push((a, wi)); stack.push((wi, b)); }
    }
    (0..n).filter(|&k| keep[k]).collect()
}

/// Finishing passes: straight lines along X, `step_cells` rows apart, the tool tip following the surface.
/// Each line is a list of (x, y, z) from left to right over the model and a margin of `margin` cells.
pub fn finish_lines(f: &Field, fp: &Footprint, margin: usize, step_cells: usize, tol: f32) -> Vec<Vec<[f64; 3]>> {
    let (i0, i1) = (f.pad as i64 - margin as i64, (f.nx - f.pad + margin) as i64 - 1);
    let (j0, j1) = (f.pad as i64 - margin as i64, (f.ny - f.pad + margin) as i64 - 1);
    let mut lines = Vec::new();
    let mut j = j0;
    while j <= j1 {
        let prof: Vec<(f64, f32)> = (i0..=i1).map(|i| (f.x(i), f.tip(i, j, fp, 0.0))).collect();
        let keep = simplify(&prof, tol);
        lines.push(keep.iter().map(|&k| [prof[k].0, f.y(j), prof[k].1 as f64]).collect());
        // always end exactly on the last row
        if j == j1 { break; }
        j = (j + step_cells.max(1) as i64).min(j1);
    }
    lines
}

/// Roughing plan: the lowest height the roughing tool may reach at each cell of a coarser grid.
pub struct Rough {
    pub ox: f64, pub oy: f64, pub gc: f64,
    pub nx: usize, pub ny: usize,
    pub t: Vec<f32>,
    pub min_t: f32,
}

impl Rough {
    /// `k`: fine cells per coarse cell. The surface is max-pooled first, so the coarse grid never lets the tool cut too deep.
    pub fn new(f: &Field, k: usize, r: f64, allow: f32, rise: impl Fn(f64) -> f64, margin: usize) -> Rough {
        let k = k.max(1);
        let (i0, i1) = (f.pad - margin, f.nx - f.pad + margin);
        let (j0, j1) = (f.pad - margin, f.ny - f.pad + margin);
        let (nx, ny) = ((i1 - i0).div_ceil(k), (j1 - j0).div_ceil(k));
        let gc = f.g * k as f64;
        let mut s = vec![f32::MIN; nx * ny];
        for cj in 0..ny { for ci in 0..nx {
            let mut m = f32::MIN;
            for dj in 0..k { for di in 0..k {
                m = m.max(f.at((i0 + ci * k + di) as i64, (j0 + cj * k + dj) as i64));
            } }
            s[cj * nx + ci] = m + allow;
        } }
        // the coarse footprint, a little wider to cover the rounding of the grid
        let rc = ((r + 0.7 * gc) / gc).floor() as i32;
        let mut fp: Vec<(i32, i32, f32)> = Vec::new();
        for dj in -rc..=rc { for di in -rc..=rc {
            let rho = gc * ((di * di + dj * dj) as f64).sqrt();
            if rho <= r + 0.7 * gc + 1e-9 { fp.push((di, dj, rise(rho.min(r)) as f32)); }
        } }
        let mut t = vec![f32::MIN; nx * ny];
        let mut min_t = f32::MAX;
        for cj in 0..ny as i32 { for ci in 0..nx as i32 {
            let mut m = f.base + allow - rise(0.0) as f32;
            for &(di, dj, rs) in &fp {
                let (a, b) = (ci + di, cj + dj);
                let v = if a < 0 || b < 0 || a >= nx as i32 || b >= ny as i32 { f.base + allow } else { s[b as usize * nx + a as usize] };
                m = m.max(v - rs);
            }
            t[cj as usize * nx + ci as usize] = m;
            min_t = min_t.min(m);
        } }
        Rough { ox: f.x((i0 + k / 2) as i64) - if k % 2 == 0 { 0.5 * f.g } else { 0.0 }, oy: f.y((j0 + k / 2) as i64) - if k % 2 == 0 { 0.5 * f.g } else { 0.0 }, gc, nx, ny, t, min_t }
    }
    pub fn x(&self, ci: usize) -> f64 { self.ox + ci as f64 * self.gc }
    pub fn y(&self, cj: usize) -> f64 { self.oy + cj as f64 * self.gc }
    /// Runs (first, last cell) of row `cj` where the tool can be at height `z`.
    pub fn runs(&self, cj: usize, z: f32) -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        let mut start: Option<usize> = None;
        for ci in 0..self.nx {
            let ok = self.t[cj * self.nx + ci] <= z + 1e-6;
            match (ok, start) {
                (true, None) => start = Some(ci),
                (false, Some(s)) => { v.push((s, ci - 1)); start = None; }
                _ => {}
            }
        }
        if let Some(s) = start { v.push((s, self.nx - 1)); }
        v
    }
    /// True when the straight line between two points stays where the tool can be at height `z`.
    pub fn open_line(&self, a: [f64; 2], b: [f64; 2], z: f32) -> bool {
        let n = (((b[0] - a[0]).hypot(b[1] - a[1])) / (self.gc * 0.5)).ceil().max(1.0) as usize;
        (0..=n).all(|s| {
            let f = s as f64 / n as f64;
            let (x, y) = (a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f);
            let ci = ((x - self.ox) / self.gc).round();
            let cj = ((y - self.oy) / self.gc).round();
            ci >= 0.0 && cj >= 0.0 && (ci as usize) < self.nx && (cj as usize) < self.ny && self.t[cj as usize * self.nx + ci as usize] <= z + 1e-6
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A binary STL of a four-sided pyramid, 20 x 20 mm at the base and 10 mm high.
    pub fn pyramid_stl() -> Vec<u8> {
        let (a, b, c, d, top) = ([0.0f32, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 20.0, 0.0], [0.0, 20.0, 0.0], [10.0f32, 10.0, 10.0]);
        let tris = [[a, b, top], [b, c, top], [c, d, top], [d, a, top], [a, c, b], [a, d, c]];
        let mut out = vec![0u8; 80];
        out.extend((tris.len() as u32).to_le_bytes());
        for t in tris {
            out.extend([0u8; 12]);
            for v in t { for x in v { out.extend(x.to_le_bytes()); } }
            out.extend([0u8; 2]);
        }
        out
    }

    #[test]
    fn base64_round_trips() {
        for n in 0..9 {
            let d: Vec<u8> = (0..n as u8).map(|i| i.wrapping_mul(37)).collect();
            assert_eq!(b64_decode(&b64_encode(&d)).unwrap(), d);
        }
    }

    #[test]
    fn an_stl_becomes_a_height_map() {
        let r = Relief::from_stl(&pyramid_stl(), "pyramid", 1.0).unwrap();
        assert_eq!((r.nx, r.ny), (20, 20));
        assert!((r.zmax - 10.0).abs() < 1e-6);
        let h = r.heights();
        // the middle is the peak, the corners are near the base
        assert!(h[10 * 20 + 10] > 0.9 && h[0] < 0.15, "{} {}", h[10 * 20 + 10], h[0]);
        // slope: halfway to the peak the pyramid is half as high
        assert!((h[10 * 20 + 5] - 0.5).abs() < 0.1, "{}", h[10 * 20 + 5]);
        assert!(Relief::from_stl(b"nonsense", "x", 1.0).is_err());
    }

    #[test]
    fn a_ball_nose_never_cuts_below_the_surface() {
        let r = Relief::from_stl(&pyramid_stl(), "pyramid", 0.5).unwrap();
        let f = Field::new(&r, 10.0, 3.0);
        let fp = Footprint::new(f.g, 1.5, |rho| 1.5 - (1.5f64 * 1.5 - rho * rho).max(0.0).sqrt());
        // at the peak the tip must be at least at the surface height minus the ball's rise there
        let ci = (f.pad + r.nx / 2) as i64;
        let t = f.tip(ci, ci, &fp, 0.0);
        assert!(t > -1.0 && t <= 0.01, "{t}");
        // on the flat margin the tip goes to the base
        assert!((f.tip(0, 0, &fp, 0.0) + 10.0).abs() < 1e-3);
        let lines = finish_lines(&f, &fp, 2, 2, 0.01);
        assert!(!lines.is_empty() && lines.iter().all(|l| l.len() >= 2));
    }
}
