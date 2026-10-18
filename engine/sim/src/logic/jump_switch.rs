//! Jump switches: invisible markers on a ledge where an enemy leaps to a
//! chosen point.
//!
//! A switch has no update, no state, no sound and no picture (its record
//! hides it). It is an attacker in pair collision: a small box
//! ([`ATTACK_RECT`]) of the jump switch kind ([`TYPE`]) that reaches the
//! human enemies, whose hit masks include that kind. The enemy's hit rule
//! turns the hit into a leap: it forgets its patrol limits, starts its
//! jump (when standing) and flies on a ballistic arc through the switch's
//! target point, the record's `x_min`, `y_min` ([`target`]), then falls on
//! to whatever floor lies below. The record's `x_max`, `y_max` are never
//! read. Because the leap goes through the enemy's hit gate, an enemy
//! takes a switch once per attack of Claw and not during a hit cooldown.
//! Claw, crates, kegs, the rat and the seagull are never launched.
//!
//! Not built yet ([`BUILT`] is false): switches stay inert placeholders
//! (not drawn, no collision, reported as not implemented), and the calls
//! below do nothing.
//!
//! # Defaults taken
//!
//! - In the widened activation mode a switch keeps the original window,
//!   like the breakables ([`crate::activation::ORIGINAL_WINDOW_LOGICS`]):
//!   a pacing guard sets it off, so waking it earlier would launch guards
//!   before Claw arrives.
//! - The enemy side (the guards' launch) reaches every guard, free or
//!   busy, that passes the shared hit gate; see
//!   [`super::enemy::townguard`].

use super::World;
use super::enemy::field;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["JumpSwitch"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// The switch's attack box.
pub const ATTACK_RECT: Rect = Rect::new(-5, -5, 5, 5);
/// The switch's kind: the human enemies' hit masks include it.
pub const TYPE: u32 = 0x1000;
/// What hits a switch: nothing does, since it has no hit box.
pub const HIT_MASK: u32 = 0x1;

/// The point a switch placed with `record` launches enemies through: its
/// `x_min`, `y_min` fields.
pub fn target(record: &[i32]) -> (i32, i32) {
    (record[field::X_MIN], record[field::Y_MIN])
}

/// A jump switch's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JumpSwitchData {}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// `attacker` hit `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_is_the_records_lower_limits() {
        let mut record = [0; 64];
        record[field::X_MIN] = 2227;
        record[field::Y_MIN] = 2646;
        record[field::X_MAX] = 2258;
        record[field::Y_MAX] = 2667;
        assert_eq!(target(&record), (2227, 2646));
    }

    #[test]
    fn its_box_is_eleven_px_square() {
        assert_eq!(ATTACK_RECT.right - ATTACK_RECT.left + 1, 11);
        assert_eq!(ATTACK_RECT.bottom - ATTACK_RECT.top + 1, 11);
        assert_eq!(TYPE & HIT_MASK, 0);
    }
}
