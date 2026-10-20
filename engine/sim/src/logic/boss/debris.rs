//! The grass, leaves and sand RedTail's wind blows across his arena:
//! decoration only, out of every collision, flying left from the arena's
//! right side until it passes [`END_X`] or after [`LIFE_MS`]. Making a
//! piece draws from the shared random stream (its place, depth, look,
//! speed and drift), so the wind makes every piece to keep the stream in
//! step with the game.
//!
//! A piece ([`launch`]) is made at a random point of the arena's right
//! end ([`ORIGIN`] plus draws of 0..=100 across and 0..=180 down) at a
//! random depth (a draw of 0..=10000), with one of its three looks (a
//! draw of 0..3); it flies left at 300 to 600 px/s (a draw of 0..=100
//! percent above 300) and drifts up or down under an acceleration of 10
//! to 100 px/s/s (a draw of 0..=90, then a draw of 0..=100 that turns it
//! upwards from 50 on). It moves without tile tests from the tick after
//! the one it is made in, on its own clock as the projectiles do
//! ([`super::step_seconds`]).

use crate::level::runtime_image_set;
use crate::logic::projectile::Motion;
use crate::logic::{ObjectData, World, new_object};
use crate::object::{ObjectId, flags};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["WindDebris"];

/// Its three looks.
pub const LOOKS: [&str; 3] = ["LEVEL_WINDGRASS", "LEVEL_WINDLEAF", "LEVEL_WINDSAND"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &LOOKS;

/// Its animation.
pub const ANIM: &str = "GAME_CYCLE50";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// A piece is gone once it reaches this x, px.
pub const END_X: i32 = 36500;
/// A piece is gone after this time, ms.
pub const LIFE_MS: u32 = 10_000;
/// The corner of the arena pieces start from: draws of 0..=100 across and
/// 0..=180 down are added.
pub const ORIGIN: (i32, i32) = (37800, 1920);
/// The slowest speed of a piece, px/s (to the left).
pub const SPEED: i32 = -300;

/// A piece's states, in its object's state.
pub mod state {
    /// Flying.
    pub const MOVING: i32 = 1;
}

/// A piece's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DebrisData {
    /// Exact position, speed and drift.
    pub motion: Motion,
    /// Its own clock, s; `None` before its first step.
    pub clock_s: Option<f64>,
    /// When it was made, ms.
    pub made_ms: u64,
}

/// Makes a piece of debris (see the module documentation) and returns
/// it. Draws, in order: across, down, depth, look, speed, drift, its sign.
pub fn launch(w: &mut World) -> ObjectId {
    let x = ORIGIN.0 + w.rng.below(101) as i32;
    let y = ORIGIN.1 + w.rng.below(181) as i32;
    let z = w.rng.below(10_001) as i32;
    let look = LOOKS[w.rng.below(3) as usize];
    let mut o = new_object(NAMES[0], x, y, z);
    o.image_set = runtime_image_set(look);
    o.frame = w.image_frames(look).map_or(1, |(first, _)| first);
    if let Some(r) = w.frame_box(look, o.frame) {
        o.move_rect = r;
        o.hit_rect = r;
    }
    o.flags |= flags::NO_COLLIDE | flags::ALWAYS_ACTIVE;
    o.physics_type = ptype::DIRECT;
    o.state = state::MOVING;
    let vx = SPEED * (100 + w.rng.below(101) as i32) / 100;
    let mut ay = f64::from(w.rng.below(91) as i32 + 10);
    if w.rng.below(101) >= 50 {
        ay = -ay;
    }
    o.speed_x = vx;
    o.data = ObjectData::WindDebris(DebrisData {
        motion: Motion {
            x: f64::from(x),
            y: f64::from(y),
            vx: f64::from(vx),
            vy: 0.0,
            ay,
        },
        clock_s: None,
        made_ms: w.now_ms,
    });
    let id = w.spawn(o);
    w.set_anim(id, ANIM);
    id
}

/// One logic call: gone once it has reached [`END_X`] or after
/// [`LIFE_MS`]; otherwise its animation and its move.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state != state::MOVING {
        return;
    }
    let ended = o.x <= END_X;
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::WindDebris(d) => d,
        _ => DebrisData::default(),
    };
    if ended || w.now_ms.saturating_sub(d.made_ms) >= u64::from(LIFE_MS) {
        w.destroy(id);
    } else {
        w.advance_anim(id);
        let t = super::step_seconds(&mut d.clock_s, w.now_ms);
        d.motion.step(t);
        if let Some(o) = w.objects.get_mut(id) {
            o.x = d.motion.x as i32;
            o.y = d.motion.y as i32;
        }
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::WindDebris(d);
    }
}

/// `attacker` hit `id`. Nothing hits debris.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. Debris attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_fly_left_at_300_to_600_px_per_second() {
        let speed = |r: i32| SPEED * (100 + r) / 100;
        assert_eq!(speed(0), -300);
        assert_eq!(speed(100), -600);
        // The truncation keeps whole pixels per second toward zero.
        assert_eq!(speed(37), -411);
    }
}
