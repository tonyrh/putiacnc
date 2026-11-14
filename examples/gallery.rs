//! Writes the projects of the gallery on the project page (docs/gallery.html), as plain project files that fit a 300 x 200 x 60 mm machine
//! with the starter tools: the program's sample projects, and some more that are drawn here with the same functions the program's own
//! buttons use. Every run gives the tools new ids, so a project that is already in the gallery is only written again with `--all`.
//! Usage: cargo run --release --example gallery -- docs/gallery [--all]
use putiacnc_core::joints::{reliefs, Cut, Relief};
use putiacnc_core::model::{LoopData, Operation, PlugPiece, ProjectData, ShapeGroup, TextSpec, Tool};
use putiacnc_core::sample::Kind;
use putiacnc_core::text::{Align, TextStyle};
use putiacnc_core::{generate, linetext, pieces, sample, save_project, shapes, Units};

fn tool<'a>(d: &'a ProjectData, name: &str) -> &'a Tool { d.tools.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no tool {name}")) }

fn op(d: &mut ProjectData, kind: &str, depth: f64, tool_id: &str, pass: f64) -> i64 {
    let mut p = shapes::default_op_params(tool_id);
    p.kind = kind.into(); p.depth = depth;
    if pass > 0.0 { p.pass_depth = Some(pass); }
    d.oid += 1;
    d.ops.push(Operation { id: d.oid, p });
    d.act = d.oid;
    d.oid
}

fn stock(d: &mut ProjectData, w: f64, h: f64, t: f64) { d.stock.w = w; d.stock.h = h; d.stock.t = t; }

fn pts(v: &[(f64, f64)], dx: f64, dy: f64) -> Vec<[f64; 2]> { v.iter().map(|&(x, y)| [x + dx, y + dy]).collect() }

/// A coaster with an eight-point star inlaid in it: the star is cut as a pocket with the V-bit, its plug is cut from a separate piece of a
/// darker wood and glued in, then the coaster is cut out.
fn coaster(d: &mut ProjectData) {
    stock(d, 120.0, 120.0, 18.0);
    let (vbit, flat) = (tool(d, "60° V-bit (1/4″)").id.clone(), tool(d, "1/8″ flat end mill").id.clone());
    let star = shapes::new_polygon([60.0, 60.0], 34.0, std::f64::consts::FRAC_PI_2, 8, Some(0.55), 0).unwrap();
    d.loops.push(star);
    let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
    shapes::add_operation(d, &sel, "inlay");
    let (pocket, plug) = (d.ops[0].id, d.ops[1].id);
    for o in d.ops.iter_mut() { o.p.depth = 4.0; o.p.tool = vbit.clone(); o.p.tool2 = Some(flat.clone()); }
    let piece = PlugPiece { t: 6.0, w: 100.0, h: 100.0, mat: "darkwood".into(), ..pieces::default_piece(&d.stock) };
    shapes::put_plug_on_piece(d, pocket, plug, Some(piece));
    let out = op(d, "outside", d.stock.t, &flat, 4.0);
    d.ops.last_mut().unwrap().p.tabs = true;
    d.loops.push(shapes::new_circle([60.0, 60.0], 50.0, out));
    d.notes = "INLAY COASTER\nA round coaster with an eight-point star inlaid in it.\n\nOperations\n1. Inlay pocket: the star is V-carved into the coaster, 4 mm deep, with the 60° V-bit; the flat end mill clears the floor.\n2. Inlay plug: the matching star, cut from a separate piece of darker wood (6 mm). It is cut a little smaller (0.05 mm) so it presses in, and the V-bit gives it the sloped sides that fit the pocket.\n3. Profile (outside): cuts the coaster out, leaving 4 small holding tabs.\n\nAssembly\n- Cut the pocket first, then put the dark piece on the machine and cut the plug (the G-code is split into one file per piece).\n- Glue the plug into the pocket, let it dry, then sand the face flat: the plug stands a little proud on purpose.\n- Cut the coaster out last, so the stock stays held while you work on the face.\n\nBefore cutting for real\n- Set X0 Y0 at the lower-left corner and Z0 on the top of the stock, and tick \"Air cut\" in each operation for a first run.".into();
}

/// A phone stand that goes together without glue: two side plates with the cradle cut into their top, and two crossbars whose ends fit the
/// slots of the plates. Cut from one piece of 9 mm plywood.
fn phone_stand(d: &mut ProjectData) {
    let t = 9.0;
    stock(d, 200.0, 150.0, t);
    let flat = tool(d, "1/8″ flat end mill").clone();
    let o = op(d, "outside", t, &flat.id, 3.0);
    d.ops.last_mut().unwrap().p.tabs = true;
    // the side plate: base 80 mm long, a back 80 mm high, a ledge for the phone and the sloped rest it leans on
    let side = [(0.0, 0.0), (80.0, 0.0), (80.0, 24.0), (56.0, 24.0), (14.0, 80.0), (0.0, 80.0)];
    // the two slots for the crossbars (a back bar and a front bar), 0.05 mm wider than the plywood
    let c = 0.05;
    let slots = [[(8.0, 30.0), (8.0 + t + c, 30.0 + 12.0 + c)], [(62.0, 6.0), (62.0 + t + c, 6.0 + 12.0 + c)]];
    for (dx, dy) in [(10.0, 10.0), (110.0, 10.0)] {
        let mut loops: Vec<LoopData> = vec![shapes::new_poly(&pts(&side, dx, dy), true, o).unwrap()];
        for s in &slots { loops.push(shapes::new_poly(&pts(&[s[0], (s[1].0, s[0].1), s[1], (s[0].0, s[1].1)], dx, dy), true, o).unwrap()); }
        let refs: Vec<&LoopData> = loops.iter().collect();
        match reliefs(&refs, flat.dia, Cut::Outside, o, Relief::Dogbone) { Some((ls, _)) => d.loops.extend(ls), None => d.loops.extend(loops) }
    }
    // the crossbars: 60 mm between the plates plus a tab through each plate
    for y in [105.0, 125.0] { d.loops.push(shapes::new_rect([10.0, y], [10.0 + 60.0 + 2.0 * t, y + 12.0], 0.0, o)); }
    d.notes = "PHONE STAND\nA phone or small tablet stand that goes together without glue, cut from one piece of 9 mm plywood.\n\nWhat is on the stock\n- Two identical side plates with a ledge for the bottom of the phone and a sloped rest it leans on (about 55°). Each has two slots with dogbone corner reliefs, so the square ends of the bars go in.\n- Two crossbars, 60 mm between the plates plus a tab at each end that goes through the plate.\n- One operation, Profile (outside), cuts everything out through the stock and leaves 4 small holding tabs on each part.\n\nAssembly\n- Break the parts off their tabs and sand the tab marks.\n- Push the bars into the slots of one side plate, then fit the other plate on the free ends.\n\nBefore cutting for real\n- The slots are made for 9 mm material: measure your plywood (it is often a little thinner), set the Thickness field in the Stock panel and change the slot widths to match, or the bars will be loose.\n- Set X0 Y0 at the lower-left corner and Z0 on the top of the stock, and tick \"Air cut\" for a first run.".into();
}

/// A cutting board with a juice groove, a hole to hang it by and a line of lettering.
fn cutting_board(d: &mut ProjectData) {
    stock(d, 270.0, 190.0, 20.0);
    let (ball, flat, engr) = (tool(d, "6 mm ball nose").id.clone(), tool(d, "6 mm flat end mill").id.clone(), tool(d, "30° engraving bit (1/8″)").id.clone());
    // 1. the juice groove, a round-bottomed channel along a rounded rectangle
    let g = op(d, "online", 6.0, &ball, 3.0);
    d.loops.push(shapes::new_rect([30.0, 30.0], [240.0, 140.0], 14.0, g));
    // 2. the lettering in the middle of the groove
    let text = "Buon appetito";
    let l = op(d, "online", 1.0, &engr, 0.0);
    let probe = linetext::line_text_loops("futural", text, TextStyle::plain(10.0, 0.0, Align::Center), [0.0, 0.0], l).unwrap();
    let b = shapes::bbox_many(probe.iter()).unwrap();
    let size = 10.0 * (150.0 / (b[2] - b[0]));
    let style = TextStyle::plain(size, size * 0.08, Align::Center);
    let mut letters = linetext::line_text_loops("futural", text, style, [0.0, 0.0], l).unwrap();
    let gid = 1;
    for lt in &mut letters { shapes::translate(lt, 135.0, 85.0 - size / 2.0); lt.grp = Some(gid); }
    let font = linetext::fonts().into_iter().find(|f| f.0 == "futural").map(|f| f.1.to_string()).unwrap_or_default();
    let spec = TextSpec { content: text.into(), font, size, spacing: style.spacing_mm, word_spacing: 0.0, kerning: false, align: Align::Center, flow: style.flow, radius: 0.0, arc_deg: 90.0, arc_in: false, fit_width: 0.0, shadow: [0.0, 0.0], path: vec![] };
    d.groups.push(ShapeGroup { id: gid, name: format!("Text: {text}"), locked: false, text: Some(spec) });
    d.loops.extend(letters);
    // 3. the hole to hang it by, cut right through
    let h = op(d, "inside", d.stock.t, &flat, 4.0);
    d.loops.push(shapes::new_rect([100.0, 154.0], [170.0, 168.0], 7.0, h));
    // 4. the outline, last, with tabs
    let o = op(d, "outside", d.stock.t, &flat, 4.0);
    d.ops.last_mut().unwrap().p.tabs = true;
    d.loops.push(shapes::new_rect([10.0, 10.0], [260.0, 180.0], 18.0, o));
    d.notes = "CUTTING BOARD\nA hardwood cutting board 250 x 170 mm, 20 mm thick, with a juice groove, a hanging hole and a line of lettering.\n\nOperations\n1. On the line: the juice groove, 6 mm deep, with the 6 mm ball nose, so the channel has a round bottom.\n2. On the line: \"Buon appetito\" engraved 1 mm deep with the 30° engraving bit. Change the words with \"Edit text\" in the Text card.\n3. Profile (inside): the hole to hang the board by, cut through.\n4. Profile (outside): cuts the board out with rounded corners, leaving 4 small holding tabs.\n\nBefore cutting for real\n- Use a hardwood that is safe for food (maple, beech, walnut) and a food-safe oil to finish; sand all the edges and the groove.\n- Set X0 Y0 at the lower-left corner and Z0 on the top of the stock, and tick \"Air cut\" in each operation for a first run.".into();
}

/// A pen holder cut from a thick block: three pen holes and a slot for cards, then the outline.
fn pen_holder(d: &mut ProjectData) {
    stock(d, 120.0, 90.0, 40.0);
    let (f4, f3) = (tool(d, "4 mm flat end mill").id.clone(), tool(d, "1/8″ flat end mill").id.clone());
    let p = op(d, "pocket", 30.0, &f4, 5.0);
    d.ops.last_mut().unwrap().p.helix = true;
    for x in [35.0, 60.0, 85.0] { d.loops.push(shapes::new_circle([x, 58.0], 6.0, p)); }
    let s = op(d, "pocket", 22.0, &f4, 5.0);
    d.ops.last_mut().unwrap().p.helix = true;
    d.loops.push(shapes::new_rect([25.0, 23.0], [95.0, 33.0], 3.0, s));
    let o = op(d, "outside", d.stock.t, &f3, 5.0);
    d.ops.last_mut().unwrap().p.tabs = true;
    d.loops.push(shapes::new_rect([10.0, 10.0], [110.0, 80.0], 10.0, o));
    d.notes = "PEN HOLDER\nA desk pen holder cut from a block of wood 40 mm thick.\n\nOperations\n1. Pocket: three round holes of 12 mm, 30 mm deep, for pens and pencils. The tool enters each one in a spiral instead of plunging.\n2. Pocket: a slot 70 x 10 mm, 22 mm deep, for visiting cards or sticky notes.\n3. Profile (outside): cuts the block out with rounded corners, leaving 4 small holding tabs.\n\nBefore cutting for real\n- The holes are deep: enter the flute length of your cutter in the tool's settings and the program warns you if it is too short.\n- Set X0 Y0 at the lower-left corner and Z0 on the top of the stock, and tick \"Air cut\" in each operation for a first run.".into();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "docs/gallery".into());
    std::fs::create_dir_all(&out).unwrap();
    if args.iter().any(|a| a == "--all") {
        for (id, kind) in [("name-plate", Kind::NamePlate), ("finger-joint-box", Kind::Box), ("oval-sign", Kind::Oval), ("relief-rosette", Kind::Relief), ("relief-from-shapes", Kind::ShapedRelief)] {
            let mut f = shapes::blank_project();
            sample::fill_kind(&mut f.data, false, kind).unwrap();
            f.name = kind.title().into();
            write(&out, id, &f);
        }
    }
    let drawn: [(&str, &str, fn(&mut ProjectData)); 4] = [("inlay-coaster", "Inlay coaster", coaster), ("phone-stand", "Phone stand", phone_stand), ("cutting-board", "Cutting board", cutting_board), ("pen-holder", "Pen holder", pen_holder)];
    for (id, name, build) in drawn {
        let mut f = shapes::blank_project();
        f.name = name.into();
        build(&mut f.data);
        write(&out, id, &f);
        let o = generate(&f.data, Units::Metric).unwrap_or_else(|e| panic!("{id}: {e}"));
        println!("   {id}: {} lines, {:.1} min, notes: {:?}", o.lines.len(), o.time_min, o.notes);
    }
}

fn write(out: &str, id: &str, f: &putiacnc_core::model::ProjectFile) {
    let path = format!("{out}/{id}.project.json");
    std::fs::write(&path, save_project(f).unwrap()).unwrap();
    println!("{path}");
}
