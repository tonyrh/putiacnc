//! Thin safe wrapper over Clipper 6.4 (the same library and version the browser app uses).
//! Paths are integer points in 1/1000 mm, like the JavaScript app's `S = 1000` scale.
use clipper_sys as c;

pub type Pt = [i64; 2];
pub type Path = Vec<Pt>;

/// Signed area (positive = counter-clockwise in a y-up system), in square integer units.
pub fn area(p: &[Pt]) -> f64 {
    let n = p.len();
    if n < 3 { return 0.0; }
    let mut a = 0.0;
    let mut j = n - 1;
    for i in 0..n {
        a += (p[j][0] as f64 + p[i][0] as f64) * (p[j][1] as f64 - p[i][1] as f64);
        j = i;
    }
    -a * 0.5
}

/// Clipper throws a C++ exception (which aborts the whole program when it reaches Rust) for a point beyond ±4.6e18. A project can hold
/// such numbers (a hand-edited file, a diameter of 1e300 turned into a point by saturating): paths that far out are given no result.
const LIMIT: i64 = 2_000_000_000_000_000_000;
fn in_range(paths: &[Path]) -> bool { paths.iter().all(|p| p.iter().all(|q| q[0].abs() < LIMIT && q[1].abs() < LIMIT)) }

struct Raw { paths: Vec<Path>, cpaths: Vec<c::Path>, cpolys: Vec<c::Polygon> }

fn to_raw(paths: &[Path]) -> Raw {
    let paths: Vec<Path> = paths.to_vec();
    let cpaths: Vec<c::Path> = paths.iter().map(|p| c::Path { vertices: p.as_ptr() as *mut c::Vertice, vertices_count: p.len(), closed: 1 }).collect();
    let cpolys = vec![c::Polygon { paths: cpaths.as_ptr() as *mut c::Path, paths_count: cpaths.len(), type_: c::PolyType_ptSubject }];
    Raw { paths, cpaths, cpolys }
}

fn polygons_of(raw: &Raw) -> c::Polygons {
    c::Polygons { polygons: raw.cpolys.as_ptr() as *mut c::Polygon, polygons_count: raw.cpolys.len() }
}

fn flatten_and_free(out: c::Polygons) -> Vec<Path> {
    let mut res: Vec<Path> = Vec::new();
    for poly in out.polygons() {
        for p in poly.paths() { res.push(p.vertices().to_vec()); }
    }
    unsafe { c::free_polygons(out) };
    res
}

/// Union with the even-odd rule (what the app uses to decide which loops are holes).
pub fn simplify_even_odd(paths: &[Path]) -> Vec<Path> {
    if paths.is_empty() || !in_range(paths) { return vec![]; }
    // many overlapping outlines with densely collinear points make Clipper's union take minutes (see `offset`)
    let cleaned: Vec<Path>;
    let paths = if paths.iter().map(|p| p.len()).sum::<usize>() > 4000 { cleaned = clean(paths, 1.415); &cleaned[..] } else { paths };
    if paths.is_empty() { return vec![]; }
    let raw = to_raw(paths);
    let out = unsafe { c::simplify(polygons_of(&raw), c::PolyFillType_pftEvenOdd) };
    let _keep = (&raw.paths, &raw.cpaths);
    flatten_and_free(out)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoolKind { Union, Intersect, Difference, Xor }

/// Boolean combination of two sets of closed paths. Both sets must be properly oriented regions
/// (outer outlines one way, holes the other, as `simplify_even_odd` returns them): they are filled with the non-zero rule.
pub fn boolean(kind: BoolKind, subject: &[Path], clip: &[Path]) -> Vec<Path> {
    if (subject.is_empty() && clip.is_empty()) || !in_range(subject) || !in_range(clip) { return vec![]; }
    let (a, b) = (to_raw(subject), to_raw(clip));
    let polys = vec![
        c::Polygon { paths: a.cpaths.as_ptr() as *mut c::Path, paths_count: a.cpaths.len(), type_: c::PolyType_ptSubject },
        c::Polygon { paths: b.cpaths.as_ptr() as *mut c::Path, paths_count: b.cpaths.len(), type_: c::PolyType_ptClip },
    ];
    let ct = match kind { BoolKind::Union => c::ClipType_ctUnion, BoolKind::Intersect => c::ClipType_ctIntersection, BoolKind::Difference => c::ClipType_ctDifference, BoolKind::Xor => c::ClipType_ctXor };
    let out = unsafe { c::execute(ct, c::Polygons { polygons: polys.as_ptr() as *mut c::Polygon, polygons_count: polys.len() }, c::PolyFillType_pftNonZero, c::PolyFillType_pftNonZero) };
    let _keep = (&a.paths, &a.cpaths, &b.paths, &b.cpaths, &polys);
    flatten_and_free(out)
}

/// Round-join offset of closed polygons (delta in integer units, negative = inward).
pub fn offset(paths: &[Path], delta: f64, arc_tolerance: f64) -> Vec<Path> {
    // the offset moves every point by delta, and the arcs of a round join need a sensible tolerance
    if paths.is_empty() || !in_range(paths) || !delta.is_finite() || delta.abs() > 1e18 || !arc_tolerance.is_finite() || arc_tolerance <= 0.0 { return vec![]; }
    // Vertices that lie on the line between their neighbours (to 1.4 thousandths of a millimetre) add nothing to the shape, but a
    // drawing made of many overlapping copies can reach Clipper's offset with thousands of them, and it then takes minutes
    // (one project: 83 s for 19,000 points, 1 s once cleaned to 800).
    // Only for big inputs: small ones are offset as they are, so ordinary programs do not change by a hair.
    let cleaned: Vec<Path>;
    let paths = if paths.iter().map(|p| p.len()).sum::<usize>() > 4000 { cleaned = clean(paths, 1.415); &cleaned[..] } else { paths };
    if paths.is_empty() { return vec![]; }
    let raw = to_raw(paths);
    let out = unsafe { c::offset(2.0, arc_tolerance, c::JoinType_jtRound, c::EndType_etClosedPolygon, polygons_of(&raw), delta) };
    let _keep = (&raw.paths, &raw.cpaths);
    flatten_and_free(out)
}

/// Remove vertices closer than `distance` to the line through their neighbours.
pub fn clean(paths: &[Path], distance: f64) -> Vec<Path> {
    if paths.is_empty() || !in_range(paths) || !distance.is_finite() { return vec![]; }
    let raw = to_raw(paths);
    let out = unsafe { c::clean(polygons_of(&raw), distance) };
    let _keep = (&raw.paths, &raw.cpaths);
    flatten_and_free(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sq(x0: i64, y0: i64, x1: i64, y1: i64) -> Path { vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]] }
    #[test]
    fn square_area_and_orientation() {
        let a = area(&sq(0, 0, 10_000, 10_000));
        assert!((a - 1e8).abs() < 1.0, "{a}");
    }
    #[test]
    fn inward_offset_shrinks() {
        let r = offset(&[sq(0, 0, 10_000, 10_000)], -1000.0, 10.0);
        assert_eq!(r.len(), 1);
        assert!((area(&r[0]).abs() - 64e6).abs() < 1e5, "{}", area(&r[0]));
    }
    #[test]
    fn even_odd_hole() {
        let r = simplify_even_odd(&[sq(0, 0, 10_000, 10_000), sq(3000, 3000, 6000, 6000)]);
        assert_eq!(r.len(), 2);
        let neg = r.iter().filter(|p| area(p) < 0.0).count();
        assert_eq!(neg, 1);
    }
}
