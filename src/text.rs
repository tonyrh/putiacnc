//! Text to outlines: the glyph contours of a font, flattened into closed polygons (one shape per contour,
//! so the holes of letters like O and A are separate shapes inside the outer one).
use crate::model::LoopData;
use crate::shapes::S;
use ttf_parser::{Face, OutlineBuilder};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
            c: 1, op, rc: None, cr: None, el: None, pt: None, pts: c.iter().flat_map(|p| [p[0], p[1]]).collect(),
        }));
    }
    Ok(out)
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
}
