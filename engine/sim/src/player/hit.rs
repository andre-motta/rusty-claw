//! What happens when something touches Claw: damage from enemies and shots
//! now, pickups later.
//!
//! The pair-collision pass calls [`claw_hit`] when an object whose kind
//! Claw can be hit by overlaps him. The common damage step is in place: an
//! enemy's body (unless its touch is harmless) or a shot takes its damage
//! (1 when it has none) from Claw's health and opens an 850 ms window in
//! which enemies' bodies and the level's own shots cannot hurt him again;
//! enemies' shots can. Health at 0 or below kills him. The hit reaction
//! state, sounds, knockback, the hit burst and pickups are not filled in
//! yet; other kinds of object have no effect.

use super::Ctx;
use super::death::{self, DeathCause};
use crate::object::{ObjectId, special, types};

/// Length of the window after a hit in which enemies cannot hurt Claw, ms.
pub const INVULNERABLE_MS: i32 = 850;

/// `attacker` touched Claw.
pub fn claw_hit(c: &mut Ctx, attacker: ObjectId) {
    let Some(a) = c.objects.get(attacker) else {
        return;
    };
    let shielded = match a.type_flags {
        types::ENEMY => {
            if a.special_flags & special::HARMLESS_TOUCH != 0
                || c.p.special.lifted == Some(attacker)
            {
                return;
            }
            types::ENEMY
        }
        types::THROWN | types::ENEMY_SHOT | types::LEVEL_SHOT => types::CLAW_SHIELDED,
        _ => return,
    };
    let damage = if a.damage == 0 { 1 } else { a.damage };
    c.p.stats.invulnerable_ms = INVULNERABLE_MS;
    c.o.hit_type_flags &= !shielded;
    take_damage(c, damage);
}

/// The common damage step: `damage` comes off Claw's health, and health at
/// 0 or below kills him. Negative damage does nothing.
pub fn take_damage(c: &mut Ctx, damage: i32) {
    if damage < 0 {
        return;
    }
    c.p.stats.health -= damage;
    if c.p.stats.health <= 0 {
        death::kill(c, DeathCause::Damage);
    }
}
