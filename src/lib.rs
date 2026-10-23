pub mod arcs;
pub mod bitmap;
pub mod clip;
pub mod machines;
pub mod model;
pub mod relief;
pub mod shapes;
pub mod sim;
pub mod dxf;
pub mod svg;
pub mod text;
pub mod toollib;
pub mod toolpath;
pub mod trim;

pub use model::{ProjectFile, ProjectData, Units};
pub use toolpath::{generate, Output};

/// Load a project file exported by the browser app (or by this crate).
pub fn load_project(json: &str) -> Result<ProjectFile, String> {
    let f: ProjectFile = serde_json::from_str(json).map_err(|e| format!("Not a valid project file: {e}"))?;
    if f.app != "putia-cnc" { return Err("This is not a PutiaCNC project file.".into()); }
    let mut f = f;
    // older projects had one project-wide air-cut switch; it now lives on each operation
    if f.data.job.air { for o in f.data.ops.iter_mut() { o.p.air = true; } f.data.job.air = false; }
    Ok(f)
}

pub fn save_project(p:&ProjectFile)->Result<String,String>{ serde_json::to_string(p).map_err(|e|e.to_string()) }
