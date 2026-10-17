//! `claw-replay`: runs a recorded input script (a route) through the
//! simulation of a level from your own install, writes a per-tick trace and
//! can compare it with a reference trace of the same route.
//!
//! ```text
//! claw-replay --install PATH [--level N | --custom FILE.WWD] --script ROUTE.csv
//!             [--trace OUT.csv] [--tuning default|original] [--seed N]
//!             [--res WxH] [--aspect pillarbox|frozen|widened]
//!             [--start X,Y] [--logics on|off] [--mid-air-hit-fix on|off]
//!             [--expect-complete]
//!             [--compare REF.csv --tolerance exact|feel|FILE.toml [--report OUT.txt]]
//! ```
//!
//! The script has the columns `tick,held_mask,pressed_mask`, one row per
//! 18 ms tick. Its header keys `# level N`, `# tuning default|original` and
//! `# seed N`, and its `# mid_air_hit_fix = on|off` line, stand in for
//! `--level`, `--tuning`, `--seed` and `--mid-air-hit-fix` when those are
//! not given; a flag given on the command line wins. Without either the
//! mid-air hit jump fix is off, as in the original.
//!
//! The script's `@` directive lines are played at their tick boundaries: a
//! command (`end_life`) takes effect before its tick, and a pause or a menu
//! changes nothing in the game (play stops and goes on where it stopped;
//! its wall time only matters to a game running in real time).
//!
//! The trace has one row per tick with Claw's position, the fractions
//! carried to the next tick, his state and physics type, the camera point,
//! the input, the activation mode, his health, score, lives, selected
//! weapon and ammunition, the tick's game events and the state of the
//! shared random stream. Two runs with the same arguments write identical
//! traces.
//!
//! `--tuning original` plays with the original game's movement numbers at
//! the fixed 18 ms step instead of the default 60 fps tuning.
//!
//! `--logics off` leaves every level object inert, as in the first playable
//! version, for comparing movement with earlier traces.
//!
//! `--start X,Y` moves Claw to (X, Y) before the script and lets him and the
//! camera settle; the script's first tick is the first traced tick.
//!
//! `--expect-complete` checks that the level ends (a level-complete event)
//! within the script.
//!
//! `--compare REF.csv` compares the run with a reference trace of the same
//! route, in the same trace format, at the `exact` or `feel` tolerance
//! preset or the tolerances of a file (see `claw_sim::trace`), prints a
//! per-column summary and the first divergence with the five rows of each
//! trace leading up to it, and with `--report` writes the same text to a
//! file.
//!
//! Exit status: 0 when everything passed, 1 on a divergence, 2 on a usage
//! or input error, 3 when `--expect-complete` saw no level end.

use std::path::PathBuf;
use std::process::ExitCode;

use claw_assets::{Install, LevelData, LevelSource, Source};
use claw_sim::claw_frame::{AspectPolicy, GameEvent, ViewConfig};
use claw_sim::input::{DirectiveKind, ScriptCommand, ScriptTuning};
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{InputScript, Sim, SimConfig, SimLevel};

const USAGE: &str = "usage: claw-replay --install PATH [--level N | --custom FILE.WWD] \
--script ROUTE.csv [--trace OUT.csv] [--tuning default|original] [--seed N] [--res WxH] \
[--aspect pillarbox|frozen|widened] [--start X,Y] [--logics on|off] \
[--mid-air-hit-fix on|off] [--expect-complete] \
[--compare REF.csv --tolerance exact|feel|FILE.toml [--report OUT.txt]]";

/// The key of the mid-air hit jump fix in a script's comment line.
const BEHAVIOUR_KEY: &str = "mid_air_hit_fix";

/// Rows of each trace shown before a divergence.
const CONTEXT_ROWS: usize = 5;

#[derive(Debug, Default)]
struct Args {
    install: Option<PathBuf>,
    level: Option<u32>,
    custom: Option<String>,
    script: Option<PathBuf>,
    trace: Option<PathBuf>,
    res: Option<(u32, u32)>,
    aspect: Option<AspectPolicy>,
    seed: Option<u32>,
    start: Option<(i32, i32)>,
    logics: Option<bool>,
    mid_air_hit_fix: Option<bool>,
    tuning: Option<ScriptTuning>,
    expect_complete: bool,
    compare: Option<PathBuf>,
    tolerance: Option<String>,
    report: Option<PathBuf>,
}

fn parse_args(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args::default();
    while let Some(flag) = it.next() {
        if flag == "--help" || flag == "-h" {
            return Err(USAGE.to_string());
        }
        if flag == "--expect-complete" {
            a.expect_complete = true;
            continue;
        }
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--install" => a.install = Some(PathBuf::from(value()?)),
            "--level" => {
                let v = value()?;
                a.level = Some(v.parse().map_err(|_| format!("bad level {v:?}"))?);
            }
            "--custom" => a.custom = Some(value()?),
            "--script" => a.script = Some(PathBuf::from(value()?)),
            "--trace" => a.trace = Some(PathBuf::from(value()?)),
            "--res" => {
                let v = value()?;
                let (w, h) = v
                    .split_once(['x', 'X'])
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .ok_or_else(|| format!("bad resolution {v:?}, expected WxH"))?;
                a.res = Some((w, h));
            }
            "--aspect" => {
                let v = value()?;
                a.aspect = Some(match v.as_str() {
                    "pillarbox" => AspectPolicy::Pillarbox,
                    "frozen" => AspectPolicy::FrozenEdgeClamped,
                    "widened" => AspectPolicy::WidenedActivation,
                    _ => return Err(format!("bad aspect {v:?}")),
                });
            }
            "--start" => {
                let v = value()?;
                let p = v
                    .split_once(',')
                    .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
                    .ok_or_else(|| format!("bad start {v:?}, expected X,Y"))?;
                a.start = Some(p);
            }
            "--logics" => {
                let v = value()?;
                a.logics = Some(match v.as_str() {
                    "on" => true,
                    "off" => false,
                    _ => return Err(format!("bad logics {v:?}, expected on or off")),
                });
            }
            "--mid-air-hit-fix" => {
                let v = value()?;
                a.mid_air_hit_fix = Some(
                    parse_on_off(&v)
                        .ok_or_else(|| format!("bad mid-air hit fix {v:?}, expected on or off"))?,
                );
            }
            "--seed" => {
                let v = value()?;
                a.seed = Some(v.parse().map_err(|_| format!("bad seed {v:?}"))?);
            }
            "--tuning" => {
                let v = value()?;
                a.tuning =
                    Some(ScriptTuning::from_name(&v).ok_or_else(|| {
                        format!("bad tuning {v:?}, expected default or original")
                    })?);
            }
            "--compare" => a.compare = Some(PathBuf::from(value()?)),
            "--tolerance" => a.tolerance = Some(value()?),
            "--report" => a.report = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
        }
    }
    if a.compare.is_some() != a.tolerance.is_some() {
        return Err(format!("--compare and --tolerance go together\n{USAGE}"));
    }
    if a.report.is_some() && a.compare.is_none() {
        return Err(format!("--report needs --compare\n{USAGE}"));
    }
    Ok(a)
}

fn parse_on_off(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Some(true),
        "off" | "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

/// The mid-air hit jump fix a script's `# mid_air_hit_fix = on|off`
/// comment line names (the last such line counts); `None` without one.
fn script_behaviour(text: &str) -> Option<bool> {
    text.lines().rev().find_map(|l| {
        let (k, v) = l.trim().strip_prefix('#')?.split_once('=')?;
        if k.trim() != BEHAVIOUR_KEY {
            return None;
        }
        parse_on_off(v)
    })
}

/// The tolerances named on the command line: a preset or a file.
fn load_tolerances(spec: &str) -> Result<Tolerances, String> {
    if let Some(t) = Tolerances::preset(spec) {
        return Ok(t);
    }
    let text = std::fs::read_to_string(spec).map_err(|e| {
        format!("--tolerance {spec:?} is neither exact, feel nor a readable file: {e}")
    })?;
    Tolerances::from_toml(&text).map_err(|e| format!("{spec}: {e}"))
}

/// How a run ended, for the exit status.
#[derive(Debug, Default)]
struct Outcome {
    diverged: bool,
    incomplete: bool,
}

fn run(args: Args) -> Result<Outcome, String> {
    let install_path = args.install.ok_or(USAGE)?;
    let script_path = args.script.ok_or(USAGE)?;
    let tolerances = args.tolerance.as_deref().map(load_tolerances).transpose()?;
    let text = std::fs::read_to_string(&script_path)
        .map_err(|e| format!("reading {}: {e}", script_path.display()))?;
    let script =
        InputScript::parse(&text).map_err(|e| format!("{}: {e}", script_path.display()))?;
    let header = script.header;
    let fix = args
        .mid_air_hit_fix
        .or(script_behaviour(&text))
        .unwrap_or(false);
    if let (Some(flag), Some(key)) = (args.level, header.level)
        && flag != key
    {
        eprintln!("claw-replay: --level {flag} overrides the script's level {key}");
    }
    let install = Install::open(&install_path)
        .map_err(|e| format!("opening install {}: {e}", install_path.display()))?;
    let mut store = install
        .store()
        .map_err(|e| format!("mounting assets: {e}"))?;
    let source = match (args.level, &args.custom) {
        (Some(level), None) => LevelSource::Retail(level),
        (None, Some(name)) => {
            let path = install.custom_level_path(name).map_err(|e| e.to_string())?;
            store
                .mount(Source::CustomLevel(path))
                .map_err(|e| e.to_string())?;
            LevelSource::Custom
        }
        (None, None) => match header.level {
            Some(level) => LevelSource::Retail(level),
            None => {
                return Err(format!(
                    "pass --level or --custom, or give the script a `# level N` line\n{USAGE}"
                ));
            }
        },
        _ => return Err(format!("pass only one of --level and --custom\n{USAGE}")),
    };
    let level = LevelData::build(&store, source).map_err(|e| format!("loading level: {e}"))?;
    let sim_level =
        SimLevel::from_level(&level, &store).map_err(|e| format!("preparing level: {e}"))?;

    let mut view = ViewConfig::default();
    if let Some((w, h)) = args.res {
        view.render_width = w;
        view.render_height = h;
    }
    if let Some(aspect) = args.aspect {
        view.aspect_policy = aspect;
    }
    let mut cfg = SimConfig::for_view(view);
    let tuning = args.tuning.or(header.tuning);
    if let Some(tuning) = tuning {
        cfg.tuning = tuning.tuning();
    }
    if let Some(seed) = args.seed.or(header.seed) {
        cfg.seed = seed;
    }
    if let Some(on) = args.logics {
        cfg.level_logics = on;
    }
    cfg.tuning.behaviour.fix_double_jump = fix;
    let mut sim = Sim::new(sim_level, cfg);
    if let Some((x, y)) = args.start {
        settle_at(&mut sim, x, y);
    }
    let mut rows = Vec::with_capacity(script.ticks.len());
    let mut completed = false;
    let mut directives = script.directives().iter().peekable();
    let mut commands = 0;
    for (tick, &input) in script.ticks.iter().enumerate() {
        while let Some(d) = directives.next_if(|d| d.tick <= tick) {
            if let DirectiveKind::Command(c) = d.kind {
                match c {
                    ScriptCommand::EndLife => sim.end_life(),
                }
                commands += 1;
            }
        }
        sim.step(input);
        completed |= sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }));
        rows.push(TraceRow::capture(&sim, tick as u64, "script"));
    }
    if let Some(path) = &args.trace {
        std::fs::write(path, trace::to_csv(&rows))
            .map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    let d = sim.diagnostics();
    let unknown: usize = d.unknown_logics.values().sum();
    let inert: usize = d.inert_logics.values().sum();
    eprintln!(
        "claw-replay: {} ticks, {} directives ({} commands), tuning {}, seed {}, mid-air hit fix {}, activation {}, level logics {}, {} objects ({} with unknown logics, {} with logics not run yet, {} records dropped){}",
        rows.len(),
        script.directives().len(),
        commands,
        tuning.unwrap_or(ScriptTuning::Default).name(),
        sim.config().seed,
        if fix { "on" } else { "off" },
        sim.config().activation.mode.name(),
        if sim.config().level_logics {
            "on"
        } else {
            "off"
        },
        sim.objects().len(),
        unknown,
        inert,
        d.dropped_records,
        args.trace
            .as_ref()
            .map(|p| format!(", trace {}", p.display()))
            .unwrap_or_default()
    );
    if let Some(last) = rows.last() {
        let events = rows.iter().filter(|r| !r.events.is_empty()).count();
        eprintln!(
            "claw-replay: at the end health {}, score {}, lives {}, weapon {}, ammunition {}/{}/{}; {} ticks with game events{}",
            last.health,
            last.score,
            last.lives,
            last.weapon,
            last.pistol,
            last.magic,
            last.dynamite,
            events,
            if completed {
                "; the level was completed"
            } else {
                ""
            }
        );
    }

    let mut outcome = Outcome::default();
    if let (Some(ref_path), Some(tol)) = (&args.compare, &tolerances) {
        let ref_text = std::fs::read_to_string(ref_path)
            .map_err(|e| format!("reading {}: {e}", ref_path.display()))?;
        let reference =
            trace::parse(&ref_text).map_err(|e| format!("{}: {e}", ref_path.display()))?;
        if reference.is_empty() {
            return Err(format!(
                "{}: the reference trace has no rows",
                ref_path.display()
            ));
        }
        let cmp = trace::compare(&rows, &reference, tol);
        let mut text = trace::report(&rows, &reference, &cmp, CONTEXT_ROWS);
        if cmp.rows_compared == 0 {
            text.push_str(
                "nothing was compared: the reference starts after the rebuild trace ends\n",
            );
        }
        print!("{text}");
        if let Some(path) = &args.report {
            std::fs::write(path, &text).map_err(|e| format!("writing {}: {e}", path.display()))?;
        }
        outcome.diverged = !cmp.passed() || cmp.rows_compared == 0;
    }
    if args.expect_complete && !completed {
        eprintln!("claw-replay: the level was not completed within the script");
        outcome.incomplete = true;
    }
    Ok(outcome)
}

/// Moves Claw and waits until he and the camera have stood still for ten
/// ticks (at most four seconds).
fn settle_at(sim: &mut Sim, x: i32, y: i32) {
    sim.teleport_player(x, y);
    let mut still = 0;
    let mut last = None;
    for _ in 0..222 {
        sim.step(Default::default());
        let now = sim
            .player()
            .map(|o| (o.x, o.y, o.state, sim.camera().current));
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still >= 10 {
            break;
        }
    }
}

fn main() -> ExitCode {
    let result = parse_args(std::env::args().skip(1)).and_then(run);
    match result {
        Ok(Outcome { diverged: true, .. }) => ExitCode::from(1),
        Ok(Outcome {
            incomplete: true, ..
        }) => ExitCode::from(3),
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("claw-replay: {e}");
            ExitCode::from(2)
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
        let a = args("--install /i --level 1 --script s.csv --trace t.csv --res 3440x1440 --aspect widened --seed 7 --start 6272,3150 --logics off").unwrap();
        assert_eq!(a.logics, Some(false));
        assert!(args("--logics maybe").is_err());
        assert_eq!(a.start, Some((6272, 3150)));
        assert_eq!(a.level, Some(1));
        assert_eq!(a.res, Some((3440, 1440)));
        assert_eq!(a.aspect, Some(AspectPolicy::WidenedActivation));
        assert_eq!(a.seed, Some(7));
        assert!(args("--level x").is_err());
        assert!(args("--bogus").is_err());
        assert!(args("--res 12").is_err());
    }

    #[test]
    fn parses_the_route_and_comparison_arguments() {
        let a = args(
            "--install /i --script r.csv --tuning original --expect-complete \
             --compare ref.csv --tolerance feel --report out.txt",
        )
        .unwrap();
        assert_eq!(a.tuning, Some(ScriptTuning::Original));
        assert!(a.expect_complete);
        assert_eq!(a.compare, Some(PathBuf::from("ref.csv")));
        assert_eq!(a.tolerance.as_deref(), Some("feel"));
        assert_eq!(a.report, Some(PathBuf::from("out.txt")));
        assert_eq!(a.trace, None, "the trace file is optional");
        assert!(args("--tuning fast").is_err());
        assert!(args("--compare ref.csv").is_err(), "needs a tolerance");
        assert!(args("--tolerance exact").is_err(), "needs a reference");
        assert!(args("--report out.txt").is_err(), "needs a reference");
    }

    #[test]
    fn tolerances_come_from_a_preset_or_a_file() {
        assert_eq!(load_tolerances("exact").unwrap(), Tolerances::exact());
        assert_eq!(load_tolerances("feel").unwrap(), Tolerances::feel());
        assert!(load_tolerances("/nonexistent/tolerances.toml").is_err());
    }

    #[test]
    fn the_mid_air_hit_fix_comes_from_the_flag_or_the_script() {
        let a = args("--install /i --script s.csv --mid-air-hit-fix on").unwrap();
        assert_eq!(a.mid_air_hit_fix, Some(true));
        assert!(args("--mid-air-hit-fix maybe").is_err());
        assert_eq!(
            script_behaviour("# mid_air_hit_fix = on\ntick,a,b\n0,0,0\n"),
            Some(true)
        );
        assert_eq!(
            script_behaviour("# mid_air_hit_fix = on\n# mid_air_hit_fix=off\n"),
            Some(false),
            "the last line counts"
        );
        assert_eq!(script_behaviour("# level 1\ntick,a,b\n"), None);
    }

    #[test]
    fn scripts_with_directives_are_read() {
        let dir = std::env::temp_dir().join(format!("claw-replay-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("route.csv");
        std::fs::write(
            &script,
            "tick,held_mask,pressed_mask\n0,0,0\n@1 menu ingame 900 keys=l,y\n@1 command end_life\n",
        )
        .unwrap();
        let a = Args {
            install: Some(dir.join("no-install")),
            script: Some(script),
            level: Some(1),
            ..Args::default()
        };
        // The script is accepted; the run stops at the missing install.
        let e = run(a).unwrap_err();
        assert!(e.contains("opening install"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
