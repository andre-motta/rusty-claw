//! Katherine, the boss of the fourth level: a whip-carrying fighter met in
//! a small walled clearing.
//!
//! She is built like Raux: a human enemy with the boss layer on top, so
//! she waits in the bosses' `Startup` until the boss stage's introduction
//! ends ([`crate::logic::boss_stage`]), goes back to where she was placed
//! when Claw respawns, and releases the gem that ends the level when she
//! dies. Once the fight is on she walks at Claw and, within 200 px,
//! cracks her whip, somersaults over him or waits; up close she stabs or
//! steps back. Unlike Raux she can be lifted and thrown.
//!
//! Not built yet ([`BUILT`] is false): Katherine stays an inert
//! placeholder (drawn, no collision, reported as not implemented), and the
//! calls below do nothing.
//!
//! # Boss stage
//!
//! Her level's door and stager ([`STAGE`]) are the shared stage pieces and
//! run already: the door leads into the stager's box and the stager plays
//! the introduction. While she is not built nobody is the current boss, so
//! the introduction shows no health bar; once built she makes herself the
//! current boss when she is set up, and the stager's health bar reads her
//! health against [`METER_MAX`]. Her defeat goes through
//! [`crate::logic::World::boss_defeated`], which launches the gem to the
//! point her record's speed fields name.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]), so she waits for the fight
//! in the bosses' `Startup` and uses the shared numbers for the states she
//! shares with other enemies (her own idle and Home included). Her
//! somersault is two states of her own, the first of the fourth level's
//! block [`STATE_BLOCK`]: [`EnemyState::Flip`], the flight over Claw, and
//! [`EnemyState::FlipLand`], the landing. The shared `Jump` keeps its own
//! handler for the other classes.
//!
//! In a reference trace of the original game on this level her states
//! carry other numbers; [`REFERENCE_STATE_IDS`] maps hers to them, so a
//! comparison can line her flip and landing up.
//!
//! # Defaults taken
//!
//! - The block of state numbers: blocks of 20 in level order, so the
//!   fourth level owns 2120 to 2139, with `Flip` 2120 and `FlipLand` 2121.
//! - Her "on screen" tests (her go-idle line, her whip lines, her landing
//!   line and her wait for Claw outside her range boxes) follow the rule
//!   Raux uses, whatever the view's size.
//! - Thrown and out of time in the air, she is removed without the boss's
//!   end (no gem), as the original does; her clearing's walls keep a throw
//!   from getting there.
//!
//! Her data ([`KatherineData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. [`super::build`] gives a new enemy plain enemy
//! data; Katherine then wraps it in her own.

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::{World, boss_stage};
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Katherine"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers reserved for the fourth level's families. Her
/// somersault takes the first two ([`EnemyState::Flip`],
/// [`EnemyState::FlipLand`]); the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2120..=2139;

/// Her states and the numbers a reference trace of the original game on
/// this level gives them.
pub const REFERENCE_STATE_IDS: [(EnemyState, i32); 4] = [
    (EnemyState::Flip, 2005),
    (EnemyState::FlipLand, 2033),
    (EnemyState::Startup, 2028),
    (EnemyState::Home, 2018),
];

/// The pieces of the boss stage on her level: the door into her clearing
/// and the stager next to where it drops Claw.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// Her health before the difficulty factor (the record's health is not
/// used), which is also the health a full health bar stands for.
pub const METER_MAX: i32 = 100;

/// The Katherine class.
pub static CLASS: Class = Class {
    name: "Katherine",
    human: true,
    construct: nothing,
    pre_tick: nothing,
    handler,
    go_home: nothing,
    go_idle: nothing,
    carry: None,
    pick_lift_voice: nothing,
    anims: ClassAnims::NONE,
    sounds: ClassSounds::NONE,
};

/// Katherine's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KatherineData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

fn handler(s: EnemyState) -> Option<Handler> {
    let _ = s;
    None
}

/// The number a reference trace of the original game gives her state `s`,
/// when it differs from the state's own number.
pub fn reference_state_id(s: EnemyState) -> Option<i32> {
    REFERENCE_STATE_IDS
        .iter()
        .find(|(state, _)| *state == s)
        .map(|&(_, id)| id)
}

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
    fn her_flip_maps_to_the_reference_numbers() {
        assert_eq!(reference_state_id(EnemyState::Flip), Some(2005));
        assert_eq!(reference_state_id(EnemyState::FlipLand), Some(2033));
        assert_eq!(reference_state_id(EnemyState::Startup), Some(2028));
        assert_eq!(reference_state_id(EnemyState::Home), Some(2018));
        assert_eq!(reference_state_id(EnemyState::Strike), None);
    }

    #[test]
    fn her_states_are_in_her_block() {
        for s in [EnemyState::Flip, EnemyState::FlipLand] {
            assert!(STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
        }
    }
}
