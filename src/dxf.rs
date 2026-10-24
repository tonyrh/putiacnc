//! DXF import (ASCII DXF, any AutoCAD version). Reads the 2D geometry of LINE, ARC, CIRCLE, ELLIPSE,
//! LWPOLYLINE, POLYLINE, SPLINE and INSERT (block references) entities, converts it to millimetres
//! using the drawing's `$INSUNITS`, and joins loose lines and arcs that touch end to end into
//! closed or open shapes. Text, dimensions, hatches and images are skipped and reported.
//!
//! Like the SVG import the result keeps the drawing's own frame (Y up); the caller places it.
use crate::shapes;
use crate::svg::Imported;
use std::collections::BTreeMap;
use std::f64::consts::TAU;

/// Curves are flattened to within this distance, in mm.
const TOL: f64 = 0.02;
/// End points closer than this (mm) count as the same point when joining.
const JOIN: f64 = 0.01;

type P = [f64; 2];
/// Affine transform: x' = a x + c y + e, y' = b x + d y + f.
#[derive(Clone, Copy)]
struct M([f64; 6]);
impl M {
    fn ap(&self, p: P) -> P { let m = self.0; [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]] }
    /// self applied after `o`
    fn after(&self, o: &M) -> M {
        let (a, b) = (self.0, o.0);
        M([a[0] * b[0] + a[2] * b[1], a[1] * b[0] + a[3] * b[1], a[0] * b[2] + a[2] * b[3], a[1] * b[2] + a[3] * b[3],
           a[0] * b[4] + a[2] * b[5] + a[4], a[1] * b[4] + a[3] * b[5] + a[5]])
    }
}

struct Ent { kind: String, g: Vec<(i32, String)> }
impl Ent {
    fn f(&self, code: i32) -> Option<f64> { self.g.iter().find(|(c, _)| *c == code).and_then(|(_, v)| v.parse().ok()) }
    fn s(&self, code: i32) -> Option<&str> { self.g.iter().find(|(c, _)| *c == code).map(|(_, v)| v.as_str()) }
    fn all(&self, code: i32) -> Vec<f64> { self.g.iter().filter(|(c, _)| *c == code).filter_map(|(_, v)| v.parse().ok()).collect() }
    fn flag(&self) -> i32 { self.f(70).unwrap_or(0.0) as i32 }
}

struct Path { pts: Vec<P>, closed: bool }

#[derive(Default)]
struct Doc {
    units_mm: f64,
    blocks: BTreeMap<String, (P, Vec<Ent>)>,
    ents: Vec<Ent>,
}

fn parse(text: &str) -> Result<Doc, String> {
    let mut lines = text.lines();
    let mut pairs: Vec<(i32, String)> = Vec::new();
    while let (Some(c), Some(v)) = (lines.next(), lines.next()) {
        let Ok(code) = c.trim().parse::<i32>() else { return Err("This does not look like an ASCII DXF file.".into()) };
        pairs.push((code, v.trim().to_string()));
    }
    let mut doc = Doc { units_mm: 1.0, ..Default::default() };
    let mut i = 0;
    let mut section = String::new();
    let mut cur_block: Option<(String, P)> = None;
    let mut cur: Vec<Ent> = Vec::new();
    let mut insunits = 0;
    let mut measurement = 1;
    while i < pairs.len() {
        let (c, v) = (&pairs[i].0, &pairs[i].1);
        if *c == 0 && v == "SECTION" { section = pairs.get(i + 1).map(|p| p.1.clone()).unwrap_or_default(); i += 2; continue; }
        if *c == 0 && v == "ENDSEC" { if section == "BLOCKS" { cur.clear(); } section.clear(); i += 1; continue; }
        if section == "HEADER" {
            if *c == 9 && v == "$INSUNITS" { if let Some(p) = pairs.get(i + 1) { insunits = p.1.parse().unwrap_or(0); } }
            if *c == 9 && v == "$MEASUREMENT" { if let Some(p) = pairs.get(i + 1) { measurement = p.1.parse().unwrap_or(1); } }
            i += 1;
            continue;
        }
        if (section == "ENTITIES" || section == "BLOCKS") && *c == 0 {
            // one entity: this record up to the next code 0
            let mut j = i + 1;
            while j < pairs.len() && pairs[j].0 != 0 { j += 1; }
            let e = Ent { kind: v.clone(), g: pairs[i + 1..j].to_vec() };
            if section == "BLOCKS" && e.kind == "BLOCK" {
                let name = e.s(2).unwrap_or("").to_string();
                cur_block = Some((name, [e.f(10).unwrap_or(0.0), e.f(20).unwrap_or(0.0)]));
                cur.clear();
            } else if section == "BLOCKS" && e.kind == "ENDBLK" {
                if let Some((n, base)) = cur_block.take() { doc.blocks.insert(n, (base, std::mem::take(&mut cur))); }
            } else if section == "BLOCKS" { cur.push(e); } else { doc.ents.push(e); }
            i = j;
            continue;
        }
        i += 1;
    }
    doc.units_mm = match insunits {
        1 => 25.4, 2 => 304.8, 3 => 1609344.0, 5 => 10.0, 6 => 1000.0, 7 => 1e6, 8 => 25.4e-6, 9 => 0.0254,
        10 => 914.4, 13 => 1e-3, 14 => 100.0, 15 => 1e4,
        0 if measurement == 0 => 25.4, // unitless: the header's imperial/metric flag is the best hint
        _ => 1.0,
    };
    if doc.ents.is_empty() && doc.blocks.is_empty() { return Err("The file has no drawing entities.".into()); }
    Ok(doc)
}

fn arc_pts(c: P, r: f64, a0: f64, a1: f64, out: &mut Vec<P>) {
    // a1 > a0, counter-clockwise; includes both ends
    let step = if r > TOL { (2.0 * (1.0 - TOL / r).acos()).clamp(0.02, 0.5) } else { 0.5 };
    let n = (((a1 - a0) / step).ceil() as usize).clamp(1, 4000);
    for k in 0..=n { let a = a0 + (a1 - a0) * k as f64 / n as f64; out.push([c[0] + r * a.cos(), c[1] + r * a.sin()]); }
}

/// Polyline vertices with bulges to points. A positive bulge is a counter-clockwise arc;
/// bulge = tan(sweep / 4).
fn bulged(v: &[(P, f64)], closed: bool) -> Vec<P> {
    let mut out = Vec::new();
    let n = v.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    for i in 0..segs {
        let (p0, b) = v[i];
        let p1 = v[(i + 1) % n].0;
        out.push(p0);
        let d = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2)).sqrt();
        if b.abs() > 1e-9 && d > 1e-12 {
            let th = 4.0 * b.atan();
            let r = (d / (2.0 * (th / 2.0).sin())).abs();
            let mid = [(p0[0] + p1[0]) / 2.0, (p0[1] + p1[1]) / 2.0];
            let k = d / 2.0 / (th / 2.0).tan();
            let c = [mid[0] - (p1[1] - p0[1]) / d * k, mid[1] + (p1[0] - p0[0]) / d * k];
            let a0 = (p0[1] - c[1]).atan2(p0[0] - c[0]);
            let step = if r > TOL { (2.0 * (1.0 - TOL / r).acos()).clamp(0.02, 0.5) } else { 0.5 };
            let m = ((th.abs() / step).ceil() as usize).clamp(1, 4000);
            for j in 1..m { let a = a0 + th * j as f64 / m as f64; out.push([c[0] + r * a.cos(), c[1] + r * a.sin()]); }
        }
    }
    if !closed { if let Some(l) = v.last() { out.push(l.0); } }
    out
}

/// Uniform-parameter B-spline through its control points (non-rational), flattened.
fn spline_pts(degree: usize, knots: &[f64], ctrl: &[P]) -> Vec<P> {
    let n = ctrl.len();
    if n < 2 { return ctrl.to_vec(); }
    let p = degree.clamp(1, n - 1);
    let knots: Vec<f64> = if knots.len() == n + p + 1 { knots.to_vec() } else {
        // clamped uniform
        (0..n + p + 1).map(|i| if i <= p { 0.0 } else if i >= n { (n - p) as f64 } else { (i - p) as f64 }).collect()
    };
    let (t0, t1) = (knots[p], knots[n]);
    if t1 <= t0 { return ctrl.to_vec(); }
    let len: f64 = ctrl.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum();
    let steps = ((len / (TOL * 20.0)) as usize).clamp(n * 4, 3000);
    let mut out = Vec::with_capacity(steps + 1);
    for s in 0..=steps {
        let t = if s == steps { t1 } else { t0 + (t1 - t0) * s as f64 / steps as f64 };
        // find the knot span
        let mut k = p;
        while k + 1 < n && t >= knots[k + 1] { k += 1; }
        let mut d: Vec<P> = (0..=p).map(|j| ctrl[k - p + j]).collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let (lo, hi) = (knots[k - p + j], knots[k + 1 + j - r]);
                let a = if hi > lo { (t - lo) / (hi - lo) } else { 0.0 };
                d[j] = [(1.0 - a) * d[j - 1][0] + a * d[j][0], (1.0 - a) * d[j - 1][1] + a * d[j][1]];
            }
        }
        out.push(d[p]);
    }
    out
}

enum Item { Path(Path), Circle(P, f64) }

fn flip(e: &Ent) -> bool { e.f(230).is_some_and(|z| z < 0.0) }

fn entities(ents: &[Ent], doc: &Doc, m: &M, depth: usize, out: &mut Vec<Item>, skipped: &mut BTreeMap<String, usize>) {
    let mut i = 0;
    while i < ents.len() {
        let e = &ents[i];
        i += 1;
        // mirrored extrusion (negative Z) flips X
        let mm = if flip(e) && matches!(e.kind.as_str(), "ARC" | "CIRCLE" | "LWPOLYLINE" | "ELLIPSE") { m.after(&M([-1.0, 0.0, 0.0, 1.0, 0.0, 0.0])) } else { *m };
        let pt = |x: &str, y: &str| -> Option<P> { Some([e.f(x.parse().ok()?)?, e.f(y.parse().ok()?)?]) };
        match e.kind.as_str() {
            "LINE" => {
                if let (Some(a), Some(b)) = (pt("10", "20"), pt("11", "21")) { out.push(Item::Path(Path { pts: vec![m.ap(a), m.ap(b)], closed: false })); }
            }
            "CIRCLE" => {
                if let (Some(c), Some(r)) = (pt("10", "20"), e.f(40)) {
                    let c2 = mm.ap(c);
                    // a uniform similarity keeps it a circle; anything else becomes a polygon
                    let sx = (mm.0[0].powi(2) + mm.0[1].powi(2)).sqrt();
                    let sy = (mm.0[2].powi(2) + mm.0[3].powi(2)).sqrt();
                    if (sx - sy).abs() < 1e-9 * sx.max(1e-12) { out.push(Item::Circle(c2, r * sx)); }
                    else { let mut v = Vec::new(); arc_pts(c, r, 0.0, TAU, &mut v); v.pop(); out.push(Item::Path(Path { pts: v.into_iter().map(|p| mm.ap(p)).collect(), closed: true })); }
                }
            }
            "ARC" => {
                if let (Some(c), Some(r), Some(a0), Some(a1)) = (pt("10", "20"), e.f(40), e.f(50), e.f(51)) {
                    let (a0, mut a1) = (a0.to_radians(), a1.to_radians());
                    while a1 <= a0 { a1 += TAU; }
                    let mut v = Vec::new();
                    arc_pts(c, r, a0, a1, &mut v);
                    out.push(Item::Path(Path { pts: v.into_iter().map(|p| mm.ap(p)).collect(), closed: false }));
                }
            }
            "ELLIPSE" => {
                if let (Some(c), Some(ax), Some(ratio)) = (pt("10", "20"), pt("11", "21"), e.f(40)) {
                    let (t0, mut t1) = (e.f(41).unwrap_or(0.0), e.f(42).unwrap_or(TAU));
                    while t1 <= t0 { t1 += TAU; }
                    let full = (t1 - t0 - TAU).abs() < 1e-6;
                    let a = (ax[0] * ax[0] + ax[1] * ax[1]).sqrt();
                    let n = (((a * (t1 - t0)) / (TOL * 10.0)) as usize).clamp(24, 4000);
                    let mut v: Vec<P> = (0..=n).map(|k| {
                        let t = t0 + (t1 - t0) * k as f64 / n as f64;
                        let (x, y) = (t.cos(), ratio * t.sin());
                        // major axis (ax) and minor axis (ratio * perpendicular)
                        mm.ap([c[0] + ax[0] * x - ax[1] * y, c[1] + ax[1] * x + ax[0] * y])
                    }).collect();
                    if full { v.pop(); }
                    out.push(Item::Path(Path { pts: v, closed: full }));
                }
            }
            "LWPOLYLINE" => {
                // each vertex starts at a code 10; 42 (bulge) follows its vertex
                let mut v: Vec<(P, f64)> = Vec::new();
                let mut x = None;
                for (c, val) in &e.g {
                    let Ok(f) = val.parse::<f64>() else { continue };
                    match c { 10 => x = Some(f), 20 => if let Some(px) = x.take() { v.push(([px, f], 0.0)); }, 42 => if let Some(l) = v.last_mut() { l.1 = f; }, _ => {} }
                }
                let closed = e.flag() & 1 != 0;
                if v.len() >= 2 { out.push(Item::Path(Path { pts: bulged(&v, closed).into_iter().map(|p| mm.ap(p)).collect(), closed })); }
            }
            "POLYLINE" => {
                let mut v: Vec<(P, f64)> = Vec::new();
                while i < ents.len() && ents[i].kind != "SEQEND" {
                    let vt = &ents[i];
                    i += 1;
                    if vt.kind == "VERTEX" && vt.flag() & (16 | 64 | 128) == 0 {
                        if let (Some(x), Some(y)) = (vt.f(10), vt.f(20)) { v.push(([x, y], vt.f(42).unwrap_or(0.0))); }
                    }
                }
                i += 1; // SEQEND
                let fl = e.flag();
                if fl & (16 | 64) != 0 { *skipped.entry("3D mesh".into()).or_default() += 1; }
                else if v.len() >= 2 {
                    let closed = fl & 1 != 0;
                    out.push(Item::Path(Path { pts: bulged(&v, closed).into_iter().map(|p| mm.ap(p)).collect(), closed }));
                }
            }
            "SPLINE" => {
                let fl = e.flag();
                let xs: Vec<f64> = e.all(10); let ys: Vec<f64> = e.all(20);
                let ctrl: Vec<P> = xs.iter().zip(&ys).map(|(a, b)| [*a, *b]).collect();
                let fx = e.all(11); let fy = e.all(21);
                let fit: Vec<P> = fx.iter().zip(&fy).map(|(a, b)| [*a, *b]).collect();
                let pts = if ctrl.len() >= 2 { spline_pts(e.f(71).unwrap_or(3.0) as usize, &e.all(40), &ctrl) } else { fit };
                let closed = fl & 1 != 0;
                if pts.len() >= 2 { out.push(Item::Path(Path { pts: pts.into_iter().map(|p| m.ap(p)).collect(), closed })); }
            }
            "INSERT" => {
                let Some((base, ents2)) = e.s(2).and_then(|n| doc.blocks.get(n)) else { continue };
                if depth >= 8 { continue; }
                let (sx, sy) = (e.f(41).unwrap_or(1.0), e.f(42).unwrap_or(1.0));
                let rot = e.f(50).unwrap_or(0.0).to_radians();
                let (ix, iy) = (e.f(10).unwrap_or(0.0), e.f(20).unwrap_or(0.0));
                let (cs, sn) = (rot.cos(), rot.sin());
                // translate(-base), scale, rotate, translate(insert point)
                let t_base = M([1.0, 0.0, 0.0, 1.0, -base[0], -base[1]]);
                let sc = M([sx, 0.0, 0.0, sy, 0.0, 0.0]);
                let ro = M([cs, sn, -sn, cs, 0.0, 0.0]);
                let tr = M([1.0, 0.0, 0.0, 1.0, ix, iy]);
                let local = tr.after(&ro).after(&sc).after(&t_base);
                entities(ents2, doc, &m.after(&local), depth + 1, out, skipped);
            }
            "SEQEND" | "VERTEX" | "ATTRIB" | "ENDBLK" => {}
            other => { *skipped.entry(other.to_string()).or_default() += 1; }
        }
    }
}

fn near(a: P, b: P) -> bool { (a[0] - b[0]).abs() <= JOIN && (a[1] - b[1]).abs() <= JOIN && ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() <= JOIN }

/// Join open paths whose ends touch into longer paths; a chain that returns to its start is closed.
fn join(paths: Vec<Path>) -> Vec<Path> {
    let (mut done, open): (Vec<Path>, Vec<Path>) = paths.into_iter().partition(|p| p.closed);
    let mut open: Vec<Option<Path>> = open.into_iter().filter(|p| p.pts.len() >= 2).map(Some).collect();
    for s in 0..open.len() {
        let Some(mut chain) = open[s].take() else { continue };
        loop {
            let mut grew = false;
            for k in 0..open.len() {
                let Some(p) = open[k].as_ref() else { continue };
                let (cs, ce) = (chain.pts[0], *chain.pts.last().unwrap());
                let (ps, pe) = (p.pts[0], *p.pts.last().unwrap());
                let mut add: Option<Vec<P>> = None;
                let mut front = false;
                if near(ce, ps) { add = Some(p.pts[1..].to_vec()); }
                else if near(ce, pe) { add = Some(p.pts.iter().rev().skip(1).cloned().collect()); }
                else if near(cs, pe) { add = Some(p.pts[..p.pts.len() - 1].to_vec()); front = true; }
                else if near(cs, ps) { add = Some(p.pts.iter().skip(1).rev().cloned().collect()); front = true; }
                if let Some(a) = add {
                    open[k] = None;
                    if front { let mut n = a; n.append(&mut chain.pts); chain.pts = n; } else { chain.pts.extend(a); }
                    grew = true;
                    break;
                }
            }
            if !grew { break; }
        }
        if chain.pts.len() > 2 && near(chain.pts[0], *chain.pts.last().unwrap()) { chain.pts.pop(); chain.closed = true; }
        done.push(chain);
    }
    done
}

pub fn import(data: &[u8]) -> Result<Imported, String> {
    if data.starts_with(b"AutoCAD Binary DXF") { return Err("Binary DXF files are not supported; save the drawing as ASCII DXF.".into()); }
    let text = String::from_utf8_lossy(data);
    let doc = parse(&text).map_err(|e| format!("Could not read DXF: {e}"))?;
    let k = doc.units_mm;
    let mut items = Vec::new();
    let mut skipped = BTreeMap::new();
    entities(&doc.ents, &doc, &M([k, 0.0, 0.0, k, 0.0, 0.0]), 0, &mut items, &mut skipped);

    let mut paths = Vec::new();
    let mut loops = Vec::new();
    for it in items {
        match it {
            Item::Circle(c, r) => if r > 1e-6 { loops.push(shapes::new_circle(c, r, 0)); },
            Item::Path(p) => paths.push(p),
        }
    }
    for p in join(paths) {
        if let Some(l) = shapes::new_poly(&p.pts, p.closed, 0) { loops.push(l); }
    }
    let mut warnings = Vec::new();
    if !skipped.is_empty() {
        let list: Vec<String> = skipped.iter().map(|(k, n)| format!("{n} {k}")).collect();
        warnings.push(format!("Skipped entities that cannot be cut: {}.", list.join(", ")));
    }
    if loops.is_empty() { warnings.push("The DXF contains no 2D outlines.".into()); }
    let (w, h) = match shapes::bbox_many(loops.iter()) { Some(b) => (b[2] - b[0], b[3] - b[1]), None => (0.0, 0.0) };
    Ok(Imported { loops, width_mm: w, height_mm: h, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dxf(units: i32, body: &str) -> Vec<u8> {
        format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{units}\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{body}0\nENDSEC\n0\nEOF\n").into_bytes()
    }
    fn line(a: P, b: P) -> String { format!("0\nLINE\n8\n0\n10\n{}\n20\n{}\n11\n{}\n21\n{}\n", a[0], a[1], b[0], b[1]) }

    #[test]
    fn loose_lines_become_a_closed_rectangle() {
        let body = [line([0.0, 0.0], [40.0, 0.0]), line([40.0, 20.0], [40.0, 0.0]), line([40.0, 20.0], [0.0, 20.0]), line([0.0, 20.0], [0.0, 0.0])].concat();
        let r = import(&dxf(4, &body)).unwrap();
        assert_eq!(r.loops.len(), 1);
        assert!(r.loops[0].closed());
        assert_eq!(r.loops[0].pts.len(), 8);
        assert!((r.width_mm - 40.0).abs() < 1e-6 && (r.height_mm - 20.0).abs() < 1e-6);
    }

    #[test]
    fn inches_are_converted() {
        let r = import(&dxf(1, &line([0.0, 0.0], [2.0, 0.0]))).unwrap();
        assert!((r.width_mm - 50.8).abs() < 1e-6);
    }

    #[test]
    fn circles_stay_circles() {
        let r = import(&dxf(4, "0\nCIRCLE\n10\n10\n20\n10\n40\n5\n")).unwrap();
        assert_eq!(r.loops.len(), 1);
        assert!(shapes::is_circle(&r.loops[0]));
        assert!((r.width_mm - 10.0).abs() < 0.01);
    }

    #[test]
    fn arc_ends_join_lines_into_a_slot() {
        // a 20 x 10 slot: two lines and two half circles (arcs)
        let body = [
            line([0.0, 0.0], [20.0, 0.0]),
            "0\nARC\n10\n20\n20\n5\n40\n5\n50\n-90\n51\n90\n".to_string(),
            line([20.0, 10.0], [0.0, 10.0]),
            "0\nARC\n10\n0\n20\n5\n40\n5\n50\n90\n51\n270\n".to_string(),
        ].concat();
        let r = import(&dxf(4, &body)).unwrap();
        assert_eq!(r.loops.len(), 1);
        assert!(r.loops[0].closed());
        assert!((r.width_mm - 30.0).abs() < 0.05 && (r.height_mm - 10.0).abs() < 0.05, "{} x {}", r.width_mm, r.height_mm);
    }

    #[test]
    fn lwpolyline_with_bulge_makes_a_rounded_shape() {
        // a unit square whose right side is a half circle bulging outwards (bulge 1 = semicircle)
        let body = "0\nLWPOLYLINE\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n42\n1\n10\n10\n20\n10\n10\n0\n20\n10\n";
        let r = import(&dxf(4, body)).unwrap();
        assert_eq!(r.loops.len(), 1);
        assert!(r.loops[0].closed());
        assert!((r.width_mm - 15.0).abs() < 0.05, "width {}", r.width_mm);
        assert!((r.height_mm - 10.0).abs() < 0.05);
    }

    #[test]
    fn block_inserts_are_placed_scaled_and_rotated() {
        let text = "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nSQ\n10\n0\n20\n0\n0\nLWPOLYLINE\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n10\n0\n20\n10\n0\nENDBLK\n0\nENDSEC\n\
                    0\nSECTION\n2\nENTITIES\n0\nINSERT\n2\nSQ\n10\n100\n20\n50\n41\n2\n42\n1\n50\n90\n0\nENDSEC\n0\nEOF\n";
        let r = import(text.as_bytes()).unwrap();
        assert_eq!(r.loops.len(), 1);
        // 20 x 10 before the 90 degree turn, so 10 wide and 20 tall
        assert!((r.width_mm - 10.0).abs() < 0.01 && (r.height_mm - 20.0).abs() < 0.01, "{} x {}", r.width_mm, r.height_mm);
        let b = shapes::bbox(&r.loops[0]).unwrap();
        assert!((b[0] - 90.0).abs() < 0.01 && (b[1] - 50.0).abs() < 0.01, "{b:?}");
    }

    #[test]
    fn spline_is_flattened_and_unknown_things_are_reported() {
        let body = "0\nSPLINE\n70\n8\n71\n3\n10\n0\n20\n0\n10\n5\n20\n10\n10\n10\n20\n10\n10\n15\n20\n0\n0\nTEXT\n10\n0\n20\n0\n1\nhello\n";
        let r = import(&dxf(4, body)).unwrap();
        assert_eq!(r.loops.len(), 1);
        assert!(!r.loops[0].closed());
        assert!((r.width_mm - 15.0).abs() < 0.01, "width {}", r.width_mm);
        assert!(r.warnings.iter().any(|w| w.contains("1 TEXT")));
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(import(b"hello world").is_err());
        assert!(import(b"AutoCAD Binary DXF\r\n\x1a\x00").is_err());
    }
}
