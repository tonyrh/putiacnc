//! Builds the project used for the README screenshots: a rounded plate with "PutiaCNC" pocketed in it and four holes drilled through (as circles the size of the tool).
//! Usage: cargo run --release --example docproject -- <font.otf> <font name as the app lists it> <out.json>
//! The screenshots use the demo version of the E-SQUARE font (`E-SQUARE_demo.otf`, font name "E-SQUARE"). It is not part of the repository; supply your own copy.
use putiacnc_core::model::{Operation, ShapeGroup, TextSpec};
use putiacnc_core::{save_project, shapes, text};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 4 {
        eprintln!("Usage: cargo run --release --example docproject -- <font.otf> <font name as the app lists it> <out.json>");
        std::process::exit(2);
    }
    let (path, label, out) = (&a[1], &a[2], &a[3]);
    let font = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("Cannot read the font {path}: {e}");
        std::process::exit(1);
    });
    let mut f = shapes::blank_project();
    f.name = "Sign".into();
    let d = &mut f.data;
    d.stock.w = 240.0; d.stock.h = 160.0; d.stock.t = 18.0;
    let tool = d.tools.iter().find(|t| t.shape == "flat" && (t.dia - 3.175).abs() < 0.01).or_else(|| d.tools.iter().find(|t| t.shape == "flat")).map(|t| t.id.clone()).unwrap();
    let op = |d: &mut putiacnc_core::ProjectData, id: i64, kind: &str, depth: f64| {
        let mut p = shapes::default_op_params(&tool);
        p.kind = kind.into(); p.depth = depth;
        d.ops.push(Operation { id, p });
        d.oid = id; d.act = id;
    };
    op(d, 1, "outside", 18.0);
    op(d, 2, "pocket", 3.0);
    op(d, 3, "drill", 18.0);
    let (x0, y0, x1, y1) = (20.0, 15.0, 220.0, 145.0);
    d.loops.push(shapes::new_rect([x0, y0], [x1, y1], 16.0, 1));
    // the name, centred on the plate: capital height `size`, so the baseline sits half of it below the middle
    let size = 20.0;
    let style = text::TextStyle { size_mm: size, spacing_mm: 1.0, word_spacing_mm: 0.0, kerning: true, align: text::Align::Center, flow: text::Flow::Straight, radius_mm: 0.0, arc_deg: 90.0, arc_in: false, shadow: [0.0, 0.0] };
    let origin = [(x0 + x1) / 2.0, (y0 + y1) / 2.0 - size / 2.0];
    let mut t = text::text_loops(&font, 0, "PutiaCNC", style, origin, 2).unwrap();
    let bb = shapes::bbox_many(t.iter()).unwrap();
    println!("text {:.1} x {:.1} mm, centre {:.1},{:.1}", bb[2] - bb[0], bb[3] - bb[1], (bb[0] + bb[2]) / 2.0, (bb[1] + bb[3]) / 2.0);
    for l in &mut t { l.grp = Some(1); }
    d.groups.push(ShapeGroup { id: 1, name: "Text: PutiaCNC".into(), locked: false, text: Some(TextSpec {
        content: "PutiaCNC".into(), font: label.clone(), size, spacing: 1.0, word_spacing: 0.0, kerning: true, align: text::Align::Center,
        flow: text::Flow::Straight, radius: 0.0, arc_deg: 90.0, arc_in: false, fit_width: 0.0, shadow: [0.0, 0.0] }) });
    d.loops.append(&mut t);
    for [x, y] in [[x0 + 16.0, y0 + 16.0], [x1 - 16.0, y0 + 16.0], [x0 + 16.0, y1 - 16.0], [x1 - 16.0, y1 - 16.0]] { d.loops.push(shapes::new_circle([x, y], 3.175 / 2.0, 3)); }
    std::fs::write(out, save_project(&f).unwrap()).unwrap();
}
