//! The objects of a boss fight that are not the boss himself: on the
//! thirteenth level, RedTail's stakes ([`spikes`]), his wind ([`wind`])
//! and the debris it blows ([`debris`]), his knives ([`knife`]) and his
//! flares ([`flare`]). RedTail himself is an enemy
//! ([`super::enemy::redtail`]).
//!
//! The stakes and the wind are placed by the level; the knives, the
//! flares and the debris are made only by RedTail and his wind. The
//! knives, the flares and the debris move as the projectiles do
//! ([`crate::logic::projectile`]): each on its own clock ([`step_seconds`]),
//! from the tick after the one they are made in.

use crate::tuning::TICK_MS;

pub mod debris;
pub mod flare;
pub mod knife;
pub mod spikes;
pub mod wind;

/// The time the motion step of the tick starting at `now_ms` covers, s,
/// for an object that keeps its own clock in `clock_s`: one tick at
/// first, then the game time since its clock, read in seconds at the end
/// of the tick and added to the clock (as the projectiles' motion step
/// does).
pub fn step_seconds(clock_s: &mut Option<f64>, now_ms: u64) -> f64 {
    let now = (now_ms + u64::from(TICK_MS)) as f64 * 0.001;
    match *clock_s {
        Some(prev) => {
            let dt = now - prev;
            *clock_s = Some(prev + dt);
            dt
        }
        None => {
            *clock_s = Some(now);
            f64::from(TICK_MS) / 1000.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_starts_with_one_tick_and_follows_game_time() {
        let mut c = None;
        assert_eq!(step_seconds(&mut c, 1000), 0.018);
        let t = step_seconds(&mut c, 1018);
        assert!((t - 0.018).abs() < 1e-9);
        let t = step_seconds(&mut c, 1054);
        assert!((t - 0.036).abs() < 1e-9);
    }
}
