//! Text to outlines: the glyph contours of a font, flattened into closed polygons (one shape per contour,
//! so the holes of letters like O and A are separate shapes inside the outer one).
use crate::model::LoopData;
use crate::shapes::S;
use ttf_parser::{Face, OutlineBuilder};

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Align { Left, Center, Right }

/// How the letters are laid out: along a straight baseline, around a circle (outside it, reading clockwise over the top,
/// or inside the bottom, reading left to right under it), or stacked top to bottom.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Flow { #[default] Straight, ArcTop, ArcBottom, Vertical, Arc }

#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    /// Height of capital letters, in mm.
    pub size_mm: f64,
    /// Extra space added after every letter, in mm.
    pub spacing_mm: f64,
    /// Extra space added after every space character, in mm (on top of the letter spacing).
    pub word_spacing_mm: f64,
    /// Move pairs of letters (AV, To...) together or apart as the font says.
    pub kerning: bool,
    pub align: Align,
    pub flow: Flow,
    /// Radius of the circle the baseline follows, mm (arc flows only).
    pub radius_mm: f64,
    /// Flow::Arc: where on the circle the text sits (degrees, 90 = top, 270 = bottom), and whether the letters stand towards the centre.
    pub arc_deg: f64,
    pub arc_in: bool,
    /// A second copy of the letters, moved by this much (mm); [0, 0] means none.
    pub shadow: [f64; 2],
}

impl TextStyle {
    pub fn plain(size_mm: f64, spacing_mm: f64, align: Align) -> Self { TextStyle { size_mm, spacing_mm, word_spacing_mm: 0.0, kerning: false, align, flow: Flow::Straight, radius_mm: 0.0, arc_deg: 90.0, arc_in: false, shadow: [0.0, 0.0] } }
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

/// How far the font moves `right` towards (negative) or away from (positive) `left`, in font units: the pair adjustments of the
/// `kern` feature of the GPOS table when there are any, else the old `kern` table.
pub fn pair_kerning(face: &Face, left: ttf_parser::GlyphId, right: ttf_parser::GlyphId) -> f64 {
    use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
    let tables = face.tables();
    if let Some(gpos) = tables.gpos {
        let mut lookups: Vec<u16> = Vec::new();
        for i in 0..gpos.features.len() {
            let Some(feat) = gpos.features.get(i) else { continue };
            if feat.tag != ttf_parser::Tag::from_bytes(b"kern") { continue; }
            for l in feat.lookup_indices { if !lookups.contains(&l) { lookups.push(l); } }
        }
        if !lookups.is_empty() {
            let mut total = 0.0;
            for li in lookups {
                let Some(lookup) = gpos.lookups.get(li) else { continue };
                // the first subtable of a lookup that knows the pair decides
                for st in lookup.subtables.into_iter::<PositioningSubtable>() {
                    let PositioningSubtable::Pair(pa) = st else { continue };
                    let value = match pa {
                        PairAdjustment::Format1 { coverage, sets } => coverage.get(left).and_then(|i| sets.get(i)).and_then(|set| set.get(right)),
                        PairAdjustment::Format2 { coverage, classes, matrix } => {
                            if coverage.contains(left) { matrix.get((classes.0.get(left), classes.1.get(right))) } else { None }
                        }
                    };
                    if let Some((v1, _)) = value { total += v1.x_advance as f64; break; }
                }
            }
            return total;
        }
    }
    let mut total = 0.0;
    if let Some(kern) = tables.kern {
        for st in kern.subtables {
            if st.horizontal && !st.variable { if let Some(k) = st.glyphs_kerning(left, right) { total += k as f64; } }
        }
    }
    total
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
        g.and_then(|g| face.glyph_hor_advance(g)).map(|a| a as f64).unwrap_or(upm * 0.5) * scale + style.spacing_mm + if ch == ' ' { style.word_spacing_mm } else { 0.0 }
    };
    // the font's adjustment for a pair of letters, mm (0 when kerning is off or the font has none)
    let kern = |a: char, b: char| -> f64 {
        if !style.kerning { return 0.0; }
        match (face.glyph_index(a), face.glyph_index(b)) { (Some(ga), Some(gb)) => pair_kerning(&face, ga, gb) * scale, _ => 0.0 }
    };
    let mk = |c: &[[i64; 2]]| LoopData {
        c: 1, op, rc: None, cr: None, el: None, pt: None, grp: None, ug: None, pts: c.iter().flat_map(|p| [p[0], p[1]]).collect(),
    };
    let glyph = |ch: char, x: f64, y: f64, contours: &mut Vec<crate::clip::Path>| {
        if let Some(g) = face.glyph_index(ch) {
            let mut b = Builder { scale, ox: x, oy: y, cur: vec![], contours: vec![] };
            if face.outline_glyph(g, &mut b).is_some() { b.finish(); }
            for c in b.contours {
                contours.push(c.iter().map(|p| [(p[0] * S).round() as i64, (p[1] * S).round() as i64]).collect());
            }
        }
    };
    let mut out: Vec<LoopData> = Vec::new();
    if style.flow == Flow::Vertical {
        // one column per line of text, the letters centred on the column and stacked downwards
        let step = style.size_mm * 1.25 + style.spacing_mm;
        let col_w = text.lines().flat_map(|l| l.chars()).map(|c| advance(c) - style.spacing_mm).fold(0.0_f64, f64::max) + style.size_mm * 0.4;
        for (ci, line) in text.lines().enumerate() {
            let mut contours: Vec<crate::clip::Path> = Vec::new();
            let cx = origin[0] + ci as f64 * col_w;
            for (k, ch) in line.chars().enumerate() {
                if ch == ' ' { continue; }
                glyph(ch, cx - (advance(ch) - style.spacing_mm) / 2.0, origin[1] - k as f64 * step, &mut contours);
            }
            out.extend(merge_overlaps(contours).iter().map(|c| mk(c)));
        }
    } else {
        for (li, line) in text.lines().enumerate() {
            let mut contours: Vec<crate::clip::Path> = Vec::new();
            // where each letter starts along the baseline, with the font's pair adjustments
            let chars: Vec<char> = line.chars().collect();
            let mut xs: Vec<f64> = Vec::with_capacity(chars.len());
            let mut run = 0.0;
            for (i, &ch) in chars.iter().enumerate() {
                xs.push(run);
                run += advance(ch);
                if let Some(&next) = chars.get(i + 1) { run += kern(ch, next); }
            }
            let width: f64 = run - if chars.is_empty() { 0.0 } else { style.spacing_mm };
            let x0 = origin[0] - match style.align { Align::Left => 0.0, Align::Center => width / 2.0, Align::Right => width };
            let y = origin[1] - li as f64 * line_h;
            for (i, &ch) in chars.iter().enumerate() { glyph(ch, x0 + xs[i], y, &mut contours); }
            let merged = merge_overlaps(contours);
            if matches!(style.flow, Flow::ArcTop | Flow::ArcBottom | Flow::Arc) {
                let r = style.radius_mm.max(style.size_mm);
                let (deg, inside) = match style.flow { Flow::ArcTop => (90.0, false), Flow::ArcBottom => (270.0, true), _ => (style.arc_deg, style.arc_in) };
                out.extend(merged.iter().map(|c| mk(&bend(c, origin, r, deg.to_radians(), inside))));
            } else {
                out.extend(merged.iter().map(|c| mk(c)));
            }
        }
    }
    if style.shadow != [0.0, 0.0] {
        let (dx, dy) = ((style.shadow[0] * S).round() as i64, (style.shadow[1] * S).round() as i64);
        let mut sh: Vec<LoopData> = out.iter().map(|l| LoopData { pts: l.pts.chunks(2).flat_map(|p| [p[0] + dx, p[1] + dy]).collect(), ..l.clone() }).collect();
        sh.extend(out);
        out = sh;
    }
    Ok(out)
}

/// A small grey-scale picture of `text` set in the font, for showing a font's name in its own letters. Returns (width, height,
/// coverage 0..255 per pixel, row by row from the top), or None when the font cannot draw most of the letters. The letters are
/// `px` pixels per em unless the font's line is so tall that it would not fit in `max_h` pixels, or the name so long that it would not
/// fit in `max_w`: then they are smaller, down to `min_px`, below which None is returned (nothing is cut off).
pub fn name_bitmap(font: &[u8], face_index: u32, text: &str, px: f64, min_px: f64, max_w: usize, max_h: usize) -> Option<(usize, usize, Vec<u8>)> {
    let face = Face::parse(font, face_index).ok()?;
    let upm = face.units_per_em() as f64;
    let (asc, desc) = (face.ascender() as f64, face.descender() as f64);
    let line = (asc - desc).max(upm * 0.5);
    let mut scale = (px / upm).min((max_h as f64 - 2.0) / line);
    let total: f64 = text.chars().map(|ch| face.glyph_index(ch).and_then(|g| face.glyph_hor_advance(g)).map(|a| a as f64).unwrap_or(upm * 0.5)).sum();
    if total * scale > max_w as f64 - 2.0 { scale = (max_w as f64 - 2.0) / total; }
    if scale * upm < min_px { return None; }
    let h = ((line * scale).ceil() as usize + 2).min(max_h);
    let base = h as f64 - 1.0 + desc * scale; // baseline, from the top of the picture
    let mut x = 1.0;
    let mut contours: Vec<Vec<[f64; 2]>> = Vec::new();
    let (mut drawn, mut missing) = (0, 0);
    for ch in text.chars() {
        let g = face.glyph_index(ch);
        if ch != ' ' { if g.is_some() { drawn += 1 } else { missing += 1 } }
        if let Some(g) = g {
            let mut b = Builder { scale, ox: x, oy: 0.0, cur: vec![], contours: vec![] };
            if face.outline_glyph(g, &mut b).is_some() { b.finish(); }
            // outlines are y up from the baseline; the picture is y down
            contours.extend(b.contours.into_iter().map(|c| c.into_iter().map(|p| [p[0], base - p[1]]).collect::<Vec<_>>()));
        }
        x += g.and_then(|g| face.glyph_hor_advance(g)).map(|a| a as f64).unwrap_or(upm * 0.5) * scale;
    }
    if drawn == 0 || missing > drawn { return None; }
    let w = ((x + 1.0).ceil() as usize).min(max_w).max(2);
    const SS: usize = 4;
    let mut acc = vec![0u16; w * h];
    for sy in 0..h * SS {
        let y = (sy as f64 + 0.5) / SS as f64;
        let mut cross: Vec<(f64, i32)> = Vec::new();
        for c in &contours {
            for i in 0..c.len() {
                let (p, q) = (c[i], c[(i + 1) % c.len()]);
                if (p[1] <= y) == (q[1] <= y) { continue; }
                cross.push((p[0] + (y - p[1]) / (q[1] - p[1]) * (q[0] - p[0]), if q[1] > p[1] { 1 } else { -1 }));
            }
        }
        cross.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut wind = 0;
        for k in 0..cross.len().saturating_sub(1) {
            wind += cross[k].1;
            if wind == 0 { continue; }
            let from = ((cross[k].0 * SS as f64 - 0.5).ceil().max(0.0)) as usize;
            let to = ((cross[k + 1].0 * SS as f64 - 0.5).ceil().max(0.0) as usize).min(w * SS);
            for sx in from..to { acc[(sy / SS) * w + sx / SS] += 1; }
        }
    }
    Some((w, h, acc.into_iter().map(|a| ((a as u32 * 255 / (SS * SS) as u32).min(255)) as u8).collect()))
}

/// Wraps a straight-set outline round a circle of radius `r` mm. `origin` is where the baseline touches the circle, at
/// angle `phi` (radians, 0 = right, counter-clockwise, so pi/2 is the top). x along the baseline becomes distance along the
/// circle and height above the baseline becomes distance from it: outwards (reading clockwise) or, with `inside`, towards the
/// centre (reading counter-clockwise). Long edges are cut first so they bend too.
fn bend(c: &[[i64; 2]], origin: [f64; 2], r: f64, phi: f64, inside: bool) -> Vec<[i64; 2]> {
    let pts: Vec<[f64; 2]> = c.iter().map(|p| [p[0] as f64 / S, p[1] as f64 / S]).collect();
    let mut dense: Vec<[f64; 2]> = Vec::with_capacity(pts.len());
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let n = ((b[0] - a[0]).hypot(b[1] - a[1]) / 0.3).ceil().max(1.0) as usize;
        for k in 0..n { let t = k as f64 / n as f64; dense.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]); }
    }
    let centre = [origin[0] - r * phi.cos(), origin[1] - r * phi.sin()];
    dense.iter().map(|p| {
        let (s, h) = (p[0] - origin[0], p[1] - origin[1]);
        let (th, rad) = if inside { (phi + s / r, r - h) } else { (phi - s / r, r + h) };
        let q = [centre[0] + rad * th.cos(), centre[1] + rad * th.sin()];
        [(q[0] * S).round() as i64, (q[1] * S).round() as i64]
    }).collect()
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
        l.pts = v.into_iter().flatten().collect();
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
        if best.is_none_or(|(_, r)| res < r) { best = Some((af, res)); }
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
    const ST: TextStyle = TextStyle { size_mm: 10.0, spacing_mm: 0.0, word_spacing_mm: 0.0, kerning: false, align: Align::Left, flow: Flow::Straight, radius_mm: 0.0, arc_deg: 90.0, arc_in: false, shadow: [0.0, 0.0] };

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

    #[test]
    fn a_font_name_can_be_drawn_in_its_own_letters() {
        let Some(f) = font() else { return };
        let (w, h, px) = name_bitmap(&f, 0, "Hello", 20.0, 8.0, 300, 30).unwrap();
        assert_eq!(px.len(), w * h);
        assert!(w > 30 && w < 120 && h <= 30, "{w}x{h}");
        let ink = px.iter().filter(|&&v| v > 128).count();
        assert!(ink > 100 && ink < w * h / 2, "{ink}");
        assert!(px.iter().any(|&v| v > 0 && v < 255), "edges are smooth");
        // a long name is made smaller to fit, never cut off; when that would make it too small there is no picture
        let long = "DejaVu Serif Bold Condensed Italic Extra";
        let (w, _, px) = name_bitmap(&f, 0, long, 20.0, 8.0, 200, 30).unwrap();
        assert!(w <= 200, "{w}");
        assert!(px[..].chunks(w).all(|r| r[w - 1] == 0), "the last column is empty: nothing runs off the edge");
        assert!(name_bitmap(&f, 0, long, 20.0, 18.0, 200, 30).is_none(), "too small to read: no picture");
        assert!(name_bitmap(&f, 0, "\u{4e2d}\u{6587}", 20.0, 8.0, 300, 30).is_none(), "letters the font lacks are not faked");
    }

    #[test]
    fn arc_text_follows_the_circle_and_vertical_text_stacks() {
        let Some(f) = font() else { return };
        // on top: the baseline touches the circle at the origin, the letters stand outwards, so every point lies
        // between the baseline circle and one text height outside it
        let r = 50.0;
        let top = text_loops(&f, 0, "HELLO", TextStyle { flow: Flow::ArcTop, radius_mm: r, align: Align::Center, ..ST }, [0.0, 0.0], 1).unwrap();
        let c = [0.0, -r];
        for l in &top { for p in l.pts.chunks(2) {
            let d = (p[0] as f64 / S - c[0]).hypot(p[1] as f64 / S - c[1]);
            assert!(d > r - 0.3 && d < r + 10.5, "{d}");
        } }
        // underneath: the letters stand towards the centre
        let bot = text_loops(&f, 0, "HELLO", TextStyle { flow: Flow::ArcBottom, radius_mm: r, align: Align::Center, ..ST }, [0.0, 0.0], 1).unwrap();
        let c = [0.0, r];
        for l in &bot { for p in l.pts.chunks(2) {
            let d = (p[0] as f64 / S - c[0]).hypot(p[1] as f64 / S - c[1]);
            assert!(d < r + 0.3 && d > r - 10.5, "{d}");
        } }
        // a quarter turn round: at 0 degrees (the right) the letters stand outwards, so they lie right of the circle's centre
        let side = text_loops(&f, 0, "HI", TextStyle { flow: Flow::Arc, radius_mm: r, arc_deg: 0.0, align: Align::Center, ..ST }, [0.0, 0.0], 1).unwrap();
        let c = [-r, 0.0];
        for l in &side { for p in l.pts.chunks(2) {
            let d = (p[0] as f64 / S - c[0]).hypot(p[1] as f64 / S - c[1]);
            assert!(d > r - 0.3 && d < r + 10.5, "{d}");
        } }
        let v = text_loops(&f, 0, "ABC", TextStyle { flow: Flow::Vertical, ..ST }, [0.0, 0.0], 1).unwrap();
        let b = shapes::bbox_many(v.iter()).unwrap();
        assert!(b[3] - b[1] > 2.0 * (b[2] - b[0]), "three stacked letters are taller than wide: {b:?}");
    }

    #[test]
    fn shadow_is_a_moved_copy() {
        let Some(f) = font() else { return };
        let plain = text_loops(&f, 0, "AB", ST, [0.0, 0.0], 1).unwrap();
        let sh = text_loops(&f, 0, "AB", TextStyle { shadow: [1.5, -2.0], ..ST }, [0.0, 0.0], 1).unwrap();
        assert_eq!(sh.len(), plain.len() * 2);
        let (a, b) = (shapes::bbox_many(sh[..plain.len()].iter()).unwrap(), shapes::bbox_many(plain.iter()).unwrap());
        assert!((a[0] - b[0] - 1.5).abs() < 0.01 && (a[1] - b[1] + 2.0).abs() < 0.01);
        assert_eq!(shapes::bbox_many(sh[plain.len()..].iter()).unwrap(), b);
    }

    /// The width of the ink of `text` in mm.
    fn ink_width(f: &[u8], text: &str, st: TextStyle) -> f64 {
        let l = text_loops(f, 0, text, st, [0.0, 0.0], 1).unwrap();
        let b = shapes::bbox_many(l.iter()).unwrap();
        b[2] - b[0]
    }

    #[test]
    fn kerning_pulls_a_pair_like_av_together_and_leaves_plain_pairs_alone() {
        let Some(f) = font() else { return };
        let on = TextStyle { kerning: true, ..ST };
        assert!(ink_width(&f, "AV", on) < ink_width(&f, "AV", ST) - 0.3, "AV is kerned");
        assert!((ink_width(&f, "HH", on) - ink_width(&f, "HH", ST)).abs() < 0.2, "HH is not");
    }

    #[test]
    fn word_spacing_widens_only_the_gaps_between_words() {
        let Some(f) = font() else { return };
        let wide = TextStyle { word_spacing_mm: 4.0, ..ST };
        assert!((ink_width(&f, "A B", wide) - ink_width(&f, "A B", ST) - 4.0).abs() < 0.05);
        assert!((ink_width(&f, "AB", wide) - ink_width(&f, "AB", ST)).abs() < 1e-6);
    }
}
