//! Marrow's parrot, on the tenth level: the bird that circles over the
//! spike pit of Marrow's room ([`super::marrow`]), swoops at Claw and
//! carries Marrow across the pit when he whistles.
//!
//! It is a plain flying enemy, with the seagull's numbers (its beak box,
//! its hit box, depth [`Z`], one health, damage [`DAMAGE`]) but its own
//! state machine. It patrols a line near the top of the room between the
//! ends of its record's box ([`EnemyState::FlyAbout`], [`PATROL_TICKS`] a
//! leg); at a line end, with Claw on the other half of the room and the
//! fight on, it sets up a dive ([`EnemyState::StrikeSetup`]) and swoops at
//! him beak first ([`EnemyState::Strike`]). Claw's hits never hurt it: each
//! fresh attack of his that reaches it counts once, flinches it
//! ([`EnemyState::GenericHit`]) and sends it back to its patrol; the
//! [`HITS_HOME`]th sends it to its perch ([`EnemyState::FlyToIdle`]), which
//! clears Marrow's open request so the slabs close into a bridge. Perched,
//! it answers Marrow's call: it flies to him ([`EnemyState::FlyToMarrow`])
//! and carries him to the ledge away from Claw
//! ([`EnemyState::CarryMarrow`], [`CARRY_TICKS`] ticks), setting the open
//! request so the slabs open under the bridge.
//!
//! # The tick
//!
//! Each call runs the shared enemy frame ([`super::tick`]): the pre-tick
//! (the per-tick roll on frame pulse ticks, which side Claw is on, then
//! its own reset after a death of Claw's), the state's handler, then the
//! movement step with no tile test, held to its record's box across and
//! up and down; the line ends, the dive's end and the arrivals are exact
//! tests against that box. While it carries Marrow, he then moves by what
//! the bird's step moved it.
//!
//! # Random draws, in order
//!
//! The pre-tick's roll of 0..=1000 (on frame pulse ticks); patrolling, one
//! of 0..100 every tick (and one of 0..2 for a squawk), and one of 0..100
//! at a line end with Claw on the other half of the room; one of 0..31 as
//! a dive starts; perched, one of 0..100 at the end of each pose (and one
//! of 0..2 for a squawk); hit, the hit burst's.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): it perches in the shared
//! `Idle`, swoops in the shared `Strike` and flinches in the seagull's
//! `GenericHit`; its five flights are new names, the five numbers after
//! Marrow's whistle in the tenth level's block
//! ([`super::marrow::STATE_BLOCK`]), in the order the game registers them
//! ([`STATES`]).
//!
//! # Defaults taken
//!
//! - The five flights take 2241 to 2245, after Marrow's whistle at 2240.
//! - A carry whose Marrow is gone (his object removed by other means than
//!   a death, which cannot happen while he is carried and Safe) ends at
//!   once.
//! - The beak hurts Claw by the shared enemy-contact rule, limited by
//!   Claw's own protection after a hit.
//! - The slabs' own behaviour is [`crate::logic::marrow_floor`]'s; the
//!   parrot only writes the open request, the parrot call and the phase
//!   health of the Marrow link ([`super::marrow::MarrowLink`]).
//!
//! Its data ([`ParrotData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use super::seagull::{exact_y, fixed_speed_velocity};
use super::states::leap;
use super::{
    Class, ClassAnims, ClassSounds, EnemyData, EnemyFields, EnemyState, Handler, Timer, data,
    data_mut, exact_x, face_claw, facing_rect, hit_burst, marrow, motion, nothing, obj, obj_mut,
    play, rand, say, scaled, set_anim, set_facing, set_state,
};
use crate::logic::world_boss::fight;
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, Rect, flags, special, types};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Parrot"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_PARROT";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_PARROT_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Its five new states, in the order the game registers them.
pub const STATES: [EnemyState; 5] = [
    EnemyState::FlyAbout,
    EnemyState::StrikeSetup,
    EnemyState::FlyToIdle,
    EnemyState::FlyToMarrow,
    EnemyState::CarryMarrow,
];

/// Health before the difficulty factor.
pub const HEALTH: i32 = 1;
/// Damage of its beak, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Its depth when its record's is 0: after Claw, the slabs and Marrow.
pub const Z: i32 = 8500;
/// Its beak's box, facing left.
pub const BEAK: Rect = Rect::new(-26, -5, -5, 22);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-30, -30, 30, 30);
/// Movement box when the level does not know its first picture.
pub const FALLBACK_MOVE_RECT: Rect = Rect::new(-32, -30, 31, 29);
/// Kinds of object that can hit it: Claw, his thrown objects and pistol
/// shots, the level's own shots and explosions, and his magic.
pub const HIT_BY: u32 = 0x300_0032;
/// Half sizes of the range boxes, outermost first (it reads none).
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 50), (200, 40), (150, 40), (100, 40)];

/// Its patrol speed, px/s.
pub const FLY_SPEED: i32 = 300;
/// The length of its swoop and dive flights (across plus down), px/s.
pub const STRIKE_SPEED: i32 = 800;
/// The length of its flight to Marrow (across plus down), px/s.
pub const TO_MARROW_SPEED: i32 = 300;
/// The x of its two dive points, left and right of the room's centre.
pub const DIVE_X: (i32, i32) = (40688, 40928);
/// How far inside its line's ends the dive points lie, px.
pub const DIVE_INSET: i32 = 200;
/// The dive point lies up to this many px above Claw (a draw of 0..31).
pub const DIVE_SPREAD: u32 = 31;
/// Chance in 100, at a line end, that it dives when it may.
pub const DIVE_CHANCE: i32 = 75;
/// Hits that send it to its perch.
pub const HITS_HOME: i32 = 3;
/// Where it carries Marrow: the right ledge when Claw is left of the
/// room's centre, the left ledge otherwise.
pub const CARRY_X: (i32, i32) = (41063, 40553);
/// How far above Marrow it takes hold of him, px.
pub const GRAB_ABOVE: i32 = 97;
/// How far above Marrow's exact height it carries him to, px.
pub const CARRY_ABOVE: i32 = 110;
/// The gravity of its flight to the perch, px/s^2.
pub const HOME_GRAVITY: f64 = 1000.0;
/// The gravity of the carry, px/s^2.
pub const CARRY_GRAVITY: f64 = 200.0;
/// Chance in 100 per patrol tick of a squawk, once its wait is over and
/// the fight is on.
pub const SQUAWK_CHANCE: i32 = 1;
/// The wait between two squawks on the patrol, ms.
pub const SQUAWK_WAIT_MS: u64 = 15_000;
/// Chance in 100 per perch pose of a squawk.
pub const PERCH_SQUAWK_CHANCE: i32 = 10;

/// Ticks of one patrol leg at the 18 ms step (measured).
pub const PATROL_TICKS: u32 = 119;
/// Ticks of a dive's set-up at the 18 ms step (measured).
pub const STRIKE_SETUP_TICKS: u32 = 28;
/// Ticks of a swoop at the 18 ms step (measured).
pub const STRIKE_TICKS: u32 = 31;
/// Ticks of its flinch at the 18 ms step (measured).
pub const HIT_TICKS: u32 = 19;
/// Ticks of its flight home to the perch at the 18 ms step (measured).
pub const HOME_TICKS: u32 = 56;
/// Ticks of the carry at the 18 ms step (measured).
pub const CARRY_TICKS: u32 = 119;

/// Perched.
pub const IDLE_ANIM: &str = "LEVEL_PARROT_IDLE";
/// Every flight.
pub const FLY_ANIM: &str = "LEVEL_PARROT_FLY";
/// The flinch.
pub const HIT_ANIM: &str = "LEVEL_PARROT_HIT";
/// The swoop.
pub const STRIKE_ANIM: &str = "LEVEL_PARROT_STRIKE1";
/// The flinch's sound.
pub const HIT_SOUND: &str = "LEVEL_PARROT_PARROTHIT";
/// The swoop's sound.
pub const SWOOP_SOUND: &str = "LEVEL_PARROT_PARROTSWOOP";
/// Its squawks, one of two.
pub const SQUAWKS: [&str; 2] = ["LEVEL_PARROT_POLLY2", "LEVEL_PARROT_POLLY3"];

/// The parrot class.
pub static CLASS: Class = Class {
    name: "Parrot",
    human: false,
    construct,
    pre_tick,
    handler,
    go_home: nothing,
    go_idle,
    carry: None,
    pick_lift_voice: nothing,
    lift_roll: true,
    anims: ClassAnims::NONE,
    sounds: ClassSounds::NONE,
};

/// What a flinch goes on to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum After {
    /// Back to the patrol.
    #[default]
    Fly,
    /// Home to the perch.
    Perch,
}

/// The parrot's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParrotData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// Where it perches: its record's position.
    pub perch: (i32, i32),
    /// Claw's attacks that reached it since it last left its perch.
    pub hits: i32,
    /// Claw's attempt counter when it last looked.
    pub attempt: u32,
    /// Where the flight it is on ends: the dive point, the strike's aim,
    /// the grab point or the carry's end.
    pub target: (i32, i32),
    /// What its flinch goes on to.
    pub after: After,
    /// The wait between two squawks on the patrol.
    pub squawk: Timer,
    /// The grab point beside Marrow the flight to him aims at, exact (x,
    /// y), kept to the part of a pixel. Once the flight has set out
    /// (its velocity aimed at the exact point), only the x is read: the
    /// flight takes hold of him once its exact x has passed the exact
    /// grab x. The y is kept for completeness; the target holds the
    /// point in whole pixels.
    pub grab: (f64, f64),
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Idle => Some(perch_tick),
        EnemyState::FlyAbout => Some(fly_about_tick),
        EnemyState::StrikeSetup => Some(strike_setup_tick),
        EnemyState::Strike => Some(strike_tick),
        EnemyState::FlyToIdle => Some(fly_to_idle_tick),
        EnemyState::FlyToMarrow => Some(fly_to_marrow_tick),
        EnemyState::CarryMarrow => Some(carry_tick),
        EnemyState::GenericHit => Some(generic_hit_tick),
        _ => None,
    }
}

fn parrot<'a>(w: &'a World, id: ObjectId) -> Option<&'a ParrotData> {
    match &w.objects.get(id)?.data {
        ObjectData::Parrot(d) => Some(d),
        _ => None,
    }
}

fn parrot_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut ParrotData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Parrot(d) => Some(d),
        _ => None,
    }
}

/// The parrot data of `id`, if it is built.
pub fn data_of<'a>(w: &'a World, id: ObjectId) -> Option<&'a ParrotData> {
    parrot(w, id)
}

fn bounds(w: &World, id: ObjectId) -> ((i32, i32), (i32, i32)) {
    data(w, id).map_or(((0, 0), (0, 0)), |d| (d.bounds, d.bounds_y))
}

fn set_velocity(w: &mut World, id: ObjectId, (vx, vy): (f64, f64)) {
    if let Some(d) = data_mut(w, id) {
        d.vx = vx;
        d.vy = vy;
    }
}

fn set_gravity(w: &mut World, id: ObjectId, ay: f64) {
    if let Some(d) = data_mut(w, id) {
        d.ay = ay;
    }
}

fn at(w: &World, id: ObjectId) -> (f64, f64) {
    (exact_x(w, id), exact_y(w, id))
}

fn claw_at(w: &World) -> Option<(i32, i32)> {
    w.claw_object().map(|c| (c.x, c.y))
}

/// Marrow, while the link holds him.
fn marrow_of(w: &World) -> Option<ObjectId> {
    let id = w.marrow_link()?.marrow;
    w.objects.get(id).map(|_| id)
}

/// Turns to face `x`: right when it lies right of the bird.
fn face_x(w: &mut World, id: ObjectId, x: i32) {
    let right = f64::from(x) > exact_x(w, id);
    set_facing(w, id, right);
}

/// The beak hurts no more.
fn beak_off(w: &mut World, id: ObjectId) {
    obj_mut(w, id).attack_rect = Rect::UNSET;
}

// ----------------------------------------------------------------------
// The logic call

/// One logic call: the shared enemy frame ([`super::tick`]), then, when
/// it carries Marrow, the carry: Marrow moves by what the bird's movement
/// step moved it.
pub fn tick(w: &mut World, id: ObjectId) {
    let before = data(w, id).map(|_| at(w, id));
    super::tick(w, id, &CLASS);
    let Some(before) = before else { return };
    if super::state(w, id) != Some(EnemyState::CarryMarrow) {
        return;
    }
    let now = at(w, id);
    let moved = (now.0 - before.0, now.1 - before.1);
    if let Some(m) = marrow_of(w) {
        carry_marrow(w, m, moved);
    }
}

/// Moves the carried Marrow `m` by `moved`, exactly: he flies without tile
/// tests and without gravity, keeping his speed, and his own movement step
/// then runs (it covers no time in a tick that already had one).
fn carry_marrow(w: &mut World, m: ObjectId, moved: (f64, f64)) {
    let (x, y) = at(w, m);
    let (nx, ny) = (x + moved.0, y + moved.1);
    {
        let o = obj_mut(w, m);
        o.physics_type = ptype::DIRECT;
        o.x = nx.trunc() as i32;
        o.y = ny.trunc() as i32;
    }
    let to = {
        let o = obj(w, m);
        (o.x, o.y)
    };
    if let Some(d) = data_mut(w, m) {
        d.ay = 0.0;
        d.carry = (nx.fract(), ny.fract());
        d.moved_to = Some(to);
    }
    motion::integrate(w, m);
}

/// Builds the parrot: 1 health and 10 damage (times the difficulty
/// factors), flying with no gravity and no tile test, held to its record's
/// box, its hit box, no attack box, depth 8500 when placed at 0, perched
/// at its record's place with no hits; then it flies. Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    // The construction step every enemy shares moves a bird placed beside
    // its box onto the nearer limits before its perch is read.
    super::clamp_on_record_range(w, id);
    let (x_min, x_max) = (
        super::record(w, id, super::field::X_MIN),
        super::record(w, id, super::field::X_MAX),
    );
    let (y_min, y_max) = (
        super::record(w, id, super::field::Y_MIN),
        super::record(w, id, super::field::Y_MAX),
    );
    let fields = EnemyFields {
        bounds: (x_min, x_max),
        bounds_y: (y_min, y_max),
        ranges: RANGES,
        ay: 0.0,
        strike_rect: BEAK,
        smarts: super::smarts(50),
        ..EnemyFields::default()
    };
    let frames = w.image_frames(IMAGE_SET);
    let move_rect = frames
        .and_then(|(first, _)| w.frame_box(IMAGE_SET, first))
        .unwrap_or(FALLBACK_MOVE_RECT);
    let attempt = w.attempt();
    let o = obj_mut(w, id);
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    o.type_flags = types::ENEMY;
    o.hit_type_flags = HIT_BY;
    o.special_flags &= !special::LIFTABLE;
    o.hit_handler = true;
    o.physics_type = ptype::DIRECT;
    o.move_rect = move_rect;
    o.hit_rect = HIT_RECT;
    o.attack_rect = Rect::UNSET;
    if o.z == 0 {
        o.z = Z;
        o.flags |= flags::RESORT;
    }
    o.health = scaled(HEALTH, super::HEALTH_FACTOR);
    o.damage = scaled(DAMAGE, super::DAMAGE_FACTOR);
    let perch = (o.x, o.y);
    o.data = ObjectData::Parrot(ParrotData {
        enemy: fields.into(),
        perch,
        attempt,
        ..ParrotData::default()
    });
    start_flying(w, id);
}

/// Its pre-tick step after the shared one: after a death of Claw's (his
/// attempt counter changed) it is back at its perch, with full health and
/// no hits, and flies again.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    let attempt = w.attempt();
    let Some((seen, perch)) = parrot(w, id).map(|p| (p.attempt, p.perch)) else {
        return;
    };
    if seen == attempt {
        return;
    }
    {
        let o = obj_mut(w, id);
        o.x = perch.0;
        o.y = perch.1;
        o.health = scaled(HEALTH, super::HEALTH_FACTOR);
    }
    if let Some(p) = parrot_mut(w, id) {
        p.hits = 0;
        p.attempt = attempt;
    }
    if let Some(d) = data_mut(w, id) {
        d.ay = 0.0;
        d.carry = (0.0, 0.0);
    }
    start_flying(w, id);
}

// ----------------------------------------------------------------------
// The patrol

/// Starts flying: below its line it climbs straight up; on the line it
/// flies along it towards the room's centre; the beak hurts no more.
pub fn start_flying(w: &mut World, id: ObjectId) {
    let (_, (y_min, _)) = bounds(w, id);
    if exact_y(w, id) != f64::from(y_min) {
        set_velocity(w, id, (0.0, -f64::from(FLY_SPEED)));
    } else {
        patrol_on(w, id);
        if let Some(d) = data_mut(w, id) {
            d.vy = 0.0;
        }
    }
    beak_off(w, id);
    set_state(w, id, EnemyState::FlyAbout);
    set_anim(w, id, FLY_ANIM);
}

/// Along the line towards the room's centre: left from its right half,
/// right otherwise.
fn patrol_on(w: &mut World, id: ObjectId) {
    let right_half = obj(w, id).x > marrow::ROOM_CENTRE_X;
    let vx = if right_half { -FLY_SPEED } else { FLY_SPEED };
    if let Some(d) = data_mut(w, id) {
        d.vx = f64::from(vx);
    }
    set_facing(w, id, !right_half);
}

/// The patrol: the flight animation loops; a squawk now and then once the
/// fight is on; on its line it flies level, and at each end of the line it
/// turns, and with Claw on the other half of the room it may dive (below
/// the line it only climbs). Draws:
/// one of 0..100 (then one of 0..2 for a squawk); at a line end with Claw
/// on the other half, one of 0..100.
pub fn fly_about_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        set_anim(w, id, FLY_ANIM);
    }
    let now = w.now_ms;
    let r = rand(w, 100);
    let waited = parrot(w, id).is_some_and(|p| p.squawk.done(now));
    if r < SQUAWK_CHANCE && waited && w.boss_fight() == fight::ON {
        let pick = rand(w, 2) as usize;
        say(w, id, SQUAWKS[pick]);
        if let Some(p) = parrot_mut(w, id) {
            p.squawk = Timer::new(now, SQUAWK_WAIT_MS);
        }
    }
    let ((x_min, x_max), (y_min, _)) = bounds(w, id);
    if exact_y(w, id) != f64::from(y_min) {
        return;
    }
    let still = data(w, id).is_some_and(|d| d.vx == 0.0);
    if let Some(d) = data_mut(w, id) {
        d.vy = 0.0;
    }
    if still {
        patrol_on(w, id);
    }
    let claw_x = claw_at(w).map_or(0, |c| c.0);
    if exact_x(w, id) == f64::from(x_min) {
        if let Some(d) = data_mut(w, id) {
            d.vx = f64::from(FLY_SPEED);
        }
        set_facing(w, id, true);
        if claw_x >= marrow::ROOM_CENTRE_X && rand(w, 100) < DIVE_CHANCE {
            enter_strike_setup(w, id);
        }
    }
    if exact_x(w, id) == f64::from(x_max) {
        if let Some(d) = data_mut(w, id) {
            d.vx = -f64::from(FLY_SPEED);
        }
        set_facing(w, id, false);
        if claw_x <= marrow::ROOM_CENTRE_X && rand(w, 100) < DIVE_CHANCE {
            enter_strike_setup(w, id);
        }
    }
}

// ----------------------------------------------------------------------
// The dive

/// Sets up a dive: it faces Claw; only once the fight is on and with
/// Marrow there, it then flies at 800 px/s (across plus down) to a point
/// 200 px inside the end of its line on its own side of the room, up to
/// 30 px above Claw. Draws: one of 0..31 when it dives.
pub fn enter_strike_setup(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    if w.boss_fight() != fight::ON || marrow_of(w).is_none() {
        return;
    }
    let Some((cx, cy)) = claw_at(w) else { return };
    let ((x_min, x_max), _) = bounds(w, id);
    let dive_x = if cx > marrow::ROOM_CENTRE_X {
        x_min + DIVE_INSET
    } else {
        x_max - DIVE_INSET
    };
    set_state(w, id, EnemyState::StrikeSetup);
    set_anim(w, id, FLY_ANIM);
    let dive_y = cy - rand(w, DIVE_SPREAD);
    if let Some(p) = parrot_mut(w, id) {
        p.target = (dive_x, dive_y);
    }
    let v = fixed_speed_velocity(
        at(w, id),
        (f64::from(dive_x), f64::from(dive_y)),
        f64::from(STRIKE_SPEED),
    );
    set_velocity(w, id, v);
}

/// The dive's set-up: the swoop once it is below the dive point or on an
/// end of its line.
pub fn strike_setup_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let ((x_min, x_max), _) = bounds(w, id);
    let dive_y = parrot(w, id).map_or(i32::MAX, |p| p.target.1);
    let (x, y) = at(w, id);
    if y > f64::from(dive_y) || x == f64::from(x_max) || x == f64::from(x_min) {
        enter_strike(w, id);
    }
}

/// The swoop: it faces Claw with its beak out (it can be hit again), and
/// flies at 800 px/s (across plus down) at Claw's x at the dive point's
/// height, its swoop sound playing.
pub fn enter_strike(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let beak = facing_rect(obj(w, id), BEAK);
    {
        let o = obj_mut(w, id);
        o.attack_rect = beak;
        o.flags &= !flags::NO_COLLIDE;
    }
    set_state(w, id, EnemyState::Strike);
    set_anim(w, id, STRIKE_ANIM);
    let cx = claw_at(w).map_or(0, |c| c.0);
    let dive_y = parrot(w, id).map_or(0, |p| p.target.1);
    if let Some(p) = parrot_mut(w, id) {
        p.target = (cx, dive_y);
    }
    let v = fixed_speed_velocity(
        at(w, id),
        (f64::from(cx), f64::from(dive_y)),
        f64::from(STRIKE_SPEED),
    );
    set_velocity(w, id, v);
    play(w, SWOOP_SOUND);
}

/// The swoop goes on across the room until an end of its line, then it
/// flies again (climbing back).
pub fn strike_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let ((x_min, x_max), _) = bounds(w, id);
    let x = exact_x(w, id);
    if x == f64::from(x_max) || x == f64::from(x_min) {
        start_flying(w, id);
    }
}

// ----------------------------------------------------------------------
// Hits

/// `attacker` hit `id`: a fresh attack of Claw's (whatever reached the
/// bird: his sword, a shot, which flies on, dynamite, magic or a thrown
/// object) counts once, with a hit burst; it never hurts. The third sends
/// it home to its perch, the others back to its patrol. Draws: the
/// burst's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let serial = w.claw.attack.serial;
    let Some(d) = data_mut(w, id) else { return };
    if d.serial_seen == Some(serial) {
        return;
    }
    d.serial_seen = Some(serial);
    hit_burst(w, id, attacker);
    let hits = parrot_mut(w, id).map_or(0, |p| {
        p.hits += 1;
        p.hits
    });
    let after = if hits >= HITS_HOME {
        After::Perch
    } else {
        After::Fly
    };
    enter_generic_hit(w, id, after);
}

/// `id` attacked `victim`: the beak needs nothing here.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

/// The flinch: it can no longer be hit (until its next swoop), stops,
/// and its hit sound and animation play.
pub fn enter_generic_hit(w: &mut World, id: ObjectId, after: After) {
    if let Some(p) = parrot_mut(w, id) {
        p.after = after;
    }
    obj_mut(w, id).flags |= flags::NO_COLLIDE;
    set_velocity(w, id, (0.0, 0.0));
    set_state(w, id, EnemyState::GenericHit);
    play(w, HIT_SOUND);
    set_anim(w, id, HIT_ANIM);
}

/// The flinch: when its animation ends it flies on (or home to its
/// perch), showing the flinch's animation once more.
pub fn generic_hit_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    match parrot(w, id).map_or(After::Fly, |p| p.after) {
        After::Fly => start_flying(w, id),
        After::Perch => enter_fly_to_idle(w, id),
    }
    set_anim(w, id, HIT_ANIM);
}

// ----------------------------------------------------------------------
// The perch

/// Flies home to its perch: harmless, facing it, on the slowest arc that
/// reaches it under a gravity of 1000 px/s^2.
pub fn enter_fly_to_idle(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::FlyToIdle);
    beak_off(w, id);
    set_anim(w, id, FLY_ANIM);
    let Some(perch) = parrot(w, id).map(|p| p.perch) else {
        return;
    };
    face_x(w, id, perch.0);
    set_gravity(w, id, HOME_GRAVITY);
    let (x, y) = at(w, id);
    let v = leap(f64::from(perch.0) - x, f64::from(perch.1) - y, HOME_GRAVITY);
    set_velocity(w, id, v);
}

/// True once the flight has passed `x`: moving right at or past it,
/// moving left past it.
fn passed(w: &World, id: ObjectId, x: i32) -> bool {
    passed_exact(w, id, f64::from(x))
}

/// True once the flight's exact x has passed the exact `x`: moving right
/// at or past it, moving left below it.
fn passed_exact(w: &World, id: ObjectId, x: f64) -> bool {
    let vx = data(w, id).map_or(0.0, |d| d.vx);
    past(vx, exact_x(w, id), x)
}

/// True when a flight at `vx` px/s whose exact x is `at` has passed `x`:
/// moving right at or past it, moving left below it.
fn past(vx: f64, at: f64, x: f64) -> bool {
    (vx > 0.0 && at >= x) || (vx < 0.0 && at < x)
}

/// The flight home: once past the perch, it settles there.
pub fn fly_to_idle_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let perch = parrot(w, id).map_or(0, |p| p.perch.0);
    if passed(w, id, perch) {
        set_gravity(w, id, 0.0);
        go_idle(w, id);
    }
}

/// Settles on its perch: no hits counted, exactly at the perch, still,
/// facing left, its perch pose, out of reach of every hit; Marrow's open
/// request is cleared, so the slabs close.
pub fn go_idle(w: &mut World, id: ObjectId) {
    let Some(perch) = parrot(w, id).map(|p| p.perch) else {
        return;
    };
    if let Some(p) = parrot_mut(w, id) {
        p.hits = 0;
    }
    {
        let o = obj_mut(w, id);
        o.x = perch.0;
        o.y = perch.1;
        o.flags |= flags::NO_COLLIDE;
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
        d.carry = (0.0, 0.0);
    }
    set_facing(w, id, false);
    set_anim(w, id, IDLE_ANIM);
    if marrow_of(w).is_some()
        && let Some(l) = w.marrow_link_mut()
    {
        l.open_request = false;
    }
    set_state(w, id, EnemyState::Idle);
}

/// Perched: at the end of each perch pose a squawk now and then; when
/// Marrow calls, it flies to him. Draws: one of 0..100 at the end of each
/// pose (then one of 0..2 for a squawk).
pub fn perch_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        set_anim(w, id, IDLE_ANIM);
        if rand(w, 100) < PERCH_SQUAWK_CHANCE {
            let pick = rand(w, 2) as usize;
            say(w, id, SQUAWKS[pick]);
        }
    }
    let called = marrow_of(w).is_some() && w.marrow_link().is_some_and(|l| l.parrot_call);
    if called {
        enter_fly_to_marrow(w, id);
    }
}

// ----------------------------------------------------------------------
// The carry

/// Flies to Marrow: to the point beside him that his whistle named, 97 px
/// above him, at 300 px/s (across plus down), facing it. The point is
/// kept exact, to the part of a pixel Marrow stands at across and below
/// his whole pixel.
pub fn enter_fly_to_marrow(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::FlyToMarrow);
    set_anim(w, id, FLY_ANIM);
    let Some(m) = marrow_of(w) else { return };
    let (mx, my) = at(w, m);
    let grab = (
        mx + f64::from(marrow::grab_offset(w, m)),
        my - f64::from(GRAB_ABOVE),
    );
    face_x(w, id, grab.0.trunc() as i32);
    let v = fixed_speed_velocity(at(w, id), grab, f64::from(TO_MARROW_SPEED));
    set_velocity(w, id, v);
    if let Some(p) = parrot_mut(w, id) {
        p.target = (grab.0.trunc() as i32, grab.1.trunc() as i32);
        p.grab = grab;
    }
}

/// The flight to Marrow: once its exact x has passed the exact grab x it
/// carries him. Coming from the right, it takes hold of him as soon as
/// it is below the exact point, which can be a tick before it would be
/// below that point's whole pixel; coming from the left, only once it is
/// at or past the exact point, which can be a tick after it reaches the
/// whole pixel.
pub fn fly_to_marrow_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let grab = parrot(w, id).map_or(0.0, |p| p.grab.0);
    if passed_exact(w, id, grab) {
        set_gravity(w, id, 0.0);
        enter_carry(w, id);
    }
}

/// Carries Marrow to the ledge away from Claw (the right one when Claw is
/// left of the room's centre; Claw's x on this first step of the carry
/// decides it, once), 110 px above his exact height (the part of a pixel
/// he stands below his whole pixel counts: a Marrow who crept 0.972 px
/// down is aimed 0.972 px lower), on the slowest arc under a gravity of
/// 200 px/s^2; his limit on that side is lifted, the slabs are asked to
/// open and he says his line. Without Marrow it flies on.
pub fn enter_carry(w: &mut World, id: ObjectId) {
    let Some(m) = marrow_of(w) else {
        start_flying(w, id);
        return;
    };
    set_state(w, id, EnemyState::CarryMarrow);
    set_anim(w, id, FLY_ANIM);
    let claw_left = claw_at(w).is_some_and(|c| c.0 < marrow::ROOM_CENTRE_X);
    let tx = if claw_left { CARRY_X.0 } else { CARRY_X.1 };
    if let Some(d) = data_mut(w, m) {
        if claw_left {
            d.bounds.1 = super::UNBOUNDED.1;
        } else {
            d.bounds.0 = super::UNBOUNDED.0;
        }
    }
    let ty = at(w, m).1 - f64::from(CARRY_ABOVE);
    face_x(w, id, tx);
    set_gravity(w, id, CARRY_GRAVITY);
    let (x, y) = at(w, id);
    let v = leap(f64::from(tx) - x, ty - y, CARRY_GRAVITY);
    set_velocity(w, id, v);
    if let Some(p) = parrot_mut(w, id) {
        p.target = (tx, ty.trunc() as i32);
    }
    if let Some(l) = w.marrow_link_mut() {
        l.open_request = true;
    }
    say(w, m, marrow::CARRY_LINE);
}

/// The carry: once past its end it lets Marrow drop
/// ([`marrow::drop_down`]) and flies again. Without Marrow it flies on.
pub fn carry_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let Some(m) = marrow_of(w) else {
        set_gravity(w, id, 0.0);
        start_flying(w, id);
        return;
    };
    let tx = parrot(w, id).map_or(0, |p| p.target.0);
    if passed(w, id, tx) {
        set_gravity(w, id, 0.0);
        start_flying(w, id);
        marrow::drop_down(w, m);
    }
}

#[cfg(test)]
mod tests {
    use super::super::marrow;
    use super::*;

    #[test]
    fn its_flights_follow_the_whistle_in_the_tenth_levels_block() {
        let block = marrow::STATE_BLOCK;
        for (i, s) in STATES.into_iter().enumerate() {
            assert_eq!(s as i32, block.start() + 1 + i as i32, "{}", s.name());
            assert!(block.contains(&(s as i32)));
        }
        // It perches, swoops and flinches under shared numbers.
        assert_eq!(EnemyState::Idle as i32, 2000);
        assert_eq!(EnemyState::Strike as i32, 2002);
        assert_eq!(EnemyState::GenericHit as i32, 2143);
        assert!(!CLASS.human, "a plain flying enemy");
        assert!(CLASS.carry.is_none());
    }

    #[test]
    fn it_carries_marrow_away_from_claw() {
        // The dive points and the carry's ends lie either side of the
        // room's centre.
        assert!(DIVE_X.0 < marrow::ROOM_CENTRE_X && marrow::ROOM_CENTRE_X < DIVE_X.1);
        assert!(CARRY_X.1 < marrow::ROOM_CENTRE_X && marrow::ROOM_CENTRE_X < CARRY_X.0);
        assert_eq!(HITS_HOME, 3);
        // Its depth sorts it after Marrow.
        const { assert!(Z > marrow::SORTED_Z) };
    }

    /// The grab point's part of a pixel moves the grab a tick either way:
    /// earlier coming from the right, later coming from the left.
    #[test]
    fn the_exact_grab_point_can_come_a_tick_earlier_or_later() {
        let grab = 41043.44;
        // From the right: at 41043.249 the bird is below the exact point
        // but not yet below its whole pixel.
        assert!(past(-300.0, 41043.249, grab));
        assert!(!past(-300.0, 41043.249, grab.trunc()));
        // From the left: at 41043.3 the bird has reached the whole pixel
        // but not yet the exact point; a tick on, it has.
        assert!(!past(300.0, 41043.3, grab));
        assert!(past(300.0, 41043.3, grab.trunc()));
        assert!(past(300.0, 41043.3 + 300.0 * 0.018, grab));
        // Standing still, never.
        assert!(!past(0.0, grab, grab));
    }

    #[test]
    fn its_art_is_its_own() {
        for key in [
            IDLE_ANIM,
            FLY_ANIM,
            HIT_ANIM,
            STRIKE_ANIM,
            HIT_SOUND,
            SWOOP_SOUND,
        ]
        .iter()
        .chain(&SQUAWKS)
        {
            assert!(key.starts_with(ANIM_PREFIXES[0]), "{key}");
        }
    }
}
