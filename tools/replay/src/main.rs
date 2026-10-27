//! `claw-replay`: runs a recorded input script (a route) through the
//! simulation of a level from your own install, writes a per-tick trace and
//! can compare it with a reference trace of the same route.
//!
//! ```text
//! claw-replay --install PATH [--level N | --custom FILE.WWD] --script ROUTE.csv
//!             [--trace OUT.csv] [--tuning default|original] [--seed N]
//!             [--res WxH] [--aspect pillarbox|frozen|widened]
//!             [--start X,Y] [--checkpoint X,Y] [--logics on|off] [--mid-air-hit-fix on|off]
//!             [--invincible] [--infinite-lives] [--powerup NAME[:SECONDS]]
//!             [--crabs FILE | --crab-variant N] [--expect-complete]
//!             [--compare REF.csv --tolerance exact|feel|FILE.toml [--report OUT.txt]]
//!             [--carry-out FILE]
//! claw-replay --install PATH --script ROUTE.csv --next ROUTE.csv [--next ROUTE.csv ...]
//!             [--trace OUT.csv] [--res WxH] [--aspect pillarbox|frozen|widened]
//!             [--expect-complete] [--carry-out FILE]
//! ```
//!
//! The script has the columns `tick,held_mask,pressed_mask`, one row per
//! 18 ms tick. Its header keys `# level N`, `# tuning default|original` and
//! `# seed N`, and its `# mid_air_hit_fix = on|off` line, stand in for
//! `--level`, `--tuning`, `--seed` and `--mid-air-hit-fix` when those are
//! not given; a flag given on the command line wins. Without either the
//! mid-air hit jump fix is off, as in the original.
//!
//! A recording the game made with a play-test aid on names it in its
//! header (`# invincible = on`, `# infinite_lives = on`) and replays with
//! it; `--invincible` and `--infinite-lives` turn an aid on as the game's
//! flags of the same names do. The powerup aid is named the same way
//! (`# powerup = fire:30`, the powerup given at the start and after every
//! respawn, which `--powerup` also gives), and a powerup given with the
//! game's play-test key is a `powerup` command played at its tick boundary
//! (with `# powerup_key = used` in the header). A run with an aid on is not
//! plain play, so `--compare` refuses it.
//!
//! A route or recording of a later level of one game starts from the
//! progress the game carried into it, named in its `# start_score = ...`
//! lines (score, health, lives, ammunition, weapon and the game's
//! treasure): the level starts with that progress, as the game's own flow
//! starts it. The lines of a save point a recording started from are not
//! applied.
//!
//! The script's `@` directive lines are played at their tick boundaries,
//! as the game's replay and the bot play them (one shared function,
//! `claw_sim::Sim::play_directive`): a command (`end_life`, `powerup`)
//! takes effect before its tick; the return from a help screen
//! (`menu help`) runs the update pass of that return with the screen's
//! time (the dynamite fuse burns on, every object runs once more, and the
//! next frame pulse comes as many frames later as the help screen ran);
//! the in-game menu's frames keep the enemies' rhythm going: the return
//! from it (`menu ingame`) moves the next frame pulse by one frame per
//! 18 ms of the menu, rounded up, and an End Life (`command end_life`) by
//! the five frames of the menu's visit (it opens, three frames for the
//! confirmation page, one for the yes), or four when a `menu ingame` line
//! at the same tick already opened the menu (a menu line counts its
//! opening frame however short its time); the game counts them the same
//! way, live and replayed; after a help screen or the in-game menu the keys down count as already
//! pressed until released; a pause (`pause`, or the pause screen
//! `menu pause`) latches no key; and as play resumes after any of them the
//! game's clock is reset. A pause's wall time changes nothing else in the
//! game, and the in-game menu's nothing but the frame pulse.
//!
//! A recording names the pause and screen switches the game played it
//! with (`# fuse_clock = original|frozen`, `# pause_input = original|...`;
//! the last line of each counts). With `# fuse_clock = frozen` the help
//! screen's return runs no update pass, as in the game with that switch.
//! Keys across a pause or a screen are only played the original's way, so
//! a script with `# pause_input` other than `original` is refused.
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
//! camera settle; the script's first tick is the first traced tick. The
//! shared random stream is set to the seed once he stands, so the seed is
//! its state at the script's time zero, as for a segment of a level in a
//! reference store (`make compare` reads the start point from the store).
//!
//! `--checkpoint X,Y` sets the point Claw respawns at to (X, Y) at the
//! script's time zero (after `--start` has settled him), a place no
//! checkpoint of the level need give him: a diagnostic set-up, for
//! comparing a reference run that moved his respawn point the same way.
//! A script's `# checkpoint X,Y` comment line stands in for the flag when
//! it is not given. The run is still plain play and can be compared.
//!
//! A script's `# health N` comment line gives Claw health N at the
//! script's time zero (after `--start` has settled him, as the reference
//! store's runs of such a route set it): a diagnostic set-up too, for a
//! stretch that starts from a health plain play does not bring there; the
//! run can still be compared with those runs. A respawn refills it as any
//! respawn does.
//!
//! A script's `# boss_health N` comment line is a set-up for measuring a
//! stretch of a boss fight: the level's boss is given health N on the
//! first arena step, after the first tick that has him as the current
//! boss (see `claw_sim::SimConfig::boss_health`), as the reference
//! store's runs of such a route set it. Like `--checkpoint`, it is a
//! diagnostic set-up; the run can still be compared with those runs.
//!
//! Each hermit crab starts with a side its first regroup faces by and a
//! second range box (half width, half height) that the original game
//! does not fix: they differ from crab to crab and from run to run, and a
//! box that covers Claw makes the crab decide as if he were far whenever
//! he is outside its nearer boxes. A crab a nest hatched can also start
//! stationary, which the original game does not fix either: such a crab
//! has no ledge guard (it may stand with its leading edge over a drop)
//! and strikes with the bomb's throw sound. A run's values are given per
//! crab, one entry per line:
//!
//! ```text
//! crab RECORD = SIDE, HALF_WIDTH, HALF_HEIGHT[, STATIONARY]
//! nest_crab NEST_RECORD HATCH = SIDE, HALF_WIDTH, HALF_HEIGHT[, STATIONARY]
//! ```
//!
//! STATIONARY is 1 for a crab that starts stationary and 0, the default
//! when it is left out, for one that does not
//! (`nest_crab 1306 3 = 0, 0, 0, 1`). A placed crab is named by its record (its place in the level's list of
//! objects, from 0), a crab a nest hatched by the nest's record and its
//! place in the hatch (1 for the nest's first crab); a `*` names every
//! crab of that kind, and the entry that names a crab most exactly
//! counts. The script's header gives them as comment lines
//! (`# crab 101 = 2, 300, 40`); `--crabs FILE` gives them from a file
//! instead (the same lines, with or without the `#`; blank lines and
//! other `#` lines are skipped) and replaces the header's, and
//! `--crab-variant N` takes instead the header's further set N (its
//! `# crab_variant N: ...` lines, the sets a route was made to win under
//! too). Each crab
//! takes its values when it is built, before its first tick; a crab no
//! entry names starts with an empty box facing left and not stationary,
//! the plain default.
//! The values change nothing but the crabs' decisions, so a run with them
//! can be compared.
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
//! A reference trace in a reference store (`trace-<clock>-r<k>.csv` beside
//! the store's `meta.toml`) of a segment that the store's runs started by
//! placing Claw on the level's first tick (`t0_rule = "teleport"`, the
//! start point in `teleport = "X,Y"`) starts the run the way those runs
//! did: Claw is placed at the start point on the level's first tick and
//! left there, without settling, until the route's time zero (`t0_ms` of
//! the trace's clock, in whole 18 ms ticks), and the random stream is set
//! there, once, to the store's state at time zero (`rng_at_t0`). The
//! store's start point and random state stand in for `--start` and
//! `--seed` when those are not given ([`StoreStart`]).
//!
//! `--carry-out FILE` writes the progress the level hands on to the next
//! one in one game, as the `# start_score = ...` lines a route of that
//! level starts with (see [`hands_on`]); nothing is written when the level
//! did not end.
//!
//! `--next ROUTE.csv`, once or more, plays a chain: one game through every
//! booty screen. The script's level is played first, then each `--next`
//! route in turn as the level after the previous one (its `# level` line
//! names that level, and every route names the same tuning). Each level
//! is played as a route of it alone is played (with its own seed, mid-air
//! hit jump fix, switches, crab values and `@` lines), but it starts with
//! the progress the previous level hands on ([`hands_on`]), as the game's
//! flow starts it after the booty screen; when the route's own
//! `# start_...` lines name other progress, the run says so, prints the
//! lines the game carries in and counts it as a divergence. A level that
//! does not end stops the chain there. A chain is plain play: the
//! options that change one level only (`--level`, `--custom`, `--seed`,
//! `--tuning`, `--start`, `--checkpoint`, `--logics`, `--mid-air-hit-fix`,
//! the play-test aids, the crab options and `--compare`) are refused with
//! it, and so are a route's diagnostic set-ups (`# checkpoint`, `# health`,
//! `# boss_health`) and play-test aids. `--trace OUT.csv` writes one trace
//! per level, the first level's to OUT.csv and level N's to
//! OUT.levelN.csv, as a recorded game names its route files, so each
//! level's part can be compared with a reference trace of that level;
//! `--carry-out` writes what the chain's last level hands on. Each level's
//! summary gives its end tick, health, score, lives and deaths, and the
//! deaths other than its route's End Life commands are counted as a
//! divergence.
//!
//! Exit status: 0 when everything passed, 1 on a divergence, 2 on a usage
//! or input error, 3 when `--expect-complete` saw no level end (or a level
//! of a chain did not end).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use claw_assets::{Install, LevelData, LevelSource, Source};
use claw_sim::claw_frame::{AspectPolicy, GameEvent, ViewConfig};
use claw_sim::input::{
    DirectiveKind, ScriptCommand, ScriptTuning, script_checkpoint, script_health,
};
use claw_sim::logic::enemy::crab::CrabValues;
use claw_sim::logic::pickup::perfect_tally;
use claw_sim::player::hit::MAX_LIVES;
use claw_sim::player::powerup_aid::AidPowerup;
use claw_sim::player::progress::{parse_start_lines, start_lines};
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::tuning::{FuseClock, PlayAids, TICK_MS};
use claw_sim::{Carry, InputScript, Sim, SimConfig, SimLevel, StartState};

const USAGE: &str = "usage: claw-replay --install PATH [--level N | --custom FILE.WWD] \
--script ROUTE.csv [--trace OUT.csv] [--tuning default|original] [--seed N] [--res WxH] \
[--aspect pillarbox|frozen|widened] [--start X,Y] [--checkpoint X,Y] [--logics on|off] \
[--mid-air-hit-fix on|off] [--invincible] [--infinite-lives] [--powerup NAME[:SECONDS]] \
[--crabs FILE | --crab-variant N] [--expect-complete] \
[--compare REF.csv --tolerance exact|feel|FILE.toml [--report OUT.txt]] [--carry-out FILE]
   or: claw-replay --install PATH --script ROUTE.csv --next ROUTE.csv [--next ROUTE.csv ...] \
[--trace OUT.csv] [--res WxH] [--aspect pillarbox|frozen|widened] [--expect-complete] [--carry-out FILE]
--next: the routes of the next levels of one game, played through every booty screen, each \
level starting with the progress the previous one hands on; --trace writes OUT.csv and OUT.levelN.csv
the script's @ lines play as in the game: a help screen's return runs its update pass, the in-game menu's frames and an End Life's move the frame pulse, a screen latches the keys down, a pause latches none, a command acts before its tick; `# fuse_clock = frozen` runs no help return pass and moves no frame pulse, and `# pause_input` other than original is refused
--crabs FILE: one entry per line, `crab RECORD = SIDE, HALF_WIDTH, HALF_HEIGHT[, STATIONARY]` or `nest_crab NEST_RECORD HATCH = SIDE, HALF_WIDTH, HALF_HEIGHT[, STATIONARY]` (STATIONARY 1 or 0, 0 when left out)";

/// The key of the mid-air hit jump fix in a script's comment line.
const BEHAVIOUR_KEY: &str = "mid_air_hit_fix";

/// The key of the pause input switch in a recording's comment lines.
const PAUSE_INPUT_KEY: &str = "pause_input";

/// The key of the fuse clock switch in a recording's comment lines.
const FUSE_CLOCK_KEY: &str = "fuse_clock";

/// Rows of each trace shown before a divergence.
const CONTEXT_ROWS: usize = 5;

/// How the runs of a reference store started a segment of a level: Claw
/// placed at `start` on the level's first tick and left there until the
/// route's time zero, `t0_ms` of game time, where the random stream was
/// `seed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StoreStart {
    start: Option<(i32, i32)>,
    t0_ms: u64,
    seed: Option<u32>,
}

impl StoreStart {
    /// The ticks played before the route's first tick.
    fn ticks(&self) -> u64 {
        self.t0_ms.div_ceil(u64::from(TICK_MS))
    }
}

/// A value of a store's `meta.toml`: `key` in the table `[section]`, or
/// at the top when `section` is empty, unquoted.
fn meta_value(meta: &str, section: &str, key: &str) -> Option<String> {
    let mut current = String::new();
    for line in meta.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = name.trim().to_string();
        } else if current == section
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// The clock of a reference trace named `trace-<clock>-r<k>.csv`.
fn trace_clock(trace: &std::path::Path) -> Option<String> {
    let name = trace.file_name()?.to_str()?;
    let rest = name.strip_prefix("trace-")?;
    let (clock, _) = rest.split_once('-')?;
    (!clock.is_empty()).then(|| clock.to_string())
}

/// How the store's runs of the clock `clock` started, from its `meta.toml`
/// (the clock's own table first, then the top level): `None` unless they
/// placed Claw on the level's first tick (`t0_rule = "teleport"`) and the
/// meta names the time zero (`t0_ms`). Without a clock, the store's one
/// clock (`clock = ...` at the top) is used.
fn store_start(meta: &str, clock: Option<&str>) -> Result<Option<StoreStart>, String> {
    let top_clock = meta_value(meta, "", "clock");
    let clock = clock.map(str::to_string).or(top_clock).unwrap_or_default();
    let get = |key: &str| meta_value(meta, &clock, key).or_else(|| meta_value(meta, "", key));
    if get("t0_rule").as_deref() != Some("teleport") {
        return Ok(None);
    }
    let Some(t0) = get("t0_ms") else {
        return Ok(None);
    };
    let t0_ms = t0
        .parse()
        .map_err(|_| format!("meta.toml: bad t0_ms {t0:?}"))?;
    let seed = get("rng_at_t0")
        .map(|v| {
            v.parse()
                .map_err(|_| format!("meta.toml: bad rng_at_t0 {v:?}"))
        })
        .transpose()?;
    let start = get("teleport")
        .map(|v| parse_point(&v).ok_or_else(|| format!("meta.toml: bad teleport {v:?}")))
        .transpose()?;
    Ok(Some(StoreStart { start, t0_ms, seed }))
}

/// How the store holding the reference trace `trace` started its runs of
/// the trace's clock (see [`store_start`]); `None` when no `meta.toml`
/// lies beside it.
fn store_start_of(trace: &std::path::Path) -> Result<Option<StoreStart>, String> {
    let Some(dir) = trace.parent() else {
        return Ok(None);
    };
    let path = dir.join("meta.toml");
    let Ok(meta) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    store_start(&meta, trace_clock(trace).as_deref())
        .map_err(|e| format!("{}: {e}", path.display()))
}

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
    checkpoint: Option<(i32, i32)>,
    logics: Option<bool>,
    mid_air_hit_fix: Option<bool>,
    aids: PlayAids,
    powerup: Option<AidPowerup>,
    tuning: Option<ScriptTuning>,
    crabs: Option<PathBuf>,
    crab_variant: Option<usize>,
    expect_complete: bool,
    compare: Option<PathBuf>,
    tolerance: Option<String>,
    report: Option<PathBuf>,
    next: Vec<PathBuf>,
    carry_out: Option<PathBuf>,
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
        if flag == "--invincible" {
            a.aids.invincible = true;
            continue;
        }
        if flag == "--infinite-lives" {
            a.aids.infinite_lives = true;
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
            "--powerup" => {
                let v = value()?;
                a.powerup = Some(AidPowerup::parse(&v).map_err(|e| format!("--powerup: {e}"))?);
            }
            "--script" => a.script = Some(PathBuf::from(value()?)),
            "--crabs" => a.crabs = Some(PathBuf::from(value()?)),
            "--crab-variant" => {
                let v = value()?;
                a.crab_variant = Some(
                    v.parse()
                        .ok()
                        .filter(|&n| n >= 1)
                        .ok_or_else(|| format!("bad crab value set {v:?}, expected 1 or more"))?,
                );
            }
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
                let p = parse_point(&v).ok_or_else(|| format!("bad start {v:?}, expected X,Y"))?;
                a.start = Some(p);
            }
            "--checkpoint" => {
                let v = value()?;
                let p =
                    parse_point(&v).ok_or_else(|| format!("bad checkpoint {v:?}, expected X,Y"))?;
                a.checkpoint = Some(p);
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
            "--next" => a.next.push(PathBuf::from(value()?)),
            "--carry-out" => a.carry_out = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
        }
    }
    if a.compare.is_some() != a.tolerance.is_some() {
        return Err(format!("--compare and --tolerance go together\n{USAGE}"));
    }
    if a.report.is_some() && a.compare.is_none() {
        return Err(format!("--report needs --compare\n{USAGE}"));
    }
    if a.crabs.is_some() && a.crab_variant.is_some() {
        return Err(format!(
            "--crabs and --crab-variant do not go together\n{USAGE}"
        ));
    }
    if !a.next.is_empty() {
        let refused = [
            ("--level", a.level.is_some()),
            ("--custom", a.custom.is_some()),
            ("--seed", a.seed.is_some()),
            ("--tuning", a.tuning.is_some()),
            ("--start", a.start.is_some()),
            ("--checkpoint", a.checkpoint.is_some()),
            ("--logics", a.logics.is_some()),
            ("--mid-air-hit-fix", a.mid_air_hit_fix.is_some()),
            ("--invincible", a.aids.invincible),
            ("--infinite-lives", a.aids.infinite_lives),
            ("--powerup", a.powerup.is_some()),
            ("--crabs", a.crabs.is_some()),
            ("--crab-variant", a.crab_variant.is_some()),
            ("--compare", a.compare.is_some()),
        ];
        if let Some((flag, _)) = refused.iter().find(|(_, given)| *given) {
            return Err(format!(
                "{flag} does not go with --next: each level of a chain plays as its route \
                 names\n{USAGE}"
            ));
        }
    }
    Ok(a)
}

/// `X,Y` as a point.
fn parse_point(v: &str) -> Option<(i32, i32)> {
    let (x, y) = v.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
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

/// The fuse clock a script's `# fuse_clock = ...` line names (the last
/// such line counts; the original's without one), after checking that its
/// `# pause_input = ...` line, if any, names the original's: the replay
/// plays keys across a pause or a screen only the original's way, so a
/// recording made another way would not replay as it was played.
fn script_switches(text: &str) -> Result<FuseClock, String> {
    let mut fuse = FuseClock::Original;
    let mut pause: Option<(usize, String)> = None;
    for (n, line) in text.lines().enumerate() {
        let Some((k, v)) = line
            .trim()
            .strip_prefix('#')
            .and_then(|r| r.split_once('='))
        else {
            continue;
        };
        let v = v.trim().trim_matches('"');
        match k.trim() {
            FUSE_CLOCK_KEY => {
                fuse = FuseClock::from_name(v).ok_or_else(|| {
                    format!(
                        "line {}: `{FUSE_CLOCK_KEY} = {v}` is neither original nor frozen",
                        n + 1
                    )
                })?;
            }
            PAUSE_INPUT_KEY => pause = Some((n + 1, v.to_string())),
            _ => {}
        }
    }
    if let Some((n, v)) = pause
        && v != "original"
    {
        return Err(format!(
            "line {n}: the script was recorded with `{PAUSE_INPUT_KEY} = {v}`; the replay \
             plays keys across a pause or a screen only the original's way"
        ));
    }
    Ok(fuse)
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
    let script = InputScript::parse_with_aids(&text)
        .map_err(|e| format!("{}: {e}", script_path.display()))?;
    let header = script.header;
    let carry = parse_start_lines(&text).map_err(|e| format!("{}: {e}", script_path.display()))?;
    let aids = args.aids.union(header.aids);
    let powerup = args.powerup.or(header.powerup).filter(|p| !p.is_none());
    let powerup_commands = script
        .directives()
        .iter()
        .any(|d| matches!(d.kind, DirectiveKind::Command(ScriptCommand::Powerup(_))));
    // Every play-test aid of the run, from the script and the command line.
    let mut run_aids = header;
    run_aids.aids = aids;
    run_aids.powerup = powerup;
    run_aids.powerup_key |= powerup_commands;
    if args.compare.is_some() && run_aids.any_aid() {
        return Err(format!(
            "--compare takes plain play only, and this run has a play-test aid on ({}; from {}): \
             a run with an aid says nothing about the game's own play",
            run_aids.aid_lines().join(", "),
            if header.any_aid() || powerup_commands {
                "the script's header"
            } else {
                "the command line"
            }
        ));
    }
    let checkpoint = match args.checkpoint {
        Some(p) => Some(p),
        None => script_checkpoint(&text).map_err(|e| format!("{}: {e}", script_path.display()))?,
    };
    let start_health =
        script_health(&text).map_err(|e| format!("{}: {e}", script_path.display()))?;
    let crabs = crab_values(&text, args.crabs.as_deref(), args.crab_variant)
        .map_err(|e| format!("{}: {e}", script_path.display()))?;
    let fix = args
        .mid_air_hit_fix
        .or(script_behaviour(&text))
        .unwrap_or(false);
    let fuse_clock =
        script_switches(&text).map_err(|e| format!("{}: {e}", script_path.display()))?;
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
    let store_t0 = match &args.compare {
        Some(path) => store_start_of(path)?,
        None => None,
    };
    if let (Some(flag), Some(want)) = (args.seed, store_t0.and_then(|t| t.seed))
        && flag != want
    {
        eprintln!(
            "claw-replay: --seed {flag} overrides the store's random state {want} at time zero"
        );
    }
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
    if let Some(seed) = args.seed.or(store_t0.and_then(|t| t.seed)).or(header.seed) {
        cfg.seed = seed;
    }
    if let Some(on) = args.logics {
        cfg.level_logics = on;
    }
    cfg.tuning.behaviour.fix_double_jump = fix;
    cfg.fuse_clock = fuse_clock;
    if fuse_clock != FuseClock::Original {
        eprintln!(
            "claw-replay: the script was recorded with `{FUSE_CLOCK_KEY} = {}`: a help screen's \
             return runs no update pass",
            fuse_clock.name()
        );
    }
    cfg.aids = aids;
    cfg.start_powerup = powerup;
    cfg.boss_health = claw_sim::input::script_boss_health(&text)
        .map_err(|e| format!("{}: {e}", script_path.display()))?;
    if let Some(h) = cfg.boss_health {
        eprintln!("claw-replay: the boss is given health {h} on the first arena step");
    }
    let carried = carry.as_ref().map(|c| {
        format!(
            ", carried in: score {}, health {}, lives {}",
            c.score, c.health, c.lives
        )
    });
    let mut sim = Sim::with_start(sim_level, cfg, StartState { carry, save: None });
    crabs.apply(&mut sim);
    if !crabs.is_empty() {
        eprintln!(
            "claw-replay: crab values given for {} entr{} ({})",
            crabs.entries.len(),
            if crabs.entries.len() == 1 { "y" } else { "ies" },
            match (&args.crabs, args.crab_variant) {
                (Some(p), _) => p.display().to_string(),
                (None, Some(n)) => format!("the script's further set {n}"),
                (None, None) => "the script's header".to_string(),
            }
        );
    }
    let start = args.start.or(store_t0.and_then(|t| t.start));
    match (start, store_t0) {
        (Some((x, y)), Some(t0)) => {
            start_at_time_zero(&mut sim, (x, y), t0, &crabs);
            eprintln!(
                "claw-replay: as the store's runs: Claw placed at {x},{y} on the level's first \
                 tick, the route's time zero at {} ms ({} ticks), the random stream set to {} there",
                t0.t0_ms,
                t0.ticks(),
                sim.config().seed
            );
        }
        (Some((x, y)), None) => settle_at(&mut sim, x, y, &crabs),
        (None, _) => {}
    }
    if let Some(p) = checkpoint {
        sim.set_checkpoint(Some(p));
    }
    if let Some(h) = start_health {
        sim.set_health(h);
        eprintln!("claw-replay: Claw's health set to {h} at the script's time zero");
    }
    let mut rows = Vec::with_capacity(script.ticks.len());
    let mut completed = false;
    let mut directives = script.directives().iter().peekable();
    let mut commands = 0;
    for (tick, &input) in script.ticks.iter().enumerate() {
        while let Some(d) = directives.next_if(|d| d.tick <= tick) {
            if sim.play_directive(&d.kind) {
                commands += 1;
            }
        }
        sim.step(input);
        crabs.apply(&mut sim);
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
    if run_aids.any_aid() {
        eprintln!(
            "claw-replay: play-test aids on: {}",
            run_aids.aid_lines().join(", ")
        );
    }
    eprintln!(
        "claw-replay: {} ticks, {} directives ({} commands), tuning {}, seed {}, mid-air hit fix {}, activation {}, level logics {}, {} objects ({} with unknown logics, {} with logics not run yet, {} records dropped){}{}{}",
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
        carried.unwrap_or_default(),
        checkpoint
            .map(|(x, y)| format!(", respawn point set to {x},{y} at time zero"))
            .unwrap_or_default(),
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

    if let Some(path) = &args.carry_out {
        write_carry(path, completed.then(|| hands_on(&sim)))?;
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

/// Starts a segment as a reference store's runs started it (see
/// [`StoreStart`]): Claw placed at `at` on the level's first tick and left
/// there, with no button held, for the whole ticks up to the route's time
/// zero, then the shared random stream set to the configured seed, once.
fn start_at_time_zero(sim: &mut Sim, at: (i32, i32), t0: StoreStart, crabs: &CrabValues) {
    sim.teleport_player(at.0, at.1);
    for _ in 0..t0.ticks() {
        sim.step(Default::default());
        crabs.apply(sim);
    }
    let seed = sim.config().seed;
    sim.with_world(|w| *w.rng = Rng::new(seed));
}

/// Moves Claw and waits until he and the camera have stood still for ten
/// ticks (at most four seconds), then sets the shared random stream to the
/// configured seed: the script starts from it.
fn settle_at(sim: &mut Sim, x: i32, y: i32, crabs: &CrabValues) {
    settle(sim, x, y, crabs);
    let seed = sim.config().seed;
    sim.with_world(|w| *w.rng = Rng::new(seed));
}

/// Moves Claw and waits until he and the camera have stood still for ten
/// ticks (at most four seconds).
fn settle(sim: &mut Sim, x: i32, y: i32, crabs: &CrabValues) {
    sim.teleport_player(x, y);
    let mut still = 0;
    let mut last = None;
    for _ in 0..222 {
        sim.step(Default::default());
        crabs.apply(sim);
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

/// The crab values of a run (see the module documentation): the file
/// `file` names, else the header's further set `variant`, else the
/// script's own header lines.
fn crab_values(
    script_text: &str,
    file: Option<&std::path::Path>,
    variant: Option<usize>,
) -> Result<CrabValues, String> {
    if let Some(n) = variant {
        let sets = CrabValues::variants_from_header(script_text)
            .map_err(|e| format!("the further crab values: {e}"))?;
        let count = sets.len();
        return sets
            .into_iter()
            .nth(n - 1)
            .ok_or_else(|| format!("--crab-variant {n}: the script names {count} further set(s)"));
    }
    match file {
        Some(p) => {
            let text =
                std::fs::read_to_string(p).map_err(|e| format!("reading {}: {e}", p.display()))?;
            CrabValues::parse(&text).map_err(|e| format!("--crabs {}: {e}", p.display()))
        }
        None => CrabValues::from_header(script_text).map_err(|e| format!("the crab values: {e}")),
    }
}

/// What a won level hands on to the next level of one game, as the
/// game's flow carries it through the booty screen: Claw's progress at
/// the level's end (play stops in the tick the level is won, see
/// [`Sim::level_ended`]), with one more life when the tally was perfect
/// (every treasure row collected everything the level holds of its kind,
/// the booty screen's test, see [`perfect_tally`]) and Claw has fewer
/// than the most lives.
pub fn hands_on(sim: &Sim) -> Carry {
    let mut carry = sim.carry();
    let perfect = perfect_tally(&sim.stats().treasure_level, &sim.treasure_census());
    if perfect && carry.lives < MAX_LIVES {
        carry.lives += 1;
    }
    carry
}

/// Writes the start lines of `carry` to `path` for `--carry-out`, or says
/// why nothing was written.
fn write_carry(path: &Path, carry: Option<Carry>) -> Result<(), String> {
    match carry {
        Some(c) => {
            std::fs::write(path, start_lines(&c))
                .map_err(|e| format!("writing {}: {e}", path.display()))?;
            eprintln!(
                "claw-replay: the progress handed on to the next level written to {}",
                path.display()
            );
        }
        None => eprintln!(
            "claw-replay: the level did not end, so nothing is handed on and {} was not written",
            path.display()
        ),
    }
    Ok(())
}

/// The trace file of level `level` of a chain whose first level's trace
/// is `base`: `base` itself for the first level, `<stem>.level<N>.csv`
/// beside it after it.
fn chain_trace_path(base: &Path, level: u32, first: bool) -> PathBuf {
    if first {
        return base.to_path_buf();
    }
    let stem = base
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    base.with_file_name(format!("{stem}.level{level}.csv"))
}

/// One level of a chain, played from its route.
struct ChainPart {
    level: u32,
    tuning: Option<ScriptTuning>,
    rows: Vec<TraceRow>,
    /// The tick the level was won.
    won_at: Option<usize>,
    /// Claw's deaths.
    deaths: usize,
    /// The route's End Life commands (the deaths it gives up on purpose).
    end_lives: usize,
    hands_on: Carry,
}

/// Plays the route `path` as one level of a chain: as a replay of it alone
/// plays it, but from the progress `carry` (none for a new game).
fn play_part(
    store: &claw_assets::AssetStore,
    path: &Path,
    text: &str,
    carry: Option<Carry>,
    view: ViewConfig,
) -> Result<ChainPart, String> {
    let named = |e: String| format!("{}: {e}", path.display());
    let named_err = |e: claw_sim::input::ScriptError| named(e.to_string());
    let script = InputScript::parse_with_aids(text).map_err(named_err)?;
    let header = script.header;
    let powerup_commands = script
        .directives()
        .iter()
        .any(|d| matches!(d.kind, DirectiveKind::Command(ScriptCommand::Powerup(_))));
    if header.any_aid() || powerup_commands {
        return Err(named(
            "a chain is plain play, and the route was played with a play-test aid".to_string(),
        ));
    }
    if script_checkpoint(text).map_err(named)?.is_some()
        || script_health(text).map_err(named)?.is_some()
        || claw_sim::input::script_boss_health(text)
            .map_err(named)?
            .is_some()
    {
        return Err(named(
            "a chain is plain play, and the route names a diagnostic set-up \
             (`# checkpoint`, `# health` or `# boss_health`)"
                .to_string(),
        ));
    }
    let level = header
        .level
        .ok_or_else(|| named("the route of a chain names its level (`# level N`)".to_string()))?;
    let crabs =
        CrabValues::from_header(text).map_err(|e| named(format!("the crab values: {e}")))?;
    let mut cfg = SimConfig::for_view(view);
    if let Some(tuning) = header.tuning {
        cfg.tuning = tuning.tuning();
    }
    if let Some(seed) = header.seed {
        cfg.seed = seed;
    }
    cfg.tuning.behaviour.fix_double_jump = script_behaviour(text).unwrap_or(false);
    cfg.fuse_clock = script_switches(text).map_err(named)?;
    let built = LevelData::build(store, LevelSource::Retail(level))
        .map_err(|e| format!("loading level {level}: {e}"))?;
    let sim_level =
        SimLevel::from_level(&built, store).map_err(|e| format!("preparing level {level}: {e}"))?;
    let mut sim = Sim::with_start(sim_level, cfg, StartState { carry, save: None });
    crabs.apply(&mut sim);
    let mut rows = Vec::with_capacity(script.ticks.len());
    let mut won_at = None;
    let mut deaths = 0;
    let mut end_lives = 0;
    let mut directives = script.directives().iter().peekable();
    for (tick, &input) in script.ticks.iter().enumerate() {
        while let Some(d) = directives.next_if(|d| d.tick <= tick) {
            if d.kind == DirectiveKind::Command(ScriptCommand::EndLife) {
                end_lives += 1;
            }
            sim.play_directive(&d.kind);
        }
        sim.step(input);
        crabs.apply(&mut sim);
        for e in sim.last_events() {
            match e {
                GameEvent::LevelComplete { .. } => {
                    won_at.get_or_insert(tick);
                }
                GameEvent::ClawDied { .. } | GameEvent::GameOver => deaths += 1,
                _ => {}
            }
        }
        rows.push(TraceRow::capture(&sim, tick as u64, "script"));
    }
    Ok(ChainPart {
        level,
        tuning: header.tuning,
        rows,
        won_at,
        deaths,
        end_lives,
        hands_on: hands_on(&sim),
    })
}

/// Plays a chain of routes as one game through every booty screen (see
/// the module documentation, `--next`).
fn run_chain(args: Args) -> Result<Outcome, String> {
    let install_path = args.install.ok_or(USAGE)?;
    let first = args.script.ok_or(USAGE)?;
    let mut view = ViewConfig::default();
    if let Some((w, h)) = args.res {
        view.render_width = w;
        view.render_height = h;
    }
    if let Some(aspect) = args.aspect {
        view.aspect_policy = aspect;
    }
    // Every route is read before anything is played.
    let mut routes = Vec::new();
    for path in std::iter::once(first).chain(args.next) {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        routes.push((path, text));
    }
    let install = Install::open(&install_path)
        .map_err(|e| format!("opening install {}: {e}", install_path.display()))?;
    let store = install
        .store()
        .map_err(|e| format!("mounting assets: {e}"))?;
    let mut outcome = Outcome::default();
    let mut carried: Option<Carry> = None;
    let mut previous: Option<(u32, Option<ScriptTuning>)> = None;
    let mut last_hands_on = None;
    for (i, (path, text)) in routes.iter().enumerate() {
        let own = parse_start_lines(text).map_err(|e| format!("{}: {e}", path.display()))?;
        let carry = match &carried {
            None => own,
            Some(c) => {
                if own.as_ref() != Some(c) {
                    outcome.diverged = true;
                    println!(
                        "{}: the route starts with other progress than the game carries in; \
                         the game carries in:\n{}",
                        path.display(),
                        start_lines(c).trim_end()
                    );
                }
                Some(c.clone())
            }
        };
        let part = play_part(&store, path, text, carry, view)?;
        if let Some((level, tuning)) = previous {
            if part.level != level + 1 {
                return Err(format!(
                    "{}: level {} does not follow level {level}",
                    path.display(),
                    part.level
                ));
            }
            if part.tuning != tuning {
                return Err(format!(
                    "{}: one game has one tuning, and the route names another",
                    path.display()
                ));
            }
        }
        if let Some(base) = &args.trace {
            let out = chain_trace_path(base, part.level, i == 0);
            std::fs::write(&out, trace::to_csv(&part.rows))
                .map_err(|e| format!("writing {}: {e}", out.display()))?;
        }
        let end = part.rows.last();
        let other_deaths = part.deaths.saturating_sub(part.end_lives);
        if other_deaths > 0 {
            outcome.diverged = true;
        }
        println!(
            "level {} ({}): {} of {} ticks, health {}, score {}, lives {}, deaths {} ({} by End \
             Life){}",
            part.level,
            path.display(),
            part.won_at
                .map(|t| format!("won at tick {t}"))
                .unwrap_or_else(|| "not won".to_string()),
            part.rows.len(),
            end.map_or(0, |r| r.health),
            end.map_or(0, |r| r.score),
            end.map_or(0, |r| r.lives),
            part.deaths,
            part.end_lives.min(part.deaths),
            if other_deaths > 0 {
                format!("; {other_deaths} other death(s)")
            } else {
                String::new()
            }
        );
        if part.won_at.is_none() {
            eprintln!(
                "claw-replay: level {} was not completed, so the chain stops there",
                part.level
            );
            outcome.incomplete = true;
            last_hands_on = None;
            break;
        }
        previous = Some((part.level, part.tuning));
        carried = Some(part.hands_on.clone());
        last_hands_on = Some(part.hands_on);
    }
    if let Some(path) = &args.carry_out {
        write_carry(path, last_hands_on)?;
    }
    Ok(outcome)
}

fn main() -> ExitCode {
    let result = parse_args(std::env::args().skip(1)).and_then(|a| {
        if a.next.is_empty() {
            run(a)
        } else {
            run_chain(a)
        }
    });
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
        let a = args("--install /i --level 1 --script s.csv --trace t.csv --res 3440x1440 --aspect widened --seed 7 --start 6272,3150 --checkpoint 6300,3150 --logics off").unwrap();
        assert_eq!(a.checkpoint, Some((6300, 3150)));
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
    fn a_chain_takes_its_routes_and_refuses_the_options_of_one_level() {
        let a = args(
            "--install /i --script a.csv --next b.csv --next c.csv --trace t.csv \
             --carry-out c.txt --expect-complete --res 640x480",
        )
        .unwrap();
        assert_eq!(a.next, vec![PathBuf::from("b.csv"), PathBuf::from("c.csv")]);
        assert_eq!(a.carry_out, Some(PathBuf::from("c.txt")));
        for extra in [
            "--level 2",
            "--seed 7",
            "--tuning original",
            "--start 1,2",
            "--checkpoint 1,2",
            "--logics off",
            "--mid-air-hit-fix on",
            "--invincible",
            "--infinite-lives",
            "--powerup fire",
            "--crabs f.txt",
            "--crab-variant 1",
            "--compare r.csv --tolerance exact",
        ] {
            let e = args(&format!("--install /i --script a.csv --next b.csv {extra}")).unwrap_err();
            assert!(e.contains("does not go with --next"), "{extra}: {e}");
            assert!(
                args(&format!("--install /i --script a.csv {extra}")).is_ok(),
                "{extra} alone"
            );
        }
        // The carried progress is written for one level too.
        assert!(args("--install /i --script a.csv --carry-out c.txt").is_ok());
    }

    #[test]
    fn a_chain_writes_one_trace_per_level_as_a_recorded_game_names_its_files() {
        let base = Path::new("out/game.csv");
        assert_eq!(
            chain_trace_path(base, 1, true),
            PathBuf::from("out/game.csv")
        );
        assert_eq!(
            chain_trace_path(base, 2, false),
            PathBuf::from("out/game.level2.csv")
        );
        assert_eq!(
            chain_trace_path(Path::new("game.csv"), 14, false),
            PathBuf::from("game.level14.csv")
        );
    }

    #[test]
    fn a_level_hands_on_its_progress_and_a_perfect_tallys_extra_life() {
        use claw_sim::{SimConfig, TileMap};
        let mut rows = vec![".".repeat(40); 15];
        rows.push("#".repeat(40));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (300, 880));
        let lines = "# start_score = 99700\n# start_health = 35\n# start_lives = 7\n\
                     # start_pistol = 4\n# start_magic = 5\n# start_dynamite = 3\n\
                     # start_weapon = pistol\n# start_treasure_game = 87, 0, 4, 4, 3, 0, 6, 0, 0\n";
        let carry = parse_start_lines(lines).unwrap().unwrap();
        let sim = Sim::with_start(
            level.clone(),
            SimConfig::default(),
            StartState {
                carry: Some(carry.clone()),
                save: None,
            },
        );
        // A level with no treasure: the tally is perfect (every row
        // collected all of its kind), so one more life goes with Claw.
        assert_eq!(
            sim.treasure_census(),
            [0; claw_sim::player::stats::TREASURE_KINDS]
        );
        let out = hands_on(&sim);
        assert_eq!(out.lives, carry.lives + 1);
        assert_eq!(
            Carry {
                lives: carry.lives,
                ..out.clone()
            },
            carry,
            "the rest is the progress as it is"
        );
        assert_eq!(parse_start_lines(&start_lines(&out)).unwrap(), Some(out));
        // Never more than the most lives.
        let full = Carry {
            lives: MAX_LIVES,
            ..carry
        };
        let sim = Sim::with_start(
            level,
            SimConfig::default(),
            StartState {
                carry: Some(full.clone()),
                save: None,
            },
        );
        assert_eq!(hands_on(&sim).lives, MAX_LIVES);
    }

    #[test]
    fn the_fuse_clock_and_pause_input_come_from_the_script() {
        assert_eq!(
            script_switches("tick,a,b\n0,0,0\n"),
            Ok(FuseClock::Original)
        );
        assert_eq!(
            script_switches("# pause_input = original\n# fuse_clock = original\n"),
            Ok(FuseClock::Original)
        );
        assert_eq!(
            script_switches("# level 9\n# fuse_clock = frozen\ntick,a,b\n0,0,0\n"),
            Ok(FuseClock::Frozen)
        );
        assert_eq!(
            script_switches("# fuse_clock = frozen\n# fuse_clock = \"original\"\n"),
            Ok(FuseClock::Original),
            "the last line counts"
        );
        let e = script_switches("# fuse_clock = sometimes\n").unwrap_err();
        assert!(e.contains("fuse_clock = sometimes"), "{e}");
        let e = script_switches("# pause_input = release_all\n").unwrap_err();
        assert!(
            e.contains("line 1") && e.contains("pause_input = release_all"),
            "{e}"
        );
    }

    #[test]
    fn a_frozen_scripts_help_screen_runs_no_return_pass() {
        use claw_sim::{SimConfig, TileMap};
        let ticks: String = (0..12).map(|n| format!("{n},0,0\n")).collect();
        let text = format!(
            "# fuse_clock = frozen\n# pause_input = original\n\
             tick,held_mask,pressed_mask\n{ticks}@4 menu help 3000\n"
        );
        let text = text.as_str();
        let mut rows = vec![".".repeat(40); 15];
        rows.push("#".repeat(40));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        // The ticks on which the frame pulse comes, with the script's
        // directives played as the replay plays them.
        let pulses = |fuse_clock: FuseClock, script: &InputScript| {
            let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (300, 880));
            let cfg = SimConfig {
                fuse_clock,
                ..SimConfig::default()
            };
            let mut sim = Sim::new(level, cfg);
            let mut out = Vec::new();
            for n in 0..12 {
                sim.play_directives(script, n);
                sim.step(Default::default());
                if sim.pulse() {
                    out.push(n);
                }
            }
            out
        };
        let script = InputScript::parse(text).unwrap();
        let frozen = script_switches(text).unwrap();
        assert_eq!(frozen, FuseClock::Frozen);
        let plain = pulses(FuseClock::Original, &InputScript::default());
        assert_eq!(
            pulses(frozen, &script),
            plain,
            "a frozen fuse clock leaves the pulse as without the screen"
        );
        assert_ne!(
            pulses(FuseClock::Original, &script),
            plain,
            "the original's return pass moves the pulse"
        );
    }

    #[test]
    fn tolerances_come_from_a_preset_or_a_file() {
        assert_eq!(load_tolerances("exact").unwrap(), Tolerances::exact());
        assert_eq!(load_tolerances("feel").unwrap(), Tolerances::feel());
        assert!(load_tolerances("/nonexistent/tolerances.toml").is_err());
    }

    #[test]
    fn the_respawn_point_comes_from_the_flag_or_the_script() {
        let a = args("--install /i --script s.csv --checkpoint 40600,7245").unwrap();
        assert_eq!(a.checkpoint, Some((40600, 7245)));
        assert!(args("--checkpoint 40600").is_err());
        assert!(args("--checkpoint a,b").is_err());
        assert_eq!(
            args("--install /i --script s.csv").unwrap().checkpoint,
            None
        );
        assert_eq!(
            script_checkpoint("# level 10\n# checkpoint 40600,7245\ntick,a,b\n0,0,0\n"),
            Ok(Some((40600, 7245)))
        );
        assert_eq!(
            script_checkpoint("# checkpoint = 1, 2\n# checkpoint 3,4\n"),
            Ok(Some((3, 4))),
            "the last line counts"
        );
        assert_eq!(script_checkpoint("# checkpoints are flags\n"), Ok(None));
        assert_eq!(script_checkpoint("# level 10\ntick,a,b\n"), Ok(None));
        assert!(script_checkpoint("# checkpoint here\n").is_err());
    }

    #[test]
    fn a_respawn_point_set_at_time_zero_is_where_claw_respawns() {
        use claw_sim::{SimConfig, TileMap};
        let mut rows = vec![".".repeat(40); 15];
        rows.push("#".repeat(40));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (300, 880));
        let mut sim = Sim::new(level, SimConfig::default());
        sim.set_checkpoint(Some((1500, 880)));
        sim.end_life();
        for _ in 0..400 {
            sim.step(Default::default());
            if sim.player().is_some_and(|c| c.x == 1500) {
                break;
            }
        }
        assert_eq!(
            sim.player().map(|c| c.x),
            Some(1500),
            "respawned at the set point"
        );
    }

    #[test]
    fn a_health_set_at_time_zero_holds_until_a_respawn_refills_it() {
        use claw_sim::{SimConfig, TileMap};
        assert_eq!(
            script_health("# level 14\n# health 90\ntick,a,b\n0,0,0\n"),
            Ok(Some(90))
        );
        assert_eq!(script_health("# level 14\ntick,a,b\n"), Ok(None));
        assert!(script_health("# health ninety\n").is_err());
        let mut rows = vec![".".repeat(40); 15];
        rows.push("#".repeat(40));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (300, 880));
        let mut sim = Sim::new(level, SimConfig::default());
        sim.step(Default::default());
        sim.set_health(90);
        sim.step(Default::default());
        assert_eq!(sim.claw().stats.health, 90);
        assert_eq!(sim.hud().health, 90);
        sim.end_life();
        let mut refilled = false;
        for _ in 0..400 {
            sim.step(Default::default());
            if sim.claw().stats.health == 100 {
                refilled = true;
                break;
            }
        }
        assert!(refilled, "a respawn refills it");
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
    fn the_aids_come_from_the_flags_or_the_script() {
        let a = args("--install /i --script s.csv --invincible").unwrap();
        assert!(a.aids.invincible && !a.aids.infinite_lives);
        let a = args("--install /i --script s.csv --infinite-lives --invincible").unwrap();
        assert_eq!(
            a.aids,
            PlayAids {
                invincible: true,
                infinite_lives: true
            }
        );
        assert_eq!(
            args("--install /i --script s.csv").unwrap().aids,
            PlayAids::NONE
        );
    }

    /// Writes `text` as a script in a fresh folder and runs `extra`
    /// arguments on it, with an install that does not exist.
    fn run_script(name: &str, text: &str, extra: &str) -> String {
        let dir =
            std::env::temp_dir().join(format!("claw-replay-aids-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("route.csv");
        std::fs::write(&script, text).unwrap();
        let mut a = args(&format!(
            "--install {} --level 1 --script {} {extra}",
            dir.join("no-install").display(),
            script.display()
        ))
        .unwrap();
        a.script = Some(script);
        let e = run(a).unwrap_err();
        std::fs::remove_dir_all(&dir).unwrap();
        e
    }

    #[test]
    fn the_powerup_aid_comes_from_the_flag_or_the_script_and_is_never_compared() {
        let a = args("--install /i --script s.csv --powerup lightning:20").unwrap();
        assert_eq!(
            a.powerup.map(|p| p.to_string()).as_deref(),
            Some("lightning:20")
        );
        assert!(args("--install /i --script s.csv --powerup sword").is_err());
        assert_eq!(args("--install /i --script s.csv").unwrap().powerup, None);
        let plain = "tick,held_mask,pressed_mask\n0,0,0\n";
        // From the command line.
        let e = run_script(
            "powerup-flag",
            plain,
            "--powerup fire --compare ref.csv --tolerance exact",
        );
        assert!(e.contains("plain play only"), "{e}");
        assert!(e.contains("powerup = fire:30"), "{e}");
        assert!(e.contains("command line"), "{e}");
        // From the script: its header, or a powerup command alone.
        for (name, text, line) in [
            (
                "powerup-header",
                "# powerup = catnip:30\ntick,held_mask,pressed_mask\n0,0,0\n",
                "powerup = catnip:30",
            ),
            (
                "powerup-command",
                "tick,held_mask,pressed_mask\n0,0,0\n@1 command powerup ice:30\n",
                "powerup_key = used",
            ),
        ] {
            let e = run_script(name, text, "--compare ref.csv --tolerance exact");
            assert!(e.contains(line), "{name}: {e}");
            assert!(e.contains("header"), "{name}: {e}");
            // Replayed: the run stops at the missing install.
            let e = run_script(&format!("{name}-replay"), text, "");
            assert!(e.contains("opening install"), "{name}: {e}");
        }
    }

    #[test]
    fn a_recording_with_an_aid_replays_but_is_never_compared() {
        let aided = "# invincible = on\ntick,held_mask,pressed_mask\n0,0,0\n";
        // Read and replayed: the run stops at the missing install.
        let e = run_script("replay", aided, "");
        assert!(e.contains("opening install"), "{e}");
        // Compared: refused before anything is loaded.
        let e = run_script("compare", aided, "--compare ref.csv --tolerance exact");
        assert!(e.contains("plain play only"), "{e}");
        assert!(e.contains("invincible = on"), "{e}");
        assert!(e.contains("header"), "{e}");
        // An aid from the command line is refused the same way.
        let plain = "tick,held_mask,pressed_mask\n0,0,0\n";
        let e = run_script(
            "flag",
            plain,
            "--infinite-lives --compare ref.csv --tolerance exact",
        );
        assert!(e.contains("infinite_lives = on"), "{e}");
        assert!(e.contains("command line"), "{e}");
        // Plain play is compared as before (here it stops at the install).
        let e = run_script("plain", plain, "--compare ref.csv --tolerance exact");
        assert!(e.contains("opening install"), "{e}");
    }

    #[test]
    fn a_later_level_starts_from_the_progress_its_start_lines_name() {
        let lines = "# level 2\n# start_score = 99700\n# start_health = 100\n\
                     # start_lives = 7\n# start_pistol = 10\n# start_magic = 5\n\
                     # start_dynamite = 3\n# start_weapon = pistol\n\
                     # start_treasure_game = 87, 0, 4, 4, 3, 0, 6, 0, 0\n";
        // Read: the run stops at the missing install.
        let e = run_script(
            "carry",
            &format!("{lines}tick,held_mask,pressed_mask\n0,0,0\n"),
            "",
        );
        assert!(e.contains("opening install"), "{e}");
        // Part of the progress only: refused before anything is loaded.
        let e = run_script(
            "carry-part",
            "# start_score = 99700\ntick,held_mask,pressed_mask\n0,0,0\n",
            "",
        );
        assert!(e.contains("start_health is missing"), "{e}");
        // Applied to the level's start.
        use claw_sim::{SimConfig, TileMap};
        let mut rows = vec![".".repeat(40); 15];
        rows.push("#".repeat(40));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (300, 880));
        let carry = parse_start_lines(lines).unwrap();
        let sim = Sim::with_start(
            level,
            SimConfig::default(),
            StartState { carry, save: None },
        );
        assert_eq!(
            (sim.stats().score, sim.stats().health, sim.stats().lives),
            (99_700, 100, 7)
        );
    }

    #[test]
    fn a_start_point_settles_claw_and_starts_the_random_stream_from_the_seed() {
        use claw_sim::{SimConfig, TileMap};
        let mut rows = vec![".".repeat(40); 15];
        rows.push("#".repeat(40));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (300, 880));
        let cfg = SimConfig {
            seed: 111_567,
            ..SimConfig::default()
        };
        let mut sim = Sim::new(level, cfg);
        settle_at(&mut sim, 700, 700, &CrabValues::default());
        assert!(sim.tick() > 10, "Claw fell and stood");
        let claw = sim.player().unwrap();
        assert_eq!(claw.x, 700);
        assert!(claw.y > 700, "landed on the floor: {}", claw.y);
        assert_eq!(sim.rng_state(), 111_567);
    }

    /// A store's meta as the reference stores write it, with one clock
    /// table.
    const META: &str = "# Reference store.\nlevel = 10\nname = \"boss-x-c18\"\n\
        clock = \"c18\"\nrng_at_t0 = 111567\n\n[c18]\nclock = \"c18\"\n\
        traces = [\"trace-c18-r1.csv\"]\nt0_ms = 828\nt0_rule = \"teleport\"\n\
        rng_at_t0 = 111567\nteleport = \"32340,7740\"\n";

    #[test]
    fn a_stores_time_zero_comes_from_its_meta() {
        let want = StoreStart {
            start: Some((32340, 7740)),
            t0_ms: 828,
            seed: Some(111_567),
        };
        assert_eq!(store_start(META, Some("c18")), Ok(Some(want)));
        assert_eq!(want.ticks(), 46, "828 ms in whole 18 ms ticks");
        // The store's one clock when the trace names none.
        assert_eq!(store_start(META, None), Ok(Some(want)));
        // The clock's own table first, then the top level.
        let two = "rng_at_t0 = 5\nteleport = \"1,2\"\n[c18]\nt0_ms = 828\nt0_rule = \"teleport\"\n\
                   [c60]\nt0_ms = 817\nt0_rule = \"teleport\"\nrng_at_t0 = 6\n";
        assert_eq!(
            store_start(two, Some("c60")),
            Ok(Some(StoreStart {
                start: Some((1, 2)),
                t0_ms: 817,
                seed: Some(6),
            }))
        );
        assert_eq!(
            store_start(two, Some("c18")).unwrap().unwrap().seed,
            Some(5)
        );
        // A store whose runs start at the level's start changes nothing.
        let level_start = "level = 1\nrng_at_t0 = 7\n[c18]\nt0_ms = 0\nt0_rule = \"level_start\"\n";
        assert_eq!(store_start(level_start, Some("c18")), Ok(None));
        assert_eq!(store_start("level = 1\n", Some("c18")), Ok(None));
        assert!(store_start("[c18]\nt0_rule = \"teleport\"\nt0_ms = soon\n", Some("c18")).is_err());
        assert!(
            store_start(
                "[c18]\nt0_rule = \"teleport\"\nt0_ms = 1\nteleport = \"1\"\n",
                Some("c18")
            )
            .is_err()
        );
    }

    #[test]
    fn a_reference_trace_names_its_clock() {
        let clock = |p: &str| trace_clock(std::path::Path::new(p));
        assert_eq!(
            clock("/r/level10/boss/trace-c18-r1.csv").as_deref(),
            Some("c18")
        );
        assert_eq!(clock("trace-c60-r2.csv").as_deref(), Some("c60"));
        assert_eq!(clock("/r/run.csv"), None);
    }

    #[test]
    fn a_trace_beside_a_meta_starts_from_the_stores_time_zero() {
        let dir = std::env::temp_dir().join(format!("claw-replay-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let trace = dir.join("trace-c18-r1.csv");
        assert_eq!(store_start_of(&trace), Ok(None), "no meta");
        std::fs::write(dir.join("meta.toml"), META).unwrap();
        let got = store_start_of(&trace).unwrap().expect("the store's start");
        assert_eq!((got.t0_ms, got.seed), (828, Some(111_567)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_stores_time_zero_places_claw_on_the_first_tick_without_settling() {
        use claw_sim::{SimConfig, TileMap};
        let mut rows = vec![".".repeat(40); 15];
        rows.push("#".repeat(40));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (300, 880));
        let cfg = SimConfig {
            seed: 111_567,
            ..SimConfig::default()
        };
        let t0 = StoreStart {
            start: Some((700, 700)),
            t0_ms: 828,
            seed: Some(111_567),
        };
        let mut sim = Sim::new(level.clone(), cfg.clone());
        start_at_time_zero(&mut sim, (700, 700), t0, &CrabValues::default());
        // 46 ticks of the level, whatever Claw did in them, and the seed
        // there.
        assert_eq!(sim.tick(), 46);
        assert_eq!(sim.player().unwrap().x, 700);
        assert_eq!(sim.rng_state(), 111_567);
        // The settled start waits for him to stand instead.
        let mut settled = Sim::new(level, cfg);
        settle_at(&mut settled, 700, 700, &CrabValues::default());
        assert_ne!(settled.tick(), sim.tick());
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

    #[test]
    fn crab_values_come_from_the_header_or_from_a_file() {
        assert_eq!(
            args("--crabs c.txt").unwrap().crabs,
            Some(PathBuf::from("c.txt"))
        );
        assert!(args("--crabs").is_err());
        let script = "# level 7\n# crab 101 = 2, 300, 40\ntick,held_mask,pressed_mask\n0,0x0,0x0\n";
        let v = crab_values(script, None, None).unwrap();
        assert_eq!(v.header_lines(), "# crab 101 = 2, 300, 40\n");
        assert!(crab_values("0,0x0,0x0\n", None, None).unwrap().is_empty());
        // A further set of the header.
        let with_sets = format!("{script}# crab_variant 1: crab 101 = 2, 382, 382\n");
        let v = crab_values(&with_sets, None, Some(1)).unwrap();
        assert_eq!(v.file_lines(), "crab 101 = 2, 382, 382\n");
        let e = crab_values(&with_sets, None, Some(2)).unwrap_err();
        assert!(e.contains("names 1 further set"), "{e}");
        assert_eq!(args("--crab-variant 2").unwrap().crab_variant, Some(2));
        assert!(args("--crab-variant 0").is_err());
        assert!(args("--crabs c.txt --crab-variant 1").is_err());
        // A file replaces the header's values.
        let dir = std::env::temp_dir().join(format!("claw-replay-crabs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("crabs.txt");
        std::fs::write(&file, "# a run\nnest_crab 1304 1 = 0, 0, 636097\n").unwrap();
        let v = crab_values(script, Some(&file), None).unwrap();
        assert_eq!(v.file_lines(), "nest_crab 1304 1 = 0, 0, 636097\n");
        std::fs::write(&file, "crab 101 = 2, 300\n").unwrap();
        let e = crab_values(script, Some(&file), None).unwrap_err();
        assert!(e.contains("--crabs") && e.contains("line 1"), "{e}");
        assert!(crab_values(script, Some(&dir.join("missing.txt")), None).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
        let e = crab_values("# crab 101 = x, 1, 1\n", None, None).unwrap_err();
        assert!(e.contains("bad side"), "{e}");
    }
}
