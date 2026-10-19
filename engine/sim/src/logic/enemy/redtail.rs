//! RedTail, the pirate captain who ends the thirteenth level on a log
//! stage over the water, with his stakes, his wind, his knives and his
//! flares ([`crate::logic::boss`]).
//!
//! He is a human enemy on the shared boss staging
//! ([`crate::logic::boss_stage`]): the warp takes Claw into his arena, the
//! staged introduction freezes Claw, and the fight starts when the flag
//! reaches 2. His arena has two levels: a ledge on the right, where he
//! starts, and the sand below it. Before every attack he asks where Claw
//! stands: on his own level he fires his flare gun, slides along the sand
//! at Claw or kicks him; on the other level he throws knives, leaps down
//! or up to Claw's level, or (from the ledge, Claw on the sand) raises
//! the wind that drives Claw into the stakes at the left wall.
//!
//! He has [`HEALTH`] health, each damaging blow costing him what Claw's
//! blow deals ([`BLOW_DAMAGE`] for the plain sword). Claw's blows are
//! counted: every third counted blow that finds him free makes him leap
//! to the other level instead, unhurt. The count starts at
//! [`HIT_COUNTER_START`] on the first arena step, the value the original
//! game shows there (its set-up never writes the count, which so reads
//! what the memory held); a count of 1 at the start means the second
//! counted blow after an intro skip already sends him away. Matching the
//! original here is a stand-in decision.
//!
//! What his helpers share with him lives in the RedTail link
//! ([`RedTailLink`], reached through [`World::redtail_link`] and
//! [`World::redtail_link_mut`]): his object while he is there and the
//! wind flag he raises and the wind reads.
//!
//! # The rules
//!
//! "Low" is a height below the line between the levels ([`LOW_Y`]): Claw
//! is low when his y is greater than it, RedTail when he also stands. Two
//! figures are on the same level when both are low or neither is.
//!
//! - Built: the link's RedTail, and the current boss from his first logic
//!   call on (built by a warp between two ticks, he keeps his own health
//!   through the tick that follows it); his body box [`BODY_RECT`] for
//!   moving and touching, [`HIT_RECT`] for being hit;
//!   [`HEALTH`] health and [`DAMAGE`] (times the difficulty factors); the
//!   range boxes [`RANGES`]; no patrol limits until his first landing;
//!   never liftable; going by sight; always active. He drops onto the
//!   ledge and waits in `Startup`, his idle pose playing, his body still
//!   hurting a Claw who touches it.
//! - Startup: when the fight flag reaches 2, the reset (below). A Claw who
//!   skipped the introduction leaves him there at full health for as long
//!   as nothing touches him; a blow, a block, the hop back or Claw's
//!   respawn wakes him.
//! - The reset (the start of the fight and every respawn of Claw): once
//!   defeated he only goes Home. Otherwise he is put back where he was
//!   placed and makes one movement step in the air there, then falls from
//!   rest; his health is full again, the wind flag is lowered, the blow
//!   count is 0, his death's end may come again, and he goes Home.
//! - Going Home: stopped, his idle pose, a decision in [`HOME_MS`];
//!   unless defeated, he takes part in collisions and can be hit again.
//! - Home: nothing once defeated. Otherwise he faces Claw; once his wait
//!   is over he decides by Claw's range mode: far away (modes 0, 5, 6) he
//!   runs at him; in modes 1 to 3 a draw of 0..=100 below 90 makes the
//!   ranged choice, otherwise he changes level; within 100 px (mode 4) a
//!   draw of 0..=100 picks, on the same level, the kick (below 33), the
//!   slide (below 66) or a change of level, and on the other level the
//!   knives (below 45), the wind (below 80) or a change of level.
//! - The ranged choice, once his attack cooldown is over (otherwise Home
//!   again): a draw of 0..=100, he faces Claw, then by mode: 0 and 1 the
//!   knives (below 40), the gun (below 80), the slide (below 90) or the
//!   wind; 2 and 3 the knives (below 20), the gun (below 40), the slide
//!   (below 80) or the wind; 4 the kick (below 70) or the wind.
//! - Each attack first checks the levels: the knives fly only between the
//!   levels (on the same level they turn into the kick within 100 px and
//!   the gun otherwise); the gun fires only on Claw's level, RedTail
//!   jumping there first; the slide only on the sand, at a Claw on the
//!   sand (otherwise the ranged choice again), RedTail jumping down or
//!   running at Claw first; the kick only within 100 px on Claw's level,
//!   RedTail jumping or running there first; the wind only from the
//!   ledge at a Claw on the sand while it is not already blowing,
//!   RedTail jumping up first (otherwise the ranged choice again).
//! - Running at Claw ([`RUN_SPEED`]): each tick, standing exactly on his
//!   patrol limit on Claw's side makes the attack he runs for; then he
//!   faces Claw and by mode runs on (0 to 3, 5), makes the attack (4) or
//!   paces (6).
//! - The knives: three at once at a Claw on the other level when the
//!   animation ends ([`super::super::boss::knife`]), the second
//!   [`KNIFE_SPREAD`] px/s faster across, the third also that much slower
//!   down (or faster up); a cooldown of [`COOLDOWN_MS`] plus a draw of
//!   0..=500, and a taunt a quarter of the time.
//! - The gun: on the first tick showing [`GUN_FRAME`] a flare leaves
//!   [`GUN_MUZZLE`] in front of him at [`super::super::boss::flare::SPEED`]
//!   ([`super::super::boss::flare`]); Home when the animation ends.
//! - The kick: while the picture [`KICK_FRAME`] shows, [`KICK_RECT`] is
//!   his attack box; Home when the animation ends.
//! - The slide ([`SLIDE_SPEED`]) toward a point [`SLIDE_PAST`] px beyond
//!   Claw: from the picture [`SLIDE_FRAME`] on, [`SLIDE_RECT`] is his
//!   attack box; Home once he has passed Claw or the point, or stands
//!   exactly on his limit on Claw's side.
//! - The wind: the link's wind flag goes up (the wind object blows), he
//!   faces right and says a line; when the animation ends he goes Home
//!   with a wait of [`WIND_REST_MS`] plus a draw of 0..=500.
//! - The jumps between the levels: only while standing, busy, moving
//!   without tile tests on the slowest arc to [`JUMP_UP_TARGET`] (up) or
//!   to a point of the sand at x [`JUMP_DOWN_X`] plus a draw of 0..=300
//!   (down), his jump sound; up, he takes the tiles again above
//!   [`JUMP_UP_FLY_Y`]; down, below [`JUMP_DOWN_FLY_Y`]. Landed: the
//!   landing (stopped, patrol limits [`LANDED_BOUNDS`], free, a wait of
//!   [`LANDED_HOME_MS`]), then the attack he jumped for.
//! - Hits (see [`on_hit`]): Claw's blows go through the count; pistol
//!   shots (used up) and magic make him block, a one-tick pose; an
//!   explosion makes him hop back when free; nothing else touches him. At
//!   0 health or below he dies.
//! - Death: on the ledge he first jumps down, dying on the landing. On the
//!   sand his fall plays, he hurts nothing and nothing hits him; he says
//!   his last line once no line of his runs; when the fall and the line
//!   are over, with Claw alive, Claw is held and the gem rises out of him
//!   ([`World::boss_defeated`]) toward [`GEM_TARGET`] (his record's
//!   speed fields), and he is defeated. He is never removed.
//! - Claw's respawn: the reset, in the respawn's own update passes; his
//!   knives and flares still flying are gone with it.
//!
//! # State numbers
//!
//! The thirteenth level's block [`STATE_BLOCK`] (2300 to 2319) holds his
//! new names from 2300 ([`STATES`]: the knives, the kick, the slide, the
//! gun, the wind and the two jumps) and the Chameleon's from 2310
//! ([`super::chameleon::STATES`]). He reuses the numbers of names that
//! already have one: `Startup`, `Home`, `FastAdvance`, `Block`, `HitHigh`,
//! `HitLow` and `Dead`.
//!
//! # Random draws, in order
//!
//! - each call: the shared pre-tick's (frame pulse ticks only);
//! - Home, once his wait is over in modes 1 to 4: one of 0..=100;
//! - the ranged choice, once the cooldown is over: one of 0..=100;
//! - the knives, the gun and the slide: one of 0..=500 (the cooldown),
//!   one of 0..=100, then below 25 one of 0..4 (the taunt);
//! - the kick: one of 0..=500;
//! - the wind: one of 0..2 (the line); at its end one of 0..=500;
//! - the jump down: one of 0..=300, even when it does not start;
//! - the block: one of 0..=100, then below 33 one of 0..3;
//! - a damaging blow (in the collision pass): the hit burst's two, then
//!   one of 0..4 for a high flinch.
//!
//! # Defaults taken
//!
//! - Standing, his exact height cycles below his whole pixel as gravity
//!   pulls him and the floor holds him (the fourth level's boss's rule,
//!   [`super::wolvington::next_stand_fraction`]); he leaves the floor
//!   from that exact height. Before his first time in the air the speed
//!   his drop onto the ledge left him with keeps shrinking toward rest
//!   ([`next_stand_speed`]); the start of the fight's movement step in the
//!   air uses it.

use std::ops::RangeInclusive;

use super::human::{self, BossFields, HumanData};
use super::states::{enter_hit, hit_tick};
use super::{
    Body, Class, ClassAnims, ClassSounds, EnemyData, EnemyFields, EnemyState, HIT_LOW_SECOND,
    Handler, Timer, UNBOUNDED, accept_hit, attacker, build, data, data_mut, exact_x, face_claw,
    facing_rect, hit_burst, human_fields, motion, obj, obj_mut, play, rand, say, scaled, set_anim,
    set_facing, set_state, take_damage,
};
use crate::logic::boss::{flare, knife};
use crate::logic::world_boss::fight;
use crate::logic::{ObjectData, World, boss_stage, pickup, sound};
use crate::object::{ObjectId, Rect, engine_state, flags, types};
use crate::physics::ptype;
use crate::player::state as claw_state;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTail"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_REDTAIL";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_REDTAIL_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers reserved for the thirteenth level's families:
/// RedTail's from 2300 ([`STATES`]) and the Chameleon's from 2310
/// ([`super::chameleon::STATES`]); the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2300..=2319;

/// His new state names, in number order from the start of
/// [`STATE_BLOCK`].
pub const STATES: [EnemyState; 7] = [
    EnemyState::KnifeAttack,
    EnemyState::KickAttack,
    EnemyState::SlideAttack,
    EnemyState::GunAttack,
    EnemyState::WindAttack,
    EnemyState::JumpUp,
    EnemyState::JumpDown,
];

/// The staging objects of his fight.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// His health, before the difficulty factor.
pub const HEALTH: i32 = 100;
/// What one damaging blow of Claw's plain sword costs him (twenty kill
/// him): the blow's own damage.
pub const BLOW_DAMAGE: i32 = 5;
/// What touching him, his kick and his slide cost Claw, before the
/// difficulty factor.
pub const DAMAGE: i32 = 10;
/// The count of Claw's blows on the first arena step (see the module
/// documentation).
pub const HIT_COUNTER_START: u32 = 1;
/// Every this many counted blows that find him free, he leaps away.
pub const ESCAPE_EVERY: u32 = 3;
/// His speed running at Claw, px/s.
pub const RUN_SPEED: i32 = 250;
/// His speed sliding at Claw, px/s.
pub const SLIDE_SPEED: i32 = 280;
/// How far beyond Claw the slide aims, px.
pub const SLIDE_PAST: i32 = 150;
/// His hit box.
pub const HIT_RECT: Rect = Rect::new(-31, -51, 38, 62);
/// His movement and contact box: his first picture's.
pub const BODY_RECT: Rect = Rect::new(-37, -36, 54, 55);
/// The slide's box, facing left.
pub const SLIDE_RECT: Rect = Rect::new(-52, -17, 51, 41);
/// The kick's box, facing left.
pub const KICK_RECT: Rect = Rect::new(-56, -28, 0, 22);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [
    (2000, 1000),
    (600, 1000),
    (300, 1000),
    (200, 1000),
    (100, 1000),
];
/// Kinds of object that can hit him: Claw, his pistol shots, explosions
/// and the level's own shots, and his magic. His own knives and flares
/// pass through him.
pub const HIT_MASK: u32 = 0x300_0022;
/// Below this height (greater y) a figure stands on the sand ("low"),
/// px.
pub const LOW_Y: i32 = 1920;
/// The patrol limits a landing sets (min, max).
pub const LANDED_BOUNDS: (i32, i32) = (37100, 37900);
/// The lowest height a landing allows him, px.
pub const LANDED_FLOOR: i32 = 2150;
/// A landing lower than this puts him here, px.
pub const LANDED_CLAMP: i32 = 2095;
/// Lower than this he is caught (the fall guard), px.
pub const FALL_GUARD_Y: i32 = 2090;
/// Where the fall guard puts him, px.
pub const FALL_GUARD_TO: i32 = 2085;
/// Where the jump up aims.
pub const JUMP_UP_TARGET: (i32, i32) = (37500, 1770);
/// Above this height the jump up takes the tiles again, px.
pub const JUMP_UP_FLY_Y: i32 = 1845;
/// The leftmost x the jump down aims at; a draw of 0..=300 is added.
pub const JUMP_DOWN_X: i32 = 37350;
/// Spread of the jump down's aim, draws of 0..`JUMP_DOWN_SPREAD`.
pub const JUMP_DOWN_SPREAD: u32 = 301;
/// The height the jump down aims at, px.
pub const JUMP_DOWN_Y: i32 = 2150;
/// Below this height the jump down takes the tiles again, px.
pub const JUMP_DOWN_FLY_Y: i32 = 2000;
/// The speed a jump up starts from before its arc is set, px/s.
pub const JUMP_UP_SPEED: f64 = -600.0;
/// The speed a jump down starts from before its arc is set, px/s.
pub const JUMP_DOWN_SPEED: f64 = 250.0;
/// Where the gem he leaves flies to.
pub const GEM_TARGET: (i32, i32) = (37509, 1750);

/// His wait at Home before a decision, ms.
pub const HOME_MS: u64 = 500;
/// His wait after a landing, ms.
pub const LANDED_HOME_MS: u64 = 750;
/// His wait after the wind, ms; a draw of 0..=500 is added.
pub const WIND_REST_MS: u64 = 2000;
/// His attack cooldown after the knives, the gun, the slide and the
/// kick, ms; a draw of 0..=500 is added.
pub const COOLDOWN_MS: u64 = 1000;
/// The spread of the cooldowns, draws of 0..`SPREAD`.
pub const SPREAD: u32 = 501;
/// The hit cooldown after a damaging blow, ms.
pub const HIT_COOLDOWN_MS: u64 = 300;

/// Where the knives leave him: across, facing right (mirrored facing
/// left), and down, px.
pub const KNIFE_MUZZLE: (i32, i32) = (35, 4);
/// The knives' speed across (in his facing) and up or down (toward
/// Claw's level), px/s.
pub const KNIFE_SPEED: (i32, i32) = (250, 300);
/// What the second and third knives add (across, to the left) and the
/// third (up), px/s.
pub const KNIFE_SPREAD: i32 = 150;
/// Where the flare leaves the gun: across, facing right (mirrored facing
/// left), and down, px.
pub const GUN_MUZZLE: (i32, i32) = (66, -9);
/// The picture on which the flare leaves.
pub const GUN_FRAME: i32 = 209;
/// The kick's striking picture.
pub const KICK_FRAME: i32 = 402;
/// The slide's sliding picture.
pub const SLIDE_FRAME: i32 = 302;

/// His decision pose.
pub const IDLE_ANIM: &str = "LEVEL_REDTAIL_IDLE";
/// His body falling after death.
pub const DEATH_ANIM: &str = "LEVEL_REDTAIL_KILLFALL";
/// Running at Claw.
pub const RUN_ANIM: &str = "LEVEL_REDTAIL_FASTADVANCE";
/// Both jumps.
pub const JUMP_ANIM: &str = "LEVEL_REDTAIL_JUMP";
/// The knives thrown down.
pub const KNIVES_DOWN_ANIM: &str = "LEVEL_REDTAIL_STRIKE2";
/// The knives thrown up.
pub const KNIVES_UP_ANIM: &str = "LEVEL_REDTAIL_STRIKE3";
/// The flare gun.
pub const GUN_ANIM: &str = "LEVEL_REDTAIL_STRIKE4";
/// The wind.
pub const WIND_ANIM: &str = "LEVEL_REDTAIL_STRIKE6";
/// The slide.
pub const SLIDE_ANIM: &str = "LEVEL_REDTAIL_STRIKE7";
/// The kick.
pub const KICK_ANIM: &str = "LEVEL_REDTAIL_STRIKE8";
/// The block.
pub const BLOCK_ANIM: &str = "LEVEL_REDTAIL_BLOCK";
/// The high flinch.
pub const HIT_HIGH_ANIM: &str = "LEVEL_REDTAIL_HITHIGH";
/// The low flinch.
pub const HIT_LOW_ANIM: &str = "LEVEL_REDTAIL_HITLOW";

/// His taunts with the knives, the gun and the slide.
pub const TAUNTS: [&str; 4] = [
    "LEVEL_REDTAIL_01040022",
    "LEVEL_REDTAIL_01040023",
    "LEVEL_REDTAIL_01040025",
    "LEVEL_REDTAIL_01040001",
];
/// Chance in 101 of a taunt.
pub const TAUNT_CHANCE: i32 = 25;
/// His lines as he raises the wind.
pub const WIND_LINES: [&str; 2] = ["LEVEL_REDTAIL_01040012", "LEVEL_REDTAIL_01040020"];
/// His block's sound.
pub const BLOCK_SOUND: &str = "LEVEL_REDTAIL_BLOCK2";
/// His lines as he blocks.
pub const BLOCK_LINES: [&str; 3] = [
    "LEVEL_REDTAIL_01040004",
    "LEVEL_REDTAIL_01040002",
    BLOCK_SOUND,
];
/// Chance in 101 of a block line.
pub const BLOCK_LINE_CHANCE: i32 = 33;
/// His last line.
pub const DEATH_LINE: &str = "LEVEL_REDTAIL_01040021";
/// The low flinch's sound.
pub const HIT_LOW_SOUND: &str = "LEVEL_REDTAIL_01040006";
/// The high flinch's sound.
pub const HIT_HIGH_SOUND: &str = "LEVEL_REDTAIL_01040007";
/// The first knife's sound.
pub const KNIVES_SOUND: &str = "LEVEL_REDTAIL_3KNIVES";
/// The shot's sound.
pub const GUN_SOUND: &str = "LEVEL_REDTAIL_FLAREGUN3";
/// The slide's sound.
pub const SLIDE_SOUND: &str = "LEVEL_REDTAIL_RTSLIDE";
/// The jumps' sound.
pub const JUMP_SOUND: &str = "LEVEL_REDTAIL_RTJUMP";

/// Ticks after one of his lines has run out during which it still holds
/// the enemies' speaking slot for his death: the game frees the slot two
/// helper calls after the line's time is up.
pub const SPEAKER_TAIL: u32 = 2;

/// The speed his drop onto the ledge leaves him with when he is built,
/// px/s (upwards): one tick standing later it is the speed the original
/// shows on the first arena step (see [`next_stand_speed`]).
pub const BUILT_STAND_SPEED: f64 = -(103.460_137 * 103.460_137) / 72.0 - 18.0;

/// The RedTail class.
pub static CLASS: Class = Class {
    name: "RedTail",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home,
    go_idle: go_home,
    carry: None,
    pick_lift_voice: super::nothing,
    lift_roll: false,
    anims: ClassAnims {
        fast_advance: RUN_ANIM,
        hit_low: HIT_LOW_ANIM,
        hit_high: HIT_HIGH_ANIM,
        jump: JUMP_ANIM,
        ..ClassAnims::NONE
    },
    sounds: ClassSounds {
        strike: "",
        hit_low: [HIT_LOW_SOUND, HIT_LOW_SECOND],
        hit_high: HIT_HIGH_SOUND,
    },
};

/// What RedTail's helpers share with him (see the module documentation).
/// It lasts the whole level and starts as [`RedTailLink::default`] leaves
/// it: no RedTail, the wind flag down.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RedTailLink {
    /// RedTail's object, while he is there.
    pub redtail: Option<ObjectId>,
    /// The wind flag: raised by his wind attack, read by the wind, which
    /// clears it when its blow ends; his respawn reset clears it too.
    pub wind: bool,
}

/// A call made in the post-tick after a hit, before his movement step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Later {
    /// The third counted blow found him free: he leaps to the other level.
    Escape,
    /// An explosion found him free: he hops back.
    HopBack,
}

/// RedTail's data.
#[derive(Debug, Clone, PartialEq)]
pub struct RedTailData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// The count of Claw's blows (see [`HIT_COUNTER_START`]).
    pub hits: u32,
    /// The attack he makes once a run or a jump is over: 0 Home, 1 the
    /// wind, 2 the gun, 3 the knives, 4 the slide, 5 the kick, 6 his death.
    pub pending: u8,
    /// His wait at Home.
    pub home: Timer,
    /// His attack cooldown.
    pub cooldown: Timer,
    /// Where the slide aims, px.
    pub slide_to: i32,
    /// The knives or the flare of the attack have left.
    pub fired: bool,
    /// Where the knives or the flare leave him (across, down), px.
    pub muzzle: (i32, i32),
    /// The speed of the knives or the flare, px/s.
    pub shot_speed: (i32, i32),
    /// His last line has been asked for.
    pub line_said: bool,
    /// His death's end has been handled (it may come again after a
    /// reset).
    pub death_done: bool,
    /// He is defeated: the gem has risen.
    pub defeated: bool,
    /// The call to make in the post-tick.
    pub later: Option<Later>,
    /// How far below his whole pixel he stands, px; 0 in the air.
    pub stand_y: f64,
    /// He has been in the air: [`Self::stand_y`] changes from tick to tick.
    pub falls: bool,
    /// His speed up or down while he stands before his first time in the
    /// air, px/s (see [`next_stand_speed`]).
    pub stand_vy: f64,
    /// Ticks of his death since a line of his last ran (see
    /// [`SPEAKER_TAIL`]).
    pub quiet: u32,
    /// He has been named the current boss (on his first logic call).
    pub named: bool,
}

impl Default for RedTailData {
    /// Data with the blow count at its start, [`HIT_COUNTER_START`].
    fn default() -> RedTailData {
        RedTailData {
            enemy: EnemyData::default(),
            hits: HIT_COUNTER_START,
            pending: 0,
            home: Timer::default(),
            cooldown: Timer::default(),
            slide_to: 0,
            fired: false,
            muzzle: (0, 0),
            shot_speed: (0, 0),
            line_said: false,
            death_done: false,
            defeated: false,
            later: None,
            stand_y: 0.0,
            falls: false,
            stand_vy: BUILT_STAND_SPEED,
            quiet: 0,
            named: false,
        }
    }
}

impl World<'_> {
    /// The RedTail link (see [`RedTailLink`]).
    pub fn redtail_link(&self) -> &RedTailLink {
        &self.out.redtail
    }

    /// The RedTail link, mutable (see [`World::redtail_link`]).
    pub fn redtail_link_mut(&mut self) -> &mut RedTailLink {
        &mut self.out.redtail
    }
}

/// RedTail's own data of `id`.
pub fn redtail_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a RedTailData> {
    match &w.objects.get(id)?.data {
        ObjectData::RedTail(d) => Some(d),
        _ => None,
    }
}

/// RedTail's own data of `id`, mutable.
pub fn redtail_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut RedTailData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::RedTail(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Startup => Some(startup_tick),
        EnemyState::Home => Some(home_tick),
        EnemyState::FastAdvance => Some(run_tick),
        EnemyState::KnifeAttack => Some(knives_tick),
        EnemyState::KickAttack => Some(kick_tick),
        EnemyState::SlideAttack => Some(slide_tick),
        EnemyState::GunAttack => Some(gun_tick),
        EnemyState::WindAttack => Some(wind_tick),
        EnemyState::JumpUp => Some(jump_up_tick),
        EnemyState::JumpDown => Some(jump_down_tick),
        EnemyState::Block => Some(block_tick),
        EnemyState::HitHigh | EnemyState::HitLow => Some(hit_tick),
        EnemyState::Dead => Some(death_tick),
        _ => None,
    }
}

// ----------------------------------------------------------------------
// Where things stand

/// True when Claw stands below the line between the levels.
pub fn claw_low(w: &World) -> bool {
    w.claw_object().is_some_and(|c| c.y > LOW_Y)
}

/// True when RedTail stands below the line between the levels.
pub fn redtail_low(w: &World, id: ObjectId) -> bool {
    let o = obj(w, id);
    o.physics_type == ptype::STAND && o.y > LOW_Y
}

fn same_level(w: &World, id: ObjectId) -> bool {
    claw_low(w) == redtail_low(w, id)
}

fn mode(w: &World, id: ObjectId) -> u8 {
    data(w, id).map_or(6, |d| d.mode)
}

fn claw_right(w: &World, id: ObjectId) -> bool {
    data(w, id).is_some_and(|d| d.claw_right)
}

fn defeated(w: &World, id: ObjectId) -> bool {
    redtail_data(w, id).is_some_and(|d| d.defeated)
}

fn faces_right(w: &World, id: ObjectId) -> bool {
    super::faces_right(obj(w, id))
}

fn standing(w: &World, id: ObjectId) -> bool {
    obj(w, id).physics_type == ptype::STAND
}

fn set_vx(w: &mut World, id: ObjectId, vx: f64) {
    if let Some(d) = data_mut(w, id) {
        d.vx = vx;
    }
}

fn set_busy(w: &mut World, id: ObjectId, busy: bool) {
    if let Some(d) = data_mut(w, id) {
        d.busy = busy;
    }
}

/// A cooldown of `base` ms plus a draw of 0..=500.
fn spread_timer(w: &mut World, base: u64) -> Timer {
    let ms = base + rand(w, SPREAD) as u64;
    Timer::new(w.now_ms, ms)
}

/// A taunt a quarter of the time. Draws: one of 0..=100, then below 25
/// one of 0..4.
fn taunt(w: &mut World, id: ObjectId) {
    if rand(w, 101) < TAUNT_CHANCE {
        let line = TAUNTS[rand(w, 4) as usize];
        say(w, id, line);
    }
}

// ----------------------------------------------------------------------
// The logic call

/// One logic call (see the module documentation): the reset when Claw
/// has respawned, then the human logic call with the post-tick call of a
/// hit made before his movement step, and the exact height he stands at.
/// The first call names him the current boss. Being removed, he is no
/// longer the current boss nor named in the link.
pub fn tick(w: &mut World, id: ObjectId) {
    let raw = obj(w, id).state;
    if raw == engine_state::DESTROYING {
        if w.current_boss() == Some(id) {
            w.set_current_boss(None);
        }
        if w.redtail_link().redtail == Some(id) {
            w.redtail_link_mut().redtail = None;
        }
    }
    // Built by a warp's set-up between two ticks, he is the current boss
    // from his first logic call on: a boss health set-up made between
    // two ticks reaches him after that tick, which shows his own health.
    if raw != engine_state::DESTROYING
        && let Some(d) = redtail_mut(w, id)
        && !d.named
    {
        d.named = true;
        w.set_current_boss(Some(id));
    }
    if EnemyState::from_id(raw).is_none() {
        super::tick(w, id, &CLASS);
        return;
    }
    if claw_respawned(w, id) {
        let attempt = w.attempt();
        if let Some(b) = data_mut(w, id).and_then(|d| d.human.boss.as_mut()) {
            b.attempt = attempt;
        }
        reset(w, id);
        // Everything thrown or fired goes with the life Claw lost.
        crate::logic::projectile::purge(w.objects);
    }
    super::pre_tick(w, id, &CLASS);
    pre_tick(w, id);
    let s = super::state(w, id).unwrap_or_default();
    if let Some(h) = handler(s).or_else(|| super::states::shared_handler(s, true)) {
        h(w, id);
    }
    let later = redtail_mut(w, id).and_then(|d| d.later.take());
    match later {
        Some(Later::Escape) => escape(w, id),
        Some(Later::HopBack) => hop_back(w, id),
        None => {}
    }
    let stood = standing(w, id);
    let before = exact_x(w, id);
    let dt = w.dt_ms;
    super::post_tick(w, id, &CLASS);
    let walked = exact_x(w, id) != before;
    settle(w, id, stood, walked, dt);
}

/// True once per death of Claw, from his respawn on: his attempt number
/// has moved on since the last reset and he is no longer dying.
fn claw_respawned(w: &World, id: ObjectId) -> bool {
    let dying = w.claw_object().is_none_or(|c| {
        matches!(
            c.state,
            claw_state::DEATH_FALL | claw_state::DEATH | claw_state::INIT
        )
    });
    let attempt = w.attempt();
    !dying && data(w, id).is_some_and(|d| d.human.boss.is_some_and(|b| b.attempt != attempt))
}

/// After his movement step: standing on from the last tick, his exact
/// height sinks on below his pixel once he has been in the air (and the
/// speed he was built with shrinks before that), unless he moved across,
/// which puts him back on his pixel; landing, he stands
/// [`super::wolvington::LANDED_FRACTION`] below it; in the air nothing.
/// A step in no game time changes nothing.
fn settle(w: &mut World, id: ObjectId, stood: bool, walked: bool, dt_ms: u32) {
    if dt_ms == 0 {
        return;
    }
    let stands = standing(w, id);
    let (ay, vy) = data(w, id).map_or((super::GRAVITY, 0.0), |d| (d.ay, d.vy));
    if let Some(d) = redtail_mut(w, id) {
        if stood && stands {
            if walked {
                d.stand_y = 0.0;
            } else if d.falls {
                d.stand_y = super::wolvington::next_stand_fraction(d.stand_y, ay);
            } else {
                d.stand_vy = next_stand_speed(d.stand_vy, ay);
            }
        } else if stands {
            d.stand_y = super::wolvington::LANDED_FRACTION;
            d.stand_vy = 0.0;
        } else {
            d.stand_y = 0.0;
            d.stand_vy = vy;
            d.falls = true;
        }
    }
}

/// The speed up or down of a figure that stands from the speed `v` it
/// had a tick ago: the tick's fall from `v` (`(v + a dt / 2) dt`, upwards
/// while `v` points up hard enough) is undone by the floor, which leaves
/// the speed that move's height gives from rest, upwards; a move down
/// leaves none.
pub fn next_stand_speed(v: f64, ay: f64) -> f64 {
    let dt = super::TICK_S;
    let rise = -(v + 0.5 * ay * dt) * dt;
    if rise > 0.0 {
        -(2.0 * ay * rise).sqrt()
    } else {
        0.0
    }
}

/// He leaves the floor from exactly where he stands: across, with the
/// part of a pixel his last move left; up and down, the part of a pixel he
/// stands below his whole pixel. Returns that exact position.
fn leave_floor(w: &mut World, id: ObjectId) -> (f64, f64) {
    let (x, y) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    let fx = exact_x(w, id) - f64::from(x);
    let fy = redtail_data(w, id).map_or(0.0, |d| d.stand_y);
    if let Some(d) = data_mut(w, id) {
        d.carry = (fx, fy);
        d.moved_to = Some((x, y));
    }
    if let Some(d) = redtail_mut(w, id) {
        d.falls = true;
    }
    (f64::from(x) + fx, f64::from(y) + fy)
}

// ----------------------------------------------------------------------
// Building and the reset

/// Builds RedTail (see the module documentation). Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    let start = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    let full_health = scaled(HEALTH, super::HEALTH_FACTOR);
    let fields = EnemyFields {
        ranges: RANGES,
        bounds: UNBOUNDED,
        contact_rect: BODY_RECT,
        strike_rect: Rect::UNSET,
        sight: true,
        may_lift: false,
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
        move_rect: BODY_RECT,
        hit_rect: HIT_RECT,
        attack_rect: BODY_RECT,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HIT_MASK,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    o.flags |= flags::ALWAYS_ACTIVE;
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::RedTail(RedTailData {
            enemy,
            ..RedTailData::default()
        });
    }
    w.redtail_link_mut().redtail = Some(id);
    set_state(w, id, EnemyState::Startup);
    set_anim(w, id, IDLE_ANIM);
}

/// The reset (see the module documentation). Draws nothing.
pub fn reset(w: &mut World, id: ObjectId) {
    if defeated(w, id) {
        go_home(w, id);
        return;
    }
    let Some(b) = data(w, id).and_then(|d| d.human.boss) else {
        return;
    };
    // Standing before his first time in the air, the speed his drop left
    // him with is still his.
    let lift = redtail_data(w, id).filter(|r| !r.falls).map(|r| r.stand_vy);
    {
        let o = obj_mut(w, id);
        o.physics_type = ptype::DIRECT;
        o.x = b.start.0;
        o.y = b.start.1;
    }
    if let Some(v) = lift
        && let Some(d) = data_mut(w, id)
    {
        d.vy = v;
    }
    motion::integrate(w, id);
    obj_mut(w, id).physics_type = ptype::FLY;
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
        d.ay = super::GRAVITY;
    }
    obj_mut(w, id).health = b.full_health;
    w.redtail_link_mut().wind = false;
    if let Some(d) = redtail_mut(w, id) {
        d.death_done = false;
        d.hits = 0;
        d.falls = true;
        d.stand_y = 0.0;
    }
    go_home(w, id);
}

// ----------------------------------------------------------------------
// Pre-tick

/// His pre-tick steps after the shared ones: Claw's side judged from his
/// exact position (Claw is to his right when Claw's x is at or past it,
/// fraction included), then the fall guard. Lower than [`FALL_GUARD_Y`]
/// he is put at [`FALL_GUARD_TO`], stopped, makes one movement step there
/// without falling, then falls again and goes Home.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    if let Some(cx) = w.claw_object().map(|c| c.x) {
        let x = exact_x(w, id);
        if let Some(d) = data_mut(w, id) {
            d.claw_right = f64::from(cx) >= x;
        }
    }
    if obj(w, id).y <= FALL_GUARD_Y {
        return;
    }
    {
        let o = obj_mut(w, id);
        o.y = FALL_GUARD_TO;
        o.physics_type = ptype::DIRECT;
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
        d.ay = 0.0;
    }
    motion::integrate(w, id);
    if let Some(d) = data_mut(w, id) {
        d.ay = super::GRAVITY;
    }
    obj_mut(w, id).physics_type = ptype::FLY;
    go_home(w, id);
}

// ----------------------------------------------------------------------
// Startup and Home

/// Startup: the pose plays; when the fight flag reaches 2, the reset.
pub fn startup_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if w.boss_fight() == fight::ON {
        reset(w, id);
    }
}

/// Goes Home (see the module documentation). Draws nothing.
pub fn go_home(w: &mut World, id: ObjectId) {
    super::states::go_home(w, id);
    set_anim(w, id, IDLE_ANIM);
    let now = w.now_ms;
    let defeated = defeated(w, id);
    if let Some(d) = redtail_mut(w, id) {
        d.home = Timer::new(now, HOME_MS);
    }
    if !defeated {
        obj_mut(w, id).flags &= !(flags::NO_COLLIDE | flags::SAFE);
    }
}

/// Home (see the module documentation). Draws: once the wait is over in
/// modes 1 to 4, one of 0..=100, then the choice's.
pub fn home_tick(w: &mut World, id: ObjectId) {
    if defeated(w, id) {
        return;
    }
    w.advance_anim(id);
    face_claw(w, id);
    let now = w.now_ms;
    if !redtail_data(w, id).is_some_and(|d| d.home.done(now)) {
        return;
    }
    match mode(w, id) {
        0 | 5 | 6 => enter_run(w, id, 0),
        1..=3 => {
            if rand(w, 101) < 90 {
                ranged_choice(w, id);
            } else {
                change_level(w, id, 0);
            }
        }
        _ => {
            let r = rand(w, 101);
            if same_level(w, id) {
                match r {
                    0..33 => kick_entry(w, id),
                    33..66 => slide_entry(w, id),
                    _ => change_level(w, id, 0),
                }
            } else {
                match r {
                    0..45 => knives_entry(w, id),
                    45..80 => wind_entry(w, id),
                    _ => change_level(w, id, 0),
                }
            }
        }
    }
}

/// The ranged choice (see the module documentation). Draws: one of
/// 0..=100 once the cooldown is over, then the attack's.
pub fn ranged_choice(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if !redtail_data(w, id).is_some_and(|d| d.cooldown.done(now)) {
        go_home(w, id);
        return;
    }
    let r = rand(w, 101);
    face_claw(w, id);
    match mode(w, id) {
        0 | 1 => match r {
            0..40 => knives_entry(w, id),
            40..80 => gun_entry(w, id),
            80..90 => slide_entry(w, id),
            _ => wind_entry(w, id),
        },
        2 | 3 => match r {
            0..20 => knives_entry(w, id),
            20..40 => gun_entry(w, id),
            40..80 => slide_entry(w, id),
            _ => wind_entry(w, id),
        },
        4 => {
            if r < 70 {
                kick_entry(w, id);
            } else {
                wind_entry(w, id);
            }
        }
        _ => go_home(w, id),
    }
}

/// Makes the attack `p` (see [`RedTailData::pending`]); any other value
/// goes Home.
pub fn do_attack(w: &mut World, id: ObjectId, p: u8) {
    match p {
        1 => wind_entry(w, id),
        2 => gun_entry(w, id),
        3 => knives_entry(w, id),
        4 => slide_entry(w, id),
        5 => kick_entry(w, id),
        6 => die(w, id),
        _ => go_home(w, id),
    }
}

/// Changes level: up from the sand, down from the ledge, then the attack
/// `p`.
pub fn change_level(w: &mut World, id: ObjectId, p: u8) {
    if redtail_low(w, id) {
        enter_jump_up(w, id, p);
    } else {
        enter_jump_down(w, id, p);
    }
}

// ----------------------------------------------------------------------
// Running at Claw

/// Runs at Claw for the attack `p`.
pub fn enter_run(w: &mut World, id: ObjectId, p: u8) {
    face_claw(w, id);
    if let Some(d) = redtail_mut(w, id) {
        d.pending = p;
    }
    let right = claw_right(w, id);
    set_vx(w, id, toward(right, RUN_SPEED));
    set_state(w, id, EnemyState::FastAdvance);
    set_anim(w, id, RUN_ANIM);
}

fn toward(right: bool, speed: i32) -> f64 {
    f64::from(if right { speed } else { -speed })
}

/// FastAdvance (see the module documentation).
pub fn run_tick(w: &mut World, id: ObjectId) {
    let (min, max) = data(w, id).map_or(UNBOUNDED, |d| d.bounds);
    let right = claw_right(w, id);
    let x = exact_x(w, id);
    let p = redtail_data(w, id).map_or(0, |d| d.pending);
    if (x == f64::from(min) && !right) || (x == f64::from(max) && right) {
        do_attack(w, id, p);
    }
    face_claw(w, id);
    w.advance_anim(id);
    match mode(w, id) {
        0..=3 | 5 => set_vx(w, id, toward(right, RUN_SPEED)),
        4 => do_attack(w, id, p),
        _ => super::states::start_pacing(w, id),
    }
}

// ----------------------------------------------------------------------
// The knives

/// The knives' entry: on Claw's level the kick within 100 px and the gun
/// otherwise; on the other level the knives.
pub fn knives_entry(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    if same_level(w, id) {
        if mode(w, id) == 4 {
            kick_entry(w, id);
        } else {
            gun_entry(w, id);
        }
        return;
    }
    let low = claw_low(w);
    let (anim, vy) = if low {
        (KNIVES_DOWN_ANIM, KNIFE_SPEED.1)
    } else {
        (KNIVES_UP_ANIM, -KNIFE_SPEED.1)
    };
    set_anim(w, id, anim);
    set_state(w, id, EnemyState::KnifeAttack);
    let cooldown = spread_timer(w, COOLDOWN_MS);
    let right = faces_right(w, id);
    let (dx, vx) = if right {
        (KNIFE_MUZZLE.0, KNIFE_SPEED.0)
    } else {
        (-KNIFE_MUZZLE.0, -KNIFE_SPEED.0)
    };
    if let Some(d) = redtail_mut(w, id) {
        d.cooldown = cooldown;
        d.fired = false;
        d.muzzle = (dx, KNIFE_MUZZLE.1);
        d.shot_speed = (vx, vy);
    }
    taunt(w, id);
}

/// KnifeAttack: when the animation ends, three knives, then Home.
pub fn knives_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    let Some(r) = redtail_data(w, id).cloned() else {
        return;
    };
    let (x, y, z) = {
        let o = obj(w, id);
        (o.x, o.y, o.z)
    };
    let at = (x + r.muzzle.0, y + r.muzzle.1, z - 1);
    let (vx, vy) = r.shot_speed;
    knife::launch(w, id, at, (vx, vy));
    play(w, KNIVES_SOUND);
    knife::launch(w, id, at, (vx - KNIFE_SPREAD, vy));
    knife::launch(w, id, at, (vx - KNIFE_SPREAD, vy - KNIFE_SPREAD));
    if let Some(d) = redtail_mut(w, id) {
        d.fired = true;
    }
    go_home(w, id);
}

// ----------------------------------------------------------------------
// The gun

/// The gun's entry: a jump to Claw's level first when he is on the other
/// one, then the shot.
pub fn gun_entry(w: &mut World, id: ObjectId) {
    let (claw, own) = (claw_low(w), redtail_low(w, id));
    if claw && !own {
        enter_jump_down(w, id, 2);
        return;
    }
    if !claw && own {
        enter_jump_up(w, id, 2);
        return;
    }
    let cooldown = spread_timer(w, COOLDOWN_MS);
    set_state(w, id, EnemyState::GunAttack);
    set_anim(w, id, GUN_ANIM);
    set_vx(w, id, 0.0);
    face_claw(w, id);
    if let Some(d) = redtail_mut(w, id) {
        d.cooldown = cooldown;
    }
    taunt(w, id);
    let right = faces_right(w, id);
    let (dx, vx) = if right {
        (GUN_MUZZLE.0, flare::SPEED)
    } else {
        (-GUN_MUZZLE.0, -flare::SPEED)
    };
    if let Some(d) = redtail_mut(w, id) {
        d.fired = false;
        d.muzzle = (dx, GUN_MUZZLE.1);
        d.shot_speed = (vx, 0);
    }
}

/// GunAttack: the flare on the first tick showing [`GUN_FRAME`]; Home
/// when the animation ends.
pub fn gun_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let fire = obj(w, id).frame == GUN_FRAME && redtail_data(w, id).is_some_and(|d| !d.fired);
    if fire && let Some(r) = redtail_data(w, id).cloned() {
        if let Some(d) = redtail_mut(w, id) {
            d.fired = true;
        }
        let (x, y, z) = {
            let o = obj(w, id);
            (o.x, o.y, o.z)
        };
        flare::launch(
            w,
            id,
            (x + r.muzzle.0, y + r.muzzle.1, z - 1),
            r.shot_speed.0,
        );
        play(w, GUN_SOUND);
    }
    if obj(w, id).anim.is_done() {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// The kick

/// The kick's entry: a jump to Claw's level first, a run at him outside
/// 100 px, then the kick (he keeps the facing he has).
pub fn kick_entry(w: &mut World, id: ObjectId) {
    if !same_level(w, id) {
        change_level(w, id, 5);
        return;
    }
    if mode(w, id) != 4 {
        enter_run(w, id, 5);
        return;
    }
    set_state(w, id, EnemyState::KickAttack);
    set_anim(w, id, KICK_ANIM);
    let cooldown = spread_timer(w, COOLDOWN_MS);
    if let Some(d) = redtail_mut(w, id) {
        d.cooldown = cooldown;
    }
    if let Some(d) = data_mut(w, id) {
        d.strike_rect = KICK_RECT;
    }
}

/// KickAttack: [`KICK_RECT`] hurts while [`KICK_FRAME`] shows; Home when
/// the animation ends.
pub fn kick_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let o = obj_mut(w, id);
    if o.frame == KICK_FRAME {
        o.attack_rect = facing_rect(o, KICK_RECT);
    }
    if obj(w, id).anim.is_done() {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// The slide

/// The slide's entry: the ranged choice again when Claw is on the ledge,
/// a jump down first from the ledge, a run at Claw outside 100 px, then
/// the slide.
pub fn slide_entry(w: &mut World, id: ObjectId) {
    if !claw_low(w) {
        ranged_choice(w, id);
        return;
    }
    if !redtail_low(w, id) {
        enter_jump_down(w, id, 4);
        return;
    }
    if mode(w, id) != 4 {
        enter_run(w, id, 4);
        return;
    }
    let right = claw_right(w, id);
    set_facing(w, id, right);
    set_state(w, id, EnemyState::SlideAttack);
    set_anim(w, id, SLIDE_ANIM);
    let cooldown = spread_timer(w, COOLDOWN_MS);
    set_vx(w, id, toward(right, SLIDE_SPEED));
    let cx = w.claw_object().map_or(0, |c| c.x);
    let to = if right {
        cx + SLIDE_PAST
    } else {
        cx - SLIDE_PAST
    };
    if let Some(d) = redtail_mut(w, id) {
        d.cooldown = cooldown;
        d.slide_to = to;
    }
    taunt(w, id);
    play(w, SLIDE_SOUND);
    if let Some(d) = data_mut(w, id) {
        d.strike_rect = SLIDE_RECT;
    }
}

/// SlideAttack (see the module documentation).
pub fn slide_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let o = obj_mut(w, id);
    if o.frame == SLIDE_FRAME {
        o.attack_rect = facing_rect(o, SLIDE_RECT);
    }
    let x = obj(w, id).x;
    let right = claw_right(w, id);
    let to = redtail_data(w, id).map_or(x, |d| d.slide_to);
    let (min, max) = data(w, id).map_or(UNBOUNDED, |d| d.bounds);
    let passed = if right { x >= to } else { x <= to };
    let limit = if right { max } else { min };
    if passed || f64::from(limit) == exact_x(w, id) {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// The wind

/// The wind's entry: only at a Claw on the sand while the wind is not
/// blowing (a jump up first from the sand); otherwise the ranged choice.
/// Draws: one of 0..2 (his line).
pub fn wind_entry(w: &mut World, id: ObjectId) {
    if !claw_low(w) || w.redtail_link().wind {
        ranged_choice(w, id);
        return;
    }
    if redtail_low(w, id) {
        enter_jump_up(w, id, 1);
        return;
    }
    w.redtail_link_mut().wind = true;
    set_state(w, id, EnemyState::WindAttack);
    set_anim(w, id, WIND_ANIM);
    let line = WIND_LINES[rand(w, 2) as usize];
    say(w, id, line);
    set_facing(w, id, true);
}

/// WindAttack: when the animation ends, Home with a longer wait. Draws:
/// at the end, one of 0..=500.
pub fn wind_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        go_home(w, id);
        let rest = spread_timer(w, WIND_REST_MS);
        if let Some(d) = redtail_mut(w, id) {
            d.home = rest;
        }
    }
}

// ----------------------------------------------------------------------
// The jumps

/// The jump up for the attack `p` (at once when he is not on the sand).
pub fn enter_jump_up(w: &mut World, id: ObjectId, p: u8) {
    if !redtail_low(w, id) {
        do_attack(w, id, p);
        return;
    }
    if let Some(d) = redtail_mut(w, id) {
        d.pending = p;
    }
    if !standing(w, id) {
        return;
    }
    let (x, y) = leave_floor(w, id);
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
        d.vx = 0.0;
        d.vy = JUMP_UP_SPEED;
    }
    obj_mut(w, id).physics_type = ptype::DIRECT;
    set_state(w, id, EnemyState::JumpUp);
    set_anim(w, id, JUMP_ANIM);
    launch_to(w, id, (x, y), JUMP_UP_TARGET);
    if obj(w, id).x != JUMP_UP_TARGET.0 {
        set_facing(w, id, obj(w, id).x < JUMP_UP_TARGET.0);
    }
    play(w, JUMP_SOUND);
}

/// The jump down for the attack `p` (at once when he is on the sand).
/// Draws: one of 0..=300, even when he cannot jump.
pub fn enter_jump_down(w: &mut World, id: ObjectId, p: u8) {
    if redtail_low(w, id) {
        do_attack(w, id, p);
        return;
    }
    if let Some(d) = redtail_mut(w, id) {
        d.pending = p;
    }
    let tx = JUMP_DOWN_X + rand(w, JUMP_DOWN_SPREAD);
    if !standing(w, id) {
        return;
    }
    let (x, y) = leave_floor(w, id);
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
        d.vx = 0.0;
        d.vy = JUMP_DOWN_SPEED;
    }
    obj_mut(w, id).physics_type = ptype::DIRECT;
    set_state(w, id, EnemyState::JumpDown);
    set_anim(w, id, JUMP_ANIM);
    launch_to(w, id, (x, y), (tx, JUMP_DOWN_Y));
    play(w, JUMP_SOUND);
}

/// The slowest arc from `from` to `to` under his gravity (none when it
/// would be slower than 0.1 px/s, keeping the speed he has).
fn launch_to(w: &mut World, id: ObjectId, from: (f64, f64), to: (i32, i32)) {
    if let Some(d) = data_mut(w, id) {
        let dx = f64::from(to.0) - from.0;
        let dy = f64::from(to.1) - from.1;
        let (vx, vy) = human::launch(dx, dy, d.ay);
        if vx != 0.0 || vy != 0.0 {
            d.vx = vx;
            d.vy = vy;
        }
    }
}

/// JumpUp: above [`JUMP_UP_FLY_Y`] he takes the tiles again; landed, the
/// landing and the attack he jumped for.
pub fn jump_up_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let o = obj_mut(w, id);
    if o.physics_type == ptype::DIRECT && o.y < JUMP_UP_FLY_Y {
        o.physics_type = ptype::FLY;
    }
    if o.physics_type == ptype::STAND {
        landing(w, id);
        let p = redtail_data(w, id).map_or(0, |d| d.pending);
        do_attack(w, id, p);
    }
}

/// JumpDown: below [`JUMP_DOWN_FLY_Y`] he takes the tiles again; landed,
/// the landing and the attack he jumped for.
pub fn jump_down_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let o = obj_mut(w, id);
    if o.physics_type == ptype::DIRECT && o.y > JUMP_DOWN_FLY_Y {
        o.physics_type = ptype::FLY;
    }
    if o.physics_type == ptype::STAND {
        landing(w, id);
        let p = redtail_data(w, id).map_or(0, |d| d.pending);
        do_attack(w, id, p);
    }
}

/// The landing (see the module documentation).
pub fn landing(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
        d.bounds = LANDED_BOUNDS;
        d.bounds_y.1 = LANDED_FLOOR;
        d.busy = false;
    }
    let o = obj_mut(w, id);
    if o.physics_type == ptype::DIRECT {
        o.physics_type = ptype::FLY;
    }
    if o.y > LANDED_CLAMP {
        o.y = LANDED_CLAMP;
    }
    o.flags &= !(flags::NO_COLLIDE | flags::SAFE);
    let now = w.now_ms;
    if let Some(d) = redtail_mut(w, id) {
        d.home = Timer::new(now, LANDED_HOME_MS);
        // Put down on his pixel: he stands exactly on it.
        d.stand_y = 0.0;
    }
}

/// The third counted blow found him free: the landing, then a leap away
/// from Claw's level (Home when he is already on the other one).
pub fn escape(w: &mut World, id: ObjectId) {
    landing(w, id);
    if claw_low(w) {
        enter_jump_up(w, id, 0);
    } else {
        enter_jump_down(w, id, 0);
    }
}

/// An explosion found him free: the landing, then the shared hop back.
pub fn hop_back(w: &mut World, id: ObjectId) {
    landing(w, id);
    human::enter_jump_back(w, id);
}

// ----------------------------------------------------------------------
// The block

/// The block: he faces Claw, poses and plays his block sound; a third of
/// the time a block line. Draws: one of 0..=100, then below 33 one of
/// 0..3.
pub fn enter_block(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    set_state(w, id, EnemyState::Block);
    set_anim(w, id, BLOCK_ANIM);
    play(w, BLOCK_SOUND);
    if rand(w, 101) < BLOCK_LINE_CHANCE {
        let line = BLOCK_LINES[rand(w, 3) as usize];
        say(w, id, line);
    }
}

/// Block: Home when the pose ends (on its first call).
pub fn block_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// Death

/// His death: on the ledge the jump down first (he dies on its landing;
/// in the air only the attack it would make is kept); on the sand his
/// fall, busy, hurting nothing and safe from every hit.
pub fn die(w: &mut World, id: ObjectId) {
    if !redtail_low(w, id) {
        enter_jump_down(w, id, 6);
        return;
    }
    set_state(w, id, EnemyState::Dead);
    set_anim(w, id, DEATH_ANIM);
    set_busy(w, id, true);
    obj_mut(w, id).flags |= flags::HARMLESS | flags::SAFE;
    if let Some(d) = redtail_mut(w, id) {
        d.line_said = false;
        d.death_done = false;
        d.quiet = SPEAKER_TAIL + 1;
    }
}

/// True while a line of his holds the enemies' speaking slot: while it
/// runs and for [`SPEAKER_TAIL`] ticks after (see the module
/// documentation). Counts the quiet ticks.
fn holds_the_slot(w: &mut World, id: ObjectId) -> bool {
    let speaking = sound::enemy_is_speaking(w, id);
    match redtail_mut(w, id) {
        Some(d) => {
            d.quiet = if speaking {
                0
            } else {
                d.quiet.saturating_add(1)
            };
            d.quiet <= SPEAKER_TAIL
        }
        None => speaking,
    }
}

/// Dead (see the module documentation).
pub fn death_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let busy = holds_the_slot(w, id);
    if !busy && redtail_data(w, id).is_some_and(|d| !d.line_said) {
        say(w, id, DEATH_LINE);
        if let Some(d) = redtail_mut(w, id) {
            d.line_said = true;
            d.quiet = 0;
        }
    }
    let end = obj(w, id).anim.is_done()
        && !busy
        && redtail_data(w, id).is_some_and(|d| d.line_said && !d.death_done);
    if !end {
        return;
    }
    if let Some(d) = redtail_mut(w, id) {
        d.death_done = true;
    }
    if w.claw.stats.health <= 0 {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        d.dead = true;
        d.frozen = true;
    }
    release_gem(w, id);
    sound::enemy_silence(w, id);
    if let Some(d) = redtail_mut(w, id) {
        d.defeated = true;
    }
}

/// His defeat ([`World::boss_defeated`]): Claw held and the gem out of
/// him. The gem's flight runs on game time, so in the tick it is made it
/// does not move: when it would get its first call in this very update
/// pass (it lands after the object the pass goes on with), that call is
/// kept for the next tick and the camera heads for it now, as its first
/// call would have it (as for the twelfth level's boss).
fn release_gem(w: &mut World, id: ObjectId) {
    let kept = w.objects.next_of(id);
    w.boss_defeated(id);
    let gem_z = obj(w, id).z.saturating_add(1);
    let runs_now = kept
        .and_then(|k| w.objects.get(k))
        .is_some_and(|k| k.z <= gem_z || k.flags & flags::RESORT != 0);
    if !runs_now {
        return;
    }
    let gem = w
        .objects
        .all()
        .find(|(_, o)| &*o.logic == pickup::END_GEM && !o.is_listed())
        .map(|(g, o)| (g, (o.x, o.y)));
    if let Some((g, at)) = gem {
        if let Some(o) = w.objects.get_mut(g) {
            o.frame_delay = 1;
        }
        w.set_camera_override(Some(at));
    }
}

// ----------------------------------------------------------------------
// Hits

/// `attacker` hit `id` (see the module documentation). The same attack of
/// Claw hits only once and nothing hits during the hit cooldown; then, by
/// what hit:
///
/// - Claw himself: the blow is counted; the third resets the count and,
///   when he is free, makes him leap away in the post-tick, unhurt
///   (during a flinch it is lost); any other blow during a flinch only
///   sends him Home, and otherwise costs him the blow's damage with a hit
///   burst, a [`HIT_COOLDOWN_MS`] cooldown and a flinch (high after an
///   uppercut);
/// - a pistol shot or a stick of dynamite: used up, and he blocks;
/// - magic: he blocks;
/// - an explosion or another shot of the level: when free, the hop back
///   in the post-tick.
///
/// At 0 health or below he dies. Draws: the burst's two and one of 0..4
/// for a high flinch, or the block's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let Some(kind) = w.objects.get(attacker).map(|a| a.type_flags) else {
        return;
    };
    if kind == types::ENEMY_SHOT {
        return;
    }
    if !accept_hit(w, id) {
        return;
    }
    let busy = data(w, id).is_some_and(|d| d.busy);
    match kind {
        attacker::CLAW => {
            let third = match redtail_mut(w, id) {
                Some(d) => {
                    d.hits += 1;
                    if d.hits >= ESCAPE_EVERY {
                        d.hits = 0;
                        true
                    } else {
                        false
                    }
                }
                None => false,
            };
            if third {
                if !busy && let Some(d) = redtail_mut(w, id) {
                    d.later = Some(Later::Escape);
                }
            } else if matches!(
                super::state(w, id),
                Some(EnemyState::HitLow | EnemyState::HitHigh)
            ) {
                go_home(w, id);
            } else {
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
                let now = w.now_ms;
                if let Some(d) = data_mut(w, id) {
                    d.cooldown = Timer::new(now, HIT_COOLDOWN_MS);
                }
                let high = w.claw.attack.uppercut;
                enter_hit(w, id, high);
            }
        }
        attacker::PISTOL_SHOT => {
            w.destroy(attacker);
            enter_block(w, id);
        }
        attacker::MAGIC | attacker::THROWN => enter_block(w, id),
        attacker::LEVEL_SHOT => {
            if !busy && let Some(d) = redtail_mut(w, id) {
                d.later = Some(Later::HopBack);
            }
        }
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id);
    }
}

/// `id` attacked `victim`: nothing more than any enemy.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

/// His own sounds and lines, for tests and tools that check them against
/// the level.
pub fn sound_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = TAUNTS.to_vec();
    v.extend(WIND_LINES);
    v.extend(BLOCK_LINES);
    v.extend([
        DEATH_LINE,
        HIT_LOW_SOUND,
        HIT_HIGH_SOUND,
        KNIVES_SOUND,
        GUN_SOUND,
        SLIDE_SOUND,
        JUMP_SOUND,
    ]);
    v
}

/// His own animations, for tests and tools that check them against the
/// level.
pub fn anim_names() -> [&'static str; 13] {
    [
        IDLE_ANIM,
        DEATH_ANIM,
        RUN_ANIM,
        JUMP_ANIM,
        KNIVES_DOWN_ANIM,
        KNIVES_UP_ANIM,
        GUN_ANIM,
        WIND_ANIM,
        SLIDE_ANIM,
        KICK_ANIM,
        BLOCK_ANIM,
        HIT_HIGH_ANIM,
        HIT_LOW_ANIM,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn his_states_open_the_thirteenth_levels_block() {
        assert_eq!((*STATE_BLOCK.start(), *STATE_BLOCK.end()), (2300, 2319));
        assert_eq!(
            *super::super::aquatis::STATE_BLOCK.end() + 1,
            *STATE_BLOCK.start()
        );
        for (i, s) in STATES.iter().enumerate() {
            assert_eq!(*s as i32, *STATE_BLOCK.start() + i as i32, "{}", s.name());
            assert!(handler(*s).is_some(), "{}", s.name());
        }
        assert!(CLASS.human, "a human enemy");
        assert!(CLASS.carry.is_none(), "never lifted");
        for s in [
            EnemyState::Startup,
            EnemyState::Home,
            EnemyState::FastAdvance,
            EnemyState::Block,
            EnemyState::HitHigh,
            EnemyState::HitLow,
            EnemyState::Dead,
        ] {
            assert!(handler(s).is_some(), "{}", s.name());
        }
    }

    #[test]
    fn the_blow_count_starts_at_one_and_twenty_blows_kill_him() {
        assert_eq!(RedTailData::default().hits, 1);
        assert_eq!(HEALTH / BLOW_DAMAGE, 20);
        let l = RedTailLink::default();
        assert_eq!(l.redtail, None);
        assert!(!l.wind);
    }

    #[test]
    fn his_art_is_his_own() {
        for anim in anim_names() {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
        for s in sound_names() {
            assert!(s.starts_with(ANIM_PREFIXES[0]), "{s}");
        }
        assert!(!"LEVEL_REDTAILPIRATE_HOME".starts_with(ANIM_PREFIXES[0]));
    }

    #[test]
    fn his_standing_speed_shrinks_toward_rest() {
        // One tick standing after his drop: the first arena step's speed.
        let v = next_stand_speed(BUILT_STAND_SPEED, 2000.0);
        assert!((v + 103.460_137).abs() < 1e-6, "{v}");
        let mut v = v;
        for _ in 31..=643 {
            v = next_stand_speed(v, 2000.0);
        }
        assert!((v + 36.118_414).abs() < 1e-5, "{v}");
        // From rest or moving down, none.
        assert_eq!(next_stand_speed(0.0, 2000.0), 0.0);
        assert_eq!(next_stand_speed(115.0, 2000.0), 0.0);
    }

    #[test]
    fn his_hit_mask_leaves_out_his_own_shots() {
        assert_eq!(HIT_MASK & types::ENEMY_SHOT, 0);
        assert_ne!(HIT_MASK & types::PLAYER, 0);
        assert_ne!(HIT_MASK & types::PISTOL_SHOT, 0);
        assert_ne!(HIT_MASK & types::LEVEL_SHOT, 0);
        assert_ne!(HIT_MASK & attacker::MAGIC, 0);
        assert_eq!(HIT_MASK & types::THROWN, 0);
    }
}
