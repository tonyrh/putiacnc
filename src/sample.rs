//! The sample projects (Generate → Sample projects): a small name plate made to fit the user's own machine, tools and units.
//! The Notes tab explains it and how to try it as an air cut first.
use crate::linetext::line_text_loops;
use crate::model::{Operation, ProjectData, ShapeGroup, TextSpec, Tool};
use crate::shapes;
use crate::text::{Align, TextStyle};

fn floor_to(v: f64, step: f64) -> f64 { (v / step).floor() * step }

/// The flat cutter for the plate: the one closest to 1/8″ (3.175 mm) between 2 and 6 mm, else any flat one.
fn pick_flat(tools: &[Tool]) -> Option<&Tool> {
    let flats = || tools.iter().filter(|t| t.shape == "flat");
    flats().filter(|t| (2.0..=6.0).contains(&t.dia)).min_by(|a, b| (a.dia - 3.175).abs().total_cmp(&(b.dia - 3.175).abs())).or_else(|| flats().next())
}

/// The lettering tool: a V-bit or engraving bit when there is one, else the smallest flat cutter.
fn pick_engraver(tools: &[Tool]) -> Option<&Tool> {
    tools.iter().find(|t| t.shape == "engrave").or_else(|| tools.iter().find(|t| t.shape == "vbit"))
        .or_else(|| tools.iter().filter(|t| t.shape == "flat").min_by(|a, b| a.dia.total_cmp(&b.dia)))
}

/// Fills `d` (a new project that already has its machine and tools) with the sample. Sizes follow the machine's work area and
/// travel, rounded to round numbers in the display units. Returns an error when there is no cutter to use.
pub fn fill(d: &mut ProjectData, inches: bool) -> Result<(), String> {
    let (mx, my, mz) = d.machine.as_ref().map(|m| (m.x, m.y, m.z)).unwrap_or((300.0, 200.0, 60.0));
    let flat = pick_flat(&d.tools).ok_or("There is no flat end mill in the tool list.")?.clone();
    let engr = pick_engraver(&d.tools).unwrap_or(&flat).clone();
    // round numbers: whole 5 mm / quarter inches for sizes, whole mm / sixteenths for the thickness
    let (big, small) = if inches { (6.35, 1.5875) } else { (5.0, 1.0) };
    d.stock.w = floor_to(mx.min(200.0).min(mx * 0.9), big).max(big * 4.0);
    d.stock.h = floor_to(my.min(150.0).min(my * 0.9), big).max(big * 3.0);
    d.stock.t = floor_to((mz * 0.4).min(if inches { 12.7 } else { 12.0 }), small).max(small * 2.0);
    let w = floor_to((d.stock.w * 0.6).min(100.0), big).max(big * 2.0);
    let h = floor_to((w * 0.6).min(d.stock.h * 0.6), big).max(big);
    let (x0, y0) = ((d.stock.w - w) / 2.0, (d.stock.h - h) / 2.0);
    let r = (h * 0.2).min(10.0);
    let inset = (h * 0.15).max(flat.dia * 1.5);
    let u = |mm: f64| if inches { format!("{} in", (mm / 25.4 * 1000.0).round() / 1000.0) } else { format!("{} mm", (mm * 100.0).round() / 100.0) };
    d.notes = format!("SAMPLE NAME PLATE
This project was made for your machine and tools: a plate of {pw} x {ph} on a piece of stock {sw} x {sh} x {st}.

Operations
1. Profile (outside): cuts the plate out, all the way through, leaving 4 small holding tabs so it stays in place.
2. Pocket: a shallow recess in the plate.
3. Online: engraves \"PutiaCNC\" into the floor of the recess, along the lines of the letters.

Look around
- The 2D + 3D view shows the stock being cut as the preview plays; use the Play button at the bottom.
- Click an operation to see its shapes; change the depth, the pass depth or the tool and watch the toolpath follow.
- Change the lettering: press \"Edit text\" in the Text card of the left panel.

Before cutting for real
- Put a piece of scrap under the tool, set X0 Y0 at the lower-left corner and Z0 on the top of the stock.
- Tick \"Air cut (run without cutting)\" in each operation for a first run: the tool then runs above the stock and nothing is cut. Untick it to cut.
- Check the Estimated time and the warnings bar under the view tabs, then Export the G-code from the File menu.",
        pw = u(w), ph = u(h), sw = u(d.stock.w), sh = u(d.stock.h), st = u(d.stock.t));
    // the operations: cut-out with tabs, a shallow recess, the lettering
    d.ops.clear(); d.loops.clear(); d.groups.clear();
    let op = |d: &mut ProjectData, id: i64, kind: &str, depth: f64, tool: &Tool, tabs: bool| {
        let mut p = shapes::default_op_params(&tool.id);
        p.kind = kind.into(); p.depth = depth; p.tabs = tabs;
        if kind != "online" { p.pass_depth = Some(d.stock.t / 6.0); } // a short first run
        d.ops.push(Operation { id, p });
        d.oid = id; d.act = id;
    };
    op(d, 1, "outside", d.stock.t, &flat, true);
    let recess = (d.stock.t * 0.2).clamp(0.5, 2.0);
    op(d, 2, "pocket", recess, &flat, false);
    // the lettering sits on the floor of the recess, so its depth is measured from the top of the stock: the recess plus 1 mm
    op(d, 3, "online", (recess + 1.0).min(d.stock.t), &engr, false);
    d.loops.push(shapes::new_rect([x0, y0], [x0 + w, y0 + h], r, 1));
    let (px0, py0, px1, py1) = (x0 + inset, y0 + inset, x0 + w - inset, y0 + h - inset);
    if px1 - px0 > flat.dia * 2.0 && py1 - py0 > flat.dia * 2.0 { d.loops.push(shapes::new_rect([px0, py0], [px1, py1], 0.0, 2)); }
    // the lettering: as large as the recess allows, centred
    let text = "PutiaCNC";
    let probe = line_text_loops("futural", text, TextStyle::plain(10.0, 0.0, Align::Center), [0.0, 0.0], 3)?;
    if let Some(b) = shapes::bbox_many(probe.iter()) {
        let k = ((px1 - px0) * 0.8 / (b[2] - b[0]).max(1e-6)).min((py1 - py0) * 0.5 / (b[3] - b[1]).max(1e-6));
        let size = 10.0 * k;
        // a text group, like the Text tool makes: the letters set at the origin, then moved into place, so "Edit text" works on them
        let style = TextStyle::plain(size, size * 0.08, Align::Center);
        let mut letters = line_text_loops("futural", text, style, [0.0, 0.0], 3)?;
        let (cx, cy) = (x0 + w / 2.0, y0 + h / 2.0 - size / 2.0);
        let gid = d.groups.iter().map(|g| g.id).max().unwrap_or(0) + 1;
        for l in &mut letters { shapes::translate(l, cx, cy); l.grp = Some(gid); }
        let font = crate::linetext::fonts().into_iter().find(|f| f.0 == "futural").map(|f| f.1.to_string()).unwrap_or_default();
        let spec = TextSpec { content: text.into(), font, size, spacing: style.spacing_mm, word_spacing: 0.0, kerning: false, align: Align::Center, flow: style.flow, radius: 0.0, arc_deg: 90.0, arc_in: false, fit_width: 0.0, shadow: [0.0, 0.0], path: vec![] };
        d.groups.push(ShapeGroup { id: gid, name: format!("Text: {text}"), locked: false, text: Some(spec) });
        d.loops.extend(letters);
    }
    Ok(())
}

/// The oval sign (`examples/projects/oval-sign.project.json`): an oval plate with its name V-carved in the middle, a line along the top and one along
/// the bottom of the oval, an engraved border and four holding tabs. The lettering is stored as outlines, so it needs no fonts. The stock and the
/// drawing keep their size (about 260 x 170 mm), so the machine's work area has to take them; the tools are the user's own: a V-bit for the name, an
/// engraving bit (or the narrowest V-bit) for the small lines and the border, a flat end mill for the cut-out.
pub fn fill_oval(d: &mut ProjectData) -> Result<(), String> {
    let f = crate::load_project(include_str!("../examples/projects/oval-sign.project.json"))?;
    let src = f.data;
    let (mx, my, mz) = d.machine.as_ref().map(|m| (m.x, m.y, m.z)).unwrap_or((300.0, 200.0, 60.0));
    if mx < src.stock.w || my < src.stock.h { return Err(format!("The oval sign needs a work area of at least {:.0} x {:.0} mm.", src.stock.w, src.stock.h)); }
    let flat = pick_flat(&d.tools).ok_or("There is no flat end mill in the tool list.")?.clone();
    // the name: the V-bit nearest 60 degrees; the fine work: an engraving bit, else the sharpest V-bit
    let vbit = d.tools.iter().filter(|t| t.shape == "vbit").min_by(|a, b| (a.angle - 60.0).abs().total_cmp(&(b.angle - 60.0).abs())).cloned();
    let fine = d.tools.iter().find(|t| t.shape == "engrave").or_else(|| d.tools.iter().filter(|t| t.shape == "vbit").min_by(|a, b| a.angle.total_cmp(&b.angle))).cloned();
    let (vbit, fine) = match (vbit, fine) { (Some(v), Some(f)) => (v, f), (Some(v), None) => (v.clone(), v), (None, Some(f)) => (f.clone(), f), (None, None) => return Err("There is no V-bit or engraving bit in the tool list.".into()) };
    d.stock.w = src.stock.w; d.stock.h = src.stock.h;
    d.stock.t = src.stock.t.min((mz * 0.5).floor()).max(3.0);
    d.ops = src.ops; d.loops = src.loops; d.groups = src.groups;
    for o in d.ops.iter_mut() {
        let t = match o.id { 1 => &vbit, 4 => &flat, _ => &fine };
        o.p.tool = t.id.clone();
        if o.id == 4 { o.p.depth = d.stock.t; o.p.pass_depth = Some((d.stock.t / 5.0).max(0.5)); }
    }
    d.oid = 4; d.act = 4;
    d.notes = format!("SAMPLE OVAL SIGN
An oval plate on a piece of stock {w:.0} x {h:.0} x {t:.0} mm. The lettering is stored as outlines, so it looks the same on every computer.

Operations
1. V-carve: \"PutiaCNC\" in the middle, with the {v}.
2. V-carve: \"toolpath studio\" along the top of the oval and \"free open source software\" along the bottom, with the {f}.
3. On the line: a fine border engraved inside the edge of the oval, with the {f}.
4. Profile (outside): cuts the oval out with the {c}, leaving 4 small holding tabs.

Look around
- The 3D preview shows the stock being cut; use the Play button at the bottom.
- The two lines of small text follow the oval: select one and press \"Edit text\" in the Text card of the left panel to change the words (the font it was made with may be missing from your computer: then pick another).
- Change the depth of the V-carve and see how wide the letters become.

Before cutting for real
- Put a piece of scrap under the tool, set X0 Y0 at the lower-left corner and Z0 on the top of the stock.
- Tick \"Air cut (run without cutting)\" in each operation for a first run: the tool then runs above the stock and nothing is cut. Untick it to cut.
- Check the Estimated time and the warnings bar under the view tabs, then Export the G-code from the File menu.",
        w = d.stock.w, h = d.stock.h, t = d.stock.t, v = vbit.name, f = fine.name, c = flat.name);
    Ok(())
}

/// Which sample to make.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { NamePlate, Box, Relief, Oval }

impl Kind { pub fn title(self) -> &'static str { match self { Kind::NamePlate => "Sample name plate", Kind::Box => "Sample finger-joint box", Kind::Relief => "Sample 3D relief", Kind::Oval => "Sample oval sign" } } }

pub fn fill_kind(d: &mut ProjectData, inches: bool, kind: Kind) -> Result<(), String> {
    match kind { Kind::NamePlate => fill(d, inches), Kind::Box => fill_box(d, inches), Kind::Relief => fill_relief(d, inches), Kind::Oval => fill_oval(d) }
}

/// A six-petal rosette as a height map: the 3D relief sample needs no model file. Cells run from the bottom row up; 65535 is the top.
fn rosette(n: usize) -> Vec<u16> {
    let mut hm = vec![0u16; n * n];
    for j in 0..n { for i in 0..n {
        let (dx, dy) = ((i as f64 + 0.5) / n as f64 * 2.0 - 1.0, (j as f64 + 0.5) / n as f64 * 2.0 - 1.0);
        let (r, th) = (dx.hypot(dy), dy.atan2(dx));
        let edge = 0.62 + 0.34 * (3.0 * th).cos().abs();
        let t = r / edge;
        // a round petal, with a small dimple in the middle
        let mut h = if t < 1.0 { (1.0 - t * t).powf(1.2) } else { 0.0 };
        h *= 1.0 - 0.25 * (-(r / 0.12).powi(2)).exp();
        hm[j * n + i] = (h.clamp(0.0, 1.0) * 65535.0).round() as u16;
    } }
    // the highest cell holds 65535
    let top = hm.iter().copied().max().unwrap_or(1).max(1) as f64;
    for v in hm.iter_mut() { *v = (*v as f64 / top * 65535.0).round() as u16; }
    hm
}

/// A 3D relief: a six-petal rosette carved in the stock with a flat end mill for the roughing, a ball nose for the finishing and, when
/// there is a smaller ball nose, a detail pass. The model is made here, so nothing has to be imported.
pub fn fill_relief(d: &mut ProjectData, inches: bool) -> Result<(), String> {
    let (mx, my, mz) = d.machine.as_ref().map(|m| (m.x, m.y, m.z)).unwrap_or((300.0, 200.0, 60.0));
    // the roughing cutter: the widest flat end mill up to 6.5 mm (the roughing is quicker with a wide one), else any flat one
    let flat = d.tools.iter().filter(|t| t.shape == "flat" && t.dia <= 6.5).max_by(|a, b| a.dia.total_cmp(&b.dia)).or_else(|| pick_flat(&d.tools)).ok_or("There is no flat end mill in the tool list.")?.clone();
    // a project without a ball nose gets the two of the starter set (1/8 and 1/4 inch), so the sample can run
    let mut added = false;
    if !d.tools.iter().any(|t| t.shape == "ball" && t.dia <= 7.0) {
        for t in crate::setup::starter_tools(crate::setup::ToolChoice { ball: true, ..Default::default() }).into_iter().filter(|t| t.shape == "ball") { d.tools.push(t); added = true; }
    }
    let mut balls: Vec<&Tool> = d.tools.iter().filter(|t| t.shape == "ball" && t.dia <= 7.0).collect();
    balls.sort_by(|a, b| b.dia.total_cmp(&a.dia));
    let ball = (*balls.first().ok_or("There is no ball nose in the tool list.")?).clone();
    let detail = balls.iter().find(|t| t.dia < ball.dia - 1e-6 && t.dia >= 0.5).map(|t| (*t).clone());
    let (big, small) = if inches { (6.35, 1.5875) } else { (5.0, 1.0) };
    d.stock.w = floor_to(mx.min(90.0).min(mx * 0.9), big).max(big * 6.0);
    d.stock.h = floor_to(my.min(90.0).min(my * 0.9), big).max(big * 6.0);
    d.stock.t = floor_to((mz * 0.4).min(if inches { 12.7 } else { 12.0 }), small).max(small * 4.0);
    let s = floor_to((d.stock.w.min(d.stock.h) * 0.8).min(60.0), big).max(big * 3.0);
    let n = 160usize;
    let zmax = floor_to((d.stock.t * 0.5).clamp(small * 2.0, if inches { 6.35 } else { 6.0 }), small).max(small);
    let mut bytes = Vec::with_capacity(n * n * 2);
    for v in rosette(n) { bytes.extend(v.to_le_bytes()); }
    d.relief = Some(crate::model::Relief { name: "Rosette".into(), nx: n, ny: n, res: s / n as f64, zmax, hm: crate::relief::b64_encode(&bytes), x: (d.stock.w - s) / 2.0, y: (d.stock.h - s) / 2.0, scale: 1.0 });
    let u = |mm: f64| if inches { format!("{} in", (mm / 25.4 * 1000.0).round() / 1000.0) } else { format!("{} mm", (mm * 100.0).round() / 100.0) };
    d.ops.clear(); d.loops.clear(); d.groups.clear();
    let mut p = shapes::default_op_params(&ball.id);
    p.kind = "relief".into();
    p.depth = zmax; p.d_man = true;
    p.pass_depth = Some((zmax / 4.0).max(0.5));
    p.f_so = Some(20.0); p.d_so = Some(35.0); // a sample favours speed over a perfect surface
    p.tool2 = Some(flat.id.clone());
    p.rest = detail.as_ref().map(|t| t.id.clone());
    d.ops.push(Operation { id: 1, p });
    d.oid = 1; d.act = 1;
    let steps = match &detail {
        Some(t) => format!("3. Detail: the {} finishes only the places the {} could not reach (the dimple in the middle and the corners where the petals meet the floor).\n", t.name, ball.name),
        None => "3. Detail: add a smaller ball nose in the Detail row of the Finishing step to finish the corners the big one cannot reach.\n".into(),
    };
    d.notes = format!("SAMPLE 3D RELIEF
A six-petal rosette, {s} square and {h} high, carved into a piece of stock {sw} x {sh} x {st}. The model is made by the program, so nothing has to be imported: your own STL or OBJ files work the same way (File menu, Import 3D model).

Operations
1. Roughing: the {flat} takes most of the material away in layers, leaving a little for the finishing.
2. Finishing: the {ball} follows the surface line by line.
{steps}
Look around
- The 3D preview shows the stock being cut as the preview plays; use the Play button at the bottom.
- In the Finishing step, \"Lines\" chooses the direction of the finishing lines (along X, along Y, or both) and \"Stepover\" how far apart they are.
- Switch the Detail tool off with Remove, or Roughing off, and watch the toolpath and the time change.
- Move or resize the model in the Imported card of the left panel.

Before cutting for real
{added}- Put a piece of scrap under the tool, set X0 Y0 at the lower-left corner and Z0 on the top of the stock.
- Tick \"Air cut (run without cutting)\" in the operation for a first run: the tool then runs above the stock and nothing is cut. Untick it to cut.
- Check the Estimated time and the warnings bar under the view tabs, then Export the G-code from the File menu.",
        s = u(s), h = u(zmax), sw = u(d.stock.w), sh = u(d.stock.h), st = u(d.stock.t), flat = flat.name, ball = ball.name, steps = steps,
        added = if added { "- The ball nose bits of this sample (1/8 and 1/4 inch) were not in your tool list, so they were added to this project: check their feeds and speeds against your own bits.\n" } else { "" });
    Ok(())
}

/// A small open box with finger joints: its five plates laid out on the stock, one outside cut through all of them with holding tabs,
/// and dogbone reliefs in the finger notches for the round cutter. The box is made as large as the stock takes it (up to 80 x 60 x 40 mm),
/// the plates are cut from the stock's own thickness and the fingers are twice as wide.
pub fn fill_box(d: &mut ProjectData, inches: bool) -> Result<(), String> {
    use crate::joints::{box_panels, layout, reliefs, BoxSpec, Cut, Relief};
    let (mx, my, mz) = d.machine.as_ref().map(|m| (m.x, m.y, m.z)).unwrap_or((300.0, 200.0, 60.0));
    let flat = pick_flat(&d.tools).ok_or("There is no flat end mill in the tool list.")?.clone();
    let (big, small) = if inches { (6.35, 1.5875) } else { (5.0, 1.0) };
    d.stock.w = floor_to(mx.min(200.0).min(mx * 0.9), big).max(big * 4.0);
    d.stock.h = floor_to(my.min(150.0).min(my * 0.9), big).max(big * 3.0);
    // the plates are cut right through: 6 mm (1/4 in) material, or thinner on a machine with little travel
    d.stock.t = floor_to((mz * 0.4).min(if inches { 6.35 } else { 6.0 }), small).max(small * 2.0);
    let t = d.stock.t;
    let margin = 3.0_f64.max(flat.dia);
    let gap = 6.0_f64.max(flat.dia * 2.0);
    // the largest box whose plates, laid out in rows, fit on the stock
    let (mut w, mut dd, mut h) = (80.0, 60.0, 40.0);
    let fit = loop {
        let spec = BoxSpec { w, d: dd, h, t, finger: t * 2.0, clearance: 0.05, lid: false, div_w: 0, div_d: 0 };
        let made = box_panels(spec).map(|panels| { let pos = layout(&panels, gap, d.stock.w - 2.0 * margin); (panels, pos) });
        if let Ok((panels, pos)) = made {
            let top = panels.iter().zip(&pos).map(|(p, o)| o[1] + p.h).fold(0.0, f64::max);
            let right = panels.iter().zip(&pos).map(|(p, o)| o[0] + p.w).fold(0.0, f64::max);
            if top <= d.stock.h - 2.0 * margin && right <= d.stock.w - 2.0 * margin { break (spec, panels, pos); }
        }
        // too big for the stock (or too small for the fingers): a little smaller, down to the smallest box that still has fingers
        if w <= t * 5.0 || dd <= t * 5.0 || h <= t * 3.0 { return Err("The stock is too small for the sample box.".into()); }
        w = (w * 0.9).floor(); dd = (dd * 0.9).floor(); h = (h * 0.9).floor();
    };
    let (spec, panels, pos) = fit;
    d.ops.clear(); d.loops.clear(); d.groups.clear();
    let mut p = shapes::default_op_params(&flat.id);
    p.kind = "outside".into(); p.depth = t; p.tabs = true;
    d.ops.push(Operation { id: 1, p });
    d.oid = 1; d.act = 1;
    for (pn, o) in panels.iter().zip(&pos) {
        let at = |q: &[f64; 2]| [q[0] + o[0] + margin, q[1] + o[1] + margin];
        let mut loops = Vec::new();
        if let Some(l) = shapes::new_poly(&pn.outline.iter().map(at).collect::<Vec<_>>(), true, 1) { loops.push(l); }
        for hole in &pn.holes { if let Some(l) = shapes::new_poly(&hole.iter().map(at).collect::<Vec<_>>(), true, 1) { loops.push(l); } }
        let refs: Vec<&crate::model::LoopData> = loops.iter().collect();
        match reliefs(&refs, flat.dia, Cut::Outside, 1, Relief::Dogbone) {
            Some((ls, _)) => d.loops.extend(ls),
            None => d.loops.extend(loops),
        }
    }
    let u = |mm: f64| if inches { format!("{} in", (mm / 25.4 * 1000.0).round() / 1000.0) } else { format!("{} mm", (mm * 100.0).round() / 100.0) };
    d.notes = format!("SAMPLE FINGER-JOINT BOX
This project was made for your machine and tools: an open box {w} x {dd} x {h} outside, made of {t} material, cut from one piece of stock {sw} x {sh}.

What is on the stock
- The five plates of the box (bottom, front, back, left, right), laid out in rows.
- One operation, Profile (outside), cuts all the plates out, all the way through, leaving 4 small holding tabs on each so they stay in place.
- The notches of the fingers have small round reliefs (dogbones) in the corners, so a round cutter leaves room for the square corners of the fingers.

Look around
- The 3D preview shows the plates being cut as the preview plays.
- Open Edit, then \"Box with finger joints…\" to see the settings of this box. In the box window you can change the size, the finger width, the clearance, add a lid or dividers, and the plates follow.
- Draw a rectangle, select it and open Edit, then \"Finger joint…\" to put fingers on any other edge.

Before cutting for real
- The plates are cut from the thickness of the stock: measure your material, set the Thickness field in the Stock panel to it, and make the box again so the fingers fit. A fit that is too tight can be eased with Clearance.
- Put a piece of scrap under the tool, set X0 Y0 at the lower-left corner and Z0 on the top of the stock.
- Tick \"Air cut (run without cutting)\" in the operation for a first run: the tool then runs above the stock and nothing is cut. Untick it to cut.
- After cutting, break the plates off their tabs, sand the tab marks and glue the box together.",
        w = u(spec.w), dd = u(spec.d), h = u(spec.h), t = u(t), sw = u(d.stock.w), sh = u(d.stock.h));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generate, Units};

    fn project(x: f64, y: f64, z: f64, inch: bool) -> ProjectData {
        let mut f = shapes::blank_project();
        let mut m = crate::model::Machine::default();
        m.x = x; m.y = y; m.z = z;
        f.data.machine = Some(m);
        f.data.tools = crate::setup::starter_tools(crate::setup::ToolChoice { inch_sizes: inch, ..Default::default() });
        f.data.job.safe = 5.0;
        f.data
    }

    /// The sample fits whatever the machine, stays inside the Z travel, has notes and makes G-code.
    #[test]
    fn fits_every_machine() {
        for (x, y, z, inch) in [(180.0, 100.0, 45.0, false), (300.0, 200.0, 60.0, false), (800.0, 1200.0, 150.0, false), (300.0, 200.0, 60.0, true), (400.0, 400.0, 80.0, true)] {
            let mut d = project(x, y, z, inch);
            fill(&mut d, inch).unwrap();
            assert!(d.stock.w <= x && d.stock.h <= y && d.stock.t <= z * 0.4 + 1e-9, "{x}x{y}x{z}: stock {}x{}x{}", d.stock.w, d.stock.h, d.stock.t);
            let b = shapes::bbox_many(d.loops.iter()).unwrap();
            assert!(b[0] >= 0.0 && b[1] >= 0.0 && b[2] <= d.stock.w && b[3] <= d.stock.h, "{x}x{y}: the shapes leave the stock {b:?}");
            assert!(d.ops.len() == 3 && d.ops.iter().all(|o| !o.p.air), "a normal cutting project");
            // the lettering is one text group that the Text tool can edit again
            let g = d.groups.iter().find(|g| g.text.is_some()).expect("a text group");
            let letters: Vec<&crate::model::LoopData> = d.loops.iter().filter(|l| l.grp == Some(g.id)).collect();
            assert!(letters.len() >= 8 && letters.iter().all(|l| l.op == 3));
            let src = line_text_loops("futural", &g.text.as_ref().unwrap().content, TextStyle::plain(g.text.as_ref().unwrap().size, g.text.as_ref().unwrap().spacing, Align::Center), [0.0, 0.0], 3).unwrap();
            let fit = crate::text::fit_affine(&src, &letters);
            assert!(fit.is_some_and(|(_, e)| e < 1e-6), "the letters no longer match the text settings");
            assert!(d.notes.contains("Air cut") && d.notes.contains(if inch { "in" } else { "mm" }));
            assert!(d.ops.iter().all(|o| o.p.depth <= d.stock.t + 1e-9));
            assert!(d.ops[2].p.depth > d.ops[1].p.depth, "the lettering must go below the floor of the recess");
            let o = generate(&d, if inch { Units::Imperial } else { Units::Metric }).unwrap_or_else(|e| panic!("{x}x{y}x{z}: {e}"));
            assert!(o.lines.len() > 50);
            assert!(!o.notes.iter().any(|n| n.starts_with('⚠')), "{x}x{y}x{z}: {:?}", o.notes);
            if inch { // round inch sizes
                let q = d.stock.w / 6.35; assert!((q - q.round()).abs() < 1e-6, "stock width {} is not a whole quarter inch", d.stock.w);
            }
        }
    }

    /// Ticking "Air cut" (on all operations, or on any one of them) never lets the tool touch the stock: no cutting move goes below 2 mm
    /// above the top, whatever the depths (the lettering is deeper than the recess it sits in), and the program still has all its moves.
    #[test]
    fn air_cut_never_touches_the_stock() {
        for inch in [false, true] {
            let mut base = project(300.0, 200.0, 60.0, inch);
            fill(&mut base, inch).unwrap();
            let units = if inch { Units::Imperial } else { Units::Metric };
            let f = if inch { 25.4 } else { 1.0 };
            let normal = generate(&base, units).unwrap();
            for mask in 1u8..8 {
                let mut d = base.clone();
                for (k, o) in d.ops.iter_mut().enumerate() { o.p.air = mask & (1 << k) != 0; }
                let o = generate(&d, units).unwrap_or_else(|e| panic!("air mask {mask}: {e}"));
                assert!(o.lines.iter().any(|l| l.starts_with("G1")));
                // the cutting moves of the air operations stay above the stock; the others still cut
                let zs = |lines: &[String]| lines.iter().filter(|l| l.starts_with("G1") || l.starts_with("G2") || l.starts_with("G3")).flat_map(|l| l.split_whitespace().filter_map(|w| w.strip_prefix('Z').and_then(|v| v.parse::<f64>().ok()))).map(|z| z * f).collect::<Vec<_>>();
                let low = zs(&o.lines).into_iter().fold(f64::MAX, f64::min);
                if mask == 7 { assert!(low >= 1.99, "all air: the tool goes down to Z {low}"); }
                assert!(low >= -d.stock.t - 1e-6, "mask {mask}: below the bottom of the stock ({low})");
                assert!(o.lines.len() >= normal.lines.len() - 5, "mask {mask}: the program lost moves");
            }
        }
    }

    /// The box sample fits the stock of every machine, is one profile operation over all the plates, and makes G-code.
    #[test]
    fn box_sample_fits_every_machine() {
        for (x, y, z, inch) in [(180.0, 100.0, 45.0, false), (300.0, 200.0, 60.0, false), (800.0, 1200.0, 150.0, false), (300.0, 200.0, 60.0, true), (400.0, 400.0, 80.0, true)] {
            let mut d = project(x, y, z, inch);
            fill_box(&mut d, inch).unwrap_or_else(|e| panic!("{x}x{y}: {e}"));
            let b = shapes::bbox_many(d.loops.iter()).unwrap();
            assert!(b[0] >= 0.0 && b[1] >= 0.0 && b[2] <= d.stock.w && b[3] <= d.stock.h, "{x}x{y}: the plates leave the stock {b:?} on {}x{}", d.stock.w, d.stock.h);
            assert!(d.loops.len() >= 5 && d.loops.iter().all(|l| l.op == 1) && d.ops.len() == 1 && d.ops[0].p.tabs && d.ops[0].p.depth == d.stock.t);
            assert!(d.stock.t <= z * 0.4 + 1e-9);
            assert!(d.notes.contains("Box with finger joints"));
            let o = generate(&d, if inch { Units::Imperial } else { Units::Metric }).unwrap_or_else(|e| panic!("{x}x{y}: {e}"));
            assert!(o.lines.len() > 100);
            assert!(!o.notes.iter().any(|n| n.starts_with('⚠')), "{x}x{y}: {:?}", o.notes);
        }
    }

    /// The relief sample fits every machine, is one relief operation with roughing, finishing and a detail ball, and makes G-code.
    #[test]
    fn relief_sample_fits_every_machine() {
        for (x, y, z, inch) in [(180.0, 100.0, 45.0, false), (300.0, 200.0, 60.0, false), (800.0, 1200.0, 150.0, false), (300.0, 200.0, 60.0, true), (400.0, 400.0, 80.0, true)] {
            let mut d = project(x, y, z, inch);
            fill_relief(&mut d, inch).unwrap_or_else(|e| panic!("{x}x{y}: {e}"));
            let rel = d.relief.as_ref().unwrap();
            let (w, h) = rel.size();
            assert!(rel.x >= 0.0 && rel.y >= 0.0 && rel.x + w <= d.stock.w + 1e-9 && rel.y + h <= d.stock.h + 1e-9, "{x}x{y}: the model leaves the stock");
            assert!(d.ops.len() == 1 && d.ops[0].p.kind == "relief" && d.ops[0].p.tool2.is_some() && d.ops[0].p.rest.is_some());
            assert!(d.ops[0].p.depth <= d.stock.t + 1e-9 && d.stock.t <= z * 0.4 + 1e-9);
            assert!(d.notes.contains("SAMPLE 3D RELIEF") && d.notes.contains("Detail"));
            let o = generate(&d, if inch { Units::Imperial } else { Units::Metric }).unwrap_or_else(|e| panic!("{x}x{y}: {e}"));
            assert!(o.moves.iter().any(|m| m.alt && m.cut) && o.moves.iter().any(|m| m.det && m.cut), "{x}x{y}: roughing and detail both cut");
            assert!(o.time_min < 30.0, "{x}x{y}x{z}: a sample should not take {:.0} minutes", o.time_min);
            assert!(!o.notes.iter().any(|n| n.starts_with('⚠')), "{x}x{y}: {:?}", o.notes);
        }
        // without a smaller ball nose there is no detail tool, and the notes say how to add one
        let mut d = project(300.0, 200.0, 60.0, false);
        d.tools.extend(crate::setup::starter_tools(crate::setup::ToolChoice { ball: true, ..Default::default() }).into_iter().filter(|t| t.shape == "ball"));
        let big = d.tools.iter().filter(|t| t.shape == "ball").map(|t| t.dia).fold(0.0, f64::max);
        d.tools.retain(|t| t.shape != "ball" || t.dia >= big);
        fill_relief(&mut d, false).unwrap();
        assert!(d.ops[0].p.rest.is_none() && d.notes.contains("add a smaller ball nose"));
        // no ball nose at all: the starter ones are added to the project, and the notes say so
        d.tools.retain(|t| t.shape != "ball");
        fill_relief(&mut d, false).unwrap();
        assert!(d.tools.iter().any(|t| t.shape == "ball") && d.notes.contains("were not in your tool list"));
    }

    #[test]
    fn the_oval_sign_fits_a_big_enough_machine_and_cuts() {
        let mut d = project(300.0, 200.0, 60.0, false);
        fill_oval(&mut d).unwrap();
        assert_eq!(d.ops.len(), 4);
        for o in &d.ops { assert!(d.tools.iter().any(|t| t.id == o.p.tool), "op {} has a tool of the project", o.id); }
        let v = d.tools.iter().find(|t| t.id == d.ops[0].p.tool).unwrap();
        assert_eq!(v.shape, "vbit");
        assert!(d.ops[3].p.tabs && d.ops[3].p.depth <= d.stock.t + 1e-9 && d.notes.contains("SAMPLE OVAL SIGN"));
        let o = generate(&d, Units::Metric).unwrap();
        assert!(o.moves.iter().any(|m| m.cut));
        assert!(!o.notes.iter().any(|n| n.starts_with('⚠')), "{:?}", o.notes);
        // a work area smaller than the sign is turned down, with its size in the message
        let mut small = project(180.0, 100.0, 45.0, false);
        assert!(fill_oval(&mut small).unwrap_err().contains("260"));
        // with no V-bit or engraving bit there is nothing to carve the name with
        let mut flat_only = project(300.0, 200.0, 60.0, false);
        flat_only.tools.retain(|t| t.shape == "flat");
        assert!(fill_oval(&mut flat_only).is_err());
    }

    /// A warning note starts with the warning sign (the program's panels tell warnings from information by it), also with the operation's name.
    #[test]
    fn warning_notes_start_with_the_sign() {
        let mut d = project(300.0, 200.0, 60.0, false);
        fill(&mut d, false).unwrap();
        d.machine.as_mut().unwrap().rpm_max = Some(8000.0);
        d.ops[0].p.custom = Some(crate::model::CustomFeeds { rpm: Some(20000.0), feed: None, plunge: None, so: None });
        let o = generate(&d, Units::Metric).unwrap();
        let held: Vec<&String> = o.notes.iter().filter(|n| n.contains("Custom feeds and speeds")).collect();
        assert!(!held.is_empty(), "{:?}", o.notes);
        for n in o.notes.iter().filter(|n| n.contains('⚠')) { assert!(n.starts_with("⚠ "), "{n}"); }
    }

    #[test]
    fn no_cutter_is_an_error() {
        let mut d = project(300.0, 200.0, 60.0, false);
        d.tools.clear();
        assert!(fill(&mut d, false).is_err());
    }

    /// A wide roughing cutter must leave a straight wall at the ends of its rows, not the scallops of its round end between two rows.
    #[test]
    fn roughing_with_a_wide_cutter_leaves_straight_side_walls() {
        let mut d = project(300.0, 200.0, 60.0, false);
        fill_relief(&mut d, false).unwrap();
        let wide = d.tools.iter().find(|t| t.name == "8 mm flat end mill").unwrap().id.clone();
        d.ops[0].p.tool2 = Some(wide);
        d.ops[0].p.f_off = true;
        d.ops[0].p.rest = None;
        let out = generate(&d, Units::Metric).unwrap();
        let mut sim = crate::sim::Sim::new(&d, 400_000);
        sim.advance(&out.moves, out.moves.len());
        // the left wall of the cleared area, along the middle of the model
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for iy in 0..sim.ny {
            let y = iy as f64 * sim.cell;
            if (y - d.stock.h / 2.0).abs() > 12.0 { continue; }
            let Some(ix) = (0..sim.nx).find(|&ix| sim.at(ix, iy) < -1.0) else { continue };
            let x = ix as f64 * sim.cell;
            lo = lo.min(x); hi = hi.max(x);
            // and the cutter has reached the full depth right beside it, wherever the model's rim rises
            assert!(sim.at(ix + 7, iy) < -5.5, "at y {y:.1} the strip beside the wall is only {:.2} deep", sim.at(ix + 7, iy));
        }
        assert!(hi - lo < 0.1 + sim.cell, "the wall wanders by {:.2} mm", hi - lo);
    }
}
