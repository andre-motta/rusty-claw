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
//! # The fight
//!
//! - Going idle he faces Claw and picks his pose with a draw of 0..100
//!   (below 50 the first, [`IDLE1_TICKS`] long, otherwise the second,
//!   [`IDLE2_TICKS`]). Once the pose has played out, at least
//!   [`IDLE_MS`] have passed and the cannon has acknowledged his last
//!   reset ([`GabrielLink::wait_for_cannon`]), he faces Claw and draws
//!   0..3 for his attack.
//! - The bombs: he faces left, says one of three lines and waits
//!   [`ATTACK_DELAY_MS`]; then his throw plays, and on its first picture
//!   [`STRIKE_FRAME`] he draws 0..4 for the lane he leaves out and three
//!   bombs leave. He goes idle when the throw ends: bombs 36 ticks and
//!   idle 66 ticks after the attack started.
//! - The pirate: his call plays (one of four lines, drawn on its first
//!   tick) and when it ends, 13 ticks in, the pirate appears and is sent
//!   along the deck ([`crate::logic::enemy::redtail_pirate`]'s boss fight
//!   attack), and he goes idle.
//! - The lever: he faces right, says one of two lines and waits
//!   [`ATTACK_DELAY_MS`]; on [`STRIKE_FRAME`] of the same throw he gives the
//!   cannon its fire order, and when the throw ends he faces left and goes
//!   idle.
//! - A blow of Claw's (a new attack, outside his block, his cannon hit and
//!   his death acts, and [`BLOCK_COOLDOWN_MS`] after the last one), a
//!   pistol shot, magic or a blast: he faces Claw and blocks
//!   ([`BLOCKED_ANIM`], one of three lines on its first tick), then goes
//!   idle 30 ticks later; a pistol shot or magic is destroyed.
//! - A cannonball (any enemy shot that reaches him outside his death
//!   acts): he counts the hit, plays [`CANNON_HIT_ANIM`] after a
//!   [`ATTACK_DELAY_MS`] wait (one of two lines with it, for the first
//!   four), and when it ends loses [`CANNON_HIT_DAMAGE`] and answers with
//!   his bombs, 46 ticks after the hit. The fifth needs no wait: 19 ticks
//!   after it his health is 0 and his death acts start. The ball is
//!   destroyed and a hit burst shows where it struck; no blow can reach
//!   him for [`CANNON_HIT_COOLDOWN_MS`].
//!
//! # Death acts
//!
//! In his death acts he hurts nobody and nothing hits him.
//!
//! 1. [`DEATH_ANIMS`]`[0]` loops; on its first tick (once no line of his
//!    plays) he says [`DEATH_LINES`]`[0]`, and [`DEAD_ACT1_MS`] later the
//!    second act starts. While its first picture shows he turns round
//!    every [`TURN_MS`].
//! 2. He faces right and leaps towards [`DEATH_LEAP`] of his first place on
//!    the lowest arc that reaches it, from where he stands exactly (see
//!    Standing below); he flies past it off the deck's end and lands on
//!    the dock, 32 ticks later.
//! 3. [`DEATH_ANIMS`]`[2]` plays, 122 ticks.
//! 4. Once no line of his plays he says [`DEATH_LINES`]`[1]` and
//!    [`DEATH_ANIMS`]`[3]` plays; on its last tick, with Claw alive, Claw is
//!    held still, the gem rises out of Gabriel
//!    ([`crate::logic::World::boss_defeated`]) and he is defeated
//!    ([`GabrielLink::defeated`]): every reader of his link stops, and he
//!    stays there, never removed, so the health bar stays on screen,
//!    empty. With Claw dead at that moment nothing more happens, ever,
//!    until Claw's respawn starts the fight over.
//!
//! # Standing
//!
//! Like Wolvington, once he has been set down (his start of fight lifts
//! him and sets him back on his deck) he sinks a third of a tick's fall
//! below his whole pixel on each tick he stands on
//! ([`wolvington::next_stand_fraction`]), landing
//! [`wolvington::LANDED_FRACTION`] below it, so his leap in the second
//! death act leaves from his exact height. The pirates he calls in stand
//! the same way from the tick they are made, set down on his deck, and
//! leave on their leap from their exact place ([`pirates`]).
//!
//! # Lifting him
//!
//! He can be lifted and thrown like a red-tail pirate (Claw carries him
//! the same way) in every state, his start-up pose included, and keeps the
//! picture he showed when he was lifted: he has no animation of his own
//! for being carried, thrown or getting up. A throw's landing costs him 4,
//! and he cannot be lifted again for 1.5 s. Getting up he decides as any
//! human enemy (Home): a pause of 300 to 600 ms or his strike, and his
//! strike never ends, so after a throw or a drop he never attacks again
//! unless a blow wakes him. A throw into the water ends the fight at once:
//! he is gone, and with Claw alive the gem rises from where he sank.
//!
//! # Boss stage
//!
//! His level's door and stager ([`STAGE`]) are the shared stage pieces: the
//! door leads into the stager's box and the stager plays the introduction.
//! He makes himself the current boss when he is set up, the stager's
//! health bar reads his health against [`METER_MAX`] (his record's health
//! is not used), and the introduction lasts [`STAGING_TICKS`] ticks to the
//! fight flag, Claw standing again [`THAW_TICKS`] ticks after the first
//! tick in the room. When the fight flag reaches 2 he runs his start of
//! fight ([`start_of_fight`]) and goes idle. His defeat goes through
//! [`crate::logic::World::boss_defeated`], which launches the gem to the
//! point his record's speed fields name.
//!
//! A Claw who jumps into the room on its first tick skips the
//! introduction, and Gabriel then stays in `Startup` for good until
//! something acts on him: a blow, a lift, walking back into the stager's
//! box (the whole introduction then runs), or Claw coming back at the level
//! start after losing a life (he is then out of all of Gabriel's range
//! boxes [`RANGE`], which cover the whole room, and Gabriel goes idle and
//! fights on alone).
//!
//! # When Claw respawns
//!
//! While the fight flag is set, Claw's respawn runs Gabriel's start of
//! fight again: full health, back at his place, no cannon hits, no pirates
//! counted, the cannon's flags cleared and its reset asked for, and idle
//! (once defeated, only idle). Everything thrown or fired and the pirates
//! he called in are marked for removal. With the flag at 0 (after a
//! skipped introduction) nothing of this happens.
//!
//! The respawn itself runs this reset ([`respawn_reset`]), once Claw is
//! back at his checkpoint and before the respawn's update passes, not his
//! own logic call: in the first pass, an object listed before him (a
//! pirate he called in, made one step in front of him) already sees him
//! reset, and its own call there is its last, since what was marked for
//! removal is gone before the second pass.
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
//! In a reference trace of the original game on this level his states
//! carry other numbers; [`REFERENCE_STATE_IDS`] maps his to them, so a
//! comparison can line them up.
//!
//! # Random draws, in order
//!
//! - building: one of 0..100 for his pose;
//! - each logic call: the shared pre-tick's roll (frame pulse ticks only);
//! - going idle: one of 0..100 (not once defeated);
//! - Idle: one of 0..3 for the attack, then the attack's line (the bombs'
//!   one of 0..3, the lever's one of 0..2);
//! - the bombs: one of 0..4 for the lane left out; each bomb draws one of
//!   0..3 on its first tick and one of 0..3 when it explodes;
//! - the pirate: one of 0..4 for the line on its first tick; the pirate's
//!   own building draws, then his going idle's;
//! - a block: one of 0..3 for the line on its first tick;
//! - a cannonball hit (in the collision pass): one of 0..2 for the line
//!   (not for the fifth), then the hit burst's two.
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
//! - Measured in runs of the original game: the
//!   introduction ([`STAGING_TICKS`], [`THAW_TICKS`]), the idle waits
//!   ([`IDLE1_TICKS`], [`IDLE2_TICKS`]), the lift protection after a
//!   throw's landing ([`LIFT_PROTECTION_MS`]) and the four death acts
//!   ([`DEATH_ACT_TICKS`]); after a throw's recovery he settles in the
//!   shared strike for good, because his strike animation is never
//!   loaded; Claw's respawn runs his start of fight only while the fight
//!   flag is set.
//! - The removal of his pirates and of everything thrown or fired when
//!   Claw respawns during the fight: the original no longer has the
//!   pirates after the respawn; how it removes them is not known.
//! - Of the pirates' boss fight mode, beyond the attack he sends them on
//!   (their landing after the leap, their wide range boxes while he lives,
//!   the level's other pirates): as the pirate's own rules, with the
//!   additions of [`pirates`].
//!
//! His data ([`GabrielData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use std::ops::RangeInclusive;

use super::human::{self, BossFields, HumanData};
use super::lift::{self, Carry};
use super::{
    Body, Class, ClassAnims, ClassSounds, EnemyData, EnemyFields, EnemyState, GRAVITY, Handler,
    Timer, attacker, build, data, data_mut, drop_to_floor, face_claw, hit_burst, human_fields, obj,
    obj_mut, play, rand, redtail_pirate, say, scaled, set_anim, set_facing, set_state, states,
    wolvington,
};
use crate::logic::projectile::RESPAWN_PURGE;
use crate::logic::world_boss::fight;
use crate::logic::{ObjectData, World, boss_stage, new_object, projectile, sound};
use crate::object::{ObjectId, Rect, engine_state, flags, types};
use crate::physics::ptype;

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
pub const BUILT: bool = true;

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

/// His states (and the shared ones he reaches) and the numbers a reference
/// trace of the original game on this level gives them.
pub const REFERENCE_STATE_IDS: [(EnemyState, i32); 23] = [
    (EnemyState::Idle, 2000),
    (EnemyState::Strike, 2003),
    (EnemyState::Dead, 2004),
    (EnemyState::Bounce, 2008),
    (EnemyState::Lifted, 2009),
    (EnemyState::Thrown, 2010),
    (EnemyState::Pause, 2011),
    (EnemyState::Dropped, 2012),
    (EnemyState::Flailing, 2017),
    (EnemyState::Home, 2018),
    (EnemyState::Startup, 2028),
    (EnemyState::MultiBomb, 2082),
    (EnemyState::ThugAttack, 2083),
    (EnemyState::CannonAttack, 2084),
    (EnemyState::CannonHit, 2085),
    (EnemyState::Blocked, 2086),
    (EnemyState::DeadAct1, 2087),
    (EnemyState::DeadAct2, 2088),
    (EnemyState::DeadAct3, 2089),
    (EnemyState::DeadAct4, 2090),
    (EnemyState::GabrielAttack, 2039),
    (EnemyState::Jump, 2005),
    (EnemyState::Advance, 2021),
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

/// His movement box while Claw carries him, facing left.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);

/// What may hit him: Claw himself, his pistol shots and magic, the enemy
/// shots (cannonballs) and blasts. Thrown objects pass through him.
pub const HIT_BY: u32 =
    types::PLAYER | types::PISTOL_SHOT | types::ENEMY_SHOT | types::LEVEL_SHOT | attacker::MAGIC;

/// How Claw carries him: the shared human offsets.
pub const CARRY: Carry = Carry {
    beside: (-20, 20, -15),
    overhead: (10, -10, -45),
    move_rect: CARRIED_MOVE_RECT,
};

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

/// A pirate he called in runs along his deck until it is no further right
/// than this far left of his first place, px; then it leaps.
pub const PIRATE_RUN_TO: i32 = 10;

/// How far left of where his pirates stop running they aim their leap,
/// px.
pub const PIRATE_LEAP_ACROSS: i32 = 150;

/// The wait from the start of an attack (and of a cannon hit) to its first
/// animation step, ms.
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

/// How often he turns round in his first death act, ms.
pub const TURN_MS: u32 = 100;

/// The picture of his first death act on which he turns round.
pub const TURN_FRAME: i32 = 900;

/// Where his fall in the second death act aims, from his first place: the
/// point the pirates aim at, 100 px higher than the dock instead of 400.
pub const DEATH_LEAP: (i32, i32) = (-PIRATE_RUN_TO - PIRATE_LEAP_ACROSS, -100);

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

/// His lines as he starts lobbing bombs, one at random.
pub const BOMB_LINES: [&str; 3] = [
    "LEVEL_GABRIEL_00400002",
    "LEVEL_GABRIEL_00400028",
    "LEVEL_GABRIEL_00400029",
];
/// His lines as he calls a pirate in, one at random.
pub const THUG_LINES: [&str; 4] = [
    "LEVEL_GABRIEL_00400011",
    "LEVEL_GABRIEL_00400012",
    "LEVEL_GABRIEL_00400022",
    "LEVEL_GABRIEL_00400022",
];
/// His lines as he goes for the lever, one at random.
pub const CANNON_LINES: [&str; 2] = ["LEVEL_GABRIEL_00400014", "LEVEL_GABRIEL_00400015"];
/// His lines as he shrugs off a blow, one at random.
pub const BLOCKED_LINES: [&str; 3] = [
    "LEVEL_GABRIEL_00400016",
    "LEVEL_GABRIEL_00400017",
    "LEVEL_GABRIEL_00400030",
];
/// His cry at the first four cannonballs, one at random (both the same),
/// played as a plain sound.
pub const HIT_LINES: [&str; 2] = ["LEVEL_GABRIEL_00400023", "LEVEL_GABRIEL_00400023"];
/// His lines in the first and in the last death act.
pub const DEATH_LINES: [&str; 2] = ["LEVEL_GABRIEL_00400009", "LEVEL_GABRIEL_00400010"];

/// The Gabriel class.
pub static CLASS: Class = Class {
    name: "Gabriel",
    human: true,
    construct,
    pre_tick: super::nothing,
    handler,
    go_home: states::go_home,
    go_idle,
    carry: Some(CARRY),
    pick_lift_voice: super::nothing,
    lift_roll: true,
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
    /// His pirates run along the deck while their x is greater than this.
    pub run_to: i32,
    /// Where his pirates aim their leap down.
    pub leap_to: (i32, i32),
    /// His bombs explode at this height or lower.
    pub floor_y: i32,
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
    /// The idle's shortest time.
    pub idle_wait: Timer,
    /// The wait before an attack's (or a cannon hit's) animation starts.
    pub delay: Timer,
    /// The once-per-state step of the state has been done: the bombs have
    /// left, the lever order is given, the line is said, the pirate is
    /// called in, the cannon hit is paid for, the death act's line is said.
    pub done: bool,
    /// The first death act's time, from its line.
    pub act_timer: Timer,
    /// The first death act's turning round.
    pub turn_timer: Timer,
    /// The last death act reached its end (checked once).
    pub ended: bool,
    /// The pirates he called in that he still counts as alive.
    pub summoned: Vec<ObjectId>,
    /// When each of [`GabrielData::summoned`] was made, game ms.
    pub summoned_ms: Vec<u64>,
    /// How far below his whole pixel he stands, px (see
    /// [`wolvington::next_stand_fraction`]).
    pub stand_y: f64,
    /// He has been set down or in the air since he was built: standing, he
    /// sinks below his pixel from then on.
    pub falls: bool,
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

/// Gabriel's own data.
pub fn gabriel_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a GabrielData> {
    match &w.objects.get(id)?.data {
        ObjectData::Gabriel(d) => Some(d),
        _ => None,
    }
}

fn gd_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut GabrielData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Gabriel(d) => Some(d),
        _ => None,
    }
}

fn link(w: &World, id: ObjectId) -> GabrielLink {
    gabriel_data(w, id)
        .map(|d| d.link.clone())
        .unwrap_or_default()
}

fn defeated(w: &World, id: ObjectId) -> bool {
    gabriel_data(w, id).is_some_and(|d| d.link.defeated)
}

fn done(w: &World, id: ObjectId) -> bool {
    gabriel_data(w, id).is_some_and(|d| d.done)
}

fn set_done(w: &mut World, id: ObjectId, value: bool) {
    if let Some(d) = gd_mut(w, id) {
        d.done = value;
    }
}

fn start_delay(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if let Some(d) = gd_mut(w, id) {
        d.delay = Timer::new(now, u64::from(ATTACK_DELAY_MS));
    }
}

fn delay_over(w: &World, id: ObjectId) -> bool {
    let now = w.now_ms;
    gabriel_data(w, id).is_none_or(|d| d.delay.done(now))
}

fn anim_done(w: &World, id: ObjectId) -> bool {
    obj(w, id).anim.is_done()
}

/// The number a reference trace of the original game gives his state
/// `s`.
pub fn reference_state_id(s: EnemyState) -> Option<i32> {
    REFERENCE_STATE_IDS
        .iter()
        .find(|(state, _)| *state == s)
        .map(|&(_, id)| id)
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Startup => Some(startup_tick),
        EnemyState::Idle => Some(idle_tick),
        EnemyState::MultiBomb => Some(multibomb_tick),
        EnemyState::ThugAttack => Some(thug_tick),
        EnemyState::CannonAttack => Some(cannon_attack_tick),
        EnemyState::CannonHit => Some(cannon_hit_tick),
        EnemyState::Blocked => Some(blocked_tick),
        EnemyState::DeadAct1 => Some(dead_act1_tick),
        EnemyState::DeadAct2 => Some(dead_act2_tick),
        EnemyState::DeadAct3 => Some(dead_act3_tick),
        EnemyState::DeadAct4 => Some(dead_act4_tick),
        EnemyState::Strike => Some(strike_tick),
        EnemyState::Lifted => Some(lifted_tick),
        _ => None,
    }
}

// ----------------------------------------------------------------------
// The logic call

/// One logic call: the human logic call ([`human::run`]), then his
/// drowning's report. Being removed, he stops being the current boss.
/// Claw's respawn resets him from the respawn itself ([`respawn_reset`]).
pub fn tick(w: &mut World, id: ObjectId) {
    let raw = obj(w, id).state;
    if raw == engine_state::DESTROYING && w.current_boss() == Some(id) {
        w.set_current_boss(None);
    }
    count_pirates(w, id);
    let stood = standing(w, id);
    human::run(w, id, &CLASS);
    settle(w, id, stood);
    after_call(w, id);
}

fn standing(w: &World, id: ObjectId) -> bool {
    w.objects
        .get(id)
        .is_some_and(|o| o.physics_type == ptype::STAND)
}

/// After his movement step: standing on from the last tick he sinks on
/// below his pixel (once he has been set down or in the air); landing he
/// stands [`wolvington::LANDED_FRACTION`] below it; in the air nothing.
fn settle(w: &mut World, id: ObjectId, stood: bool) {
    let stands = standing(w, id);
    let ay = data(w, id).map_or(GRAVITY, |d| d.ay);
    if let Some(d) = gd_mut(w, id) {
        if stood && stands {
            d.stand_y = if d.falls {
                wolvington::next_stand_fraction(d.stand_y, ay)
            } else {
                0.0
            };
        } else if stands {
            d.stand_y = wolvington::LANDED_FRACTION;
        } else {
            d.stand_y = 0.0;
            d.falls = true;
        }
    }
}

/// Claw's respawn, as the current boss: run by the respawn once Claw is
/// back at his checkpoint and before its update passes
/// ([`World::reset_boss_on_respawn`], see the module documentation). He
/// starts his fight over when the fight flag is set ([`respawned`]).
pub fn respawn_reset(w: &mut World, id: ObjectId) {
    respawned(w, id);
}

/// Each of his pirates that died (or is gone) since his last call lowers
/// his count of live pirates by one.
fn count_pirates(w: &mut World, id: ObjectId) {
    let Some(list) = gabriel_data(w, id).map(|d| d.summoned.clone()) else {
        return;
    };
    if list.is_empty() {
        return;
    }
    let made = gabriel_data(w, id).map_or_else(Vec::new, |d| d.summoned_ms.clone());
    let (alive, alive_ms): (Vec<ObjectId>, Vec<u64>) = list
        .iter()
        .copied()
        .zip(made.iter().copied().chain(std::iter::repeat(0)))
        .filter(|&(p, _)| {
            w.objects
                .get(p)
                .is_some_and(|o| o.flags & flags::DESTROY == 0)
                && data(w, p).is_some_and(|d| !d.dead)
        })
        .unzip();
    let gone = (list.len() - alive.len()) as i32;
    if let Some(d) = gd_mut(w, id) {
        d.summoned = alive;
        d.summoned_ms = alive_ms;
        if !d.link.defeated {
            d.link.pirates -= gone;
        }
    }
}

/// Claw respawned: with the fight flag set, the start of fight again, and
/// everything thrown or fired and his pirates removed.
fn respawned(w: &mut World, id: ObjectId) {
    let attempt = w.attempt();
    with_boss(w, id, |b| b.attempt = attempt);
    if w.boss_fight() == fight::NONE {
        return;
    }
    start_of_fight(w, id);
    projectile::purge(w.objects);
}

fn with_boss(w: &mut World, id: ObjectId, f: impl FnOnce(&mut BossFields)) {
    if let Some(b) = data_mut(w, id).and_then(|d| d.human.boss.as_mut()) {
        f(b);
    }
}

/// What follows each logic call: a death seen for the first time (in the
/// water) is reported.
fn after_call(w: &mut World, id: ObjectId) {
    let Some((dead, seen)) = data(w, id).map(|d| {
        (
            d.dead && super::state(w, id) == Some(EnemyState::Dead),
            d.human.boss.is_some_and(|b| b.death_seen),
        )
    }) else {
        return;
    };
    if dead && !seen {
        boss_died(w, id);
    }
}

/// He died: when Claw is alive, his defeat is reported (once).
fn boss_died(w: &mut World, id: ObjectId) {
    let claw_alive = w.claw.stats.health > 0;
    let mut report = false;
    with_boss(w, id, |b| {
        b.death_seen = true;
        if claw_alive && !b.defeat_reported {
            b.defeat_reported = true;
            report = true;
        }
    });
    if report {
        w.boss_defeated(id);
    }
}

// ----------------------------------------------------------------------
// Building

/// Builds Gabriel: the current boss, his place kept for the start of
/// fight, his box, 100 health and 10 damage (times the difficulty
/// factors), smarts 99, liftable, always active, his link with the
/// cannon's reset asked for; then he drops to his deck and waits for the
/// fight in `Startup` in the pose he draws. Draws: one of 0..100.
pub fn construct(w: &mut World, id: ObjectId) {
    let start = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    w.set_current_boss(Some(id));
    let full_health = scaled(HEALTH, super::HEALTH_FACTOR);
    let fields = EnemyFields {
        ranges: [RANGE; 5],
        contact_rect: BOX,
        strike_rect: BOX,
        strike_anim: "",
        death_anim: "",
        death_sound: "",
        lifted_voice: "",
        may_lift: true,
        human: HumanData {
            boss: Some(BossFields {
                start,
                full_health,
                attempt: w.attempt(),
                ..BossFields::default()
            }),
            ..HumanData::default()
        },
        ..human_fields(w, id)
    };
    let body = Body {
        image_set: IMAGE_SET,
        move_rect: BOX,
        hit_rect: BOX,
        attack_rect: BOX,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HIT_BY,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    o.flags |= flags::ALWAYS_ACTIVE;
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::Gabriel(GabrielData {
            enemy,
            link: fresh_link(start.0),
            ..GabrielData::default()
        });
    }
    set_state(w, id, EnemyState::Startup);
    let anim = pick_pose(w);
    set_anim(w, id, anim);
}

/// His link as the start of a fight leaves it, for his first place `x`.
fn fresh_link(x: i32) -> GabrielLink {
    let run_to = x - PIRATE_RUN_TO;
    GabrielLink {
        wait_for_cannon: true,
        run_to,
        leap_to: (
            run_to - PIRATE_LEAP_ACROSS,
            BOMB_FLOOR_Y - redtail_pirate::GABRIEL_LEAP_RISE,
        ),
        floor_y: BOMB_FLOOR_Y,
        ..GabrielLink::default()
    }
}

/// Draws his pose: one of 0..100, below 50 the first.
fn pick_pose(w: &mut World) -> &'static str {
    if rand(w, 100) < 50 {
        IDLE1_ANIM
    } else {
        IDLE2_ANIM
    }
}

/// His start of fight, run when the fight flag reaches 2 and when Claw
/// respawns during the fight: once defeated he only goes idle; otherwise
/// full health, back at his first place and dropped to his deck, standing,
/// hurting and hittable again, no cannon hits and no pirates counted, the
/// cannon's order and button cleared and its reset asked for, and idle.
pub fn start_of_fight(w: &mut World, id: ObjectId) {
    if defeated(w, id) {
        go_idle(w, id);
        return;
    }
    let Some(b) = data(w, id).and_then(|d| d.human.boss) else {
        return;
    };
    {
        let o = obj_mut(w, id);
        o.health = b.full_health;
        o.x = b.start.0;
        o.y = b.start.1;
        o.flags &= !(flags::HARMLESS | flags::SAFE);
        o.type_flags = types::ENEMY;
        let right = super::faces_right(o);
        o.move_rect = if right { BOX.mirrored() } else { BOX };
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
        d.ay = GRAVITY;
        d.frozen = false;
        d.busy = false;
    }
    drop_to_floor(w, id);
    if let Some(d) = gd_mut(w, id) {
        d.falls = true;
        d.stand_y = wolvington::LANDED_FRACTION;
        d.cannon_hits = 0;
        d.link = fresh_link(b.start.0);
        d.ended = false;
        d.summoned.clear();
        d.summoned_ms.clear();
    }
    go_idle(w, id);
}

// ----------------------------------------------------------------------
// Idle and the attack choice

/// Goes idle: once defeated, only the state; otherwise he faces Claw,
/// draws his pose and starts the idle's shortest time. Draws: one of
/// 0..100 (not once defeated).
pub fn go_idle(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Idle);
    if defeated(w, id) {
        return;
    }
    face_claw(w, id);
    let anim = pick_pose(w);
    set_anim(w, id, anim);
    let now = w.now_ms;
    if let Some(d) = gd_mut(w, id) {
        d.idle_wait = Timer::new(now, u64::from(IDLE_MS));
    }
}

/// Startup: his pose plays (once, then holds); when the fight flag reads
/// 2 he starts the fight.
pub fn startup_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if w.boss_fight() == fight::ON {
        start_of_fight(w, id);
    }
}

/// Idle (not once defeated): his pose plays; once it has played out, the
/// idle's shortest time has passed and the cannon has acknowledged his
/// reset, he chooses his attack.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    if defeated(w, id) {
        return;
    }
    w.advance_anim(id);
    let now = w.now_ms;
    let ready =
        gabriel_data(w, id).is_some_and(|d| d.idle_wait.done(now) && !d.link.wait_for_cannon);
    if ready && anim_done(w, id) {
        choose_attack(w, id);
    }
}

/// The attack choice: a draw of 0..3, then he faces Claw: 0 the bombs, 1
/// the pirate, 2 the lever.
pub fn choose_attack(w: &mut World, id: ObjectId) {
    let r = rand(w, 3);
    face_claw(w, id);
    match r {
        0 => enter_multibomb(w, id),
        1 => enter_thug(w, id),
        _ => enter_cannon_attack(w, id),
    }
}

// ----------------------------------------------------------------------
// The bombs

/// Starts the bombs: he faces left, his throw, one of three lines and the
/// attack's wait. Draws: one of 0..3.
pub fn enter_multibomb(w: &mut World, id: ObjectId) {
    set_facing(w, id, false);
    set_state(w, id, EnemyState::MultiBomb);
    set_anim(w, id, STRIKE_ANIM);
    set_done(w, id, false);
    let line = BOMB_LINES[rand(w, 3) as usize];
    say(w, id, line);
    start_delay(w, id);
}

/// MultiBomb: after the wait his throw plays; on its first
/// [`STRIKE_FRAME`] three bombs leave (a draw of 0..4 picks the lane left
/// out); when it ends he goes idle.
pub fn multibomb_tick(w: &mut World, id: ObjectId) {
    if !delay_over(w, id) {
        return;
    }
    w.advance_anim(id);
    if obj(w, id).frame == STRIKE_FRAME && !done(w, id) {
        set_done(w, id, true);
        let skip = rand(w, 4) as usize;
        let (x, y, z) = {
            let o = obj(w, id);
            (o.x, o.y, o.z)
        };
        for (i, &dx) in BOMB_LANES.iter().enumerate() {
            if i != skip {
                bomb::spawn(w, (x + BOMB_SPAWN.0, y + BOMB_SPAWN.1, z - 1), dx);
            }
        }
    }
    if anim_done(w, id) {
        go_idle(w, id);
    }
}

// ----------------------------------------------------------------------
// The pirate

/// Starts calling a pirate in: with [`MAX_PIRATES`] of his alive he goes
/// idle instead; otherwise his call plays (he keeps facing Claw).
pub fn enter_thug(w: &mut World, id: ObjectId) {
    if link(w, id).pirates >= MAX_PIRATES {
        go_idle(w, id);
        return;
    }
    set_state(w, id, EnemyState::ThugAttack);
    set_anim(w, id, THUG_ANIM);
    set_done(w, id, false);
}

/// ThugAttack: his call plays, one of four lines on its first tick; when it
/// ends the pirate appears and he goes idle. Draws: one of 0..4 on the
/// first tick; then the pirate's building and his going idle.
pub fn thug_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !done(w, id) {
        set_done(w, id, true);
        let line = THUG_LINES[rand(w, 4) as usize];
        say(w, id, line);
    }
    if anim_done(w, id) {
        summon(w, id);
        if let Some(d) = gd_mut(w, id) {
            d.link.pirates += 1;
        }
        go_idle(w, id);
    }
}

/// The pirate he calls in: made at [`PIRATE_SPAWN`] of him one step in
/// front of him, built at once and sent on his attack. It goes when Claw
/// respawns during the fight, as everything thrown or fired does.
fn summon(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let (x, y, z) = {
        let o = obj(w, id);
        (o.x, o.y, o.z)
    };
    let mut o = new_object(
        redtail_pirate::NAMES[0],
        x + PIRATE_SPAWN.0,
        y + PIRATE_SPAWN.1,
        z - 1,
    );
    o.special_flags |= RESPAWN_PURGE;
    let pirate = w.spawn(o);
    redtail_pirate::construct(w, pirate);
    redtail_pirate::enter_gabriel_attack(w, pirate);
    if let Some(d) = gd_mut(w, id) {
        d.summoned.push(pirate);
        d.summoned_ms.push(now);
    }
}

// ----------------------------------------------------------------------
// The lever

/// Starts the lever: his throw, facing right, one of two lines and the
/// attack's wait. Draws: one of 0..2.
pub fn enter_cannon_attack(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::CannonAttack);
    set_anim(w, id, STRIKE_ANIM);
    set_facing(w, id, true);
    set_done(w, id, false);
    let line = CANNON_LINES[rand(w, 2) as usize];
    say(w, id, line);
    start_delay(w, id);
}

/// CannonAttack: after the wait his throw plays; on its first
/// [`STRIKE_FRAME`] he gives the cannon its fire order; when it ends he
/// faces left and goes idle.
pub fn cannon_attack_tick(w: &mut World, id: ObjectId) {
    if !delay_over(w, id) {
        return;
    }
    w.advance_anim(id);
    if obj(w, id).frame == STRIKE_FRAME && !done(w, id) {
        set_done(w, id, true);
        if let Some(d) = gd_mut(w, id) {
            d.link.fire_order = true;
        }
    }
    if anim_done(w, id) {
        set_facing(w, id, false);
        go_idle(w, id);
    }
}

// ----------------------------------------------------------------------
// Hits

/// `attacker` hit `id`. Once defeated, and in his death acts, nothing
/// reaches him. An enemy shot (a cannonball) is a cannon hit at once,
/// whatever else he does: a [`CANNON_HIT_COOLDOWN_MS`] cooldown, the hit,
/// a hit burst, and the shot is destroyed. Anything else is a blow: the
/// same attack of Claw counts once, and nothing happens during his block,
/// his cannon hit or the cooldown; otherwise a [`BLOCK_COOLDOWN_MS`]
/// cooldown and he blocks, and a pistol shot or magic is destroyed. He
/// never loses health to a hit. Draws: the cannon hit's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if defeated(w, id) {
        return;
    }
    let Some(kind) = w.objects.get(attacker).map(|a| a.type_flags) else {
        return;
    };
    let now = w.now_ms;
    let dying = matches!(
        super::state(w, id),
        Some(
            EnemyState::DeadAct1
                | EnemyState::DeadAct2
                | EnemyState::DeadAct3
                | EnemyState::DeadAct4
        )
    );
    if dying {
        return;
    }
    if kind == types::ENEMY_SHOT {
        if let Some(d) = data_mut(w, id) {
            d.cooldown = Timer::new(now, u64::from(CANNON_HIT_COOLDOWN_MS));
        }
        enter_cannon_hit(w, id);
        hit_burst(w, id, attacker);
        w.destroy(attacker);
        return;
    }
    let serial = w.claw.attack.serial;
    let Some(d) = data_mut(w, id) else { return };
    if d.serial_seen == Some(serial) {
        return;
    }
    d.serial_seen = Some(serial);
    let ready = d.cooldown.done(now);
    let busy = matches!(
        super::state(w, id),
        Some(EnemyState::Blocked | EnemyState::CannonHit)
    );
    if busy || !ready {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        d.cooldown = Timer::new(now, u64::from(BLOCK_COOLDOWN_MS));
    }
    enter_blocked(w, id);
    if kind == attacker::PISTOL_SHOT || kind == attacker::MAGIC {
        w.destroy(attacker);
    }
}

/// `id` attacked `victim`: nothing more than any enemy.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

/// Starts his block: he faces Claw and his block plays.
pub fn enter_blocked(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    set_state(w, id, EnemyState::Blocked);
    set_anim(w, id, BLOCKED_ANIM);
    set_done(w, id, false);
}

/// Blocked: his block plays, one of three lines on its first tick; when it
/// ends he goes idle. Draws: one of 0..3 on the first tick.
pub fn blocked_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !done(w, id) {
        set_done(w, id, true);
        let line = BLOCKED_LINES[rand(w, 3) as usize];
        say(w, id, line);
    }
    if anim_done(w, id) {
        go_idle(w, id);
    }
}

/// Starts a cannon hit: it is counted; the first four play one of his two
/// cries and start the attack's wait (the fifth keeps the last wait, long
/// over). Draws: one of 0..2, not for the fifth.
pub fn enter_cannon_hit(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::CannonHit);
    set_anim(w, id, CANNON_HIT_ANIM);
    let hits = match gd_mut(w, id) {
        Some(d) => {
            d.cannon_hits += 1;
            d.done = false;
            d.cannon_hits
        }
        None => return,
    };
    if hits < CANNON_HITS {
        let line = HIT_LINES[rand(w, 2) as usize];
        play(w, line);
        start_delay(w, id);
    }
}

/// CannonHit: after the wait his hit plays; when it ends he loses
/// [`CANNON_HIT_DAMAGE`]; at the fifth hit his health is 0 and his death
/// acts start, otherwise he answers with his bombs.
pub fn cannon_hit_tick(w: &mut World, id: ObjectId) {
    if !delay_over(w, id) {
        return;
    }
    w.advance_anim(id);
    if !anim_done(w, id) || done(w, id) {
        return;
    }
    set_done(w, id, true);
    obj_mut(w, id).health -= CANNON_HIT_DAMAGE;
    let hits = gabriel_data(w, id).map_or(0, |d| d.cannon_hits);
    if hits >= CANNON_HITS {
        obj_mut(w, id).health = 0;
        enter_dead_act1(w, id);
    } else {
        enter_multibomb(w, id);
    }
}

// ----------------------------------------------------------------------
// After a throw

/// Strike: his strike animation is never loaded, so the strike he picks
/// after getting up from a throw or a drop never ends: he stays in it
/// until a blow, a lift or a respawn takes him out.
pub fn strike_tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// Lifted: the shared lift, keeping the picture he showed when he was
/// lifted (he has no lift animation).
pub fn lifted_tick(w: &mut World, id: ObjectId) {
    let frame = obj(w, id).frame;
    lift::lifted_tick(w, id);
    obj_mut(w, id).frame = frame;
}

// ----------------------------------------------------------------------
// Death acts

/// Starts his first death act: he faces Claw, hurts nobody and nothing
/// hits him.
pub fn enter_dead_act1(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::DeadAct1);
    set_anim(w, id, DEATH_ANIMS[0]);
    face_claw(w, id);
    obj_mut(w, id).flags |= flags::HARMLESS | flags::SAFE;
    set_done(w, id, false);
}

/// DeadAct1: his fall to his knees loops; once no line of his plays he
/// says his first death line and the act lasts [`DEAD_ACT1_MS`] from
/// then; while its first picture shows he turns round every
/// [`TURN_MS`].
pub fn dead_act1_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    if !done(w, id) {
        if sound::enemy_is_speaking(w, id) {
            return;
        }
        say(w, id, DEATH_LINES[0]);
        if let Some(d) = gd_mut(w, id) {
            d.done = true;
            d.act_timer = Timer::new(now, u64::from(DEAD_ACT1_MS));
            d.turn_timer = Timer::new(now, u64::from(TURN_MS));
        }
        return;
    }
    if obj(w, id).frame == TURN_FRAME && gabriel_data(w, id).is_some_and(|d| d.turn_timer.done(now))
    {
        let right = super::faces_right(obj(w, id));
        set_facing(w, id, !right);
        if let Some(d) = gd_mut(w, id) {
            d.turn_timer = Timer::new(now, u64::from(TURN_MS));
        }
    }
    if gabriel_data(w, id).is_some_and(|d| d.act_timer.done(now)) {
        enter_dead_act2(w, id);
    }
}

/// Starts his second death act: he faces right and leaps on the lowest
/// arc towards [`DEATH_LEAP`] of his first place.
pub fn enter_dead_act2(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::DeadAct2);
    set_anim(w, id, DEATH_ANIMS[1]);
    set_facing(w, id, true);
    let start = data(w, id)
        .and_then(|d| d.human.boss)
        .map_or((0, 0), |b| b.start);
    let target = (start.0 + DEATH_LEAP.0, BOMB_FLOOR_Y + DEATH_LEAP.1);
    let (x, y) = {
        let o = obj_mut(w, id);
        o.physics_type = ptype::FLY;
        (o.x, o.y)
    };
    let fy = gabriel_data(w, id).map_or(0.0, |d| d.stand_y);
    if let Some(d) = data_mut(w, id) {
        d.ay = GRAVITY;
        // He leaves from where he stands exactly: the part of a pixel he
        // stands below his own.
        d.carry = (0.0, fy);
        d.moved_to = Some((x, y));
        let (vx, vy) = human::launch(
            f64::from(target.0 - x),
            f64::from(target.1) - (f64::from(y) + fy),
            GRAVITY,
        );
        d.vx = vx;
        d.vy = vy;
    }
}

/// DeadAct2: he flies; once he stands again he stops and the third act
/// starts.
pub fn dead_act2_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).physics_type == ptype::STAND {
        if let Some(d) = data_mut(w, id) {
            d.vx = 0.0;
            d.vy = 0.0;
        }
        set_state(w, id, EnemyState::DeadAct3);
        set_anim(w, id, DEATH_ANIMS[2]);
    }
}

/// DeadAct3: its animation plays; when it ends the last act starts.
pub fn dead_act3_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if anim_done(w, id) {
        set_state(w, id, EnemyState::DeadAct4);
        set_anim(w, id, DEATH_ANIMS[3]);
        set_done(w, id, false);
        if let Some(d) = gd_mut(w, id) {
            d.ended = false;
        }
    }
}

/// DeadAct4: once no line of his plays he says his last line; its
/// animation plays, and on the tick it ends (checked once) with Claw
/// alive Claw is held still, the gem rises out of him and he is defeated.
pub fn dead_act4_tick(w: &mut World, id: ObjectId) {
    if !done(w, id) {
        if sound::enemy_is_speaking(w, id) {
            return;
        }
        say(w, id, DEATH_LINES[1]);
        set_done(w, id, true);
    }
    w.advance_anim(id);
    let ended = gabriel_data(w, id).is_some_and(|d| d.ended);
    if ended || !anim_done(w, id) {
        return;
    }
    if let Some(d) = gd_mut(w, id) {
        d.ended = true;
    }
    if w.claw.stats.health > 0 {
        w.boss_defeated(id);
        if let Some(d) = gd_mut(w, id) {
            d.link.defeated = true;
        }
        with_boss(w, id, |b| {
            b.death_seen = true;
            b.defeat_reported = true;
        });
    }
}

/// The pirates Gabriel calls in, in his fight: what the pirate's own rules
/// call when Gabriel is there ([`redtail_pirate::in_gabriel_fight`]).
///
/// - Built while he fights, a pirate takes his fight's range boxes
///   ([`redtail_pirate::GABRIEL_RANGES`] for the first two) and stays awake
///   everywhere ([`take_mode`]).
/// - Sent on his attack ([`enter_attack`]), it runs left along his deck at
///   [`redtail_pirate::GABRIEL_ATTACK_SPEED`] with no patrol limits, busy,
///   as long as its exact x is greater than [`GabrielLink::run_to`]; then
///   it jumps on the lowest arc towards [`GabrielLink::leap_to`]
///   ([`attack_tick`]) from its exact place (the part of a pixel its run
///   left across, and below its pixel the part it stands sunk since it
///   was made, see the module's Standing), flies past that point and
///   lands on the dock, and from there fights as any pirate.
/// - A pirate he called in keeps his fight's mode for good: his reset on
///   Claw's respawn clears his list of pirates, but one leaping then goes
///   on with its leap, without the shared steps and their roll.
/// - Once he is defeated a pirate dies at once, at its next call
///   ([`mode_step`]).
///
/// He keeps the count of his live pirates himself: each of his calls, the
/// pirates he called in that died or are gone since his last call lower
/// it.
pub mod pirates {
    use super::super::{
        EnemyState, GRAVITY, UNBOUNDED, data, data_mut, human, motion, obj, set_anim, set_state,
        states, wolvington,
    };
    use crate::logic::World;
    use crate::logic::enemy::{DeathKind, die, redtail_pirate};
    use crate::object::{ObjectId, flags};
    use crate::tuning::TICK_MS;

    /// A pirate built while Gabriel fights takes his fight's range boxes
    /// and stays awake everywhere.
    pub fn take_mode(w: &mut World, id: ObjectId) {
        if let Some(d) = data_mut(w, id) {
            d.ranges[0] = redtail_pirate::GABRIEL_RANGES[0];
            d.ranges[1] = redtail_pirate::GABRIEL_RANGES[1];
        }
        if let Some(o) = w.objects.get_mut(id) {
            o.flags |= flags::ALWAYS_ACTIVE;
        }
    }

    /// The fight's step at the start of a pirate's call: with Gabriel
    /// defeated, or gone while the pirate still runs his attack, a pirate
    /// still alive dies at once (true: the call ends there). While it runs his attack, and in the leap that follows, the
    /// call is only that state's step and the movement step, without the
    /// shared steps (no roll, no reaction to Claw): true. Otherwise
    /// nothing (false).
    pub fn mode_step(w: &mut World, id: ObjectId) -> bool {
        let Some(state) = w.objects.get(id).map(|o| EnemyState::from_id(o.state)) else {
            return false;
        };
        let Some(state) = state else {
            return false;
        };
        let link = w.gabriel_link();
        let defeated = link.is_some_and(|l| l.defeated);
        // A pirate still running his attack when he is gone (drowned or
        // removed) dies as with him defeated. Default taken: the original
        // was not measured here; without it the pirate would run on
        // without limits.
        let orphaned = link.is_none() && state == EnemyState::GabrielAttack;
        let alive = data(w, id).is_some_and(|d| !d.dead);
        if (defeated || orphaned) && alive {
            die(w, id, DeathKind::Hit, None);
            return true;
        }
        match state {
            EnemyState::GabrielAttack => attack_tick(w, id),
            EnemyState::Jump if leaping(w, id) => states::jump_tick(w, id),
            _ => return false,
        }
        motion::integrate(w, id);
        true
    }

    /// How far below its whole pixel a pirate he called in stands, px: it
    /// was set down on his deck in the tick it was made, so it stood
    /// [`wolvington::LANDED_FRACTION`] below its pixel then and sinks on
    /// each tick since ([`wolvington::next_stand_fraction`]).
    fn stand_fraction(w: &World, id: ObjectId) -> f64 {
        let Some(ticks) = ticks_since_made(w, id) else {
            return 0.0;
        };
        let ay = data(w, id).map_or(GRAVITY, |d| d.ay);
        let mut f = wolvington::LANDED_FRACTION;
        for _ in 1..ticks {
            f = wolvington::next_stand_fraction(f, ay);
        }
        f
    }

    /// Whole ticks since Gabriel called pirate `id` in, while it is in his
    /// list.
    fn ticks_since_made(w: &World, id: ObjectId) -> Option<u64> {
        let made = w.current_boss().and_then(|b| {
            let d = super::gabriel_data(w, b)?;
            let i = d.summoned.iter().position(|&p| p == id)?;
            d.summoned_ms.get(i).copied()
        })?;
        Some(w.now_ms.saturating_sub(made) / u64::from(TICK_MS))
    }

    /// True until pirate `id` has made its first movement step: its exact
    /// height is then still where it was made, below the pixel its
    /// building set on the floor.
    fn first_call(w: &World, id: ObjectId) -> bool {
        data(w, id).is_some_and(|d| !d.stepped)
    }

    /// True while pirate `id` is in the leap Gabriel's attack ends in: one
    /// he called in, jumping, with him there. His list does not decide it:
    /// his reset on Claw's respawn clears the list, and a pirate leaping
    /// then keeps its leap (and makes no per-pulse roll) all the same.
    fn leaping(w: &World, id: ObjectId) -> bool {
        let Some(boss) = w.current_boss() else {
            return false;
        };
        (redtail_pirate::called_in(w, id)
            || super::gabriel_data(w, boss).is_some_and(|d| d.summoned.contains(&id)))
            && w.objects
                .get(id)
                .is_some_and(|o| o.state == EnemyState::Jump as i32)
    }

    /// Sends pirate `id` on Gabriel's attack: running left along his deck,
    /// busy, with no patrol limits.
    pub fn enter_attack(w: &mut World, id: ObjectId) {
        set_state(w, id, EnemyState::GabrielAttack);
        set_anim(w, id, redtail_pirate::CLASS.anims.fast_advance);
        if let Some(d) = data_mut(w, id) {
            d.bounds = UNBOUNDED;
            d.bounds_y = UNBOUNDED;
            d.busy = true;
            d.vx = -f64::from(redtail_pirate::GABRIEL_ATTACK_SPEED);
        }
    }

    /// GabrielAttack: it runs on while its x is greater than
    /// [`super::GabrielLink::run_to`]; then it jumps on the lowest arc
    /// towards [`super::GabrielLink::leap_to`]. It leaves from its exact
    /// position: after a run, the part of a pixel it stands below its own
    /// ([`stand_fraction`]); at its first call, as when Gabriel stands
    /// down on the dock and makes it past the end of its run already, the
    /// height it was made at ([`super::PIRATE_SPAWN`] of him), which its
    /// building keeps when it sets its pixel on the floor above it.
    pub fn attack_tick(w: &mut World, id: ObjectId) {
        w.advance_anim(id);
        let Some(link) = w.gabriel_link().cloned() else {
            return;
        };
        let (x, y) = {
            let o = obj(w, id);
            (o.x, o.y)
        };
        let run_x = f64::from(x) + data(w, id).map_or(0.0, |d| d.carry.0);
        if run_x > f64::from(link.run_to) {
            return;
        }
        // A leap at its first call leaves from the height it was made at,
        // which it still carries below its pixel; after a run it leaves
        // from the part of a pixel it stands below its own.
        let fy = (!first_call(w, id)).then(|| stand_fraction(w, id));
        human::enter_jump(w, id);
        if let Some(d) = data_mut(w, id) {
            d.bounds = UNBOUNDED;
            d.bounds_y = UNBOUNDED;
            // Where it leaves from, exactly: the part of a pixel it ran
            // with, and the part of a pixel below its own it leaves from.
            if let Some(fy) = fy {
                d.carry.1 = fy;
            }
            d.moved_to = Some((x, y));
            let from = (f64::from(x) + d.carry.0, f64::from(y) + d.carry.1);
            let (vx, vy) = human::launch(
                f64::from(link.leap_to.0) - from.0,
                f64::from(link.leap_to.1) - from.1,
                d.ay,
            );
            d.vx = vx;
            d.vy = vy;
        }
    }
}

/// The bombs Gabriel lobs: each flies on the lowest arc that comes back to
/// its starting height [`BOMB_LANES`] px to his left, under a gravity of
/// [`bomb::GRAVITY`], and explodes on the first tick at or below the
/// dock's height [`BOMB_FLOOR_Y`] into a blast of [`bomb::EXPLODE_TICKS`]
/// ticks. In flight and in the blast it costs Claw [`bomb::DAMAGE`]
/// (Claw's hit removes it in flight); the blast also hurts the pirates he
/// calls in. It has no fuse and never bounces: one that lands above the
/// dock lies there unexploded. It is always active, so it never leaves
/// with its activation window. All of them are removed when Gabriel is
/// defeated, and when Claw respawns during the fight.
///
/// It moves as the projectiles do (each step covers the game time since its
/// last on its own clock, [`crate::logic::projectile::Motion`]): it waits
/// in the tick it is thrown and moves from the next. Each tick, in order:
/// on its first tick its whistle (a draw of 0..3); with Gabriel there and
/// defeated it goes; with him there its fall animation plays and, at or
/// below his bombs' height, it explodes (a draw of 0..3 for the sound),
/// hittable by nothing, the level's own kind of shot, standing still; then
/// its motion step.
pub mod bomb {
    use claw_frame::SoundChannel;

    use crate::level::runtime_image_set;
    use crate::logic::projectile::{Motion, RESPAWN_PURGE};
    use crate::logic::{ObjectData, World, new_object, sound};
    use crate::object::{ObjectId, Rect, flags, special, types};
    use crate::physics::{ptype, res};
    use crate::tuning::TICK_MS;

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
    /// Its whistle, on its first tick (the same for the three draws).
    pub const WHISTLE_SOUNDS: [&str; 3] = [
        "LEVEL_GABRIELBOMB_BOMBWHISTLE1",
        "LEVEL_GABRIELBOMB_BOMBWHISTLE1",
        "LEVEL_GABRIELBOMB_BOMBWHISTLE1",
    ];
    /// Its explosion, one at random.
    pub const EXPLOSION_SOUNDS: [&str; 3] = [
        "LEVEL_GABRIELBOMB_SEXPLOSION1",
        "LEVEL_GABRIELBOMB_SEXPLOSION2",
        "LEVEL_GABRIELBOMB_SEXPLOSION3",
    ];

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = true;

    /// Damage of the bomb and of its blast (no difficulty factor).
    pub const DAMAGE: i32 = 5;
    /// The bomb's box in flight and the blast's.
    pub const BLAST: Rect = Rect::new(-50, -50, 50, 50);
    /// Gravity on a bomb, px/s^2.
    pub const GRAVITY: f64 = 2000.0;
    /// Ticks of the blast at the 18 ms step.
    pub const EXPLODE_TICKS: u32 = 16;
    /// Its movement box when the level does not know its first picture.
    pub const MOVE_RECT: Rect = Rect::new(-12, -12, 12, 12);

    /// The bomb's states.
    pub mod state {
        /// Flying.
        pub const FALLING: i32 = 1;
        /// Exploding.
        pub const EXPLODING: i32 = 2;
    }

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
    pub struct BombData {
        /// Exact position, velocity and gravity.
        pub motion: Motion,
        /// Its own clock, s (see the projectiles' motion step).
        pub clock_s: Option<f64>,
        /// Its whistle has played.
        pub whistled: bool,
        /// No motion: exploding.
        pub frozen: bool,
    }

    fn take(w: &mut World, id: ObjectId) -> Option<BombData> {
        let o = w.objects.get_mut(id)?;
        match std::mem::take(&mut o.data) {
            ObjectData::GabrielBomb(d) => Some(d),
            other => {
                o.data = other;
                None
            }
        }
    }

    fn put(w: &mut World, id: ObjectId, d: BombData) {
        if let Some(o) = w.objects.get_mut(id) {
            o.data = ObjectData::GabrielBomb(d);
        }
    }

    /// Makes a bomb at (x, y, z) and throws it at the point `dx` px to the
    /// side at its own height. It is an enemy shot hittable by nothing and
    /// always active: the leave pass never removes it for leaving its
    /// window, so it flies its whole arc wherever Claw is (dead far below
    /// it, say); only its far-outside removal and Claw's respawn remove it.
    pub fn spawn(w: &mut World, (x, y, z): (i32, i32, i32), dx: i32) -> ObjectId {
        let mut o = new_object(NAMES[0], x, y, z);
        o.image_set = runtime_image_set(IMAGE_SET);
        o.frame = w.image_frames(IMAGE_SET).map_or(1, |(first, _)| first);
        o.move_rect = w.frame_box(IMAGE_SET, o.frame).unwrap_or(MOVE_RECT);
        o.attack_rect = BLAST;
        o.type_flags = types::ENEMY_SHOT;
        o.hit_type_flags = 0;
        o.damage = DAMAGE;
        o.flags |= flags::ALWAYS_ACTIVE | flags::DESTROY_OUTSIDE | flags::DESTROY_FAR;
        o.special_flags |= RESPAWN_PURGE | special::OWN_IMPACT;
        o.physics_type = ptype::FLY;
        o.state = state::FALLING;
        let (vx, vy) = launch_velocity(dx);
        o.speed_x = vx as i32;
        o.speed_y = vy as i32;
        o.data = ObjectData::GabrielBomb(BombData {
            motion: Motion {
                x: f64::from(x),
                y: f64::from(y),
                vx,
                vy,
                ay: GRAVITY,
            },
            ..BombData::default()
        });
        let id = w.spawn(o);
        w.set_anim(id, FALL_ANIM);
        id
    }

    /// One logic call.
    pub fn tick(w: &mut World, id: ObjectId) {
        let Some(state) = w.objects.get(id).map(|o| o.state) else {
            return;
        };
        let Some(mut d) = take(w, id) else {
            return;
        };
        match state {
            state::FALLING => falling(w, id, &mut d),
            state::EXPLODING => exploding(w, id),
            _ => {}
        }
        put(w, id, d);
    }

    fn falling(w: &mut World, id: ObjectId, d: &mut BombData) {
        if !d.whistled {
            d.whistled = true;
            let key = WHISTLE_SOUNDS[w.rng.below(3) as usize];
            sound::play(w, key, SoundChannel::Effect);
        }
        if let Some(link) = w.gabriel_link().cloned() {
            if link.defeated {
                if let Some(o) = w.objects.get_mut(id) {
                    o.flags |= flags::HARMLESS | flags::DESTROY;
                }
                return;
            }
            w.advance_anim(id);
            if w.objects.get(id).is_some_and(|o| o.y >= link.floor_y) {
                d.frozen = true;
                explode(w, id);
            }
        }
        integrate(w, id, d);
    }

    /// A bomb in flight that strikes Claw explodes where it is, still in
    /// the same collision pass (see [`crate::logic::projectile::impact`]):
    /// it stops there and its blast plays as on the dock. Draws: the
    /// explosion's one of 0..3.
    pub fn impact(w: &mut World, id: ObjectId) {
        if w.objects.get(id).is_none_or(|o| o.state != state::FALLING) {
            return;
        }
        let Some(mut d) = take(w, id) else {
            return;
        };
        d.frozen = true;
        put(w, id, d);
        explode(w, id);
    }

    /// Explodes: hittable by nothing, the level's own kind of shot, its
    /// blast animation, and one of its three sounds. Draws: one of 0..3.
    fn explode(w: &mut World, id: ObjectId) {
        if let Some(o) = w.objects.get_mut(id) {
            o.state = state::EXPLODING;
            o.flags |= flags::SAFE;
            o.type_flags = types::LEVEL_SHOT;
        }
        w.set_anim(id, EXPLODE_ANIM);
        let key = EXPLOSION_SOUNDS[w.rng.below(3) as usize];
        sound::play(w, key, SoundChannel::Effect);
    }

    fn exploding(w: &mut World, id: ObjectId) {
        if w.gabriel_link().is_some_and(|l| l.defeated) {
            w.destroy(id);
            return;
        }
        w.advance_anim(id);
        if w.objects.get(id).is_none_or(|o| o.anim.is_done()) {
            w.destroy(id);
        }
    }

    /// The time the motion step of the tick covers, s: one tick at first,
    /// then the game time since the bomb's own clock.
    fn step_seconds(d: &mut BombData, now_ms: u64) -> f64 {
        let now = (now_ms + u64::from(TICK_MS)) as f64 * 0.001;
        match d.clock_s {
            Some(prev) => {
                let dt = now - prev;
                d.clock_s = Some(prev + dt);
                dt
            }
            None => {
                d.clock_s = Some(now);
                f64::from(TICK_MS) / 1000.0
            }
        }
    }

    /// One motion step through the tiles (not the platforms), unless it is
    /// frozen; a landing stops it where it lies.
    fn integrate(w: &mut World, id: ObjectId, d: &mut BombData) {
        if d.frozen {
            return;
        }
        if w.objects
            .get(id)
            .is_some_and(|o| o.physics_type == ptype::STAND)
        {
            d.frozen = true;
            return;
        }
        let t = step_seconds(d, w.now_ms);
        d.motion.step(t);
        let (tx, ty) = (d.motion.x as i32, d.motion.y as i32);
        let r = w.physics(id, tx, ty, 0);
        let Some(o) = w.objects.get_mut(id) else {
            return;
        };
        if o.x != tx {
            d.motion.x = f64::from(o.x);
        }
        if o.y != ty {
            d.motion.snap_y(f64::from(o.y));
        }
        if r & res::CEILING != 0 {
            d.motion.vy = -d.motion.vy;
        } else if r & (res::RIGHT | res::LEFT) != 0 {
            d.motion.vx = 0.0;
        }
        o.speed_x = d.motion.vx as i32;
        o.speed_y = d.motion.vy as i32;
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
        assert_eq!(u64::from(LIFT_PROTECTION_MS), lift::LIFT_GUARD_MS);
        assert_eq!(DEATH_ACT_TICKS.iter().sum::<u32>(), 671);
        // The first death act: its line on its first tick, then 4000 ms.
        assert_eq!(1 + DEAD_ACT1_MS.div_ceil(18), DEATH_ACT_TICKS[0]);
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
            // The same as any enemy's lowest arc.
            let (lx, ly) = human::launch(f64::from(dx), 0.0, bomb::GRAVITY);
            assert!((lx - vx).abs() < 1e-9 && (ly - vy).abs() < 1e-9, "{dx}");
        }
        assert_eq!(bomb::launch_velocity(0), (0.0, -0.0));
    }

    #[test]
    fn his_bombs_miss_him_by_one_pixel() {
        // Facing left, his hit box starts 24 px left of him; a bomb's box
        // ends 25 px left of him where it leaves.
        let bomb_right = BOMB_SPAWN.0 + bomb::BLAST.right;
        assert_eq!(bomb_right, BOX.left - 1);
        // Facing right he would be hit.
        assert!(bomb_right >= BOX.mirrored().left);
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
        for line in BOMB_LINES
            .iter()
            .chain(&THUG_LINES)
            .chain(&CANNON_LINES)
            .chain(&BLOCKED_LINES)
            .chain(&HIT_LINES)
            .chain(&DEATH_LINES)
        {
            assert!(line.starts_with(ANIM_PREFIXES[0]), "{line}");
        }
    }

    #[test]
    fn his_link_points_his_pirates_and_bombs_at_the_dock() {
        let l = fresh_link(43223);
        assert!(l.wait_for_cannon);
        assert_eq!(l.run_to, 43213);
        assert_eq!(l.leap_to, (43063, 4834));
        assert_eq!(l.floor_y, 5234);
        assert!(!l.fire_order && !l.button && !l.defeated);
        assert_eq!(l.pirates, 0);
        // His own fall aims 100 px above the dock at the pirates' x.
        assert_eq!(
            (43223 + DEATH_LEAP.0, BOMB_FLOOR_Y + DEATH_LEAP.1),
            (43063, 5134)
        );
    }

    #[test]
    fn what_hits_him() {
        assert_eq!(HIT_BY, 0x300_0062);
        assert_eq!(HIT_BY & types::THROWN, 0, "thrown objects pass through");
        assert_eq!(HIT_BY & types::ENEMY, 0, "enemies never hit him");
    }

    #[test]
    fn his_states_map_to_the_reference_numbers() {
        assert_eq!(reference_state_id(EnemyState::Startup), Some(2028));
        assert_eq!(reference_state_id(EnemyState::Idle), Some(2000));
        assert_eq!(reference_state_id(EnemyState::MultiBomb), Some(2082));
        assert_eq!(reference_state_id(EnemyState::DeadAct4), Some(2090));
        assert_eq!(reference_state_id(EnemyState::Strike), Some(2003));
        // Each reference number stands for one state.
        let mut ids: Vec<i32> = REFERENCE_STATE_IDS.iter().map(|&(_, n)| n).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), REFERENCE_STATE_IDS.len());
    }

    #[test]
    fn he_has_his_own_handlers_for_his_own_states() {
        for s in
            STATES
                .into_iter()
                .chain([EnemyState::Startup, EnemyState::Idle, EnemyState::Strike])
        {
            assert!(handler(s).is_some(), "{s:?}");
        }
        // The shared human lift, throw and pause.
        for s in [
            EnemyState::Flailing,
            EnemyState::Thrown,
            EnemyState::Bounce,
            EnemyState::Pause,
            EnemyState::Home,
        ] {
            assert!(handler(s).is_none(), "{s:?}");
        }
    }
}
