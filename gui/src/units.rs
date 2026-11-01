//! Messages that come out of the core library and the windows are written in millimetres. `localize` rewrites the lengths in such a
//! text to inches when the app shows inches: "a cut 12.2 mm deep" becomes "a cut 0.48 in deep", "300 × 200 × 60 mm" becomes
//! "11.811 × 7.874 × 2.362 in", "800 mm/min" becomes "31.5 in/min". Texts without a length are returned as they are.

fn shown(v: f64, dec: usize, frac: bool) -> String { crate::widgets::fmt_inch(v, dec, frac) }

/// The numbers of the chain ending just before byte `end` (`12`, `300 × 200 × 60`, `1 to 8`): (start, end) of each, last first.
fn chain_before(t: &str, end: usize) -> Vec<(usize, usize)> {
    let b = t.as_bytes();
    let mut out = Vec::new();
    let mut e = end;
    loop {
        while e > 0 && b[e - 1] == b' ' { e -= 1; }
        let mut s = e;
        while s > 0 && (b[s - 1].is_ascii_digit() || b[s - 1] == b'.') { s -= 1; }
        if s == e || !b[s].is_ascii_digit() && b[s] != b'.' { break; }
        if s > 0 && b[s - 1] == b'-' && (s == 1 || b[s - 2] == b' ' || b[s - 2] == b'(') { s -= 1; }
        out.push((s, e));
        // a connector to the number before: "×" (multi-byte) or " to " or " x "
        let head = &t[..s];
        let trimmed = head.trim_end_matches(' ');
        if let Some(h) = trimmed.strip_suffix('×') { e = h.len(); }
        else if head.ends_with(" to ") { e = s - 4; }
        else if let Some(h) = trimmed.strip_suffix(" x") { e = h.len(); }
        else { break; }
    }
    out
}

/// `text` with every length in millimetres (and mm/min, mm²) written in inches. `frac`: as fractions of an inch where they are one.
pub fn localize(text: &str, inches: bool, frac: bool) -> String {
    if !inches || !text.contains("mm") { return text.to_string(); }
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut from = 0;
    while let Some(p) = text[from..].find("mm") {
        let at = from + p;
        from = at + 2;
        let bytes = text.as_bytes();
        // "mm" is the unit when it follows a number and does not start a longer word
        if at > 0 && bytes[at - 1].is_ascii_alphabetic() { continue; }
        let rest = &text[at + 2..];
        let (tail, new_tail, area, speed) = if rest.starts_with("/min") { (4, "in/min", false, true) }
            else if rest.starts_with('²') { ('²'.len_utf8(), "in²", true, false) }
            else if rest.chars().next().is_some_and(|c| c.is_alphabetic() || c == '/' && !rest.starts_with("/pass") && !rest.starts_with("/tooth")) { continue } else { (0, "in", false, false) };
        let nums = chain_before(text, at);
        if nums.is_empty() { continue; }
        for &(s, e) in &nums {
            let Ok(v) = text[s..e].parse::<f64>() else { continue };
            let s_new = if area { shown(v / 645.16, 4, false) } else if speed { shown(v / 25.4, 2, false) } else { shown(v / 25.4, 3, frac) };
            edits.push((s, e, s_new));
        }
        edits.push((at, at + 2 + tail, new_tail.to_string()));
        from = at + 2 + tail;
    }
    edits.sort_by_key(|e| e.0);
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    for (s, e, r) in edits { if s < pos { continue; } out.push_str(&text[pos..s]); out.push_str(&r); pos = e; }
    out.push_str(&text[pos..]);
    out
}

#[cfg(test)]
mod tests {
    use super::localize;

    #[test]
    fn lengths_in_messages_follow_the_units() {
        let l = |t: &str| localize(t, true, false);
        assert_eq!(l("an outside cut 12.2 mm deep"), "an outside cut 0.48 in deep");
        assert_eq!(l("Work area 300 × 200 × 60 mm"), "Work area 11.811 × 7.874 × 2.362 in");
        assert_eq!(l("feed 800 mm/min, plunge 250 mm/min"), "feed 31.5 in/min, plunge 9.84 in/min");
        assert_eq!(l("1 to 8 mm"), "0.039 to 0.315 in");
        assert_eq!(l("25.4 mm"), "1 in");
        assert_eq!(l("Specks of 645.16 mm² are left out"), "Specks of 1 in² are left out");
        assert_eq!(l("3.175 mm per pass"), "0.125 in per pass");
        assert_eq!(localize("3.175 mm per pass", true, true), "1/8 in per pass");
    }

    #[test]
    fn other_text_is_left_alone() {
        let l = |t: &str| localize(t, true, false);
        assert_eq!(l("The summary mentions nothing"), "The summary mentions nothing");
        assert_eq!(l("mm without a number"), "mm without a number");
        assert_eq!(l("a commit message"), "a commit message");
        assert_eq!(localize("12.2 mm deep", false, false), "12.2 mm deep");
    }
}
