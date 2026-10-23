//! Omar, the temple's guardian who ends the fourteenth level, with the
//! ring of fire or ice that shields him ([`shield`]) and his energy balls
//! ([`bullet`]).
//!
//! He is a human enemy on the shared boss staging
//! ([`crate::logic::boss_stage`]): the warp drops Claw, falling, onto the
//! ladder top of the corridor above his chamber; Claw lands in the
//! trigger's box three ticks later and the staged introduction freezes
//! him. Omar takes no part in it: he waits idle from his first tick, and
//! nothing of his reads the fight flag. A falling Claw cannot jump, so the
//! introduction cannot be skipped; ending a life while it runs brings
//! Claw back on the ladder top at full health, long before it is over.
//!
//! He never walks, jumps or strikes: he floats at one of four places and
//! fights in two phases. In the first phase he stands over a ledge of the
//! chamber inside a ring of nine pieces of fire or ice ([`RING_PIECES`]
//! of them, [`RING_SPACING`] degrees apart on a circle of [`RING_RADIUS`]
//! px, with a gap of 96 degrees) that turns on the level clock
//! ([`ring_angle`]). Only a sword shot of the other element hurts him
//! ([`ELEMENT_DAMAGE`]); he then flashes in place and appears over the
//! other ledge, and his ring changes element and turns the other way. Any
//! other magic heals him. When his health runs out the ring is gone, his
//! health is full again and he appears in the hall above: there only
//! Claw's blows hurt him ([`BLOW_DAMAGE`]), each sending him to the
//! hall's other end, and he casts energy balls along it. His gem comes
//! [`GEM_TICKS`] ticks after the killing blow.
//!
//! # The rules
//!
//! - Built (by the warp's passes, or as Claw comes near): the current
//!   boss; [`HEALTH`] health and [`DAMAGE`] (times the difficulty
//!   factors); his hit and movement box [`BODY_RECT`]; a contact box of
//!   3 x 3 px around his position ([`CONTACT_RECT`]) that costs a Claw who
//!   touches it [`DAMAGE`], also while nothing can hit him; the range
//!   boxes [`RANGES`]; never liftable; stationary; always active; drawn
//!   at depth [`DEPTH`]. Then the reset (below), with his ring made
//!   around the place he was put at; he drops onto the floor below it and
//!   starts in `Startup`.
//! - His position: only the reset and the end of a move set it, and it
//!   reaches the object in the movement step of the same tick. No speed,
//!   gravity or wall moves him; knockbacks do nothing.
//! - The ring, each tick: before his state's handler every piece is put
//!   [`RING_RADIUS`] px out from his position along its angle (each axis
//!   truncated) and shows the ring's element; after the handler every
//!   angle turns by its speed times the game time since its last turn,
//!   read on the level clock in seconds. The reset sets the angles to
//!   [`RING_SPACING`] times the piece's place, the speeds to
//!   [`RING_SPEED`] and the last turn to the clock's zero, so the next
//!   turn puts the ring where the level clock says ([`ring_angle`]),
//!   whatever happened before.
//! - Startup: his idle pose plays; when the fight flag reaches 2, he goes
//!   idle. Claw out of every range box sends him idle on his first tick
//!   (the shared human rule), which is what happens through the warp.
//! - Idle: his pose plays; the taunt (below); he faces Claw. Going idle
//!   starts his humming loop ([`HUM_SOUND`]).
//! - Home: the same, and in the second phase a roll of 0..=49 (a frame
//!   pulse tick's roll, see [`super::pulse_roll`]) starts a cast.
//! - The taunt: a roll of 0..=9 draws one of 0..5 and, while he is on
//!   screen, he says that line of [`TAUNTS`].
//! - The cast ([`CAST_ANIM`]): the side he faces when it starts fixes
//!   where the ball leaves him ([`CAST_OFFSET`], mirrored facing left) and
//!   its speed ([`bullet::SPEED`]); he keeps facing Claw; on the tick the
//!   animation marks one energy ball leaves, with [`BALL_SOUND`]; Home
//!   when the animation ends.
//! - The move to a place: nothing can hit him and he flashes where he is,
//!   facing Claw, for [`MOVE_MS`] ([`MOVE_TICKS`] ticks at the 18 ms
//!   step); then he stands at the place, can be hit again and goes Home.
//!   It starts with [`MOVE_SOUND`] and leaves his pose as it is.
//! - Hits (see [`on_hit`]): the same attack of Claw hits only once and
//!   nothing hits during the hit cooldown. In the first phase Claw's blows
//!   do nothing; magic starts a cooldown of [`MAGIC_COOLDOWN_MS`], a hit
//!   burst and a low flinch, is used up, flinches him low again, and then
//!   a sword shot of the other element (ice on fire, fire on ice) costs
//!   him [`ELEMENT_DAMAGE`], flips the element and the ring's turn and
//!   moves him to the other ledge; anything else heals him by
//!   [`ELEMENT_DAMAGE`] up to [`HEALTH`] with [`HEAL_LINE`] when he is on
//!   screen. In the second phase a blow of Claw's shows a hit burst,
//!   flinches him (high after an uppercut), costs him [`BLOW_DAMAGE`] and
//!   moves him to the hall's other end; magic makes him block (the shot
//!   flies on). Pistol shots and dynamite are used up and make him block;
//!   thrown objects are used up. At 0 health or below he dies.
//! - Death in the first phase: the ring is gone, he moves to the hall's
//!   right end ([`HALL`]), his health is full again and his humming
//!   stops. In the second phase: his fall ([`DEATH_ANIM`]), out of every
//!   collision, his last line forced while on screen; when the fall ends
//!   ([`GEM_TICKS`] ticks), with Claw alive, Claw is held and the gem rises
//!   out of him toward [`GEM_TARGET`] (his record's speed fields), and he
//!   waits for good. With Claw dead then, no gem comes, ever.
//! - Claw's respawn: the reset, in the respawn's own update passes: no
//!   longer hit-proof nor flashing, the first phase, his place [`HOME`],
//!   idle, the ring of fire turning back from where the clock puts it (new
//!   pieces made around him when it was gone), full health; his energy
//!   balls still flying are gone with it. Being out of every collision
//!   and his death's end stay as they were: after a death with no gem he
//!   cannot be hit any more.
//!
//! # State numbers
//!
//! The fourteenth level's block [`super::tiger_guard::STATE_BLOCK`] (2320
//! to 2339) holds his new names from 2330 ([`STATES`]: the cast, the move
//! and the wait after his death). He reuses the numbers of names that
//! already have one: `Startup`, `Idle`, `Home`, `HitHigh`, `HitLow`,
//! `BlockLow` and `Dead`.
//!
//! # Random draws, in order
//!
//! - each call: the shared pre-tick's (frame pulse ticks only);
//! - Idle and Home: one of 0..5 when the roll is 0..=9 (the taunt);
//! - a blow in the second phase (in the collision pass): the hit burst's
//!   two, then one of 0..4 for a high flinch;
//! - magic in the first phase: the hit burst's two;
//! - his energy balls: their sparks (see [`bullet`]).
//!
//! # Defaults taken
//!
//! - The sword shots' explosion the original shows on him when magic hits
//!   him in the first phase is not shown (nothing in it changes the
//!   fight).

pub mod bullet;
pub mod shield;

use claw_frame::{AudioCue, SoundCue};

use super::states::{enter_hit, hit_tick};
use super::{
    Body, Class, ClassAnims, ClassSounds, EnemyData, EnemyFields, EnemyState, HIT_LOW_SECOND,
    HUMAN_HIT_BY, Handler, Timer, UNBOUNDED, accept_hit, attacker, build, data, data_mut,
    face_claw, hit_burst, human, human_fields, obj, obj_mut, on_screen, play, rand, scaled,
    set_anim, set_state,
};
use crate::logic::enemy::human::{BossFields, HumanData};
use crate::logic::world_boss::fight;
use crate::logic::{ObjectData, World, boss_stage, pickup, sound};
use crate::object::{ObjectId, Rect, draw_flags, engine_state, flags};
use crate::player::attack::{FIRE_SHOT_FLAG, ICE_SHOT_FLAG};
use crate::player::state as claw_state;
use crate::tuning::TICK_MS;

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
pub const BUILT: bool = true;

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
/// What a sword shot of the other element costs him in the first phase,
/// and what any other magic heals him by.
pub const ELEMENT_DAMAGE: i32 = 25;
/// What a blow of Claw's costs him in the second phase.
pub const BLOW_DAMAGE: i32 = 25;
/// What one of his energy balls costs Claw.
pub const BALL_DAMAGE: i32 = 15;
/// His hit and movement box.
pub const BODY_RECT: Rect = Rect::new(-11, -47, 23, 61);
/// His contact box: the 3 x 3 px around his position.
pub const CONTACT_RECT: Rect = Rect::new(-1, -1, 1, 1);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (56, 40), (56, 40)];
/// His depth: drawn and run after Claw, before his ring.
pub const DEPTH: i32 = 4101;
/// Where his reset puts him: over the chamber's right ledge.
pub const HOME: (i32, i32) = (42250, 2475);
/// The place over the chamber's left ledge.
pub const LEDGE_LEFT: (i32, i32) = (41400, 2475);
/// Where the first phase's end sends him: the hall's right end.
pub const HALL: (i32, i32) = (42320, 1067);
/// The hall's left end.
pub const HALL_LEFT: (i32, i32) = (41325, 1067);
/// Where his gem flies to.
pub const GEM_TARGET: (i32, i32) = (41825, 930);
/// Ticks from the killing blow to the gem at the 18 ms step (his death
/// fall's length).
pub const GEM_TICKS: u64 = 176;
/// How long a move to a place takes, ms.
pub const MOVE_MS: u64 = 2000;
/// Ticks his move takes at the 18 ms step: he flashes in place, then
/// appears at the new place.
pub const MOVE_TICKS: u64 = 112;
/// The hit cooldown magic starts in the first phase, ms.
pub const MAGIC_COOLDOWN_MS: u64 = 500;
/// The pieces of his ring.
pub const RING_PIECES: usize = 9;
/// How far out the ring's pieces circle him, px.
pub const RING_RADIUS: f64 = 80.0;
/// The degrees between two pieces of the ring.
pub const RING_SPACING: f64 = 33.0;
/// How fast the ring turns, degrees per second (backwards: anticlockwise
/// on screen); the ice ring turns the other way.
pub const RING_SPEED: f64 = -250.0;
/// Where his energy ball leaves him, facing right: across and up.
pub const CAST_OFFSET: (i32, i32) = (65, -20);
/// The roll below or at which Home starts a cast in the second phase.
pub const CAST_ROLL: i32 = 49;
/// The roll below or at which Idle and Home taunt.
pub const TAUNT_ROLL: i32 = 9;

/// His decision and idle pose.
pub const IDLE_ANIM: &str = "LEVEL_OMAR_IDLE";
/// His death fall, at whose end the gem comes.
pub const DEATH_ANIM: &str = "LEVEL_OMAR_KILLFALL3";
/// His cast.
pub const CAST_ANIM: &str = "LEVEL_OMAR_STRIKE3";
/// His block.
pub const BLOCK_ANIM: &str = "LEVEL_OMAR_BLOCK";
/// His high flinch.
pub const HIT_HIGH_ANIM: &str = "LEVEL_OMAR_HITHIGH";
/// His low flinch.
pub const HIT_LOW_ANIM: &str = "LEVEL_OMAR_HITLOW";

/// His taunts, by the draw of 0..5.
pub const TAUNTS: [&str; 5] = [
    "LEVEL_OMAR_00520014",
    "LEVEL_OMAR_00520015",
    "LEVEL_OMAR_00520008",
    "LEVEL_OMAR_00520010",
    BALL_SOUND,
];
/// His line when magic heals him.
pub const HEAL_LINE: &str = "LEVEL_OMAR_00520004";
/// His cast's release.
pub const BALL_SOUND: &str = "LEVEL_OMAR_ENERGYBALL";
/// The hum that loops while he is idle, until the first phase ends.
pub const HUM_SOUND: &str = "LEVEL_OMAR_ENERGYLOOP";
/// The handle of the looping hum.
pub const HUM_VOICE: u32 = 0x0B00_0014;
/// The thunder of every move to a place.
pub const MOVE_SOUND: &str = "LEVEL_OMAR_SKYTHUNDER";
/// His last line.
pub const DEATH_LINE: &str = "LEVEL_OMAR_THENFALL";
/// The low flinch's sound.
pub const HIT_LOW_SOUND: &str = "GAME_SOLHITL2";
/// The high flinch's sound.
pub const HIT_HIGH_SOUND: &str = "GAME_SOLHITHI";

/// The angle of piece `i` of his ring, degrees, at `ms` of the level
/// clock while the ring keeps the speed the reset gives it: the pieces
/// sit [`RING_SPACING`] degrees apart and turn at [`RING_SPEED`] from the
/// level's start, whatever happens to Omar, so the gap in the ring is
/// where the clock puts it.
pub fn ring_angle(i: usize, ms: u64) -> f64 {
    RING_SPACING * i as f64 + RING_SPEED * ms as f64 / 1000.0
}

/// Where piece `i` of his ring stands at `ms` of the level clock around
/// Omar at `at`: [`RING_RADIUS`] px out along [`ring_angle`], each axis
/// truncated.
pub fn ring_point(i: usize, ms: u64, at: (i32, i32)) -> (i32, i32) {
    piece_point(ring_angle(i, ms), at)
}

/// The value of pi the ring's degrees are turned into radians with: the
/// game's own, a little short of the true one, which decides on which
/// side of a whole pixel a piece at a right angle falls.
// Deliberately short of the true value: the ring's pieces fall on the
// pixels the game's own value puts them on.
#[allow(clippy::approx_constant)]
pub const RING_PI: f64 = 3.141_592_6;

/// Where a piece at `angle` degrees stands around Omar at `at`:
/// [`RING_RADIUS`] px out along it (in radians with [`RING_PI`]), each
/// axis truncated.
pub fn piece_point(angle: f64, at: (i32, i32)) -> (i32, i32) {
    let a = angle * RING_PI / 180.0;
    (
        (RING_RADIUS * a.cos() + f64::from(at.0)).trunc() as i32,
        (RING_RADIUS * a.sin() + f64::from(at.1)).trunc() as i32,
    )
}

/// The Omar class.
pub static CLASS: Class = Class {
    name: "Omar",
    human: true,
    construct,
    pre_tick: super::nothing,
    handler,
    go_home,
    go_idle,
    carry: None,
    pick_lift_voice: super::nothing,
    lift_roll: false,
    anims: ClassAnims {
        hit_low: HIT_LOW_ANIM,
        hit_high: HIT_HIGH_ANIM,
        ..ClassAnims::NONE
    },
    sounds: ClassSounds {
        strike: "",
        hit_low: [HIT_LOW_SOUND, HIT_LOW_SECOND],
        hit_high: HIT_HIGH_SOUND,
    },
};

/// The turn of one piece of his ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spin {
    /// Its angle, degrees.
    pub angle: f64,
    /// Its speed, degrees per second.
    pub speed: f64,
    /// The level clock at its last turn, s (0 after the reset).
    pub base: f64,
}

/// The turns of a ring the reset leaves: piece `i` at
/// [`RING_SPACING`] `i` degrees, every speed [`RING_SPEED`], every last
/// turn at the clock's zero.
pub fn reset_spins() -> [Spin; RING_PIECES] {
    std::array::from_fn(|i| Spin {
        angle: RING_SPACING * i as f64,
        speed: RING_SPEED,
        base: 0.0,
    })
}

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
    /// The turns of his ring's pieces.
    pub spins: [Spin; RING_PIECES],
    /// He stands at the hall's right end: the next blow sends him to the
    /// left one (and the other way round).
    pub right_end: bool,
    /// Where his position goes in the movement step.
    pub place: (i32, i32),
    /// Where his move takes him.
    pub target: (i32, i32),
    /// His move's time.
    pub moving: Timer,
    /// His cast: where the ball leaves him across, px, and its speed,
    /// px/s.
    pub cast: (i32, i32),
    /// His cast's ball has left.
    pub fired: bool,
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
            spins: reset_spins(),
            right_end: true,
            place: HOME,
            target: HOME,
            moving: Timer::default(),
            cast: (0, 0),
            fired: false,
        }
    }
}

/// Omar's own data of `id`.
pub fn omar_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a OmarData> {
    match &w.objects.get(id)?.data {
        ObjectData::Omar(d) => Some(d),
        _ => None,
    }
}

/// Omar's own data of `id`, mutable.
pub fn omar_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut OmarData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Omar(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Startup => Some(startup_tick),
        EnemyState::Idle => Some(idle_tick),
        EnemyState::Home => Some(home_tick),
        EnemyState::MagicAttack => Some(cast_tick),
        EnemyState::MoveToPosition => Some(move_tick),
        EnemyState::HitHigh | EnemyState::HitLow => Some(hit_tick),
        EnemyState::BlockLow => Some(human::block_tick),
        EnemyState::Dead => Some(death_tick),
        EnemyState::Wait => Some(super::nothing),
        _ => None,
    }
}

fn phase(w: &World, id: ObjectId) -> u8 {
    omar_data(w, id).map_or(1, |d| d.phase)
}

// ----------------------------------------------------------------------
// The logic call

/// One logic call (see the module documentation): the reset when Claw
/// has respawned, then the shared pre-tick, his ring put around him, the
/// state's handler and his movement step with the ring's turn. Being
/// removed, he is no longer the current boss.
pub fn tick(w: &mut World, id: ObjectId) {
    let raw = obj(w, id).state;
    if raw == engine_state::DESTROYING && w.current_boss() == Some(id) {
        w.set_current_boss(None);
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
        // Everything fired goes with the life Claw lost.
        crate::logic::projectile::purge(w.objects);
    }
    super::pre_tick(w, id, &CLASS);
    place_ring(w, id);
    let s = super::state(w, id).unwrap_or_default();
    if let Some(h) = handler(s) {
        h(w, id);
    }
    post_tick(w, id);
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

/// The pieces of his ring put around him (see the module documentation).
fn place_ring(w: &mut World, id: ObjectId) {
    let Some(d) = omar_data(w, id) else { return };
    if d.pieces.is_empty() {
        return;
    }
    let (pieces, spins, ice) = (d.pieces.clone(), d.spins, d.ice);
    let at = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    for (i, &p) in pieces.iter().enumerate() {
        let (x, y) = piece_point(spins[i].angle, at);
        if let Some(o) = w.objects.get_mut(p) {
            o.x = x;
            o.y = y;
        }
        shield::show_element(w, p, ice);
    }
}

/// His movement step: the scheduled call, his position taken from his
/// place, then the ring's turn on the level clock.
fn post_tick(w: &mut World, id: ObjectId) {
    let deferred = data_mut(w, id).and_then(|d| d.deferred.take());
    if deferred == Some(super::Deferred::GoIdle) {
        go_idle(w, id);
    }
    let now_s = (w.now_ms + u64::from(TICK_MS)) as f64 * 0.001;
    let Some(d) = omar_mut(w, id) else { return };
    let place = d.place;
    for s in &mut d.spins {
        let dt = now_s - s.base;
        s.base = now_s;
        s.angle += s.speed * dt;
    }
    let o = obj_mut(w, id);
    o.x = place.0;
    o.y = place.1;
}

// ----------------------------------------------------------------------
// Building and the reset

/// Builds Omar (see the module documentation). Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    let placed = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    let full_health = scaled(HEALTH, super::HEALTH_FACTOR);
    let fields = EnemyFields {
        ranges: RANGES,
        bounds: UNBOUNDED,
        contact_rect: CONTACT_RECT,
        strike_rect: Rect::UNSET,
        may_lift: false,
        z_sort: false,
        smarts: super::smarts(50).max(99),
        human: HumanData {
            anims: human::HumanAnims {
                block_low: BLOCK_ANIM,
                idle: IDLE_ANIM,
                ..human::HumanAnims::default()
            },
            boss: Some(BossFields {
                start: HOME,
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
        hit_rect: BODY_RECT,
        attack_rect: CONTACT_RECT,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let dropped = {
        let o = obj_mut(w, id);
        o.flags |= flags::ALWAYS_ACTIVE;
        o.flags &= !flags::SAFE;
        if o.z != DEPTH {
            o.z = DEPTH;
            o.flags |= flags::RESORT;
        }
        if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
            o.data = ObjectData::Omar(OmarData {
                enemy,
                ..OmarData::default()
            });
        }
        (o.x, o.y)
    };
    w.set_current_boss(Some(id));
    // The reset makes his ring around the place he was put at, before the
    // drop to the floor below it.
    {
        let o = obj_mut(w, id);
        o.x = placed.0;
        o.y = placed.1;
    }
    reset(w, id);
    {
        let o = obj_mut(w, id);
        o.x = dropped.0;
        o.y = dropped.1;
    }
    set_state(w, id, EnemyState::Startup);
    set_anim(w, id, IDLE_ANIM);
}

/// The reset (see the module documentation). Draws nothing.
pub fn reset(w: &mut World, id: ObjectId) {
    {
        let o = obj_mut(w, id);
        o.flags &= !flags::SAFE;
        o.draw_flags &= !draw_flags::FLASH;
    }
    if let Some(d) = omar_mut(w, id) {
        d.phase = 1;
        d.right_end = true;
        d.place = HOME;
    }
    go_idle(w, id);
    let (at, z) = {
        let o = obj(w, id);
        ((o.x, o.y), o.z)
    };
    let missing = omar_data(w, id).is_some_and(|d| d.pieces.is_empty());
    if let Some(d) = omar_mut(w, id) {
        d.ice = false;
        d.spins = reset_spins();
    }
    if missing {
        let spins = reset_spins();
        let pieces: Vec<ObjectId> = (0..RING_PIECES)
            .map(|i| {
                let (x, y) = piece_point(spins[i].angle, at);
                shield::spawn(w, id, i, (x, y, z + 1))
            })
            .collect();
        if let Some(d) = omar_mut(w, id) {
            d.pieces = pieces;
        }
    }
    let full = data(w, id)
        .and_then(|d| d.human.boss)
        .map_or(HEALTH, |b| b.full_health);
    obj_mut(w, id).health = full;
}

// ----------------------------------------------------------------------
// Idle, Home and Startup

/// Goes idle: his pose, stopped, and his hum looping. Draws nothing.
pub fn go_idle(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Idle);
    set_anim(w, id, IDLE_ANIM);
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    if w.level.sounds.contains_key(HUM_SOUND) {
        w.cue(AudioCue::Stop { voice: HUM_VOICE });
        w.cue(AudioCue::Play(SoundCue {
            key: HUM_SOUND.to_string(),
            looped: true,
            voice: Some(HUM_VOICE),
            ..SoundCue::default()
        }));
    }
}

/// Goes Home: his pose, stopped; it leaves him as busy as he was. Draws
/// nothing.
pub fn go_home(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Home);
    set_anim(w, id, IDLE_ANIM);
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
}

/// Startup: the pose plays; when the fight flag reaches 2, he goes idle.
pub fn startup_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if w.boss_fight() == fight::ON {
        go_idle(w, id);
    }
}

/// The taunt (see the module documentation). Draws: one of 0..5 when the
/// roll is 0..=9.
fn taunt(w: &mut World, id: ObjectId) {
    let roll = data(w, id).map_or(i32::MAX, |d| d.roll);
    if (0..=TAUNT_ROLL).contains(&roll) {
        let line = TAUNTS[rand(w, TAUNTS.len() as u32) as usize];
        if on_screen(w, id) {
            sound::enemy_say(w, id, line, false);
        }
    }
}

/// Idle: the pose, the taunt, facing Claw.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    taunt(w, id);
    face_claw(w, id);
}

/// Home: the pose, the taunt, facing Claw; in the second phase a roll of
/// 0..=49 starts a cast.
pub fn home_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    taunt(w, id);
    face_claw(w, id);
    let (mode, roll) = data(w, id).map_or((6, i32::MAX), |d| (d.mode, d.roll));
    if phase(w, id) == 0 && mode <= 6 && (0..=CAST_ROLL).contains(&roll) {
        enter_cast(w, id);
    }
}

// ----------------------------------------------------------------------
// The cast

/// Starts a cast (see the module documentation).
pub fn enter_cast(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::MagicAttack);
    set_anim(w, id, CAST_ANIM);
    let right = super::faces_right(obj(w, id));
    let cast = if right {
        (CAST_OFFSET.0, bullet::SPEED)
    } else {
        (-CAST_OFFSET.0, -bullet::SPEED)
    };
    if let Some(d) = omar_mut(w, id) {
        d.cast = cast;
        d.fired = false;
    }
}

/// MagicAttack: facing Claw; the ball on the tick the animation marks,
/// once; Home when the animation ends.
pub fn cast_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let event = w.advance_anim(id);
    if event == 1 && omar_data(w, id).is_some_and(|d| !d.fired) {
        play(w, BALL_SOUND);
        let cast = omar_data(w, id).map_or((0, 0), |d| d.cast);
        if let Some(d) = omar_mut(w, id) {
            d.fired = true;
        }
        let (x, y, z) = {
            let o = obj(w, id);
            (o.x, o.y, o.z)
        };
        bullet::launch(w, (x + cast.0, y + CAST_OFFSET.1, z - 1), cast.1);
    }
    if obj(w, id).anim.is_done() {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// The move to a place

/// Starts a move to `to` (see the module documentation).
pub fn move_to(w: &mut World, id: ObjectId, to: (i32, i32)) {
    {
        let o = obj_mut(w, id);
        o.flags |= flags::SAFE;
        o.draw_flags |= draw_flags::FLASH;
    }
    set_state(w, id, EnemyState::MoveToPosition);
    let now = w.now_ms;
    if let Some(d) = omar_mut(w, id) {
        d.target = to;
        d.moving = Timer::new(now, MOVE_MS);
    }
    play(w, MOVE_SOUND);
}

/// MoveToPosition: facing Claw; once the move's time is over, at the
/// place, hittable again and Home.
pub fn move_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let now = w.now_ms;
    let Some((done, to)) = omar_data(w, id).map(|d| (d.moving.done(now), d.target)) else {
        return;
    };
    if !done {
        return;
    }
    {
        let o = obj_mut(w, id);
        o.flags &= !flags::SAFE;
        o.draw_flags &= !draw_flags::FLASH;
    }
    if let Some(d) = omar_mut(w, id) {
        d.place = to;
    }
    go_home(w, id);
}

// ----------------------------------------------------------------------
// Death

/// His death (see the module documentation).
pub fn die(w: &mut World, id: ObjectId) {
    if phase(w, id) == 1 {
        let pieces = omar_mut(w, id).map_or(Vec::new(), |d| std::mem::take(&mut d.pieces));
        for p in pieces {
            w.destroy(p);
        }
        move_to(w, id, HALL);
        if let Some(d) = omar_mut(w, id) {
            d.right_end = true;
            d.phase = 0;
        }
        let full = data(w, id)
            .and_then(|d| d.human.boss)
            .map_or(HEALTH, |b| b.full_health);
        obj_mut(w, id).health = full;
        w.cue(AudioCue::Stop { voice: HUM_VOICE });
        return;
    }
    set_state(w, id, EnemyState::Dead);
    {
        let o = obj_mut(w, id);
        o.draw_flags &= !draw_flags::FLASH;
        o.flags |= flags::NO_COLLIDE;
    }
    set_anim(w, id, DEATH_ANIM);
    if on_screen(w, id) {
        sound::enemy_say(w, id, DEATH_LINE, true);
    }
}

/// Dead: his fall plays; once, when it ends, with Claw alive, Claw is held
/// and the gem rises out of him, and he waits for good.
pub fn death_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let end = obj(w, id).anim.is_done() && omar_data(w, id).is_some_and(|d| !d.gem_given);
    if !end {
        return;
    }
    if let Some(d) = omar_mut(w, id) {
        d.gem_given = true;
    }
    if w.claw.stats.health <= 0 {
        return;
    }
    release_gem(w, id);
    set_state(w, id, EnemyState::Wait);
}

/// His defeat ([`World::boss_defeated`]): Claw held and the gem out of
/// him. The gem's flight runs on game time, so in the tick it is made it
/// does not move: when it would get its first call in this very update
/// pass, that call is kept for the next tick and the camera heads for it
/// now, as its first call would have it.
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

/// `attacker` hit `id` (see the module documentation). Draws: the hit
/// burst's two (and one of 0..4 for a high flinch).
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let Some((kind, special)) = w
        .objects
        .get(attacker)
        .map(|a| (a.type_flags, a.special_flags))
    else {
        return;
    };
    if !accept_hit(w, id) {
        return;
    }
    let first = phase(w, id) == 1;
    match kind {
        attacker::CLAW if !first => blow(w, id, attacker),
        attacker::MAGIC if first => magic(w, id, attacker, special),
        attacker::MAGIC => human::enter_block(w, id, false),
        attacker::PISTOL_SHOT => {
            w.destroy(attacker);
            human::enter_block(w, id, false);
        }
        attacker::THROWN => w.destroy(attacker),
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id);
    }
}

/// A blow of Claw's in the second phase.
fn blow(w: &mut World, id: ObjectId, attacker: ObjectId) {
    hit_burst(w, id, attacker);
    let high = w.claw.attack.uppercut;
    enter_hit(w, id, high);
    let o = obj_mut(w, id);
    if o.health > 0 {
        o.health -= BLOW_DAMAGE;
    }
    let right_end = omar_data(w, id).is_none_or(|d| d.right_end);
    if let Some(d) = omar_mut(w, id) {
        d.right_end = !right_end;
    }
    move_to(w, id, if right_end { HALL_LEFT } else { HALL });
}

/// Magic in the first phase: a sword shot of the other element hurts him,
/// anything else heals him.
fn magic(w: &mut World, id: ObjectId, attacker: ObjectId, special: u32) {
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.cooldown = Timer::new(now, MAGIC_COOLDOWN_MS);
    }
    hit_burst(w, id, attacker);
    enter_hit(w, id, false);
    w.destroy(attacker);
    enter_hit(w, id, false);
    let ice = omar_data(w, id).is_some_and(|d| d.ice);
    let good = if ice {
        special & FIRE_SHOT_FLAG != 0
    } else {
        special & ICE_SHOT_FLAG != 0
    };
    if good {
        obj_mut(w, id).health -= ELEMENT_DAMAGE;
        let (to, speed) = if ice {
            (HOME, RING_SPEED)
        } else {
            (LEDGE_LEFT, -RING_SPEED)
        };
        move_to(w, id, to);
        if let Some(d) = omar_mut(w, id) {
            d.ice = !ice;
            for s in &mut d.spins {
                s.speed = speed;
            }
        }
    } else {
        let o = obj_mut(w, id);
        o.health = (o.health + ELEMENT_DAMAGE).min(HEALTH);
        if on_screen(w, id) {
            sound::enemy_say(w, id, HEAL_LINE, false);
        }
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
    v.extend([HEAL_LINE, HUM_SOUND, MOVE_SOUND, DEATH_LINE]);
    v
}

/// His own animations, for tests and tools that check them against the
/// level.
pub fn anim_names() -> [&'static str; 6] {
    [
        IDLE_ANIM,
        DEATH_ANIM,
        CAST_ANIM,
        BLOCK_ANIM,
        HIT_HIGH_ANIM,
        HIT_LOW_ANIM,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn his_states_take_the_second_half_of_the_fourteenth_levels_block() {
        let block = super::super::tiger_guard::STATE_BLOCK;
        for (i, s) in STATES.iter().enumerate() {
            assert_eq!(*s as i32, block.start() + 10 + i as i32, "{}", s.name());
            assert!(handler(*s).is_some(), "{}", s.name());
        }
        assert!(CLASS.human, "a human enemy");
        for s in [
            EnemyState::Startup,
            EnemyState::Idle,
            EnemyState::Home,
            EnemyState::HitHigh,
            EnemyState::HitLow,
            EnemyState::BlockLow,
            EnemyState::Dead,
        ] {
            assert!(handler(s).is_some(), "{}", s.name());
        }
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
        // The reset's turns: from the clock's zero, the next turn puts
        // each piece where the formula does.
        let mut spins = reset_spins();
        for s in &mut spins {
            let dt = 1.458 - s.base;
            s.base = 1.458;
            s.angle += s.speed * dt;
        }
        for (i, s) in spins.iter().enumerate() {
            assert!((s.angle - ring_angle(i, 1458)).abs() < 1e-9, "{i}");
        }
    }

    #[test]
    fn his_numbers() {
        assert_eq!(HEALTH / ELEMENT_DAMAGE, 4);
        assert_eq!(HEALTH / BLOW_DAMAGE, 4);
        let d = OmarData::default();
        assert_eq!((d.phase, d.ice, d.gem_given), (1, false, false));
        assert!(d.pieces.is_empty());
        assert!(d.right_end);
        assert_eq!(d.place, HOME);
        for anim in anim_names() {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
        // His move: 112 ticks at the 18 ms step.
        assert_eq!(MOVE_MS.div_ceil(18), MOVE_TICKS);
        // The ring's gap: 96 degrees between the last piece and the first.
        assert_eq!(360.0 - RING_SPACING * (RING_PIECES - 1) as f64, 96.0);
    }
}
