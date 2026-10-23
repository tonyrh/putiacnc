//! Text to outlines: the glyph contours of a font, flattened into closed polygons (one shape per contour,
//! so the holes of letters like O and A are separate shapes inside the outer one).
use crate::model::LoopData;
use crate::shapes::S;
use ttf_parser::{Face, OutlineBuilder};

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Align { Left, Center, Right }

#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    /// Height of capital letters, in mm.
    pub size_mm: f64,
    /// Extra space added after every letter, in mm.
    pub spacing_mm: f64,
    pub align: Align,
}

struct Builder {
    scale: f64,
    ox: f64,
    oy: f64,
    cur: Vec<[f64; 2]>,
    contours: Vec<Vec<[f64; 2]>>,
}

impl Builder {
    fn p(&self, x: f32, y: f32) -> [f64; 2] { [self.ox + x as f64 * self.scale, self.oy + y as f64 * self.scale] }
    fn finish(&mut self) {
        let mut c = std::mem::take(&mut self.cur);
        c.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6);
        if c.len() > 1 && (c[0][0] - c[c.len() - 1][0]).abs() < 1e-6 && (c[0][1] - c[c.len() - 1][1]).abs() < 1e-6 { c.pop(); }
        if c.len() >= 3 { self.contours.push(c); }
    }
    fn steps(&self, len_units: f64) -> usize { ((len_units * self.scale / 0.12).ceil() as usize).clamp(3, 40) }
}

impl OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) { self.finish(); let p = self.p(x, y); self.cur.push(p); }
    fn line_to(&mut self, x: f32, y: f32) { let p = self.p(x, y); self.cur.push(p); }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let Some(&a) = self.cur.last() else { return };
        let (c, b) = (self.p(x1, y1), self.p(x, y));
        let n = self.steps(((c[0] - a[0]).hypot(c[1] - a[1]) + (b[0] - c[0]).hypot(b[1] - c[1])) / self.scale);
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            self.cur.push([u * u * a[0] + 2.0 * u * t * c[0] + t * t * b[0], u * u * a[1] + 2.0 * u * t * c[1] + t * t * b[1]]);
        }
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let Some(&a) = self.cur.last() else { return };
        let (c1, c2, b) = (self.p(x1, y1), self.p(x2, y2), self.p(x, y));
        let n = self.steps(((c1[0] - a[0]).hypot(c1[1] - a[1]) + (c2[0] - c1[0]).hypot(c2[1] - c1[1]) + (b[0] - c2[0]).hypot(b[1] - c2[1])) / self.scale);
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            self.cur.push([
                u * u * u * a[0] + 3.0 * u * u * t * c1[0] + 3.0 * u * t * t * c2[0] + t * t * t * b[0],
                u * u * u * a[1] + 3.0 * u * u * t * c1[1] + 3.0 * u * t * t * c2[1] + t * t * t * b[1],
            ]);
        }
    }
    fn close(&mut self) { self.finish(); }
}

/// Joins contours that overlap. Many fonts (variable fonts, scripts, letters that touch their neighbours)
/// draw a glyph from overlapping parts; with the even-odd rule used for holes the overlap would show up
/// as a hole. Outer outlines and holes run in opposite directions in a font, so filling by the non-zero
/// rule gives the true shape. The result is ordered left to right.
fn merge_overlaps(contours: Vec<crate::clip::Path>) -> Vec<crate::clip::Path> {
    if contours.is_empty() { return contours; }
    let mut out = crate::clip::boolean(crate::clip::BoolKind::Union, &contours, &[]);
    out.retain(|p| p.len() >= 3);
    out.sort_by_key(|p| p.iter().map(|q| q[0]).min().unwrap_or(0));
    out
}

/// Outlines of `text` set in the font, with the first line's baseline starting at `origin` (mm, y up) and
/// further lines below it. Every contour becomes a closed shape of operation `op`.
pub fn text_loops(font: &[u8], face_index: u32, text: &str, style: TextStyle, origin: [f64; 2], op: i64) -> Result<Vec<LoopData>, String> {
    let face = Face::parse(font, face_index).map_err(|_| "This font cannot be read.".to_string())?;
    let upm = face.units_per_em() as f64;
    // measure the real height of a capital H: the cap-height field in fonts is often a little off
    let cap = face.glyph_index('H').and_then(|g| face.glyph_bounding_box(g)).map(|b| b.y_max as f64)
        .or_else(|| face.capital_height().map(|h| h as f64)).filter(|&h| h > 0.0).unwrap_or(upm * 0.7);
    let scale = style.size_mm / cap;
    let line_h = (face.ascender() as f64 - face.descender() as f64 + face.line_gap() as f64) * scale;
    let advance = |ch: char| -> f64 {
        let g = face.glyph_index(ch);
        g.and_then(|g| face.glyph_hor_advance(g)).map(|a| a as f64).unwrap_or(upm * 0.5) * scale + style.spacing_mm
    };
    let mut out = Vec::new();
    for (li, line) in text.lines().enumerate() {
        let mut contours: Vec<crate::clip::Path> = Vec::new();
        let width: f64 = line.chars().map(&advance).sum::<f64>() - if line.is_empty() { 0.0 } else { style.spacing_mm };
        let mut x = origin[0] - match style.align { Align::Left => 0.0, Align::Center => width / 2.0, Align::Right => width };
        let y = origin[1] - li as f64 * line_h;
        for ch in line.chars() {
            if let Some(g) = face.glyph_index(ch) {
                let mut b = Builder { scale, ox: x, oy: y, cur: vec![], contours: vec![] };
                if face.outline_glyph(g, &mut b).is_some() { b.finish(); }
                for c in b.contours {
                    contours.push(c.iter().map(|p| [(p[0] * S).round() as i64, (p[1] * S).round() as i64]).collect());
                }
            }
            x += advance(ch);
        }
        out.extend(merge_overlaps(contours).into_iter().map(|c| LoopData {
            c: 1, op, rc: None, cr: None, el: None, pt: None, grp: None, pts: c.iter().flat_map(|p| [p[0], p[1]]).collect(),
        }));
    }
    Ok(out)
}

/// A flat map of the plane: `x' = m[0] x + m[1] y + t[0]`, `y' = m[2] x + m[3] y + t[1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine { pub m: [f64; 4], pub t: [f64; 2] }

impl Affine {
    pub const ID: Affine = Affine { m: [1.0, 0.0, 0.0, 1.0], t: [0.0, 0.0] };
    pub fn det(&self) -> f64 { self.m[0] * self.m[3] - self.m[1] * self.m[2] }
    pub fn apply(&self, p: [f64; 2]) -> [f64; 2] { [self.m[0] * p[0] + self.m[1] * p[1] + self.t[0], self.m[2] * p[0] + self.m[3] * p[1] + self.t[1]] }

    /// Splits the linear part into a size factor, a turn (radians, counter-clockwise) and what is left
    /// (a stretch and maybe a flip), so that `m = s * Rot(turn) * rest`. A plain move, turn or uniform scale
    /// leaves `rest` as the identity.
    pub fn decompose(&self) -> (f64, f64, [f64; 4]) {
        let flip = self.det() < 0.0;
        let m = self.m;
        // undo a flip first (mirror the columns), so what is left can be read as a turn and a stretch
        let m1 = if flip { [m[0], -m[1], m[2], -m[3]] } else { m };
        let s = (m1[0] * m1[3] - m1[1] * m1[2]).abs().sqrt().max(1e-12);
        let turn = (m1[2] - m1[1]).atan2(m1[0] + m1[3]);
        let (sn, cs) = turn.sin_cos();
        // rest1 = Rot(-turn) * m1 / s
        let r1 = [(cs * m1[0] + sn * m1[2]) / s, (cs * m1[1] + sn * m1[3]) / s, (-sn * m1[0] + cs * m1[2]) / s, (-sn * m1[1] + cs * m1[3]) / s];
        // m = m1 * flip, so rest = rest1 * flip
        let rest = if flip { [r1[0], -r1[1], r1[2], -r1[3]] } else { r1 };
        (s, turn, rest)
    }

    /// The map `Rot(turn) * rest` followed by a move to `t`.
    pub fn compose(turn: f64, rest: [f64; 4], t: [f64; 2]) -> Affine {
        let (sn, cs) = turn.sin_cos();
        Affine { m: [cs * rest[0] - sn * rest[2], cs * rest[1] - sn * rest[3], sn * rest[0] + cs * rest[2], sn * rest[1] + cs * rest[3]], t }
    }
}

/// Moves shapes made at the origin to where the text sits. A map that flips the drawing makes closed shapes
/// run the other way round, so they are turned back, as mirroring does.
pub fn apply_affine(ls: &mut [LoopData], a: &Affine) {
    let flip = a.det() < 0.0;
    for l in ls.iter_mut() {
        let mut v: Vec<[i64; 2]> = l.pts.chunks_exact(2).map(|c| {
            let q = a.apply([c[0] as f64 / S, c[1] as f64 / S]);
            [(q[0] * S).round() as i64, (q[1] * S).round() as i64]
        }).collect();
        if flip && l.closed() && v.len() > 2 { v.reverse(); }
        l.pts = v.into_iter().flat_map(|p| p).collect();
        l.rc = None; l.el = None; l.cr = None;
    }
}

fn solve3(mut a: [[f64; 4]; 3]) -> Option<[f64; 3]> {
    for c in 0..3 {
        let p = (c..3).max_by(|&i, &j| a[i][c].abs().partial_cmp(&a[j][c].abs()).unwrap())?;
        if a[p][c].abs() < 1e-9 { return None; }
        a.swap(c, p);
        for r in 0..3 { if r != c { let f = a[r][c] / a[c][c]; for k in c..4 { a[r][k] -= f * a[c][k]; } } }
    }
    Some([a[0][3] / a[0][0], a[1][3] / a[1][1], a[2][3] / a[2][2]])
}

/// The map that takes the shapes `src` (the text as it was set, at the origin) to the shapes `now` (where
/// they are on the drawing), with the largest distance (mm) left between a mapped point and its partner.
/// `None` when the two do not correspond: a different number of shapes or points, which means the outlines
/// were edited. Shapes that were mirrored run the other way round, which is tried too.
pub fn fit_affine(src: &[LoopData], now: &[&LoopData]) -> Option<(Affine, f64)> {
    if src.len() != now.len() || src.is_empty() { return None; }
    if src.iter().zip(now).any(|(a, b)| a.pts.len() != b.pts.len() || a.closed() != b.closed()) { return None; }
    let mut best: Option<(Affine, f64)> = None;
    for reversed in [false, true] {
        let mut pairs: Vec<([f64; 2], [f64; 2])> = Vec::new();
        for (a, b) in src.iter().zip(now) {
            let s: Vec<[f64; 2]> = a.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect();
            let mut d: Vec<[f64; 2]> = b.pts.chunks_exact(2).map(|c| [c[0] as f64 / S, c[1] as f64 / S]).collect();
            if reversed && a.closed() { d.reverse(); }
            pairs.extend(s.into_iter().zip(d));
        }
        // least squares, x and y separately: [x y 1] * [m0 m1 t0]^T = x'
        let mut ata = [[0.0_f64; 3]; 3];
        let (mut bx, mut by) = ([0.0_f64; 3], [0.0_f64; 3]);
        for (p, q) in &pairs {
            let v = [p[0], p[1], 1.0];
            for i in 0..3 { for j in 0..3 { ata[i][j] += v[i] * v[j]; } for (b, t) in [(&mut bx, q[0]), (&mut by, q[1])] { b[i] += v[i] * t; } }
        }
        let row = |b: [f64; 3]| [[ata[0][0], ata[0][1], ata[0][2], b[0]], [ata[1][0], ata[1][1], ata[1][2], b[1]], [ata[2][0], ata[2][1], ata[2][2], b[2]]];
        let (Some(x), Some(y)) = (solve3(row(bx)), solve3(row(by))) else { continue };
        let af = Affine { m: [x[0], x[1], y[0], y[1]], t: [x[2], y[2]] };
        let res = pairs.iter().map(|(p, q)| { let r = af.apply(*p); (r[0] - q[0]).hypot(r[1] - q[1]) }).fold(0.0_f64, f64::max);
        if best.map_or(true, |(_, r)| res < r) { best = Some((af, res)); }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes;

    fn font() -> Option<Vec<u8>> {
        ["/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/Library/Fonts/Arial.ttf", "C:\\Windows\\Fonts\\arial.ttf"].iter().find_map(|p| std::fs::read(p).ok())
    }
    const ST: TextStyle = TextStyle { size_mm: 10.0, spacing_mm: 0.0, align: Align::Left };

    #[test]
    fn letters_have_the_right_size_and_holes() {
        let Some(f) = font() else { return }; // needs a system font; skipped where there is none
        let o = text_loops(&f, 0, "O", ST, [5.0, 20.0], 2).unwrap();
        assert_eq!(o.len(), 2, "an O is an outer contour and a hole");
        assert!(o.iter().all(|l| l.closed() && l.op == 2));
        let b = shapes::bbox_many(o.iter()).unwrap();
        // capital letters are about as tall as the requested height (round letters overshoot a little)
        assert!((b[3] - b[1] - 10.0).abs() < 1.0, "{b:?}");
        assert!(b[0] > 5.0 && b[1] < 20.0 + 0.2 && b[1] > 19.0);
        let i = text_loops(&f, 0, "I", ST, [0.0, 0.0], 1).unwrap();
        assert_eq!(i.len(), 1);
        let bi = shapes::bbox(&i[0]).unwrap();
        assert!((bi[3] - 10.0).abs() < 0.05 && bi[1].abs() < 0.05, "{bi:?}");
    }

    #[test]
    fn lines_alignment_and_spacing() {
        let Some(f) = font() else { return };
        let one = text_loops(&f, 0, "HH", ST, [0.0, 0.0], 1).unwrap();
        let wide = text_loops(&f, 0, "HH", TextStyle { spacing_mm: 3.0, ..ST }, [0.0, 0.0], 1).unwrap();
        let (b1, b2) = (shapes::bbox_many(one.iter()).unwrap(), shapes::bbox_many(wide.iter()).unwrap());
        assert!(((b2[2] - b2[0]) - (b1[2] - b1[0]) - 3.0).abs() < 0.05);
        // two lines: the second sits lower
        let two = text_loops(&f, 0, "H\nH", ST, [0.0, 0.0], 1).unwrap();
        assert_eq!(two.len(), 2);
        let (t, u) = (shapes::bbox(&two[0]).unwrap(), shapes::bbox(&two[1]).unwrap());
        assert!(u[3] < t[1], "second line below the first: {t:?} {u:?}");
        // centred text straddles the origin
        let c = text_loops(&f, 0, "HHHH", TextStyle { align: Align::Center, ..ST }, [0.0, 0.0], 1).unwrap();
        let bc = shapes::bbox_many(c.iter()).unwrap();
        assert!(bc[0] < -5.0 && bc[2] > 5.0);
        assert!(text_loops(b"not a font", 0, "x", ST, [0.0, 0.0], 1).is_err());
        assert!(text_loops(&f, 0, "   ", ST, [0.0, 0.0], 1).unwrap().is_empty());
    }

    #[test]
    fn overlapping_parts_become_one_outline() {
        // two squares of the same direction that overlap: one shape, no hole where they cross
        let a: crate::clip::Path = vec![[0, 0], [0, 10_000], [10_000, 10_000], [10_000, 0]];
        let b: crate::clip::Path = vec![[5_000, 5_000], [5_000, 15_000], [15_000, 15_000], [15_000, 5_000]];
        let m = merge_overlaps(vec![a.clone(), b]);
        assert_eq!(m.len(), 1);
        // a hole (opposite direction) inside stays a hole
        let hole: crate::clip::Path = a.iter().rev().map(|p| [p[0] / 2 + 2_000, p[1] / 2 + 2_000]).collect();
        let m = merge_overlaps(vec![a, hole]);
        assert_eq!(m.len(), 2);
    }

    fn plus() -> Vec<LoopData> {
        // an L-shaped letter and a square one: not symmetric, so a wrong match would show
        vec![
            crate::shapes::new_poly(&[[0.0, 0.0], [10.0, 0.0], [10.0, 3.0], [3.0, 3.0], [3.0, 12.0], [0.0, 12.0]], true, 0).unwrap(),
            crate::shapes::new_poly(&[[15.0, 0.0], [20.0, 0.0], [20.0, 5.0], [15.0, 5.0]], true, 0).unwrap(),
        ]
    }

    #[test]
    fn moved_turned_scaled_and_mirrored_shapes_are_found_again() {
        let src = plus();
        for (turn, k, flip) in [(0.0, 1.0, false), (0.6, 1.0, false), (-1.2, 2.5, false), (0.3, 0.4, true), (3.0, 1.7, true)] {
            let rest = if flip { [1.0, 0.0, 0.0, -1.0] } else { [1.0, 0.0, 0.0, 1.0] };
            let mut a = Affine::compose(turn, rest, [33.3, -7.1]);
            for v in a.m.iter_mut() { *v *= k; }
            let mut now = src.clone();
            apply_affine(&mut now, &a);
            let refs: Vec<&LoopData> = now.iter().collect();
            let (f, res) = fit_affine(&src, &refs).expect("fit");
            assert!(res < 0.01, "turn {turn} k {k} flip {flip}: residual {res}");
            let (s, t, rest2) = f.decompose();
            assert!((s - k).abs() < 1e-3, "size {s} vs {k}");
            let back = Affine::compose(t, rest2, f.t);
            for i in 0..4 { assert!((back.m[i] * s - f.m[i]).abs() < 1e-6, "{back:?} {f:?}"); }
        }
    }

    #[test]
    fn edited_outlines_do_not_fit() {
        let src = plus();
        let mut now = src.clone();
        now[0].pts.truncate(10); // a point less
        let refs: Vec<&LoopData> = now.iter().collect();
        assert!(fit_affine(&src, &refs).is_none());
        let mut now = src.clone();
        now[1].pts[2] += 2000; // one corner pulled out by 2 mm
        let refs: Vec<&LoopData> = now.iter().collect();
        assert!(fit_affine(&src, &refs).unwrap().1 > 0.1);
    }
}
