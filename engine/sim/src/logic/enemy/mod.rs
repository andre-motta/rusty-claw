//! Enemies: the officer, the soldier, the rat, the punk rat ([`punkrat`])
//! and Raux ([`raux`]), on shared enemy behaviour and the shared human
//! states ([`human`]), with the cutthroat ([`cutthroat`]) and the robber
//! thief ([`robberthief`]), the seagull ([`seagull`]), the first flyer,
//! Katherine ([`katherine`]), the boss of the fourth level, the town guards
//! ([`townguard`]), Wolvington ([`wolvington`]), the red-tail pirates
//! ([`redtail_pirate`]), the bear sailors ([`bear_sailor`]), the crabs
//! ([`crab`]), Gabriel with his bomb ([`gabriel`]), the hook-handed
//! pirate ([`hook_pirate`]), PegLeg with his bullet ([`pegleg`]),
//! Marrow ([`marrow`]) with his parrot ([`parrot`]), the eleventh
//! level's Mercat ([`mercat`]), Siren ([`siren`]) and Fish ([`fish`]),
//! the twelfth level's Aquatis ([`aquatis`]) with his helpers and the
//! tentacles ([`tentacle`]), the thirteenth level's Chameleon
//! ([`chameleon`]) and RedTail ([`redtail`]) with his stakes and his wind,
//! and the fourteenth level's tiger guards ([`tiger_guard`]) and Omar
//! ([`omar`]) with his shield and his energy balls.
//!
//! Every enemy is a small state machine over named states ([`EnemyState`]).
//! Each class handles a few states itself and inherits every other one from
//! the shared behaviour: the officer and the soldier are human enemies
//! (they pace, advance, flinch, can be lifted and drop loot), the rat is a
//! plain enemy (it walks, throws bombs and dies at the first blow).
//!
//! Each logic call after the first runs, in order:
//!
//! 1. the pre-tick ([`pre_tick`]): on a frame pulse tick (every third
//!    tick), one draw of 0..=1000 from the random stream (kept so the
//!    stream stays in step with the game; nothing reads it), which side Claw is on and, unless Claw is invisible, the
//!    range mode ([`range_mode`]); human enemies then refresh their
//!    liftable flag and contact box, notice Claw leaving range, become
//!    stationary on a platform and check for a narrow ledge; each class may
//!    add steps after these;
//! 2. the handler of the current state: the class's own, or the shared one
//!    ([`states`]);
//! 3. the post-tick: a scheduled call (going idle after Claw left range,
//!    or the jump at Claw from a narrow ledge), then the movement step and
//!    what follows it ([`motion`]): death tiles, the ledge guard and the
//!    depth against Claw.
//!
//! The first call builds the enemy (its class's `construct`). Hits arrive
//! from the collision pass through the class's `on_hit`.
//!
//! An enemy's data is [`EnemyData`], held as [`ObjectData::Enemy`], or
//! inside a family's own data variant that wraps it (the punk rat's and
//! Raux's): [`data`], [`data_mut`], [`data_of`] and [`with_data`] reach it
//! either way.
//!
//! Random draws happen in the game's order, from the shared stream the
//! whole level draws from; every function that draws says which draws it
//! makes, in order.
//!
//! Two things are shared by every enemy of the level: the strike token (one
//! armed strike at a time across the level, see [`take_strike_token`]) and
//! the enemies' speaking slot (one line at a time, see
//! [`crate::logic::sound::enemy_say`]).

use claw_frame::SoundChannel;

use crate::anim::AnimPlayer;
use crate::logic::{GameLogic, ObjectData, World, pickup, sound};
use crate::object::{LogicKind, Object, ObjectId, Rect, draw_flags, engine_state, flags, types};
use crate::physics::ptype;
use crate::tiles::SOLID;
use crate::tuning::TICK_MS;

pub mod aquatis;
pub mod bear_sailor;
pub mod chameleon;
pub mod crab;
pub mod cutthroat;
pub mod fish;
pub mod gabriel;
pub mod goo;
pub mod hook_pirate;
pub mod human;
pub mod katherine;
pub mod lift;
pub mod marrow;
pub mod mercat;
pub mod motion;
pub mod officer;
pub mod omar;
pub mod parrot;
pub mod pegleg;
pub mod punkrat;
pub mod rat;
pub mod raux;
pub mod redtail;
pub mod redtail_pirate;
pub mod robberthief;
pub mod seagull;
pub mod siren;
pub mod soldier;
pub mod states;
pub mod tentacle;
pub mod tiger_guard;
pub mod townguard;
pub mod wolvington;

pub use states::{enter_hit, enter_pause, enter_strike, go_home, human_home_tick, start_pacing};

/// Image sets every enemy may switch to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations every enemy may play: the deaths by fire
/// and by ice, chosen by what dealt the last blow.
pub const ANIM_PREFIXES: &[&str] = &["GAME_FIREDEATH", "GAME_ICEDEATH"];

/// Damage factor of the difficulty: enemies' damage is multiplied by it
/// when they are built. 1.0 in normal play.
pub const DAMAGE_FACTOR: f32 = 1.0;
/// Health factor of the difficulty: enemies' health is multiplied by it
/// when they are built. 1.0 in normal play.
pub const HEALTH_FACTOR: f32 = 1.0;
/// Smarts factor of the difficulty, applied when enemies are built. 1.0 in
/// normal play.
pub const SMARTS_FACTOR: f32 = 1.0;

/// `value` times a difficulty factor, truncated.
pub fn scaled(value: i32, factor: f32) -> i32 {
    (value as f32 * factor) as i32
}

/// An enemy's smarts from its class base: the base times the smarts
/// factor, truncated, with values below 100 raised to 99.
pub fn smarts(base: i32) -> i32 {
    scaled(base, SMARTS_FACTOR).max(99)
}

/// Kinds of object (type flags) that can hit a human enemy: Claw, his
/// thrown objects, his pistol shots, springboard-like objects, the level's
/// own shots and explosions, and his magic.
pub const HUMAN_HIT_BY: u32 = 0x300_1032;

/// What a hit came from, by the attacker's type flags.
pub mod attacker {
    use crate::object::types;

    /// Claw himself: his sword, punches and kicks.
    pub const CLAW: u32 = types::PLAYER;
    /// An object Claw threw.
    pub const THROWN: u32 = types::THROWN;
    /// A pistol shot (and a stick of dynamite).
    pub const PISTOL_SHOT: u32 = types::PISTOL_SHOT;
    /// A springboard-like object.
    pub const SPRING: u32 = 0x1000;
    /// A shot of the level itself, and explosions.
    pub const LEVEL_SHOT: u32 = types::LEVEL_SHOT;
    /// Claw's magic.
    pub const MAGIC: u32 = 0x200_0000;
}

/// Special flags of a blow that kill with fire.
pub const FIRE_BLOW: u32 = 0x4_0000 | 0x10_0000;
/// Special flag of a blow that kills with ice.
pub const ICE_BLOW: u32 = 0x8_0000;
/// Death animation of a kill by fire.
pub const FIRE_DEATH_ANIM: &str = "GAME_FIREDEATH";
/// Death animation of a kill by ice.
pub const ICE_DEATH_ANIM: &str = "GAME_ICEDEATH";

/// The powerup code that hides Claw from enemies.
pub const INVISIBILITY_POWERUP: u32 = 0x26B7;

/// Time Claw must stay out of range, on screen, before the mode says so,
/// ms.
pub const OUT_OF_RANGE_MS: u64 = 500;

/// Gravity of enemies, px/s^2.
pub const GRAVITY: f64 = 2000.0;

/// Length of one tick, s.
pub const TICK_S: f64 = TICK_MS as f64 * 0.001;

/// Unbounded patrol limits.
pub const UNBOUNDED: (i32, i32) = (i32::MIN, i32::MAX);

/// Indices of the level record fields enemies read.
pub mod field {
    /// Powerup: the last loot slot.
    pub const POWERUP: usize = 15;
    /// First user box: left, top, right, bottom (loot slots).
    pub const USER_RECT_1: [usize; 4] = [35, 36, 37, 38];
    /// Second user box: left, top, right, bottom (loot slots).
    pub const USER_RECT_2: [usize; 4] = [39, 40, 41, 42];
    /// First user value: above 0, the enemy never walks.
    pub const USER_0: usize = 43;
    /// Patrol limits.
    pub const X_MIN: usize = 51;
    /// Patrol limits.
    pub const Y_MIN: usize = 52;
    /// Patrol limits.
    pub const X_MAX: usize = 53;
    /// Patrol limits.
    pub const Y_MAX: usize = 54;
    /// Horizontal speed.
    pub const SPEED_X: usize = 55;
}

/// An enemy's named states. The numbers are fixed: traces show them.
///
/// Each state name has one number, whichever class runs it: a class that
/// runs a state of an existing name reuses its number. New names take
/// numbers in blocks: 2000-2021 the first level's enemies, 2022-2029 every
/// boss, 2030-2049 the shared human states, 2050-2059 the punk rat,
/// 2060-2069 cannons (with the turn of the eighth level's deck cannon),
/// 2070-2089 Raux and 2100-2119 the third level's
/// thieves (reserved, no state yet: see [`cutthroat::STATE_BLOCK`]),
/// 2120-2139 the fourth level's (Katherine's somersault, see
/// [`katherine::STATE_BLOCK`]) and 2140-2159 the fifth level's (the town
/// guards' second idle and parry and the seagull's climb back and flinch,
/// see [`townguard::STATE_BLOCK`]), 2160-2179 the sixth level's
/// (Wolvington's five attacks, see [`wolvington::STATE_BLOCK`]) and
/// 2180-2199 the seventh level's (the bear's hug and the pirate's boss
/// mode from 2180, the crabs' states from 2190, see
/// [`bear_sailor::STATE_BLOCK`]) and 2200-2219 the eighth level's
/// (Gabriel's attacks, hits and death acts, see [`gabriel::STATE_BLOCK`])
/// and 2220-2239 the ninth level's (PegLeg's rolls and the hook hold, see
/// [`hook_pirate::STATE_BLOCK`]) and 2240-2259 the tenth level's
/// (Marrow's whistle and the parrot's flights, see
/// [`marrow::STATE_BLOCK`]) and 2260-2279 the eleventh level's (the
/// Fish's swim, leap and flop and the Siren's song from 2270, see
/// [`mercat::STATE_BLOCK`]) and 2280-2299 the twelfth level's (Aquatis's
/// punches, frenzy, stalactite hit and fright from 2280, the tentacles'
/// grab, squeeze and regrowth from 2290, see [`aquatis::STATE_BLOCK`])
/// and 2300-2319 the thirteenth level's (RedTail's attacks and jumps from
/// 2300, the Chameleon's fade out and back from 2310, see
/// [`redtail::STATE_BLOCK`]) and 2320-2339 the fourteenth level's (the
/// tiger guards' lunge and flips from 2320, Omar's cast, move and wait
/// from 2330, see [`tiger_guard::STATE_BLOCK`]).
/// Numbers of a block that no state has yet stay free, so the
/// numbers have gaps ([`EnemyState::from_id`] knows them).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum EnemyState {
    /// Waiting where it stands.
    #[default]
    Idle = 2000,
    /// Deciding what to do next.
    Home = 2001,
    /// Swinging at Claw.
    Strike = 2002,
    /// Walking up and down its patrol.
    Pace = 2003,
    /// Flinching from a low blow.
    HitLow = 2004,
    /// Flinching from a high blow.
    HitHigh = 2005,
    /// Dead.
    Dead = 2006,
    /// Waiting before going back to the previous state.
    Pause = 2007,
    /// Falling until it lands.
    Falling = 2008,
    /// Walking toward Claw.
    Advance = 2009,
    /// Running toward Claw.
    FastAdvance = 2010,
    /// Leaping at Claw from a narrow ledge.
    JumpAtClaw = 2011,
    /// Thrown into the air by a springboard.
    Jump = 2012,
    /// Firing a shot.
    Fire = 2013,
    /// Walking left.
    MoveLeft = 2014,
    /// Walking right.
    MoveRight = 2015,
    /// Throwing a bomb.
    Throw = 2016,
    /// Being picked up by Claw.
    Lifted = 2017,
    /// Held over Claw's head.
    Flailing = 2018,
    /// Flying after Claw threw it.
    Thrown = 2019,
    /// Bouncing on landing after a throw.
    Bounce = 2020,
    /// Let go by Claw.
    Dropped = 2021,
    /// A boss waiting for the fight to start.
    Startup = 2022,
    /// Hopping back, away from Claw.
    JumpBack = 2030,
    /// Backing off from Claw.
    MoveBack = 2031,
    /// Stepping in towards Claw.
    MoveAhead = 2032,
    /// Blocking a high blow.
    BlockHigh = 2033,
    /// Blocking a low blow.
    BlockLow = 2034,
    /// The idle state of a human class without one of its own.
    HumanIdle = 2035,
    /// Launched off a springboard.
    JumpOffSpringboard = 2036,
    /// Thrown back by its own cannon's shot (the punk rat).
    Recoil = 2050,
    /// Firing upwards (a cannon).
    VertFiring = 2060,
    /// Turning the barrel up or back down (the eighth level's deck cannon,
    /// see [`crate::logic::cannon_switch::cannon`]).
    Rotate = 2061,
    /// Leaping over Claw to the far end of its ground (Raux).
    JumpOver = 2070,
    /// Somersaulting over Claw to land on his far side (Katherine).
    Flip = 2120,
    /// Landing from the somersault (Katherine).
    FlipLand = 2121,
    /// The second idle pose, after a random wait (the town guards).
    Idle2 = 2140,
    /// Parrying a blow, unhurt (the second town guard).
    Block = 2141,
    /// Flying back up to its patrol height after a dive (the seagull).
    Withdraw = 2142,
    /// Flinching from a hit, then back to what it was doing (the seagull).
    GenericHit = 2143,
    /// Choosing the next shot of a magic volley, or ending it
    /// (Wolvington).
    MagicVolley = 2160,
    /// Casting one magic shot, high or low (Wolvington).
    MagicShot = 2161,
    /// The standing uppercut (Wolvington).
    Uppercut = 2162,
    /// The low sword thrust at a crouching Claw (Wolvington).
    SwordThrust = 2163,
    /// Sprinting at Claw into an uppercut (Wolvington).
    Sprint = 2164,
    /// Grabbing Claw for a hug (the bear sailor).
    StartHug = 2180,
    /// Holding Claw in a hug and squeezing him (the bear sailor).
    Hug = 2181,
    /// Letting Claw go from a hug (the bear sailor; no rule enters it).
    Release = 2182,
    /// The attack a pirate makes in the eighth level's boss fight (the
    /// red-tail pirate; never entered on the seventh level).
    GabrielAttack = 2183,
    /// Facing Claw and choosing between hiding and crouching (the crabs).
    Regroup = 2190,
    /// Withdrawn into the shell (the crabs).
    Hide = 2191,
    /// Crouched in the shell, then choosing what to do (the crabs).
    Defense = 2192,
    /// Choosing between charging, snipping and going back (the crabs).
    Attack = 2193,
    /// Charging at Claw (the crabs).
    MovingAttack = 2194,
    /// Snipping at Claw (the crabs).
    Snip = 2195,
    /// Falling out of a nest until it lands (the crabs).
    Tumble = 2196,
    /// Knocked aside in its shell (the crabs).
    Nudge = 2197,
    /// Throwing a bomb (the bomber crabs).
    Launch = 2198,
    /// Lobbing three bombs to his left (Gabriel).
    MultiBomb = 2200,
    /// Calling a pirate in (Gabriel).
    ThugAttack = 2201,
    /// Pulling the lever that fires his cannon (Gabriel).
    CannonAttack = 2202,
    /// Struck by a cannonball (Gabriel).
    CannonHit = 2203,
    /// Shrugging off a blow, unhurt (Gabriel).
    Blocked = 2204,
    /// The first of his four death acts (Gabriel).
    DeadAct1 = 2205,
    /// The second death act: the fall from the deck (Gabriel).
    DeadAct2 = 2206,
    /// The third death act (Gabriel).
    DeadAct3 = 2207,
    /// The last death act, which ends the fight (Gabriel).
    DeadAct4 = 2208,
    /// Rolling towards Claw (PegLeg).
    RollForward = 2220,
    /// Rolling away from Claw (PegLeg).
    RollBackward = 2221,
    /// Holding the hook out, its whole picture hurting on contact (the
    /// hook-handed pirate).
    HoldHook = 2222,
    /// Whistling for the parrot and waiting to be carried, Safe (Marrow).
    WaitForParrot = 2240,
    /// Patrolling the line near the top of the room (Marrow's parrot).
    FlyAbout = 2241,
    /// Turning down towards a dive point (Marrow's parrot).
    StrikeSetup = 2242,
    /// Flying home to the perch (Marrow's parrot).
    FlyToIdle = 2243,
    /// Flying to Marrow to take hold of him (Marrow's parrot).
    FlyToMarrow = 2244,
    /// Carrying Marrow across the pit (Marrow's parrot).
    CarryMarrow = 2245,
    /// Swimming up and down its pool (the Fish).
    Swim = 2270,
    /// Leaping out of the water at Claw (the Fish).
    JumpStrike = 2271,
    /// Flopping on dry ground until it slides back in (the Fish).
    OutOfWater = 2272,
    /// Singing, a scream every few ticks (the Siren).
    Singing = 2273,
    /// A punch high over the water (Aquatis).
    HighPunch = 2280,
    /// A punch low along the water (Aquatis).
    LowPunch = 2281,
    /// Thrashing in a frenzy after a stalactite's hit (Aquatis and the
    /// tentacles).
    SuperFrenzy = 2282,
    /// Reeling from a stalactite that fell on him (Aquatis).
    StalactiteHit = 2283,
    /// Cowering, during an earthquake or after three flinches (Aquatis).
    Scared = 2284,
    /// Taking hold of Claw (a tentacle).
    StartGrab = 2290,
    /// Squeezing the held Claw (a tentacle).
    Squeeze = 2291,
    /// Growing back out of the water (a tentacle).
    Respawn = 2292,
    /// Throwing three knives at Claw on the other level (RedTail).
    KnifeAttack = 2300,
    /// Kicking at Claw beside him (RedTail).
    KickAttack = 2301,
    /// Sliding along the sand at Claw (RedTail).
    SlideAttack = 2302,
    /// Firing the flare gun (RedTail).
    GunAttack = 2303,
    /// Raising the wind from the ledge (RedTail).
    WindAttack = 2304,
    /// Leaping up from the sand to the ledge (RedTail).
    JumpUp = 2305,
    /// Leaping down from the ledge to the sand (RedTail).
    JumpDown = 2306,
    /// Fading out where it stands (the Chameleon).
    PhaseOut = 2310,
    /// Hidden somewhere else, then fading back in (the Chameleon).
    PhaseIn = 2311,
    /// Crouching, then dashing at where Claw stood, claws out (the tiger
    /// guards).
    LungeAttack = 2320,
    /// Somersaulting towards Claw (the tiger guards).
    FrontFlip = 2321,
    /// Somersaulting away from Claw (the tiger guards).
    BackFlip = 2322,
    /// Casting an energy ball at Claw (Omar).
    MagicAttack = 2330,
    /// Flashing in place before he appears somewhere else (Omar).
    MoveToPosition = 2331,
    /// Doing nothing for the rest of the level (Omar, after his death).
    Wait = 2332,
}

impl EnemyState {
    /// Every state, in number order.
    pub const ALL: [EnemyState; 103] = [
        EnemyState::Idle,
        EnemyState::Home,
        EnemyState::Strike,
        EnemyState::Pace,
        EnemyState::HitLow,
        EnemyState::HitHigh,
        EnemyState::Dead,
        EnemyState::Pause,
        EnemyState::Falling,
        EnemyState::Advance,
        EnemyState::FastAdvance,
        EnemyState::JumpAtClaw,
        EnemyState::Jump,
        EnemyState::Fire,
        EnemyState::MoveLeft,
        EnemyState::MoveRight,
        EnemyState::Throw,
        EnemyState::Lifted,
        EnemyState::Flailing,
        EnemyState::Thrown,
        EnemyState::Bounce,
        EnemyState::Dropped,
        EnemyState::Startup,
        EnemyState::JumpBack,
        EnemyState::MoveBack,
        EnemyState::MoveAhead,
        EnemyState::BlockHigh,
        EnemyState::BlockLow,
        EnemyState::HumanIdle,
        EnemyState::JumpOffSpringboard,
        EnemyState::Recoil,
        EnemyState::VertFiring,
        EnemyState::Rotate,
        EnemyState::JumpOver,
        EnemyState::Flip,
        EnemyState::FlipLand,
        EnemyState::Idle2,
        EnemyState::Block,
        EnemyState::Withdraw,
        EnemyState::GenericHit,
        EnemyState::MagicVolley,
        EnemyState::MagicShot,
        EnemyState::Uppercut,
        EnemyState::SwordThrust,
        EnemyState::Sprint,
        EnemyState::StartHug,
        EnemyState::Hug,
        EnemyState::Release,
        EnemyState::GabrielAttack,
        EnemyState::Regroup,
        EnemyState::Hide,
        EnemyState::Defense,
        EnemyState::Attack,
        EnemyState::MovingAttack,
        EnemyState::Snip,
        EnemyState::Tumble,
        EnemyState::Nudge,
        EnemyState::Launch,
        EnemyState::MultiBomb,
        EnemyState::ThugAttack,
        EnemyState::CannonAttack,
        EnemyState::CannonHit,
        EnemyState::Blocked,
        EnemyState::DeadAct1,
        EnemyState::DeadAct2,
        EnemyState::DeadAct3,
        EnemyState::DeadAct4,
        EnemyState::RollForward,
        EnemyState::RollBackward,
        EnemyState::HoldHook,
        EnemyState::WaitForParrot,
        EnemyState::FlyAbout,
        EnemyState::StrikeSetup,
        EnemyState::FlyToIdle,
        EnemyState::FlyToMarrow,
        EnemyState::CarryMarrow,
        EnemyState::Swim,
        EnemyState::JumpStrike,
        EnemyState::OutOfWater,
        EnemyState::Singing,
        EnemyState::HighPunch,
        EnemyState::LowPunch,
        EnemyState::SuperFrenzy,
        EnemyState::StalactiteHit,
        EnemyState::Scared,
        EnemyState::StartGrab,
        EnemyState::Squeeze,
        EnemyState::Respawn,
        EnemyState::KnifeAttack,
        EnemyState::KickAttack,
        EnemyState::SlideAttack,
        EnemyState::GunAttack,
        EnemyState::WindAttack,
        EnemyState::JumpUp,
        EnemyState::JumpDown,
        EnemyState::PhaseOut,
        EnemyState::PhaseIn,
        EnemyState::LungeAttack,
        EnemyState::FrontFlip,
        EnemyState::BackFlip,
        EnemyState::MagicAttack,
        EnemyState::MoveToPosition,
        EnemyState::Wait,
    ];

    /// The state with this number; `None` for a number no state has,
    /// including the free numbers of a block.
    pub fn from_id(id: i32) -> Option<EnemyState> {
        EnemyState::ALL
            .binary_search_by_key(&id, |s| *s as i32)
            .ok()
            .map(|i| EnemyState::ALL[i])
    }

    /// Name.
    pub fn name(self) -> &'static str {
        match self {
            EnemyState::Idle => "Idle",
            EnemyState::Home => "Home",
            EnemyState::Strike => "Strike",
            EnemyState::Pace => "Pace",
            EnemyState::HitLow => "HitLow",
            EnemyState::HitHigh => "HitHigh",
            EnemyState::Dead => "Dead",
            EnemyState::Pause => "Pause",
            EnemyState::Falling => "Falling",
            EnemyState::Advance => "Advance",
            EnemyState::FastAdvance => "FastAdvance",
            EnemyState::JumpAtClaw => "JumpAtClaw",
            EnemyState::Jump => "Jump",
            EnemyState::Fire => "Fire",
            EnemyState::MoveLeft => "MoveLeft",
            EnemyState::MoveRight => "MoveRight",
            EnemyState::Throw => "Throw",
            EnemyState::Lifted => "Lifted",
            EnemyState::Flailing => "Flailing",
            EnemyState::Thrown => "Thrown",
            EnemyState::Bounce => "Bounce",
            EnemyState::Dropped => "Dropped",
            EnemyState::Startup => "Startup",
            EnemyState::JumpBack => "JumpBack",
            EnemyState::MoveBack => "MoveBack",
            EnemyState::MoveAhead => "MoveAhead",
            EnemyState::BlockHigh => "BlockHigh",
            EnemyState::BlockLow => "BlockLow",
            EnemyState::HumanIdle => "HumanIdle",
            EnemyState::JumpOffSpringboard => "JumpOffSpringboard",
            EnemyState::Recoil => "Recoil",
            EnemyState::VertFiring => "VertFiring",
            EnemyState::Rotate => "Rotate",
            EnemyState::JumpOver => "JumpOver",
            EnemyState::Flip => "Flip",
            EnemyState::FlipLand => "FlipLand",
            EnemyState::Idle2 => "Idle2",
            EnemyState::Block => "Block",
            EnemyState::Withdraw => "Withdraw",
            EnemyState::GenericHit => "GenericHit",
            EnemyState::MagicVolley => "MagicVolley",
            EnemyState::MagicShot => "MagicShot",
            EnemyState::Uppercut => "Uppercut",
            EnemyState::SwordThrust => "SwordThrust",
            EnemyState::Sprint => "Sprint",
            EnemyState::StartHug => "StartHug",
            EnemyState::Hug => "Hug",
            EnemyState::Release => "Release",
            EnemyState::GabrielAttack => "GabrielAttack",
            EnemyState::Regroup => "Regroup",
            EnemyState::Hide => "Hide",
            EnemyState::Defense => "Defense",
            EnemyState::Attack => "Attack",
            EnemyState::MovingAttack => "MovingAttack",
            EnemyState::Snip => "Snip",
            EnemyState::Tumble => "Tumble",
            EnemyState::Nudge => "Nudge",
            EnemyState::Launch => "Launch",
            EnemyState::MultiBomb => "MultiBomb",
            EnemyState::ThugAttack => "ThugAttack",
            EnemyState::CannonAttack => "CannonAttack",
            EnemyState::CannonHit => "CannonHit",
            EnemyState::Blocked => "Blocked",
            EnemyState::DeadAct1 => "DeadAct1",
            EnemyState::DeadAct2 => "DeadAct2",
            EnemyState::DeadAct3 => "DeadAct3",
            EnemyState::DeadAct4 => "DeadAct4",
            EnemyState::RollForward => "RollForward",
            EnemyState::RollBackward => "RollBackward",
            EnemyState::HoldHook => "HoldHook",
            EnemyState::WaitForParrot => "WaitForParrot",
            EnemyState::FlyAbout => "FlyAbout",
            EnemyState::StrikeSetup => "StrikeSetup",
            EnemyState::FlyToIdle => "FlyToIdle",
            EnemyState::FlyToMarrow => "FlyToMarrow",
            EnemyState::CarryMarrow => "CarryMarrow",
            EnemyState::Swim => "Swim",
            EnemyState::JumpStrike => "JumpStrike",
            EnemyState::OutOfWater => "OutOfWater",
            EnemyState::Singing => "Singing",
            EnemyState::HighPunch => "HighPunch",
            EnemyState::LowPunch => "LowPunch",
            EnemyState::SuperFrenzy => "SuperFrenzy",
            EnemyState::StalactiteHit => "StalactiteHit",
            EnemyState::Scared => "Scared",
            EnemyState::StartGrab => "StartGrab",
            EnemyState::Squeeze => "Squeeze",
            EnemyState::Respawn => "Respawn",
            EnemyState::KnifeAttack => "KnifeAttack",
            EnemyState::KickAttack => "KickAttack",
            EnemyState::SlideAttack => "SlideAttack",
            EnemyState::GunAttack => "GunAttack",
            EnemyState::WindAttack => "WindAttack",
            EnemyState::JumpUp => "JumpUp",
            EnemyState::JumpDown => "JumpDown",
            EnemyState::PhaseOut => "PhaseOut",
            EnemyState::PhaseIn => "PhaseIn",
            EnemyState::LungeAttack => "LungeAttack",
            EnemyState::FrontFlip => "FrontFlip",
            EnemyState::BackFlip => "BackFlip",
            EnemyState::MagicAttack => "MagicAttack",
            EnemyState::MoveToPosition => "MoveToPosition",
            EnemyState::Wait => "Wait",
        }
    }
}

/// A call scheduled for the post-tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deferred {
    /// Go idle (Claw left range).
    GoIdle,
    /// Standing on a narrow ledge: leap at Claw when he is near, otherwise
    /// stay put for good.
    JumpCheck,
    /// Let go by Claw.
    Dropped,
    /// A call of the shared human states (see [`human::run_deferred`]).
    Human(human::HumanCall),
}

/// Why an enemy died.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DeathKind {
    /// Killed by a hit: the body flies off.
    #[default]
    Hit,
    /// Killed by landing after being thrown.
    Landing,
    /// Died on spikes.
    Spikes,
    /// Died in goo.
    Goo,
}

/// A timer: running since `start` for `len` ms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timer {
    /// Start, simulation ms.
    pub start: u64,
    /// Length, ms.
    pub len: u64,
}

impl Timer {
    /// A timer started at `now` for `len` ms.
    pub fn new(now: u64, len: u64) -> Timer {
        Timer { start: now, len }
    }

    /// True once `len` ms have passed since `start`.
    pub fn done(&self, now: u64) -> bool {
        now.saturating_sub(self.start) >= self.len
    }
}

/// A shot or bomb an enemy is about to release: where it leaves, relative
/// to the enemy, and its speed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Shot {
    /// Offset from the enemy, px.
    pub dx: i32,
    /// Offset from the enemy, px.
    pub dy: i32,
    /// Speed, px/s.
    pub vx: i32,
    /// Speed, px/s.
    pub vy: i32,
}

/// Data every enemy keeps, boxed so that the objects of other families
/// stay small; it reads and writes as [`EnemyFields`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnemyData(pub Box<EnemyFields>);

impl std::ops::Deref for EnemyData {
    type Target = EnemyFields;

    fn deref(&self) -> &EnemyFields {
        &self.0
    }
}

impl std::ops::DerefMut for EnemyData {
    fn deref_mut(&mut self) -> &mut EnemyFields {
        &mut self.0
    }
}

impl From<EnemyFields> for EnemyData {
    fn from(f: EnemyFields) -> EnemyData {
        EnemyData(Box::new(f))
    }
}

/// The fields of [`EnemyData`].
#[derive(Debug, Clone, PartialEq)]
pub struct EnemyFields {
    /// State before the last change.
    pub prev: EnemyState,
    /// Busy: striking, flinching, jumping, carried or dead; nothing is
    /// scheduled for it in the post-tick.
    pub busy: bool,
    /// Claw is to the right: his whole pixel at or past the enemy's exact
    /// x (Claw at 16222 is left of an enemy at 16222.6).
    pub claw_right: bool,
    /// Range mode, 0 to 6 (see [`range_mode`]).
    pub mode: u8,
    /// Claw has left every range box (set on the first tick out of range).
    pub out_of_range: bool,
    /// When Claw was first seen out of range on screen.
    pub miss_since: Option<u64>,
    /// This tick's draw of 0..=1000.
    pub roll: i32,
    /// Half width and half height of the five range boxes, outermost
    /// first.
    pub ranges: [(i32, i32); 5],
    /// Patrol limits across (min, max); [`UNBOUNDED`] without.
    pub bounds: (i32, i32),
    /// Limits up and down (min, max); [`UNBOUNDED`] without.
    pub bounds_y: (i32, i32),
    /// Never walks: no pacing, no ledge guard, no leap at Claw.
    pub stationary: bool,
    /// Turns at ledges.
    pub ledge_guard: bool,
    /// Lands only on the tiles: falls through pegs, elevators and every
    /// other platform object, in every state (a hermit crab).
    pub tiles_only: bool,
    /// Drawn in front of or behind Claw by who stands higher.
    pub z_sort: bool,
    /// Depth base of the depth against Claw.
    pub z_base: i32,
    /// Has died once (no second loot drop, no second death on a death
    /// tile).
    pub dead: bool,
    /// Goes by sight: notices Claw only when it can see him.
    pub sight: bool,
    /// Claw may lift it.
    pub may_lift: bool,
    /// Number of the last attack of Claw that hit it.
    pub serial_seen: Option<u32>,
    /// Hit cooldown.
    pub cooldown: Timer,
    /// Why it died.
    pub death: DeathKind,
    /// Removal timer of a death on spikes or after a throw.
    pub death_timer: Timer,
    /// Pause timer.
    pub pause: Timer,
    /// Velocity, px/s.
    pub vx: f64,
    /// Velocity, px/s.
    pub vy: f64,
    /// Vertical acceleration, px/s^2.
    pub ay: f64,
    /// The height a dive ends below (the seagull: where Claw stood when
    /// it dived).
    pub target_y: i32,
    /// Sub-pixel position carried to the next tick.
    pub carry: (f64, f64),
    /// Where the last movement step left the object; a move made by
    /// anything else drops the carried fraction.
    pub moved_to: Option<(i32, i32)>,
    /// The tile result of the last movement step (the bits of
    /// [`crate::physics::res`]); a step that does not run keeps it.
    pub last_result: u32,
    /// Game time of the last movement step, ms (see [`motion`]); `None`
    /// before the first.
    pub last_move_ms: Option<u64>,
    /// No movement at all (dead in place, or carried by Claw).
    pub frozen: bool,
    /// An x the enemy's own logic wrote as its new position in this tick
    /// (the Chameleon's phase): the next movement step starts from it
    /// instead of where the object stands, moving the object there through
    /// the tiles, and a wall or a ledge that stops the move makes it the
    /// patrol limit on that side (see [`motion`]). `None` otherwise.
    pub teleport_to: Option<i32>,
    /// Call scheduled for the post-tick.
    pub deferred: Option<Deferred>,
    /// Object holding the level's strike token.
    pub token_at: Option<ObjectId>,
    /// Contact box, facing left (the sprites' own direction).
    pub contact_rect: Rect,
    /// Strike box in use, facing left.
    pub strike_rect: Rect,
    /// Strike animation in use.
    pub strike_anim: &'static str,
    /// Death animation in use.
    pub death_anim: &'static str,
    /// Death sound.
    pub death_sound: &'static str,
    /// Spoken when lifted.
    pub lifted_voice: &'static str,
    /// Stand pose timer.
    pub pose: Timer,
    /// Time idle before pacing may start.
    pub pace_wait: Timer,
    /// Smarts, a percentage some decisions roll against.
    pub smarts: i32,
    /// Loot codes, in drop order (0 for an empty slot).
    pub loot: [i32; 9],
    /// The alert has been shouted (once per soldier).
    pub alerted: bool,
    /// Wait between shots or throws.
    pub fire_wait: Timer,
    /// Event ticks of the current firing or throwing animation.
    pub shots: u32,
    /// The shot or bomb of the current firing or throwing animation.
    pub shot: Shot,
    /// Lifting: the movement box saved while carried.
    pub saved_move_rect: Rect,
    /// Lifting: Claw is winding up a throw.
    pub winding_up: bool,
    /// Lifting: time after a throw before it can be lifted again.
    pub lift_guard: Timer,
    /// Lifting: time a thrown enemy flies before it is gone.
    pub thrown_timer: Timer,
    /// Lifting: landings after a throw.
    pub bounces: u32,
    /// Lifting: it is let go once its pause after a throw ends.
    pub dropped_timer: Timer,
    /// What the shared human states keep (see [`human`]).
    pub human: human::HumanData,
    /// What Wolvington keeps beyond the shared data (see
    /// [`wolvington::Fields`]).
    pub wolvington: wolvington::Fields,
}

impl Default for EnemyFields {
    fn default() -> Self {
        EnemyFields {
            prev: EnemyState::default(),
            busy: false,
            claw_right: false,
            mode: 6,
            out_of_range: false,
            miss_since: None,
            roll: 0,
            ranges: [(0, 0); 5],
            bounds: UNBOUNDED,
            bounds_y: UNBOUNDED,
            stationary: false,
            ledge_guard: false,
            tiles_only: false,
            z_sort: false,
            z_base: 0,
            dead: false,
            sight: false,
            may_lift: false,
            serial_seen: None,
            cooldown: Timer::default(),
            death: DeathKind::default(),
            death_timer: Timer::default(),
            pause: Timer::default(),
            vx: 0.0,
            vy: 0.0,
            ay: GRAVITY,
            target_y: 0,
            carry: (0.0, 0.0),
            moved_to: None,
            last_result: 0,
            last_move_ms: None,
            frozen: false,
            teleport_to: None,
            deferred: None,
            token_at: None,
            contact_rect: Rect::UNSET,
            strike_rect: Rect::UNSET,
            strike_anim: "",
            death_anim: "",
            death_sound: "",
            lifted_voice: "",
            pose: Timer::default(),
            pace_wait: Timer::default(),
            smarts: 0,
            loot: [0; 9],
            alerted: false,
            fire_wait: Timer::default(),
            shots: 0,
            shot: Shot::default(),
            saved_move_rect: Rect::UNSET,
            winding_up: false,
            lift_guard: Timer::default(),
            thrown_timer: Timer::default(),
            bounces: 0,
            dropped_timer: Timer::default(),
            human: human::HumanData::default(),
            wolvington: wolvington::Fields::default(),
        }
    }
}

/// A state handler.
pub type Handler = fn(&mut World, ObjectId);

/// One enemy class: its own handlers and what the shared states use.
#[derive(Debug)]
pub struct Class {
    /// Name, for messages.
    pub name: &'static str,
    /// A human enemy (officer, soldier): the human pre-tick, hit rule,
    /// death and states apply.
    pub human: bool,
    /// Builds the enemy on its first logic call.
    pub construct: fn(&mut World, ObjectId),
    /// The class's pre-tick steps, run after the shared ones.
    pub pre_tick: fn(&mut World, ObjectId),
    /// The class's own handler of a state, when it has one.
    pub handler: fn(EnemyState) -> Option<Handler>,
    /// Goes Home.
    pub go_home: fn(&mut World, ObjectId),
    /// Goes idle.
    pub go_idle: fn(&mut World, ObjectId),
    /// How Claw carries it; `None` when it cannot be lifted.
    pub carry: Option<lift::Carry>,
    /// Picks the line it says when lifted (into
    /// [`EnemyFields::lifted_voice`]).
    pub pick_lift_voice: fn(&mut World, ObjectId),
    /// Whether the shared lift draws its one of 0..100 for saying the
    /// lifted line (see [`lift::lifted`]); false for a class that always
    /// says its own line and makes no draw for it.
    pub lift_roll: bool,
    /// Animations the shared states play.
    pub anims: ClassAnims,
    /// Sounds the shared states play.
    pub sounds: ClassSounds,
}

/// Animations of a class used by the shared states ("" for none).
#[derive(Debug, Clone, Copy)]
pub struct ClassAnims {
    /// Walking toward Claw.
    pub advance: &'static str,
    /// Running toward Claw.
    pub fast_advance: &'static str,
    /// Pacing.
    pub pace: &'static str,
    /// Walking (the rat).
    pub walk: &'static str,
    /// Low hit reaction.
    pub hit_low: &'static str,
    /// High hit reaction.
    pub hit_high: &'static str,
    /// Standing at home before a pause, and after landing from a throw.
    pub home: &'static str,
    /// Jumping.
    pub jump: &'static str,
    /// Falling.
    pub fall: &'static str,
    /// Being lifted.
    pub lifted: &'static str,
    /// Held overhead, and flying after a throw.
    pub lifted2: &'static str,
    /// Bouncing after a throw.
    pub bounce: &'static str,
}

impl ClassAnims {
    /// No animations.
    pub const NONE: ClassAnims = ClassAnims {
        advance: "",
        fast_advance: "",
        pace: "",
        walk: "",
        hit_low: "",
        hit_high: "",
        home: "",
        jump: "",
        fall: "",
        lifted: "",
        lifted2: "",
        bounce: "",
    };
}

/// Sounds of a class used by the shared states ("" for none).
#[derive(Debug, Clone, Copy)]
pub struct ClassSounds {
    /// The strike landing.
    pub strike: &'static str,
    /// A low hit, then a second one.
    pub hit_low: [&'static str; 2],
    /// A high hit, then one of the four hit sounds.
    pub hit_high: &'static str,
}

impl ClassSounds {
    /// No sounds.
    pub const NONE: ClassSounds = ClassSounds {
        strike: "",
        hit_low: ["", ""],
        hit_high: "",
    };
}

/// The four sounds one of which follows a high hit.
pub const HIT_SOUNDS: [&str; 4] = ["GAME_HIT1", "GAME_HIT2", "GAME_HIT3", "GAME_HIT4"];
/// The second sound of a low hit.
pub const HIT_LOW_SECOND: &str = "GAME_HIT1";
/// The sound of loot being dropped.
pub const DROP_SOUND: &str = "GAME_PURELEASE1";
/// The sound of a dead enemy leaving the screen.
pub const SPLASH_SOUND: &str = "GAME_SPLASH";
/// The sound of an enemy dying on spikes or in goo.
pub const DEATH_TILE_SOUND: &str = "LEVEL_DEATHTILE";
/// The sound of a thrown enemy landing for the first time.
pub const LAND_SOUND: &str = "GAME_LELAND";
/// Claw's lines after a kill, one chosen at random.
pub const CLAW_KILL_LINES: [&str; 10] = [
    "CLAW_1001",
    "CLAW_1004A",
    "CLAW_1004B",
    "CLAW_1007A",
    "CLAW_1007B",
    "CLAW_1010",
    "CLAW_1110033",
    "CLAW_1110059",
    "CLAW_1110063",
    "CLAW_1110058",
];

/// A class step that does nothing.
pub fn nothing(_: &mut World, _: ObjectId) {}

/// The class of an enemy family.
pub fn class(g: GameLogic) -> Option<&'static Class> {
    match g {
        GameLogic::Officer => Some(&officer::CLASS),
        GameLogic::Soldier => Some(&soldier::CLASS),
        GameLogic::Rat => Some(&rat::CLASS),
        GameLogic::PunkRat => Some(&punkrat::CLASS),
        GameLogic::Raux => Some(&raux::CLASS),
        GameLogic::CutThroat => Some(&cutthroat::CLASS),
        GameLogic::RobberThief => Some(&robberthief::CLASS),
        GameLogic::Katherine => Some(&katherine::CLASS),
        GameLogic::TownGuard1 => Some(&townguard::guard1::CLASS),
        GameLogic::TownGuard2 => Some(&townguard::guard2::CLASS),
        GameLogic::Seagull => Some(&seagull::CLASS),
        GameLogic::Wolvington => Some(&wolvington::CLASS),
        GameLogic::RedTailPirate => Some(&redtail_pirate::CLASS),
        GameLogic::BearSailor => Some(&bear_sailor::CLASS),
        GameLogic::HermitCrab => Some(&crab::CLASS),
        GameLogic::Gabriel => Some(&gabriel::CLASS),
        GameLogic::HookPirate => Some(&hook_pirate::CLASS),
        GameLogic::PegLeg => Some(&pegleg::CLASS),
        GameLogic::Marrow => Some(&marrow::CLASS),
        GameLogic::Parrot => Some(&parrot::CLASS),
        GameLogic::Mercat => Some(&mercat::CLASS),
        GameLogic::Siren => Some(&siren::CLASS),
        GameLogic::Fish => Some(&fish::CLASS),
        GameLogic::Aquatis => Some(&aquatis::CLASS),
        GameLogic::Tentacle => Some(&tentacle::CLASS),
        GameLogic::Chameleon => Some(&chameleon::CLASS),
        GameLogic::RedTail => Some(&redtail::CLASS),
        GameLogic::TigerGuard => Some(&tiger_guard::CLASS),
        GameLogic::Omar => Some(&omar::CLASS),
        _ => None,
    }
}

/// The class of object `id`.
pub fn class_of(w: &World, id: ObjectId) -> Option<&'static Class> {
    match w.objects.get(id)?.kind {
        LogicKind::Game(g) => class(g),
        _ => None,
    }
}

// ----------------------------------------------------------------------
// Access helpers

/// The enemy's object. Logic calls are made only for objects that exist,
/// and objects are removed only between passes.
pub(crate) fn obj<'a>(w: &'a World, id: ObjectId) -> &'a Object {
    w.objects
        .get(id)
        .expect("an enemy's object exists during its call")
}

/// The enemy's object, mutable.
pub(crate) fn obj_mut<'a>(w: &'a mut World, id: ObjectId) -> &'a mut Object {
    w.objects
        .get_mut(id)
        .expect("an enemy's object exists during its call")
}

impl ObjectData {
    /// The enemy data this holds: plain enemy data, or the enemy data
    /// inside a family's own data (the punk rat's, Raux's, the thieves',
    /// Katherine's, the town guards', the seagull's, Wolvington's, the
    /// red-tail pirates', the bear sailors', the crabs', Gabriel's, the
    /// hook-handed pirates', PegLegs', Marrow's and his parrot's, the
    /// Mercats', Sirens' and Fish's, Aquatis's and the tentacles', the
    /// Chameleons' and RedTail's, and the tiger guards' and Omar's).
    pub fn enemy(&self) -> Option<&EnemyData> {
        match self {
            ObjectData::Enemy(d) => Some(d),
            ObjectData::PunkRat(d) => Some(&d.enemy),
            ObjectData::Raux(d) => Some(&d.enemy),
            ObjectData::CutThroat(d) => Some(&d.enemy),
            ObjectData::RobberThief(d) => Some(&d.enemy),
            ObjectData::Katherine(d) => Some(&d.enemy),
            ObjectData::TownGuard(d) => Some(&d.enemy),
            ObjectData::Seagull(d) => Some(&d.enemy),
            ObjectData::Wolvington(d) => Some(&d.enemy),
            ObjectData::RedTailPirate(d) => Some(&d.enemy),
            ObjectData::BearSailor(d) => Some(&d.enemy),
            ObjectData::HermitCrab(d) => Some(&d.enemy),
            ObjectData::Gabriel(d) => Some(&d.enemy),
            ObjectData::HookPirate(d) => Some(&d.enemy),
            ObjectData::PegLeg(d) => Some(&d.enemy),
            ObjectData::Marrow(d) => Some(&d.enemy),
            ObjectData::Parrot(d) => Some(&d.enemy),
            ObjectData::Mercat(d) => Some(&d.enemy),
            ObjectData::Siren(d) => Some(&d.enemy),
            ObjectData::Fish(d) => Some(&d.enemy),
            ObjectData::Aquatis(d) => Some(&d.enemy),
            ObjectData::Tentacle(d) => Some(&d.enemy),
            ObjectData::Chameleon(d) => Some(&d.enemy),
            ObjectData::RedTail(d) => Some(&d.enemy),
            ObjectData::TigerGuard(d) => Some(&d.enemy),
            ObjectData::Omar(d) => Some(&d.enemy),
            _ => None,
        }
    }

    /// The enemy data this holds, mutable (see [`ObjectData::enemy`]).
    pub fn enemy_mut(&mut self) -> Option<&mut EnemyData> {
        match self {
            ObjectData::Enemy(d) => Some(d),
            ObjectData::PunkRat(d) => Some(&mut d.enemy),
            ObjectData::Raux(d) => Some(&mut d.enemy),
            ObjectData::CutThroat(d) => Some(&mut d.enemy),
            ObjectData::RobberThief(d) => Some(&mut d.enemy),
            ObjectData::Katherine(d) => Some(&mut d.enemy),
            ObjectData::TownGuard(d) => Some(&mut d.enemy),
            ObjectData::Seagull(d) => Some(&mut d.enemy),
            ObjectData::Wolvington(d) => Some(&mut d.enemy),
            ObjectData::RedTailPirate(d) => Some(&mut d.enemy),
            ObjectData::BearSailor(d) => Some(&mut d.enemy),
            ObjectData::HermitCrab(d) => Some(&mut d.enemy),
            ObjectData::Gabriel(d) => Some(&mut d.enemy),
            ObjectData::HookPirate(d) => Some(&mut d.enemy),
            ObjectData::PegLeg(d) => Some(&mut d.enemy),
            ObjectData::Marrow(d) => Some(&mut d.enemy),
            ObjectData::Parrot(d) => Some(&mut d.enemy),
            ObjectData::Mercat(d) => Some(&mut d.enemy),
            ObjectData::Siren(d) => Some(&mut d.enemy),
            ObjectData::Fish(d) => Some(&mut d.enemy),
            ObjectData::Aquatis(d) => Some(&mut d.enemy),
            ObjectData::Tentacle(d) => Some(&mut d.enemy),
            ObjectData::Chameleon(d) => Some(&mut d.enemy),
            ObjectData::RedTail(d) => Some(&mut d.enemy),
            ObjectData::TigerGuard(d) => Some(&mut d.enemy),
            ObjectData::Omar(d) => Some(&mut d.enemy),
            _ => None,
        }
    }
}

/// Runs `f` with the enemy's data taken out of its object (so `f` can use
/// the whole world), then puts it back. `None` before the enemy is built.
pub fn with_data<R>(
    w: &mut World,
    id: ObjectId,
    f: impl FnOnce(&mut World, &mut EnemyData) -> R,
) -> Option<R> {
    let o = w.objects.get_mut(id)?;
    o.data.enemy()?;
    let mut data = std::mem::take(&mut o.data);
    let r = f(w, data.enemy_mut().expect("the data holds enemy data"));
    if let Some(o) = w.objects.get_mut(id) {
        o.data = data;
    }
    Some(r)
}

/// The enemy's exact position across: its x plus the fraction its last
/// movement step carried, unless something else has moved it since (that
/// fraction is then dropped and x is exact). Patrol limits are tested
/// against it: an enemy is at a limit only when it stands exactly on it.
pub fn exact_x(w: &World, id: ObjectId) -> f64 {
    let o = obj(w, id);
    let carry = data(w, id)
        .filter(|d| d.moved_to == Some((o.x, o.y)))
        .map_or(0.0, |d| d.carry.0);
    f64::from(o.x) + carry
}

/// The enemy's data.
pub fn data<'a>(w: &'a World, id: ObjectId) -> Option<&'a EnemyData> {
    w.objects.get(id)?.data.enemy()
}

/// The enemy's data, mutable.
pub fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut EnemyData> {
    w.objects.get_mut(id)?.data.enemy_mut()
}

/// The enemy data of `o`, if it has any.
pub fn data_of(o: &Object) -> Option<&EnemyData> {
    o.data.enemy()
}

/// The enemy's state.
pub fn state(w: &World, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(w.objects.get(id)?.state)
}

/// Changes the enemy's state, remembering the previous one.
pub fn set_state(w: &mut World, id: ObjectId, s: EnemyState) {
    let prev = state(w, id).unwrap_or_default();
    if let Some(d) = data_mut(w, id) {
        d.prev = prev;
    }
    obj_mut(w, id).state = s as i32;
}

/// One draw of `0..n` from the shared random stream.
pub fn rand(w: &mut World, n: u32) -> i32 {
    w.rng.below(n) as i32
}

/// True when the enemy is on screen (its picture overlaps the view).
pub fn on_screen(w: &World, id: ObjectId) -> bool {
    w.objects.get(id).is_some_and(|o| w.on_screen(o))
}

/// True while Claw's invisibility powerup runs: enemies do not see him.
pub fn claw_invisible(w: &World) -> bool {
    w.claw.stats.powerup_ms > 0 && w.claw.stats.powerup_kind == INVISIBILITY_POWERUP
}

/// True when the enemy faces right (its sprite is mirrored).
pub fn faces_right(o: &Object) -> bool {
    o.draw_flags & draw_flags::MIRROR != 0
}

/// Turns the enemy to face right or left, mirroring its boxes.
pub fn set_facing(w: &mut World, id: ObjectId, right: bool) {
    let o = obj_mut(w, id);
    if faces_right(o) != right {
        o.draw_flags ^= draw_flags::MIRROR;
        o.mirror_rects();
    }
}

/// Turns the enemy toward Claw.
pub fn face_claw(w: &mut World, id: ObjectId) {
    let right = data(w, id).is_some_and(|d| d.claw_right);
    set_facing(w, id, right);
}

/// `r` as the enemy's facing has it: boxes are given facing left.
pub fn facing_rect(o: &Object, r: Rect) -> Rect {
    if faces_right(o) { r.mirrored() } else { r }
}

/// Starts animation `key` on the enemy. When the level has no such
/// animation the enemy plays none, as the game shows its empty animation
/// instead: a state that waits for its animation to end goes on at once.
pub fn set_anim(w: &mut World, id: ObjectId, key: &str) {
    if !w.set_anim(id, key)
        && let Some(o) = w.objects.get_mut(id)
    {
        o.anim = AnimPlayer::default();
    }
}

/// Starts animation `key` on the enemy and advances it one tick.
pub fn set_anim_now(w: &mut World, id: ObjectId, key: &str) {
    set_anim(w, id, key);
    w.advance_anim(id);
}

/// Plays an enemy sound on the effects channel, centred.
pub fn play(w: &mut World, key: &str) {
    if !key.is_empty() {
        sound::play(w, key, SoundChannel::Effect);
    }
}

/// The enemy says `key` through the enemies' speaking slot, unless
/// another enemy line is running.
pub fn say(w: &mut World, id: ObjectId, key: &str) {
    if !key.is_empty() {
        sound::enemy_say(w, id, key, false);
    }
}

/// A level record field of the enemy (0 without a record).
pub fn record(w: &World, id: ObjectId, i: usize) -> i32 {
    w.objects
        .get(id)
        .and_then(|o| o.record.as_deref())
        .map_or(0, |r| r[i])
}

/// The loot slots of a record, in drop order: the first user box's left,
/// right, top and bottom, the second's, then the powerup field.
pub fn loot_of(w: &World, id: ObjectId) -> [i32; 9] {
    let r = |i| record(w, id, i);
    let [l1, t1, r1, b1] = field::USER_RECT_1;
    let [l2, t2, r2, b2] = field::USER_RECT_2;
    [
        r(l1),
        r(r1),
        r(t1),
        r(b1),
        r(l2),
        r(r2),
        r(t2),
        r(b2),
        r(field::POWERUP),
    ]
}

/// A patrol limit from the record: 0 means none.
pub fn limit(v: i32, none: i32) -> i32 {
    if v == 0 { none } else { v }
}

/// The record's patrol limits across.
pub fn record_bounds(w: &World, id: ObjectId) -> (i32, i32) {
    (
        limit(record(w, id, field::X_MIN), UNBOUNDED.0),
        limit(record(w, id, field::X_MAX), UNBOUNDED.1),
    )
}

/// The record's patrol limits up and down.
pub fn record_bounds_y(w: &World, id: ObjectId) -> (i32, i32) {
    (
        limit(record(w, id, field::Y_MIN), UNBOUNDED.0),
        limit(record(w, id, field::Y_MAX), UNBOUNDED.1),
    )
}

// ----------------------------------------------------------------------
// Building

/// Health of a human enemy of the first level, and of later levels.
pub const HUMAN_HEALTH: (i32, i32) = (3, 7);

/// Health of an officer or a soldier on the level being played: 3 on the
/// first level, 7 on the others, before the difficulty factor.
pub fn human_health(w: &World) -> i32 {
    if w.level_number() == 1 {
        HUMAN_HEALTH.0
    } else {
        HUMAN_HEALTH.1
    }
}

/// The fields every human enemy starts with, before its class sets its
/// own: range mode 6, the record's patrol limits across (none up and
/// down), stationary when the record's first user value is above 0, the
/// ledge guard, the depth sort with base 10, liftable, smarts from 50, and
/// the record's loot.
pub fn human_fields(w: &World, id: ObjectId) -> EnemyFields {
    EnemyFields {
        mode: 6,
        bounds: record_bounds(w, id),
        bounds_y: UNBOUNDED,
        stationary: record(w, id, field::USER_0) > 0,
        ledge_guard: true,
        z_sort: true,
        z_base: 10,
        may_lift: true,
        smarts: smarts(50),
        loot: loot_of(w, id),
        ..EnemyFields::default()
    }
}

/// An enemy's body: what its class gives its object when it is built.
#[derive(Debug, Clone, Copy)]
pub struct Body {
    /// Image set.
    pub image_set: &'static str,
    /// Movement box, facing left.
    pub move_rect: Rect,
    /// Hit box, facing left.
    pub hit_rect: Rect,
    /// Attack (contact) box, facing left.
    pub attack_rect: Rect,
    /// Health before the difficulty factor.
    pub health: i32,
    /// Damage before the difficulty factor.
    pub damage: i32,
    /// Kinds of object that can hit it.
    pub hit_by: u32,
}

/// Gives the enemy its body and `fields`: an enemy that can be hit, is
/// removed far outside the level and (for human enemies) is told before it
/// is removed. Every enemy placed outside its patrol limits is first moved
/// onto the nearer limit (see [`clamp_on_construction`]); then it is
/// dropped to the floor below it. Its exact height stays at the (clamped)
/// point it was placed at: its first movement step starts from there, so
/// an enemy placed 27 px above its floor starts standing with an upward
/// speed of about 327 px/s (see [`motion`]). Its movement clock starts
/// now (see [`start_move_clock`]).
pub fn build(w: &mut World, id: ObjectId, body: Body, fields: EnemyFields, human: bool) {
    let bounds = fields.bounds;
    let bounds_y = record_bounds_y(w, id);
    let o = obj_mut(w, id);
    o.image_set = crate::level::runtime_image_set(body.image_set);
    o.type_flags = types::ENEMY;
    o.hit_type_flags = body.hit_by;
    o.special_flags &= !crate::object::special::LIFTABLE;
    o.flags |= flags::DESTROY_FAR;
    if human {
        o.flags |= flags::NOTIFY_DESTROY;
        o.special_flags |= crate::player::attack::PUNCHABLE;
    }
    o.hit_handler = true;
    o.move_rect = body.move_rect;
    o.hit_rect = body.hit_rect;
    o.attack_rect = body.attack_rect;
    if faces_right(o) {
        o.mirror_rects();
    }
    o.health = scaled(body.health, HEALTH_FACTOR);
    o.damage = scaled(body.damage, DAMAGE_FACTOR);
    o.data = ObjectData::Enemy(fields.into());
    clamp_on_construction(o, bounds, bounds_y);
    let placed_y = o.y;
    drop_to_floor(w, id);
    let (x, y) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    if let Some(d) = data_mut(w, id) {
        d.carry.1 = f64::from(placed_y - y);
        d.moved_to = Some((x, y));
    }
    start_move_clock(w, id);
}

/// The time an enemy is built is the time of its last movement step: its
/// movement clock starts when it is built, not at its first step. So its
/// first step covers the time since then (see [`motion`]): one tick when
/// it first moves in the tick after, nothing in the tick it was built in,
/// and the whole time since when it went back to sleep at the end of that
/// tick (built at the edge of its activation window, then dropped to a
/// floor outside it) and wakes some ticks later. An enemy built in a
/// set-up pass between ticks takes the time of the tick before the next
/// one, so its first step in that next tick covers one tick (while the
/// level loads, before time zero, no time is recorded, which covers one
/// tick too). A class that builds its enemy without [`build`] calls this
/// once its data is set.
pub fn start_move_clock(w: &mut World, id: ObjectId) {
    let at = built_at_ms(w);
    if let Some(d) = data_mut(w, id) {
        d.last_move_ms = at;
    }
}

/// The game time an object built in this call records as the time of its
/// last movement step (see [`start_move_clock`]).
pub fn built_at_ms(w: &World) -> Option<u64> {
    if w.set_up {
        w.now_ms.checked_sub(u64::from(TICK_MS))
    } else {
        Some(w.now_ms)
    }
}

/// The construction step every enemy shares (rats, crabs, seagulls and
/// punk rats as well as the human enemies and the bosses): one movement
/// step from where it was placed, whose clamp moves it onto its patrol
/// limits, across `bounds` and up and down `bounds_y` (each (min, max),
/// each limit when set). An enemy placed beside its range so starts on
/// the nearer limit and, once its class drops it, stands on the floor
/// below that limit; it may now lie outside the activation window that
/// woke it by its placement point and go back to sleep at the end of the
/// tick.
///
/// `bounds` must be the record's range at this step (every class passes
/// its record bounds today, before deriving any of its own). The limits
/// up and down are the record's, which every class sees at this step even
/// when it sets its own afterwards. Provisional: no
/// enemy met on a compared route is placed outside a range up and down.
pub fn clamp_on_construction(o: &mut Object, bounds: (i32, i32), bounds_y: (i32, i32)) {
    clamp_into_bounds(o, bounds);
    let (lo, hi) = bounds_y;
    if lo <= hi {
        o.y = o.y.clamp(lo, hi);
    }
}

/// [`clamp_on_construction`] with the record's own limits, for a class
/// that builds its enemy without [`build`].
pub fn clamp_on_record_range(w: &mut World, id: ObjectId) {
    let (bounds, bounds_y) = (record_bounds(w, id), record_bounds_y(w, id));
    clamp_on_construction(obj_mut(w, id), bounds, bounds_y);
}

/// Moves `o` across into its patrol limits `bounds` (min, max), each when
/// set: an enemy placed beside its range starts on the limit.
pub fn clamp_into_bounds(o: &mut Object, bounds: (i32, i32)) {
    let (lo, hi) = bounds;
    if lo > hi {
        return;
    }
    o.x = o.x.clamp(lo, hi);
}

/// Puts the enemy 32 px higher, then down on the first floor below it,
/// standing.
pub fn drop_to_floor(w: &mut World, id: ObjectId) {
    let map = &w.level.map;
    if let Some(o) = w.objects.get_mut(id) {
        o.y -= 32;
        crate::physics::align_to_ground(map, o);
        o.physics_type = ptype::STAND;
    }
}

// ----------------------------------------------------------------------
// Sight

/// The step of the sight ray, px.
pub const SIGHT_STEP: i32 = 32;
/// How far above or below the enemy Claw can be seen, px (exclusive).
pub const SIGHT_HEIGHT: i32 = 50;

/// True when nothing Solid lies on the row `y` between `x1` and `x2`,
/// looking every `step` px from `x1` toward `x2` and at `x2` itself.
/// Only Solid pixels block; one-way floors, ladders and death tiles do
/// not. Points outside the plane read as its nearest edge.
pub fn line_of_sight(w: &World, x1: i32, y: i32, x2: i32, step: i32) -> bool {
    let map = &w.level.map;
    let step = step.max(1);
    let mut x = x1;
    if x1 <= x2 {
        while x < x2 {
            if map.attr(x, y) == SOLID {
                return false;
            }
            x = x.saturating_add(step);
        }
    } else {
        while x > x2 {
            if map.attr(x, y) == SOLID {
                return false;
            }
            x = x.saturating_sub(step);
        }
    }
    map.attr(x2, y) != SOLID
}

/// True when the enemy sees Claw: Claw is not invisible, he is less than
/// 50 px above or below it, the enemy was on screen, and nothing Solid lies
/// on the enemy's row between it and Claw.
///
/// On screen here means drawn in the frame shown after the last tick
/// ([`crate::object::Object::drawn_last_frame`]): with the frame, facing
/// and position it had then and the view of that frame, not its box now.
/// An enemy that has just switched to a wider frame reaching into the
/// view does not see Claw until a frame has shown it there, and a new
/// enemy sees him only from its second tick.
pub fn sees_claw(w: &World, id: ObjectId) -> bool {
    if claw_invisible(w) {
        return false;
    }
    let Some((cx, cy)) = w.claw_object().map(|c| (c.x, c.y)) else {
        return false;
    };
    let o = obj(w, id);
    (cy - o.y).abs() < SIGHT_HEIGHT
        && o.drawn_last_frame
        && line_of_sight(w, o.x, o.y, cx, SIGHT_STEP)
}

// ----------------------------------------------------------------------
// The level's strike token

/// Logic name of the hidden object holding the level's strike token.
pub const STRIKE_TOKEN_LOGIC: &str = "EnemyStrikeToken";

/// The object holding the level's strike token, created on first use: a
/// hidden object outside the live list whose health counts the tokens
/// (one at the start).
pub fn strike_token_holder(w: &mut World) -> ObjectId {
    if let Some(id) = w
        .objects
        .all()
        .find(|(_, o)| &*o.logic == STRIKE_TOKEN_LOGIC)
        .map(|(id, _)| id)
    {
        return id;
    }
    let mut o = Object::new(0, 0, 0, 0, STRIKE_TOKEN_LOGIC.into(), LogicKind::Inert);
    o.flags = flags::UNLISTED | flags::NO_COLLIDE;
    o.draw_flags = draw_flags::HIDDEN;
    o.health = 1;
    w.spawn(o)
}

/// Takes the level's strike token for `id`; false when another enemy's
/// strike holds it.
pub fn take_strike_token(w: &mut World, id: ObjectId) -> bool {
    let holder = match data(w, id).and_then(|d| d.token_at) {
        Some(h) if w.objects.get(h).is_some() => h,
        _ => {
            let h = strike_token_holder(w);
            if let Some(d) = data_mut(w, id) {
                d.token_at = Some(h);
            }
            h
        }
    };
    let Some(o) = w.objects.get_mut(holder) else {
        return false;
    };
    if o.health != 0 {
        o.health -= 1;
        true
    } else {
        false
    }
}

/// Gives the level's strike token back (any strike ending does).
pub fn give_strike_token(w: &mut World, id: ObjectId) {
    let holder = match data(w, id).and_then(|d| d.token_at) {
        Some(h) if w.objects.get(h).is_some() => h,
        _ => strike_token_holder(w),
    };
    if let Some(o) = w.objects.get_mut(holder) {
        o.health = 1;
    }
}

/// The level's strike tokens left (1 or 0).
pub fn strike_tokens(w: &World) -> i32 {
    w.objects
        .all()
        .find(|(_, o)| &*o.logic == STRIKE_TOKEN_LOGIC)
        .map_or(1, |(_, o)| o.health)
}

// ----------------------------------------------------------------------
// The per-tick run

/// One logic call of an enemy of `class`: builds it on the first call,
/// then runs the pre-tick, the state's handler and the post-tick.
pub fn tick(w: &mut World, id: ObjectId, class: &Class) {
    let raw = obj(w, id).state;
    match raw {
        engine_state::INIT => {
            (class.construct)(w, id);
            return;
        }
        engine_state::DESTROYING => {
            // Being removed: its running line ends.
            sound::enemy_silence(w, id);
            return;
        }
        engine_state::KILLED => {
            // Another logic killed it outright.
            if data(w, id).is_some_and(|d| d.dead) {
                obj_mut(w, id).state = EnemyState::Dead as i32;
            } else {
                obj_mut(w, id).state = EnemyState::Home as i32;
                die(w, id, DeathKind::Hit, None);
            }
            return;
        }
        _ => {}
    }
    if EnemyState::from_id(raw).is_none() {
        return;
    }
    pre_tick(w, id, class);
    (class.pre_tick)(w, id);
    let s = state(w, id).unwrap_or_default();
    if let Some(h) = (class.handler)(s).or_else(|| states::shared_handler(s, class.human)) {
        h(w, id);
    }
    post_tick(w, id, class);
}

/// The per-tick roll of the shared pre-tick, which the crates of a stack
/// and powder kegs make too (they share the enemies' base): on a frame
/// pulse tick (every third tick, see [`World::pulse`]) one draw of
/// 0..=1000; between pulses no draw and a value no chance is ever below.
pub fn pulse_roll(w: &mut World) -> i32 {
    if w.pulse { rand(w, 1001) } else { 0x1FFF_FFFF }
}

/// The shared pre-tick: the per-tick draw, the side and range mode, and,
/// for human enemies, the human steps (see the module documentation).
/// Draws: one of 0..=1000, on a frame pulse tick only (every third tick,
/// see [`World::pulse`]); between pulses the roll holds a value no chance
/// is ever below.
pub fn pre_tick(w: &mut World, id: ObjectId, class: &Class) {
    let roll = pulse_roll(w);
    let (ex, ey) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    let exact = exact_x(w, id);
    let claw = w.claw_object().map(|c| (c.x, c.y));
    let invisible = claw_invisible(w);
    // A hidden enemy is not drawn, so it is never on screen.
    let screen = on_screen(w, id) && obj(w, id).draw_flags & draw_flags::HIDDEN == 0;
    let now = w.now_ms;
    let Some(d) = data_mut(w, id) else { return };
    d.roll = roll;
    if let Some((cx, cy)) = claw {
        d.claw_right = f64::from(cx) >= exact;
        if !invisible {
            d.mode = range_mode(d, (ex, ey), (cx, cy), screen, now);
        }
    }
    if class.human {
        human_pre_tick(w, id);
    }
}

/// The human steps of the pre-tick, in order: the liftable flag and the
/// contact box; a sight-driven enemy that sees Claw stops here; noticing
/// Claw leaving range (going idle in the post-tick when free); on a
/// platform, its pokes and the springboard launch, or becoming stationary
/// (see [`human::platform_step`]); on a narrow ledge, the leap check in
/// the post-tick when free.
fn human_pre_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let Some((may_lift, guard, contact, sight)) =
        data(w, id).map(|d| (d.may_lift, d.lift_guard, d.contact_rect, d.sight))
    else {
        return;
    };
    {
        let o = obj_mut(w, id);
        if may_lift && guard.done(now) {
            o.special_flags |= crate::object::special::LIFTABLE;
        } else {
            o.special_flags &= !crate::object::special::LIFTABLE;
        }
        o.attack_rect = facing_rect(o, contact);
        if faces_right(o) {
            o.special_flags |= MIRRORED;
        } else {
            o.special_flags &= !MIRRORED;
        }
    }
    if sight && sees_claw(w, id) {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        if d.mode >= 5 {
            if !d.out_of_range {
                d.out_of_range = true;
                if !d.busy {
                    d.deferred = Some(Deferred::GoIdle);
                }
            }
        } else {
            d.out_of_range = false;
        }
    }
    if human::platform_step(w, id) {
        return;
    }
    let free = data(w, id).is_some_and(|d| !d.stationary && !d.busy);
    if free
        && motion::on_narrow_ledge(w, id)
        && let Some(d) = data_mut(w, id)
    {
        d.deferred = Some(Deferred::JumpCheck);
    }
}

/// Special flag of a human enemy that faces right.
pub const MIRRORED: u32 = 0x2;

/// The range mode: 4 down to 0 when Claw is inside range box 4 (the
/// innermost) down to 0 (the outermost), tested from the innermost out;
/// otherwise 6 while the enemy is off screen, and 5 once Claw has been
/// outside every box for [`OUT_OF_RANGE_MS`] with the enemy on screen
/// (until then the mode keeps its value). Going off screen cancels that
/// wait as a box hit does, so an enemy that comes back on screen with Claw
/// out of range stays in mode 6 for the first 500 ms.
///
/// Box `i` is centred on the enemy with the half sizes in
/// [`EnemyFields::ranges`], cut across to the patrol limits widened by box
/// 3's half width; its right and bottom edges are outside it.
pub fn range_mode(
    d: &mut EnemyFields,
    (ex, ey): (i32, i32),
    (cx, cy): (i32, i32),
    screen: bool,
    now: u64,
) -> u8 {
    let reach = i64::from(d.ranges[3].0);
    let lo = i64::from(d.bounds.0) - reach;
    let hi = i64::from(d.bounds.1) + reach;
    let (ex, ey, cx, cy) = (i64::from(ex), i64::from(ey), i64::from(cx), i64::from(cy));
    for i in (0..5).rev() {
        let (hw, hh) = (i64::from(d.ranges[i].0), i64::from(d.ranges[i].1));
        let left = (ex - hw).max(lo);
        let right = (ex + hw).min(hi);
        if cx >= left && cx < right && cy >= ey - hh && cy < ey + hh {
            d.miss_since = None;
            return i as u8;
        }
    }
    if !screen {
        d.miss_since = None;
        return 6;
    }
    let since = *d.miss_since.get_or_insert(now);
    if now.saturating_sub(since) >= OUT_OF_RANGE_MS {
        5
    } else {
        d.mode
    }
}

/// The post-tick: the scheduled call, then the movement step.
pub fn post_tick(w: &mut World, id: ObjectId, class: &Class) {
    let deferred = data_mut(w, id).and_then(|d| d.deferred.take());
    match deferred {
        Some(Deferred::GoIdle) => (class.go_idle)(w, id),
        Some(Deferred::JumpCheck) => states::jump_check(w, id, class),
        Some(Deferred::Dropped) => lift::let_go(w, id),
        Some(Deferred::Human(call)) => human::run_deferred(w, id, class, call),
        None => {}
    }
    motion::integrate(w, id);
}

// ----------------------------------------------------------------------
// Hits

/// The hit rule of human enemies: `attacker` hit `id`.
///
/// A blow carrying fire or ice picks the matching death. The same attack
/// of Claw hits only once (any attacker counts as Claw's current attack),
/// and nothing hits during the hit cooldown. Then, by what hit:
///
/// - Claw himself: during a hit reaction the enemy only goes back Home
///   (no damage); otherwise it takes the damage, shows a hit burst, starts
///   a 300 ms cooldown and flinches high after an uppercut, low otherwise;
/// - a pistol shot: damage, burst, the shot is destroyed, low flinch;
/// - magic or a thrown object: damage, burst, low flinch;
/// - a shot of the level or an explosion: damage, burst, 2000 ms cooldown,
///   low flinch;
/// - a springboard: thrown toward the springboard's target.
///
/// At 0 health or below it dies (again, for a body that is hit).
///
/// Draws: the burst's two, then one more for a high flinch, then the
/// death's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let Some(class) = class_of(w, id) else { return };
    let blow = w.objects.get(attacker).map_or(0, |a| a.special_flags);
    if let Some(d) = data_mut(w, id) {
        if blow & FIRE_BLOW != 0 {
            d.death_anim = FIRE_DEATH_ANIM;
        } else if blow & ICE_BLOW != 0 {
            d.death_anim = ICE_DEATH_ANIM;
        }
    }
    if !accept_hit(w, id) {
        return;
    }
    let now = w.now_ms;
    let Some(kind) = w.objects.get(attacker).map(|a| a.type_flags) else {
        return;
    };
    match kind {
        attacker::CLAW => {
            if matches!(state(w, id), Some(EnemyState::HitLow | EnemyState::HitHigh)) {
                (class.go_home)(w, id);
            } else {
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
                if let Some(d) = data_mut(w, id) {
                    d.cooldown = Timer::new(now, 300);
                }
                let high = w.claw.attack.uppercut;
                states::enter_hit(w, id, high);
            }
        }
        attacker::PISTOL_SHOT => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            w.destroy(attacker);
            states::enter_hit(w, id, false);
        }
        attacker::MAGIC | attacker::THROWN => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            states::enter_hit(w, id, false);
        }
        attacker::LEVEL_SHOT => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            if let Some(d) = data_mut(w, id) {
                d.cooldown = Timer::new(now, 2000);
            }
            states::enter_hit(w, id, false);
        }
        attacker::SPRING => states::enter_spring_jump(w, id, attacker),
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id, DeathKind::Hit, Some(attacker));
    }
}

/// The gate every enemy hit goes through first: the attack of Claw that
/// hit last does not hit again (whatever the attacker is), and a running
/// hit cooldown turns hits away. The attack's number is remembered even
/// when the cooldown turns the hit away.
pub fn accept_hit(w: &mut World, id: ObjectId) -> bool {
    let serial = w.claw.attack.serial;
    let now = w.now_ms;
    let Some(d) = data_mut(w, id) else {
        return false;
    };
    if d.serial_seen == Some(serial) {
        return false;
    }
    d.serial_seen = Some(serial);
    d.cooldown.done(now)
}

/// The attacker's damage comes off the enemy's health while it has some.
pub fn take_damage(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let damage = w.objects.get(attacker).map_or(0, |a| a.damage);
    let o = obj_mut(w, id);
    if o.health > 0 {
        o.health -= damage;
    }
}

/// Depth of a hit burst in front of the enemy.
pub const BURST_DEPTH: i32 = 8501;

/// A hit burst at the enemy's x and at the height of the middle of the
/// attacker's attack box (kept inside the enemy's hit box), each moved by
/// up to 4 px at random. Draws: the height's, then the x's (then the
/// burst's own). The enemy owns the burst, so an enemy that was not drawn
/// in the last frame (off screen) gets no burst and makes no draw for its
/// size (see [`crate::logic::effect::spawn_hit_burst`]).
pub fn hit_burst(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let (x, y, z, hit) = {
        let o = obj(w, id);
        (o.x, o.y, o.z, o.hit_rect)
    };
    let mid = w
        .objects
        .get(attacker)
        .filter(|a| a.attack_rect.is_set())
        .map_or(y, |a| {
            let r = a.attack_rect.translate(a.x, a.y);
            (r.top + r.bottom) / 2
        });
    let mid = if hit.is_set() {
        let hit = hit.translate(x, y);
        mid.clamp(hit.top, hit.bottom.max(hit.top))
    } else {
        mid
    };
    let dy = rand(w, 9) - 4;
    let dx = rand(w, 9) - 4;
    crate::logic::effect::spawn_hit_burst(
        w,
        x + dx,
        mid + dy,
        z.saturating_add(BURST_DEPTH),
        Some(id),
        false,
    );
}

/// `id` attacked `victim`: enemies need nothing here.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

// ----------------------------------------------------------------------
// Death

/// Speed of a killed body, px/s across (away from Claw) and up.
pub const DEATH_LAUNCH: (f64, f64) = (150.0, -700.0);
/// Depth of a killed body: in front of everything.
pub const DEATH_DEPTH: i32 = 8500;
/// Removal delay of a death on spikes or after a throw, ms.
pub const DEATH_DELAY_MS: u64 = 1000;

/// Dies of `kind`, `by` being what dealt the blow. A death on a death
/// tile (kind [`DeathKind::Spikes`], from the movement step) goes through
/// [`goo::enter_goo`], which decides how the enemy dies there; every other
/// kind is [`die_of`].
pub fn die(w: &mut World, id: ObjectId, kind: DeathKind, by: Option<ObjectId>) {
    if kind == DeathKind::Spikes {
        goo::enter_goo(w, id);
    } else {
        die_of(w, id, kind, by);
    }
}

/// Dies of exactly `kind`, `by` being what dealt the blow. Human enemies
/// first drop their loot (once). Then the enemy is dead, busy and harmless
/// to others:
///
/// - killed by a hit, the body is drawn in front of everything and thrown
///   up at 700 px/s and away from Claw at 150 px/s, under gravity and
///   through the tiles; a quarter of the time Claw says a line (for kills
///   by his blows or his pistol only), otherwise the death sound plays;
/// - killed by a landing, on spikes or in goo, the body stays where it is:
///   spikes and goo play the death tile sound; goo removes it at once, the
///   others after a second.
///
/// Draws: the loot's, then one of 0..100 for a kill by a hit, and one of
/// 0..10 when Claw speaks.
pub fn die_of(w: &mut World, id: ObjectId, kind: DeathKind, by: Option<ObjectId>) {
    let human = class_of(w, id).is_some_and(|c| c.human);
    if human && data(w, id).is_some_and(|d| !d.dead) {
        drop_loot(w, id);
    }
    let by_type = by
        .and_then(|a| w.objects.get(a))
        .map_or(0, |a| a.type_flags);
    if let Some(d) = data_mut(w, id) {
        d.death = kind;
        d.busy = true;
        d.cooldown = Timer::default();
    }
    set_state(w, id, EnemyState::Dead);
    obj_mut(w, id).flags |= flags::HARMLESS;
    let now = w.now_ms;
    match kind {
        DeathKind::Hit => {
            let o = obj_mut(w, id);
            o.z = DEATH_DEPTH;
            o.flags |= flags::RESORT;
            if rand(w, 100) < 25 {
                if by_type == attacker::CLAW || by_type == attacker::PISTOL_SHOT {
                    let line = CLAW_KILL_LINES[rand(w, 10) as usize];
                    sound::claw_say(w, line, false);
                }
            } else {
                let s = data(w, id).map_or("", |d| d.death_sound);
                play(w, s);
            }
            let anim = data(w, id).map_or("", |d| d.death_anim);
            set_anim(w, id, anim);
            obj_mut(w, id).physics_type = ptype::DIRECT;
            if let Some(d) = data_mut(w, id) {
                d.z_sort = false;
                d.bounds = UNBOUNDED;
                d.bounds_y = UNBOUNDED;
                d.ay = GRAVITY;
                d.vx = if d.claw_right {
                    -DEATH_LAUNCH.0
                } else {
                    DEATH_LAUNCH.0
                };
                d.vy = DEATH_LAUNCH.1;
            }
        }
        DeathKind::Landing | DeathKind::Spikes | DeathKind::Goo => {
            if kind != DeathKind::Landing {
                play(w, DEATH_TILE_SOUND);
            }
            if let Some(d) = data_mut(w, id) {
                d.frozen = true;
                d.death_timer = Timer::new(now, DEATH_DELAY_MS);
            }
            if kind == DeathKind::Goo {
                w.destroy(id);
            } else {
                obj_mut(w, id).draw_flags |= draw_flags::FLASH;
            }
        }
    }
    if let Some(d) = data_mut(w, id) {
        d.dead = true;
    }
}

/// Drops the enemy's loot: the drop sound once, then one pickup per
/// filled slot through the shared loot drop (fanned out 25 px/s apart, a
/// single one at a random speed across, all thrown up at random; see
/// [`pickup::drop_loot`]). Nothing plays when the enemy carries no loot.
pub fn drop_loot(w: &mut World, id: ObjectId) {
    let Some(loot) = data(w, id).map(|d| d.loot) else {
        return;
    };
    if loot.iter().all(|&c| c == 0) {
        return;
    }
    let (x, y) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    play(w, DROP_SOUND);
    pickup::drop_loot(w, x, y, loot);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_round_trip_and_are_fixed() {
        for s in EnemyState::ALL {
            assert_eq!(EnemyState::from_id(s as i32), Some(s));
        }
        assert_eq!(EnemyState::Idle as i32, 2000);
        assert_eq!(EnemyState::Pause as i32, 2007);
        assert_eq!(EnemyState::Dropped as i32, 2021);
        assert_eq!(EnemyState::from_id(0), None);
        assert_eq!(EnemyState::from_id(1999), None);
        assert_eq!(EnemyState::from_id(2023), None, "a free number");
        assert_eq!(EnemyState::from_id(2090), None);
        assert_eq!(EnemyState::from_id(i32::MIN), None);
        assert_eq!(EnemyState::from_id(i32::MAX), None);
        assert_eq!(EnemyState::Strike.name(), "Strike");
        // The first level's states are numbered without gaps.
        for (i, s) in EnemyState::ALL[..22].iter().enumerate() {
            assert_eq!(*s as i32, 2000 + i as i32);
        }
        assert!(EnemyState::ALL.is_sorted_by_key(|s| *s as i32));
    }

    #[test]
    fn factors_and_smarts() {
        assert_eq!(scaled(10, DAMAGE_FACTOR), 10);
        assert_eq!(scaled(7, 1.5), 10);
        assert_eq!(smarts(50), 99);
        assert_eq!(smarts(25), 99);
        assert_eq!(smarts(10), 99);
    }

    fn officer_ranges() -> EnemyFields {
        EnemyFields {
            ranges: [(100, 40); 5],
            ..EnemyFields::default()
        }
    }

    #[test]
    fn range_modes() {
        let mut d = officer_ranges();
        assert_eq!(range_mode(&mut d, (100, 100), (150, 120), true, 0), 4);
        d.mode = 4;
        // Out of range on screen: the mode holds for 500 ms, then is 5.
        assert_eq!(range_mode(&mut d, (100, 100), (300, 100), true, 1000), 4);
        assert_eq!(range_mode(&mut d, (100, 100), (300, 100), true, 1499), 4);
        assert_eq!(range_mode(&mut d, (100, 100), (300, 100), true, 1500), 5);
        // Off screen: 6 at once.
        assert_eq!(range_mode(&mut d, (100, 100), (300, 100), false, 1500), 6);
        // The box excludes its right and bottom edges.
        assert_eq!(range_mode(&mut d, (100, 100), (200, 100), false, 0), 6);
        assert_eq!(range_mode(&mut d, (100, 100), (0, 60), false, 0), 4);
        assert_eq!(range_mode(&mut d, (100, 100), (0, 140), false, 0), 6);
    }

    #[test]
    fn going_off_screen_cancels_the_out_of_range_wait() {
        let mut d = officer_ranges();
        d.mode = 4;
        assert_eq!(range_mode(&mut d, (100, 100), (300, 100), true, 1000), 4);
        assert_eq!(d.miss_since, Some(1000));
        // Off screen: mode 6, and the wait is cancelled.
        d.mode = range_mode(&mut d, (100, 100), (300, 100), false, 2000);
        assert_eq!(d.mode, 6);
        assert_eq!(d.miss_since, None);
        // Back on screen long after the first miss, the wait starts again
        // and the mode stays 6 for 500 ms (a town guard back on screen at
        // 172962 ms is still in mode 6 when it reaches its limit two ticks
        // later).
        assert_eq!(range_mode(&mut d, (100, 100), (300, 100), true, 172_962), 6);
        assert_eq!(d.miss_since, Some(172_962));
        assert_eq!(range_mode(&mut d, (100, 100), (300, 100), true, 172_998), 6);
        assert_eq!(range_mode(&mut d, (100, 100), (300, 100), true, 173_461), 6);
        assert_eq!(range_mode(&mut d, (100, 100), (300, 100), true, 173_462), 5);
    }

    #[test]
    fn range_boxes_nest_from_the_inside_out() {
        // The soldier's boxes: fire at 400 and 300, strike at 80.
        let mut d = EnemyFields {
            ranges: [(400, 50), (300, 50), (300, 50), (80, 50), (80, 50)],
            ..EnemyFields::default()
        };
        let at = |d: &mut EnemyFields, cx: i32| range_mode(d, (1000, 500), (cx, 500), false, 0);
        assert_eq!(at(&mut d, 1050), 4);
        assert_eq!(at(&mut d, 920), 4);
        assert_eq!(at(&mut d, 1080), 2);
        assert_eq!(at(&mut d, 1299), 2);
        assert_eq!(at(&mut d, 1300), 0);
        assert_eq!(at(&mut d, 600), 0);
        assert_eq!(at(&mut d, 1400), 6);
        // The patrol limits, widened by box 3's half width, cut the boxes.
        d.bounds = (950, 1050);
        assert_eq!(at(&mut d, 1129), 2);
        assert_eq!(at(&mut d, 1130), 6);
        assert_eq!(at(&mut d, 870), 2);
        assert_eq!(at(&mut d, 869), 6);
    }

    #[test]
    fn timers() {
        let t = Timer::new(100, 300);
        assert!(!t.done(399));
        assert!(t.done(400));
        assert!(Timer::default().done(0));
    }
}
