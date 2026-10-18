//! The town guards and the jump switches on a synthetic floor with the
//! fifth level's animation timings: how a guard is built, its idle
//! routine (82 or 85 ticks, a random wait, then pacing), engaging Claw and
//! its strikes high and low (the event 20 or 21 ticks in, Home on 36 or
//! 35, the strike sound on the event tick, 10 damage), the sword reaction
//! (the hop back 100 px from where it stands, the decaying chance), the
//! second guard's parry (35 ticks, unhurt, sliding on), its flinch and
//! death, a lifted second guard's picture, the jump switch's launch, and
//! a guard's strike or any enemy's touch that meets Claw in the air (the
//! shared hit state: hovering 22 ticks, standing on h+22, a jump on h+23).
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::lift::NO_LIFT_ANIM_FRAME;
use claw_sim::logic::enemy::townguard::{
    self, GUARD1, GUARD2, HIGH_STRIKE_ANIM, LOW_STRIKE_ANIM, TownGuardData,
};
use claw_sim::logic::enemy::{self, EnemyState, UNBOUNDED, field};
use claw_sim::logic::{self, ObjectData, jump_switch};
use claw_sim::object::{LogicKind, Object, ObjectId, flags, types};
use claw_sim::physics::ptype;
use claw_sim::player::{attack, special, state};
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

const ATTACK: u32 = 0x2;
const LIFT: u32 = 0x20;

/// An animation of (image, duration, event) elements; `once` makes the
/// last element finish it.
fn anim(key: &str, frames: &[(i32, i32, i32)], once: bool) -> Arc<Animation> {
    let pairs: Vec<(i32, i32)> = frames.iter().map(|&(i, d, _)| (i, d)).collect();
    let mut a = if once {
        Animation::once(key, &pairs)
    } else {
        Animation::looping(key, &pairs)
    };
    for (f, &(_, _, event)) in a.frames.iter_mut().zip(frames) {
        f.event = event;
    }
    Arc::new(a)
}

/// The first guard's first idle pose: nine pictures, 1380 ms, 82 calls.
const IDLE1_1: &[(i32, i32, i32)] = &[
    (100, 100, 0),
    (101, 100, 0),
    (102, 120, 0),
    (103, 200, 0),
    (104, 100, 0),
    (103, 200, 0),
    (104, 90, 0),
    (103, 70, 0),
    (104, 400, 0),
];
/// The second guard's: ten pictures, 85 calls.
const IDLE1_2: &[(i32, i32, i32)] = &[
    (100, 150, 0),
    (101, 150, 0),
    (102, 150, 0),
    (103, 150, 0),
    (104, 150, 0),
    (105, 150, 0),
    (104, 150, 0),
    (103, 150, 0),
    (102, 150, 0),
    (101, 40, 0),
];
/// The second idle pose: four pictures, 19 calls.
const IDLE2: &[(i32, i32, i32)] = &[(103, 70, 0), (102, 100, 0), (101, 70, 0), (100, 70, 0)];
/// The high strike: the event on its fifth picture (calls 20 to 25), done
/// on call 36.
const STRIKE2: &[(i32, i32, i32)] = &[
    (200, 70, 0),
    (201, 80, 0),
    (300, 80, 0),
    (301, 80, 0),
    (302, 100, 1),
    (303, 60, 0),
    (302, 50, 0),
    (301, 50, 0),
    (200, 0, 0),
];
/// The low strike and parry pose: the event on its sixth picture (calls
/// 21 to 23), done on call 35.
const STRIKE1: &[(i32, i32, i32)] = &[
    (200, 70, 0),
    (201, 70, 0),
    (202, 70, 0),
    (203, 90, 0),
    (204, 50, 0),
    (205, 50, 1),
    (204, 50, 0),
    (203, 50, 0),
    (202, 50, 0),
    (201, 30, 0),
    (200, 0, 0),
];
/// A flinch: 19 calls.
const HIT: &[(i32, i32, i32)] = &[(400, 50, 0), (401, 200, 0), (400, 50, 0), (400, 0, 0)];
/// A jump: 19 calls.
const JUMP_ANIM: &[(i32, i32, i32)] = &[(500, 100, 0), (501, 200, 0), (500, 0, 0)];

/// A swing of Claw: wind-up, the active frame (event 1), follow-through.
const SWING: &[(i32, i32, i32)] = &[(24, 50, 0), (25, 50, 0), (27, 100, 1), (24, 50, 0)];

fn guard_anims() -> Vec<Arc<Animation>> {
    let mut v = vec![
        anim(HIGH_STRIKE_ANIM, STRIKE2, true),
        anim(LOW_STRIKE_ANIM, STRIKE1, true),
    ];
    for (g, idle1) in [(GUARD1, IDLE1_1), (GUARD2, IDLE1_2)] {
        let a = g.anims;
        v.push(anim(g.idle1, idle1, true));
        v.push(anim(g.idle2, IDLE2, true));
        v.push(anim(a.hit_low, HIT, true));
        v.push(anim(a.hit_high, HIT, true));
        v.push(anim(a.jump, JUMP_ANIM, true));
        v.push(anim(a.home, &[(1, 0, 0)], true));
        v.push(anim(a.bounce, &[(1, 0, 0)], true));
        v.push(anim(a.advance, &[(10, 100, 0), (11, 100, 0)], false));
        v.push(anim(a.fast_advance, &[(12, 100, 0), (13, 100, 0)], false));
        v.push(anim(a.fall, &[(900, 100, 0)], false));
        v.push(anim(a.lifted2, &[(900, 100, 0), (901, 100, 0)], false));
    }
    // The second guard has no first lift animation, as on the fifth level.
    v.push(anim(
        GUARD1.anims.lifted,
        &[(600, 60, 0), (601, 100, 0), (602, 250, 0)],
        true,
    ));
    v
}

fn claw_anims() -> Vec<Arc<Animation>> {
    let mut v = Vec::new();
    for key in [
        "CLAW_SWIPE",
        "CLAW_PUNCH",
        "CLAW_UPPERCUT",
        "CLAW_KICK",
        "CLAW_DUCKSWIPE",
        "CLAW_JUMPSWIPE",
    ] {
        v.push(anim(key, SWING, true));
    }
    for key in special::HIT_ANIMS {
        v.push(anim(key, &[(200, 150, 0), (201, 200, 0)], true));
    }
    v.push(anim(
        special::LIFT_ANIM,
        &[(40, 100, 0), (42, 100, 0), (43, 100, 0)],
        true,
    ));
    v.push(anim(special::CARRY_ANIM, &[(43, 1000, 0)], false));
    v.push(anim(
        special::THROW_ANIM,
        &[(44, 100, 0), (47, 50, 1), (48, 100, 0)],
        true,
    ));
    v.push(anim("CLAW_STAND", &[(11, 110, 0), (12, 110, 0)], false));
    v.push(anim("CLAW_DUCK", &[(67, 250, 0)], false));
    v.push(anim("CLAW_JUMP", &[(271, 55, 0), (272, 55, 0)], false));
    v.push(anim("CLAW_FALL", &[(278, 65, 0), (280, 80, 0)], false));
    v
}

/// Sounds the tests listen for, with their lengths.
const SOUNDS: &[&str] = &[
    "LEVEL_TOWNGUARD_OFCDKSTB",
    "LEVEL_TOWNGUARD_00390016",
    "LEVEL_TOWNGUARD_LEJUMP2",
    "LEVEL_TOWNGUARD_BLOCK1",
    "LEVEL_TOWNGUARD_BLOCK2",
    "LEVEL_TOWNGUARD_00370013",
    "LEVEL_TOWNGUARD_00370014",
    "LEVEL_TOWNGUARD_OFCHITHI",
    "LEVEL_TOWNGUARD_00370012",
    "LEVEL_TOWNGUARD_00350009",
];

/// Where the arena's guard stands and its patrol limits.
const GUARD_AT: i32 = 3000;
const LIMITS: (i32, i32) = (2700, 3300);

/// The flat floor (feet rest on 959) as the fifth level, with `name`
/// placed at `GUARD_AT` between `LIMITS` and `extra` objects; every
/// object awake.
fn arena(name: &str, extra: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    level.number = 5;
    for a in guard_anims().into_iter().chain(claw_anims()) {
        level.animations.insert(a.key.clone(), a);
    }
    for &k in SOUNDS {
        level.sounds.insert(k.to_string(), Some(200));
    }
    let mut o = ObjectSpec::at(name, GUARD_AT, 900, 0);
    o.record[field::X_MIN] = LIMITS.0;
    o.record[field::X_MAX] = LIMITS.1;
    // The record's own health and smarts do not count.
    o.record[17] = 40;
    o.record[18] = 20;
    level.objects = vec![o];
    level.objects.extend(extra);
    level
}

fn sim_of(level: SimLevel, seed: u32) -> Sim {
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    Sim::new(
        level,
        SimConfig {
            seed,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    )
}

/// The arena with Claw put `dx` px from the guard on the first tick,
/// tough enough to take any number of strikes.
fn with_claw(name: &str, seed: u32, dx: i32) -> (Sim, ObjectId) {
    let mut sim = sim_of(arena(name, vec![]), seed);
    sim.teleport_player(GUARD_AT + dx, 899);
    sim.with_world(|w| w.claw.stats.health = 10_000);
    sim.step(TickInput::default());
    let id = guard_id(&sim);
    (sim, id)
}

/// Claw turns to face left (two ticks of Left).
fn face_left(sim: &mut Sim) {
    idle(sim, 3);
    sim.step(press(LEFT));
    sim.step(hold(LEFT));
    sim.step(TickInput::default());
    assert!(!sim.claw().facing_right, "Claw faces left");
}

fn guard_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| {
            matches!(
                o.kind,
                LogicKind::Game(GameLogic::TownGuard1 | GameLogic::TownGuard2)
            )
        })
        .map(|(id, _)| id)
        .expect("a guard")
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn gdata(sim: &Sim, id: ObjectId) -> TownGuardData {
    match &obj(sim, id).data {
        ObjectData::TownGuard(d) => d.clone(),
        d => panic!("guard data: {d:?}"),
    }
}

fn played(sim: &Sim, key: &str) -> bool {
    sim.last_audio()
        .iter()
        .any(|c| matches!(c, AudioCue::Play(s) if s.key == key))
}

/// Steps until the guard's state is `s` (at most `max` ticks); returns the
/// ticks taken.
fn until_state(sim: &mut Sim, id: ObjectId, s: EnemyState, max: usize) -> usize {
    for n in 1..=max {
        sim.step(TickInput::default());
        if state_of(sim, id) == Some(s) {
            return n;
        }
    }
    panic!(
        "no {} within {max} ticks (now {:?})",
        s.name(),
        state_of(sim, id)
    );
}

// ----------------------------------------------------------------------
// The animations

#[test]
fn the_synthetic_animations_keep_the_fifth_levels_timings() {
    let level = arena("TownGuard1", vec![]);
    let calls = |key: &str| {
        let mut p = claw_sim::anim::AnimPlayer::default();
        p.set(level.animations[key].clone());
        let mut events = Vec::new();
        let mut n = 0;
        while !p.is_done() {
            n += 1;
            if p.advance_in(18, 0, false, None).event == 1 {
                events.push(n);
            }
        }
        (n, events.first().copied())
    };
    assert_eq!(calls(GUARD1.idle1), (82, None));
    assert_eq!(calls(GUARD2.idle1), (85, None));
    assert_eq!(calls(GUARD1.idle2), (19, None));
    assert_eq!(calls(HIGH_STRIKE_ANIM), (36, Some(20)));
    assert_eq!(calls(LOW_STRIKE_ANIM), (35, Some(21)));
    assert_eq!(calls(GUARD1.anims.hit_low), (19, None));
    assert_eq!(calls(GUARD2.anims.jump), (19, None));
}

// ----------------------------------------------------------------------
// Building

#[test]
fn a_guard_is_built_with_its_classes_numbers_and_goes_idle() {
    for (name, g, chance) in [("TownGuard1", GUARD1, 0), ("TownGuard2", GUARD2, 90)] {
        let (sim, id) = with_claw(name, 7, 600);
        let o = obj(&sim, id);
        assert_eq!(o.image_set, g.image_set, "{name}");
        assert_eq!(
            (o.health, o.damage),
            (9, 10),
            "{name}: the record's do not count"
        );
        assert_eq!(o.hit_rect, townguard::HIT_RECT);
        assert_eq!(o.hit_type_flags, enemy::HUMAN_HIT_BY);
        assert_ne!(o.hit_type_flags & jump_switch::TYPE, 0, "switches reach it");
        assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
        let d = gdata(&sim, id);
        assert_eq!(d.jump_back_chance, chance, "{name}");
        assert_eq!(d.second, name == "TownGuard2");
        assert_eq!(d.enemy.ranges, townguard::RANGES);
        assert_eq!(d.enemy.smarts, 99);
        assert_eq!(d.enemy.bounds, LIMITS);
        assert!(d.enemy.may_lift && !d.enemy.sight);
        assert!(
            townguard::DEATH_SOUNDS.contains(&d.enemy.death_sound),
            "{name}"
        );
        assert_eq!(d.enemy.lifted_voice, g.lifted_line);
    }
    // Ten health on any level but the fifth.
    let mut level = arena("TownGuard1", vec![]);
    level.number = 6;
    let mut sim = sim_of(level, 1);
    sim.step(TickInput::default());
    let id = guard_id(&sim);
    assert_eq!(obj(&sim, id).health, 10);
    assert_eq!(
        gdata(&sim, id).jump_back_chance,
        90,
        "a first guard on the sixth"
    );
}

// ----------------------------------------------------------------------
// The idle routine

#[test]
fn the_idle_routine_ignores_claw_in_range_then_engages() {
    for (name, idle1) in [("TownGuard1", 82), ("TownGuard2", 85)] {
        // Claw 141 px away, in range, standing still.
        let (mut sim, id) = with_claw(name, 11, 141);
        let first = until_state(&mut sim, id, EnemyState::Idle2, 200);
        // Built on the first tick and in range from the start: its first
        // pose runs to its end, then the second pose waits.
        assert_eq!(first + 1, idle1, "{name}: the first pose");
        let wait = gdata(&sim, id).idle_wait.len;
        assert!(wait <= 2000, "{name}: wait {wait}");
        let n = until_state(&mut sim, id, EnemyState::Pace, 400);
        assert_eq!(n, wait.div_ceil(18) as usize + 18, "{name}: wait and pose");
        // Pacing with Claw in range goes Home on the next tick, then walks
        // up to him.
        sim.step(TickInput::default());
        assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
        sim.step(TickInput::default());
        assert_eq!(state_of(&sim, id), Some(EnemyState::Advance));
        assert_eq!(enemy::data_of(obj(&sim, id)).unwrap().vx, 60.0);
    }
}

#[test]
fn claw_leaving_range_restarts_the_idle_routine() {
    let (mut sim, id) = with_claw("TownGuard1", 3, 141);
    idle(&mut sim, 40);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    // Claw goes far away: on the first tick out of range the guard goes
    // idle again from the start of its first pose.
    sim.teleport_player(GUARD_AT + 1500, 899);
    sim.step(TickInput::default());
    sim.step(TickInput::default());
    let n = until_state(&mut sim, id, EnemyState::Idle2, 200);
    assert!(n > 60, "the first pose started again ({n} ticks)");
}

// ----------------------------------------------------------------------
// Strikes

/// One tick of a strike: ticks since it started, damage Claw took so far,
/// back Home, a strike sound played.
type StrikeRow = (usize, i32, bool, bool);

/// The guard put at Home with Claw `dx` px away (`duck` crouching), left
/// to decide (`decide`) or made to strike at once; returns, from the tick
/// its strike starts, (ticks since, damage Claw took, back Home, a strike
/// sound played).
fn first_strike(name: &str, dx: i32, duck: bool, decide: bool) -> (Sim, ObjectId, Vec<StrikeRow>) {
    let (mut sim, id) = with_claw(name, 5, dx);
    let input = if duck {
        hold(DOWN)
    } else {
        TickInput::default()
    };
    sim.step(input);
    sim.with_world(|w| {
        enemy::go_home(w, id);
        if !decide {
            enemy::face_claw(w, id);
            enemy::enter_strike(w, id);
        }
    });
    let mut log = Vec::new();
    let health = sim.stats().health;
    let mut start = (!decide).then_some(0);
    for k in 1..400 {
        sim.step(input);
        let s = state_of(&sim, id);
        if start.is_none() && s == Some(EnemyState::Strike) {
            start = Some(k);
        }
        if let Some(st) = start {
            let strike_sound = played(&sim, townguard::STRIKE_SOUNDS[0])
                || played(&sim, townguard::STRIKE_SOUNDS[1]);
            log.push((
                k - st,
                health - sim.stats().health,
                s == Some(EnemyState::Home),
                strike_sound,
            ));
            if k - st > 40 {
                break;
            }
        }
    }
    assert!(start.is_some(), "{name} strikes");
    (sim, id, log)
}

#[test]
fn the_high_strike_lands_20_ticks_in_and_ends_on_36() {
    let (_, _, log) = first_strike("TownGuard1", 60, false, true);
    let hurt = log.iter().find(|r| r.1 > 0).expect("Claw is hit");
    assert_eq!((hurt.0, hurt.1), (20, 10), "the event tick, 10 damage");
    let sound: Vec<usize> = log.iter().filter(|r| r.3).map(|r| r.0).collect();
    assert_eq!(sound, [20], "the strike sound on the event tick");
    let home = log.iter().find(|r| r.2).expect("Home again");
    assert_eq!(home.0, 36);
}

#[test]
fn the_low_strike_at_a_crouching_claw_lands_21_ticks_in_and_ends_on_35() {
    // The low strike's event is a tick later than the high one's.
    let (_, _, log) = first_strike("TownGuard1", 60, true, false);
    let hurt = log.iter().find(|r| r.1 > 0).expect("Claw is hit");
    assert_eq!((hurt.0, hurt.1), (21, 10));
    let home = log.iter().find(|r| r.2).expect("Home again");
    assert_eq!(home.0, 35);
}

#[test]
fn the_strike_choice_follows_claws_posture_every_tick() {
    let (mut sim, id) = with_claw("TownGuard2", 1, 300);
    sim.step(hold(DOWN));
    let d = enemy::data_of(obj(&sim, id)).unwrap().clone();
    assert_eq!(
        (d.strike_anim, d.strike_rect),
        (LOW_STRIKE_ANIM, GUARD2.low_strike)
    );
    sim.step(TickInput::default());
    sim.step(TickInput::default());
    let d = enemy::data_of(obj(&sim, id)).unwrap().clone();
    assert_eq!(
        (d.strike_anim, d.strike_rect),
        (HIGH_STRIKE_ANIM, GUARD2.high_strike)
    );
}

// ----------------------------------------------------------------------
// The sword reaction

/// The guard idle with Claw `dx` px away facing it; Claw swings on the
/// next tick. Returns the states of the guard from the swing on.
fn swing_at(
    name: &str,
    level_number: u32,
    dx: i32,
    ticks: usize,
) -> (Sim, ObjectId, Vec<Option<EnemyState>>) {
    let mut level = arena(name, vec![]);
    level.number = level_number;
    let mut sim = sim_of(level, 9);
    sim.teleport_player(GUARD_AT + dx, 899);
    sim.with_world(|w| w.claw.stats.health = 10_000);
    face_left(&mut sim);
    let id = guard_id(&sim);
    idle(&mut sim, 5);
    let mut states = Vec::new();
    sim.step(press(ATTACK));
    states.push(state_of(&sim, id));
    for _ in 0..ticks {
        sim.step(hold(ATTACK));
        states.push(state_of(&sim, id));
    }
    (sim, id, states)
}

#[test]
fn an_idle_second_guard_hops_back_from_claws_sword() {
    let (mut sim, id) = {
        let mut level = arena("TownGuard2", vec![]);
        level.number = 5;
        let mut sim = sim_of(level, 9);
        sim.teleport_player(GUARD_AT + 85, 899);
        sim.with_world(|w| w.claw.stats.health = 10_000);
        face_left(&mut sim);
        let id = guard_id(&sim);
        idle(&mut sim, 5);
        (sim, id)
    };
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    let x0 = obj(&sim, id).x;
    // The seed's roll passes the 90 chance: on the swing's tick it shouts,
    // and the hop back starts in that tick's post-tick.
    sim.step(press(ATTACK));
    assert!(sim.claw().attacking);
    assert!(townguard::is_sword_swipe(sim.claw().attack.kind));
    assert_eq!(state_of(&sim, id), Some(EnemyState::JumpBack));
    assert!(played(&sim, townguard::JUMP_SOUND), "the jump sound");
    assert_eq!(
        gdata(&sim, id).jump_back_chance,
        60,
        "the chance falls by 30"
    );
    let o = obj(&sim, id);
    assert!(o.flags & flags::SAFE != 0, "safe in the air");
    assert_eq!(o.physics_type, ptype::FLY);
    // It lands 18 ticks later, about 100 px from where it stood.
    let n = until_state(&mut sim, id, EnemyState::Home, 30);
    assert_eq!(n, 18);
    let x = obj(&sim, id).x;
    assert!((x0 - 103..=x0 - 97).contains(&x), "landed at {x} from {x0}");
    assert_eq!(obj(&sim, id).flags & flags::SAFE, 0);
}

#[test]
fn a_first_guard_on_the_fifth_level_does_not_hop_back_until_it_has_been_home_out_of_range() {
    let (_, id_states_sim, states) = swing_at("TownGuard1", 5, 85, 20);
    let _ = id_states_sim;
    assert!(
        !states.contains(&Some(EnemyState::JumpBack)),
        "chance 0: {states:?}"
    );
    // On the sixth level it starts at 90.
    let (_, _, states) = swing_at("TownGuard1", 6, 85, 20);
    assert!(states.contains(&Some(EnemyState::JumpBack)), "{states:?}");
}

#[test]
fn the_reaction_needs_claw_facing_the_guard_and_within_110_px() {
    // Facing away (Claw to the right, facing right).
    let mut level = arena("TownGuard2", vec![]);
    level.number = 5;
    let mut sim = sim_of(level, 9);
    sim.teleport_player(GUARD_AT + 85, 899);
    sim.with_world(|w| w.claw.stats.health = 10_000);
    sim.step(press(RIGHT));
    sim.step(TickInput::default());
    let id = guard_id(&sim);
    idle(&mut sim, 5);
    sim.step(press(ATTACK));
    for _ in 0..10 {
        sim.step(hold(ATTACK));
        assert_ne!(state_of(&sim, id), Some(EnemyState::JumpBack));
    }
    assert_eq!(gdata(&sim, id).jump_back_chance, 90, "no roll");
    // Too far: 150 px.
    let (_, id, states) = swing_at("TownGuard2", 5, 150, 10);
    let _ = id;
    assert!(!states.contains(&Some(EnemyState::JumpBack)));
}

// ----------------------------------------------------------------------
// The parry, the flinch and death

/// A synthetic sword blow of Claw (a new attack) on `id`.
fn sword_blow(sim: &mut Sim, id: ObjectId, kind: i32) {
    let mut a = Object::new(0, 0, 0, 0, "TestBlow".into(), LogicKind::Inert);
    a.type_flags = types::PLAYER;
    a.damage = 5;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    let a = sim.spawn(a);
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        w.claw.attack.kind = kind;
        w.claw.attack.uppercut = false;
        logic::on_hit(w, id, a);
    });
}

#[test]
fn the_second_guard_parries_one_blow_in_four_unhurt_and_sliding() {
    let mut parried = 0;
    let mut hurt = 0;
    for seed in 0..40 {
        let (mut sim, id) = with_claw("TownGuard2", seed, 60);
        // Walking at Claw.
        sim.with_world(|w| {
            enemy::go_home(w, id);
            let d = enemy::data_mut(w, id).unwrap();
            d.vx = 60.0;
        });
        let health = obj(&sim, id).health;
        sword_blow(&mut sim, id, attack::kind::SWIPE);
        match state_of(&sim, id) {
            Some(EnemyState::Block) => {
                parried += 1;
                assert_eq!(obj(&sim, id).health, health, "unhurt");
                assert!(obj(&sim, id).flags & flags::SAFE != 0);
                let x0 = obj(&sim, id).x;
                let n = until_state(&mut sim, id, EnemyState::Home, 60);
                assert_eq!(n, 35, "35 ticks of the parry pose");
                let moved = obj(&sim, id).x - x0;
                assert!((30..=40).contains(&moved.abs()), "slid {moved} px");
                assert_eq!(obj(&sim, id).flags & flags::SAFE, 0);
            }
            Some(EnemyState::HitLow) => {
                hurt += 1;
                assert_eq!(obj(&sim, id).health, health - 5);
            }
            s => panic!("seed {seed}: {s:?}"),
        }
    }
    assert!(
        parried > 0 && hurt > parried,
        "parried {parried}, hurt {hurt}"
    );
    // Never a punch, nor on another level than the fifth and sixth.
    for seed in 0..20 {
        let (mut sim, id) = with_claw("TownGuard2", seed, 60);
        sword_blow(&mut sim, id, attack::kind::PUNCH);
        assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    }
}

#[test]
fn the_first_guard_never_parries_on_the_fifth_level() {
    for seed in 0..30 {
        let (mut sim, id) = with_claw("TownGuard1", seed, 60);
        sword_blow(&mut sim, id, attack::kind::SWIPE);
        assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow), "seed {seed}");
    }
}

#[test]
fn a_flinch_lasts_19_ticks_and_the_second_blow_kills() {
    let (mut sim, id) = with_claw("TownGuard1", 2, 60);
    sword_blow(&mut sim, id, attack::kind::SWIPE);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 4);
    let n = until_state(&mut sim, id, EnemyState::Home, 40);
    assert_eq!(n, 19);
    // A blow during the cooldown does nothing; after it, the second blow
    // kills.
    idle(&mut sim, 20);
    sword_blow(&mut sim, id, attack::kind::SWIPE);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
}

#[test]
fn a_pistol_shot_flies_on_through_a_guard() {
    let (mut sim, id) = with_claw("TownGuard1", 2, 60);
    let mut a = Object::new(0, 0, 0, 0, "TestShot".into(), LogicKind::Inert);
    a.type_flags = types::PISTOL_SHOT;
    a.damage = 8;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    let a = sim.spawn(a);
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        logic::on_hit(w, id, a);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 1);
    assert_eq!(obj(&sim, a).flags & flags::DESTROY, 0, "the shot lives on");
}

// ----------------------------------------------------------------------
// Lifting

#[test]
fn a_lifted_second_guard_shows_its_first_picture_while_claw_lifts_it() {
    let (mut sim, id) = with_claw("TownGuard2", 4, 40);
    sim.with_world(|w| w.claw.stats.health = 100);
    face_left(&mut sim);
    assert!(!sim.level().animations.contains_key(GUARD2.anims.lifted));
    let mut lifted_frames = Vec::new();
    sim.step(press(LIFT));
    for _ in 0..20 {
        if state_of(&sim, id) == Some(EnemyState::Lifted)
            && sim.player().unwrap().frame == enemy::lift::CLAW_LIFT_FRAME
        {
            lifted_frames.push(obj(&sim, id).frame);
        }
        sim.step(hold(LIFT));
    }
    assert!(!lifted_frames.is_empty(), "lifted on Claw's lift frame");
    assert!(
        lifted_frames.iter().all(|&f| f == NO_LIFT_ANIM_FRAME),
        "{lifted_frames:?}"
    );
    assert_eq!(state_of(&sim, id), Some(EnemyState::Flailing));
    assert_eq!(
        obj(&sim, id).draw_flags & claw_sim::object::draw_flags::HIDDEN,
        0
    );
}

// ----------------------------------------------------------------------
// The jump switch

/// A switch at `at` aimed at `target`, inside the guard's patrol.
fn switch(at: (i32, i32), target: (i32, i32)) -> ObjectSpec {
    let mut s = ObjectSpec::at("JumpSwitch", at.0, at.1, 0);
    s.record[field::X_MIN] = target.0;
    s.record[field::Y_MIN] = target.1;
    s.record[field::X_MAX] = target.0 + 30;
    s.record[field::Y_MAX] = target.1 + 20;
    // Hidden, as every switch of the fifth level.
    s.record[11] = 1;
    s
}

/// The arena with a ledge above the guard's patrol, out of its reach
/// (cells 45 to 50 of row 12, top at y 768), and the original
/// activation windows: Claw on the ledge near the switch keeps the switch
/// awake while the guard paces below.
fn ledge_sim(extra: Vec<ObjectSpec>, seed: u32) -> Sim {
    let mut level = arena("TownGuard1", extra);
    let mut rows = vec![".".repeat(200); 15];
    rows[12].replace_range(45..51, "######");
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    level.map = claw_sim::tiles::TileMap::from_ascii(64, &rows);
    level.start = (3000, 700);
    Sim::new(
        level,
        SimConfig {
            seed,
            ..SimConfig::default()
        },
    )
}

#[test]
fn a_switch_is_set_up_as_a_small_attacker_and_never_moves() {
    let mut sim = ledge_sim(vec![switch((2800, 930), (2600, 900))], 1);
    idle(&mut sim, 3);
    let (_, s) = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::JumpSwitch))
        .expect("a switch");
    assert_eq!(s.state, jump_switch::SET_UP);
    assert_eq!(s.attack_rect, jump_switch::ATTACK_RECT);
    assert_eq!(s.type_flags, jump_switch::TYPE);
    assert_eq!(s.hit_type_flags, jump_switch::HIT_MASK);
    assert_eq!(s.attack_type_flags, 0);
    assert_eq!((s.x, s.y), (2800, 930));
    assert_eq!(
        jump_switch::target(s.record.as_deref().unwrap()),
        (2600, 900)
    );
}

/// A first guard pacing over a switch with Claw far away is launched on
/// the tick its box meets the switch's.
#[test]
fn a_pacing_guard_is_launched_through_the_switchs_target_and_lands_below() {
    let target = (2600, 880);
    let mut sim = ledge_sim(vec![switch((2800, 930), target)], 1);
    let id = {
        sim.step(TickInput::default());
        guard_id(&sim)
    };
    let mut launched = None;
    let mut prev = (0, 0);
    for k in 0..2000 {
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        if state_of(&sim, id) == Some(EnemyState::Jump) {
            let d = enemy::data_of(o).unwrap();
            launched = Some((k, d.vx, d.vy, prev, d.bounds));
            break;
        }
        prev = (o.x, o.y);
    }
    let (k, vx, vy, from, bounds) = launched.expect("launched by the switch");
    assert_eq!(bounds, UNBOUNDED, "no patrol limits any more");
    assert!(obj(&sim, id).flags & flags::SAFE != 0, "safe in the air");
    // The solve from the guard's position: it passes through the target.
    let (dx, dy) = (
        f64::from(target.0 - obj(&sim, id).x),
        f64::from(target.1 - obj(&sim, id).y),
    );
    let (wx, wy) = jump_switch::launch_speed(dx, dy, enemy::GRAVITY);
    assert!(
        (vx - wx).abs() < 3.0 && (vy - wy).abs() < 3.0,
        "{vx},{vy} against {wx},{wy}"
    );
    assert!(vx < 0.0 && vy < 0.0, "up and to the left");
    assert!(k > 82, "after its idle routine ({k})");
    // It flies on past the target and lands on the floor below, then goes
    // Home, free and hittable again.
    let mut closest = i64::MAX;
    for _ in 0..200 {
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        let d = i64::from(o.x - target.0).pow(2) + i64::from(o.y - target.1).pow(2);
        closest = closest.min(d);
        if state_of(&sim, id) != Some(EnemyState::Jump) {
            break;
        }
    }
    assert!(
        closest <= 36,
        "passes within 6 px of the target ({closest})"
    );
    let o = obj(&sim, id);
    assert_eq!(o.physics_type, ptype::STAND);
    assert_eq!(o.y, from.1, "on the floor it paced");
    assert!(o.x < target.0, "beyond the target");
    assert_eq!(o.flags & flags::SAFE, 0);
    assert!(!enemy::data_of(o).unwrap().busy);
    assert_eq!(enemy::data_of(o).unwrap().bounds, UNBOUNDED);
}

#[test]
fn a_switch_launches_a_guard_once_per_attack_and_never_claw() {
    let (mut sim, id) = with_claw("TownGuard1", 3, 600);
    let mut sw = Object::new(0, 0, 0, 0, "JumpSwitch".into(), LogicKind::Inert);
    sw.type_flags = jump_switch::TYPE;
    sw.flags = flags::NO_COLLIDE | flags::UNLISTED;
    let mut record = Box::new([0; claw_assets::wwd::OBJECT_INTS]);
    record[field::X_MIN] = GUARD_AT - 200;
    record[field::Y_MIN] = 880;
    sw.record = Some(record);
    let sw = sim.spawn(sw);
    let hit = |sim: &mut Sim| sim.with_world(|w| logic::on_hit(w, id, sw));
    hit(&mut sim);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Jump));
    let v = enemy::data_of(obj(&sim, id)).unwrap().vx;
    // A second overlap in the same attack of Claw does nothing.
    sim.with_world(|w| enemy::data_mut(w, id).unwrap().vx = 1.0);
    hit(&mut sim);
    assert_eq!(enemy::data_of(obj(&sim, id)).unwrap().vx, 1.0);
    assert!(v < 0.0);
    // Claw's hit mask lacks the switch's kind.
    assert_eq!(sim.player().unwrap().hit_type_flags & jump_switch::TYPE, 0);
}

#[test]
fn a_switch_reached_during_a_hit_cooldown_is_used_up() {
    let (mut sim, id) = with_claw("TownGuard1", 3, 600);
    sword_blow(&mut sim, id, attack::kind::SWIPE);
    let mut sw = Object::new(0, 0, 0, 0, "JumpSwitch".into(), LogicKind::Inert);
    sw.type_flags = jump_switch::TYPE;
    sw.flags = flags::NO_COLLIDE | flags::UNLISTED;
    let mut record = Box::new([0; claw_assets::wwd::OBJECT_INTS]);
    record[field::X_MIN] = GUARD_AT - 200;
    record[field::Y_MIN] = 880;
    sw.record = Some(record);
    let sw = sim.spawn(sw);
    sim.step(TickInput::default());
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        logic::on_hit(w, id, sw);
    });
    assert_ne!(state_of(&sim, id), Some(EnemyState::Jump), "cooldown");
    // The attack's number is spent: after the cooldown the same attack
    // still does nothing.
    idle(&mut sim, 20);
    sim.with_world(|w| logic::on_hit(w, id, sw));
    assert_ne!(state_of(&sim, id), Some(EnemyState::Jump));
}

// ----------------------------------------------------------------------
// A strike that meets Claw in the air

/// Claw jumps beside a striking guard so the strike reaches him in the
/// air; `input(t)` is played on tick `t` counted from the hit (0). Returns
/// Claw's (state, y) from the hit tick on.
fn struck_in_the_air(input: impl Fn(u64) -> TickInput) -> Vec<(i32, i32)> {
    let (mut sim, id) = with_claw("TownGuard1", 5, 60);
    sim.with_world(|w| w.claw.stats.health = 100);
    sim.with_world(|w| enemy::go_home(w, id));
    // Find the strike's event tick standing, then jump so he is rising
    // when it comes.
    let mut event = None;
    {
        let mut trial = sim_of(arena("TownGuard1", vec![]), 5);
        trial.teleport_player(GUARD_AT + 60, 899);
        trial.with_world(|w| w.claw.stats.health = 10_000);
        trial.step(TickInput::default());
        let tid = guard_id(&trial);
        trial.with_world(|w| enemy::go_home(w, tid));
        let h = trial.stats().health;
        for k in 0..200 {
            trial.step(TickInput::default());
            if trial.stats().health < h {
                event = Some(k);
                break;
            }
        }
    }
    let event = event.expect("the strike lands");
    let jump_at = event - 3;
    let health = sim.stats().health;
    let mut hit = None;
    let mut rows = Vec::new();
    for k in 0..event + 60 {
        let t = hit.map_or(0, |h: usize| k - h);
        let i = if k == jump_at {
            press(JUMP)
        } else if hit.is_some() {
            input(t as u64)
        } else {
            TickInput::default()
        };
        sim.step(i);
        if hit.is_none() && sim.stats().health < health {
            hit = Some(k);
            let o = sim.player().unwrap();
            assert_eq!(o.state, state::HIT);
            assert_eq!(o.physics_type, ptype::FALL, "hit in the air");
        }
        if hit.is_some() {
            let o = sim.player().unwrap();
            rows.push((o.state, o.y));
        }
    }
    assert!(hit.is_some(), "the strike hit Claw");
    rows
}

#[test]
fn a_guards_strike_in_the_air_holds_claw_22_ticks_then_one_tick_takes_a_jump() {
    let rows = struck_in_the_air(|_| TickInput::default());
    let y = rows[0].1;
    for (t, &(s, ry)) in rows.iter().enumerate().take(22) {
        assert_eq!((s, ry), (state::HIT, y), "tick h+{t}");
    }
    assert_eq!(rows[22], (state::STAND, y), "standing in mid-air on h+22");
    assert_eq!(rows[23].0, state::FALL, "falling from h+23");
    let rows = struck_in_the_air(|t| {
        if t == 23 {
            press(JUMP)
        } else {
            TickInput::default()
        }
    });
    assert_eq!(rows[23].0, state::JUMP, "a jump on h+23");
    assert!(rows[30].1 < y - 40, "a full jump: {:?}", &rows[23..31]);
}

/// Any enemy's touch in the air does the same (the seagull's beak is one):
/// a bare enemy body put where Claw rises.
#[test]
fn any_enemys_touch_in_the_air_reaches_the_same_hit_state() {
    let run = |jump_on: Option<usize>| -> Vec<(i32, i32)> {
        let mut sim = sim_of(arena("TownGuard1", vec![]), 3);
        sim.teleport_player(1000, 899);
        idle(&mut sim, 30);
        sim.step(press(JUMP));
        for _ in 0..4 {
            sim.step(hold(JUMP));
        }
        let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
        let mut beak = Object::new(0, x, y, 300, "TestBeak".into(), LogicKind::Inert);
        beak.flags = flags::WORLD;
        beak.type_flags = types::ENEMY;
        beak.damage = 10;
        beak.attack_rect = claw_sim::object::Rect::new(-5, -5, 5, 5);
        let beak = sim.spawn(beak);
        let health = sim.stats().health;
        let mut rows = Vec::new();
        let mut hit = None;
        for k in 0..60 {
            let t = hit.map(|h: usize| k - h);
            sim.step(if t.is_some() && t == jump_on {
                press(JUMP)
            } else {
                TickInput::default()
            });
            if hit.is_none() && sim.stats().health < health {
                hit = Some(k);
                // The touch is over: the body goes.
                sim.with_world(|w| {
                    if let Some(o) = w.objects.get_mut(beak) {
                        o.flags |= flags::NO_COLLIDE;
                    }
                });
                let o = sim.player().unwrap();
                assert_eq!((o.state, o.physics_type), (state::HIT, ptype::FALL));
            }
            if hit.is_some() {
                let o = sim.player().unwrap();
                rows.push((o.state, o.y));
            }
        }
        assert_eq!(sim.stats().health, health - 10, "one touch, 10 health");
        rows
    };
    let rows = run(None);
    let y = rows[0].1;
    assert!(rows[..22].iter().all(|&r| r == (state::HIT, y)), "{rows:?}");
    assert_eq!(rows[22], (state::STAND, y));
    assert_eq!(rows[23].0, state::FALL);
    let rows = run(Some(23));
    assert_eq!(rows[23].0, state::JUMP, "a jump on h+23");
    assert!(rows[30].1 < y - 40, "a full jump");
}

// ---------------------------------------------------------------- install

fn level5() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(5)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

#[test]
fn install_level5_every_guard_and_switch_is_built() {
    let Some(level) = level5() else { return };
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level.clone(),
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    idle(&mut sim, 3);
    let mut guards = [0, 0];
    let mut switches = 0;
    for (_, o) in sim.objects().all() {
        match o.kind {
            LogicKind::Game(GameLogic::TownGuard1 | GameLogic::TownGuard2) => {
                let ObjectData::TownGuard(d) = &o.data else {
                    panic!("{} at {},{}: {:?}", o.logic, o.x, o.y, o.data)
                };
                guards[usize::from(d.second)] += 1;
                assert_eq!(o.health, 9);
                assert_eq!(
                    o.physics_type,
                    ptype::STAND,
                    "{} at {},{}",
                    o.logic,
                    o.x,
                    o.y
                );
            }
            LogicKind::Game(GameLogic::JumpSwitch) => switches += 1,
            _ => {}
        }
    }
    assert_eq!((guards, switches), ([73, 30], 7));
    // A switch keeps the original's small window even in the widened
    // mode: the first one is set up once Claw is near it.
    let mut near = level.clone();
    near.start = (2300, 2800);
    let mut sim = Sim::new(near, SimConfig::default());
    idle(&mut sim, 3);
    let (_, s) = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::JumpSwitch) && o.x == 2402)
        .expect("the first switch");
    assert_eq!(s.state, jump_switch::SET_UP);
    assert_eq!(s.type_flags, jump_switch::TYPE);
    assert_eq!(s.attack_rect, jump_switch::ATTACK_RECT);
    // Their art: the second guard has no first lift animation.
    let lv = sim.level();
    assert!(lv.animations.contains_key(GUARD1.anims.lifted));
    assert!(!lv.animations.contains_key(GUARD2.anims.lifted));
    for g in [GUARD1, GUARD2] {
        for key in [g.idle1, g.idle2, g.anims.jump, g.anims.hit_low] {
            assert!(lv.animations.contains_key(key), "{key}");
        }
    }
}
