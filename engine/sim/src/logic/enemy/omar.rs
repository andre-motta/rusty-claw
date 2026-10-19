//! Omar, the temple's guardian who ends the fourteenth level, with the
//! ring of fire or ice that shields him ([`shield`]) and his energy balls
//! ([`bullet`]).
//!
//! He is a human enemy on the shared boss staging
//! ([`crate::logic::boss_stage`]): the warp drops Claw onto the ladder top
//! of his chamber, the staged introduction freezes Claw, and the fight
//! starts when the flag reaches 2. In the first phase nine shield pieces
//! circle him ([`RING_PIECES`] of them, [`RING_RADIUS`] px out) on the
//! level clock ([`ring_angle`]), all of fire or all of ice; only a sword
//! shot of the other element hurts him ([`ELEMENT_DAMAGE`]), and he then
//! moves to the chamber's other ledge. When his health runs out the ring
//! is gone and the second phase starts in the hall above: there only
//! Claw's blows hurt him ([`BLOW_DAMAGE`]), and he casts energy balls.
//! His gem comes [`GEM_TICKS`] ticks after the killing blow.
//!
//! # State numbers
//!
//! The fourteenth level's block [`super::tiger_guard::STATE_BLOCK`] (2320
//! to 2339) holds his new names from 2330 ([`STATES`]: the cast, the move
//! and the wait after his death). He reuses the numbers of names that
//! already have one: `Startup`, `Idle`, `Home`, `HitHigh`, `HitLow`,
//! `BlockLow` and `Dead`.
//!
//! Not built yet ([`BUILT`] is false): his record stays an inert
//! placeholder and the calls below do nothing.

pub mod bullet;
pub mod shield;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::{World, boss_stage};
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Omar"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_OMAR";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_OMAR_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// His new state names, in number order from 2330 (see
/// [`super::tiger_guard::STATE_BLOCK`]).
pub const STATES: [EnemyState; 3] = [
    EnemyState::MagicAttack,
    EnemyState::MoveToPosition,
    EnemyState::Wait,
];

/// The staging objects of his fight.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// His health in each phase, before the difficulty factor.
pub const HEALTH: i32 = 100;
/// What touching him costs Claw, before the difficulty factor.
pub const DAMAGE: i32 = 30;
/// What a sword shot of the other element costs him in the first phase.
pub const ELEMENT_DAMAGE: i32 = 25;
/// What a blow of Claw's costs him in the second phase.
pub const BLOW_DAMAGE: i32 = 25;
/// What one of his energy balls costs Claw.
pub const BALL_DAMAGE: i32 = 15;
/// His hit and movement box.
pub const BODY_RECT: Rect = Rect::new(-11, -47, 23, 61);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (56, 40), (56, 40)];
/// Where his reset puts him: his place in the chamber.
pub const HOME: (i32, i32) = (42250, 2475);
/// Where the first phase's end sends him: the hall above the chamber.
pub const HALL: (i32, i32) = (42320, 1067);
/// Where his gem flies to.
pub const GEM_TARGET: (i32, i32) = (41825, 930);
/// Ticks from the killing blow to the gem at the 18 ms step (his death
/// fall's length).
pub const GEM_TICKS: u64 = 176;
/// Ticks his move takes at the 18 ms step: he flashes in place, then
/// appears at the new place.
pub const MOVE_TICKS: u64 = 112;
/// The pieces of his ring.
pub const RING_PIECES: usize = 9;
/// How far out the ring's pieces circle him, px.
pub const RING_RADIUS: f64 = 80.0;
/// The degrees between two pieces of the ring.
pub const RING_SPACING: f64 = 33.0;
/// How fast the ring turns, degrees per second (clockwise on screen).
pub const RING_SPEED: f64 = -250.0;

/// His decision and idle pose.
pub const IDLE_ANIM: &str = "LEVEL_OMAR_IDLE";
/// His death fall, at whose end the gem comes.
pub const DEATH_ANIM: &str = "LEVEL_OMAR_KILLFALL3";

/// The angle of piece `i` of his ring, degrees, at `ms` of the level
/// clock: the pieces sit [`RING_SPACING`] degrees apart and turn at
/// [`RING_SPEED`] from the level's start, whatever happens to Omar, so
/// the gap in the ring is where the clock puts it.
pub fn ring_angle(i: usize, ms: u64) -> f64 {
    RING_SPACING * i as f64 + RING_SPEED * ms as f64 / 1000.0
}

/// Where piece `i` of his ring stands at `ms` of the level clock around
/// Omar at `at`: [`RING_RADIUS`] px out along [`ring_angle`], each axis
/// truncated.
pub fn ring_point(i: usize, ms: u64, at: (i32, i32)) -> (i32, i32) {
    let a = ring_angle(i, ms).to_radians();
    (
        (RING_RADIUS * a.cos() + f64::from(at.0)).trunc() as i32,
        (RING_RADIUS * a.sin() + f64::from(at.1)).trunc() as i32,
    )
}

/// The Omar class (not built yet: every step does nothing).
pub static CLASS: Class = Class {
    name: "Omar",
    human: true,
    construct: nothing,
    pre_tick: nothing,
    handler,
    go_home: nothing,
    go_idle: nothing,
    carry: None,
    pick_lift_voice: nothing,
    lift_roll: false,
    anims: ClassAnims::NONE,
    sounds: ClassSounds::NONE,
};

/// Omar's data.
#[derive(Debug, Clone, PartialEq)]
pub struct OmarData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// The phase: 1 while his ring is up, 0 after.
    pub phase: u8,
    /// His ring's element: false for fire, true for ice.
    pub ice: bool,
    /// The pieces of his ring, while they are there.
    pub pieces: Vec<ObjectId>,
    /// His death has given its gem (or found Claw dead): it gives no
    /// other.
    pub gem_given: bool,
}

impl Default for OmarData {
    /// Data in his first phase, his ring of fire not made yet.
    fn default() -> OmarData {
        OmarData {
            enemy: EnemyData::default(),
            phase: 1,
            ice: false,
            pieces: Vec::new(),
            gem_given: false,
        }
    }
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
    use super::*;

    #[test]
    fn his_states_take_the_second_half_of_the_fourteenth_levels_block() {
        let block = super::super::tiger_guard::STATE_BLOCK;
        for (i, s) in STATES.iter().enumerate() {
            assert_eq!(*s as i32, block.start() + 10 + i as i32, "{}", s.name());
        }
        assert!(CLASS.human, "a human enemy");
        assert!(handler(EnemyState::Home).is_none(), "not built yet");
    }

    #[test]
    fn his_ring_turns_on_the_level_clock() {
        // Nine pieces 33 degrees apart, turning a quarter of a degree a
        // millisecond backwards: 4.5 degrees a tick at the 18 ms step.
        assert_eq!(ring_angle(0, 0), 0.0);
        assert_eq!(ring_angle(3, 0), 99.0);
        assert_eq!(ring_angle(0, 18), -4.5);
        assert_eq!(ring_angle(0, 1458), -364.5);
        assert_eq!(ring_angle(8, 1000) - ring_angle(0, 1000), 264.0);
        // Piece 0 at -4.5 degrees around his place: 80 px out, a little
        // above his row.
        assert_eq!(ring_point(0, 18, HOME), (42329, 2468));
        assert_eq!(ring_point(0, 0, HOME), (42330, 2475));
    }

    #[test]
    fn his_numbers() {
        assert_eq!(HEALTH / ELEMENT_DAMAGE, 4);
        assert_eq!(HEALTH / BLOW_DAMAGE, 4);
        let d = OmarData::default();
        assert_eq!((d.phase, d.ice, d.gem_given), (1, false, false));
        assert!(d.pieces.is_empty());
        for anim in [IDLE_ANIM, DEATH_ANIM] {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
    }
}
