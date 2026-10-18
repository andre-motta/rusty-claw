//! Marrow, the boss of the tenth level: the pirate captain who waits on
//! the right-hand ledge of a cave room whose middle is a spike pit. Two
//! sliding floor slabs ([`crate::logic::marrow_floor`]) can bridge the pit,
//! and his parrot ([`super::parrot`]) circles above it.
//!
//! He is a human enemy with the boss layer on top, like Raux. His main
//! weapon is a pistol: whenever Claw is in sight and a 1 to 4 s wait has
//! run out he fires a bullet ([`BULLET_SPEED`] px/s, [`BULLET_DAMAGE`]
//! damage) from [`MUZZLE`] of his position, on the 19th tick of the shot
//! ([`FIRE_TICKS`]). He blocks some sword blows and every bullet, magic
//! shot and thrown object, and cannot be lifted.
//!
//! The fight is a loop. The pit starts open and the parrot swoops at Claw;
//! three hits send the parrot to its perch, and the slabs close into a
//! bridge. Claw crosses and hits Marrow. Once Marrow has lost
//! [`PHASE_DAMAGE`] health below his phase health he blocks every blow
//! and whistles for the parrot ([`EnemyState::WaitForParrot`]), which
//! carries him to the far ledge while the slabs slide open under anyone
//! still on the bridge. His going Home works only once the fight is on
//! (the boss fight flag at 2): after an intro skip he only idles, paces
//! and reacts, and the reference speedrun kills that passive Marrow with
//! the sword.
//!
//! Not built yet ([`BUILT`] is false): Marrow stays an inert placeholder
//! (drawn, no collision, reported as not implemented), the calls below do
//! nothing, and the Marrow link ([`World::marrow_link`]) stays `None`, so
//! the parrot and the slabs, which only act through it, stay still too.
//!
//! # The Marrow link
//!
//! The parrot and the two slabs talk to nothing but Marrow, through one
//! level-wide value kept in the tick's [`Outbox`](crate::logic::Outbox)
//! ([`MarrowLink`], reached through [`World::marrow_link`] and
//! [`World::marrow_link_mut`]): his object, set when he is built
//! ([`World::link_marrow`]) and cleared when his object is removed
//! ([`World::unlink_marrow`]), and four values they share with him: the
//! open request, the parrot call, the floors-open status and the phase
//! health. Every reader treats `None` as "do nothing": before Marrow is
//! built the slabs stay closed where the level puts them, and after his
//! body is removed they stop wherever they are for the rest of the level.
//!
//! # Boss stage
//!
//! His level's door and stager ([`STAGE`]) are the shared stage pieces and
//! run already. The room is built inside the passes of the door's warp:
//! on Claw's first tick in the room Marrow already idles, the slabs have
//! already started to open and the phase health reads 0. Measured on the
//! original at the 18 ms step, the introduction runs [`STAGING_TICKS`]
//! ticks, the fight comes on [`FIGHT_TICKS`] ticks after Claw's first tick
//! in the room and Claw stands again [`THAW_TICKS`] ticks after it; Marrow
//! is pacing by then.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he waits for the fight in
//! the bosses' `Startup`, decides in `Home`, idles in `Idle`, shoots in the
//! shared `Fire` and flinches in Raux's `HitHigh` and `HitLow`; his one new
//! name, [`EnemyState::WaitForParrot`], opens the tenth level's block
//! [`STATE_BLOCK`] (2240 to 2259), and the parrot's five new names follow
//! it ([`super::parrot::STATES`]).
//!
//! # Defaults taken
//!
//! - The tenth level's block of state numbers: blocks of 20 in level
//!   order, so the tenth level owns 2240 to 2259: the whistle 2240, the
//!   parrot's flights 2241 to 2245; the numbers after them stay free. The
//!   slabs keep their own states in their data
//!   ([`crate::logic::marrow_floor::state`]).
//! - The link is one level-wide value rather than fields of his data, so
//!   the parrot and the slabs read it without finding him first; it holds
//!   his object while it exists.
//! - The phase health before its first write: 0, as measured on Claw's
//!   first tick in the room in every run, so no parrot call and no
//!   health-rule block come before the slabs first close or Claw first
//!   dies in the room.
//! - While Safe (the whistle) he takes no hit; the other flag the whistle
//!   sets has no effect in this build.
//! - A blow that lands while another enemy holds the strike token: the
//!   shared token rule, unchanged.
//! - His idle sets no animation: the animation player then keeps the
//!   picture he showed last and every advance reports no event, which is
//!   what the game does (he idles frozen in his last pose).
//! - The slabs' own behaviour is [`crate::logic::marrow_floor`]'s; Marrow
//!   only writes the open request, the parrot call, the status and the
//!   phase health listed in [`MarrowLink`].
//!
//! His data ([`MarrowData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::{World, boss_stage};
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Marrow"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_MARROW";

/// His bullet's picture.
pub const BULLET_IMAGE_SET: &str = "GAME_BULLETS";

/// Image sets this family may switch an object to: his own and his
/// bullet's.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, BULLET_IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_MARROW_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers reserved for the tenth level's families: the
/// whistle ([`STATES`]) and the parrot's five flights
/// ([`super::parrot::STATES`]); the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2240..=2259;

/// His one new state.
pub const STATES: [EnemyState; 1] = [EnemyState::WaitForParrot];

/// The pieces of the boss stage on his level: the door into his room and
/// the stager next to where it drops Claw.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// His health before the difficulty factor (the record's is not used),
/// which is also the health a full health bar stands for.
pub const HEALTH: i32 = 100;

/// See [`HEALTH`].
pub const METER_MAX: i32 = HEALTH;

/// Damage of touching him and of his strike, before the difficulty factor.
pub const DAMAGE: i32 = 10;

/// His smarts at the default factor.
pub const SMARTS: i32 = 100;

/// Health he loses below his phase health before he blocks every blow and
/// whistles for the parrot.
pub const PHASE_DAMAGE: i32 = 25;

/// His strike's box, facing left.
pub const STRIKE: Rect = Rect::new(-117, -4, -30, 21);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-15, -60, 15, 52);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(0, -60, 20, 35);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [(500, 20), (300, 40), (300, 20), (150, 20), (100, 20)];

/// Where his bullet leaves, facing right; mirrored when he faces left.
pub const MUZZLE: (i32, i32) = (65, -24);
/// His bullet's speed, px/s.
pub const BULLET_SPEED: i32 = 600;
/// His bullet's damage (not scaled by the difficulty).
pub const BULLET_DAMAGE: i32 = 20;
/// The shot's bullet tick and its last tick, counted from the shot's
/// first tick, at the 18 ms step.
pub const FIRE_TICKS: (u32, u32) = (19, 34);
/// The shortest and longest wait between two of his shots, ms: 1000
/// times one more than a draw of 0..4.
pub const FIRE_WAIT_MS: (u32, u32) = (1000, 4000);

/// How long he waits for the parrot after the whistle before he drops
/// back, ms.
pub const WAIT_FOR_PARROT_MS: u32 = 5000;
/// The tick of the whistle on which the parrot is called, counted from
/// the whistle's first tick, at the 18 ms step.
pub const CALL_TICK: u32 = 16;
/// How far beside him the parrot takes hold of him, px (on the side of
/// the room's centre).
pub const GRAB_OFFSET: i32 = 58;

/// The x of the middle of his room: Claw left of it is on the left half
/// for him and his parrot.
pub const ROOM_CENTRE_X: i32 = 40808;

/// His depth once he is built: after Claw and the slabs, so a request he
/// writes reaches the slabs one tick later (measured).
pub const SORTED_Z: i32 = 4010;

/// Ticks of the introduction at the 18 ms step (measured).
pub const STAGING_TICKS: u32 = 747;
/// Ticks from Claw's first tick in the room to the tick the fight comes on
/// (measured).
pub const FIGHT_TICKS: u32 = 748;
/// Ticks from Claw's first tick in the room to the tick he stands again
/// (measured).
pub const THAW_TICKS: u32 = 749;

/// Ticks of his death before the gem rises, at the 18 ms step (measured).
pub const DEAD_TICKS: u32 = 52;
/// Ticks of the gem's rise to the point his record names, at the 18 ms
/// step (measured).
pub const GEM_RISE_TICKS: u32 = 148;

/// His pose while he waits for the fight.
pub const HOME_ANIM: &str = "LEVEL_MARROW_HOME";
/// The shot.
pub const FIRE_ANIM: &str = "LEVEL_MARROW_STRIKE1";
/// The strike.
pub const STRIKE_ANIM: &str = "LEVEL_MARROW_STRIKE2";
/// The whistle.
pub const WHISTLE_ANIM: &str = "LEVEL_MARROW_IDLE1";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_MARROW_KILLFALL";
/// The whistle's sound.
pub const WHISTLE_SOUND: &str = "LEVEL_MARROW_WHISTLE";
/// The shot's sound.
pub const PISTOL_SOUND: &str = "LEVEL_MARROW_PISTOL";

/// The Marrow class.
pub static CLASS: Class = Class {
    name: "Marrow",
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

/// What the parrot and the slabs share with Marrow (see the module
/// documentation). It exists from his building to the removal of his
/// object.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarrowLink {
    /// Marrow's object.
    pub marrow: ObjectId,
    /// The slabs open while it is set and close while it is clear: set when
    /// he is built, when Claw respawns and when the parrot lifts him;
    /// cleared when the parrot settles on its perch.
    pub open_request: bool,
    /// He called the parrot (the whistle); the perched parrot flies to him.
    pub parrot_call: bool,
    /// The slabs finished opening (set) or closing (clear); nothing reads
    /// it.
    pub floors_open: bool,
    /// The health from which he counts [`PHASE_DAMAGE`] before the next
    /// call: written by the slabs when they finish closing and when Claw's
    /// respawn resets them, by his drop after the whistle and by the end of
    /// the parrot's carry; 0 before the first write.
    pub phase_health: i32,
}

impl MarrowLink {
    /// The link of Marrow's object `marrow` as his building leaves it: the
    /// slabs asked to open, no call, closed, phase health 0.
    pub fn new(marrow: ObjectId) -> MarrowLink {
        MarrowLink {
            marrow,
            open_request: true,
            ..MarrowLink::default()
        }
    }
}

/// Marrow's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarrowData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

impl World<'_> {
    /// The Marrow link (see [`MarrowLink`]): `None` before Marrow is built
    /// and after his object is removed.
    pub fn marrow_link(&self) -> Option<&MarrowLink> {
        self.out.marrow.as_ref()
    }

    /// The Marrow link, mutable (see [`World::marrow_link`]).
    pub fn marrow_link_mut(&mut self) -> Option<&mut MarrowLink> {
        self.out.marrow.as_mut()
    }

    /// Marrow's object `id` is built: the link starts as
    /// [`MarrowLink::new`] leaves it.
    pub fn link_marrow(&mut self, id: ObjectId) {
        self.out.marrow = Some(MarrowLink::new(id));
    }

    /// Marrow's object `id` is being removed: the link ends, unless it
    /// holds another object.
    pub fn unlink_marrow(&mut self, id: ObjectId) {
        if self.out.marrow.is_some_and(|l| l.marrow == id) {
            self.out.marrow = None;
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
    fn his_whistle_opens_the_tenth_levels_block() {
        assert_eq!((*STATE_BLOCK.start(), *STATE_BLOCK.end()), (2240, 2259));
        assert_eq!(STATES[0] as i32, *STATE_BLOCK.start());
        assert_eq!(
            *super::super::hook_pirate::STATE_BLOCK.end() + 1,
            *STATE_BLOCK.start()
        );
        assert!(CLASS.human, "a human enemy");
        assert!(CLASS.carry.is_none(), "never lifted");
        assert!(handler(EnemyState::Home).is_none(), "not built yet");
    }

    #[test]
    fn four_phases_empty_his_health() {
        assert_eq!(HEALTH / PHASE_DAMAGE, 4);
        assert_eq!(METER_MAX, 100);
        // His bullet leaves 65 px ahead, at 600 px/s: 10.8 px a tick.
        assert_eq!(BULLET_SPEED * 18, 10_800);
        const { assert!(FIRE_TICKS.0 < FIRE_TICKS.1) };
        assert_eq!(FIRE_WAIT_MS, (1000, 4000));
        // The whistle's 5000 ms wait ends on its 278th tick.
        assert_eq!(WAIT_FOR_PARROT_MS.div_ceil(18), 278);
    }

    #[test]
    fn the_introduction_ends_one_tick_before_the_fight_and_two_before_the_thaw() {
        assert_eq!(FIGHT_TICKS, 1 + STAGING_TICKS);
        assert_eq!(THAW_TICKS, FIGHT_TICKS + 1);
        // He sorts after Claw (4000).
        const { assert!(SORTED_Z > 4000) };
    }

    #[test]
    fn the_link_starts_with_the_slabs_asked_open() {
        let l = MarrowLink::new(7);
        assert_eq!(l.marrow, 7);
        assert!(l.open_request);
        assert!(!l.parrot_call && !l.floors_open);
        assert_eq!(l.phase_health, 0);
    }

    #[test]
    fn his_art_is_his_own() {
        for anim in [HOME_ANIM, FIRE_ANIM, STRIKE_ANIM, WHISTLE_ANIM, DEATH_ANIM] {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
        for sound in [WHISTLE_SOUND, PISTOL_SOUND] {
            assert!(sound.starts_with(ANIM_PREFIXES[0]), "{sound}");
        }
        assert!(IMAGE_SETS.contains(&IMAGE_SET));
    }
}
