//! The Siren's scream ([`crate::logic::enemy::siren`]): a slow ring of
//! sound flying straight ahead at [`SPEED`] px/s without falling, gone at
//! a wall or when its animation ends ([`LIFE_TICKS`] ticks). It deals no
//! damage: when it touches Claw, crouching or not, it plays his hit sound,
//! sets the scream lockout to [`LOCK_MS`] (set, not added) and is gone;
//! nothing else of a shot's hit happens (no health lost, no knockback, no
//! window in which he cannot be hurt). Enemies are not hit by it and
//! crates let it through.
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
//!   the keys Claw's logic would see: while the counter runs it is lowered
//!   by the tick's length and, while it is still above 0, the keys are
//!   taken away (nothing held, nothing pressed) and the latch of keys
//!   already counted as pressed is emptied; on the tick it reaches 0 every
//!   key held counts as pressed. A scream on tick H so locks ticks H + 1
//!   to H + [`LOCK_TICKS`], and on the next tick a key held all along acts
//!   at once (a held jump jumps).
//! - [`hit_claw`], when a scream touches Claw in the collision pass,
//!   before the shared shot rule: true when the scream was handled there.
//!   The tick of the hit keeps its own keys.
//! - [`after_tick`], at the end of every tick, on Claw's object: while the
//!   counter is above 0 he shows his dazed picture ([`DAZED_FRAME`]),
//!   which stays until his animation applies its next element.
//!
//! While locked, Claw does only what needs no key: a walk stops, a crouch
//! rises, a rising jump loses its hold and he falls, an attack under way
//! runs to its end; he can be hit and killed. The input itself
//! ([`claw_frame::TickInput`]), routes and recordings keep the raw keys;
//! the lockout is applied from them. The trace ([`crate::trace`]) writes
//! what the game's controls saw instead: nothing held on a locked tick
//! ([`crate::Sim::input_locked`]). Pause, the help screen and the menus
//! are not locked, and as no tick runs while they are up the counter
//! stands still; the tick that resumes play is a whole tick and lowers it
//! as any other. The respawn's passes between two ticks do not lower it.
//!
//! It flies as the projectiles do ([`crate::logic::projectile`]): the
//! projectile family sets it up through [`set_up`] and runs each of its
//! calls through [`step`]; whatever a scream keeps beyond the
//! projectiles' data is its [`Fields`].
//!
//! # Defaults taken
//!
//! - Claw's dazed picture is his current frame for every other logic too:
//!   after every tick with the counter above 0 his frame reads
//!   [`DAZED_FRAME`] until his animation applies its next element.
//! - With the invincibility play-test aid on, a scream flies through Claw
//!   as every shot does, and locks nothing.

use claw_frame::{Buttons, SoundChannel, TickInput};

use super::{Class, ProjectileData, ProjectileSpec};
use crate::logic::World;
use crate::object::{Object, ObjectId, Rect, draw_flags, flags, types};
use crate::player::Ctx;
use crate::player::hit::SHOT_SOUND;

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
pub const BUILT: bool = true;

/// Speed, px/s.
pub const SPEED: i32 = 200;
/// The scream's movement and attack box, flying right; turned to face
/// left for a scream flying left.
pub const FRAME_BOX: Rect = Rect::new(-2, -5, 5, 4);
/// The points value that marks a shot as a scream in the original game;
/// this game tells a scream by its class ([`is_scream`]), which makes
/// Claw's hit rule lock his controls instead of hurting him.
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
/// the shared set-up): an enemy's shot that deals nothing and that
/// nothing hits; one flying left turns its picture and boxes.
pub(super) fn set_up(o: &mut Object, d: &mut ProjectileData, spec: &ProjectileSpec) {
    let _ = d;
    o.type_flags = types::ENEMY_SHOT;
    o.hit_type_flags = 0;
    o.damage = 0;
    if spec.vx < 0 {
        o.draw_flags |= draw_flags::MIRROR;
        o.mirror_rects();
    }
}

/// One call of a scream: the straight shots' call (its animation, gone at
/// a wall or outside the level, its move), then gone once its animation
/// has ended. Without its animation it flies until a wall or the edge of
/// the window stops it.
pub(super) fn step(w: &mut World, id: ObjectId, d: ProjectileData) {
    super::shot_tick(w, id, d, Class::SirenBullet);
    if w.objects
        .get(id)
        .is_some_and(|o| o.anim.animation().is_some() && o.anim.is_done())
    {
        w.destroy(id);
    }
}

/// Whether `id` is a scream.
pub fn is_scream(o: &Object) -> bool {
    super::data(o).is_some_and(|d| d.class == Some(Class::SirenBullet))
}

/// The input step of the lockout: `lock_ms` is the counter, `dt_ms` the
/// tick's length and `claw_input` the keys Claw's logic sees this tick
/// (see the module documentation). True when the tick is locked: its keys
/// were taken away, and the caller empties the latch of keys already
/// counted as pressed.
pub fn lock_input(lock_ms: &mut u32, dt_ms: u32, claw_input: &mut TickInput) -> bool {
    if !BUILT || *lock_ms == 0 {
        return false;
    }
    *lock_ms = lock_ms.saturating_sub(dt_ms);
    if *lock_ms > 0 {
        claw_input.held = Buttons::NONE;
        claw_input.pressed = Buttons::NONE;
        true
    } else {
        // The latch was emptied by the locked ticks: every key held now
        // counts as pressed.
        claw_input.pressed = claw_input.held;
        false
    }
}

/// The scream `scream` touched Claw: true when it was handled here and
/// the shared shot rule must not run. His hit sound plays, the lockout is
/// set to [`LOCK_MS`] and the scream is gone; nothing else happens.
pub fn hit_claw(c: &mut Ctx, scream: ObjectId) -> bool {
    if !BUILT || !c.objects.get(scream).is_some_and(is_scream) {
        return false;
    }
    c.play(SHOT_SOUND, SoundChannel::Effect);
    c.out.scream_lock_ms = LOCK_MS;
    if let Some(o) = c.objects.get_mut(scream) {
        o.flags |= flags::DESTROY;
    }
    true
}

/// The end of a tick with the counter at `lock_ms`, on Claw's object:
/// while it is above 0 he shows his dazed picture.
pub fn after_tick(lock_ms: u32, claw: &mut Object) {
    if BUILT && lock_ms > 0 {
        claw.frame = DAZED_FRAME;
    }
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
    fn the_lockout_takes_the_keys_until_it_runs_out() {
        let keys = TickInput {
            held: Buttons(0x1),
            pressed: Buttons::NONE,
        };
        let mut lock = LOCK_MS;
        for k in 1..=LOCK_TICKS {
            let mut input = keys;
            assert!(lock_input(&mut lock, 18, &mut input), "tick {k}");
            assert_eq!(input, TickInput::default(), "tick {k}");
        }
        assert_eq!(lock, 2);
        // The tick it reaches 0: the held key counts as pressed.
        let mut input = keys;
        assert!(!lock_input(&mut lock, 18, &mut input));
        assert_eq!(
            (lock, input.held, input.pressed),
            (0, Buttons(0x1), Buttons(0x1))
        );
        // Then the keys are left alone.
        let mut input = keys;
        assert!(!lock_input(&mut lock, 18, &mut input));
        assert_eq!(input, keys);
    }

    #[test]
    fn claw_is_dazed_while_locked() {
        let mut o = Object::new(
            0,
            0,
            0,
            0,
            std::sync::Arc::from("CaptainClaw"),
            crate::object::LogicKind::Inert,
        );
        o.frame = 17;
        after_tick(0, &mut o);
        assert_eq!(o.frame, 17);
        after_tick(2, &mut o);
        assert_eq!(o.frame, DAZED_FRAME);
    }
}
