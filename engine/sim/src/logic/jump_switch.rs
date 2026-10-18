//! Jump switches: invisible markers on a ledge where an enemy leaps to a
//! chosen point.
//!
//! A switch has no update, no state, no sound and no picture (its record
//! hides it). It is an attacker in pair collision: a small box
//! ([`ATTACK_RECT`]) of the jump switch kind ([`TYPE`]) that reaches the
//! human enemies, whose hit masks include that kind. The enemy's hit rule
//! turns the hit into a leap ([`launch`]): it forgets its patrol limits,
//! starts its jump (when standing) and flies on a ballistic arc through
//! the switch's target point, the record's `x_min`, `y_min` ([`target`]),
//! then falls on to whatever floor lies below. The record's `x_max`,
//! `y_max` are never read. Because the leap goes through the enemy's hit
//! gate, an enemy takes a switch once per attack of Claw and not during a
//! hit cooldown. Claw, crates, kegs, the rat and the seagull are never
//! launched. An enemy that is jumping, hopping back, carried or thrown
//! (safe from hits) takes no switch.
//!
//! The arc starts from the enemy's exact position on the tick its box
//! first meets the switch's (the collision pass comes after every
//! object's update, so the enemy has already moved on that tick); the
//! jump itself shows from the next tick. With the enemies' gravity of
//! 2000 px/s^2 the flight to the target takes `sqrt(|d| / 1000)` s for a
//! target `d` px away, and nothing stops the enemy there: no target of the
//! fifth level lies on a floor, so a guard passes within a few pixels of
//! it and lands on the floor below, 30 to 240 px lower.
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

use super::enemy::human::{self, launch as ballistic};
use super::enemy::{UNBOUNDED, data, data_mut, field};
use super::{ObjectData, World};
use crate::object::{ObjectId, Rect, engine_state};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["JumpSwitch"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

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

/// The state of a switch once it is set up; it never changes.
pub const SET_UP: i32 = 1000;

/// A jump switch's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JumpSwitchData {}

/// One logic call: the first sets the switch up (its box, its kind and
/// its hit mask); the switch does nothing after that.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state != engine_state::INIT {
        return;
    }
    o.state = SET_UP;
    o.attack_rect = ATTACK_RECT;
    o.type_flags = TYPE;
    o.hit_type_flags = HIT_MASK;
    o.attack_type_flags = 0;
    o.hit_handler = false;
    o.data = ObjectData::JumpSwitch(JumpSwitchData {});
}

/// `attacker` hit `id`: nothing hits a switch.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`: the victim's hit rule does the work.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

/// The launch speed (vx, vy) from (dx, dy) px short of the target under
/// the downward acceleration `ay`: the ballistic arc that passes through
/// the target after `sqrt(2 |d| / ay)` s; none for a target straight
/// below or no distance away.
pub fn launch_speed(dx: f64, dy: f64, ay: f64) -> (f64, f64) {
    ballistic(dx, dy, ay)
}

/// The exact position of enemy `id`: its position plus the fractions its
/// last movement step carried (dropped when something else moved it
/// since).
fn exact_position(w: &World, id: ObjectId) -> Option<(f64, f64)> {
    let o = w.objects.get(id)?;
    let carry = data(w, id)
        .filter(|d| d.moved_to == Some((o.x, o.y)))
        .map_or((0.0, 0.0), |d| d.carry);
    Some((f64::from(o.x) + carry.0, f64::from(o.y) + carry.1))
}

/// Switch `switch` hit the human enemy `id` (after the enemy's hit gate):
/// the enemy loses its patrol limits, starts its jump when it is standing
/// ([`human::enter_jump`]: busy, safe, flying, its jump animation) and, in
/// every case, flies towards the switch's target from its exact position.
/// No damage, no cooldown, no sound and no draw. On landing the jump ends
/// as any jump does: hits allowed again, Home, free.
pub fn launch(w: &mut World, id: ObjectId, switch: ObjectId) {
    let Some((tx, ty)) = w
        .objects
        .get(switch)
        .and_then(|s| s.record.as_deref())
        .map(|r| target(r))
    else {
        return;
    };
    if let Some(d) = data_mut(w, id) {
        d.bounds = UNBOUNDED;
        d.bounds_y = UNBOUNDED;
    }
    human::enter_jump(w, id);
    let Some((x, y)) = exact_position(w, id) else {
        return;
    };
    if let Some(d) = data_mut(w, id) {
        let (vx, vy) = launch_speed(f64::from(tx) - x, f64::from(ty) - y, d.ay);
        d.vx = vx;
        d.vy = vy;
    }
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
    fn the_first_switch_of_the_fifth_level_sends_its_guard_down_and_left() {
        // From (2426, 2622) to (2227, 2646): about 0.448 s of flight.
        let (vx, vy) = launch_speed(-199.0, 24.0, 2000.0);
        assert!((vx - -444.5).abs() < 0.5, "{vx}");
        assert!((vy - -394.1).abs() < 0.5, "{vy}");
        // The arc passes through the target at the solve time.
        let t = (f64::hypot(199.0, 24.0) / 1000.0).sqrt();
        let x = vx * t;
        let y = vy * t + 1000.0 * t * t;
        assert!((x + 199.0).abs() < 1e-9 && (y - 24.0).abs() < 1e-9);
        // A target straight below gives no speed at all.
        assert_eq!(launch_speed(0.0, 50.0, 2000.0), (0.0, 0.0));
    }

    #[test]
    fn its_box_is_eleven_px_square() {
        assert_eq!(ATTACK_RECT.right - ATTACK_RECT.left + 1, 11);
        assert_eq!(ATTACK_RECT.bottom - ATTACK_RECT.top + 1, 11);
        assert_eq!(TYPE & HIT_MASK, 0);
    }
}
