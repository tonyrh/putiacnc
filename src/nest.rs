//! Nesting: packing rectangles (the bounding boxes of parts) onto a sheet in rows ("shelves"), optionally turning parts a quarter turn.
//! Simple and predictable rather than optimal: the parts are sorted tallest first, filled in rows from the lower left, and when turning is
//! allowed the better of three ways of orienting them (as drawn, all lying flat, all standing up) is kept.

/// Where one part goes: `x` and `y` are the lower left corner of its box on the sheet (after turning, if `turned`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed { pub item: usize, pub x: f64, pub y: f64, pub turned: bool }

/// The result: where each part that fits goes, and the parts that do not fit (indices into the input).
#[derive(Debug, Clone, Default)]
pub struct Nesting { pub placed: Vec<Placed>, pub left_out: Vec<usize> }

fn shelves(dims: &[(f64, f64)], turned: &[bool], sheet: [f64; 4], gap: f64, blocked: &[[f64; 4]]) -> Nesting {
    let size = |i: usize| if turned[i] { (dims[i].1, dims[i].0) } else { dims[i] };
    let mut order: Vec<usize> = (0..dims.len()).collect();
    // tallest first, then widest, then as given (so the result is the same every time)
    order.sort_by(|&a, &b| size(b).1.total_cmp(&size(a).1).then(size(b).0.total_cmp(&size(a).0)).then(a.cmp(&b)));
    let (mut x, mut y, mut row_h) = (sheet[0], sheet[1], 0.0_f64);
    let mut out = Nesting::default();
    for i in order {
        let (w, h) = size(i);
        if w > sheet[2] - sheet[0] + 1e-9 || h > sheet[3] - sheet[1] + 1e-9 { out.left_out.push(i); continue; }
        // walk along the row, past anything in the way, and on to the next row when the row runs out
        loop {
            if x + w > sheet[2] + 1e-9 {
                if row_h > 0.0 { y += row_h + gap; row_h = 0.0; }
                else {
                    // nothing was placed in this row: the next row starts above whatever blocked it
                    let up = blocked.iter().filter(|o| o[1] < y + h + gap && o[3] + gap > y).map(|o| o[3] + gap).fold(f64::INFINITY, f64::min);
                    y = if up.is_finite() { up } else { y + gap.max(1.0) };
                }
                x = sheet[0];
            }
            if y + h > sheet[3] + 1e-9 { break; }
            let hit = blocked.iter().find(|o| x < o[2] + gap - 1e-9 && o[0] < x + w + gap - 1e-9 && y < o[3] + gap - 1e-9 && o[1] < y + h + gap - 1e-9);
            match hit { Some(o) => x = o[2] + gap, None => break }
        }
        if y + h > sheet[3] + 1e-9 { out.left_out.push(i); continue; }
        out.placed.push(Placed { item: i, x, y, turned: turned[i] });
        x += w + gap;
        row_h = row_h.max(h);
    }
    out.left_out.sort_unstable();
    out
}

/// Packs boxes `(width, height)` onto `sheet` (`[x0, y0, x1, y1]`, mm) with `gap` mm between neighbouring boxes, keeping `gap` clear of
/// every rectangle in `blocked` (`[x0, y0, x1, y1]`, the clamps).
pub fn nest(items: &[(f64, f64)], sheet: [f64; 4], gap: f64, rotate: bool, blocked: &[[f64; 4]]) -> Nesting {
    let n = items.len();
    let mut tries: Vec<Vec<bool>> = vec![vec![false; n]];
    if rotate {
        tries.push(items.iter().map(|&(w, h)| h > w).collect()); // lying flat: wider than tall
        tries.push(items.iter().map(|&(w, h)| w > h).collect()); // standing up: taller than wide
    }
    let gap = gap.max(0.0);
    let mut best: Option<(Nesting, f64)> = None;
    for t in &tries {
        let r = shelves(items, t, sheet, gap, blocked);
        // the most parts first, then the least sheet used up the side
        let top = r.placed.iter().map(|p| { let (w, h) = if p.turned { (items[p.item].1, items[p.item].0) } else { items[p.item] }; let _ = w; p.y + h }).fold(sheet[1], f64::max);
        let better = match &best { None => true, Some((b, bt)) => r.placed.len() > b.placed.len() || (r.placed.len() == b.placed.len() && top < *bt - 1e-9) };
        if better { best = Some((r, top)); }
    }
    best.map(|b| b.0).unwrap_or_default()
}

// ---- Nesting by the true shape ----
// The sheet is a grid of small cells. Each part, in each turn allowed, is drawn on the grid and tried at every spot from the lower left
// upwards; the first spot where it touches nothing is its place, and the turn that reaches the lowest wins. What is already placed (and
// the clamps) is drawn wider by the gap, and a part counts every cell it touches, so the gap is kept (a little more, never less).

use crate::clip;

/// One closed outline, in mm.
pub type Poly = Vec<[f64; 2]>;

/// One part: its outer outlines, the holes cut right through it (other parts may go into those) and anything standing inside a hole.
#[derive(Debug, Clone, Default)]
pub struct Part { pub outline: Vec<Poly>, pub holes: Vec<Poly>, pub islands: Vec<Poly> }

impl Part {
    /// A part without usable holes.
    pub fn solid(outline: Vec<Poly>) -> Part { Part { outline, holes: vec![], islands: vec![] } }
    fn map(&self, f: impl Fn(&[Poly]) -> Vec<Poly>) -> Part { Part { outline: f(&self.outline), holes: f(&self.holes), islands: f(&self.islands) } }
}

/// Where a part goes: turn it `angle` degrees (counter-clockwise, about the origin), then put the lower left corner of its box at `x`, `y`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fitted { pub item: usize, pub angle: f64, pub x: f64, pub y: f64 }

#[derive(Debug, Clone, Default)]
pub struct Fitting { pub placed: Vec<Fitted>, pub left_out: Vec<usize> }

struct Bits { w: usize, h: usize, wpr: usize, rows: Vec<u64> }

impl Bits {
    fn new(w: usize, h: usize) -> Bits { let wpr = w.div_ceil(64).max(1); Bits { w, h, wpr, rows: vec![0; wpr * h] } }
    fn set(&mut self, x: i64, y: i64) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h { self.rows[y as usize * self.wpr + x as usize / 64] |= 1 << (x as usize % 64); }
    }
    fn row(&self, y: usize) -> &[u64] { &self.rows[y * self.wpr..(y + 1) * self.wpr] }
    fn count(&self) -> usize { self.rows.iter().map(|w| w.count_ones() as usize).sum() }
}

/// Marks every cell the outline touches or encloses (`poly` in cell units).
fn fill(bits: &mut Bits, poly: &[[f64; 2]]) {
    let n = poly.len();
    if n < 2 { return; }
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) * 4.0).ceil().max(1.0) as usize;
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            bits.set((a[0] + (b[0] - a[0]) * t).floor() as i64, (a[1] + (b[1] - a[1]) * t).floor() as i64);
        }
    }
    if n < 3 { return; }
    let mut xs: Vec<f64> = vec![];
    for row in 0..bits.h {
        let yc = row as f64 + 0.5;
        xs.clear();
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            if (a[1] <= yc) != (b[1] <= yc) { xs.push(a[0] + (yc - a[1]) / (b[1] - a[1]) * (b[0] - a[0])); }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks_exact(2) {
            let (from, to) = ((pair[0] - 0.5).ceil() as i64, (pair[1] - 0.5).floor() as i64);
            for x in from..=to { bits.set(x, row as i64); }
        }
    }
}

/// Clears the cells whose middle is inside the outline (`poly` in cell units).
fn clear_inside(bits: &mut Bits, poly: &[[f64; 2]]) {
    let n = poly.len();
    if n < 3 { return; }
    let mut xs: Vec<f64> = vec![];
    for row in 0..bits.h {
        let yc = row as f64 + 0.5;
        xs.clear();
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            if (a[1] <= yc) != (b[1] <= yc) { xs.push(a[0] + (yc - a[1]) / (b[1] - a[1]) * (b[0] - a[0])); }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks_exact(2) {
            for x in ((pair[0] - 0.5).ceil() as i64)..=((pair[1] - 0.5).floor() as i64) {
                if x >= 0 && (x as usize) < bits.w { bits.rows[row * bits.wpr + x as usize / 64] &= !(1 << (x as usize % 64)); }
            }
        }
    }
}

fn turned(polys: &[Poly], deg: f64) -> Vec<Poly> {
    let (sn, cs) = deg.to_radians().sin_cos();
    polys.iter().map(|p| p.iter().map(|q| [q[0] * cs - q[1] * sn, q[0] * sn + q[1] * cs]).collect()).collect()
}

fn bounds(polys: &[Poly]) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for q in polys.iter().flatten() {
        b = Some(match b { None => [q[0], q[1], q[0], q[1]], Some(a) => [a[0].min(q[0]), a[1].min(q[1]), a[2].max(q[0]), a[3].max(q[1])] });
    }
    b
}

/// The outlines wider by `by` mm (round corners), as polygons in mm.
fn widened(polys: &[Poly], by: f64) -> Vec<Poly> {
    let paths: Vec<clip::Path> = polys.iter().filter(|p| p.len() > 2).map(|p| {
        let mut v: clip::Path = p.iter().map(|q| [(q[0] * 1000.0).round() as i64, (q[1] * 1000.0).round() as i64]).collect();
        if clip::area(&v) < 0.0 { v.reverse(); }
        v
    }).collect();
    clip::offset(&paths, by * 1000.0, (by * 1000.0 * 0.05).max(40.0)).iter().map(|p| p.iter().map(|q| [q[0] as f64 / 1000.0, q[1] as f64 / 1000.0]).collect()).collect()
}

fn fits(occ: &Bits, mask: &Bits, ix: usize, iy: usize) -> bool {
    let (base, sh) = (ix / 64, ix % 64);
    for r in 0..mask.h {
        let (o, m) = (occ.row(iy + r), mask.row(r));
        for (k, &mw) in m.iter().enumerate() {
            if mw == 0 { continue; }
            if o.get(base + k).is_none_or(|ow| ow & (mw << sh) != 0) { return false; }
            if sh > 0 {
                let hi = mw >> (64 - sh);
                if hi != 0 && o.get(base + k + 1).is_none_or(|ow| ow & hi != 0) { return false; }
            }
        }
    }
    true
}

/// Packs parts by their outlines (`parts[i]` = the closed outlines of part `i`, mm) onto `sheet` (`[x0, y0, x1, y1]`), `gap` mm apart and
/// `gap` mm from every rectangle in `blocked`, trying each of `angles` (degrees) for every part. Parts go largest first.
pub fn nest_shapes(parts: &[Part], sheet: [f64; 4], gap: f64, angles: &[f64], blocked: &[[f64; 4]]) -> Fitting {
    let (sw, sh) = (sheet[2] - sheet[0], sheet[3] - sheet[1]);
    let mut out = Fitting::default();
    if sw <= 0.0 || sh <= 0.0 { out.left_out = (0..parts.len()).collect(); return out; }
    let gap = gap.max(0.0);
    let cell = (sw * sh / 300_000.0).sqrt().max(0.5);
    let pad = gap + 0.75 * cell;
    let (nx, ny) = ((sw / cell).floor() as usize, (sh / cell).floor() as usize);
    let mut occ = Bits::new(nx.max(1), ny.max(1));
    let to_cells = |polys: &[Poly], ox: f64, oy: f64| -> Vec<Poly> { polys.iter().map(|p| p.iter().map(|q| [(q[0] - ox) / cell, (q[1] - oy) / cell]).collect()).collect() };
    for b in blocked {
        let r: Poly = vec![[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]];
        for w in to_cells(&widened(&[r], pad), sheet[0], sheet[1]) { fill(&mut occ, &w); }
    }
    let area = |polys: &[Poly]| -> f64 { polys.iter().map(|p| { let n = p.len(); (0..n).map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1]).sum::<f64>().abs() / 2.0 }).sum() };
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by(|&a, &b| area(&parts[b].outline).total_cmp(&area(&parts[a].outline)).then(a.cmp(&b)));
    let angles: Vec<f64> = if angles.is_empty() { vec![0.0] } else { angles.to_vec() };
    for i in order {
        // (height reached, x cell, y cell, angle, the part placed at the origin)
        let mut best: Option<(usize, usize, usize, f64, Part)> = None;
        for &deg in &angles {
            let rot = parts[i].map(|ps| turned(ps, deg));
            let Some(b) = bounds(&rot.outline) else { continue };
            let at0 = rot.map(|ps| ps.iter().map(|p| p.iter().map(|q| [q[0] - b[0], q[1] - b[1]]).collect()).collect());
            let (mw, mh) = (((b[2] - b[0]) / cell).floor() as usize + 2, ((b[3] - b[1]) / cell).floor() as usize + 2);
            if mw > occ.w || mh > occ.h { continue; }
            let mut mask = Bits::new(mw, mh);
            for p in to_cells(&at0.outline, 0.0, 0.0) { fill(&mut mask, &p); }
            if mask.count() == 0 { continue; }
            let limit = best.as_ref().map_or(usize::MAX, |b| b.0);
            'scan: for iy in 0..=(occ.h - mh) {
                if iy + mh > limit { break; }
                for ix in 0..=(occ.w - mw) {
                    if fits(&occ, &mask, ix, iy) {
                        let key = (iy + mh, ix);
                        if best.as_ref().is_none_or(|b| key < (b.0, b.1)) { best = Some((iy + mh, ix, iy, deg, at0)); }
                        break 'scan;
                    }
                }
            }
        }
        let Some((_, ix, iy, deg, at0)) = best else { out.left_out.push(i); continue };
        let (x, y) = (sheet[0] + ix as f64 * cell, sheet[1] + iy as f64 * cell);
        let placed = at0.map(|ps| ps.iter().map(|p| p.iter().map(|q| [q[0] + x, q[1] + y]).collect()).collect());
        for w in to_cells(&widened(&placed.outline, pad), sheet[0], sheet[1]) { fill(&mut occ, &w); }
        // the room inside a hole: the cells that lie wholly inside it, kept the gap away from its wall, are free again (nothing was
        // there when the part went down, as the part's footprint was free), and what stands inside the hole is drawn back
        if !placed.holes.is_empty() {
            let room = widened(&placed.holes, -(pad + 0.75 * cell));
            for w in to_cells(&room, sheet[0], sheet[1]) { clear_inside(&mut occ, &w); }
            for w in to_cells(&widened(&placed.islands, pad), sheet[0], sheet[1]) { fill(&mut occ, &w); }
        }
        out.placed.push(Fitted { item: i, angle: deg, x, y });
    }
    out.left_out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(items: &[(f64, f64)], r: &Nesting, gap: f64) -> bool {
        let b = |p: &Placed| { let (w, h) = if p.turned { (items[p.item].1, items[p.item].0) } else { items[p.item] }; [p.x, p.y, p.x + w, p.y + h] };
        r.placed.iter().enumerate().any(|(i, p)| r.placed.iter().skip(i + 1).any(|q| {
            let (a, c) = (b(p), b(q));
            a[0] < c[2] + gap - 1e-9 && c[0] < a[2] + gap - 1e-9 && a[1] < c[3] + gap - 1e-9 && c[1] < a[3] + gap - 1e-9
        }))
    }

    #[test]
    fn parts_land_in_rows_inside_the_sheet_with_the_gap_between_them() {
        let items = [(40.0, 20.0), (30.0, 20.0), (50.0, 10.0), (20.0, 20.0), (60.0, 30.0)];
        let r = nest(&items, [0.0, 0.0, 100.0, 100.0], 5.0, false, &[]);
        assert!(r.left_out.is_empty() && r.placed.len() == 5, "{r:?}");
        assert!(!overlaps(&items, &r, 5.0));
        assert!(r.placed.iter().all(|p| p.x >= 0.0 && p.y >= 0.0 && p.x + items[p.item].0 <= 100.0 + 1e-9 && p.y + items[p.item].1 <= 100.0 + 1e-9));
        // the tallest part goes first, in the lower left corner
        assert_eq!((r.placed[0].item, r.placed[0].x, r.placed[0].y), (4, 0.0, 0.0));
    }

    #[test]
    fn what_does_not_fit_is_left_out_not_squeezed_in() {
        let items = [(60.0, 60.0), (60.0, 60.0), (200.0, 10.0)];
        let r = nest(&items, [0.0, 0.0, 100.0, 100.0], 2.0, false, &[]);
        assert_eq!(r.left_out, vec![1, 2], "{r:?}"); // the second does not fit beside or above the first; the third is wider than the sheet
        assert_eq!(r.placed.len(), 1);
    }

    #[test]
    fn turning_a_part_makes_room_for_more() {
        // tall strips on a wide, short sheet only fit lying down
        let items = [(10.0, 90.0), (10.0, 90.0), (10.0, 90.0)];
        let plain = nest(&items, [0.0, 0.0, 100.0, 40.0], 2.0, false, &[]);
        let turned = nest(&items, [0.0, 0.0, 100.0, 40.0], 2.0, true, &[]);
        assert!(plain.placed.is_empty(), "{plain:?}");
        assert_eq!(turned.placed.len(), 3, "{turned:?}");
        assert!(turned.placed.iter().all(|p| p.turned));
        assert!(!overlaps(&items, &turned, 2.0));
    }

    #[test]
    fn the_same_input_always_gives_the_same_layout() {
        let items: Vec<(f64, f64)> = (0..12).map(|i| (10.0 + (i % 5) as f64 * 7.0, 8.0 + (i % 3) as f64 * 9.0)).collect();
        let a = nest(&items, [5.0, 5.0, 195.0, 145.0], 3.0, true, &[]);
        let b = nest(&items, [5.0, 5.0, 195.0, 145.0], 3.0, true, &[]);
        assert_eq!(a.placed, b.placed);
        assert!(!overlaps(&items, &a, 3.0));
    }

    #[test]
    fn parts_keep_clear_of_the_clamps() {
        let items = [(30.0, 20.0), (30.0, 20.0), (30.0, 20.0), (30.0, 20.0)];
        // a clamp across the lower left of the sheet
        let clamp = [[0.0, 0.0, 40.0, 25.0]];
        let r = nest(&items, [0.0, 0.0, 100.0, 100.0], 5.0, false, &clamp);
        assert_eq!(r.placed.len(), 4, "{r:?}");
        for p in &r.placed {
            let (w, h) = items[p.item];
            let hit = p.x < clamp[0][2] + 5.0 - 1e-9 && clamp[0][0] < p.x + w + 5.0 - 1e-9 && p.y < clamp[0][3] + 5.0 - 1e-9 && clamp[0][1] < p.y + h + 5.0 - 1e-9;
            assert!(!hit, "part at {},{} is on the clamp", p.x, p.y);
        }
        assert!(!overlaps(&items, &r, 5.0));
        // a clamp across the whole width leaves only the part above it
        let wall = [[0.0, 0.0, 100.0, 60.0]];
        let r = nest(&items, [0.0, 0.0, 100.0, 100.0], 5.0, false, &wall);
        assert!(r.placed.iter().all(|p| p.y >= 65.0 - 1e-9), "{r:?}");
        assert_eq!(r.placed.len(), 3, "one row of three fits in the 35 mm above: {r:?}");
    }

    #[test]
    fn nothing_to_nest_is_nothing() {
        assert!(nest(&[], [0.0, 0.0, 10.0, 10.0], 1.0, true, &[]).placed.is_empty());
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Poly { vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]] }
    fn at(poly: &Poly, f: &Fitted) -> Poly {
        let r = turned(std::slice::from_ref(poly), f.angle);
        let b = bounds(&r).unwrap();
        r[0].iter().map(|q| [q[0] - b[0] + f.x, q[1] - b[1] + f.y]).collect()
    }
    fn apart(a: &Poly, b: &Poly, gap: f64) -> bool {
        let to = |p: &Poly| -> clip::Path { let mut v: clip::Path = p.iter().map(|q| [(q[0] * 1000.0).round() as i64, (q[1] * 1000.0).round() as i64]).collect(); if clip::area(&v) < 0.0 { v.reverse(); } v };
        let wide = clip::offset(&[to(a)], gap * 1000.0 * 0.98, 50.0);
        clip::boolean(clip::BoolKind::Intersect, &wide, &[to(b)]).is_empty()
    }
    /// An L: 60 x 60 with a 40 x 40 corner cut out.
    fn ell() -> Poly { vec![[0.0, 0.0], [60.0, 0.0], [60.0, 20.0], [20.0, 20.0], [20.0, 60.0], [0.0, 60.0]] }

    #[test]
    fn by_shape_an_l_goes_into_the_corner_of_the_one_before() {
        // two L shapes: their boxes (60 x 60) do not fit twice side by side on 100 wide, but the shapes interlock when one is turned
        let parts = vec![Part::solid(vec![ell()]), Part::solid(vec![ell()])];
        let by_box = nest(&[(60.0, 60.0), (60.0, 60.0)], [0.0, 0.0, 100.0, 100.0], 4.0, true, &[]);
        let r = nest_shapes(&parts, [0.0, 0.0, 100.0, 100.0], 4.0, &[0.0, 90.0, 180.0, 270.0], &[]);
        assert_eq!(r.placed.len(), 2, "{r:?}");
        let (a, b) = (at(&parts[0].outline[0], &r.placed[0]), at(&parts[1].outline[0], &r.placed[1]));
        assert!(apart(&a, &b, 4.0), "the two L shapes touch: {a:?} {b:?}");
        let top = |p: &Poly| p.iter().map(|q| q[1]).fold(0.0_f64, f64::max);
        let used = top(&a).max(top(&b));
        let by_box_top = by_box.placed.iter().map(|p| p.y + 60.0).fold(0.0_f64, f64::max);
        assert!(used < by_box_top - 1.0 || by_box.placed.len() < 2, "interlocked ({used}) should use less height than the boxes ({by_box_top})");
        assert!(a.iter().chain(&b).all(|q| q[0] >= -1e-6 && q[1] >= -1e-6 && q[0] <= 100.0 + 1e-6 && q[1] <= 100.0 + 1e-6));
    }

    #[test]
    fn by_shape_parts_keep_the_gap_and_clear_of_clamps() {
        let parts: Vec<Part> = (0..6).map(|i| Part::solid(vec![if i % 2 == 0 { ell() } else { rect(0.0, 0.0, 25.0, 15.0) }])).collect();
        let clamp = [[0.0, 0.0, 30.0, 100.0]];
        let r = nest_shapes(&parts, [0.0, 0.0, 200.0, 100.0], 5.0, &[0.0, 90.0], &clamp);
        assert!(r.placed.len() >= 5, "{r:?}");
        let shapes: Vec<Poly> = r.placed.iter().map(|f| at(&parts[f.item].outline[0], f)).collect();
        for (i, a) in shapes.iter().enumerate() {
            assert!(apart(a, &rect(0.0, 0.0, 30.0, 100.0), 5.0), "part {i} is too near the clamp");
            for b in shapes.iter().skip(i + 1) { assert!(apart(a, b, 5.0), "two parts are closer than the gap"); }
        }
    }

    #[test]
    fn by_shape_what_does_not_fit_is_left_out_and_the_layout_repeats() {
        let parts = vec![Part::solid(vec![rect(0.0, 0.0, 80.0, 80.0)]), Part::solid(vec![rect(0.0, 0.0, 80.0, 80.0)]), Part::solid(vec![rect(0.0, 0.0, 300.0, 5.0)])];
        let a = nest_shapes(&parts, [0.0, 0.0, 100.0, 100.0], 2.0, &[0.0], &[]);
        assert_eq!(a.left_out, vec![1, 2], "{a:?}");
        assert_eq!(a.placed.len(), 1);
        assert_eq!(a.placed, nest_shapes(&parts, [0.0, 0.0, 100.0, 100.0], 2.0, &[0.0], &[]).placed);
    }

    #[test]
    fn a_small_part_goes_into_the_hole_of_a_big_one() {
        // a 90 x 90 plate with a 50 x 50 hole, and a 30 x 30 part: the sheet is only just big enough for the plate
        let plate = Part { outline: vec![rect(0.0, 0.0, 90.0, 90.0)], holes: vec![rect(20.0, 20.0, 50.0, 50.0)], islands: vec![] };
        let small = Part::solid(vec![rect(0.0, 0.0, 30.0, 30.0)]);
        let sheet = [0.0, 0.0, 100.0, 100.0];
        let with = nest_shapes(&[plate.clone(), small.clone()], sheet, 4.0, &[0.0], &[]);
        assert_eq!(with.placed.len(), 2, "{with:?}");
        let b = at(&small.outline[0], with.placed.iter().find(|f| f.item == 1).unwrap());
        // the small part is inside the hole's window, apart from its wall by the gap
        let (px, py) = (with.placed.iter().find(|f| f.item == 0).unwrap().x, with.placed.iter().find(|f| f.item == 0).unwrap().y);
        assert!(b.iter().all(|q| q[0] >= px + 20.0 + 4.0 - 1e-6 && q[0] <= px + 70.0 - 4.0 + 1e-6 && q[1] >= py + 20.0 + 4.0 - 1e-6 && q[1] <= py + 70.0 - 4.0 + 1e-6), "{b:?} not in the hole of the plate at {px},{py}");
        // without the hole being usable it does not fit
        let solid = Part::solid(plate.outline.clone());
        let without = nest_shapes(&[solid, small], sheet, 4.0, &[0.0], &[]);
        assert_eq!(without.left_out, vec![1], "{without:?}");
    }

    #[test]
    fn a_part_too_big_for_the_hole_stays_out_and_what_stands_in_the_hole_stays_clear() {
        // an island of 20 x 20 stands in the middle of a 50 x 50 hole: a 30 x 30 part has no room there any more
        let plate = Part { outline: vec![rect(0.0, 0.0, 90.0, 90.0)], holes: vec![rect(20.0, 20.0, 50.0, 50.0)], islands: vec![rect(35.0, 35.0, 20.0, 20.0)] };
        let small = Part::solid(vec![rect(0.0, 0.0, 30.0, 30.0)]);
        let r = nest_shapes(&[plate, small], [0.0, 0.0, 100.0, 100.0], 4.0, &[0.0], &[]);
        assert_eq!(r.left_out, vec![1], "{r:?}");
    }
}

