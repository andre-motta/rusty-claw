//! The crack in the rock wall behind Aquatis ([`super`]): a blast from
//! one of Claw's sticks of dynamite that reaches its box starts an
//! earthquake.
//!
//! The crack glows ([`GLOW_ANIM`]) and waits. A dynamite blast that
//! touches its box ([`HIT_RECT`]) while Aquatis is alive and not in his
//! frenzy, no stalactite is falling or armed, and [`super::CRACK_COOLDOWN_MS`]
//! have passed since its last earthquake, starts one: a rumble drawn from
//! the shared random stream, a camera shake of [`super::QUAKE_MS`] and the
//! earthquake flag of the Aquatis link. When the quake ends it arms the
//! next stalactite ([`super::stalactite`]). Its reset bit
//! ([`super::CRACK_BIT`]) stays set while it sleeps, so after the staged
//! introduction Aquatis waits until Claw comes close enough to wake it.
//!
//! Not built yet ([`BUILT`] is false): it stays an inert placeholder and
//! the calls below do nothing.

use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["AquatisCrack"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_AQUATISWALLCRACK";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// The glow.
pub const GLOW_ANIM: &str = "LEVEL_CRACKGLOW";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[GLOW_ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Its box: its picture's widened 25 px to the right of its left edge and
/// 100 px past its right edge.
pub const HIT_RECT: Rect = Rect::new(-4, -64, 130, 63);

/// The crack's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrackData {}

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
