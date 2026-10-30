//! Hostile values. The random-project tests build sensible projects; these take the sample projects and replace numbers anywhere in
//! them with zero, negatives, tiny, huge and wrong-typed values, as a hand-edited or damaged project file could. Whatever loads must
//! never panic or hang, and must not write NaN or inf into the G-code. Also: the same project gives the same program twice, and
//! saving and loading a project does not change its program.
//!   cargo test --release --test hostile                 (150 cases)
//!   COUNT=3000 SEED=7 cargo test --release --test hostile -- --ignored --nocapture
use putiacnc_core::{checks, generate, load_project, replay::replay, save_project, summary, Units};
use serde_json::Value;
use std::sync::mpsc;
use std::time::Duration;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn below(&mut self, n: usize) -> usize { (self.next() % n as u64) as usize }
}

fn numbers(v: &Value, path: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    match v {
        Value::Number(_) => out.push(path.clone()),
        Value::Array(a) => for (i, x) in a.iter().enumerate() { path.push(i.to_string()); numbers(x, path, out); path.pop(); },
        Value::Object(o) => for (k, x) in o { path.push(k.clone()); numbers(x, path, out); path.pop(); },
        _ => {}
    }
}

fn set(v: &mut Value, path: &[String], new: Value) {
    let mut cur = v;
    for p in &path[..path.len() - 1] {
        cur = match cur { Value::Array(a) => &mut a[p.parse::<usize>().unwrap()], Value::Object(o) => o.get_mut(p).unwrap(), _ => return };
    }
    match cur { Value::Array(a) => a[path[path.len() - 1].parse::<usize>().unwrap()] = new, Value::Object(o) => { o.insert(path[path.len() - 1].clone(), new); } _ => {} }
}

fn hostile(r: &mut Rng) -> Value {
    match r.below(12) {
        0 => 0.into(), 1 => (-1).into(), 2 => serde_json::json!(1e-9), 3 => serde_json::json!(1e9), 4 => serde_json::json!(-1e9),
        5 => serde_json::json!(0.5), 6 => 1000000.into(), 7 => Value::Null, 8 => Value::String("x".into()), 9 => serde_json::json!(1e300),
        10 => serde_json::json!(0.001), _ => 2.into(),
    }
}

/// Everything the program does with a project, returning a description of what went wrong (or nothing).
fn exercise(json: &str) -> Vec<String> {
    let mut bad = vec![];
    let Ok(f) = load_project(json) else { return bad };
    for units in [Units::Metric, Units::Imperial] {
        let Ok(o) = generate(&f.data, units) else { continue };
        for (i, l) in o.lines.iter().enumerate() {
            let low = l.to_ascii_lowercase();
            if !l.trim_start().starts_with('(') && (low.contains("nan") || low.contains("inf")) { bad.push(format!("{units:?}: line {i} has a non-number: {l}")); break; }
        }
        if !o.time_min.is_finite() || !o.cut_len_mm.is_finite() { bad.push(format!("{units:?}: time {} or length {} is not finite", o.time_min, o.cut_len_mm)); }
        let _ = checks::run(&f.data, &o);
        let _ = summary::describe(&f.data, &o, units, false);
        let r = replay(&o.lines, &f.data);
        if r.moves.iter().any(|m| !m.dt.is_finite() || m.to.iter().any(|c| !c.is_finite())) { bad.push(format!("{units:?}: replay has a non-finite move")); }
        if units == Units::Metric {
            // the same project twice, and through save and load
            if let Ok(o2) = generate(&f.data, units) { if o2.lines != o.lines { bad.push("two runs give different programs".into()); } }
            if let Ok(s) = save_project(&f) { if let Ok(g) = load_project(&s) { if let Ok(o3) = generate(&g.data, units) { if o3.lines != o.lines { bad.push("save and load changes the program".into()); } } } }
        }
    }
    bad
}

fn run(seeds: std::ops::Range<u64>, per: usize) {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(".project.json")).collect();
    files.sort();
    let mut report = vec![];
    for seed in seeds {
        let mut r = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
        let path = &files[r.below(files.len())];
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut spots = vec![];
        numbers(&v, &mut vec![], &mut spots);
        let mut what = vec![];
        for _ in 0..1 + r.below(per) {
            let at = spots[r.below(spots.len())].clone();
            let h = hostile(&mut r);
            what.push(format!("{} = {}", at.join("."), h));
            set(&mut v, &at, h);
        }
        let json = v.to_string();
        if std::env::var("TRACE").is_ok() { eprintln!("seed {seed}: {what:?}"); }
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new().stack_size(64 << 20).spawn(move || { let _ = tx.send(std::panic::catch_unwind(|| exercise(&json))); }).unwrap();
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(bad)) if bad.is_empty() => {}
            Ok(Ok(bad)) => report.push(format!("seed {seed} ({}): {bad:?}\n    changed: {what:?}", path.file_name().unwrap().to_string_lossy())),
            Ok(Err(_)) => report.push(format!("seed {seed} ({}): PANIC\n    changed: {what:?}", path.file_name().unwrap().to_string_lossy())),
            Err(_) => report.push(format!("seed {seed} ({}): HANG (30 s)\n    changed: {what:?}", path.file_name().unwrap().to_string_lossy())),
        }
    }
    assert!(report.is_empty(), "{} problem(s):\n{}", report.len(), report.join("\n"));
}

#[test]
fn hostile_values_in_sample_projects() { run(0..150, 3); }

#[test]
#[ignore]
fn thousands_of_hostile_projects() {
    let n: u64 = std::env::var("COUNT").ok().and_then(|v| v.parse().ok()).unwrap_or(2000);
    let s: u64 = std::env::var("SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1000);
    run(s..s + n, 4);
}

// the things the hostile runs found, each kept as a plain test

fn sample(name: &str) -> putiacnc_core::model::ProjectData {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/data/{name}.project.json"));
    load_project(&std::fs::read_to_string(p).unwrap()).unwrap().data
}

#[test]
fn absurd_numbers_are_refused_with_a_message_not_a_crash() {
    type Edit = fn(&mut putiacnc_core::model::ProjectData);
    let cases: [(&str, Edit); 4] = [
        ("a tool 1e300 mm wide", |d| d.tools[0].dia = 1e300),
        ("a stock 1e300 mm wide", |d| d.stock.w = 1e300),
        ("a safe Z of 1e300", |d| d.job.safe = 1e300),
        ("a pass depth of 1e-9 mm", |d| d.tools[0].pd = 1e-9),
    ];
    for (what, edit) in cases {
        let mut d = sample("outside_holes");
        edit(&mut d);
        assert!(generate(&d, Units::Metric).is_err(), "{what}: expected a message");
    }
}

#[test]
fn a_machine_with_zero_speed_or_huge_factors_still_gives_a_finite_time() {
    for (rapid, tsc) in [(0.0, 1.2), (2000.0, 0.0), (2000.0, 1e300), (1e300, 1.2)] {
        let mut d = sample("outside_holes");
        let m = d.machine.get_or_insert_with(Default::default);
        m.rapid = rapid; m.tsc = tsc;
        let o = generate(&d, Units::Metric).unwrap();
        assert!(o.time_min.is_finite() && o.time_min > 0.0, "rapid {rapid}, tsc {tsc}: time {}", o.time_min);
    }
}

#[test]
fn absurd_holder_sizes_and_stick_outs_give_a_finite_check_and_no_panic() {
    use putiacnc_core::checks::{clamp_contact, holder_of, Holder};
    for (holder, stick) in [(0.0, 10.0), (-5.0, 10.0), (30.0, 0.0), (30.0, -1.0), (1e300, 1e-300), (1e300, 1e300), (1e-300, 1e-300), (30.0, 1e300)] {
        let mut d = sample("outside_holes");
        d.clamps.push(putiacnc_core::model::Clamp { x: 124.0, y: 45.0, w: 14.0, h: 20.0, height: 25.0 });
        d.machine.get_or_insert_with(Default::default).holder = Some(holder);
        d.tools[0].stick = stick;
        let o = generate(&d, Units::Metric).unwrap();
        for f in checks::run(&d, &o) { assert!(f.at.iter().all(|v| v.is_finite()), "holder {holder}, stick {stick}: {:?}", f.at); }
        let h = holder_of(&d, 0, false);
        assert!(h.is_none_or(|h| h.r > 0.0 && h.stick > 0.0), "{h:?}");
    }
    // a holder or a position that is not a number never touches anything and never panics
    let c = putiacnc_core::model::Clamp { x: 0.0, y: 0.0, w: 10.0, h: 10.0, height: 20.0 };
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300] {
        let _ = clamp_contact(&c, [v, v, v], v, Some(Holder { r: v, stick: v }));
    }
}

#[test]
fn rounding_a_huge_number_does_not_panic() {
    for v in [1e22, 1e30, 1e300, -1e300, f64::MAX] { let _ = putiacnc_core::toolpath::to_fixed(v, 3); }
}

#[test]
fn the_clipping_library_is_never_given_points_it_would_throw_on() {
    let far: putiacnc_core::clip::Path = vec![[0, 0], [i64::MAX / 2, 0], [i64::MAX / 2, i64::MAX / 2]];
    assert!(putiacnc_core::clip::offset(std::slice::from_ref(&far), 5e302, 10.0).is_empty());
    assert!(putiacnc_core::clip::offset(&[vec![[0, 0], [1000, 0], [1000, 1000]]], f64::NAN, 10.0).is_empty());
    assert!(putiacnc_core::clip::simplify_even_odd(std::slice::from_ref(&far)).is_empty());
    assert!(putiacnc_core::clip::boolean(putiacnc_core::clip::BoolKind::Union, &[far], &[]).is_empty());
}

#[test]
fn a_drawing_of_many_overlapping_copies_does_not_make_the_offset_take_minutes() {
    // found by the random action sequences: 20 shapes with overlapping inlays made one Clipper offset take 83 s (19,000 points,
    // 462 polygons); with its input cleaned it takes about a second
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/slow_inlay.json");
    let d = load_project(&std::fs::read_to_string(p).unwrap()).unwrap().data;
    let t = std::time::Instant::now();
    let o = generate(&d, Units::Metric).unwrap();
    assert!(!o.lines.is_empty());
    assert!(t.elapsed().as_secs() < 30, "took {:?}", t.elapsed());
}
