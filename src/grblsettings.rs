//! Read a Grbl `$$` settings dump into a machine profile: travel, top speeds, accelerations, junction deviation and spindle range.
use crate::model::Machine;

/// The `$N=value` lines of a dump as (number, value). Lines that are not settings (`ok`, `[MSG:...]`, comments, banners) are skipped;
/// a trailing `(description)` or `;comment` on a line is ignored.
pub fn settings(text: &str) -> Vec<(u32, f64)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        let Some(rest) = l.strip_prefix('$') else { continue };
        let Some((n, v)) = rest.split_once('=') else { continue };
        let Ok(n) = n.trim().parse::<u32>() else { continue };
        let v = v.trim().split(|c: char| c == '(' || c == ';' || c.is_whitespace()).next().unwrap_or("");
        if let Ok(v) = v.parse::<f64>() { if v.is_finite() { out.push((n, v)); } }
    }
    out
}

/// A machine from a `$$` dump, or the reason there is none. Settings the dump does not hold keep the new machine's defaults.
/// Travel (`$130` to `$132`) is the work area, `$110` to `$112` the top speeds, `$120` to `$122` the accelerations, `$11` the junction deviation,
/// `$30` and `$31` the spindle range. The cutting and plunge limits are not in a dump, so the fastest axis (X and Y) stands in for the feed limit.
pub fn machine_from_dump(text: &str) -> Result<Machine, String> {
    let s = settings(text);
    if s.is_empty() { return Err("No Grbl settings found. Paste the lines that `$$` prints, such as $110=3000.000.".into()); }
    let get = |n: u32| s.iter().rev().find(|(k, _)| *k == n).map(|(_, v)| *v).filter(|v| *v > 0.0);
    let mut m = Machine::default();
    m.id = String::new();
    m.name = "Grbl machine".into();
    m.post = Some(String::new());
    m.safe = Some(5.0);
    if let Some(v) = get(130) { m.x = v; }
    if let Some(v) = get(131) { m.y = v; }
    if let Some(v) = get(132) { m.z = v; }
    m.rate = get(110); m.rate_y = get(111); m.rate_z = get(112);
    m.accel = get(120); m.accel_y = get(121); m.accel_z = get(122);
    if m.accel.is_some() { m.tsc = 1.0; }
    // the rapid rate is how fast the slowest of X and Y can go; cutting cannot be faster than that either
    if let Some(r) = [m.rate, m.rate_y].into_iter().flatten().reduce(f64::min) { m.rapid = r; }
    if let Some(r) = [m.rate, m.rate_y].into_iter().flatten().reduce(f64::min) { m.feed_max = Some(r); }
    if let Some(r) = m.rate_z.or(m.rate) { m.plunge_max = Some(r); }
    if let Some(j) = s.iter().rev().find(|(k, _)| *k == 11).map(|(_, v)| *v).filter(|v| *v > 0.0) { m.junction = Some(j); }
    m.rpm_max = get(30);
    m.rpm_min = s.iter().rev().find(|(k, _)| *k == 31).map(|(_, v)| *v).filter(|v| *v > 0.0);
    let found: Vec<String> = [(130, "travel"), (110, "speeds"), (120, "accelerations"), (30, "spindle")].iter().filter(|(n, _)| get(*n).is_some()).map(|(_, w)| w.to_string()).collect();
    m.notes = format!("Imported from a Grbl $$ dump ({}). Check the values against your machine.", if found.is_empty() { "no known settings".to_string() } else { found.join(", ") });
    crate::machines::normalize(&mut m);
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DUMP: &str = "$0=10\n$11=0.020\n$30=24000\n$31=8000\n$32=0\n$110=3000.000\n$111=2500.000\n$112=800.000\n$120=100.000\n$121=80.000\n$122=40.000\n$130=1200.000\n$131=800.000\n$132=100.000\nok\n";
    #[test]
    fn a_dump_fills_the_machine() {
        let m = machine_from_dump(DUMP).unwrap();
        assert_eq!((m.x, m.y, m.z), (1200.0, 800.0, 100.0));
        assert_eq!((m.rate, m.rate_y, m.rate_z), (Some(3000.0), Some(2500.0), Some(800.0)));
        assert_eq!((m.accel, m.accel_y, m.accel_z), (Some(100.0), Some(80.0), Some(40.0)));
        assert_eq!(m.junction, Some(0.02));
        assert_eq!((m.rpm_min, m.rpm_max), (Some(8000.0), Some(24000.0)));
        assert_eq!(m.rapid, 2500.0);
        assert_eq!(m.plunge_max, Some(800.0));
    }
    #[test]
    fn noise_and_comments_are_skipped() {
        let s = settings("Grbl 1.1h ['$' for help]\n$110=500 (x max rate, mm/min)\n[MSG:Ok]\n$130 = 300.5 ; travel\n$x=1\nok");
        assert_eq!(s, vec![(110, 500.0), (130, 300.5)]);
    }
    #[test]
    fn text_without_settings_is_refused() {
        assert!(machine_from_dump("hello\nok").is_err());
    }
    #[test]
    fn a_partial_dump_keeps_defaults() {
        let m = machine_from_dump("$130=500").unwrap();
        assert_eq!(m.x, 500.0);
        assert!(m.rate.is_none() && m.accel.is_none());
    }

    #[test]
    fn a_real_vasto_dump_with_descriptions_reads() {
        let d = "$11=0.010 (Junction deviation, mm)\n$30=10000 (Maximum spindle speed, rpm)\n$31=0 (Minimum spindle speed, rpm)\n$110=1500.000 (X-axis maximum rate, mm/min)\n$111=1500.000 (Y-axis maximum rate, mm/min)\n$112=600.000 (Z-axis maximum rate, mm/min)\n$120=300.000 (X-axis acceleration, mm/sec^2)\n$121=300.000 (Y-axis acceleration, mm/sec^2)\n$122=100.000 (Z-axis acceleration, mm/sec^2)\n$130=410.000 (X-axis maximum travel, mm)\n$131=420.000 (Y-axis maximum travel, mm)\n$132=90.000 (Z-axis maximum travel, mm)\n";
        let m = machine_from_dump(d).unwrap();
        assert_eq!((m.x, m.y, m.z), (410.0, 420.0, 90.0));
        assert_eq!((m.rapid, m.rpm_max, m.rpm_min), (1500.0, Some(10000.0), None));
        assert_eq!((m.accel, m.accel_z, m.rate_z), (Some(300.0), Some(100.0), Some(600.0)));
    }
}
