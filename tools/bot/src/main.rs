//! `claw-bot`: searches the headless simulation of a level from your own
//! install for a route that finishes it, and writes the route.
//!
//! ```text
//! claw-bot --install PATH --level N --out ROUTE.csv
//!          [--tuning default|original] [--nodes N] [--budget-min M]
//!          [--seed S] [--hints FILE] [--from-route FILE --from-tick T]
//!          [--start X,Y] [--land-gap N] [--fragile-cost PX] [--threads N]
//!          [--margin-samples N] [--quiet]
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
//! route's time zero (a reference trace's `rng_at_t0`), and breaks ties.
//! Keys down when the level starts count only once released and pressed
//! again, so a route's first press comes after its first tick.
//! `--hints` names a file of `x,y` waypoints, one per line, written from
//! the level's geometry; the file's name is recorded in the header.
//! `--from-route` starts from the game after the first T ticks of another
//! route, which lead the new one. When the prefix changed its held mask
//! less than two ticks before T, the new part first keeps that mask for
//! the ticks missing. The other route's `# level`, `# tuning` and
//! `# seed` lines, when it has them, must match the arguments.
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
//! 400; 0 turns the check off.
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
use claw_bot::{Goal, SearchConfig, search_report};
use claw_sim::{InputScript, ScriptHeader, Sim, SimLevel};

const USAGE: &str = "usage: claw-bot --install PATH --level N --out ROUTE.csv \
[--tuning default|original] [--nodes N] [--budget-min M] [--seed S] [--hints FILE] \
[--from-route FILE --from-tick T] [--start X,Y] [--land-gap N] [--fragile-cost PX] \
[--threads N] [--margin-samples N] [--quiet]";

/// Priority cost of a fragile move, px.
const DEFAULT_FRAGILE_COST: i64 = 400;

/// Ticks a jump press keeps from a landing with the default tuning.
const DEFAULT_LAND_GAP: u64 = 2;

#[derive(Debug)]
struct Args {
    install: Option<PathBuf>,
    level: Option<u32>,
    out: Option<PathBuf>,
    tuning: TuningName,
    nodes: u64,
    budget_min: Option<f64>,
    seed: u32,
    hints: Option<PathBuf>,
    from_route: Option<PathBuf>,
    from_tick: Option<usize>,
    start: Option<(i32, i32)>,
    land_gap: Option<u64>,
    fragile_cost: i64,
    threads: usize,
    margin_samples: usize,
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
            seed: 1,
            hints: None,
            from_route: None,
            from_tick: None,
            start: None,
            land_gap: None,
            fragile_cost: DEFAULT_FRAGILE_COST,
            threads: 1,
            margin_samples: 40,
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
            "--seed" => a.seed = v.parse().map_err(|_| num("seed"))?,
            "--hints" => a.hints = Some(PathBuf::from(&v)),
            "--from-route" => a.from_route = Some(PathBuf::from(&v)),
            "--from-tick" => a.from_tick = Some(v.parse().map_err(|_| num("tick"))?),
            "--start" => {
                a.start = Some(
                    v.split_once(',')
                        .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
                        .ok_or_else(|| num("start point"))?,
                );
            }
            "--land-gap" => a.land_gap = Some(v.parse().map_err(|_| num("landing gap"))?),
            "--fragile-cost" => {
                a.fragile_cost = v.parse().map_err(|_| num("fragile cost"))?;
                if a.fragile_cost < 0 {
                    return Err(num("fragile cost"));
                }
            }
            "--threads" => a.threads = v.parse::<usize>().map_err(|_| num("thread count"))?.max(1),
            "--margin-samples" => a.margin_samples = v.parse().map_err(|_| num("sample count"))?,
            other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
        }
    }
    if a.from_route.is_some() != a.from_tick.is_some() {
        return Err(format!("--from-route and --from-tick go together\n{USAGE}"));
    }
    if a.start.is_some() && a.from_route.is_some() {
        return Err(format!(
            "--start and --from-route do not go together\n{USAGE}"
        ));
    }
    Ok(a)
}

/// The game a route starts from: the level start, or with `start` the
/// segment start (see [`route::settle_at`]).
fn start_sim(level: SimLevel, tuning: TuningName, seed: u32, start: Option<(i32, i32)>) -> Sim {
    let mut sim = Sim::new(level, route::sim_config(tuning, seed));
    if let Some((x, y)) = start {
        route::settle_at(&mut sim, x, y, seed);
    }
    sim
}

/// Reads a hints file: one `x,y` per line; blank lines and `#` comments
/// are skipped.
fn read_hints(path: &Path) -> Result<Vec<(i32, i32)>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let p = line
            .split_once(',')
            .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
            .ok_or_else(|| format!("{} line {}: expected x,y", path.display(), i + 1))?;
        out.push(p);
    }
    Ok(out)
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
    let mut start = start_sim(sim_level, a.tuning, a.seed, a.start);

    let mut prefix = InputScript::default();
    if let (Some(path), Some(t)) = (&a.from_route, a.from_tick) {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        let script = InputScript::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        prefix_matches(&script.header, level_no, a.tuning, a.seed)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if t > script.ticks.len() {
            return Err(format!(
                "{} has only {} ticks",
                path.display(),
                script.ticks.len()
            ));
        }
        // The search plays the prefix as plain ticks, so a directive inside
        // it would make the continued route play differently.
        if let Some(d) = script.directives.iter().find(|d| d.tick <= t) {
            return Err(format!(
                "{} has a directive at tick {}, at or before the continue point {t}; only plain ticks can be continued",
                path.display(),
                d.tick
            ));
        }
        prefix.ticks = script.ticks[..t].to_vec();
        for &input in &prefix.ticks {
            start.step(input);
        }
    }

    let hints = match &a.hints {
        Some(p) => read_hints(p)?,
        None => Vec::new(),
    };
    let (start_held, start_held_for) = held_at_end(&prefix);
    let scfg = SearchConfig {
        start_held,
        start_held_for,
        nodes: a.nodes,
        seed: u64::from(a.seed),
        hints,
        threads: a.threads,
        land_gap: a.land_gap.unwrap_or(match a.tuning {
            TuningName::Default => DEFAULT_LAND_GAP,
            TuningName::Original => 0,
        }),
        fragile_cost: a.fragile_cost,
        wall_limit: a.budget_min.map(|m| Duration::from_secs_f64(m * 60.0)),
        log_every: if a.quiet { 0 } else { 500 },
        ..SearchConfig::default()
    };
    let began = Instant::now();
    let found = match search_report(&start, Goal::LevelComplete, &scfg) {
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
        seed: a.seed,
        mid_air_hit_fix: false,
        hints: a
            .hints
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned()),
        start: a.start,
    };
    std::fs::write(&out, route::route_csv(&header, &script))
        .map_err(|e| format!("writing {}: {e}", out.display()))?;

    let fresh = start_sim(start.level().clone(), a.tuning, a.seed, a.start);
    let played = route::play(&fresh, &script, &Goal::LevelComplete);
    if !played.won() {
        return Err("the route found does not win when played from the start".into());
    }
    let margin = route::margin(&fresh, &script, &Goal::LevelComplete, a.margin_samples);
    let mut stats = found.stats.clone();
    stats.ticks = script.ticks.len() as u64;
    stats.health_lost = played.health_lost;
    let json = out.with_extension("json");
    let done_at = played.done_at.unwrap_or_default();
    std::fs::write(
        &json,
        route::report_json(&header, &stats, done_at, &margin, seconds),
    )
    .map_err(|e| format!("writing {}: {e}", json.display()))?;
    eprintln!(
        "claw-bot: route of {} ticks ({:.1} s of play), level complete at tick {}, {} nodes, {} segments, {} deaths, {} health lost, margin {}/{} early and {}/{} late, {:.0} s; wrote {} and {}",
        script.ticks.len(),
        script.ticks.len() as f64 * 0.018,
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
    fn parses_the_documented_arguments() {
        let a = args("--install /i --level 1 --out r.csv --tuning original --nodes 5000 --budget-min 30 --seed 7 --hints h.txt --from-route p.csv --from-tick 100 --land-gap 3 --threads 4 --quiet").unwrap();
        assert_eq!(a.land_gap, Some(3));
        assert_eq!(a.fragile_cost, DEFAULT_FRAGILE_COST);
        assert_eq!(args("--fragile-cost 0").map(|a| a.fragile_cost), Ok(0));
        assert!(args("--fragile-cost -1").is_err());
        let s = args("--install /i --level 1 --out r.csv --start 10355,2051").unwrap();
        assert_eq!(s.start, Some((10355, 2051)));
        assert!(args("--start 10").is_err());
        assert!(args("--start 1,2 --from-route p.csv --from-tick 3").is_err());
        assert_eq!(a.level, Some(1));
        assert_eq!(a.tuning, TuningName::Original);
        assert_eq!(a.nodes, 5000);
        assert_eq!(a.budget_min, Some(30.0));
        assert_eq!(a.seed, 7);
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
