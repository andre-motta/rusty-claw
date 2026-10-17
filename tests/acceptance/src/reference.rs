//! Comparing the game with reference traces of the same route.
//!
//! A reference store holds runs of one route in the original game, each
//! written as a reference trace of the same route, in the same trace format
//! (see [`claw_sim::trace`]). It lives outside the repository, in the folder
//! the `CLAW_REFERENCE` environment variable names, one folder per route:
//!
//! ```text
//! $CLAW_REFERENCE/level<N>/<name>/route.csv           the route played
//! $CLAW_REFERENCE/level<N>/<name>/trace-<clock>-r<k>.csv
//! $CLAW_REFERENCE/level<N>/<name>/meta.toml           how it was played
//! ```
//!
//! The clock is `c18`, one 18 ms logic step per frame (the game's own
//! step, compared tick for tick with the original speeds and the same
//! random state), or `c60`, frames of 17, 17 and 16 ms (60 frames per
//! second, compared for feel with the default tuning). `meta.toml` names
//! the level, the state of the shared random stream at the route's time
//! zero (`rng_at_t0`) and, per clock, a table with the tuning and the trace
//! files.
//!
//! [`Store::compare_clock`] replays the route in the game with that clock's
//! tuning and the random state at time zero, keeps stepping with no button
//! held for as long as the reference runs past the end of the route, and
//! compares the result with every reference trace of the clock.
//!
//! The tolerances live in `tests/acceptance/tolerances/`: `exact.toml` and
//! `feel.toml`, and `references.toml`, the references the install test
//! checks with the first divergence of each column already known.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use claw_frame::ViewConfig;
use claw_game::load::LevelBundle;
use claw_game::session::script_behaviour;
use claw_game::settings::LevelChoice;
use claw_sim::input::{DirectiveKind, ScriptCommand, ScriptTuning};
use claw_sim::trace::{self, Comparison, Tolerances, TraceRow};
use claw_sim::tuning::TICK_MS;
use claw_sim::{InputScript, Sim, SimConfig};

/// Rows of each trace shown before a divergence.
pub const CONTEXT_ROWS: usize = 5;

/// The folder of the tolerance files.
pub fn tolerance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tolerances")
}

/// Reads `CLAW_REFERENCE`, or prints a skip note and returns `None`.
pub fn reference_root() -> Option<PathBuf> {
    match std::env::var_os("CLAW_REFERENCE") {
        Some(p) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => {
            eprintln!("skipped: CLAW_REFERENCE is not set");
            None
        }
    }
}

/// The clock a reference run was played at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Clock {
    /// One 18 ms logic step per frame.
    C18,
    /// Frames of 17, 17 and 16 ms.
    C60,
}

impl Clock {
    /// Both clocks.
    pub const ALL: [Clock; 2] = [Clock::C18, Clock::C60];

    /// `c18` or `c60`.
    pub fn name(self) -> &'static str {
        match self {
            Clock::C18 => "c18",
            Clock::C60 => "c60",
        }
    }

    /// The clock of a name.
    pub fn from_name(name: &str) -> Option<Clock> {
        Clock::ALL.into_iter().find(|c| c.name() == name)
    }

    /// The tuning the game plays a route of this clock with when the store
    /// does not say: the original speeds for `c18`, the default for `c60`.
    pub fn default_tuning(self) -> ScriptTuning {
        match self {
            Clock::C18 => ScriptTuning::Original,
            Clock::C60 => ScriptTuning::Default,
        }
    }

    /// The tolerance file a comparison at this clock uses when
    /// `references.toml` does not name one: `exact` for `c18`, `feel` for
    /// `c60`.
    pub fn default_tolerance(self) -> &'static str {
        match self {
            Clock::C18 => "exact",
            Clock::C60 => "feel",
        }
    }
}

// ------------------------------------------------------------- small TOML

/// A value of the flat TOML subset the store and tolerance lists use.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `"text"`.
    Text(String),
    /// A whole number.
    Int(i64),
    /// `[a, b, ...]` of texts or whole numbers.
    List(Vec<Value>),
}

impl Value {
    fn parse(s: &str) -> Option<Value> {
        let s = s.trim();
        if let Some(inner) = s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            let items: Option<Vec<Value>> = inner
                .split(',')
                .map(str::trim)
                .filter(|i| !i.is_empty())
                .map(Value::parse)
                .collect();
            return items.map(Value::List);
        }
        if let Some(inner) = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
            return (!inner.contains('"')).then(|| Value::Text(inner.to_string()));
        }
        s.replace('_', "").parse().ok().map(Value::Int)
    }

    /// The text of a `"text"` value.
    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }

    /// The number of a whole-number value.
    pub fn int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// The texts of a list of texts.
    pub fn texts(&self) -> Option<Vec<&str>> {
        match self {
            Value::List(items) => items.iter().map(Value::text).collect(),
            _ => None,
        }
    }
}

fn strip_comment(line: &str) -> &str {
    let mut in_text = false;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => in_text = !in_text,
            '#' if !in_text => return &line[..i],
            _ => {}
        }
    }
    line
}

/// One `[table]` (or `[[table]]`) of a flat TOML file: its name and keys.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Table {
    /// The table name, empty for the keys before the first table.
    pub name: String,
    /// Whether the header was `[[name]]` (one entry of a list of tables).
    pub repeated: bool,
    /// The keys in file order.
    pub keys: Vec<(String, Value)>,
}

impl Table {
    /// The value of `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.keys.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

/// Reads a flat TOML file: top-level keys, then `[table]` and `[[table]]`
/// sections, each with `key = value` lines (texts, whole numbers and
/// one-line lists of them). The first table holds the top-level keys.
pub fn parse_tables(text: &str) -> Result<Vec<Table>, String> {
    let mut tables = vec![Table::default()];
    for (i, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let err = |m: String| format!("line {}: {m}", i + 1);
        if let Some(rest) = line.strip_prefix("[[") {
            let name = rest
                .strip_suffix("]]")
                .ok_or_else(|| err(format!("bad table {line:?}")))?;
            tables.push(Table {
                name: name.trim().to_string(),
                repeated: true,
                keys: Vec::new(),
            });
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            let name = rest
                .strip_suffix(']')
                .ok_or_else(|| err(format!("bad table {line:?}")))?
                .trim();
            if tables.iter().any(|t| !t.repeated && t.name == name) {
                return Err(err(format!("table [{name}] appears twice")));
            }
            tables.push(Table {
                name: name.to_string(),
                repeated: false,
                keys: Vec::new(),
            });
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .map(|(k, v)| (k.trim(), v.trim()))
            .ok_or_else(|| err(format!("expected key = value, found {line:?}")))?;
        let value = Value::parse(value).ok_or_else(|| err(format!("bad value {value:?}")))?;
        let table = tables.last_mut().expect("the top-level table");
        if table.get(key).is_some() {
            return Err(err(format!("key {key} appears twice")));
        }
        table.keys.push((key.to_string(), value));
    }
    Ok(tables)
}

// ------------------------------------------------------------------ stores

/// How the runs of one clock were played.
#[derive(Debug, Clone, PartialEq)]
pub struct ClockRun {
    /// The clock.
    pub clock: Clock,
    /// The tuning the game replays the route with.
    pub tuning: ScriptTuning,
    /// The state of the shared random stream at the route's time zero.
    pub rng_at_t0: Option<u32>,
    /// The reference traces, one per repeat.
    pub traces: Vec<PathBuf>,
}

/// A reference store of one route.
#[derive(Debug, Clone, PartialEq)]
pub struct Store {
    /// The route's name below the store root, for example
    /// `level1/opening`.
    pub name: String,
    /// The route's folder.
    pub dir: PathBuf,
    /// The level the route plays.
    pub level: u32,
    /// The route file.
    pub route: PathBuf,
    /// The runs, one per clock the store holds.
    pub runs: Vec<ClockRun>,
}

fn whole_u32(v: &Value, what: &str) -> Result<u32, String> {
    v.int()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| format!("{what} is not a whole number from 0 to 4294967295"))
}

impl Store {
    /// Reads the store of route `name` (for example `level1/opening`) below
    /// `root`.
    pub fn open(root: &Path, name: &str) -> Result<Store, String> {
        let dir = root.join(name);
        let meta_path = dir.join("meta.toml");
        let text = std::fs::read_to_string(&meta_path)
            .map_err(|e| format!("reading {}: {e}", meta_path.display()))?;
        Store::from_meta(name, &dir, &text).map_err(|e| format!("{}: {e}", meta_path.display()))
    }

    /// The store of route `name` in folder `dir`, from its `meta.toml`
    /// text.
    pub fn from_meta(name: &str, dir: &Path, meta: &str) -> Result<Store, String> {
        let tables = parse_tables(meta)?;
        let top = &tables[0];
        let level = whole_u32(top.get("level").ok_or("no level key")?, "level")?;
        let route = dir.join(
            top.get("route")
                .map(|v| v.text().ok_or("route is not a text"))
                .transpose()?
                .unwrap_or("route.csv"),
        );
        let top_rng = top
            .get("rng_at_t0")
            .map(|v| whole_u32(v, "rng_at_t0"))
            .transpose()?;
        let mut runs = Vec::new();
        for clock in Clock::ALL {
            let table = tables
                .iter()
                .find(|t| !t.repeated && t.name == clock.name());
            let Some(table) = table else {
                continue;
            };
            let tuning = match table.get("tuning") {
                None => clock.default_tuning(),
                Some(v) => {
                    let name = v.text().ok_or("tuning is not a text")?;
                    ScriptTuning::from_name(name).ok_or_else(|| {
                        format!("[{}] names an unknown tuning {name:?}", clock.name())
                    })?
                }
            };
            let rng_at_t0 = match table.get("rng_at_t0") {
                Some(v) => Some(whole_u32(v, "rng_at_t0")?),
                None => top_rng,
            };
            let traces = table
                .get("traces")
                .ok_or_else(|| format!("[{}] lists no traces", clock.name()))?
                .texts()
                .ok_or_else(|| format!("[{}] traces is not a list of file names", clock.name()))?
                .into_iter()
                .map(|t| dir.join(t))
                .collect::<Vec<_>>();
            if traces.is_empty() {
                return Err(format!("[{}] lists no traces", clock.name()));
            }
            runs.push(ClockRun {
                clock,
                tuning,
                rng_at_t0,
                traces,
            });
        }
        if runs.is_empty() {
            return Err("no [c18] or [c60] table".to_string());
        }
        Ok(Store {
            name: name.to_string(),
            dir: dir.to_path_buf(),
            level,
            route,
            runs,
        })
    }

    /// The run of `clock`, if the store holds one.
    pub fn run(&self, clock: Clock) -> Option<&ClockRun> {
        self.runs.iter().find(|r| r.clock == clock)
    }
}

/// The game's configuration for replaying a route of `run`: the default
/// view, the run's tuning, the random state at time zero as the seed (the
/// level load draws nothing, so the first tick starts from it) and the
/// mid-air hit jump fix the route names (off without one, as in the
/// original).
pub fn replay_config(run: &ClockRun, route_text: &str) -> SimConfig {
    let mut cfg = SimConfig::for_view(ViewConfig::default());
    cfg.tuning = run.tuning.tuning();
    if let Some(seed) = run.rng_at_t0 {
        cfg.seed = seed;
    }
    cfg.tuning.behaviour.fix_double_jump = script_behaviour(route_text).unwrap_or(false);
    cfg
}

/// Replays `script` in `bundle` with `cfg`, one trace row per tick, and
/// keeps stepping with no button held until the trace covers `until_ms`.
/// A command directive takes effect before its tick; pauses and menus
/// change nothing in the game.
pub fn replay(
    bundle: &LevelBundle,
    cfg: SimConfig,
    script: &InputScript,
    until_ms: u64,
) -> Result<Vec<TraceRow>, String> {
    let seed = cfg.seed;
    let mut sim = Sim::new(bundle.sim_level.clone(), cfg);
    if sim.rng_state() != seed {
        return Err(format!(
            "loading the level drew from the random stream ({} after the load, seed {seed}), \
             so the first tick no longer starts from the seed",
            sim.rng_state()
        ));
    }
    let ticks = script
        .ticks
        .len()
        .max(until_ms.div_ceil(u64::from(TICK_MS)) as usize);
    let mut directives = script.directives().iter().peekable();
    let mut rows = Vec::with_capacity(ticks);
    for tick in 0..ticks {
        while let Some(d) = directives.next_if(|d| d.tick <= tick) {
            if let DirectiveKind::Command(ScriptCommand::EndLife) = d.kind {
                sim.end_life();
            }
        }
        sim.step(script.ticks.get(tick).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, tick as u64, "script"));
    }
    Ok(rows)
}

/// One reference trace compared.
#[derive(Debug, Clone)]
pub struct TraceResult {
    /// The reference trace.
    pub trace: PathBuf,
    /// The comparison.
    pub comparison: Comparison,
    /// The report: summary, per-column table and the rows leading up to the
    /// first divergence.
    pub report: String,
}

/// The comparisons of one clock.
#[derive(Debug, Clone)]
pub struct ClockResult {
    /// The run compared.
    pub run: ClockRun,
    /// The game's configuration seed.
    pub seed: u32,
    /// The rebuild trace.
    pub rebuild: Vec<TraceRow>,
    /// One result per reference trace.
    pub traces: Vec<TraceResult>,
}

impl Store {
    /// Replays the route with `run`'s tuning and seed in the level of
    /// `bundle` and compares it with each of the run's traces within `tol`.
    pub fn compare_clock(
        &self,
        bundle: &LevelBundle,
        run: &ClockRun,
        tol: &Tolerances,
    ) -> Result<ClockResult, String> {
        let route_text = std::fs::read_to_string(&self.route)
            .map_err(|e| format!("reading {}: {e}", self.route.display()))?;
        let script = InputScript::parse(&route_text)
            .map_err(|e| format!("{}: {e}", self.route.display()))?;
        if let Some(level) = script.header.level
            && level != self.level
        {
            return Err(format!(
                "{} names level {level}, the store level {}",
                self.route.display(),
                self.level
            ));
        }
        let mut references = Vec::new();
        for path in &run.traces {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?;
            let rows = trace::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            if rows.is_empty() {
                return Err(format!(
                    "{}: the reference trace has no rows",
                    path.display()
                ));
            }
            references.push((path.clone(), rows));
        }
        let until = references
            .iter()
            .filter_map(|(_, rows)| rows.last().map(|r| r.t_ms))
            .max()
            .unwrap_or(0);
        let cfg = replay_config(run, &route_text);
        let seed = cfg.seed;
        let rebuild = replay(bundle, cfg, &script, until)?;
        let traces = references
            .into_iter()
            .map(|(trace, reference)| {
                let comparison = trace::compare(&rebuild, &reference, tol);
                let mut report = trace::report(&rebuild, &reference, &comparison, CONTEXT_ROWS);
                if comparison.rows_compared == 0 {
                    report.push_str("nothing was compared: the traces do not overlap in time\n");
                }
                TraceResult {
                    trace,
                    comparison,
                    report,
                }
            })
            .collect();
        Ok(ClockResult {
            run: run.clone(),
            seed,
            rebuild,
            traces,
        })
    }
}

/// Loads level `level` of the install at `install`.
pub fn load(install: &Path, level: u32) -> Result<LevelBundle, String> {
    claw_game::load_level(install, &LevelChoice::Retail(level))
        .map_err(|e| format!("loading level {level}: {e:#}"))
}

/// Reads tolerance file `name` (`exact`, `feel` or another file in
/// [`tolerance_dir`], with or without `.toml`).
pub fn tolerances(name: &str) -> Result<Tolerances, String> {
    let file = if name.ends_with(".toml") {
        name.to_string()
    } else {
        format!("{name}.toml")
    };
    let path = tolerance_dir().join(file);
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    Tolerances::from_toml(&text).map_err(|e| format!("{}: {e}", path.display()))
}

// --------------------------------------------------------- known results

/// One `[[compare]]` entry of `references.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The route's store name, for example `level1/opening`.
    pub reference: String,
    /// The clock compared.
    pub clock: Clock,
    /// The tolerance file.
    pub tolerance: String,
    /// The columns known to leave the tolerance, with the reference time of
    /// their first divergence, ms (the `known.<column> = <ms>` keys).
    pub known: BTreeMap<String, u64>,
}

/// Reads `references.toml`: `[[compare]]` entries with the keys
/// `reference`, `clock`, `tolerance` and any number of
/// `known.<column> = <ms>`.
pub fn entries(text: &str) -> Result<Vec<Entry>, String> {
    let tables = parse_tables(text)?;
    if let Some((k, _)) = tables[0].keys.first() {
        return Err(format!("unexpected top-level key {k}"));
    }
    let mut out: Vec<Entry> = Vec::new();
    for t in &tables[1..] {
        if t.name != "compare" || !t.repeated {
            return Err(format!(
                "unexpected table {:?}, expected [[compare]]",
                t.name
            ));
        }
        let text_key = |k: &str| -> Result<String, String> {
            t.get(k)
                .and_then(Value::text)
                .map(str::to_string)
                .ok_or_else(|| format!("a [[compare]] entry needs {k} = \"...\""))
        };
        let reference = text_key("reference")?;
        let clock_name = text_key("clock")?;
        let clock = Clock::from_name(&clock_name).ok_or_else(|| {
            format!("{reference}: unknown clock {clock_name:?}, expected c18 or c60")
        })?;
        let tolerance = match t.get("tolerance") {
            None => clock.default_tolerance().to_string(),
            Some(v) => v
                .text()
                .ok_or_else(|| format!("{reference}: tolerance is not a text"))?
                .to_string(),
        };
        let mut known = BTreeMap::new();
        for (k, v) in &t.keys {
            match k.as_str() {
                "reference" | "clock" | "tolerance" => {}
                _ => {
                    let column = k
                        .strip_prefix("known.")
                        .ok_or_else(|| format!("{reference} {clock_name}: unknown key {k}"))?;
                    if Tolerances::exact().column(column).is_none() {
                        return Err(format!(
                            "{reference} {clock_name}: unknown column {column:?}"
                        ));
                    }
                    let ms = v.int().and_then(|n| u64::try_from(n).ok()).ok_or_else(|| {
                        format!("{reference} {clock_name}: {k} takes a time in ms")
                    })?;
                    known.insert(column.to_string(), ms);
                }
            }
        }
        if out
            .iter()
            .any(|e| e.reference == reference && e.clock == clock)
        {
            return Err(format!("{reference} {clock_name} is listed twice"));
        }
        out.push(Entry {
            reference,
            clock,
            tolerance,
            known,
        });
    }
    Ok(out)
}

/// Reads `references.toml` from [`tolerance_dir`].
pub fn load_entries() -> Result<Vec<Entry>, String> {
    let path = tolerance_dir().join("references.toml");
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    entries(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// How a comparison stands against an entry's known divergences.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Verdict {
    /// Divergences that are new or earlier than known: each a line.
    pub regressions: Vec<String>,
    /// Known divergences that came later or went away: each a line.
    pub improvements: Vec<String>,
}

impl Verdict {
    /// No regression.
    pub fn holds(&self) -> bool {
        self.regressions.is_empty()
    }
}

/// Checks `cmp` against `entry`: every column must stay in tolerance up to
/// its known first divergence (or throughout, when none is known),
/// something must have been compared, and the game's trace must last as
/// long as the reference trace.
pub fn verdict(entry: &Entry, cmp: &Comparison) -> Verdict {
    let mut v = Verdict::default();
    if cmp.rows_compared == 0 {
        v.regressions
            .push("nothing was compared: the traces do not overlap in time".to_string());
    }
    if cmp.rows_unmatched > 0 {
        v.regressions.push(format!(
            "the game's trace ends before the reference trace: {} reference row(s) unmatched",
            cmp.rows_unmatched
        ));
    }
    for s in &cmp.per_column {
        let known = entry.known.get(&s.column).copied();
        match (s.first_t_ms, known) {
            (Some(t), None) => v
                .regressions
                .push(format!("{} leaves the tolerance at {t} ms", s.column)),
            (Some(t), Some(k)) if t < k => v.regressions.push(format!(
                "{} leaves the tolerance at {t} ms, earlier than the known {k} ms",
                s.column
            )),
            (Some(t), Some(k)) if t > k => v.improvements.push(format!(
                "{} now holds until {t} ms (known: {k} ms)",
                s.column
            )),
            (None, Some(k)) => v.improvements.push(format!(
                "{} now stays in tolerance (known: {k} ms)",
                s.column
            )),
            _ => {}
        }
    }
    for column in entry.known.keys() {
        if !cmp.per_column.iter().any(|s| &s.column == column) {
            v.improvements
                .push(format!("{column} is not compared by {}", entry.tolerance));
        }
    }
    v
}

/// The earliest divergence of any column but the random stream: its
/// reference time and column.
pub fn first_outside_rng(cmp: &Comparison) -> Option<(u64, &str)> {
    cmp.per_column
        .iter()
        .filter(|c| c.column != "rng")
        .filter_map(|c| c.first_t_ms.map(|t| (t, c.column.as_str())))
        .min_by_key(|(t, _)| *t)
}

/// A one-line description of a clock result's first divergences, one
/// clause per trace.
pub fn summary_line(store: &Store, result: &ClockResult) -> String {
    let mut s = format!("{} {}:", store.name, result.run.clock.name());
    for t in &result.traces {
        let name = t
            .trace
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let _ = match &t.comparison.first_divergence {
            _ if t.comparison.passed() => write!(s, " {name} within tolerance;"),
            Some(d) => {
                let _ = write!(
                    s,
                    " {name} first leaves the tolerance at {} ms in {} (reference {}, game {})",
                    d.t_ms, d.column, d.reference, d.rebuild
                );
                if d.column == "rng"
                    && let Some((t, column)) = first_outside_rng(&t.comparison)
                {
                    let _ = write!(s, ", outside the random stream at {t} ms in {column}");
                }
                write!(s, ";")
            }
            None => write!(s, " {name} compared nothing;"),
        };
    }
    s.pop();
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use claw_sim::trace::ColumnSummary;

    const META: &str = r#"
# A store.
level = 1
name = "opening"
route = "route.csv"
clocks = ["c18", "c60"]
rng_at_t0 = 4105512405

[c18]
clock = "c18"
tuning = "original"
traces = ["trace-c18-r1.csv", "trace-c18-r2.csv"]
rows = [1804, 1804]

[c60]
tuning = "default"
traces = ["trace-c60-r1.csv"]
"#;

    #[test]
    fn stores_name_the_level_tuning_seed_and_traces() {
        let s = Store::from_meta("level1/opening", Path::new("/r/level1/opening"), META).unwrap();
        assert_eq!(s.level, 1);
        assert_eq!(s.route, Path::new("/r/level1/opening/route.csv"));
        let c18 = s.run(Clock::C18).unwrap();
        assert_eq!(c18.tuning, ScriptTuning::Original);
        assert_eq!(c18.rng_at_t0, Some(4_105_512_405));
        assert_eq!(c18.traces.len(), 2);
        let c60 = s.run(Clock::C60).unwrap();
        assert_eq!(c60.tuning, ScriptTuning::Default);
        assert_eq!(
            c60.traces,
            vec![PathBuf::from("/r/level1/opening/trace-c60-r1.csv")]
        );
        let cfg = replay_config(c18, "# mid_air_hit_fix = on\n");
        assert_eq!(cfg.seed, 4_105_512_405);
        assert!(cfg.tuning.behaviour.fix_double_jump);
        assert!(!replay_config(c60, "").tuning.behaviour.fix_double_jump);
    }

    #[test]
    fn bad_stores_are_errors() {
        let dir = Path::new("/r");
        assert!(
            Store::from_meta("x", dir, "level = 1\n").is_err(),
            "no clock"
        );
        assert!(Store::from_meta("x", dir, "[c18]\ntraces = [\"a.csv\"]\n").is_err());
        assert!(Store::from_meta("x", dir, "level = 1\n[c18]\ntraces = []\n").is_err());
        assert!(
            Store::from_meta(
                "x",
                dir,
                "level = 1\n[c18]\ntuning = \"fast\"\ntraces = [\"a\"]\n"
            )
            .is_err()
        );
        assert!(Store::from_meta("x", dir, "level = 1\nlevel = 2\n").is_err());
        assert!(Store::from_meta("x", dir, "level = -1\n[c18]\ntraces = [\"a\"]\n").is_err());
    }

    #[test]
    fn the_committed_tolerance_files_read() {
        assert_eq!(tolerances("exact").unwrap(), Tolerances::exact());
        assert_eq!(tolerances("feel.toml").unwrap(), Tolerances::feel());
        let list = load_entries().unwrap();
        assert!(!list.is_empty());
        for e in &list {
            tolerances(&e.tolerance).unwrap();
        }
    }

    #[test]
    fn entries_read_known_divergences() {
        let list = entries(
            "# comment\n[[compare]]\nreference = \"level1/opening\"\nclock = \"c18\"\n\
             tolerance = \"exact\"\nknown.rng = 18\nknown.state_id = 16416\n\n\
             [[compare]]\nreference = \"level1/opening\"\nclock = \"c60\"\n",
        )
        .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].known.get("rng"), Some(&18));
        assert_eq!(list[0].known.get("state_id"), Some(&16416));
        assert_eq!(list[1].tolerance, "feel", "the clock's default");
        assert!(entries("[[compare]]\nreference = \"a\"\nclock = \"c30\"\n").is_err());
        assert!(
            entries("[[compare]]\nreference = \"a\"\nclock = \"c18\"\nknown.nope = 1\n").is_err()
        );
        assert!(entries("[[compare]]\nreference = \"a\"\nclock = \"c18\"\nother = 1\n").is_err());
        assert!(entries("[[other]]\n").is_err());
        assert!(
            entries("[[compare]]\nreference = \"a\"\nclock = \"c18\"\n[[compare]]\nreference = \"a\"\nclock = \"c18\"\n")
                .is_err(),
            "listed twice"
        );
    }

    fn summary(column: &str, first: Option<u64>) -> ColumnSummary {
        ColumnSummary {
            column: column.to_string(),
            compared: 10,
            diverged: usize::from(first.is_some()),
            max_diff: None,
            first_t_ms: first,
        }
    }

    #[test]
    fn verdicts_fail_only_on_new_or_earlier_divergences() {
        let entry = Entry {
            reference: "level1/opening".into(),
            clock: Clock::C18,
            tolerance: "exact".into(),
            known: [("rng".to_string(), 18), ("x".to_string(), 1000)].into(),
        };
        let cmp = |cols: Vec<ColumnSummary>| Comparison {
            rows_compared: 10,
            compared_ms: 180,
            rows_unmatched: 0,
            first_divergence: None,
            per_column: cols,
        };
        let same = cmp(vec![
            summary("x", Some(1000)),
            summary("rng", Some(18)),
            summary("y", None),
        ]);
        assert!(verdict(&entry, &same).holds());
        assert!(verdict(&entry, &same).improvements.is_empty());
        let earlier = cmp(vec![summary("x", Some(990)), summary("rng", Some(18))]);
        assert!(!verdict(&entry, &earlier).holds());
        let new = cmp(vec![summary("y", Some(5000)), summary("rng", Some(18))]);
        assert!(!verdict(&entry, &new).holds());
        let better = cmp(vec![summary("x", Some(2000)), summary("rng", None)]);
        let v = verdict(&entry, &better);
        assert!(v.holds());
        assert_eq!(v.improvements.len(), 2);
        let mut nothing = cmp(vec![]);
        nothing.rows_compared = 0;
        assert!(!verdict(&entry, &nothing).holds());
        let mut short = cmp(vec![summary("x", Some(1000)), summary("rng", Some(18))]);
        short.rows_unmatched = 3;
        assert!(!verdict(&entry, &short).holds());
    }

    #[test]
    fn clocks_have_names_and_defaults() {
        assert_eq!(Clock::from_name("c60"), Some(Clock::C60));
        assert_eq!(Clock::from_name("c30"), None);
        assert_eq!(Clock::C18.default_tolerance(), "exact");
        assert_eq!(Clock::C60.default_tuning(), ScriptTuning::Default);
    }
}
