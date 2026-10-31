//! How long a frame of the 3D preview takes with big toolpaths.
//!
//! What runs here is the CPU side of the window, the same code the program runs every frame: building the interface, drawing every
//! move of the toolpath, advancing the cut simulation, and egui turning the shapes into triangles. The GPU drawing of the wood
//! (a paint callback) is recorded but not run, because there is no GPU here; a slow graphics card is not measured.
//!
//! `cargo test --release perf -- --nocapture` prints the numbers (with `PERF_COUNT=1` also how many paths and points egui gets each frame). The limits are checked in release builds only (a debug build is
//! 10 times slower and says nothing about the real thing), and the big cases are `#[ignore]`d: `--ignored` runs them.
use crate::app::App;
use eframe::egui;
use putiacnc_core::{model::Operation, shapes};

/// A pocket of `n` circles on a grid, cut with a 1 mm tool at 20 % stepover in 0.5 mm passes: a lot of short moves.
fn busy_project(n: usize) -> App {
    let mut a = App::new(None);
    let d = &mut a.project.data;
    d.stock.w = 300.0; d.stock.h = 200.0; d.stock.t = 12.0;
    d.tools[0].dia = 1.0; d.tools[0].so = 20.0; d.tools[0].pd = 0.5;
    let tool = d.tools[0].id.clone();
    let mut p = shapes::default_op_params(&tool);
    p.kind = "pocket".into(); p.depth = 6.0;
    d.ops.push(Operation { id: 1, p });
    d.loops.clear();
    for i in 0..n {
        let (col, row) = (i % 18, i / 18);
        d.loops.push(shapes::new_circle([14.0 + col as f64 * 16.0, 14.0 + row as f64 * 16.0], 7.0, 1));
    }
    a.touch();
    a
}

thread_local! { static SPLIT: std::cell::RefCell<(f64, f64, usize)> = const { std::cell::RefCell::new((0.0, 0.0, 0)) }; }

fn count(v: &[egui::Shape], paths: &mut usize, pts: &mut usize, other: &mut usize) {
    for s in v {
        match s {
            egui::Shape::Vec(x) => count(x, paths, pts, other),
            egui::Shape::Path(p) => { *paths += 1; *pts += p.points.len(); }
            _ => *other += 1,
        }
    }
}

struct Stats { median: f64, p95: f64, max: f64, n: usize }

fn stats(mut ms: Vec<f64>) -> Stats {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = ms.len().max(1);
    Stats { median: ms.get(n / 2).copied().unwrap_or(0.0), p95: ms.get((n * 95 / 100).min(n - 1)).copied().unwrap_or(0.0), max: ms.last().copied().unwrap_or(0.0), n: ms.len() }
}

/// One frame of the window at 60 frames a second of the clock; the milliseconds it took, with the triangles made.
fn frame(ctx: &egui::Context, a: &mut App, t: &mut f64) -> f64 {
    *t += 1.0 / 60.0;
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1500.0, 900.0))), time: Some(*t), ..Default::default() };
    let t0 = std::time::Instant::now();
    let out = ctx.run(input, |ctx| a.ui_frame(ctx));
    let built = t0.elapsed().as_secs_f64() * 1000.0;
    if std::env::var_os("PERF_COUNT").is_some() { let (mut paths, mut pts, mut other) = (0, 0, 0); count(&out.shapes.iter().map(|c| c.shape.clone()).collect::<Vec<_>>(), &mut paths, &mut pts, &mut other); eprintln!("  shapes: {paths} paths with {pts} points, {other} others"); }
    let _tris = ctx.tessellate(out.shapes, out.pixels_per_point);
    let total = t0.elapsed().as_secs_f64() * 1000.0;
    SPLIT.with(|s| { let mut s = s.borrow_mut(); s.0 += built; s.1 += total - built; s.2 += 1; });
    total
}

struct Run { moves: usize, gen_s: f64, catch: Stats, catch_s: f64, idle: Stats, play: Stats }

fn measure(n_circles: usize) -> Run {
    let mut a = busy_project(n_circles);
    let t0 = std::time::Instant::now();
    crate::fuzz::settle(&mut a).expect("the toolpath settles");
    let gen_s = t0.elapsed().as_secs_f64();
    let moves = a.output.as_ref().map_or(0, |o| o.moves.len());
    let ctx = egui::Context::default();
    let mut clock = 0.0;
    a.view_tab = crate::view3d::ViewTab::Preview;
    // the preview builds the cut simulation of the whole program in slices; every one of those frames counts
    let mut catch = vec![];
    let c0 = std::time::Instant::now();
    for _ in 0..20000 {
        catch.push(frame(&ctx, &mut a, &mut clock));
        if a.v3.sim.is_some() && !a.v3.catching_up && a.v3.ver == a.out_ver { break; }
    }
    let catch_s = c0.elapsed().as_secs_f64();
    assert!(!a.v3.catching_up, "the simulation never caught up");
    // nothing moves: the cost of drawing the picture again
    SPLIT.with(|s| *s.borrow_mut() = (0.0, 0.0, 0));
    let idle: Vec<f64> = (0..60).map(|_| frame(&ctx, &mut a, &mut clock)).collect();
    SPLIT.with(|s| { let s = s.borrow(); eprintln!("  idle split: building the window {:.1} ms, tessellating {:.1} ms (per frame)", s.0 / s.2 as f64, s.1 / s.2 as f64); });
    // playing from the start at 200 times speed: the simulation has to cut as the cutter goes
    a.v3.t = 0.0; a.v3.playing = true; a.v3.speed_idx = 4;
    let mut play = vec![];
    for _ in 0..120 { play.push(frame(&ctx, &mut a, &mut clock)); }
    Run { moves, gen_s, catch: stats(catch), catch_s, idle: stats(idle), play: stats(play) }
}

fn report(name: &str, r: &Run) {
    eprintln!("{name}: {} moves (generated in {:.1} s)", r.moves, r.gen_s);
    eprintln!("  catching up  {:>4} frames in {:>5.1} s   median {:>6.1} ms   p95 {:>6.1}   max {:>6.1}", r.catch.n, r.catch_s, r.catch.median, r.catch.p95, r.catch.max);
    eprintln!("  idle         {:>4} frames             median {:>6.1} ms   p95 {:>6.1}   max {:>6.1}", r.idle.n, r.idle.median, r.idle.p95, r.idle.max);
    eprintln!("  playing 200x {:>4} frames             median {:>6.1} ms   p95 {:>6.1}   max {:>6.1}", r.play.n, r.play.median, r.play.p95, r.play.max);
}

#[test]
fn the_preview_of_a_busy_toolpath_keeps_a_usable_frame_time() {
    // release: 12 circles, about 140,000 moves (measured on a laptop-class CPU: catching up 40 ms a frame, idle 40 ms, playing 17 ms);
    // a debug build is 10 times slower, so it only runs a small project to see that nothing breaks
    let release = !cfg!(debug_assertions);
    let r = measure(if release { 12 } else { 4 });
    report(if release { "12 circles" } else { "4 circles (debug build)" }, &r);
    assert!(r.moves > 20_000, "the project is busy enough to mean something: {} moves", r.moves);
    if release {
        // about four times what was measured: a slower machine passes, a frame that costs ten times more does not
        assert!(r.catch.max < 400.0, "a frame while the simulation catches up took {:.0} ms; it works in slices so the window keeps answering", r.catch.max);
        assert!(r.idle.median < 150.0, "drawing the picture again took {:.0} ms (median)", r.idle.median);
        assert!(r.play.p95 < 120.0, "playing at 200x: 1 frame in 20 took over {:.0} ms", r.play.p95);
    }
}

#[test]
fn the_split_view_with_a_busy_toolpath_costs_about_the_two_views_together() {
    let release = !cfg!(debug_assertions);
    let mut a = busy_project(if release { 12 } else { 4 });
    crate::fuzz::settle(&mut a).expect("the toolpath settles");
    let ctx = egui::Context::default();
    let mut clock = 0.0;
    let mut median_of = |a: &mut App, tab| {
        a.view_tab = tab;
        for _ in 0..20000 {
            frame(&ctx, a, &mut clock);
            if tab == crate::view3d::ViewTab::Design || (a.v3.sim.is_some() && !a.v3.catching_up && a.v3.ver == a.out_ver) { break; }
        }
        stats((0..40).map(|_| frame(&ctx, a, &mut clock)).collect()).median
    };
    let (d, p, sp) = (median_of(&mut a, crate::view3d::ViewTab::Design), median_of(&mut a, crate::view3d::ViewTab::Preview), median_of(&mut a, crate::view3d::ViewTab::Split));
    eprintln!("idle frame, median: 2D {d:.1} ms, 3D {p:.1} ms, split {sp:.1} ms");
    assert!(a.split_rect.is_some(), "the split view was drawn");
    if release { assert!(sp < 1.5 * (d + p) + 20.0, "the split view costs {sp:.0} ms where the two views alone cost {d:.0} and {p:.0}"); }
}

#[test]
#[ignore]
fn frame_times_for_small_medium_and_large_toolpaths() {
    for n in [4, 12, 30, 60] {
        let r = measure(n);
        report(&format!("{n} circles"), &r);
    }
}
