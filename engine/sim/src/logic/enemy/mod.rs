//! Enemies: the officer, the soldier and the rat, on shared enemy
//! behaviour (to be filled in; until then enemies stand still and cannot
//! be hurt).

pub mod officer;
pub mod rat;
pub mod soldier;

/// Image sets every enemy may switch to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations every enemy may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Data every enemy keeps.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnemyData {}
