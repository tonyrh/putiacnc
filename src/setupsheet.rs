//! The setup sheet: one printable page for the person standing at the machine. The stock and where the work zero goes (with a picture),
//! the tools in the order they are used, the operations with their depths and times, what to do before starting and what to watch for.
//! It is a self-contained HTML page (no outside files) laid out for A4 or Letter paper: open it in a browser and print it or save it as PDF.
use crate::model::{ProjectData, Tool};
use crate::pieces::PartOutput;
use crate::shapes;
use crate::summary::{describe, duration};
use crate::toolpath::Timing;
use crate::Units;

/// What the sheet needs besides the project: the program's version. The date is the browser's own, in the person's language and time zone.
pub struct SheetInfo { pub version: String }

const COLORS: [&str; 8] = ["#2b6cd4", "#e07b1a", "#2a9d5c", "#b8438f", "#7a5cc7", "#c4a000", "#d1453b", "#168b96"];

fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;") }

fn kind_name(k: &str) -> &str {
    match k { "outside" => "Profile outside", "inside" => "Profile inside", "online" => "On line", "pocket" => "Pocket", "vcarve" => "V-carve", "drill" => "Drill",
        "inlay" => "Inlay pocket", "plug" => "Inlay plug", "relief" => "3D relief", "draw" => "Draw (pen)", other => other }
}

fn shape_name(s: &str) -> &str { crate::toollib::SHAPES.iter().find(|(k, _)| *k == s).map_or(s, |(_, n)| *n) }

fn zero_name(zr: &str) -> &'static str {
    match zr { "lr" => "lower-right corner", "ul" => "upper-left corner", "ur" => "upper-right corner", "c" => "center", _ => "lower-left corner" }
}

/// A length in the display units: "12 mm" or "0.472 in".
fn len(mm: f64, units: Units) -> String {
    match units {
        Units::Metric => format!("{} mm", trim(mm, 2)),
        Units::Imperial => format!("{} in", trim(mm / 25.4, 3)),
    }
}

/// A feed in the display units: "800 mm/min" or "31.5 in/min".
fn feed(mm_min: f64, units: Units) -> String {
    match units { Units::Metric => format!("{} mm/min", trim(mm_min, 0)), Units::Imperial => format!("{} in/min", trim(mm_min / 25.4, 1)) }
}

fn trim(v: f64, dec: usize) -> String {
    let s = format!("{:.*}", dec, v);
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// The tools in the order the program uses them: (tool, the operations (1-based) that use it).
fn tools_in_order(data: &ProjectData) -> Vec<(&Tool, Vec<usize>)> {
    let mut out: Vec<(&Tool, Vec<usize>)> = Vec::new();
    let mut add = |id: &str, k: usize| {
        let Some(t) = data.tools.iter().find(|t| t.id == id) else { return };
        match out.iter_mut().find(|(x, _)| x.id == t.id) {
            Some((_, v)) => { if !v.contains(&k) { v.push(k); } }
            None => out.push((t, vec![k])),
        }
    };
    for (i, o) in data.ops.iter().enumerate() {
        if !data.loops.iter().any(|l| l.op == o.id) && o.p.kind != "relief" { continue; }
        if o.p.kind == "relief" {
            if !o.p.r_off { if let Some(t) = &o.p.tool2 { add(t, i + 1); } }
            if !o.p.f_off { add(&o.p.tool, i + 1); }
        } else {
            add(&o.p.tool, i + 1);
            if matches!(o.p.kind.as_str(), "inlay" | "plug") { if let Some(t) = &o.p.tool2 { add(t, i + 1); } }
            if o.p.kind == "pocket" { if let Some(t) = &o.p.rest { add(t, i + 1); } }
        }
    }
    out
}

/// The picture of the stock: its outline, the shapes in the colour of their operation, and the work zero.
fn diagram(data: &ProjectData) -> String {
    let st = &data.stock;
    let (w, h) = (st.w.max(1.0), st.h.max(1.0));
    let pad = w.max(h) * 0.06;
    let mut s = format!("<svg class=\"pic\" viewBox=\"{} {} {} {}\" xmlns=\"http://www.w3.org/2000/svg\">", -pad, -pad, w + 2.0 * pad, h + 2.0 * pad);
    s += &format!("<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"#f4ead8\" stroke=\"#8a6d3b\" stroke-width=\"{}\"/>", w.max(h) * 0.004);
    let sw = w.max(h) * 0.006;
    for l in &data.loops {
        let pts = shapes::points(l);
        if pts.len() < 2 { continue; }
        let color = data.ops.iter().position(|o| o.id == l.op).map_or("#888", |i| COLORS[i % COLORS.len()]);
        let path: Vec<String> = pts.iter().map(|p| format!("{},{}", trim(p[0], 2), trim(h - p[1], 2))).collect();
        let tag = if l.closed() { "polygon" } else { "polyline" };
        s += &format!("<{tag} points=\"{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{sw}\" stroke-linejoin=\"round\"/>", path.join(" "));
    }
    // the work zero: a cross with a ring, labelled
    let (zx, zy) = match st.zr.as_str() { "lr" | "ur" => (w, 0.0), "c" => (w / 2.0, 0.0), _ => (0.0, 0.0) };
    let zy = match st.zr.as_str() { "ul" | "ur" => h, "c" => h / 2.0, _ => zy };
    let (cx, cy) = (zx, h - zy);
    let r = w.max(h) * 0.025;
    s += &format!("<g stroke=\"#d1453b\" stroke-width=\"{sw}\" fill=\"none\"><circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\"/><line x1=\"{}\" y1=\"{cy}\" x2=\"{}\" y2=\"{cy}\"/><line x1=\"{cx}\" y1=\"{}\" x2=\"{cx}\" y2=\"{}\"/></g>", cx - r * 1.6, cx + r * 1.6, cy - r * 1.6, cy + r * 1.6);
    let (tx, anchor) = if cx > w * 0.6 { (cx - r * 2.0, "end") } else { (cx + r * 2.0, "start") };
    let ty = if cy > h * 0.5 { cy - r * 1.4 } else { cy + r * 2.6 };
    s += &format!("<text x=\"{tx}\" y=\"{ty}\" text-anchor=\"{anchor}\" font-size=\"{}\" fill=\"#d1453b\" font-family=\"sans-serif\" font-weight=\"bold\">X0 Y0</text>", w.max(h) * 0.04);
    s + "</svg>"
}

fn z_zero(st: &crate::model::Stock, units: Units) -> String {
    if st.zm == "h" { format!("{} above the bottom of the stock", len(st.zh, units)) } else { "the top surface of the stock".into() }
}

/// The setup sheet of a job: `parts` are its programs (the main one first, then the separate plug pieces) as `generate_parts` makes them.
pub fn html(data: &ProjectData, parts: &[PartOutput], units: Units, info: &SheetInfo, project_name: &str) -> String {
    let main = &parts[0].out;
    let n = data.ops.len();
    let mut timing: Timing = main.timing(n);
    let mut total_min = main.time_min;
    let mut cut_min = main.cut_time_min;
    let mut cut_len = main.cut_len_mm;
    for p in &parts[1..] {
        let t = p.out.timing(n);
        if t.ops.len() > timing.ops.len() { timing.ops.resize(t.ops.len(), Default::default()); }
        for (a, b) in timing.ops.iter_mut().zip(&t.ops) { a.cut += b.cut; a.travel += b.travel; }
        total_min += p.out.time_min; cut_min += p.out.cut_time_min; cut_len += p.out.cut_len_mm;
    }
    let rows = describe(data, main, units, false);
    let st = &data.stock;
    let machine = data.machine.as_ref();
    let name = if project_name.trim().is_empty() { "Untitled" } else { project_name.trim() };

    let mut h = String::new();
    h += "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">";
    h += &format!("<title>Setup sheet - {}</title>", esc(name));
    h += "<style>
:root{color-scheme:light}
*{box-sizing:border-box}
body{font:13px/1.45 -apple-system,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;color:#1c1c1c;background:#fff;margin:0;padding:18mm 16mm}
h1{font-size:22px;margin:0 0 2px}h2{font-size:13px;text-transform:uppercase;letter-spacing:.06em;color:#555;border-bottom:1.5px solid #333;padding-bottom:3px;margin:20px 0 8px}
.sub{color:#666;font-size:12px}
.top{display:grid;grid-template-columns:1fr 1fr;gap:18px;align-items:start}
dl{display:grid;grid-template-columns:auto 1fr;gap:3px 14px;margin:0}dt{color:#666}dd{margin:0;font-weight:600}
.pic{width:100%;height:auto;max-height:210px;border:1px solid #ddd;border-radius:6px;background:#fff}
table{width:100%;border-collapse:collapse}th{text-align:left;font-size:11px;text-transform:uppercase;letter-spacing:.05em;color:#666;border-bottom:1px solid #999;padding:4px 6px}
td{padding:5px 6px;border-bottom:1px solid #e3e3e3;vertical-align:top}td:first-child{white-space:nowrap}td.n,th.n{text-align:right;white-space:nowrap}
.dot{display:inline-block;width:10px;height:10px;border-radius:50%;margin-right:6px;vertical-align:-1px}
ul.check{list-style:none;margin:0;padding:0}ul.check li{padding:3px 0 3px 26px;position:relative}ul.check li:before{content:'';position:absolute;left:0;top:5px;width:13px;height:13px;border:1.5px solid #333;border-radius:3px}
.warn{color:#a4161a;font-weight:600}.notes{white-space:pre-wrap;background:#fafafa;border:1px solid #e3e3e3;border-radius:6px;padding:8px 10px}
.foot{margin-top:22px;color:#888;font-size:11px;border-top:1px solid #ddd;padding-top:6px}
.piece{page-break-inside:avoid}
@media print{body{padding:0}@page{margin:14mm}h2{page-break-after:avoid}tr,li{page-break-inside:avoid}}
</style></head><body>";
    h += &format!("<h1>{}</h1><div class=\"sub\">Setup sheet &middot; <span id=\"d\"></span> &middot; PutiaCNC {}</div><script>try{{document.getElementById('d').textContent=new Date().toLocaleDateString(undefined,{{dateStyle:'long'}})}}catch(e){{}}</script>", esc(name), esc(&info.version));

    // the job in numbers, beside the picture of the stock
    h += "<h2>The job</h2><div class=\"top\"><dl>";
    let mut dl = |k: &str, v: String| { h += &format!("<dt>{}</dt><dd>{}</dd>", esc(k), esc(&v)); };
    if let Some(m) = machine { dl("Machine", m.name.clone()); }
    dl("Stock", format!("{} \u{d7} {} \u{d7} {}", len(st.w, units), len(st.h, units), len(st.t, units)));
    if !st.cut.is_empty() { dl("Material", crate::feeds::MATERIALS.iter().find(|m| m.id == st.cut).map_or(st.cut.clone(), |m| m.label.to_string())); }
    dl("X0 Y0", zero_name(&st.zr).to_string());
    dl("Z0", z_zero(st, units));
    dl("Estimated time", duration(total_min));
    dl("Cutting", format!("{} of {}", duration(cut_min), match units { Units::Metric => format!("{} mm", trim(cut_len, 0)), Units::Imperial => format!("{} in", trim(cut_len / 25.4, 1)) }));
    dl("Safe height", len(data.job.safe, units));
    h += "</dl>";
    h += &diagram(data);
    h += "</div>";

    // the tools, in order
    let tools = tools_in_order(data);
    if !tools.is_empty() {
        h += "<h2>Tools, in the order they are used</h2><table><tr><th>#</th><th>Tool</th><th>Type</th><th class=\"n\">Diameter</th><th class=\"n\">Speed</th><th class=\"n\">Feed</th><th class=\"n\">Plunge</th><th>Operations</th></tr>";
        for (i, (t, used)) in tools.iter().enumerate() {
            let pen = t.shape == "pen";
            h += &format!("<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td>{}</td></tr>",
                i + 1, esc(&t.name), shape_name(&t.shape), match units { Units::Metric => format!("{} mm", trim(t.dia, 3)), Units::Imperial => len(t.dia, units) },
                if pen { "-".to_string() } else { format!("{} rpm", trim(t.rpm, 0)) }, feed(t.feed, units), feed(t.plunge, units),
                used.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(", "));
        }
        h += "</table>";
        if tools.len() > 1 { h += "<p class=\"sub\">The program stops for each tool change. Touch off Z0 again with the new tool before you resume.</p>"; }
    }

    // the operations
    h += "<h2>Operations</h2><table><tr><th>#</th><th>Operation</th><th>Shapes</th><th>Tool</th><th class=\"n\">Depth</th><th class=\"n\">Pass</th><th>Notes</th><th class=\"n\">Time</th></tr>";
    for (i, o) in data.ops.iter().enumerate() {
        let shapes_n = data.loops.iter().filter(|l| l.op == o.id).count();
        let tool = data.tools.iter().find(|t| t.id == o.p.tool).map_or("-".to_string(), |t| t.name.clone());
        let pass = o.p.pass_depth.or_else(|| data.tools.iter().find(|t| t.id == o.p.tool).map(|t| t.pd)).map_or("-".to_string(), |v| len(v, units));
        let mut flags: Vec<String> = Vec::new();
        if o.p.tabs { flags.push("holding tabs".into()); }
        if o.p.air || data.job.air { flags.push("<span class=\"warn\">air cut: nothing is cut</span>".into()); }
        if let Some(m) = &o.p.pause { flags.push(format!("pause{}", if m.is_empty() { String::new() } else { format!(": {}", esc(m)) })); }
        if crate::pieces::is_piece_op(o) { flags.push("separate piece of stock".into()); }
        let t = timing.ops.get(i).map_or(0.0, |t| t.total());
        h += &format!("<tr><td><span class=\"dot\" style=\"background:{}\"></span>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td>{}</td><td class=\"n\">{}</td></tr>",
            COLORS[i % COLORS.len()], i + 1, kind_name(&o.p.kind), shapes_n, esc(&tool), len(o.p.depth, units), pass, flags.join(", "), duration(t));
    }
    h += "</table>";

    // before you start
    h += "<h2>Before you start</h2><ul class=\"check\">";
    let corner = zero_name(&st.zr);
    h += "<li>The stock is held down and the clamps are clear of the toolpath.</li>";
    h += &format!("<li>X0 Y0 is set at the {corner} of the stock; Z0 is {}.</li>", esc(&z_zero(st, units)));
    if !tools.is_empty() { h += &format!("<li>The first tool is in the holder: {}.</li>", esc(&tools[0].0.name)); }
    for r in &rows {
        if matches!(r.label, "Before you start" | "At a tool change" | "Pauses" | "Air cut") { h += &format!("<li{}><b>{}.</b> {}</li>", if r.warn { " class=\"warn\"" } else { "" }, esc(r.label), esc(&r.text)); }
    }
    h += "<li>Dust collection, eye and ear protection are on. A hand on the stop button for the first minute.</li></ul>";

    // warnings
    let warns: Vec<String> = main.notes.iter().chain(parts[1..].iter().flat_map(|p| p.out.notes.iter())).filter(|n| n.starts_with('\u{26a0}')).map(|n| n.trim_start_matches('\u{26a0}').trim().to_string()).collect();
    if !warns.is_empty() {
        h += "<h2>Warnings</h2><ul>";
        for w in &warns { h += &format!("<li class=\"warn\">{}</li>", esc(w)); }
        h += "</ul>";
    }

    // separate pieces of stock
    for (i, p) in parts.iter().enumerate().skip(1) {
        let d = match &p.piece { Some(j) => &j.data, None => continue };
        h += &format!("<div class=\"piece\"><h2>Separate piece {}</h2><div class=\"top\"><dl><dt>Stock</dt><dd>{} \u{d7} {} \u{d7} {}</dd><dt>X0 Y0</dt><dd>{}</dd><dt>Z0</dt><dd>{}</dd><dt>Time</dt><dd>{}</dd></dl>{}</div></div>",
            i, len(d.stock.w, units), len(d.stock.h, units), len(d.stock.t, units), zero_name(&d.stock.zr), esc(&z_zero(&d.stock, units)), duration(p.out.time_min), diagram(d));
    }

    if !data.notes.trim().is_empty() { h += &format!("<h2>Project notes</h2><div class=\"notes\">{}</div>", esc(data.notes.trim())); }
    h += "<div class=\"foot\">Made by PutiaCNC. Check the toolpath on screen and run an air cut first on a new setup.</div></body></html>";
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pieces::generate_parts;

    fn sheet(units: Units, inch: bool) -> String {
        let mut f = shapes::blank_project();
        f.data.machine = Some(crate::model::Machine::default());
        f.data.tools = crate::setup::starter_tools(crate::setup::ToolChoice { inch_sizes: inch, ..Default::default() });
        crate::sample::fill_kind(&mut f.data, inch, crate::sample::Kind::NamePlate).unwrap();
        f.data.notes += "\nA <b>note</b> & more";
        let parts = generate_parts(&f.data, units).unwrap();
        html(&f.data, &parts, units, &SheetInfo { version: "0.4.2".into() }, "My <job>")
    }

    #[test]
    fn the_sheet_has_every_section_and_escapes_text() {
        let s = sheet(Units::Metric, false);
        for want in ["The job", "Tools, in the order they are used", "Operations", "Before you start", "Project notes", "<svg", "X0 Y0", "200 mm \u{d7} 150 mm \u{d7} 12 mm", "Profile outside", "My &lt;job&gt;", "&lt;b&gt;note&lt;/b&gt; &amp; more", "toLocaleDateString"] {
            assert!(s.contains(want), "missing {want:?}");
        }
        assert!(!s.contains("<b>note</b>") && !s.contains("My <job>"), "text is not escaped");
        assert!(s.contains("PutiaCNC 0.4.2"));
    }

    #[test]
    fn inch_sheet_uses_inches() {
        let s = sheet(Units::Imperial, true);
        assert!(s.contains(" in\u{a0}") || s.contains(" in<") || s.contains(" in "), "no inches");
        assert!(s.contains("in/min") && !s.contains("mm/min"), "feeds are not in inches");
    }

    #[test]
    fn air_cut_and_pauses_show_up() {
        let mut f = shapes::blank_project();
        f.data.machine = Some(crate::model::Machine::default());
        f.data.tools = crate::setup::starter_tools(Default::default());
        crate::sample::fill_kind(&mut f.data, false, crate::sample::Kind::NamePlate).unwrap();
        f.data.ops[0].p.air = true;
        f.data.ops[1].p.pause = Some("Vacuum the chips".into());
        let parts = generate_parts(&f.data, Units::Metric).unwrap();
        let s = html(&f.data, &parts, Units::Metric, &SheetInfo { version: "0".into() }, "t");
        assert!(s.contains("air cut: nothing is cut") && s.contains("Vacuum the chips"));
    }
}
