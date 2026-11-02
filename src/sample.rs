//! The sample project ("Try a sample"): a small name plate made to fit the user's own machine, tools and units.
//! The Notes tab explains it and how to try it as an air cut first.
use crate::linetext::line_text_loops;
use crate::model::{Operation, ProjectData, Tool};
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
3. Online: engraves \"PutiaCNC\" along the lines of the letters.

Look around
- The 2D + 3D view shows the stock being cut as the preview plays; use the Play button at the bottom.
- Click an operation to see its shapes; change the depth, the pass depth or the tool and watch the toolpath follow.
- Edit the text with the Text tool (the T in the toolbar).

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
    op(d, 2, "pocket", (d.stock.t * 0.2).min(2.0).max(0.5), &flat, false);
    op(d, 3, "online", 0.6_f64.min(d.stock.t * 0.2), &engr, false);
    d.loops.push(shapes::new_rect([x0, y0], [x0 + w, y0 + h], r, 1));
    let (px0, py0, px1, py1) = (x0 + inset, y0 + inset, x0 + w - inset, y0 + h - inset);
    if px1 - px0 > flat.dia * 2.0 && py1 - py0 > flat.dia * 2.0 { d.loops.push(shapes::new_rect([px0, py0], [px1, py1], 0.0, 2)); }
    // the lettering: as large as the recess allows, centred
    let text = "PutiaCNC";
    let probe = line_text_loops("futural", text, TextStyle::plain(10.0, 0.0, Align::Center), [0.0, 0.0], 3)?;
    if let Some(b) = shapes::bbox_many(probe.iter()) {
        let k = ((px1 - px0) * 0.8 / (b[2] - b[0]).max(1e-6)).min((py1 - py0) * 0.5 / (b[3] - b[1]).max(1e-6));
        let size = 10.0 * k;
        let (cx, cy) = (x0 + w / 2.0, y0 + h / 2.0 - size / 2.0);
        d.loops.extend(line_text_loops("futural", text, TextStyle::plain(size, size * 0.08, Align::Center), [cx, cy], 3)?);
    }
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
            assert!(d.notes.contains("Air cut") && d.notes.contains(if inch { "in" } else { "mm" }));
            assert!(d.ops.iter().all(|o| o.p.depth <= d.stock.t + 1e-9));
            let o = generate(&d, if inch { Units::Imperial } else { Units::Metric }).unwrap_or_else(|e| panic!("{x}x{y}x{z}: {e}"));
            assert!(o.lines.len() > 50);
            if inch { // round inch sizes
                let q = d.stock.w / 6.35; assert!((q - q.round()).abs() < 1e-6, "stock width {} is not a whole quarter inch", d.stock.w);
            }
        }
    }

    #[test]
    fn no_cutter_is_an_error() {
        let mut d = project(300.0, 200.0, 60.0, false);
        d.tools.clear();
        assert!(fill(&mut d, false).is_err());
    }
}
