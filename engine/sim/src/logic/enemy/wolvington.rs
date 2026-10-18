//! Wolvington, the boss of the sixth level: a wolf magistrate with a sword
//! and a cape who waits at the end of the port town, with his magic shot
//! ([`shot`]) and the light it carries ([`light`]).
//!
//! He is built like Raux and Katherine: a human enemy with the boss layer
//! on top, so he waits in the bosses' `Startup` until the boss stage's
//! introduction ends ([`crate::logic::boss_stage`]), goes back to where he
//! was placed with his health restored when Claw respawns, and releases
//! the gem that ends the level when he dies. Once the fight is on, at
//! range he walks in, sprints in for a running uppercut or stands and
//! throws a volley of magic shots straight across the room; up close he
//! answers a standing Claw with an uppercut and a crouching one with a
//! low sword thrust, or steps back. He hops back from most sword swings
//! aimed at him from close by, blocks shots, magic and thrown objects,
//! flinches from Claw's blows and then counters with a running uppercut.
//! Claw can lift and throw him, and every throw he survives ends in a
//! volley. He has [`HEALTH`] health; each of Claw's blows costs him 5 and
//! each of his hits costs Claw [`DAMAGE`].
//!
//! Not built yet ([`BUILT`] is false): Wolvington, his shot and its light
//! stay inert placeholders (drawn, no collision, reported as not
//! implemented), and the calls below do nothing.
//!
//! # Boss stage
//!
//! His level's door and stager ([`STAGE`]) are the shared stage pieces and
//! run already: the door leads into the stager's box and the stager plays
//! the introduction. While he is not built nobody is the current boss, so
//! the introduction shows no health bar; once built he makes himself the
//! current boss when he is set up, and the stager's health bar reads his
//! health against [`METER_MAX`] (his record's health is not used). His
//! defeat goes through [`crate::logic::World::boss_defeated`], which
//! launches the gem to the point his record's speed fields name. A Claw
//! who jumps into the room on its first tick skips the introduction; with
//! the usual jump Wolvington then stays in `Startup` until Claw acts on
//! him (a blow, a lift, a touch, or walking back into the stager's box),
//! because Claw comes back within 40 px of his height two ticks before
//! the half second that would wake him.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he waits for the fight in
//! the bosses' `Startup`, idles in the shared human `Idle`, decides in
//! `Home` and walks in `Advance` with handlers of his own under those
//! numbers, and inherits the other shared human states with their
//! numbers. His five attacks are new names, the first five numbers of the
//! sixth level's block [`STATE_BLOCK`]: [`EnemyState::MagicVolley`] (the
//! choice of the next shot), [`EnemyState::MagicShot`] (one cast),
//! [`EnemyState::Uppercut`], [`EnemyState::SwordThrust`] and
//! [`EnemyState::Sprint`] (the run into the uppercut).
//!
//! In a reference trace of the original game on this level his states
//! carry other numbers; [`REFERENCE_STATE_IDS`] maps his to them, so a
//! comparison can line them up.
//!
//! # Defaults taken
//!
//! - The block of state numbers: blocks of 20 in level order, so the
//!   sixth level owns 2160 to 2179, his five attacks 2160 to 2164 in the
//!   order the game registers them; the goo vents and the ground blowers
//!   take none.
//! - The shot leaves 65 px ahead of him, 20 px above his position (the
//!   high shot) or 10 px below it (the low one), and flies at 300 px/s the
//!   way he faces when the cast starts ([`shot::spawn_offset`],
//!   [`shot::speed`]).
//! - The shot's light is drawn as the magic claw's light flare is, with
//!   its own picture ([`light::IMAGE_SET`]), through the same drawing
//!   path; no other drawing is added for it.
//!
//! His data ([`WolvingtonData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. [`super::build`] gives a new enemy plain enemy
//! data; Wolvington then wraps it in his own.

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::{World, boss_stage};
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Wolvington"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_WOLVINGTON";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_WOLVINGTON_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers reserved for the sixth level's families. His five
/// attacks take the first five ([`EnemyState::MagicVolley`] to
/// [`EnemyState::Sprint`]); the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2160..=2179;

/// His states and the numbers a reference trace of the original game on
/// this level gives them.
pub const REFERENCE_STATE_IDS: [(EnemyState, i32); 8] = [
    (EnemyState::Startup, 2028),
    (EnemyState::Home, 2018),
    (EnemyState::Advance, 2021),
    (EnemyState::MagicVolley, 2047),
    (EnemyState::MagicShot, 2048),
    (EnemyState::Uppercut, 2049),
    (EnemyState::SwordThrust, 2050),
    (EnemyState::Sprint, 2051),
];

/// The pieces of the boss stage on his level: the door into his room and
/// the stager next to where it drops Claw.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// His health before the difficulty factor (the record's health is not
/// used), which is also the health a full health bar stands for.
pub const HEALTH: i32 = 100;

/// See [`HEALTH`].
pub const METER_MAX: i32 = HEALTH;

/// Damage of his strikes and of touching him, before the difficulty
/// factor; his shot deals the same, unscaled.
pub const DAMAGE: i32 = 5;

/// The Wolvington class.
pub static CLASS: Class = Class {
    name: "Wolvington",
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

/// Wolvington's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WolvingtonData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

fn handler(s: EnemyState) -> Option<Handler> {
    let _ = s;
    None
}

/// The number a reference trace of the original game gives his state
/// `s`, when it differs from the state's own number.
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

/// His magic shot: a ball of light cast from his hand that flies straight
/// across the room without falling, hurts Claw (5, whatever the
/// difficulty) and is gone, breaks the crates it passes and flies on, and
/// is ignored by enemies and kegs. A wall stops it on the tick after it
/// touches one. It sheds two sparkles a tick and carries a light
/// ([`super::light`]).
///
/// Not built yet ([`BUILT`] is false).
pub mod shot {
    use crate::logic::World;
    use crate::object::ObjectId;

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["WolvingtonBullet"];

    /// The shot's picture.
    pub const IMAGE_SET: &str = "LEVEL_WOLVINGTONMAGIC";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

    /// The shot's animation.
    pub const ANIM: &str = "GAME_CYCLE50";

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &[ANIM];

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = false;

    /// Damage to Claw.
    pub const DAMAGE: i32 = 5;
    /// How far ahead of Wolvington a shot leaves, px.
    pub const AHEAD: i32 = 65;
    /// Height of a high shot's start from his position, px.
    pub const HIGH_Y: i32 = -20;
    /// Height of a low shot's start from his position, px.
    pub const LOW_Y: i32 = 10;
    /// Speed of a shot, px/s.
    pub const SPEED: i32 = 300;

    /// Where a shot leaves, from Wolvington's position, when he faces
    /// right (`facing_right`) or left, high or low.
    pub fn spawn_offset(facing_right: bool, high: bool) -> (i32, i32) {
        let x = if facing_right { AHEAD } else { -AHEAD };
        (x, if high { HIGH_Y } else { LOW_Y })
    }

    /// The shot's horizontal speed, px/s, when he faces right or left.
    pub fn speed(facing_right: bool) -> i32 {
        if facing_right { SPEED } else { -SPEED }
    }

    /// A shot's data.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct ShotData {}

    /// One logic call.
    pub fn tick(w: &mut World, id: ObjectId) {
        let _ = (w, id);
    }

    /// `attacker` hit `id`. Nothing hits a shot.
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = (w, id, attacker);
    }

    /// `id` attacked `victim`. The victim's own hit handler decides.
    pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
        let _ = (w, id, victim);
    }
}

/// The light a magic shot carries: one picture drawn see-through just
/// behind the shot, following it and removed with it. It never collides.
///
/// Not built yet ([`BUILT`] is false).
pub mod light {
    use crate::logic::World;
    use crate::object::ObjectId;

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["WolvingtonLFX"];

    /// The light's picture.
    pub const IMAGE_SET: &str = "GAME_LIGHTFX_WOLVINGTONLFX";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &[IMAGE_SET];

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = false;

    /// A light's data.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct LightData {}

    /// One logic call.
    pub fn tick(w: &mut World, id: ObjectId) {
        let _ = (w, id);
    }

    /// `attacker` hit `id`. Nothing hits a light.
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = (w, id, attacker);
    }

    /// `id` attacked `victim`. A light attacks nothing.
    pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
        let _ = (w, id, victim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn his_states_map_to_the_reference_numbers() {
        assert_eq!(reference_state_id(EnemyState::Startup), Some(2028));
        assert_eq!(reference_state_id(EnemyState::Home), Some(2018));
        assert_eq!(reference_state_id(EnemyState::MagicVolley), Some(2047));
        assert_eq!(reference_state_id(EnemyState::Sprint), Some(2051));
        assert_eq!(reference_state_id(EnemyState::Idle), None);
    }

    #[test]
    fn his_attacks_open_the_sixth_levels_block_in_order() {
        let attacks = [
            EnemyState::MagicVolley,
            EnemyState::MagicShot,
            EnemyState::Uppercut,
            EnemyState::SwordThrust,
            EnemyState::Sprint,
        ];
        for (i, s) in attacks.into_iter().enumerate() {
            assert!(STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
            assert_eq!(s as i32, STATE_BLOCK.start() + i as i32, "{}", s.name());
        }
    }

    #[test]
    fn a_shot_leaves_ahead_of_him_high_or_low() {
        assert_eq!(shot::spawn_offset(false, true), (-65, -20));
        assert_eq!(shot::spawn_offset(false, false), (-65, 10));
        assert_eq!(shot::spawn_offset(true, true), (65, -20));
        assert_eq!(shot::spawn_offset(true, false), (65, 10));
        assert_eq!(shot::speed(true), 300);
        assert_eq!(shot::speed(false), -300);
        // 5.4 px per 18 ms tick.
        assert_eq!(shot::SPEED * 18, 5400);
    }
}
