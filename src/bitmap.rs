//! Bitmap import: a picture (PNG, JPG, BMP) is cut to black and white at a threshold and its outlines are
//! traced into closed shapes, in millimetres, ready for Profile, Pocket and the other operations.
//!
//! The tracing itself (outlines, holes, curve fitting) is done by `visioncortex`.
use crate::model::LoopData;
use crate::svg::Imported;
use visioncortex::{BinaryImage, CompoundPathElement, PathSimplifyMode, PointF64};

/// Pictures are reduced to this many pixels on the long side before tracing; more detail than this only slows it down.
const MAX_SIDE: u32 = 1600;
const S: f64 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum Mode {
    /// Closed outlines of the dark (or light) areas, holes included: for Profile, Pocket, V-carve.
    #[default]
    Outline,
    /// One open line along the middle of each stroke: for engraving lines and writing.
    Centerline,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Opts {
    pub mode: Mode,
    /// Cut the picture down to the area that has something in it.
    pub crop: bool,
    /// Ignore this percentage of the picture along every edge (a scanner's border, a frame).
    pub trim_pct: f64,
    /// A pixel darker than this (0..=255) is part of the shape.
    pub threshold: u8,
    /// Trace the light areas instead of the dark ones.
    pub invert: bool,
    /// Width of the result in mm; the height follows the picture's proportions.
    pub width_mm: f64,
    /// Specks smaller than this area (mm²) are dropped.
    pub speck_mm2: f64,
    /// Round the outlines with curves (true) or keep straight, pixel-following polygons (false).
    pub smooth: bool,
}

impl Default for Opts {
    fn default() -> Self { Opts { mode: Mode::Outline, crop: false, trim_pct: 0.0, threshold: 128, invert: false, width_mm: 100.0, speck_mm2: 0.5, smooth: true } }
}

/// A decoded picture, as brightness only (transparent areas count as white).
pub struct Bitmap {
    pub w: usize,
    pub h: usize,
    pub luma: Vec<u8>,
}

impl Bitmap {
    pub fn load(bytes: &[u8]) -> Result<Bitmap, String> {
        let mut img = image::load_from_memory(bytes).map_err(|e| format!("Could not read the picture: {e}"))?;
        let (w, h) = (img.width(), img.height());
        if w < 2 || h < 2 { return Err("The picture is too small.".into()); }
        if w.max(h) > MAX_SIDE {
            let f = MAX_SIDE as f64 / w.max(h) as f64;
            img = img.resize_exact(((w as f64 * f).round() as u32).max(2), ((h as f64 * f).round() as u32).max(2), image::imageops::FilterType::Triangle);
        }
        let rgba = img.to_rgba8();
        let luma = rgba.pixels().map(|p| {
            let (r, g, b, a) = (p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32 / 255.0);
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            (y * a + 255.0 * (1.0 - a)).round().clamp(0.0, 255.0) as u8
        }).collect();
        Ok(Bitmap { w: rgba.width() as usize, h: rgba.height() as usize, luma })
    }

    /// True where the shape is.
    pub fn is_ink(&self, i: usize, o: &Opts) -> bool { (self.luma[i] < o.threshold) != o.invert }

    /// The part of the picture that is traced, as pixels `(x0, y0, x1, y1)` with the end excluded:
    /// the picture less the trimmed edges, then (when asked) cut down to where the ink is.
    pub fn region(&self, o: &Opts) -> (usize, usize, usize, usize) {
        let t = o.trim_pct.clamp(0.0, 45.0) / 100.0;
        let (mut x0, mut y0) = ((self.w as f64 * t).round() as usize, (self.h as f64 * t).round() as usize);
        let (mut x1, mut y1) = (self.w - x0, self.h - y0);
        if o.crop {
            let (mut bx0, mut by0, mut bx1, mut by1) = (usize::MAX, usize::MAX, 0, 0);
            for y in y0..y1 { for x in x0..x1 {
                if self.is_ink(y * self.w + x, o) { bx0 = bx0.min(x); by0 = by0.min(y); bx1 = bx1.max(x + 1); by1 = by1.max(y + 1); }
            } }
            if bx1 > bx0 && by1 > by0 { (x0, y0, x1, y1) = (bx0, by0, bx1, by1); }
        }
        (x0, y0, x1.max(x0 + 2), y1.max(y0 + 2))
    }

    pub fn binary(&self, o: &Opts) -> BinaryImage {
        let (x0, y0, x1, y1) = self.region(o);
        let mut b = BinaryImage::new_w_h(x1 - x0, y1 - y0);
        for y in y0..y1 { for x in x0..x1 { if self.is_ink(y * self.w + x, o) { b.set_pixel(x - x0, y - y0, true); } } }
        b
    }

    /// Size of the result in mm.
    pub fn size_mm(&self, o: &Opts) -> (f64, f64) {
        let (x0, y0, x1, y1) = self.region(o);
        let w = o.width_mm.max(0.1);
        (w, w * (y1 - y0) as f64 / (x1 - x0) as f64)
    }

    pub fn trace(&self, o: &Opts) -> Result<Imported, String> {
        let (wmm, hmm) = self.size_mm(o);
        let bin = self.binary(o);
        let k = wmm / bin.width as f64; // mm per pixel
        let min_px = (o.speck_mm2 / (k * k)).ceil().max(1.0) as usize;
        let clusters = bin.to_clusters(false);
        let mut loops: Vec<LoopData> = vec![];
        match o.mode {
            Mode::Outline => {
                let mode = if o.smooth { PathSimplifyMode::Spline } else { PathSimplifyMode::Polygon };
                for c in clusters.iter() {
                    if c.size() < min_px { continue; }
                    // corner threshold 60 degrees, segments of at least 4 px, 10 refinements, splice at 45 degrees (vtracer's defaults)
                    let cp = c.to_compound_path(mode, 60.0_f64.to_radians(), 4.0, 10, 45.0_f64.to_radians());
                    for el in cp.iter() {
                        let pts: Vec<[f64; 2]> = match el {
                            CompoundPathElement::PathI32(p) => p.path.iter().map(|q| [q.x as f64, q.y as f64]).collect(),
                            CompoundPathElement::PathF64(p) => p.path.iter().map(|q| [q.x, q.y]).collect(),
                            CompoundPathElement::Spline(s) => flatten(&s.points, k),
                        };
                        if let Some(l) = to_loop(pts, k, hmm, true) { loops.push(l); }
                    }
                }
            }
            Mode::Centerline => {
                // drop the specks first, so they do not leave stray dots
                let mut keep = BinaryImage::new_w_h(bin.width, bin.height);
                for c in clusters.iter() { if c.size() >= min_px { for p in &c.points { keep.set_pixel(p.x as usize, p.y as usize, true); } } }
                let skel = thin(&keep);
                // a tail shorter than about half a millimetre (and never under 2 px) is a thinning artefact, not part of the drawing
                let spur = ((0.5 / k).ceil() as usize).max(2);
                for (pts, closed) in skeleton_paths(&skel, bin.width, bin.height, spur) {
                    let tol = (0.05 / k).max(0.7); // simplify to 0.05 mm, but never finer than the pixel grid can tell
                    let mut pts = simplify(&pts, tol, closed);
                    if o.smooth { for _ in 0..2 { pts = chaikin(&pts, closed); } }
                    if let Some(l) = to_loop(pts, k, hmm, closed) { loops.push(l); }
                }
            }
        }
        let mut warnings = vec![];
        if loops.is_empty() { warnings.push("Nothing was traced: adjust the threshold, or switch Invert.".into()); }
        Ok(Imported { loops, width_mm: wmm, height_mm: hmm, warnings })
    }
}

// ------------------------------------------------------------------ centre lines

/// Zhang-Suen thinning: every stroke shrinks to a line one pixel wide, keeping its connections.
fn thin(img: &BinaryImage) -> Vec<bool> {
    let (w, h) = (img.width, img.height);
    let mut g: Vec<bool> = (0..w * h).map(|i| img.get_pixel(i % w, i / w)).collect();
    // neighbours P2..P9, clockwise from the pixel above
    const N: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];
    let at = |g: &Vec<bool>, x: i32, y: i32| x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && g[y as usize * w + x as usize];
    loop {
        let mut changed = false;
        for pass in 0..2 {
            let mut kill = vec![];
            for y in 0..h as i32 { for x in 0..w as i32 {
                if !g[y as usize * w + x as usize] { continue; }
                let p: Vec<bool> = N.iter().map(|d| at(&g, x + d.0, y + d.1)).collect();
                let b = p.iter().filter(|&&v| v).count();
                if !(2..=6).contains(&b) { continue; }
                let a = (0..8).filter(|&i| !p[i] && p[(i + 1) % 8]).count();
                if a != 1 { continue; }
                let ok = if pass == 0 { !(p[0] && p[2] && p[4]) && !(p[2] && p[4] && p[6]) } else { !(p[0] && p[2] && p[6]) && !(p[0] && p[4] && p[6]) };
                if ok { kill.push(y as usize * w + x as usize); }
            } }
            if !kill.is_empty() { changed = true; for i in kill { g[i] = false; } }
        }
        if !changed { break; }
    }
    g
}

const N8: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// How many separate runs of neighbours a skeleton pixel has: 1 = the end of a line, 2 = along a line, 3+ = a junction.
fn crossings(s: &[bool], w: usize, h: usize, x: usize, y: usize) -> usize {
    let p: Vec<bool> = N8.iter().map(|d| {
        let (nx, ny) = (x as i32 + d.0, y as i32 + d.1);
        nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && s[ny as usize * w + nx as usize]
    }).collect();
    (0..8).filter(|&i| !p[i] && p[(i + 1) % 8]).count()
}

/// The lines of a one-pixel skeleton, as pixel-centre polylines (and whether each closes on itself).
/// Tails shorter than `spur` pixels that hang off a junction are dropped.
fn skeleton_paths(s: &[bool], w: usize, h: usize, spur: usize) -> Vec<(Vec<[f64; 2]>, bool)> {
    let idx = |x: usize, y: usize| y * w + x;
    let mut cn = vec![0usize; w * h];
    for y in 0..h { for x in 0..w { if s[idx(x, y)] { cn[idx(x, y)] = crossings(s, w, h, x, y); } } }
    let is_node = |i: usize| s[i] && cn[i] != 2;
    let neighbours = |x: usize, y: usize| -> Vec<(usize, usize, bool)> {
        N8.iter().enumerate().filter_map(|(k, d)| {
            let (nx, ny) = (x as i32 + d.0, y as i32 + d.1);
            if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h || !s[idx(nx as usize, ny as usize)] { None } else { Some((nx as usize, ny as usize, k % 2 == 0)) }
        }).collect()
    };
    let mut seen = vec![false; w * h];
    let mut out: Vec<(Vec<[f64; 2]>, bool)> = vec![];
    let mut spurs: Vec<bool> = vec![];
    let c = |x: usize, y: usize| [x as f64 + 0.5, y as f64 + 0.5];
    // walk from every node along every line leaving it
    for y in 0..h { for x in 0..w {
        if !is_node(idx(x, y)) { continue; }
        for (nx, ny, _) in neighbours(x, y) {
            if seen[idx(nx, ny)] { continue; }
            let mut path = vec![c(x, y)];
            let (mut px, mut py, mut cx, mut cy) = (x, y, nx, ny);
            loop {
                path.push(c(cx, cy));
                seen[idx(cx, cy)] = true;
                if is_node(idx(cx, cy)) { break; }
                // continue to the unvisited neighbour, preferring a side-on one; the rest next to it are redundant
                let mut next: Option<(usize, usize)> = None;
                let mut cand = neighbours(cx, cy);
                cand.sort_by_key(|&(_, _, orth)| !orth);
                for (ax, ay, _) in cand {
                    if (ax, ay) == (px, py) || seen[idx(ax, ay)] && !is_node(idx(ax, ay)) { continue; }
                    if next.is_none() { next = Some((ax, ay)); }
                }
                match next { Some((ax, ay)) => { px = cx; py = cy; cx = ax; cy = ay; } None => break }
            }
            let end_is_tip = !is_node(idx(cx, cy)) || cn[idx(cx, cy)] == 1;
            let start_is_tip = cn[idx(x, y)] == 1;
            spurs.push((end_is_tip ^ start_is_tip) && path.len() < spur);
            out.push((path, false));
        }
    } }
    // lines with no node at all are loops (a ring, an O)
    for y in 0..h { for x in 0..w {
        if !s[idx(x, y)] || seen[idx(x, y)] { continue; }
        let mut path = vec![];
        let (mut px, mut py, mut cx, mut cy) = (usize::MAX, usize::MAX, x, y);
        loop {
            path.push(c(cx, cy));
            seen[idx(cx, cy)] = true;
            let mut cand = neighbours(cx, cy);
            cand.sort_by_key(|&(_, _, orth)| !orth);
            let next = cand.into_iter().find(|&(ax, ay, _)| (ax, ay) != (px, py) && !seen[idx(ax, ay)]);
            match next { Some((ax, ay, _)) => { px = cx; py = cy; cx = ax; cy = ay; } None => break }
        }
        let closed = path.len() > 3;
        out.push((path, closed));
        spurs.push(false);
    } }
    let mut keep = spurs.into_iter();
    out.into_iter().filter(|_| !keep.next().unwrap_or(false)).filter(|(p, _)| p.len() >= 3).collect()
}

/// Douglas-Peucker: fewest points that stay within `tol` pixels of the line (a closed shape is split at its far point).
fn simplify(p: &[[f64; 2]], tol: f64, closed: bool) -> Vec<[f64; 2]> {
    fn dp(p: &[[f64; 2]], tol: f64, out: &mut Vec<[f64; 2]>) {
        let (a, b) = (p[0], p[p.len() - 1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        let (mut worst, mut wi) = (0.0, 0);
        for (i, q) in p.iter().enumerate().take(p.len() - 1).skip(1) {
            let d = if len < 1e-12 { (q[0] - a[0]).hypot(q[1] - a[1]) } else { ((q[0] - a[0]) * dy - (q[1] - a[1]) * dx).abs() / len };
            if d > worst { worst = d; wi = i; }
        }
        if worst > tol { dp(&p[..=wi], tol, out); out.pop(); dp(&p[wi..], tol, out); } else { out.push(a); out.push(b); }
    }
    if p.len() < 3 { return p.to_vec(); }
    let mut out = vec![];
    if closed {
        let far = (1..p.len()).max_by(|&i, &j| {
            let d = |k: usize| (p[k][0] - p[0][0]).hypot(p[k][1] - p[0][1]);
            d(i).partial_cmp(&d(j)).unwrap()
        }).unwrap();
        let mut a = vec![]; dp(&p[..=far], tol, &mut a);
        let mut rest = p[far..].to_vec(); rest.push(p[0]);
        let mut b = vec![]; dp(&rest, tol, &mut b);
        a.pop(); b.pop();
        out.extend(a); out.extend(b);
    } else { dp(p, tol, &mut out); }
    out.dedup();
    out
}

/// One round of corner cutting, which rounds a polyline; the ends of an open line stay where they are.
fn chaikin(p: &[[f64; 2]], closed: bool) -> Vec<[f64; 2]> {
    if p.len() < 3 { return p.to_vec(); }
    let mut out = vec![];
    let n = p.len();
    if !closed { out.push(p[0]); }
    let last = if closed { n } else { n - 1 };
    for i in 0..last {
        let (a, b) = (p[i], p[(i + 1) % n]);
        out.push([0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]]);
        out.push([0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]);
    }
    if !closed { out.push(p[n - 1]); }
    out
}

/// A chain of cubic Béziers (1 + 3n points) as a polyline, with points about 0.4 mm apart along the curve.
fn flatten(p: &[PointF64], k: f64) -> Vec<[f64; 2]> {
    let mut out = vec![];
    if p.is_empty() { return out; }
    out.push([p[0].x, p[0].y]);
    for c in p[1..].chunks_exact(3) {
        let a = out[out.len() - 1];
        let (c1, c2, d) = ([c[0].x, c[0].y], [c[1].x, c[1].y], [c[2].x, c[2].y]);
        let len = ((c1[0] - a[0]).hypot(c1[1] - a[1]) + (c2[0] - c1[0]).hypot(c2[1] - c1[1]) + (d[0] - c2[0]).hypot(d[1] - c2[1])) * k;
        let n = ((len / 0.4).ceil() as usize).clamp(2, 48);
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            out.push([w0 * a[0] + w1 * c1[0] + w2 * c2[0] + w3 * d[0], w0 * a[1] + w1 * c1[1] + w2 * c2[1] + w3 * d[1]]);
        }
    }
    out
}

/// Pixel coordinates (y down) to a closed loop in 1/1000 mm (y up).
fn to_loop(px: Vec<[f64; 2]>, k: f64, hmm: f64, closed: bool) -> Option<LoopData> {
    let mut pts: Vec<[i64; 2]> = vec![];
    for p in px {
        let q = [(p[0] * k * S).round() as i64, ((hmm - p[1] * k) * S).round() as i64];
        if pts.last() != Some(&q) { pts.push(q); }
    }
    if closed && pts.len() > 1 && pts.first() == pts.last() { pts.pop(); }
    if pts.len() < if closed { 3 } else { 2 } { return None; }
    Some(LoopData { c: closed as u8, op: 0, rc: None, cr: None, el: None, pt: None, grp: None, ug: None, pts: pts.iter().flat_map(|p| [p[0], p[1]]).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 60x40 picture: a black 30x20 rectangle with a 10x6 white hole, and a 2x2 speck.
    fn picture() -> Bitmap {
        let (w, h) = (60usize, 40usize);
        let mut luma = vec![255u8; w * h];
        for y in 10..30 { for x in 10..40 { luma[y * w + x] = 0; } }
        for y in 17..23 { for x in 20..30 { luma[y * w + x] = 255; } }
        for y in 2..4 { for x in 50..52 { luma[y * w + x] = 0; } }
        Bitmap { w, h, luma }
    }

    fn area(l: &LoopData) -> f64 {
        let p = l.path();
        let mut a = 0.0;
        for i in 0..p.len() { let (u, v) = (p[i], p[(i + 1) % p.len()]); a += (u[0] as f64) * (v[1] as f64) - (v[0] as f64) * (u[1] as f64); }
        (a / 2.0).abs() / 1e6
    }

    #[test]
    fn traces_the_shape_and_its_hole_and_drops_the_speck() {
        let b = picture();
        let o = Opts { width_mm: 60.0, smooth: false, speck_mm2: 6.0, ..Opts::default() }; // 1 px = 1 mm
        let r = b.trace(&o).unwrap();
        assert_eq!((r.width_mm, r.height_mm), (60.0, 40.0));
        assert_eq!(r.loops.len(), 2, "outline + hole");
        let mut a: Vec<f64> = r.loops.iter().map(area).collect();
        a.sort_by(|x, y| x.partial_cmp(y).unwrap());
        assert!((a[0] - 60.0).abs() < 2.0, "{a:?}"); // the hole
        assert!((a[1] - 600.0).abs() < 8.0, "{a:?}"); // the rectangle
        assert!(r.loops.iter().all(|l| l.closed()));
    }

    #[test]
    fn y_is_up_and_the_size_follows_the_width() {
        let b = picture();
        let o = Opts { width_mm: 120.0, smooth: false, speck_mm2: 30.0, ..Opts::default() }; // 1 px = 2 mm: the 4 px speck is 16 mm²
        let r = b.trace(&o).unwrap();
        let bb = crate::shapes::bbox_many(r.loops.iter()).unwrap();
        // the rectangle spans x 10..40 px and y 10..30 px from the top of an 40 px picture: y 10..30 from the bottom as well
        assert!((bb[0] - 20.0).abs() < 1.0 && (bb[2] - 80.0).abs() < 1.0, "{bb:?}");
        assert!((bb[1] - 20.0).abs() < 1.0 && (bb[3] - 60.0).abs() < 1.0, "{bb:?}");
    }

    #[test]
    fn invert_traces_the_light_areas_and_smoothing_still_closes_shapes() {
        let b = picture();
        let o = Opts { width_mm: 60.0, invert: true, smooth: true, speck_mm2: 6.0, ..Opts::default() };
        let r = b.trace(&o).unwrap();
        assert!(!r.loops.is_empty() && r.loops.iter().all(|l| l.closed() && l.pts.len() >= 6));
        // an all-black picture traced as dark gives one shape; as light, nothing and a hint
        let black = Bitmap { w: 10, h: 10, luma: vec![0; 100] };
        assert_eq!(black.trace(&Opts { invert: true, ..Opts::default() }).unwrap().loops.len(), 0);
        assert!(!black.trace(&Opts { invert: true, ..Opts::default() }).unwrap().warnings.is_empty());
    }

    /// Dark strokes 4 px thick: a horizontal bar, and a plus sign.
    fn strokes() -> Bitmap {
        let (w, h) = (80usize, 60usize);
        let mut luma = vec![255u8; w * h];
        for y in 8..12 { for x in 5..45 { luma[y * w + x] = 0; } }       // bar: y centre 10, x 5..45
        for y in 30..54 { for x in 58..62 { luma[y * w + x] = 0; } }     // vertical of the plus: x centre 60, y 30..54
        for y in 40..44 { for x in 48..72 { luma[y * w + x] = 0; } }     // horizontal of the plus: y centre 42
        Bitmap { w, h, luma }
    }

    fn length_mm(l: &LoopData) -> f64 {
        let p = l.path();
        (1..p.len()).map(|i| ((p[i][0] - p[i - 1][0]) as f64).hypot((p[i][1] - p[i - 1][1]) as f64)).sum::<f64>() / 1000.0
    }

    #[test]
    fn a_centre_line_runs_down_the_middle_of_a_stroke() {
        let o = Opts { mode: Mode::Centerline, width_mm: 80.0, smooth: false, speck_mm2: 4.0, ..Opts::default() }; // 1 px = 1 mm
        let r = strokes().trace(&o).unwrap();
        assert!(r.loops.iter().all(|l| !l.closed()), "centre lines are open");
        // the bar: one line about 40 mm long (the thinning shortens the ends by about half a stroke), at y 10 from the top = 50 from the bottom
        let bar = r.loops.iter().find(|l| { let b = crate::shapes::bbox(l).unwrap(); b[3] > 45.0 }).expect("the bar");
        let b = crate::shapes::bbox(bar).unwrap();
        assert!((b[1] - 50.0).abs() < 1.5 && (b[3] - 50.0).abs() < 1.5, "{b:?}");
        assert!(length_mm(bar) > 34.0 && length_mm(bar) < 41.0, "{}", length_mm(bar));
        // the plus: every line stays inside it and the total length is about 24 + 24
        let plus: Vec<&LoopData> = r.loops.iter().filter(|l| crate::shapes::bbox(l).unwrap()[3] < 35.0).collect();
        assert!(!plus.is_empty());
        let total: f64 = plus.iter().map(|l| length_mm(l)).sum();
        assert!(total > 36.0 && total < 52.0, "{total}");
    }

    #[test]
    fn a_ring_becomes_one_closed_centre_line() {
        let (w, h) = (60usize, 60usize);
        let mut luma = vec![255u8; w * h];
        for y in 0..h { for x in 0..w {
            let d = ((x as f64 - 30.0).powi(2) + (y as f64 - 30.0).powi(2)).sqrt();
            if d > 16.0 && d < 20.0 { luma[y * w + x] = 0; }
        } }
        let r = Bitmap { w, h, luma }.trace(&Opts { mode: Mode::Centerline, width_mm: 60.0, ..Opts::default() }).unwrap();
        assert_eq!(r.loops.len(), 1);
        assert!(r.loops[0].closed());
        let b = crate::shapes::bbox(&r.loops[0]).unwrap();
        assert!((b[2] - b[0] - 36.0).abs() < 2.5, "{b:?}"); // radius 18 on the middle of the ring
    }

    #[test]
    fn crop_to_content_and_edge_trim_change_the_traced_area() {
        let b = picture(); // ink spans x 10..40, y 10..30 (the speck is at 50..52 x, 2..4 y)
        let plain = Opts { width_mm: 60.0, ..Opts::default() };
        assert_eq!(b.region(&plain), (0, 0, 60, 40));
        let crop = Opts { crop: true, ..plain.clone() };
        assert_eq!(b.region(&crop), (10, 2, 52, 30)); // the speck counts as content
        let trim = Opts { trim_pct: 10.0, crop: true, ..plain.clone() }; // 6 px and 4 px off every edge: the speck is out
        assert_eq!(b.region(&trim), (10, 10, 40, 30));
        let r = b.trace(&Opts { width_mm: 30.0, smooth: false, crop: true, trim_pct: 10.0, speck_mm2: 6.0, ..Opts::default() }).unwrap();
        assert_eq!((r.width_mm, r.height_mm), (30.0, 20.0));
        let bb = crate::shapes::bbox_many(r.loops.iter()).unwrap();
        assert!(bb[0].abs() < 1.0 && bb[1].abs() < 1.0 && (bb[2] - 30.0).abs() < 1.0 && (bb[3] - 20.0).abs() < 1.0, "{bb:?}");
    }

    #[test]
    fn reads_a_png() {
        let mut img = image::GrayImage::from_pixel(20, 10, image::Luma([255]));
        for y in 2..8 { for x in 4..16 { img.put_pixel(x, y, image::Luma([0])); } }
        let mut png = vec![];
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let b = Bitmap::load(&png).unwrap();
        assert_eq!((b.w, b.h), (20, 10));
        let r = b.trace(&Opts { width_mm: 20.0, speck_mm2: 0.0, ..Opts::default() }).unwrap();
        assert_eq!(r.loops.len(), 1);
        assert!(Bitmap::load(b"not a picture").is_err());
    }
}
