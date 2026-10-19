//! The seagull: the first enemy that flies.
//!
//! It patrols a fixed height of sky between its two limits at 120 px/s,
//! turning at each limit, with no gravity and no tile collision: it flies
//! through walls and floors. When Claw is ahead of it in its direction of
//! flight, inside its limits, lower than it by more than 100 and less than
//! 240 px and further away along x than down, it screeches and dives at a
//! point 40 px above where Claw stands, at a fixed 600 px/s along the
//! sum of the two distances, without re-aiming. During the dive its beak
//! ([`BEAK`]) hurts Claw on contact; outside a dive it cannot hurt him.
//! The dive ends once the bird is lower than Claw was when it started, or
//! at a limit; it then flies back up to its patrol height
//! ([`EnemyState::Withdraw`]) and patrols again.
//!
//! It has 1 health, so any hit kills it; a hit it survives (only with a
//! stronger difficulty) flinches in [`EnemyState::GenericHit`]. Dead, it
//! drops its record's loot, falls through the level and goes once off
//! screen. It cannot be lifted and never speaks.
//!
//! # The tick
//!
//! Each call runs the shared enemy frame ([`super::tick`]): the pre-tick
//! (the per-tick roll on frame pulse ticks, which side Claw is on), the
//! state's handler, then the movement step with no gravity and no tile
//! test, held to the patrol limits across and free up and down.
//!
//! - Patrol ([`EnemyState::Idle`]): at the left limit it turns right at
//!   120 px/s, then at the right limit it turns left. It dives when this
//!   tick's roll is 0 to 999 (on a frame pulse tick only, so the dive comes
//!   on the first pulse tick the geometry allows), Claw is strictly inside
//!   the limits, below the bird, further away across than down (whole
//!   pixels, cut towards zero), more than 100 and less than 240 px down,
//!   and the bird flies towards him.
//! - Dive ([`EnemyState::Strike`]): the dive sound, the beak as its attack
//!   box (mirrored when it faces right), and the velocity that reaches
//!   (Claw's x, Claw's y - 40) at 600 px/s along the sum of the two
//!   distances ([`fixed_speed_velocity`]), from the bird's exact position.
//!   It withdraws once it is lower than Claw's y at the dive's start, or
//!   on either limit.
//! - Withdraw ([`EnemyState::Withdraw`]): no attack box; it flies at
//!   600 px/s to its patrol height at the left limit when it was flying
//!   left, at the right one otherwise, keeping its facing. Once above its
//!   patrol height it is put back on it, stops and patrols again.
//!
//! The limits are exact: the movement step holds the bird on a limit it
//! would cross, so it stands exactly on it on the tick it arrives.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): the seagull patrols in
//! `Idle`, dives in `Strike` and dies in `Dead` with handlers of its own
//! under those numbers. Its two new names take the third and fourth
//! numbers of the fifth level's block ([`STATE_BLOCK`], shared with the
//! town guards): [`EnemyState::Withdraw`] and [`EnemyState::GenericHit`].
//!
//! # Defaults taken
//!
//! - No separate flying-enemy layer: this module holds the flight rules
//!   (no gravity, no tile collision, no vertical limits), since the
//!   seagull is the only flyer built on them.
//! - The "off screen" test that removes a dead seagull, under a widened
//!   view, follows the rule the other enemies' falling bodies use.
//! - The seagulls of later levels run this code with their own animations
//!   and records; their timings are checked with those levels.
//! - A flinch (only with more than 1 health) ends by starting again the
//!   animation of the state it came from: the dive's pose after a dive,
//!   flying otherwise, and the flinch it shows after a flinch.
//!
//! Its data ([`SeagullData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use std::ops::RangeInclusive;

use super::{
    Class, ClassAnims, ClassSounds, EnemyData, EnemyFields, EnemyState, Handler, Timer, UNBOUNDED,
    accept_hit, attacker, data, data_mut, drop_loot, exact_x, facing_rect, hit_burst, loot_of,
    nothing, obj, obj_mut, play, rand, record_bounds, scaled, set_anim, set_facing, set_state,
    take_damage,
};
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, Rect, engine_state, flags, special, types};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Seagull"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SEAGULL_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers of the fifth level's families (see
/// [`super::townguard::STATE_BLOCK`]).
pub const STATE_BLOCK: RangeInclusive<i32> = super::townguard::STATE_BLOCK;

/// Image set.
pub const IMAGE_SET: &str = "LEVEL_SEAGULL";
/// Patrolling and flying back up.
pub const FLYING_ANIM: &str = "LEVEL_SEAGULL_FLYING";
/// The dive's pose.
pub const DIVE_ANIM: &str = "LEVEL_SEAGULL_DIVE3";
/// A high flinch.
pub const HIT_HIGH_ANIM: &str = "LEVEL_SEAGULL_HITHIGH";
/// A low flinch.
pub const HIT_LOW_ANIM: &str = "LEVEL_SEAGULL_HITLOW";
/// The body falling after death.
pub const DEATH_ANIM: &str = "LEVEL_SEAGULL_KILLFALL";
/// The screech of a dive.
pub const DIVE_SOUND: &str = "LEVEL_SEAGULL_SEAGULLDIVE";
/// The squawk of every hit, the killing one included.
pub const HIT_SOUND: &str = "LEVEL_SEAGULL_SEAGULLDIE";

/// Patrol speed, px/s.
pub const PATROL_SPEED: i32 = 120;
/// Speed of the dive and of the climb back, px/s, measured along the sum
/// of the horizontal and vertical distances.
pub const DIVE_SPEED: f64 = 600.0;
/// The dive aims this far above where Claw stands, px.
pub const AIM_ABOVE: i32 = 40;
/// Claw must be more than this far below the bird, and less than the
/// second value, for a dive, px.
pub const DIVE_BAND: (i32, i32) = (100, 240);
/// The beak's box during a dive, facing left.
pub const BEAK: Rect = Rect::new(-26, -5, -5, 22);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-30, -30, 30, 30);
/// Movement box when the level does not know the bird's first picture.
pub const FALLBACK_MOVE_RECT: Rect = Rect::new(-32, -30, 31, 29);
/// Depth given to a seagull placed at depth 0: in front of Claw.
pub const DEFAULT_Z: i32 = 8500;
/// Health before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 1;
/// Damage of the beak before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Kinds of object that can hit a seagull: Claw, his thrown objects and
/// pistol shots, the level's own shots and explosions, and his magic.
pub const HIT_BY: u32 = 0x300_0032;
/// Half sizes of the range boxes, outermost first (the bird reads none).
pub const RANGES: [(i32, i32); 5] = [(500, 40), (0, 0), (200, 40), (150, 40), (100, 40)];
/// Hit cooldown after a shot of the level or an explosion, ms.
pub const LEVEL_SHOT_COOLDOWN_MS: u64 = 2000;
/// Chance in 100 that a flinch is the high one.
pub const HIT_HIGH_CHANCE: i32 = 50;
/// Speed of the body after death: across, away from Claw, and up, px/s.
pub const DEATH_LAUNCH: (f64, f64) = (100.0, -100.0);
/// Gravity on the body after death, px/s^2.
pub const DEATH_GRAVITY: f64 = 2000.0;

/// The velocity that flies from `from` to `to` at `speed`, measured along
/// the sum of the two distances (the dive and the climb back); zero when
/// the two points are the same.
pub fn fixed_speed_velocity(from: (f64, f64), to: (f64, f64), speed: f64) -> (f64, f64) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let l1 = dx.abs() + dy.abs();
    if l1 == 0.0 {
        return (0.0, 0.0);
    }
    (speed * dx / l1, speed * dy / l1)
}

/// Where a bird at exactly (`x`, `y`), flying at `vx` across between the
/// limits `bounds`, stands towards Claw at (`cx`, `cy`): true when it
/// dives at him, with `roll` this tick's roll (see [`super::pulse_roll`])
/// and `claw_right` whether Claw is at or past the bird's x.
pub fn dive_gate(
    (x, y): (f64, f64),
    vx: f64,
    bounds: (i32, i32),
    (cx, cy): (i32, i32),
    claw_right: bool,
    roll: i32,
) -> bool {
    let dx = (f64::from(cx) - x).trunc().abs();
    let dy = (f64::from(cy) - y).trunc().abs();
    (0..1000).contains(&roll)
        && bounds.0 < cx
        && cx < bounds.1
        && y < f64::from(cy)
        && dx > dy
        && f64::from(DIVE_BAND.0) < dy
        && dy < f64::from(DIVE_BAND.1)
        && ((vx < 0.0 && !claw_right) || (vx > 0.0 && claw_right))
}

/// The seagull class.
pub static CLASS: Class = Class {
    name: "Seagull",
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

/// A seagull's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SeagullData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Idle => Some(idle_tick),
        EnemyState::Strike => Some(strike_tick),
        EnemyState::Withdraw => Some(withdraw_tick),
        EnemyState::GenericHit => Some(generic_hit_tick),
        // The body falls and goes once off screen, as the other enemies'
        // bodies do.
        _ => None,
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    if obj(w, id).state == engine_state::KILLED {
        // Another logic killed it outright.
        if data(w, id).is_some_and(|d| d.dead) {
            obj_mut(w, id).state = EnemyState::Dead as i32;
        } else {
            die(w, id);
        }
        return;
    }
    super::tick(w, id, &CLASS);
}

/// The bird's exact height: its y plus the fraction its last movement step
/// carried, unless something else has moved it since.
pub fn exact_y(w: &World, id: ObjectId) -> f64 {
    let o = obj(w, id);
    let carry = data(w, id)
        .filter(|d| d.moved_to == Some((o.x, o.y)))
        .map_or(0.0, |d| d.carry.1);
    f64::from(o.y) + carry
}

/// The bird's patrol height: where it was placed.
pub fn home_y(w: &World, id: ObjectId) -> i32 {
    obj(w, id).editor.1
}

/// Builds the seagull: 1 health, 10 damage, flying with no gravity and no
/// tile test between the record's limits across (none up and down), its
/// hit box, its movement box from its first picture, no attack box, depth
/// 8500 when placed at 0; then it patrols. Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    // The construction step every enemy shares moves a bird placed beside
    // its range onto the nearer limit, and up or down onto its record's
    // limits (provisional); its patrol height stays the placement height.
    super::clamp_on_record_range(w, id);
    let fields = EnemyFields {
        bounds: record_bounds(w, id),
        bounds_y: UNBOUNDED,
        ranges: RANGES,
        ay: 0.0,
        strike_rect: BEAK,
        strike_anim: DIVE_ANIM,
        death_anim: DEATH_ANIM,
        death_sound: HIT_SOUND,
        smarts: super::smarts(50),
        loot: loot_of(w, id),
        ..EnemyFields::default()
    };
    let frames = w.image_frames(IMAGE_SET);
    let move_rect = frames
        .and_then(|(first, _)| w.frame_box(IMAGE_SET, first))
        .unwrap_or(FALLBACK_MOVE_RECT);
    let o = obj_mut(w, id);
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    if let Some((first, _)) = frames {
        o.frame = first;
    }
    o.type_flags = types::ENEMY;
    o.hit_type_flags = HIT_BY;
    o.special_flags &= !special::LIFTABLE;
    o.flags |= flags::DESTROY_FAR;
    o.hit_handler = true;
    o.physics_type = ptype::DIRECT;
    // The boxes are kept in their own shape even when the record starts
    // the bird mirrored; each turn mirrors them.
    o.move_rect = move_rect;
    o.hit_rect = HIT_RECT;
    o.attack_rect = Rect::UNSET;
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    o.health = scaled(HEALTH, super::HEALTH_FACTOR);
    o.damage = scaled(DAMAGE, super::DAMAGE_FACTOR);
    let vy = f64::from(o.speed_y);
    o.data = ObjectData::Seagull(SeagullData {
        enemy: EnemyFields { vy, ..fields }.into(),
    });
    go_idle(w, id);
}

/// True when the bird stands exactly on the limit `limit`.
fn on_limit(w: &World, id: ObjectId, limit: i32) -> bool {
    exact_x(w, id) == f64::from(limit)
}

fn bounds(w: &World, id: ObjectId) -> (i32, i32) {
    data(w, id).map_or(UNBOUNDED, |d| d.bounds)
}

fn set_vx(w: &mut World, id: ObjectId, vx: f64) {
    if let Some(d) = data_mut(w, id) {
        d.vx = vx;
    }
}

/// Patrols again: towards the left limit when it stands on the right one,
/// facing it, otherwise towards the right one; its speed up and down is
/// left as it is.
pub fn go_idle(w: &mut World, id: ObjectId) {
    let (_, hi) = bounds(w, id);
    if on_limit(w, id, hi) {
        set_vx(w, id, -f64::from(PATROL_SPEED));
        set_facing(w, id, false);
    } else {
        set_vx(w, id, f64::from(PATROL_SPEED));
        set_facing(w, id, true);
    }
    set_state(w, id, EnemyState::Idle);
    set_anim(w, id, FLYING_ANIM);
}

/// The patrol: turns at the limits, then dives when the gate opens.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let (lo, hi) = bounds(w, id);
    if on_limit(w, id, lo) {
        set_vx(w, id, f64::from(PATROL_SPEED));
        set_facing(w, id, true);
    }
    if on_limit(w, id, hi) {
        set_vx(w, id, -f64::from(PATROL_SPEED));
        set_facing(w, id, false);
    }
    let Some(claw) = w.claw_object().map(|c| (c.x, c.y)) else {
        return;
    };
    let at = (exact_x(w, id), exact_y(w, id));
    let Some((vx, roll, claw_right)) = data(w, id).map(|d| (d.vx, d.roll, d.claw_right)) else {
        return;
    };
    if dive_gate(at, vx, (lo, hi), claw, claw_right, roll) {
        enter_dive(w, id, claw);
    }
}

/// Starts a dive at Claw standing at `claw`.
pub fn enter_dive(w: &mut World, id: ObjectId, (cx, cy): (i32, i32)) {
    play(w, DIVE_SOUND);
    let beak = {
        let o = obj(w, id);
        facing_rect(o, BEAK)
    };
    obj_mut(w, id).attack_rect = beak;
    set_state(w, id, EnemyState::Strike);
    set_anim(w, id, DIVE_ANIM);
    let from = (exact_x(w, id), exact_y(w, id));
    let aim = (f64::from(cx), f64::from(cy - AIM_ABOVE));
    let (vx, vy) = fixed_speed_velocity(from, aim, DIVE_SPEED);
    if let Some(d) = data_mut(w, id) {
        d.target_y = cy;
        d.vx = vx;
        d.vy = vy;
    }
}

/// The dive: on until the bird is lower than its target or on a limit.
pub fn strike_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let (lo, hi) = bounds(w, id);
    let below = data(w, id).is_some_and(|d| f64::from(d.target_y) < exact_y(w, id));
    if below || on_limit(w, id, hi) || on_limit(w, id, lo) {
        enter_withdraw(w, id);
    }
}

/// Flies back up to the patrol height at a limit: the left one when it
/// was flying left, the right one otherwise.
pub fn enter_withdraw(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Withdraw);
    obj_mut(w, id).attack_rect = Rect::UNSET;
    set_anim(w, id, FLYING_ANIM);
    let (lo, hi) = bounds(w, id);
    let left = data(w, id).is_some_and(|d| d.vx < 0.0);
    let to = (
        f64::from(if left { lo } else { hi }),
        f64::from(home_y(w, id)),
    );
    let from = (exact_x(w, id), exact_y(w, id));
    let (vx, vy) = fixed_speed_velocity(from, to, DIVE_SPEED);
    if let Some(d) = data_mut(w, id) {
        d.vx = vx;
        d.vy = vy;
    }
}

/// Flying back up: once above the patrol height, back on it, stopped,
/// and patrolling again.
pub fn withdraw_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let home = home_y(w, id);
    if exact_y(w, id) < f64::from(home) {
        let o = obj_mut(w, id);
        o.y = home;
        let at = (o.x, o.y);
        if let Some(d) = data_mut(w, id) {
            d.carry.1 = 0.0;
            if d.moved_to.is_some() {
                d.moved_to = Some(at);
            }
            d.vx = 0.0;
            d.vy = 0.0;
        }
        go_idle(w, id);
    }
}

/// The animation of `s`, the state a flinch goes back to (`current`, the
/// flinch shown, after a flinch).
fn anim_of(s: EnemyState, current: &'static str) -> &'static str {
    match s {
        EnemyState::Strike => DIVE_ANIM,
        EnemyState::GenericHit => current,
        _ => FLYING_ANIM,
    }
}

/// A flinch: the squawk and, half the time each, the high or the low
/// flinch; the bird keeps flying as it was. Draws: one of 0..100.
pub fn enter_generic_hit(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::GenericHit);
    play(w, HIT_SOUND);
    let anim = if rand(w, 100) < HIT_HIGH_CHANCE {
        HIT_HIGH_ANIM
    } else {
        HIT_LOW_ANIM
    };
    set_anim(w, id, anim);
}

/// The flinch: at its end, back to the state it came from and its
/// animation.
pub fn generic_hit_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    let Some(prev) = data(w, id).map(|d| d.prev) else {
        return;
    };
    let current = match obj(w, id).anim.key() {
        Some(HIT_HIGH_ANIM) => HIT_HIGH_ANIM,
        _ => HIT_LOW_ANIM,
    };
    obj_mut(w, id).state = prev as i32;
    if let Some(d) = data_mut(w, id) {
        d.prev = EnemyState::GenericHit;
    }
    set_anim(w, id, anim_of(prev, current));
}

/// `attacker` hit `id`: the shared gate (each attack of Claw once, no hit
/// while the cooldown runs), then the damage, a hit burst and a flinch for
/// every kind it can be hit by (a pistol shot is used up; a shot of the
/// level or an explosion starts a 2000 ms cooldown); at 0 health it dies.
/// Draws: the burst's two, the flinch's one, then the loot's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if data(w, id).is_none_or(|d| d.dead) || !accept_hit(w, id) {
        return;
    }
    let Some(kind) = w.objects.get(attacker).map(|a| a.type_flags) else {
        return;
    };
    match kind {
        attacker::CLAW | attacker::MAGIC | attacker::THROWN | attacker::PISTOL_SHOT => {}
        attacker::LEVEL_SHOT => {
            let now = w.now_ms;
            if let Some(d) = data_mut(w, id) {
                d.cooldown = Timer::new(now, LEVEL_SHOT_COOLDOWN_MS);
            }
        }
        _ => return,
    }
    take_damage(w, id, attacker);
    hit_burst(w, id, attacker);
    if kind == attacker::PISTOL_SHOT {
        w.destroy(attacker);
    }
    enter_generic_hit(w, id);
    if obj(w, id).health <= 0 {
        die(w, id);
    }
}

/// `id` attacked `victim`: the beak needs nothing here.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

/// Dies: drops its loot (once), stops attacking and being hit, is drawn in
/// front of everything, loses its limits, and falls from a hop of
/// 100 px/s up and 100 px/s away from Claw under gravity, through the
/// tiles, until it leaves the screen. Draws: the loot's.
pub fn die(w: &mut World, id: ObjectId) {
    if data(w, id).is_some_and(|d| !d.dead) {
        drop_loot(w, id);
    }
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
        d.death = super::DeathKind::Hit;
        d.cooldown = Timer::default();
    }
    set_state(w, id, EnemyState::Dead);
    {
        let o = obj_mut(w, id);
        o.flags |= flags::HARMLESS | flags::NO_COLLIDE;
        if o.z != DEFAULT_Z {
            o.z = DEFAULT_Z;
            o.flags |= flags::RESORT;
        }
        o.physics_type = ptype::DIRECT;
    }
    set_anim(w, id, DEATH_ANIM);
    if let Some(d) = data_mut(w, id) {
        d.bounds = UNBOUNDED;
        d.bounds_y = UNBOUNDED;
        d.ay = DEATH_GRAVITY;
        d.vx = if d.claw_right {
            -DEATH_LAUNCH.0
        } else {
            DEATH_LAUNCH.0
        };
        d.vy = DEATH_LAUNCH.1;
        d.dead = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_new_states_follow_the_guards_in_the_block() {
        assert_eq!(EnemyState::Withdraw as i32, STATE_BLOCK.start() + 2);
        assert_eq!(EnemyState::GenericHit as i32, STATE_BLOCK.start() + 3);
        assert!(!CLASS.human, "the seagull is no human enemy");
        assert!(CLASS.carry.is_none(), "never lifted");
    }

    #[test]
    fn the_dive_flies_at_a_fixed_speed_along_both_distances() {
        // Claw 200 px ahead and 150 px below: the aim is 110 px below.
        let (vx, vy) = fixed_speed_velocity((0.0, 0.0), (200.0, 110.0), DIVE_SPEED);
        assert!(
            (vx - 387.1).abs() < 0.1 && (vy - 212.9).abs() < 0.1,
            "{vx} {vy}"
        );
        assert!((vx.abs() + vy.abs() - DIVE_SPEED).abs() < 1e-9);
        // Diving left and down.
        let (vx, vy) = fixed_speed_velocity((6404.0, 2700.0), (6304.0, 2900.0), DIVE_SPEED);
        assert!((vx - -200.0).abs() < 1e-9 && (vy - 400.0).abs() < 1e-9);
        assert_eq!(
            fixed_speed_velocity((5.0, 5.0), (5.0, 5.0), DIVE_SPEED),
            (0.0, 0.0)
        );
    }

    #[test]
    fn the_first_dive_of_the_test_room_leaves_at_the_measured_velocity() {
        // One tick after turning on its right limit 6736 at height 2662,
        // with Claw standing at (6490, 2805): (-421.8, 178.2) px/s.
        let at = (6736.0 - 2.16, 2662.0);
        let (vx, vy) = fixed_speed_velocity(at, (6490.0, 2765.0), DIVE_SPEED);
        assert!(
            (vx - -421.8).abs() < 0.05 && (vy - 178.2).abs() < 0.05,
            "{vx} {vy}"
        );
        assert!(dive_gate(at, -120.0, (6462, 6736), (6490, 2805), false, 0));
    }

    #[test]
    fn the_dive_gate_needs_every_condition() {
        let at = (1000.0, 500.0);
        let bounds = (600, 1200);
        // Claw 150 px left and 120 px down, the bird flying left.
        let ok = |at, vx, claw, right, roll| dive_gate(at, vx, bounds, claw, right, roll);
        assert!(ok(at, -120.0, (850, 620), false, 999));
        // Not a pulse tick, or the one roll in 1001 that fails.
        assert!(!ok(at, -120.0, (850, 620), false, 1000));
        assert!(!ok(at, -120.0, (850, 620), false, 0x1FFF_FFFF));
        // Flying away from him.
        assert!(!ok(at, 120.0, (850, 620), false, 0));
        // On or past a limit.
        assert!(!ok(at, -120.0, (600, 620), false, 0));
        assert!(ok(at, -120.0, (601, 620), false, 0));
        assert!(!ok((1100.0, 500.0), 120.0, (1200, 620), true, 0));
        // Down by 100 or by 240 is out of the band; 101 and 239 are in.
        assert!(!ok(at, -120.0, (850, 600), false, 0));
        assert!(ok(at, -120.0, (850, 601), false, 0));
        assert!(ok(at, -120.0, (700, 739), false, 0));
        assert!(!ok(at, -120.0, (700, 740), false, 0));
        // As far down as across, or further: no dive.
        assert!(!ok(at, -120.0, (880, 630), false, 0));
        assert!(!ok(at, -120.0, (870, 630), false, 0));
        assert!(ok(at, -120.0, (869, 630), false, 0));
        // Claw above the bird.
        assert!(!ok(at, -120.0, (850, 380), false, 0));
        // Distances are whole pixels, cut towards zero: 120.9 px down is
        // 120.
        assert!(ok((1000.0, 499.1), -120.0, (879, 620), false, 0));
    }

    #[test]
    fn the_beak_reaches_ahead_and_below_a_bird_facing_left() {
        const { assert!(BEAK.right < 0 && BEAK.bottom > 0) };
        const { assert!(HIT_RECT.left < BEAK.left) };
        assert_eq!(BEAK.mirrored(), Rect::new(5, -5, 26, 22));
    }

    #[test]
    fn a_flinch_goes_back_to_the_animation_of_its_state() {
        assert_eq!(anim_of(EnemyState::Strike, HIT_LOW_ANIM), DIVE_ANIM);
        assert_eq!(anim_of(EnemyState::Idle, HIT_LOW_ANIM), FLYING_ANIM);
        assert_eq!(anim_of(EnemyState::Withdraw, HIT_LOW_ANIM), FLYING_ANIM);
        assert_eq!(
            anim_of(EnemyState::GenericHit, HIT_HIGH_ANIM),
            HIT_HIGH_ANIM
        );
    }
}
