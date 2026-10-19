//! A lava hand's lava ball: an arching shot that rests where it lands
//! and bursts at its fuse.
//!
//! A ball leaves the hand aimed [`super::hand::AIM`] px to one side on the
//! crab bomb's arc ([`throw_velocity`]: 387.3 px/s across and 687.3 px/s
//! up, under 2000 px/s²) and flies through the tiles from the tick after
//! the hand's. It does not bounce: its first landing rests it where it
//! is. In flight and at rest it is an enemy shot of [`super::DAMAGE`]
//! with its picture's box ([`RECT`]): touching Claw hurts him and makes it
//! burst at once. Otherwise it bursts at its fuse, [`FUSE_MS`] after the
//! throw, wherever it is.
//!
//! The burst is a blast of the level's own kind with the same small box,
//! out of every hit itself: it hurts Claw and the enemies on the burst's
//! first tick and on the ninth tick after it, which its animation marks,
//! and is gone when its animation ends ([`BLAST_TICKS`] ticks at the 18 ms
//! step).
//!
//! While it flies and rests, every tick sheds two sparks at random points
//! of its box (two draws each, across then down), the burst's tick
//! included; none while it bursts. Nothing else of it draws.

use crate::logic::enemy::crab;
use crate::logic::projectile::{Motion, RESPAWN_PURGE};
use crate::logic::{ObjectData, World, effect, new_object};
use crate::object::{ObjectId, Rect, flags, special, types};
use crate::physics::{ptype, res};
use crate::tuning::TICK_MS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["LavaHandProjectile"];

/// Its picture in flight.
pub const IMAGE_SET: &str = "LEVEL_LAVAHANDPROJECTILE";
/// The burst's picture.
pub const BLAST_SET: &str = "GAME_DYNAMITEEXPLO";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, BLAST_SET];

/// Its flight.
pub const FLIGHT_ANIM: &str = "LEVEL_LAVAHANDPROJECTILE";
/// The burst.
pub const BLAST_ANIM: &str = "GAME_DYNAMITEEXPLO";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[FLIGHT_ANIM, BLAST_ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Its box in flight, at rest and in the burst: its picture's.
pub const RECT: Rect = Rect::new(-18, -16, 17, 15);
/// Time from the throw to the burst, ms.
pub const FUSE_MS: u64 = 1000;
/// Gravity on a ball, px/s².
pub const GRAVITY: f64 = 2000.0;
/// Sparks shed each tick of flight and rest.
pub const SPARKS: u32 = 2;
/// Ticks of the burst at the 18 ms step.
pub const BLAST_TICKS: u64 = 73;
/// The burst's sound.
pub const BLAST_SOUND: &str = "GAME_KEGBOOM";

/// A ball's states, in its object's state.
pub mod state {
    /// Flying or resting.
    pub const FALLING: i32 = 1;
    /// Bursting.
    pub const EXPLODING: i32 = 2;
}

/// The throw's velocity for a ball aimed `distance` px to one side, px/s:
/// the crab bomb's (see [`crab::bomb::throw_velocity`]).
pub fn throw_velocity(distance: i32) -> (f64, f64) {
    crab::bomb::throw_velocity(distance)
}

/// A ball's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BallData {
    /// Exact position, velocity and gravity.
    pub motion: Motion,
    /// Its own clock, s; `None` before its first step.
    pub clock_s: Option<f64>,
    /// When the fuse was lit, ms.
    pub fuse_start_ms: u64,
    /// No motion: resting or bursting.
    pub frozen: bool,
    /// The burst has passed its marked tick.
    pub latched: bool,
}

/// The ball data of `o`, if it is a ball.
pub fn data_of(o: &crate::object::Object) -> Option<&BallData> {
    match &o.data {
        ObjectData::LavaHandProjectile(d) => Some(d),
        _ => None,
    }
}

/// Throws a ball from (x, y) at depth z, aimed `distance` px to one side,
/// and returns it. Its fuse is lit now; it first moves on the next tick.
pub fn spawn(w: &mut World, x: i32, y: i32, z: i32, distance: i32) -> ObjectId {
    let mut o = new_object(NAMES[0], x, y, z);
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    o.frame = w.image_frames(IMAGE_SET).map_or(1, |(first, _)| first);
    o.move_rect = RECT;
    o.attack_rect = RECT;
    o.type_flags = types::ENEMY_SHOT;
    o.attack_type_flags = types::PLAYER;
    o.attack_handler = true;
    o.hit_type_flags = 0;
    o.damage = super::DAMAGE;
    o.special_flags |= special::OWN_IMPACT | RESPAWN_PURGE;
    o.flags |= flags::ALWAYS_ACTIVE | flags::DESTROY_OUTSIDE | flags::DESTROY_FAR;
    o.physics_type = ptype::FLY;
    o.state = state::FALLING;
    let (vx, vy) = throw_velocity(distance);
    o.speed_x = vx as i32;
    o.speed_y = vy as i32;
    o.data = ObjectData::LavaHandProjectile(BallData {
        motion: Motion {
            x: f64::from(x),
            y: f64::from(y),
            vx,
            vy,
            ay: GRAVITY,
        },
        fuse_start_ms: w.now_ms,
        ..BallData::default()
    });
    let id = w.spawn(o);
    w.set_anim(id, FLIGHT_ANIM);
    id
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let s = o.state;
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::LavaHandProjectile(d) => d,
        _ => BallData::default(),
    };
    match s {
        state::FALLING => falling(w, id, &mut d),
        state::EXPLODING => exploding(w, id, &mut d),
        _ => {}
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::LavaHandProjectile(d);
    }
}

/// Flying or resting: the fuse, the landing that rests it, the move, then
/// the sparks.
fn falling(w: &mut World, id: ObjectId, d: &mut BallData) {
    w.advance_anim(id);
    if w.now_ms.saturating_sub(d.fuse_start_ms) >= FUSE_MS {
        d.frozen = true;
        explode(w, id);
    }
    if w.objects
        .get(id)
        .is_some_and(|o| o.physics_type == ptype::STAND)
    {
        d.frozen = true;
    }
    integrate(w, id, d);
    let Some((x, y, z)) = w.objects.get(id).map(|o| (o.x, o.y, o.z)) else {
        return;
    };
    for _ in 0..SPARKS {
        effect::spawn_sparkle(w, f64::from(x), f64::from(y), RECT, z - 1);
    }
}

/// Bursting: out of every hit by its own blast except on the marked tick;
/// gone when the burst ends.
fn exploding(w: &mut World, id: ObjectId, d: &mut BallData) {
    if let Some(o) = w.objects.get_mut(id) {
        o.flags |= flags::HARMLESS;
    }
    if w.advance_anim(id) == 1 && !d.latched {
        d.latched = true;
        if let Some(o) = w.objects.get_mut(id) {
            o.flags &= !flags::HARMLESS;
        }
    }
    if w.objects.get(id).is_none_or(|o| o.anim.is_done()) {
        w.destroy(id);
    }
}

/// Starts the burst where the ball is: safe from hits, the level's own
/// kind of shot, its burst animation and sound.
fn explode(w: &mut World, id: ObjectId) {
    let first = w.image_frames(BLAST_SET).map_or(1, |(f, _)| f);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.state = state::EXPLODING;
    o.flags |= flags::SAFE;
    o.type_flags = types::LEVEL_SHOT;
    o.image_set = crate::level::runtime_image_set(BLAST_SET);
    o.frame = first;
    w.set_anim(id, BLAST_ANIM);
    effect::boom(w, BLAST_SOUND);
}

/// `attacker` hit `id`: nothing hits a ball.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` reached `victim`: when that is Claw and he takes the hit, the
/// ball bursts where it is (his own hit rule applies the damage); a
/// bursting ball does not burst again.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    if w.claw_id != Some(victim) {
        return;
    }
    let (Some(b), Some(c)) = (w.objects.get(id), w.objects.get(victim)) else {
        return;
    };
    if b.state != state::FALLING || b.type_flags & c.hit_type_flags == 0 || w.aids.invincible {
        return;
    }
    let mut d = data_of(b).cloned().unwrap_or_default();
    d.frozen = true;
    explode(w, id);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::LavaHandProjectile(d);
    }
}

/// The time the motion step of the tick covers, s: one tick at first,
/// then the game time since the ball's own clock.
fn step_seconds(d: &mut BallData, now_ms: u64) -> f64 {
    let now = (now_ms + u64::from(TICK_MS)) as f64 * 0.001;
    match d.clock_s {
        Some(prev) => {
            let dt = now - prev;
            d.clock_s = Some(prev + dt);
            dt
        }
        None => {
            d.clock_s = Some(now);
            f64::from(TICK_MS) / 1000.0
        }
    }
}

/// One motion step through the tiles (not the platforms), unless it is
/// frozen.
fn integrate(w: &mut World, id: ObjectId, d: &mut BallData) {
    if d.frozen {
        return;
    }
    let t = step_seconds(d, w.now_ms);
    d.motion.step(t);
    let (tx, ty) = (d.motion.x as i32, d.motion.y as i32);
    let r = w.physics(id, tx, ty, 0);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.x != tx {
        d.motion.x = f64::from(o.x);
    }
    if o.y != ty {
        d.motion.snap_y(f64::from(o.y));
    }
    if r & res::CEILING != 0 {
        d.motion.vy = -d.motion.vy;
    } else if r & (res::RIGHT | res::LEFT) != 0 {
        d.motion.vx *= -0.5;
    }
    o.speed_x = d.motion.vx as i32;
    o.speed_y = d.motion.vy as i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_leaves_on_the_crab_bombs_arc() {
        let (vx, vy) = throw_velocity(-150);
        assert!((vx + 387.298).abs() < 1e-3, "{vx}");
        assert!((vy + 687.298).abs() < 1e-3, "{vy}");
        // Its fuse: the 56th tick after the throw.
        assert_eq!(FUSE_MS.div_ceil(18), 56);
        // Its box is 36 by 32 px.
        assert_eq!(
            (RECT.right - RECT.left + 1, RECT.bottom - RECT.top + 1),
            (36, 32)
        );
    }
}
