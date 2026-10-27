//! `rusty-claw`: play Captain Claw from your own install.
//!
//! ```text
//! rusty-claw --install PATH [--level N | --custom FILE.WWD] [--res WxH]
//!            [--fullscreen] [--aspect fill|centered|4:3] [--scale fit|integer]
//!            [--widen-activation] [--mid-air-hit-fix on|off]
//!            [--invincible] [--infinite-lives] [--powerup NAME[:SECONDS]]
//!            [--vsync on|off] [--fps-cap N | --uncapped]
//!            [--no-interp] [--smooth-rope on|off] [--no-audio]
//!            [--no-movies] [--record FILE.csv]
//!            [--replay FILE.csv [--trace OUT.csv] [--render-hz HZ]]
//!            [--bench SECONDS [--headless] [--render-hz HZ]]
//!            [--bench-route FILE.csv --headless [--bench SECONDS]
//!             [--render-hz HZ]]
//! rusty-claw --install PATH --movie INDEX|FILE [--no-audio]
//! ```
//!
//! With only `--install` (and options), the game starts at the title and
//! its main menu, after the start-up movies when the build plays movies
//! and finds them: the logo then the intro on a first run, the logo only
//! later (none with `skip_logo = on`). `--level` and `--custom` start
//! straight into that level with the in-game menu, the end-of-level tally
//! and the main menu after it; `--replay` and `--bench` play one level without menus. A replay in
//! the window plays the game's sound and music as play does (silent with
//! `--no-audio`); the window benchmark and a replay with `--trace` are
//! silent. Run
//! `rusty-claw --help` for every option; settings can also come from a
//! config file (see the `settings` module).
//!
//! A replay plays with the mid-air hit jump fix its script was recorded
//! with (off when the script does not say), unless `--mid-air-hit-fix`
//! is given; a benchmark plays with it off unless the option is given. A
//! replay also plays with the movement tuning and the random seed its
//! script's header names (`# tuning default|original`, `# seed N`), and
//! plays its pauses, menus and commands (`@` lines) where they were
//! recorded. Its hermit crabs start with the side, second range box and
//! stationary value its header gives each (`# crab RECORD = SIDE,
//! HALF_WIDTH, HALF_HEIGHT[, STATIONARY]` and `# nest_crab NEST_RECORD
//! HATCH = ...` lines), as `claw-replay` gives them; a crab no line names
//! starts with the plain default.
//!
//! The play-test aids `--invincible` (Claw takes no damage; death tiles and
//! pits still end his life) and `--infinite-lives` (a death never costs a
//! life) are on for that run only, as is `--powerup NAME[:SECONDS]` (Claw
//! has that powerup, as if just picked up, at every level start and after
//! every respawn). While any of them is on, F6 gives Claw the next powerup
//! in turn (invisible, invulnerable, catnip, fire, ice and lightning
//! sword, then none) and names it on screen. A recording made with
//! an aid on names it in its header (`# invincible = on`,
//! `# infinite_lives = on`, `# powerup = fire:30`, `# powerup_key = used`)
//! and a replay plays it with that aid and gives the powerups of the key
//! where they were given, as does `--bench-route`.
//!
//! A replay plays the level its recording names (`# level N`) from the
//! start it was recorded with: a later level of a recorded game starts
//! with the progress carried into it, at the save point it was loaded
//! from, or at the `--start` point it was played from (its `# start_` lines).
//! A command line that names another level or another start point is
//! refused rather than replaying the recording from a start it was not
//! made with.
//!
//! `--bench-route FILE.csv --headless` benchmarks a route instead of the
//! combat route: the input script plays from the level's start with the
//! level, tuning, seed and behaviour switches its header names, as a
//! replay plays it, until its last row has run (or for at most `--bench
//! SECONDS`), drawn offscreen at `--res`. Its report also says whether any
//! sprite set was loaded while drawing (a budget: none may be) and gives a
//! digest of the route's trace, the same on every run. A route that
//! starts from carried progress or a save point is refused: the benchmark
//! plays from the level's own start (or the start point the route names).
//!
//! `--movie INDEX|FILE` plays one movie full screen with its sound, then
//! exits: an index from the install, one of -2, -1, 0, 2, 4, 6, 8, 10, 12,
//! 13 and 14 (-2 is the attract clip, -1 the logo, 0 the intro, 2 to 13
//! after those levels, 14 the end of the game), or FILE, any movie file.
//! Escape, or a pad's Back or Start button, skips it. A build without the
//! movie player says so and fails.
//!
//! In a game with its menus the movies play where the original's do: at
//! start-up, before the map screen after levels 2, 4, 6, 8, 10, 12, 13 and
//! 14, now and then when the main menu is left alone, and from its Replay
//! Movies page. `--no-movies` turns them off for one run; the `movies`,
//! `skip_logo` and `movie_folder` settings choose where they play from,
//! whether the logo plays and where upscaled copies are (see the
//! `settings` module).
//!
//! A benchmark prints its report as one line of JSON on standard output and
//! a one-line PASS or FAIL verdict naming each budget on standard error. It
//! exits with status [`BUDGET_FAILED`] when a measured budget is missed.

use std::process::ExitCode;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Arg, ArgAction, ArgMatches, CommandFactory, FromArgMatches};
use claw_game::bench::{Report, RouteFile, RouteReport, run_headless, run_headless_route};
use claw_game::session::{LevelRecording, replay_trace_with_crabs, start_sim_with_crabs};
use claw_game::settings::{Args, Aspect, Mode, Settings};
use claw_game::window::{self, WindowMode};
use claw_game::{load_graphics, load_level};
use claw_sim::StartState;

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
  --replay FILE.csv, --bench SECONDS one level without menus, as before; a
                                     replay in the window plays the sound and
                                     music (--no-audio silences it), the
                                     benchmark is silent
  --bench-route FILE.csv --headless  benchmark along a route from the level's
                                     start until its end
  --movie INDEX|FILE                 one movie, then exit: INDEX is -2 the
                                     attract clip, -1 the logo, 0 the intro,
                                     2, 4, 6, 8, 10, 12, 13 after those
                                     levels or 14 the end of the game; FILE
                                     is any movie file; Escape or the pad's
                                     Back or Start button skips it
  --no-movies                        no movie this run (at start-up, after
                                     levels, in the menus)

Play-test aids, off by default, for trying out hard levels:
  --invincible                       Claw takes no damage (death tiles and pits
                                     still end his life)
  --infinite-lives                   a death never lowers the lives count
  --powerup NAME[:SECONDS]           Claw has this powerup at the start of every
                                     level and after every respawn, as if just
                                     picked up: invisible, invulnerable, catnip,
                                     fire, ice or lightning (the swords),
                                     for SECONDS (default 30)
  F6 (while any aid is on)           give the next powerup in that order, then
                                     none, for 30 seconds; its name shows on
                                     screen (try each look and effect in turn)
The first two are also on the Gameplay page (Options, Gameplay) and in the
config file; on the command line the aids apply to that run only. A run with
an aid on says nothing about how hard a level is: recordings name the aids
and the powerups F6 gave, and routes, the bot and comparisons with reference
traces refuse them.

Saved games are kept in $XDG_DATA_HOME/rusty-claw/saves/default/ (by default
~/.local/share/rusty-claw/saves/default/, or
%LOCALAPPDATA%\\rusty-claw\\saves\\default\\ on Windows). Options changed in
the menus are written back to the config file; options given on the command
line are not, unless changed in the menus.

Movies play from the install, or from your own copies (for example upscaled
ones, named LOGO and CLAW0 to CLAW8 with .mkv, .mp4, .mov or .avi) in
$XDG_DATA_HOME/rusty-claw/movies/ (by default
~/.local/share/rusty-claw/movies/, or %LOCALAPPDATA%\\rusty-claw\\movies\\
on Windows; `movie_folder` in the config file moves it). Escape, or the
pad's Back or Start button, skips a movie; any key, click or pad button skips
a clip of the movie archive (CLAW.FEC).";

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
    // The benchmark starts at the level's start, or at a start point.
    let start =
        LevelRecording::parse(&route.text).with_context(|| format!("route {}", route.name))?;
    if start.start != StartState::default() {
        bail!(
            "route {} starts {}, which --bench-route cannot apply: it plays from the level's \
             own start; play it with --replay instead",
            route.name,
            start_note(&start)
        );
    }
    match (start.start_at, settings.start) {
        (Some(at), Some(given)) if at != given => bail!(
            "route {} starts Claw at {},{} (its `# start_at` line), not --start {},{}",
            route.name,
            at.0,
            at.1,
            given.0,
            given.1
        ),
        (Some(at), _) => settings.start = Some(at),
        (None, _) => {}
    }
    let Mode::Bench { render_hz, .. } = settings.mode else {
        bail!("--bench-route is a benchmark");
    };
    let report = run_headless_route(&settings, &route, cap, render_hz)?;
    println!("{}", report.to_json());
    eprintln!("rusty-claw: {}", report.summary());
    Ok(Outcome::RouteBench(Box::new(report)))
}

/// How recording `rec` starts, in words: "with carried progress (score
/// 25000, 4 lives)", "at save point 2", "at 750,895", or a new game's
/// values at the level's start.
fn start_note(rec: &LevelRecording) -> String {
    let mut parts = Vec::new();
    if let Some(c) = &rec.start.carry {
        parts.push(format!(
            "with carried progress (score {}, {} lives)",
            c.score, c.lives
        ));
    }
    if let Some(p) = rec.start.save {
        parts.push(format!("at save point {} ({},{})", p.slot, p.x, p.y));
    }
    if let Some((x, y)) = rec.start_at {
        parts.push(format!("at {x},{y}"));
    }
    if parts.is_empty() {
        "from a new game at the level's start".to_string()
    } else {
        parts.join(" ")
    }
}

/// Reads a recording for a replay and makes `settings` play it as it was
/// recorded (see [`Settings::follow_recording`]).
fn read_recording(settings: &mut Settings, path: &std::path::Path) -> Result<LevelRecording> {
    let rec = LevelRecording::read(path)?;
    settings
        .follow_recording(&rec)
        .with_context(|| format!("replaying {}", path.display()))?;
    if rec.has_start() {
        eprintln!("rusty-claw: {} starts {}", path.display(), start_note(&rec));
    }
    Ok(rec)
}

fn run(args: Args, matches: &ArgMatches) -> Result<Outcome> {
    if let Some(path) = matches.get_one::<std::path::PathBuf>(BENCH_ROUTE) {
        return bench_route(args, path);
    }
    if args.headless && args.bench.is_none() {
        bail!("--headless goes with --bench or --bench-route");
    }
    let mut settings = Settings::from_args(&args)?;
    if let Some(target) = settings.movie.clone() {
        window::run_movie(settings, target, Instant::now())?;
        return Ok(Outcome::Done);
    }
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
            let rec = read_recording(&mut settings, &script)?;
            let bundle = load_level(&settings.install, &settings.level)?;
            let sim = start_sim_with_crabs(
                bundle.sim_level.clone(),
                settings.sim_config(settings.res),
                rec.start,
                settings.start,
                &rec.crabs,
            );
            let rows = replay_trace_with_crabs(
                sim,
                rec.script,
                settings.pause_input,
                render_hz,
                rec.crabs,
            );
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
                    WindowMode::Watch(Box::new(read_recording(&mut settings, &script)?))
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
