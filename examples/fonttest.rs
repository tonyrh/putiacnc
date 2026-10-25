//! Writes the outlines of a text as an SVG (even-odd fill, like the app) to check a font: fonttest <font> <out.svg>
use putiacnc_core::text::{text_loops, Align, TextStyle};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let f = std::fs::read(&a[0]).unwrap();
    let st = TextStyle::plain(20.0, 0.0, Align::Left);
    let r = text_loops(&f, 0, "PutiaCNC Rag", st, [2.0, 8.0], 1).unwrap();
    let mut d = String::new();
    for l in &r {
        for (i, c) in l.pts.chunks(2).enumerate() { d += &format!("{}{:.2},{:.2} ", if i == 0 { "M" } else { "L" }, c[0] as f64 / 1000.0, 40.0 - c[1] as f64 / 1000.0); }
        d += "Z ";
    }
    std::fs::write(&a[1], format!("<svg xmlns='http://www.w3.org/2000/svg' width='1000' height='320' viewBox='0 0 125 40'><rect width='125' height='40' fill='white'/><path d='{d}' fill='black' fill-rule='evenodd'/></svg>")).unwrap();
    println!("{} loops", r.len());
}
