//! Human enemies picked up, carried, thrown and let go by Claw.
//!
//! Claw's side calls [`lifted`] when he picks an enemy up, [`wind_up`]
//! when he starts a throw, [`thrown`] when he lets fly and [`dropped`]
//! when he lets go (he is hit, falls, or holds it too long). The rat can
//! never be lifted.
//!
//! - Lifted: the enemy may say its lifted line (a third of the time, on
//!   screen); it is busy, cannot be hit, stays active everywhere, takes its
//!   carried movement box and no longer moves by itself. On Claw's lift
//!   frame it is placed beside him; on the next frame it is raised over
//!   his head (Flailing).
//! - Flailing: held over Claw's head, facing away from his front; a little
//!   back and down while he winds up a throw.
//! - Thrown: it flies under gravity as a thrown object (hurting the
//!   enemies it meets) for at most 2 s, after which it is gone with its
//!   death sound, a splash and its loot. Its first two landings bounce it
//!   (lifted 1 px with the fractions of its exact position kept, at a
//!   third of the speed, the first with a thud; the lift raises only its
//!   exact height, so one that landed on a platform object and still
//!   rides it bounces from the pixel it landed on, see [`super::motion`]);
//!   the third stops it for 500 ms, and then the fall costs it 4 health:
//!   at 0 or below it dies where it lies, otherwise it gets up, goes Home
//!   and cannot be lifted for 1.5 s.
//! - Dropped: it falls, stops on landing, and after 1 s gets up and goes
//!   Home. When Claw walks off a ledge carrying it, it is dropped too, not
//!   thrown ([`walked_off`]): it keeps his walk's speed across (225 px/s)
//!   while it falls, but it neither bounces nor pays for a landing.
//!
//! A defeated officer or soldier can be lifted too, as long as it is
//! still an enemy (killed by a blow or on spikes; one killed by the
//! landing of a throw is a thrown object, and one killed in goo is gone at
//! once). The body goes through the same Lifted, Flailing, Thrown and
//! Bounce steps (it may say its lifted line), but:
//!
//! - thrown, it hurts nobody (a dead enemy stays harmless to others);
//! - after its rest it dies again whatever its health, where it lies,
//!   with no loot and no sound, and is removed a second later;
//! - out of time in the air it is gone with its death sound and a splash,
//!   but drops no loot a second time;
//! - dropped, it stays dead: once its second is up and it is on the
//!   ground, it lies still with no loot and no sound and is removed a
//!   second later.

use crate::logic::World;
use crate::object::{ObjectId, Rect, flags, types};
use crate::physics::ptype;

use super::states::enter_pause;
use super::{
    DeathKind, Deferred, EnemyState, LAND_SOUND, SPLASH_SOUND, Timer, UNBOUNDED, class_of, data,
    data_mut, die, drop_loot, facing_rect, obj, obj_mut, on_screen, play, rand, say, set_anim,
    set_facing, set_state,
};

/// How Claw carries an enemy of a class.
#[derive(Debug, Clone, Copy)]
pub struct Carry {
    /// Offset from Claw on his lift frame: x when he faces left, x when he
    /// faces right, y.
    pub beside: (i32, i32, i32),
    /// Offset from Claw over his head: x when he faces left, x when he
    /// faces right, y.
    pub overhead: (i32, i32, i32),
    /// Movement box while carried, facing left.
    pub move_rect: Rect,
}

/// The frame of Claw's lift on which a lifted enemy is placed beside him.
pub const CLAW_LIFT_FRAME: i32 = 42;
/// The frame of Claw's lift on which a lifted enemy goes over his head.
pub const CLAW_OVERHEAD_FRAME: i32 = 43;
/// Time a thrown enemy flies before it is gone, ms.
pub const THROWN_MS: u64 = 2000;
/// Rest after the third landing, ms.
pub const REST_MS: u64 = 500;
/// Health a thrown enemy loses when it comes to rest.
pub const LANDING_DAMAGE: i32 = 4;
/// Time after getting up from a throw before it can be lifted again, ms.
pub const LIFT_GUARD_MS: u64 = 1500;
/// Time a dropped enemy takes to get up, ms.
pub const DROPPED_MS: u64 = 1000;
/// Shift of a held enemy while Claw winds up a throw: back, down, px.
pub const WIND_UP_SHIFT: (i32, i32) = (4, 5);

/// `carrier` (Claw) picked up `id`, alive or dead. A body is taken only
/// while it is still an enemy; nothing being removed is taken. Draws: the
/// class's line choice (the officer's), then, unless the class says its
/// own line without one ([`super::Class::lift_roll`]), one of 0..100
/// (below 33 it speaks, when on screen).
pub fn lifted(w: &mut World, id: ObjectId, carrier: ObjectId) {
    let Some(class) = class_of(w, id) else { return };
    let Some(carry) = class.carry else { return };
    let Some(dead) = data(w, id).map(|d| d.dead) else {
        return;
    };
    let o = obj(w, id);
    if o.flags & flags::DESTROY != 0 || (dead && o.type_flags != types::ENEMY) {
        return;
    }
    let z = w.objects.get(carrier).map(|c| c.z.saturating_add(1));
    if let Some(d) = data_mut(w, id) {
        d.z_sort = false;
    }
    if let Some(z) = z {
        let o = obj_mut(w, id);
        o.z = z;
        o.flags |= flags::RESORT;
    }
    (class.pick_lift_voice)(w, id);
    if class.lift_roll && rand(w, 100) < 33 && on_screen(w, id) {
        let line = data(w, id).map_or("", |d| d.lifted_voice);
        say(w, id, line);
    }
    set_state(w, id, EnemyState::Lifted);
    let o = obj_mut(w, id);
    o.physics_type = ptype::DIRECT;
    o.y -= 1;
    o.flags |= flags::ALWAYS_ACTIVE | flags::SAFE;
    let saved = facing_rect(o, o.move_rect);
    o.move_rect = facing_rect(o, carry.move_rect);
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
        d.bounds = UNBOUNDED;
        d.bounds_y = UNBOUNDED;
        d.frozen = true;
        d.winding_up = false;
        d.saved_move_rect = saved;
        d.vx = 0.0;
        d.vy = 0.0;
    }
}

/// Claw starts winding up a throw of `id`.
pub fn wind_up(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.winding_up = true;
    }
}

/// Places a carried enemy at Claw plus `offset` (by his facing), facing
/// away from his front.
fn place(w: &mut World, id: ObjectId, offset: (i32, i32, i32)) {
    let Some((cx, cy)) = w.claw_object().map(|c| (c.x, c.y)) else {
        return;
    };
    let right = w.claw.facing_right;
    let mut x = cx + if right { offset.1 } else { offset.0 };
    let mut y = cy + offset.2;
    if data(w, id).is_some_and(|d| d.winding_up) {
        x += if right {
            -WIND_UP_SHIFT.0
        } else {
            WIND_UP_SHIFT.0
        };
        y += WIND_UP_SHIFT.1;
    }
    set_facing(w, id, !right);
    let o = obj_mut(w, id);
    o.x = x;
    o.y = y;
}

/// The picture a lifted enemy shows on Claw's lift frame when the level
/// has no lift animation for its class (the second town guard on the
/// fifth level): the first of its own set, drawn as usual.
pub const NO_LIFT_ANIM_FRAME: i32 = 1;

/// Lifted: on Claw's lift frame it is placed beside him, showing its lift
/// animation (or, when the level has none for its class, the first
/// picture of its own set, [`NO_LIFT_ANIM_FRAME`]); on the next frame it
/// goes over his head.
pub fn lifted_tick(w: &mut World, id: ObjectId) {
    let Some(class) = class_of(w, id) else { return };
    let Some(carry) = class.carry else { return };
    match w.claw_object().map(|c| c.frame) {
        Some(CLAW_LIFT_FRAME) => {
            set_anim(w, id, class.anims.lifted);
            if !w.level.animations.contains_key(class.anims.lifted) {
                obj_mut(w, id).frame = NO_LIFT_ANIM_FRAME;
            }
            place(w, id, carry.beside);
        }
        Some(CLAW_OVERHEAD_FRAME) => {
            set_anim(w, id, class.anims.lifted2);
            set_state(w, id, EnemyState::Flailing);
            place(w, id, carry.overhead);
        }
        _ => {}
    }
}

/// Flailing: held over Claw's head.
pub fn flailing_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let Some(carry) = class_of(w, id).and_then(|c| c.carry) else {
        return;
    };
    place(w, id, carry.overhead);
}

/// Claw threw `id` at (vx, vy) px/s.
pub fn thrown(w: &mut World, id: ObjectId, vx: i32, vy: i32) {
    let Some(class) = class_of(w, id) else { return };
    if !matches!(
        super::state(w, id),
        Some(EnemyState::Lifted | EnemyState::Flailing)
    ) {
        return;
    }
    let now = w.now_ms;
    set_state(w, id, EnemyState::Thrown);
    set_anim(w, id, class.anims.lifted2);
    let o = obj_mut(w, id);
    o.physics_type = ptype::FLY;
    o.flags |= flags::SAFE;
    o.type_flags = types::THROWN;
    if let Some(d) = data_mut(w, id) {
        d.thrown_timer = Timer::new(now, THROWN_MS);
        d.busy = true;
        d.frozen = false;
        d.winding_up = false;
        d.z_sort = true;
        d.vx = f64::from(vx);
        d.vy = f64::from(vy);
        d.carry = (0.0, 0.0);
        d.bounces = 0;
    }
}

/// Claw let go of `id`: it is dropped in its post-tick.
pub fn dropped(w: &mut World, id: ObjectId) {
    if matches!(
        super::state(w, id),
        Some(EnemyState::Lifted | EnemyState::Flailing)
    ) && let Some(d) = data_mut(w, id)
    {
        d.deferred = Some(Deferred::Dropped);
    }
}

/// Claw walked off a ledge carrying `id`, his walk moving it `vx` px/s
/// across (225 either way, or 0 when he stood still): it is dropped as in
/// [`dropped`], not thrown, so it neither bounces nor pays for a landing,
/// but it keeps that speed across while it falls, until it stands.
pub fn walked_off(w: &mut World, id: ObjectId, vx: i32) {
    if !matches!(
        super::state(w, id),
        Some(EnemyState::Lifted | EnemyState::Flailing)
    ) {
        return;
    }
    dropped(w, id);
    if let Some(d) = data_mut(w, id) {
        d.vx = f64::from(vx);
    }
}

/// Drops a carried enemy: it falls, cannot be hit or touch anything, and
/// gets up after a second.
pub fn let_go(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    set_state(w, id, EnemyState::Dropped);
    let o = obj_mut(w, id);
    o.physics_type = ptype::FLY;
    o.flags |= flags::SAFE | flags::NO_COLLIDE;
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
        d.frozen = false;
        d.winding_up = false;
        d.z_sort = true;
        d.carry = (0.0, 0.0);
        d.dropped_timer = Timer::new(now, DROPPED_MS);
    }
}

/// Dropped: stops on landing; gets up after its time, or, for a body,
/// lies still once it is also on the ground (see [`lie_still`]).
pub fn dropped_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    let standing = obj(w, id).physics_type == ptype::STAND;
    if standing && let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    let Some((up, dead)) = data(w, id).map(|d| (d.dropped_timer.done(now), d.dead)) else {
        return;
    };
    if up && dead {
        // Like any body, it may leave the active area from now on.
        obj_mut(w, id).flags &= !flags::ALWAYS_ACTIVE;
        if standing {
            lie_still(w, id);
        }
    } else if up {
        restore_move_rect(w, id);
        let o = obj_mut(w, id);
        o.flags &= !(flags::ALWAYS_ACTIVE | flags::SAFE | flags::NO_COLLIDE);
        if let Some(c) = class_of(w, id) {
            (c.go_home)(w, id);
        }
        if let Some(d) = data_mut(w, id) {
            d.busy = false;
        }
    }
}

/// Thrown: gone once its time is up; a landing bounces it.
pub fn thrown_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    if data(w, id).is_some_and(|d| d.thrown_timer.done(now)) {
        vanish(w, id);
    } else if obj(w, id).physics_type == ptype::STAND {
        landing(w, id);
    }
}

/// Bounce: after its rest, the fall's cost; gone once its time is up;
/// otherwise the next landing.
pub fn bounce_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if data(w, id).is_some_and(|d| d.prev == EnemyState::Pause) {
        landing(w, id);
    } else if data(w, id).is_some_and(|d| d.thrown_timer.done(now)) {
        vanish(w, id);
    } else {
        w.advance_anim(id);
        if obj(w, id).physics_type == ptype::STAND {
            landing(w, id);
        }
    }
}

/// A dropped body on the ground: its own movement box, touchable again,
/// and dead where it lies (no loot, no sound), removed a second later.
fn lie_still(w: &mut World, id: ObjectId) {
    restore_move_rect(w, id);
    let o = obj_mut(w, id);
    o.flags &= !(flags::ALWAYS_ACTIVE | flags::SAFE | flags::NO_COLLIDE);
    die(w, id, DeathKind::Landing, None);
}

/// A thrown enemy out of time: its death sound, a splash and its loot
/// (none for a body, which dropped its loot when it died), and it is gone.
fn vanish(w: &mut World, id: ObjectId) {
    let s = data(w, id).map_or("", |d| d.death_sound);
    play(w, s);
    play(w, SPLASH_SOUND);
    if data(w, id).is_some_and(|d| !d.dead) {
        drop_loot(w, id);
    }
    if let Some(d) = data_mut(w, id) {
        d.dead = true;
    }
    w.destroy(id);
}

/// One landing after a throw (see the module documentation).
fn landing(w: &mut World, id: ObjectId) {
    let Some(class) = class_of(w, id) else { return };
    let n = match data_mut(w, id) {
        Some(d) => {
            d.bounces += 1;
            d.bounces
        }
        None => return,
    };
    if n == 1 {
        play(w, LAND_SOUND);
    }
    match n {
        1 | 2 => {
            set_state(w, id, EnemyState::Bounce);
            set_anim(w, id, class.anims.bounce);
            let o = obj_mut(w, id);
            o.physics_type = ptype::FLY;
            o.y -= 1;
            let at = (o.x, o.y);
            if let Some(d) = data_mut(w, id) {
                let v = (d.vy * d.vy - 2.0 * d.ay).max(0.0).sqrt();
                d.vx /= 3.0;
                d.vy = -v / 3.0;
                // The lift keeps the fractions of its exact position.
                d.moved_to = Some(at);
                d.bounce_lift = true;
            }
        }
        3 => {
            if let Some(d) = data_mut(w, id) {
                d.vx = 0.0;
            }
            enter_pause(w, id, REST_MS);
        }
        _ => {
            obj_mut(w, id).health -= LANDING_DAMAGE;
            let dead = data(w, id).is_some_and(|d| d.dead);
            if dead || obj(w, id).health <= 0 {
                die(w, id, DeathKind::Landing, None);
            } else {
                recover(w, id);
            }
        }
    }
}

/// Gets up after a throw: an enemy again, Home, free, and not liftable
/// for a while.
fn recover(w: &mut World, id: ObjectId) {
    let Some(class) = class_of(w, id) else { return };
    let now = w.now_ms;
    restore_move_rect(w, id);
    let o = obj_mut(w, id);
    o.type_flags = types::ENEMY;
    o.flags &= !(flags::SAFE | flags::ALWAYS_ACTIVE);
    set_anim(w, id, class.anims.home);
    (class.go_home)(w, id);
    if let Some(d) = data_mut(w, id) {
        d.bounces = 0;
        d.busy = false;
        d.lift_guard = Timer::new(now, LIFT_GUARD_MS);
    }
}

/// The movement box saved at the lift, keeping the feet where they are.
/// The move to the new height keeps the fraction of a pixel the enemy
/// carries across (it walks on from there), and drops the one up and
/// down. The crab's landing after a throw puts its box back the same way.
pub(super) fn restore_move_rect(w: &mut World, id: ObjectId) {
    let Some(saved) = data(w, id).map(|d| d.saved_move_rect) else {
        return;
    };
    if !saved.is_set() {
        return;
    }
    let o = obj_mut(w, id);
    let before = (o.x, o.y);
    let r = facing_rect(o, saved);
    if o.move_rect.is_set() {
        o.y += o.move_rect.bottom - r.bottom;
    }
    o.move_rect = r;
    let after = (o.x, o.y);
    if let Some(d) = data_mut(w, id)
        && d.moved_to == Some(before)
    {
        d.moved_to = Some(after);
        d.carry.1 = 0.0;
    }
}
