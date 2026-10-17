//! The shared enemy states, and the moves into them.
//!
//! Every enemy: Strike, Pause, Falling, Dead and walking (MoveLeft,
//! MoveRight). Human enemies also: Home, Advance, FastAdvance, Pace, the
//! hit reactions (HitLow, HitHigh), the leap at Claw (JumpAtClaw) and the
//! hop up (Jump), plus the states of [`super::human`] (backing off,
//! stepping in, the hop back, blocks, the human idle state, the
//! springboard launch and the leap over Claw). Being lifted and thrown is
//! in [`super::lift`].
//!
//! Patrol limits are compared for equality with the position: the
//! movement step clamps a walker onto a limit it would cross, so it stands
//! exactly on it on the tick it arrives.
//!
//! A walker turns at a limit with the speed it had, reversed. A wall the
//! movement step runs into also reverses its speed (at half) and moves the
//! limit to where it stood; turning that already reversed speed again
//! would point it back into the wall and hold it on the limit for good.
//! So a walker whose speed no longer leads away from the limit turns at
//! the walking speed instead: it walks back and forth between walls as it
//! does between ledges.

use crate::logic::World;
use crate::object::{ObjectId, Rect};
use crate::physics::ptype;

use super::{
    Class, DeathKind, EnemyState, HIT_SOUNDS, Handler, Timer, UNBOUNDED, class_of, data, data_mut,
    face_claw, facing_rect, field, give_strike_token, human, lift, obj, obj_mut, on_screen, play,
    rand, sees_claw, set_anim, set_facing, set_state, take_strike_token,
};
use crate::object::flags;

/// Walking speed toward Claw, px/s.
pub const ADVANCE_SPEED: f64 = 60.0;
/// Running speed toward Claw, px/s.
pub const FAST_ADVANCE_SPEED: f64 = 120.0;
/// Walking speed of a walker turning away from a wall, px/s (the rat's
/// walking speed).
pub const WALK_SPEED: f64 = 60.0;
/// Pacing speed, px/s.
pub const PACE_SPEED: f64 = 120.0;
/// Knockback speed of a hit, px/s.
pub const KNOCKBACK: f64 = 200.0;
/// Knockback slowdown per tick, px/s.
pub const KNOCKBACK_STEP: f64 = 25.0;
/// Claw within this distance (exclusive) makes an enemy on a narrow
/// ledge leap at him, px.
pub const LEAP_REACH: i64 = 250;
/// Shortest and longest pause at Home, ms (from `PAUSE_MIN` to
/// `PAUSE_MIN + PAUSE_SPREAD - 1`).
pub const PAUSE_MIN: u64 = 300;
/// See [`PAUSE_MIN`].
pub const PAUSE_SPREAD: u32 = 301;

/// The shared handler of a state, for a human enemy or not. A human class
/// without an idle handler of its own idles as [`human::idle_tick`] says.
pub fn shared_handler(s: EnemyState, human: bool) -> Option<Handler> {
    let common: Option<Handler> = match s {
        EnemyState::Strike => Some(strike_tick),
        EnemyState::Pause => Some(pause_tick),
        EnemyState::Falling => Some(falling_tick),
        EnemyState::Dead => Some(dead_tick),
        EnemyState::MoveLeft => Some(walk_left_tick),
        EnemyState::MoveRight => Some(walk_right_tick),
        EnemyState::Lifted => Some(lift::lifted_tick),
        EnemyState::Flailing => Some(lift::flailing_tick),
        EnemyState::Dropped => Some(lift::dropped_tick),
        _ => None,
    };
    if common.is_some() || !human {
        return common;
    }
    match s {
        EnemyState::Home => Some(human_home_tick),
        EnemyState::Advance => Some(advance_tick),
        EnemyState::FastAdvance => Some(fast_advance_tick),
        EnemyState::Pace => Some(pace_tick),
        EnemyState::HitLow | EnemyState::HitHigh => Some(hit_tick),
        EnemyState::JumpAtClaw => Some(jump_at_claw_tick),
        EnemyState::Jump => Some(jump_tick),
        EnemyState::Thrown => Some(lift::thrown_tick),
        EnemyState::Bounce => Some(lift::bounce_tick),
        _ => human::handler(s),
    }
}

fn go_idle(w: &mut World, id: ObjectId) {
    if let Some(c) = class_of(w, id) {
        (c.go_idle)(w, id);
    }
}

fn class_go_home(w: &mut World, id: ObjectId) {
    if let Some(c) = class_of(w, id) {
        (c.go_home)(w, id);
    }
}

fn bounds(w: &World, id: ObjectId) -> (i32, i32) {
    data(w, id).map_or(UNBOUNDED, |d| d.bounds)
}

fn mode(w: &World, id: ObjectId) -> u8 {
    data(w, id).map_or(6, |d| d.mode)
}

fn claw_right(w: &World, id: ObjectId) -> bool {
    data(w, id).is_some_and(|d| d.claw_right)
}

fn set_vx(w: &mut World, id: ObjectId, vx: f64) {
    if let Some(d) = data_mut(w, id) {
        d.vx = vx;
    }
}

fn set_busy(w: &mut World, id: ObjectId, busy: bool) {
    if let Some(d) = data_mut(w, id) {
        d.busy = busy;
    }
}

/// `speed` toward Claw.
fn toward_claw(w: &World, id: ObjectId, speed: f64) -> f64 {
    if claw_right(w, id) { speed } else { -speed }
}

// ----------------------------------------------------------------------
// Home

/// Goes Home (human enemies): stops and decides again in Home's handler.
/// It leaves the busy flag as it is.
pub fn go_home(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
    }
    set_state(w, id, EnemyState::Home);
}

/// Home of human enemies (the soldier's when it neither strikes nor
/// fires): nothing unless the enemy is still at Home; otherwise it faces
/// Claw and, by mode, runs at him (0, 1), walks at him (2), strikes or
/// pauses 300 to 600 ms (3, 4: it strikes when the draw is below its
/// smarts) or paces (5, 6). Draws: one of 0..421, then one of 0..301
/// before a pause.
pub fn human_home_tick(w: &mut World, id: ObjectId) {
    if super::state(w, id) != Some(EnemyState::Home) {
        return;
    }
    face_claw(w, id);
    let r = rand(w, 421);
    match mode(w, id) {
        0 | 1 => enter_fast_advance(w, id),
        2 => enter_advance(w, id),
        3 | 4 => {
            let smarts = data(w, id).map_or(0, |d| d.smarts);
            if r < smarts {
                enter_strike(w, id);
            } else {
                let home = class_of(w, id).map_or("", |c| c.anims.home);
                super::set_anim_now(w, id, home);
                let ms = PAUSE_MIN + rand(w, PAUSE_SPREAD) as u64;
                enter_pause(w, id, ms);
            }
        }
        5 | 6 => start_pacing(w, id),
        _ => {}
    }
}

// ----------------------------------------------------------------------
// Strike

/// Starts a strike: busy, the strike animation in use from its start.
pub fn enter_strike(w: &mut World, id: ObjectId) {
    let anim = data(w, id).map_or("", |d| d.strike_anim);
    set_busy(w, id, true);
    set_state(w, id, EnemyState::Strike);
    set_anim(w, id, anim);
}

/// Strike: on the animation's strike frame, when the level's strike token
/// is free, the enemy takes it, the strike box becomes its attack box for
/// that tick and the strike sound plays (the enemy's own when it has one,
/// see [`human::HumanSounds::strike`], otherwise its class's); when the
/// animation ends the token is given back and the enemy goes Home, free
/// again.
pub fn strike_tick(w: &mut World, id: ObjectId) {
    let event = w.advance_anim(id);
    let Some(class) = class_of(w, id) else { return };
    if event == 1 && take_strike_token(w, id) {
        let own = data(w, id).map_or("", |d| d.human.sounds.strike);
        play(
            w,
            if own.is_empty() {
                class.sounds.strike
            } else {
                own
            },
        );
        let rect = data(w, id).map_or(Rect::UNSET, |d| d.strike_rect);
        let o = obj_mut(w, id);
        o.attack_rect = facing_rect(o, rect);
    }
    if obj(w, id).anim.is_done() {
        give_strike_token(w, id);
        (class.go_home)(w, id);
        set_busy(w, id, false);
    }
}

// ----------------------------------------------------------------------
// Walking

/// Starts pacing (human enemies): a stationary enemy goes idle instead;
/// otherwise it walks left at 120 px/s from its right limit and right
/// from anywhere else, facing the way it walks.
pub fn start_pacing(w: &mut World, id: ObjectId) {
    let Some(class) = class_of(w, id) else { return };
    let (stationary, max) = data(w, id).map_or((true, 0), |d| (d.stationary, d.bounds.1));
    if stationary {
        (class.go_idle)(w, id);
        return;
    }
    let right = obj(w, id).x != max;
    set_vx(w, id, if right { PACE_SPEED } else { -PACE_SPEED });
    set_facing(w, id, right);
    set_state(w, id, EnemyState::Pace);
    set_anim(w, id, class.anims.pace);
}

/// Pace: at the left limit, even odds of going idle or turning right;
/// then the same at the right limit. A sight-driven enemy goes Home when
/// it sees Claw; any other goes Home when Claw is in range (modes 0 to
/// 4). Draws: one of 0..100 per limit reached.
pub fn pace_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let (min, max) = bounds(w, id);
    if obj(w, id).x == min {
        if rand(w, 100) < 50 {
            go_idle(w, id);
        } else {
            set_vx(w, id, PACE_SPEED);
            set_facing(w, id, true);
        }
    }
    if obj(w, id).x == max {
        if rand(w, 100) < 50 {
            go_idle(w, id);
        } else {
            set_vx(w, id, -PACE_SPEED);
            set_facing(w, id, false);
        }
    }
    if data(w, id).is_some_and(|d| d.sight) {
        if sees_claw(w, id) {
            class_go_home(w, id);
        }
        return;
    }
    if mode(w, id) <= 4 {
        class_go_home(w, id);
    }
}

/// Starts walking toward Claw at 60 px/s, facing him.
pub fn enter_advance(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let vx = toward_claw(w, id, ADVANCE_SPEED);
    set_vx(w, id, vx);
    set_state(w, id, EnemyState::Advance);
    let anim = class_of(w, id).map_or("", |c| c.anims.advance);
    set_anim(w, id, anim);
}

/// Advance: at either limit it goes idle (and carries on); it faces Claw
/// and walks at him at 60 px/s; then by mode it runs (0), keeps walking
/// (1, 2) or goes Home (3 to 6).
pub fn advance_tick(w: &mut World, id: ObjectId) {
    let (min, max) = bounds(w, id);
    let x = obj(w, id).x;
    if x == min || x == max {
        go_idle(w, id);
    }
    face_claw(w, id);
    let vx = toward_claw(w, id, ADVANCE_SPEED);
    set_vx(w, id, vx);
    w.advance_anim(id);
    match mode(w, id) {
        0 => enter_fast_advance(w, id),
        1 | 2 => {}
        _ => class_go_home(w, id),
    }
}

/// Starts running toward Claw at 120 px/s, facing him.
pub fn enter_fast_advance(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let vx = toward_claw(w, id, FAST_ADVANCE_SPEED);
    set_vx(w, id, vx);
    set_state(w, id, EnemyState::FastAdvance);
    let anim = class_of(w, id).map_or("", |c| c.anims.fast_advance);
    set_anim(w, id, anim);
}

/// FastAdvance: at the limit on Claw's side it goes idle (and carries
/// on); it faces Claw; then by mode it keeps running at him (0, 1, 5),
/// walks (2), goes Home (3, 4) or paces (6). Draws: one, discarded.
pub fn fast_advance_tick(w: &mut World, id: ObjectId) {
    let (min, max) = bounds(w, id);
    let x = obj(w, id).x;
    let right = claw_right(w, id);
    if (x == min && !right) || (x == max && right) {
        go_idle(w, id);
    }
    face_claw(w, id);
    w.advance_anim(id);
    let _ = w.rng.next_u15();
    match mode(w, id) {
        0 | 1 | 5 => {
            let vx = toward_claw(w, id, FAST_ADVANCE_SPEED);
            set_vx(w, id, vx);
        }
        2 => enter_advance(w, id),
        3 | 4 => class_go_home(w, id),
        6 => start_pacing(w, id),
        _ => {}
    }
}

/// Starts walking left at `speed` px/s (the rat), without turning.
pub fn move_left(w: &mut World, id: ObjectId, speed: f64) {
    set_vx(w, id, -speed);
    set_state(w, id, EnemyState::MoveLeft);
    let anim = class_of(w, id).map_or("", |c| c.anims.walk);
    set_anim(w, id, anim);
}

/// Starts walking right at `speed` px/s (the rat), without turning.
pub fn move_right(w: &mut World, id: ObjectId, speed: f64) {
    set_vx(w, id, speed);
    set_state(w, id, EnemyState::MoveRight);
    let anim = class_of(w, id).map_or("", |c| c.anims.walk);
    set_anim(w, id, anim);
}

/// MoveLeft: at the left limit it walks right at its current speed, or
/// at [`WALK_SPEED`] when a wall already turned that speed around.
pub fn walk_left_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).x == bounds(w, id).0 {
        let vx = data(w, id).map_or(0.0, |d| d.vx);
        move_right(w, id, turn_speed(-vx));
    }
}

/// MoveRight: at the right limit it walks left at its current speed, or
/// at [`WALK_SPEED`] when a wall already turned that speed around.
pub fn walk_right_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).x == bounds(w, id).1 {
        let vx = data(w, id).map_or(0.0, |d| d.vx);
        move_left(w, id, turn_speed(vx));
    }
}

/// The speed a walker turns at, given its current speed reversed (so a
/// positive value leads away from the limit): that speed, truncated, or
/// [`WALK_SPEED`] when it does not lead away (see the module
/// documentation).
pub fn turn_speed(reversed: f64) -> f64 {
    let speed = reversed.trunc();
    if speed > 0.0 { speed } else { WALK_SPEED }
}

// ----------------------------------------------------------------------
// Hits

/// Starts a flinch, high or low: busy, its animation and sounds (low: the
/// class's low hit sound then the second one; high: the class's high hit
/// sound then one of the four hit sounds), and a knockback of 200 px/s
/// away from Claw. Draws: one of 0..4 for a high flinch's sound.
pub fn enter_hit(w: &mut World, id: ObjectId, high: bool) {
    let Some(class) = class_of(w, id) else { return };
    set_busy(w, id, true);
    if high {
        set_anim(w, id, class.anims.hit_high);
        play(w, class.sounds.hit_high);
        let pick = rand(w, 4) as usize;
        play(w, HIT_SOUNDS[pick]);
    } else {
        set_anim(w, id, class.anims.hit_low);
        play(w, class.sounds.hit_low[0]);
        play(w, class.sounds.hit_low[1]);
    }
    let vx = -toward_claw(w, id, KNOCKBACK);
    set_vx(w, id, vx);
    set_state(
        w,
        id,
        if high {
            EnemyState::HitHigh
        } else {
            EnemyState::HitLow
        },
    );
}

/// Hit: when the animation ends, Home and free; otherwise, while the
/// knockback is not exactly 0, it slows by 25 px/s (toward Claw's side as
/// it is now), so 200 px/s stops after 8 ticks.
pub fn hit_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        class_go_home(w, id);
        set_busy(w, id, false);
        return;
    }
    if let Some(d) = data_mut(w, id)
        && d.vx != 0.0
    {
        d.vx += if d.claw_right {
            KNOCKBACK_STEP
        } else {
            -KNOCKBACK_STEP
        };
    }
}

// ----------------------------------------------------------------------
// Pause, falling, death

/// Starts a pause of `ms`.
pub fn enter_pause(w: &mut World, id: ObjectId, ms: u64) {
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.pause = Timer::new(now, ms);
    }
    set_state(w, id, EnemyState::Pause);
}

/// Pause: back to the previous state when the time is up.
pub fn pause_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let Some((pause, prev)) = data(w, id).map(|d| (d.pause, d.prev)) else {
        return;
    };
    if pause.done(now) {
        set_state(w, id, prev);
    }
}

/// Starts falling: the fall animation, flying until it lands.
pub fn enter_falling(w: &mut World, id: ObjectId) {
    let anim = class_of(w, id).map_or("", |c| c.anims.fall);
    set_anim(w, id, anim);
    set_state(w, id, EnemyState::Falling);
    obj_mut(w, id).physics_type = ptype::FLY;
}

/// Falling: idle once it has landed.
pub fn falling_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).physics_type == ptype::STAND {
        go_idle(w, id);
    }
}

/// Dead: a body killed by a hit is removed with a splash once off screen;
/// one dead on spikes or after a throw after its delay; one in goo is
/// already gone.
pub fn dead_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    let Some((kind, timer)) = data(w, id).map(|d| (d.death, d.death_timer)) else {
        return;
    };
    match kind {
        DeathKind::Hit => {
            if !on_screen(w, id) {
                play(w, super::SPLASH_SOUND);
                w.destroy(id);
            }
        }
        DeathKind::Landing | DeathKind::Spikes => {
            if timer.done(now) {
                w.destroy(id);
            }
        }
        DeathKind::Goo => {}
    }
}

// ----------------------------------------------------------------------
// Leaps

/// Launch speed (vx, vy) of a leap of (dx, dy) px under the downward
/// acceleration `a`: the slowest launch that gets there, with a flight
/// time of `sqrt(2 |d| / a)`. A leap of no length has no speed.
pub fn leap(dx: f64, dy: f64, a: f64) -> (f64, f64) {
    let dist = (dx * dx + dy * dy).sqrt();
    if dist == 0.0 || a == 0.0 {
        return (0.0, 0.0);
    }
    let t = (2.0 * dist / a.abs()).sqrt();
    (dx / t, dy / t - a * t / 2.0)
}

/// The check of an enemy standing on a narrow ledge, made in the
/// post-tick: with Claw nearer than 250 px it faces him and leaps at him
/// (no patrol limits from then on); otherwise it stays where it is for
/// good and goes idle.
pub fn jump_check(w: &mut World, id: ObjectId, class: &Class) {
    let Some((cx, cy)) = w.claw_object().map(|c| (c.x, c.y)) else {
        return;
    };
    let (ex, ey) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    let (dx, dy) = (i64::from(cx) - i64::from(ex), i64::from(cy) - i64::from(ey));
    if dx * dx + dy * dy < LEAP_REACH * LEAP_REACH + 1 {
        face_claw(w, id);
        obj_mut(w, id).physics_type = ptype::FLY;
        set_state(w, id, EnemyState::JumpAtClaw);
        set_anim(w, id, class.anims.jump);
        if let Some(d) = data_mut(w, id) {
            d.bounds = UNBOUNDED;
            d.bounds_y = UNBOUNDED;
            let (vx, vy) = leap(dx as f64, dy as f64, d.ay);
            d.vx = vx;
            d.vy = vy;
        }
    } else {
        if let Some(d) = data_mut(w, id) {
            d.stationary = true;
        }
        (class.go_idle)(w, id);
    }
}

/// JumpAtClaw: facing Claw in the air; Home once it has landed.
pub fn jump_at_claw_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    face_claw(w, id);
    if obj(w, id).physics_type == ptype::STAND {
        class_go_home(w, id);
    }
}

/// A springboard threw the enemy: first the hop up ([`human::enter_jump`],
/// which makes a standing enemy safe until it lands), then busy, no patrol
/// limits, flying toward the springboard's target (the first point of its
/// own limits, or the springboard itself without one).
pub fn enter_spring_jump(w: &mut World, id: ObjectId, spring: ObjectId) {
    let Some(class) = class_of(w, id) else { return };
    let Some(target) = w.objects.get(spring).map(|s| {
        let r = |i: usize| s.record.as_deref().map_or(0, |r| r[i]);
        match (r(field::X_MIN), r(field::Y_MIN)) {
            (0, 0) => (s.x, s.y),
            (x, y) => (x, y),
        }
    }) else {
        return;
    };
    human::enter_jump(w, id);
    let (ex, ey) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    set_busy(w, id, true);
    set_state(w, id, EnemyState::Jump);
    set_anim(w, id, class.anims.jump);
    obj_mut(w, id).physics_type = ptype::FLY;
    if let Some(d) = data_mut(w, id) {
        d.bounds = UNBOUNDED;
        d.bounds_y = UNBOUNDED;
        let (vx, vy) = leap(f64::from(target.0 - ex), f64::from(target.1 - ey), d.ay);
        d.vx = vx;
        d.vy = vy;
    }
}

/// Jump: once it has landed, stopped, hits are allowed again and it goes
/// Home, free.
pub fn jump_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).physics_type == ptype::STAND {
        set_vx(w, id, 0.0);
        obj_mut(w, id).flags &= !flags::SAFE;
        class_go_home(w, id);
        set_busy(w, id, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leap_reaches_its_target() {
        for &(dx, dy) in &[(100.0, 0.0), (-120.0, 40.0), (0.0, -80.0), (200.0, -150.0)] {
            let (vx, vy) = leap(dx, dy, 2000.0);
            let t = (2.0 * f64::hypot(dx, dy) / 2000.0).sqrt();
            let x = vx * t;
            let y = vy * t + 0.5 * 2000.0 * t * t;
            assert!((x - dx).abs() < 1e-9 && (y - dy).abs() < 1e-9, "{dx} {dy}");
        }
        assert_eq!(leap(0.0, 0.0, 2000.0), (0.0, 0.0));
    }

    #[test]
    fn the_knockback_stops_at_exactly_zero() {
        let mut v = KNOCKBACK;
        let mut ticks = 0;
        while v != 0.0 {
            v -= KNOCKBACK_STEP;
            ticks += 1;
        }
        assert_eq!(ticks, 8);
    }
}
