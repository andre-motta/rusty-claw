//! Enemies: the officer, the soldier and the rat, on shared enemy
//! behaviour.
//!
//! Every enemy is a small state machine over named states ([`EnemyState`]).
//! Each logic call after the first runs, in order:
//!
//! 1. the pre-tick: one draw of 0..=1000 from the random stream (kept so the
//!    stream stays in step with the game; nothing reads it), which side
//!    Claw is on, and the range mode (see [`mode`]); human enemies then
//!    update their liftable flag and contact box, notice Claw leaving
//!    range and become stationary on a platform; each class may add steps
//!    after these;
//! 2. the handler of the current state: the class's own, or the shared one
//!    in `provisional` when the class has none;
//! 3. the post-tick: a scheduled call (going idle after Claw left range),
//!    then the movement step with the tiles, the ledge guard and the depth
//!    against Claw.
//!
//! The first call builds the enemy (its class's `construct`). Hits arrive
//! from the collision pass through [`on_hit`], which skips the same attack
//! of Claw twice and a running hit cooldown, applies the hit by what hit,
//! and kills the enemy when its health runs out.
//!
//! Random draws happen in the game's order, from the shared stream the
//! whole level draws from; every draw an enemy makes is listed with the
//! function that makes it.
//!
//! Only the officer is filled in. The soldier and the rat stand still and
//! cannot be hurt yet.

use claw_frame::SoundChannel;

use crate::logic::{GameLogic, ObjectData, World, sound};
use crate::object::{LogicKind, Object, ObjectId, Rect, draw_flags};
use crate::tuning::TICK_MS;

pub mod officer;
mod provisional;
pub mod rat;
pub mod soldier;

pub use provisional::{
    die, drop_loot, enter_hit, enter_pause, enter_strike, go_home, start_pacing,
};

/// Image sets every enemy may switch to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations every enemy may play.
pub const ANIM_PREFIXES: &[&str] = &[];

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
/// own shots and his magic.
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
    /// A shot of the level itself.
    pub const LEVEL_SHOT: u32 = types::LEVEL_SHOT;
    /// Claw's magic.
    pub const MAGIC: u32 = 0x200_0000;
}

/// The powerup code that hides Claw from enemies.
pub const INVISIBILITY_POWERUP: u32 = 0x26B7;

/// Time after the level starts during which enemies skip their per-tick
/// draw, ms.
pub const STARTUP_QUIET_MS: u64 = 100;

/// Time Claw must stay out of range, on screen, before the mode says so,
/// ms.
pub const OUT_OF_RANGE_MS: u64 = 500;

/// Gravity of enemies, px/s^2.
pub const GRAVITY: f64 = 2000.0;

/// Length of one tick, s.
pub const TICK_S: f64 = TICK_MS as f64 * 0.001;

/// Unbounded patrol limits.
pub const UNBOUNDED: (i32, i32) = (i32::MIN, i32::MAX);

/// An enemy's named states. The numbers are fixed: traces show them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum EnemyState {
    /// Waiting where it stands.
    #[default]
    Idle = 2000,
    /// Deciding what to do next, facing Claw.
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
}

impl EnemyState {
    /// Every state, in number order.
    pub const ALL: [EnemyState; 8] = [
        EnemyState::Idle,
        EnemyState::Home,
        EnemyState::Strike,
        EnemyState::Pace,
        EnemyState::HitLow,
        EnemyState::HitHigh,
        EnemyState::Dead,
        EnemyState::Pause,
    ];

    /// The state with this number.
    pub fn from_id(id: i32) -> Option<EnemyState> {
        EnemyState::ALL.iter().copied().find(|s| *s as i32 == id)
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
        }
    }
}

/// A call scheduled for the post-tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deferred {
    /// Go idle (Claw left range).
    GoIdle,
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
    /// Busy: striking, flinching or dead; the enemy may not start
    /// something else on its own.
    pub busy: bool,
    /// Claw is to the right (at or past the enemy's x).
    pub claw_right: bool,
    /// Range mode, 0 to 6 (see [`mode`]).
    pub mode: u8,
    /// Claw has left every range box (set on the first tick out of range).
    pub out_of_range: bool,
    /// When Claw was first seen out of range on screen.
    pub miss_since: Option<u64>,
    /// This tick's draw of 0..=1000.
    pub roll: i32,
    /// Patrol limits (x min, x max); [`UNBOUNDED`] without.
    pub bounds: (i32, i32),
    /// Never paces.
    pub stationary: bool,
    /// Turns at ledges.
    pub ledge_guard: bool,
    /// Drawn in front of or behind Claw by who stands higher.
    pub z_sort: bool,
    /// Depth base of the depth against Claw.
    pub z_base: i32,
    /// Has died once (no second loot drop).
    pub dead: bool,
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
    /// Sub-pixel movement carried to the next tick.
    pub carry: (f64, f64),
    /// No movement at all (frozen in place).
    pub frozen: bool,
    /// Call scheduled for the post-tick.
    pub deferred: Option<Deferred>,
    /// The enemy's strike token: one armed strike per swing.
    pub strike_token: bool,
    /// Contact box, facing left (the sprites' own direction).
    pub contact_rect: Rect,
    /// Strike box in use, facing left.
    pub strike_rect: Rect,
    /// Strike animation in use.
    pub strike_anim: &'static str,
    /// Death animation.
    pub death_anim: &'static str,
    /// Death sound.
    pub death_sound: &'static str,
    /// Stand pose timer.
    pub pose: Timer,
    /// Time idle before pacing may start.
    pub pace_wait: Timer,
    /// The enemy's spoken line ends at this time, ms.
    pub talking_until: u64,
    /// Smarts, a percentage some decisions roll against.
    pub smarts: i32,
    /// Loot codes, in drop order (0 for an empty slot).
    pub loot: [i32; 9],
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
            bounds: UNBOUNDED,
            stationary: false,
            ledge_guard: false,
            z_sort: false,
            z_base: 0,
            dead: false,
            may_lift: false,
            serial_seen: None,
            cooldown: Timer::default(),
            death: DeathKind::default(),
            death_timer: Timer::default(),
            pause: Timer::default(),
            vx: 0.0,
            vy: 0.0,
            ay: 0.0,
            carry: (0.0, 0.0),
            frozen: false,
            deferred: None,
            strike_token: true,
            contact_rect: Rect::UNSET,
            strike_rect: Rect::UNSET,
            strike_anim: "",
            death_anim: "",
            death_sound: "",
            pose: Timer::default(),
            pace_wait: Timer::default(),
            talking_until: 0,
            smarts: 0,
            loot: [0; 9],
        }
    }
}

/// A state handler.
pub type Handler = fn(&mut World, ObjectId);

/// One enemy class: its slots and its own state handlers.
#[derive(Debug)]
pub struct Class {
    /// Name, for messages.
    pub name: &'static str,
    /// Builds the enemy on its first logic call.
    pub construct: fn(&mut World, ObjectId),
    /// The class's pre-tick steps, run after the shared ones.
    pub pre_tick: fn(&mut World, ObjectId),
    /// The class's own handler of a state, when it has one.
    pub handler: fn(EnemyState) -> Option<Handler>,
    /// Goes idle.
    pub go_idle: fn(&mut World, ObjectId),
    /// Animations the shared states play.
    pub anims: ClassAnims,
    /// Sounds the shared states play.
    pub sounds: ClassSounds,
}

/// Animations of a class used by the shared states.
#[derive(Debug, Clone, Copy)]
pub struct ClassAnims {
    /// Pacing.
    pub pace: &'static str,
    /// Low hit reaction.
    pub hit_low: &'static str,
    /// High hit reaction.
    pub hit_high: &'static str,
    /// Standing at home before a pause.
    pub home: &'static str,
}

/// Sounds of a class used by the shared states.
#[derive(Debug, Clone, Copy)]
pub struct ClassSounds {
    /// The strike landing.
    pub strike: &'static str,
    /// A low hit, then a second one.
    pub hit_low: [&'static str; 2],
    /// A high hit, then one of the four hit sounds.
    pub hit_high: &'static str,
}

/// The four sounds one of which follows a high hit.
pub const HIT_SOUNDS: [&str; 4] = ["GAME_HIT1", "GAME_HIT2", "GAME_HIT3", "GAME_HIT4"];
/// The sound of loot being dropped.
pub const DROP_SOUND: &str = "GAME_PURELEASE1";
/// The sound of a dead enemy leaving the screen.
pub const SPLASH_SOUND: &str = "GAME_SPLASH";
/// The sound of an enemy dying on spikes or in goo.
pub const DEATH_TILE_SOUND: &str = "LEVEL_DEATHTILE";
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

/// The class of an enemy family.
pub fn class(g: GameLogic) -> Option<&'static Class> {
    match g {
        GameLogic::Officer => Some(&officer::CLASS),
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

/// Runs `f` with the enemy's data taken out of its object (so `f` can use
/// the whole world), then puts it back. `None` before the enemy is built.
pub fn with_data<R>(
    w: &mut World,
    id: ObjectId,
    f: impl FnOnce(&mut World, &mut EnemyData) -> R,
) -> Option<R> {
    let o = w.objects.get_mut(id)?;
    let ObjectData::Enemy(mut d) = std::mem::take(&mut o.data) else {
        return None;
    };
    let r = f(w, &mut d);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::Enemy(d);
    }
    Some(r)
}

/// The enemy's data.
pub fn data<'a>(w: &'a World, id: ObjectId) -> Option<&'a EnemyData> {
    match &w.objects.get(id)?.data {
        ObjectData::Enemy(d) => Some(d),
        _ => None,
    }
}

/// The enemy's data, mutable.
pub fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut EnemyData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Enemy(d) => Some(d),
        _ => None,
    }
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

/// True when the enemy is on screen: its hit box (or position) is inside
/// the view.
pub fn on_screen(w: &World, o: &Object) -> bool {
    if o.hit_rect.is_set() {
        o.hit_rect.translate(o.x, o.y).overlaps(&w.view)
    } else {
        w.view.contains(o.x, o.y)
    }
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

/// Starts animation `key` on the enemy and advances it one tick.
pub fn set_anim_now(w: &mut World, id: ObjectId, key: &str) {
    if w.set_anim(id, key) {
        w.advance_anim(id);
    }
}

/// Plays an enemy sound on the effects channel, centred.
pub fn play(w: &mut World, key: &str) {
    if !key.is_empty() {
        sound::play(w, key, SoundChannel::Effect);
    }
}

/// The enemy says `key` unless it is still saying something; spoken lines
/// play on the voice channel.
pub fn say(w: &mut World, id: ObjectId, key: &str) {
    let now = w.now_ms;
    let len = u64::from(w.sound_ms(key).unwrap_or(0));
    let Some(d) = data_mut(w, id) else { return };
    if d.talking_until > now {
        return;
    }
    d.talking_until = now + len;
    sound::play(w, key, SoundChannel::Voice);
}

// ----------------------------------------------------------------------
// The per-tick run

/// One logic call of an enemy of `class`: builds it on the first call,
/// then runs the pre-tick, the state's handler and the post-tick.
pub fn tick(w: &mut World, id: ObjectId, class: &Class) {
    let raw = obj(w, id).state;
    if raw == crate::object::engine_state::INIT {
        (class.construct)(w, id);
        return;
    }
    let Some(s) = EnemyState::from_id(raw) else {
        // Engine notices (destroying, sleeping) need nothing.
        return;
    };
    pre_tick(w, id);
    (class.pre_tick)(w, id);
    let s = state(w, id).unwrap_or(s);
    match (class.handler)(s) {
        Some(h) => h(w, id),
        None => provisional::handler(s)(w, id),
    }
    post_tick(w, id, class);
}

/// The shared pre-tick: the per-tick draw, the side and range mode, and
/// the human steps.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    let roll = if w.now_ms < STARTUP_QUIET_MS {
        0x1FFF_FFFF
    } else {
        rand(w, 1001)
    };
    let (ex, ey) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    let claw = w.claw_object().map(|c| (c.x, c.y));
    let invisible =
        w.claw.stats.powerup_ms > 0 && w.claw.stats.powerup_kind == INVISIBILITY_POWERUP;
    let screen = on_screen(w, obj(w, id));
    let now = w.now_ms;
    let Some(d) = data_mut(w, id) else { return };
    d.roll = roll;
    if let Some((cx, cy)) = claw {
        d.claw_right = cx >= ex;
        if !invisible {
            d.mode = mode(d, (ex, ey), (cx, cy), screen, now);
        }
    }
    // The human steps.
    let free = !d.busy;
    let mut go_idle = false;
    if d.mode >= 5 {
        if !d.out_of_range {
            d.out_of_range = true;
            go_idle = free;
        }
    } else {
        d.out_of_range = false;
    }
    if go_idle {
        d.deferred = Some(Deferred::GoIdle);
    }
    let contact = d.contact_rect;
    provisional::update_liftable(w, id);
    let o = obj_mut(w, id);
    o.attack_rect = facing_rect(o, contact);
    if o.flags & crate::object::flags::ON_ELEVATOR != 0
        && let Some(d) = data_mut(w, id)
    {
        d.stationary = true;
    }
}

/// Half sizes of the five range boxes every class here uses: Claw within
/// 100 px across and 40 px up or down.
pub const RANGE_BOX: (i32, i32) = (100, 40);

/// The range mode: 0 to 4 when Claw is inside the range boxes around the
/// enemy (here all five are the same box, so 4), 6 when he is outside and
/// the enemy is off screen, 5 once he has been outside for
/// [`OUT_OF_RANGE_MS`] with the enemy on screen (until then the mode keeps
/// its value).
pub fn mode(
    d: &mut EnemyFields,
    (ex, ey): (i32, i32),
    (cx, cy): (i32, i32),
    screen: bool,
    now: u64,
) -> u8 {
    let (hw, hh) = RANGE_BOX;
    let inside = cx >= ex - hw && cx < ex + hw && cy >= ey - hh && cy < ey + hh;
    if inside {
        d.miss_since = None;
        return 4;
    }
    if !screen {
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
    if deferred == Some(Deferred::GoIdle) {
        (class.go_idle)(w, id);
    }
    provisional::integrate(w, id);
}

// ----------------------------------------------------------------------
// Hits

/// The shared hit rule of human enemies: `attacker` hit `id`.
///
/// The same attack of Claw hits only once, and nothing hits during the hit
/// cooldown. Then, by what hit:
///
/// - Claw himself: during a hit reaction the enemy only turns back to
///   Home (no damage); otherwise it takes the damage, shows a hit burst,
///   starts a 300 ms cooldown and flinches high after an uppercut, low
///   otherwise;
/// - a pistol shot: damage, burst, the shot is destroyed, low flinch;
/// - magic or a thrown object: damage, burst, low flinch;
/// - a shot of the level: damage, burst, 2000 ms cooldown, low flinch.
///
/// At 0 health or below it dies. Draws: the burst's two, then one more for
/// a high flinch, then the death draws.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let Some(class) = class_of(w, id) else { return };
    let serial = w.claw.attack.serial;
    let now = w.now_ms;
    let Some(d) = data_mut(w, id) else { return };
    if d.serial_seen == Some(serial) {
        return;
    }
    d.serial_seen = Some(serial);
    if !d.cooldown.done(now) {
        return;
    }
    let Some(kind) = w.objects.get(attacker).map(|a| a.type_flags) else {
        return;
    };
    match kind {
        attacker::CLAW => {
            if matches!(state(w, id), Some(EnemyState::HitLow | EnemyState::HitHigh)) {
                go_home(w, id);
            } else {
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
                if let Some(d) = data_mut(w, id) {
                    d.cooldown = Timer::new(now, 300);
                }
                let high = w.claw.attack.uppercut;
                provisional::enter_hit(w, id, high, class);
            }
        }
        attacker::PISTOL_SHOT => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            w.destroy(attacker);
            provisional::enter_hit(w, id, false, class);
        }
        attacker::MAGIC | attacker::THROWN => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            provisional::enter_hit(w, id, false, class);
        }
        attacker::LEVEL_SHOT => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            if let Some(d) = data_mut(w, id) {
                d.cooldown = Timer::new(now, 2000);
            }
            provisional::enter_hit(w, id, false, class);
        }
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id, DeathKind::Hit, Some(attacker));
    }
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
/// up to 4 px at random. Draws: the height's, then the x's.
pub fn hit_burst(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let (x, z, hit) = {
        let o = obj(w, id);
        (o.x, o.z, o.hit_rect.translate(o.x, o.y))
    };
    let mid = w
        .objects
        .get(attacker)
        .filter(|a| a.attack_rect.is_set())
        .map_or(obj(w, id).y, |a| {
            let r = a.attack_rect.translate(a.x, a.y);
            (r.top + r.bottom) / 2
        });
    let mid = if hit.left != Rect::UNSET_LEFT {
        mid.clamp(hit.top, hit.bottom.max(hit.top))
    } else {
        mid
    };
    let dy = rand(w, 9) - 4;
    let dx = rand(w, 9) - 4;
    crate::logic::effect::spawn_hit_burst(w, x + dx, mid + dy, z + BURST_DEPTH, None, false);
}

/// `id` attacked `victim`: enemies need nothing here.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
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
        assert_eq!(EnemyState::from_id(0), None);
        assert_eq!(EnemyState::Strike.name(), "Strike");
    }

    #[test]
    fn factors_and_smarts() {
        assert_eq!(scaled(10, DAMAGE_FACTOR), 10);
        assert_eq!(scaled(7, 1.5), 10);
        assert_eq!(smarts(50), 99);
        assert_eq!(smarts(25), 99);
    }

    #[test]
    fn range_modes() {
        let mut d = EnemyFields::default();
        assert_eq!(mode(&mut d, (100, 100), (150, 120), true, 0), 4);
        d.mode = 4;
        // Out of range on screen: the mode holds for 500 ms, then is 5.
        assert_eq!(mode(&mut d, (100, 100), (300, 100), true, 1000), 4);
        assert_eq!(mode(&mut d, (100, 100), (300, 100), true, 1499), 4);
        assert_eq!(mode(&mut d, (100, 100), (300, 100), true, 1500), 5);
        // Off screen: 6 at once.
        assert_eq!(mode(&mut d, (100, 100), (300, 100), false, 1500), 6);
        // The box excludes its right and bottom edges.
        assert_eq!(mode(&mut d, (100, 100), (200, 100), false, 0), 6);
        assert_eq!(mode(&mut d, (100, 100), (0, 60), false, 0), 4);
        assert_eq!(mode(&mut d, (100, 100), (0, 140), false, 0), 6);
    }

    #[test]
    fn timers() {
        let t = Timer::new(100, 300);
        assert!(!t.done(399));
        assert!(t.done(400));
        assert!(Timer::default().done(0));
    }
}
