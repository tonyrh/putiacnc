//! Random projects through the generator and the independent checker.
//!
//! Every project comes from a seed (a tiny xorshift generator, no dependency), so a failure can be repeated:
//! `SEED=1234 cargo test --test random_projects one_seed -- --nocapture` runs only that project and writes it to
//! the temp folder as `putia-random-1234.project.json`, ready to open in the app.
//! The normal run covers 20 seeds (a few seconds); `cargo test --release --test random_projects -- --ignored --nocapture`
//! runs 5000 (or `COUNT=...`).
use putiacnc_core::model::{CustomFeeds, LoopData, Machine, Operation, ProjectFile, Tool};
use putiacnc_core::verify::trace;
use putiacnc_core::{generate, shapes, Output, Units};
use std::collections::BTreeSet;
use std::panic::{catch_unwind, AssertUnwindSafe};

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1); for _ in 0..4 { r.next(); } r }
    fn next(&mut self) -> u64 { self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27; self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) }
    fn unit(&mut self) -> f64 { (self.next() >> 11) as f64 / (1u64 << 53) as f64 }
    fn f(&mut self, a: f64, b: f64) -> f64 { a + (b - a) * self.unit() }
    /// Whole number from a to b, both included.
    fn i(&mut self, a: usize, b: usize) -> usize { a + (self.next() % (b - a + 1) as u64) as usize }
    fn chance(&mut self, p: f64) -> bool { self.unit() < p }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T { &v[self.i(0, v.len() - 1)] }
}

fn random_tool(r: &mut Rng, n: usize, shape: &str) -> Tool {
    let mut t = shapes::blank_project().data.tools[0].clone();
    t.id = format!("rnd{n}"); t.name = format!("random {shape} {n}");
    t.shape = shape.into();
    t.dia = (r.f(0.5, 12.0) * 10.0).round() / 10.0;
    t.rpm = r.f(5000.0, 24000.0).round();
    t.feed = r.f(100.0, 2000.0).round();
    t.plunge = r.f(50.0, 500.0).round();
    t.so = r.f(10.0, 60.0).round();
    t.pd = r.f(0.2, 6.0);
    t.angle = 60.0; t.tip = 0.0; t.arc = 0.0;
    match shape {
        "vbit" => t.angle = r.f(20.0, 120.0).round(),
        "engrave" => { t.angle = r.f(10.0, 40.0).round(); t.tip = r.f(0.0, 0.2); }
        "taper" => { t.angle = r.f(4.0, 20.0).round(); t.tip = r.f(0.1, (t.dia / 2.0).min(1.0)); }
        "arc" => t.arc = r.f(0.2, 0.45) * t.dia,
        _ => {}
    }
    t
}

/// `max`: the largest size; areas that are cleared (pockets, V-carving) are kept small so a run stays quick, a 1 mm bit clearing a hand's width takes seconds.
fn random_shape(r: &mut Rng, w: f64, h: f64, op: i64, open_ok: bool, max: f64) -> LoopData {
    let size = |r: &mut Rng| if r.chance(0.15) { r.f(0.5, 6.0) } else { r.f(5.0, (0.9 * w.min(h)).min(max).max(5.0)) };
    // mostly inside the stock, sometimes touching or beyond its edge
    let place = |r: &mut Rng, sw: f64, sh: f64| -> [f64; 2] {
        if r.chance(0.1) { [r.f(-sw / 2.0, w - sw / 2.0), r.f(-sh / 2.0, h - sh / 2.0)] } else { [r.f(0.0, (w - sw).max(0.0)), r.f(0.0, (h - sh).max(0.0))] }
    };
    loop {
        let (sw, sh) = (size(r), size(r));
        let o = place(r, sw, sh);
        let l = match r.i(0, 5) {
            0 => Some(shapes::new_rect(o, [o[0] + sw, o[1] + sh], r.f(0.0, 0.3) * sw.min(sh), op)),
            1 => Some(shapes::new_circle([o[0] + sw / 2.0, o[1] + sw / 2.0], sw / 2.0, op)),
            2 => Some(shapes::new_ellipse(o, [o[0] + sw, o[1] + sh], op)),
            3 => shapes::new_polygon([o[0] + sw / 2.0, o[1] + sw / 2.0], sw / 2.0, r.f(0.0, 90.0), r.i(3, 8), if r.chance(0.3) { Some(r.f(0.3, 0.8)) } else { None }, op),
            4 => {
                // a random outline: points round a centre at random distances, so it may be concave
                let n = r.i(5, 12);
                let (cx, cy) = (o[0] + sw / 2.0, o[1] + sh / 2.0);
                let pts: Vec<[f64; 2]> = (0..n).map(|k| { let a = std::f64::consts::TAU * k as f64 / n as f64; let d = r.f(0.35, 1.0); [cx + d * sw / 2.0 * a.cos(), cy + d * sh / 2.0 * a.sin()] }).collect();
                shapes::new_poly(&pts, true, op)
            }
            _ => if open_ok { shapes::new_poly(&[o, [o[0] + sw, o[1] + sh * r.f(-1.0, 1.0)], [o[0] + sw * r.f(0.0, 1.0), o[1] + sh]], false, op) } else { None },
        };
        if let Some(l) = l { return l; }
    }
}

/// Builds a project from a seed: stock, tools, 1 to 3 operations with their shapes, and a machine.
#[allow(clippy::field_reassign_with_default)]
fn random_project(seed: u64) -> ProjectFile {
    let mut r = Rng::new(seed);
    let mut f = shapes::blank_project();
    f.name = format!("random {seed}");
    let d = &mut f.data;
    d.stock.w = r.f(20.0, 400.0).round(); d.stock.h = r.f(20.0, 300.0).round(); d.stock.t = r.f(2.0, 40.0).round();
    d.stock.zr = r.pick(&["ll", "lr", "ul", "ur", "c"]).to_string();
    if r.chance(0.25) { d.stock.zm = "h".into(); d.stock.zh = r.f(0.0, d.stock.t); }
    d.job.safe = r.f(1.0, 20.0).round();
    for k in 0..r.i(1, 3) { let s = *r.pick(&["flat", "flat", "ball", "bull"]); d.tools.push(random_tool(&mut r, k, s)); }
    for k in 0..r.i(1, 2) { let s = *r.pick(&["vbit", "engrave", "taper", "arc"]); d.tools.push(random_tool(&mut r, 10 + k, s)); }
    let (w, h, t) = (d.stock.w, d.stock.h, d.stock.t);

    let mut m = Machine::default();
    m.post = Some(if r.chance(0.3) { "mach" } else { "" }.into());
    d.job.post = m.post.clone().unwrap();
    m.numbered = Some(r.chance(0.2)); d.job.numbered = m.numbered.unwrap();
    m.arcs = Some(r.chance(0.3));
    m.safe = Some(d.job.safe);
    m.spin_up = Some(*r.pick(&[0.0, 2.0, 2.0, 4.5]));
    if r.chance(0.2) { m.rpm_max = Some(r.f(8000.0, 18000.0).round()); }
    if r.chance(0.2) { m.feed_max = Some(r.f(200.0, 1200.0).round()); }
    d.machine = Some(m);
    d.job.air = r.chance(0.05);

    for _ in 0..r.i(1, 3) {
        let kind = *r.pick(&["outside", "inside", "online", "pocket", "pocket", "vcarve", "drill", "inlay"]);
        let cone: Vec<String> = d.tools.iter().filter(|t| ["vbit", "engrave", "taper"].contains(&t.shape.as_str())).map(|t| t.id.clone()).collect();
        let flat: Vec<String> = d.tools.iter().filter(|t| ["flat", "ball", "bull"].contains(&t.shape.as_str())).map(|t| t.id.clone()).collect();
        let first = d.loops.len();
        let id = d.oid.max(d.ops.iter().map(|o| o.id).max().unwrap_or(0)) + 1;
        let n = r.i(1, 3);
        for _ in 0..n {
            let l = if kind == "drill" { shapes::new_point([r.f(2.0, w - 2.0), r.f(2.0, h - 2.0)], id) } else { random_shape(&mut r, w, h, id, kind == "online", if ["outside", "inside", "online", "drill"].contains(&kind) { 1e9 } else { 60.0 }) };
            d.loops.push(l);
        }
        // sometimes a shape inside another one of the same operation: a hole or an island
        if kind != "drill" && kind != "online" && kind != "inlay" && r.chance(0.3) {
            if let Some(b) = shapes::bbox(&d.loops[first]) {
                let (cx, cy, rad) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0, (b[2] - b[0]).min(b[3] - b[1]) / 2.0 * r.f(0.2, 0.6));
                d.loops.push(shapes::new_circle([cx, cy], rad.max(0.3), id));
            }
        }
        if kind == "inlay" {
            let sel: BTreeSet<usize> = (first..d.loops.len()).collect();
            for i in &sel { d.loops[*i].op = 0; }
            shapes::add_operation(d, &sel, "inlay");
            let last = d.ops.len();
            for o in &mut d.ops[last - 2..] { o.p.depth = r.f(0.5, t.min(8.0)); }
            continue;
        }
        let tool = if kind == "vcarve" { r.pick(&cone).clone() } else { r.pick(&flat).clone() };
        let mut p = shapes::default_op_params(&tool);
        p.kind = kind.into();
        p.dir = if r.chance(0.5) { "climb" } else { "conv" }.into();
        p.depth = if r.chance(0.05) { r.f(t, t * 1.3) } else { r.f(0.2, t) };
        p.ramp = r.chance(0.6);
        p.air = r.chance(0.04);
        if r.chance(0.15) { p.pause = Some(if r.chance(0.5) { String::new() } else { "Flip the (stock)".into() }); }
        p.pass_depth = if r.chance(0.4) { Some(r.f(0.3, 5.0)) } else { None };
        if kind == "outside" || kind == "inside" {
            if r.chance(0.2) { p.lead = Some(r.f(1.0, 6.0)); }
            if kind == "outside" && r.chance(0.3) { p.tabs = true; p.tab_n = Some(r.i(2, 6) as f64); p.tab_w = Some(r.f(2.0, 10.0)); p.tab_h = Some(r.f(0.5, 3.0)); }
        }
        if r.chance(0.2) {
            p.custom = Some(CustomFeeds { rpm: if r.chance(0.5) { Some(r.f(4000.0, 20000.0).round()) } else { None }, feed: if r.chance(0.5) { Some(r.f(100.0, 1500.0).round()) } else { None }, plunge: if r.chance(0.3) { Some(r.f(50.0, 300.0).round()) } else { None }, so: if r.chance(0.3) { Some(r.f(10.0, 60.0).round()) } else { None } });
        }
        d.oid = id;
        d.ops.push(Operation { id, p });
        d.act = id;
    }
    f
}

fn strip_number(l: &str) -> &str {
    let b = l.as_bytes();
    if b.first() == Some(&b'N') {
        let digits = b[1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && b.get(1 + digits) == Some(&b' ') { return &l[digits + 2..]; }
    }
    l
}

/// Everything the invariants say about one project. Returns the problems found, empty when all is well.
fn problems(f: &ProjectFile) -> Vec<String> {
    let mut bad = Vec::new();
    let d = &f.data;
    let mut outs: Vec<(Units, Output)> = Vec::new();
    for units in [Units::Metric, Units::Imperial] {
        match catch_unwind(AssertUnwindSafe(|| generate(d, units))) {
            Err(_) => { bad.push(format!("{units:?}: the generator panicked")); }
            // a project that cannot work is refused with a sentence, never with nothing
            Ok(Err(e)) => { if e.trim().is_empty() { bad.push(format!("{units:?}: error without a message")); } }
            Ok(Ok(o)) => outs.push((units, o)),
        }
    }
    // a refusal in one unit must be a refusal in the other
    if outs.len() == 1 { bad.push("one unit generates, the other refuses".into()); }
    for (units, o) in &outs {
        let tag = format!("{units:?}");
        let t = trace(&o.lines);
        for h in &t.hazards { bad.push(format!("{tag}: checker: {h}")); }
        if !o.time_min.is_finite() || o.time_min < 0.0 { bad.push(format!("{tag}: time {}", o.time_min)); }
        if o.lines.iter().any(|l| l.contains("NaN") || l.contains("inf")) { bad.push(format!("{tag}: NaN or inf in the text")); }
        // text against generator
        let arcs = d.machine.as_ref().is_some_and(|m| m.arcs == Some(true));
        let tol = 0.05 + o.cut_len_mm * if arcs { 0.01 } else { 0.0005 } + if *units == Units::Imperial { 0.002 * t.cut_moves as f64 } else { 0.0 };
        if (t.cut_len_mm - o.cut_len_mm).abs() > tol { bad.push(format!("{tag}: the text cuts {:.2} mm, the generator says {:.2} mm", t.cut_len_mm, o.cut_len_mm)); }
        let st = &d.stock;
        let zt = if st.zm == "top" { 0.0 } else { st.t - st.zh };
        let low = o.moves.iter().filter(|m| m.cut).map(|m| m.from[2].min(m.to[2])).fold(f64::MAX, f64::min) + zt;
        if let Some(deep) = t.deepest_cut_mm { if (deep - low).abs() > 0.01 { bad.push(format!("{tag}: deepest {deep} in the text, {low} in the generator")); } }
        // the end: lift to safe Z, spindle off, X0 Y0, end of program
        let l: Vec<&str> = o.lines.iter().map(|x| strip_number(x)).collect();
        let n = l.len();
        // a pause after the last operation adds "M5, M0 + message" in front of the usual ending; its lift comes before them
        let mut at = n.saturating_sub(4);
        if at >= 2 && l[at].starts_with("M0 ") && l[at - 1] == "M5" { at -= 2; }
        let end_ok = n >= 4 && l[n - 3] == "M5" && l[n - 2] == "G0 X0 Y0" && (l[n - 1] == "M2" || l[n - 1] == "M30") && (l[n - 4].starts_with("G0 Z") || (l[n - 4].starts_with("M0 ") && l[at].starts_with("G0 Z")));
        if !end_ok { bad.push(format!("{tag}: odd ending {:?}", &l[n.saturating_sub(4)..])); }
        else {
            let z: f64 = l[at][4..].parse().unwrap_or(f64::NAN);
            // safe Z, or 10 mm above it when the last operation is an air cut
            let safe = (d.job.safe + zt) / units.factor();
            let top = safe + 10.0 / units.factor();
            if z < safe - 0.01 || z > top + 0.01 { bad.push(format!("{tag}: ends at Z {z}, safe Z is {safe}")); }
        }
        // nothing cut below the stock bottom without a warning
        let bottom = -(if st.zm == "top" { st.t } else { st.zh });
        let air = d.job.air || d.ops.iter().all(|op| op.p.air);
        if let Some(deep) = t.deepest_cut_mm { if !air && deep < bottom - 0.01 && !o.notes.iter().any(|x| x.contains("exceeds stock thickness") || x.contains("through")) {
            bad.push(format!("{tag}: cuts to Z {deep}, below the stock bottom {bottom}, and no note says so"));
        } }
    }
    // inches and millimetres describe the same job
    if outs.len() == 2 {
        let (a, b) = (trace(&outs[0].1.lines), trace(&outs[1].1.lines));
        if (a.cut_len_mm - b.cut_len_mm).abs() > 0.05 + a.cut_len_mm * 0.001 + 0.003 * a.cut_moves as f64 { bad.push(format!("mm and in differ: {:.2} against {:.2} mm", a.cut_len_mm, b.cut_len_mm)); }
        if outs[0].1.lines.len() != outs[1].1.lines.len() && d.machine.as_ref().is_none_or(|m| m.arcs != Some(true)) { bad.push(format!("mm and in: {} against {} lines", outs[0].1.lines.len(), outs[1].1.lines.len())); }
        // arcs on or off: the same path (a curve for a chain of short lines)
        let mut flip = d.clone();
        let m = flip.machine.get_or_insert_with(Default::default);
        m.arcs = Some(m.arcs != Some(true));
        if let Ok(o2) = generate(&flip, Units::Metric) {
            let (t1, t2) = (trace(&outs[0].1.lines), trace(&o2.lines));
            if (t1.cut_len_mm - t2.cut_len_mm).abs() > 0.1 + t1.cut_len_mm * 0.01 { bad.push(format!("arcs on/off: {:.2} against {:.2} mm", t1.cut_len_mm, t2.cut_len_mm)); }
            if t1.deepest_cut_mm.zip(t2.deepest_cut_mm).is_some_and(|(a, b)| (a - b).abs() > 0.01) { bad.push("arcs on/off: different depth".into()); }
            for h in &t2.hazards { bad.push(format!("arcs flipped: checker: {h}")); }
        }
        // the same project twice gives the same text
        if let Ok(again) = generate(d, Units::Metric) { if again.lines != outs[0].1.lines { bad.push("the same project gave different text twice".into()); } }
    }
    bad
}

fn run(seeds: std::ops::Range<u64>) {
    std::panic::set_hook(Box::new(|_| {}));
    let mut why = std::collections::BTreeMap::<String, usize>::new();
    let (mut made, mut refused, mut kinds) = (0usize, 0usize, std::collections::BTreeMap::<String, usize>::new());
    let mut failures: Vec<(u64, Vec<String>)> = Vec::new();
    for seed in seeds {
        let f = random_project(seed);
        match generate(&f.data, Units::Metric) {
            Ok(_) => { made += 1; for o in &f.data.ops { *kinds.entry(o.p.kind.clone()).or_default() += 1; } }
            Err(e) => { refused += 1; *why.entry(e.chars().take(70).collect::<String>()).or_default() += 1; }
        }
        let t0 = std::time::Instant::now();
        let bad = catch_unwind(AssertUnwindSafe(|| problems(&f))).unwrap_or_else(|_| vec!["the checks themselves panicked".into()]);
        if t0.elapsed().as_secs_f64() > 2.0 { eprintln!("seed {seed}: slow, {:.1} s", t0.elapsed().as_secs_f64()); }
        if !bad.is_empty() { failures.push((seed, bad)); }
    }
    let _ = std::panic::take_hook();
    eprintln!("{made} projects generated, {refused} refused with a message; operations in the generated ones: {kinds:?}\nrefusals: {why:#?}");
    // a test that mostly gets refusals checks nothing
    assert!(made >= (made + refused) * 7 / 10, "only {made} of {} projects generate", made + refused);
    if !failures.is_empty() {
        for (seed, bad) in failures.iter().take(15) {
            let path = std::env::temp_dir().join(format!("putia-random-{seed}.project.json"));
            std::fs::write(&path, serde_json::to_string(&random_project(*seed)).unwrap()).ok();
            eprintln!("seed {seed} ({}):", path.display());
            for b in bad { eprintln!("    {b}"); }
        }
        panic!("{} project(s) with problems; first seeds: {:?}", failures.len(), failures.iter().take(15).map(|f| f.0).collect::<Vec<_>>());
    }
}

#[test]
fn twenty_random_projects_are_clean() { run(0..20); }

#[test]
#[ignore]
fn thousands_of_random_projects_are_clean() {
    let n: u64 = std::env::var("COUNT").ok().and_then(|v| v.parse().ok()).unwrap_or(5000);
    run(0..n);
}

/// One seed, for looking at a failure: `SEED=1234 cargo test --test random_projects one_seed -- --nocapture --ignored`
#[test]
#[ignore]
fn one_seed() {
    let seed: u64 = std::env::var("SEED").ok().and_then(|v| v.parse().ok()).expect("set SEED");
    let f = random_project(seed);
    let path = std::env::temp_dir().join(format!("putia-random-{seed}.project.json"));
    std::fs::write(&path, serde_json::to_string(&f).unwrap()).unwrap();
    eprintln!("written to {}", path.display());
    let d = &f.data;
    eprintln!("stock {} x {} x {} zero {} Z0 {} {}; safe {}; air {}", d.stock.w, d.stock.h, d.stock.t, d.stock.zr, d.stock.zm, d.stock.zh, d.job.safe, d.job.air);
    for o in &d.ops {
        let t = d.tools.iter().find(|t| t.id == o.p.tool).unwrap();
        eprintln!("  op {} {} depth {:.3} passD {:?} tool {} {} dia {} angle {} tip {:.2} arc {:.2} ramp {} tabs {} lead {:?} air {}", o.id, o.p.kind, o.p.depth, o.p.pass_depth, t.shape, t.id, t.dia, t.angle, t.tip, t.arc, o.p.ramp, o.p.tabs, o.p.lead, o.p.air);
    }
    if let Ok(o) = generate(d, Units::Metric) { for n in &o.notes { eprintln!("  note: {n}"); } }
    for b in problems(&f) { eprintln!("  PROBLEM {b}"); }
}
