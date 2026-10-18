//! The fourth level's boss fight on your own install: her defeat in a
//! scripted fight, the gem it releases and the end of the level when
//! Claw collects it with a plain jump; then the boss room's routes played
//! against the reference traces of the same routes, in the same trace
//! format: the jump into the door with the introduction, the jump-in entry
//! that skips it (with Right, and straight up), the walk back into the
//! stager's box after a skip, the late trigger, lifting Katherine (after
//! the introduction and after a skip), the fight with and without the
//! introduction, and the end of a life in the middle of the fight.
//!
//! Each route starts under the door into her clearing: Claw is put there
//! on the level's first tick and left there until the route's time zero
//! (828 ms of game time, as in the reference runs, so the frame pulse and
//! the level's timed ambient sounds are in the same phase), then the
//! random stream is set to the store's state at time zero. The game then
//! plays the route tick by tick and its trace is compared with each
//! reference trace of the route's clocks: tick for tick, the random stream
//! included, at the original speeds (c18); within the feel tolerances at
//! the default speeds (c60). On every respawn of Claw at c18 she must be
//! back at her place on that same step.
//!
//! When the reference runs also list Katherine's position, state and
//! health per step (`boss.csv` beside each run, in the folder named by
//! `CLAW_REFERENCE_RUNS`, by default `rt` next to the reference store),
//! hers are compared too.
//!
//! Needs `CLAW_INSTALL`, and `CLAW_REFERENCE` for the comparison; each
//! test skips itself when what it needs is unset. Nothing from an install
//! or a reference store is kept in the repository.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{AudioCue, Buttons, GameEvent};
use claw_sim::input::{DirectiveKind, ScriptCommand, ScriptTuning};
use claw_sim::logic::enemy::{EnemyState, katherine};
use claw_sim::logic::pickup;
use claw_sim::logic::world_boss::{AMULET_SOUND, fight};
use claw_sim::object::ObjectId;
use claw_sim::player::special::BOSS_END;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Comparison, Tolerances, TraceRow};
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, TickInput};

/// Where every route starts: under the door into her clearing.
const START: (i32, i32) = (16107, 1500);
/// Katherine on her floor at her place.
const HER_AT: (i32, i32) = (18586, 1769);

fn level4() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(4)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

fn reference_root() -> Option<PathBuf> {
    match std::env::var_os("CLAW_REFERENCE") {
        Some(p) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => {
            eprintln!("skipped: CLAW_REFERENCE is not set");
            None
        }
    }
}

/// The folder of the reference runs, if there is one.
fn runs_root(reference: &Path) -> Option<PathBuf> {
    let dir = match std::env::var_os("CLAW_REFERENCE_RUNS") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => reference.parent()?.join("rt"),
    };
    dir.is_dir().then_some(dir)
}

/// Claw put at `at` on the level's first tick and left to settle until
/// `t0_ms` of game time has passed since the level started (as in the
/// reference run, so the frame pulse and the level's timed ambient sounds
/// are in the same phase), then the random stream set to `seed`: the next
/// tick is the route's time zero.
fn start_at(sim: &mut Sim, at: (i32, i32), t0_ms: u64, seed: u32) {
    sim.teleport_player(at.0, at.1);
    for _ in 0..t0_ms.div_ceil(18) {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(seed));
}

/// A value of the meta's section for `clock`.
fn clock_value<'a>(meta: &'a str, clock: &str, key: &str) -> Option<&'a str> {
    let section = meta.split(&format!("\n[{clock}]")).nth(1)?;
    let section = section.split("\n[").next()?;
    meta_value(section, key)
}

fn meta_value<'a>(meta: &'a str, key: &str) -> Option<&'a str> {
    meta.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().trim_matches('"'))
}

/// The first run named in the meta's section for `clock`.
fn run_id(meta: &str, clock: &str) -> Option<String> {
    let section = meta.split(&format!("\n[{clock}]")).nth(1)?;
    let line = section
        .lines()
        .find(|l| l.trim_start().starts_with("run_ids"))?;
    let first = line.split('"').nth(1)?;
    Some(first.to_string())
}

/// Katherine on one tick: x, y, state and health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Her {
    x: i32,
    y: i32,
    state: EnemyState,
    health: i32,
}

fn find_her(sim: &Sim) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| katherine::NAMES.contains(&&*o.logic))
        .map(|(id, _)| id)
}

fn her(sim: &Sim) -> Option<Her> {
    let id = find_her(sim)?;
    let o = sim.objects().get(id)?;
    Some(Her {
        x: o.x,
        y: o.y,
        state: EnemyState::from_id(o.state)?,
        health: o.health,
    })
}

/// The product state a reference run's state name stands for.
fn state_named(name: &str) -> Option<EnemyState> {
    Some(match name {
        "LS_IDLE" => EnemyState::Idle,
        "LS_HOME" => EnemyState::Home,
        "LS_STRIKE" => EnemyState::Strike,
        "LS_PACING" => EnemyState::Pace,
        "LS_HITLOW" => EnemyState::HitLow,
        "LS_HITHIGH" => EnemyState::HitHigh,
        "LS_DEAD" => EnemyState::Dead,
        "LS_PAUSE" => EnemyState::Pause,
        "LS_FALLING" => EnemyState::Falling,
        "LS_ADVANCE" => EnemyState::Advance,
        "LS_FASTADVANCE" => EnemyState::FastAdvance,
        "LS_LIFTED" => EnemyState::Lifted,
        "LS_FLAILING" => EnemyState::Flailing,
        "LS_THROWN" => EnemyState::Thrown,
        "LS_BOUNCE" => EnemyState::Bounce,
        "LS_DROPPED" => EnemyState::Dropped,
        "LS_STARTUP" => EnemyState::Startup,
        "LS_MOVEBACK" => EnemyState::MoveBack,
        "LS_BLOCKLOW" => EnemyState::BlockLow,
        "LS_BLOCKHIGH" => EnemyState::BlockHigh,
        "LS_JUMP" => EnemyState::Flip,
        "LS_FLIP3" => EnemyState::FlipLand,
        _ => return None,
    })
}

/// Katherine per step of a reference run: (t_ms, her), from its
/// `boss.csv`.
fn reference_her(path: &Path) -> Option<Vec<(u64, Option<Her>)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| header.iter().position(|h| *h == name);
    let (t, name, x, y, health) = (
        col("t_ms")?,
        col("boss_state_name")?,
        col("boss_x")?,
        col("boss_y")?,
        col("boss_health")?,
    );
    let mut rows = Vec::new();
    for line in lines {
        let cells: Vec<&str> = line.split(',').collect();
        let t_ms: u64 = cells.get(t)?.parse().ok()?;
        let her = (|| {
            Some(Her {
                x: cells.get(x)?.parse().ok()?,
                y: cells.get(y)?.parse().ok()?,
                state: state_named(cells.get(name)?)?,
                health: cells.get(health)?.parse().ok()?,
            })
        })();
        rows.push((t_ms, her));
    }
    Some(rows)
}

/// One route of the store played in the game.
struct Played {
    rows: Vec<TraceRow>,
    her: Vec<Option<Her>>,
}

/// Plays the store's route at `clock` for as long as `until_ms`, from
/// [`START`] with the store's random state at time zero.
fn play(
    level: &SimLevel,
    route: &str,
    (seed, t0_ms): (u32, u64),
    clock: &str,
    until_ms: u64,
) -> Played {
    let script = InputScript::parse(route).unwrap();
    let tuning = if clock == "c18" {
        ScriptTuning::Original
    } else {
        ScriptTuning::Default
    };
    let cfg = SimConfig {
        tuning: tuning.tuning(),
        seed,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level.clone(), cfg);
    start_at(&mut sim, START, t0_ms, seed);
    let ticks = script.ticks.len().max(until_ms.div_ceil(18) as usize);
    let mut directives = script.directives().iter().peekable();
    let mut rows = Vec::new();
    let mut hers = Vec::new();
    for tick in 0..ticks {
        while let Some(d) = directives.next_if(|d| d.tick <= tick) {
            if let DirectiveKind::Command(ScriptCommand::EndLife) = d.kind {
                sim.end_life();
            }
        }
        sim.step(script.ticks.get(tick).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, tick as u64, "script"));
        hers.push(her(&sim));
    }
    Played { rows, her: hers }
}

/// The first reference time (ms) Katherine's x, y, state and health leave
/// the reference run's, each by column: exact for c18; for c60 positions
/// within 8 px and states and health within two ticks of the reference.
fn compare_her(
    played: &Played,
    reference: &[(u64, Option<Her>)],
    clock: &str,
) -> BTreeMap<&'static str, u64> {
    let exact = clock == "c18";
    let at = |t: u64| -> Option<Option<Her>> {
        let i = (t.saturating_sub(1) / 18) as usize;
        played.her.get(i).copied()
    };
    let near = |t: u64, f: &dyn Fn(Option<Her>) -> bool| -> bool {
        if exact {
            return at(t).is_some_and(f);
        }
        (0..=4).any(|k| {
            let s = t.saturating_sub(36) + 18 * k;
            at(s).is_some_and(f)
        })
    };
    let mut first: BTreeMap<&'static str, u64> = BTreeMap::new();
    for &(t, r) in reference {
        let Some(mine) = at(t) else { break };
        let pos_ok = |a: i32, b: i32| if exact { a == b } else { (a - b).abs() <= 8 };
        let checks: [(&'static str, bool); 5] = match (r, mine) {
            (None, None) => [("present", true); 5],
            (Some(r), Some(m)) => [
                ("present", true),
                (
                    "her_x",
                    pos_ok(r.x, m.x)
                        || (!exact && near(t, &|h| h.is_some_and(|h| pos_ok(h.x, r.x)))),
                ),
                (
                    "her_y",
                    pos_ok(r.y, m.y)
                        || (!exact && near(t, &|h| h.is_some_and(|h| pos_ok(h.y, r.y)))),
                ),
                (
                    "her_state",
                    near(t, &|h| h.is_some_and(|h| h.state == r.state)),
                ),
                (
                    "her_health",
                    near(t, &|h| h.is_some_and(|h| h.health == r.health)),
                ),
            ],
            _ => [("present", false); 5],
        };
        for (name, ok) in checks {
            if !ok {
                first.entry(name).or_insert(t);
            }
        }
    }
    first
}

/// The first divergence of every column of one comparison, by column.
fn trace_firsts(cmp: &Comparison) -> BTreeMap<String, u64> {
    cmp.per_column
        .iter()
        .filter_map(|c| c.first_t_ms.map(|t| (c.column.clone(), t)))
        .collect()
}

/// The boss room's reference routes with the clocks each is compared at.
const STORES: &[(&str, &[&str])] = &[
    ("boss-intro-normal", &["c18", "c60"]),
    ("boss-intro-skip-c18", &["c18"]),
    ("boss-intro-skip", &["c60"]),
    ("boss-intro-skip-return-c18", &["c18"]),
    ("boss-intro-skip-up-c18", &["c18"]),
    ("boss-intro-normal-lift", &["c18", "c60"]),
    ("boss-intro-normal-end-life", &["c18", "c60"]),
    ("boss-intro-normal-fight", &["c18"]),
    ("boss-intro-skip-fight-c18", &["c18"]),
    ("boss-intro-skip-late-c18", &["c18"]),
    ("boss-intro-skip-lift-c18", &["c18"]),
];

/// A store, a clock, and the first divergence (ms) of each column known
/// there; a column not listed must stay within its tolerance, and one
/// listed must not leave it earlier. What they come from:
///
/// - c18: the walk through the door, the whole introduction, her fight
///   (whips, stabs, somersaults, blocks, flinches, steps back), lifting
///   and throwing her, the jump-in skip, the late trigger after it, the
///   fight after a skip and her blows that end Claw's freeze follow the
///   original step for step, the random stream included, and so do
///   Claw's own death fall, the camera after his respawn and her reset
///   when he respawns, near her clearing or far from it.
/// - c18 end of life: the in-game menu opened before the life ends puts
///   the original's frame pulse a tick earlier from then on (14418 ms);
///   the replay plays no time for that menu, so her rolls come a tick
///   later from there. Her reset on the respawn step still holds.
/// - c60 (the default tuning played tick by tick): the introduction's
///   delays count 18 ms ticks against frames of 16 and 17 ms, so the flag
///   reaches 2 about 350 ms later (10000 ms) and her fight comes later
///   with it; the jump-in's arc is up to 9 px lower (750 ms), and she
///   still wakes through the 500 ms rule within two ticks of the
///   original, with Claw landing on her.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

const KNOWN: &[Known] = &[
    (
        "boss-intro-normal",
        "c60",
        &[
            ("boss_fight", 10000),
            ("her_state", 10017),
            ("state_id", 10017),
            ("health", 10367),
            ("cam_x", 10417),
            ("x", 10417),
            ("her_x", 11900),
            ("physics_type", 33617),
            ("y", 34433),
            ("events", 35300),
            ("lives", 35300),
        ],
    ),
    (
        "boss-intro-skip",
        "c60",
        &[
            ("y", 750),
            ("cam_y", 750),
            ("cam_x", 1017),
            ("x", 1017),
            ("her_state", 1500),
            ("her_x", 1617),
            ("health", 1867),
            ("state_id", 1867),
            ("events", 7506),
            ("boss_fight", 7550),
            ("physics_type", 24167),
            ("lives", 25850),
        ],
    ),
    (
        "boss-intro-normal-lift",
        "c60",
        &[
            ("boss_fight", 10000),
            ("her_state", 10017),
            ("state_id", 10017),
            ("health", 10367),
            ("cam_x", 10417),
            ("x", 10417),
            ("physics_type", 12250),
            ("her_x", 12933),
            ("her_y", 12933),
            ("boss_health", 15150),
            ("her_health", 15150),
        ],
    ),
    (
        "boss-intro-normal-end-life",
        "c18",
        &[
            ("rng", 14418),
            ("her_state", 14940),
            ("cam_x", 16398),
            ("health", 16380),
            ("state_id", 16380),
            ("x", 16398),
            ("her_x", 18108),
        ],
    ),
    (
        "boss-intro-normal-end-life",
        "c60",
        &[
            ("boss_fight", 10000),
            ("her_state", 10017),
            ("state_id", 10017),
            ("health", 10367),
            ("cam_x", 10417),
            ("x", 10417),
            ("her_x", 11900),
            ("y", 15716),
        ],
    ),
];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
}

#[test]
fn install_level4_boss_room_against_the_reference_traces() {
    let Some(level) = level4() else { return };
    let Some(root) = reference_root() else { return };
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for &(store, clocks) in STORES {
        let dir = root.join("level4").join(store);
        let Ok(meta) = std::fs::read_to_string(dir.join("meta.toml")) else {
            println!("== level4/{store}: no store, skipped");
            continue;
        };
        let seed: u32 = meta_value(&meta, "rng_at_t0").unwrap().parse().unwrap();
        let route = std::fs::read_to_string(dir.join("route.csv")).unwrap();
        for &clock in clocks {
            let path = dir.join(format!("trace-{clock}-r1.csv"));
            let Ok(text) = std::fs::read_to_string(&path) else {
                println!("== level4/{store} {clock}: no trace, skipped");
                continue;
            };
            let reference = trace::parse(&text).unwrap();
            let until = reference.last().map_or(0, |r| r.t_ms);
            let t0_ms: u64 = clock_value(&meta, clock, "t0_ms").unwrap().parse().unwrap();
            let played = play(&level, &route, (seed, t0_ms), clock, until);
            let tol = if clock == "c18" {
                Tolerances::exact()
            } else {
                Tolerances::feel()
            };
            let cmp = trace::compare(&played.rows, &reference, &tol);
            // Claw's respawns: on that very step she is back at her place on
            // her floor with full health, as in the original.
            if clock == "c18" {
                for w in reference.windows(2) {
                    if w[0].health > 0 || w[1].health <= 0 {
                        continue;
                    }
                    let mine = played.her.get(w[1].tick as usize).copied().flatten();
                    println!("respawn on step {}: Katherine {mine:?}", w[1].tick);
                    let mine = mine.expect("Katherine");
                    assert_eq!((mine.x, mine.y, mine.health), (HER_AT.0, HER_AT.1, 100));
                }
            }
            println!("== level4/{store} {clock}");
            print!("{}", trace::report(&played.rows, &reference, &cmp, 3));
            assert!(cmp.rows_compared > 0);
            let mut firsts = trace_firsts(&cmp);
            if let Some(her_ref) = runs
                .as_ref()
                .and_then(|r| Some(r.join(run_id(&meta, clock)?).join("boss.csv")))
                .and_then(|p| reference_her(&p))
            {
                for (k, v) in compare_her(&played, &her_ref, clock) {
                    firsts.insert(k.to_string(), v);
                }
            }
            let list = known(store, clock);
            for (column, first) in &firsts {
                println!("first {store} {clock} {column} {first}");
                match list.iter().find(|(name, _)| name == column) {
                    Some(&(_, at)) if *first >= at => {}
                    _ => failures.push(format!("{store} {clock}: {column} at {first} ms")),
                }
            }
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ----------------------------------------------------------------------
// The fight to the gem

/// Katherine's gem target, from her level record: the gem lands on it or
/// 1 px short of it across, by where she fell.
const GEM_TARGET: (i32, i32) = (18551, 1596);
const ATTACK: u32 = 0x2;
const JUMP: u32 = 0x1;
const LEFT: u32 = 0x100_0000;
const RIGHT: u32 = 0x200_0000;

fn input(held: u32, pressed: u32) -> TickInput {
    TickInput {
        held: Buttons(held),
        pressed: Buttons(pressed),
    }
}

/// The scripted player of the fight, decided each tick from what is on
/// screen: walk at Katherine, turn to her and swing every eighth tick
/// once within 60 px; once the gem has landed, walk under it and jump
/// from the floor with Jump held.
fn fight_input(sim: &Sim, her: ObjectId, k: u64) -> TickInput {
    let c = sim.player().unwrap();
    let toward = |dx: i32| if dx > 0 { RIGHT } else { LEFT };
    if let Some(gx) = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == pickup::END_OF_LEVEL_LOGIC)
        .map(|(_, o)| o.x)
    {
        if !matches!(
            c.state,
            state::STAND | state::MOVE_RIGHT | state::MOVE_LEFT | state::JUMP | state::FALL
        ) {
            return TickInput::default();
        }
        let dx = gx - c.x;
        if dx.abs() > 4 {
            return input(toward(dx), 0);
        }
        let press = if c.state == state::STAND { JUMP } else { 0 };
        return input(JUMP, press);
    }
    match sim
        .objects()
        .get(her)
        .and_then(|o| EnemyState::from_id(o.state).map(|s| (o.x, s)))
    {
        Some((x, s)) if s != EnemyState::Dead && c.state != BOSS_END => {
            let dx = x - c.x;
            if dx.abs() > 60 {
                input(toward(dx), 0)
            } else if (dx > 0) != sim.claw().facing_right {
                input(toward(dx), toward(dx))
            } else if k.is_multiple_of(8) {
                input(ATTACK, ATTACK)
            } else {
                TickInput::default()
            }
        }
        _ => TickInput::default(),
    }
}

#[test]
fn install_level4_her_defeat_releases_the_gem_and_a_plain_jump_ends_the_level() {
    let Some(level) = level4() else { return };
    let cfg = SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level, cfg);
    start_at(&mut sim, START, 828, 111_567);
    // Into her clearing through the door, and the introduction.
    for t in 0..600u64 {
        let i = match t {
            15 => input(JUMP, JUMP),
            16..=20 => input(JUMP, 0),
            _ => TickInput::default(),
        };
        sim.step(i);
    }
    assert_eq!(sim.boss().fight(), fight::ON);
    let her = sim.boss().current().expect("Katherine");
    // The fight: Claw kept on his feet, the bar following her health.
    let mut healths = vec![100];
    let mut bar = vec![100];
    let mut k = 0u64;
    let fell_at = loop {
        assert!(k < 20_000, "she falls");
        let i = fight_input(&sim, her, k);
        sim.step(i);
        sim.with_world(|w| {
            if w.claw.stats.health < 50 {
                w.claw.stats.health = 100;
            }
        });
        let o = sim.objects().get(her).unwrap().clone();
        if healths.last() != Some(&o.health) {
            healths.push(o.health);
        }
        if let Some(m) = sim.hud().boss_meter {
            assert_eq!(m.max, katherine::METER_MAX);
            if bar.last() != Some(&m.value) {
                bar.push(m.value);
            }
        }
        if EnemyState::from_id(o.state) == Some(EnemyState::Dead) {
            break (o.x, o.y);
        }
        k += 1;
    };
    println!("Katherine fell after {k} ticks of the fight at {fell_at:?}: health {healths:?}");
    assert!(healths.len() >= 5, "several blows");
    // The bar showed every health she had while she lived.
    assert_eq!(bar[..healths.len() - 1], healths[..healths.len() - 1]);
    // Her defeat: Claw held, the gem out of her and its sound.
    assert_eq!(sim.player().unwrap().state, BOSS_END);
    assert_eq!(sim.boss().defeated(), Some(her));
    assert!(
        sim.last_audio()
            .iter()
            .any(|c| matches!(c, AudioCue::Play(s) if s.key == AMULET_SOUND))
    );
    let gem = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == pickup::END_GEM)
        .map(|(id, _)| id)
        .expect("the gem");
    // It lands 1 px short of its target and Claw is let go.
    let mut n = 0;
    while &*sim.objects().get(gem).unwrap().logic != pickup::END_OF_LEVEL_LOGIC {
        sim.step(TickInput::default());
        n += 1;
        assert!(n < 400, "the gem lands");
    }
    let o = sim.objects().get(gem).unwrap();
    assert_eq!(o.y, GEM_TARGET.1);
    assert!((o.x - GEM_TARGET.0).abs() <= 1, "at its target: {}", o.x);
    assert_eq!(sim.player().unwrap().state, state::STAND);
    // A plain jump from her clearing's floor collects it.
    let floor_y = sim.player().unwrap().y;
    let mut top = floor_y;
    let mut done = false;
    for k in 0..600u64 {
        let i = fight_input(&sim, her, k);
        sim.step(i);
        top = top.min(sim.player().unwrap().y);
        if sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
        {
            done = true;
            break;
        }
    }
    println!(
        "the level ended with Claw {} px above the floor",
        floor_y - top
    );
    assert!(done, "the level ends");
    assert!(floor_y - top < 150, "a plain jump, no climb");
}
