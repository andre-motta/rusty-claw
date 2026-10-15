//! The enemies of the first level of your own install (skipped without
//! `CLAW_INSTALL`): every officer, soldier and rat, found through the
//! level's own records, starts on the ground, is killed with the sword in
//! an encounter placed from its record, and with every enemy awake two
//! minutes of idle play leave every enemy inside the level; enemy traces
//! are the same at 60, 144 and 240 Hz.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{Buttons, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::logic::enemy::{self, DeathKind, EnemyState};
use claw_sim::object::{LogicKind, Object, ObjectId};
use claw_sim::physics::{self, ptype};
use claw_sim::player::state;
use claw_sim::tiles::SOLID;
use claw_sim::trace::{self, TraceRow};
use claw_sim::{GameLogic, InputLatch, InputScript, Sim, SimConfig, SimLevel};

const ATTACK: u32 = 0x2;
const LEFT: u32 = 0x100_0000;
const RIGHT: u32 = 0x200_0000;
const DOWN: u32 = 0x800_0000;

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

fn is_enemy(o: &Object) -> bool {
    matches!(
        o.kind,
        LogicKind::Game(GameLogic::Officer | GameLogic::Soldier | GameLogic::Rat)
    )
}

/// The enemy records of the level: logic name and placed position.
fn enemy_records(level: &SimLevel) -> Vec<(String, i32, i32, i32)> {
    level
        .objects
        .iter()
        .filter(|o| matches!(o.logic.as_str(), "Officer" | "Soldier" | "Rat"))
        .map(|o| (o.logic.clone(), o.record[5], o.record[6], o.record[7]))
        .collect()
}

/// Every level object awake from the start: activation windows widened to
/// twice the level's size, which keeps their centre inside the level (a
/// centre far outside it removes the enemies).
fn all_awake(level: &SimLevel) -> SimConfig {
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    SimConfig {
        activation: ActivationConfig::widened((2 * w, 2 * h)),
        ..SimConfig::default()
    }
}

/// The enemy placed at (x, y, z) by its record.
fn enemy_from(sim: &Sim, x: i32, y: i32, z: i32) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| is_enemy(o) && o.editor == (x, y, z))
        .map(|(id, _)| id)
}

/// How a fight stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// The enemy is alive.
    Alive,
    /// Seen dead of a hit: a kill.
    Killed,
    /// Removed, or dead some other way, without being seen dead of a hit
    /// first: not a sword kill.
    Lost,
}

/// How the fight with `id` stands. Only an enemy seen dead of a hit counts
/// as killed; one that disappears before that does not.
fn outcome(sim: &Sim, id: ObjectId) -> Outcome {
    let Some(o) = sim.objects().get(id) else {
        return Outcome::Lost;
    };
    match EnemyState::from_id(o.state) {
        Some(EnemyState::Dead) => match enemy::data_of(o) {
            Some(d) if d.dead && d.death == DeathKind::Hit => Outcome::Killed,
            _ => Outcome::Lost,
        },
        _ => Outcome::Alive,
    }
}

fn feet(o: &Object) -> i32 {
    o.y + o.move_rect.bottom
}

#[test]
fn install_every_enemy_starts_on_the_ground() {
    let Some(level) = level1() else { return };
    let records = enemy_records(&level);
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for (logic, ..) in &records {
        *count.entry(logic.as_str()).or_default() += 1;
    }
    eprintln!("enemy records: {count:?}");
    assert_eq!(count.get("Officer"), Some(&26));
    assert_eq!(count.get("Soldier"), Some(&16));
    assert_eq!(count.get("Rat"), Some(&12));
    let mut sim = Sim::new(level.clone(), all_awake(&level));
    sim.step(TickInput::default());
    let mut bad = Vec::new();
    for (logic, x, y, z) in &records {
        let id = enemy_from(&sim, *x, *y, *z).expect("the enemy exists");
        let o = sim.objects().get(id).unwrap();
        let built = enemy::data_of(o).is_some();
        let standing = o.physics_type == ptype::STAND;
        let floor =
            (o.move_rect.left..=o.move_rect.right).any(|dx| physics::support_at(&level.map, o, dx));
        if !(built && standing && floor) {
            bad.push(format!(
                "{logic} at ({x}, {y}): built {built}, standing {standing}, floor {floor}, now ({}, {})",
                o.x, o.y
            ));
        }
    }
    assert!(bad.is_empty(), "not on the ground: {bad:#?}");
}

/// Runs idle ticks until Claw has stood still for five ticks (at most 60).
fn settle(sim: &mut Sim) {
    let mut still = 0;
    let mut last = None;
    for _ in 0..60 {
        sim.step(TickInput::default());
        let now = sim.player().map(|o| (o.x, o.y, o.state));
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still >= 5 {
            break;
        }
    }
}

/// Offsets from an enemy's record tried for Claw, in order.
const OFFSETS: [i32; 8] = [-70, 70, -50, 50, -100, 100, -30, 30];

/// Puts Claw beside the enemy placed at (x, y, z), standing at the height
/// of its feet; returns the enemy, or `None` when no spot works.
fn place(sim: &mut Sim, (x, y, z): (i32, i32, i32), rat: bool) -> Option<ObjectId> {
    for dx in OFFSETS {
        let mut trial = sim.clone();
        // Claw's feet are 60 px below his position, a rat's 33 px below
        // its own and the others' about 50 px.
        trial.teleport_player(x + dx, if rat { y - 40 } else { y - 30 });
        settle(&mut trial);
        let Some(id) = enemy_from(&trial, x, y, z) else {
            continue;
        };
        let (c, o) = (trial.player().unwrap(), trial.objects().get(id).unwrap());
        // Standing level with it, with room for his whole body (not
        // squeezed into a gap in a wall).
        let map = &trial.level().map;
        let room = (c.y + c.move_rect.top..=c.y + c.move_rect.bottom)
            .step_by(8)
            .all(|y| map.attr(c.x, y) != SOLID);
        if c.state != state::STAND || (feet(c) - feet(o)).abs() > 12 || !room {
            continue;
        }
        *sim = trial;
        return Some(id);
    }
    None
}

/// Fights the enemy `id` with the sword: walks up to it, faces it and
/// attacks (crouching for a rat) until it is seen dead of a hit, it is
/// lost some other way, or `ticks` ran out. Returns the tick it died on.
fn fight(sim: &mut Sim, id: ObjectId, rat: bool, ticks: usize) -> Option<usize> {
    let mut since_press = 0;
    for t in 0..ticks {
        match outcome(sim, id) {
            Outcome::Killed => return Some(t),
            Outcome::Lost => return None,
            Outcome::Alive => {}
        }
        let (c, o) = (sim.player().unwrap(), sim.objects().get(id).unwrap());
        let dx = o.x - c.x;
        let toward = if dx > 0 { RIGHT } else { LEFT };
        let facing = sim.claw().facing_right == (dx > 0);
        let input = if dx.abs() > 55 || (dx.abs() > 25 && !facing) {
            since_press = 0;
            TickInput {
                held: Buttons(toward),
                pressed: Buttons(toward),
            }
        } else if !facing {
            TickInput {
                held: Buttons(toward),
                pressed: Buttons(toward),
            }
        } else if !sim.claw().attacking && since_press > 2 {
            since_press = 0;
            let held = if rat { DOWN | ATTACK } else { ATTACK };
            TickInput {
                held: Buttons(held),
                pressed: Buttons(ATTACK),
            }
        } else {
            since_press += 1;
            TickInput {
                held: Buttons(if rat { DOWN } else { 0 }),
                pressed: Buttons::NONE,
            }
        };
        sim.step(input);
    }
    // A kill on the last tick of the budget counts too.
    match outcome(sim, id) {
        Outcome::Killed => Some(ticks),
        _ => None,
    }
}

/// Strikes at the enemy `id` from the air: Claw is put beside it at its
/// height, facing it, and swipes as he falls past. Tried from both sides
/// at a few heights; returns the tick of the try that killed it (seen dead
/// of a hit, as in [`fight`]).
fn strike_from_the_air(sim: &mut Sim, id: ObjectId, ticks: usize) -> Option<usize> {
    let mut used = 0;
    for rise in [10, 20, 0, 30] {
        for side in [-1, 1] {
            match outcome(sim, id) {
                Outcome::Killed => return Some(used),
                Outcome::Lost => return None,
                Outcome::Alive => {}
            }
            let o = sim.objects().get(id)?;
            let (x, y) = (o.x, o.y);
            sim.teleport_player(x + side * 55, y - rise);
            let toward = if side < 0 { RIGHT } else { LEFT };
            sim.step(TickInput {
                held: Buttons(toward),
                pressed: Buttons(toward),
            });
            sim.step(TickInput {
                held: Buttons(toward | ATTACK),
                pressed: Buttons(ATTACK),
            });
            for _ in 0..20 {
                used += 1;
                match outcome(sim, id) {
                    Outcome::Killed => return Some(used),
                    Outcome::Lost => return None,
                    Outcome::Alive => {}
                }
                sim.step(TickInput::default());
            }
            if used >= ticks {
                return None;
            }
        }
    }
    None
}

#[test]
fn install_every_enemy_can_be_killed_with_the_sword() {
    let Some(level) = level1() else { return };
    let base = Sim::new(level.clone(), SimConfig::default());
    let mut failed = Vec::new();
    let mut slowest = 0;
    let mut from_the_air = 0;
    for (logic, x, y, z) in enemy_records(&level) {
        let rat = logic == "Rat";
        let mut sim = base.clone();
        let placed = place(&mut sim, (x, y, z), rat);
        let id = match placed {
            Some(id) => id,
            None => {
                // No floor beside it Claw can stand on (an alcove in a
                // wall): wake it, then strike from the air. A few ticks
                // only: below the alcove there may be spikes.
                sim.teleport_player(x - 55, y - 10);
                for _ in 0..5 {
                    sim.step(TickInput::default());
                }
                match enemy_from(&sim, x, y, z) {
                    Some(id) => id,
                    None => {
                        failed.push(format!("{logic} at ({x}, {y}): never woke"));
                        continue;
                    }
                }
            }
        };
        let health = sim.objects().get(id).unwrap().health;
        assert_eq!(
            health,
            if rat { 1 } else { 3 },
            "{logic} at ({x}, {y}): health on the first level"
        );
        let killed = if placed.is_some() {
            fight(&mut sim, id, rat, 1500)
        } else {
            from_the_air += 1;
            strike_from_the_air(&mut sim, id, 400)
        };
        match killed {
            Some(t) => slowest = slowest.max(t),
            None => {
                let c = sim.player().unwrap();
                let at = format!("Claw at ({}, {}) health {}", c.x, c.y, sim.stats().health);
                match sim.objects().get(id) {
                    Some(o) if outcome(&sim, id) == Outcome::Alive => failed.push(format!(
                        "{logic} at ({x}, {y}): alive after the fight, at ({}, {}) in {:?}, {at}",
                        o.x,
                        o.y,
                        EnemyState::from_id(o.state),
                    )),
                    Some(o) => failed.push(format!(
                        "{logic} at ({x}, {y}): dead of {:?}, not of a hit, at ({}, {}), {at}",
                        enemy::data_of(o).map(|d| d.death),
                        o.x,
                        o.y,
                    )),
                    None => failed.push(format!(
                        "{logic} at ({x}, {y}): removed without being seen dead of a hit, {at}"
                    )),
                }
            }
        }
    }
    eprintln!("slowest kill: {slowest} ticks; {from_the_air} struck from the air");
    assert!(failed.is_empty(), "{failed:#?}");
}

/// Every enemy's position and state, and the random stream.
fn enemies_line(sim: &Sim) -> String {
    let mut s = format!("rng {};", sim.rng().state());
    for (id, o) in sim.objects().all() {
        if is_enemy(o) {
            s.push_str(&format!("{id}:{},{},{},{};", o.x, o.y, o.state, o.health));
        }
    }
    s
}

#[test]
fn install_two_idle_minutes_with_every_enemy_awake() {
    let Some(level) = level1() else { return };
    let mut sim = Sim::new(level.clone(), all_awake(&level));
    let (w, h) = (level.map.width(), level.map.height());
    let ticks = 2 * 60 * 1000 / 18;
    let mut costs = Vec::with_capacity(ticks);
    let mut outside = Vec::new();
    let mut states: BTreeMap<&str, usize> = BTreeMap::new();
    // Per walking rat: ticks in a row walking one way with a speed the
    // other way (held against a limit), the longest such run, and how
    // often it turned.
    let mut held: BTreeMap<ObjectId, (usize, usize, usize, f64)> = BTreeMap::new();
    for t in 0..ticks {
        let t0 = Instant::now();
        sim.step(TickInput::default());
        costs.push(t0.elapsed().as_secs_f64() * 1e6);
        for (id, o) in sim.objects().all() {
            if o.kind == LogicKind::Game(GameLogic::Rat)
                && let Some(d) = enemy::data_of(o)
                && !d.stationary
            {
                let e = held.entry(id).or_default();
                let pinned = match EnemyState::from_id(o.state) {
                    Some(EnemyState::MoveLeft) => d.vx > 0.0,
                    Some(EnemyState::MoveRight) => d.vx < 0.0,
                    _ => false,
                };
                e.0 = if pinned { e.0 + 1 } else { 0 };
                e.1 = e.1.max(e.0);
                if d.vx != 0.0 {
                    if e.3 != 0.0 && d.vx.signum() != e.3.signum() {
                        e.2 += 1;
                    }
                    e.3 = d.vx;
                }
            }
            if is_enemy(o) {
                if o.x < 0 || o.y < 0 || o.x >= w || o.y >= h {
                    outside.push(format!("tick {t}: {} at ({}, {})", o.logic, o.x, o.y));
                }
                if t + 1 == ticks
                    && let Some(s) = EnemyState::from_id(o.state)
                {
                    *states.entry(s.name()).or_default() += 1;
                }
            }
        }
        if outside.len() > 20 {
            break;
        }
    }
    let live = sim
        .objects()
        .listed()
        .iter()
        .filter(|&&id| sim.objects().get(id).is_some_and(is_enemy))
        .count();
    costs.sort_by(f64::total_cmp);
    let pct = |p: f64| costs[((costs.len() as f64 * p) as usize).min(costs.len() - 1)];
    eprintln!(
        "{live} enemies live; tick cost p50 {:.1} us, p99 {:.1} us, max {:.1} us; states at the end {states:?}",
        pct(0.5),
        pct(0.99),
        pct(1.0)
    );
    assert!(live >= 54, "every enemy is live: {live}");
    assert!(
        outside.is_empty(),
        "enemies outside the level: {outside:#?}"
    );
    // A walking rat turns at a wall as at a ledge: it is never held
    // against a limit walking into it, and keeps walking back and forth.
    let rats: Vec<String> = held
        .iter()
        .map(|(id, (_, longest, turns, _))| format!("{id}: held {longest}, turns {turns}"))
        .collect();
    eprintln!("walking rats: {rats:?}");
    let stuck: Vec<&String> = held
        .iter()
        .zip(&rats)
        .filter(|((_, (_, longest, turns, _)), _)| *longest > 2 || *turns < 10)
        .map(|(_, r)| r)
        .collect();
    assert!(stuck.is_empty(), "walking rats held on a limit: {stuck:#?}");
    // The enemies' own share: every enemy's logic call, once per round.
    let ids: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| is_enemy(o))
        .map(|(id, _)| id)
        .collect();
    let mut rounds = Vec::new();
    for _ in 0..500 {
        let t0 = Instant::now();
        sim.with_world(|w| {
            for &id in &ids {
                claw_sim::logic::tick(w, id);
            }
        });
        rounds.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    rounds.sort_by(f64::total_cmp);
    eprintln!(
        "the {} enemies' logic calls alone: p50 {:.1} us, p99 {:.1} us per tick",
        ids.len(),
        rounds[rounds.len() / 2],
        rounds[rounds.len() * 99 / 100]
    );
}

fn trace_at_rate(level: &Arc<SimLevel>, script: &InputScript, hz: u64) -> String {
    let mut sim = Sim::new(level.clone(), all_awake(level));
    let mut latch = InputLatch::new();
    let events = script.events();
    let (mut next_event, mut tick, mut frame) = (0, 0u64, 0u64);
    let n = script.ticks.len() as u64;
    let mut out = String::new();
    while tick < n {
        let now_us = frame * 1_000_000 / hz;
        while next_event < events.len() && events[next_event].0 <= now_us {
            let (_, b, down) = events[next_event];
            latch.event(now_us, b, down);
            next_event += 1;
        }
        while tick < n && (tick + 1) * claw_sim::input::TICK_US <= now_us {
            sim.step(latch.take(tick));
            out.push_str(&trace::to_csv(&[TraceRow::capture(&sim, tick, "script")]));
            out.push_str(&enemies_line(&sim));
            out.push('\n');
            tick += 1;
        }
        frame += 1;
    }
    out
}

#[test]
fn install_enemy_traces_are_identical_at_60_144_and_240_hz() {
    let Some(level) = level1() else { return };
    let mut held = Vec::new();
    for (mask, n) in [
        (0u32, 60),
        (RIGHT, 160),
        (ATTACK, 2),
        (0, 20),
        (RIGHT, 120),
        (ATTACK, 2),
        (0, 30),
        (LEFT, 50),
        (DOWN, 30),
        (DOWN | ATTACK, 2),
        (0, 60),
        (RIGHT, 200),
        (0, 200),
    ] {
        held.extend(std::iter::repeat_n(mask, n));
    }
    let script = InputScript::from_held(&held);
    let base = trace_at_rate(&level, &script, 1000);
    for hz in [60, 144, 240] {
        assert_eq!(trace_at_rate(&level, &script, hz), base, "{hz} Hz");
    }
}
