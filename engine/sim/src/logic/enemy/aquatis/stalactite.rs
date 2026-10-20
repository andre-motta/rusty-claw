//! The five stalactites over Aquatis ([`super`]), which fall on him one by
//! one.
//!
//! They hang from the cave's ceiling with the cave stalactites' picture
//! and box ([`crate::logic::stalactite`]) but follow Aquatis, not Claw:
//!
//! - Built: the hanging picture, an enemy's body (touching it hurts as the
//!   enemies' bodies do, for its record's damage) that nothing can hurt,
//!   its picture's box for moving, being hit and touching, never moved,
//!   always active. Its reset bit is the next of
//!   [`super::STALACTITE_BITS`] in the order they are built. It hangs.
//! - Hanging: the hanging picture shown at once; the link's falling flag
//!   is cleared.
//! - Each call, before its state: while Aquatis is there and his reset
//!   bits hold its bit, it resets: shown, in every collision, back at its
//!   home, not moving, the order counter back to 1, its bit cleared, and
//!   it hangs.
//! - Hanging, each call: while Aquatis is alive, the next stalactite is
//!   armed and the order counter equals its record's smarts, the counter
//!   moves on and it falls: the snap ([`crate::logic::stalactite::SNAP_SOUND`]),
//!   in every collision, the armed flag cleared and the falling flag set.
//! - Falling: under [`crate::logic::stalactite::GRAVITY`] (at most
//!   [`super::super::motion::STEP_LIMIT`] px a tick), through everything,
//!   from the call it lets go on; while Aquatis is alive, once it has
//!   reached the link's landing height, the falling flag is cleared and
//!   it crumbles: out of every collision, the crash, the crumble; when the
//!   crumble ends it is hidden and waits for the next reset.
//!
//! While it falls it is an enemy's body to whatever it touches, so it
//! hurts Aquatis ([`super::stalactite_hit`]). From the ceiling over him a
//! fall takes [`FALL_TICKS`] ticks, the hit landing on the 19th.
//!
//! It draws nothing from the random streams.

use claw_frame::SoundChannel;

use super::super::motion::axis_step_for;
use crate::logic::stalactite::{self as cave, BOX, CRUMBLE_ANIM, GRAVITY, HOME_ANIM};
use crate::logic::{ObjectData, World, sound};
use crate::object::{ObjectId, draw_flags, flags, types};
use crate::physics::ptype;
use crate::tuning::TICK_MS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["AquatisStalactite"];

/// Image sets this family may switch an object to: the cave stalactites'.
pub const IMAGE_SETS: &[&str] = &[cave::IMAGE_SET];

/// Prefixes of the animations this family may play: the cave
/// stalactites'.
pub const ANIM_PREFIXES: &[&str] = cave::ANIM_PREFIXES;

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Ticks of a fall from the ceiling over Aquatis to the landing height at
/// the 18 ms step (measured), the hit on him on the 19th.
pub const FALL_TICKS: u32 = 25;
/// What may hit it: a kind none of the level's objects carries.
pub const HIT_MASK: u32 = 0x1;
/// The record field of its place in the order of the falls (its smarts).
pub const ORDER_FIELD: usize = 17;
/// The record field of its damage.
pub const DAMAGE_FIELD: usize = 16;

/// Its logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Hanging.
    pub const IDLE: i32 = 1;
    /// Falling.
    pub const FALLING: i32 = 2;
    /// Crumbling where it stopped.
    pub const DEAD: i32 = 3;
    /// Gone, until the next reset.
    pub const WAITING: i32 = 4;
}

/// A stalactite's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StalactiteData {
    /// Where it hangs.
    pub home: (i32, i32),
    /// Its reset bit (one of [`super::STALACTITE_BITS`]); 0 before it is
    /// built.
    pub bit: u32,
    /// Its place in the order of the falls, from 1.
    pub order: i32,
    /// Speed downwards, px/s.
    pub vy: f64,
    /// How far its exact height is past its whole pixel, px.
    pub carry: f64,
}

fn stal_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut StalactiteData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::AquatisStalactite(d) => Some(d),
        _ => None,
    }
}

fn stal<'a>(w: &'a World, id: ObjectId) -> Option<&'a StalactiteData> {
    match &w.objects.get(id)?.data {
        ObjectData::AquatisStalactite(d) => Some(d),
        _ => None,
    }
}

fn state_of(w: &World, id: ObjectId) -> Option<i32> {
    w.objects.get(id).map(|o| o.state)
}

fn set_state(w: &mut World, id: ObjectId, s: i32) {
    if let Some(o) = w.objects.get_mut(id) {
        o.state = s;
    }
}

/// One logic call (see the module documentation).
pub fn tick(w: &mut World, id: ObjectId) {
    super::reset_on_respawn(w);
    let Some(s) = state_of(w, id) else { return };
    if s == state::SET_UP {
        set_up(w, id);
        return;
    }
    let bit = stal(w, id).map_or(0, |d| d.bit);
    if w.aquatis().is_some() && w.aquatis_reset_bits() & bit != 0 {
        reset(w, id);
        return;
    }
    match s {
        state::IDLE => {
            w.advance_anim(id);
            let link = *w.aquatis_link();
            let order = stal(w, id).map_or(0, |d| d.order);
            if w.aquatis_alive() && link.armed && link.stalactite_order == order {
                w.aquatis_link_mut().stalactite_order += 1;
                fall(w, id);
            }
        }
        state::FALLING => {
            let landed = w.objects.get(id).map(|o| o.y);
            let land_y = w.aquatis_link().land_y;
            if w.aquatis_alive() && landed.is_some_and(|y| y >= land_y) {
                w.aquatis_link_mut().falling = false;
                crumble(w, id);
            } else {
                step(w, id);
            }
        }
        state::DEAD => {
            w.advance_anim(id);
            if w.objects.get(id).is_some_and(|o| o.anim.is_done()) {
                set_state(w, id, state::WAITING);
                if let Some(o) = w.objects.get_mut(id) {
                    o.draw_flags |= draw_flags::HIDDEN;
                }
            }
        }
        _ => {}
    }
}

fn set_up(w: &mut World, id: ObjectId) {
    let built = w
        .objects
        .all()
        .filter(|&(other, o)| {
            other != id && matches!(&o.data, ObjectData::AquatisStalactite(d) if d.bit != 0)
        })
        .count();
    let bit = super::STALACTITE_BITS.get(built).copied().unwrap_or(0);
    let first = w.image_frames(cave::IMAGE_SET).map_or(1, |f| f.0);
    let rect = w.frame_box(cave::IMAGE_SET, first).unwrap_or(BOX);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let r = |i: usize| o.record.as_ref().map_or(0, |r| r[i]);
    let (order, damage) = (r(ORDER_FIELD), r(DAMAGE_FIELD));
    o.image_set = crate::level::runtime_image_set(cave::IMAGE_SET);
    o.frame = first;
    o.physics_type = ptype::DIRECT;
    o.type_flags = types::ENEMY;
    o.hit_type_flags = HIT_MASK;
    o.attack_type_flags = 0;
    o.move_rect = rect;
    o.hit_rect = rect;
    o.attack_rect = rect;
    o.damage = damage;
    o.flags |= flags::ALWAYS_ACTIVE;
    let home = (o.x, o.y);
    o.data = ObjectData::AquatisStalactite(StalactiteData {
        home,
        bit,
        order,
        ..StalactiteData::default()
    });
    enter_idle(w, id);
}

/// Hangs: the hanging picture shown at once, and no stalactite falls.
fn enter_idle(w: &mut World, id: ObjectId) {
    set_state(w, id, state::IDLE);
    if w.set_anim(id, HOME_ANIM) {
        w.advance_anim(id);
    }
    w.aquatis_link_mut().falling = false;
}

/// The reset (see the module documentation).
fn reset(w: &mut World, id: ObjectId) {
    let Some((home, bit)) = stal(w, id).map(|d| (d.home, d.bit)) else {
        return;
    };
    if let Some(o) = w.objects.get_mut(id) {
        o.draw_flags &= !draw_flags::HIDDEN;
        o.flags &= !flags::NO_COLLIDE;
        (o.x, o.y) = home;
        o.physics_type = ptype::DIRECT;
    }
    if let Some(d) = stal_mut(w, id) {
        d.vy = 0.0;
        d.carry = 0.0;
    }
    w.aquatis_link_mut().stalactite_order = 1;
    w.clear_aquatis_reset_bit(bit);
    enter_idle(w, id);
}

/// Lets go (see the module documentation), and makes the fall's first
/// step.
fn fall(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    set_state(w, id, state::FALLING);
    if let Some(o) = w.objects.get_mut(id) {
        o.physics_type = ptype::FLY;
        o.flags &= !flags::NO_COLLIDE;
    }
    if let Some(d) = stal_mut(w, id) {
        d.vy = 0.0;
        d.carry = 0.0;
    }
    sound::play(w, cave::SNAP_SOUND, SoundChannel::Effect);
    let link = w.aquatis_link_mut();
    link.armed = false;
    link.falling = true;
    step(w, id);
}

/// One tick of the fall, through everything.
fn step(w: &mut World, id: ObjectId) {
    let dt = f64::from(TICK_MS) * 0.001;
    let Some(y) = w.objects.get(id).map(|o| o.y) else {
        return;
    };
    let Some(d) = stal_mut(w, id) else {
        return;
    };
    let py = f64::from(y) + d.carry + axis_step_for(&mut d.vy, GRAVITY, dt);
    let ty = py.trunc() as i32;
    d.carry = py - f64::from(ty);
    if let Some(o) = w.objects.get_mut(id) {
        o.y = ty;
    }
}

/// Stops and crumbles where it is (see the module documentation).
fn crumble(w: &mut World, id: ObjectId) {
    set_state(w, id, state::DEAD);
    if let Some(o) = w.objects.get_mut(id) {
        o.flags |= flags::NO_COLLIDE;
        o.physics_type = ptype::DIRECT;
    }
    w.set_anim(id, CRUMBLE_ANIM);
    sound::play(w, cave::HIT_SOUND, SoundChannel::Effect);
}

/// `attacker` hit `id`. Nothing hurts a stalactite.
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

    #[test]
    fn a_fall_from_the_ceiling_over_him_reaches_the_landing_height_in_time() {
        // From y 3351 to 3550: the 24th step after the one it lets go on
        // passes it, the hit (his box from y 3472) on the 19th.
        let mut v = 0.0;
        let mut y = 3351.0;
        let mut at = Vec::new();
        y += axis_step_for(&mut v, GRAVITY, 0.018);
        at.push(y.trunc() as i32);
        for _ in 0..24 {
            y += axis_step_for(&mut v, GRAVITY, 0.018);
            at.push(y.trunc() as i32);
        }
        assert_eq!(&at[..5], &[3351, 3352, 3353, 3356, 3359]);
        assert_eq!(at[19], 3480);
        assert!(at[18] < 3472 && at[19] >= 3472);
        assert_eq!(at[24], 3553);
        assert!(at[23] < 3550);
        assert_eq!(FALL_TICKS as usize, at.len());
    }
}
