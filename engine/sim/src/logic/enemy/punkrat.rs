//! The punk rat: a small rat gunner that paces a short beat behind its
//! own ground cannon and fires it every time it reaches the back of the
//! beat.
//!
//! On its first logic call it is built and sets its cannon down:
//!
//! - its beat runs from 30 px left of where it was placed to 15 px right
//!   of it; it starts walking right at 60 px/s, showing its standing
//!   animation until its first turn;
//! - it has no gravity, never turns at ledges, is never drawn in front of
//!   or behind Claw by height (its depth is always 3800) and cannot be
//!   lifted;
//! - its cannon (a ground cannon, see [`crate::logic::cannon`]) is created
//!   and built at once, 15 px below it (20 px to its left when the rat is
//!   mirrored), one depth step behind it so it runs first in every pass;
//!   a mirrored rat turns its cannon around, so it fires left.
//!
//! Then it walks its beat. Each walking call it may squeak (one chance in
//! ten, on a frame pulse tick only). On arriving exactly at an end of the
//! beat it turns, except at the back end (the left end for a rat that
//! fires right, the right end for a mirrored one), where it strikes
//! instead, unless Claw is invisible. The strike animation plays out; when
//! it ends the cannon fires and the rat recoils; when the recoil animation
//! ends it walks away from the back end again. It never looks for Claw:
//! only his invisibility stops it striking.
//!
//! One cycle lasts 226 ticks of 18 ms: the strike (99), the recoil (43)
//! and 42 ticks each way along the beat. An unmirrored rat strikes first
//! on its 57th tick and its first ball leaves on its 157th; a mirrored
//! one strikes on its 15th.
//!
//! Its touch hurts nobody (it has no contact box). Any blow that reaches
//! it (Claw's, a thrown object, a pistol shot, an explosion, his magic)
//! kills it at once, with no hit cooldown and no check of the attack's
//! number, and a pistol shot that kills it flies on; its body flies off
//! showing its last picture, drops nothing, and can be hit again. Its
//! cannon stays where it is and never fires again.
//!
//! State numbers ([`EnemyState`]): it walks in `MoveLeft` and `MoveRight`,
//! strikes in `Strike`, recoils in `Recoil` and dies in `Dead`.
//!
//! Draws, each tick after the first: the shared roll (on a frame pulse
//! tick), then, while walking, one of 0..100 for the squeak (on a frame
//! pulse tick). Each accepted hit draws the death's (one of 0..100, then
//! one of 0..10 when Claw speaks).
//!
//! Its data ([`PunkRatData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. Its cannon knows it ([`cannon::CannonData::owner`],
//! see [`cannon_of`]).

use super::states::{move_left, move_right};
use super::{
    Class, ClassAnims, ClassSounds, DeathKind, EnemyData, EnemyFields, EnemyState, HEALTH_FACTOR,
    Handler, claw_invisible, data, data_mut, die, faces_right, obj, obj_mut, play, rand, scaled,
    set_anim, set_anim_now, set_state,
};
use crate::level::runtime_image_set;
use crate::logic::{ObjectData, World, cannon, new_object};
use crate::object::{ObjectId, Rect, flags, special, types};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["PunkRat"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_PUNKRAT_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Image set of a punk rat.
pub const IMAGE_SET: &str = "LEVEL_PUNKRAT";
/// Walking.
pub const WALK_ANIM: &str = "LEVEL_PUNKRAT_WALK";
/// Standing, shown on its first walk.
pub const STAND_ANIM: &str = "LEVEL_PUNKRAT_STAND";
/// Striking: the cannon fires when it ends.
pub const STRIKE_ANIM: &str = "LEVEL_PUNKRAT_STRIKE";
/// Recoiling after a shot.
pub const RECOIL_ANIM: &str = "LEVEL_PUNKRAT_RECOIL";

/// The squeak while walking.
pub const SQUEAK_SOUND: &str = "GAME_EEHH";
/// The death sound.
pub const DEATH_SOUND: &str = "GAME_MOUSEKILL";
/// Chance in 100 of a squeak on a walking call (on a frame pulse tick).
pub const SQUEAK_CHANCE: i32 = 10;

/// Walking speed, px/s.
pub const WALK_SPEED: f64 = 60.0;
/// How far the beat reaches left of where the rat was placed, px.
pub const BEAT_BACK: i32 = 30;
/// How far the beat reaches right of where the rat was placed, px.
pub const BEAT_AHEAD: i32 = 15;
/// Depth of a punk rat.
pub const Z: i32 = 3800;
/// Health of a punk rat (any accepted hit kills it whatever its health).
pub const HEALTH: i32 = 2;
/// Smarts of a punk rat (nothing reads them).
pub const SMARTS: i32 = 10;
/// Speed written to a punk rat placed without one, px/s (nothing reads
/// it).
pub const RECORD_SPEED: i32 = 500;
/// Kinds of object that can hit a punk rat: Claw, his thrown objects, his
/// pistol shots, explosions and his magic.
pub const HIT_BY: u32 = 0x300_0032;
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -40, 20, 40);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-40, -40, 40, 40);
/// Where its cannon is set down, relative to the rat (the cannon of a
/// mirrored rat is a further [`MIRRORED_CANNON_SHIFT`] px to the left).
pub const CANNON_OFFSET: (i32, i32) = (0, 15);
/// See [`CANNON_OFFSET`].
pub const MIRRORED_CANNON_SHIFT: i32 = 20;

/// The punk rat class.
pub static CLASS: Class = Class {
    name: "PunkRat",
    human: false,
    construct,
    pre_tick: super::nothing,
    handler,
    go_home: super::nothing,
    go_idle: super::nothing,
    carry: None,
    pick_lift_voice: super::nothing,
    anims: ClassAnims {
        walk: WALK_ANIM,
        ..ClassAnims::NONE
    },
    sounds: ClassSounds::NONE,
};

/// A punk rat's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PunkRatData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::MoveLeft => Some(move_left_tick),
        EnemyState::MoveRight => Some(move_right_tick),
        EnemyState::Strike => Some(strike_tick),
        EnemyState::Recoil => Some(recoil_tick),
        _ => None,
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    super::tick(w, id, &CLASS);
}

/// `attacker` hit `id`: any hit kills it (again, for a body), with no
/// check of the attack's number and no cooldown, and a pistol shot flies
/// on. Draws: the death's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if data(w, id).is_none() {
        return;
    }
    die(w, id, DeathKind::Hit, Some(attacker));
}

/// `id` attacked `victim`: a punk rat attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

/// The ground cannon punk rat `id` set down: the cannon whose owner it
/// is.
pub fn cannon_of(w: &World, id: ObjectId) -> Option<ObjectId> {
    w.objects
        .all()
        .find(|(_, o)| match &o.data {
            ObjectData::Cannon(c) => c.owner == Some(id),
            _ => false,
        })
        .map(|(cid, _)| cid)
}

/// Builds the punk rat and sets its cannon down (see the module
/// documentation). Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    // The construction step every enemy shares moves a rat placed beside
    // its record's range onto the nearer limit before its beat is set.
    super::clamp_on_record_range(w, id);
    let x = obj(w, id).x;
    let fields = EnemyFields {
        bounds: (x - BEAT_BACK, x + BEAT_AHEAD),
        smarts: SMARTS,
        vx: WALK_SPEED,
        ay: 0.0,
        death_sound: DEATH_SOUND,
        ..EnemyFields::default()
    };
    {
        let o = obj_mut(w, id);
        o.image_set = runtime_image_set(IMAGE_SET);
        if o.speed_x == 0 {
            o.speed_x = RECORD_SPEED;
        }
        o.health = scaled(HEALTH, HEALTH_FACTOR);
        o.type_flags = types::ENEMY;
        o.hit_type_flags = HIT_BY;
        o.special_flags &= !special::LIFTABLE;
        o.flags |= flags::DESTROY_FAR;
        o.hit_handler = true;
        o.hit_rect = HIT_RECT;
        o.move_rect = MOVE_RECT;
        o.attack_rect = Rect::UNSET;
        o.data = ObjectData::PunkRat(PunkRatData {
            enemy: fields.into(),
        });
    }
    super::start_move_clock(w, id);
    move_right(w, id, WALK_SPEED);
    obj_mut(w, id).physics_type = ptype::WALK;
    // The first walk shows the standing animation, advanced once.
    set_anim_now(w, id, STAND_ANIM);
    set_down_cannon(w, id);
}

/// The rat's depth becomes [`Z`], and its ground cannon is created, built
/// at once and, for a mirrored rat, turned around.
fn set_down_cannon(w: &mut World, id: ObjectId) {
    let (x, y, mirrored) = {
        let o = obj_mut(w, id);
        if o.z != Z {
            o.z = Z;
            o.flags |= flags::RESORT;
        }
        (o.x, o.y, faces_right(o))
    };
    let dx = CANNON_OFFSET.0 - if mirrored { MIRRORED_CANNON_SHIFT } else { 0 };
    let c = new_object(cannon::GROUND_CANNON, x + dx, y + CANNON_OFFSET.1, Z - 1);
    let cid = w.spawn(c);
    cannon::tick(w, cid);
    if mirrored {
        cannon::mirror(w, cid);
    }
    cannon::set_owner(w, cid, id);
}

/// True when the rat stands exactly on `limit`: its position, with the
/// fraction the movement step carries, equals it.
fn exactly_at(w: &World, id: ObjectId, limit: i32) -> bool {
    let o = obj(w, id);
    data(w, id).is_some_and(|d| o.x == limit && d.carry.0 == 0.0)
}

/// A walking call: the squeak, then, exactly at an end of the beat, the
/// strike (at the back end, with Claw visible) or the turn. Draws: one of
/// 0..100, on a frame pulse tick only.
fn walk_tick(w: &mut World, id: ObjectId, right: bool) {
    w.advance_anim(id);
    if w.pulse && rand(w, 100) < SQUEAK_CHANCE {
        play(w, SQUEAK_SOUND);
    }
    let Some(bounds) = data(w, id).map(|d| d.bounds) else {
        return;
    };
    let limit = if right { bounds.1 } else { bounds.0 };
    if !exactly_at(w, id, limit) {
        return;
    }
    let mirrored = faces_right(obj(w, id));
    let back_end = right == mirrored;
    if back_end && !claw_invisible(w) {
        enter_strike(w, id);
    } else if right {
        move_left(w, id, WALK_SPEED);
    } else {
        move_right(w, id, WALK_SPEED);
    }
}

/// MoveLeft: the squeak, then the strike or the turn at the left end.
pub fn move_left_tick(w: &mut World, id: ObjectId) {
    walk_tick(w, id, false);
}

/// MoveRight: the squeak, then the strike or the turn at the right end.
pub fn move_right_tick(w: &mut World, id: ObjectId) {
    walk_tick(w, id, true);
}

/// Starts a strike: busy, the strike animation (not advanced in this
/// call); its speed is left as it was, held against the end of the beat.
pub fn enter_strike(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
    }
    set_state(w, id, EnemyState::Strike);
    set_anim(w, id, STRIKE_ANIM);
}

/// Strike: when the strike animation ends the cannon fires and the rat
/// recoils.
pub fn strike_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    if let Some(c) = cannon_of(w, id) {
        cannon::fire(w, c);
    }
    set_anim(w, id, RECOIL_ANIM);
    set_state(w, id, EnemyState::Recoil);
}

/// Recoil: when the recoil animation ends it walks away from the back end
/// of its beat at 60 px/s.
pub fn recoil_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        d.busy = false;
    }
    if faces_right(obj(w, id)) {
        move_left(w, id, WALK_SPEED);
    } else {
        move_right(w, id, WALK_SPEED);
    }
}
