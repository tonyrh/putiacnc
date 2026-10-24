//! SVG import. Parsing, style/units/transform resolution and text-to-outline conversion are done
//! by `usvg`; this module flattens the resulting paths into polylines (loops) in millimetres.
//!
//! The result uses the SVG page as the coordinate frame with the origin at its lower-left corner
//! (Y up, like the editor); the caller decides where to place it.
use crate::model::LoopData;
use usvg::tiny_skia_path::{PathSegment, Point};

const PX_MM: f64 = 25.4 / 96.0; // usvg resolves every unit to CSS pixels (96 dpi)
const S: f64 = 1000.0;

pub struct Imported {
    pub loops: Vec<LoopData>,
    pub width_mm: f64,
    pub height_mm: f64,
    pub warnings: Vec<String>,
}

pub fn import(data: &[u8]) -> Result<Imported, String> {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_data(data, &opt).map_err(|e| format!("Could not read SVG: {e}"))?;
    let h_px = tree.size().height() as f64;
    let mut ctx = Ctx { h_px, loops: vec![], images: 0 };
    walk(tree.root(), &mut ctx);
    let mut warnings = vec![];
    if ctx.images > 0 { warnings.push(format!("{} embedded image(s) were ignored (only vector shapes can be cut).", ctx.images)); }
    if ctx.loops.is_empty() { warnings.push("The SVG contains no vector outlines.".into()); }
    Ok(Imported { loops: ctx.loops, width_mm: tree.size().width() as f64 * PX_MM, height_mm: h_px * PX_MM, warnings })
}

struct Ctx { h_px: f64, loops: Vec<LoopData>, images: usize }

fn walk(g: &usvg::Group, ctx: &mut Ctx) {
    for n in g.children() {
        match n {
            usvg::Node::Group(g) => walk(g, ctx),
            usvg::Node::Path(p) => {
                if !p.is_visible() { continue; }
                let Some(path) = p.data().clone().transform(p.abs_transform()) else { continue };
                add_path(&path, ctx);
            }
            usvg::Node::Text(t) => walk(t.flattened(), ctx),
            usvg::Node::Image(_) => ctx.images += 1,
        }
    }
}

fn to_mm(p: Point, h_px: f64) -> [f64; 2] { [p.x as f64 * PX_MM, (h_px - p.y as f64) * PX_MM] }

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 { (a[0] - b[0]).hypot(a[1] - b[1]) }

fn steps(len_mm: f64) -> usize { ((len_mm / 0.25).ceil() as usize).clamp(2, 400) }

fn add_path(path: &usvg::tiny_skia_path::Path, ctx: &mut Ctx) {
    let h = ctx.h_px;
    let mut cur: Vec<[f64; 2]> = vec![];
    let mut closed_flag = false;
    let flush = |cur: &mut Vec<[f64; 2]>, closed: bool, loops: &mut Vec<LoopData>| {
        let mut pts: Vec<[f64; 2]> = vec![];
        for p in cur.drain(..) { if pts.last().is_none_or(|q| dist(*q, p) > 1e-4) { pts.push(p); } }
        let mut closed = closed;
        // an open path that ends where it starts is a closed shape
        if pts.len() > 2 && dist(pts[0], *pts.last().unwrap()) < 1e-3 { pts.pop(); closed = true; }
        if closed && pts.len() > 1 && dist(pts[0], *pts.last().unwrap()) < 1e-4 { pts.pop(); }
        if pts.len() < 2 || (closed && pts.len() < 3) { return; }
        loops.push(LoopData {
            c: closed as u8, op: 0, rc: None, cr: None, el: None, pt: None, grp: None,
            pts: pts.iter().flat_map(|p| [(p[0] * S).round() as i64, (p[1] * S).round() as i64]).collect(),
        });
    };
    let mut last = [0.0, 0.0];
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                flush(&mut cur, closed_flag, &mut ctx.loops);
                closed_flag = false;
                last = to_mm(p, h);
                cur.push(last);
            }
            PathSegment::LineTo(p) => { last = to_mm(p, h); cur.push(last); }
            PathSegment::QuadTo(c, p) => {
                let (c, p) = (to_mm(c, h), to_mm(p, h));
                let n = steps(dist(last, c) + dist(c, p));
                for i in 1..=n {
                    let t = i as f64 / n as f64;
                    let u = 1.0 - t;
                    cur.push([u * u * last[0] + 2.0 * u * t * c[0] + t * t * p[0], u * u * last[1] + 2.0 * u * t * c[1] + t * t * p[1]]);
                }
                last = p;
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let (c1, c2, p) = (to_mm(c1, h), to_mm(c2, h), to_mm(p, h));
                let n = steps(dist(last, c1) + dist(c1, c2) + dist(c2, p));
                for i in 1..=n {
                    let t = i as f64 / n as f64;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    cur.push([a * last[0] + b * c1[0] + c * c2[0] + d * p[0], a * last[1] + b * c1[1] + c * c2[1] + d * p[1]]);
                }
                last = p;
            }
            PathSegment::Close => { flush(&mut cur, true, &mut ctx.loops); closed_flag = false; }
        }
    }
    flush(&mut cur, closed_flag, &mut ctx.loops);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::bbox;

    fn bb(l: &LoopData) -> [f64; 4] { bbox(l).unwrap() }

    #[test]
    fn rect_in_mm_with_y_flip() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="60mm" viewBox="0 0 100 60">
            <rect x="10" y="5" width="30" height="20"/></svg>"#;
        let r = import(svg.as_bytes()).unwrap();
        assert!((r.width_mm - 100.0).abs() < 0.01 && (r.height_mm - 60.0).abs() < 0.01);
        assert_eq!(r.loops.len(), 1);
        assert!(r.loops[0].closed());
        let b = bb(&r.loops[0]);
        // SVG y=5..25 from the top of a 60 mm page -> 35..55 from the bottom
        for (a, e) in b.iter().zip([10.0, 35.0, 40.0, 55.0]) { assert!((a - e).abs() < 0.01, "{b:?}"); }
    }

    #[test]
    fn circle_and_transform() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="50mm" height="50mm" viewBox="0 0 50 50">
            <g transform="translate(10,10) scale(2)"><circle cx="5" cy="5" r="5"/></g></svg>"#;
        let r = import(svg.as_bytes()).unwrap();
        let b = bb(&r.loops[0]);
        assert!((b[2] - b[0] - 20.0).abs() < 0.05 && (b[3] - b[1] - 20.0).abs() < 0.05, "{b:?}");
        assert!(r.loops[0].pts.len() / 2 > 40);
    }

    #[test]
    fn curves_open_paths_and_subpaths() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100">
            <path d="M10 10 C 30 0, 50 0, 70 10 S 90 40, 70 60 Z M20 20 L 40 20 L 40 40"/>
            <path d="M0 90 L 50 90" stroke="black" fill="none"/></svg>"#;
        let r = import(svg.as_bytes()).unwrap();
        assert_eq!(r.loops.len(), 3);
        assert_eq!(r.loops.iter().filter(|l| l.closed()).count(), 1);
        assert!(r.loops.iter().any(|l| !l.closed() && l.pts.len() == 6));
    }

    #[test]
    fn pixel_units_use_96_dpi() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><rect width="96" height="96"/></svg>"#;
        let r = import(svg.as_bytes()).unwrap();
        assert!((r.width_mm - 25.4).abs() < 0.01);
        let b = bb(&r.loops[0]);
        assert!((b[2] - b[0] - 25.4).abs() < 0.01);
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(import(b"not an svg").is_err());
    }

    #[test]
    fn imported_shapes_can_be_cut() {
        use crate::{generate, shapes, Units};
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="60mm" height="40mm" viewBox="0 0 60 40">
            <rect x="5" y="5" width="50" height="30" rx="6"/><circle cx="30" cy="20" r="6"/></svg>"#;
        let r = import(svg.as_bytes()).unwrap();
        let mut p = shapes::blank_project();
        p.data.ops.push(crate::model::Operation { id: 1, p: shapes::default_op_params(&p.data.tools[1].id.clone()) });
        p.data.act = 1; p.data.oid = 1;
        for mut l in r.loops { l.op = 1; p.data.loops.push(l); }
        let out = generate(&p.data, Units::Metric).unwrap();
        assert!(out.cut_len_mm > 100.0);
    }
}
