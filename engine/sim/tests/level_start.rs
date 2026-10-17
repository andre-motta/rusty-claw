//! How play starts: keys already down count only once released and pressed
//! again, the level's objects are set up while it loads, and the frame
//! pulse that times the enemies' per-tick roll.

mod common;

use claw_sim::logic::enemy::{self, EnemyState};
use claw_sim::object::{LogicKind, ObjectId, engine_state};
use claw_sim::player::state;
use claw_sim::sim::{LEVEL_LOAD_SEED, PULSE_MS, PULSE_START_MS};
use claw_sim::{Buttons, GameLogic, InputScript, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The first rows of the reference store level1/bot-original (c18): Right
/// and Jump held from the first tick, Jump let go at tick 14, Right at
/// tick 16, Right pressed again at tick 22 and let go at tick 28.
fn opening_rows() -> InputScript {
    let mut held = vec![RIGHT | JUMP; 14];
    held.extend([RIGHT; 2]);
    held.extend([0; 6]);
    held.extend([RIGHT; 6]);
    held.extend([0; 6]);
    InputScript::from_held(&held)
}

#[test]
fn keys_held_when_play_starts_count_only_once_released_and_pressed_again() {
    // Reference store level1/bot-original, c18: with Right and Jump down
    // from the first tick Claw stands at his start through t_ms 396 (tick
    // 21, pressed 0 at tick 0), and walks from t_ms 414 (tick 22), the tick
    // Right is pressed again.
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    assert_eq!(sim.latched(), Buttons(u32::MAX), "every key starts latched");
    let start = sim.player().unwrap().x;
    let rows = opening_rows();
    for (k, &t) in rows.ticks.iter().enumerate() {
        sim.step(t);
        let o = sim.player().unwrap();
        // The input as given is kept (and traced); Claw sees what is left
        // after the latch.
        assert_eq!(sim.input(), t, "tick {k}");
        if k < 22 {
            assert_eq!(sim.claw_input(), TickInput::default(), "tick {k}");
            assert_eq!((o.x, o.state), (start, state::STAND), "tick {k}");
        }
    }
    let walked: Vec<i32> = {
        let mut sim = Sim::new(flat_level(), SimConfig::default());
        rows.ticks
            .iter()
            .map(|&t| {
                sim.step(t);
                sim.player().unwrap().x - start
            })
            .collect()
    };
    assert_eq!(walked[21], 0);
    assert!(walked[22] > 0, "walks at tick 22: {walked:?}");
    assert!(walked[27] > walked[22]);
    // Once released, a key is not latched again.
    assert_eq!(sim.latched(), Buttons::NONE);
}

#[test]
fn a_jump_held_from_the_start_jumps_only_after_a_release() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    for k in 0..10 {
        sim.step(if k == 0 { press(JUMP) } else { hold(JUMP) });
        assert_eq!(sim.player().unwrap().state, state::STAND, "tick {k}");
    }
    sim.step(TickInput::default());
    sim.step(press(JUMP));
    assert_eq!(sim.player().unwrap().state, state::JUMP);
}

#[test]
fn keys_down_at_a_latch_count_only_once_released() {
    // The return from a screen over play latches every key down, as the
    // level start does.
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    sim.step(TickInput::default());
    sim.step(press(RIGHT));
    for _ in 0..5 {
        sim.step(hold(RIGHT));
    }
    let x = sim.player().unwrap().x;
    assert!(x > 0);
    sim.latch_keys();
    for _ in 0..30 {
        sim.step(hold(RIGHT));
    }
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::STAND, "Right held on does not walk");
    let stopped = o.x;
    assert!(
        stopped - x < 32,
        "only the walk's own slow-down: {x} to {stopped}"
    );
    sim.step(TickInput::default());
    sim.step(press(RIGHT));
    sim.step(hold(RIGHT));
    assert!(sim.player().unwrap().x > stopped, "a new press walks");
    // A test can lift the latch to press from the first tick.
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    sim.unlatch_keys();
    sim.step(press(JUMP));
    assert_eq!(sim.player().unwrap().state, state::JUMP);
}

/// A flat level with an officer near Claw.
fn officer_level() -> SimLevel {
    let mut l = flat_level();
    l.objects
        .push(claw_sim::level::ObjectSpec::at("Officer", 1300, 899, 4000));
    l
}

fn officer(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Officer))
        .map(|(id, _)| id)
        .expect("the officer")
}

#[test]
fn the_level_objects_are_set_up_while_the_level_loads() {
    // Reference store level1/bot-original, c18: the original's random
    // state at time zero is the level-load state advanced by the set-up
    // draws, and the first tick (t_ms 18) already draws for the enemies'
    // first updates.
    let cfg = SimConfig {
        seed: 4321,
        ..SimConfig::default()
    };
    let sim = Sim::new(officer_level(), cfg);
    let id = officer(&sim);
    let o = sim.objects().get(id).unwrap();
    assert_ne!(o.state, engine_state::INIT, "built before time zero");
    assert_eq!(
        EnemyState::from_id(o.state),
        Some(EnemyState::Idle),
        "and gone idle"
    );
    assert!(enemy::data_of(o).is_some());
    // Its set-up drew from the level-load state; time zero starts from the
    // seed.
    assert_ne!(sim.rng_after_set_up(), LEVEL_LOAD_SEED);
    assert_eq!(sim.rng_state(), 4321);
    // The same set-up draws whatever the seed.
    let other = Sim::new(
        officer_level(),
        SimConfig {
            seed: 99,
            ..SimConfig::default()
        },
    );
    assert_eq!(other.rng_after_set_up(), sim.rng_after_set_up());
    // Without objects nothing draws while loading.
    let empty = Sim::new(flat_level(), SimConfig::default());
    assert_eq!(empty.rng_after_set_up(), LEVEL_LOAD_SEED);
}

#[test]
fn the_frame_pulse_starts_at_100_ms_and_comes_every_33_ms() {
    // Reference store level1/bot-original, c18: the enemies' rolls come at
    // t_ms 108, 162, 216, ... (ticks 5, 8, 11, ...).
    assert_eq!((PULSE_START_MS, PULSE_MS), (100, 33));
    let mut sim = Sim::new(officer_level(), SimConfig::default());
    let mut rolls = Vec::new();
    for k in 0..20u64 {
        let before = sim.rng_state();
        sim.step(TickInput::default());
        let drew = sim.rng_state() != before;
        if sim.pulse() {
            rolls.push(k);
            assert!(drew, "the officer rolls on the pulse at tick {k}");
        }
    }
    assert_eq!(rolls, [5, 8, 11, 14, 17]);
}
