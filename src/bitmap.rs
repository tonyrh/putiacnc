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

#[derive(Clone, Debug)]
pub struct Opts {
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
    fn default() -> Self { Opts { threshold: 128, invert: false, width_mm: 100.0, speck_mm2: 0.5, smooth: true } }
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

    pub fn binary(&self, o: &Opts) -> BinaryImage {
        let mut b = BinaryImage::new_w_h(self.w, self.h);
        for i in 0..self.luma.len() { if self.is_ink(i, o) { b.set_pixel(i % self.w, i / self.w, true); } }
        b
    }

    /// Size of the result in mm.
    pub fn size_mm(&self, o: &Opts) -> (f64, f64) {
        let w = o.width_mm.max(0.1);
        (w, w * self.h as f64 / self.w as f64)
    }

    pub fn trace(&self, o: &Opts) -> Result<Imported, String> {
        let (wmm, hmm) = self.size_mm(o);
        let k = wmm / self.w as f64; // mm per pixel
        let min_px = (o.speck_mm2 / (k * k)).ceil().max(1.0) as usize;
        let bin = self.binary(o);
        let clusters = bin.to_clusters(false);
        let mode = if o.smooth { PathSimplifyMode::Spline } else { PathSimplifyMode::Polygon };
        let mut loops: Vec<LoopData> = vec![];
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
                if let Some(l) = to_loop(pts, k, hmm) { loops.push(l); }
            }
        }
        let mut warnings = vec![];
        if loops.is_empty() { warnings.push("Nothing was traced: adjust the threshold, or switch Invert.".into()); }
        Ok(Imported { loops, width_mm: wmm, height_mm: hmm, warnings })
    }
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
fn to_loop(px: Vec<[f64; 2]>, k: f64, hmm: f64) -> Option<LoopData> {
    let mut pts: Vec<[i64; 2]> = vec![];
    for p in px {
        let q = [(p[0] * k * S).round() as i64, ((hmm - p[1] * k) * S).round() as i64];
        if pts.last() != Some(&q) { pts.push(q); }
    }
    if pts.len() > 1 && pts.first() == pts.last() { pts.pop(); }
    if pts.len() < 3 { return None; }
    Some(LoopData { c: 1, op: 0, rc: None, cr: None, el: None, pt: None, grp: None, pts: pts.iter().flat_map(|p| [p[0], p[1]]).collect() })
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
