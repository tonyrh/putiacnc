//! How long the machine really takes to run a toolpath: it cannot jump to the feed rate, it speeds up and slows down at a limited
//! acceleration, and it takes a corner at a speed the corner allows (the way Grbl's planner does). Used for the time estimate and
//! the 3D playback when the machine has an acceleration set; without one the time is the plain length / feed.
use crate::model::Machine;
use crate::toolpath::Move;

/// The machine's motion limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    /// mm/s² (Grbl's `$120` to `$122`).
    pub accel: f64,
    /// Junction deviation in mm (Grbl's `$11`): how far the path may cut a corner off; the larger it is, the faster corners are taken.
    pub junction: f64,
}

/// Grbl's default junction deviation.
pub const DEFAULT_JUNCTION: f64 = 0.01;

impl Machine {
    /// The motion limits, when the machine has an acceleration set.
    pub fn motion(&self) -> Option<Motion> {
        let a = self.accel.filter(|a| a.is_finite() && *a > 0.0)?;
        Some(Motion { accel: a.clamp(1.0, 100_000.0), junction: self.junction.filter(|j| j.is_finite() && *j > 0.0).unwrap_or(DEFAULT_JUNCTION).clamp(0.0001, 10.0) })
    }
}

/// Seconds to cover `len` mm entering at `vin` and leaving at `vout` (mm/s, both reachable), never faster than `vmax`.
fn segment_secs(len: f64, vin: f64, vout: f64, vmax: f64, a: f64) -> f64 {
    // the fastest the segment gets: the cruise speed, or the peak of the triangle when it is too short to reach it
    let peak = vmax.min(((2.0 * a * len + vin * vin + vout * vout) / 2.0).max(0.0).sqrt()).max(vin).max(vout);
    let (up, down) = ((peak * peak - vin * vin) / (2.0 * a), (peak * peak - vout * vout) / (2.0 * a));
    let cruise = (len - up - down).max(0.0);
    (peak - vin) / a + (peak - vout) / a + if peak > 0.0 { cruise / peak } else { 0.0 }
}

/// The speed at which the path may turn from direction `u0` into `u1`, between segments at most `cap` mm/s fast.
fn corner_speed(u0: [f64; 3], u1: [f64; 3], cap: f64, m: &Motion) -> f64 {
    let cos = -(u0[0] * u1[0] + u0[1] * u1[1] + u0[2] * u1[2]);
    if cos <= -0.999_999 { return cap; } // straight on
    if cos >= 0.999_999 { return 0.0; }  // turning right back
    let s = (0.5 * (1.0 - cos)).sqrt();
    (m.accel * m.junction * s / (1.0 - s)).sqrt().min(cap)
}

/// Gives every move the time it takes with `m`'s limits (minutes, times `tsc`, the machine's own correction on top). Moves that follow a
/// stop (`Move::stop`: the spindle starting, a dwell, a tool change) or do not start where the last one ended start from rest, and so
/// do the ones after a move with no length. Returns the total of all the times.
pub fn retime(moves: &mut [Move], m: &Motion, tsc: f64) -> f64 {
    let n = moves.len();
    let len = |mv: &Move| ((mv.to[0] - mv.from[0]).powi(2) + (mv.to[1] - mv.from[1]).powi(2) + (mv.to[2] - mv.from[2]).powi(2)).sqrt();
    let mut i = 0;
    while i < n {
        // a run of moves that follow each other without stopping
        if !(len(&moves[i]) > 1e-9 && moves[i].feed > 0.0) { i += 1; continue; }
        let mut j = i + 1;
        while j < n && !moves[j].stop && len(&moves[j]) > 1e-9 && moves[j].feed > 0.0
            && (0..3).all(|k| (moves[j].from[k] - moves[j - 1].to[k]).abs() < 1e-7) { j += 1; }
        let seg = &moves[i..j];
        let l: Vec<f64> = seg.iter().map(len).collect();
        let u: Vec<[f64; 3]> = seg.iter().zip(&l).map(|(mv, l)| [(mv.to[0] - mv.from[0]) / l, (mv.to[1] - mv.from[1]) / l, (mv.to[2] - mv.from[2]) / l]).collect();
        let vmax: Vec<f64> = seg.iter().map(|mv| mv.feed / 60.0).collect();
        // the speed at each junction: 0 at both ends, what the corner allows between
        let mut v = vec![0.0; seg.len() + 1];
        for k in 1..seg.len() { v[k] = corner_speed(u[k - 1], u[k], vmax[k - 1].min(vmax[k]), m); }
        for k in 0..seg.len() { v[k + 1] = v[k + 1].min((v[k] * v[k] + 2.0 * m.accel * l[k]).sqrt()); }
        for k in (0..seg.len()).rev() { v[k] = v[k].min((v[k + 1] * v[k + 1] + 2.0 * m.accel * l[k]).sqrt()); }
        for k in 0..seg.len() {
            let secs = segment_secs(l[k], v[k], v[k + 1], vmax[k], m.accel);
            moves[i + k].dt = secs / 60.0 * tsc;
        }
        i = j;
    }
    moves.iter().map(|mv| mv.dt).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(from: [f64; 3], to: [f64; 3], feed: f64) -> Move {
        Move { from, to, cut: true, dt: 0.0, feed, stop: false, pass: 0, op: 0, alt: false, det: false, lap: false }
    }
    fn motion() -> Motion { Motion { accel: 100.0, junction: 0.01 } }

    #[test]
    fn a_long_straight_move_reaches_the_feed_rate() {
        // 1200 mm/min = 20 mm/s at 100 mm/s²: 0.2 s up and 0.2 s down cover 2 mm each, the other 296 mm take 14.8 s
        let mut v = vec![mv([0.0; 3], [300.0, 0.0, 0.0], 1200.0)];
        let t = retime(&mut v, &motion(), 1.0);
        assert!((t * 60.0 - 15.2).abs() < 1e-6, "{}", t * 60.0);
    }

    #[test]
    fn a_short_move_never_reaches_the_feed_rate() {
        // 1 mm from rest to rest at 100 mm/s²: 2 x sqrt(1 / 100) = 0.2 s, not 1 / 20 s
        let mut v = vec![mv([0.0; 3], [1.0, 0.0, 0.0], 1200.0)];
        let t = retime(&mut v, &motion(), 1.0);
        assert!((t * 60.0 - 0.2).abs() < 1e-6, "{}", t * 60.0);
    }

    #[test]
    fn a_straight_line_in_two_pieces_takes_as_long_as_in_one() {
        let one = retime(&mut [mv([0.0; 3], [300.0, 0.0, 0.0], 1200.0)], &motion(), 1.0);
        let two = retime(&mut [mv([0.0; 3], [100.0, 0.0, 0.0], 1200.0), mv([100.0, 0.0, 0.0], [300.0, 0.0, 0.0], 1200.0)], &motion(), 1.0);
        assert!((one - two).abs() < 1e-9);
    }

    #[test]
    fn corners_cost_time_and_sharper_corners_cost_more() {
        let line = |turn: [f64; 3]| {
            let mut v = vec![mv([0.0; 3], [50.0, 0.0, 0.0], 1200.0), mv([50.0, 0.0, 0.0], [50.0 + turn[0], turn[1], 0.0], 1200.0)];
            retime(&mut v, &motion(), 1.0)
        };
        let straight = line([50.0, 0.0, 0.0]);
        let right = line([0.0, 50.0, 0.0]);
        let hairpin = line([-50.0, 0.1, 0.0]);
        assert!(straight < right && right < hairpin, "{straight} {right} {hairpin}");
    }

    #[test]
    fn a_stop_starts_the_next_move_from_rest() {
        let mut a = vec![mv([0.0; 3], [50.0, 0.0, 0.0], 1200.0), mv([50.0, 0.0, 0.0], [100.0, 0.0, 0.0], 1200.0)];
        let together = retime(&mut a, &motion(), 1.0);
        a[1].stop = true;
        let stopped = retime(&mut a, &motion(), 1.0);
        assert!(((stopped - together) * 60.0 - 0.2).abs() < 1e-6, "{}", (stopped - together) * 60.0); // 0.2 s: down to rest and up again
    }

    #[test]
    fn the_time_correction_still_applies_on_top() {
        let mut v = vec![mv([0.0; 3], [300.0, 0.0, 0.0], 1200.0)];
        let t = retime(&mut v, &motion(), 1.5);
        assert!((t * 60.0 - 22.8).abs() < 1e-6);
    }

    #[test]
    fn faster_acceleration_is_never_slower() {
        let path = || (0..200).map(|k| { let (a, b) = (k as f64 * 0.5, (k + 1) as f64 * 0.5); mv([a, (a * 0.7).sin(), 0.0], [b, (b * 0.7).sin(), 0.0], 1500.0) }).collect::<Vec<_>>();
        let slow = retime(&mut path(), &Motion { accel: 50.0, junction: 0.01 }, 1.0);
        let fast = retime(&mut path(), &Motion { accel: 500.0, junction: 0.01 }, 1.0);
        assert!(fast < slow, "{fast} {slow}");
        // and neither beats the plain length / feed
        let plain = 100.0 / 1500.0;
        assert!(fast > plain * 0.99);
    }
}
