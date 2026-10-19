//! The eleventh level's Siren and the lockout of her scream on your own
//! install, against the original's runs of the same short routes.
//!
//! Each route puts Claw down left of the Siren on the floor at 8559 after
//! the level's first tick, the way the original's runs put him there
//! (between two ticks, as a warp does), waits until the level clock
//! reaches the time the original's run started at, and starts the shared
//! random stream from the run's state, once. She was built on the
//! original's first tick and her wait at home started then, with a draw
//! the replay does not have: at time zero her wait is put where the run
//! shows it ([`align`]). The routes, Claw put down 139 px to her left
//! unless said otherwise:
//!
//! - siren-stand: he stands; her songs' screams lock him three times;
//! - siren-jump: he holds Jump from row 30: a jump, and a jump again on
//!   the first free row after each lockout;
//! - siren-walk: put down 214 px to her left, he holds Right from row 30:
//!   the walk stops on the first locked row and starts again on the first
//!   free one;
//! - siren-duck: he holds Down from row 20: the scream touches him
//!   crouching, the crouch rises while locked and comes back after;
//! - siren-air: he holds Jump on rows 62 to 101: the scream meets the
//!   rising jump, which loses its hold;
//! - siren-screens: the pause key on rows 80, 81 and 82, the help screen
//!   on row 120 and the in-game menu on row 160, all during the first
//!   lockout, which they stop for as long as they are up;
//! - siren-death: an end of life through the in-game menu on row 100,
//!   during the first lockout, which runs on through the respawn.
//!
//! Each route is played tick by tick and compared with the reference
//! trace of the same route: at the 18 ms step with the original's speeds
//! every column exactly (Claw's position and state, health, score, the
//! camera, the random stream and the events), at 60 frames per second
//! with the default tuning within the feel tolerances. The lockout is
//! compared too, with the counter the original's run reads after each
//! step (`probe.csv` beside the run, in the folder named by
//! `CLAW_REFERENCE_RUNS`, by default `rt` next to the reference store):
//! exactly at the 18 ms step, and at 60 frames per second its spans
//! within the feel tolerance's slack. At the 18 ms step so are the
//! Siren's position and state and whether Claw shows his dazed picture
//! (`obj-log.csv` there), her states lined up by the numbers the run
//! gives them ([`REFERENCE_STATES`]). While Mercat is not built the
//! random stream is taken from the run at every row ([`MERCAT_STREAM`]).
//! Divergences that are known, with their causes, are listed in
//! [`KNOWN`]; any other fails the test. The routes are written below;
//! each store's own copy of its route must agree with them. Needs
//! `CLAW_INSTALL`, and `CLAW_REFERENCE` for the comparisons: each test
//! skips itself when what it needs is unset, and fails when
//! `CLAW_REFERENCE` is set but a listed store lacks its route, meta or
//! trace, or its run lacks its probe or object log; `CLAW_L11_ONLY` names
//! one store to play alone. Nothing from an install or a reference store
//! is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{self, EnemyState, mercat, siren};
use claw_sim::logic::projectile::scream;
use claw_sim::object::ObjectId;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// One stretch of a route: from `from` to `to` (inclusive) `held` is down,
/// pressed on the stretch's first row.
#[derive(Debug, Clone, Copy)]
struct Span {
    from: usize,
    to: usize,
    held: u32,
}

const fn span(from: usize, to: usize, held: u32) -> Span {
    Span { from, to, held }
}

/// What a route does before one of its rows besides its keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hold {
    /// The pause key, for this long, ms.
    Pause(u32),
    /// The help screen, for this long, ms.
    Help(u32),
    /// The in-game menu, for this long, ms.
    Menu(u32),
    /// An end of life (taken from the in-game menu).
    EndLife,
}

impl Hold {
    /// The line a store's route writes for it.
    fn line(self, row: usize) -> String {
        match self {
            Hold::Pause(ms) => format!("@{row} pause {ms}"),
            Hold::Help(ms) => format!("@{row} menu help {ms}"),
            Hold::Menu(ms) => format!("@{row} menu ingame {ms}"),
            Hold::EndLife => format!("@{row} command end_life"),
        }
    }

    /// Carries it out before the row's tick, as the game does when play
    /// resumes after it.
    fn apply(self, sim: &mut Sim) {
        match self {
            Hold::Pause(_) => sim.restart_idle_wait(),
            Hold::Help(ms) => {
                sim.pass_menu_time(ms);
                sim.restart_idle_wait();
                sim.latch_keys();
            }
            Hold::Menu(_) => {
                sim.restart_idle_wait();
                sim.latch_keys();
            }
            Hold::EndLife => sim.end_life(),
        }
    }
}

/// A route of the stores: its name, where Claw is put down, how many rows
/// its own copy has, its stretches, its pauses, screens and commands, and
/// the clocks it was run at.
struct Route {
    store: &'static str,
    start: (i32, i32),
    rows: usize,
    spans: &'static [Span],
    holds: &'static [(usize, Hold)],
    clocks: &'static [&'static str],
}

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;
/// The original's game time at the routes' time zero, at the 18 ms step.
const T0_MS: u64 = 828;
/// Where the Siren stands.
const SIREN: (i32, i32) = (8559, 2783);
/// Where Claw is put down, 139 px to her left.
const NEAR: (i32, i32) = (8420, 2747);

const ROUTES: [Route; 7] = [
    Route {
        store: "siren-stand",
        start: NEAR,
        rows: 700,
        spans: &[],
        holds: &[],
        clocks: &["c18", "c60"],
    },
    Route {
        store: "siren-jump",
        start: NEAR,
        rows: 500,
        spans: &[span(30, 479, JUMP)],
        holds: &[],
        clocks: &["c18"],
    },
    Route {
        store: "siren-walk",
        start: (8345, 2747),
        rows: 500,
        spans: &[span(30, 479, RIGHT)],
        holds: &[],
        clocks: &["c18"],
    },
    Route {
        store: "siren-duck",
        start: NEAR,
        rows: 500,
        spans: &[span(20, 479, DOWN)],
        holds: &[],
        clocks: &["c18"],
    },
    Route {
        store: "siren-air",
        start: NEAR,
        rows: 400,
        spans: &[span(62, 101, JUMP)],
        holds: &[],
        clocks: &["c18"],
    },
    Route {
        store: "siren-screens",
        start: NEAR,
        rows: 500,
        spans: &[],
        holds: &[
            (80, Hold::Pause(300)),
            (81, Hold::Pause(300)),
            (82, Hold::Pause(300)),
            (120, Hold::Help(400)),
            (160, Hold::Menu(400)),
        ],
        clocks: &["c18"],
    },
    Route {
        store: "siren-death",
        start: NEAR,
        rows: 700,
        spans: &[],
        holds: &[(100, Hold::Menu(300)), (100, Hold::EndLife)],
        clocks: &["c18"],
    },
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. Besides the trace's
/// columns: `lock` (the lockout), `siren_x`, `siren_y`, `siren_state` and
/// `dazed` (Claw shows his dazed picture).
///
/// - The held keys (siren-jump, siren-walk, siren-duck, siren-air, from
///   the first locked row): the original's trace shows the keys its
///   controls read, which the lockout empties; the replay's trace shows
///   the keys it was given, which the lockout leaves as they are (it is
///   applied where they reach Claw).
/// - siren-screens: the original's return from the help screen runs every
///   object's logic once more, outside any step, and in that pass the
///   Siren's song ends 14 rows before its time is over (she goes home on
///   row 120, not 134); the game's return from the help screen runs no
///   logic but the dynamite fuses. From her early going home on, her
///   draws come earlier (the random stream at row 120), her next song
///   comes 39 rows earlier and so does the next lockout (row 308 against
///   347).
/// - siren-stand at 60 frames per second: the random stream is stepped a
///   different number of times at the two clocks, so her wait after her
///   first song is another draw and her second song, with its lockout,
///   comes at another time; the first lockout is within the slack.
const KNOWN: &[Known] = &[
    ("siren-stand", "c60", &[("lock", 6050)]),
    ("siren-jump", "c18", &[("held", 1188)]),
    ("siren-walk", "c18", &[("held", 1908)]),
    ("siren-duck", "c18", &[("held", 1170)]),
    ("siren-air", "c18", &[("held", 1188)]),
    (
        "siren-screens",
        "c18",
        &[
            ("rng", 2178),
            ("siren_state", 2178),
            ("lock", 5562),
            ("dazed", 5562),
        ],
    ),
];

/// The divergences Mercat, while not built, causes, per store and clock,
/// as [`KNOWN`]: the Mercat below the Siren's floor is awake in every
/// route and in the original's runs draws from the shared random stream
/// from the second row on (see [`MERCAT_STREAM`]).
const UNBUILT_MERCAT: &[Known] = &[
    ("siren-stand", "c18", &[("rng", 36)]),
    ("siren-jump", "c18", &[("rng", 36)]),
    ("siren-walk", "c18", &[("rng", 36)]),
    ("siren-duck", "c18", &[("rng", 36)]),
    ("siren-air", "c18", &[("rng", 36)]),
    ("siren-screens", "c18", &[("rng", 36)]),
    ("siren-death", "c18", &[("rng", 36)]),
];

fn known(store: &str, clock: &str) -> Vec<(&'static str, u64)> {
    let mut out: Vec<(&'static str, u64)> = KNOWN
        .iter()
        .filter(|(s, c, _)| *s == store && *c == clock)
        .flat_map(|(_, _, k)| k.iter().copied())
        .collect();
    if !mercat::BUILT {
        for (s, c, k) in UNBUILT_MERCAT {
            if *s == store && *c == clock {
                out.extend(k.iter().copied());
            }
        }
    }
    out
}

/// Whether the routes take the random stream from the original's runs:
/// while Mercat is not built, the Mercat below the Siren's floor, awake
/// in every route, makes none of his draws, so the stream would leave the
/// original's on the second row and with it every later draw of the
/// Siren's. Until he is built, each row's random stream starts from the
/// original's state after the row before (at the 18 ms step, where the
/// runs give it), so the Siren's own draws are made from the original's
/// stream; the stream itself is still compared and its divergence is
/// listed in [`UNBUILT_MERCAT`]. The route checks below need
/// `CLAW_REFERENCE` for it until then.
const MERCAT_STREAM: bool = !mercat::BUILT;

/// The original's random state after each row of a reference trace.
fn reference_stream(reference: &[TraceRow]) -> Vec<u32> {
    reference
        .iter()
        .map(|row| row.rng.expect("the reference trace has the random stream"))
        .collect()
}

/// The numbers the reference runs give the Siren's states, where they
/// differ from the states' own numbers.
const REFERENCE_STATES: [(i32, EnemyState); 9] = [
    (2000, EnemyState::Idle),
    (2004, EnemyState::Dead),
    (2009, EnemyState::Lifted),
    (2018, EnemyState::Home),
    (2021, EnemyState::Advance),
    (2023, EnemyState::Pace),
    (2024, EnemyState::HitLow),
    (2025, EnemyState::HitHigh),
    (2101, EnemyState::Singing),
];

/// The state a reference run's number stands for.
fn reference_state(id: i32) -> Option<EnemyState> {
    REFERENCE_STATES
        .iter()
        .find(|(n, _)| *n == id)
        .map(|&(_, s)| s)
}

fn route_named(store: &str) -> &'static Route {
    ROUTES
        .iter()
        .find(|r| r.store == store)
        .unwrap_or_else(|| panic!("no route {store}"))
}

fn route_ticks(r: &Route) -> Vec<TickInput> {
    let mut held = vec![0u32; r.rows];
    let mut pressed = vec![0u32; r.rows];
    for s in r.spans {
        for h in &mut held[s.from..=s.to] {
            *h |= s.held;
        }
        pressed[s.from] |= s.held;
    }
    held.iter()
        .zip(&pressed)
        .map(|(&h, &p)| TickInput {
            held: Buttons(h),
            pressed: Buttons(p),
        })
        .collect()
}

/// A store's own copy of a route: (held, pressed) by row, and its `@`
/// lines.
type StoreRoute = (Vec<(u32, u32)>, Vec<String>);

/// The rows of a store's own copy of a route: (held, pressed) by row, and
/// its `@` lines.
fn store_route(dir: &Path) -> Option<StoreRoute> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).unwrap();
    let rows = text
        .lines()
        .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            (hex(f[1]), hex(f[2]))
        })
        .collect();
    let holds = text
        .lines()
        .filter(|l| l.starts_with('@'))
        .map(|l| l.trim().to_string())
        .collect();
    Some((rows, holds))
}

/// A value of a store's `meta.toml`: `key` in the table `[section]`, or
/// at the top when `section` is empty.
fn meta_value(meta: &str, section: &str, key: &str) -> Option<String> {
    let mut current = String::new();
    for line in meta.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = name.to_string();
        } else if current == section
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn level11() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(11)).unwrap();
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

/// The folder of the reference runs.
fn runs_root(reference: &Path) -> PathBuf {
    match std::env::var_os("CLAW_REFERENCE_RUNS") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => reference
            .parent()
            .expect("the reference store has a parent")
            .join("rt"),
    }
}

/// The first run of `clock` a store's meta names.
fn run_id(meta: &str, clock: &str) -> Option<String> {
    let line = meta_value(meta, clock, "run_ids")?;
    Some(
        line.trim_start_matches('[')
            .split('"')
            .find(|p| !p.is_empty())?
            .to_string(),
    )
}

/// The Siren's wait at home at time zero, by clock: when it started, on
/// the replay's clock, and how long it is. The original built her in its
/// first step, at game time 18 (17 at 60 frames per second), and drew a
/// wait of 1327 ms; its game time runs 18 ms (7 ms at 60 frames per
/// second) ahead of the replay's at the same row.
fn wait_at_time_zero(clock: &str) -> enemy::Timer {
    if clock == "c18" {
        enemy::Timer::new(0, 1327)
    } else {
        enemy::Timer::new(10, 1327)
    }
}

/// Puts the Siren `id` where the original's run shows her at time zero:
/// at home, her wait started when she was built (see
/// [`wait_at_time_zero`]), and not counting Claw out of her range. The
/// replay builds her between its first two ticks, where she is not drawn
/// yet and so cannot see Claw on her first call; the original built her
/// inside its first tick and she saw him from her first call on.
fn align(sim: &mut Sim, id: ObjectId, clock: &str) {
    sim.with_world(|w| {
        let o = w.objects.get(id).expect("the Siren");
        assert_eq!((o.x, o.y), SIREN, "the Siren stands where the run shows");
        assert_eq!(enemy::state(w, id), Some(EnemyState::Home));
        if let Some(s) = siren::siren_mut(w, id) {
            s.wait = wait_at_time_zero(clock);
        }
        if let Some(d) = enemy::data_mut(w, id) {
            d.out_of_range = false;
            d.miss_since = None;
            d.deferred = None;
        }
    });
}

fn find_siren(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == siren::NAMES[0])
        .min_by_key(|(_, o)| (o.x - SIREN.0).abs() + (o.y - SIREN.1).abs())
        .map(|(id, _)| id)
        .expect("the Siren")
}

/// The game at a route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, clock: &str, r: &Route, t0_ms: u64, seed: u32) -> Sim {
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
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some(r.start));
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
    }
    let id = find_siren(&sim);
    align(&mut sim, id, clock);
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// A store's time zero and random state for `clock`.
fn time_zero(dir: &Path, clock: &str) -> Option<(u64, u32)> {
    let meta = std::fs::read_to_string(dir.join("meta.toml")).ok()?;
    let t0 = meta_value(&meta, clock, "t0_ms")?.parse().ok()?;
    let seed = meta_value(&meta, clock, "rng_at_t0")
        .or_else(|| meta_value(&meta, "", "rng_at_t0"))?
        .parse()
        .ok()?;
    Some((t0, seed))
}

/// The Siren on a row: her position and state.
type SirenAt = (i32, i32, Option<EnemyState>);

/// A row of a run's object log: the row, the Siren (when logged) and
/// whether Claw shows his dazed picture.
type RunRow = (usize, Option<SirenAt>, bool);

/// What one row shows besides the trace: the lockout, the Siren, and
/// whether Claw shows his dazed picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Extra {
    lock: u32,
    siren: Option<SirenAt>,
    dazed: bool,
}

/// One play of a route.
struct Played {
    rows: Vec<TraceRow>,
    reference: Vec<TraceRow>,
    extra: Vec<Extra>,
}

/// Plays route `r` at `clock` from its time zero for `rows` rows (with the
/// original's random state there), calling `each` after every row. With
/// `stream`, the original's random state after each row, the random
/// stream starts every row from the original's state after the row before
/// (see [`MERCAT_STREAM`]).
#[allow(clippy::too_many_arguments)]
fn play_rows(
    level: &SimLevel,
    r: &Route,
    clock: &str,
    t0: u64,
    seed: u32,
    rows: usize,
    stream: Option<&[u32]>,
    mut each: impl FnMut(usize, &Sim, ObjectId),
) {
    let mut sim = at_time_zero(level, clock, r, t0, seed);
    let id = find_siren(&sim);
    let ticks = route_ticks(r);
    for k in 0..rows {
        if k > 0
            && let Some(&state) = stream.and_then(|s| s.get(k - 1))
        {
            sim.with_world(|w| *w.rng = Rng::new(state));
        }
        for &(row, hold) in r.holds {
            if row == k {
                hold.apply(&mut sim);
            }
        }
        sim.step(ticks.get(k).copied().unwrap_or_default());
        each(k, &sim, id);
    }
}

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs. A store without the trace or its meta
/// fails the test.
fn play(level: &SimLevel, root: &Path, r: &Route, clock: &str) -> Played {
    let dir = root.join("level11").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if clock == "c18" {
        assert_eq!(t0, T0_MS, "{}: time zero", r.store);
        assert_eq!(seed, SEED, "{}: random state", r.store);
    }
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let n = end_ms.div_ceil(TICK) as usize;
    let stream = (MERCAT_STREAM && clock == "c18").then(|| reference_stream(&reference));
    let mut rows = Vec::new();
    let mut extra = Vec::new();
    play_rows(
        level,
        r,
        clock,
        t0,
        seed,
        n,
        stream.as_deref(),
        |k, sim, id| {
            rows.push(TraceRow::capture(sim, k as u64, "script"));
            extra.push(extra_of(sim, id));
        },
    );
    Played {
        rows,
        reference,
        extra,
    }
}

fn extra_of(sim: &Sim, id: ObjectId) -> Extra {
    Extra {
        lock: sim.scream_lock_ms(),
        siren: sim
            .objects()
            .get(id)
            .map(|o| (o.x, o.y, EnemyState::from_id(o.state))),
        dazed: sim.player().is_some_and(|o| o.frame == scream::DAZED_FRAME),
    }
}

/// The run's rows: the row, its time (ms) and the lockout after the step
/// (`probe.csv`).
fn reference_lock(run: &Path) -> Option<Vec<(usize, u64, u32)>> {
    let text = std::fs::read_to_string(run.join("probe.csv")).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, t_ms, lock) = (col("tick")?, col("t_ms")?, col("siren_ms")?);
    lines
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            Some((
                f.get(tick)?.parse().ok()?,
                f.get(t_ms)?.parse().ok()?,
                f.get(lock)?.parse().ok()?,
            ))
        })
        .collect()
}

/// The spans (first, last) of times (ms) at which the lockout runs, from
/// rows of (time, lockout).
fn lock_spans(rows: impl Iterator<Item = (u64, u32)>) -> Vec<(u64, u64)> {
    let mut out: Vec<(u64, u64)> = Vec::new();
    let mut open = false;
    for (t, lock) in rows {
        match (lock > 0, open) {
            (true, false) => out.push((t, t)),
            (true, true) => {
                if let Some(last) = out.last_mut() {
                    last.1 = t;
                }
            }
            _ => {}
        }
        open = lock > 0;
    }
    out
}

/// At 60 frames per second: the first reference time (ms) at which the
/// lockout's spans leave the run's by more than the feel tolerance's
/// slack for stats.
fn compare_lock_feel(played: &[Extra], lock: &[(usize, u64, u32)]) -> Option<u64> {
    let slack = u64::from(Tolerances::feel().stats_slack_ms);
    let ours = lock_spans(
        played
            .iter()
            .enumerate()
            .map(|(k, e)| ((k as u64 + 1) * TICK, e.lock)),
    );
    let theirs = lock_spans(lock.iter().map(|&(_, t, l)| (t, l)));
    for (i, &(a, b)) in theirs.iter().enumerate() {
        let Some(&(oa, ob)) = ours.get(i) else {
            return Some(a);
        };
        if oa.abs_diff(a) > slack {
            return Some(a.min(oa));
        }
        if ob.abs_diff(b) > slack {
            return Some(b.min(ob));
        }
    }
    (ours.len() > theirs.len()).then(|| ours[theirs.len()].0)
}

/// The run's rows, by row: the Siren nearest her place (position and
/// state) and whether Claw shows his dazed picture (`obj-log.csv`).
fn reference_objects(run: &Path) -> Option<Vec<RunRow>> {
    let text = std::fs::read_to_string(run.join("obj-log.csv")).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, logic, st, x, y, frame) = (
        col("tick")?,
        col("logic_name")?,
        col("state")?,
        col("x")?,
        col("y")?,
        col("frame")?,
    );
    let int = |s: &str| s.trim().parse::<i32>().ok();
    let mut out: Vec<RunRow> = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        let k: usize = f.get(tick)?.parse().ok()?;
        while out.len() <= k {
            out.push((out.len(), None, false));
        }
        match f.get(logic).copied() {
            Some("CaptainClaw") => out[k].2 = int(f[frame])? == scream::DAZED_FRAME,
            Some("Siren") => {
                let at = (int(f[x])?, int(f[y])?);
                let near = |p: (i32, i32)| (p.0 - SIREN.0).abs() + (p.1 - SIREN.1).abs();
                if out[k].1.is_none_or(|(ox, oy, _)| near(at) < near((ox, oy))) {
                    out[k].1 = Some((at.0, at.1, reference_state(int(f[st])?)));
                }
            }
            _ => {}
        }
    }
    Some(out)
}

/// The first reference time (ms) at which the replay leaves the run in
/// the extra columns (`lock`, `siren_x`, `siren_y`, `siren_state`,
/// `dazed`).
fn compare_extra(
    played: &[Extra],
    lock: &[(usize, u64, u32)],
    objects: &[RunRow],
) -> Vec<(&'static str, u64)> {
    let mut firsts: Vec<(&'static str, u64)> = Vec::new();
    let mut note = |name: &'static str, k: usize| {
        if !firsts.iter().any(|(n, _)| *n == name) {
            firsts.push((name, (k as u64 + 1) * TICK));
        }
    };
    for &(k, _, want) in lock {
        if played.get(k).is_some_and(|p| p.lock != want) {
            note("lock", k);
        }
    }
    for &(k, siren, dazed) in objects {
        let Some(p) = played.get(k) else { continue };
        if p.dazed != dazed {
            note("dazed", k);
        }
        match (p.siren, siren) {
            (Some((x, y, s)), Some((wx, wy, ws))) => {
                if x != wx {
                    note("siren_x", k);
                }
                if y != wy {
                    note("siren_y", k);
                }
                if ws.is_none() || s != ws {
                    note("siren_state", k);
                }
            }
            // Not logged: out of the run's window around Claw.
            (_, None) => {}
            (None, Some(_)) => {
                for name in ["siren_x", "siren_y", "siren_state"] {
                    note(name, k);
                }
            }
        }
    }
    firsts
}

/// Compares a play of `store` at `clock` with the reference trace at the
/// clock's tolerances (exact for c18, feel for c60), and at c18 the extra
/// columns with the run's (see the module documentation); returns the
/// divergences not in [`KNOWN`].
fn new_divergences(store: &str, clock: &str, played: &Played, run: &Path) -> Vec<String> {
    let (rows, reference) = (&played.rows, &played.reference);
    let tol = if clock == "c18" {
        Tolerances::exact()
    } else {
        Tolerances::feel()
    };
    let cmp = trace::compare(rows, reference, &tol);
    print!("{}", trace::report(rows, reference, &cmp, 3));
    let mut out = Vec::new();
    if cmp.rows_compared != reference.len() {
        out.push(format!(
            "{store} {clock}: {} of {} rows compared",
            cmp.rows_compared,
            reference.len()
        ));
    }
    let mut firsts: Vec<(String, u64)> = cmp
        .per_column
        .iter()
        .filter_map(|c| Some((c.column.clone(), c.first_t_ms?)))
        .collect();
    let lock =
        reference_lock(run).unwrap_or_else(|| panic!("{store}: no probe.csv in {}", run.display()));
    if clock != "c18" {
        firsts.extend(compare_lock_feel(&played.extra, &lock).map(|t| ("lock".to_string(), t)));
    } else {
        let objects = reference_objects(run)
            .unwrap_or_else(|| panic!("{store}: no obj-log.csv in {}", run.display()));
        firsts.extend(
            compare_extra(&played.extra, &lock, &objects)
                .into_iter()
                .map(|(c, t)| (c.to_string(), t)),
        );
    }
    let list = known(store, clock);
    for (column, first) in firsts {
        println!("first {store} {clock} {column} {first}");
        // The earliest of the known divergences of the column counts.
        let at = list
            .iter()
            .filter(|(name, _)| *name == column)
            .map(|&(_, at)| at)
            .min();
        match at {
            Some(at) if first >= at => {}
            _ => out.push(format!("{store} {clock}: {column} at {first} ms")),
        }
    }
    out
}

/// The folder of the first run of `clock` of a store.
fn run_dir(root: &Path, store: &str, clock: &str) -> PathBuf {
    let dir = root.join("level11").join(store);
    let meta = std::fs::read_to_string(dir.join("meta.toml"))
        .unwrap_or_else(|e| panic!("{store}: meta: {e}"));
    let id = run_id(&meta, clock).unwrap_or_else(|| panic!("{store} {clock}: no run id"));
    runs_root(root).join(id)
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let dir = root.join("level11").join(r.store);
        let (rows, holds) =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
        let our_holds: Vec<String> = r.holds.iter().map(|&(k, h)| h.line(k)).collect();
        assert_eq!(
            our_holds, holds,
            "{}: pauses, screens and commands",
            r.store
        );
        // Each clock the route lists has its meta and its trace.
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        for clock in r.clocks {
            assert!(
                meta_value(&meta, clock, "t0_ms").is_some(),
                "{} {clock}: no time zero",
                r.store
            );
            let trace = dir.join(format!("trace-{clock}-r1.csv"));
            assert!(trace.is_file(), "{}", trace.display());
        }
    }
}

#[test]
fn install_siren_routes_follow_the_reference_traces() {
    let Some(level) = level11() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L11_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        for &clock in r.clocks {
            println!("== level11/{} {clock}", r.store);
            let played = play(&level, &root, r, clock);
            let run = run_dir(&root, r.store, clock);
            failures.extend(new_divergences(r.store, clock, &played, &run));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row shows: Claw, the lockout and the Siren.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    x: i32,
    y: i32,
    state: i32,
    health: i32,
    lives: i32,
    lock: u32,
    dazed: bool,
    siren: Option<EnemyState>,
}

/// Plays `rows` rows of `store`'s route at the 18 ms step from its time
/// zero with the original's random state there; `None` (the test skips)
/// when the route needs the original's random stream (see
/// [`MERCAT_STREAM`]) and `CLAW_REFERENCE` is not set.
fn watch(level: &SimLevel, store: &str, rows: usize) -> Option<Vec<Look>> {
    let r = route_named(store);
    let stream = if MERCAT_STREAM {
        let root = reference_root()?;
        let path = root.join("level11").join(store).join("trace-c18-r1.csv");
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        Some(reference_stream(&trace::parse(&text).unwrap()))
    } else {
        None
    };
    let mut looks = Vec::new();
    play_rows(
        level,
        r,
        "c18",
        T0_MS,
        SEED,
        rows,
        stream.as_deref(),
        |_, sim, id| {
            let c = sim.player().expect("Claw");
            looks.push(Look {
                x: c.x,
                y: c.y,
                state: c.state,
                health: sim.claw().stats.health,
                lives: sim.claw().stats.lives,
                lock: sim.scream_lock_ms(),
                dazed: c.frame == scream::DAZED_FRAME,
                siren: sim
                    .objects()
                    .get(id)
                    .and_then(|o| EnemyState::from_id(o.state)),
            });
        },
    );
    Some(looks)
}

/// The spans (first, last) of the rows on which `f` holds.
fn spans_of(rows: &[Look], f: impl Fn(&Look) -> bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < rows.len() {
        if f(&rows[k]) {
            let a = k;
            while k + 1 < rows.len() && f(&rows[k + 1]) {
                k += 1;
            }
            out.push((a, k));
        }
        k += 1;
    }
    out
}

#[test]
fn install_siren_her_songs_lock_a_standing_claw_three_times() {
    let Some(level) = level11() else { return };
    let Some(rows) = watch(&level, "siren-stand", 812) else {
        return;
    };
    // She decides on row 28 and sings until row 133 (her call and first
    // scream on row 39, home on 134), and again from 311 and 594.
    let songs = spans_of(&rows, |l| l.siren == Some(EnemyState::Singing));
    assert_eq!(songs, [(28, 133), (311, 416), (594, 699)]);
    // Each song's four screams reach him 17 rows apart, each setting the
    // lockout to 2000 again: locked from the first hit until 111 rows
    // after the last.
    let locked = spans_of(&rows, |l| l.lock > 0);
    assert_eq!(locked, [(64, 226), (347, 509), (630, 792)]);
    for hit in [64, 81, 98, 115] {
        assert_eq!(rows[hit].lock, 2000, "row {hit}");
    }
    assert_eq!(rows[226].lock, 2);
    // A scream does no harm, and Claw stands throughout.
    assert!(
        rows.iter()
            .all(|l| l.health == 100 && l.state == state::STAND)
    );
    // Dazed from the hit's row to the last locked one at least.
    assert!(rows[64..=226].iter().all(|l| l.dazed));
    assert!(!rows[63].dazed);
}

#[test]
fn install_siren_a_held_jump_fires_when_the_lockout_ends() {
    let Some(level) = level11() else { return };
    let Some(rows) = watch(&level, "siren-jump", 612) else {
        return;
    };
    let jumps: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].state == state::JUMP && rows[k - 1].state != state::JUMP)
        .collect();
    // The jump pressed on row 30, then a jump on the first free row after
    // each lockout, the key held all along.
    assert_eq!(jumps, [30, 227, 473]);
    assert!(rows[62..=226].iter().all(|l| l.state == state::STAND));
}

#[test]
fn install_siren_a_walk_stops_while_locked() {
    let Some(level) = level11() else { return };
    let Some(rows) = watch(&level, "siren-walk", 300) else {
        return;
    };
    // Hit on row 104: the walk stops on 105 and starts again on 267.
    assert_eq!(rows[104].lock, 2000);
    assert_eq!(rows[104].state, state::MOVE_RIGHT);
    assert!(rows[105..=266].iter().all(|l| l.state == state::STAND));
    assert_eq!(rows[267].state, state::MOVE_RIGHT);
    let x = rows[105].x;
    assert!(rows[105..=266].iter().all(|l| l.x == x));
}

#[test]
fn install_siren_the_scream_touches_a_crouching_claw() {
    let Some(level) = level11() else { return };
    let Some(rows) = watch(&level, "siren-duck", 300) else {
        return;
    };
    // One row earlier than a standing Claw; the crouch rises while locked
    // and comes back on the first free row, Down held all along.
    assert_eq!(rows[62].lock, 0);
    assert_eq!(rows[63].lock, 2000);
    assert!(rows[20..=63].iter().all(|l| l.state == state::CROUCH));
    assert!(rows[64..=226].iter().all(|l| l.state == state::STAND));
    assert_eq!(rows[227].state, state::CROUCH);
}

#[test]
fn install_siren_a_rising_jump_loses_its_hold() {
    let Some(level) = level11() else { return };
    let Some(rows) = watch(&level, "siren-air", 120) else {
        return;
    };
    // Hit on row 64, two rows into the jump: the rise ends on row 68 at
    // 2725 (2651 with the hold), he falls from row 69 and lands on row 80.
    assert_eq!(rows[64].lock, 2000);
    assert!(rows[62..=68].iter().all(|l| l.state == state::JUMP));
    assert_eq!(rows[68].y, 2725);
    assert!(rows[69..=79].iter().all(|l| l.state == state::FALL));
    assert_eq!(rows[80].state, state::STAND);
}

#[test]
fn install_siren_screens_stop_the_lockout_and_a_death_keeps_it() {
    let Some(level) = level11() else { return };
    // The pauses, the help screen and the menu hold the counter: it reads
    // what it would without them and runs out on row 226.
    let Some(rows) = watch(&level, "siren-screens", 240) else {
        return;
    };
    assert_eq!(spans_of(&rows, |l| l.lock > 0), [(64, 226)]);
    assert_eq!((rows[120].lock, rows[121].lock), (1910, 1892));
    assert_eq!((rows[160].lock, rows[161].lock), (1190, 1172));
    // The end of life on row 100: he dies, respawns at the level's start
    // on row 194 with a life less, and the lockout runs on to row 209,
    // 111 rows after the third scream's hit.
    let Some(rows) = watch(&level, "siren-death", 260) else {
        return;
    };
    assert_eq!(rows[100].health, 0);
    assert_eq!((rows[194].x, rows[194].y), (3393, 1167));
    assert_eq!(rows[194].lives, rows[99].lives - 1);
    assert_eq!(spans_of(&rows, |l| l.lock > 0), [(64, 209)]);
}
