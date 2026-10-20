//! RedTail's flares: fired from his flare gun at a Claw on his own level
//! ([`crate::logic::enemy::redtail`]), a flare flies straight at
//! [`SPEED`] px/s through the tiles, glowing and shedding [`SPARKLES`]
//! sparkles a tick; it is gone on the tick after it meets a wall or
//! floor, or leaves the level, and when Claw respawns. On Claw it bursts
//! where it is, hurting him for [`DAMAGE`].
//!
//! # The rules
//!
//! - Fired ([`launch`]): an enemy shot dealing [`DAMAGE`] that nothing
//!   can hit, its box from its first picture ([`FRAME_BOX`] flying
//!   right, turned with its picture for a flare flying left), a light
//!   one depth behind it. It joins the level one depth behind RedTail and
//!   moves from the next tick, on its own clock as the projectiles do
//!   ([`super::step_seconds`]).
//! - In flight, every tick: gone on the tick after it touched a wall (or
//!   outside the level); its sparkles, each at a random point of its box
//!   (a draw across, then one up and down); its light brought to it; its
//!   move.
//! - On Claw (see [`on_attack`]): it bursts where it is, frozen in place
//!   and safe from every hit, the level's own kind of shot with the same
//!   box and damage, playing its explosion; Claw's shot rule takes the
//!   damage and leaves it. Claw, just hurt, is shielded from it; RedTail,
//!   whom the level's shots reach, hops back when it finds him free. It is
//!   gone when the explosion ends.

use crate::level::runtime_image_set;
use crate::logic::projectile::{Motion, RESPAWN_PURGE};
use crate::logic::{ObjectData, World, effect, new_object};
use crate::object::{ObjectId, Rect, draw_flags, flags, special, types};
use crate::physics::{ptype, res};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTailBullet"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_REDTAILBULLET";
/// The picture of its explosion.
pub const EXPLOSION_SET: &str = "LEVEL_REDTAILBULLET_EXPLOSION";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, EXPLOSION_SET];

/// The explosion.
pub const EXPLOSION_ANIM: &str = "LEVEL_REDTAILBULLETEXPLOSION";
/// Its glow in flight.
pub const FLIGHT_ANIM: &str = "GAME_CYCLE50";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[EXPLOSION_ANIM, FLIGHT_ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// What a flare costs Claw.
pub const DAMAGE: i32 = 15;
/// Its speed across, px/s.
pub const SPEED: i32 = 500;
/// Sparkles a flare sheds per tick in flight.
pub const SPARKLES: u32 = 2;
/// Its box, flying right, when the level does not know its first
/// picture; turned to face left for a flare flying left.
pub const FRAME_BOX: Rect = Rect::new(-6, -2, 5, 3);

/// A flare's states, in its object's state.
pub mod state {
    /// Flying.
    pub const FLYING: i32 = 1;
    /// Bursting.
    pub const EXPLODING: i32 = 2;
}

/// A flare's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlareData {
    /// Exact position and speed.
    pub motion: Motion,
    /// Its own clock, s; `None` before its first step.
    pub clock_s: Option<f64>,
    /// The tile physics' result of its last move.
    pub last_result: u32,
    /// Its light.
    pub light: Option<ObjectId>,
}

/// Fires a flare for `owner` from `at` (x, y, depth) at `vx` px/s and
/// returns it.
pub fn launch(w: &mut World, owner: ObjectId, at: (i32, i32, i32), vx: i32) -> ObjectId {
    let (x, y, z) = at;
    let mut o = new_object(NAMES[0], x, y, z);
    o.image_set = runtime_image_set(IMAGE_SET);
    o.frame = w.image_frames(IMAGE_SET).map_or(1, |(first, _)| first);
    let frame_box = w.frame_box(IMAGE_SET, o.frame).unwrap_or(FRAME_BOX);
    o.move_rect = frame_box;
    o.attack_rect = frame_box;
    o.type_flags = types::ENEMY_SHOT;
    o.attack_type_flags = types::PLAYER;
    o.attack_handler = true;
    o.hit_type_flags = 0;
    o.damage = DAMAGE;
    o.special_flags |= special::OWN_IMPACT | RESPAWN_PURGE;
    o.flags |= flags::DESTROY_OUTSIDE | flags::DESTROY_FAR | flags::NOTIFY_DESTROY;
    o.physics_type = ptype::FLY;
    o.state = state::FLYING;
    o.speed_x = vx;
    if vx < 0 {
        o.draw_flags |= draw_flags::MIRROR;
        o.mirror_rects();
    }
    let id = w.spawn(o);
    let light = effect::spawn_flare(w, id, x, y, z - 1);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::RedTailBullet(FlareData {
            motion: Motion {
                x: f64::from(x),
                y: f64::from(y),
                vx: f64::from(vx),
                vy: 0.0,
                ay: 0.0,
            },
            clock_s: None,
            last_result: 0,
            light: Some(light),
        });
    }
    w.set_anim(id, FLIGHT_ANIM);
    let _ = owner;
    id
}

/// One logic call, flying or bursting (see the module documentation).
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let st = o.state;
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::RedTailBullet(d) => d,
        _ => FlareData::default(),
    };
    match st {
        state::FLYING => fly(w, id, &mut d),
        state::EXPLODING => {
            w.advance_anim(id);
            if w.objects.get(id).is_none_or(|o| o.anim.is_done()) {
                w.destroy(id);
            }
        }
        _ => {}
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::RedTailBullet(d);
    }
}

fn fly(w: &mut World, id: ObjectId, d: &mut FlareData) {
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
    for _ in 0..SPARKLES {
        effect::spawn_sparkle(w, d.motion.x, d.motion.y, rect, z - 1);
    }
    if let Some(light) = d.light
        && let Some(l) = w.objects.get_mut(light)
    {
        l.x = d.motion.x as i32;
        l.y = d.motion.y as i32;
    }
    if gone {
        return;
    }
    let t = super::step_seconds(&mut d.clock_s, w.now_ms);
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

/// Bursts the flare where it is (see the module documentation).
fn burst(w: &mut World, id: ObjectId) {
    let first = w.image_frames(EXPLOSION_SET).map(|(f, _)| f);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.state = state::EXPLODING;
    o.type_flags = types::LEVEL_SHOT;
    o.attack_type_flags = 0;
    o.flags |= flags::SAFE;
    o.image_set = runtime_image_set(EXPLOSION_SET);
    if let Some(f) = first {
        o.frame = f;
    }
    w.set_anim(id, EXPLOSION_ANIM);
}

/// `attacker` hit `id`. Nothing hits a flare.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` reached `victim`: when that is Claw and he takes the hit, the
/// flare bursts where it is. His own shot rule applies the damage.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    if w.claw_id != Some(victim) {
        return;
    }
    let (Some(f), Some(c)) = (w.objects.get(id), w.objects.get(victim)) else {
        return;
    };
    if f.state != state::FLYING || f.type_flags & c.hit_type_flags == 0 || w.aids.invincible {
        return;
    }
    burst(w, id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flare_flies_nine_pixels_a_tick() {
        assert_eq!(SPEED * 18 / 1000, 9);
        assert_eq!(FRAME_BOX.mirrored(), Rect::new(-5, -2, 6, 3));
    }
}
