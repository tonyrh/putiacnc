//! Typed input while drawing: start typing a number with the Line, Rectangle or Ellipse tool and a small
//! panel takes the exact values (position, length and angle, or width and height).
use crate::app::{App, Mode};
use eframe::egui::{self, Key, Modifiers, Vec2};
use putiacnc_core::shapes;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// The first point of a line: X and Y.
    Start,
    /// The next point of a line: length and angle from the last point.
    Segment,
    /// A rectangle or ellipse: X, Y, width and height (the lower-left corner is X, Y).
    Box,
    /// A circle: centre X, Y and radius (or diameter).
    Circle,
    /// Polygon, star or bolt circle: centre X, Y, radius and the direction of the first corner or hole.
    Radial,
}

#[derive(Clone, Debug)]
pub struct DynIn {
    pub kind: Kind,
    pub vals: Vec<String>,
    pub active: usize,
    pub focus: bool,
    /// The size field is a diameter, not a radius.
    pub diam: bool,
}

impl DynIn {
    fn labels(&self) -> &'static [&'static str] {
        match self.kind { Kind::Start => &["X", "Y"], Kind::Segment => &["Length", "Angle"], Kind::Box => &["X", "Y", "Width", "Height"], Kind::Circle => if self.diam { &["X", "Y", "Diameter"] } else { &["X", "Y", "Radius"] }, Kind::Radial => &["X", "Y", "Radius", "Angle"] }
    }
}

fn num(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', ".");
    if t.is_empty() { None } else { t.parse::<f64>().ok().filter(|v| v.is_finite()) }
}

/// What the typed values mean right now. Empty fields follow the pointer; at least one value must be typed.
pub enum Out {
    Point([f64; 2]),
    Box([f64; 2], [f64; 2]),
    Circle([f64; 2], f64),
    /// Centre, radius and the angle of the first corner (radians).
    Radial([f64; 2], f64, f64),
}

/// `last` is the previous point of the line being drawn, `hover` the pointer (already snapped), `zero` the XY zero.
pub fn evaluate(d: &DynIn, last: Option<[f64; 2]>, hover: [f64; 2], zero: (f64, f64)) -> Option<Out> {
    let v: Vec<Option<f64>> = d.vals.iter().map(|s| num(s)).collect();
    if v.iter().all(|x| x.is_none()) { return None; }
    match d.kind {
        Kind::Start => Some(Out::Point([v[0].map_or(hover[0], |x| x + zero.0), v[1].map_or(hover[1], |y| y + zero.1)])),
        Kind::Segment => {
            let last = last?;
            let ang = v[1].map_or_else(|| (hover[1] - last[1]).atan2(hover[0] - last[0]), |a| a.to_radians());
            let len = v[0].unwrap_or_else(|| (hover[0] - last[0]).hypot(hover[1] - last[1]));
            Some(Out::Point([last[0] + len * ang.cos(), last[1] + len * ang.sin()]))
        }
        Kind::Circle => {
            let r = v[2]? / if d.diam { 2.0 } else { 1.0 };
            if r <= 0.005 { return None; }
            Some(Out::Circle([v[0].map_or(hover[0], |x| x + zero.0), v[1].map_or(hover[1], |y| y + zero.1)], r))
        }
        Kind::Radial => {
            let r = v[2]?;
            if r <= 0.005 { return None; }
            let ang = v[3].map_or(std::f64::consts::FRAC_PI_2, |a| a.to_radians());
            Some(Out::Radial([v[0].map_or(hover[0], |x| x + zero.0), v[1].map_or(hover[1], |y| y + zero.1)], r, ang))
        }
        Kind::Box => {
            let (w, h) = (v[2]?, v[3]?);
            if w.abs() < 0.01 || h.abs() < 0.01 { return None; }
            let a = [v[0].map_or(hover[0], |x| x + zero.0), v[1].map_or(hover[1], |y| y + zero.1)];
            Some(Out::Box(a, [a[0] + w, a[1] + h]))
        }
    }
}

/// What Enter would add, as outlines for the preview (world mm): one polyline per outline or mark.
pub fn preview(app: &App, hover: [f64; 2], zero: (f64, f64)) -> Option<Vec<Vec<[f64; 2]>>> {
    let d = app.dyn_in.as_ref()?;
    let closed = |l: putiacnc_core::model::LoopData| -> Vec<[f64; 2]> {
        let mut v: Vec<[f64; 2]> = l.pts.chunks_exact(2).map(|c| [c[0] as f64 / shapes::S, c[1] as f64 / shapes::S]).collect();
        if let Some(f) = v.first().copied() { v.push(f); }
        v
    };
    let cross = |p: [f64; 2]| vec![vec![[p[0] - 1.5, p[1]], [p[0] + 1.5, p[1]]], vec![[p[0], p[1] - 1.5], [p[0], p[1] + 1.5]]];
    let marks = |v: Vec<putiacnc_core::model::LoopData>| -> Vec<Vec<[f64; 2]>> {
        v.iter().filter_map(shapes::bbox).flat_map(|b| cross([(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0])).collect()
    };
    match evaluate(d, app.poly.last().copied(), hover, zero)? {
        Out::Point(p) if app.mode == Mode::Point => Some(cross(p)),
        Out::Point(p) => {
            let mut v: Vec<[f64; 2]> = app.poly.clone();
            v.push(p);
            Some(vec![v])
        }
        Out::Circle(c, r) => Some(vec![closed(shapes::new_circle(c, r, 0))]),
        Out::Radial(c, r, ang) if app.mode == Mode::Polygon => {
            let inner = if app.poly_star { Some(app.poly_inner / 100.0) } else { None };
            shapes::new_polygon(c, r, ang, app.poly_sides.round() as usize, inner, 0).map(|l| vec![closed(l)])
        }
        Out::Radial(c, r, ang) => Some(marks(shapes::bolt_circle(c, r, ang, app.point_count.round() as usize, app.point_sweep, 0))),
        Out::Box(a, b) if app.mode == Mode::Point => Some(marks(shapes::point_grid(a, b, app.grid_cols.round() as usize, app.grid_rows.round() as usize, 0))),
        Out::Box(a, b) => {
            let l = if app.mode == Mode::Ellipse { shapes::new_ellipse(a, b, 0) } else { shapes::new_rect(a, b, app.rect_radius, 0) };
            Some(vec![closed(l)])
        }
    }
}

/// Drill points typed in: they join the drawing and the tool stays active.
fn add_points(app: &mut App, v: Vec<putiacnc_core::model::LoopData>) {
    if v.is_empty() { return; }
    app.hist.record(&app.project.data);
    app.project.data.loops.extend(v);
    app.sel.clear();
    app.touch();
}

/// Opens the panel when a number is typed over the drawing area, and runs it while it is open.
/// `hover` is the snapped pointer position, `anchor` the screen position the panel hangs from.
pub fn drive(ctx: &egui::Context, app: &mut App, pointer_over: bool, hover: Option<[f64; 2]>, zero: (f64, f64), anchor: egui::Pos2) {
    let usable = matches!(app.mode, Mode::Line | Mode::Rect | Mode::Ellipse | Mode::Polygon | Mode::Point) && app.tab_edit.is_none() && app.node_edit.is_none() && matches!(app.drag, crate::app::Drag::None);
    if !usable { app.dyn_in = None; return; }
    // the kind follows the tool, and for lines whether a point has been placed
    let want = match app.mode { Mode::Line if app.poly.is_empty() => Kind::Start, Mode::Line => Kind::Segment, Mode::Ellipse if app.circle_mode => Kind::Circle, Mode::Polygon => Kind::Radial, Mode::Point if app.point_ring => Kind::Radial, Mode::Point if app.point_grid => Kind::Box, Mode::Point => Kind::Start, _ => Kind::Box };
    if let Some(d) = &app.dyn_in { if d.kind != want { app.dyn_in = None; } }

    if app.dyn_in.is_none() {
        if ctx.wants_keyboard_input() || !pointer_over { return; }
        let typed = ctx.input(|i| i.events.iter().find_map(|e| match e { egui::Event::Text(t) if t.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '.' || c == ',') && !t.is_empty() => Some(t.clone()), _ => None }));
        let Some(t) = typed else { return };
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Text(_))));
        let n = match want { Kind::Start | Kind::Segment => 2, Kind::Circle => 3, Kind::Box | Kind::Radial => 4 };
        let mut vals = vec![String::new(); n];
        // a rectangle starts at the pointer, so the first thing to type is its width
        let active = if matches!(want, Kind::Box | Kind::Circle | Kind::Radial) { 2 } else { 0 };
        vals[active] = t;
        app.dyn_in = Some(DynIn { kind: want, vals, active, focus: true, diam: app.circle_diam });
    }

    // keys first, so Tab does not move the focus out of the panel
    let (enter, esc, tab) = ctx.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::Enter), i.consume_key(Modifiers::NONE, Key::Escape), i.consume_key(Modifiers::NONE, Key::Tab) || i.consume_key(Modifiers::SHIFT, Key::Tab)));
    if esc { app.dyn_in = None; return; }
    let mut d = app.dyn_in.clone().unwrap();
    if tab { d.active = (d.active + 1) % d.vals.len(); d.focus = true; }
    let title = match d.kind { Kind::Start if app.mode == Mode::Point => "Drill point", Kind::Radial if app.mode == Mode::Point => "Bolt circle", Kind::Radial => if app.poly_star { "Star" } else { "Polygon" }, Kind::Box if app.mode == Mode::Point => "Point grid", Kind::Start => "Start point", Kind::Segment => "Next point", Kind::Circle => "Circle", Kind::Box => if app.mode == Mode::Ellipse { "Ellipse" } else { "Rectangle" } };
    let labels = d.labels();
    egui::Area::new(egui::Id::new("dynin")).order(egui::Order::Foreground).fixed_pos(anchor + Vec2::new(10.0, 108.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(8.0, 6.0)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            ui.strong(title);
            egui::Grid::new("dynin_grid").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
                for (k, lab) in labels.iter().enumerate() {
                    ui.label(*lab);
                    let id = egui::Id::new(("dynin_field", k));
                    let r = ui.add(egui::TextEdit::singleline(&mut d.vals[k]).id(id).desired_width(64.0));
                    if d.focus && d.active == k { r.request_focus(); }
                    if r.has_focus() { d.active = k; }
                    let unit = match (d.kind, k) { (Kind::Segment, 1) | (Kind::Radial, 3) => "°", _ => "mm" };
                    ui.weak(unit);
                    if (labels.len() == 4 && k % 2 == 1) || labels.len() == 2 || (labels.len() == 3 && k != 0) { ui.end_row(); }
                }
            });
            ui.weak("Enter adds · Tab next · Esc cancels");
            ui.weak("Empty fields follow the pointer");
        });
    });
    d.focus = false;
    if enter {
        let res = hover.and_then(|h| evaluate(&d, app.poly.last().copied(), h, zero));
        match res {
            Some(Out::Point(p)) if app.mode == Mode::Point => {
                let op = app.project.data.act;
                add_points(app, vec![shapes::new_point(p, op)]);
                app.dyn_in = None;
            }
            Some(Out::Point(p)) => {
                app.poly.push(p);
                app.dyn_in = None;
            }
            Some(Out::Radial(c, r, ang)) => {
                let op = app.project.data.act;
                if app.mode == Mode::Polygon {
                    let inner = if app.poly_star { Some(app.poly_inner / 100.0) } else { None };
                    if let Some(l) = shapes::new_polygon(c, r, ang, app.poly_sides.round() as usize, inner, op) { app.add_loop(l); }
                } else {
                    add_points(app, shapes::bolt_circle(c, r, ang, app.point_count.round() as usize, app.point_sweep, op));
                }
                app.dyn_in = None;
            }
            Some(Out::Box(a, b)) if app.mode == Mode::Point => {
                let op = app.project.data.act;
                add_points(app, shapes::point_grid(a, b, app.grid_cols.round() as usize, app.grid_rows.round() as usize, op));
                app.dyn_in = None;
            }
            Some(Out::Circle(c, r)) => {
                let op = app.project.data.act;
                app.add_loop(shapes::new_circle(c, r, op));
                app.dyn_in = None;
            }
            Some(Out::Box(a, b)) => {
                let op = app.project.data.act;
                let l = if app.mode == Mode::Ellipse { shapes::new_ellipse(a, b, op) } else { shapes::new_rect(a, b, app.rect_radius, op) };
                app.add_loop(l);
                app.dyn_in = None;
            }
            None => app.dyn_in = Some(d),
        }
        return;
    }
    app.dyn_in = Some(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(kind: Kind, v: &[&str]) -> DynIn { DynIn { kind, vals: v.iter().map(|s| s.to_string()).collect(), active: 0, focus: false, diam: false } }

    #[test]
    fn typed_length_and_angle_make_the_next_point() {
        let last = Some([10.0, 10.0]);
        let Some(Out::Point(p)) = evaluate(&d(Kind::Segment, &["20", "90"]), last, [0.0, 0.0], (0.0, 0.0)) else { panic!() };
        assert!((p[0] - 10.0).abs() < 1e-9 && (p[1] - 30.0).abs() < 1e-9, "{:?}", p);
        // no angle typed: the pointer gives the direction
        let Some(Out::Point(p)) = evaluate(&d(Kind::Segment, &["5", ""]), last, [10.0, 100.0], (0.0, 0.0)) else { panic!() };
        assert!((p[0] - 10.0).abs() < 1e-9 && (p[1] - 15.0).abs() < 1e-9);
        // decimal comma, nothing typed
        let Some(Out::Point(p)) = evaluate(&d(Kind::Segment, &["2,5", "0"]), last, [0.0, 0.0], (0.0, 0.0)) else { panic!() };
        assert!((p[0] - 12.5).abs() < 1e-9);
        assert!(evaluate(&d(Kind::Segment, &["", ""]), last, [0.0, 0.0], (0.0, 0.0)).is_none());
    }

    #[test]
    fn the_start_point_is_relative_to_the_xy_zero() {
        let Some(Out::Point(p)) = evaluate(&d(Kind::Start, &["5", "-3"]), None, [0.0, 0.0], (200.0, 150.0)) else { panic!() };
        assert_eq!(p, [205.0, 147.0]);
        // one value typed: the other follows the pointer
        let Some(Out::Point(p)) = evaluate(&d(Kind::Start, &["5", ""]), None, [1.0, 2.0], (0.0, 0.0)) else { panic!() };
        assert_eq!(p, [5.0, 2.0]);
    }

    #[test]
    fn a_polygon_or_ring_takes_centre_radius_and_angle() {
        let Some(Out::Radial(c, r, a)) = evaluate(&d(Kind::Radial, &["10", "20", "15", "30"]), None, [0.0, 0.0], (0.0, 0.0)) else { panic!() };
        assert_eq!((c, r), ([10.0, 20.0], 15.0));
        assert!((a - 30.0_f64.to_radians()).abs() < 1e-12);
        // no angle: the first corner points straight up
        let Some(Out::Radial(_, _, a)) = evaluate(&d(Kind::Radial, &["", "", "5", ""]), None, [3.0, 4.0], (0.0, 0.0)) else { panic!() };
        assert!((a - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(evaluate(&d(Kind::Radial, &["1", "2", "", "45"]), None, [0.0, 0.0], (0.0, 0.0)).is_none());
    }

    #[test]
    fn a_circle_takes_radius_or_diameter() {
        let Some(Out::Circle(c, r)) = evaluate(&d(Kind::Circle, &["10", "20", "6"]), None, [0.0, 0.0], (0.0, 0.0)) else { panic!() };
        assert_eq!((c, r), ([10.0, 20.0], 6.0));
        let mut dd = d(Kind::Circle, &["", "", "6"]);
        dd.diam = true;
        let Some(Out::Circle(c, r)) = evaluate(&dd, None, [7.0, 8.0], (0.0, 0.0)) else { panic!() };
        assert_eq!((c, r), ([7.0, 8.0], 3.0)); // empty centre: the pointer
        assert!(evaluate(&d(Kind::Circle, &["10", "20", ""]), None, [0.0, 0.0], (0.0, 0.0)).is_none());
    }

    #[test]
    fn a_box_needs_both_sizes() {
        assert!(evaluate(&d(Kind::Box, &["", "", "40", ""]), None, [0.0, 0.0], (0.0, 0.0)).is_none());
        let Some(Out::Box(a, b)) = evaluate(&d(Kind::Box, &["10", "20", "40", "-15"]), None, [0.0, 0.0], (0.0, 0.0)) else { panic!() };
        assert_eq!((a, b), ([10.0, 20.0], [50.0, 5.0]));
        // empty X, Y: at the pointer
        let Some(Out::Box(a, _)) = evaluate(&d(Kind::Box, &["", "", "40", "10"]), None, [7.0, 8.0], (0.0, 0.0)) else { panic!() };
        assert_eq!(a, [7.0, 8.0]);
    }
}
