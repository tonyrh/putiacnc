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
