use putiacnc_core::{generate, load_project, Units};

fn sample(name: &str) -> String { std::fs::read_to_string(format!("{}/tests/data/{}.project.json", env!("CARGO_MANIFEST_DIR"), name)).unwrap() }

/// Length of the cutting path (G1, G2, G3) and the largest radius mismatch among the arcs.
fn walk(g: &str) -> (f64, f64, usize) {
    let (mut p, mut z) = ([0.0_f64; 2], 0.0_f64);
    let (mut len, mut gap, mut arcs) = (0.0, 0.0_f64, 0);
    for l in g.lines() {
        if l.starts_with('(') { continue; }
        let w: Vec<&str> = l.split_whitespace().collect();
        let get = |c: char| w.iter().find(|x| x.starts_with(c) && x.len() > 1).and_then(|x| x[1..].parse::<f64>().ok());
        let cmd = w.first().copied().unwrap_or("");
        let (x, y) = (get('X').unwrap_or(p[0]), get('Y').unwrap_or(p[1]));
        let zn = get('Z').unwrap_or(z);
        match cmd {
            "G1" => { len += ((x - p[0]).powi(2) + (y - p[1]).powi(2) + (zn - z).powi(2)).sqrt(); }
            "G2" | "G3" => {
                arcs += 1;
                let c = [p[0] + get('I').unwrap(), p[1] + get('J').unwrap()];
                let (r0, r1) = ((p[0] - c[0]).hypot(p[1] - c[1]), (x - c[0]).hypot(y - c[1]));
                gap = gap.max((r0 - r1).abs());
                let (a0, a1) = ((p[1] - c[1]).atan2(p[0] - c[0]), (y - c[1]).atan2(x - c[0]));
                let mut da = a1 - a0;
                if cmd == "G3" { while da <= 0.0 { da += std::f64::consts::TAU; } } else { while da >= 0.0 { da -= std::f64::consts::TAU; } }
                assert!(da.abs() <= std::f64::consts::PI + 1e-6, "arc over half a circle: {l}");
                len += r0 * da.abs();
            }
            _ => {}
        }
        if matches!(cmd, "G0" | "G1" | "G2" | "G3") { p = [x, y]; z = zn; }
    }
    (len, gap, arcs)
}

#[test]
fn arcs_keep_the_path_and_shorten_the_file() {
    for name in ["outside_holes", "pocket_island", "vbit_profile", "two_ops_toolchange"] {
        let mut p = load_project(&sample(name)).unwrap();
        p.data.machine.get_or_insert_with(Default::default).arcs = Some(false);
        let plain = generate(&p.data, Units::Metric).unwrap();
        p.data.machine.as_mut().unwrap().arcs = Some(true);
        let arc = generate(&p.data, Units::Metric).unwrap();
        let (lp, _, n0) = walk(&plain.gcode());
        let (la, gap, n1) = walk(&arc.gcode());
        assert_eq!(n0, 0);
        assert!(n1 > 0, "{name}: no arcs");
        assert!(arc.lines.len() < plain.lines.len(), "{name}: not shorter");
        assert!(gap < 0.004, "{name}: radius gap {gap}");
        assert!((la / lp - 1.0).abs() < 0.01, "{name}: path length {la} vs {lp}");
        // the time estimate comes from the same moves
        assert!((arc.time_min - plain.time_min).abs() < 1e-9);
        assert!(arc.notes.iter().any(|n| n.contains("G2/G3")));
        println!("{name}: {} -> {} lines, {} arcs", plain.lines.len(), arc.lines.len(), n1);
    }
}

#[test]
fn off_by_default_and_in_inches() {
    let mut p = load_project(&sample("outside_holes")).unwrap();
    assert!(!generate(&p.data, Units::Metric).unwrap().gcode().contains("G2 "));
    p.data.machine.get_or_insert_with(Default::default).arcs = Some(true);
    let g = generate(&p.data, Units::Imperial).unwrap().gcode();
    let (_, gap, n) = walk(&g);
    assert!(n > 0 && gap < 0.004 / 25.4 * 2.0, "inches: {n} arcs, gap {gap}");
}
