//! Gabriel, the boss of the eighth level: the pirate captain who waits on
//! the raised deck of his ship above the dock, with the bombs he lobs
//! ([`bomb`]). His cannon, its lever and the button that turns it up are
//! the props of [`crate::logic::cannon_switch`].
//!
//! He is a human enemy with the boss layer on top, like Raux, Katherine
//! and Wolvington, but he never fights hand to hand. He stands on his deck
//! and runs a loop: an idle pose (`IDLE1` or `IDLE2`, picked at random),
//! then one of three attacks, one chance in three each:
//!
//! - he lobs bombs to his left ([`EnemyState::MultiBomb`]): three of the
//!   four lanes [`BOMB_LANES`], one left out at random, each bomb leaving
//!   from [`BOMB_SPAWN`] of his position;
//! - he calls a pirate in ([`EnemyState::ThugAttack`]): a red-tail pirate
//!   appears at [`PIRATE_SPAWN`] of his position, runs left along the deck
//!   and leaps down at Claw. With [`MAX_PIRATES`] of his pirates alive he
//!   only waits another idle cycle;
//! - he pulls the lever ([`EnemyState::CannonAttack`]): an order to his
//!   cannon, which fires along the dock at Claw, or straight up into him
//!   when Claw has struck the button and turned the barrel up.
//!
//! Swords, pistol shots, magic and blasts only make him shrug them off
//! ([`EnemyState::Blocked`]); the only thing that hurts him is a
//! cannonball ([`EnemyState::CannonHit`], [`CANNON_HIT_DAMAGE`] each).
//! The fifth ([`CANNON_HITS`]) runs his four death acts
//! ([`EnemyState::DeadAct1`] to [`EnemyState::DeadAct4`]), and the last
//! one releases the gem that ends the level. He can also be lifted and
//! thrown, and a throw into the water drowns him, which ends the fight at
//! once.
//!
//! Not built yet ([`BUILT`] is false): Gabriel and his bombs stay inert
//! placeholders (drawn, no collision, reported as not implemented), the
//! calls below do nothing, and [`World::gabriel_link`] is `None`, so his
//! cannon, its lever and button and the pirates' boss fight mode stay
//! inert too.
//!
//! # Boss stage
//!
//! His level's door and stager ([`STAGE`]) are the shared stage pieces and
//! run already: the door leads into the stager's box and the stager plays
//! the introduction. While he is not built nobody is the current boss, so
//! the introduction shows no health bar and his line takes no time; once
//! built he makes himself the current boss when he is set up, the
//! stager's health bar reads his health against [`METER_MAX`] (his
//! record's health is not used), and the introduction lasts
//! [`STAGING_TICKS`] ticks to the fight flag, Claw standing again
//! [`THAW_TICKS`] ticks after the first tick in the room. His defeat goes
//! through [`crate::logic::World::boss_defeated`], which launches the gem
//! to the point his record's speed fields name. A Claw who jumps into the
//! room on its first tick skips the introduction, and Gabriel then stays
//! in `Startup` for good until something acts on him: his range boxes
//! ([`RANGE`]) cover the whole room, so nothing ever sends him idle while
//! Claw is in it.
//!
//! # The link
//!
//! The cannon, the lever, the button, the bombs and the pirates he calls
//! reach him through [`World::gabriel_link`]: his [`GabrielLink`], held in
//! his data and found through the current boss, or `None` when he is not
//! there. Every reader treats `None`, or a link marked defeated, as "do
//! nothing".
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he waits for the fight in
//! the bosses' `Startup` and idles in the shared `Idle`, and his attacks,
//! his hits and his death acts are new names, the first nine numbers of
//! the eighth level's block [`STATE_BLOCK`] in the order the game
//! registers them ([`STATES`]). When Claw lifts and throws him he runs
//! the shared human states under their numbers. His cannon's turn is
//! [`EnemyState::Rotate`], in the cannons' block.
//!
//! # Defaults taken
//!
//! - The block of state numbers: blocks of 20 in level order, so the
//!   eighth level owns 2200 to 2219, his nine states 2200 to 2208; the
//!   numbers after them stay free. The cannon's turn takes the cannons'
//!   block (2061), next to their `VertFiring`, rather than 2209.
//! - The link lives in his data and is found through the current boss
//!   ([`World::gabriel_link`]); the cannon, lever, button, bombs and
//!   pirates read it there.
//! - Measured on the original, which wins over the static reading: the
//!   introduction ([`STAGING_TICKS`], [`THAW_TICKS`]), the idle waits
//!   ([`IDLE1_TICKS`], [`IDLE2_TICKS`]), the lift protection after a
//!   throw's landing ([`LIFT_PROTECTION_MS`]) and the four death acts
//!   ([`DEATH_ACT_TICKS`]); after a throw's recovery he settles in the
//!   shared strike for good, because his strike animation is never
//!   loaded.
//! - Of the pirates' boss fight mode, beyond the attack he sends them on
//!   (their landing after the leap, their wide range boxes while he lives,
//!   the level's other pirates): as the pirate's own rules, with the two
//!   additions in [`super::redtail_pirate`] (see its boss fight hooks).
//!
//! His data ([`GabrielData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::{ObjectData, World, boss_stage};
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Gabriel"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_GABRIEL";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_GABRIEL_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers reserved for the eighth level's families. His nine
/// states take the first nine ([`STATES`]); the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2200..=2219;

/// His own states, in the order the game registers them.
pub const STATES: [EnemyState; 9] = [
    EnemyState::MultiBomb,
    EnemyState::ThugAttack,
    EnemyState::CannonAttack,
    EnemyState::CannonHit,
    EnemyState::Blocked,
    EnemyState::DeadAct1,
    EnemyState::DeadAct2,
    EnemyState::DeadAct3,
    EnemyState::DeadAct4,
];

/// The pieces of the boss stage on his level: the door into his room and
/// the stager next to where it drops Claw.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// His health before the difficulty factor (the record's health is not
/// used), which is also the health a full health bar stands for.
pub const HEALTH: i32 = 100;

/// See [`HEALTH`].
pub const METER_MAX: i32 = HEALTH;

/// Damage of touching him, before the difficulty factor (the human
/// enemies' default).
pub const DAMAGE: i32 = 10;

/// Health a cannonball costs him.
pub const CANNON_HIT_DAMAGE: i32 = 20;

/// Cannonballs that kill him: the fifth runs the death acts, whatever his
/// health.
pub const CANNON_HITS: i32 = 5;

/// His five range boxes, all the same (half width, half height): Claw
/// anywhere in the room is close in.
pub const RANGE: (i32, i32) = (3000, 2000);

/// His move, hit, attack and contact box, facing left, from the first
/// picture of his set; facing right it is mirrored. It never changes with
/// the picture.
pub const BOX: Rect = Rect::new(-24, -55, 35, 34);

/// The horizontal targets of his four bomb lanes, px left of where a bomb
/// leaves; one lane is left out of each attack.
pub const BOMB_LANES: [i32; 4] = [-475, -350, -225, -100];

/// Where a bomb leaves, from his position.
pub const BOMB_SPAWN: (i32, i32) = (-75, -75);

/// The height below which a bomb explodes, px: the dock's.
pub const BOMB_FLOOR_Y: i32 = 5234;

/// Where a pirate he calls in appears, from his position.
pub const PIRATE_SPAWN: (i32, i32) = (150, -10);

/// With this many of his pirates alive he calls no more.
pub const MAX_PIRATES: i32 = 2;

/// The wait from the start of an attack to its first animation step, ms.
pub const ATTACK_DELAY_MS: u32 = 500;

/// The shortest idle before an attack, ms (the idle pose always lasts
/// longer).
pub const IDLE_MS: u32 = 1000;

/// Ticks from going idle to the next attack with the first idle pose, at
/// the 18 ms step (measured).
pub const IDLE1_TICKS: u32 = 142;

/// Ticks from going idle to the next attack with the second idle pose, at
/// the 18 ms step (measured).
pub const IDLE2_TICKS: u32 = 135;

/// Time after a blow he shrugged off before another can reach him, ms.
pub const BLOCK_COOLDOWN_MS: u32 = 500;

/// Time after a cannonball before a blow can reach him, ms.
pub const CANNON_HIT_COOLDOWN_MS: u32 = 1000;

/// Time of the first death act after its line starts, ms.
pub const DEAD_ACT1_MS: u32 = 4000;

/// Ticks of each death act at the 18 ms step, from the fifth hit's death
/// act on (measured): the first act, the fall from the deck to the dock,
/// the third and the fourth, which ends in the boss's defeat.
pub const DEATH_ACT_TICKS: [u32; 4] = [224, 32, 122, 293];

/// Time after a throw's landing before Claw can lift him again, ms
/// (measured).
pub const LIFT_PROTECTION_MS: u32 = 1500;

/// Ticks of the introduction at the 18 ms step, with him built: the fight
/// is on this many ticks after the fight flag first reads staging, which
/// is the tick after Claw's first tick in the room (measured).
pub const STAGING_TICKS: u32 = 734;

/// Ticks from Claw's first tick in the room to the tick he stands again
/// and Gabriel goes idle, the tick after the fight comes on (measured).
pub const THAW_TICKS: u32 = 736;

/// The first idle pose.
pub const IDLE1_ANIM: &str = "LEVEL_GABRIEL_IDLE1";
/// The second idle pose.
pub const IDLE2_ANIM: &str = "LEVEL_GABRIEL_IDLE2";
/// Calling a pirate in.
pub const THUG_ANIM: &str = "LEVEL_GABRIEL_STRIKE1";
/// Lobbing bombs and pulling the lever.
pub const STRIKE_ANIM: &str = "LEVEL_GABRIEL_STRIKE7";
/// The picture of [`STRIKE_ANIM`] on which the bombs leave and the lever
/// order is given.
pub const STRIKE_FRAME: i32 = 401;
/// Shrugging off a blow.
pub const BLOCKED_ANIM: &str = "LEVEL_GABRIEL_BLOCKED";
/// Struck by a cannonball.
pub const CANNON_HIT_ANIM: &str = "LEVEL_GABRIEL_HITHIGH";
/// The four death acts.
pub const DEATH_ANIMS: [&str; 4] = [
    "LEVEL_GABRIEL_KILLFALL1",
    "LEVEL_GABRIEL_KILLFALL2",
    "LEVEL_GABRIEL_KILLFALL3",
    "LEVEL_GABRIEL_KILLFALL4",
];

/// The Gabriel class.
pub static CLASS: Class = Class {
    name: "Gabriel",
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

/// What the cannon, its lever and button, his bombs and his pirates share
/// with Gabriel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GabrielLink {
    /// He pulled the lever: the cannon fires at its next chance and clears
    /// it; the lever flips while it is set.
    pub fire_order: bool,
    /// Claw struck the button: the cannon turns up and clears it when it
    /// turns back down after an upward shot.
    pub button: bool,
    /// His last death act ended the fight.
    pub defeated: bool,
    /// He waits for the cannon to be reset before choosing an attack.
    pub wait_for_cannon: bool,
    /// How many of the pirates he called in are alive.
    pub pirates: i32,
}

/// Gabriel's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GabrielData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// What his props, bombs and pirates read and write.
    pub link: GabrielLink,
    /// Cannonballs that hit him.
    pub cannon_hits: i32,
}

impl World<'_> {
    /// Gabriel's link (see [`GabrielLink`]): the current boss's, when the
    /// current boss is Gabriel; otherwise `None`.
    pub fn gabriel_link(&self) -> Option<&GabrielLink> {
        let boss = self.current_boss()?;
        match &self.objects.get(boss)?.data {
            ObjectData::Gabriel(d) => Some(&d.link),
            _ => None,
        }
    }

    /// Gabriel's link, mutable (see [`World::gabriel_link`]).
    pub fn gabriel_link_mut(&mut self) -> Option<&mut GabrielLink> {
        let boss = self.current_boss()?;
        match &mut self.objects.get_mut(boss)?.data {
            ObjectData::Gabriel(d) => Some(&mut d.link),
            _ => None,
        }
    }

    /// True when Gabriel is there and not defeated: the link's readers act
    /// only then.
    pub fn gabriel_fights(&self) -> bool {
        self.gabriel_link().is_some_and(|l| !l.defeated)
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

/// The bombs Gabriel lobs: each flies on the lowest arc that comes back to
/// its starting height [`BOMB_LANES`] px to his left, under a gravity of
/// [`bomb::GRAVITY`], and explodes on the first tick at or below the
/// dock's height [`BOMB_FLOOR_Y`] into a blast of [`bomb::EXPLODE_TICKS`]
/// ticks. In flight and in the blast it costs Claw [`bomb::DAMAGE`]; the
/// blast also hurts the pirates he calls in. It has no fuse and never
/// bounces: one that lands above the dock lies there unexploded. All of
/// them are removed when Gabriel is defeated.
///
/// Not built yet ([`BUILT`](bomb::BUILT) is false).
pub mod bomb {
    use crate::logic::World;
    use crate::object::{ObjectId, Rect};

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["GabrielBomb"];

    /// The bomb's picture.
    pub const IMAGE_SET: &str = "LEVEL_GABRIELBOMB";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &["LEVEL_GABRIELBOMB_"];

    /// The bomb in flight.
    pub const FALL_ANIM: &str = "LEVEL_GABRIELBOMB_FALL";
    /// The blast.
    pub const EXPLODE_ANIM: &str = "LEVEL_GABRIELBOMB_EXPLODE";

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = false;

    /// Damage of the bomb and of its blast (no difficulty factor).
    pub const DAMAGE: i32 = 5;
    /// The bomb's box in flight and the blast's.
    pub const BLAST: Rect = Rect::new(-50, -50, 50, 50);
    /// Gravity on a bomb, px/s^2.
    pub const GRAVITY: f64 = 2000.0;
    /// Ticks of the blast at the 18 ms step.
    pub const EXPLODE_TICKS: u32 = 16;

    /// The launch velocity, px/s, of a bomb aimed at the point `dx` px to
    /// the side at its own height (negative to the left): the lowest
    /// speed that reaches it, which leaves at 45 degrees with a speed of
    /// `sqrt(GRAVITY * |dx|)`.
    pub fn launch_velocity(dx: i32) -> (f64, f64) {
        let d = f64::from(dx);
        let v = (GRAVITY * d.abs() / 2.0).sqrt();
        (v.copysign(d), -v)
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
    fn his_states_open_the_eighth_levels_block_in_order() {
        for (i, s) in STATES.into_iter().enumerate() {
            assert!(STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
            assert_eq!(s as i32, STATE_BLOCK.start() + i as i32, "{}", s.name());
        }
        assert!(CLASS.human, "a human enemy");
        // The cannon's turn is a cannon state.
        assert_eq!(EnemyState::Rotate as i32, EnemyState::VertFiring as i32 + 1);
    }

    #[test]
    fn five_cannonballs_empty_his_health() {
        assert_eq!(CANNON_HIT_DAMAGE * CANNON_HITS, HEALTH);
        assert_eq!(METER_MAX, 100);
        // His box faces left: 24 px behind him, 35 ahead.
        assert_eq!((BOX.left, BOX.right), (-24, 35));
    }

    #[test]
    fn his_waits_follow_the_eighteen_millisecond_step() {
        // The attack's 500 ms wait ends on its 28th tick.
        assert_eq!(ATTACK_DELAY_MS.div_ceil(18), 28);
        // The 1000 ms idle is shorter than either pose.
        assert!(IDLE_MS.div_ceil(18) < IDLE2_TICKS.min(IDLE1_TICKS));
        // Counted from Claw's first tick in the room: the introduction
        // starts on the next tick, and Claw stands again the tick after
        // the fight comes on.
        assert_eq!(THAW_TICKS, 1 + STAGING_TICKS + 1);
        // The lift protection: 84 ticks.
        assert_eq!(LIFT_PROTECTION_MS.div_ceil(18), 84);
        assert_eq!(DEATH_ACT_TICKS.iter().sum::<u32>(), 671);
    }

    #[test]
    fn three_lanes_of_four_go_left_of_him() {
        assert!(BOMB_LANES.iter().all(|&dx| dx < 0));
        assert!(BOMB_LANES.windows(2).all(|p| p[1] - p[0] == 125));
        assert_eq!(BOMB_SPAWN, (-75, -75));
        // The pirate appears behind him, to his right.
        const { assert!(PIRATE_SPAWN.0 > 0) };
    }

    #[test]
    fn a_bomb_leaves_at_forty_five_degrees() {
        let (vx, vy) = bomb::launch_velocity(-475);
        assert!((vx + 689.2).abs() < 0.1, "{vx}");
        assert_eq!(vx, vy);
        let (vx, vy) = bomb::launch_velocity(100);
        assert!((vx - 316.2).abs() < 0.1, "{vx}");
        assert_eq!(vx, -vy);
        // Its speed reaches exactly the lane's point at its own height.
        for dx in BOMB_LANES {
            let (vx, vy) = bomb::launch_velocity(dx);
            let t = -2.0 * vy / bomb::GRAVITY;
            assert!((vx * t - f64::from(dx)).abs() < 1e-9, "{dx}");
        }
        assert_eq!(bomb::launch_velocity(0), (0.0, -0.0));
    }

    #[test]
    fn his_art_is_his_own() {
        for anim in [IDLE1_ANIM, IDLE2_ANIM, THUG_ANIM, STRIKE_ANIM, BLOCKED_ANIM]
            .iter()
            .chain(&[CANNON_HIT_ANIM])
            .chain(&DEATH_ANIMS)
        {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
        for anim in [bomb::FALL_ANIM, bomb::EXPLODE_ANIM] {
            assert!(anim.starts_with(bomb::ANIM_PREFIXES[0]), "{anim}");
        }
    }
}
