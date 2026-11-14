//! Writes the projects of the gallery on the project page (docs/gallery.html): the sample projects of the program, as plain project files
//! that fit a 300 x 200 x 60 mm machine with the starter tools. Usage: cargo run --release --example gallery -- docs/gallery
use putiacnc_core::sample::Kind;
use putiacnc_core::{sample, save_project, shapes};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "docs/gallery".into());
    std::fs::create_dir_all(&out).unwrap();
    for (id, kind) in [("name-plate", Kind::NamePlate), ("finger-joint-box", Kind::Box), ("oval-sign", Kind::Oval), ("relief-rosette", Kind::Relief), ("relief-from-shapes", Kind::ShapedRelief)] {
        let mut f = shapes::blank_project();
        sample::fill_kind(&mut f.data, false, kind).unwrap();
        f.name = kind.title().into();
        let path = format!("{out}/{id}.project.json");
        std::fs::write(&path, save_project(&f).unwrap()).unwrap();
        println!("{path}");
    }
}
