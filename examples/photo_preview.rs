//! Writes the G-code of a Photo V-carve of a picture: `cargo run --release --example photo_preview -- picture.jpg out.gcode [width_mm]`
use putiacnc_core::relief::{shrink_picture, PicParams};
use putiacnc_core::model::Relief;
use putiacnc_core::{generate, setup, shapes, Units};
fn env(k: &str, d: f64) -> f64 { std::env::var(k).ok().and_then(|s| s.parse().ok()).unwrap_or(d) }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let w: f64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(93.4);
    let bytes = shrink_picture(&std::fs::read(&a[1]).unwrap(), 1500).unwrap();
    let p = PicParams { width_mm: w, smooth_mm: 0.15, zmax: 1.0, gamma: env("GAMMA", 1.0), contrast: env("CONTRAST", 1.0), brightness: env("BRIGHT", 0.0), lighten_bg: env("LIGHTEN", 0.0), ..Default::default() };
    let mut rel = Relief::from_picture(&bytes, "photo", &p, 600).unwrap();
    let mut f = shapes::blank_project();
    let d = &mut f.data;
    d.stock.w = 200.0; d.stock.h = 150.0; d.stock.t = 12.0;
    d.tools = setup::starter_tools(setup::ToolChoice { vbits: true, ..Default::default() });
    let (mw, mh) = rel.size();
    rel.x = (d.stock.w - mw) / 2.0; rel.y = (d.stock.h - mh) / 2.0;
    d.relief = Some(rel);
    shapes::add_operation(d, &Default::default(), "photo").unwrap();
    if std::env::var("XY").is_ok() { d.ops[0].p.f_dir = Some("xy".into()); }
    let out = generate(d, Units::Metric).unwrap();
    std::fs::write(&a[2], out.gcode()).unwrap();
}
