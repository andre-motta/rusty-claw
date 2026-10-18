//! The crabs of the seventh level: the hermit crab, the sand nests that
//! hatch them ([`nest`]) and the bomb a bomber crab throws ([`bomb`]).
//!
//! The hermit crab is a plain enemy (not a human one) that guards a short
//! stretch of beach. Far from Claw it mostly hides in its shell
//! ([`EnemyState::Hide`]) or crouches in it ([`EnemyState::Defense`]);
//! closer in it charges at him ([`EnemyState::MovingAttack`], 300 to 500
//! px/s), snips at him when it is within 50 px ([`EnemyState::Snip`],
//! live on its 7th to 14th ticks, [`DAMAGE`] each), or backs off. A
//! bomber crab (the [`BOMBER_IMAGE_SET`] picture) throws a bomb instead
//! ([`EnemyState::Launch`]) at most every [`LAUNCH_EVERY_MS`].
//!
//! Its shell protects it: a hit while it shows a shell picture
//! ([`SHELL_FRAMES`]) does no harm but knocks it aside
//! ([`EnemyState::Nudge`], 5.4 px a tick for as long as the blow keeps
//! touching it, about 37 px under a standing sword swing), with a boink
//! at most every 200 ms. A hit on any other picture kills it at once,
//! whatever its health. Hiding, it can be lifted and thrown.
//!
//! Not built yet ([`BUILT`] is false): hermit crabs, nests and bombs stay
//! inert placeholders (drawn, no collision, reported as not implemented),
//! and the calls below do nothing.
//!
//! # State numbers
//!
//! The crab's nine states are new names and take the upper half of the
//! seventh level's block ([`STATE_BLOCK`], from
//! [`super::bear_sailor::CRAB_STATES_START`]), in the order the game
//! registers them: [`EnemyState::Regroup`] (facing Claw and choosing
//! between hiding and crouching), [`EnemyState::Hide`],
//! [`EnemyState::Defense`], [`EnemyState::Attack`] (choosing between
//! charging, snipping and going back), [`EnemyState::MovingAttack`],
//! [`EnemyState::Snip`], [`EnemyState::Tumble`] (falling out of a nest),
//! [`EnemyState::Nudge`] and [`EnemyState::Launch`]. Its own states that
//! share a name with the human enemies' (deciding, striking, falling) are
//! distinct states of the original game, so they take new numbers too;
//! later families that run them reuse these numbers. It also runs the
//! shared `MoveBack`, `Lifted`, `Flailing`, `Thrown`, `Dropped` and
//! `Dead` under their numbers. The nest and the bomb are no enemies: their
//! states live in their own data.
//!
//! # Defaults taken
//!
//! - The block of state numbers: the crabs take 2190 to 2198 of the
//!   seventh level's 2180 to 2199, in registration order.
//! - A crab's second range box and the side its first go-home reads are
//!   left over from memory in the original game; each crab keeps the
//!   value it starts with, as the reference runs show it, with an empty
//!   box and an unmirrored start as the plain default.
//! - A nest hands its record's loot to its crabs in the original order,
//!   with its skipping of empty entries kept (no retail nest carries
//!   loot).
//! - Under a widened view a dead crab leaves the screen, and a nest wakes
//!   with its window, by the rules the built enemies and the triggers that
//!   wake around Claw already use.
//!
//! Its data ([`HermitCrabData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["HermitCrab"];

/// The plain crab's picture.
pub const IMAGE_SET: &str = "LEVEL_HERMITCRAB";
/// The bomber crab's picture (same frame numbers as the plain one's).
pub const BOMBER_IMAGE_SET: &str = "LEVEL_BOMBERCRAB";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, BOMBER_IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_HERMITCRAB_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers of the seventh level's families (see
/// [`super::bear_sailor::STATE_BLOCK`]).
pub const STATE_BLOCK: RangeInclusive<i32> = super::bear_sailor::STATE_BLOCK;

/// The crab's nine states, in the order they take their numbers.
pub const STATES: [EnemyState; 9] = [
    EnemyState::Regroup,
    EnemyState::Hide,
    EnemyState::Defense,
    EnemyState::Attack,
    EnemyState::MovingAttack,
    EnemyState::Snip,
    EnemyState::Tumble,
    EnemyState::Nudge,
    EnemyState::Launch,
];

/// Damage of a snip and of touching a crab, when its record sets none (no
/// difficulty factor).
pub const DAMAGE: i32 = 5;
/// The pictures of the shell: crouched, then withdrawn.
pub const SHELL_FRAMES: [i32; 3] = [800, 801, 802];
/// The picture of the throw.
pub const THROW_FRAME: i32 = 201;
/// The pictures on which a snip is live.
pub const SNIP_FRAMES: [i32; 2] = [203, 204];
/// Hit box: the box of the crab's first picture.
pub const HIT_RECT: Rect = Rect::new(-25, -19, 30, 18);
/// Movement box: the hit box lowered 10 px at the top and 5 at the bottom.
pub const MOVE_RECT: Rect = Rect::new(-25, -9, 30, 23);
/// The snip's box: the hit box widened by 15 px and heightened by 10 px
/// on each side.
pub const SNIP_BOX: Rect = Rect::new(-40, -29, 45, 28);
/// The range boxes (half width, half height), outermost first; the
/// second is left as each crab starts with it (empty by default).
pub const RANGES: [(i32, i32); 5] = [(400, 50), (0, 0), (300, 50), (100, 50), (50, 50)];
/// Time between two throws of a bomber crab, ms.
pub const LAUNCH_EVERY_MS: u32 = 4000;
/// The distance a bomb is aimed at, px, towards Claw.
pub const THROW_DISTANCE: i32 = 150;
/// Speed of a knock aside in the shell, px/s.
pub const NUDGE_SPEED: i32 = 300;
/// Speed of a back-off, px/s.
pub const MOVE_BACK_SPEED: i32 = 150;

/// The snip.
pub const SNIP_ANIM: &str = "LEVEL_HERMITCRAB_STRIKE1";
/// The crouch in the shell.
pub const DEFENSE_ANIM: &str = "LEVEL_HERMITCRAB_CROUCH4";
/// Withdrawn in the shell.
pub const HIDE_ANIM: &str = "LEVEL_HERMITCRAB_HIDE";
/// The snip's sound.
pub const SNIP_SOUND: &str = "LEVEL_HERMITCRAB_CRABSNIP";
/// The sound of a throw by a crab built as a bomber (no seventh-level
/// record is).
pub const THROW_SOUND: &str = "GAME_THRWBOMB";
/// The sound of a blow on the shell.
pub const SHELL_SOUND: &str = "GAME_BOINKJR";

/// Whether picture `frame` is a shell picture, on which a hit only knocks
/// the crab aside.
pub fn is_shell_frame(frame: i32) -> bool {
    SHELL_FRAMES.contains(&frame)
}

/// The hermit crab class.
pub static CLASS: Class = Class {
    name: "HermitCrab",
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

/// A hermit crab's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HermitCrabData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
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

/// The sand nests: a patch of sand that hatches hermit crabs the first
/// time Claw stands or walks onto it.
///
/// The trigger is the record's box, or the nest picture's box when the
/// record leaves all four values at 0; Claw's movement box must share a
/// pixel with it while he stands or walks (jumping, falling or climbing
/// never sets it off). The nest then plays its hatch ([`HATCH_ANIM`]): on
/// the hatch's fourth tick, as its second picture appears, it boinks and
/// throws out its crabs ([`crab_count`]) in a fan from left to right, each
/// a bomber at even odds; the hatch ends on its 16th tick and the nest
/// never hatches again, not even after Claw dies. Nothing can hit a nest.
///
/// Not built yet ([`BUILT`] is false).
pub mod nest {
    use crate::logic::World;
    use crate::object::ObjectId;

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["CrabNest"];

    /// The nest's picture (the records name it; the nest keeps it).
    pub const IMAGE_SET: &str = "LEVEL_SANDHOLE";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[];

    /// The hatch.
    pub const HATCH_ANIM: &str = "LEVEL_HATCHNEST";

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &[HATCH_ANIM];

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = false;

    /// The sound of the hatch.
    pub const HATCH_SOUND: &str = "GAME_BOINK";
    /// The hatch tick on which the crabs appear.
    pub const SPAWN_TICK: u32 = 4;
    /// The hatch tick on which the nest goes quiet for good.
    pub const DONE_TICK: u32 = 16;
    /// Crabs of a nest whose record asks for none.
    pub const DEFAULT_CRABS: i32 = 3;
    /// The most crabs a nest hatches.
    pub const MAX_CRABS: i32 = 9;
    /// Horizontal spacing of the fan of crabs, px/s.
    pub const FAN_STEP: i32 = 50;

    /// The crabs a nest hatches when its record asks for `user_0`: 3 for
    /// 0, at most 9.
    pub fn crab_count(user_0: i32) -> i32 {
        match user_0 {
            0 => DEFAULT_CRABS,
            n if n > MAX_CRABS => MAX_CRABS,
            n => n,
        }
    }

    /// The horizontal speed of the fan's first crab when `total` crabs and
    /// loot items leave the nest, px/s; each next one leaves
    /// [`FAN_STEP`] faster, except the last crab, whose speed is drawn.
    pub fn fan_start(total: i32) -> i32 {
        -25 * total
    }

    /// A nest's data.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct NestData {}

    /// One logic call.
    pub fn tick(w: &mut World, id: ObjectId) {
        let _ = (w, id);
    }

    /// `attacker` hit `id`. Nothing hits a nest.
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = (w, id, attacker);
    }

    /// `id` attacked `victim`. A nest attacks nothing.
    pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
        let _ = (w, id, victim);
    }
}

/// The bomb a bomber crab throws: a small round bomb lobbed in an arc
/// towards Claw ([`throw_velocity`]) that bounces twice and then rests,
/// and explodes [`FUSE_MS`] after the throw, or at once when it touches
/// Claw (costing him [`DAMAGE`]). The blast ([`BLAST`]) hurts Claw,
/// kills any crab caught outside its shell, the thrower included, hurts
/// the other enemies, sets off kegs and breaks crates, for the whole of
/// its 16 ticks. Nothing can hit a bomb, and it never sleeps.
///
/// Not built yet ([`BUILT`] is false).
pub mod bomb {
    use crate::logic::World;
    use crate::object::{ObjectId, Rect};

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["CrabBomb"];

    /// The bomb's picture.
    pub const IMAGE_SET: &str = "LEVEL_CRABBOMB";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &["LEVEL_CRABBOMB_"];

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = false;

    /// Damage of the bomb and of its blast (no difficulty factor).
    pub const DAMAGE: i32 = 5;
    /// The bomb's box in flight and the blast's.
    pub const BLAST: Rect = Rect::new(-50, -50, 50, 50);
    /// Time from the throw to the explosion, ms.
    pub const FUSE_MS: u32 = 1000;
    /// Gravity on a bomb, px/s^2.
    pub const GRAVITY: f64 = 2000.0;
    /// Landings that bounce; the next one rests.
    pub const BOUNCES: u32 = 2;
    /// Ticks of the explosion at the 18 ms step.
    pub const EXPLODE_TICKS: u32 = 16;

    /// The throw's velocity for a bomb aimed `distance` px ahead (negative
    /// to the left), px/s: a time of flight `t = sqrt(|distance| / 1000)`
    /// s gives `(distance / t, -1000 t - 2 |distance|)`.
    pub fn throw_velocity(distance: i32) -> (f64, f64) {
        let p = f64::from(distance);
        let t = (p.abs() / 1000.0).sqrt();
        if t == 0.0 {
            return (0.0, 0.0);
        }
        (p / t, -t * 1000.0 - (2.0 * p).abs())
    }

    /// A bomb's data.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct BombData {}

    /// One logic call.
    pub fn tick(w: &mut World, id: ObjectId) {
        let _ = (w, id);
    }

    /// `attacker` hit `id`. Nothing hits a bomb.
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = (w, id, attacker);
    }

    /// `id` attacked `victim`. The victim's own hit handler decides.
    pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
        let _ = (w, id, victim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_states_fill_the_upper_half_of_the_block_in_order() {
        let start = super::super::bear_sailor::CRAB_STATES_START;
        for (i, s) in STATES.into_iter().enumerate() {
            assert_eq!(s as i32, start + i as i32, "{}", s.name());
            assert!(STATE_BLOCK.contains(&(s as i32)));
        }
        assert_eq!(*STATE_BLOCK.end(), start + 9, "one number left free");
        assert!(!CLASS.human, "the crab is no human enemy");
    }

    #[test]
    fn only_the_shell_pictures_protect_it() {
        for f in SHELL_FRAMES {
            assert!(is_shell_frame(f), "{f}");
        }
        for f in [1, 2, 200, THROW_FRAME, 203, 204, 799, 803, 950] {
            assert!(!is_shell_frame(f), "{f}");
        }
        assert!(SNIP_FRAMES.iter().all(|f| !is_shell_frame(*f)));
    }

    #[test]
    fn its_boxes_come_from_its_first_picture() {
        let h = HIT_RECT;
        assert_eq!(
            MOVE_RECT,
            Rect::new(h.left, h.top + 10, h.right, h.bottom + 5)
        );
        assert_eq!(
            SNIP_BOX,
            Rect::new(h.left - 15, h.top - 10, h.right + 15, h.bottom + 10)
        );
    }

    #[test]
    fn a_nest_hatches_three_crabs_unless_its_record_asks_otherwise() {
        assert_eq!(nest::crab_count(0), 3);
        assert_eq!(nest::crab_count(1), 1);
        assert_eq!(nest::crab_count(9), 9);
        assert_eq!(nest::crab_count(10), 9);
        assert_eq!(nest::crab_count(100), 9);
        // Three crabs leave at -75 and -25 px/s, the last at a drawn speed.
        assert_eq!(nest::fan_start(3), -75);
        assert_eq!(nest::fan_start(3) + nest::FAN_STEP, -25);
        const { assert!(nest::SPAWN_TICK < nest::DONE_TICK) };
    }

    #[test]
    fn a_bomb_is_lobbed_one_hundred_and_fifty_px() {
        let (vx, vy) = bomb::throw_velocity(THROW_DISTANCE);
        assert!((vx - 387.298).abs() < 1e-3, "{vx}");
        assert!((vy - -687.298).abs() < 1e-3, "{vy}");
        let (vx, vy2) = bomb::throw_velocity(-THROW_DISTANCE);
        assert!((vx - -387.298).abs() < 1e-3, "{vx}");
        assert_eq!(vy, vy2, "the same height either way");
        assert_eq!(bomb::throw_velocity(0), (0.0, 0.0));
        // A 1000 ms fuse: the 56th tick after the throw.
        assert_eq!(bomb::FUSE_MS.div_ceil(18), 56);
    }
}
