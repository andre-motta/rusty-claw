//! Claw's deaths, lives and powerups.
//!
//! # Dying
//!
//! Claw dies in two ways:
//!
//! - Losing all his health ([`DeathCause::Damage`]): he plays the falling
//!   death and, after hanging in the air for [`DEATH_FALL_HOLD_MS`] (his
//!   logic does not run meanwhile), drops through the floor and out of
//!   view (state [`state::DEATH_FALL`]), drawn in front of the level. Once
//!   he has fallen [`DEATH_FALL_DEPTH`] pixels the lives step runs.
//! - Touching a death tile ([`DeathCause::DeathTile`]): he plays the spike
//!   death in place (state [`state::DEATH`]) with the death-tile sound, and
//!   the lives step runs when the animation ends. On a level whose death
//!   tiles are a liquid (goo, water or lava, see
//!   [`crate::logic::enemy::goo::is_goo_level`]) he is not drawn: he
//!   disappears into a splash, raised by the level's splash height and
//!   just in front of him ([`liquid_splash`]); the animation still runs
//!   and times the lives step.
//!
//! Either way he stops reacting to anything (no collisions, safe from
//! attacks), drops what he carries, stops his attack and loses his powerup.
//!
//! With the level logics off a death tile only counts the touch (see
//! [`kill`]).
//!
//! The lives step runs once per death: a life is taken and the death is
//! counted in [`super::stats::PlayerStats::attempt`], then with lives
//! left the game reports [`GameEvent::ClawDied`] and the tick ends with
//! Claw dead. Claw respawns between that tick and the next, taking no game
//! time ([`respawn_between_ticks`], run by [`crate::Sim::step`] or by
//! [`crate::Sim::respawn_now`]): he is back at the last checkpoint (or the
//! level start) with full health, standing, and plays on from the next
//! tick. The circle that closes around the body ([`CLOSE_MS`], with the
//! circle sound), the black screen ([`HOLD_MS`]) and the circle that opens
//! on him ([`OPEN_MS`], with the flag sound at the level start) are drawn
//! by the presentation between the two ticks, outside game time: the
//! simulation reports them as its screen transition (the closing circle
//! from the lives step, the opening one from the respawn, gone at the
//! next tick), so recordings and traces have no ticks for them. With no
//! lives left the game reports [`GameEvent::GameOver`] and Claw stays
//! dead.
//!
//! A new game starts with 7 lives, so the 7th death ends it. With the
//! infinite lives aid on ([`crate::tuning::PlayAids::infinite_lives`]) the
//! lives step takes no life: the death is counted and Claw respawns with
//! the lives he had.
//!
//! Ending a life from the in-game menu ([`end_life`]) is the timer death:
//! the falling death from wherever Claw is, ladders included.
//!
//! # Powerups
//!
//! One powerup runs at a time ([`set_powerup`]). Taking the running one
//! again adds its time, up to [`POWERUP_MAX_MS`]; a different one replaces
//! it. Starting one switches the music to the powerup tune and asks for
//! its voice line (a Claw line through [`Ctx::say`], so it is refused while
//! another of his lines runs); when it ends the level music comes back. Catnip starts the
//! glitter that trails Claw; Vader makes him immune to enemies and every
//! kind of shot for as long as it lasts. [`powerup_tick`] counts the time
//! down after every state handler. Ghost and Vader also change how Claw
//! is drawn, see [`super::look`].

use claw_frame::{
    AudioCue, GameEvent, MusicTrack, Point, ScreenTransition, SoundChannel, TransitionKind,
};

use super::{Ctx, fallback_tick, progress, state, transition};
use crate::logic::effect;
use crate::logic::enemy::goo;
use crate::object::{ObjectId, draw_flags, flags, types};
use crate::physics::ptype;
use crate::tuning::TICK_MS;

/// Why Claw dies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeathCause {
    /// Health ran out.
    Damage,
    /// He touched a death tile (spikes, goo, lava).
    DeathTile,
    /// The checkpoint countdown ran out.
    Timer,
}

/// Powerup codes, as the pickups carry them.
pub mod powerup {
    /// Invisible to enemies.
    pub const GHOST: u32 = 9911;
    /// Fire sword.
    pub const FIRE_SWORD: u32 = 9912;
    /// Lightning sword.
    pub const LIGHTNING_SWORD: u32 = 9913;
    /// Ice sword.
    pub const ICE_SWORD: u32 = 9914;
    /// Plasma sword.
    pub const PLASMA_SWORD: u32 = 9915;
    /// Catnip: faster and stronger.
    pub const CATNIP: u32 = 9916;
    /// Invulnerable.
    pub const VADER: u32 = 9917;
}

/// Longest time a powerup can build up by being taken again, ms.
pub const POWERUP_MAX_MS: u32 = 999_000;

/// Logic name of the object that drives the catnip glitter.
pub const CATNIP_GLITTER_LOGIC: &str = "GlitterMother";

/// Pixels Claw falls after losing all health before the lives step.
pub const DEATH_FALL_DEPTH: i32 = 800;
/// How long Claw hangs where he lost his last health before the falling
/// death starts, ms: his logic waits this long, as any object's does when
/// its logic is delayed (41 ticks of 18 ms).
pub const DEATH_FALL_HOLD_MS: u32 = 750;
/// Gravity of the falling death, px/s².
pub const DEATH_FALL_GRAVITY: f64 = 2250.0;
/// Depth Claw is drawn at while he falls to his death: in front of the
/// level.
pub const DEATH_FALL_Z: i32 = 8500;

/// Length of the circle closing on Claw's body, ms (drawn outside game
/// time).
pub const CLOSE_MS: u32 = 1250;
/// Black screen between the closing and the opening circle, ms.
pub const HOLD_MS: u32 = 500;
/// Length of the circle opening on the respawned Claw, ms.
pub const OPEN_MS: u32 = 1000;
/// The closing circle's centre: this far ahead of Claw (behind when he
/// faces left) and below his position, px.
pub const CLOSE_CENTRE: (i32, i32) = (30, 55);
/// The opening circle's centre: this far above the respawned Claw, px.
pub const OPEN_CENTRE_RISE: i32 = 32;

/// Sound of the circle closing.
pub const CIRCLE_SOUND: &str = "GAME_CIRCLEFADE";
/// Sound of Claw coming back at the level start.
pub const FLAG_SOUND: &str = "GAME_FLAGWAVE";
/// Sound of a death tile.
pub const DEATH_TILE_SOUND: &str = "LEVEL_DEATHTILE";
/// Animation of the falling death.
pub const FALL_DEATH_ANIM: &str = "CLAW_FALLDEATH";
/// Animation of the death on a death tile.
pub const TILE_DEATH_ANIM: &str = "CLAW_SPIKEDEATH";

/// Bits of Claw's own [`crate::object::Object::special_flags`] used here.
pub mod claw_flags {
    /// The lives step of the current death has run.
    pub const LIVES_COUNTED: u32 = 0x4000_0000;
    /// No longer set: the respawn takes no game time. Kept so the bit
    /// stays reserved.
    pub const RESPAWNING: u32 = 0x2000_0000;
}

/// Ticks a transition of `ms` takes: whole ticks, rounded up.
pub const fn ticks_of(ms: u32) -> u64 {
    ms.div_ceil(TICK_MS) as u64
}

fn dying(state_id: i32) -> bool {
    state_id == state::DEATH_FALL || state_id == state::DEATH
}

/// Kills Claw.
///
/// A death tile is always counted in [`super::Claw::death_tile_hits`].
/// With the level logics off ([`Ctx::level_logics`]) that is all it does:
/// Claw carries on, as in the first playable version.
pub fn kill(c: &mut Ctx, cause: DeathCause) {
    if cause == DeathCause::DeathTile {
        c.p.death_tile_hits += 1;
        if !c.level_logics {
            return;
        }
    }
    if dying(c.o.state) {
        return;
    }
    match cause {
        DeathCause::Damage | DeathCause::Timer => kill_falling(c),
        DeathCause::DeathTile => kill_in_place(c),
    }
}

/// Ends Claw's life on request (the in-game menu's end life, through
/// [`crate::Sim::end_life`]): the timer death, when his health is above 0
/// and he is not already dying. He lets go of what he carries and falls
/// from where he is, whatever his state (a ladder included). Returns true
/// when he died.
pub fn end_life(c: &mut Ctx) -> bool {
    if c.p.stats.health <= 0 || dying(c.o.state) {
        return false;
    }
    kill(c, DeathCause::Timer);
    true
}

/// What every death shares: input, carried object and attack let go.
fn let_go(c: &mut Ctx) {
    if let Some(lifted) = c.p.special.lifted.take() {
        c.with_world(|w| super::special::tell_dropped(w, lifted));
    }
    super::attack::cancel(c);
    c.o.special_flags &= !(claw_flags::LIVES_COUNTED | claw_flags::RESPAWNING);
}

/// The falling death: out of reach, in front of the level, from rest,
/// after a hold of [`DEATH_FALL_HOLD_MS`]. The fractions of a pixel his
/// last move left are kept, so the fall starts from his exact height.
fn kill_falling(c: &mut Ctx) {
    let_go(c);
    c.o.state = state::DEATH_FALL;
    c.set_anim(FALL_DEATH_ANIM, true);
    c.o.flags &= !flags::ON_ELEVATOR;
    c.o.flags |= flags::NO_COLLIDE | flags::SAFE;
    c.o.physics_type = ptype::FALL;
    c.speeds_zero();
    if c.o.z != DEATH_FALL_Z {
        c.o.z = DEATH_FALL_Z;
        c.o.flags |= flags::RESORT;
    }
    c.p.stats.health = 0;
    c.p.fall_height = 0;
    c.p.mover.x.v = 0.0;
    c.p.mover.y.v = 0.0;
    c.p.mover.y.a = DEATH_FALL_GRAVITY;
    c.o.time_delay = DEATH_FALL_HOLD_MS;
    set_powerup(c, 0, 0);
}

/// The death on a death tile, in place.
fn kill_in_place(c: &mut Ctx) {
    let_go(c);
    c.o.state = state::DEATH;
    c.set_anim(TILE_DEATH_ANIM, true);
    c.o.physics_type = ptype::FALL;
    c.o.flags &= !flags::ON_ELEVATOR;
    c.o.flags |= flags::NO_COLLIDE | flags::SAFE;
    c.speeds_zero();
    c.p.stats.health = 0;
    if let Some((x, y)) = liquid_splash(c.level_number(), c.o.x, c.o.y) {
        c.o.draw_flags |= draw_flags::HIDDEN;
        c.with_world(|w| effect::spawn(w, effect::EffectKind::Splash, x, y));
    }
    c.play(DEATH_TILE_SOUND, SoundChannel::Effect);
    set_powerup(c, 0, 0);
}

/// Where the splash of Claw's death on a death tile at (x, y) goes on level
/// `number`: on a level whose death tiles are a liquid, at his position
/// raised by the level's splash height (the splash itself sits just in
/// front of him, see [`effect::spawn`]); `None` on a level of spikes.
pub fn liquid_splash(number: u32, x: i32, y: i32) -> Option<(i32, i32)> {
    goo::is_goo_level(number).then(|| (x, y - goo::splash_y(number).unwrap_or(0)))
}

/// Falling off the screen after losing all health (state 5004). Entered
/// without a death (health above 0) the state falls back to standing.
pub fn death_fall_tick(c: &mut Ctx) {
    if c.p.stats.health > 0 {
        fallback_tick(c);
        return;
    }
    if c.o.special_flags & claw_flags::LIVES_COUNTED != 0 {
        // The body stays where it is until the respawn between ticks (with
        // no lives left, for good).
        return;
    }
    c.advance_anim();
    c.p.mover.step(c.dt_s());
    let _ = c.p.mover.take_x();
    let dy = c.p.mover.take_y();
    c.o.y += dy;
    // The fall's depth reads the vertical step a second time, and that
    // read takes the step's fraction into the carried remainder again, so
    // the body falls a little faster than the step alone gives.
    let dh = c.p.mover.take_y();
    c.p.fall_height += dh + 1;
    if c.p.fall_height > DEATH_FALL_DEPTH {
        lives_step(c);
    }
}

/// Dying in place on a death tile (state 5005). Entered without a death
/// (health above 0) the state falls back to standing.
pub fn death_tick(c: &mut Ctx) {
    if c.p.stats.health > 0 {
        fallback_tick(c);
        return;
    }
    if c.o.special_flags & claw_flags::LIVES_COUNTED != 0 {
        // The body stays where it is until the respawn between ticks (with
        // no lives left, for good).
        return;
    }
    c.advance_anim();
    if c.o.anim.is_done() {
        lives_step(c);
    }
}

/// Takes a life and counts the death in the attempt counter, then asks for
/// the respawn between this tick and the next or ends the game. Runs once
/// per death.
fn lives_step(c: &mut Ctx) {
    c.o.special_flags |= claw_flags::LIVES_COUNTED;
    if !c.aids.infinite_lives {
        c.p.stats.lives -= 1;
    }
    c.p.stats.count_attempt();
    if c.p.stats.lives > 0 {
        c.out.event(GameEvent::ClawDied {
            lives_left: c.p.stats.lives,
        });
        let mirrored = c.o.draw_flags & crate::object::draw_flags::MIRROR != 0;
        let dx = if mirrored {
            -CLOSE_CENTRE.0
        } else {
            CLOSE_CENTRE.0
        };
        c.play(CIRCLE_SOUND, SoundChannel::Effect);
        c.out.set_transition(Some(ScreenTransition {
            kind: TransitionKind::Circle,
            closing: true,
            start_tick: c.tick,
            duration_ms: CLOSE_MS,
            center: Point {
                x: f64::from(c.o.x + dx),
                y: f64::from(c.o.y + CLOSE_CENTRE.1),
            },
        }));
        c.p.respawn_pending = true;
    } else {
        c.out.event(GameEvent::GameOver);
    }
}

/// The respawn after the lives step, between two ticks: Claw is back at
/// the checkpoint (or the level start) with full health, standing, and
/// the opening circle is reported for the presentation (see the [module
/// documentation](self)). [`crate::Sim`] then centres the camera on him,
/// puts the objects out of its reach to sleep, runs an update pass, wakes
/// the objects around him and runs a second update pass. Inferred: that
/// order is the one that follows the original's runs; the original's own
/// order of these steps is not known.
pub fn respawn_between_ticks(c: &mut Ctx) {
    c.p.respawn_pending = false;
    let at_level_start = c.p.checkpoint.is_none();
    transition(c, state::RESPAWN);
    c.o.flags |= flags::RESORT;
    set_powerup(c, 0, 0);
    if at_level_start {
        c.play(FLAG_SOUND, SoundChannel::Effect);
    }
    c.o.special_flags &= !(claw_flags::LIVES_COUNTED | claw_flags::RESPAWNING);
    c.o.time_delay = 0;
    c.out.set_transition(Some(ScreenTransition {
        kind: TransitionKind::Circle,
        closing: false,
        start_tick: c.tick,
        duration_ms: OPEN_MS,
        center: Point {
            x: f64::from(c.o.x),
            y: f64::from(c.o.y - OPEN_CENTRE_RISE),
        },
    }));
}

/// Starts powerup `kind` for `ms` milliseconds, or ends the running one
/// with `kind` or `ms` 0.
///
/// The same powerup again adds `ms` to its time, up to
/// [`POWERUP_MAX_MS`]. A different one replaces it: the old one's effect
/// ends, the new one starts and asks for its voice line, and the music switches to
/// the powerup tune. Ending a powerup brings the level music back. While
/// the boss fight flag is set (from a boss introduction's start, see
/// [`crate::logic::world_boss`]) the music is left alone, so the boss
/// track plays on.
pub fn set_powerup(c: &mut Ctx, kind: u32, ms: u32) {
    let current = c.p.stats.powerup_kind;
    if c.out.boss.fight() == crate::logic::world_boss::fight::NONE {
        if kind != 0 && ms != 0 {
            if kind != current {
                c.out.cue(AudioCue::Music {
                    track: MusicTrack::Powerup,
                    looped: true,
                });
            }
        } else if current != 0 {
            c.out.cue(AudioCue::Music {
                track: MusicTrack::Level,
                looped: true,
            });
        }
    }
    if kind == current {
        c.p.stats.powerup_ms = c.p.stats.powerup_ms.saturating_add(ms).min(POWERUP_MAX_MS);
        return;
    }
    match current {
        powerup::GHOST => c.p.look.set_normal(),
        powerup::CATNIP => end_glitter(c),
        powerup::VADER => {
            c.o.hit_type_flags |= types::ENEMY | types::ENEMY_SHOT | types::LEVEL_SHOT;
            c.p.look.set_normal();
        }
        _ => {}
    }
    c.p.stats.powerup_kind = kind;
    c.p.stats.powerup_ms = ms;
    let voice = match kind {
        powerup::GHOST | powerup::VADER => Some("CLAW_1110064"),
        powerup::FIRE_SWORD => Some("CLAW_1110001"),
        powerup::LIGHTNING_SWORD => Some("CLAW_1110022"),
        powerup::ICE_SWORD => Some("CLAW_1110012"),
        _ => None,
    };
    if let Some(v) = voice {
        c.say(v, false);
    }
    if kind == powerup::CATNIP {
        let id = c.id;
        c.with_world(|w| effect::spawn_glitter(w, id));
    }
}

/// Removes the catnip glitter.
fn end_glitter(c: &mut Ctx) {
    let glitter: Vec<ObjectId> = c
        .objects
        .all()
        .filter(|(_, o)| &*o.logic == CATNIP_GLITTER_LOGIC)
        .map(|(id, _)| id)
        .collect();
    for id in glitter {
        if let Some(o) = c.objects.get_mut(id) {
            o.flags |= flags::DESTROY;
        }
    }
}

/// Runs after every state handler: counts the running powerup down and
/// applies its per-tick effect (Claw's look for Ghost and Vader, see
/// [`super::look`]) and awards extra lives from the score.
pub fn powerup_tick(c: &mut Ctx) {
    progress::extra_life_tick(c);
    let dt = c.dt_ms;
    let s = &mut c.p.stats;
    s.powerup_ms = s.powerup_ms.saturating_sub(dt);
    if s.powerup_ms == 0 {
        if s.powerup_kind != 0 {
            set_powerup(c, 0, 0);
        }
        return;
    }
    match s.powerup_kind {
        powerup::GHOST => c.p.look.set_ghost(),
        powerup::VADER => {
            c.p.look.silhouette_tick(dt);
            c.o.hit_type_flags &= !(types::ENEMY | types::ENEMY_SHOT | types::LEVEL_SHOT);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_lengths_in_ticks() {
        assert_eq!(ticks_of(CLOSE_MS), 70);
        assert_eq!(ticks_of(HOLD_MS), 28);
        assert_eq!(ticks_of(OPEN_MS), 56);
        assert_eq!(ticks_of(0), 0);
    }
}
