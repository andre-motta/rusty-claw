//! Skull cannons: the ninth level's skull-shaped cannons, which fire a
//! cannonball ([`ball`]) every three seconds whether Claw is near or not.
//!
//! A skull cannon is a tower cannon of the second level's kind
//! ([`super::cannon`]) with its own art: built on its first call (balls at
//! [`SPEED`] px/s, depth [`DEFAULT_Z`] when placed at 0), it waits in
//! `Home` and fires on its first waiting call and then every
//! [`PERIOD_TICKS`] ticks ([`PERIOD_MS`] of game time), counted from the
//! tick it woke. Its firing animation ([`FIRING_ANIM`], [`FIRE_TICKS`]
//! ticks) releases the ball on its first call, with the animation's own
//! sounds. Facing right or left (a mirrored record), the ball always
//! leaves 50 px to the right of the cannon and 10 px below it
//! ([`ball::start`]), so a cannon facing left fires its balls from behind
//! itself, through itself. It never tests where Claw is and draws no
//! random number.
//!
//! | State | Per call |
//! | --- | --- |
//! | Home | advance the Home animation; when 3000 ms or more of game time have passed since the last shot was ordered (at once on the first call), order a shot: start the firing animation and go to Fire |
//! | Fire | advance the firing animation; on its first call whose event is 1, release a ball; when it ends, go Home |
//!
//! With C the tick it enters Fire, the ball leaves on C + 1, the shot's
//! two sounds play on C + 7 and C + 19 and the cannon is Home again on
//! C + 43; the next shot is ordered 167 ticks after C.
//!
//! # State numbers
//!
//! As every cannon it waits in `Home` and fires in `Fire`
//! ([`crate::logic::enemy::EnemyState`]).
//!
//! # Defaults taken
//!
//! - The muzzle is (50, 10) for both facings, as measured (the mirrored
//!   muzzle is overwritten when the cannon is built).
//! - Asleep in the side areas of a wide display, a cannon never woken is
//!   drawn as it waits at Home: its own picture, image 1, facing its
//!   record's way ([`first_look`]).
//!
//! Its data ([`SkullCannonData`]) is its own.

use super::enemy::{self, EnemyState, Timer};
use super::projectile::{self, ProjectileSpec};
use super::{ObjectData, World};
use crate::level::{SimLevel, runtime_image_set};
use crate::logic::Look;
use crate::object::{Object, ObjectId, draw_flags, engine_state, flags, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["SkullCannon"];

/// The cannon's picture.
pub const IMAGE_SET: &str = "LEVEL_SKULLCANNON";

/// Image sets this family may switch an object to: its own and its ball's.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, ball::IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SKULLCANNON_"];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// Firing.
pub const FIRING_ANIM: &str = "LEVEL_SKULLCANNON_FIRING";
/// Waiting.
pub const HOME_ANIM: &str = "LEVEL_SKULLCANNON_HOME";

/// Speed of its balls when the record gives none, px/s.
pub const SPEED: i32 = 500;
/// Depth of a cannon placed at depth 0.
pub const DEFAULT_Z: i32 = 3990;
/// Time between two shots, ms of game time.
pub const PERIOD_MS: u32 = 3000;
/// Ticks between two shots at the 18 ms step (measured).
pub const PERIOD_TICKS: u32 = 167;
/// Ticks of the firing animation at the 18 ms step.
pub const FIRE_TICKS: u32 = 43;

/// The ball a skull cannon fires: the second level's cannonball, an
/// enemy's shot that costs Claw [`DAMAGE`](ball::DAMAGE), also when he
/// ducks, and flies straight at the cannon's speed. The ninth level has
/// no animation for it, so it shows its first picture throughout. It is
/// not a family of its own: the projectile family runs it.
pub mod ball {
    /// Logic name of the ball.
    pub const LOGIC: &str = "CannonBall";
    /// Its picture.
    pub const IMAGE_SET: &str = "LEVEL_CANNONBALL";
    /// The picture it shows throughout on the ninth level.
    pub const FRAME: i32 = 1;
    /// Damage to Claw.
    pub const DAMAGE: i32 = 10;
    /// Where it leaves, from the cannon, whichever way the cannon faces.
    pub const MUZZLE: (i32, i32) = (50, 10);

    /// Where a ball of the cannon at (`x`, `y`) leaves, and its speed for a
    /// cannon facing left (`mirrored`) or right.
    pub fn start((x, y): (i32, i32), mirrored: bool) -> ((i32, i32), i32) {
        let speed = if mirrored {
            -super::SPEED
        } else {
            super::SPEED
        };
        ((x + MUZZLE.0, y + MUZZLE.1), speed)
    }
}

/// What may hit a ball (nothing reaches it: it has no hit box).
pub const BALL_HIT_BY: u32 = 0x2;

/// A skull cannon's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkullCannonData {
    /// Balls fired so far.
    pub fired: u32,
    /// The shot timer: the last shot was ordered at `start`, the next is
    /// due `len` ms later. Both start at 0, so the first call at Home
    /// fires.
    pub timer: Timer,
    /// The current firing animation has released its ball.
    pub shot: bool,
    /// The last ball fired.
    pub last_ball: Option<ObjectId>,
}

/// The data of skull cannon `id`, if it is one.
pub fn data<'a>(w: &'a World, id: ObjectId) -> Option<&'a SkullCannonData> {
    match &w.objects.get(id)?.data {
        ObjectData::SkullCannon(d) => Some(d),
        _ => None,
    }
}

fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut SkullCannonData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::SkullCannon(d) => Some(d),
        _ => None,
    }
}

/// Builds the skull cannon `o` as its first call does, without its
/// animation: its picture, the speed of its balls (turned for a mirrored
/// record), its depth, its damage and its data, waiting at Home.
fn build(level: &SimLevel, o: &mut Object) {
    o.image_set = runtime_image_set(IMAGE_SET);
    if let Some((first, _)) = level.image_frames(IMAGE_SET) {
        o.frame = first;
    }
    o.flags |= flags::NO_COLLIDE | flags::DESTROY_FAR;
    if o.speed_x == 0 {
        o.speed_x = SPEED;
    }
    if o.damage == 0 {
        o.damage = super::cannon::DAMAGE;
    }
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    if o.draw_flags & draw_flags::MIRROR != 0 && o.speed_x > 0 {
        o.speed_x = -o.speed_x;
    }
    o.data = ObjectData::SkullCannon(SkullCannonData::default());
    o.state = EnemyState::Home as i32;
}

/// How a skull cannon whose first call has not run yet looks once it has
/// (see [`super::first_look`]): waiting at Home in its own picture.
/// `None` once it has run.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    if o.state != engine_state::INIT {
        return None;
    }
    let mut c = o.clone();
    build(level, &mut c);
    Some(vec![Look::of(&c)])
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(state) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    if state == engine_state::INIT {
        let level = w.level;
        if let Some(o) = w.objects.get_mut(id) {
            build(level, o);
        }
        enemy::set_anim(w, id, HOME_ANIM);
        return;
    }
    match EnemyState::from_id(state) {
        Some(EnemyState::Home) => home_tick(w, id),
        Some(EnemyState::Fire) => firing_tick(w, id),
        _ => {}
    }
}

/// Home: the Home animation plays, and the cannon fires when its timer
/// has run out, restarting it.
fn home_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    let due = data(w, id).is_some_and(|d| d.timer.done(now));
    if !due {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        d.timer = Timer::new(now, u64::from(PERIOD_MS));
        d.shot = false;
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.state = EnemyState::Fire as i32;
    }
    enemy::set_anim(w, id, FIRING_ANIM);
}

/// Fire: the first call whose animation event is 1 releases the ball;
/// when the animation ends the cannon goes Home.
fn firing_tick(w: &mut World, id: ObjectId) {
    let event = w.advance_anim(id);
    if event == 1 && data(w, id).is_some_and(|d| !d.shot) {
        if let Some(d) = data_mut(w, id) {
            d.shot = true;
        }
        release(w, id);
    }
    if w.objects.get(id).is_none_or(|o| o.anim.is_done()) {
        if let Some(o) = w.objects.get_mut(id) {
            o.state = EnemyState::Home as i32;
        }
        enemy::set_anim(w, id, HOME_ANIM);
    }
}

/// Launches a ball from the muzzle at the cannon's speed, one depth step
/// behind the cannon.
fn release(w: &mut World, id: ObjectId) {
    let Some((x, y, z, vx, vy)) = w
        .objects
        .get(id)
        .map(|o| (o.x, o.y, o.z, o.speed_x, o.speed_y))
    else {
        return;
    };
    let ((bx, by), _) = ball::start((x, y), vx < 0);
    let shot = projectile::spawn(
        w,
        ProjectileSpec {
            logic: ball::LOGIC,
            x: bx,
            y: by,
            z: z - 1,
            vx,
            vy,
            damage: 0,
            type_flags: types::ENEMY_SHOT,
            hit_type_flags: BALL_HIT_BY,
            special_flags: 0,
            image_set: "",
            owner: Some(id),
            fuse_ms: None,
            power: 0,
        },
    );
    if let Some(b) = shot {
        super::dart_trap::start_shot_clock(w, b);
    }
    if let Some(d) = data_mut(w, id) {
        d.fired += 1;
        d.last_ball = shot;
    }
}

/// `attacker` hit `id`. Nothing hits a cannon.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A cannon attacks nothing; its balls do.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balls_leave_on_the_right_whichever_way_the_cannon_faces() {
        // The crossfire shaft's cannons: 326 fires right, 322 left from
        // behind itself.
        assert_eq!(ball::start((7490, 6693), false), ((7540, 6703), 500));
        assert_eq!(ball::start((8251, 6948), true), ((8301, 6958), -500));
        // 9 px a tick, rounded.
        assert_eq!((SPEED * 18 + 500) / 1000, 9);
    }

    #[test]
    fn it_fires_as_a_tower_cannon() {
        assert_eq!(PERIOD_MS.div_ceil(18), PERIOD_TICKS);
        assert_eq!(u64::from(PERIOD_MS), super::super::cannon::PERIOD_MS);
        assert_eq!(SPEED, super::super::cannon::SPEED);
        assert_eq!(DEFAULT_Z, super::super::cannon::DEFAULT_Z);
        assert_eq!(ball::MUZZLE, super::super::cannon::MUZZLE);
        assert_eq!(ball::LOGIC, super::super::cannon::BALL_LOGIC);
        // Seven pictures of 100 ms, six calls each, and the finishing call.
        assert_eq!(FIRE_TICKS, 7 * 100u32.div_ceil(18) + 1);
    }
}
