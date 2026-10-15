//! The game's volume, pan and pitch formulas.
//!
//! Levels are in hundredths of a decibel, the unit the game works in: 0 is
//! full level and -10000 is silence.
//!
//! - Volume: a slider value `v` in 0..=100 gives `trunc(-1000 * log2(100 / v))`,
//!   so each halving of `v` lowers the level by 10 dB. 100 is 0 and 0 is
//!   silence.
//! - Pan: the same curve read from the other end. A pan `p` in -100..=100
//!   attenuates the opposite channel by the level of volume `100 - |p|`, so
//!   +-100 silences one side and 0 is centred.
//! - Pitch: a percent change of the sample's own rate,
//!   `f = base + trunc(base * pitch / 100)`, clamped to 101..=99999 Hz with an
//!   unsigned comparison, so a negative result lands on 99999.
//! - Music: a 0..=100 volume maps linearly to the 0..=127 MIDI master volume
//!   `m`, which scales every channel volume of the music by `m / 127`; the
//!   synthesizer squares channel volume into gain, so the music's gain is
//!   `(m / 127)^2`.

/// The silent level, in hundredths of a decibel.
pub const SILENCE: i32 = -10000;

/// Lowest playback frequency in Hz after the pitch clamp.
pub const MIN_FREQUENCY: u32 = 101;

/// Highest playback frequency in Hz after the pitch clamp.
pub const MAX_FREQUENCY: u32 = 99999;

/// Level of a 0..=100 volume in hundredths of a decibel.
///
/// Values above 100 are treated as 100.
pub fn attenuation(v: u8) -> i32 {
    match v {
        0 => SILENCE,
        100.. => 0,
        _ => (-1000.0 * (100.0 / f64::from(v)).log2()) as i32,
    }
}

/// Signed pan value in hundredths of a decibel for a pan of -100..=100.
///
/// Positive values attenuate the left channel by that amount, negative values
/// attenuate the right channel. Inputs beyond +-100 are treated as +-100.
pub fn pan_value(p: i8) -> i32 {
    let p = p.clamp(-100, 100);
    let mag = 100 - p.unsigned_abs();
    if p >= 0 {
        -attenuation(mag)
    } else {
        attenuation(mag)
    }
}

/// Linear gain for a level in hundredths of a decibel. [`SILENCE`] and below
/// give exactly 0, 0 and above give 1.
pub fn gain(level: i32) -> f32 {
    if level <= SILENCE {
        0.0
    } else if level >= 0 {
        1.0
    } else {
        10f64.powf(f64::from(level) / 2000.0) as f32
    }
}

/// Left and right gains of a pan value of -100..=100.
pub fn pan_gains(p: i8) -> (f32, f32) {
    let v = pan_value(p);
    if v > 0 {
        (gain(-v), 1.0)
    } else {
        (1.0, gain(v))
    }
}

/// Playback frequency in Hz of a sample with base rate `base` at a pitch
/// change of `pitch` percent.
///
/// The arithmetic is signed 32-bit with division truncating toward zero, and
/// the clamp compares the result as unsigned, as the game does: a result of
/// 100000 or more becomes 99999, 100 or less becomes 101, and a negative
/// result counts as a large value and becomes 99999.
pub fn pitch_frequency(base: u32, pitch: i16) -> u32 {
    let base = base as i32;
    let scaled = base.wrapping_mul(i32::from(pitch)) / 100;
    let f = base.wrapping_add(scaled) as u32;
    if f >= 100_000 {
        MAX_FREQUENCY
    } else if f <= 100 {
        MIN_FREQUENCY
    } else {
        f
    }
}

/// MIDI master volume 0..=127 of a 0..=100 music volume.
pub fn music_master(v: u8) -> u8 {
    match v {
        0 => 0,
        100.. => 127,
        _ => (u32::from(v) * 127 / 100) as u8,
    }
}

/// Linear gain applied to the music source at a 0..=100 music volume: the
/// square of the MIDI master volume over 127.
pub fn music_gain(v: u8) -> f32 {
    let m = f32::from(music_master(v)) / 127.0;
    m * m
}

/// A request volume of 0..=100 taken relative to a configured channel volume
/// of 0..=100, so a request of 100 plays at the configured volume.
pub fn relative_volume(request: u8, channel: u8) -> u8 {
    (u32::from(request.min(100)) * u32::from(channel.min(100)) / 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_reference_points() {
        assert_eq!(attenuation(100), 0);
        assert_eq!(attenuation(50), -1000);
        assert_eq!(attenuation(25), -2000);
        assert_eq!(attenuation(1), -6643);
        assert_eq!(attenuation(0), -10000);
        assert_eq!(attenuation(200), 0);
    }

    #[test]
    fn curve_matches_integer_formula_everywhere() {
        // A few more points of trunc(-1000 * log2(100 / v)).
        let expected = [
            (2, -5643),
            (3, -5058),
            (10, -3321),
            (33, -1599),
            (80, -321),
            (99, -14),
        ];
        for (v, e) in expected {
            assert_eq!(attenuation(v), e, "v = {v}");
        }
        for v in 1..100u8 {
            let a = attenuation(v);
            assert!(a < 0 && a > SILENCE);
            assert!(attenuation(v + 1) >= a, "monotonic at {v}");
        }
    }

    #[test]
    fn pan_reference_points() {
        assert_eq!(pan_value(0), 0);
        assert_eq!(pan_value(100), 10000);
        assert_eq!(pan_value(-100), -10000);
        assert_eq!(pan_value(50), 1000);
        assert_eq!(pan_value(-50), -1000);
        assert_eq!(pan_value(127), 10000);
        assert_eq!(pan_value(-128), -10000);

        assert_eq!(pan_gains(0), (1.0, 1.0));
        assert_eq!(pan_gains(100), (0.0, 1.0));
        assert_eq!(pan_gains(-100), (1.0, 0.0));
        let (l, r) = pan_gains(50);
        assert!((l - 10f32.powf(-0.5)).abs() < 1e-6);
        assert_eq!(r, 1.0);
    }

    #[test]
    fn pitch_formula_and_clamps() {
        assert_eq!(pitch_frequency(22050, 0), 22050);
        assert_eq!(pitch_frequency(22050, 50), 33075);
        assert_eq!(pitch_frequency(11025, -10), 11025 - 1102);
        // Upper clamp.
        assert_eq!(pitch_frequency(22050, 400), 99999);
        assert_eq!(pitch_frequency(50000, 100), 99999);
        assert_eq!(pitch_frequency(99999, 0), 99999);
        // Lower clamp.
        assert_eq!(pitch_frequency(22050, -100), 101);
        assert_eq!(pitch_frequency(100, 0), 101);
        assert_eq!(pitch_frequency(101, 0), 101);
        assert_eq!(pitch_frequency(200, -50), 101);
        // A negative result compares as a large unsigned value.
        assert_eq!(pitch_frequency(22050, -150), 99999);
        assert_eq!(pitch_frequency(8000, -101), 99999);
    }

    #[test]
    fn gains() {
        assert_eq!(gain(0), 1.0);
        assert_eq!(gain(SILENCE), 0.0);
        assert!((gain(-2000) - 0.1).abs() < 1e-7);
        assert!((gain(-1000) - 0.316_227_77).abs() < 1e-6);
    }

    #[test]
    fn music_mapping() {
        assert_eq!(music_master(0), 0);
        assert_eq!(music_master(50), 63);
        assert_eq!(music_master(100), 127);
        assert_eq!(music_master(250), 127);
        assert_eq!(music_gain(100), 1.0);
        assert_eq!(music_gain(0), 0.0);
        // The default 50 is about -12.2 dB.
        let db = 20.0 * music_gain(50).log10();
        assert!((db + 12.2).abs() < 0.05, "{db}");
        assert_eq!(music_master(1), 1);
        assert_eq!(music_master(99), 125);
    }

    #[test]
    fn relative_volumes() {
        assert_eq!(relative_volume(100, 80), 80);
        assert_eq!(relative_volume(50, 80), 40);
        assert_eq!(relative_volume(0, 80), 0);
        assert_eq!(relative_volume(255, 100), 100);
    }
}
