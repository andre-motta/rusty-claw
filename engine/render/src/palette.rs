//! Palette fades.
//!
//! A fade never touches pixels: it recomputes the 256 palette entries for
//! the frame's simulation time and the final pass reads the result. The
//! per-channel rule is the game's integer fade:
//!
//! `value = from + trunc((to - from) * e / d)`
//!
//! with `e` the milliseconds since the fade started and `d` its length; the
//! first visible step always uses `e = 10`, a fade of 10 ms or less shows its
//! end colours at once, and from `e >= d` on the colours are exactly the end
//! colours.
//!
//! A [`PaletteFade`] gives brightness levels from 0 (black) to 256
//! (unchanged). The start and end colours of each entry are the level colour
//! scaled by those levels, so a fade from 256 to 0 is exactly the game's fade
//! to black and a fade from 0 to 256 its fade back in.

use claw_assets::Palette;
use claw_frame::{PaletteFade, TICK_MS};

/// Full brightness.
pub const FULL_BRIGHTNESS: u32 = 256;

/// The elapsed time the first visible fade step uses.
pub const FIRST_STEP_MS: u32 = 10;

/// One channel of a fade step.
pub fn fade_entry(from: u8, to: u8, elapsed_ms: u32, duration_ms: u32) -> u8 {
    if elapsed_ms >= duration_ms {
        return to;
    }
    let delta = i64::from(to) - i64::from(from);
    // Integer division in Rust truncates toward zero.
    let step = delta * i64::from(elapsed_ms) / i64::from(duration_ms.max(1));
    (i64::from(from) + step).rem_euclid(256) as u8
}

/// The elapsed time a fade step uses `ms` after the fade started: at least
/// [`FIRST_STEP_MS`], and the full length for fades of 10 ms or less.
pub fn step_elapsed(ms: f64, duration_ms: u32) -> u32 {
    if duration_ms <= FIRST_STEP_MS {
        return duration_ms;
    }
    let ms = if ms.is_finite() { ms.max(0.0) } else { 0.0 };
    (ms.floor() as u64)
        .max(u64::from(FIRST_STEP_MS))
        .min(u64::from(duration_ms)) as u32
}

fn scale(c: u8, level: u32) -> u8 {
    (u32::from(c) * level.min(FULL_BRIGHTNESS) / FULL_BRIGHTNESS) as u8
}

/// The palette of a frame at simulation time `time_ms` under `fade`.
pub fn faded(palette: &Palette, fade: Option<&PaletteFade>, time_ms: f64) -> Palette {
    let Some(f) = fade else {
        return palette.clone();
    };
    let start = f.start_tick as f64 * f64::from(TICK_MS);
    let e = step_elapsed(time_ms - start, f.duration_ms);
    let mut out = palette.clone();
    for c in out.0.iter_mut() {
        for ch in c.iter_mut() {
            let a = scale(*ch, f.from);
            let b = scale(*ch, f.to);
            *ch = fade_entry(a, b, e, f.duration_ms);
        }
    }
    out
}

/// The brightness of a fade at simulation time `time_ms`, as a fraction of
/// full (1.0 when no fade runs). Images that bypass the palette are scaled
/// by it, the continuous form of the same fade.
pub fn brightness(fade: Option<&PaletteFade>, time_ms: f64) -> f32 {
    let Some(f) = fade else {
        return 1.0;
    };
    let start = f.start_tick as f64 * f64::from(TICK_MS);
    let e = step_elapsed(time_ms - start, f.duration_ms);
    let from = f64::from(f.from.min(FULL_BRIGHTNESS));
    let to = f64::from(f.to.min(FULL_BRIGHTNESS));
    let t = if f.duration_ms == 0 {
        1.0
    } else {
        f64::from(e) / f64::from(f.duration_ms)
    };
    ((from + (to - from) * t.min(1.0)) / f64::from(FULL_BRIGHTNESS)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_rule_truncates_toward_zero() {
        assert_eq!(fade_entry(200, 0, 10, 250), 192);
        assert_eq!(fade_entry(0, 200, 10, 250), 8);
        assert_eq!(fade_entry(7, 0, 249, 250), 1);
        assert_eq!(fade_entry(7, 0, 250, 250), 0);
        assert_eq!(fade_entry(5, 9, 0, 0), 9);
    }

    #[test]
    fn first_step_uses_ten_ms() {
        assert_eq!(step_elapsed(0.0, 250), 10);
        assert_eq!(step_elapsed(3.9, 250), 10);
        assert_eq!(step_elapsed(120.7, 250), 120);
        assert_eq!(step_elapsed(900.0, 250), 250);
        assert_eq!(step_elapsed(0.0, 10), 10);
        assert_eq!(step_elapsed(-5.0, 500), 10);
    }

    #[test]
    fn fade_to_black_and_back() {
        let mut pal = Palette::default();
        pal.0[1] = [200, 100, 50];
        let out = FULL_BRIGHTNESS;
        let fade = PaletteFade {
            from: out,
            to: 0,
            start_tick: 10,
            duration_ms: 250,
        };
        let at = |ms: f64| faded(&pal, Some(&fade), 180.0 + ms).0[1];
        assert_eq!(at(0.0), [192, 96, 48]);
        assert_eq!(at(125.0), [100, 50, 25]);
        assert_eq!(at(250.0), [0, 0, 0]);
        assert_eq!(faded(&pal, None, 0.0), pal);
        let back = PaletteFade {
            from: 0,
            to: out,
            ..fade
        };
        assert_eq!(faded(&pal, Some(&back), 180.0 + 125.0).0[1], [100, 50, 25]);
        assert_eq!(faded(&pal, Some(&back), 1000.0).0[1], [200, 100, 50]);
        assert!((brightness(Some(&fade), 180.0 + 125.0) - 0.5).abs() < 1e-6);
        assert_eq!(brightness(None, 0.0), 1.0);
    }
}
