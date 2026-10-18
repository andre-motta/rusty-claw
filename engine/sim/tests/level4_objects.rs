//! The fourth level's springboards and stepping stones on synthetic
//! levels: the fern's throw (its delay, its height, the keys that change
//! nothing and the walk off that cancels it), its rustle, the enemy's
//! launch off a fern, and the stones' sink and rise on the clock or under
//! Claw, all silent.

mod common;

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::claw_frame::AudioCue;
use claw_sim::input::ScriptTuning;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::enemy::{self, EnemyState};
use claw_sim::logic::platform::{self, NOT_PLATFORM, PLATFORM};
use claw_sim::logic::stepping_stone::{self, RISE_SOUND, SINK_SOUND, state as stone};
use claw_sim::logic::{self, GameLogic, springboard};
use claw_sim::object::{ObjectId, flags};
use claw_sim::player::{self, state};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

const FERN_SET: &str = "LEVEL_SPRINGYFERN";
const STONE_SET: &str = "LEVEL_STEPPINGSTONE";
const RUSTLE: &str = "LEVEL_SPRINGYFERN_RUSTLE";
const SPRING: &str = "LEVEL_SPRINGYFERN_SPRING";
const Y_MAX: usize = 54;
const SPEED_X: usize = 55;
const SPEED_Y: usize = 56;

fn element(image_op: i32, image_index: i32, next_op: i32, duration: i32) -> AnimFrame {
    AnimFrame {
        image_op,
        image_index,
        next_op,
        duration,
        ..AnimFrame::default()
    }
}

fn image(i: i32, ms: i32) -> AnimFrame {
    element(3, i, 0, ms)
}

fn add_anim(level: &mut SimLevel, key: &str, frames: Vec<AnimFrame>, sounds: &[(usize, &str)]) {
    let mut frame_sounds = vec![Vec::new(); frames.len()];
    for &(i, s) in sounds {
        frame_sounds[i] = vec![Some(s.to_string())];
    }
    level.animations.insert(
        key.to_string(),
        Arc::new(Animation {
            key: key.to_string(),
            frames,
            frame_sounds,
            ..Animation::default()
        }),
    );
}

/// The flat test level as the fourth level, with the fern's and the
/// stones' animations and image sets, and `objects`.
fn level4_with(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    level.number = 4;
    // The fern at rest on image 4; a rustle every 4572 ms.
    add_anim(
        &mut level,
        "LEVEL_SPRINGYFERN_IDLE",
        vec![
            image(4, 2000),
            image(3, 100),
            image(2, 100),
            image(1, 100),
            image(2, 100),
            image(3, 100),
            image(4, 2000),
        ],
        &[(1, RUSTLE)],
    );
    // The spring: the sound on image 5, the launch event on image 6.
    let mut spring: Vec<AnimFrame> = [4, 5, 6, 7, 8, 9, 10, 11]
        .iter()
        .map(|&i| image(i, 50))
        .collect();
    spring[2].event = springboard::LAUNCH_EVENT;
    spring.push(image(5, 75));
    spring.push(element(3, 4, 9, 75));
    add_anim(
        &mut level,
        "LEVEL_SPRINGYFERN_SPRING",
        spring,
        &[(1, SPRING)],
    );
    // Forward: next image every 50 ms until the last, then finish.
    add_anim(
        &mut level,
        platform::FORWARD_ANIM,
        vec![element(1, 0, 3, 50), element(0, 0, 9, 0)],
        &[],
    );
    // Backward: the last image, previous every 50 ms until the first,
    // then finish.
    add_anim(
        &mut level,
        platform::BACKWARD_ANIM,
        vec![
            element(5, 0, 0, 0),
            element(2, 0, 2, 50),
            element(0, 0, 9, 0),
        ],
        &[],
    );
    level
        .images
        .insert(FERN_SET.into(), ImageFrames::range(1, 11));
    level
        .images
        .insert(STONE_SET.into(), ImageFrames::range(1, 6));
    level.objects = objects;
    level
}

fn fern(x: i32, y: i32, height: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at("SpringBoard", x, y, 2000);
    s.image_set = FERN_SET.into();
    s.frame = 1;
    s.record[Y_MAX] = height;
    s
}

fn stone(name: &str, x: i32, y: i32, z: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at(name, x, y, z);
    s.image_set = STONE_SET.into();
    s.frame = 1;
    s
}

fn original() -> SimConfig {
    SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        ..SimConfig::default()
    }
}

fn find(sim: &Sim, logic: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic}"))
}

fn obj_state(sim: &Sim, id: ObjectId) -> i32 {
    sim.objects().get(id).unwrap().state
}

fn solid(sim: &Sim, id: ObjectId) -> bool {
    sim.objects().get(id).unwrap().type_flags == PLATFORM
}

fn cues(sim: &Sim, key: &str) -> usize {
    sim.last_audio()
        .iter()
        .filter(|c| matches!(c, AudioCue::Play(s) if s.key == key))
        .count()
}

fn claw(sim: &Sim) -> (i32, i32, i32) {
    let o = sim.player().unwrap();
    (o.x, o.y, o.state)
}

fn riding(sim: &Sim, id: ObjectId) -> bool {
    let c = sim.player().unwrap();
    c.flags & flags::ON_ELEVATOR != 0 && c.object_below == Some(id)
}

/// Drops Claw onto the platform `id` from just above it; returns once he
/// rides it, in the landing tick.
fn land_on(sim: &mut Sim, id: ObjectId, input: TickInput) {
    let (x, y, top) = {
        let o = sim.objects().get(id).unwrap();
        (o.x, o.y, o.hit_rect.top)
    };
    let bottom = sim.player().unwrap().move_rect.bottom;
    sim.teleport_player(x, y + top - bottom - 10);
    for _ in 0..200 {
        sim.step(input);
        if riding(sim, id) {
            return;
        }
    }
    panic!("Claw never landed on the platform");
}

// ---------------------------------------------------------------- ferns

/// One fern in the air above the floor, set up, with Claw standing on it
/// (the landing tick just run).
fn on_a_fern(height: i32, input: TickInput) -> (Sim, ObjectId) {
    let mut sim = Sim::new(level4_with(vec![fern(1300, 850, height)]), original());
    sim.unlatch_keys();
    idle(&mut sim, 2);
    let id = find(&sim, "SpringBoard");
    land_on(&mut sim, id, input);
    (sim, id)
}

/// Steps `sim` `n` ticks with `input` (pressed on the first only) and
/// records Claw, the spring sound and the fern's state each tick.
fn record(
    sim: &mut Sim,
    id: ObjectId,
    n: usize,
    input: TickInput,
) -> Vec<((i32, i32, i32), bool, i32)> {
    (0..n)
        .map(|k| {
            let i = if k == 0 {
                input
            } else {
                TickInput {
                    held: input.held,
                    pressed: claw_sim::Buttons::NONE,
                }
            };
            sim.step(i);
            (claw(sim), cues(sim, SPRING) > 0, obj_state(sim, id))
        })
        .collect()
}

#[test]
fn a_fern_is_a_platform_with_its_level_surface() {
    let (sim, id) = on_a_fern(0, TickInput::default());
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.hit_rect, springboard::surface(4));
    assert_eq!(o.type_flags, PLATFORM);
    assert!(o.flags & flags::NO_COLLIDE != 0);
    assert_eq!(
        o.special_flags & (springboard::special::ENEMY_ARMABLE | springboard::special::MARKER),
        springboard::special::ENEMY_ARMABLE | springboard::special::MARKER
    );
    assert_eq!(o.z, springboard::DEFAULT_Z);
    assert_eq!(springboard::data_of(o).unwrap().height, 450);
    assert!(logic::built(GameLogic::SpringBoard));
    assert_eq!(springboard::height_of(0), 450);
    assert_eq!(springboard::height_of(-5), 450);
    assert_eq!(springboard::height_of(650), 650);
}

#[test]
fn standing_on_a_fern_throws_claw_450_px_21_ticks_after_he_lands() {
    let (mut sim, id) = on_a_fern(0, TickInput::default());
    let landed = claw(&sim);
    assert_eq!(landed.2, state::STAND);
    // Index k is tick L + 1 + k.
    let rows = record(&mut sim, id, 80, TickInput::default());
    let at = |l: usize| &rows[l - 1];
    assert_eq!(at(1).2, springboard::state::CLAW_SPRING, "seen on L + 1");
    let sound: Vec<usize> = (1..=80).filter(|&l| at(l).1).collect();
    assert_eq!(sound, [18], "the spring sound on L + 18");
    assert_eq!(at(20).0.2, state::STAND);
    assert_eq!(at(21).0.2, state::JUMP, "thrown on L + 21");
    // The first step of the throw is 25 px; the apex is exactly 450 px
    // above where he stood, 28 ticks later.
    assert_eq!(at(21).0.1, landed.1 - 25);
    let apex = (21..=80).min_by_key(|&l| (at(l).0.1, l)).unwrap();
    assert_eq!((apex, at(apex).0.1), (21 + 28, landed.1 - 450));
    assert_eq!(at(apex + 1).0.2, state::FALL);
    // The fern is idle again on L + 49.
    let idle_again = (2..=80)
        .find(|&l| at(l).2 == springboard::state::IDLE)
        .unwrap();
    assert_eq!(idle_again, 49);
}

#[test]
fn the_record_sets_the_throws_height() {
    for h in [300, 350, 650] {
        let (mut sim, id) = on_a_fern(h, TickInput::default());
        let y = claw(&sim).1;
        let rows = record(&mut sim, id, 90, TickInput::default());
        let top = rows.iter().map(|r| r.0.1).min().unwrap();
        assert_eq!(top, y - h, "height {h}");
    }
}

#[test]
fn holding_jump_on_a_fern_changes_nothing() {
    let (mut plain, id) = on_a_fern(0, TickInput::default());
    let (mut held, _) = on_a_fern(0, hold(JUMP));
    assert_eq!(claw(&plain), claw(&held));
    let a = record(&mut plain, id, 120, TickInput::default());
    let b = record(&mut held, id, 120, hold(JUMP));
    let path = |r: &[((i32, i32, i32), bool, i32)]| r.iter().map(|x| x.0).collect::<Vec<_>>();
    assert_eq!(path(&a), path(&b));
}

#[test]
fn a_tapped_jump_before_the_throw_is_an_ordinary_jump() {
    let (mut sim, id) = on_a_fern(0, TickInput::default());
    let y = claw(&sim).1;
    let mut rows = record(&mut sim, id, 19, TickInput::default());
    // L + 20: Jump tapped, released on the next tick.
    rows.extend(record(&mut sim, id, 1, press(JUMP)));
    rows.extend(record(&mut sim, id, 60, TickInput::default()));
    assert_eq!(rows[19].0.2, state::JUMP);
    // Back on the fern before it is idle again: no throw then either.
    let top = rows[19..48].iter().map(|r| r.0.1).min().unwrap();
    assert_eq!(top, y - 45, "the base jump height, not the throw");
}

#[test]
fn walking_off_a_fern_before_the_throw_cancels_it() {
    let (mut sim, id) = on_a_fern(0, TickInput::default());
    let rows = record(&mut sim, id, 21, press(LEFT));
    let off = rows.iter().position(|r| r.0.2 == state::FALL);
    assert!(off.is_some_and(|k| k < 20), "off the fern before L + 21");
    assert!(rows.iter().all(|r| r.0.2 != state::JUMP), "never thrown");
    // The fern's cycle runs on, and the spring sound still plays.
    assert!(rows.iter().any(|r| r.1));
    assert_eq!(rows[20].2, springboard::state::CLAW_SPRING);
}

#[test]
fn walking_on_a_fern_does_not_cancel_the_throw() {
    let (mut sim, id) = on_a_fern(0, TickInput::default());
    let mut rows = Vec::new();
    for k in 0..21 {
        let dir = if (k / 4) % 2 == 0 { RIGHT } else { LEFT };
        let input = if k % 4 == 0 { press(dir) } else { hold(dir) };
        rows.extend(record(&mut sim, id, 1, input));
    }
    assert!(rows[..20].iter().all(|r| r.0.2 != state::JUMP));
    assert_eq!(rows[20].0.2, state::JUMP, "thrown from the walk on L + 21");
}

#[test]
fn ferns_in_phase_rustle_together() {
    let ferns = (0..4).map(|i| fern(1100 + 200 * i, 600, 0)).collect();
    let mut sim = Sim::new(level4_with(ferns), original());
    let mut rustles = Vec::new();
    for k in 0..600 {
        sim.step(TickInput::default());
        let n = cues(&sim, RUSTLE);
        if n > 0 {
            rustles.push((k, n));
        }
    }
    // Four voices at once, every 254 ticks.
    assert_eq!(rustles.len(), 2);
    assert!(rustles.iter().all(|&(_, n)| n == 4));
    assert_eq!(rustles[1].0 - rustles[0].0, 254);
}

#[test]
fn a_throw_drops_what_claw_carries_and_never_starts_from_a_death() {
    let (mut sim, _) = on_a_fern(0, TickInput::default());
    sim.with_world(|w| {
        w.claw.attacking = true;
        assert!(player::bounce(w, 100));
    });
    assert_eq!(claw(&sim).2, state::JUMP);
    assert!(!sim.claw().attacking);
    assert_eq!(sim.claw().jump_press_time, 0.0);
    let y = claw(&sim).1;
    assert_eq!(sim.claw().jump_peak_y, y - 100);
    let (mut sim, _) = on_a_fern(0, TickInput::default());
    sim.set_player_state(state::DEATH);
    let before = claw(&sim);
    assert!(!sim.with_world(|w| player::bounce(w, 450)));
    assert_eq!(claw(&sim), before);
}

// ---------------------------------------------------------------- enemies

/// A robber thief dropped onto a fern, far from Claw.
fn thief_on_a_fern() -> (Sim, ObjectId, ObjectId) {
    let mut level = level4_with(vec![fern(3000, 600, 0)]);
    level
        .objects
        .push(ObjectSpec::at("RobberThief", 3000, 500, 4000));
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let cfg = SimConfig {
        activation: claw_sim::ActivationConfig::widened((2 * w, 2 * h)),
        ..original()
    };
    let mut sim = Sim::new(level, cfg);
    let f = find(&sim, "SpringBoard");
    let t = find(&sim, "RobberThief");
    // Set up on the floor below; dropped from above the fern, it lands on
    // it.
    sim.with_world(|w| {
        let o = w.objects.get_mut(t).unwrap();
        o.y = 500;
        o.physics_type = claw_sim::physics::ptype::FALL;
    });
    (sim, f, t)
}

#[test]
fn an_enemy_on_an_idle_fern_arms_it_and_is_launched_off_it() {
    let (mut sim, f, t) = thief_on_a_fern();
    let mut armed_at = None;
    let mut launch_seen = Vec::new();
    let mut thief_states = Vec::new();
    for k in 0..200 {
        sim.step(TickInput::default());
        let fs = obj_state(&sim, f);
        if armed_at.is_none() && fs == springboard::state::ENEMY_ARMED {
            armed_at = Some(k);
        }
        let launch = sim.objects().get(f).unwrap().special_flags & springboard::special::LAUNCH;
        launch_seen.push(launch != 0);
        thief_states.push(EnemyState::from_id(obj_state(&sim, t)));
    }
    let e = armed_at.expect("the thief arms the fern");
    // The fern starts the enemy cycle on E + 1, raises its launch flag on
    // E + 21 and lowers it on E + 49.
    let up: Vec<usize> = (0..200).filter(|&k| launch_seen[k]).collect();
    assert_eq!(up.first(), Some(&(e + 21)));
    assert_eq!(up.last(), Some(&(e + 48)));
    // The thief jumps off on the tick the flag goes up.
    let jumped = (0..200)
        .find(|&k| thief_states[k] == Some(EnemyState::JumpOffSpringboard))
        .expect("launched");
    assert_eq!(jumped, e + 21);
    let d = enemy::data_of(sim.objects().get(t).unwrap()).unwrap();
    assert!(!d.human.springboard_armed, "the launch clears its flag");
    assert!(d.stationary, "an enemy that waited on a fern stays put");
    // Claw's cycle never ran.
    assert!(
        thief_states[..e]
            .iter()
            .all(|s| *s != Some(EnemyState::JumpOffSpringboard))
    );
}

#[test]
fn each_cycle_launches_only_the_rider_that_started_it() {
    // Claw's cycle never raises the enemies' launch flag.
    let (mut sim, f) = on_a_fern(0, TickInput::default());
    let rows = record(&mut sim, f, 60, TickInput::default());
    assert!(rows.iter().any(|r| r.0.2 == state::JUMP));
    assert_eq!(
        sim.objects().get(f).unwrap().special_flags & springboard::special::LAUNCH,
        0
    );
    // An enemy's cycle under Claw never throws him: poked as an enemy
    // pokes it before it sees him, the fern runs the enemy cycle.
    let (mut sim, f) = on_a_fern(0, TickInput::default());
    sim.with_world(|w| w.objects.get_mut(f).unwrap().state = springboard::state::ENEMY_ARMED);
    let mut launched = false;
    for _ in 0..45 {
        sim.step(TickInput::default());
        let o = sim.objects().get(f).unwrap();
        launched |= o.special_flags & springboard::special::LAUNCH != 0;
        assert_eq!(claw(&sim).2, state::STAND, "Claw stays on the fern");
    }
    assert!(launched, "the enemy cycle raised its flag");
}

// ---------------------------------------------------------------- stones

fn stone_level(objects: Vec<ObjectSpec>) -> Sim {
    Sim::new(level4_with(objects), original())
}

/// Steps until `id` stores `s`; returns the ticks stepped (the last one
/// stores it).
fn until_state(sim: &mut Sim, id: ObjectId, s: i32, limit: usize) -> usize {
    for n in 1..=limit {
        sim.step(TickInput::default());
        if obj_state(sim, id) == s {
            return n;
        }
    }
    panic!("state {s} never came");
}

#[test]
fn a_clock_stone_sinks_17_calls_and_rises_18_calls_after_its_edges() {
    let mut sim = stone_level(vec![stone("SteppingStone", 1300, 700, 2000)]);
    let id = find(&sim, "SteppingStone");
    {
        let o = sim.objects().get(id).unwrap();
        assert_eq!(o.state, stone::clock::UP, "set up while the level loads");
        assert_eq!(o.hit_rect, stepping_stone::surface(4));
        assert_eq!(o.type_flags, PLATFORM);
        let d = stepping_stone::data_of(o).unwrap();
        assert_eq!(
            (d.phase, d.on_ms, d.period, d.switch_frame),
            (0, 1500, 3000, 3)
        );
    }
    // E: the clock edge seen, still solid.
    until_state(&mut sim, id, stone::clock::SINK_START, 400);
    assert!(solid(&sim, id));
    let mut frames = Vec::new();
    let mut solid_from_e = Vec::new();
    for _ in 0..17 {
        sim.step(TickInput::default());
        let o = sim.objects().get(id).unwrap();
        frames.push(o.frame);
        solid_from_e.push(o.type_flags == PLATFORM);
    }
    // Not solid from E + 1; frames 2 to 6 on E + 2, 5, 8, 11, 14; under on
    // E + 17.
    assert!(solid_from_e.iter().all(|s| !s));
    let change: Vec<(usize, i32)> = (1..17)
        .filter(|&k| frames[k] != frames[k - 1])
        .map(|k| (k + 1, frames[k]))
        .collect();
    assert_eq!(change, [(2, 2), (5, 3), (8, 4), (11, 5), (14, 6)]);
    assert_eq!(obj_state(&sim, id), stone::clock::UNDER);
    // R: the rise seen.
    until_state(&mut sim, id, stone::clock::RISE_START, 400);
    let mut states = Vec::new();
    let mut solids = Vec::new();
    for _ in 0..18 {
        sim.step(TickInput::default());
        states.push(obj_state(&sim, id));
        solids.push(solid(&sim, id));
    }
    assert_eq!(states[0], stone::clock::RISING, "R + 1");
    let first_solid = solids.iter().position(|&s| s).unwrap() + 1;
    assert_eq!(first_solid, 9, "solid again on R + 9");
    assert!(solids[8..].iter().all(|&s| s));
    assert_eq!(states[16], stone::clock::RISING);
    assert_eq!(states[17], stone::clock::UP, "up on R + 18");
    assert_eq!(sim.objects().get(id).unwrap().frame, 1);
}

#[test]
fn clock_stones_are_solid_75_or_76_ticks_a_quarter_period_apart() {
    let names = [
        "SteppingStone",
        "SteppingStone2",
        "SteppingStone3",
        "SteppingStone4",
    ];
    let mut sim = stone_level(
        names
            .iter()
            .enumerate()
            .map(|(i, n)| stone(n, 1200 + 60 * i as i32, 700, 2000))
            .collect(),
    );
    let ids: Vec<ObjectId> = names.iter().map(|n| find(&sim, n)).collect();
    let mut solid_rows = vec![Vec::new(); 4];
    let mut rises = vec![Vec::new(); 4];
    for k in 0..1000 {
        sim.step(TickInput::default());
        for i in 0..4 {
            if solid(&sim, ids[i]) {
                solid_rows[i].push(k);
            }
            if obj_state(&sim, ids[i]) == stone::clock::RISE_START {
                rises[i].push(k);
            }
        }
    }
    for i in 0..4 {
        // Runs of solid ticks after the first rise.
        let first = rises[i][0];
        let mut runs = Vec::new();
        let mut run = 0;
        for k in first..1000 {
            if solid_rows[i].binary_search(&k).is_ok() {
                run += 1;
            } else if run > 0 {
                runs.push(run);
                run = 0;
            }
        }
        assert!(!runs.is_empty());
        assert!(runs.iter().all(|r| matches!(r, 75 | 76)), "{i}: {runs:?}");
    }
    // Each variant rises 750 ms (41 or 42 ticks) after the one before.
    for i in 1..4 {
        let d = rises[i]
            .iter()
            .find(|&&r| r > rises[0][0] + 3)
            .copied()
            .unwrap() as i64
            - rises[i - 1]
                .iter()
                .find(|&&r| r > rises[0][0] + 3 - 50)
                .copied()
                .unwrap() as i64;
        let d = d.rem_euclid(167);
        assert!(matches!(d, 41 | 42), "variant {i}: {d}");
    }
}

#[test]
fn record_halves_set_a_clock_stones_cycle() {
    let mut s = stone("SteppingStone", 1300, 700, 2000);
    s.record[SPEED_X] = 2000;
    s.record[SPEED_Y] = 500;
    s.record[stepping_stone::PHASE_FIELD] = 2000;
    let sim = stone_level(vec![s]);
    let o = sim.objects().get(find(&sim, "SteppingStone")).unwrap();
    let d = stepping_stone::data_of(o).unwrap();
    assert_eq!((d.phase, d.on_ms, d.period), (2000, 2000, 2500));
}

#[test]
fn claw_falls_off_a_stone_the_moment_it_starts_to_sink() {
    let mut sim = stone_level(vec![stone("SteppingStone", 1300, 600, 2000)]);
    sim.unlatch_keys();
    let id = find(&sim, "SteppingStone");
    // Land while it is up early in its cycle.
    until_state(&mut sim, id, stone::clock::UP, 400);
    land_on(&mut sim, id, TickInput::default());
    let mut n = 0;
    while obj_state(&sim, id) != stone::clock::SINK_START {
        sim.step(TickInput::default());
        assert!(riding(&sim, id), "carried while up");
        n += 1;
        assert!(n < 200);
    }
    sim.step(TickInput::default());
    assert!(!riding(&sim, id));
    assert_eq!(claw(&sim).2, state::FALL, "falls on E + 1");
}

#[test]
fn stones_are_silent_unless_the_level_has_their_sounds() {
    let mut sim = stone_level(vec![stone("SteppingStone", 1000, 700, 2000)]);
    let id = find(&sim, "SteppingStone");
    let mut any = false;
    for _ in 0..400 {
        sim.step(TickInput::default());
        any |= sim
            .last_audio()
            .iter()
            .any(|c| matches!(c, AudioCue::Play(_)));
    }
    assert!(!any, "no sound at all");
    let _ = id;
    // A level that ships the keys: the sink plays the first, the rise the
    // second, from the stone while it is on screen.
    let mut level = level4_with(vec![stone("SteppingStone", 1000, 700, 2000)]);
    level.sounds.insert(SINK_SOUND.into(), Some(100));
    level.sounds.insert(RISE_SOUND.into(), Some(100));
    level.images.get_mut(STONE_SET).unwrap().boxes = (1..=6)
        .map(|f| (f, claw_sim::object::Rect::new(-32, 0, 32, 13)))
        .collect();
    let mut sim = Sim::new(level, original());
    let id = find(&sim, "SteppingStone");
    until_state(&mut sim, id, stone::clock::SINK_START, 400);
    assert_eq!(cues(&sim, SINK_SOUND), 0);
    sim.step(TickInput::default());
    assert_eq!(cues(&sim, SINK_SOUND), 1, "on E + 1");
    until_state(&mut sim, id, stone::clock::RISE_START, 400);
    sim.step(TickInput::default());
    assert_eq!(cues(&sim, RISE_SOUND), 1, "on R + 1");
}

#[test]
fn a_contact_stone_drops_claw_12_ticks_after_he_lands_and_comes_back() {
    let mut sim = stone_level(vec![stone("StartSteppingStone", 1300, 600, 2000)]);
    sim.unlatch_keys();
    let id = find(&sim, "StartSteppingStone");
    assert_eq!(obj_state(&sim, id), stone::contact::WAITING);
    land_on(&mut sim, id, TickInput::default());
    // Index k is tick L + 1 + k.
    let mut states = Vec::new();
    let mut solids = Vec::new();
    let mut claws = Vec::new();
    for _ in 0..110 {
        sim.step(TickInput::default());
        states.push(obj_state(&sim, id));
        solids.push(solid(&sim, id));
        claws.push(claw(&sim).2);
    }
    let at = |l: usize| l - 1;
    assert_eq!(states[at(1)], stone::contact::DELAYED, "seen on T = L + 1");
    assert_eq!(states[at(8)], stone::contact::SINKING, "T + 7");
    assert!(solids[at(11)]);
    assert!(!solids[at(12)], "not solid on L + 12");
    assert_eq!(claws[at(12)], state::FALL, "Claw falls on L + 12");
    assert_eq!(states[at(24)], stone::contact::SUNK, "T + 23");
    assert_eq!(states[at(25)], stone::contact::RISE_START, "T + 24");
    assert_eq!(states[at(81)], stone::contact::RISING, "T + 80");
    let back = (at(81)..110).find(|&k| solids[k]).unwrap() + 1;
    assert_eq!(back, 89, "solid again on T + 88");
    assert_eq!(states[at(98)], stone::contact::WAITING, "T + 97");
}

#[test]
fn a_one_time_stone_stays_sunk_until_claw_loses_a_life() {
    let mut sim = stone_level(vec![stone("OneTimeStartSteppingStone", 1300, 600, 2000)]);
    sim.unlatch_keys();
    let id = find(&sim, "OneTimeStartSteppingStone");
    land_on(&mut sim, id, TickInput::default());
    idle(&mut sim, 200);
    assert_eq!(obj_state(&sim, id), stone::contact::SINKING);
    assert_eq!(sim.objects().get(id).unwrap().type_flags, NOT_PLATFORM);
    sim.with_world(|w| w.claw.stats.attempt += 1);
    sim.step(TickInput::default());
    assert_eq!(obj_state(&sim, id), stone::SET_UP);
    sim.step(TickInput::default());
    assert_eq!(obj_state(&sim, id), stone::contact::WAITING);
    assert!(solid(&sim, id));
}
