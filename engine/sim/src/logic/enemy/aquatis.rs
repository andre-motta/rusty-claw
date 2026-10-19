//! Aquatis, the boss of the twelfth level: the octopus king who sits in
//! the water of a narrow cave right of the ledge Claw arrives on, with
//! three tentacles ([`super::tentacle`]) rising from the water along that
//! ledge, a row of five stalactites over his head and a glowing crack in
//! the rock behind him.
//!
//! He is a boss-class human enemy who never walks, jumps or blocks. He
//! idles, punches high or low ([`EnemyState::HighPunch`],
//! [`EnemyState::LowPunch`]) when Claw comes close, flinches when hit and
//! cowers after three flinches ([`EnemyState::Scared`]). Claw's sword,
//! pistol, magic and dynamite never take a point of his health: only a
//! falling stalactite does, [`STALACTITE_DAMAGE`] a stalactite, and the
//! fifth kills him.
//!
//! The fight is a loop driven by three helpers, each in its own file:
//!
//! - the crack in the wall ([`crack`]): a blast on it starts a
//!   [`QUAKE_MS`] earthquake that scares him;
//! - the five stalactites over him ([`stalactite`]): when the quake ends
//!   the next one in a fixed order falls on his head; he reels
//!   ([`EnemyState::StalactiteHit`]) and then thrashes in a
//!   [`FRENZY_MS`] frenzy ([`EnemyState::SuperFrenzy`]) with every
//!   tentacle;
//! - the hidden dynamite helper ([`dynamite`]), which puts one stick of
//!   dynamite back on a ledge of the room once Claw has used the last one.
//!
//! He can also be lifted: he never loses the human enemies' liftable
//! flag, so a Claw standing in mid air after Aquatis's own contact hit can
//! lift him on the next tick and drop him into the deadly water, where he
//! dies at once and releases the gem.
//!
//! Not built yet ([`BUILT`] is false): Aquatis, his three helpers and the
//! tentacles stay inert placeholders (drawn, no collision, reported as not
//! implemented), the calls below do nothing and the Aquatis link
//! ([`AquatisLink`]) keeps the values a level starts with.
//!
//! # The Aquatis link
//!
//! The helpers and the tentacles talk to Aquatis only through one
//! level-wide value kept in the tick's [`Outbox`](crate::logic::Outbox)
//! ([`AquatisLink`], reached through [`World::aquatis_link`] and
//! [`World::aquatis_link_mut`]): his object once he is built, whether he
//! is alive, his frenzy, the earthquake, the armed and the falling
//! stalactite, the outstanding dynamite refill, the stalactites' landing
//! height, the reset bits of his helpers ([`RESET_BITS`]), the frenzy's
//! punch toggle, the order counter of the stalactites and the tentacles'
//! hold on Claw. Every reader treats "no Aquatis" as "do nothing".
//!
//! Three paths into his file are wired already, each doing nothing while
//! he is not built:
//!
//! - a stalactite that touches him reaches [`stalactite_hit`], whichever
//!   of the two stalactite families it belongs to (his own five or a cave
//!   stalactite of [`crate::logic::stalactite`]);
//! - the return from the help screen passes its time to
//!   [`pass_help_time`] (from [`crate::Sim::pass_menu_time`]), for the
//!   gem's flight, which runs on game time;
//! - his respawn reset sets every reset bit ([`World::reset_aquatis`]),
//!   and each helper clears its own ([`World::clear_aquatis_reset_bit`]).
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he waits in the bosses'
//! `Startup`, idles in `Idle`, flinches in Raux's `HitHigh` and `HitLow`
//! and dies in Gabriel's `DeadAct1`; his five new names open the twelfth
//! level's block [`STATE_BLOCK`] (2280 to 2299) and the tentacles' three
//! follow from 2290 ([`super::tentacle::STATES`]). His helpers keep their
//! own states in their data.
//!
//! # Defaults taken
//!
//! - The state numbers: blocks of 20 in level order, so the twelfth level
//!   owns 2280 to 2299, Aquatis's names from 2280 and the tentacles' from
//!   2290; the frenzy number is shared by both.
//! - The frenzy's punch toggle and the stalactites' order counter are
//!   kept in the game's state, in the link: 0 and 1 when a level loads,
//!   the counter put back to 1 by every stalactite reset. A reference run
//!   that carries either over from an earlier attempt in the same game
//!   would show a divergence to record.
//! - The run-time depth of Aquatis and the tentacles, which decides their
//!   order against Claw in the update pass (3909 and 3899 while Claw is
//!   below them in the passage under the water, 4010 and 4000 once he is
//!   above): it is the shared enemy depth sort against Claw's feet
//!   ([`super::motion::DEPTH_BEHIND`], [`super::motion::DEPTH_IN_FRONT`]
//!   over a base of 10 for him and 0 for them), which puts an enemy
//!   behind Claw when his feet are below its own.
//! - The fight-pocket store of his room is played with Claw's dynamite at
//!   9 from route time zero: the replay's carried-in start lines do that
//!   from the level's start (all eight lines, see
//!   [`crate::player::progress::start_lines`]):
//!
//!   ```text
//!   # start_score = 0
//!   # start_health = 100
//!   # start_lives = 7
//!   # start_pistol = 10
//!   # start_magic = 5
//!   # start_dynamite = 9
//!   # start_weapon = pistol
//!   # start_treasure_game = 0, 0, 0, 0, 0, 0, 0, 0, 0
//!   ```
//!
//!   Claw's placement and the wait to time zero keep the count. Only the
//!   dynamite helper can tell the two apart (it remembers Claw's count
//!   when it is built, 3 in the original's run and 9 in the replay's), and
//!   that count matters only once the first stick has been taken, which
//!   that route never does.
//!
//! His data ([`AquatisData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::{GameLogic, World, boss_stage};
use crate::object::{LogicKind, ObjectId, Rect};

pub mod crack;
pub mod dynamite;
pub mod stalactite;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Aquatis"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_KINGAQUATIS";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_AQUATIS_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers reserved for the twelfth level's families:
/// Aquatis's five new states ([`STATES`]) from 2280 and the tentacles'
/// three ([`super::tentacle::STATES`]) from 2290; the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2280..=2299;

/// His new states.
pub const STATES: [EnemyState; 5] = [
    EnemyState::HighPunch,
    EnemyState::LowPunch,
    EnemyState::SuperFrenzy,
    EnemyState::StalactiteHit,
    EnemyState::Scared,
];

/// The pieces of the boss stage on his level: the warp into his room and
/// the stager beside where it puts Claw.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// His health before the difficulty factor, which is also the health a
/// full health bar stands for.
pub const HEALTH: i32 = 100;
/// Health a falling stalactite takes from him.
pub const STALACTITE_DAMAGE: i32 = 20;
/// Stalactites that kill him.
pub const STALACTITES: i32 = HEALTH / STALACTITE_DAMAGE;
/// Damage of touching him, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// His box (movement, hit and contact) facing left, the one of his idle
/// picture (measured).
pub const BOX: Rect = Rect::new(-62, -85, 109, 96);
/// How far left of him Claw must be for his punches, px.
pub const REACH: i32 = 200;

/// The earthquake a blast on the crack starts, ms.
pub const QUAKE_MS: u32 = 4000;
/// The crack's wait from one earthquake to the next blast it accepts, ms.
pub const CRACK_COOLDOWN_MS: u32 = 6000;
/// His frenzy after a stalactite's hit, ms.
pub const FRENZY_MS: u32 = 4000;
/// How far above his position the falling stalactites stop, px.
pub const LAND_ABOVE: i32 = 50;

/// Ticks of the introduction at the 18 ms step, from Claw's first tick in
/// the room (measured): the fight comes on one tick later and Claw stands
/// again the tick after.
pub const STAGING_TICKS: u32 = 735;
/// Ticks from his death's last act to the gem's landing at the 18 ms
/// step (measured).
pub const GEM_FLIGHT_TICKS: u32 = 140;

/// The crack's reset bit.
pub const CRACK_BIT: u32 = 0x2;
/// The tentacles' reset bits, by construction order.
pub const TENTACLE_BITS: [u32; 3] = [0x4, 0x8, 0x10];
/// The stalactites' reset bits, by construction order.
pub const STALACTITE_BITS: [u32; 5] = [0x20, 0x40, 0x80, 0x100, 0x200];
/// Every reset bit his respawn reset sets.
pub const RESET_BITS: u32 = 0x3FE;

/// His idle pose.
pub const IDLE_ANIM: &str = "LEVEL_AQUATIS_IDLE1";
/// The high punch.
pub const HIGH_PUNCH_ANIM: &str = "LEVEL_AQUATIS_STRIKE1";
/// The low punch.
pub const LOW_PUNCH_ANIM: &str = "LEVEL_AQUATIS_STRIKE2";
/// Cowering.
pub const SCARED_ANIM: &str = "LEVEL_AQUATIS_IDLE3";
/// His death.
pub const DEATH_ANIM: &str = "LEVEL_AQUATIS_KILLFALL";

/// The Aquatis class.
pub static CLASS: Class = Class {
    name: "Aquatis",
    human: true,
    construct: nothing,
    pre_tick: nothing,
    handler,
    go_home: nothing,
    go_idle: nothing,
    carry: None,
    pick_lift_voice: nothing,
    lift_roll: true,
    anims: ClassAnims::NONE,
    sounds: ClassSounds::NONE,
};

/// What the helpers and the tentacles share with Aquatis (see the module
/// documentation). It lasts the whole level and starts as
/// [`AquatisLink::default`] leaves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AquatisLink {
    /// Aquatis's object, once he is built.
    pub aquatis: Option<ObjectId>,
    /// He is alive: set when he is built, cleared at the end of his death.
    pub alive: bool,
    /// His frenzy runs; the tentacles thrash while it is set.
    pub frenzy: bool,
    /// An earthquake from the crack runs; he cowers while it is set.
    pub quake: bool,
    /// The next stalactite is armed: set when an earthquake ends, cleared
    /// when it falls.
    pub armed: bool,
    /// A stalactite is falling.
    pub falling: bool,
    /// A stick of dynamite the helper put down waits to be taken.
    pub refill_outstanding: bool,
    /// The height the falling stalactites stop at, px ([`LAND_ABOVE`]
    /// above him).
    pub land_y: i32,
    /// The reset bits still set ([`RESET_BITS`]): his respawn reset sets
    /// them all and each helper clears its own once it has reset.
    pub reset_bits: u32,
    /// The frenzy's punch toggle: the next frenzy punch is high when 0.
    pub frenzy_toggle: u32,
    /// The order of the next stalactite to fall (its record's smarts),
    /// from 1.
    pub stalactite_order: i32,
    /// A tentacle holds Claw.
    pub tentacle_grab: bool,
}

impl Default for AquatisLink {
    /// The link as a level loads: no Aquatis, nothing running, no reset
    /// bit, the toggle at 0 and the order counter at 1.
    fn default() -> AquatisLink {
        AquatisLink {
            aquatis: None,
            alive: false,
            frenzy: false,
            quake: false,
            armed: false,
            falling: false,
            refill_outstanding: false,
            land_y: 0,
            reset_bits: 0,
            frenzy_toggle: 0,
            stalactite_order: 1,
            tentacle_grab: false,
        }
    }
}

/// Aquatis's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AquatisData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

impl World<'_> {
    /// The Aquatis link (see [`AquatisLink`]).
    pub fn aquatis_link(&self) -> &AquatisLink {
        &self.out.aquatis
    }

    /// The Aquatis link, mutable (see [`World::aquatis_link`]).
    pub fn aquatis_link_mut(&mut self) -> &mut AquatisLink {
        &mut self.out.aquatis
    }

    /// Aquatis's object, while he is built and current.
    pub fn aquatis(&self) -> Option<ObjectId> {
        self.out.aquatis.aquatis
    }

    /// The reset bits still set (see [`AquatisLink::reset_bits`]).
    pub fn aquatis_reset_bits(&self) -> u32 {
        self.out.aquatis.reset_bits
    }

    /// Sets every reset bit ([`RESET_BITS`]), as his respawn reset and the
    /// start of a staged fight do: each helper resets itself on its next
    /// call and clears its bit.
    pub fn reset_aquatis(&mut self) {
        self.out.aquatis.reset_bits = RESET_BITS;
    }

    /// Clears the reset bit `bit`; true when it was set.
    pub fn clear_aquatis_reset_bit(&mut self, bit: u32) -> bool {
        let was = self.out.aquatis.reset_bits & bit != 0;
        self.out.aquatis.reset_bits &= !bit;
        was
    }

    /// True while an earthquake from the crack runs.
    pub fn aquatis_quake(&self) -> bool {
        self.out.aquatis.quake
    }

    /// True while a tentacle holds Claw.
    pub fn tentacle_grab(&self) -> bool {
        self.out.aquatis.tentacle_grab
    }

    /// Sets or clears the tentacles' hold on Claw.
    pub fn set_tentacle_grab(&mut self, held: bool) {
        self.out.aquatis.tentacle_grab = held;
    }
}

/// True when `id` is a stalactite of either family: one of Aquatis's five
/// or a cave stalactite.
pub fn is_stalactite(w: &World, id: ObjectId) -> bool {
    match w.objects.get(id).map(|o| o.kind) {
        Some(LogicKind::Game(g)) => {
            matches!(g, GameLogic::AquatisStalactite | GameLogic::Stalactite)
        }
        _ => false,
    }
}

/// The stalactite `attacker` touched Aquatis `id`: the path by which a
/// stalactite takes his health. Nothing yet.
pub fn stalactite_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// Play comes back from the help screen at game time `now_ms` between two
/// ticks (see [`crate::Sim::pass_menu_time`]): the time reaches what of
/// his fight runs on game time (the gem's flight). Nothing yet.
pub fn pass_help_time(w: &mut World, now_ms: u64) {
    let _ = (w, now_ms);
}

fn handler(s: EnemyState) -> Option<Handler> {
    let _ = s;
    None
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// `attacker` hit `id`: a stalactite goes to [`stalactite_hit`].
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if is_stalactite(w, attacker) {
        stalactite_hit(w, id, attacker);
    }
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn his_states_open_the_twelfth_levels_block() {
        assert_eq!((*STATE_BLOCK.start(), *STATE_BLOCK.end()), (2280, 2299));
        assert_eq!(STATES[0] as i32, *STATE_BLOCK.start());
        for s in STATES {
            assert!(STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
        }
        assert_eq!(
            *super::super::mercat::STATE_BLOCK.end() + 1,
            *STATE_BLOCK.start()
        );
        assert!(CLASS.human, "a human enemy");
        assert!(handler(EnemyState::Idle).is_none(), "not built yet");
    }

    #[test]
    fn five_stalactites_empty_his_health() {
        assert_eq!(STALACTITES, 5);
        assert_eq!(STALACTITE_BITS.len() as i32, STALACTITES);
        // The nine reset bits: the crack, three tentacles, five stalactites.
        let all = STALACTITE_BITS
            .iter()
            .chain(&TENTACLE_BITS)
            .fold(CRACK_BIT, |a, b| a | b);
        assert_eq!(all, RESET_BITS);
        assert_eq!(RESET_BITS.count_ones(), 9);
        // The quake and the frenzy each last 223 ticks at 18 ms.
        assert_eq!(QUAKE_MS.div_ceil(18), 223);
        assert_eq!(FRENZY_MS.div_ceil(18), 223);
    }

    #[test]
    fn the_link_starts_empty_with_the_order_at_one() {
        let l = AquatisLink::default();
        assert_eq!(l.aquatis, None);
        assert!(!l.alive && !l.frenzy && !l.quake && !l.armed && !l.falling);
        assert!(!l.refill_outstanding && !l.tentacle_grab);
        assert_eq!((l.reset_bits, l.frenzy_toggle, l.land_y), (0, 0, 0));
        assert_eq!(l.stalactite_order, 1);
    }

    #[test]
    fn his_art_is_his_own() {
        for anim in [
            IDLE_ANIM,
            HIGH_PUNCH_ANIM,
            LOW_PUNCH_ANIM,
            SCARED_ANIM,
            DEATH_ANIM,
        ] {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
    }
}
