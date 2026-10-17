//! Raux's fight around its introduction: what a jump-in entry that skips
//! the introduction leaves behind (Raux waiting in his start-up state for
//! as long as Claw stays within his reach, the "AI breakdown" speedruns
//! use), the health bar the introduction makes, and the gem that ends the
//! level after either entry.
//!
//! The synthetic tests run Raux on a flat floor with the fight flag left
//! at 0, as after a skipped introduction. The tests named `install_*` play
//! the jump-in through the second level's door on the user's own install
//! (`CLAW_INSTALL`) and skip themselves when it is unset; nothing from an
//! install is stored in the repository.

mod common;

use std::path::PathBuf;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::output::BossMeter;
use claw_frame::{AudioCue, Buttons, GameEvent, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::input::ScriptTuning;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::boss_stage::{self, stage_state};
use claw_sim::logic::enemy::{self, EnemyState, raux};
use claw_sim::logic::world_boss::{AMULET_SOUND, fight};
use claw_sim::logic::{self, pickup};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::player::special::BOSS_END;
use claw_sim::player::state;
use claw_sim::tiles::TileMap;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

/// Where Raux is placed on the synthetic floor, and where he stands once
/// built (feet on 959); his patrol limits.
const RAUX_PLACED: (i32, i32) = (3000, 900);
const RAUX: (i32, i32) = (3000, 907);
const LIMITS: (i32, i32) = (2700, 3300);
/// Where Claw stands within Raux's reach: 132 px left of him, where he
/// stood in the original after the jump-in at 60 frames per second.
const NEAR: (i32, i32) = (RAUX.0 - 132, 899);
/// The top of a one-tile step right of Raux: Claw standing there is 72 px
/// above him, outside every range box, with Raux still on screen.
const STEP_COLUMN: usize = 48;
const ON_STEP: (i32, i32) = (3104, 835);
/// The gem's target: 35 px left of Raux and 155 px up, as on the second
/// level.
const GEM_TARGET: (i32, i32) = (RAUX.0 - 35, RAUX.1 - 155);
/// A stager on the floor left of Claw (47 px below his feet line, as on
/// the second level).
const STAGER: (i32, i32) = (2400, 899 + 47);

/// The flat floor with the step, Raux, and optionally a stager; every
/// object awake, Claw standing at [`NEAR`].
fn arena(with_stager: bool) -> Sim {
    let mut rows = vec![".".repeat(200); 15];
    rows[14].replace_range(STEP_COLUMN..STEP_COLUMN + 1, "#");
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), NEAR);
    let mut o = ObjectSpec::at("Raux", RAUX_PLACED.0, RAUX_PLACED.1, 0);
    o.image_set = "LEVEL_RAUX".into();
    o.record[enemy::field::X_MIN] = LIMITS.0;
    o.record[enemy::field::X_MAX] = LIMITS.1;
    o.record[enemy::field::SPEED_X] = GEM_TARGET.0;
    o.record[enemy::field::SPEED_X + 1] = GEM_TARGET.1;
    level.objects.push(o);
    if with_stager {
        level
            .objects
            .push(ObjectSpec::at(boss_stage::STAGER, STAGER.0, STAGER.1, 8000));
    }
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed: 3,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    idle(&mut sim, 30);
    sim
}

fn raux_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Raux))
        .map(|(id, _)| id)
        .expect("Raux exists")
}

fn raux_state(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    sim.objects()
        .get(id)
        .and_then(|o| EnemyState::from_id(o.state))
}

fn find(sim: &Sim, logic: &str) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
}

/// A blow of Claw's sword on Raux, as a new attack, with his roll set so
/// that he does not block it.
fn sword_blow(sim: &mut Sim, id: ObjectId) {
    let mut a = Object::new(0, 0, 0, 0, "TestBlow".into(), LogicKind::Inert);
    a.type_flags = types::PLAYER;
    a.damage = 10;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    let blow = sim.spawn(a);
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        let d = enemy::data_mut(w, id).unwrap();
        d.roll = 500;
        d.cooldown = enemy::Timer::default();
        let at = w.objects.get(id).map(|o| (o.x, o.y)).unwrap();
        if let Some(o) = w.objects.get_mut(blow) {
            o.x = at.0;
            o.y = at.1;
            o.attack_rect = Rect::new(-5, -5, 5, 5);
        }
        logic::on_hit(w, id, blow);
    });
}

// ----------------------------------------------------------------------
// What a skipped introduction leaves behind

#[test]
fn with_the_introduction_skipped_raux_waits_while_claw_stays_near() {
    let mut sim = arena(false);
    let id = raux_id(&sim);
    assert_eq!(sim.boss().current(), Some(id));
    let health = sim.claw().stats.health;
    // 34 s with Claw standing 132 px from him, as in the original at 60
    // frames per second: he never leaves his start-up state, never moves
    // and never strikes.
    for k in 0..ticks(34_000) {
        sim.step(TickInput::default());
        assert_eq!(raux_state(&sim, id), Some(EnemyState::Startup), "tick {k}");
        assert_eq!(
            enemy::data_of(sim.objects().get(id).unwrap()).unwrap().mode,
            3
        );
    }
    let o = sim.objects().get(id).unwrap();
    assert_eq!((o.x, o.y), RAUX);
    assert_eq!(o.health, 100);
    assert_eq!(sim.claw().stats.health, health);
    assert_eq!(sim.boss().fight(), fight::NONE);
    assert_eq!(sim.hud().boss_meter, None);
}

/// Claw stands out of every range box (on the step) for `out` ticks, then
/// back within reach; returns the tick (from the first one out) on which
/// Raux left his start-up state within 34 s, if he did.
fn out_of_range_for(out: usize) -> Option<usize> {
    let mut sim = arena(false);
    let id = raux_id(&sim);
    assert_eq!(raux_state(&sim, id), Some(EnemyState::Startup));
    sim.teleport_player(ON_STEP.0, ON_STEP.1);
    for k in 1..=ticks(34_000) {
        if k == out + 1 {
            sim.teleport_player(NEAR.0, NEAR.1);
        }
        sim.step(TickInput::default());
        if k <= out {
            assert_eq!(sim.player().unwrap().y, ON_STEP.1, "standing on the step");
        }
        if raux_state(&sim, id) != Some(EnemyState::Startup) {
            return Some(k);
        }
    }
    None
}

#[test]
fn out_of_his_reach_for_467_ms_raux_keeps_waiting() {
    // The original at 60 frames per second: Claw's jump kept him out of
    // every box for 467 ms, and Raux stayed in his start-up state.
    assert_eq!(out_of_range_for(26), None);
    // Up to the last tick before the 500 ms have passed.
    assert_eq!(out_of_range_for(28), None);
}

#[test]
fn out_of_his_reach_for_500_ms_raux_goes_idle_and_fights() {
    // The original at 18 ms steps: out from step 43, idle on step 71, 504
    // ms later.
    assert_eq!(out_of_range_for(29), Some(29));
    let mut sim = arena(false);
    let id = raux_id(&sim);
    sim.teleport_player(ON_STEP.0, ON_STEP.1);
    idle(&mut sim, 29);
    assert_eq!(raux_state(&sim, id), Some(EnemyState::Idle));
    let d = enemy::data_of(sim.objects().get(id).unwrap()).unwrap();
    assert_eq!(d.mode, 5);
    assert_eq!(d.pace_wait.len, raux::IDLE_WAIT_MS);
    // Back within his reach, he comes at Claw although the flag is 0.
    sim.teleport_player(NEAR.0, NEAR.1);
    let mut left_idle = false;
    for _ in 0..ticks(2000) {
        sim.step(TickInput::default());
        left_idle |= !matches!(
            raux_state(&sim, id),
            Some(EnemyState::Idle | EnemyState::Startup)
        );
    }
    assert!(left_idle, "he fights");
    assert_eq!(sim.boss().fight(), fight::NONE);
}

#[test]
fn a_sword_blow_wakes_the_waiting_raux() {
    let mut sim = arena(false);
    let id = raux_id(&sim);
    sword_blow(&mut sim, id);
    assert!(matches!(
        raux_state(&sim, id),
        Some(EnemyState::HitLow | EnemyState::HitHigh)
    ));
    assert_eq!(sim.objects().get(id).unwrap().health, 90);
    assert_eq!(sim.boss().fight(), fight::NONE);
}

// ----------------------------------------------------------------------
// The health bar

#[test]
fn the_health_bar_is_the_introductions_own_with_the_health_it_copied() {
    let mut sim = arena(true);
    let id = raux_id(&sim);
    // Raux holds other health than his full health when the introduction
    // starts: the bar takes that as full.
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 60);
    sim.teleport_player(STAGER.0, NEAR.1);
    sim.step(TickInput::default());
    assert!(
        sim.last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::Checkpoint { .. })),
        "the trigger fired"
    );
    sim.step(TickInput::default());
    assert_eq!(sim.boss().fight(), fight::STAGING);
    let bar = |sim: &Sim| sim.hud().boss_meter;
    assert_eq!(bar(&sim), Some(BossMeter { value: 60, max: 60 }));
    // A blow: the bar falls against the health it copied, not against
    // Raux's full health.
    sword_blow(&mut sim, id);
    sim.step(TickInput::default());
    assert_eq!(sim.objects().get(id).unwrap().health, 50);
    assert_eq!(bar(&sim), Some(BossMeter { value: 50, max: 60 }));
    // Raux's reset when Claw respawns gives him his full health back; the
    // bar keeps its full mark.
    sim.with_world(|w| raux::reset(w, id));
    sim.step(TickInput::default());
    assert_eq!(
        bar(&sim),
        Some(BossMeter {
            value: 100,
            max: 60
        })
    );
    // Without the introduction there is no bar at all.
    let mut skipped = arena(true);
    let id = raux_id(&skipped);
    sword_blow(&mut skipped, id);
    idle(&mut skipped, 5);
    assert_eq!(bar(&skipped), None);
}

// ----------------------------------------------------------------------
// The gem

/// The gem's flight measured in the original on the second level: ticks
/// after the boss's defeat and the gem's offset from where it appeared.
const GEM_FLIGHT: [(u64, (i32, i32)); 5] = [
    (1, (-1, -4)),
    (2, (-1, -7)),
    (11, (-4, -33)),
    (51, (-19, -120)),
    (100, (-36, -155)),
];

#[test]
fn after_a_skipped_introduction_the_gem_still_ends_the_level() {
    let mut sim = arena(false);
    let id = raux_id(&sim);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 10);
    sword_blow(&mut sim, id);
    // His defeat holds Claw at once and releases the gem out of him.
    assert_eq!(raux_state(&sim, id), Some(EnemyState::Dead));
    assert_eq!(sim.boss().defeated(), Some(id));
    assert_eq!(sim.player().unwrap().state, BOSS_END);
    let gem = find(&sim, pickup::END_GEM).expect("the gem");
    let g = sim.objects().get(gem).unwrap();
    assert_eq!((g.x, g.y), RAUX);
    assert!(
        g.flags & flags::NO_COLLIDE != 0,
        "it touches nothing in flight"
    );
    assert_eq!(g.image_set, pickup::map_piece_set(sim.level().number));
    sim.step(TickInput::default());
    assert!(
        sim.last_audio()
            .iter()
            .any(|c| matches!(c, AudioCue::Play(s) if s.key == AMULET_SOUND))
    );
    // It rises on the original's arc with the camera on it, and lands on
    // the 101st tick.
    let held = sim.player().unwrap().clone();
    let mut k = 1u64;
    loop {
        let o = sim.objects().get(gem).unwrap();
        if &*o.logic == pickup::END_OF_LEVEL_LOGIC {
            break;
        }
        assert!(k < 200);
        if let Some(&(_, d)) = GEM_FLIGHT.iter().find(|(t, _)| *t == k) {
            assert_eq!((o.x - RAUX.0, o.y - RAUX.1), d, "tick {k}");
        }
        let at = (o.x, o.y);
        sim.step(TickInput::default());
        k += 1;
        if &*sim.objects().get(gem).unwrap().logic == pickup::END_GEM {
            assert_eq!(sim.camera_override(), Some(at), "tick {k}");
            let c = sim.player().unwrap();
            assert_eq!((c.state, c.x, c.y), (BOSS_END, held.x, held.y));
        }
    }
    assert_eq!(k, 101);
    // Landed: Claw is let go, the camera given back, and the gem is the
    // level's end where it stopped.
    let o = sim.objects().get(gem).unwrap();
    assert_eq!((o.x, o.y), (GEM_TARGET.0 - 1, GEM_TARGET.1));
    assert_eq!(o.kind, LogicKind::Game(GameLogic::Pickup));
    assert!(o.flags & flags::NO_COLLIDE == 0);
    assert_eq!(sim.player().unwrap().state, state::STAND);
    assert_eq!(sim.camera_override(), None);
    assert_eq!(sim.boss().fight(), fight::NONE, "the flag never moved");
    // Claw reaching it ends the level.
    let (x, y) = (o.x, o.y);
    sim.teleport_player(x, y + 40);
    let mut done = false;
    for _ in 0..5 {
        sim.step(TickInput::default());
        done |= sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }));
    }
    assert!(done, "the level ends");
}

#[test]
fn a_direction_held_through_the_gems_flight_walks_claw_once_he_is_let_go() {
    let mut sim = arena(false);
    let id = raux_id(&sim);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 10);
    sword_blow(&mut sim, id);
    let gem = find(&sim, pickup::END_GEM).expect("the gem");
    let right = hold(Buttons::RIGHT.0 | Buttons::PISTOL.0);
    sim.step(right);
    // A screen over play during the flight latches every key down, and
    // Right stays latched while it is held.
    sim.latch_keys();
    let held = sim.player().unwrap().clone();
    while &*sim.objects().get(gem).unwrap().logic == pickup::END_GEM {
        sim.step(right);
        assert!(sim.latched().0 & Buttons::RIGHT.0 != 0);
        assert_eq!(sim.claw_input().held, Buttons::NONE);
    }
    // The gem's landing lets Claw go; the next tick takes Left and Right
    // out of the latch and he walks right on the key still held.
    let c = sim.player().unwrap();
    assert_eq!((c.state, c.x), (state::STAND, held.x));
    sim.step(right);
    assert_eq!(sim.latched().0 & (Buttons::LEFT.0 | Buttons::RIGHT.0), 0);
    assert_eq!(sim.claw_input().held, Buttons::RIGHT);
    let c = sim.player().unwrap();
    assert_eq!(c.state, state::MOVE_RIGHT);
    assert!(c.x > held.x, "{} after {}", c.x, held.x);
    // Only the directions: another key held stays latched.
    assert_eq!(sim.latched().0 & Buttons::PISTOL.0, Buttons::PISTOL.0);
    assert!(!sim.claw().special.unlatch_directions);
}

#[test]
fn keys_latched_after_claw_is_let_go_stay_latched() {
    let mut sim = arena(false);
    let id = raux_id(&sim);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 10);
    sword_blow(&mut sim, id);
    let gem = find(&sim, pickup::END_GEM).expect("the gem");
    let right = hold(Buttons::RIGHT.0);
    sim.step(right);
    while &*sim.objects().get(gem).unwrap().logic == pickup::END_GEM {
        sim.step(right);
    }
    // The gem's landing lets Claw go, so Left and Right would come off the
    // latch at the next tick; a screen over play between the ticks latches
    // every key down again, and that latch wins.
    assert!(sim.claw().special.unlatch_directions);
    let held = sim.player().unwrap().clone();
    sim.latch_keys();
    assert!(!sim.claw().special.unlatch_directions);
    sim.step(right);
    assert_eq!(sim.latched().0 & Buttons::RIGHT.0, Buttons::RIGHT.0);
    assert_eq!(sim.claw_input().held, Buttons::NONE);
    let c = sim.player().unwrap();
    assert_eq!((c.state, c.x), (state::STAND, held.x));
    // Released and pressed again, Right walks him.
    sim.step(TickInput::default());
    sim.step(press(Buttons::RIGHT.0));
    assert_eq!(sim.claw_input().held, Buttons::RIGHT);
}

#[test]
fn a_gem_with_no_target_lands_at_once() {
    let mut sim = arena(false);
    sim.with_world(|w| {
        assert!(w.freeze_claw_boss_end());
        let id = pickup::spawn_end_gem(w, (2000, 700, 1), (0, 0));
        let o = w.objects.get(id).unwrap();
        assert_eq!(&*o.logic, pickup::END_OF_LEVEL_LOGIC);
        assert_eq!((o.x, o.y), (2000, 700));
    });
    assert_eq!(sim.player().unwrap().state, state::STAND);
    assert_eq!(sim.camera_override(), None);
}

#[test]
fn the_gem_takes_the_slowest_arc_to_its_target() {
    // The second level's flight: 35 px left and 155 px up.
    let (vx, vy) = pickup::end_gem_launch((23970.0, 3955.0), (23935, 3800));
    assert!((vx + 19.633).abs() < 1e-3, "{vx}");
    assert!((vy + 176.082).abs() < 1e-3, "{vy}");
    // Its speed squared is g |d| + g * rise.
    let d = 35f64.hypot(155.0);
    let g = pickup::END_GEM_GRAVITY;
    assert!((vx * vx + vy * vy - (g * d + g * 155.0)).abs() < 1e-6);
    assert_eq!(pickup::end_gem_launch((10.0, 10.0), (10, 10)), (0.0, 0.0));
}

// ----------------------------------------------------------------------
// The jump-in entry on the second level

/// Where the boss room's stretches start: next to the door.
const START: (i32, i32) = (20150, 2091);
/// The stager's record on the second level.
const STAGER_RECORD: (i32, i32) = (23642, 3994);

fn level2() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(2)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

fn settle_at(sim: &mut Sim, x: i32, y: i32) {
    sim.teleport_player(x, y);
    let mut still = 0;
    let mut last = None;
    for _ in 0..222 {
        sim.step(TickInput::default());
        let now = sim
            .player()
            .map(|o| (o.x, o.y, o.state, sim.camera().current));
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still >= 10 {
            break;
        }
    }
}

/// What the jump-in left, read tick by tick from the stretch's time zero.
struct JumpIn {
    sim: Sim,
    raux: ObjectId,
    /// The first tick in the boss room.
    arena: u64,
    /// The first tick Raux was out of his start-up state, with the state
    /// and his range mode then.
    woke: Option<(u64, EnemyState, u8)>,
    /// The tick Claw landed after the jump, and where.
    landed: Option<(u64, i32, i32)>,
}

/// Plays the reference routes' jump-in with `tuning` for `ticks` ticks: 15
/// ticks still, Right held through the door, then on the first tick in the
/// room Jump pressed with Right, both held for 12 ticks, Right alone for
/// 28, then nothing. Checks on every tick that the introduction never
/// starts: no trigger, no flag, no freeze, no health bar.
fn jump_in(level: SimLevel, tuning: ScriptTuning, ticks: u64) -> JumpIn {
    let cfg = SimConfig {
        tuning: tuning.tuning(),
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level, cfg);
    settle_at(&mut sim, START.0, START.1);
    let mut arena = None;
    let mut raux = None;
    let mut woke = None;
    let mut landed = None;
    for t in 0..ticks {
        let (held, pressed) = match arena {
            None if t < 15 => (0, 0),
            None => (RIGHT, if t == 15 { RIGHT } else { 0 }),
            Some(a) if t == a => (RIGHT | JUMP, JUMP),
            Some(a) if t < a + 12 => (RIGHT | JUMP, 0),
            Some(a) if t < a + 40 => (RIGHT, 0),
            Some(_) => (0, 0),
        };
        sim.step(TickInput {
            held: Buttons(held),
            pressed: Buttons(pressed),
        });
        let c = sim.player().unwrap().clone();
        // The door moves Claw between ticks: a warp pending after tick t
        // puts him in the room for tick t + 1.
        if arena.is_none() && sim.claw().special.warp_to.is_some() {
            arena = Some(t + 1);
        }
        if raux.is_none() && arena.is_some_and(|a| t >= a) {
            raux = sim.boss().current();
        }
        if let (Some(a), Some(r)) = (arena, raux) {
            if t == a {
                assert_eq!(c.state, state::JUMP, "the jump on the first tick");
            }
            let o = sim.objects().get(r).unwrap();
            let s = EnemyState::from_id(o.state);
            if woke.is_none() && t >= a && s != Some(EnemyState::Startup) {
                let mode = enemy::data_of(o).map_or(0, |d| d.mode);
                woke = Some((t, s.unwrap(), mode));
            }
            if landed.is_none() && t > a && matches!(c.state, state::MOVE_RIGHT | state::STAND) {
                landed = Some((t, c.x, c.y));
            }
        }
        assert!(
            !sim.last_events()
                .iter()
                .any(|e| matches!(e, GameEvent::Checkpoint { .. })),
            "tick {t}: the trigger fired"
        );
        assert_eq!(sim.boss().fight(), fight::NONE, "tick {t}");
        assert_eq!(sim.hud().boss_meter, None, "tick {t}");
        assert_ne!(c.state, state::FREEZE, "tick {t}");
    }
    let stager = find(&sim, boss_stage::STAGER).expect("the stager");
    let o = sim.objects().get(stager).unwrap();
    assert_eq!(o.state, stage_state::ARMED, "the trigger stays armed");
    assert_eq!((o.x, o.y), STAGER_RECORD);
    JumpIn {
        sim,
        raux: raux.expect("Raux is built in his room"),
        arena: arena.expect("through the door"),
        woke,
        landed,
    }
}

#[test]
fn install_level2_jump_in_at_the_original_speeds_wakes_raux_after_500_ms() {
    let Some(level) = level2() else { return };
    // The original at 18 ms steps: the room on step 41, Claw out of Raux's
    // reach from step 43, Raux idle on step 71 (504 ms later) and fighting
    // after it, Claw landing at x 23790 on step 76.
    let j = jump_in(level, ScriptTuning::Original, 400);
    println!(
        "jump-in at the original speeds: room {}, Raux woke {:?}, Claw landed {:?}",
        j.arena, j.woke, j.landed
    );
    assert_eq!(j.arena, 41);
    assert_eq!(j.woke, Some((71, EnemyState::Idle, 5)));
    assert_eq!(j.landed, Some((76, 23790, 3947)));
    let o = j.sim.objects().get(j.raux).unwrap();
    assert_eq!(o.health, 100);
}

#[test]
fn install_level2_jump_in_at_60_fps_leaves_raux_waiting() {
    let Some(level) = level2() else { return };
    // The original at 60 frames per second: Claw's jump kept him out of
    // Raux's reach for only 467 ms; Raux then stood in his start-up state
    // for the remaining 34 s of the run, with Claw standing at x 23838,
    // never attacked. The default tuning plays that feel.
    let j = jump_in(level, ScriptTuning::Default, ticks(36_200) as u64);
    println!(
        "jump-in at 60 fps: room {}, Raux woke {:?}, Claw landed {:?}, stands at {:?}",
        j.arena,
        j.woke,
        j.landed,
        j.sim.player().map(|c| (c.x, c.y))
    );
    assert_eq!(j.arena, 37);
    assert_eq!(j.woke, None, "Raux never leaves his start-up state");
    assert_eq!(raux_state(&j.sim, j.raux), Some(EnemyState::Startup));
    let c = j.sim.player().unwrap();
    assert_eq!(c.state, state::STAND);
    assert!((c.x - 23838).abs() <= 6, "{}", c.x);
    assert_eq!(c.y, 3947);
    assert_eq!(j.sim.claw().stats.health, 100, "never attacked");
    let o = j.sim.objects().get(j.raux).unwrap();
    assert_eq!((o.x, o.y, o.health), (23970, 3955, 100));
}
