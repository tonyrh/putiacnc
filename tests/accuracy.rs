//! The toolpath follows a smooth curve closely: no chord of an outside profile of a big ellipse cuts into the part.
use putiacnc_core::{checks, generate, shapes, Units};

fn gap_to_polygon(poly: &[[f64; 2]], p: [f64; 2]) -> f64 {
    let mut best = f64::MAX;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-12)).clamp(0.0, 1.0);
        best = best.min((p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy));
    }
    best
}

#[test]
fn an_outside_profile_of_a_big_ellipse_stays_one_radius_from_it_along_every_chord() {
    for (a, b) in [([20.0, 20.0], [120.0, 80.0]), ([62.26, 42.51], [147.64, 119.25])] {
        let mut d = shapes::blank_project().data;
        d.loops.push(shapes::new_ellipse(a, b, 0));
        let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
        shapes::add_operation(&mut d, &sel, "outside");
        d.ops[0].p.depth = 3.0;
        d.ops[0].p.pass_depth = Some(2.0);
        d.ops[0].p.tool = d.tools.iter().find(|t| (t.dia - 6.0).abs() < 0.01).expect("a 6 mm tool").id.clone();
        let o = generate(&d, Units::Metric).unwrap();
        let poly: Vec<[f64; 2]> = d.loops[0].pts.chunks(2).map(|c| [c[0] as f64 / 1000.0, c[1] as f64 / 1000.0]).collect();
        let mut worst = 0.0_f64;
        for m in o.moves.iter().filter(|m| m.cut && m.to[2] < -0.01 && m.from[2] < -0.01) {
            for k in 0..=10 {
                let f = k as f64 / 10.0;
                let p = [m.from[0] + (m.to[0] - m.from[0]) * f, m.from[1] + (m.to[1] - m.from[1]) * f];
                worst = worst.max((gap_to_polygon(&poly, p) - 3.0).abs());
            }
        }
        assert!(worst < 0.04, "{a:?}-{b:?}: a chord strays {worst:.3} mm from the 3 mm offset");
        assert!(!checks::run(&d, &o).iter().any(|f| f.kind == checks::Kind::Overcut), "a false overcut warning");
    }
}
