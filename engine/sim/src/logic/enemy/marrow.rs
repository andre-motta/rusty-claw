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
//! and reacts, and the sword can still kill that passive Marrow.
//!
//! # Random draws, in order
//!
//! Each tick (his logic call): the shared pre-tick's roll of 0..=1000 (on
//! frame pulse ticks); when no parrot call comes due and Claw attacks
//! facing him from within his two inner range boxes, one of 0..100
//! against his smarts; then the state's draws: Idle one of 0..100 on a
//! frame pulse tick once his wait is over and he does not see Claw, and
//! one more of 0..100 when the first is below 5; Home one of 0..121, in
//! mode 4 one of 0..100, then moving back's or moving ahead's, one of 0..4
//! when he fires, and the shared Home's when he is still Home; Pace one
//! of 0..100 per limit reached; FastAdvance one, discarded. When hit by
//! Claw's sword: one of 0..2 for a block's sound, or one of 0..4 for a
//! high flinch's sound and then the hit burst's; at his death the death's.
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
//! run already. The room is built with the door's warp: the warp's second
//! pass makes the first calls of the room's objects (Marrow, the parrot and
//! the slabs are built there), so on Claw's first tick in the room Marrow
//! idles (Claw is out of his range and he is off screen), the slabs start
//! to open and the phase health reads 0, as the original shows it at the
//! end of that tick. Measured on the
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
//! - Claw's respawn resets him ([`respawn_reset`]) when the first of his
//!   readers notices the death after Claw has come back (the slabs, which
//!   run before him, or his own call): the slabs' reset then reads his
//!   restored health, as the original's respawn, which resets him itself,
//!   gives it.
//! - Standing, his exact height creeps down a part of a pixel a tick and
//!   back, as the original keeps it ([`MarrowData::stand_fraction`]); the
//!   parrot's hold on him, the carry and his body's flight after his death
//!   read it. This build's other enemies stand on the whole pixel.
//! - A move up or down by other means than his movement step (a drop back
//!   onto the floor, the fall reset) leaves his exact position across as
//!   it was.
//!
//! His data ([`MarrowData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use std::ops::RangeInclusive;

use super::human::{self, BossFields, HumanAnims, HumanCall, HumanData, HumanSounds, StraightShot};
use super::states::{enter_hit, enter_strike, hit_tick, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, Deferred, EnemyData, EnemyFields, EnemyState,
    FIRE_BLOW, FIRE_DEATH_ANIM, GRAVITY, HUMAN_HIT_BY, Handler, ICE_BLOW, ICE_DEATH_ANIM, Timer,
    UNBOUNDED, accept_hit, attacker, build, data, data_mut, die, face_claw, hit_burst,
    human_fields, lift, motion, obj, obj_mut, rand, say, scaled, sees_claw, set_anim, set_facing,
    set_state, states, take_damage,
};
use crate::anim::AnimPlayer;
use crate::logic::world_boss::fight;
use crate::logic::{ObjectData, World, boss_stage};
use crate::object::{ObjectId, Rect, engine_state, flags};
use crate::physics::ptype;
use crate::player::attack::kind as attack_kind;
use crate::player::state as claw_state;

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
pub const BUILT: bool = true;

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

/// Time idle before he may start pacing, ms.
pub const IDLE_WAIT_MS: u64 = 1000;
/// Chance in 100 per frame pulse tick, once his idle wait is over, of
/// pacing.
pub const PACE_CHANCE: i32 = 5;
/// Chance in 100 that he says his pacing line as he starts pacing.
pub const PACE_LINE_CHANCE: i32 = 25;
/// His idle line: tried when the roll is below this.
pub const IDLE_LINE_ROLL: i32 = 500;
/// Home: draws of 0..121.
pub const HOME_SPREAD: u32 = 121;
/// Home in modes 2 and 3: a draw below smarts + this (and not below
/// smarts) steps in.
pub const MOVE_AHEAD_BAND: i32 = 20;
/// Home in mode 4: a draw of 0..100 below this backs off.
pub const MOVE_BACK_CHANCE: i32 = 40;
/// The hit cooldown of a sword blow, ms.
pub const HIT_COOLDOWN_MS: u64 = 300;
/// A sword blow with a roll below this is blocked.
pub const BLOCK_ROLL: i32 = 400;
/// The rolls of a block on which he also says his block line.
pub const BLOCK_LINE_ROLLS: RangeInclusive<i32> = 300..=399;
/// The rolls of a shot on which he says his shot line.
pub const FIRE_LINE_ROLLS: RangeInclusive<i32> = 0..=99;
/// Dodge bands of the roll: below the first he hops back, below the
/// second he hops up.
pub const DODGE_ROLLS: [i32; 2] = [200, 600];
/// How far above his start height the fall reset puts him back, px.
pub const FALL_RESET_RISE: i32 = 10;

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
/// Both hops.
pub const JUMP_ANIM: &str = "LEVEL_MARROW_JUMP";
/// The block.
pub const BLOCK_ANIM: &str = "LEVEL_MARROW_BLOCK";
/// The whistle's sound.
pub const WHISTLE_SOUND: &str = "LEVEL_MARROW_WHISTLE";
/// The shot's sound.
pub const PISTOL_SOUND: &str = "LEVEL_MARROW_PISTOL";
/// The strike's sound.
pub const STAB_SOUND: &str = "LEVEL_MARROW_STAB2";
/// The block sounds, one picked at random.
pub const BLOCK_SOUNDS: [&str; 2] = ["LEVEL_MARROW_BLOCK1", "LEVEL_MARROW_BLOCK2"];
/// His death sound, when Claw says no line.
pub const DEATH_SOUND: &str = "LEVEL_MARROW_00430003";
/// His line as the parrot lifts him.
pub const CARRY_LINE: &str = "LEVEL_MARROW_00430006";
/// His line as he starts pacing.
pub const PACE_LINE: &str = "LEVEL_MARROW_00430008";
/// His line as he fires.
pub const FIRE_LINE: &str = "LEVEL_MARROW_00430010";
/// His line on a blocked sword blow.
pub const BLOCK_LINE: &str = "LEVEL_MARROW_00430015";
/// His idle line, which the level does not have: tried, never heard.
pub const IDLE_LINE: &str = "LEVEL_MARROW_00840003";

/// The shot he fires.
pub const SHOT: StraightShot = StraightShot {
    image: BULLET_IMAGE_SET,
    damage: BULLET_DAMAGE,
    sound: PISTOL_SOUND,
};

/// The Marrow class.
pub static CLASS: Class = Class {
    name: "Marrow",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home,
    go_idle,
    carry: None,
    pick_lift_voice: super::nothing,
    lift_roll: true,
    anims: ClassAnims {
        advance: "LEVEL_MARROW_ADVANCE",
        fast_advance: "LEVEL_MARROW_FASTADVANCE",
        pace: "LEVEL_MARROW_FASTADVANCE",
        walk: "",
        hit_low: "LEVEL_MARROW_HITLOW",
        hit_high: "LEVEL_MARROW_HITHIGH",
        home: HOME_ANIM,
        jump: JUMP_ANIM,
        fall: "LEVEL_MARROW_IMPACT",
        lifted: "LEVEL_MARROW_LIFTED",
        lifted2: "LEVEL_MARROW_LIFTED",
        bounce: "LEVEL_MARROW_LIFTED",
    },
    sounds: ClassSounds {
        strike: STAB_SOUND,
        hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
        hit_high: "GAME_SOLHITHI",
    },
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
    /// The whistle is due in this tick's post-tick (the parrot call of the
    /// pre-tick).
    pub whistle_due: bool,
    /// The whistle's end has called the parrot.
    pub called: bool,
    /// The wait for the parrot after the whistle.
    pub wait: Timer,
    /// How far beside him, across, the parrot takes hold of him.
    pub grab_offset: i32,
    /// The part of a pixel he stands below his height (see
    /// [`stand_step`]).
    pub stand_fraction: f64,
    /// Standing, his part of a pixel creeps down a tick's fall at a time
    /// (until a drop down onto the floor stops it).
    pub creeping: bool,
    /// He was put somewhere by other means than his movement step: his
    /// next standing step starts from the whole pixel.
    pub placed: bool,
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
    match s {
        EnemyState::Startup => Some(startup_tick),
        EnemyState::Idle => Some(idle_tick),
        EnemyState::Home => Some(home_tick),
        EnemyState::Fire => Some(fire_tick),
        EnemyState::WaitForParrot => Some(wait_tick),
        EnemyState::HitLow | EnemyState::HitHigh => Some(flinch_tick),
        _ => None,
    }
}

fn marrow_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a MarrowData> {
    match &w.objects.get(id)?.data {
        ObjectData::Marrow(d) => Some(d),
        _ => None,
    }
}

fn marrow_data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut MarrowData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Marrow(d) => Some(d),
        _ => None,
    }
}

/// How far beside Marrow `id`, across, the parrot takes hold of him: set
/// by his whistle (0 before the first).
pub fn grab_offset(w: &World, id: ObjectId) -> i32 {
    marrow_data(w, id).map_or(0, |d| d.grab_offset)
}

/// His phase health, from the link (0 without it).
fn phase_health(w: &World) -> i32 {
    w.marrow_link().map_or(0, |l| l.phase_health)
}

/// True while his health is at or below the phase health less
/// [`PHASE_DAMAGE`]: he blocks every sword blow and whistles when free.
pub fn phase_over(w: &World, id: ObjectId) -> bool {
    obj(w, id).health <= phase_health(w) - PHASE_DAMAGE
}

// ----------------------------------------------------------------------
// The logic call

/// One logic call: the respawn reset when Claw respawns, then the human
/// logic call with his own scheduled whistle, then his death's report.
/// Being removed, he stops being the current boss and the Marrow link
/// ends.
pub fn tick(w: &mut World, id: ObjectId) {
    let raw = obj(w, id).state;
    if raw == engine_state::DESTROYING {
        if w.current_boss() == Some(id) {
            w.set_current_boss(None);
        }
        w.unlink_marrow(id);
    }
    if EnemyState::from_id(raw).is_some() {
        respawn_reset(w);
    }
    run(w, id);
    after_call(w, id);
}

/// The human logic call ([`human::run`]) with his own post-tick: the
/// whistle the pre-tick scheduled takes the place of the shared scheduled
/// call.
fn run(w: &mut World, id: ObjectId) {
    let raw = obj(w, id).state;
    if matches!(
        raw,
        engine_state::INIT | engine_state::DESTROYING | engine_state::KILLED
    ) || EnemyState::from_id(raw).is_none()
    {
        super::tick(w, id, &CLASS);
        return;
    }
    super::pre_tick(w, id, &CLASS);
    pre_tick(w, id);
    let s = super::state(w, id).unwrap_or_default();
    if let Some(h) = handler(s).or_else(|| states::shared_handler(s, true)) {
        h(w, id);
    }
    post_tick(w, id);
}

/// The post-tick: the block's release and the scheduled call (the whistle
/// first), then the movement step, as the shared human post-tick.
fn post_tick(w: &mut World, id: ObjectId) {
    let whistle = marrow_data_mut(w, id).is_some_and(|d| std::mem::take(&mut d.whistle_due));
    let deferred = data_mut(w, id).and_then(|d| d.deferred.take());
    if whistle || deferred.is_some() {
        let now = super::state(w, id);
        let armed = data_mut(w, id).and_then(|d| d.human.release_in.take());
        if armed.is_some() && armed == now {
            obj_mut(w, id).flags &= !flags::SAFE;
            if let Some(d) = data_mut(w, id) {
                d.busy = false;
            }
        }
    }
    if whistle {
        enter_wait(w, id);
    } else {
        match deferred {
            Some(Deferred::GoIdle) => go_idle(w, id),
            Some(Deferred::JumpCheck) => states::jump_check(w, id, &CLASS),
            Some(Deferred::Dropped) => lift::let_go(w, id),
            Some(Deferred::Human(call)) => human::run_deferred(w, id, &CLASS, call),
            None => {}
        }
    }
    let x = obj(w, id).x;
    let vx = data(w, id).map_or(0.0, |d| d.vx);
    let bounds = data(w, id).map_or(UNBOUNDED, |d| d.bounds);
    if let Some(d) = data_mut(w, id) {
        d.human.pre_move_x = x;
    }
    motion::integrate(w, id);
    if let Some(d) = data_mut(w, id) {
        d.human.tile_hit = vx != 0.0 && d.vx == -0.5 * vx;
    }
    let guarded = data(w, id).is_some_and(|d| d.bounds != bounds && !d.human.tile_hit);
    stand_step(w, id, guarded);
}

/// The part of a pixel a standing Marrow stands below his whole-pixel
/// height, as the original keeps it: each movement step of his while he
/// stands adds a tick's fall under gravity from rest (0.324 px at the
/// 18 ms step), and the step that would take him a whole pixel down puts
/// him back on the whole pixel; a step after he was put somewhere (built,
/// reset, dropped back, held back at a ledge) starts from the whole pixel,
/// and after a drop down onto the floor the part stays 0. The movement
/// step itself keeps a standing enemy on the whole pixel: the part only
/// shows where his exact height is read (the parrot's hold on him, his
/// carry, his body's flight after his death).
fn stand_step(w: &mut World, id: ObjectId, guarded: bool) {
    let standing = obj(w, id).physics_type == ptype::STAND;
    let dt = f64::from(w.dt_ms) * 0.001;
    let Some(m) = marrow_data_mut(w, id) else {
        return;
    };
    if !standing {
        m.stand_fraction = 0.0;
        return;
    }
    if std::mem::take(&mut m.placed) || guarded || !m.creeping {
        m.stand_fraction = 0.0;
    } else {
        let next = m.stand_fraction + 0.5 * GRAVITY * dt * dt;
        m.stand_fraction = if next >= 1.0 { 0.0 } else { next };
    }
    let fraction = m.stand_fraction;
    let at = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    if let Some(d) = data_mut(w, id)
        && d.moved_to == Some(at)
    {
        d.carry.1 = fraction;
    }
}

/// After a move up or down by other means than his movement step, his
/// exact position across is still `across`: the move leaves the part of a
/// pixel he carries across alone.
fn keep_across(w: &mut World, id: ObjectId, across: f64) {
    let at = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    if let Some(d) = data_mut(w, id) {
        d.carry = (across - f64::from(at.0), 0.0);
        d.moved_to = Some(at);
    }
}

/// Marks Marrow as put somewhere by other means than his movement step
/// (see [`stand_step`]); `creeping` says whether his part of a pixel creeps
/// from there.
fn placed(w: &mut World, id: ObjectId, creeping: bool) {
    if let Some(m) = marrow_data_mut(w, id) {
        m.placed = true;
        m.creeping = creeping;
        m.stand_fraction = 0.0;
    }
}

/// `attacker` hit `id`. A blow carrying fire or ice picks the matching
/// death; the same attack of Claw hits only once and nothing hits during
/// the cooldown. Then, by what hit:
///
/// - Claw himself: a 300 ms cooldown; during a flinch he only goes Home;
///   with a roll below 400, or once his health is [`PHASE_DAMAGE`] below
///   his phase health, he blocks low with one of his two block sounds
///   (no damage; on rolls 300 to 399 he also says his block line);
///   otherwise he flinches (high after an uppercut), takes the damage and
///   shows a hit burst, and a sword stab while Claw ducks makes him strike
///   back at once;
/// - a pistol shot (or dynamite), magic or a thrown object: it is
///   destroyed and he blocks low, silently, unhurt;
/// - an environment shot: nothing;
/// - a springboard: thrown toward its target, as any human enemy.
///
/// At 0 health or below he dies (and the death is reported).
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
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
    let Some(kind) = w.objects.get(attacker).map(|a| a.type_flags) else {
        return;
    };
    match kind {
        attacker::CLAW => {
            let now = w.now_ms;
            if let Some(d) = data_mut(w, id) {
                d.cooldown = Timer::new(now, HIT_COOLDOWN_MS);
            }
            let roll = data(w, id).map_or(i32::MAX, |d| d.roll);
            if matches!(
                super::state(w, id),
                Some(EnemyState::HitLow | EnemyState::HitHigh)
            ) {
                go_home(w, id);
            } else if (0..BLOCK_ROLL).contains(&roll) || phase_over(w, id) {
                let pick = rand(w, 2) as usize;
                if let Some(d) = data_mut(w, id) {
                    d.human.sounds.block_low = BLOCK_SOUNDS[pick];
                }
                human::enter_block(w, id, false);
                if BLOCK_LINE_ROLLS.contains(&roll) {
                    say(w, id, BLOCK_LINE);
                }
            } else {
                let high = w.claw.attack.uppercut;
                enter_hit(w, id, high);
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
                let stab = matches!(
                    w.claw.attack.kind,
                    attack_kind::SWIPE | 2 | attack_kind::DUCK_SWIPE | attack_kind::JUMP_SWIPE
                );
                let ducking = w
                    .claw_object()
                    .is_some_and(|c| c.physics_type == ptype::DUCK);
                if stab && ducking {
                    enter_strike(w, id);
                }
            }
        }
        attacker::PISTOL_SHOT | attacker::MAGIC | attacker::THROWN => {
            w.destroy(attacker);
            if let Some(d) = data_mut(w, id) {
                d.human.sounds.block_low = "";
            }
            human::enter_block(w, id, false);
        }
        attacker::SPRING => states::enter_spring_jump(w, id, attacker),
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id, DeathKind::Hit, Some(attacker));
        boss_died(w, id);
    }
}

/// `id` attacked `victim`: nothing more than any enemy.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

/// What follows each logic call: a death seen for the first time (on a
/// death tile, or a kill by another logic) is reported.
fn after_call(w: &mut World, id: ObjectId) {
    let Some((dead, seen)) = data(w, id).map(|d| (d.dead, boss(d).death_seen)) else {
        return;
    };
    if dead && !seen {
        boss_died(w, id);
    }
}

fn boss(d: &EnemyFields) -> BossFields {
    d.human.boss.unwrap_or_default()
}

fn with_boss(w: &mut World, id: ObjectId, f: impl FnOnce(&mut BossFields)) {
    if let Some(b) = data_mut(w, id).and_then(|d| d.human.boss.as_mut()) {
        f(b);
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

/// The start of the fight, run when he is built and once per respawn of
/// Claw: back where he was placed, unbounded across, down on the floor
/// below, full health, the slabs asked to open, the parrot call and the
/// floors-open status clear. His state, speed, timers, busy flag and
/// phase health stay as they are.
fn start_of_fight(w: &mut World, id: ObjectId) {
    let attempt = w.attempt();
    let Some(b) = data(w, id).and_then(|d| d.human.boss) else {
        return;
    };
    with_boss(w, id, |b| b.attempt = attempt);
    {
        let o = obj_mut(w, id);
        o.x = b.start.0;
        o.y = b.start.1;
    }
    if let Some(d) = data_mut(w, id) {
        d.bounds = UNBOUNDED;
    }
    super::drop_to_floor(w, id);
    let creeping = marrow_data(w, id).is_none_or(|m| m.creeping);
    placed(w, id, creeping);
    obj_mut(w, id).health = b.full_health;
    if let Some(l) = w.marrow_link_mut() {
        l.open_request = true;
        l.parrot_call = false;
        l.floors_open = false;
    }
}

/// Claw's respawn runs the start of the fight on Marrow (see
/// [`start_of_fight`]) before anything reads him: the slabs' reset after
/// a death calls it first, and so does his own logic call; it runs once
/// per death of Claw's, once he is respawning or alive again (not while
/// he is still dying). Draws nothing.
pub fn respawn_reset(w: &mut World) {
    let Some(id) = w.marrow_link().map(|l| l.marrow) else {
        return;
    };
    let respawning = w
        .claw_object()
        .is_some_and(|c| c.state == claw_state::RESPAWN)
        || w.claw.stats.health > 0;
    let attempt = w.attempt();
    let due = data(w, id).is_some_and(|d| d.human.boss.is_some_and(|b| b.attempt != attempt));
    if respawning && due {
        start_of_fight(w, id);
    }
}

// ----------------------------------------------------------------------
// Building

/// Builds Marrow: the current boss, his place kept for the respawn
/// reset, his boxes, 100 health and 10 damage (times the difficulty
/// factors), smarts 100, never liftable, going by sight, always active;
/// the Marrow link starts; then the start of the fight, the drop to the
/// floor and `Startup`. Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    let start = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    w.set_current_boss(Some(id));
    let full_health = scaled(HEALTH, super::HEALTH_FACTOR);
    let fields = EnemyFields {
        ranges: RANGES,
        contact_rect: CONTACT,
        strike_rect: STRIKE,
        strike_anim: STRIKE_ANIM,
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        smarts: super::smarts(SMARTS),
        may_lift: false,
        sight: true,
        bounds: UNBOUNDED,
        human: HumanData {
            anims: HumanAnims {
                jump_back: JUMP_ANIM,
                block_high: BLOCK_ANIM,
                block_low: BLOCK_ANIM,
                idle: "",
            },
            sounds: HumanSounds::default(),
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
        move_rect: MOVE_RECT,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    o.flags |= flags::ALWAYS_ACTIVE;
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::Marrow(MarrowData {
            enemy,
            creeping: true,
            ..MarrowData::default()
        });
    }
    w.link_marrow(id);
    start_of_fight(w, id);
    set_state(w, id, EnemyState::Startup);
    set_anim(w, id, HOME_ANIM);
}

// ----------------------------------------------------------------------
// Pre-tick

/// His pre-tick steps after the shared ones, in order:
///
/// 1. the fall reset: below the height he was placed at (and not dead),
///    he is put back 10 px above it, still, falling, and goes Home;
/// 2. the parrot call: once his health is [`PHASE_DAMAGE`] below his
///    phase health, the whistle is scheduled when he is free, and the
///    pre-tick ends either way;
/// 3. the dodge: when Claw attacks facing him from within 150 px (mode 3
///    or 4) and he is not waiting for the parrot, a draw of 0..100 below
///    his smarts lets the roll decide: below 200 the hop back, below 600
///    the hop up, each scheduled when he is free.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    let start_y = data(w, id)
        .and_then(|d| d.human.boss)
        .map_or(i32::MAX, |b| b.start.1);
    let dead = super::state(w, id) == Some(EnemyState::Dead);
    if obj(w, id).y > start_y && !dead {
        let across = super::exact_x(w, id);
        obj_mut(w, id).y = start_y - FALL_RESET_RISE;
        keep_across(w, id, across);
        if let Some(d) = data_mut(w, id) {
            d.vx = 0.0;
            d.vy = 0.0;
            d.ay = GRAVITY;
        }
        obj_mut(w, id).physics_type = ptype::FLY;
        placed(w, id, true);
        go_home(w, id);
    }
    if phase_over(w, id) {
        let free = data(w, id).is_some_and(|d| !d.busy);
        if free {
            if let Some(d) = data_mut(w, id) {
                d.deferred = None;
            }
            if let Some(m) = marrow_data_mut(w, id) {
                m.whistle_due = true;
            }
        }
        return;
    }
    let attacking = w.claw.attack.kind != attack_kind::NONE;
    let claw_facing_right = w.claw.facing_right;
    let Some((right, mode, busy, roll, smarts)) =
        data(w, id).map(|d| (d.claw_right, d.mode, d.busy, d.roll, d.smarts))
    else {
        return;
    };
    let faces_him = claw_facing_right != right;
    let waiting = super::state(w, id) == Some(EnemyState::WaitForParrot);
    if !(attacking && faces_him && (mode == 3 || mode == 4) && !waiting) {
        return;
    }
    if rand(w, 100) >= smarts {
        return;
    }
    let call = if roll < 0 {
        None
    } else if roll < DODGE_ROLLS[0] {
        Some(HumanCall::JumpBack)
    } else if roll < DODGE_ROLLS[1] {
        Some(HumanCall::Jump)
    } else {
        None
    };
    if let Some(call) = call
        && !busy
        && let Some(d) = data_mut(w, id)
    {
        d.deferred = Some(Deferred::Human(call));
    }
}

// ----------------------------------------------------------------------
// States

/// Goes Home, only once the fight is on (the boss fight flag at 2): he
/// stops across and decides again in Home. With the flag below 2 nothing
/// happens and the caller's state stays.
pub fn go_home(w: &mut World, id: ObjectId) {
    if w.boss_fight() != fight::ON {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    set_state(w, id, EnemyState::Home);
}

/// Startup: waits in his Home pose until the fight is on; then he goes
/// Home.
pub fn startup_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if w.boss_fight() == fight::ON {
        go_home(w, id);
    }
}

/// Goes idle: stops, keeps the picture he shows (no animation), tries his
/// idle line when the roll is below 500 (the level has none) and waits
/// 1000 ms before pacing may start. Draws nothing.
pub fn go_idle(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    set_state(w, id, EnemyState::Idle);
    obj_mut(w, id).anim = AnimPlayer::default();
    let roll = data(w, id).map_or(i32::MAX, |d| d.roll);
    if (0..IDLE_LINE_ROLL).contains(&roll) {
        say(w, id, IDLE_LINE);
    }
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.pace_wait = Timer::new(now, IDLE_WAIT_MS);
    }
}

/// Idle: Home when he sees Claw; otherwise, once his wait is over, a 5%
/// chance of pacing on each frame pulse tick, a quarter of the time with
/// his pacing line. Draws: one of 0..100, then one more of 0..100 when the
/// first is below 5.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if sees_claw(w, id) {
        go_home(w, id);
        return;
    }
    let now = w.now_ms;
    let waited = data(w, id).is_some_and(|d| d.pace_wait.done(now));
    if waited && w.pulse && rand(w, 100) < PACE_CHANCE {
        if rand(w, 100) < PACE_LINE_CHANCE {
            say(w, id, PACE_LINE);
        }
        start_pacing(w, id);
    }
}

/// Home: faces Claw and draws `r` of 0..121; within 300 or 150 px (modes
/// 2, 3) he strikes when `r` is below his smarts, or steps in when below
/// smarts + 20; within 100 px (mode 4) he backs off on a draw below 40 of
/// 0..100, otherwise strikes; off screen (mode 6) he fires when he may
/// and stops there. Then, when he sees Claw, he fires when he may and
/// stops there; otherwise the shared Home rule decides when he is still
/// Home.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let r = rand(w, HOME_SPREAD);
    let (mode, smarts) = data(w, id).map_or((6, 0), |d| (d.mode, d.smarts));
    match mode {
        2 | 3 if r < smarts => enter_strike(w, id),
        2 | 3 if r < smarts + MOVE_AHEAD_BAND => human::enter_move_ahead(w, id),
        4 => {
            if rand(w, 100) < MOVE_BACK_CHANCE {
                human::enter_move_back(w, id);
            } else {
                enter_strike(w, id);
            }
        }
        6 => {
            fire(w, id);
            return;
        }
        _ => {}
    }
    if sees_claw(w, id) {
        fire(w, id);
        return;
    }
    super::human_home_tick(w, id);
}

/// Fires when his wait between shots is over: the shot ([`enter_fire`]),
/// the next wait of 1 to 4 s, and his shot line on rolls 0 to 99.
/// Otherwise nothing. Draws: one of 0..4 when he fires.
pub fn fire(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if !data(w, id).is_some_and(|d| d.fire_wait.done(now)) {
        return;
    }
    enter_fire(w, id);
    let ms = (rand(w, 4) as u64 + 1) * u64::from(FIRE_WAIT_MS.0);
    let roll = data(w, id).map_or(i32::MAX, |d| d.roll);
    if let Some(d) = data_mut(w, id) {
        d.fire_wait = Timer::new(now, ms);
    }
    if FIRE_LINE_ROLLS.contains(&roll) {
        say(w, id, FIRE_LINE);
    }
}

/// The shot: facing Claw, the bullet loaded at [`MUZZLE`] and
/// [`BULLET_SPEED`] his way, the shot's animation.
pub fn enter_fire(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Fire);
    face_claw(w, id);
    human::load_shot(w, id, MUZZLE, BULLET_SPEED);
    set_anim(w, id, FIRE_ANIM);
}

/// Firing: the bullet leaves on the animation's shot frame, once; Home
/// when it ends.
pub fn fire_tick(w: &mut World, id: ObjectId) {
    human::fire_tick(w, id, SHOT);
}

/// The whistle: facing the side of the room's centre he is not on (the
/// parrot takes hold of him on that side), busy and Safe, the whistle's
/// animation and sound, and a 5000 ms wait for the parrot.
pub fn enter_wait(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::WaitForParrot);
    let right_half = obj(w, id).x > ROOM_CENTRE_X;
    set_facing(w, id, !right_half);
    let now = w.now_ms;
    if let Some(m) = marrow_data_mut(w, id) {
        m.grab_offset = if right_half {
            -GRAB_OFFSET
        } else {
            GRAB_OFFSET
        };
        m.called = false;
        m.wait = Timer::new(now, u64::from(WAIT_FOR_PARROT_MS));
    }
    set_anim(w, id, WHISTLE_ANIM);
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
    }
    obj_mut(w, id).flags |= flags::SAFE;
    say(w, id, WHISTLE_SOUND);
}

/// Waiting for the parrot: when the whistle's animation has ended (once)
/// he calls the parrot, whistles again and stands still; once the wait is
/// over he drops back to the floor ([`drop_down`]), on every tick from
/// then on while he is still waiting.
pub fn wait_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let done = obj(w, id).anim.is_done();
    if done && marrow_data(w, id).is_some_and(|m| !m.called) {
        if let Some(m) = marrow_data_mut(w, id) {
            m.called = true;
        }
        if let Some(l) = w.marrow_link_mut() {
            l.parrot_call = true;
        }
        say(w, id, WHISTLE_SOUND);
        if let Some(d) = data_mut(w, id) {
            d.vx = 0.0;
            d.vy = 0.0;
        }
    }
    let now = w.now_ms;
    if marrow_data(w, id).is_some_and(|m| m.wait.done(now)) {
        drop_down(w, id);
    }
}

/// The end of a wait for the parrot, by its time or by the end of the
/// parrot's carry: gravity again, no longer Safe, free, 32 px up and down
/// onto the floor below, standing, the parrot call clear, the phase health
/// from his health, and Home.
pub fn drop_down(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.ay = GRAVITY;
        d.busy = false;
    }
    obj_mut(w, id).flags &= !flags::SAFE;
    let across = super::exact_x(w, id);
    super::drop_to_floor(w, id);
    keep_across(w, id, across);
    placed(w, id, false);
    let health = obj(w, id).health;
    if let Some(l) = w.marrow_link_mut() {
        l.parrot_call = false;
        l.phase_health = health;
    }
    go_home(w, id);
}

/// HitLow and HitHigh: the shared flinch; when it ends he strikes at once
/// (whatever the fight flag says).
pub fn flinch_tick(w: &mut World, id: ObjectId) {
    hit_tick(w, id);
    if obj(w, id).anim.is_done() {
        enter_strike(w, id);
    }
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
        assert!(handler(EnemyState::WaitForParrot).is_some());
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
