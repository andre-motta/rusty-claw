//! Checkpoints, death and save points on the first level of the user's own
//! install. The tests read the folder named by the `CLAW_INSTALL`
//! environment variable at run time and skip themselves when it is unset;
//! positions come from the level's own objects and tiles, found by logic
//! name and tile kind at run time.

mod common;

use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_frame::{AudioCue, GameEvent};
use claw_sim::container::CollisionEvent;
use claw_sim::object::{ObjectId, draw_flags, flags};
use claw_sim::player::{death, hit, state};
use claw_sim::tiles::{CLEAR, DEATH};
use claw_sim::trace::{self, TraceRow};
use claw_sim::{InputLatch, InputScript, SavePos, Sim, SimConfig, SimLevel, StartState, TickInput};
use common::*;

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

fn level1(store: &AssetStore) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(1)).unwrap();
    SimLevel::from_level(&data, store).unwrap()
}

/// Position of the object running `logic` nearest to the level start.
fn nearest(l: &SimLevel, logic: &str) -> (i32, i32) {
    let (sx, sy) = l.start;
    l.objects
        .iter()
        .filter(|o| o.logic == logic)
        .map(|o| (o.record[5], o.record[6]))
        .min_by_key(|&(x, y)| {
            let (dx, dy) = (i64::from(x - sx), i64::from(y - sy));
            dx * dx + dy * dy
        })
        .unwrap_or_else(|| panic!("no {logic} in the level"))
}

/// A point above a death tile with clear air above it, nearest to `near`:
/// Claw dropped there lands on the death tile.
fn above_death_tile(l: &SimLevel, near: (i32, i32)) -> (i32, i32) {
    let map = &l.map;
    let mut best: Option<((i32, i32), i64)> = None;
    for x in (0..map.width()).step_by(8) {
        for y in 130..map.height() {
            if map.attr(x, y) != DEATH {
                continue;
            }
            let clear = (y - 130..y).all(|yy| {
                (x - 24..=x + 24)
                    .step_by(8)
                    .all(|xx| map.attr(xx, yy) == CLEAR)
            });
            if clear {
                let (dx, dy) = (i64::from(x - near.0), i64::from(y - near.1));
                let d = dx * dx + dy * dy;
                if best.is_none_or(|(_, b)| d < b) {
                    best = Some(((x, y - 70), d));
                }
            }
            break;
        }
    }
    best.expect("a death tile in the level").0
}

/// One step of a scenario: an optional teleport, then held buttons for a
/// number of ticks.
struct Segment {
    teleport: Option<(i32, i32)>,
    held: u32,
    ticks: usize,
}

/// Runs `segments` with the buttons fed through the input latch at a
/// display rate of `hz` (`None` feeds each tick's input directly); returns the
/// trace and every event.
fn run(l: &SimLevel, segments: &[Segment], hz: Option<u64>) -> (String, Vec<GameEvent>) {
    let mut held = Vec::new();
    let mut teleports = Vec::new();
    for s in segments {
        if let Some(p) = s.teleport {
            teleports.push((held.len() as u64, p));
        }
        held.extend(std::iter::repeat_n(s.held, s.ticks));
    }
    let script = InputScript::from_held(&held);
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    let mut rows = Vec::new();
    let mut events = Vec::new();
    let mut step = |sim: &mut Sim, tick: u64, input: TickInput| {
        for &(at, (x, y)) in &teleports {
            if at == tick {
                sim.teleport_player(x, y);
            }
        }
        let snap = sim.step(input);
        events.extend(snap.events);
        rows.push(TraceRow::capture(sim, sim.tick() - 1, "script"));
    };
    let n = script.ticks.len() as u64;
    let Some(hz) = hz else {
        for (t, &input) in script.ticks.iter().enumerate() {
            step(&mut sim, t as u64, input);
        }
        return (trace::to_csv(&rows), events);
    };
    let mut latch = InputLatch::new();
    let changes = script.events();
    let mut next = 0;
    let mut tick = 0u64;
    let mut frame = 0u64;
    while tick < n {
        let now_us = frame * 1_000_000 / hz;
        while next < changes.len() && changes[next].0 <= now_us {
            let (_, b, down) = changes[next];
            latch.event(now_us, b, down);
            next += 1;
        }
        while tick < n && (tick + 1) * claw_sim::input::TICK_US <= now_us {
            step(&mut sim, tick, latch.take(tick));
            tick += 1;
        }
        frame += 1;
    }
    (trace::to_csv(&rows), events)
}

#[test]
fn install_death_respawns_at_the_first_checkpoint() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    let flag = nearest(&l, "Checkpoint");
    let pit = above_death_tile(&l, flag);
    let mut sim = Sim::new(l, SimConfig::default());
    sim.teleport_player(flag.0 - 10, flag.1);
    let mut events = Vec::new();
    for _ in 0..120 {
        events.extend(sim.step(TickInput::default()).events);
    }
    let point = (flag.0, flag.1 - 32);
    assert!(
        events.contains(&GameEvent::Checkpoint {
            x: point.0,
            y: point.1
        }),
        "{events:?}"
    );
    assert_eq!(sim.claw().checkpoint, Some(point));
    sim.teleport_player(pit.0, pit.1);
    let mut died = None;
    let mut spiked = false;
    for i in 0..600 {
        let snap = sim.step(TickInput::default());
        spiked |= snap
            .audio
            .iter()
            .any(|a| matches!(a, AudioCue::Play(s) if s.key == death::DEATH_TILE_SOUND));
        if snap.events.contains(&GameEvent::ClawDied { lives_left: 6 }) {
            died = Some(i);
            break;
        }
        assert!(snap.events.is_empty(), "{:?}", snap.events);
    }
    assert!(died.is_some(), "Claw died on the death tile");
    assert!(spiked, "the death-tile sound played");
    assert_eq!(sim.stats().health, 0);
    assert_eq!(sim.stats().attempt, 1);
    assert_eq!(sim.player().unwrap().state, state::DEATH);
    assert_eq!(sim.claw().death_tile_hits, 1);
    idle(&mut sim, 70);
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::STAND);
    assert_eq!(o.x, flag.0);
    assert!(
        (o.y - point.1).abs() <= 64,
        "on the ground by the flag: {}",
        o.y
    );
    assert_eq!(sim.stats().health, 100);
    assert_eq!(sim.stats().lives, 6);
    idle(&mut sim, 100);
    assert!(sim.snapshot().transition.is_none());
    assert_eq!(sim.player().unwrap().x, flag.0);
}

#[test]
fn install_save_point_saves_and_a_loaded_save_starts_there() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    let save = nearest(&l, "FirstSuperCheckpoint");
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    sim.teleport_player(save.0 - 10, save.1);
    let mut events = Vec::new();
    for _ in 0..120 {
        events.extend(sim.step(TickInput::default()).events);
    }
    assert!(
        events.contains(&GameEvent::SavePoint { slot: 1 }),
        "{events:?}"
    );
    assert!(events.contains(&GameEvent::Message { id: 1, ms: 5000 }));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, GameEvent::SavePoint { .. }))
            .count(),
        1
    );
    let point = sim
        .claw()
        .checkpoint
        .expect("the save point is the checkpoint");
    let start = StartState {
        carry: Some(sim.carry()),
        save: Some(SavePos {
            slot: 1,
            x: point.0,
            y: point.1,
        }),
    };
    let mut loaded = Sim::with_start(l, SimConfig::default(), start);
    let o = loaded.player().unwrap();
    assert_eq!(o.x, save.0);
    assert!((o.y - point.1).abs() <= 64);
    assert_eq!(loaded.claw().checkpoint, Some(point));
    assert_eq!(loaded.carry(), sim.carry());
    let flag_of = |s: &Sim| {
        s.objects()
            .all()
            .find(|(_, o)| &*o.logic == "FirstSuperCheckpoint")
            .map(|(_, o)| o.flags & flags::DESTROY != 0 || o.draw_flags & draw_flags::HIDDEN != 0)
    };
    assert_eq!(flag_of(&loaded), Some(true), "the flag is going");
    let mut events = Vec::new();
    for _ in 0..120 {
        events.extend(loaded.step(TickInput::default()).events);
    }
    assert_eq!(flag_of(&loaded), None, "the flag is gone");
    assert!(events.is_empty(), "{events:?}");
    let o = loaded.player().unwrap();
    assert_eq!(o.x, save.0, "Claw stays where the save was made");
}

#[test]
fn install_traces_are_identical_at_60_144_and_240_hz() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    let flag = nearest(&l, "Checkpoint");
    let pit = above_death_tile(&l, flag);
    let save = nearest(&l, "FirstSuperCheckpoint");
    let segments = [
        Segment {
            teleport: None,
            held: RIGHT,
            ticks: 60,
        },
        Segment {
            teleport: Some((flag.0 - 80, flag.1)),
            held: RIGHT,
            ticks: 40,
        },
        Segment {
            teleport: None,
            held: 0,
            ticks: 30,
        },
        Segment {
            teleport: Some(pit),
            held: 0,
            ticks: 400,
        },
        Segment {
            teleport: None,
            held: LEFT | JUMP,
            ticks: 50,
        },
        Segment {
            teleport: Some((save.0 - 10, save.1)),
            held: 0,
            ticks: 100,
        },
    ];
    let (direct, events) = run(&l, &segments, None);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::Checkpoint { .. }))
    );
    assert!(events.contains(&GameEvent::ClawDied { lives_left: 6 }));
    assert!(events.contains(&GameEvent::SavePoint { slot: 1 }));
    for hz in [60, 144, 240] {
        let (t, _) = run(&l, &segments, Some(hz));
        assert_eq!(t, direct, "{hz} Hz");
    }
}

/// The officer running `logic` placed at `at` on the level.
fn object_at(sim: &Sim, logic: &str, at: (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic && (o.x, o.y) == at)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic} at {at:?}"))
}

#[test]
fn install_the_first_officer_hurts_claw_until_he_dies_and_respawns() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    let start = l.start;
    let at = nearest(&l, "Officer");
    let mut sim = Sim::new(l, SimConfig::default());
    let officer = object_at(&sim, "Officer", at);
    // Claw stands still in front of the officer and lets him strike.
    sim.teleport_player(at.0 - 60, at.1);
    let mut health = sim.stats().health;
    let mut hits = Vec::new();
    let mut blow = 0;
    let mut died = None;
    for i in 0..900 {
        let snap = sim.step(TickInput::default());
        let now = sim.stats().health;
        if now != health {
            // The blow's damage is the officer's while he swings.
            let damage = sim.objects().get(officer).unwrap().damage.max(1);
            assert_eq!(now, health - damage, "tick {i}: one blow's damage");
            blow = damage;
            assert!(
                sim.collision_events().iter().any(|e| matches!(
                    e,
                    CollisionEvent::Hit { attacker, .. } if *attacker == officer
                )),
                "tick {i}: the officer's blow"
            );
            assert!(
                snap.audio.iter().any(|a| matches!(
                    a,
                    AudioCue::Play(s) if hit::HIT_SOUNDS.contains(&s.key.as_str())
                )),
                "tick {i}: a hit sound"
            );
            hits.push(i);
            health = now;
        }
        if snap.events.contains(&GameEvent::ClawDied { lives_left: 6 }) {
            died = Some(i);
            break;
        }
    }
    assert!(
        hits.first().is_some_and(|&t| t < 120),
        "the officer strikes soon: {hits:?}"
    );
    // The blows land no faster than the window after each hit allows.
    let window = death::ticks_of(hit::INVULNERABLE_MS as u32) as usize;
    for w in hits.windows(2) {
        assert!(w[1] - w[0] >= window, "{hits:?}");
    }
    assert_eq!(hits.len() as i32, (100 + blow - 1) / blow, "{hits:?}");
    assert!(died.is_some(), "Claw died of his wounds");
    assert_eq!(sim.stats().health, 0);
    assert_eq!(sim.player().unwrap().state, state::DEATH_FALL);
    assert_eq!(sim.claw().death_tile_hits, 0);
    assert_eq!(sim.stats().attempt, 1);
    // No checkpoint yet: he comes back at the level start, whole.
    idle(&mut sim, 70);
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::STAND);
    assert_eq!(o.x, start.0);
    assert!(
        (o.y - start.1).abs() <= 64,
        "on the ground at the start: {}",
        o.y
    );
    assert_eq!(sim.stats().health, 100);
    assert_eq!(sim.stats().lives, 6);
}
