//! Springboards: platforms that throw whoever stands on them straight up.
//! On the fourth level each one is a springy fern; later levels use the
//! same logic with a rock or a box.
//!
//! Claw lands on a springboard like on any platform. In its idle state
//! the springboard loops its idle animation and checks every call whether
//! Claw stands on it; when he does, it starts its spring animation, and
//! on the first call that reaches the animation's launch frame it throws
//! him into a jump whose apex is a fixed height above where he stands
//! ([`DEFAULT_HEIGHT`] unless the record's y_max field gives another),
//! whatever the jump key does. A Claw who stepped off before that call
//! is not thrown. At the end of the spring animation it goes back to
//! idle.
//!
//! Enemies use a second cycle: a human enemy standing on an idle
//! springboard arms it ([`state::ENEMY_ARMED`]); on that cycle's launch
//! frame the springboard raises [`special::LAUNCH`], which the enemy reads
//! in its next call and answers by jumping off. A cycle Claw set off never
//! launches an enemy, and one an enemy set off never launches Claw.
//!
//! The springboard never moves, never takes part in pair collision and is
//! solid in every state. Its landing surface and its two animations come
//! from the level ([`level_table`]).
//!
//! Not built yet ([`BUILT`] is false): springboards stay inert
//! placeholders (drawn, no collision, reported as not implemented), and
//! the calls below do nothing.
//!
//! # Defaults taken
//!
//! - The set-up sets the special flag [`special::MARKER`] besides
//!   [`special::ENEMY_ARMABLE`]. Nothing is known to read it, so it is set
//!   and given no behaviour.

use super::World;
use super::platform::SURFACE_DEPTH;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["SpringBoard"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// Height of the throw's apex above Claw, px, for a record whose y_max
/// field is zero or negative.
pub const DEFAULT_HEIGHT: i32 = 450;
/// Depth given to a springboard placed at depth 0.
pub const DEFAULT_Z: i32 = 2000;
/// Time the logic waits after starting a spring cycle, ms.
pub const SPRING_DELAY_MS: u32 = 250;

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Idle: looping the idle animation, waiting for Claw.
    pub const IDLE: i32 = 5;
    /// The spring cycle Claw started.
    pub const CLAW_SPRING: i32 = 6;
    /// The spring cycle an enemy started.
    pub const ENEMY_SPRING: i32 = 1008;
    /// Armed by an enemy standing on it: the next call starts
    /// [`ENEMY_SPRING`].
    pub const ENEMY_ARMED: i32 = 1111;
}

/// Bits of the springboard's special flags.
pub mod special {
    /// A human enemy standing on it may arm it.
    pub const ENEMY_ARMABLE: u32 = 0x10000;
    /// Set during an enemy cycle from its launch frame to its end: the
    /// enemy on it jumps off.
    pub const LAUNCH: u32 = 0x20000;
    /// Set at set-up with no behaviour (see the module documentation).
    pub const MARKER: u32 = 0x40;
}

/// A level's springboard data: the landing rectangle (before
/// [`SURFACE_DEPTH`] is added to its bottom) and the idle and spring
/// animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelSprings {
    /// Landing rectangle relative to the springboard's position.
    pub rect: (i32, i32, i32, i32),
    /// Animation looped while idle.
    pub idle_anim: &'static str,
    /// Animation of a spring cycle.
    pub spring_anim: &'static str,
}

const FERN: LevelSprings = LevelSprings {
    rect: (-32, 10, 24, 32),
    idle_anim: "LEVEL_SPRINGYFERN_IDLE",
    spring_anim: "LEVEL_SPRINGYFERN_SPRING",
};

const ROCK: LevelSprings = LevelSprings {
    rect: (-24, 89, 23, 130),
    idle_anim: "LEVEL_ROCKSPRING_IDLE",
    spring_anim: "LEVEL_ROCKSPRING_SPRING",
};

const BOX: LevelSprings = LevelSprings {
    rect: (-24, 0, 22, 40),
    idle_anim: "LEVEL_SPRINGBOX_IDLE",
    spring_anim: "LEVEL_SPRINGBOX_SPRING",
};

/// The springboard data of levels 1 to 14; the first three levels have
/// none.
const LEVELS: [Option<LevelSprings>; 14] = [
    None,
    None,
    None,
    Some(FERN),
    Some(FERN),
    Some(FERN),
    Some(ROCK),
    Some(FERN),
    Some(BOX),
    Some(FERN),
    Some(FERN),
    Some(ROCK),
    Some(FERN),
    Some(FERN),
];

/// The springboard data of level `level` (1 to 14); `None` for a level
/// without any, and for other level numbers.
pub fn level_table(level: u32) -> Option<LevelSprings> {
    LEVELS
        .get(level.wrapping_sub(1) as usize)
        .copied()
        .flatten()
}

/// The landing surface of a springboard in level `level`, relative to its
/// position: the level's rectangle with [`SURFACE_DEPTH`] added to its
/// bottom; an empty rectangle for a level without springboards.
pub fn surface(level: u32) -> Rect {
    let (l, t, r, b) = level_table(level).map_or((0, 0, 0, 0), |s| s.rect);
    Rect::new(l, t, r, b + SURFACE_DEPTH)
}

/// A springboard's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpringBoardData {}

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
    fn the_fourth_level_lands_claw_on_a_fern() {
        let s = level_table(4).expect("the fourth level has springboards");
        assert_eq!(s, FERN);
        assert_eq!(surface(4), Rect::new(-32, 10, 24, 64));
    }

    #[test]
    fn levels_without_springboards_have_no_data() {
        for n in [0, 1, 2, 3, 15, u32::MAX] {
            assert_eq!(level_table(n), None, "level {n}");
            assert_eq!(surface(n), Rect::new(0, 0, 0, 32), "level {n}");
        }
        assert_eq!(level_table(7), Some(ROCK));
        assert_eq!(level_table(12), Some(ROCK));
        assert_eq!(level_table(9), Some(BOX));
    }
}
