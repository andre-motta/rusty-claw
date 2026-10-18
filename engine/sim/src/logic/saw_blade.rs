//! Saw blades: round saws set into slots in the ninth level's decks that
//! rise, spin and sink again forever, on a rhythm fixed by the level
//! clock.
//!
//! A saw is a floor spike ([`super::floor_spike`]) with other art and a
//! few differences. The names differ only in their phase on the clock:
//! `SawBlade` starts its cycle at clock 0 (or at the record's speed field,
//! in ms), `SawBlade2` 750 ms later, `SawBlade3` 1500 ms later and
//! `SawBlade4` (which no level places) 2250 ms later ([`phase_of`]); every
//! saw of the ninth level is up for 1500 ms and down for 1500 ms.
//!
//! - Its strike box is the box of its retracted picture narrowed by 20 px
//!   on each side and raised by 20 px ([`strike_box`]): (-33, -10, 34, 17)
//!   on the ninth level, fixed when it is set up.
//! - It turns dangerous when its rise ([`UP_ANIM`], 31 ticks) ends, 32
//!   ticks after the clock enters its up half, and harmless on the tick
//!   after the clock leaves it; while up it keeps spinning ([`SPIN_ANIM`]),
//!   and its sink ([`DOWN_ANIM`]) takes 28 ticks. A saw woken inside its
//!   up half rises at once, so it may be dangerous for only two ticks.
//! - Its hit costs Claw [`DAMAGE`] as a hazard's shot (so at most twice per
//!   up half, his protection lasting 850 ms) and reaches enemies, kegs and
//!   crates the same way.
//! - Its rise and sink sounds ([`RISE_SOUND`], [`SINK_SOUND`]) are not on
//!   the ninth level: nothing plays and nothing is reported (the hum comes
//!   from sound objects beside the saws).
//!
//! It draws no random number.
//!
//! Not built yet ([`BUILT`] is false): saws stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.
//!
//! # State numbers
//!
//! A saw is no enemy: it runs the floor spikes' states ([`state`]) with
//! their numbers.
//!
//! # Defaults taken
//!
//! - A saw plays its rise and sink sounds only when the level has them, so
//!   on the ninth level nothing is played and nothing is reported.
//! - The hook-handed pirates and the PegLeg whose patrols end beside a saw
//!   get no special case: the saw's hit reaches them whenever its strike
//!   box overlaps their hit box.
//!
//! Its data ([`SawData`]) is its own.

pub use super::floor_spike::state;

use super::World;
use crate::level::SimLevel;
use crate::logic::Look;
use crate::object::{Object, ObjectId, Rect, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["SawBlade", "SawBlade2", "SawBlade3", "SawBlade4"];

/// The saws' picture on the ninth level (the records name it).
pub const IMAGE_SET: &str = "LEVEL_FLOORSAW";

/// Image sets this family may switch an object to: none (the saws keep
/// the set their records name).
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SAWBLADE_"];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// The rise.
pub const UP_ANIM: &str = "LEVEL_SAWBLADE_UP";
/// The spin while up.
pub const SPIN_ANIM: &str = "LEVEL_SAWBLADE_SPIN";
/// The sink.
pub const DOWN_ANIM: &str = "LEVEL_SAWBLADE_DOWN";
/// The rise's sound (no level has it).
pub const RISE_SOUND: &str = "LEVEL_SAWBLADEUP";
/// The sink's sound (no level has it).
pub const SINK_SOUND: &str = "LEVEL_SAWBLADEDOWN";

/// What a saw is to whatever it touches: a hazard's shot of the level.
pub const TYPE: u32 = types::LEVEL_SHOT;
/// Damage of a saw placed without any.
pub const DAMAGE: i32 = 10;
/// Depth of a saw placed at depth 0.
pub const DEFAULT_Z: i32 = 2000;
/// Length of each half of the cycle when the record gives none, ms.
pub const DEFAULT_HALF_MS: i32 = 1500;
/// How much the strike box is narrowed on each side and raised, px.
pub const SHRINK: i32 = 20;
/// Ticks from the first tick the clock is in the up half to the first
/// dangerous tick (measured).
pub const DANGER_DELAY_TICKS: u32 = 32;
/// Ticks of the rise at the 18 ms step.
pub const RISE_TICKS: u32 = 31;
/// Ticks of the sink at the 18 ms step.
pub const SINK_TICKS: u32 = 28;

/// The cycle offset of the saw running `name`, ms: `SawBlade` takes
/// `record_speed`, the others a quarter, half and three quarters of the
/// default cycle; `None` for a name no saw runs.
pub fn phase_of(name: &str, record_speed: i32) -> Option<i32> {
    match name {
        "SawBlade" => Some(record_speed),
        "SawBlade2" => Some(750),
        "SawBlade3" => Some(1500),
        "SawBlade4" => Some(2250),
        _ => None,
    }
}

/// The strike box of a saw whose retracted picture covers `picture`: 20 px
/// narrower on each side and 20 px taller at the top.
pub fn strike_box(picture: Rect) -> Rect {
    Rect::new(
        picture.left + SHRINK,
        picture.top - SHRINK,
        picture.right - SHRINK,
        picture.bottom,
    )
}

/// A saw's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SawData {
    /// Clock offset of the cycle, ms.
    pub phase: i32,
    /// Length of the up half, ms.
    pub on_ms: i32,
    /// Length of the whole cycle, ms.
    pub period: i32,
}

/// How a saw whose first call has not run yet looks once it has (see
/// [`super::first_look`]): `None`, as it looks now.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    let _ = (level, o);
    None
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// `attacker` hit `id`. Nothing hits a saw.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strike_box_of_the_ninth_levels_saws() {
        // The retracted picture: 108 x 9 px at (1, 14).
        let picture = Rect::new(-53, 10, 54, 17);
        assert_eq!(strike_box(picture), Rect::new(-33, -10, 34, 17));
    }

    #[test]
    fn the_names_differ_only_in_their_phase() {
        let phases: Vec<_> = NAMES.iter().map(|n| phase_of(n, 0)).collect();
        assert_eq!(phases, [Some(0), Some(750), Some(1500), Some(2250)]);
        assert_eq!(phase_of("SawBlade", 300), Some(300));
        assert_eq!(phase_of("FloorSpike", 0), None);
    }

    #[test]
    fn its_timings_at_eighteen_milliseconds() {
        assert_eq!(DANGER_DELAY_TICKS, RISE_TICKS + 1);
        // The rise and the sink fit each half of the cycle.
        let half = (DEFAULT_HALF_MS as u32).div_ceil(18);
        assert!(RISE_TICKS.max(SINK_TICKS) < half);
        // The floor spikes' states.
        assert_eq!(state::DOWN, 1007);
        assert_eq!(state::RISE_START, 1002);
    }
}
