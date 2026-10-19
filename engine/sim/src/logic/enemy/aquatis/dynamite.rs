//! The hidden helper of Aquatis's room ([`super`]) that puts a stick of
//! dynamite back on a ledge of the room.
//!
//! - Built: hidden, always active, hit only by the level's shots and
//!   explosions, its box the rectangle its record's limits give (the whole
//!   room). It remembers how many sticks Claw carries and refills (below),
//!   which always puts the first stick down.
//! - The refill: when Claw carries more sticks than it remembers, nothing;
//!   otherwise it remembers his count, the link's refill flag is set, and
//!   a stick of dynamite ([`STICK_IMAGE`], one stick) is put down at its
//!   own position ([`STICK_AT`] on the twelfth level) at depth
//!   [`STICK_Z`].
//! - A blast anywhere in its box, at most once every [`GATE_MS`]: with
//!   Aquatis alive and the refill flag clear, the refill. The gate starts
//!   again from every blast it answers.
//! - Each call: once a stick it put down has been taken, the refill flag
//!   is cleared, as Claw's taking of a stick of dynamite does while
//!   Aquatis is the current boss. Aquatis's respawn reset clears the flag
//!   too.
//!
//! It draws nothing from the random streams.
//!
//! # Defaults taken
//!
//! - The refill after the first stick has been taken has not been seen on
//!   the original: it follows the rule above (a blast in the room, the
//!   flag clear, Claw holding no more sticks than at the last refill).
//! - The refill flag is cleared on the helper's call after the stick is
//!   taken (the original clears it as Claw takes it); a blast in between
//!   cannot reach the helper within its gate in any route met.

use crate::logic::{ObjectData, World, new_object};
use crate::object::{ObjectId, Rect, draw_flags, flags, types};

use super::super::{Timer, field};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["AquatisDynamite"];

/// The stick's picture.
pub const IMAGE_SET: &str = "GAME_DYNAMITE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play: none.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// The shortest time between two blasts it answers, ms.
pub const GATE_MS: u32 = 750;
/// Where the stick waits on the twelfth level: the helper's position.
pub const STICK_AT: (i32, i32) = (37250, 3490);
/// The logic of the stick it puts down.
pub const STICK_LOGIC: &str = "AmmoPowerup";
/// The stick's picture.
pub const STICK_IMAGE: &str = IMAGE_SET;
/// The stick's depth.
pub const STICK_Z: i32 = 1000;
/// Sticks in one stick it puts down.
pub const STICK_COUNT: i32 = 1;
/// The record field that gives a pickup its count (its smarts).
pub const SMARTS_FIELD: usize = 17;
/// What may hit it: the level's shots and explosions.
pub const HIT_MASK: u32 = types::LEVEL_SHOT;

/// Its logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Waiting.
    pub const IDLE: i32 = 1;
}

/// The helper's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DynamiteData {
    /// Claw's count of sticks at the last refill.
    pub remembered: i32,
    /// The gate between two blasts it answers.
    pub gate: Timer,
    /// The sticks it put down that are not taken yet.
    pub sticks: Vec<ObjectId>,
}

fn helper_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut DynamiteData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::AquatisDynamite(d) => Some(d),
        _ => None,
    }
}

fn helper<'a>(w: &'a World, id: ObjectId) -> Option<&'a DynamiteData> {
    match &w.objects.get(id)?.data {
        ObjectData::AquatisDynamite(d) => Some(d),
        _ => None,
    }
}

/// One logic call (see the module documentation).
pub fn tick(w: &mut World, id: ObjectId) {
    super::reset_on_respawn(w);
    match w.objects.get(id).map(|o| o.state) {
        Some(state::SET_UP) => set_up(w, id),
        Some(state::IDLE) => watch_sticks(w, id),
        _ => {}
    }
}

fn set_up(w: &mut World, id: ObjectId) {
    let claw = w.claw.stats.dynamite;
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let r = |i: usize| o.record.as_ref().map_or(0, |r| r[i]);
    let rect = Rect::new(
        r(field::X_MIN),
        r(field::Y_MIN),
        r(field::X_MAX),
        r(field::Y_MAX),
    );
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    o.draw_flags |= draw_flags::HIDDEN;
    o.type_flags = 1;
    o.hit_type_flags = HIT_MASK;
    o.attack_type_flags = 0;
    o.attack_rect = Rect::UNSET;
    o.hit_rect = rect;
    o.hit_handler = true;
    o.flags |= flags::ALWAYS_ACTIVE;
    o.state = state::IDLE;
    o.data = ObjectData::AquatisDynamite(DynamiteData {
        remembered: claw,
        ..DynamiteData::default()
    });
    refill(w, id);
}

/// The refill (see the module documentation).
pub fn refill(w: &mut World, id: ObjectId) {
    let claw = w.claw.stats.dynamite;
    let Some(remembered) = helper(w, id).map(|d| d.remembered) else {
        return;
    };
    if claw > remembered {
        return;
    }
    let Some((x, y)) = w.objects.get(id).map(|o| (o.x, o.y)) else {
        return;
    };
    w.aquatis_link_mut().refill_outstanding = true;
    let first = w.image_frames(STICK_IMAGE).map(|f| f.0);
    let mut o = new_object(STICK_LOGIC, x, y, STICK_Z);
    o.image_set = crate::level::runtime_image_set(STICK_IMAGE);
    if let Some(first) = first {
        o.frame = first;
    }
    let mut record = Box::new([0; claw_assets::wwd::OBJECT_INTS]);
    record[SMARTS_FIELD] = STICK_COUNT;
    o.record = Some(record);
    let stick = w.spawn(o);
    if let Some(d) = helper_mut(w, id) {
        d.remembered = claw;
        d.sticks.push(stick);
    }
}

/// Clears the refill flag once a stick it put down has been taken.
fn watch_sticks(w: &mut World, id: ObjectId) {
    let Some(sticks) = helper(w, id).map(|d| d.sticks.clone()) else {
        return;
    };
    if sticks.is_empty() {
        return;
    }
    let (taken, left): (Vec<ObjectId>, Vec<ObjectId>) = sticks.into_iter().partition(|&s| {
        w.objects
            .get(s)
            .is_none_or(|o| o.flags & flags::DESTROY != 0 || &*o.logic != STICK_LOGIC)
    });
    if taken.is_empty() {
        return;
    }
    if let Some(d) = helper_mut(w, id) {
        d.sticks = left;
    }
    if w.aquatis().is_some() && w.current_boss() == w.aquatis() {
        w.aquatis_link_mut().refill_outstanding = false;
    }
}

/// `attacker` hit `id`: a blast in the room (see the module
/// documentation).
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = attacker;
    let now = w.now_ms;
    if !helper(w, id).is_some_and(|d| d.gate.done(now)) {
        return;
    }
    if w.aquatis_alive() && !w.aquatis_link().refill_outstanding {
        refill(w, id);
    }
    if let Some(d) = helper_mut(w, id) {
        d.gate = Timer::new(now, u64::from(GATE_MS));
    }
}

/// `id` attacked `victim`: it never attacks.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}
