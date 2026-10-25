//! The tool field of an operation: a button that opens a panel with a search box on top and the tools below it, grouped by
//! unit and type. Typing narrows the list (all words must match the name, type or size); Enter takes the first match.
use eframe::egui::{self, Ui};
use putiacnc_core::model::Tool;
use putiacnc_core::toollib;

/// What the user chose in the panel.
pub enum Picked { Tool(String), Manage }

/// Plain quote instead of the inch sign, which the default font cannot draw.
fn clean(s: &str) -> String { s.replace('″', "\"") }

/// A tool's line in the list. Many names already carry the size, as in "90° V-bit (6 mm)", and then it is not repeated.
fn tool_line(t: &Tool) -> String {
    let (name, dia) = (clean(&t.name), toollib::fmt_dia(t.dia, &t.sys, true));
    if name.contains(&dia) { name } else { format!("{name}  ({dia})") }
}

/// True when every word of `words` (lower case) is found in the tool's name, type, size or notes.
pub fn matches(t: &Tool, words: &[String]) -> bool {
    if words.is_empty() { return true; }
    let hay = format!("{} {} {} {} {} {}", t.name, toollib::shape_label(&t.shape), toollib::category(&t.shape).1, toollib::fmt_dia(t.dia, &t.sys, false), t.dia, t.notes)
        .to_lowercase().replace('\u{2033}', "\"").replace('″', "\"");
    words.iter().all(|w| hay.contains(w.as_str()))
}

fn words(q: &str) -> Vec<String> { q.to_lowercase().replace('\u{2033}', "\"").replace('″', "\"").split_whitespace().map(String::from).collect() }

/// The selector. `current` is the id of the tool in use; `used` are the tools the job's operations already use (offered first while
/// nothing is typed); `only` limits which tools are offered (for example flat end mills).
pub fn picker(ui: &mut Ui, id: impl std::hash::Hash, width: f32, shown: String, current: Option<&str>, used: &[String], tools: &[Tool], only: impl Fn(&Tool) -> bool) -> (egui::Response, Option<Picked>) {
    let cid = egui::Id::new(id);
    let (qid, oid) = (cid.with("q"), cid.with("opened"));
    let mut picked: Option<Picked> = None;
    let out = egui::ComboBox::from_id_source(cid).width(width).selected_text(shown).show_ui(ui, |ui| {
        ui.set_min_width(width.max(280.0));
        let first = !ui.data(|d| d.get_temp::<bool>(oid)).unwrap_or(false);
        let mut q: String = ui.data(|d| d.get_temp(qid)).unwrap_or_default();
        let te = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Search tools (name, type, size)").desired_width(f32::INFINITY));
        if first { te.request_focus(); ui.data_mut(|d| d.insert_temp(oid, true)); }
        ui.add_space(4.0);
        let w = words(&q);
        let list: Vec<&Tool> = tools.iter().filter(|t| only(t) && matches(t, &w)).collect();
        if te.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(t) = list.first() { picked = Some(Picked::Tool(t.id.clone())); }
        }
        egui::ScrollArea::vertical().max_height(340.0).auto_shrink([false, true]).show(ui, |ui| {
            if list.is_empty() { ui.weak("No tool matches."); }
            let mut job_shown = false;
            // the tools this job already uses come first: most jobs need only two or three
            if w.is_empty() {
                let job: Vec<&&Tool> = used.iter().filter_map(|id| list.iter().find(|t| &t.id == id)).collect();
                if !job.is_empty() {
                    job_shown = true;
                    ui.label(egui::RichText::new("In this job").strong());
                    for t in job {
                        let sel = current == Some(t.id.as_str());
                        if ui.selectable_label(sel, tool_line(t)).clicked() { picked = Some(Picked::Tool(t.id.clone())); }
                    }
                    ui.add_space(4.0);
                    ui.separator();
                }
            }
            for (sys, label) in [("mm", "Metric"), ("in", "Imperial")] {
                let g: Vec<&&Tool> = list.iter().filter(|t| t.sys == sys).collect();
                if g.is_empty() { continue; }
                ui.label(egui::RichText::new(format!("{label}  ({})", g.len())).strong());
                let mut kinds: Vec<(usize, &str)> = g.iter().map(|t| toollib::category(&t.shape)).collect();
                kinds.sort();
                kinds.dedup();
                for (ko, kname) in kinds {
                    ui.label(egui::RichText::new(kname).small().weak());
                    for t in g.iter().filter(|t| toollib::category(&t.shape).0 == ko) {
                        let sel = current == Some(t.id.as_str());
                        let r = ui.selectable_label(sel, tool_line(t));
                        if sel && first && q.is_empty() && !job_shown { r.scroll_to_me(Some(egui::Align::Center)); }
                        if r.clicked() { picked = Some(Picked::Tool(t.id.clone())); }
                    }
                }
            }
        });
        ui.separator();
        if ui.selectable_label(false, "Manage tools…").on_hover_text("Open the tool library").clicked() { picked = Some(Picked::Manage); }
        if picked.is_some() { ui.close_menu(); }
        ui.data_mut(|d| d.insert_temp(qid, q));
    });
    if out.inner.is_none() { ui.data_mut(|d| { d.remove::<String>(qid); d.remove::<bool>(oid); }); }
    (out.response, picked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, shape: &str, dia: f64, sys: &str) -> Tool {
        let mut t = putiacnc_core::toollib::seed_tools().into_iter().next().unwrap();
        t.name = name.into(); t.shape = shape.into(); t.dia = dia; t.sys = sys.into(); t.notes = String::new();
        t
    }

    #[test]
    fn every_word_must_match_name_type_or_size() {
        let a = tool("1/8\" flat end mill", "flat", 3.175, "in");
        let b = tool("60 degree V-bit", "vbit", 12.7, "mm");
        assert!(matches(&a, &words("")));
        assert!(matches(&a, &words("flat")));
        assert!(matches(&a, &words("1/8 end")));
        assert!(!matches(&a, &words("v-bit")));
        assert!(matches(&b, &words("v-grooving")), "the type name counts too");
        assert!(matches(&b, &words("12.7")), "so does the size");
        assert!(!matches(&b, &words("flat 12.7")));
    }
}
