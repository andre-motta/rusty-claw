//! The Siren's scream ([`crate::logic::enemy::siren`]): a slow ring of
//! sound flying straight ahead at [`SPEED`] px/s without falling, gone at
//! a wall or when its animation ends. It deals no damage: when it touches
//! Claw, crouching or not, it plays his hit sound, sets the scream
//! lockout to [`LOCK_MS`] (set, not added) and is gone. Crates let it
//! through.
//!
//! # The scream lockout
//!
//! One counter of the game, not of Claw ([`World::scream_lock_ms`], read
//! from outside through [`crate::Sim::scream_lock_ms`]): 0 when the game
//! is created and at every level load, kept across Claw's deaths and
//! respawns. The simulation hands it three places where held input
//! reaches Claw, all in this file, so the lockout lives here:
//!
//! - [`lock_input`], at the start of every tick, before any logic, with
//!   the keys Claw's logic would see: while the counter runs down it
//!   lowers it by the tick's length and, while it is still above 0,
//!   clears the keys; on the tick it reaches 0 every key held counts as
//!   pressed ([`LOCK_TICKS`] locked ticks after the last scream);
//! - [`hit_claw`], when a scream touches Claw in the collision pass,
//!   before the shared shot rule: true when the scream was handled there;
//! - [`after_tick`], at the end of every tick, on Claw's object: while the
//!   counter is above 0 he shows his dazed picture ([`DAZED_FRAME`]) until
//!   his animation applies its next element.
//!
//! The input itself ([`claw_frame::TickInput`]), routes and recordings
//! keep the raw keys; the lockout is applied from them. Pause, the help
//! screen and the menus are not locked, and as no tick runs while they
//! are up the counter stands still.
//!
//! It flies as the projectiles do ([`crate::logic::projectile`]): the
//! projectile family sets it up through [`set_up`] and runs each of its
//! calls through [`step`]; whatever a scream keeps beyond the
//! projectiles' data is its [`Fields`].
//!
//! Not built yet ([`BUILT`] is false): a scream asked for is an inert
//! placeholder holding the request, the three calls above change nothing
//! and the counter stays at 0.
//!
//! # Defaults taken
//!
//! - Claw's dazed picture is his current frame for every other logic too,
//!   as the original's frame field is: after every tick with the counter
//!   above 0 his frame reads [`DAZED_FRAME`] until his animation applies
//!   its next element.

use claw_frame::TickInput;

use super::{ProjectileData, ProjectileSpec, store};
use crate::logic::World;
use crate::object::{Object, ObjectId, Rect};
use crate::player::Ctx;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["SirenBullet"];

/// The scream's pictures.
pub const IMAGE_SET: &str = "LEVEL_SIRENPROJECTILE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// The scream's flight.
pub const ANIM: &str = "LEVEL_SIREN_PROJECTILE";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Speed, px/s.
pub const SPEED: i32 = 200;
/// The scream's movement and attack box, flying right; turned to face
/// left for a scream flying left.
pub const FRAME_BOX: Rect = Rect::new(-2, -5, 5, 4);
/// The points a scream carries, which make Claw's hit rule lock his
/// controls instead of hurting him.
pub const POINTS: i32 = 9919;
/// The lockout a scream sets, ms.
pub const LOCK_MS: u32 = 2000;
/// Ticks the lockout locks after the last scream at the 18 ms step:
/// 2000 ms down to 2 in 18 ms steps (measured); the next tick is free.
pub const LOCK_TICKS: u32 = 111;
/// Claw's dazed picture while locked.
pub const DAZED_FRAME: i32 = 100;
/// Ticks of a scream's flight before its animation ends at the 18 ms
/// step.
pub const LIFE_TICKS: u32 = 85;

/// What a scream keeps beyond the projectiles' data (see
/// [`ProjectileData::scream`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields {}

/// A scream's data. A scream keeps the projectiles' data
/// ([`ProjectileData`]); this is not used.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScreamData {}

impl World<'_> {
    /// The scream lockout, ms: 0 while Claw's controls are free (see the
    /// module documentation).
    pub fn scream_lock_ms(&self) -> u32 {
        self.out.scream_lock_ms
    }

    /// Sets the scream lockout to `ms`.
    pub fn set_scream_lock_ms(&mut self, ms: u32) {
        self.out.scream_lock_ms = ms;
    }
}

/// Sets a new scream up from `spec` (the projectiles' spawn calls it after
/// the shared set-up).
pub(super) fn set_up(o: &mut Object, d: &mut ProjectileData, spec: &ProjectileSpec) {
    let _ = (o, d, spec);
}

/// One call of a scream.
pub(super) fn step(w: &mut World, id: ObjectId, d: ProjectileData) {
    store(w, id, d);
}

/// The input step of the lockout: `lock_ms` is the counter, `dt_ms` the
/// tick's length and `claw_input` the keys Claw's logic sees this tick
/// (see the module documentation).
pub fn lock_input(lock_ms: &mut u32, dt_ms: u32, claw_input: &mut TickInput) {
    let _ = (lock_ms, dt_ms, claw_input);
}

/// The scream `scream` touched Claw: true when it was handled here and
/// the shared shot rule must not run.
pub fn hit_claw(c: &mut Ctx, scream: ObjectId) -> bool {
    let _ = (c, scream);
    false
}

/// The end of a tick with the counter at `lock_ms`, on Claw's object.
pub fn after_tick(lock_ms: u32, claw: &mut Object) {
    let _ = (lock_ms, claw);
}

/// One logic call: the projectiles' ([`super::tick`]).
pub fn tick(w: &mut World, id: ObjectId) {
    super::tick(w, id);
}

/// `attacker` hit `id`. Nothing hits a scream.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. The victim's own hit handler decides.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::enemy::siren;

    #[test]
    fn the_lockout_is_two_seconds() {
        assert_eq!(SPEED, siren::SCREAM_SPEED);
        // 2000 ms run down 18 ms a tick: still above 0 after 111 ticks,
        // 0 on the 112th.
        assert_eq!(LOCK_MS - LOCK_TICKS * 18, 2);
        assert_eq!(LOCK_MS.div_ceil(18), LOCK_TICKS + 1);
        // Pictures 2 to 8 on calls 1, 13, ..., 73, each held 200 ms (12
        // calls), and the finishing element: the 85th call ends the
        // flight.
        assert_eq!(1 + 7 * 200u32.div_ceil(18), LIFE_TICKS);
    }

    #[test]
    fn unbuilt_the_lockout_changes_nothing() {
        let mut lock = LOCK_MS;
        let mut input = TickInput::default();
        input.held.0 = 0x1;
        input.pressed.0 = 0x1;
        let before = input;
        lock_input(&mut lock, 18, &mut input);
        if !BUILT {
            assert_eq!((lock, input), (LOCK_MS, before));
        }
    }
}
