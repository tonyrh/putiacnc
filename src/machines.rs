//! Machine library: named machines (rapid rate, time correction, work area). Mirrors the browser
//! app's Machines dialog. A project stores a copy of the machine it was made for.
use crate::model::Machine;
use crate::toollib::new_id;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machines {
    #[serde(default = "one")] pub v: u32,
    /// Machine preselected for new projects (the last one the user picked).
    #[serde(default)] pub def: String,
    #[serde(default)] pub list: Vec<Machine>,
}
fn one() -> u32 { 1 }

/// Positive numbers only, sensible defaults otherwise (same rules as the browser app).
pub fn normalize(m: &mut Machine) {
    let n = |x: f64, d: f64| if x.is_finite() && x > 0.0 { x } else { d };
    if m.id.is_empty() { m.id = new_id("m"); }
    m.name = { let t: String = m.name.trim().chars().take(60).collect(); if t.is_empty() { "Machine".into() } else { t } };
    m.rapid = n(m.rapid, 2000.0);
    m.tsc = n(m.tsc, 1.2);
    m.x = n(m.x, 300.0);
    m.y = n(m.y, 200.0);
    m.z = n(m.z, 60.0);
    if let Some(s) = m.safe { m.safe = Some(if s.is_finite() && s > 0.0 { s.min(200.0) } else { 5.0 }); }
    for l in [&mut m.rpm_min, &mut m.rpm_max, &mut m.feed_max, &mut m.plunge_max] { if !l.is_none_or(|v| v.is_finite() && v > 0.0) { *l = None; } }
    if let (Some(a), Some(b)) = (m.rpm_min, m.rpm_max) { if a > b { m.rpm_min = Some(b); m.rpm_max = Some(a); } }
    if let Some(p) = &m.post { if !crate::model::DIALECTS.iter().any(|d| d.0 == p) { m.post = Some(String::new()); } }
}

impl Default for Machines {
    fn default() -> Self {
        let mut m = Machine::default();
        m.id = new_id("m");
        Machines { v: 1, def: m.id.clone(), list: vec![m] }
    }
}

impl Machines {
    pub fn from_json(s: &str) -> Result<Machines, String> {
        let mut m: Machines = serde_json::from_str(s).map_err(|e| format!("Not a valid machine list: {e}"))?;
        m.repair();
        Ok(m)
    }
    pub fn to_json(&self) -> String { serde_json::to_string_pretty(self).unwrap_or_default() }

    /// Fix ids, ranges and the default so the list is always usable.
    pub fn repair(&mut self) {
        let mut seen: Vec<String> = vec![];
        for m in self.list.iter_mut() {
            normalize(m);
            if seen.contains(&m.id) { m.id = new_id("m"); }
            seen.push(m.id.clone());
        }
        if self.list.is_empty() { *self = Machines::default(); }
        if !self.list.iter().any(|m| m.id == self.def) { self.def = self.list[0].id.clone(); }
        self.v = 1;
    }

    pub fn get(&self, id: &str) -> Option<&Machine> { self.list.iter().find(|m| m.id == id) }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Machine> { self.list.iter_mut().find(|m| m.id == id) }
    /// The machine with this id, or the default one.
    pub fn resolve(&self, id: &str) -> &Machine { self.get(id).or_else(|| self.get(&self.def)).unwrap_or(&self.list[0]) }

    /// "New machine" or, with `copy`, a copy of `base`. Returns the new id.
    pub fn add(&mut self, base: Option<&str>, copy: bool) -> String {
        let b = base.and_then(|id| self.get(id)).cloned().unwrap_or_default();
        let mut m = if copy { Machine { name: format!("{} copy", b.name), ..b } } else { Machine { name: "New machine".into(), ..Machine::default() } };
        m.id = new_id("m");
        let id = m.id.clone();
        self.list.push(m);
        id
    }

    /// Delete a machine; at least one always stays. Returns the id to select next.
    pub fn delete(&mut self, id: &str) -> Result<String, String> {
        if self.list.len() < 2 { return Err("Keep at least one machine.".into()); }
        self.list.retain(|m| m.id != id);
        if self.def == id { self.def = self.list[0].id.clone(); }
        Ok(self.def.clone())
    }

    /// One machine as a shareable file.
    pub fn export_machine(&self, id: &str) -> String {
        let m = self.resolve(id);
        serde_json::to_string_pretty(&serde_json::json!({ "app": "putia-cnc-machine", "v": 1, "machines": [m] })).unwrap_or_default()
    }

    /// Add the machines of a `putia-cnc-machine` file (with new ids). Returns the id of the last one and how many were added.
    pub fn import(&mut self, json: &str) -> Result<(String, usize), String> {
        #[derive(serde::Deserialize)]
        struct File { app: String, machines: Vec<Machine> }
        let bad = || "This is not a PutiaCNC machine file.".to_string();
        let f: File = serde_json::from_str(json).map_err(|_| bad())?;
        if f.app != "putia-cnc-machine" || f.machines.is_empty() { return Err(bad()); }
        let mut last = String::new();
        let mut n = 0;
        for mut m in f.machines.into_iter().take(20) {
            m.id = new_id("m");
            normalize(&mut m);
            last = m.id.clone();
            self.list.push(m);
            n += 1;
        }
        Ok((last, n))
    }

    /// Add a machine that came with a project if the list doesn't know it yet. Returns true when added.
    pub fn adopt(&mut self, m: &Machine) -> bool {
        if m.id.is_empty() || self.get(&m.id).is_some() { return false; }
        let mut m = m.clone();
        normalize(&mut m);
        self.list.push(m);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_machine_lists_load_and_new_settings_survive() {
        // a list saved before the dialect, line numbers and safe Z moved onto the machine
        let old = r#"{"v":1,"def":"a","list":[{"id":"a","name":"Old","rapid":3000,"tsc":1.1,"x":400,"y":300,"z":80}]}"#;
        let m = Machines::from_json(old).unwrap();
        assert_eq!((m.list[0].post.clone(), m.list[0].numbered, m.list[0].safe), (None, None, None));
        // the JSON of a machine without them does not grow new keys
        assert!(!m.to_json().contains("safe"));
        let mut n = Machines::default();
        n.list[0].post = Some("mach".into()); n.list[0].numbered = Some(true); n.list[0].safe = Some(7.5);
        let back = Machines::from_json(&n.to_json()).unwrap();
        assert_eq!((back.list[0].post.as_deref(), back.list[0].numbered, back.list[0].safe), (Some("mach"), Some(true), Some(7.5)));
        // nonsense is repaired
        let mut bad = Machines::default();
        bad.list[0].post = Some("weird".into()); bad.list[0].safe = Some(-3.0);
        bad.repair();
        assert_eq!((bad.list[0].post.as_deref(), bad.list[0].safe), (Some(""), Some(5.0)));
    }

    #[test]
    fn machine_files_round_trip() {
        let mut a = Machines::default();
        a.list[0].name = "Shop router".into(); a.list[0].rpm_max = Some(18000.0); a.list[0].inches = Some(true);
        let id = a.list[0].id.clone();
        let file = a.export_machine(&id);
        let mut b = Machines::default();
        let (new_id, n) = b.import(&file).unwrap();
        assert_eq!(n, 1);
        assert_eq!(b.list.len(), 2);
        let m = b.get(&new_id).unwrap();
        assert!(m.id != id && m.name == "Shop router" && m.rpm_max == Some(18000.0) && m.inches == Some(true));
        assert!(b.import("{\"app\":\"putia-cnc-tools\",\"tools\":[]}").is_err());
        assert!(b.import("nonsense").is_err());
    }

    #[test]
    fn default_has_one_machine() {
        let m = Machines::default();
        assert_eq!(m.list.len(), 1);
        assert_eq!(m.def, m.list[0].id);
    }

    #[test]
    fn add_duplicate_delete() {
        let mut m = Machines::default();
        let first = m.def.clone();
        let c = m.add(Some(&first), true);
        assert_eq!(m.get(&c).unwrap().name, "My CNC machine copy");
        let n = m.add(None, false);
        assert_eq!(m.get(&n).unwrap().name, "New machine");
        assert_eq!(m.list.len(), 3);
        m.delete(&c).unwrap();
        m.delete(&n).unwrap();
        assert!(m.delete(&first).is_err());
        assert_eq!(m.list.len(), 1);
    }

    #[test]
    fn repair_and_round_trip() {
        let mut m = Machines::default();
        m.list[0].x = -5.0;
        m.list[0].name = "   ".into();
        m.def = "nope".into();
        let json = m.to_json();
        let back = Machines::from_json(&json).unwrap();
        assert_eq!(back.list[0].x, 300.0);
        assert_eq!(back.list[0].name, "Machine");
        assert_eq!(back.def, back.list[0].id);
    }

    #[test]
    fn adopt_unknown_machine_once() {
        let mut m = Machines::default();
        let mut p = Machine::default();
        p.id = "other".into();
        p.name = "Shop router".into();
        assert!(m.adopt(&p));
        assert!(!m.adopt(&p));
        assert_eq!(m.resolve("other").name, "Shop router");
        assert_eq!(m.resolve("missing").id, m.def);
    }
}
