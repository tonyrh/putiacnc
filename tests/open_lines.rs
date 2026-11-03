use putiacnc_core::model::Operation;
use putiacnc_core::{generate, shapes, Units};

/// An open line cut in several depths is cut back and forth: no depth runs back across the work from the end of the line to its start.
#[test]
fn an_open_line_is_cut_back_and_forth_not_retraced() {
    let mut f = shapes::blank_project();
    let d = &mut f.data;
    d.stock.w = 60.0; d.stock.h = 40.0; d.stock.t = 12.0;
    let tool = d.tools.iter().find(|t| t.shape == "flat").unwrap().id.clone();
    let mut p = shapes::default_op_params(&tool);
    p.kind = "online".into();
    p.depth = 3.0;
    p.pass_depth = Some(1.0);
    // an L-shaped open line: its start and end are far apart
    d.loops.push(shapes::new_poly(&[[10.0, 10.0], [40.0, 10.0], [40.0, 30.0]], false, 1).unwrap());
    d.ops.push(Operation { id: 1, p });
    d.oid = 1; d.act = 1;
    let o = generate(&f.data, Units::Metric).unwrap();
    let cuts: Vec<_> = o.moves.iter().filter(|m| m.cut && (m.from[0] != m.to[0] || m.from[1] != m.to[1])).collect();
    assert!(cuts.len() >= 6, "{} cutting moves", cuts.len());
    // every cutting move along the line follows one of its two legs (none is the chord from the end back to the start)
    for m in &cuts {
        let leg1 = (m.from[1] - 10.0).abs() < 0.2 && (m.to[1] - 10.0).abs() < 0.2;
        let leg2 = (m.from[0] - 40.0).abs() < 0.2 && (m.to[0] - 40.0).abs() < 0.2;
        assert!(leg1 || leg2, "a cut across the work: {:?} -> {:?}", m.from, m.to);
    }
    // and all three depths are reached
    for z in [-1.0, -2.0, -3.0] { assert!(cuts.iter().any(|m| (m.to[2] - z).abs() < 1e-6), "depth {z} missing"); }
}
