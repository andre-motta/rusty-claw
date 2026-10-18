//! Marrow's parrot, on the tenth level: the bird that circles over the
//! spike pit of Marrow's room ([`super::marrow`]), swoops at Claw and
//! carries Marrow across the pit when he whistles.
//!
//! It is a plain flying enemy, with the seagull's numbers (its beak box,
//! its hit box, depth [`Z`], one health, damage [`DAMAGE`]) but its own
//! state machine. It patrols a line near the top of the room between the
//! ends of its record's box ([`EnemyState::FlyAbout`], [`PATROL_TICKS`] a
//! leg); at a line end, with Claw on the other half of the room and the
//! fight on, it sets up a dive ([`EnemyState::StrikeSetup`]) and swoops at
//! him beak first ([`EnemyState::Strike`]). Claw's hits never hurt it: each
//! fresh attack of his that reaches it counts once, flinches it
//! ([`EnemyState::GenericHit`]) and sends it back to its patrol; the
//! [`HITS_HOME`]th sends it to its perch ([`EnemyState::FlyToIdle`]), which
//! clears Marrow's open request so the slabs close into a bridge. Perched,
//! it answers Marrow's call: it flies to him ([`EnemyState::FlyToMarrow`])
//! and carries him to the ledge away from Claw
//! ([`EnemyState::CarryMarrow`], [`CARRY_TICKS`] ticks), setting the open
//! request so the slabs open under the bridge.
//!
//! Not built yet ([`BUILT`] is false): the parrot stays an inert
//! placeholder (drawn, no collision, reported as not implemented) and the
//! calls below do nothing.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): it perches in the shared
//! `Idle`, swoops in the shared `Strike` and flinches in the seagull's
//! `GenericHit`; its five flights are new names, the five numbers after
//! Marrow's whistle in the tenth level's block
//! ([`super::marrow::STATE_BLOCK`]), in the order the game registers them
//! ([`STATES`]).
//!
//! # Defaults taken
//!
//! - The five flights take 2241 to 2245, after Marrow's whistle at 2240.
//! - A carry whose Marrow is gone (his object removed by other means than
//!   a death, which cannot happen while he is carried and Safe) ends at
//!   once.
//! - The beak hurts Claw by the shared enemy-contact rule, limited by
//!   Claw's own protection after a hit.
//! - The slabs' own behaviour is [`crate::logic::marrow_floor`]'s; the
//!   parrot only writes the open request, the parrot call and the phase
//!   health of the Marrow link ([`super::marrow::MarrowLink`]).
//!
//! Its data ([`ParrotData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Parrot"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_PARROT";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_PARROT_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Its five new states, in the order the game registers them.
pub const STATES: [EnemyState; 5] = [
    EnemyState::FlyAbout,
    EnemyState::StrikeSetup,
    EnemyState::FlyToIdle,
    EnemyState::FlyToMarrow,
    EnemyState::CarryMarrow,
];

/// Health before the difficulty factor.
pub const HEALTH: i32 = 1;
/// Damage of its beak, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Its depth when its record's is 0: after Claw, the slabs and Marrow.
pub const Z: i32 = 8500;
/// Its beak's box, facing left.
pub const BEAK: Rect = Rect::new(-26, -5, -5, 22);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-30, -30, 30, 30);

/// Its patrol speed, px/s.
pub const FLY_SPEED: i32 = 300;
/// The length of its swoop and dive flights (across plus down), px/s.
pub const STRIKE_SPEED: i32 = 800;
/// The length of its flight to Marrow (across plus down), px/s.
pub const TO_MARROW_SPEED: i32 = 300;
/// The x of its two dive points, left and right of the room's centre.
pub const DIVE_X: (i32, i32) = (40688, 40928);
/// The dive point lies up to this many px above Claw (a draw of 0..31).
pub const DIVE_SPREAD: u32 = 31;
/// Chance in 100, at a line end, that it dives when it may.
pub const DIVE_CHANCE: i32 = 75;
/// Hits that send it to its perch.
pub const HITS_HOME: i32 = 3;
/// Where it carries Marrow: the right ledge when Claw is left of the
/// room's centre, the left ledge otherwise.
pub const CARRY_X: (i32, i32) = (41063, 40553);
/// How far above Marrow it takes hold of him, px.
pub const GRAB_ABOVE: i32 = 97;
/// How far above Marrow's height it carries him to, px.
pub const CARRY_ABOVE: i32 = 110;
/// The gravity of its flight to the perch, px/s^2.
pub const HOME_GRAVITY: f64 = 1000.0;
/// The gravity of the carry, px/s^2.
pub const CARRY_GRAVITY: f64 = 200.0;

/// Ticks of one patrol leg at the 18 ms step (measured).
pub const PATROL_TICKS: u32 = 119;
/// Ticks of a dive's set-up at the 18 ms step (measured).
pub const STRIKE_SETUP_TICKS: u32 = 28;
/// Ticks of a swoop at the 18 ms step (measured).
pub const STRIKE_TICKS: u32 = 31;
/// Ticks of its flinch at the 18 ms step (measured).
pub const HIT_TICKS: u32 = 19;
/// Ticks of its flight home to the perch at the 18 ms step (measured).
pub const HOME_TICKS: u32 = 56;
/// Ticks of the carry at the 18 ms step (measured).
pub const CARRY_TICKS: u32 = 119;

/// Perched.
pub const IDLE_ANIM: &str = "LEVEL_PARROT_IDLE";
/// Every flight.
pub const FLY_ANIM: &str = "LEVEL_PARROT_FLY";
/// The flinch.
pub const HIT_ANIM: &str = "LEVEL_PARROT_HIT";
/// The swoop.
pub const STRIKE_ANIM: &str = "LEVEL_PARROT_STRIKE1";
/// The flinch's sound.
pub const HIT_SOUND: &str = "LEVEL_PARROT_PARROTHIT";
/// The swoop's sound.
pub const SWOOP_SOUND: &str = "LEVEL_PARROT_PARROTSWOOP";
/// Its squawks, one of two.
pub const SQUAWKS: [&str; 2] = ["LEVEL_PARROT_POLLY2", "LEVEL_PARROT_POLLY3"];

/// The parrot class.
pub static CLASS: Class = Class {
    name: "Parrot",
    human: false,
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

/// The parrot's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParrotData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// Where it perches: its record's position.
    pub perch: (i32, i32),
    /// Claw's attacks that reached it since it last left its perch.
    pub hits: i32,
}

fn handler(s: EnemyState) -> Option<Handler> {
    let _ = s;
    None
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
    use super::super::marrow;
    use super::*;

    #[test]
    fn its_flights_follow_the_whistle_in_the_tenth_levels_block() {
        let block = marrow::STATE_BLOCK;
        for (i, s) in STATES.into_iter().enumerate() {
            assert_eq!(s as i32, block.start() + 1 + i as i32, "{}", s.name());
            assert!(block.contains(&(s as i32)));
        }
        // It perches, swoops and flinches under shared numbers.
        assert_eq!(EnemyState::Idle as i32, 2000);
        assert_eq!(EnemyState::Strike as i32, 2002);
        assert_eq!(EnemyState::GenericHit as i32, 2143);
        assert!(!CLASS.human, "a plain flying enemy");
        assert!(CLASS.carry.is_none());
    }

    #[test]
    fn it_carries_marrow_away_from_claw() {
        // The dive points and the carry's ends lie either side of the
        // room's centre.
        assert!(DIVE_X.0 < marrow::ROOM_CENTRE_X && marrow::ROOM_CENTRE_X < DIVE_X.1);
        assert!(CARRY_X.1 < marrow::ROOM_CENTRE_X && marrow::ROOM_CENTRE_X < CARRY_X.0);
        assert_eq!(HITS_HOME, 3);
        // Its depth sorts it after Marrow.
        const { assert!(Z > marrow::SORTED_Z) };
    }

    #[test]
    fn its_art_is_its_own() {
        for key in [
            IDLE_ANIM,
            FLY_ANIM,
            HIT_ANIM,
            STRIKE_ANIM,
            HIT_SOUND,
            SWOOP_SOUND,
        ]
        .iter()
        .chain(&SQUAWKS)
        {
            assert!(key.starts_with(ANIM_PREFIXES[0]), "{key}");
        }
    }
}
