//! Raux on a synthetic arena, with the boss fight flag set directly: how
//! he is built, waiting for the fight, his decisions at Home, his strike,
//! his leap over Claw, his dodges, blocks and flinches, his health and
//! the health bar, his death and its report, the reset when Claw
//! respawns, and the order of his random draws.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::output::BossMeter;
use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::human::{self, HumanCall};
use claw_sim::logic::enemy::{self, Deferred, EnemyState, raux};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, special, types};
use claw_sim::physics::ptype;
use claw_sim::player::attack::kind as attack_kind;
use claw_sim::player::state as claw_state;
use claw_sim::rng::Rng;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

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

/// Raux's animations with the lengths of the game's: the strike of 350 ms
/// with its blow on the fourth element, flinches of 322 ms, the block of
/// 600 ms, the hop back of 1322 ms.
fn raux_anims() -> Vec<Arc<Animation>> {
    let k = |s: &str| format!("LEVEL_RAUX_{s}");
    let flinch = [(40, 80, 0), (41, 80, 0), (42, 81, 0), (43, 81, 0)];
    vec![
        anim(
            &k("STRIKE"),
            &[
                (30, 20, 0),
                (31, 40, 0),
                (32, 90, 0),
                (33, 100, 1),
                (34, 100, 0),
            ],
            true,
        ),
        anim(&k("HITLOW"), &flinch, true),
        anim(&k("HITHIGH"), &flinch, true),
        anim(&k("BLOCKLOW"), &[(50, 100, 0), (51, 500, 0)], true),
        anim(&k("BLOCKHIGH"), &[(52, 100, 0), (53, 500, 0)], true),
        anim(&k("JUMP"), &[(60, 57, 0), (61, 57, 0), (62, 58, 0)], false),
        anim(&k("JUMPBACK"), &[(70, 146, 0); 9], true),
        anim(&k("IDLE"), &[(1, 3000, 0), (2, 233, 0)], false),
        anim(&k("ADVANCE"), &[(3, 200, 0), (4, 200, 0)], false),
        anim(&k("FASTADVANCE"), &[(5, 200, 0), (6, 200, 0)], false),
        anim(&k("HOME"), &[(7, 0, 0)], true),
        anim(&k("FALL"), &[(8, 100, 0)], false),
        anim(&k("DEAD"), &[(9, 100, 0)], false),
    ]
}

/// Sounds the tests listen for, with their lengths.
const SOUNDS: [(&str, u32); 9] = [
    (raux::IDLE_LINE, 900),
    (raux::DEATH_SOUND, 900),
    ("LEVEL_STAB", 200),
    ("LEVEL_STAB2", 200),
    ("LEVEL_STAB3", 200),
    ("LEVEL_RAUX_BLOCK1", 300),
    ("LEVEL_RAUX_BLOCK2", 300),
    ("GAME_SOLHITL2", 200),
    ("CLAW_1001", 900),
];

/// The patrol limits of the arena's Raux.
const LIMITS: (i32, i32) = (2700, 3300);

/// The flat floor of the common helpers (feet rest on 959) with Raux at
/// x = 3000 between `LIMITS`, every object awake.
fn arena(seed: u32) -> Sim {
    let mut level = flat_level();
    for a in raux_anims() {
        level.animations.insert(a.key.clone(), a);
    }
    for (k, ms) in SOUNDS {
        level.sounds.insert(k.to_string(), Some(ms));
    }
    let mut o = ObjectSpec::at("Raux", 3000, 900, 0);
    o.image_set = "LEVEL_RAUX".into();
    o.record[enemy::field::X_MIN] = LIMITS.0;
    o.record[enemy::field::X_MAX] = LIMITS.1;
    // The record's own health and smarts do not count.
    o.record[17] = 7;
    o.record[18] = 33;
    level.objects = vec![o];
    sim_of(level, seed)
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

fn raux_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Raux))
        .map(|(id, _)| id)
        .expect("Raux exists")
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn data(sim: &Sim, id: ObjectId) -> enemy::EnemyData {
    enemy::data_of(obj(sim, id)).expect("built").clone()
}

fn safe(sim: &Sim, id: ObjectId) -> bool {
    obj(sim, id).flags & flags::SAFE != 0
}

fn cues(sim: &Sim) -> Vec<String> {
    sim.last_audio()
        .iter()
        .filter_map(|c| match c {
            AudioCue::Play(s) => Some(s.key.clone()),
            _ => None,
        })
        .collect()
}

/// The arena with Claw `dx` px from Raux (standing, tough), fight flag
/// as given, after `ticks` ticks.
fn with_claw(seed: u32, dx: i32, flag: u8, ticks: usize) -> (Sim, ObjectId) {
    let mut sim = arena(seed);
    let id = raux_id(&sim);
    sim.teleport_player(3000 + dx, 899);
    sim.with_world(|w| {
        w.claw.stats.health = 10_000;
        w.set_boss_fight(flag);
    });
    idle(&mut sim, ticks);
    (sim, id)
}

/// Raux in the fight with Claw `dx` px away, past his start-up, put at
/// Home, standing still and free.
fn in_fight(dx: i32) -> (Sim, ObjectId) {
    let (mut sim, id) = with_claw(3, dx, fight::ON, 3);
    assert_ne!(state_of(&sim, id), Some(EnemyState::Startup));
    sim.with_world(|w| {
        enemy::go_home(w, id);
        enemy::data_mut(w, id).unwrap().busy = false;
    });
    (sim, id)
}

/// True when tick `k` (counted from 0) is a frame pulse tick, the only
/// ticks on which enemies make their per-tick roll.
fn pulse_tick(k: u64) -> bool {
    k >= 5 && (k - 5).is_multiple_of(3)
}

/// A stream state whose next draws make Raux's pre-tick roll `roll_in`
/// (on a pulse tick); searched from 0.
fn stream_with_roll(roll_in: std::ops::Range<u32>) -> u32 {
    (0..1_000_000u32)
        .find(|&s| roll_in.contains(&Rng::new(s).below(1001)))
        .expect("a stream")
}

/// A synthetic attacker of `type_flags` with `damage`.
fn attacker(sim: &mut Sim, type_flags: u32, damage: i32, special: u32) -> ObjectId {
    let mut a = Object::new(0, 0, 0, 0, "TestShot".into(), LogicKind::Inert);
    a.type_flags = type_flags;
    a.damage = damage;
    a.special_flags = special;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    sim.spawn(a)
}

/// `attacker` hits `id` as a new attack of Claw, Raux's roll set to
/// `roll`; returns the random draws the hit made.
fn hit(sim: &mut Sim, id: ObjectId, attacker: ObjectId, roll: i32, upper: bool) -> u32 {
    let before = sim.rng().state();
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        w.claw.attack.uppercut = upper;
        enemy::data_mut(w, id).unwrap().roll = roll;
        let at = w.objects.get(id).map(|o| (o.x, o.y)).unwrap();
        if let Some(o) = w.objects.get_mut(attacker) {
            o.x = at.0;
            o.y = at.1;
            o.attack_rect = Rect::new(-5, -5, 5, 5);
        }
        logic::on_hit(w, id, attacker);
    });
    draws_between(before, sim.rng().state())
}

fn draws_between(a: u32, b: u32) -> u32 {
    let mut r = Rng::new(a);
    for n in 0..=64 {
        if r.state() == b {
            return n;
        }
        r.next_u15();
    }
    panic!("more than 64 draws");
}

// ----------------------------------------------------------------------
// Building and waiting for the fight

#[test]
fn raux_is_built_as_the_boss_waiting_for_the_fight() {
    let (sim, id) = with_claw(3, 120, fight::NONE, 2);
    let o = obj(&sim, id);
    assert_eq!(o.kind, LogicKind::Game(GameLogic::Raux));
    assert!(matches!(o.data, ObjectData::Raux(_)), "his own data");
    assert_eq!(sim.boss().current(), Some(id), "the current boss");
    assert_eq!(o.health, 100, "the record's health does not count");
    assert_eq!(o.damage, 5);
    assert_eq!(o.image_set, "LEVEL_RAUX");
    assert_eq!(o.move_rect, raux::MOVE_RECT);
    assert_eq!(o.hit_rect, raux::HIT_RECT);
    assert_eq!(o.attack_rect, raux::CONTACT);
    assert!(o.flags & flags::ALWAYS_ACTIVE != 0);
    assert_eq!(o.special_flags & special::LIFTABLE, 0, "never liftable");
    assert_eq!(o.hit_type_flags, enemy::HUMAN_HIT_BY);
    assert_eq!(o.physics_type, ptype::STAND);
    assert_eq!(o.y + o.move_rect.bottom, 959, "on the floor");
    let d = data(&sim, id);
    assert_eq!(d.smarts, 100);
    assert_eq!(d.ranges, raux::RANGES);
    assert_eq!(d.bounds, LIMITS);
    assert!(!d.may_lift);
    let b = d.human.boss.expect("the boss layer");
    assert_eq!(b.start, (3000, 900), "placed position kept");
    assert_eq!(b.full_health, 100);
    assert!(enemy::class(GameLogic::Raux).unwrap().carry.is_none());
    // Claw within 150 px: he waits in Startup, idle.
    assert_eq!(d.mode, 3);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    assert_eq!(obj(&sim, id).anim.key(), Some(raux::IDLE_ANIM));
}

#[test]
fn startup_holds_until_the_fight_is_on() {
    let (mut sim, id) = with_claw(3, 120, fight::NONE, 2);
    for flag in [fight::NONE, fight::STAGING] {
        sim.with_world(|w| w.set_boss_fight(flag));
        idle(&mut sim, 30);
        assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    }
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    let d = data(&sim, id);
    assert_eq!(d.pace_wait.len, raux::IDLE_WAIT_MS);
    // Within 150 px, idle sends him Home on the next tick.
    sim.step(TickInput::default());
    assert_ne!(state_of(&sim, id), Some(EnemyState::Idle));
}

#[test]
fn out_of_range_on_his_first_tick_he_leaves_startup_whatever_the_flag() {
    let (sim, id) = with_claw(3, 2000, fight::NONE, 1);
    assert_eq!(sim.boss().fight(), fight::NONE);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
}

#[test]
fn going_idle_says_his_line_when_the_roll_is_below_500() {
    for (roll, says) in [(499, true), (500, false), (0x1FFF_FFFF, false)] {
        let (mut sim, id) = with_claw(3, 120, fight::NONE, 2);
        sim.with_world(|w| {
            enemy::data_mut(w, id).unwrap().roll = roll;
            raux::go_idle(w, id);
        });
        sim.step(TickInput::default());
        assert_eq!(
            cues(&sim).iter().any(|c| c == raux::IDLE_LINE),
            says,
            "roll {roll}"
        );
    }
}

#[test]
fn idle_out_of_range_he_may_pace_only_after_four_seconds_on_a_pulse() {
    let (mut sim, id) = with_claw(3, 2000, fight::ON, 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    let wait = data(&sim, id).pace_wait;
    // Before the wait is over Idle draws nothing, even on a pulse tick.
    let before = sim.rng_state();
    sim.with_world(|w| {
        w.pulse = true;
        raux::idle_tick(w, id);
    });
    assert_eq!(sim.rng_state(), before);
    // After it, a tick off the frame pulse draws nothing and he stays.
    sim.with_world(|w| {
        enemy::data_mut(w, id).unwrap().pace_wait = enemy::Timer::new(0, 0);
    });
    let before = sim.rng_state();
    sim.with_world(|w| {
        w.pulse = false;
        raux::idle_tick(w, id);
    });
    assert_eq!(sim.rng_state(), before);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    // On a pulse tick, one draw of 0..100: below 5 he paces.
    let five = (0..100_000u32)
        .find(|&s| Rng::new(s).below(100) < 5)
        .unwrap();
    let more = (0..100_000u32)
        .find(|&s| Rng::new(s).below(100) >= 5)
        .unwrap();
    let mut s = sim.clone();
    s.set_rng_state(more);
    s.with_world(|w| {
        w.pulse = true;
        raux::idle_tick(w, id);
    });
    assert_eq!(state_of(&s, id), Some(EnemyState::Idle));
    assert_eq!(draws_between(more, s.rng_state()), 1);
    let mut s = sim.clone();
    s.set_rng_state(five);
    s.with_world(|w| {
        w.pulse = true;
        raux::idle_tick(w, id);
    });
    assert_eq!(state_of(&s, id), Some(EnemyState::Pace));
    assert_eq!(wait.len, 4000);
}

// ----------------------------------------------------------------------
// Decisions and draws

/// Mirrors the draws of a Raux pre-tick without a dodge check.
fn pre_draws(m: &mut Rng, pulse: bool) -> usize {
    if pulse {
        m.below(1001);
    }
    m.below(3) as usize
}

#[test]
fn his_draws_follow_the_games_order_and_home_decides_by_them() {
    let mut seen = std::collections::BTreeSet::new();
    for seed in 1..120u32 {
        let (mut sim, id) = with_claw(seed, 120, fight::NONE, 0);
        let mut m = Rng::new(sim.rng_state());
        // Waiting: the roll on pulse ticks, then the sound pick.
        for _ in 0..8 {
            let pulse = pulse_tick(sim.tick());
            sim.step(TickInput::default());
            pre_draws(&mut m, pulse);
            assert_eq!(sim.rng_state(), m.state(), "seed {seed}: startup");
        }
        assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
        // The fight starts: idle (no draw), then Home (no draw).
        sim.with_world(|w| w.set_boss_fight(fight::ON));
        for expect in [EnemyState::Idle, EnemyState::Home] {
            let pulse = pulse_tick(sim.tick());
            sim.step(TickInput::default());
            pre_draws(&mut m, pulse);
            assert_eq!(sim.rng_state(), m.state(), "seed {seed}");
            assert_eq!(state_of(&sim, id), Some(expect), "seed {seed}");
        }
        // Home within 150 px (mode 3): one of 0..121, then by it.
        assert_eq!(data(&sim, id).mode, 3);
        let pulse = pulse_tick(sim.tick());
        sim.step(TickInput::default());
        pre_draws(&mut m, pulse);
        let r = m.below(121);
        let expect = if r < 100 {
            EnemyState::Strike
        } else if r < 120 {
            m.below(41);
            EnemyState::MoveAhead
        } else if m.below(421) < 100 {
            EnemyState::Strike
        } else {
            m.below(301);
            EnemyState::Pause
        };
        assert_eq!(sim.rng_state(), m.state(), "seed {seed}: Home");
        assert_eq!(state_of(&sim, id), Some(expect), "seed {seed}");
        seen.insert(expect as i32);
    }
    assert!(seen.contains(&(EnemyState::Strike as i32)));
    assert!(seen.contains(&(EnemyState::MoveAhead as i32)));
}

#[test]
fn up_close_home_backs_off_or_strikes_even_odds() {
    let mut seen = std::collections::BTreeSet::new();
    for seed in 1..60u32 {
        let (mut sim, id) = with_claw(seed, 60, fight::ON, 2);
        // Idle, then Home.
        while state_of(&sim, id) != Some(EnemyState::Home) {
            sim.step(TickInput::default());
        }
        assert_eq!(data(&sim, id).mode, 4);
        let mut m = Rng::new(sim.rng_state());
        let pulse = pulse_tick(sim.tick());
        sim.step(TickInput::default());
        pre_draws(&mut m, pulse);
        m.below(121);
        let expect = if m.below(100) < 50 {
            m.below(51);
            EnemyState::MoveBack
        } else {
            EnemyState::Strike
        };
        assert_eq!(sim.rng_state(), m.state(), "seed {seed}");
        assert_eq!(state_of(&sim, id), Some(expect), "seed {seed}");
        seen.insert(expect as i32);
    }
    assert_eq!(seen.len(), 2);
}

#[test]
fn at_300_px_home_leaps_over_claw_a_sixth_of_the_time_or_walks_at_him() {
    let mut seen = std::collections::BTreeSet::new();
    for seed in 1..80u32 {
        let (mut sim, id) = with_claw(seed, -250, fight::ON, 2);
        sim.with_world(|w| enemy::go_home(w, id));
        assert_eq!(data(&sim, id).mode, 2);
        let mut m = Rng::new(sim.rng_state());
        let pulse = pulse_tick(sim.tick());
        sim.step(TickInput::default());
        pre_draws(&mut m, pulse);
        let r = m.below(121);
        let expect = if r < 20 {
            EnemyState::JumpOver
        } else {
            m.below(421);
            EnemyState::Advance
        };
        assert_eq!(sim.rng_state(), m.state(), "seed {seed}");
        assert_eq!(state_of(&sim, id), Some(expect), "seed {seed}");
        seen.insert(expect as i32);
    }
    assert_eq!(seen.len(), 2);
}

#[test]
fn the_strike_and_block_sounds_are_picked_every_tick() {
    let (mut sim, id) = with_claw(9, 120, fight::NONE, 1);
    let mut block = "";
    for _ in 0..30 {
        let mut m = Rng::new(sim.rng_state());
        let pulse = pulse_tick(sim.tick());
        let pick = pre_draws(&mut m, pulse);
        sim.step(TickInput::default());
        let d = data(&sim, id);
        assert_eq!(d.human.sounds.strike, raux::STRIKE_SOUNDS[pick]);
        if pick < 2 {
            block = raux::BLOCK_SOUNDS[pick];
        }
        assert_eq!(d.human.sounds.block_low, block, "the third keeps it");
    }
}

// ----------------------------------------------------------------------
// The strike

#[test]
fn the_strike_lands_once_with_its_box_and_damage_and_ends_with_its_animation() {
    let (mut sim, id) = in_fight(80);
    sim.with_world(|w| enemy::enter_strike(w, id));
    let claw_health = sim.stats().health;
    let mut live = Vec::new();
    let mut n = 0;
    while state_of(&sim, id) == Some(EnemyState::Strike) {
        let pick = {
            let mut m = Rng::new(sim.rng_state());
            pre_draws(&mut m, pulse_tick(sim.tick()))
        };
        sim.step(TickInput::default());
        n += 1;
        let o = obj(&sim, id);
        if o.attack_rect == raux::STRIKE.mirrored() {
            live.push(n);
            assert!(
                cues(&sim).contains(&raux::STRIKE_SOUNDS[pick].to_string()),
                "the sound picked this tick"
            );
        }
        assert!(n < 40);
    }
    assert_eq!(live.len(), 1, "one tick of strike box: {live:?}");
    // Each element shows for whole ticks (20 ms takes 2, 40 ms 3, 90 ms
    // 5): the blow element comes on the eleventh tick.
    assert_eq!(live[0], 11, "lands on tick {}", live[0]);
    // Two more elements of 100 ms (6 ticks each), then Home.
    assert_eq!(n, 23, "{n} ticks");
    assert_eq!(sim.stats().health, claw_health - 5, "5 damage to Claw");
}

// ----------------------------------------------------------------------
// The leap over Claw

#[test]
fn the_leap_over_claw_flies_past_him_towards_his_far_limit() {
    // Claw 200 px to his left: he lands 20 px inside his left limit.
    let (mut sim, id) = in_fight(-200);
    sim.with_world(|w| {
        enemy::go_home(w, id);
        human::enter_jump_over(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::JumpOver));
    assert!(safe(&sim, id) && data(&sim, id).busy);
    assert!(!enemy::faces_right(obj(&sim, id)), "facing Claw");
    let mut n = 0;
    loop {
        sim.step(TickInput::default());
        n += 1;
        if state_of(&sim, id) != Some(EnemyState::JumpOver) {
            break;
        }
        assert_eq!(obj(&sim, id).attack_rect, human::JUMP_OVER_RECT);
        assert!(safe(&sim, id));
        assert!(n < 80);
    }
    // Aimed with his origin at the height of Claw's feet, he comes down
    // on the floor short of the spot 20 px inside his limit, past Claw.
    let o = obj(&sim, id);
    assert!(o.x >= LIMITS.0 + 20 && o.x < 2800, "lands at {}", o.x);
    assert_eq!(o.y + o.move_rect.bottom, 959);
    assert!(!safe(&sim, id));
    // Claw to his right: the right limit.
    let (mut sim, id) = in_fight(200);
    sim.with_world(|w| {
        enemy::go_home(w, id);
        human::enter_jump_over(w, id);
    });
    assert!(enemy::faces_right(obj(&sim, id)));
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, id).attack_rect, human::JUMP_OVER_RECT.mirrored());
    while state_of(&sim, id) == Some(EnemyState::JumpOver) {
        sim.step(TickInput::default());
    }
    let x = obj(&sim, id).x;
    assert!(x <= LIMITS.1 - 20 && x > 3200, "lands at {x}");
}

#[test]
fn the_leap_over_claw_needs_him_standing() {
    let (mut sim, id) = in_fight(-200);
    sim.with_world(|w| {
        enemy::go_home(w, id);
        w.objects.get_mut(id).unwrap().physics_type = ptype::FLY;
        human::enter_jump_over(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

#[test]
fn his_body_hurts_claw_during_the_leap() {
    let (mut sim, id) = in_fight(-200);
    let health = sim.stats().health;
    sim.with_world(|w| human::enter_jump_over(w, id));
    sim.step(TickInput::default());
    // Claw stands where Raux's body box is.
    let (x, y) = (obj(&sim, id).x, obj(&sim, id).y);
    sim.teleport_player(x - 2, y + 20);
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::JumpOver));
    assert!(safe(&sim, id), "safe himself");
    assert_eq!(sim.stats().health, health - 5, "hurt by the leap");
}

// ----------------------------------------------------------------------
// Dodges

/// Raux in Idle with Claw `dx` px away, facing him or not, attacking;
/// runs one logic call of Raux on a pulse with the stream at `stream`.
fn dodge_call(dx: i32, facing: bool, stream: u32, busy: bool) -> (Sim, ObjectId, u32) {
    let (mut sim, id) = in_fight(dx);
    sim.with_world(|w| {
        raux::go_idle(w, id);
        enemy::data_mut(w, id).unwrap().busy = busy;
    });
    sim.set_rng_state(stream);
    sim.with_world(|w| {
        w.pulse = true;
        w.claw.attack.kind = attack_kind::SWIPE;
        w.claw.facing_right = if dx > 0 { !facing } else { facing };
        logic::tick(w, id);
    });
    let draws = draws_between(stream, sim.rng_state());
    (sim, id, draws)
}

#[test]
fn he_dodges_a_facing_attack_by_the_roll() {
    // Roll below 100: the hop back; 100 to 199: the hop up; 200 to 499:
    // the leap over Claw (his right limit, Claw being to his right).
    for (band, expect) in [
        (0..100, EnemyState::JumpBack),
        (100..200, EnemyState::Jump),
        (200..500, EnemyState::JumpOver),
    ] {
        let stream = stream_with_roll(band);
        let (sim, id, draws) = dodge_call(80, true, stream, false);
        // Roll, sound pick, the smarts draw.
        assert_eq!(draws, 3, "{expect:?}");
        assert_eq!(state_of(&sim, id), Some(expect));
        assert!(safe(&sim, id));
    }
    // A higher roll: no dodge; Idle sends him Home.
    let stream = stream_with_roll(500..1001);
    let (sim, id, draws) = dodge_call(80, true, stream, false);
    assert_eq!(draws, 3);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

#[test]
fn he_dodges_only_a_facing_attack_from_close_and_only_when_free() {
    let stream = stream_with_roll(0..100);
    // Claw facing away: no smarts draw, no dodge.
    let (sim, id, draws) = dodge_call(80, false, stream, false);
    assert_eq!(draws, 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    // Too far (300 px, mode 1 or 2): no smarts draw.
    let (sim, id, draws) = dodge_call(260, true, stream, false);
    assert_eq!(draws, 2);
    assert_ne!(state_of(&sim, id), Some(EnemyState::JumpBack));
    // Busy: the draw is made, no dodge.
    let (sim, id, draws) = dodge_call(80, true, stream, true);
    assert_eq!(draws, 3);
    assert_ne!(state_of(&sim, id), Some(EnemyState::JumpBack));
    // Claw to his left, facing right, at him: the hop back goes right.
    let (sim, id, _) = dodge_call(-80, true, stream, false);
    assert_eq!(state_of(&sim, id), Some(EnemyState::JumpBack));
    assert!(data(&sim, id).vx > 0.0);
}

#[test]
fn a_scheduled_dodge_is_a_shared_human_call() {
    let stream = stream_with_roll(0..100);
    let (mut sim, id) = in_fight(80);
    sim.with_world(|w| raux::go_idle(w, id));
    sim.set_rng_state(stream);
    sim.with_world(|w| {
        w.pulse = true;
        w.claw.attack.kind = attack_kind::SWIPE;
        w.claw.facing_right = false;
        enemy::pre_tick(w, id, enemy::class(GameLogic::Raux).unwrap());
        raux::pre_tick(w, id);
    });
    assert_eq!(
        data(&sim, id).deferred,
        Some(Deferred::Human(HumanCall::JumpBack))
    );
}

// ----------------------------------------------------------------------
// Hits

#[test]
fn a_sword_blow_with_a_low_roll_is_blocked_without_damage() {
    let (mut sim, id) = in_fight(80);
    // The block sound: the first pick of a tick with a known pick.
    sim.with_world(|w| {
        enemy::data_mut(w, id).unwrap().human.sounds.block_low = "LEVEL_RAUX_BLOCK2"
    });
    let blow = attacker(&mut sim, types::PLAYER, 10, 0);
    assert_eq!(hit(&mut sim, id, blow, 99, false), 0, "no draw");
    assert_eq!(state_of(&sim, id), Some(EnemyState::BlockLow));
    assert_eq!(obj(&sim, id).health, 100);
    assert!(safe(&sim, id) && data(&sim, id).busy);
    assert_eq!(data(&sim, id).vx, -200.0, "slides away from Claw");
    assert_eq!(data(&sim, id).cooldown.len, raux::HIT_COOLDOWN_MS);
    sim.step(TickInput::default());
    assert!(cues(&sim).contains(&"LEVEL_RAUX_BLOCK2".to_string()));
    // 600 ms later he is Home again, hits allowed.
    for _ in 0..40 {
        if state_of(&sim, id) != Some(EnemyState::BlockLow) {
            break;
        }
        sim.step(TickInput::default());
    }
    assert_ne!(state_of(&sim, id), Some(EnemyState::BlockLow));
    assert!(!safe(&sim, id));
}

#[test]
fn a_sword_blow_otherwise_hurts_then_he_strikes_back_when_the_flinch_ends() {
    let (mut sim, id) = in_fight(80);
    let blow = attacker(&mut sim, types::PLAYER, 10, 0);
    // A low flinch: the hit burst's three draws.
    assert_eq!(hit(&mut sim, id, blow, 100, false), 3);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 90);
    assert_eq!(data(&sim, id).vx, -200.0);
    // During the 350 ms cooldown a new blow is turned away.
    assert_eq!(hit(&mut sim, id, blow, 500, false), 0);
    assert_eq!(obj(&sim, id).health, 90);
    // The flinch lasts 322 ms; the tick it ends he strikes.
    let mut n = 0;
    while state_of(&sim, id) == Some(EnemyState::HitLow) {
        sim.step(TickInput::default());
        n += 1;
        assert!(n < 40);
    }
    // Four elements of 80 or 81 ms, five ticks each.
    assert_eq!(n, 21, "{n} ticks");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
    assert!(data(&sim, id).busy);
}

#[test]
fn a_blow_during_a_flinch_only_sends_him_home() {
    let (mut sim, id) = in_fight(80);
    let blow = attacker(&mut sim, types::PLAYER, 10, 0);
    hit(&mut sim, id, blow, 500, false);
    // Past the cooldown, still flinching.
    sim.with_world(|w| enemy::data_mut(w, id).unwrap().cooldown = enemy::Timer::default());
    assert_eq!(hit(&mut sim, id, blow, 500, false), 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    assert_eq!(obj(&sim, id).health, 90);
}

#[test]
fn the_high_flinch_draws_its_sound_before_the_hit_burst() {
    let (mut sim, id) = in_fight(80);
    let blow = attacker(&mut sim, types::PLAYER, 10, 0);
    let first = Rng::new(sim.rng_state()).below(4) as usize;
    assert_eq!(hit(&mut sim, id, blow, 100, true), 4);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitHigh));
    sim.step(TickInput::default());
    let heard = cues(&sim);
    assert!(heard.contains(&"GAME_SOLHITHI".to_string()));
    assert!(
        heard.contains(&enemy::HIT_SOUNDS[first].to_string()),
        "{heard:?}"
    );
}

#[test]
fn shots_magic_and_thrown_objects_are_destroyed_and_blocked() {
    for kind in [types::PISTOL_SHOT, 0x200_0000, types::THROWN] {
        let (mut sim, id) = in_fight(80);
        let a = attacker(&mut sim, kind, 25, 0);
        assert_eq!(hit(&mut sim, id, a, 900, false), 0);
        assert_eq!(state_of(&sim, id), Some(EnemyState::BlockLow), "{kind:#x}");
        assert_eq!(obj(&sim, id).health, 100);
        assert!(obj(&sim, a).flags & flags::DESTROY != 0, "{kind:#x}");
        assert!(data(&sim, id).cooldown.done(sim.tick() * 18), "no cooldown");
    }
    // Environment shots do nothing.
    let (mut sim, id) = in_fight(80);
    let state = state_of(&sim, id);
    let a = attacker(&mut sim, types::LEVEL_SHOT, 25, 0);
    assert_eq!(hit(&mut sim, id, a, 900, false), 0);
    assert_eq!(state_of(&sim, id), state);
    assert_eq!(obj(&sim, id).health, 100);
    assert!(obj(&sim, a).flags & flags::DESTROY == 0);
}

#[test]
fn he_cannot_be_lifted() {
    let (mut sim, id) = in_fight(40);
    let claw = sim.with_world(|w| w.claw_id.unwrap());
    let before = (obj(&sim, id).state, data(&sim, id));
    sim.with_world(|w| {
        logic::lifted(w, id, claw);
        logic::thrown(w, id, 300, -650);
    });
    assert_eq!((obj(&sim, id).state, data(&sim, id)), before);
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, id).special_flags & special::LIFTABLE, 0);
}

// ----------------------------------------------------------------------
// Health, the health bar, death

#[test]
fn health_and_damage_follow_the_difficulty_factors() {
    // Built with the factors of normal play (1.0): 100 and 5.
    let (sim, id) = in_fight(80);
    assert_eq!(
        obj(&sim, id).health,
        enemy::scaled(raux::HEALTH, enemy::HEALTH_FACTOR)
    );
    assert_eq!(
        obj(&sim, id).damage,
        enemy::scaled(raux::DAMAGE, enemy::DAMAGE_FACTOR)
    );
    // The strong-enemy factors (5.0): 500 health, 25 damage, smarts 500.
    assert_eq!(enemy::scaled(raux::HEALTH, 5.0), 500);
    assert_eq!(enemy::scaled(raux::DAMAGE, 5.0), 25);
    assert_eq!(enemy::smarts(raux::SMARTS), 100);
}

#[test]
fn the_health_bar_shows_his_health_while_it_is_shown() {
    let (mut sim, id) = in_fight(80);
    sim.step(TickInput::default());
    assert_eq!(sim.boss().meter(), None, "he does not show it himself");
    sim.with_world(|w| {
        w.set_boss_meter(Some(BossMeter {
            value: 100,
            max: 100,
        }))
    });
    let blow = attacker(&mut sim, types::PLAYER, 10, 0);
    hit(&mut sim, id, blow, 500, false);
    sim.step(TickInput::default());
    assert_eq!(
        sim.boss().meter(),
        Some(BossMeter {
            value: 90,
            max: 100
        })
    );
    assert_eq!(sim.snapshot().hud.boss_meter.map(|m| m.value), Some(90));
}

#[test]
fn ten_sword_blows_kill_him_and_his_defeat_is_reported_once() {
    let (mut sim, id) = in_fight(80);
    let blow = attacker(&mut sim, types::PLAYER, 10, 0);
    for k in 0..10 {
        sim.with_world(|w| enemy::data_mut(w, id).unwrap().cooldown = enemy::Timer::default());
        if state_of(&sim, id).is_some_and(|s| matches!(s, EnemyState::HitLow | EnemyState::HitHigh))
        {
            sim.with_world(|w| enemy::go_home(w, id));
        }
        hit(&mut sim, id, blow, 500, false);
        if k < 9 {
            assert_eq!(sim.boss().defeated(), None);
        }
    }
    assert_eq!(obj(&sim, id).health, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(sim.boss().defeated(), Some(id));
    let b = data(&sim, id).human.boss.unwrap();
    assert!(b.death_seen && b.defeat_reported);
    // His body flies off as any.
    let d = data(&sim, id);
    assert_eq!((d.vx, d.vy), (-150.0, -700.0));
    // Still the current boss until the body is removed.
    assert_eq!(sim.boss().current(), Some(id));
}

#[test]
fn his_death_is_not_reported_when_claw_is_dead() {
    let (mut sim, id) = in_fight(80);
    sim.with_world(|w| {
        w.claw.stats.health = 0;
        w.objects.get_mut(id).unwrap().health = 5;
    });
    let blow = attacker(&mut sim, types::PLAYER, 10, 0);
    hit(&mut sim, id, blow, 500, false);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(sim.boss().defeated(), None);
    assert!(data(&sim, id).human.boss.unwrap().death_seen);
    // Nothing reports it later.
    sim.with_world(|w| w.claw.stats.health = 100);
    idle(&mut sim, 3);
    assert_eq!(sim.boss().defeated(), None);
}

#[test]
fn a_death_on_spikes_is_reported_too() {
    let (mut sim, id) = in_fight(80);
    sim.with_world(|w| enemy::die(w, id, enemy::DeathKind::Spikes, None));
    assert_eq!(sim.boss().defeated(), None, "seen at his next call");
    sim.step(TickInput::default());
    assert_eq!(sim.boss().defeated(), Some(id));
}

#[test]
fn his_body_leaving_clears_the_current_boss() {
    let (mut sim, id) = in_fight(80);
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().health = 5;
    });
    let blow = attacker(&mut sim, types::PLAYER, 10, 0);
    hit(&mut sim, id, blow, 500, false);
    // Claw walks off far: the body leaves the screen and is removed.
    sim.teleport_player(9000, 899);
    for _ in 0..200 {
        sim.step(TickInput::default());
        if sim.objects().get(id).is_none() {
            break;
        }
    }
    assert!(sim.objects().get(id).is_none(), "removed");
    assert_eq!(sim.boss().current(), None);
}

#[test]
fn claws_respawn_puts_him_back_with_full_health_in_his_state() {
    let (mut sim, id) = in_fight(80);
    let blow = attacker(&mut sim, types::PLAYER, 10, 0);
    hit(&mut sim, id, blow, 500, false);
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.x = 3150;
    });
    let state = state_of(&sim, id);
    // Claw lost a life and respawns.
    sim.with_world(|w| {
        w.claw.stats.count_attempt();
        let c = w.claw_id.unwrap();
        w.objects.get_mut(c).unwrap().state = claw_state::RESPAWN;
        raux::tick(w, id);
    });
    // Put back where he was placed, then his tick goes on from there.
    let o = obj(&sim, id);
    assert!((o.x - 3000).abs() <= 4, "near where he was placed: {}", o.x);
    assert_eq!(o.y, 900);
    assert_eq!(o.health, 100);
    assert_eq!(state_of(&sim, id), state, "his state is kept");
    // Once per respawn.
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().health = 40;
        raux::tick(w, id);
    });
    assert_eq!(obj(&sim, id).health, 40);
    // The reset itself.
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().x = 3100;
        raux::reset(w, id);
    });
    assert_eq!(obj(&sim, id).health, 100);
    assert_eq!((obj(&sim, id).x, obj(&sim, id).y), (3000, 900));
}

// ----------------------------------------------------------------------
// The install

fn store() -> Option<claw_assets::AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

#[test]
fn install_level2_raux_binds_with_his_health_and_waits_for_the_fight() {
    let Some(store) = store() else { return };
    let data2 = LevelData::build(&store, LevelSource::Retail(2)).unwrap();
    let level = SimLevel::from_level(&data2, &store).unwrap();
    let records: Vec<&ObjectSpec> = level.objects.iter().filter(|o| o.logic == "Raux").collect();
    assert_eq!(records.len(), 1, "one Raux on the level");
    let (x, y) = (records[0].record[5], records[0].record[6]);
    let limits = (
        records[0].record[enemy::field::X_MIN],
        records[0].record[enemy::field::X_MAX],
    );
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    assert_eq!(
        sim.diagnostics().inert_logics.get("Raux"),
        None,
        "not inert"
    );
    let id = raux_id(&sim);
    // He is built on his first call once awake: bring Claw to his room.
    sim.teleport_player(x - 100, y);
    let mut n = 0;
    while enemy::data_of(obj(&sim, id)).is_none() {
        sim.step(TickInput::default());
        n += 1;
        assert!(n < 50, "built once Claw is near");
    }
    let o = obj(&sim, id);
    assert_eq!(o.health, 100);
    assert_eq!(o.damage, 5);
    assert!(o.flags & flags::ALWAYS_ACTIVE != 0);
    let d = data(&sim, id);
    assert_eq!(d.smarts, 100);
    assert_eq!(d.bounds, limits);
    assert_eq!(d.human.boss.unwrap().start, (x, y));
    assert_eq!(sim.boss().current(), Some(id));
    assert_eq!(o.physics_type, ptype::STAND, "on the floor of his room");
    let s = state_of(&sim, id).expect("an enemy state");
    assert_ne!(s, EnemyState::Dead);
    // His animations are loaded with the level.
    for key in [raux::IDLE_ANIM, raux::STRIKE_ANIM, raux::BLOCK_LOW_ANIM] {
        assert!(sim.level().animations.contains_key(key), "{key}");
    }
}
