//! `claw-bot`: searches the headless simulation of a level from your own
//! install for a route that finishes it, and writes the route.
//!
//! ```text
//! claw-bot --install PATH --level N --out ROUTE.csv
//!          [--tuning default|original] [--nodes N] [--budget-min M]
//!          [--seed S|level] [--hints FILE] [--from-route FILE --from-tick T]
//!          [--start X,Y] [--land-gap N] [--fragile-cost PX] [--threads N]
//!          [--cache-games N] [--no-shadows] [--margin-samples N] [--hints-on-foot]
//!          [--intro any|play|skip] [--carry FILE] [--goal X,Y] [--goal-on-foot]
//!          [--crabs FILE] [--crab-variants FILE[,FILE...]] [--drift-cost PX] [--quiet]
//! ```
//!
//! The route is an input script (`tick,held_mask,pressed_mask`, one row
//! per 18 ms tick) led by the header lines `# level N`, `# tuning ...`,
//! `# seed N` and `# mid_air_hit_fix = off`. A report goes next to it as
//! ROUTE.json: nodes expanded, ticks, deaths, health lost and the timing
//! margin (each sampled change of the held mask shifted by one tick,
//! informational).
//!
//! The same arguments always give the same route: the budget counts
//! expanded nodes, and the thread count only spreads the work.
//! `--budget-min` is a wall-time safety stop that ends the search with a
//! failure. `--seed` is the state of the game's shared random stream at the
//! route's time zero (a reference trace's `rng_at_t0`), and breaks ties;
//! `--seed level` takes the state the level's set-up leaves the stream in,
//! which is where a level started from its beginning starts.
//! Keys down when the level starts count only once released and pressed
//! again, so a route's first press comes after its first tick.
//! `--hints` names a file of `x,y` waypoints, one per line, written from
//! the level's geometry; the file's name is recorded in the header.
//! With `--hints-on-foot` a waypoint counts as reached only with Claw on
//! his feet, on a ladder or on a rope near it (within twice the usual
//! distance), so the next stretch of the search never starts in the
//! middle of a jump; a `foot` after a waypoint's `x,y` in the file makes
//! that waypoint alone count that way.
//! `--from-route` starts from the game after the first T ticks of another
//! route, which lead the new one; a segment's prefix (`# start X,Y` in
//! its header) is played from its start point, and the new route is a
//! segment from there too. When the prefix changed its held mask
//! less than two ticks before T, the new part first keeps that mask for
//! the ticks missing. The other route's `# level`, `# tuning` and
//! `# seed` lines, when it has them, must match the arguments. Without
//! `--hints`, the new route's header names the other route's hints file,
//! since its leading ticks were found with those waypoints. The other
//! route's help screens and pauses (`@` lines) up to T are played as the
//! game plays them and kept in the new route; one that ends a life is
//! refused.
//! `--start X,Y` plans a segment: Claw is placed at (X, Y) in the
//! level-start world and the game runs with nothing held until he and the
//! camera are at rest; the random stream is then set to the seed and the
//! route starts (`# start X,Y` in its header).
//! `--land-gap N` keeps every press of jump more than N ticks from a
//! landing of Claw; it defaults to 2 with the default tuning (played at 60
//! frames per second, where the original can land a frame from where the
//! game does and a press on that frame is lost) and 0 with the original
//! tuning.
//! `--fragile-cost PX` is what a move costs the search when it fails with
//! its timing shifted by a tick (it dies or ends elsewhere): the found
//! route then prefers moves with room to spare. It defaults to
//! 400; 0 turns the check off, and `--no-shadows` keeps the check to the
//! moves' own timing (no shadow games). `--cache-games N` is how many games the
//! search keeps in memory (512 by default; a node with its shadow games
//! counts three): more is faster and takes more memory, and never changes
//! the route.
//!
//! On a level that ends with a boss, the route goes through the boss
//! room's door, defeats the boss and takes the gem his defeat releases.
//! `--intro play` keeps to routes that enter the boss room the normal way,
//! so the boss's introduction plays; `--intro skip` keeps to routes whose
//! entry skips it (a jump on the first tick in the room); `any`, the
//! default, takes either.
//! `--carry FILE` starts the level with the progress a game carried into
//! it, read from the `# start_score = ...` lines of FILE (the lines a
//! recording of a game's later level starts with); the route's header
//! repeats them, so a replay starts the level the same way.
//!
//! `--goal X,Y` ends the route where Claw first comes within
//! [`GOAL_RADIUS`] px of (X, Y) instead of at the level's end: a leg of a
//! longer route, which `--from-route` then continues, so a long level can
//! be searched a stretch at a time. Its report counts the goal as the win.
//! With `--goal-on-foot` the goal counts only with Claw on his feet, on a
//! ladder or on a rope there, so the stretch never ends in the middle of
//! a jump or a fall.
//!
//! `--crabs FILE` gives the hermit crabs the values they start with in
//! the route's game (the side a crab's first regroup faces by, its
//! second range box and whether it starts stationary, which the original
//! game does not fix; the file format is `claw-replay`'s, one `crab
//! RECORD = SIDE, HALF_WIDTH, HALF_HEIGHT[, STATIONARY]` or `nest_crab
//! NEST_RECORD HATCH = ...` entry per line).
//! The route's header repeats them (`# crab ...` lines), so a replay plays
//! the same game. `--crab-variants FILE,FILE` names further sets the route
//! must also win under: every move is played in a game under each set
//! too, and a move in which Claw dies in one of them, or that wins while
//! one of them does not (for a `--goal`, that does not come within twice
//! its distance of it, on his feet with `--goal-on-foot`), is dropped; the found route is played from the
//! start under each set, and its report gives each set's result and
//! margin. The header names them (`# crab_variant N: ...` lines). The
//! search keeps the games together: each px Claw in one of them is from
//! the route's own Claw costs `--drift-cost` (1 by default). With
//! `--from-route`, each option left out comes from the route continued
//! on its own: without `--crabs` the run's values are its `# crab` and
//! `# nest_crab` lines, and without `--crab-variants` the further sets
//! are its `# crab_variant N: ...` lines, whichever of the two options is
//! given. The prefix is played under the run's values and under each
//! further set, and the search stops with an error when Claw loses a
//! life, or is dying, in any of those games before the search starts.
//!
//! No route is written whose held mask changes twice less than two ticks
//! apart.
//!
//! Exit status: 0 with a route, 1 when the search failed, 2 on a usage
//! error.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::actions::MIN_SPACING;
use claw_bot::route::{self, Header, TuningName};
use claw_bot::search::Intro;
use claw_bot::{Goal, SearchConfig, search_report_with};
use claw_sim::logic::enemy::crab::CrabValues;
use claw_sim::{Carry, DirectiveKind, InputScript, ScriptHeader, Sim, SimLevel, StartState};

const USAGE: &str = "usage: claw-bot --install PATH --level N --out ROUTE.csv \
[--tuning default|original] [--nodes N] [--budget-min M] [--seed S|level] [--hints FILE] \
[--from-route FILE --from-tick T] [--start X,Y] [--land-gap N] [--fragile-cost PX] \
[--threads N] [--cache-games N] [--no-shadows] [--margin-samples N] [--hints-on-foot] \
[--intro any|play|skip] [--carry FILE] [--goal X,Y] [--goal-on-foot] [--crabs FILE] \
[--crab-variants FILE[,FILE...]] [--drift-cost PX] [--quiet]";

/// The random state a route starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seed {
    /// This state.
    Value(u32),
    /// The state the level's set-up leaves the stream in.
    Level,
}

/// Priority cost of a fragile move, px.
const DEFAULT_FRAGILE_COST: i64 = 400;

/// Ticks a jump press keeps from a landing with the default tuning.
const DEFAULT_LAND_GAP: u64 = 2;

/// How near Claw must come to a `--goal` point, px.
pub const GOAL_RADIUS: i32 = 48;

#[derive(Debug)]
struct Args {
    install: Option<PathBuf>,
    level: Option<u32>,
    out: Option<PathBuf>,
    tuning: TuningName,
    nodes: u64,
    budget_min: Option<f64>,
    seed: Seed,
    hints: Option<PathBuf>,
    from_route: Option<PathBuf>,
    from_tick: Option<usize>,
    start: Option<(i32, i32)>,
    land_gap: Option<u64>,
    fragile_cost: i64,
    threads: usize,
    cache_games: usize,
    shadows: bool,
    hints_on_foot: bool,
    margin_samples: usize,
    intro: Intro,
    carry: Option<PathBuf>,
    goal: Option<(i32, i32)>,
    goal_on_foot: bool,
    crabs: Option<PathBuf>,
    crab_variants: Vec<PathBuf>,
    drift_cost: i64,
    quiet: bool,
}

impl Default for Args {
    fn default() -> Args {
        Args {
            install: None,
            level: None,
            out: None,
            tuning: TuningName::Default,
            nodes: SearchConfig::default().nodes,
            budget_min: None,
            seed: Seed::Value(1),
            hints: None,
            from_route: None,
            from_tick: None,
            start: None,
            land_gap: None,
            fragile_cost: DEFAULT_FRAGILE_COST,
            threads: 1,
            cache_games: SearchConfig::default().cached_sims,
            shadows: true,
            hints_on_foot: false,
            margin_samples: 40,
            intro: Intro::Any,
            carry: None,
            goal: None,
            goal_on_foot: false,
            crabs: None,
            crab_variants: Vec::new(),
            drift_cost: SearchConfig::default().drift_cost,
            quiet: false,
        }
    }
}

fn parse_args(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args::default();
    while let Some(flag) = it.next() {
        if flag == "--help" || flag == "-h" {
            return Err(USAGE.to_string());
        }
        if flag == "--quiet" {
            a.quiet = true;
            continue;
        }
        if flag == "--no-shadows" {
            a.shadows = false;
            continue;
        }
        if flag == "--hints-on-foot" {
            a.hints_on_foot = true;
            continue;
        }
        if flag == "--goal-on-foot" {
            a.goal_on_foot = true;
            continue;
        }
        let v = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        let num = |what: &str| format!("bad {what} {v:?}");
        match flag.as_str() {
            "--install" => a.install = Some(PathBuf::from(&v)),
            "--level" => a.level = Some(v.parse().map_err(|_| num("level"))?),
            "--out" => a.out = Some(PathBuf::from(&v)),
            "--tuning" => {
                a.tuning = TuningName::parse(&v)
                    .ok_or_else(|| format!("bad tuning {v:?}, expected default or original"))?
            }
            "--nodes" => a.nodes = v.parse().map_err(|_| num("node budget"))?,
            "--budget-min" => {
                let m: f64 = v.parse().map_err(|_| num("budget"))?;
                if m.is_nan() || m <= 0.0 {
                    return Err(num("budget"));
                }
                a.budget_min = Some(m);
            }
            "--seed" => {
                a.seed = if v == "level" {
                    Seed::Level
                } else {
                    Seed::Value(v.parse().map_err(|_| num("seed"))?)
                }
            }
            "--hints" => a.hints = Some(PathBuf::from(&v)),
            "--from-route" => a.from_route = Some(PathBuf::from(&v)),
            "--from-tick" => a.from_tick = Some(v.parse().map_err(|_| num("tick"))?),
            "--start" => a.start = Some(point(&v).ok_or_else(|| num("start point"))?),
            "--goal" => a.goal = Some(point(&v).ok_or_else(|| num("goal point"))?),
            "--land-gap" => a.land_gap = Some(v.parse().map_err(|_| num("landing gap"))?),
            "--fragile-cost" => {
                a.fragile_cost = v.parse().map_err(|_| num("fragile cost"))?;
                if a.fragile_cost < 0 {
                    return Err(num("fragile cost"));
                }
            }
            "--threads" => a.threads = v.parse::<usize>().map_err(|_| num("thread count"))?.max(1),
            "--cache-games" => {
                a.cache_games = v.parse::<usize>().map_err(|_| num("game count"))?.max(1)
            }
            "--margin-samples" => a.margin_samples = v.parse().map_err(|_| num("sample count"))?,
            "--intro" => {
                a.intro = Intro::parse(&v)
                    .ok_or_else(|| format!("bad intro {v:?}, expected any, play or skip"))?
            }
            "--carry" => a.carry = Some(PathBuf::from(&v)),
            "--crabs" => a.crabs = Some(PathBuf::from(&v)),
            "--drift-cost" => {
                a.drift_cost = v.parse().map_err(|_| num("drift cost"))?;
                if a.drift_cost < 0 {
                    return Err(num("drift cost"));
                }
            }
            "--crab-variants" => {
                a.crab_variants = v
                    .split(',')
                    .map(|p| {
                        let p = p.trim();
                        (!p.is_empty())
                            .then(|| PathBuf::from(p))
                            .ok_or_else(|| format!("bad file list {v:?}"))
                    })
                    .collect::<Result<_, _>>()?;
            }
            other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
        }
    }
    if a.from_route.is_some() != a.from_tick.is_some() {
        return Err(format!("--from-route and --from-tick go together\n{USAGE}"));
    }
    if a.goal_on_foot && a.goal.is_none() {
        return Err(format!("--goal-on-foot needs --goal\n{USAGE}"));
    }
    if a.start.is_some() && a.from_route.is_some() {
        return Err(format!(
            "--start and --from-route do not go together\n{USAGE}"
        ));
    }
    Ok(a)
}

/// Reads `x,y`.
fn point(v: &str) -> Option<(i32, i32)> {
    v.split_once(',')
        .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
}

/// The game a route starts from: the level start (with `carry`, the
/// progress carried in), or with `start` the segment start (see
/// [`route::settle_at`]), the crabs given their `crabs` values.
fn start_sim(
    level: SimLevel,
    tuning: TuningName,
    seed: u32,
    start: Option<(i32, i32)>,
    carry: Option<&Carry>,
    crabs: &CrabValues,
) -> Sim {
    let cfg = route::sim_config(tuning, seed);
    let mut sim = match carry {
        Some(c) => Sim::with_start(
            level,
            cfg,
            StartState {
                carry: Some(c.clone()),
                save: None,
            },
        ),
        None => Sim::new(level, cfg),
    };
    crabs.apply(&mut sim);
    if let Some((x, y)) = start {
        route::settle_at_with(&mut sim, x, y, seed, crabs);
    }
    sim
}

/// Reads a file of crab values.
fn read_crabs(path: &Path) -> Result<CrabValues, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    CrabValues::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Plays the ticks of `prefix` on `sim` with its help screens and pauses
/// (see [`route::play_directives`]), the crabs given their `crabs`
/// values; an error when Claw loses a life in them. The directives due
/// after its last tick, before the search's first, are played too.
fn play_prefix(sim: &mut Sim, prefix: &InputScript, crabs: &CrabValues) -> Result<(), String> {
    let lives = sim.stats().lives;
    for (i, &input) in prefix.ticks.iter().enumerate() {
        route::play_directives(sim, prefix, i);
        sim.step(input);
        crabs.apply(sim);
        if sim.stats().lives < lives || claw_bot::search::dying(sim) {
            return Err(format!("Claw dies at tick {i} of the route continued"));
        }
    }
    route::play_directives(sim, prefix, prefix.ticks.len());
    Ok(())
}

/// A hints file's waypoints, and the indices of those marked `foot`.
#[derive(Debug, Default, PartialEq, Eq)]
struct Hints {
    points: Vec<(i32, i32)>,
    foot: Vec<usize>,
}

/// Reads a hints file: one `x,y` per line, which a `foot` after it makes
/// count only with Claw on his feet, on a ladder or on a rope near it (as
/// `--hints-on-foot` makes every waypoint count); blank lines and `#`
/// comments are skipped.
fn read_hints(path: &Path) -> Result<Hints, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    parse_hints(&text)
        .map_err(|line| format!("{} line {line}: expected x,y or x,y foot", path.display()))
}

/// Parses the text of a hints file; the error is the 1-based number of
/// the first line that is not a waypoint.
fn parse_hints(text: &str) -> Result<Hints, usize> {
    let mut out = Hints::default();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (point, foot) = match line.strip_suffix("foot") {
            Some(point) if point.ends_with(char::is_whitespace) => (point, true),
            _ => (line, false),
        };
        let p = point
            .split_once(',')
            .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
            .ok_or(i + 1)?;
        if foot {
            out.foot.push(out.points.len());
        }
        out.points.push(p);
    }
    Ok(out)
}

/// The hints name a route's header records: the stem of the `--hints`
/// file, or else the hints of the route it continues, whose leading ticks
/// were found with them.
fn route_hints(hints: Option<&Path>, prefix_hints: Option<String>) -> Option<String> {
    hints
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .or(prefix_hints)
}

/// What `--from-route` continues: the first ticks of another route and
/// what its header says about them.
#[derive(Debug, Default)]
struct Prefix {
    /// The ticks played before the search starts.
    script: InputScript,
    /// The hints file the route was found with.
    hints: Option<String>,
    /// The route's segment start (`# start X,Y`), which the continued
    /// route starts from too.
    start: Option<(i32, i32)>,
    /// The route's crab values and further sets.
    crabs: (CrabValues, Vec<CrabValues>),
}

/// The first `t` ticks of the route `text`, with its help screens and
/// pauses up to the boundary before tick `t`, for `--from-route`: its
/// `# level`, `# tuning` and `# seed` lines must match the arguments and
/// its progress (`# start_...` lines) `carry`, and it may hold no command
/// (the end of a life) at or before `t`. Its segment start, `# start X,Y`,
/// is always the continued route's, none when it has none.
fn read_prefix(
    text: &str,
    t: usize,
    level: u32,
    tuning: TuningName,
    seed: u32,
    carry: Option<&Carry>,
) -> Result<Prefix, String> {
    let script = InputScript::parse(text).map_err(|e| e.to_string())?;
    let header = Header::parse(text);
    prefix_matches(&script.header, level, tuning, seed)?;
    if t > script.ticks.len() {
        return Err(format!("the route has only {} ticks", script.ticks.len()));
    }
    if route::parse_carry(text)?.as_ref() != carry {
        return Err("the route starts with other progress than --carry gives".to_string());
    }
    // The search plays the prefix's help screens and pauses as the game
    // does; a route never ends a life on purpose.
    if let Some(d) = script
        .directives
        .iter()
        .find(|d| d.tick <= t && matches!(d.kind, DirectiveKind::Command(_)))
    {
        return Err(format!(
            "the route gives a command at tick {}, at or before the continue point {t}",
            d.tick
        ));
    }
    let mut ticks = InputScript::default();
    ticks.ticks = script.ticks[..t].to_vec();
    ticks.directives = script
        .directives
        .iter()
        .filter(|d| d.tick <= t)
        .cloned()
        .collect();
    Ok(Prefix {
        script: ticks,
        hints: header.hints,
        start: header.start,
        crabs: route::crab_values(text)?,
    })
}

/// Checks that the header keys a continued route states (level, tuning and
/// seed) are the ones the search runs with: a prefix played in another
/// game would lead a route that does not play as found.
fn prefix_matches(
    header: &ScriptHeader,
    level: u32,
    tuning: TuningName,
    seed: u32,
) -> Result<(), String> {
    if let Some(l) = header.level
        && l != level
    {
        return Err(format!("the route is for level {l}, not level {level}"));
    }
    if let Some(t) = header.tuning
        && t.name() != tuning.name()
    {
        return Err(format!(
            "the route is for tuning {}, not tuning {}",
            t.name(),
            tuning.name()
        ));
    }
    if let Some(s) = header.seed
        && s != seed
    {
        return Err(format!("the route is for seed {s}, not seed {seed}"));
    }
    Ok(())
}

/// The mask held in the last tick of `prefix` and the ticks it has been
/// held since it last changed (`usize::MAX` when it never changed).
fn held_at_end(prefix: &InputScript) -> (u32, usize) {
    let held = prefix.ticks.last().map_or(0, |t| t.held.0);
    let since = route::changes(prefix)
        .last()
        .map_or(usize::MAX, |&c| prefix.ticks.len() - c);
    (held, since)
}

fn run(a: Args) -> Result<bool, String> {
    let install_path = a.install.clone().ok_or(USAGE)?;
    let level_no = a.level.ok_or(USAGE)?;
    let out = a.out.clone().ok_or(USAGE)?;
    let install = Install::open(&install_path)
        .map_err(|e| format!("opening install {}: {e}", install_path.display()))?;
    let store = install
        .store()
        .map_err(|e| format!("mounting assets: {e}"))?;
    let level = LevelData::build(&store, LevelSource::Retail(level_no))
        .map_err(|e| format!("loading level {level_no}: {e}"))?;
    let sim_level =
        SimLevel::from_level(&level, &store).map_err(|e| format!("preparing level: {e}"))?;
    let carry = match &a.carry {
        Some(p) => {
            let text =
                std::fs::read_to_string(p).map_err(|e| format!("reading {}: {e}", p.display()))?;
            let c = route::parse_carry(&text).map_err(|e| format!("{}: {e}", p.display()))?;
            Some(c.ok_or_else(|| format!("{} has no start lines", p.display()))?)
        }
        None => None,
    };
    let seed = match a.seed {
        Seed::Value(s) => s,
        Seed::Level => {
            let s = start_sim(
                sim_level.clone(),
                a.tuning,
                1,
                None,
                carry.as_ref(),
                &CrabValues::default(),
            )
            .rng_after_set_up();
            eprintln!("claw-bot: the level's set-up leaves the random state at {s}");
            s
        }
    };
    let mut prefix = Prefix::default();
    if let (Some(path), Some(t)) = (&a.from_route, a.from_tick) {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        prefix = read_prefix(&text, t, level_no, a.tuning, seed, carry.as_ref())
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let Prefix {
        script: prefix,
        hints: prefix_hints,
        start: prefix_start,
        crabs: prefix_crabs,
    } = prefix;
    let seg_start = if a.from_route.is_some() {
        prefix_start
    } else {
        a.start
    };
    let crabs = match &a.crabs {
        Some(p) => read_crabs(p)?,
        None => prefix_crabs.0,
    };
    let crab_variants = if a.crab_variants.is_empty() {
        prefix_crabs.1
    } else {
        a.crab_variants
            .iter()
            .map(|p| read_crabs(p))
            .collect::<Result<Vec<_>, _>>()?
    };
    let begin = |c: &CrabValues| -> Result<Sim, String> {
        let mut sim = start_sim(
            sim_level.clone(),
            a.tuning,
            seed,
            seg_start,
            carry.as_ref(),
            c,
        );
        play_prefix(&mut sim, &prefix, c)?;
        Ok(sim)
    };
    let start = begin(&crabs)?;
    let mut variant_starts = Vec::with_capacity(crab_variants.len());
    for (i, v) in crab_variants.iter().enumerate() {
        let sim = begin(v).map_err(|e| format!("under the further crab values {}: {e}", i + 1))?;
        variant_starts.push((sim, v.clone()));
    }

    let hints = match &a.hints {
        Some(p) => read_hints(p)?,
        None => Hints::default(),
    };
    let (start_held, start_held_for) = held_at_end(&prefix);
    let scfg = SearchConfig {
        start_held,
        start_held_for,
        nodes: a.nodes,
        seed: u64::from(seed),
        hints: hints.points,
        foot_hints: hints.foot,
        threads: a.threads,
        land_gap: a.land_gap.unwrap_or(match a.tuning {
            TuningName::Default => DEFAULT_LAND_GAP,
            TuningName::Original => 0,
        }),
        fragile_cost: a.fragile_cost,
        cached_sims: a.cache_games,
        shadows: a.shadows,
        hint_on_foot: a.hints_on_foot,
        intro: a.intro,
        wall_limit: a.budget_min.map(|m| Duration::from_secs_f64(m * 60.0)),
        log_every: if a.quiet { 0 } else { 500 },
        crabs: crabs.clone(),
        drift_cost: a.drift_cost,
        ..SearchConfig::default()
    };
    let goal = match a.goal {
        Some((x, y)) if a.goal_on_foot => Goal::Stand {
            x,
            y,
            radius: GOAL_RADIUS,
        },
        Some((x, y)) => Goal::Reach {
            x,
            y,
            radius: GOAL_RADIUS,
        },
        None => Goal::LevelComplete,
    };
    let began = Instant::now();
    let found = match search_report_with(&start, &variant_starts, goal.clone(), &scfg) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("claw-bot: no route: {e}");
            return Ok(false);
        }
    };
    let seconds = began.elapsed().as_secs_f64();
    // The search played its first tick against the prefix's last mask, so
    // the route is the prefix and the search's ticks exactly as played.
    let mut script = prefix.clone();
    script.ticks.extend(found.script.ticks.iter().copied());
    let too_close = route::spacing_violations(&script, MIN_SPACING);
    if let Some(&(tick, gap)) = too_close.first() {
        return Err(format!(
            "not writing the route: the held mask changes at tick {tick}, {gap} tick(s) after the previous change ({} change(s) too close in all; at least {MIN_SPACING} ticks are needed)",
            too_close.len()
        ));
    }
    let header = Header {
        level: level_no,
        tuning: a.tuning,
        seed,
        mid_air_hit_fix: false,
        hints: route_hints(a.hints.as_deref(), prefix_hints),
        start: seg_start,
        carry: carry.clone(),
        crabs: crabs.clone(),
        crab_variants: crab_variants.clone(),
    };
    std::fs::write(&out, route::route_csv(&header, &script))
        .map_err(|e| format!("writing {}: {e}", out.display()))?;

    // The level as loaded: the game after the prefix holds the level as
    // the prefix left it (a breaking plank's floor gone, say), not as a
    // route from the start finds it.
    let fresh = start_sim(
        sim_level.clone(),
        a.tuning,
        seed,
        seg_start,
        carry.as_ref(),
        &crabs,
    );
    let played = route::play_with(&fresh, &script, &goal, &crabs);
    if !played.won() {
        return Err("the route found does not win when played from the start".into());
    }
    let margin = route::margin_with(&fresh, &script, &goal, a.margin_samples, &crabs);
    let mut variants = Vec::with_capacity(crab_variants.len());
    // Under further values the route need only come within twice a goal
    // point's distance of it, as the search asks.
    let variant_goal = match goal {
        Goal::LevelComplete => Goal::LevelComplete,
        Goal::Reach { x, y, radius } => Goal::Reach {
            x,
            y,
            radius: 2 * radius,
        },
        Goal::Stand { x, y, radius } => Goal::Stand {
            x,
            y,
            radius: 2 * radius,
        },
    };
    for (i, v) in crab_variants.iter().enumerate() {
        let fresh = start_sim(
            sim_level.clone(),
            a.tuning,
            seed,
            seg_start,
            carry.as_ref(),
            v,
        );
        let p = route::play_with(&fresh, &script, &variant_goal, v);
        if !p.won() {
            return Err(format!(
                "the route found does not win under the further crab values {} when played from the start",
                i + 1
            ));
        }
        let m = route::margin_with(&fresh, &script, &variant_goal, a.margin_samples, v);
        eprintln!(
            "claw-bot: under the further crab values {}: {} at tick {}, {} health lost, margin {}/{} early and {}/{} late",
            i + 1,
            if a.goal.is_some() {
                "goal reached"
            } else {
                "level complete"
            },
            p.done_at.unwrap_or_default(),
            p.health_lost,
            m.early_wins,
            m.sampled,
            m.late_wins,
            m.sampled
        );
        variants.push((p, m));
    }
    let mut stats = found.stats.clone();
    stats.ticks = script.ticks.len() as u64;
    stats.health_lost = played.health_lost;
    let json = out.with_extension("json");
    let done_at = played.done_at.unwrap_or_default();
    std::fs::write(
        &json,
        route::report_json(&header, &stats, done_at, &margin, &variants, seconds),
    )
    .map_err(|e| format!("writing {}: {e}", json.display()))?;
    eprintln!(
        "claw-bot: route of {} ticks ({:.1} s of play), {} at tick {}, {} nodes, {} segments, {} deaths, {} health lost, margin {}/{} early and {}/{} late, {:.0} s; wrote {} and {}",
        script.ticks.len(),
        script.ticks.len() as f64 * 0.018,
        if a.goal.is_some() {
            "goal reached"
        } else {
            "level complete"
        },
        done_at,
        stats.expanded,
        stats.segments,
        stats.deaths,
        stats.health_lost,
        margin.early_wins,
        margin.sampled,
        margin.late_wins,
        margin.sampled,
        seconds,
        out.display(),
        json.display()
    );
    Ok(true)
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("claw-bot: {e}");
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("claw-bot: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Result<Args, String> {
        parse_args(s.split_whitespace().map(String::from))
    }

    #[test]
    fn a_hint_marked_foot_counts_only_on_foot() {
        let text = "# shaft\n100,200\n\n300, 400 foot\n500,600\t foot\n700,800\n";
        let hints = parse_hints(text).unwrap();
        assert_eq!(
            hints.points,
            [(100, 200), (300, 400), (500, 600), (700, 800)]
        );
        assert_eq!(hints.foot, [1, 2]);
        assert_eq!(parse_hints("100,200\n300,400 feet\n"), Err(2));
        assert_eq!(parse_hints("100\n"), Err(1));
    }

    #[test]
    fn a_continued_route_starts_where_the_route_it_continues_starts() {
        let rows: String = (0..10).map(|t| format!("{t},0x0,0x0\n")).collect();
        let read = |head: &str, t| {
            let text = format!("# level 8\n# tuning original\n# seed 7\n{head}{rows}");
            read_prefix(&text, t, 8, TuningName::Original, 7, None)
        };
        // A segment's prefix: the continued route is a segment from the
        // same start point, with the prefix's hints and first ticks.
        let p = read("# hints level8\n# start 31700,5200\n", 4).unwrap();
        assert_eq!(p.start, Some((31700, 5200)));
        assert_eq!(p.hints, Some("level8".into()));
        assert_eq!(p.script.ticks.len(), 4);
        // A prefix from the level's start: no start point.
        assert_eq!(read("", 10).unwrap().start, None);
        // The prefix's header must match the search's game.
        assert!(read("", 11).unwrap_err().contains("only 10 ticks"));
        let other = format!("# level 7\n{rows}");
        assert!(read_prefix(&other, 1, 8, TuningName::Original, 7, None).is_err());
        let e = read("# start_score = 5\n", 1).unwrap_err();
        assert!(e.contains("start_"), "{e}");
    }

    #[test]
    fn a_continued_route_keeps_the_help_screens_and_pauses_before_its_continue_point() {
        let mut text =
            String::from("# level 9\n# tuning original\n# seed 7\ntick,held_mask,pressed_mask\n");
        for t in 0..10 {
            if t == 3 {
                text.push_str("@3 menu help 3000\n");
            }
            if t == 6 {
                text.push_str("@6 pause 400\n");
            }
            text.push_str(&format!("{t},0x0,0x0\n"));
        }
        let read = |text: &str, t| read_prefix(text, t, 9, TuningName::Original, 7, None);
        // Those up to the boundary before the continue point come along;
        // one at the continue point itself is due before the search's
        // first tick.
        let ticks = |p: &Prefix| {
            p.script
                .directives
                .iter()
                .map(|d| d.tick)
                .collect::<Vec<_>>()
        };
        assert_eq!(ticks(&read(&text, 2).unwrap()), Vec::<usize>::new());
        assert_eq!(ticks(&read(&text, 3).unwrap()), [3]);
        assert_eq!(ticks(&read(&text, 10).unwrap()), [3, 6]);
        // A route never ends a life on purpose.
        let ended = text.replace("@6 pause 400", "@6 command end_life");
        assert!(read(&ended, 5).is_ok());
        let e = read(&ended, 6).unwrap_err();
        assert!(e.contains("command at tick 6"), "{e}");
    }

    #[test]
    fn a_continued_route_names_the_hints_of_the_route_it_continues() {
        let prefix = Header::parse("# level 2\n# tuning default\n# hints level2\n").hints;
        assert_eq!(route_hints(None, prefix.clone()), Some("level2".into()));
        assert_eq!(
            route_hints(Some(Path::new("tools/bot/hints/other.txt")), prefix),
            Some("other".into())
        );
        assert_eq!(route_hints(None, None), None);
    }

    #[test]
    fn parses_the_documented_arguments() {
        let a = args("--install /i --level 1 --out r.csv --tuning original --nodes 5000 --budget-min 30 --seed 7 --hints h.txt --from-route p.csv --from-tick 100 --land-gap 3 --threads 4 --quiet").unwrap();
        assert_eq!(a.land_gap, Some(3));
        assert_eq!(args("--cache-games 1536").map(|a| a.cache_games), Ok(1536));
        assert!(!args("--no-shadows").unwrap().shadows);
        assert!(args("--hints-on-foot").unwrap().hints_on_foot);
        assert!(!a.hints_on_foot);
        assert!(a.shadows);
        assert_eq!(a.fragile_cost, DEFAULT_FRAGILE_COST);
        assert_eq!(args("--fragile-cost 0").map(|a| a.fragile_cost), Ok(0));
        assert!(args("--fragile-cost -1").is_err());
        let s = args("--install /i --level 1 --out r.csv --start 10355,2051").unwrap();
        assert_eq!(s.start, Some((10355, 2051)));
        assert!(args("--start 10").is_err());
        assert!(args("--start 1,2 --from-route p.csv --from-tick 3").is_err());
        assert_eq!(
            args("--goal 4851,3340").map(|a| a.goal),
            Ok(Some((4851, 3340)))
        );
        assert_eq!(a.goal, None);
        assert!(args("--goal 4851").is_err());
        assert!(!a.goal_on_foot);
        assert!(args("--goal 1,2 --goal-on-foot").unwrap().goal_on_foot);
        assert!(args("--goal-on-foot").is_err());
        assert_eq!(a.intro, Intro::Any);
        assert_eq!(args("--intro skip").map(|a| a.intro), Ok(Intro::Skip));
        assert_eq!(args("--intro play").map(|a| a.intro), Ok(Intro::Play));
        assert!(args("--intro maybe").is_err());
        assert_eq!(
            args("--carry c.csv").map(|a| a.carry),
            Ok(Some(PathBuf::from("c.csv")))
        );
        assert_eq!(
            args("--crabs c.txt").map(|a| a.crabs),
            Ok(Some(PathBuf::from("c.txt")))
        );
        assert_eq!(a.crabs, None);
        assert!(a.crab_variants.is_empty());
        assert_eq!(
            args("--crab-variants a.txt,b.txt").map(|a| a.crab_variants),
            Ok(vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")])
        );
        assert!(args("--crab-variants a.txt,").is_err());
        assert_eq!(a.drift_cost, 1);
        assert_eq!(args("--drift-cost 4").map(|a| a.drift_cost), Ok(4));
        assert!(args("--drift-cost -1").is_err());
        assert_eq!(a.level, Some(1));
        assert_eq!(a.tuning, TuningName::Original);
        assert_eq!(a.nodes, 5000);
        assert_eq!(a.budget_min, Some(30.0));
        assert_eq!(a.seed, Seed::Value(7));
        assert_eq!(args("--seed level").map(|a| a.seed), Ok(Seed::Level));
        assert!(args("--seed levels").is_err());
        assert_eq!(a.from_tick, Some(100));
        assert_eq!(a.threads, 4);
        assert!(a.quiet);
        assert!(args("--tuning fast").is_err());
        assert!(args("--from-route p.csv").is_err());
        assert!(args("--budget-min 0").is_err());
        assert!(args("--bogus 1").is_err());
    }

    #[test]
    fn the_held_mask_at_the_end_of_a_prefix_counts_ticks_since_its_change() {
        assert_eq!(held_at_end(&InputScript::default()), (0, usize::MAX));
        assert_eq!(
            held_at_end(&InputScript::from_held(&[0, 0, 0])),
            (0, usize::MAX)
        );
        assert_eq!(held_at_end(&InputScript::from_held(&[4, 4, 4])), (4, 3));
        assert_eq!(held_at_end(&InputScript::from_held(&[0, 4, 4, 5])), (5, 1));
        assert_eq!(held_at_end(&InputScript::from_held(&[5, 5, 0, 0])), (0, 2));
    }

    #[test]
    fn a_continued_route_must_state_the_same_level_tuning_and_seed() {
        let header = |text: &str| InputScript::parse(text).unwrap().header;
        let full = header("# level 1\n# tuning original\n# seed 7\n0,0,0\n");
        assert!(prefix_matches(&full, 1, TuningName::Original, 7).is_ok());
        let e = prefix_matches(&full, 2, TuningName::Original, 7).unwrap_err();
        assert!(e.contains("level 1"), "{e}");
        let e = prefix_matches(&full, 1, TuningName::Default, 7).unwrap_err();
        assert!(e.contains("tuning original"), "{e}");
        let e = prefix_matches(&full, 1, TuningName::Original, 8).unwrap_err();
        assert!(e.contains("seed 7"), "{e}");
        // Keys the route leaves out are not checked.
        let bare = header("0,0,0\n");
        assert!(prefix_matches(&bare, 3, TuningName::Default, 99).is_ok());
    }
}
