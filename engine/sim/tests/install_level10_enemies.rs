//! The ninth level's hook-handed pirate and PegLeg on the tenth level, on
//! your own install, against the original's runs of the same short routes.
//!
//! On the tenth level the hook-handed pirate never holds his hook out and
//! never parries a standing sword stab, and PegLeg fires his shots in
//! pairs, high then low, and rolls the full length of a forward roll, past
//! Claw. Each route puts Claw down at a fixed point of the level after its
//! first tick, the way the original's runs put him there (between two
//! ticks, as a warp does), waits until the level clock reaches the time
//! the original's run started at, and starts the shared random stream from
//! the run's state, once. Claw lands in reach of the enemy, which engages
//! him before that time with the original's own draws, which the replay
//! does not have; at time zero the enemy is put where the original's run
//! shows it, in the same state and with the same timers ([`align`]). The
//! routes:
//!
//! - beside the hook-handed pirate on the floor at 7246, his back to the
//!   wall: Claw put down 136 px to his left taps Right on rows 320 and 321
//!   and swings his sword standing every 15 rows from row 340;
//! - beside PegLeg on the floor at 6479: Claw put down 187 px to his left
//!   stands (shot pairs and backward rolls), or taps Right on rows 20 and
//!   21 and fires every 120 rows from row 60 (forward rolls).
//!
//! Each route is played tick by tick and compared with the reference trace
//! of the same route: at the 18 ms step with the original's speeds every
//! column exactly (Claw's position and state, health, score, the camera,
//! the random stream and the events), at 60 frames per second with the
//! default tuning within the feel tolerances. When the reference run also
//! lists the objects near Claw per step (`obj-log.csv` beside the run, in
//! the folder named by `CLAW_REFERENCE_RUNS`, by default `rt` next to the
//! reference store), the watched enemy's position and state are compared
//! too, exactly at the 18 ms step, its states lined up by the numbers the
//! run gives them ([`REFERENCE_STATES`]). Divergences that are known, with
//! their causes, are listed in [`KNOWN`]; any other fails the test. The
//! routes are written below; each store's own copy of its route must agree
//! with them. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE` for the
//! comparisons: each test skips itself when what it needs is unset, and
//! fails when `CLAW_REFERENCE` is set but a listed store lacks its route,
//! meta or trace; `CLAW_L10_ONLY` names one store to play alone. Nothing
//! from an install or a reference store is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::ObjectData;
use claw_sim::logic::enemy::{self, EnemyState, hook_pirate, pegleg};
use claw_sim::object::ObjectId;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const ATTACK: u32 = 0x2;
const PROJECTILE: u32 = 0x4;

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

/// A stretch repeated: its first row, its length, its keys, the distance
/// between two repeats and the first row no repeat starts on.
#[derive(Debug, Clone, Copy)]
struct Every {
    first: usize,
    len: usize,
    held: u32,
    gap: usize,
    until: usize,
}

const fn every(first: usize, len: usize, held: u32, gap: usize, until: usize) -> Option<Every> {
    Some(Every {
        first,
        len,
        held,
        gap,
        until,
    })
}

/// Which enemy a route watches: its logic name and its x on the run's
/// first step.
#[derive(Debug, Clone, Copy)]
enum Watch {
    HookPirate(i32),
    PegLeg(i32),
}

impl Watch {
    fn logic(self) -> &'static str {
        match self {
            Watch::HookPirate(_) => hook_pirate::NAMES[0],
            Watch::PegLeg(_) => "PegLeg",
        }
    }

    fn near(self) -> i32 {
        match self {
            Watch::HookPirate(x) | Watch::PegLeg(x) => x,
        }
    }
}

/// A route of the stores: its name, where Claw is put down, the
/// original's game time at its time zero at the 18 ms step, how many rows
/// it has, its stretches, its repeated stretch and the enemy it watches.
struct Route {
    store: &'static str,
    start: (i32, i32),
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
    every: Option<Every>,
    watch: Watch,
}

/// The clocks the routes were run at.
const CLOCKS: [&str; 2] = ["c18", "c60"];
/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;

/// The pirate of the pirate route, and where Claw is put down beside him.
const HOOK: Watch = Watch::HookPirate(3556);
const HOOK_START: (i32, i32) = (3420, 7200);
/// The PegLeg of the PegLeg routes, and where Claw is put down beside
/// him.
const PEG: Watch = Watch::PegLeg(25547);
const PEG_START: (i32, i32) = (25360, 6440);

const ROUTES: [Route; 3] = [
    Route {
        store: "hook-swing",
        start: HOOK_START,
        t0_ms: 828,
        rows: 1000,
        spans: &[span(320, 321, RIGHT)],
        every: every(340, 2, ATTACK, 15, 1000),
        watch: HOOK,
    },
    Route {
        store: "peg-engage",
        start: PEG_START,
        t0_ms: 1170,
        rows: 600,
        spans: &[],
        every: None,
        watch: PEG,
    },
    Route {
        store: "peg-pistol",
        start: PEG_START,
        t0_ms: 1170,
        rows: 700,
        spans: &[span(20, 21, RIGHT)],
        every: every(60, 2, PROJECTILE, 120, 700),
        watch: PEG,
    },
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. What they come from:
///
/// - c18, hook-swing, 7956 ms (enemy_y): the killed pirate's body flies
///   off on the same arc, but a pixel lower on one row: the shared flight
///   of a killed enemy starts from his whole-pixel height, the original's
///   from a part of a pixel below it. The body touches nothing on its way.
/// - c60 (the default tuning played tick by tick), the PegLeg routes:
///   Claw stands up from the low shot's hit, on the third row, a row
///   later than the original at 60 frames per second, whose hurt pose
///   counts its frames; then the original makes the enemies' draws once a
///   frame, so their decisions differ from the replay's, their shots and
///   rolls reach Claw at other times, and his health, state and position
///   follow (on both routes he loses his life before the end).
const KNOWN: &[Known] = &[
    ("hook-swing", "c18", &[("enemy_y", 7956)]),
    (
        "peg-engage",
        "c60",
        &[
            ("state_id", 434),
            ("health", 3467),
            ("x", 3517),
            ("cam_x", 3517),
            ("score", 5467),
            ("physics_type", 7517),
            ("y", 7967),
            ("cam_y", 7967),
            ("lives", 10917),
            ("events", 10917),
        ],
    ),
    (
        "peg-pistol",
        "c60",
        &[
            ("state_id", 434),
            ("health", 2284),
            ("x", 2334),
            ("cam_x", 2334),
            ("score", 3317),
            ("physics_type", 8767),
            ("y", 9584),
            ("cam_y", 10450),
            ("lives", 10434),
            ("events", 10434),
            ("pistol", 12034),
        ],
    ),
];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
}

/// The numbers the reference runs give the enemies' states, where they
/// differ from the states' own numbers.
const REFERENCE_STATES: [(i32, EnemyState); 26] = [
    (2000, EnemyState::Idle),
    (2001, EnemyState::Falling),
    (2003, EnemyState::Strike),
    (2004, EnemyState::Dead),
    (2005, EnemyState::Jump),
    (2008, EnemyState::Bounce),
    (2009, EnemyState::Lifted),
    (2010, EnemyState::Thrown),
    (2011, EnemyState::Pause),
    (2012, EnemyState::Dropped),
    (2013, EnemyState::MoveBack),
    (2014, EnemyState::MoveAhead),
    (2015, EnemyState::BlockHigh),
    (2016, EnemyState::BlockLow),
    (2017, EnemyState::Flailing),
    (2018, EnemyState::Home),
    (2019, EnemyState::JumpAtClaw),
    (2021, EnemyState::Advance),
    (2022, EnemyState::FastAdvance),
    (2023, EnemyState::Pace),
    (2024, EnemyState::HitLow),
    (2025, EnemyState::HitHigh),
    (2027, EnemyState::Fire),
    (2035, EnemyState::Block),
    (2075, EnemyState::HoldHook),
    (2076, EnemyState::RollForward),
];

/// The run's number of the backward roll.
const REFERENCE_ROLL_BACKWARD: i32 = 2077;

/// The state a reference run's number stands for.
fn reference_state(id: i32) -> Option<EnemyState> {
    if id == REFERENCE_ROLL_BACKWARD {
        return Some(EnemyState::RollBackward);
    }
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
    let mut spans: Vec<Span> = r.spans.to_vec();
    if let Some(e) = r.every {
        let mut from = e.first;
        while from < e.until.min(r.rows) {
            spans.push(span(from, (from + e.len - 1).min(r.rows - 1), e.held));
            from += e.gap;
        }
    }
    for s in &spans {
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

/// The rows of a store's own copy of a route: (held, pressed) by row.
fn store_route(dir: &Path) -> Option<Vec<(u32, u32)>> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).unwrap();
    Some(
        text.lines()
            .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
            .map(|l| {
                let f: Vec<&str> = l.split(',').collect();
                (hex(f[1]), hex(f[2]))
            })
            .collect(),
    )
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

fn level10() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(10)).unwrap();
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

/// What the original's runs show at a route's time zero, where the
/// replay's own engagement before it differs (see [`align`]).
#[derive(Debug, Clone, Copy)]
enum Engagement {
    /// The hook-handed pirate swung his hook from where he stands on the
    /// step that started 42 steps before time zero (his rest of 2000 ms
    /// running from it), went Home on the 19th step of the swing and has
    /// walked at Claw for 22 steps since.
    HookSwung,
    /// PegLeg fired his pair (the high shot hit Claw, the low one is in
    /// flight) and walks at Claw again, all three steps earlier than in
    /// the replay: one step of his walk is behind him.
    PegPairEarly,
}

/// The engagement before time zero of a route; the same at both clocks.
fn engagement(store: &str) -> Option<Engagement> {
    match store {
        "hook-swing" => Some(Engagement::HookSwung),
        "peg-engage" | "peg-pistol" => Some(Engagement::PegPairEarly),
        _ => None,
    }
}

/// Steps the original's PegLeg runs ahead of the replay's before time
/// zero.
const PEG_LEAD: u32 = 3;

/// Puts the watched enemy `id` (and PegLeg's shot in flight) where the
/// original's run shows them at time zero, `t0_ms` (see [`Engagement`]).
fn align(sim: &mut Sim, id: ObjectId, e: Engagement, t0_ms: u64) {
    let step = TICK;
    sim.with_world(|w| match e {
        Engagement::HookSwung => {
            let x = 3556.0 - 22.0 * 1.08;
            place_walking(w, id, x, hook_pirate::CLASS.anims.advance);
            if let Some(ObjectData::HookPirate(p)) = w.objects.get_mut(id).map(|o| &mut o.data) {
                p.strike_rest = enemy::Timer::new(t0_ms - 42 * step, 2000);
            }
        }
        Engagement::PegPairEarly => {
            let lead = u64::from(PEG_LEAD) * step;
            let x = 25547.0 - 1.08;
            place_walking(w, id, x, pegleg::CLASS.anims.advance);
            if let Some(ObjectData::PegLeg(p)) = w.objects.get_mut(id).map(|o| &mut o.data) {
                // The pair is over: the replay's is still on its second
                // shot.
                p.pair_shots = 0;
                p.fire_cooldown.start -= lead;
                p.roll_timer.start -= lead;
            }
            // His low shot, three of its 9 px steps further on.
            let shots: Vec<ObjectId> = w
                .objects
                .all()
                .filter(|(_, o)| &*o.logic == pegleg::bullet::LOGIC)
                .map(|(s, _)| s)
                .collect();
            assert_eq!(shots.len(), 1, "his low shot is in flight");
            let o = w.objects.get_mut(shots[0]).expect("the shot");
            let ahead = 9 * PEG_LEAD as i32;
            o.x -= ahead;
            if let ObjectData::Projectile(d) = &mut o.data {
                d.motion.x -= f64::from(ahead);
            }
        }
    });
}

/// Puts the enemy `id` at the exact x `x` on its floor, walking at Claw on
/// his left at 60 px/s, alerted, free.
fn place_walking(w: &mut claw_sim::logic::World, id: ObjectId, x: f64, anim: &str) {
    let y = {
        let o = w.objects.get_mut(id).expect("the enemy");
        o.x = x.trunc() as i32;
        o.y
    };
    enemy::set_facing(w, id, false);
    enemy::set_state(w, id, EnemyState::Advance);
    enemy::set_anim(w, id, anim);
    if let Some(d) = enemy::data_mut(w, id) {
        d.carry = (x.fract(), 0.0);
        d.moved_to = Some((x.trunc() as i32, y));
        d.busy = false;
        d.vx = -enemy::states::ADVANCE_SPEED;
        d.deferred = None;
        d.alerted = true;
    }
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
    if let Some(e) = engagement(r.store) {
        let id = find(&sim, r.watch);
        align(&mut sim, id, e, t0_ms);
    }
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

/// The watched enemy at one step of a run or of the replay: position and
/// state (the run's own numbers lined up through [`REFERENCE_STATES`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnemyAt {
    x: i32,
    y: i32,
    state: Option<EnemyState>,
}

/// The watched enemy's steps in a run's `obj-log.csv`: the enemy of its
/// family nearest its x on the run's first step, by step.
fn reference_enemy(path: &Path, watch: Watch) -> Option<Vec<(usize, EnemyAt)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, serial, logic, st, x, y) = (
        col("tick")?,
        col("serial")?,
        col("logic_name")?,
        col("state")?,
        col("x")?,
        col("y")?,
    );
    let rows: Vec<Vec<&str>> = lines
        .map(|l| l.split(',').collect::<Vec<_>>())
        .filter(|f| f.get(logic) == Some(&watch.logic()))
        .collect();
    let int = |s: &str| s.trim().parse::<i32>().ok();
    let ours = rows
        .iter()
        .filter(|f| f[tick] == "0")
        .min_by_key(|f| (int(f[x]).unwrap_or(0) - watch.near()).abs())?[serial];
    rows.iter()
        .filter(|f| f[serial] == ours)
        .map(|f| {
            Some((
                f[tick].parse().ok()?,
                EnemyAt {
                    x: int(f[x])?,
                    y: int(f[y])?,
                    state: reference_state(int(f[st])?),
                },
            ))
        })
        .collect()
}

/// The first reference time (ms) at which the replay's enemy leaves the
/// run's, by column (enemy_x, enemy_y, enemy_state); a step on which one
/// of them has no enemy counts for all three.
fn compare_enemy(
    played: &[Option<EnemyAt>],
    reference: &[(usize, EnemyAt)],
) -> Vec<(&'static str, u64)> {
    let mut firsts: Vec<(&'static str, u64)> = Vec::new();
    let mut note = |name: &'static str, k: usize| {
        if !firsts.iter().any(|(n, _)| *n == name) {
            firsts.push((name, (k as u64 + 1) * TICK));
        }
    };
    for &(k, want) in reference {
        let Some(got) = played.get(k) else { continue };
        let Some(got) = got else {
            for name in ["enemy_x", "enemy_y", "enemy_state"] {
                note(name, k);
            }
            continue;
        };
        if got.x != want.x {
            note("enemy_x", k);
        }
        if got.y != want.y {
            note("enemy_y", k);
        }
        if want.state.is_none() || got.state != want.state {
            note("enemy_state", k);
        }
    }
    firsts
}

fn find(sim: &Sim, watch: Watch) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == watch.logic())
        .min_by_key(|(_, o)| (o.x - watch.near()).abs())
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {} near {}", watch.logic(), watch.near()))
}

/// One play of a route: its trace, the reference trace, and the watched
/// enemy on each row.
struct Played {
    rows: Vec<TraceRow>,
    reference: Vec<TraceRow>,
    enemy: Vec<Option<EnemyAt>>,
}

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs. A store without the trace or its meta
/// fails the test.
fn play(level: &SimLevel, root: &Path, r: &Route, clock: &str) -> Played {
    let dir = root.join("level10").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if clock == "c18" {
        assert_eq!(t0, r.t0_ms, "{}: time zero", r.store);
        assert_eq!(seed, SEED, "{}: random state", r.store);
    }
    let mut sim = at_time_zero(level, clock, r, t0, seed);
    let id = find(&sim, r.watch);
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut enemy = Vec::new();
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        enemy.push(sim.objects().get(id).map(|o| EnemyAt {
            x: o.x,
            y: o.y,
            state: EnemyState::from_id(o.state),
        }));
        k += 1;
    }
    Played {
        rows,
        reference,
        enemy,
    }
}

/// Compares a play of `store` at `clock` with the reference trace at the
/// clock's tolerances (exact for c18, feel for c60), and at c18 its enemy
/// with the run's (`enemy`, when the run lists it); returns the
/// divergences not in [`KNOWN`].
fn new_divergences(
    store: &str,
    clock: &str,
    played: &Played,
    enemy: Option<&[(usize, EnemyAt)]>,
) -> Vec<String> {
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
    let mut firsts: Vec<(&str, u64)> = cmp
        .per_column
        .iter()
        .filter_map(|c| Some((c.column.as_str(), c.first_t_ms?)))
        .collect();
    if let Some(e) = enemy {
        firsts.extend(compare_enemy(&played.enemy, e));
    }
    let list = known(store, clock);
    for (column, first) in firsts {
        println!("first {store} {clock} {column} {first}");
        match list.iter().find(|(name, _)| *name == column) {
            Some(&(_, at)) if first >= at => {}
            _ => out.push(format!("{store} {clock}: {column} at {first} ms")),
        }
    }
    out
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let dir = root.join("level10").join(r.store);
        let rows =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
    }
}

#[test]
fn install_level10_pirate_and_pegleg_routes_follow_the_reference_traces() {
    let Some(level) = level10() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L10_ONLY").ok();
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        let dir = root.join("level10").join(r.store);
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        for clock in CLOCKS {
            println!("== level10/{} {clock}", r.store);
            let played = play(&level, &root, r, clock);
            let enemy = runs
                .as_ref()
                .filter(|_| clock == "c18")
                .and_then(|d| Some(d.join(run_id(&meta, clock)?).join("obj-log.csv")))
                .and_then(|p| reference_enemy(&p, r.watch));
            if clock == "c18" && enemy.is_none() {
                if runs.is_some() {
                    failures.push(format!("{}: the run's object log has no enemy", r.store));
                } else {
                    println!("the runs' object logs are not there: the enemy is not compared");
                }
            }
            failures.extend(new_divergences(r.store, clock, &played, enemy.as_deref()));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows: Claw and the watched enemy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    /// Claw's position, state and health.
    x: i32,
    health: i32,
    /// The enemy's position, state and health.
    ex: i32,
    ey: i32,
    es: Option<EnemyState>,
    ehealth: i32,
}

/// A shot in the level: where it is and which way it flies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shot {
    id: ObjectId,
    x: i32,
    y: i32,
    vx: i32,
}

/// Plays `rows` rows of `store`'s route at the 18 ms step from its time
/// zero with the original's random state there, watching its enemy and
/// the enemies' straight shots.
fn watch(level: &SimLevel, store: &str, rows: usize) -> (Vec<Look>, Vec<Vec<Shot>>) {
    let r = route_named(store);
    let mut sim = at_time_zero(level, "c18", r, r.t0_ms, SEED);
    let id = find(&sim, r.watch);
    let ticks = route_ticks(r);
    let mut looks = Vec::new();
    let mut shots = Vec::new();
    for k in 0..rows {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        let c = sim.player().expect("Claw");
        let e = sim.objects().get(id);
        looks.push(Look {
            x: c.x,
            health: sim.claw().stats.health,
            ex: e.map_or(0, |o| o.x),
            ey: e.map_or(0, |o| o.y),
            es: e.and_then(|o| EnemyState::from_id(o.state)),
            ehealth: e.map_or(0, |o| o.health),
        });
        shots.push(
            sim.objects()
                .all()
                .filter(|(_, o)| &*o.logic == pegleg::bullet::LOGIC)
                .map(|(sid, o)| Shot {
                    id: sid,
                    x: o.x,
                    y: o.y,
                    vx: o.speed_x,
                })
                .collect(),
        );
    }
    (looks, shots)
}

/// The rows on which `f` holds and did not on the row before.
fn starts(rows: &[Look], f: impl Fn(&Look) -> bool) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| f(&rows[k]) && (k == 0 || !f(&rows[k - 1])))
        .collect()
}

/// The spans (first, last) of the rows on which the enemy is in `s`.
fn spans(rows: &[Look], s: EnemyState) -> Vec<(usize, usize)> {
    let f = |l: &Look| l.es == Some(s);
    starts(rows, f)
        .into_iter()
        .map(|a| {
            let b = (a..rows.len()).take_while(|&k| f(&rows[k])).last().unwrap();
            (a, b)
        })
        .collect()
}

/// The rows on which Claw's health drops.
fn hits_on_claw(rows: &[Look]) -> Vec<usize> {
    (1..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .collect()
}

/// The rows on which the enemy's health drops.
fn hits_on_enemy(rows: &[Look]) -> Vec<usize> {
    (1..rows.len())
        .filter(|&k| rows[k].ehealth < rows[k - 1].ehealth)
        .collect()
}

/// The new shots of each row: (row, the shot as it first shows).
fn new_shots(shots: &[Vec<Shot>]) -> Vec<(usize, Shot)> {
    let mut out = Vec::new();
    for k in 0..shots.len() {
        for s in &shots[k] {
            if k == 0 || !shots[k - 1].iter().any(|p| p.id == s.id) {
                out.push((k, *s));
            }
        }
    }
    out
}

#[test]
fn install_level10_the_pirate_takes_standing_sword_stabs_and_strikes_back() {
    let Some(level) = level10() else { return };
    let (rows, _) = watch(&level, "hook-swing", 700);
    // He never holds his hook out and never parries.
    assert!(spans(&rows, EnemyState::HoldHook).is_empty());
    assert!(spans(&rows, EnemyState::Block).is_empty());
    // From Home 100 px from Claw he stabs every 56 rows, each stab 21
    // rows long, none reaching Claw.
    let strikes = spans(&rows, EnemyState::Strike);
    let early: Vec<(usize, usize)> = strikes.iter().copied().filter(|s| s.0 < 320).collect();
    assert_eq!(early[0], (70, 90));
    assert_eq!(early.len(), 5, "{early:?}");
    assert!(early.windows(2).all(|p| p[1].0 - p[0].0 == 56), "{early:?}");
    assert!(early.iter().all(|s| s.1 - s.0 + 1 == 21), "{early:?}");
    assert_eq!(rows[70].ex - rows[70].x, 100);
    // Claw steps in to 92 px. The standing swing of row 340 lands (5) and
    // he strikes back on the same row; his strike hits Claw for 10 and
    // knocks him out of reach, so the next two swings meet the air; he
    // strikes again from Home, walks in, and the swing of row 385 kills
    // him.
    assert_eq!(hits_on_enemy(&rows), [346, 391]);
    assert_eq!(rows[346].ehealth, 5);
    assert_eq!(rows[346].es, Some(EnemyState::Strike));
    assert_eq!(rows[346].ex - rows[346].x, 92);
    assert_eq!(
        starts(&rows, |l| l.es == Some(EnemyState::Strike))[5..],
        [346, 366]
    );
    assert_eq!(hits_on_claw(&rows), [353]);
    assert_eq!(rows[353].health, 90);
    assert_eq!(starts(&rows, |l| l.es == Some(EnemyState::Dead)), [391]);
}

#[test]
fn install_level10_pegleg_fires_pairs_high_then_low() {
    let Some(level) = level10() else { return };
    let (rows, shots) = watch(&level, "peg-engage", 500);
    // Each shot spell lasts 62 rows and fires two bullets, on its 20th and
    // 51st rows, 55 px ahead of him: the first at his height less 24, the
    // second at it plus 15.
    let fires = spans(&rows, EnemyState::Fire);
    assert_eq!(fires[..2], [(118, 179), (336, 397)]);
    let fired: Vec<(usize, i32, i32)> = new_shots(&shots)
        .into_iter()
        .filter(|p| p.0 > 0 && p.0 < 400)
        .map(|(k, s)| (k, s.x - rows[k].ex, s.y - rows[k].ey))
        .collect();
    assert_eq!(
        fired,
        [
            (137, -55, -24),
            (168, -55, 15),
            (355, -55, -24),
            (386, -55, 15),
        ]
    );
    // Both shots of the first pair meet the standing Claw.
    assert_eq!(hits_on_claw(&rows)[..3], [2, 163, 197]);
    // Claw within 150 px: the backward rolls, 34 rows each.
    let back = spans(&rows, EnemyState::RollBackward);
    assert_eq!(back[..2], [(84, 117), (302, 335)]);
}

#[test]
fn install_level10_pegleg_rolls_the_full_length_past_claw() {
    let Some(level) = level10() else { return };
    let (rows, shots) = watch(&level, "peg-pistol", 300);
    // The shot of row 60 rolls him at Claw: 35 rows, from 25462 to 25309,
    // 3 px past Claw at 25306; he never loses health.
    let fwd = spans(&rows, EnemyState::RollForward);
    assert_eq!(fwd[0], (76, 110));
    assert_eq!((rows[76].ex, rows[110].ex), (25462, 25309));
    assert_eq!(rows[108].x, 25306);
    assert!(rows.iter().all(|l| l.ehealth == pegleg::HEALTH));
    // Then the pair, fired to the side Claw is on by those 3 px: both
    // shots start beyond him, going left.
    let after: Vec<(usize, Shot)> = new_shots(&shots)
        .into_iter()
        .filter(|p| p.0 > 110 && p.0 < 180)
        .collect();
    assert_eq!(after.iter().map(|p| p.0).collect::<Vec<_>>(), [130, 161]);
    for (k, s) in &after {
        assert!(s.vx < 0, "row {k}: going left");
        assert!(s.x < rows[*k].x, "row {k}: beyond Claw");
    }
    // The backward roll when Claw is within 150 px: 34 rows.
    assert_eq!(spans(&rows, EnemyState::RollBackward)[0], (174, 207));
}
