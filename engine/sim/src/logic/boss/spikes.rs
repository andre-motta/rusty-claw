//! The stakes at the left wall of RedTail's arena: a static hazard that
//! hurts Claw for [`DAMAGE`] on touch. Nothing drives them, they never
//! animate and nothing can hit them; a Claw held against them (by the
//! wind) is hurt again each time his protection after a hit ends.
//!
//! Built on their first call: an enemy's body to Claw's hit rule, with
//! [`ATTACK_RECT`] (their picture's box) as their attack box and no hit
//! box. They draw nothing from the random stream.

use crate::level::runtime_image_set;
use crate::logic::World;
use crate::object::{ObjectId, Rect, engine_state, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTailSpikes"];

/// Their picture.
pub const IMAGE_SET: &str = "LEVEL_BOSSPIKES";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play: none.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// What touching them costs Claw, before the difficulty factor.
pub const DAMAGE: i32 = 20;

/// Their attack box: their picture's.
pub const ATTACK_RECT: Rect = Rect::new(-32, -104, 31, 103);

/// Their state once built.
pub const IDLE: i32 = 5;

/// The stakes' data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpikesData {}

/// One logic call: the first builds them; then nothing.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state != engine_state::INIT {
        return;
    }
    o.image_set = runtime_image_set(IMAGE_SET);
    o.type_flags = types::ENEMY;
    o.hit_type_flags = 0;
    o.attack_rect = ATTACK_RECT;
    o.hit_rect = Rect::UNSET;
    o.damage = crate::logic::enemy::scaled(DAMAGE, crate::logic::enemy::DAMAGE_FACTOR);
    o.state = IDLE;
}

/// `attacker` hit `id`. Nothing hits the stakes.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. Claw's own hit rule decides.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stakes_box_is_their_picture() {
        assert_eq!(ATTACK_RECT.right - ATTACK_RECT.left + 1, 64);
        assert_eq!(ATTACK_RECT.bottom - ATTACK_RECT.top + 1, 208);
    }
}
