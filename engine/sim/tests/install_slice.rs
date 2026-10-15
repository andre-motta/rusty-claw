//! The combat slice on the first level of your own install (skipped
//! without `CLAW_INSTALL`): Claw is put next to the level's first officer,
//! found through the level's own records, takes its strike, and kills it
//! with scripted Attack presses. Every sound the run asks for is one the
//! level registers, and the run traces the same at 60, 144 and 240 Hz.

use std::collections::BTreeSet;
use std::sync::Arc;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{AudioCue, Buttons, TickInput};
use claw_sim::logic::enemy::EnemyState;
use claw_sim::object::{LogicKind, ObjectId};
use claw_sim::player::attack::kind;
use claw_sim::player::state;
use claw_sim::trace::{self, TraceRow};
use claw_sim::{GameLogic, InputLatch, InputScript, Sim, SimConfig, SimLevel};

const ATTACK: u32 = 0x2;
const LEFT: u32 = 0x100_0000;
const RIGHT: u32 = 0x200_0000;

fn level1() -> Option<Arc<SimLevel>> {
    let Some(path) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let install = Install::open(path).expect("install opens");
    let store = install.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(1)).expect("level 1 loads");
    Some(Arc::new(
        SimLevel::from_level(&level, &store).expect("level 1 prepares"),
    ))
}

/// The first officer record of the level, by file order: its position.
fn first_officer_record(level: &SimLevel) -> (i32, i32) {
    let o = level
        .objects
        .iter()
        .find(|o| o.logic == "Officer")
        .expect("the level places officers");
    (o.record[5], o.record[6])
}

/// The live officer nearest to (x, y).
fn officer_near(sim: &Sim, x: i32, y: i32) -> Option<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| o.kind == LogicKind::Game(GameLogic::Officer))
        .min_by_key(|(_, o)| (o.x - x).abs() + (o.y - y).abs())
        .map(|(id, _)| id)
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    sim.objects()
        .get(id)
        .and_then(|o| EnemyState::from_id(o.state))
}

/// Runs idle ticks until Claw and the camera have stood still for five
/// ticks (at most 60).
fn settle(sim: &mut Sim) {
    let mut still = 0;
    let mut last = None;
    for _ in 0..60 {
        sim.step(TickInput::default());
        let now = sim.player().map(|o| (o.x, o.y, sim.camera().current));
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still >= 5 {
            break;
        }
    }
}

/// Offsets from the first officer tried for Claw, in order: he must end
/// up standing (not teetering at a ledge) within the officer's range.
const OFFSETS: [i32; 6] = [-60, 60, -45, 45, -75, 75];

/// Puts Claw beside the first officer of the level, standing on the floor
/// and facing it, and lets him and the camera settle; returns the officer.
fn place(sim: &mut Sim, level: &SimLevel) -> ObjectId {
    let (x, y) = first_officer_record(level);
    for dx in OFFSETS {
        let mut trial = sim.clone();
        trial.teleport_player(x + dx, y - 40);
        settle(&mut trial);
        let Some(id) = officer_near(&trial, x, y) else {
            continue;
        };
        let (c, o) = (trial.player().unwrap(), trial.objects().get(id).unwrap());
        if c.state != state::STAND || (c.x - o.x).abs() >= 100 || (c.y - o.y).abs() >= 40 {
            continue;
        }
        let left = o.x < c.x;
        if left == trial.claw().facing_right {
            // Turn toward it: one tick of walking that way, then stop.
            let b = if left { LEFT } else { RIGHT };
            trial.step(TickInput {
                held: Buttons(b),
                pressed: Buttons(b),
            });
            settle(&mut trial);
        }
        if trial.player().unwrap().state == state::STAND {
            *sim = trial;
            return id;
        }
    }
    panic!("no standing spot beside the first officer");
}

fn new_sim(level: &Arc<SimLevel>) -> Sim {
    Sim::new(level.clone(), SimConfig::default())
}

/// Taps Attack every 450 ms, `n` times, then waits.
fn attack_script(n: usize) -> InputScript {
    let mut held = vec![0u32; 10];
    for _ in 0..n {
        held.extend([ATTACK; 2]);
        held.extend([0u32; 23]);
    }
    held.extend([0u32; 120]);
    InputScript::from_held(&held)
}

#[test]
fn claw_fights_the_first_officer() {
    let Some(level) = level1() else { return };
    let mut sim = new_sim(&level);
    let id = place(&mut sim, &level);
    let o = sim.objects().get(id).expect("officer");
    assert_eq!(o.health, 3, "an officer of the first level has 3 health");
    assert_eq!(o.damage, 10);
    let claw = sim.player().unwrap();
    assert!(
        (claw.x - o.x).abs() < 100 && (claw.y - o.y).abs() < 40,
        "Claw stands in the officer's range: claw ({}, {}), officer ({}, {})",
        claw.x,
        claw.y,
        o.x,
        o.y
    );
    // Let the officer strike: Claw loses its damage, once per window.
    let start = sim.stats().health;
    let mut struck = false;
    let mut sounds = BTreeSet::new();
    for _ in 0..120 {
        sim.step(TickInput::default());
        collect(&sim, &mut sounds);
        if sim.stats().health < start {
            struck = true;
            break;
        }
    }
    assert!(struck, "the officer struck Claw");
    assert_eq!(
        start - sim.stats().health,
        10,
        "the strike costs its damage"
    );
    assert_eq!(sim.stats().invulnerable_ms, 850);
    // Scripted attack presses kill it.
    let mut killed = false;
    let mut blows = Vec::new();
    for input in attack_script(6).ticks {
        sim.step(input);
        collect(&sim, &mut sounds);
        if input.pressed.contains(Buttons::ATTACK) && sim.claw().attacking {
            blows.push(sim.claw().attack.kind);
        }
        if state_of(&sim, id) == Some(EnemyState::Dead) {
            killed = true;
        }
    }
    assert!(killed, "the officer died; blows {blows:?}");
    assert!(
        blows
            .iter()
            .all(|b| [kind::SWIPE, kind::PUNCH, kind::UPPERCUT, kind::KICK].contains(b)),
        "{blows:?}"
    );
    // The body leaves and is removed.
    for _ in 0..200 {
        sim.step(TickInput::default());
        collect(&sim, &mut sounds);
    }
    assert!(sim.objects().get(id).is_none(), "the body was removed");
    // Every sound asked for is registered by the level.
    let missing: Vec<&String> = sounds
        .iter()
        .filter(|k| !level.sounds.contains_key(*k))
        .collect();
    assert!(missing.is_empty(), "unregistered sounds: {missing:?}");
    for key in ["LEVEL_OFFICER_OFCSWSTB8-22", "GAME_SPLASH"] {
        assert!(sounds.contains(key), "{key} played: {sounds:?}");
    }
    eprintln!("sounds of the fight: {sounds:?}");
}

fn collect(sim: &Sim, out: &mut BTreeSet<String>) {
    for c in sim.last_audio() {
        if let AudioCue::Play(s) = c {
            out.insert(s.key.clone());
        }
    }
}

fn trace_direct(sim: &mut Sim, script: &InputScript) -> String {
    let rows: Vec<TraceRow> = script
        .ticks
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            sim.step(t);
            TraceRow::capture(sim, i as u64, "script")
        })
        .collect();
    trace::to_csv(&rows)
}

/// The script's button changes seen by a latch at a simulated display rate.
fn trace_at_rate(sim: &mut Sim, script: &InputScript, hz: u64) -> String {
    let mut latch = InputLatch::new();
    let events = script.events();
    let mut next_event = 0;
    let mut rows = Vec::new();
    let mut tick = 0u64;
    let n = script.ticks.len() as u64;
    let mut frame = 0u64;
    while tick < n {
        let now_us = frame * 1_000_000 / hz;
        while next_event < events.len() && events[next_event].0 <= now_us {
            let (_, b, down) = events[next_event];
            latch.event(now_us, b, down);
            next_event += 1;
        }
        while tick < n && (tick + 1) * claw_sim::input::TICK_US <= now_us {
            sim.step(latch.take(tick));
            rows.push(TraceRow::capture(sim, tick, "script"));
            tick += 1;
        }
        frame += 1;
    }
    trace::to_csv(&rows)
}

#[test]
fn the_fight_traces_the_same_at_60_144_and_240_hz() {
    let Some(level) = level1() else { return };
    let mut held = vec![0u32; 70];
    for _ in 0..4 {
        held.extend([ATTACK; 2]);
        held.extend([0u32; 25]);
    }
    held.extend([0x200_0000u32; 40]);
    held.extend([ATTACK; 1]);
    held.extend([0u32; 150]);
    let script = InputScript::from_held(&held);
    let base = {
        let mut sim = new_sim(&level);
        place(&mut sim, &level);
        trace_direct(&mut sim, &script)
    };
    assert!(
        base.lines()
            .skip(1)
            .any(|l| l.split(',').nth(15).is_some_and(|h| h != "100")),
        "Claw was hit in the traced fight"
    );
    for hz in [60, 144, 240] {
        let mut sim = new_sim(&level);
        place(&mut sim, &level);
        assert_eq!(trace_at_rate(&mut sim, &script, hz), base, "{hz} Hz");
    }
}
