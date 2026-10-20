//! Omar's energy ball: cast in the second phase of his fight, it leaves
//! him [`super::CAST_OFFSET`] px out (mirrored facing left) at [`SPEED`]
//! px/s towards the way he faced when the cast started, and flies
//! straight with no gravity.
//!
//! - Cast ([`launch`]): an enemy shot dealing [`super::BALL_DAMAGE`] that
//!   nothing can hit, drawn at depth [`DEPTH`], with its picture's box
//!   ([`RECT`] flying right, turned for a ball flying left). It moves from
//!   the tick after the cast, on its own clock as the projectiles do
//!   ([`crate::logic::boss::step_seconds`]).
//! - Each tick: gone on the tick after it met a wall (or left the level);
//!   its glow; [`SPARKS`] sparks at random points of its box (a draw
//!   across, then one up and down, for each); its move through the tiles.
//! - On Claw it costs him its damage and Claw's own rule takes it away.
//!   Claw's respawn takes it away too.

use crate::level::runtime_image_set;
use crate::logic::boss::step_seconds;
use crate::logic::projectile::{Motion, RESPAWN_PURGE};
use crate::logic::{ObjectData, World, effect, new_object};
use crate::object::{ObjectId, Rect, draw_flags, flags, types};
use crate::physics::{ptype, res};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["OmarBullet"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_OMARPROJECTILE";

/// Its glow in flight.
pub const ANIM: &str = "GAME_CYCLE50";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play: its glow.
pub const ANIM_PREFIXES: &[&str] = &[ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Where it leaves Omar, facing left: across and up.
pub const OFFSET: (i32, i32) = (-65, -20);
/// Its speed, px/s.
pub const SPEED: i32 = 300;
/// Its box, flying right.
pub const RECT: Rect = Rect::new(-24, -23, 23, 22);
/// Its depth.
pub const DEPTH: i32 = 5000;
/// Sparks it sheds per tick.
pub const SPARKS: u32 = 2;

/// Its state while it flies, in its object's state.
pub const FLYING: i32 = 1;

/// An energy ball's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BulletData {
    /// Exact position and speed.
    pub motion: Motion,
    /// Its own clock, s; `None` before its first step.
    pub clock_s: Option<f64>,
    /// The tile physics' result of its last move.
    pub last_result: u32,
}

/// Casts a ball from `at` (x, y, the depth it is cast at) at `vx` px/s and
/// returns it. It joins the level at [`DEPTH`] and makes its first call on
/// the next tick.
pub fn launch(w: &mut World, at: (i32, i32, i32), vx: i32) -> ObjectId {
    let (x, y, _) = at;
    let mut o = new_object(NAMES[0], x, y, DEPTH);
    o.image_set = runtime_image_set(IMAGE_SET);
    o.frame = w.image_frames(IMAGE_SET).map_or(1, |(first, _)| first);
    let frame_box = w.frame_box(IMAGE_SET, o.frame).unwrap_or(RECT);
    o.move_rect = frame_box;
    o.attack_rect = frame_box;
    o.type_flags = types::ENEMY_SHOT;
    o.hit_type_flags = 0;
    o.damage = super::BALL_DAMAGE;
    o.special_flags |= RESPAWN_PURGE;
    o.flags |= flags::DESTROY_OUTSIDE | flags::DESTROY_FAR;
    o.physics_type = ptype::FLY;
    o.state = FLYING;
    o.speed_x = vx;
    // Cast in front of Omar's own call, it runs from the next tick.
    o.frame_delay = 1;
    if vx < 0 {
        o.draw_flags |= draw_flags::MIRROR;
        o.mirror_rects();
    }
    o.data = ObjectData::OmarBullet(BulletData {
        motion: Motion {
            x: f64::from(x),
            y: f64::from(y),
            vx: f64::from(vx),
            vy: 0.0,
            ay: 0.0,
        },
        clock_s: None,
        last_result: 0,
    });
    let id = w.spawn(o);
    w.set_anim(id, ANIM);
    id
}

/// One logic call (see the module documentation).
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state != FLYING {
        return;
    }
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::OmarBullet(d) => d,
        _ => BulletData::default(),
    };
    fly(w, id, &mut d);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::OmarBullet(d);
    }
}

fn fly(w: &mut World, id: ObjectId, d: &mut BulletData) {
    w.advance_anim(id);
    let outside = w.objects.get(id).is_some_and(|o| {
        o.x < 0 || o.y < 0 || o.x >= w.level.map.width() || o.y >= w.level.map.height()
    });
    let gone = d.last_result & res::HIT != 0 || outside;
    if gone {
        w.destroy(id);
    }
    let Some((rect, z)) = w.objects.get(id).map(|o| (o.move_rect, o.z)) else {
        return;
    };
    for _ in 0..SPARKS {
        effect::spawn_sparkle(w, d.motion.x, d.motion.y, rect, z - 1);
    }
    if gone {
        return;
    }
    let t = step_seconds(&mut d.clock_s, w.now_ms);
    d.motion.step(t);
    let (tx, ty) = (d.motion.x as i32, d.motion.y as i32);
    let r = w.physics(id, tx, ty, 0);
    if let Some(o) = w.objects.get(id) {
        if o.x != tx {
            d.motion.x = f64::from(o.x);
        }
        if o.y != ty {
            d.motion.y = f64::from(o.y);
        }
    }
    d.last_result = r;
}

/// `attacker` hit `id`. Nothing hits a ball.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` reached `victim`: Claw's own rule takes his hit and the ball.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ball_flies_five_and_a_half_pixels_a_tick() {
        assert_eq!(SPEED * 18, 5400, "5.4 px a tick, in thousandths");
        assert_eq!(
            OFFSET,
            (-super::super::CAST_OFFSET.0, super::super::CAST_OFFSET.1)
        );
        assert_eq!(RECT.mirrored(), Rect::new(-23, -23, 24, 22));
    }
}
