//! RedTail's knives: thrown three at a time at a Claw on the other level
//! of his arena ([`crate::logic::enemy::redtail`]), they fly straight
//! through everything at the speed they were given, spinning, and hurt
//! Claw for [`DAMAGE`] (his shot rule takes the knife away); one is gone
//! below the sand ([`GONE_BELOW`]) or after [`LIFE_MS`], and when Claw
//! respawns.
//!
//! A knife is an enemy shot that nothing can hit. It joins the level one
//! depth behind RedTail and moves from the next tick, on its own clock as
//! the projectiles do ([`super::step_seconds`]), without tile tests; one
//! flying left has its picture and boxes turned to face left.

use crate::level::runtime_image_set;
use crate::logic::projectile::{Motion, RESPAWN_PURGE};
use crate::logic::{ObjectData, World, new_object};
use crate::object::{ObjectId, Rect, draw_flags, flags, types};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTailKnife"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_REDTAILKNIFE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Its spin.
pub const SPIN_ANIM: &str = "GAME_CYCLE50";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[SPIN_ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// What a knife costs Claw.
pub const DAMAGE: i32 = 5;
/// A knife is gone once below this height, px.
pub const GONE_BELOW: i32 = 2150;
/// A knife is gone after this time, ms.
pub const LIFE_MS: u32 = 10_000;
/// Its box, flying right, when the level does not know its first
/// picture; turned to face left for a knife flying left.
pub const FRAME_BOX: Rect = Rect::new(-10, -15, 9, 14);

/// A knife's states, in its object's state.
pub mod state {
    /// Flying.
    pub const FLYING: i32 = 1;
}

/// A knife's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KnifeData {
    /// Exact position and speed.
    pub motion: Motion,
    /// Its own clock, s; `None` before its first step.
    pub clock_s: Option<f64>,
    /// When it was thrown, ms.
    pub thrown_ms: u64,
}

/// Throws a knife from `at` (x, y, depth) at `speed` (px/s) for `owner`
/// and returns it.
pub fn launch(w: &mut World, owner: ObjectId, at: (i32, i32, i32), speed: (i32, i32)) -> ObjectId {
    let _ = owner;
    let (x, y, z) = at;
    let mut o = new_object(NAMES[0], x, y, z);
    o.image_set = runtime_image_set(IMAGE_SET);
    o.frame = w.image_frames(IMAGE_SET).map_or(1, |(first, _)| first);
    let frame_box = w.frame_box(IMAGE_SET, o.frame).unwrap_or(FRAME_BOX);
    o.move_rect = frame_box;
    o.attack_rect = frame_box;
    o.type_flags = types::ENEMY_SHOT;
    o.hit_type_flags = 0;
    o.damage = DAMAGE;
    o.special_flags |= RESPAWN_PURGE;
    o.flags |= flags::DESTROY_OUTSIDE | flags::DESTROY_FAR;
    o.physics_type = ptype::DIRECT;
    o.state = state::FLYING;
    o.speed_x = speed.0;
    o.speed_y = speed.1;
    if speed.0 < 0 {
        o.draw_flags |= draw_flags::MIRROR;
        o.mirror_rects();
    }
    o.data = ObjectData::RedTailKnife(KnifeData {
        motion: Motion {
            x: f64::from(x),
            y: f64::from(y),
            vx: f64::from(speed.0),
            vy: f64::from(speed.1),
            ay: 0.0,
        },
        clock_s: None,
        thrown_ms: w.now_ms,
    });
    let id = w.spawn(o);
    w.set_anim(id, SPIN_ANIM);
    id
}

/// One logic call: the spin; gone below [`GONE_BELOW`] or after
/// [`LIFE_MS`]; otherwise its move.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state != state::FLYING {
        return;
    }
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::RedTailKnife(d) => d,
        _ => KnifeData::default(),
    };
    w.advance_anim(id);
    let old = w.now_ms.saturating_sub(d.thrown_ms) >= u64::from(LIFE_MS);
    let low = w.objects.get(id).is_some_and(|o| o.y > GONE_BELOW);
    if old || low {
        w.destroy(id);
    } else {
        let t = super::step_seconds(&mut d.clock_s, w.now_ms);
        d.motion.step(t);
        if let Some(o) = w.objects.get_mut(id) {
            o.x = d.motion.x as i32;
            o.y = d.motion.y as i32;
        }
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::RedTailKnife(d);
    }
}

/// `attacker` hit `id`. Nothing hits a knife.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. Claw's shot rule decides.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_knife_lasts_556_ticks_at_most() {
        assert_eq!(LIFE_MS.div_ceil(18), 556);
        // Flying left its box is turned.
        assert_eq!(FRAME_BOX.mirrored(), Rect::new(-9, -15, 10, 14));
    }
}
