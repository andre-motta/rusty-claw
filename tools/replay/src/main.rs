//! `claw-replay`: runs a recorded input script through the simulation of a
//! level from your own install and writes a per-tick trace.
//!
//! ```text
//! claw-replay --install PATH (--level N | --custom FILE.WWD)
//!             --script INPUT.csv --trace OUT.csv
//!             [--res WxH] [--aspect pillarbox|frozen|widened] [--seed N]
//!             [--start X,Y]
//! ```
//!
//! The script has the columns `tick,held_mask,pressed_mask`, one row per
//! 18 ms tick. The trace has one row per tick with Claw's position, the
//! fractions carried to the next tick, his state and physics type, the
//! camera point, the input and the activation mode. Two runs with the same
//! arguments write identical traces.
//!
//! `--start X,Y` moves Claw to (X, Y) before the script and lets him and the
//! camera settle; the script's first tick is the first traced tick.

use std::path::PathBuf;
use std::process::ExitCode;

use claw_assets::{Install, LevelData, LevelSource, Source};
use claw_sim::claw_frame::{AspectPolicy, ViewConfig};
use claw_sim::trace::{self, TraceRow};
use claw_sim::{InputScript, Sim, SimConfig, SimLevel};

const USAGE: &str = "usage: claw-replay --install PATH (--level N | --custom FILE.WWD) \
--script INPUT.csv --trace OUT.csv [--res WxH] [--aspect pillarbox|frozen|widened] [--seed N] \
[--start X,Y]";

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
}

fn parse_args(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args::default();
    while let Some(flag) = it.next() {
        if flag == "--help" || flag == "-h" {
            return Err(USAGE.to_string());
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
            "--seed" => {
                let v = value()?;
                a.seed = Some(v.parse().map_err(|_| format!("bad seed {v:?}"))?);
            }
            other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
        }
    }
    Ok(a)
}

fn run(args: Args) -> Result<(), String> {
    let install_path = args.install.ok_or(USAGE)?;
    let script_path = args.script.ok_or(USAGE)?;
    let trace_path = args.trace.ok_or(USAGE)?;
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
        _ => return Err(format!("pass exactly one of --level and --custom\n{USAGE}")),
    };
    let level = LevelData::build(&store, source).map_err(|e| format!("loading level: {e}"))?;
    let sim_level =
        SimLevel::from_level(&level, &store).map_err(|e| format!("preparing level: {e}"))?;
    let text = std::fs::read_to_string(&script_path)
        .map_err(|e| format!("reading {}: {e}", script_path.display()))?;
    let script =
        InputScript::parse(&text).map_err(|e| format!("{}: {e}", script_path.display()))?;

    let mut view = ViewConfig::default();
    if let Some((w, h)) = args.res {
        view.render_width = w;
        view.render_height = h;
    }
    if let Some(aspect) = args.aspect {
        view.aspect_policy = aspect;
    }
    let mut cfg = SimConfig::for_view(view);
    if let Some(seed) = args.seed {
        cfg.seed = seed;
    }
    let mut sim = Sim::new(sim_level, cfg);
    if let Some((x, y)) = args.start {
        settle_at(&mut sim, x, y);
    }
    let mut rows = Vec::with_capacity(script.ticks.len());
    for (tick, &input) in script.ticks.iter().enumerate() {
        sim.step(input);
        rows.push(TraceRow::capture(&sim, tick as u64, "script"));
    }
    std::fs::write(&trace_path, trace::to_csv(&rows))
        .map_err(|e| format!("writing {}: {e}", trace_path.display()))?;
    let d = sim.diagnostics();
    let unknown: usize = d.unknown_logics.values().sum();
    eprintln!(
        "claw-replay: {} ticks, activation {}, {} objects ({} with unknown logics, {} records dropped), trace {}",
        rows.len(),
        sim.config().activation.mode.name(),
        sim.objects().len(),
        unknown,
        d.dropped_records,
        trace_path.display()
    );
    Ok(())
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
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("claw-replay: {e}");
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
        let a = args("--install /i --level 1 --script s.csv --trace t.csv --res 3440x1440 --aspect widened --seed 7 --start 6272,3150").unwrap();
        assert_eq!(a.start, Some((6272, 3150)));
        assert_eq!(a.level, Some(1));
        assert_eq!(a.res, Some((3440, 1440)));
        assert_eq!(a.aspect, Some(AspectPolicy::WidenedActivation));
        assert_eq!(a.seed, Some(7));
        assert!(args("--level x").is_err());
        assert!(args("--bogus").is_err());
        assert!(args("--res 12").is_err());
    }
}
