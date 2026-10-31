//! Single-line text: letters drawn as open strokes (the Hershey fonts) instead of outlines, so that a pen
//! or an engraving bit follows each letter's centre line. The fonts are small text files in `fonts/hershey`.
use crate::model::LoopData;
use crate::shapes::{new_poly, S};
use crate::text::{bend, Align, Flow, TextStyle};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Capital letters are this many font units tall.
const CAP: f64 = 21.0;
/// Distance between two lines, font units.
const LINE: f64 = 33.0;

/// (id, label shown in the font list, font data)
const FONTS: [(&str, &str, &str); 6] = [
    ("futural", "Single line · Sans", include_str!("../fonts/hershey/futural.hf")),
    ("futuram", "Single line · Sans bold", include_str!("../fonts/hershey/futuram.hf")),
    ("rowmans", "Single line · Roman", include_str!("../fonts/hershey/rowmans.hf")),
    ("rowmand", "Single line · Roman bold", include_str!("../fonts/hershey/rowmand.hf")),
    ("scripts", "Single line · Script", include_str!("../fonts/hershey/scripts.hf")),
    ("cursive", "Single line · Cursive", include_str!("../fonts/hershey/cursive.hf")),
];

/// The single-line fonts: (id, label).
pub fn fonts() -> Vec<(&'static str, &'static str)> { FONTS.iter().map(|f| (f.0, f.1)).collect() }

#[derive(Clone, Default)]
struct Glyph { adv: f64, strokes: Vec<Vec<[f64; 2]>> }

type Font = HashMap<char, Glyph>;

fn parse(data: &str) -> Font {
    let mut m = Font::new();
    for line in data.lines() {
        let mut it = line.splitn(3, ' ');
        let (Some(code), Some(adv)) = (it.next().and_then(|c| c.parse::<u32>().ok()), it.next().and_then(|a| a.parse::<f64>().ok())) else { continue };
        let Some(ch) = char::from_u32(code) else { continue };
        let strokes = it.next().unwrap_or("").split(';').map(|s| {
            s.split_whitespace().filter_map(|p| {
                let (x, y) = p.split_once(',')?;
                Some([x.parse::<f64>().ok()?, y.parse::<f64>().ok()?])
            }).collect::<Vec<_>>()
        }).filter(|s| s.len() >= 2).collect();
        m.insert(ch, Glyph { adv, strokes });
    }
    m
}

fn font(id: &str) -> Option<&'static Font> {
    static ALL: OnceLock<Vec<(&'static str, Font)>> = OnceLock::new();
    ALL.get_or_init(|| FONTS.iter().map(|f| (f.0, parse(f.2))).collect()).iter().find(|f| f.0 == id).map(|f| &f.1)
}

#[derive(Clone, Copy)]
enum Mark { Grave, Acute, Circumflex, Diaeresis, Tilde, Ring, Cedilla }

/// Letters made of a plain letter and a mark.
fn composed(ch: char) -> Option<(char, Mark)> {
    use Mark::*;
    const T: [(&str, char, Mark); 27] = [
        ("àÀ", 'a', Grave), ("áÁ", 'a', Acute), ("âÂ", 'a', Circumflex), ("äÄ", 'a', Diaeresis), ("ãÃ", 'a', Tilde), ("åÅ", 'a', Ring),
        ("èÈ", 'e', Grave), ("éÉ", 'e', Acute), ("êÊ", 'e', Circumflex), ("ëË", 'e', Diaeresis),
        ("ìÌ", 'i', Grave), ("íÍ", 'i', Acute), ("îÎ", 'i', Circumflex), ("ïÏ", 'i', Diaeresis),
        ("òÒ", 'o', Grave), ("óÓ", 'o', Acute), ("ôÔ", 'o', Circumflex), ("öÖ", 'o', Diaeresis), ("õÕ", 'o', Tilde),
        ("ùÙ", 'u', Grave), ("úÚ", 'u', Acute), ("ûÛ", 'u', Circumflex), ("üÜ", 'u', Diaeresis),
        ("ñÑ", 'n', Tilde), ("çÇ", 'c', Cedilla), ("ýÝ", 'y', Acute), ("ÿ", 'y', Diaeresis),
    ];
    T.iter().find_map(|t| {
        let mut cs = t.0.chars();
        let (lo, up) = (cs.next()?, cs.next());
        if ch == lo { Some((t.1, t.2)) } else if Some(ch) == up { Some((t.1.to_ascii_uppercase(), t.2)) } else { None }
    })
}

/// The strokes of a mark, centred on x = 0, with its lowest point at y = 0.
fn mark_strokes(m: Mark) -> Vec<Vec<[f64; 2]>> {
    match m {
        Mark::Grave => vec![vec![[-2.0, 3.0], [1.0, 0.0]]],
        Mark::Acute => vec![vec![[-1.0, 0.0], [2.0, 3.0]]],
        Mark::Circumflex => vec![vec![[-2.5, 0.0], [0.0, 3.0], [2.5, 0.0]]],
        Mark::Diaeresis => vec![vec![[-2.0, 0.0], [-2.0, 1.2]], vec![[2.0, 0.0], [2.0, 1.2]]],
        Mark::Tilde => vec![vec![[-3.0, 0.5], [-1.5, 2.2], [0.0, 1.2], [1.5, 0.0], [3.0, 1.7]]],
        Mark::Ring => {
            let r = 1.6;
            vec![(0..=8).map(|k| { let a = k as f64 * std::f64::consts::PI / 4.0; [r * a.cos(), r + r * a.sin()] }).collect()]
        }
        Mark::Cedilla => vec![vec![[0.0, 3.0], [0.0, 1.0], [1.5, 0.0], [0.5, -1.2], [-1.0, -1.5]]],
    }
}

fn bounds(strokes: &[Vec<[f64; 2]>]) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for p in strokes.iter().flatten() {
        let c = b.get_or_insert([p[0], p[1], p[0], p[1]]);
        c[0] = c[0].min(p[0]); c[1] = c[1].min(p[1]); c[2] = c[2].max(p[0]); c[3] = c[3].max(p[1]);
    }
    b
}

/// The glyph of `ch`, putting accented letters together and standing in for typographic characters. None: no such letter.
fn glyph(f: &Font, ch: char) -> Option<Glyph> {
    let ch = match ch {
        '‘' | '’' | '´' | '`' => '\'',
        '“' | '”' | '«' | '»' => '"',
        '–' | '—' | '−' => '-',
        '\t' | '\u{a0}' => ' ',
        c => c,
    };
    if let Some(g) = f.get(&ch) { return Some(g.clone()); }
    if ch == '°' {
        let mut g = Glyph { adv: 12.0, strokes: mark_strokes(Mark::Ring) };
        for s in &mut g.strokes { for p in s { p[0] += 6.0; p[1] += 17.0; } }
        return Some(g);
    }
    if ch == '…' {
        let d = f.get(&'.')?;
        let mut g = Glyph { adv: d.adv * 3.0, strokes: vec![] };
        for k in 0..3 { for s in &d.strokes { g.strokes.push(s.iter().map(|p| [p[0] + d.adv * k as f64, p[1]]).collect()); } }
        return Some(g);
    }
    let (base, mark) = composed(ch)?;
    let mut g = f.get(&base)?.clone();
    if base.eq_ignore_ascii_case(&'i') && g.strokes.len() > 1 {
        // the dot of an i is replaced by the mark
        let dot = (0..g.strokes.len()).filter(|&k| { let b = bounds(&g.strokes[k..=k]).unwrap_or([0.0; 4]); b[2] - b[0] < 3.5 && b[3] - b[1] < 3.5 })
            .max_by(|&a, &b| bounds(&g.strokes[a..=a]).unwrap()[1].total_cmp(&bounds(&g.strokes[b..=b]).unwrap()[1]));
        if let Some(d) = dot { g.strokes.remove(d); }
    }
    let b = bounds(&g.strokes)?;
    let cx = (b[0] + b[2]) / 2.0;
    let (dx, dy) = match mark { Mark::Cedilla => (cx, 0.0), _ => (cx, b[3] + 3.0) };
    for s in mark_strokes(mark) { g.strokes.push(s.iter().map(|p| [p[0] + dx, p[1] + dy]).collect()); }
    Some(g)
}

/// Strokes of `text` set in the single-line font `id` (see `fonts()`), the first line's baseline starting at `origin`
/// (mm, y up). Every stroke becomes an open shape of operation `op`; a stroke that ends where it began (an o) is closed.
pub fn line_text_loops(id: &str, text: &str, style: TextStyle, origin: [f64; 2], op: i64) -> Result<Vec<LoopData>, String> {
    let f = font(id).ok_or_else(|| "This single-line font is not available.".to_string())?;
    let scale = style.size_mm / CAP;
    let adv = |ch: char| -> f64 { glyph(f, ch).map(|g| g.adv).unwrap_or(16.0) * scale + style.spacing_mm + if ch == ' ' { style.word_spacing_mm } else { 0.0 } };
    // a stroke: its points in mm and whether it closes on itself
    type Stroke = (Vec<[f64; 2]>, bool);
    let put = |ch: char, x: f64, y: f64, out: &mut Vec<Stroke>| {
        let Some(g) = glyph(f, ch) else { return };
        for s in g.strokes {
            let mut p: Vec<[f64; 2]> = s.iter().map(|q| [x + q[0] * scale, y + q[1] * scale]).collect();
            let closed = p.len() >= 4 && (p[0][0] - p[p.len() - 1][0]).abs() < 1e-9 && (p[0][1] - p[p.len() - 1][1]).abs() < 1e-9;
            if closed { p.pop(); }
            out.push((p, closed));
        }
    };
    let mk = |p: &[[f64; 2]], closed: bool| new_poly(p, closed, op);
    let mut strokes: Vec<Vec<LoopData>> = Vec::new();
    let mut out: Vec<LoopData> = Vec::new();
    if style.flow == Flow::Vertical {
        let step = style.size_mm * 1.25 + style.spacing_mm;
        let col_w = text.lines().flat_map(|l| l.chars()).map(|c| adv(c) - style.spacing_mm).fold(0.0_f64, f64::max) + style.size_mm * 0.4;
        for (ci, line) in text.lines().enumerate() {
            let mut v: Vec<Stroke> = Vec::new();
            let cx = origin[0] + ci as f64 * col_w;
            for (k, ch) in line.chars().enumerate() {
                if ch == ' ' { continue; }
                put(ch, cx - (adv(ch) - style.spacing_mm) / 2.0, origin[1] - k as f64 * step, &mut v);
            }
            strokes.push(v.iter().filter_map(|(p, c)| mk(p, *c)).collect());
        }
    } else {
        for (li, line) in text.lines().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            let mut xs = Vec::with_capacity(chars.len());
            let mut run = 0.0;
            for &ch in &chars { xs.push(run); run += adv(ch); }
            let width = run - if chars.is_empty() { 0.0 } else { style.spacing_mm };
            let x0 = origin[0] - match style.align { Align::Left => 0.0, Align::Center => width / 2.0, Align::Right => width };
            let y = origin[1] - li as f64 * LINE * scale;
            let mut v: Vec<Stroke> = Vec::new();
            for (i, &ch) in chars.iter().enumerate() { put(ch, x0 + xs[i], y, &mut v); }
            if matches!(style.flow, Flow::ArcTop | Flow::ArcBottom | Flow::Arc) {
                let r = style.radius_mm.max(style.size_mm);
                let (deg, inside) = match style.flow { Flow::ArcTop => (90.0, false), Flow::ArcBottom => (270.0, true), _ => (style.arc_deg, style.arc_in) };
                strokes.push(v.iter().filter_map(|(p, c)| {
                    let q: Vec<[i64; 2]> = p.iter().map(|a| [(a[0] * S).round() as i64, (a[1] * S).round() as i64]).collect();
                    let b: Vec<[f64; 2]> = bend(&q, origin, r, deg.to_radians(), inside, *c).iter().map(|a| [a[0] as f64 / S, a[1] as f64 / S]).collect();
                    mk(&b, *c)
                }).collect());
            } else {
                strokes.push(v.iter().filter_map(|(p, c)| mk(p, *c)).collect());
            }
        }
    }
    for s in strokes { out.extend(s); }
    if style.shadow != [0.0, 0.0] {
        let (dx, dy) = ((style.shadow[0] * S).round() as i64, (style.shadow[1] * S).round() as i64);
        let mut sh: Vec<LoopData> = out.iter().map(|l| LoopData { pts: l.pts.chunks(2).flat_map(|p| [p[0] + dx, p[1] + dy]).collect(), ..l.clone() }).collect();
        sh.extend(out);
        out = sh;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(size: f64) -> TextStyle { TextStyle::plain(size, 0.0, Align::Left) }
    fn bb(ls: &[LoopData]) -> [f64; 4] {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for l in ls { for p in l.pts.chunks(2) {
            let (x, y) = (p[0] as f64 / S, p[1] as f64 / S);
            b[0] = b[0].min(x); b[1] = b[1].min(y); b[2] = b[2].max(x); b[3] = b[3].max(y);
        } }
        b
    }

    #[test]
    fn every_font_has_every_ascii_letter_and_all_points_are_finite() {
        for (id, _) in fonts() {
            let f = font(id).unwrap();
            for c in ' '..='~' { assert!(f.contains_key(&c), "{id} lacks {c:?}"); }
            for c in "àáâäãåèéêëìíîïòóôöõùúûüñçÿýÀÁÂÄÃÅÈÉÊËÌÍÎÏÒÓÔÖÕÙÚÛÜÑÇÝ°…’“–".chars() { assert!(glyph(f, c).is_some(), "{id} cannot make {c:?}"); }
            let ls = line_text_loops(id, "Hello, World! 0123456789 àèìòù", st(10.0), [0.0, 0.0], 1).unwrap();
            assert!(!ls.is_empty());
            assert!(ls.iter().all(|l| l.pts.len() >= 4));
        }
    }

    #[test]
    fn letters_are_open_strokes_as_tall_as_asked() {
        let ls = line_text_loops("futural", "H", st(10.0), [5.0, 7.0], 1).unwrap();
        assert_eq!(ls.len(), 3);
        assert!(ls.iter().all(|l| l.c == 0));
        let b = bb(&ls);
        assert!((b[3] - b[1] - 10.0).abs() < 1e-6 && (b[1] - 7.0).abs() < 1e-6, "{b:?}");
        // a round letter's stroke closes on itself
        let o = line_text_loops("futural", "o", st(10.0), [0.0, 0.0], 1).unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].c, 1);
    }

    #[test]
    fn alignment_spacing_and_lines() {
        let w = |s: TextStyle, t: &str| { let b = bb(&line_text_loops("futural", t, s, [0.0, 0.0], 1).unwrap()); b[2] - b[0] };
        let mut s = st(10.0);
        let plain = w(s, "HH");
        s.spacing_mm = 2.0;
        assert!((w(s, "HH") - plain - 2.0).abs() < 1e-6);
        let mut c = st(10.0);
        c.align = Align::Center;
        let b = bb(&line_text_loops("futural", "HH", c, [0.0, 0.0], 1).unwrap());
        assert!((b[0] + b[2]).abs() < 2.0, "{b:?}");
        let two = bb(&line_text_loops("futural", "H\nH", st(10.0), [0.0, 0.0], 1).unwrap());
        assert!(two[1] < -15.0);
    }

    #[test]
    fn accents_sit_above_the_letter_and_i_loses_its_dot() {
        let f = font("futural").unwrap();
        let a = glyph(f, 'a').unwrap();
        let e = glyph(f, 'è').unwrap();
        let (ba, be) = (bounds(&a.strokes).unwrap(), bounds(&e.strokes).unwrap());
        assert!(be[3] > ba[3] + 3.0 && (be[3] - ba[3]) < 7.0);
        assert_eq!(e.adv, f.get(&'e').unwrap().adv);
        let i = glyph(f, 'i').unwrap();
        let ii = glyph(f, 'ì').unwrap();
        assert!(bounds(&ii.strokes).unwrap()[3] < bounds(&i.strokes).unwrap()[3], "the dot must go");
        // capitals take the mark above the capital height
        let cap = glyph(f, 'È').unwrap();
        assert!(bounds(&cap.strokes).unwrap()[3] > CAP + 3.0);
        let c = glyph(f, 'ç').unwrap();
        assert!(bounds(&c.strokes).unwrap()[1] < -1.0);
    }

    #[test]
    fn arcs_follow_the_circle_and_vertical_stacks() {
        let mut s = st(8.0);
        s.flow = Flow::ArcTop;
        s.radius_mm = 40.0;
        s.align = Align::Center;
        let ls = line_text_loops("rowmans", "ARC TEXT", s, [0.0, 40.0], 1).unwrap();
        // centre of the circle is (0, 0); the letters stay between the baseline circle and the cap height
        for l in &ls { for p in l.pts.chunks(2) {
            let r = (p[0] as f64 / S).hypot(p[1] as f64 / S);
            assert!(r > 39.0 && r < 49.0, "{r}");
        } }
        let mut v = st(8.0);
        v.flow = Flow::Vertical;
        let b = bb(&line_text_loops("futural", "ABC", v, [0.0, 0.0], 1).unwrap());
        assert!(b[3] - b[1] > 20.0 && b[2] - b[0] < 10.0);
    }

    #[test]
    fn shadow_is_a_moved_copy_and_unknown_font_is_an_error() {
        let mut s = st(10.0);
        s.shadow = [1.0, -1.0];
        assert_eq!(line_text_loops("futural", "H", s, [0.0, 0.0], 1).unwrap().len(), 6);
        assert!(line_text_loops("nope", "H", st(10.0), [0.0, 0.0], 1).is_err());
    }
}
