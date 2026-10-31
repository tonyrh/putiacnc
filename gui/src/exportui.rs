//! The export window: how to split the program into files, and the files that will be written, with their times.
use crate::app::{fmt_time, App};
use eframe::egui;
use putiacnc_core::split::{self, Split};

const OPTIONS: [(Split, &str, &str); 3] = [
    (Split::One, "One file", "The whole program, as it is shown in the G-code tab."),
    (Split::PerTool, "One file per tool", "A file for each run of operations with the same tool: change the tool between files, not in the middle."),
    (Split::PerOperation, "One file per operation", "Run each operation on its own, check the result, then start the next."),
];

/// The names and times of the files a split would write for `name` (the project name), without generating anything.
pub fn file_list(app: &App, mode: Split, name: &str) -> Vec<(String, f64)> {
    let times = crate::timeui::total_timing(app);
    let t = |ops: &[usize]| times.as_ref().map_or(0.0, |t| ops.iter().filter_map(|&i| t.ops.get(i)).map(|o| o.total()).sum());
    let mut v: Vec<(String, f64)> = vec![];
    if mode == Split::One {
        v.push((format!("{name}.nc"), times.as_ref().map_or(0.0, |t| t.ops.iter().map(|o| o.total()).sum::<f64>() + t.waits)));
    } else {
        for (ops, suffix, _) in split::plan(&app.project.data, mode) { v.push((format!("{name}{suffix}.nc"), t(&ops))); }
    }
    for pp in &app.pieces {
        let own = pp.output.timing(app.project.data.ops.len());
        let suffix = if app.pieces.len() == 1 { "-plug".to_string() } else { format!("-plug-{}", pp.job.op_index + 1) };
        v.push((format!("{name}{suffix}.nc"), own.ops.iter().map(|o| o.total()).sum::<f64>() + own.waits));
    }
    v
}

pub fn window(ctx: &egui::Context, app: &mut App) {
    // a split being made: keep the frames coming until the files are written
    if app.split_job.is_some() && app.poll_split() { ctx.request_repaint_after(std::time::Duration::from_millis(50)); }
    if !app.export_open { return; }
    let name = if app.project.name.trim().is_empty() { "output".to_string() } else { app.project.name.trim().to_string() };
    let (mut close, mut go) = (false, false);
    egui::Window::new("Export G-code").frame(crate::csd::dialog_frame(ctx)).title_bar(false).collapsible(false).movable(false).resizable(false)
        .pivot(egui::Align2::CENTER_CENTER).current_pos(crate::csd::spot(ctx, "Export G-code", ctx.screen_rect().center())).show(ctx, |ui| {
        ui.set_width(480.0);
        if crate::csd::dialog_header(ui, "Export G-code", true) { close = true; }
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Split into").strong());
        ui.add_space(4.0);
        for (mode, label, sub) in OPTIONS {
            ui.radio_value(&mut app.export_split, mode, label);
            ui.indent(label, |ui| { ui.add(egui::Label::new(egui::RichText::new(sub).color(ui.visuals().weak_text_color()).small())); });
            ui.add_space(3.0);
        }
        ui.add_space(10.0);
        let files = file_list(app, app.export_split, &name);
        ui.label(egui::RichText::new(if files.len() == 1 { "File".to_string() } else { format!("{} files", files.len()) }).strong());
        ui.add_space(4.0);
        let total: f64 = files.iter().map(|f| f.1).sum();
        egui::Grid::new("export_files").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
            for (n, t) in &files {
                ui.label(egui::RichText::new(n).monospace());
                ui.label(egui::RichText::new(fmt_time(*t)).monospace());
                ui.end_row();
            }
            if files.len() > 1 {
                ui.label(egui::RichText::new("Total").strong());
                ui.label(egui::RichText::new(fmt_time(total)).monospace().strong());
                ui.end_row();
            }
        });
        if app.export_split != Split::One {
            ui.add_space(6.0);
            ui.add(egui::Label::new(egui::RichText::new("Each file starts the spindle, goes to the safe height and ends safely on its own. Files after the first say at the top what to do before you start them. A tool change or pause between files is the end of a file, not an M0.").color(ui.visuals().weak_text_color()).small()));
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("Export…").fill(ui.visuals().selection.bg_fill)).clicked() { go = true; }
                if ui.button("Cancel").clicked() { close = true; }
            });
        });
        ui.add_space(4.0);
    });
    if go { app.export_open = false; app.choose_export_file(); }
    else if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { app.export_open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(file: &str) -> App {
        let mut a = App::new(None);
        a.open(&format!("{}/../tests/data/{file}", env!("CARGO_MANIFEST_DIR")));
        a.start_generation();
        a.finish_generation();
        assert!(a.output.is_some(), "{:?}", a.error);
        a
    }

    #[test]
    fn a_project_with_one_operation_exports_without_asking() {
        let mut a = app("outside_holes.project.json");
        let n = a.project.data.ops.len();
        if n == 1 { assert!(!a.can_split()); a.export_open = false; }
        // two operations on the same tool: still two files in the per-operation mode
        let a2 = app("two_ops_toolchange.project.json");
        assert!(a2.can_split());
    }

    #[test]
    fn the_list_names_the_files_and_adds_the_times_up() {
        let a = app("hardware_m0_aircut.project.json");
        let one = file_list(&a, Split::One, "job");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].0, "job.nc");
        let per = file_list(&a, Split::PerOperation, "job");
        assert_eq!(per.len(), 3, "{per:?}");
        assert!(per[0].0.starts_with("job-1-") && per[0].0.ends_with(".nc"), "{per:?}");
        let sum: f64 = per.iter().map(|f| f.1).sum();
        assert!((sum - one[0].1).abs() < 0.05 * one[0].1.max(1.0), "{sum} vs {}", one[0].1);
        assert!(per.iter().all(|f| f.1 > 0.0));
    }

    #[test]
    fn the_names_in_the_list_are_the_names_that_are_written() {
        let a = app("hardware_m0_aircut.project.json");
        for mode in [Split::PerTool, Split::PerOperation] {
            let listed = file_list(&a, mode, "job");
            let made = split::generate_files(&a.project.data, putiacnc_core::Units::Metric, mode).unwrap();
            let written: Vec<String> = made.iter().map(|f| App::split_path(std::path::Path::new("job.nc"), &f.suffix).display().to_string()).collect();
            assert_eq!(listed.iter().map(|f| f.0.clone()).collect::<Vec<_>>(), written);
        }
    }

    #[test]
    fn a_split_export_writes_every_file_complete() {
        let mut a = app("hardware_m0_aircut.project.json");
        a.export_split = Split::PerOperation;
        let dir = std::env::temp_dir().join(format!("putia-split-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        a.finish_dialog(crate::app::DlgKind::ExportGcode, dir.join("job.nc"));
        for _ in 0..200 { if !a.poll_split() { break; } std::thread::sleep(std::time::Duration::from_millis(20)); }
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names.len(), 3, "{names:?} {:?}", a.error);
        for n in &names {
            let t = std::fs::read_to_string(dir.join(n)).unwrap();
            assert!(t.contains("M3"), "{n} starts the spindle");
            assert!(!t.lines().any(|l| l.trim_start().starts_with("M0")), "{n} has no pause inside");
            assert!(t.trim_end().ends_with("M2") || t.trim_end().ends_with("M30"), "{n} ends the program");
        }
        // one file again: a single file with the chosen name
        let dir2 = dir.join("one");
        std::fs::create_dir_all(&dir2).unwrap();
        a.export_split = Split::One;
        a.finish_dialog(crate::app::DlgKind::ExportGcode, dir2.join("all.nc"));
        assert!(dir2.join("all.nc").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pen_project_lists_and_writes_its_files_without_a_spindle() {
        let mut a = app("hardware_pen_test.project.json");
        assert!(a.can_split());
        let per = file_list(&a, Split::PerOperation, "plot");
        assert_eq!(per.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), ["plot-1-draw.nc", "plot-2-draw.nc"]);
        assert_eq!(file_list(&a, Split::PerTool, "plot").len(), 1, "one pen, one file");
        a.export_split = Split::PerOperation;
        let dir = std::env::temp_dir().join(format!("putia-pen-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        a.finish_dialog(crate::app::DlgKind::ExportGcode, dir.join("plot.nc"));
        for _ in 0..200 { if !a.poll_split() { break; } std::thread::sleep(std::time::Duration::from_millis(20)); }
        for n in ["plot-1-draw.nc", "plot-2-draw.nc"] {
            let t = std::fs::read_to_string(dir.join(n)).unwrap_or_else(|e| panic!("{n}: {e} {:?}", a.error));
            assert!(!t.lines().any(|l| l.starts_with("M3") || l.starts_with("M5") || l.starts_with("G4")), "{n}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cutting_operations_do_not_offer_a_pen_and_a_draw_operation_takes_one() {
        let mut a = app("hardware_pen_test.project.json");
        assert!(a.project.data.tools.iter().any(|t| t.shape == "pen"));
        let mut cutter = putiacnc_core::shapes::blank_project().data.tools[0].clone();
        cutter.id = "tcutter".into();
        a.project.data.tools.push(cutter);
        a.project.data.loops.push(putiacnc_core::shapes::new_rect([5.0, 5.0], [9.0, 9.0], 0.0, 0));
        let n = a.project.data.loops.len();
        let sel: std::collections::BTreeSet<usize> = [n - 1].into_iter().collect();
        let _ = putiacnc_core::shapes::add_operation(&mut a.project.data, &sel, "pocket");
        let o = a.project.data.ops.last().unwrap();
        assert_eq!(o.p.kind, "pocket");
        assert!(a.project.data.tools.iter().any(|t| t.id == o.p.tool && t.shape != "pen"), "a cutter, not the pen");
    }

    #[test]
    fn the_choice_is_remembered_in_the_settings() {
        let mut a = App::new(None);
        a.export_split = Split::PerTool;
        let mut p = crate::prefs::Settings::default();
        p.export_split = Some(a.export_split.id().to_string());
        let j = serde_json::to_string(&p).unwrap();
        let back: crate::prefs::Settings = serde_json::from_str(&j).unwrap();
        assert_eq!(Split::from_id(back.export_split.as_deref().unwrap()), Split::PerTool);
        // an old settings file without the choice keeps "one file"
        let old: crate::prefs::Settings = serde_json::from_str("{}").unwrap();
        assert!(old.export_split.is_none());
    }
}
