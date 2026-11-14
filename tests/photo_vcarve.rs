//! Photo V-carve: a picture etched with a V-bit along parallel lines, deeper where it is darker.
use putiacnc_core::model::{ProjectData, Relief};
use putiacnc_core::{checks, generate, setup, shapes, Units};

/// A 60 x 40 mm picture that goes from white on the left to black on the right, on a 100 x 100 x 12 mm stock.
fn project() -> ProjectData {
    let mut f = shapes::blank_project();
    let d = &mut f.data;
    d.stock.w = 100.0; d.stock.h = 100.0; d.stock.t = 12.0;
    d.tools = setup::starter_tools(setup::ToolChoice { vbits: true, ..Default::default() });
    let (nx, ny) = (240usize, 160usize);
    let mut b = Vec::new();
    for _ in 0..ny { for i in 0..nx { b.extend(((65535 - i * 65535 / (nx - 1)) as u16).to_le_bytes()); } }
    d.relief = Some(Relief { name: "gradient".into(), nx, ny, res: 0.25, zmax: 3.0, hm: b64(&b), x: 20.0, y: 30.0, scale: 1.0 });
    shapes::add_operation(d, &Default::default(), "photo").unwrap();
    d.clone()
}

/// The cutting moves (G1) of a program as (x, y, z), carrying over the axes a line leaves out.
fn cuts(g: &str) -> Vec<[f64; 3]> {
    let (mut cur, mut out) = ([0.0f64; 3], Vec::new());
    for l in g.lines() {
        let g1 = l.starts_with("G1");
        if !(g1 || l.starts_with("G0")) { continue; }
        for w in l.split_whitespace() {
            let v = w[1..].parse::<f64>().ok();
            match (w.as_bytes()[0], v) { (b'X', Some(v)) => cur[0] = v, (b'Y', Some(v)) => cur[1] = v, (b'Z', Some(v)) => cur[2] = v, _ => {} }
        }
        if g1 { out.push(cur); }
    }
    out
}

#[test]
fn the_picture_is_cut_deeper_where_it_is_darker_and_the_checks_are_quiet() {
    let d = project();
    let op = &d.ops[0].p;
    assert_eq!(op.kind, "photo");
    let t = d.tools.iter().find(|t| t.id == op.tool).unwrap();
    assert!(t.shape == "engrave" || t.shape == "vbit", "{}", t.shape);
    let out = generate(&d, Units::Metric).unwrap();
    let g = out.gcode();
    let m = cuts(&g);
    assert!(m.len() >= 90, "{}", m.len());
    let deepest = m.iter().map(|p| p[2]).fold(0.0, f64::min);
    assert!((deepest + op.depth).abs() < 0.05, "deepest {deepest} for a depth of {}", op.depth);
    // the left (white) part has no groove: no cutting move there is below the surface
    let left: Vec<f64> = m.iter().filter(|p| p[0] < 28.0).map(|p| p[2]).collect();
    assert!(left.iter().all(|z| *z > -0.4), "{:?}", left.iter().cloned().fold(0.0, f64::min));
    let found: Vec<String> = checks::run(&d, &out).into_iter().map(|f| f.msg).collect();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn both_directions_cut_twice_as_much_and_other_bits_are_refused() {
    let mut d = project();
    let one = generate(&d, Units::Metric).unwrap().lines.len();
    d.ops[0].p.f_dir = Some("xy".into());
    let two = generate(&d, Units::Metric).unwrap().lines.len();
    assert!(two > one * 3 / 2, "{one} {two}");
    d.ops[0].p.tool = setup::starter_tools(setup::ToolChoice::default())[0].id.clone();
    d.tools.extend(setup::starter_tools(setup::ToolChoice::default()).into_iter().take(1).map(|mut t| { t.id = d.ops[0].p.tool.clone(); t }));
    assert!(generate(&d, Units::Metric).is_err());
}

#[test]
fn without_a_picture_there_is_nothing_to_cut() {
    let mut d = project();
    d.relief = None;
    assert!(generate(&d, Units::Metric).is_err());
}

fn b64(d: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        o.push(A[(n >> 18) as usize & 63] as char); o.push(A[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        o.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    o
}
