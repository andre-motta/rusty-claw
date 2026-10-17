//! `rusty-claw`: play Captain Claw from your own install.
//!
//! ```text
//! rusty-claw --install PATH [--level N | --custom FILE.WWD] [--res WxH]
//!            [--fullscreen] [--aspect fill|centered|4:3] [--scale fit|integer]
//!            [--widen-activation] [--mid-air-hit-fix on|off]
//!            [--vsync on|off] [--fps-cap N | --uncapped]
//!            [--no-interp] [--no-audio] [--record FILE.csv]
//!            [--replay FILE.csv [--trace OUT.csv] [--render-hz HZ]]
//!            [--bench SECONDS [--headless] [--render-hz HZ]]
//!            [--bench-route FILE.csv --headless [--bench SECONDS]
//!             [--render-hz HZ]]
//! ```
//!
//! With only `--install` (and options), the game starts at the title and
//! its main menu. `--level` and `--custom` start straight into that level
//! with the in-game menu, the end-of-level tally and the main menu after
//! it; `--replay` and `--bench` play one level without menus. Run
//! `rusty-claw --help` for every option; settings can also come from a
//! config file (see the `settings` module).
//!
//! A replay plays with the mid-air hit jump fix its script was recorded
//! with (off when the script does not say), unless `--mid-air-hit-fix`
//! is given; a benchmark plays with it off unless the option is given. A
//! replay also plays with the movement tuning and the random seed its
//! script's header names (`# tuning default|original`, `# seed N`), and
//! plays its pauses, menus and commands (`@` lines) where they were
//! recorded.
//!
//! `--bench-route FILE.csv --headless` benchmarks a route instead of the
//! combat route: the input script plays from the level's start with the
//! level, tuning, seed and behaviour switches its header names, as a
//! replay plays it, until its last row has run (or for at most `--bench
//! SECONDS`), drawn offscreen at `--res`. Its report also says whether any
//! sprite set was loaded while drawing (a budget: none may be) and gives a
//! digest of the route's trace, the same on every run.
//!
//! A benchmark prints its report as one line of JSON on standard output and
//! a one-line PASS or FAIL verdict naming each budget on standard error. It
//! exits with status [`BUDGET_FAILED`] when a measured budget is missed.

use std::process::ExitCode;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Arg, ArgAction, ArgMatches, CommandFactory, FromArgMatches};
use claw_game::bench::{Report, RouteFile, RouteReport, run_headless, run_headless_route};
use claw_game::session::script_behaviour;
use claw_game::settings::{Args, Aspect, Mode, Settings};
use claw_game::window::{self, WindowMode};
use claw_game::{load_graphics, load_level, replay_headless};
use claw_sim::InputScript;

/// Exit status of a benchmark that missed a measured budget.
pub const BUDGET_FAILED: u8 = 3;

/// What `--help` adds after the options.
const AFTER_HELP: &str = "\
How a run starts:
  rusty-claw --install PATH          the title, then the main menu: new game,
                                     load game, options and quit
  rusty-claw --install PATH --level N (or --custom FILE.WWD)
                                     straight into that level; Escape, P or
                                     the pad's Start button open the in-game menu
  --replay FILE.csv, --bench SECONDS one level without menus, as before
  --bench-route FILE.csv --headless  benchmark along a route from the level's
                                     start until its end

Saved games are kept in $XDG_DATA_HOME/rusty-claw/saves/default/ (by default
~/.local/share/rusty-claw/saves/default/). Options changed in the menus are
written back to the config file; options given on the command line are not,
unless changed in the menus.";

/// What a run ended with.
enum Outcome {
    /// Done.
    Done,
    /// A benchmark finished; the report decides the exit status.
    Bench(Box<Report>),
    /// A route benchmark finished; its report decides the exit status.
    RouteBench(Box<RouteReport>),
}

fn bench_result(report: Report) -> Outcome {
    println!("{}", report.to_json());
    eprintln!("rusty-claw: {}", report.summary());
    Outcome::Bench(Box::new(report))
}

/// The id of the `--bench-route` option.
const BENCH_ROUTE: &str = "bench_route";

/// The options the game adds to the settings' own: `--bench-route`, and
/// `--headless` with it as well as with `--bench`.
fn command() -> clap::Command {
    Args::command()
        .after_help(AFTER_HELP)
        // The same option without its tie to --bench alone; run() checks
        // that one of the two is given.
        .mut_arg("headless", |_| {
            Arg::new("headless")
                .long("headless")
                .action(ArgAction::SetTrue)
                .help(
                    "With --bench or --bench-route: render offscreen at --res with a \
                     simulated display clock instead of in a window, paced to that clock \
                     unless --uncapped is given",
                )
        })
        .arg(
            Arg::new(BENCH_ROUTE)
                .long("bench-route")
                .value_name("FILE.csv")
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .conflicts_with_all(["replay", "record", "trace"])
                .help(
                    "Benchmark along a route (an input script) from the level's start \
                     until its end, offscreen with --headless; --bench SECONDS stops it \
                     sooner",
                ),
        )
}

/// A route benchmark: the settings follow the route as a replay would,
/// and the run lasts until the route's end, or `--bench SECONDS`.
fn bench_route(mut args: Args, path: &std::path::Path) -> Result<Outcome> {
    if !args.headless {
        bail!("--bench-route runs offscreen: add --headless");
    }
    let cap = args.bench;
    // The settings of a benchmark (no menus, the behaviour switches at
    // their defaults unless given), whatever the length.
    args.bench = Some(cap.unwrap_or(1.0));
    let mut settings = Settings::from_args(&args)?;
    let route = RouteFile::read(path)?;
    route.apply(&mut settings);
    let Mode::Bench { render_hz, .. } = settings.mode else {
        bail!("--bench-route is a benchmark");
    };
    let report = run_headless_route(&settings, &route, cap, render_hz)?;
    println!("{}", report.to_json());
    eprintln!("rusty-claw: {}", report.summary());
    Ok(Outcome::RouteBench(Box::new(report)))
}

/// Reads an input script and the mid-air hit jump fix it was recorded
/// with, if it says.
fn read_script(path: &std::path::Path) -> Result<(InputScript, Option<bool>)> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading input script {}", path.display()))?;
    let script =
        InputScript::parse(&text).with_context(|| format!("input script {}", path.display()))?;
    Ok((script, script_behaviour(&text)))
}

fn run(args: Args, matches: &ArgMatches) -> Result<Outcome> {
    if let Some(path) = matches.get_one::<std::path::PathBuf>(BENCH_ROUTE) {
        return bench_route(args, path);
    }
    if args.headless && args.bench.is_none() {
        bail!("--headless goes with --bench or --bench-route");
    }
    let mut settings = Settings::from_args(&args)?;
    if settings.widen_activation && settings.aspect != Aspect::Fill {
        eprintln!(
            "rusty-claw: --widen-activation applies to the fill aspect only; \
             objects stay active as in the original view"
        );
    }
    let started = Instant::now();
    match settings.mode.clone() {
        Mode::Replay {
            script,
            trace: Some(out),
            render_hz,
        } => {
            let (script, recorded) = read_script(&script)?;
            settings.follow_script(recorded);
            settings.follow_header(&script.header);
            let bundle = load_level(&settings.install, &settings.level)?;
            let rows = replay_headless(&bundle, &settings, script, render_hz, None);
            std::fs::write(&out, claw_sim::trace::to_csv(&rows))
                .with_context(|| format!("writing {}", out.display()))?;
            eprintln!(
                "rusty-claw: replayed {} ticks of {} at a simulated {render_hz} Hz display, \
                 {} activation, trace {}",
                rows.len(),
                bundle.name,
                rows.first().map_or("original", |r| r.activation),
                out.display()
            );
            Ok(Outcome::Done)
        }
        Mode::Bench {
            seconds,
            headless: true,
            render_hz,
        } => {
            let report = run_headless(&settings, seconds, render_hz)?;
            Ok(bench_result(report))
        }
        // Without a level on the command line the game starts at the title.
        Mode::Play if !settings.from_command_line.contains("level") => {
            window::run_menus(settings, started)?;
            Ok(Outcome::Done)
        }
        mode => {
            let window_mode = match mode {
                Mode::Replay { script, .. } => {
                    let (script, recorded) = read_script(&script)?;
                    settings.follow_script(recorded);
                    settings.follow_header(&script.header);
                    WindowMode::Watch(script)
                }
                Mode::Bench { seconds, .. } => WindowMode::Bench(seconds),
                Mode::Play => WindowMode::Play,
            };
            let bundle = load_level(&settings.install, &settings.level)?;
            let graphics = load_graphics(&bundle, settings.pack.as_deref())?;
            for w in graphics.pack_warnings() {
                eprintln!("rusty-claw: image pack: {w}");
            }
            match window::run(settings, window_mode, bundle, graphics, started)? {
                Some(report) => Ok(bench_result(report)),
                None => Ok(Outcome::Done),
            }
        }
    }
}

fn parse_args() -> (Args, ArgMatches) {
    let matches = command().get_matches();
    let args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    (args, matches)
}

fn main() -> ExitCode {
    let (args, matches) = parse_args();
    match run(args, &matches) {
        Ok(Outcome::Done) => ExitCode::SUCCESS,
        Ok(Outcome::Bench(report)) if report.passed() => ExitCode::SUCCESS,
        Ok(Outcome::Bench(_)) => ExitCode::from(BUDGET_FAILED),
        Ok(Outcome::RouteBench(report)) if report.passed() => ExitCode::SUCCESS,
        Ok(Outcome::RouteBench(_)) => ExitCode::from(BUDGET_FAILED),
        Err(e) => {
            eprintln!("rusty-claw: {e:#}");
            ExitCode::FAILURE
        }
    }
}
