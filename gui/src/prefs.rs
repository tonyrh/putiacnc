//! What the app remembers between runs (settings.json in the config folder): window size and place,
//! which panels are open, view options, the folders last used and the recent files.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Win {
    pub w: f32,
    pub h: f32,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub maximized: bool,
}

/// Every field is optional: a missing one means "keep the app's own default".
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Settings {
    pub update_check: bool,
    pub window: Option<Win>,
    pub show_left: Option<bool>,
    pub show_right: Option<bool>,
    pub right_tab: Option<String>,
    pub grid_on: Option<bool>,
    pub snap_on: Option<bool>,
    pub snap_mm: Option<f64>,
    pub show_toolpath: Option<bool>,
    pub show_rapids: Option<bool>,
    pub show_machine_area: Option<bool>,
    pub show_rulers: Option<bool>,
    pub show_axes: Option<bool>,
    pub tool_len_in: Option<bool>,
    pub tool_speed_idx: Option<usize>,
    pub mach_len_in: Option<bool>,
    pub mach_speed_idx: Option<usize>,
    pub dir_project: Option<String>,
    pub dir_svg: Option<String>,
    pub dir_gcode: Option<String>,
    /// The last settings of the bitmap trace dialog (the width is not kept: it depends on the picture and the stock).
    pub trace: Option<putiacnc_core::bitmap::Opts>,
    /// Which way is up when importing a 3D model, per file type.
    pub model_orient_obj: Option<putiacnc_core::relief::Orient>,
    pub model_orient_stl: Option<putiacnc_core::relief::Orient>,
    pub recent: Vec<String>,
}

pub const MAX_RECENT: usize = 10;

fn path() -> Option<PathBuf> { crate::toolmgr::config_dir().map(|d| d.join("settings.json")) }

pub fn load() -> Settings {
    path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(s: &Settings) {
    if let Some(p) = path() {
        if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
        if let Ok(j) = serde_json::to_string_pretty(s) { let _ = std::fs::write(p, j); }
    }
}

/// Change one thing in the stored settings without touching the rest.
pub fn modify(f: impl FnOnce(&mut Settings)) {
    let mut s = load();
    f(&mut s);
    save(&s);
}

/// The folder of a file path, if it still exists.
pub fn dir_of(p: &std::path::Path) -> Option<String> {
    p.parent().filter(|d| d.is_dir()).map(|d| d.display().to_string())
}

/// A remembered folder, only if it still exists.
pub fn existing_dir(d: &Option<String>) -> Option<PathBuf> {
    d.as_ref().map(PathBuf::from).filter(|p| p.is_dir())
}

/// Puts `path` first in the recent list (no duplicates, at most `MAX_RECENT`).
pub fn push_recent(list: &mut Vec<String>, path: &str) {
    let abs = std::fs::canonicalize(path).map(|p| p.display().to_string()).unwrap_or_else(|_| path.to_string());
    list.retain(|p| *p != abs);
    list.insert(0, abs);
    list.truncate(MAX_RECENT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_and_partial_files_still_load() {
        let s: Settings = serde_json::from_str(r#"{"update_check": true}"#).unwrap();
        assert!(s.update_check && s.window.is_none() && s.recent.is_empty());
        let s: Settings = serde_json::from_str(r#"{"window":{"w":1200.0,"h":700.0},"future_key":1}"#).unwrap();
        assert_eq!(s.window.unwrap().w, 1200.0);
        assert!(serde_json::from_str::<Settings>("not json").is_err());
    }

    #[test]
    fn round_trip_and_recent_list() {
        let mut s = Settings { snap_mm: Some(0.5), show_left: Some(false), right_tab: Some("gcode".into()), ..Default::default() };
        s.window = Some(Win { w: 1000.0, h: 600.0, x: Some(10.0), y: None, maximized: true });
        let t: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s, t);
        let mut r = vec![];
        for i in 0..14 { push_recent(&mut r, &format!("/nonexistent/p{i}.json")); }
        assert_eq!(r.len(), MAX_RECENT);
        assert_eq!(r[0], "/nonexistent/p13.json");
        push_recent(&mut r, "/nonexistent/p10.json");
        assert_eq!(r[0], "/nonexistent/p10.json");
        assert_eq!(r.iter().filter(|p| p.ends_with("p10.json")).count(), 1);
    }
}
