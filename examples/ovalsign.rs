//! Builds the "Oval sign" sample: an oval plate with its name V-carved in the middle, a line above and below, a fine border line,
#![allow(clippy::too_many_arguments)]
//! and the oval cut out with four holding tabs.
//! Usage: cargo run --release --example ovalsign -- <out.json> <main font file> <main font name> <small font file> <small font name>
use putiacnc_core::model::{Operation, ShapeGroup, TextSpec};
use putiacnc_core::{save_project, shapes, text};

fn add_text(d: &mut putiacnc_core::ProjectData, gid: i64, op: i64, font: &[u8], label: &str, s: &str, size: f64, spacing: f64, centre: [f64; 2]) {
    let style = text::TextStyle { size_mm: size, spacing_mm: spacing, word_spacing_mm: 0.0, kerning: true, align: text::Align::Center, flow: text::Flow::Straight, radius_mm: 0.0, arc_deg: 90.0, arc_in: false, shadow: [0.0, 0.0] };
    let mut t = text::text_loops(font, 0, s, style, [centre[0], centre[1] - size / 2.0], op).unwrap();
    // centre the ink, not the advance widths
    let bb = shapes::bbox_many(t.iter()).unwrap();
    let dx = centre[0] - (bb[0] + bb[2]) / 2.0;
    for l in &mut t { shapes::translate(l, dx, 0.0); l.grp = Some(gid); }
    d.groups.push(ShapeGroup { id: gid, name: format!("Text: {s}"), locked: false, text: Some(TextSpec {
        content: s.into(), font: label.into(), size, spacing, word_spacing: 0.0, kerning: true, align: text::Align::Center,
        flow: text::Flow::Straight, radius: 0.0, arc_deg: 90.0, arc_in: false, fit_width: 0.0, shadow: [0.0, 0.0], path: vec![] }) });
    d.loops.append(&mut t);
}

/// A line that follows the inside of a closed shape: `deg` is where it sits (90 top, 270 bottom), `gap` the distance of the letters' outer edge
/// from the shape's edge, and `face_in` is true for letters that stand towards the middle (the bottom line, which then reads left to right).
fn add_shape_text(d: &mut putiacnc_core::ProjectData, gid: i64, op: i64, font: &[u8], label: &str, s: &str, size: f64, spacing: f64, path: &[[f64; 2]], gap: f64, deg: f64, face_in: bool) {
    let style = text::TextStyle { size_mm: size, spacing_mm: spacing, word_spacing_mm: 0.0, kerning: true, align: text::Align::Center, flow: text::Flow::Shape, radius_mm: gap, arc_deg: deg, arc_in: face_in, shadow: [0.0, 0.0] };
    let flat = text::text_loops(font, 0, s, TextStyle_straight(style), [0.0, 0.0], op).unwrap();
    let mut t = text::along_shape(&flat, path, size, gap, deg, face_in).unwrap();
    for l in &mut t { l.grp = Some(gid); }
    d.groups.push(ShapeGroup { id: gid, name: format!("Text: {s}"), locked: false, text: Some(TextSpec {
        content: s.into(), font: label.into(), size, spacing, word_spacing: 0.0, kerning: true, align: text::Align::Center,
        flow: text::Flow::Shape, radius: gap, arc_deg: deg, arc_in: face_in, fit_width: 0.0, shadow: [0.0, 0.0], path: path.to_vec() }) });
    d.loops.append(&mut t);
}

#[allow(non_snake_case)]
fn TextStyle_straight(mut s: text::TextStyle) -> text::TextStyle { s.flow = text::Flow::Straight; s }

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 6 { eprintln!("Usage: ovalsign <out.json> <main font file> <main font name> <small font file> <small font name>"); std::process::exit(2); }
    let main_font = std::fs::read(&a[2]).expect("main font");
    let small_font = std::fs::read(&a[4]).expect("small font");
    let mut f = shapes::blank_project();
    f.name = "Oval sign".into();
    let d = &mut f.data;
    d.stock.w = 260.0; d.stock.h = 170.0; d.stock.t = 15.0;
    let find = |d: &putiacnc_core::ProjectData, name: &str| d.tools.iter().find(|t| t.name.starts_with(name)).map(|t| t.id.clone()).unwrap();
    let (vbit, engr, mill) = (find(d, "60° V-bit"), find(d, "30° engraving"), find(d, "3 mm flat"));
    let op = |d: &mut putiacnc_core::ProjectData, id: i64, kind: &str, tool: &str, depth: f64| {
        let mut p = shapes::default_op_params(tool);
        p.kind = kind.into(); p.depth = depth;
        if kind == "outside" { p.tabs = true; p.tab_n = Some(4.0); p.tab_w = Some(6.0); p.tab_h = Some(3.0); p.pass_depth = Some(3.0); }
        d.ops.push(Operation { id, p });
        d.oid = id; d.act = id;
    };
    op(d, 1, "vcarve", &vbit, 2.4);
    op(d, 2, "vcarve", &engr, 1.0);
    op(d, 3, "online", &engr, 0.5);
    op(d, 4, "outside", &mill, 15.0);
    let (cx, cy) = (d.stock.w / 2.0, d.stock.h / 2.0);
    let (rx, ry) = (112.0, 72.0);
    d.loops.push(shapes::new_ellipse([cx - rx, cy - ry], [cx + rx, cy + ry], 4));
    d.loops.push(shapes::new_ellipse([cx - rx + 7.0, cy - ry + 7.0], [cx + rx - 7.0, cy + ry - 7.0], 3));
    add_text(d, 1, 1, &main_font, &a[3], "PutiaCNC", 25.0, 0.8, [cx, cy + 2.0]);
    let oval = shapes::points(&d.loops[0]);
    add_shape_text(d, 2, 2, &small_font, &a[5], "toolpath studio", 7.5, 1.4, &oval, 12.0, 90.0, false);
    add_shape_text(d, 3, 2, &small_font, &a[5], "free open source software", 5.5, 1.1, &oval, 12.5, 270.0, true);
    std::fs::write(&a[1], save_project(&f).unwrap()).unwrap();
}
