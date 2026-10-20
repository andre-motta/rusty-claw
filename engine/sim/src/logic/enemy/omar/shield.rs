//! A piece of Omar's ring: one of the nine pieces of fire or ice that
//! circle him in the first phase of his fight (see [`super::ring_angle`]).
//!
//! - Made by Omar's reset one depth in front of him ([`spawn`]): an enemy
//!   of no damage of its own, always active, with the box [`RECT`] for
//!   hitting and being hit, showing the fire ring ([`FIRE_SET`]).
//! - It never moves by itself: Omar puts it around him every tick and
//!   gives it the ring's element ([`show_element`]: the ice ring's
//!   picture [`ICE_SET`], its animation restarted when the element
//!   changes). Each tick its animation plays.
//! - A piece touching Claw costs him [`DAMAGE`] (an enemy's touch with no
//!   damage of its own costs one).
//! - It removes every pistol shot and magic shot that touches it (a shot
//!   that touches Omar in the same collision pass still hits him); Claw's
//!   blows, thrown objects and the level's shots do nothing to it.
//! - Only the end of Omar's first phase removes it.

use crate::level::runtime_image_set;
use crate::logic::enemy::attacker;
use crate::logic::{ObjectData, World, new_object};
use crate::object::{ObjectId, Rect, flags, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["OmarShield"];

/// The fire piece's picture.
pub const FIRE_SET: &str = "LEVEL_FIRESHIELD";
/// The ice piece's picture.
pub const ICE_SET: &str = "LEVEL_ICESHIELD";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[FIRE_SET, ICE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[FIRE_SET, ICE_SET];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// What touching a piece costs Claw.
pub const DAMAGE: i32 = 1;
/// A piece's hit and attack box.
pub const RECT: Rect = Rect::new(-14, -6, 12, 21);
/// Kinds of object that reach a piece: Claw, thrown objects, pistol
/// shots, the level's shots and magic.
pub const HIT_BY: u32 = 0x300_0032;

/// A piece's state, in its object's state.
pub const SHOWN: i32 = 1;

/// A piece's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShieldData {
    /// Omar, whose ring it belongs to.
    pub omar: Option<ObjectId>,
    /// Its place in the ring, 0 to 8.
    pub index: usize,
}

/// Makes piece `index` of `omar`'s ring at (x, y, depth) and returns it.
pub fn spawn(w: &mut World, omar: ObjectId, index: usize, at: (i32, i32, i32)) -> ObjectId {
    let (x, y, z) = at;
    let mut o = new_object(NAMES[0], x, y, z);
    o.image_set = runtime_image_set(FIRE_SET);
    o.frame = w.image_frames(FIRE_SET).map_or(1, |(first, _)| first);
    o.type_flags = types::ENEMY;
    o.hit_type_flags = HIT_BY;
    o.hit_handler = true;
    o.hit_rect = RECT;
    o.attack_rect = RECT;
    o.damage = 0;
    o.flags |= flags::ALWAYS_ACTIVE;
    o.state = SHOWN;
    o.data = ObjectData::OmarShield(ShieldData {
        omar: Some(omar),
        index,
    });
    let id = w.spawn(o);
    w.set_anim(id, FIRE_SET);
    id
}

/// Shows the ring's element on piece `id`: the ice ring's picture when
/// `ice`, the fire ring's otherwise, its animation restarted when it
/// changes.
pub fn show_element(w: &mut World, id: ObjectId, ice: bool) {
    let set = if ice { ICE_SET } else { FIRE_SET };
    let runtime = runtime_image_set(set);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.image_set == runtime {
        return;
    }
    o.image_set = runtime;
    if let Some((first, _)) = w.image_frames(set)
        && let Some(o) = w.objects.get_mut(id)
    {
        o.frame = first;
    }
    w.set_anim(id, set);
}

/// One logic call: its animation plays.
pub fn tick(w: &mut World, id: ObjectId) {
    if w.objects.get(id).is_some_and(|o| o.state == SHOWN) {
        w.advance_anim(id);
    }
}

/// `attacker` hit `id`: a pistol shot or a magic shot is used up.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = id;
    let kind = w.objects.get(attacker).map_or(0, |a| a.type_flags);
    if kind == attacker::PISTOL_SHOT || kind == attacker::MAGIC {
        w.destroy(attacker);
    }
}

/// `id` attacked `victim`: nothing more (Claw's own rule takes his hit).
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_box_and_what_reaches_it() {
        assert_eq!(
            (RECT.right - RECT.left + 1, RECT.bottom - RECT.top + 1),
            (27, 28)
        );
        assert_ne!(HIT_BY & attacker::MAGIC, 0);
        assert_ne!(HIT_BY & attacker::PISTOL_SHOT, 0);
        assert_ne!(HIT_BY & attacker::CLAW, 0);
        assert_eq!(HIT_BY & types::ENEMY_SHOT, 0);
    }
}
