//! Nesting: packing rectangles (the bounding boxes of parts) onto a sheet in rows ("shelves"), optionally turning parts a quarter turn.
//! Simple and predictable rather than optimal: the parts are sorted tallest first, filled in rows from the lower left, and when turning is
//! allowed the better of three ways of orienting them (as drawn, all lying flat, all standing up) is kept.

/// Where one part goes: `x` and `y` are the lower left corner of its box on the sheet (after turning, if `turned`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed { pub item: usize, pub x: f64, pub y: f64, pub turned: bool }

/// The result: where each part that fits goes, and the parts that do not fit (indices into the input).
#[derive(Debug, Clone, Default)]
pub struct Nesting { pub placed: Vec<Placed>, pub left_out: Vec<usize> }

fn shelves(dims: &[(f64, f64)], turned: &[bool], sheet: [f64; 4], gap: f64) -> Nesting {
    let size = |i: usize| if turned[i] { (dims[i].1, dims[i].0) } else { dims[i] };
    let mut order: Vec<usize> = (0..dims.len()).collect();
    // tallest first, then widest, then as given (so the result is the same every time)
    order.sort_by(|&a, &b| size(b).1.total_cmp(&size(a).1).then(size(b).0.total_cmp(&size(a).0)).then(a.cmp(&b)));
    let (mut x, mut y, mut row_h) = (sheet[0], sheet[1], 0.0_f64);
    let mut out = Nesting::default();
    for i in order {
        let (w, h) = size(i);
        if w > sheet[2] - sheet[0] + 1e-9 || h > sheet[3] - sheet[1] + 1e-9 { out.left_out.push(i); continue; }
        if x + w > sheet[2] + 1e-9 && row_h > 0.0 { x = sheet[0]; y += row_h + gap; row_h = 0.0; }
        if y + h > sheet[3] + 1e-9 { out.left_out.push(i); continue; }
        out.placed.push(Placed { item: i, x, y, turned: turned[i] });
        x += w + gap;
        row_h = row_h.max(h);
    }
    out.left_out.sort_unstable();
    out
}

/// Packs boxes `(width, height)` onto `sheet` (`[x0, y0, x1, y1]`, mm) with `gap` mm between neighbouring boxes.
pub fn nest(items: &[(f64, f64)], sheet: [f64; 4], gap: f64, rotate: bool) -> Nesting {
    let n = items.len();
    let mut tries: Vec<Vec<bool>> = vec![vec![false; n]];
    if rotate {
        tries.push(items.iter().map(|&(w, h)| h > w).collect()); // lying flat: wider than tall
        tries.push(items.iter().map(|&(w, h)| w > h).collect()); // standing up: taller than wide
    }
    let gap = gap.max(0.0);
    let mut best: Option<(Nesting, f64)> = None;
    for t in &tries {
        let r = shelves(items, t, sheet, gap);
        // the most parts first, then the least sheet used up the side
        let top = r.placed.iter().map(|p| { let (w, h) = if p.turned { (items[p.item].1, items[p.item].0) } else { items[p.item] }; let _ = w; p.y + h }).fold(sheet[1], f64::max);
        let better = match &best { None => true, Some((b, bt)) => r.placed.len() > b.placed.len() || (r.placed.len() == b.placed.len() && top < *bt - 1e-9) };
        if better { best = Some((r, top)); }
    }
    best.map(|b| b.0).unwrap_or_default()
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
        let r = nest(&items, [0.0, 0.0, 100.0, 100.0], 5.0, false);
        assert!(r.left_out.is_empty() && r.placed.len() == 5, "{r:?}");
        assert!(!overlaps(&items, &r, 5.0));
        assert!(r.placed.iter().all(|p| p.x >= 0.0 && p.y >= 0.0 && p.x + items[p.item].0 <= 100.0 + 1e-9 && p.y + items[p.item].1 <= 100.0 + 1e-9));
        // the tallest part goes first, in the lower left corner
        assert_eq!((r.placed[0].item, r.placed[0].x, r.placed[0].y), (4, 0.0, 0.0));
    }

    #[test]
    fn what_does_not_fit_is_left_out_not_squeezed_in() {
        let items = [(60.0, 60.0), (60.0, 60.0), (200.0, 10.0)];
        let r = nest(&items, [0.0, 0.0, 100.0, 100.0], 2.0, false);
        assert_eq!(r.left_out, vec![1, 2], "{r:?}"); // the second does not fit beside or above the first; the third is wider than the sheet
        assert_eq!(r.placed.len(), 1);
    }

    #[test]
    fn turning_a_part_makes_room_for_more() {
        // tall strips on a wide, short sheet only fit lying down
        let items = [(10.0, 90.0), (10.0, 90.0), (10.0, 90.0)];
        let plain = nest(&items, [0.0, 0.0, 100.0, 40.0], 2.0, false);
        let turned = nest(&items, [0.0, 0.0, 100.0, 40.0], 2.0, true);
        assert!(plain.placed.is_empty(), "{plain:?}");
        assert_eq!(turned.placed.len(), 3, "{turned:?}");
        assert!(turned.placed.iter().all(|p| p.turned));
        assert!(!overlaps(&items, &turned, 2.0));
    }

    #[test]
    fn the_same_input_always_gives_the_same_layout() {
        let items: Vec<(f64, f64)> = (0..12).map(|i| (10.0 + (i % 5) as f64 * 7.0, 8.0 + (i % 3) as f64 * 9.0)).collect();
        let a = nest(&items, [5.0, 5.0, 195.0, 145.0], 3.0, true);
        let b = nest(&items, [5.0, 5.0, 195.0, 145.0], 3.0, true);
        assert_eq!(a.placed, b.placed);
        assert!(!overlaps(&items, &a, 3.0));
    }

    #[test]
    fn nothing_to_nest_is_nothing() {
        assert!(nest(&[], [0.0, 0.0, 10.0, 10.0], 1.0, true).placed.is_empty());
    }
}
