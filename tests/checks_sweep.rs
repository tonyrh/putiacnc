//! False-positive sweep for the "against the drawing" checks (Overcut, TooSmall): every combination of
//! tool shape, operation kind, outline and depth below is a job that is fine as drawn, so none may be reported.
//! Slow (about a minute in release): `cargo test --release --test checks_sweep -- --ignored --nocapture`.
//! A second part checks that parts packed closer than the tool are reported (true positives).
use putiacnc_core::checks::{self, Kind};
use putiacnc_core::model::{LoopData, Operation, ProjectData, Tool};
use putiacnc_core::{generate, shapes, Units};

fn tools() -> Vec<(String, Tool)> {
    let mk = |shape: &str, dia: f64, angle: f64, tip: f64, arc: f64| {
        let mut t = shapes::blank_project().data.tools[0].clone();
        t.id = format!("sw-{shape}-{dia}-{angle}"); t.name = t.id.clone();
        t.shape = shape.into(); t.dia = dia; t.angle = angle; t.tip = tip; t.arc = arc;
        t
    };
    vec![
        ("flat3", mk("flat", 3.0, 0.0, 0.0, 0.0)), ("flat6", mk("flat", 6.0, 0.0, 0.0, 0.0)),
        ("ball3", mk("ball", 3.0, 0.0, 0.0, 0.0)), ("ball6", mk("ball", 6.0, 0.0, 0.0, 0.0)),
        ("bull6", mk("bull", 6.0, 0.0, 0.0, 0.0)),
        ("vbit60x6", mk("vbit", 6.0, 60.0, 0.0, 0.0)), ("vbit90x12", mk("vbit", 12.0, 90.0, 0.0, 0.0)),
        ("engrave20x3", mk("engrave", 3.0, 20.0, 0.1, 0.0)), ("taper10x3", mk("taper", 3.0, 10.0, 0.5, 0.0)),
        ("arc6", mk("arc", 6.0, 0.0, 0.0, 2.0)),
    ].into_iter().map(|(n, t)| (n.to_string(), t)).collect()
}

fn base(t: &Tool, kind: &str, depth: f64) -> ProjectData {
    let mut f = shapes::blank_project();
    let d = &mut f.data;
    d.stock.w = 200.0; d.stock.h = 150.0; d.stock.t = 12.0;
    d.tools = vec![t.clone()];
    d.ops.clear();
    d.ops.push(Operation { id: 1, p: shapes::default_op_params(&t.id) });
    d.ops[0].p.kind = kind.into(); d.ops[0].p.depth = depth;
    d.oid = 1; d.act = 1; d.machine = None;
    f.data
}

fn outlines(op: i64, dx: f64) -> Vec<(&'static str, LoopData)> {
    vec![
        ("rect", shapes::new_rect([50.0 + dx, 40.0], [100.0 + dx, 80.0], 0.0, op)),
        ("round-rect", shapes::new_rect([50.0 + dx, 40.0], [100.0 + dx, 80.0], 6.0, op)),
        ("ellipse", shapes::new_ellipse([50.0 + dx, 40.0], [100.0 + dx, 80.0], op)),
        ("hexagon", shapes::new_polygon([75.0 + dx, 60.0], 25.0, 0.0, 6, None, op).unwrap()),
        ("star", shapes::new_polygon([75.0 + dx, 60.0], 28.0, 0.0, 5, Some(14.0), op).unwrap()),
        ("circle", shapes::new_circle([75.0 + dx, 60.0], 22.0, op)),
    ]
}

fn bad(d: &ProjectData) -> Option<Vec<(Kind, String)>> {
    let o = generate(d, Units::Metric).ok()?;
    let f: Vec<_> = checks::run(d, &o).into_iter().filter(|f| matches!(f.kind, Kind::Overcut | Kind::TooSmall)).map(|f| (f.kind, f.msg)).collect();
    if f.is_empty() { None } else { Some(f) }
}

#[test]
#[ignore]
fn nothing_is_reported_for_jobs_that_are_fine_as_drawn() {
    let (mut runs, mut skipped, mut fails) = (0, 0, vec![]);
    for (tn, t) in tools() {
        for kind in ["outside", "inside", "pocket"] {
            for depth in [0.5, 1.0, 2.0, 4.0] {
                for (on, lp) in outlines(1, 0.0) {
                    let mut d = base(&t, kind, depth);
                    d.loops.push(lp);
                    if generate(&d, Units::Metric).is_err() { skipped += 1; continue; }
                    runs += 1;
                    if let Some(f) = bad(&d) { fails.push(format!("{tn} {kind} depth {depth} {on}: {:?}", f)); }
                    // two separate parts with a gap wider than the tool: still fine
                    if kind == "outside" {
                        let gap = t.dia + 3.0;
                        let mut d2 = base(&t, kind, depth);
                        d2.ops.push(Operation { id: 2, p: d2.ops[0].p.clone() }); d2.oid = 2;
                        d2.loops.push(lp_clone(on, 1, 0.0));
                        d2.loops.push(lp_clone(on, 2, 60.0 + gap));
                        if generate(&d2, Units::Metric).is_err() { skipped += 1; continue; }
                        runs += 1;
                        if let Some(f) = bad(&d2) { fails.push(format!("{tn} two parts gap>{gap} depth {depth} {on}: {:?}", f)); }
                    }
                }
            }
        }
    }
    eprintln!("sweep: {runs} jobs checked, {skipped} skipped (no toolpath), {} false positives", fails.len());
    for f in &fails { eprintln!("  {f}"); }
    assert!(fails.is_empty());
}

fn lp_clone(name: &str, op: i64, dx: f64) -> LoopData { outlines(op, dx).into_iter().find(|(n, _)| *n == name).unwrap().1 }

#[test]
#[ignore]
fn parts_packed_closer_than_the_tool_are_reported() {
    let (mut runs, mut missed) = (0, vec![]);
    for (tn, t) in tools() {
        // a flat, ball or bull cutter reaches its full width at these depths; cones reach it only as deep as their height
        for depth in [3.0, 5.0] {
            let mut d = base(&t, "outside", depth);
            d.ops.push(Operation { id: 2, p: d.ops[0].p.clone() }); d.oid = 2;
            d.loops.push(shapes::new_rect([50.0, 40.0], [100.0, 80.0], 0.0, 1));
            // second part 1 mm away, much closer than any of the tools is wide
            d.loops.push(shapes::new_rect([101.0, 40.0], [150.0, 80.0], 0.0, 2));
            if generate(&d, Units::Metric).is_err() { continue; }
            runs += 1;
            let g = putiacnc_core::toolpath::ToolGeo::new(&t);
            // the cutter edge must reach beyond 1 mm + tolerance at this depth for a report to be due
            if g.r_at(depth) <= 1.5 { continue; }
            if bad(&d).is_none() { missed.push(format!("{tn} depth {depth}")); }
        }
    }
    eprintln!("sweep: {runs} packed jobs, {} not reported", missed.len());
    for m in &missed { eprintln!("  missed: {m}"); }
    assert!(missed.is_empty());
}
