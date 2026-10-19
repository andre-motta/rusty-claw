//! The tentacles of the twelfth level: Aquatis's ([`super::aquatis`])
//! three green arms rising out of the water along the ledge where Claw
//! arrives for his fight.
//!
//! A tentacle stays rooted in place. When Claw is up to [`REACH`] px to
//! its left and it has idled a second or so, it either whips sideways at
//! him (three times in four, [`STRIKE_BOX`] for [`DAMAGE`]) or grabs him
//! ([`EnemyState::StartGrab`]): Claw is frozen where he stands, lifted
//! after [`START_GRAB_TICKS`] ticks to [`HOLD_AT`] from the tentacle and
//! squeezed ([`EnemyState::Squeeze`]) for one health every
//! [`SQUEEZE_STEP_MS`] until he mashes his way out ([`MASH_PRESSES`]
//! ticks with a fresh press), [`SQUEEZE_MS`] pass, or he dies. A second
//! tentacle can grab a Claw the first still holds, and any strike on him
//! ends every hold.
//!
//! It has no real health: any single hit puts it down, and it grows back
//! ([`EnemyState::Respawn`]) after [`REGROW_MS`] or at once when Aquatis
//! starts a frenzy. During his frenzy every tentacle thrashes
//! ([`EnemyState::SuperFrenzy`]), swiping left and right, and nothing can
//! hurt it; facing right, its box reaches everything to its right on its
//! own row. When Aquatis dies the tentacles die with him. They talk to him
//! only through the Aquatis link ([`super::aquatis::AquatisLink`]), whose
//! tentacle hold ([`crate::logic::World::tentacle_grab`]) they share.
//!
//! # The rules
//!
//! "Aquatis is current" below means the link names his object
//! ([`World::aquatis`]); "alive" and "his frenzy" are the link's flags.
//!
//! - Built: its box from its first picture for moving and being hit
//!   ([`BOX`]), no contact box, hit by Claw, his pistol shots, his magic
//!   and the level's shots and explosions ([`HIT_MASK`]), damage
//!   [`DAMAGE`] unless its record sets one, health 10 (never read: any
//!   hit kills it), never moved and never lifted, the range boxes
//!   [`RANGES`]. Its reset bit is the next of
//!   [`super::aquatis::TENTACLE_BITS`] in the order the tentacles are
//!   built. Then it resets (below).
//! - The reset: the tentacles' hold is cleared, its counts are zeroed, a
//!   held Claw (whoever holds him) is let go, it goes idle and its reset
//!   bit is cleared.
//! - Each call, before its state: when Aquatis is current and his reset
//!   bits hold its bit, it resets (and makes no other pre-tick step);
//!   otherwise the shared pre-tick (the frame pulse's draw, Claw's side
//!   and the range mode, see [`super::pre_tick`]). After its state: its
//!   depth against Claw (below).
//! - Going idle: idle, the idle sway, the idle time of 1000 ms and a draw
//!   of 0..=300 ms, facing left.
//! - Idle: nothing at all unless Aquatis is current. When he is no longer
//!   alive it dies, and goes on. The sway plays; when his frenzy runs it
//!   starts to thrash, and goes on. Then, in range mode 3 or 4 with Claw
//!   at or left of it and its idle time over, the attack choice, which
//!   can replace the thrashing started in the same call.
//! - The attack choice: when a tentacle holds Claw it first goes idle
//!   (and does not stop there); then a draw of 0..=100: below
//!   [`STRIKE_CHANCE`] it strikes, otherwise it grabs.
//! - The strike: the swipe sound, the strike; while the strike shows its
//!   picture [`STRIKE_FRAME`] the strike box is its attack box, at other
//!   pictures it hurts nobody; when the strike ends it goes idle.
//! - The grab: the grab plays and Claw is held at once, frozen where he
//!   stands ([`crate::player::hug`]); the tentacles' hold is set. When the
//!   grab ends Claw faces right at [`HOLD_AT`] from the tentacle and the
//!   squeeze starts: the squeeze plays, its time of [`SQUEEZE_MS`] starts
//!   and both counts are zeroed.
//! - The squeeze, each call: a call on which Claw pressed a key counts.
//!   While Claw is held, the squeeze's time is not over and fewer than
//!   [`MASH_PRESSES`] calls counted: every [`SQUEEZE_STEP_MS`] (the first
//!   at once) one health off Claw (not while he is invulnerable); when
//!   that empties his health he is let go, the hold is cleared, the
//!   tentacle goes idle and he dies. Then the squeeze plays and Claw shows
//!   his crushed picture ([`HELD_FRAMES`]) for the squeeze's. Otherwise
//!   he is let go (standing where he is held), the hold is cleared and
//!   the tentacle goes idle.
//! - Thrashing (Aquatis's frenzy): the strike from its start, facing
//!   left, after a delay of a draw of 0..=200 ms. Each call while Aquatis
//!   is current and the delay is over: the strike plays with its box as
//!   in the strike, each time it ends it starts again facing the other
//!   way, and once his frenzy is over the attack box is cleared and it
//!   goes idle. Facing right, a picture other than the strike's leaves
//!   it the attack box [`FRENZY_RIGHT_BOX`]: one row at its own height
//!   from its x to the far right of the level. Whatever turns it back
//!   left (the next swipe, going idle, the reset) leaves it no box.
//! - Hits: none while dead, grabbing, squeezing or thrashing; the attack
//!   of Claw that hit it last hits no more (whatever the attacker is);
//!   otherwise a hit burst, a pistol shot is used up, and it dies.
//! - Death: the death sound, no attack box, dead, the death animation,
//!   harmless, its regrowth time of [`REGROW_MS`] starts. Dead, while
//!   Aquatis is current: the animation plays, a splash is made once (at
//!   its x and at the height of its box's bottom edge taken as a level
//!   height, that is near the top of the level, where it is never seen);
//!   while he is alive it grows back when his frenzy runs or its regrowth
//!   time is over: the growing sound, the regrowth animation, harmful
//!   again; when that ends it goes idle.
//!
//! # State numbers
//!
//! It idles in `Idle`, strikes in `Strike` and dies in `Dead`; it thrashes
//! in Aquatis's [`EnemyState::SuperFrenzy`] and its three new names take
//! 2290 to 2292 in the twelfth level's block
//! ([`super::aquatis::STATE_BLOCK`]).
//!
//! # Random draws, in order
//!
//! - building: going idle's;
//! - each call: the shared pre-tick's (on a frame pulse tick, unless it
//!   resets), the reset's going idle;
//! - idle: the attack choice's going idle (when a tentacle holds Claw),
//!   then one of 0..=100; thrashing's start, one of 0..=200;
//! - going idle: one of 0..=300;
//! - a hit: the hit burst's;
//! - the strike, the grab, the squeeze, death and regrowth: none.
//!
//! # Defaults taken
//!
//! - Its order against Claw in the update pass: the enemies' depth sort
//!   against Claw's feet ([`super::motion::DEPTH_BEHIND`] while his feet
//!   are below its own, as in the passage under the water, so it updates
//!   before him; [`super::motion::DEPTH_IN_FRONT`] once they are at or
//!   above, so after him), made after every call, the first excepted, as
//!   the original's runs show it (3899 and 4000): the grabbing tentacle
//!   places Claw before he runs, and runs after him for the rest of the
//!   squeeze.
//! - The twentieth point a squeeze could take, which would drop Claw into
//!   a fall, is never reached: [`SQUEEZE_MS`] allows 15 at most. It is not
//!   built.
//! - A squeeze counts a fresh press through Claw's own held state, which
//!   notes the keys pressed on his call: during a squeeze he always runs
//!   before the tentacle.
//!
//! Its data ([`TentacleData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use super::{
    Class, ClassAnims, ClassSounds, EnemyData, EnemyFields, EnemyState, Handler, Timer, UNBOUNDED,
    accept_hit, data, faces_right, hit_burst, nothing, obj, obj_mut, play, rand, set_anim,
    set_facing, set_state,
};
use crate::logic::effect::{self, EffectKind};
use crate::logic::{GameLogic, ObjectData, World};
use crate::object::{LogicKind, ObjectId, Rect, engine_state, flags, special, types};
use crate::player::hug;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Tentacle"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_AQUATISTENTACLE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_AQUATISTENTACLE_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Its new states.
pub const STATES: [EnemyState; 3] = [
    EnemyState::StartGrab,
    EnemyState::Squeeze,
    EnemyState::Respawn,
];

/// Damage of its strike when its record sets none (no record does); not
/// scaled by the difficulty.
pub const DAMAGE: i32 = 5;
/// Its health before the difficulty factor; nothing reads it.
pub const HEALTH: i32 = 10;
/// Its box for moving and being hit when the level does not know its
/// first picture: the box of that picture, reaching down into the water.
pub const BOX: Rect = Rect::new(5, -99, 68, 110);
/// The strike's box facing left, on the strike's picture only.
pub const STRIKE_BOX: Rect = Rect::new(-136, -44, 0, 39);
/// The picture of the strike that carries the strike box.
pub const STRIKE_FRAME: i32 = 301;
/// How far right its attack box reaches while it thrashes facing right at
/// another picture than the strike's, px: anything right of it on its
/// own row, however far.
pub const FRENZY_RIGHT_REACH: i32 = 1 << 30;
/// Its attack box while it thrashes facing right at another picture than
/// the strike's: one row at its own height, from its x to
/// [`FRENZY_RIGHT_REACH`] px right of it.
pub const FRENZY_RIGHT_BOX: Rect = Rect::new(0, 0, FRENZY_RIGHT_REACH, 0);
/// What may hit it: Claw, his pistol shots, the level's shots and
/// explosions, and his magic.
pub const HIT_MASK: u32 = 0x300_0022;
/// The five range boxes (half width, half height), outermost first; the
/// second is never set. An attack needs Claw in the fourth or the fifth.
pub const RANGES: [(i32, i32); 5] = [(400, 50), (0, 0), (300, 50), (200, 300), (50, 50)];
/// How far left of it Claw must be for an attack, px.
pub const REACH: i32 = RANGES[3].0;
/// Its idle time before an attack, ms, plus a draw of 0..=[`IDLE_EXTRA_MS`].
pub const IDLE_MS: u64 = 1000;
/// The idle time's random part, ms.
pub const IDLE_EXTRA_MS: u32 = 300;
/// The chance in 101 that an attack is a strike rather than a grab.
pub const STRIKE_CHANCE: i32 = 75;
/// The longest delay before thrashing starts, ms.
pub const FRENZY_DELAY_MS: u32 = 200;
/// Ticks of the grab before Claw is lifted, at the 18 ms step (measured).
pub const START_GRAB_TICKS: u32 = 18;
/// Where the held Claw is put, from the tentacle.
pub const HOLD_AT: (i32, i32) = (50, -70);
/// The longest squeeze, ms.
pub const SQUEEZE_MS: u32 = 3000;
/// Time between two points of damage while it squeezes, ms.
pub const SQUEEZE_STEP_MS: u32 = 200;
/// Ticks with a fresh press that free Claw.
pub const MASH_PRESSES: u32 = 10;
/// Time a cut-down tentacle waits before it grows back, ms.
pub const REGROW_MS: u32 = 20000;
/// The squeeze's pictures and Claw's crushed pictures shown with them.
pub const HELD_FRAMES: [(i32, i32); 3] = [(302, 450), (303, 451), (304, 452)];

/// Its idle sway.
pub const IDLE_ANIM: &str = "LEVEL_AQUATISTENTACLE_IDLE";
/// The strike, and each swipe of the frenzy.
pub const STRIKE_ANIM: &str = "LEVEL_AQUATISTENTACLE_STRIKE1";
/// The grab.
pub const GRAB_ANIM: &str = "LEVEL_AQUATISTENTACLE_STRIKE2";
/// The squeeze.
pub const SQUEEZE_ANIM: &str = "LEVEL_AQUATISTENTACLE_STRIKE3";
/// Sinking into the water.
pub const DEATH_ANIM: &str = "LEVEL_AQUATISTENTACLE_KILLFALL";
/// Growing back.
pub const REGROW_ANIM: &str = "LEVEL_AQUATISTENTACLE_RESPAWN";
/// The strike's swipe.
pub const SWIPE_SOUND: &str = "LEVEL_AQUATISTENTACLE_TENTSWIPE";
/// Its death.
pub const DEATH_SOUND: &str = "LEVEL_AQUATISTENTACLE_TENTDEATH";
/// Growing back.
pub const GROW_SOUND: &str = "LEVEL_AQUATISTENTACLE_TENTGROW";

/// The tentacle class: a plain enemy, never lifted.
pub static CLASS: Class = Class {
    name: "Tentacle",
    human: false,
    construct,
    pre_tick: nothing,
    handler,
    go_home: nothing,
    go_idle,
    carry: None,
    pick_lift_voice: nothing,
    lift_roll: true,
    anims: ClassAnims::NONE,
    sounds: ClassSounds::NONE,
};

/// A tentacle's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TentacleData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// Its reset bit (one of [`super::aquatis::TENTACLE_BITS`]); 0 before
    /// it is built.
    pub bit: u32,
    /// Its idle time before an attack.
    pub idle: Timer,
    /// The squeeze's whole time.
    pub squeeze: Timer,
    /// The time to the next point of damage of the squeeze.
    pub squeeze_step: Timer,
    /// Points of damage the squeeze has taken.
    pub damage_count: u32,
    /// Calls with a fresh press during the squeeze.
    pub mash: u32,
    /// The delay before thrashing starts.
    pub frenzy_delay: Timer,
    /// The time before a cut-down tentacle grows back.
    pub regrow: Timer,
    /// The splash of its death has been made.
    pub splashed: bool,
}

/// The tentacle data of `id`.
pub fn tentacle_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a TentacleData> {
    match &w.objects.get(id)?.data {
        ObjectData::Tentacle(d) => Some(d),
        _ => None,
    }
}

/// The tentacle data of `id`, mutable.
pub fn tentacle_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut TentacleData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Tentacle(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Idle => Some(idle_tick),
        EnemyState::Strike => Some(strike_tick),
        EnemyState::StartGrab => Some(grab_tick),
        EnemyState::Squeeze => Some(squeeze_tick),
        EnemyState::SuperFrenzy => Some(frenzy_tick),
        EnemyState::Dead => Some(dead_tick),
        EnemyState::Respawn => Some(regrow_tick),
        _ => None,
    }
}

/// True while Aquatis is current (see the module documentation).
fn aquatis_current(w: &World) -> bool {
    w.aquatis().is_some()
}

/// One logic call (see the module documentation).
pub fn tick(w: &mut World, id: ObjectId) {
    let raw = obj(w, id).state;
    match raw {
        engine_state::INIT => {
            construct(w, id);
            return;
        }
        engine_state::DESTROYING => return,
        engine_state::KILLED => {
            // Another logic killed it outright.
            obj_mut(w, id).state = EnemyState::Idle as i32;
            die(w, id);
            return;
        }
        _ => {}
    }
    let Some(s) = EnemyState::from_id(raw) else {
        return;
    };
    pre_tick(w, id);
    // The pre-tick may have reset it.
    let s = super::state(w, id).unwrap_or(s);
    if let Some(h) = handler(s) {
        h(w, id);
    }
    depth_sort(w, id);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building and the reset

/// Builds a tentacle (see the module documentation), then it resets.
/// Draws: going idle's.
pub fn construct(w: &mut World, id: ObjectId) {
    // Its reset bit: the next one in the order the tentacles are built.
    let built = w
        .objects
        .all()
        .filter(|&(other, o)| {
            other != id && matches!(&o.data, ObjectData::Tentacle(d) if d.bit != 0)
        })
        .count();
    let bit = super::aquatis::TENTACLE_BITS
        .get(built)
        .copied()
        .unwrap_or(0);
    let damage = match obj(w, id).damage {
        0 => DAMAGE,
        d => d,
    };
    let frames = w.image_frames(IMAGE_SET);
    let rect = frames
        .and_then(|(first, _)| w.frame_box(IMAGE_SET, first))
        .unwrap_or(BOX);
    let fields = EnemyFields {
        mode: 6,
        ranges: RANGES,
        bounds: UNBOUNDED,
        bounds_y: UNBOUNDED,
        stationary: true,
        z_sort: true,
        z_base: 0,
        contact_rect: Rect::UNSET,
        strike_rect: STRIKE_BOX,
        strike_anim: STRIKE_ANIM,
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        frozen: true,
        ..EnemyFields::default()
    };
    let o = obj_mut(w, id);
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    if let Some((first, _)) = frames {
        o.frame = first;
    }
    o.type_flags = types::ENEMY;
    o.hit_type_flags = HIT_MASK;
    o.special_flags &= !special::LIFTABLE;
    o.hit_handler = true;
    o.move_rect = rect;
    o.hit_rect = rect;
    o.attack_rect = Rect::UNSET;
    if faces_right(o) {
        o.mirror_rects();
    }
    o.health = super::scaled(HEALTH, super::HEALTH_FACTOR);
    o.damage = damage;
    o.data = ObjectData::Tentacle(TentacleData {
        enemy: fields.into(),
        bit,
        ..TentacleData::default()
    });
    reset(w, id);
}

/// The reset (see the module documentation). Draws: going idle's.
pub fn reset(w: &mut World, id: ObjectId) {
    w.set_tentacle_grab(false);
    if let Some(t) = tentacle_mut(w, id) {
        t.damage_count = 0;
        t.mash = 0;
    }
    hug::release_bearhug(w);
    go_idle(w, id);
    let bit = tentacle_data(w, id).map_or(0, |t| t.bit);
    w.clear_aquatis_reset_bit(bit);
}

/// Before its state: the reset when Aquatis asks for it, otherwise the
/// shared pre-tick. Draws: the reset's or the shared pre-tick's.
fn pre_tick(w: &mut World, id: ObjectId) {
    let bit = tentacle_data(w, id).map_or(0, |t| t.bit);
    if aquatis_current(w) && w.aquatis_reset_bits() & bit != 0 {
        reset(w, id);
        return;
    }
    super::pre_tick(w, id, &CLASS);
}

/// Drawn in front of Claw when his feet are at or above its own, behind
/// him otherwise; a change takes effect in the next update pass.
fn depth_sort(w: &mut World, id: ObjectId) {
    let Some(claw_feet) = w.claw_object().map(|c| c.y + c.move_rect.bottom) else {
        return;
    };
    let base = data(w, id).map_or(0, |d| d.z_base);
    let o = obj_mut(w, id);
    let feet = o.y + o.move_rect.bottom;
    let z = base
        + if claw_feet <= feet {
            super::motion::DEPTH_IN_FRONT
        } else {
            super::motion::DEPTH_BEHIND
        };
    if o.z != z {
        o.z = z;
        o.flags |= flags::RESORT;
    }
}

// ----------------------------------------------------------------------
// Idle and the attack choice

/// Goes idle (see the module documentation). Draws: one of 0..=300.
pub fn go_idle(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Idle);
    set_anim(w, id, IDLE_ANIM);
    let now = w.now_ms;
    let extra = rand(w, IDLE_EXTRA_MS + 1);
    if let Some(t) = tentacle_mut(w, id) {
        t.idle = Timer::new(now, IDLE_MS + extra as u64);
    }
    turn(w, id, false);
}

/// Turns it to face right (`right`) or left. Its box facing right on
/// another picture than the strike's turns into no box at all when it
/// turns left, whatever turns it: mirrored, that box would reach its
/// whole row to its left.
fn turn(w: &mut World, id: ObjectId, right: bool) {
    let o = obj_mut(w, id);
    if !right && o.attack_rect == FRENZY_RIGHT_BOX {
        o.attack_rect = Rect::UNSET;
    }
    set_facing(w, id, right);
}

/// Idle (see the module documentation). Draws: the attack choice's.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    if !aquatis_current(w) {
        return;
    }
    if !w.aquatis_link().alive {
        die(w, id);
    }
    w.advance_anim(id);
    if w.aquatis_link().frenzy {
        enter_frenzy(w, id);
    }
    let now = w.now_ms;
    let Some(mode) = data(w, id).map(|d| d.mode) else {
        return;
    };
    let left = w.claw_object().is_some_and(|c| c.x <= obj(w, id).x);
    let rested = tentacle_data(w, id).is_some_and(|t| t.idle.done(now));
    if matches!(mode, 3 | 4) && left && rested {
        attack_choice(w, id);
    }
}

/// The attack choice (see the module documentation). Draws: going idle's
/// when a tentacle holds Claw, then one of 0..=100.
pub fn attack_choice(w: &mut World, id: ObjectId) {
    if w.tentacle_grab() {
        go_idle(w, id);
    }
    if rand(w, 101) < STRIKE_CHANCE {
        enter_strike(w, id);
    } else {
        start_grab(w, id);
    }
}

// ----------------------------------------------------------------------
// The strike and its box

/// Its attack box for the picture it shows: the strike box on the
/// strike's picture, none at others, as it faces (see the module
/// documentation for the box it keeps facing right).
fn arm(w: &mut World, id: ObjectId) {
    let o = obj_mut(w, id);
    let strike = o.frame == STRIKE_FRAME;
    o.attack_rect = match (strike, faces_right(o)) {
        (true, false) => STRIKE_BOX,
        (true, true) => STRIKE_BOX.mirrored(),
        (false, false) => Rect::UNSET,
        (false, true) => FRENZY_RIGHT_BOX,
    };
}

/// The strike: the swipe sound and the strike, not yet played.
pub fn enter_strike(w: &mut World, id: ObjectId) {
    play(w, SWIPE_SOUND);
    set_state(w, id, EnemyState::Strike);
    set_anim(w, id, STRIKE_ANIM);
}

/// Strike (see the module documentation).
pub fn strike_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    arm(w, id);
    if obj(w, id).anim.is_done() {
        go_idle(w, id);
    }
}

// ----------------------------------------------------------------------
// The grab and the squeeze

/// The grab (see the module documentation).
pub fn start_grab(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::StartGrab);
    set_anim(w, id, GRAB_ANIM);
    hug::enter_bearhug(w);
    w.set_tentacle_grab(true);
}

/// StartGrab: the grab plays; when it ends Claw faces right at
/// [`HOLD_AT`] from the tentacle and the squeeze starts.
pub fn grab_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    let (x, y) = {
        let o = obj(w, id);
        (o.x + HOLD_AT.0, o.y + HOLD_AT.1)
    };
    hug::place(w, x, y, true);
    enter_squeeze(w, id);
}

/// Starts the squeeze: the squeeze, not yet played, its time, the first
/// point of damage due at once and no presses counted.
fn enter_squeeze(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Squeeze);
    set_anim(w, id, SQUEEZE_ANIM);
    let now = w.now_ms;
    if let Some(t) = tentacle_mut(w, id) {
        t.squeeze = Timer::new(now, u64::from(SQUEEZE_MS));
        t.squeeze_step = Timer::default();
        t.damage_count = 0;
        t.mash = 0;
    }
}

/// Squeeze (see the module documentation). Draws: going idle's when Claw
/// is let go.
pub fn squeeze_tick(w: &mut World, id: ObjectId) {
    if hug::escape_pressed(w)
        && let Some(t) = tentacle_mut(w, id)
    {
        t.mash += 1;
    }
    let now = w.now_ms;
    let Some((squeeze, step, mash)) =
        tentacle_data(w, id).map(|t| (t.squeeze, t.squeeze_step, t.mash))
    else {
        return;
    };
    if !hug::held(w) || squeeze.done(now) || mash >= MASH_PRESSES {
        hug::release_bearhug(w);
        w.set_tentacle_grab(false);
        go_idle(w, id);
        return;
    }
    if step.done(now) {
        if let Some(t) = tentacle_mut(w, id) {
            t.squeeze_step = Timer::new(now, u64::from(SQUEEZE_STEP_MS));
            t.damage_count += 1;
        }
        if hug::squeeze(w) {
            // Emptied: he was let go and dies.
            w.set_tentacle_grab(false);
            go_idle(w, id);
            return;
        }
    }
    w.advance_anim(id);
    let frame = obj(w, id).frame;
    if let Some(&(_, held)) = HELD_FRAMES.iter().find(|(f, _)| *f == frame) {
        hug::show_frame(w, held);
    }
}

// ----------------------------------------------------------------------
// Thrashing

/// Starts thrashing (see the module documentation). Draws: one of
/// 0..=200.
pub fn enter_frenzy(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::SuperFrenzy);
    set_anim(w, id, STRIKE_ANIM);
    turn(w, id, false);
    let now = w.now_ms;
    let delay = rand(w, FRENZY_DELAY_MS + 1);
    if let Some(t) = tentacle_mut(w, id) {
        t.frenzy_delay = Timer::new(now, delay as u64);
    }
}

/// SuperFrenzy (see the module documentation).
pub fn frenzy_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if !aquatis_current(w) || !tentacle_data(w, id).is_some_and(|t| t.frenzy_delay.done(now)) {
        return;
    }
    w.advance_anim(id);
    arm(w, id);
    if obj(w, id).anim.is_done() {
        set_anim(w, id, STRIKE_ANIM);
        let right = faces_right(obj(w, id));
        turn(w, id, !right);
    }
    if !w.aquatis_link().frenzy {
        obj_mut(w, id).attack_rect = Rect::UNSET;
        go_idle(w, id);
    }
}

// ----------------------------------------------------------------------
// Hits, death and regrowth

/// `attacker` hit `id` (see the module documentation). Draws: the hit
/// burst's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if matches!(
        super::state(w, id),
        Some(
            EnemyState::Dead
                | EnemyState::Squeeze
                | EnemyState::StartGrab
                | EnemyState::SuperFrenzy
        )
    ) {
        return;
    }
    if !accept_hit(w, id) {
        return;
    }
    hit_burst(w, id, attacker);
    if w.objects.get(attacker).map(|a| a.type_flags) == Some(types::PISTOL_SHOT) {
        w.destroy(attacker);
    }
    die(w, id);
}

/// Dies (see the module documentation).
pub fn die(w: &mut World, id: ObjectId) {
    play(w, DEATH_SOUND);
    set_state(w, id, EnemyState::Dead);
    set_anim(w, id, DEATH_ANIM);
    let now = w.now_ms;
    {
        let o = obj_mut(w, id);
        o.attack_rect = Rect::UNSET;
        o.flags |= flags::HARMLESS;
    }
    if let Some(t) = tentacle_mut(w, id) {
        t.regrow = Timer::new(now, u64::from(REGROW_MS));
        t.splashed = false;
    }
}

/// Dead (see the module documentation).
pub fn dead_tick(w: &mut World, id: ObjectId) {
    if !aquatis_current(w) {
        return;
    }
    w.advance_anim(id);
    if tentacle_data(w, id).is_some_and(|t| !t.splashed) {
        if let Some(t) = tentacle_mut(w, id) {
            t.splashed = true;
        }
        let (x, bottom, z) = {
            let o = obj(w, id);
            (o.x, o.move_rect.bottom, o.z)
        };
        if let Some(s) = effect::spawn(w, EffectKind::Splash, x, bottom)
            && let Some(o) = w.objects.get_mut(s)
        {
            o.z = z + 2;
        }
    }
    let now = w.now_ms;
    let link = w.aquatis_link();
    let (alive, frenzy) = (link.alive, link.frenzy);
    let regrown = tentacle_data(w, id).is_some_and(|t| t.regrow.done(now));
    if alive && (frenzy || regrown) {
        regrow(w, id);
    }
}

/// Grows back: the growing sound, the regrowth, harmful again.
pub fn regrow(w: &mut World, id: ObjectId) {
    play(w, GROW_SOUND);
    set_state(w, id, EnemyState::Respawn);
    set_anim(w, id, REGROW_ANIM);
    obj_mut(w, id).flags &= !flags::HARMLESS;
}

/// Respawn: the regrowth plays; when it ends it goes idle.
pub fn regrow_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        go_idle(w, id);
    }
}

/// The tentacles of the level, in no particular order.
pub fn tentacles(w: &World) -> Vec<ObjectId> {
    w.objects
        .all()
        .filter(|(_, o)| o.kind == LogicKind::Game(GameLogic::Tentacle))
        .map(|(id, _)| id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::aquatis;
    use super::*;

    #[test]
    fn its_states_follow_aquatis_in_the_twelfth_levels_block() {
        let ids: Vec<i32> = STATES.iter().map(|&s| s as i32).collect();
        assert_eq!(ids, [2290, 2291, 2292]);
        for id in ids {
            assert!(aquatis::STATE_BLOCK.contains(&id));
        }
        assert!(!CLASS.human, "a plain enemy");
        assert!(CLASS.carry.is_none(), "never lifted");
        for s in STATES.into_iter().chain([
            EnemyState::Idle,
            EnemyState::Strike,
            EnemyState::Dead,
            EnemyState::SuperFrenzy,
        ]) {
            assert!(handler(s).is_some(), "{}", s.name());
        }
        assert!(handler(EnemyState::Home).is_none());
    }

    #[test]
    fn a_squeeze_costs_at_most_fourteen_health_at_eighteen_ms() {
        // One point on its first tick, then one every 12 ticks (216 ms),
        // until 3000 ms have passed: 167 ticks.
        let step = SQUEEZE_STEP_MS.div_ceil(18);
        assert_eq!(step, 12);
        let ticks = SQUEEZE_MS.div_ceil(18);
        assert_eq!(ticks, 167);
        assert_eq!((ticks - 1) / step + 1, 14);
        // The strike reaches 136 px to its left.
        assert_eq!(-STRIKE_BOX.left, 136);
        // Twenty points never fit in a squeeze.
        const { assert!(SQUEEZE_MS / SQUEEZE_STEP_MS + 1 < 20) };
    }

    #[test]
    fn its_boxes_facing_right() {
        assert_eq!(STRIKE_BOX.mirrored(), Rect::new(0, -44, 136, 39));
        assert!(FRENZY_RIGHT_BOX.is_set());
        // Translated to the far right of a level, the box still points
        // right.
        let far = FRENZY_RIGHT_BOX.translate(60_000, 9_000);
        assert!(far.right > far.left && far.top == far.bottom);
        assert_eq!(REACH, 200);
    }
}
