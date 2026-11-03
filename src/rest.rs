//! Rest machining: what a big cutter leaves in a pocket, and the area a smaller cutter has to visit to clear it.
//! A round cutter of radius `rb` reaches every point of the pocket that a disc of that radius can touch while staying inside, which is
//! the pocket "opened" by the disc (shrunk by `rb`, grown back by `rb`). What it cannot reach is the pocket minus that: the corners, and
//! any part narrower than the cutter.
use crate::clip::{self, BoolKind, Path};

const S: f64 = 1000.0;
/// Leftovers narrower than this (mm, twice) are rounding noise along the walls, not material.
const SLIVER: f64 = 0.1;

fn off(p: &[Path], d_mm: f64) -> Vec<Path> { clip::offset(p, d_mm * S, S * 0.01) }

/// The material a cutter of radius `rb` (mm) leaves in `region`, as closed paths.
pub fn leftover(region: &[Path], rb: f64) -> Vec<Path> {
    if region.is_empty() { return vec![]; }
    let reached = off(&off(region, -rb), rb);
    let left = if reached.is_empty() { region.to_vec() } else { clip::boolean(BoolKind::Difference, region, &reached) };
    // drop slivers: shrink and grow back, which removes everything narrower than twice SLIVER
    let shrunk = off(&left, -SLIVER);
    if shrunk.is_empty() { return vec![]; }
    off(&shrunk, SLIVER)
}

/// The area for the smaller cutter of radius `rs` (mm): the leftover, grown by the cutter's diameter and cut to the pocket, so a pocket
/// of it with that cutter covers the leftover. Empty when there is nothing to clear.
pub fn cleanup_region(region: &[Path], rb: f64, rs: f64) -> Vec<Path> {
    let l = leftover(region, rb);
    if l.is_empty() { return vec![]; }
    // a disc of radius rs that touches the leftover reaches 2 rs from it, so the pocket for the small cutter must hold all of that
    let grown = off(&l, 2.0 * rs);
    clip::boolean(BoolKind::Intersect, region, &grown)
}

pub fn area_mm2(p: &[Path]) -> f64 { p.iter().map(|q| clip::area(q)).sum::<f64>() / (S * S) }

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> Vec<Path> {
        vec![vec![[0, 0], [(w * S) as i64, 0], [(w * S) as i64, (h * S) as i64], [0, (h * S) as i64]]]
    }

    #[test]
    fn a_square_pocket_leaves_the_corners() {
        let r = rect(40.0, 30.0);
        let l = leftover(&r, 3.0);
        // four corners, each (1 - pi/4) r^2 of material
        let want = 4.0 * (1.0 - std::f64::consts::FRAC_PI_4) * 9.0;
        let got = area_mm2(&l);
        assert!((got - want).abs() < 0.5, "{got} vs {want}");
        assert_eq!(l.len(), 4);
    }

    #[test]
    fn a_cutter_as_small_as_the_pocket_features_leaves_almost_nothing() {
        assert!(area_mm2(&leftover(&rect(40.0, 30.0), 0.1)) < 0.01);
    }

    #[test]
    fn a_slot_narrower_than_the_cutter_is_all_left() {
        let r = rect(40.0, 4.0);
        let l = leftover(&r, 3.0);
        assert!((area_mm2(&l) - 160.0).abs() < 1.0, "{}", area_mm2(&l));
    }

    #[test]
    fn the_cleanup_region_stays_inside_the_pocket_and_covers_the_corners() {
        let r = rect(40.0, 30.0);
        let c = cleanup_region(&r, 3.0, 1.0);
        assert!(!c.is_empty());
        let a = area_mm2(&c);
        assert!(a < area_mm2(&r) / 4.0, "{a}");
        // inside the pocket
        let out = clip::boolean(BoolKind::Difference, &c, &r);
        assert!(area_mm2(&out) < 1e-6);
        // it holds every corner leftover
        let l = leftover(&r, 3.0);
        let miss = clip::boolean(BoolKind::Difference, &l, &c);
        assert!(area_mm2(&miss) < 0.01, "{}", area_mm2(&miss));
    }

    fn pocket_project(rest: bool) -> crate::model::ProjectData {
        let mut f = crate::shapes::blank_project();
        f.data.machine = Some(crate::model::Machine::default());
        f.data.tools = crate::setup::starter_tools(Default::default());
        let by = |n: &str| f.data.tools.iter().find(|t| t.name == n).unwrap().id.clone();
        let (big, small) = (by("6 mm flat end mill"), by("2 mm flat end mill"));
        f.data.stock.w = 100.0; f.data.stock.h = 80.0; f.data.stock.t = 10.0;
        let mut p = crate::shapes::default_op_params(&big);
        p.kind = "pocket".into(); p.depth = 4.0;
        if rest { p.rest = Some(small); }
        f.data.ops.push(crate::model::Operation { id: 1, p });
        f.data.oid = 1; f.data.act = 1;
        f.data.loops.push(crate::shapes::new_rect([20.0, 20.0], [80.0, 60.0], 0.0, 1));
        f.data
    }

    #[test]
    fn the_small_tool_comes_in_after_the_big_one_and_stays_in_the_corners() {
        use crate::{generate, Units};
        let plain = generate(&pocket_project(false), Units::Metric).unwrap();
        let both = generate(&pocket_project(true), Units::Metric).unwrap();
        assert_eq!(plain.lines.iter().filter(|l| l.starts_with("(Tool:")).count(), 1);
        let tools: Vec<&String> = both.lines.iter().filter(|l| l.starts_with("(Tool:")).collect();
        assert_eq!(tools.len(), 2, "{tools:?} {:?}", both.notes);
        assert!(tools[1].contains("2 mm"), "{tools:?}");
        // a tool change stop sits between them, and the cleanup is short next to the main pocket
        assert!(both.lines.iter().any(|l| l.starts_with("M0")));
        let extra = both.cut_len_mm - plain.cut_len_mm;
        assert!(extra > 0.0 && extra < plain.cut_len_mm / 2.0, "{extra} of {}", plain.cut_len_mm);
        // the cleanup never leaves the pocket (tool centre at least a radius inside the walls) and reaches the corners
        let at = both.lines.iter().position(|l| l.contains("corners and narrow parts")).unwrap();
        let (mut minx, mut maxx, mut miny, mut maxy) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for l in both.lines[at..].iter().filter(|l| l.starts_with("G1")) {
            for w in l.split_whitespace() {
                let v = |p: &str| w.strip_prefix(p).and_then(|x| x.parse::<f64>().ok());
                if let Some(x) = v("X") { minx = minx.min(x); maxx = maxx.max(x); }
                if let Some(y) = v("Y") { miny = miny.min(y); maxy = maxy.max(y); }
            }
        }
        assert!(minx >= 21.0 - 1e-6 && maxx <= 79.0 + 1e-6 && miny >= 21.0 - 1e-6 && maxy <= 59.0 + 1e-6, "{minx} {maxx} {miny} {maxy}");
        assert!(minx < 22.0 && maxx > 78.0 && miny < 22.0 && maxy > 58.0, "{minx} {maxx} {miny} {maxy}");
    }

    #[test]
    fn a_bigger_cleanup_tool_is_refused_with_a_note() {
        use crate::{generate, Units};
        let mut d = pocket_project(true);
        d.ops[0].p.rest = Some(d.tools.iter().find(|t| t.name == "8 mm flat end mill").unwrap().id.clone());
        let o = generate(&d, Units::Metric).unwrap();
        assert!(o.notes.iter().any(|n| n.contains("must be smaller")), "{:?}", o.notes);
        assert_eq!(o.lines.iter().filter(|l| l.starts_with("(Tool:")).count(), 1);
    }

    #[test]
    fn the_checker_and_the_simulation_know_the_cleanup_tool() {
        use crate::{generate, Units};
        let d = pocket_project(true);
        let o = generate(&d, Units::Metric).unwrap();
        // the corner passes are made by the second tool, which is the 2 mm one
        assert!(o.moves.iter().any(|m| m.alt && m.cut));
        assert!((crate::checks::tool_of(&d, 0, true).unwrap().dia - 2.0).abs() < 1e-9);
        let f = crate::checks::run(&d, &o);
        assert!(!f.iter().any(|f| f.kind == crate::checks::Kind::Overcut), "{:?}", f.iter().map(|f| &f.msg).collect::<Vec<_>>());
        // reading the program back agrees about which moves are the second tool's
        let r = crate::replay::replay(&o.lines, &d);
        assert!(r.moves.iter().any(|m| m.alt && m.cut));
    }
}
