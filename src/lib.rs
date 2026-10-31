// Style lints that do not suit this code: geometry loops index several arrays by one counter (clearer than zips), and drawing/toolpath helpers take many plain parameters. Rewriting them would only risk changing the G-code.
// Shapes keep their points flat (x, y, x, y ...), so `chunks_exact(2)` is the natural way to read them; newer clippy suggests `as_chunks`.
#![allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
#![allow(clippy::needless_range_loop, clippy::too_many_arguments, clippy::type_complexity, clippy::field_reassign_with_default, clippy::explicit_counter_loop, clippy::manual_clamp, clippy::while_let_loop, clippy::neg_cmp_op_on_partial_ord)]
pub mod arcs;
pub mod bitmap;
pub mod checks;
pub mod history;
pub mod selection;
pub mod summary;
pub mod verify;
pub mod cleanup;
pub mod clip;
pub mod machines;
pub mod model;
pub mod pieces;
pub mod relief;
pub mod replay;
pub mod shapes;
pub mod sim;
pub mod split;
pub mod dxf;
pub mod svg;
pub mod feeds;
pub mod linetext;
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
