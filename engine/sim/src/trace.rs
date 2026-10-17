//! Per-tick traces: one CSV row per tick with Claw's position, state and
//! the camera, his stats, the game events of the tick and the state of the
//! shared random stream, for comparing runs and checking movement.
//!
//! The `weapon` column is `pistol`, `magic` or `dynamite`. The `events`
//! column lists the tick's game events separated by `|`, each as its name
//! followed by its values, separated by `:` (for example
//! `checkpoint:6200:3100|message:3:2000`); it is empty when nothing
//! happened. The `rng` column is the state of the shared random stream
//! after the tick, in decimal.
//!
//! A reference trace of the same route, in the same trace format, has one
//! row per logic step of the run it records: `tick` is the step index,
//! `t_ms` the game time since the route's time zero at the end of the
//! step, `dt_ms` the step's length (which may vary) and `source` is
//! `reference`. [`parse`] reads both kinds by column name, and [`compare`]
//! lines a rebuild trace up against a reference trace within
//! [`Tolerances`] and reports the first divergence.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use claw_frame::{GameEvent, Weapon};

use crate::activation::ActivationMode;
use crate::sim::Sim;
use crate::tuning::TICK_MS;

/// Column names of a trace. Columns are only ever appended, so older traces
/// are a prefix of this list.
pub const HEADER: &str = "tick,t_ms,dt_ms,x,y,frac_x,frac_y,state_id,physics_type,cam_x,cam_y,held,pressed,source,activation,health,score,lives,weapon,pistol,magic,dynamite,events,rng";

/// One trace row.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceRow {
    /// Index of the tick (0 for the first).
    pub tick: u64,
    /// Simulation time at the end of the tick, ms.
    pub t_ms: u64,
    /// Tick length, ms.
    pub dt_ms: u32,
    /// Claw's position.
    pub x: i32,
    /// Claw's position.
    pub y: i32,
    /// Horizontal fraction carried to the next tick (NaN when a parsed
    /// trace leaves the cell empty).
    pub frac_x: f64,
    /// Vertical fraction carried to the next tick (NaN when a parsed trace
    /// leaves the cell empty).
    pub frac_y: f64,
    /// Claw's state id.
    pub state_id: i32,
    /// Claw's physics type.
    pub physics_type: i32,
    /// Camera point.
    pub cam_x: i32,
    /// Camera point.
    pub cam_y: i32,
    /// Buttons held this tick.
    pub held: u32,
    /// Buttons pressed this tick.
    pub pressed: u32,
    /// Where the input came from.
    pub source: String,
    /// Activation mode name (empty when a parsed trace leaves it empty).
    pub activation: &'static str,
    /// Claw's health.
    pub health: i32,
    /// Score.
    pub score: u32,
    /// Lives left.
    pub lives: i32,
    /// Selected projectile weapon.
    pub weapon: &'static str,
    /// Pistol shots.
    pub pistol: i32,
    /// Magic charges.
    pub magic: i32,
    /// Sticks of dynamite.
    pub dynamite: i32,
    /// The tick's game events, encoded as described in the module
    /// documentation.
    pub events: String,
    /// State of the shared random stream after the tick; `None` when the
    /// trace does not record it.
    pub rng: Option<u32>,
}

/// Name of a weapon in traces.
pub fn weapon_name(w: Weapon) -> &'static str {
    match w {
        Weapon::Pistol => "pistol",
        Weapon::Magic => "magic",
        Weapon::Dynamite => "dynamite",
    }
}

/// One game event as written in traces.
pub fn event_text(e: &GameEvent) -> String {
    match *e {
        GameEvent::LevelComplete { after_ms } => format!("level_complete:{after_ms}"),
        GameEvent::ClawDied { lives_left } => format!("claw_died:{lives_left}"),
        GameEvent::GameOver => "game_over".to_string(),
        GameEvent::Checkpoint { x, y } => format!("checkpoint:{x}:{y}"),
        GameEvent::SavePoint { slot } => format!("save_point:{slot}"),
        GameEvent::Message { id, ms } => format!("message:{id}:{ms}"),
    }
}

impl TraceRow {
    /// The row for the tick `sim` has just run, numbered `tick` (0 for the
    /// first tick of the recording).
    pub fn capture(sim: &Sim, tick: u64, source: &str) -> TraceRow {
        let (x, y, state_id, physics_type) = sim
            .player()
            .map_or((0, 0, -1, 0), |o| (o.x, o.y, o.state, o.physics_type));
        let mover = &sim.claw().mover;
        let (cam_x, cam_y) = sim.camera().current;
        let input = sim.input();
        let stats = sim.stats();
        let events: Vec<String> = sim.last_events().iter().map(event_text).collect();
        TraceRow {
            tick,
            t_ms: (tick + 1) * u64::from(TICK_MS),
            dt_ms: TICK_MS,
            x,
            y,
            frac_x: mover.x.carry,
            frac_y: mover.y.carry,
            state_id,
            physics_type,
            cam_x,
            cam_y,
            held: input.held.0,
            pressed: input.pressed.0,
            source: source.to_string(),
            activation: sim.config().activation.mode.name(),
            health: stats.health,
            score: stats.score,
            lives: stats.lives,
            weapon: weapon_name(stats.weapon),
            pistol: stats.pistol,
            magic: stats.magic,
            dynamite: stats.dynamite,
            events: events.join("|"),
            rng: Some(sim.rng_state()),
        }
    }

    /// The row as CSV (no line end). A NaN fraction and a missing `rng`
    /// are written as empty cells.
    pub fn to_csv(&self) -> String {
        let frac = |f: f64| {
            if f.is_nan() {
                String::new()
            } else {
                format!("{f:.6}")
            }
        };
        let mut s = String::new();
        let _ = write!(
            s,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.tick,
            self.t_ms,
            self.dt_ms,
            self.x,
            self.y,
            frac(self.frac_x),
            frac(self.frac_y),
            self.state_id,
            self.physics_type,
            self.cam_x,
            self.cam_y,
            self.held,
            self.pressed,
            self.source,
            self.activation,
            self.health,
            self.score,
            self.lives,
            self.weapon,
            self.pistol,
            self.magic,
            self.dynamite,
            self.events,
            self.rng.map(|r| r.to_string()).unwrap_or_default()
        );
        s
    }
}

/// A whole trace as CSV text with a header line.
pub fn to_csv(rows: &[TraceRow]) -> String {
    let mut out = String::with_capacity(64 * (rows.len() + 1));
    out.push_str(HEADER);
    out.push('\n');
    for r in rows {
        out.push_str(&r.to_csv());
        out.push('\n');
    }
    out
}

// ------------------------------------------------------------------ parsing

fn activation_name(s: &str) -> Option<&'static str> {
    if s.is_empty() {
        return Some("");
    }
    [
        ActivationMode::Original,
        ActivationMode::FrozenEdge,
        ActivationMode::Widened,
    ]
    .into_iter()
    .map(ActivationMode::name)
    .find(|n| *n == s)
}

fn weapon_from_name(s: &str) -> Option<&'static str> {
    match s {
        "" => Some(""),
        _ => [Weapon::Pistol, Weapon::Magic, Weapon::Dynamite]
            .into_iter()
            .map(weapon_name)
            .find(|n| *n == s),
    }
}

fn parse_number<T: std::str::FromStr>(s: &str) -> Option<T> {
    s.parse().ok()
}

fn parse_mask_cell(s: &str) -> Option<u32> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

/// Parses trace CSV text. Columns are found by the names on the header
/// line, so their order does not matter and unknown columns are skipped.
/// `tick` and `t_ms` are required. Columns missing from the header (an
/// older trace without `rng`, for example) or cells missing at the end of
/// a row take defaults: `dt_ms` the time since the previous row, `rng`
/// none, fractions NaN, numbers 0 and text empty. The cells of `frac_x`,
/// `frac_y`, `activation` and `rng` may be empty. Lines starting with `#`
/// are comments.
pub fn parse(text: &str) -> Result<Vec<TraceRow>, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
    let Some((_, header)) = lines.next() else {
        return Ok(Vec::new());
    };
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, name) in header.split(',').map(str::trim).enumerate() {
        if index.insert(name, i).is_some() {
            return Err(format!("line 1: column {name:?} appears twice"));
        }
    }
    for required in ["tick", "t_ms"] {
        if !index.contains_key(required) {
            return Err(format!("the header has no {required} column"));
        }
    }
    let columns = index.len();
    let mut rows = Vec::new();
    let mut prev_t = 0u64;
    for (line, text) in lines {
        let cells: Vec<&str> = text.split(',').map(str::trim).collect();
        if cells.len() > columns {
            return Err(format!(
                "line {line}: {} cells, the header has {columns} columns",
                cells.len()
            ));
        }
        let cell = |name: &str| index.get(name).and_then(|&i| cells.get(i).copied());
        let err = |name: &str, v: &str| format!("line {line}: bad {name} {v:?}");
        macro_rules! num {
            ($name:literal, $t:ty) => {{
                match cell($name) {
                    None => None,
                    Some(v) => Some(parse_number::<$t>(v).ok_or_else(|| err($name, v))?),
                }
            }};
        }
        let tick = num!("tick", u64).ok_or_else(|| format!("line {line}: no tick"))?;
        let t_ms = num!("t_ms", u64).ok_or_else(|| format!("line {line}: no t_ms"))?;
        let dt_ms = match num!("dt_ms", u32) {
            Some(dt) => dt,
            None => u32::try_from(t_ms.saturating_sub(prev_t)).unwrap_or(u32::MAX),
        };
        prev_t = t_ms;
        // Fractions, like `activation` and `rng`, may be left empty.
        let frac = |name: &str| -> Result<f64, String> {
            match cell(name) {
                None | Some("") => Ok(f64::NAN),
                Some(v) => v.parse().map_err(|_| err(name, v)),
            }
        };
        let mask = |name: &str| -> Result<u32, String> {
            match cell(name) {
                None => Ok(0),
                Some(v) => parse_mask_cell(v).ok_or_else(|| err(name, v)),
            }
        };
        let activation = cell("activation").unwrap_or("");
        let activation =
            activation_name(activation).ok_or_else(|| err("activation", activation))?;
        let weapon = cell("weapon").unwrap_or("");
        let weapon = weapon_from_name(weapon).ok_or_else(|| err("weapon", weapon))?;
        let rng = match cell("rng") {
            None | Some("") => None,
            Some(v) => Some(parse_number::<u32>(v).ok_or_else(|| err("rng", v))?),
        };
        rows.push(TraceRow {
            tick,
            t_ms,
            dt_ms,
            x: num!("x", i32).unwrap_or(0),
            y: num!("y", i32).unwrap_or(0),
            frac_x: frac("frac_x")?,
            frac_y: frac("frac_y")?,
            state_id: num!("state_id", i32).unwrap_or(0),
            physics_type: num!("physics_type", i32).unwrap_or(0),
            cam_x: num!("cam_x", i32).unwrap_or(0),
            cam_y: num!("cam_y", i32).unwrap_or(0),
            held: mask("held")?,
            pressed: mask("pressed")?,
            source: cell("source").unwrap_or("").to_string(),
            activation,
            health: num!("health", i32).unwrap_or(0),
            score: num!("score", u32).unwrap_or(0),
            lives: num!("lives", i32).unwrap_or(0),
            weapon,
            pistol: num!("pistol", i32).unwrap_or(0),
            magic: num!("magic", i32).unwrap_or(0),
            dynamite: num!("dynamite", i32).unwrap_or(0),
            events: cell("events").unwrap_or("").to_string(),
            rng,
        });
    }
    Ok(rows)
}

// --------------------------------------------------------------- tolerances

/// How a column is compared by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    /// Positions: within [`Tolerances::pos_px`].
    Position,
    /// State and input: equal within [`Tolerances::state_slack_ms`].
    State,
    /// Stats: equal within [`Tolerances::stats_slack_ms`].
    Stats,
    /// Game events: matched within [`Tolerances::events_slack_ms`].
    Events,
    /// The random stream: equal when [`Tolerances::rng`] is on.
    Rng,
    /// Not compared unless a tolerance file turns it on.
    Off,
}

/// A comparable column.
#[derive(Debug, Clone, Copy)]
struct Column {
    name: &'static str,
    group: Group,
    text: bool,
}

const fn col(name: &'static str, group: Group, text: bool) -> Column {
    Column { name, group, text }
}

/// The columns [`compare`] knows, in header order. `tick`, `t_ms`, `dt_ms`
/// and `source` describe the rows and are never compared.
const COLUMNS: [Column; 20] = [
    col("x", Group::Position, false),
    col("y", Group::Position, false),
    col("frac_x", Group::Off, false),
    col("frac_y", Group::Off, false),
    col("state_id", Group::State, false),
    col("physics_type", Group::State, false),
    col("cam_x", Group::Position, false),
    col("cam_y", Group::Position, false),
    col("held", Group::State, false),
    col("pressed", Group::Off, false),
    col("activation", Group::Off, true),
    col("health", Group::Stats, false),
    col("score", Group::Stats, false),
    col("lives", Group::Stats, false),
    col("weapon", Group::Stats, true),
    col("pistol", Group::Stats, false),
    col("magic", Group::Stats, false),
    col("dynamite", Group::Stats, false),
    col("events", Group::Events, true),
    col("rng", Group::Rng, false),
];

/// Event kinds both traces record and [`compare`] matches.
pub const COMPARED_EVENTS: [&str; 4] = ["checkpoint", "level_complete", "claw_died", "game_over"];

/// Event kinds [`compare`] matches by name alone: a reference trace writes
/// the level end as plain `level_complete`, without the time value the
/// rebuild adds.
pub const NAME_ONLY_EVENTS: [&str; 1] = ["level_complete"];

/// The name of an encoded event (the text before its first `:`).
fn event_kind(e: &str) -> &str {
    e.split(':').next().unwrap_or_default()
}

/// Whether two encoded events are the same event: equal text, or the same
/// kind for the kinds in [`NAME_ONLY_EVENTS`].
fn same_event(a: &str, b: &str) -> bool {
    a == b || (event_kind(a) == event_kind(b) && NAME_ONLY_EVENTS.contains(&event_kind(a)))
}

/// The resolved tolerance of one column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnTolerance {
    /// Whether the column is compared at all.
    pub compared: bool,
    /// Largest difference accepted (px for positions; 0 means equal).
    pub max_diff: f64,
    /// How far, in ms either way, the rebuild may be from the reference
    /// time and still show the reference value.
    pub slack_ms: u32,
}

/// A per-column change to the tolerances; `None` keeps the default.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColumnOverride {
    /// Turns the column's comparison on or off.
    pub compared: Option<bool>,
    /// Largest difference accepted.
    pub max_diff: Option<f64>,
    /// Slack, ms.
    pub slack_ms: Option<u32>,
}

/// How closely a rebuild trace must follow a reference trace.
///
/// Positions (`x`, `y`, `cam_x`, `cam_y`) may differ by up to `pos_px`.
/// State and input (`state_id`, `physics_type`, `held`), stats (`health`,
/// `score`, `lives`, `weapon` and the ammunition) and game events must be
/// equal, but may change up to their slack earlier or later than in the
/// reference. The random stream state is compared only with `rng` on.
/// `overrides` change single columns, and can turn on columns that are off
/// by default (`frac_x`, `frac_y`, `pressed`, `activation`).
#[derive(Debug, Clone, PartialEq)]
pub struct Tolerances {
    /// Largest position difference, px.
    pub pos_px: u32,
    /// Position slack, ms.
    pub pos_slack_ms: u32,
    /// State and input slack, ms.
    pub state_slack_ms: u32,
    /// Stats slack, ms.
    pub stats_slack_ms: u32,
    /// Game event slack, ms.
    pub events_slack_ms: u32,
    /// Compare the random stream state.
    pub rng: bool,
    /// Per-column changes, by column name.
    pub overrides: BTreeMap<String, ColumnOverride>,
}

impl Default for Tolerances {
    fn default() -> Self {
        Tolerances::exact()
    }
}

impl Tolerances {
    /// Tick for tick: for a run of the original at one 18 ms logic step per
    /// frame against the rebuild with the original speeds and the same
    /// random seed. Every compared column is equal at the same time,
    /// including the random stream.
    pub fn exact() -> Tolerances {
        Tolerances {
            pos_px: 0,
            pos_slack_ms: 0,
            state_slack_ms: 0,
            stats_slack_ms: 0,
            events_slack_ms: 0,
            rng: true,
            overrides: BTreeMap::new(),
        }
    }

    /// Feel: for a run of the original at 60 frames per second against the
    /// default tuning. Positions within 8 px, allowing one tick of timing
    /// either way; states and stats within two ticks; game events within
    /// three ticks; the random stream is not compared, since the two runs
    /// step it a different number of times.
    ///
    /// The position, state and stats values are the smallest that hold
    /// Claw's own movement on the first level's opening route (standing,
    /// tapped and full jumps, walking off a step, walking, running jumps
    /// and treasure pickups) at 60 frames per second: his position there
    /// stays within 8 px of the original after one tick of timing, and his
    /// state and stats change within two ticks of it. Climbing and fights
    /// with enemies are outside these values. The event slack is still a
    /// provisional starting value: that route raises no game event. The
    /// tolerance files in the acceptance tests refine these per route.
    pub fn feel() -> Tolerances {
        Tolerances {
            pos_px: 8,
            pos_slack_ms: TICK_MS,
            state_slack_ms: 2 * TICK_MS,
            stats_slack_ms: 2 * TICK_MS,
            events_slack_ms: 3 * TICK_MS,
            rng: false,
            overrides: BTreeMap::new(),
        }
    }

    /// A preset by name: `exact` or `feel`.
    pub fn preset(name: &str) -> Option<Tolerances> {
        match name {
            "exact" => Some(Tolerances::exact()),
            "feel" => Some(Tolerances::feel()),
            _ => None,
        }
    }

    /// The tolerance of column `name` with the overrides applied; `None`
    /// for a column [`compare`] does not know.
    pub fn column(&self, name: &str) -> Option<ColumnTolerance> {
        let c = COLUMNS.iter().find(|c| c.name == name)?;
        let base = match c.group {
            Group::Position => ColumnTolerance {
                compared: true,
                max_diff: f64::from(self.pos_px),
                slack_ms: self.pos_slack_ms,
            },
            Group::State => ColumnTolerance {
                compared: true,
                max_diff: 0.0,
                slack_ms: self.state_slack_ms,
            },
            Group::Stats => ColumnTolerance {
                compared: true,
                max_diff: 0.0,
                slack_ms: self.stats_slack_ms,
            },
            Group::Events => ColumnTolerance {
                compared: true,
                max_diff: 0.0,
                slack_ms: self.events_slack_ms,
            },
            Group::Rng => ColumnTolerance {
                compared: self.rng,
                max_diff: 0.0,
                slack_ms: self.state_slack_ms,
            },
            Group::Off => ColumnTolerance {
                compared: false,
                max_diff: 0.0,
                slack_ms: self.state_slack_ms,
            },
        };
        let Some(o) = self.overrides.get(name) else {
            return Some(base);
        };
        Some(ColumnTolerance {
            compared: o.compared.unwrap_or(true),
            max_diff: o.max_diff.unwrap_or(base.max_diff),
            slack_ms: o.slack_ms.unwrap_or(base.slack_ms),
        })
    }

    /// Reads a tolerance file, a small flat subset of TOML:
    ///
    /// ```toml
    /// # Start from a preset (exact when absent); must come first.
    /// base = "feel"
    /// pos_px = 6            # also pos_slack_ms, state_slack_ms,
    /// stats_slack_ms = 54   # events_slack_ms and rng = true|false
    ///
    /// [cam_y]               # one section per column
    /// max_px = 12           # largest difference
    /// slack_ms = 36         # slack either way
    ///
    /// [score]
    /// exact = true          # equal at the same time (no slack)
    ///
    /// [pressed]
    /// compare = true        # turn a column on (or off with false)
    /// ```
    ///
    /// A section turns its column on unless it says `compare = false`.
    /// Unknown keys, sections and duplicates are errors.
    pub fn from_toml(text: &str) -> Result<Tolerances, String> {
        let mut tol = Tolerances::exact();
        let mut section: Option<String> = None;
        let mut seen: Vec<(Option<String>, String)> = Vec::new();
        let mut exact_sections: Vec<Option<String>> = Vec::new();
        let mut any_top_key = false;
        for (i, raw) in text.lines().enumerate() {
            let line = strip_toml_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            let err = |m: String| format!("line {}: {m}", i + 1);
            if let Some(name) = line.strip_prefix('[') {
                let name = name
                    .strip_suffix(']')
                    .ok_or_else(|| err(format!("bad section {line:?}")))?
                    .trim();
                let c = COLUMNS
                    .iter()
                    .find(|c| c.name == name)
                    .ok_or_else(|| err(format!("unknown column {name:?}")))?;
                if tol.overrides.contains_key(name) {
                    return Err(err(format!("section [{name}] appears twice")));
                }
                tol.overrides
                    .insert(c.name.to_string(), ColumnOverride::default());
                section = Some(c.name.to_string());
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .map(|(k, v)| (k.trim(), v.trim()))
                .ok_or_else(|| err(format!("expected key = value, found {line:?}")))?;
            if seen.contains(&(section.clone(), key.to_string())) {
                return Err(err(format!("key {key} appears twice")));
            }
            let value =
                TomlValue::parse(value).ok_or_else(|| err(format!("bad value {value:?}")))?;
            // `exact = true` and a tolerance in one section contradict.
            let in_section = |k: &str| seen.contains(&(section.clone(), k.to_string()));
            let contradiction = match key {
                "exact" => {
                    value.boolean() == Some(true)
                        && (in_section("max_px") || in_section("slack_ms"))
                }
                "max_px" | "slack_ms" => exact_sections.contains(&section),
                _ => false,
            };
            if contradiction {
                return Err(err(
                    "exact = true and a tolerance in one section".to_string()
                ));
            }
            if key == "exact" && value.boolean() == Some(true) {
                exact_sections.push(section.clone());
            }
            seen.push((section.clone(), key.to_string()));
            match &section {
                None => {
                    if key == "base" {
                        if any_top_key {
                            return Err(err("base must be the first key".to_string()));
                        }
                        let name = value
                            .string()
                            .ok_or_else(|| err("base takes a string".into()))?;
                        tol = Tolerances::preset(name).ok_or_else(|| {
                            err(format!("unknown base {name:?}, expected exact or feel"))
                        })?;
                        any_top_key = true;
                        continue;
                    }
                    any_top_key = true;
                    let ms = || {
                        value
                            .whole()
                            .ok_or_else(|| err(format!("{key} takes a whole number")))
                    };
                    match key {
                        "pos_px" => tol.pos_px = ms()?,
                        "pos_slack_ms" => tol.pos_slack_ms = ms()?,
                        "state_slack_ms" => tol.state_slack_ms = ms()?,
                        "stats_slack_ms" => tol.stats_slack_ms = ms()?,
                        "events_slack_ms" => tol.events_slack_ms = ms()?,
                        "rng" => {
                            tol.rng = value
                                .boolean()
                                .ok_or_else(|| err("rng takes true or false".into()))?;
                        }
                        _ => return Err(err(format!("unknown key {key:?}"))),
                    }
                }
                Some(name) => {
                    let c = COLUMNS.iter().find(|c| c.name == name).copied();
                    let o = tol.overrides.get_mut(name).expect("section was inserted");
                    match key {
                        "max_px" => {
                            if c.is_some_and(|c| c.text) {
                                return Err(err(format!("[{name}] is text and takes no max_px")));
                            }
                            let v = value
                                .number()
                                .filter(|v| *v >= 0.0)
                                .ok_or_else(|| err("max_px takes a number of at least 0".into()))?;
                            o.max_diff = Some(v);
                        }
                        "slack_ms" => {
                            o.slack_ms = Some(
                                value
                                    .whole()
                                    .ok_or_else(|| err("slack_ms takes a whole number".into()))?,
                            );
                        }
                        "exact" => {
                            if value
                                .boolean()
                                .ok_or_else(|| err("exact takes true or false".into()))?
                            {
                                o.max_diff = Some(0.0);
                                o.slack_ms = Some(0);
                                o.compared = Some(true);
                            }
                        }
                        "compare" => {
                            o.compared = Some(
                                value
                                    .boolean()
                                    .ok_or_else(|| err("compare takes true or false".into()))?,
                            );
                        }
                        _ => return Err(err(format!("unknown key {key:?} in [{name}]"))),
                    }
                }
            }
        }
        Ok(tol)
    }
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_string = false;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '#' if !in_string => return &line[..i],
            _ => {}
        }
    }
    line
}

/// A value in a tolerance file.
enum TomlValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

impl TomlValue {
    fn parse(s: &str) -> Option<TomlValue> {
        match s {
            "true" => Some(TomlValue::Bool(true)),
            "false" => Some(TomlValue::Bool(false)),
            _ => {
                if let Some(inner) = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
                    return (!inner.contains('"')).then(|| TomlValue::Text(inner.to_string()));
                }
                s.replace('_', "")
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .map(TomlValue::Number)
            }
        }
    }

    fn boolean(&self) -> Option<bool> {
        match self {
            TomlValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn number(&self) -> Option<f64> {
        match self {
            TomlValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    fn whole(&self) -> Option<u32> {
        self.number()
            .filter(|n| n.fract() == 0.0 && *n >= 0.0 && *n <= f64::from(u32::MAX))
            .map(|n| n as u32)
    }

    fn string(&self) -> Option<&str> {
        match self {
            TomlValue::Text(s) => Some(s),
            _ => None,
        }
    }
}

// --------------------------------------------------------------- comparing

/// The first place a rebuild trace leaves the tolerances.
#[derive(Debug, Clone, PartialEq)]
pub struct Divergence {
    /// Reference time, ms since the route's time zero (for an event only
    /// the rebuild has, the rebuild row's time).
    pub t_ms: u64,
    /// Column name.
    pub column: String,
    /// The reference value.
    pub reference: String,
    /// The rebuild value at that time.
    pub rebuild: String,
    /// Index of the reference row.
    pub reference_row: usize,
    /// Index of the rebuild row.
    pub rebuild_row: usize,
}

/// How one column fared.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSummary {
    /// Column name.
    pub column: String,
    /// Values (or events) compared.
    pub compared: usize,
    /// Of those, how many were outside the tolerance.
    pub diverged: usize,
    /// Largest difference seen after the slack (numeric columns).
    pub max_diff: Option<f64>,
    /// Time of the first value outside the tolerance.
    pub first_t_ms: Option<u64>,
}

/// The result of [`compare`].
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// Reference rows that had a rebuild row to compare with.
    pub rows_compared: usize,
    /// Reference time of the last compared row, ms.
    pub compared_ms: u64,
    /// Reference rows later than the end of the rebuild trace.
    pub rows_unmatched: usize,
    /// The earliest divergence (ties go to the column earlier in the
    /// header).
    pub first_divergence: Option<Divergence>,
    /// One summary per compared column, in header order.
    pub per_column: Vec<ColumnSummary>,
}

impl Comparison {
    /// No divergence, and at least one reference row was compared: two
    /// traces that do not overlap in time never pass.
    pub fn passed(&self) -> bool {
        self.first_divergence.is_none() && self.rows_compared > 0
    }
}

/// A cell as a comparable value.
enum Value<'a> {
    Num(f64),
    Text(&'a str),
    Missing,
}

impl Value<'_> {
    fn diff(&self, other: &Value<'_>) -> Option<f64> {
        match (self, other) {
            (Value::Num(a), Value::Num(b)) => Some((a - b).abs()),
            (Value::Text(a), Value::Text(b)) => Some(if a == b { 0.0 } else { f64::INFINITY }),
            _ => None,
        }
    }

    fn show(&self) -> String {
        match self {
            Value::Num(n) if n.fract() == 0.0 => format!("{}", *n as i64),
            Value::Num(n) => format!("{n:.6}"),
            Value::Text(s) => s.to_string(),
            Value::Missing => String::new(),
        }
    }
}

fn value<'a>(r: &'a TraceRow, name: &str) -> Value<'a> {
    let n = |v: f64| Value::Num(v);
    match name {
        "x" => n(f64::from(r.x)),
        "y" => n(f64::from(r.y)),
        "frac_x" if r.frac_x.is_nan() => Value::Missing,
        "frac_x" => n(r.frac_x),
        "frac_y" if r.frac_y.is_nan() => Value::Missing,
        "frac_y" => n(r.frac_y),
        "state_id" => n(f64::from(r.state_id)),
        "physics_type" => n(f64::from(r.physics_type)),
        "cam_x" => n(f64::from(r.cam_x)),
        "cam_y" => n(f64::from(r.cam_y)),
        "held" => n(f64::from(r.held)),
        "pressed" => n(f64::from(r.pressed)),
        "activation" if r.activation.is_empty() => Value::Missing,
        "activation" => Value::Text(r.activation),
        "health" => n(f64::from(r.health)),
        "score" => n(f64::from(r.score)),
        "lives" => n(f64::from(r.lives)),
        "weapon" if r.weapon.is_empty() => Value::Missing,
        "weapon" => Value::Text(r.weapon),
        "pistol" => n(f64::from(r.pistol)),
        "magic" => n(f64::from(r.magic)),
        "dynamite" => n(f64::from(r.dynamite)),
        "rng" => r.rng.map_or(Value::Missing, |v| n(f64::from(v))),
        _ => Value::Missing,
    }
}

/// The compared game events of a row.
fn compared_events(r: &TraceRow) -> Vec<&str> {
    r.events
        .split('|')
        .filter(|e| COMPARED_EVENTS.contains(&event_kind(e)))
        .collect()
}

/// Rows of `rows` whose span `(t_ms - dt_ms, t_ms]` meets the closed
/// window `[lo, hi]` (times in ms, possibly negative).
fn rows_meeting(rows: &[TraceRow], lo: i64, hi: i64) -> impl Iterator<Item = usize> + '_ {
    let start = rows.partition_point(|r| (r.t_ms as i64) < lo);
    (start..rows.len()).take_while(move |&k| (rows[k].t_ms as i64 - i64::from(rows[k].dt_ms)) < hi)
}

/// The index of the row whose span `(t_ms - dt_ms, t_ms]` contains `t`:
/// the first row ending at or after `t`.
fn row_at(rows: &[TraceRow], t: u64) -> Option<usize> {
    let k = rows.partition_point(|r| r.t_ms < t);
    (k < rows.len()).then_some(k)
}

/// Compares a rebuild trace with a reference trace of the same route.
///
/// Rows are lined up by time: for each reference row, the rebuild value is
/// taken from the rebuild tick whose span `(t_ms - dt_ms, t_ms]` contains
/// the reference row's `t_ms`, so 16 and 17 ms reference steps line up with
/// 18 ms ticks. A value outside the column's tolerance there still passes
/// when the rebuild shows an accepted value at some time within the
/// column's slack either way (a step function shifted by at most the
/// slack). Game events of the kinds in [`COMPARED_EVENTS`] must appear in
/// the other trace within the event slack, in both directions (the kinds in
/// [`NAME_ONLY_EVENTS`] by name alone). Reference rows past the end of the
/// rebuild trace are counted, not compared, except that a game event in
/// them is a divergence: the rebuild trace ended before reaching it (a
/// level end the rebuild never shows, for example).
pub fn compare(rebuild: &[TraceRow], reference: &[TraceRow], tol: &Tolerances) -> Comparison {
    let columns: Vec<(usize, Column, ColumnTolerance)> = COLUMNS
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let t = tol.column(c.name)?;
            t.compared.then_some((i, *c, t))
        })
        .collect();
    let mut per_column: Vec<ColumnSummary> = columns
        .iter()
        .map(|(_, c, _)| ColumnSummary {
            column: c.name.to_string(),
            compared: 0,
            diverged: 0,
            max_diff: None,
            first_t_ms: None,
        })
        .collect();
    let mut firsts: Vec<Option<Divergence>> = vec![None; columns.len()];
    let mut rows_compared = 0;
    let mut rows_unmatched = 0;
    let mut compared_ms = 0;
    // Keeps the earliest divergence per column (rebuild-only events are
    // found after the reference pass, so they may come earlier).
    let mut note = |slot: usize, summary: &mut ColumnSummary, d: Divergence| {
        summary.diverged += 1;
        summary.first_t_ms = Some(summary.first_t_ms.map_or(d.t_ms, |t| t.min(d.t_ms)));
        if firsts[slot].as_ref().is_none_or(|f| d.t_ms < f.t_ms) {
            firsts[slot] = Some(d);
        }
    };

    let events_slot = columns
        .iter()
        .position(|(_, c, _)| c.group == Group::Events);
    for (ri, r) in reference.iter().enumerate() {
        let Some(k0) = row_at(rebuild, r.t_ms) else {
            rows_unmatched += 1;
            if let Some(slot) = events_slot {
                for e in compared_events(r) {
                    per_column[slot].compared += 1;
                    let d = Divergence {
                        t_ms: r.t_ms,
                        column: "events".to_string(),
                        reference: e.to_string(),
                        rebuild: "the trace has ended".to_string(),
                        reference_row: ri,
                        rebuild_row: rebuild.len().saturating_sub(1),
                    };
                    note(slot, &mut per_column[slot], d);
                }
            }
            continue;
        };
        rows_compared += 1;
        compared_ms = r.t_ms;
        let t = r.t_ms as i64;
        for (slot, (_, c, ct)) in columns.iter().enumerate() {
            let summary = &mut per_column[slot];
            if c.group == Group::Events {
                let slack = i64::from(ct.slack_ms);
                let start = t - i64::from(r.dt_ms);
                for e in compared_events(r) {
                    summary.compared += 1;
                    let found = rows_meeting(rebuild, start - slack + 1, t + slack).any(|k| {
                        compared_events(&rebuild[k])
                            .iter()
                            .any(|b| same_event(b, e))
                    });
                    if !found {
                        let d = Divergence {
                            t_ms: r.t_ms,
                            column: c.name.to_string(),
                            reference: e.to_string(),
                            rebuild: events_or_none(&rebuild[k0]),
                            reference_row: ri,
                            rebuild_row: k0,
                        };
                        note(slot, summary, d);
                    }
                }
                continue;
            }
            let rv = value(r, c.name);
            if matches!(rv, Value::Missing) {
                continue;
            }
            let at = value(&rebuild[k0], c.name);
            let mut best = rv.diff(&at);
            if best.is_none_or(|d| d > ct.max_diff) && ct.slack_ms > 0 {
                let slack = i64::from(ct.slack_ms);
                for k in rows_meeting(rebuild, t - slack, t + slack) {
                    if let Some(d) = rv.diff(&value(&rebuild[k], c.name)) {
                        best = Some(best.map_or(d, |b: f64| b.min(d)));
                    }
                }
            }
            let Some(best) = best else {
                continue;
            };
            summary.compared += 1;
            if !c.text {
                summary.max_diff = Some(summary.max_diff.map_or(best, |m| m.max(best)));
            }
            if best > ct.max_diff {
                let d = Divergence {
                    t_ms: r.t_ms,
                    column: c.name.to_string(),
                    reference: rv.show(),
                    rebuild: at.show(),
                    reference_row: ri,
                    rebuild_row: k0,
                };
                note(slot, summary, d);
            }
        }
    }

    // Events only the rebuild has, within the time the reference covers.
    if let (Some(slot), Some(last)) = (events_slot, reference.last()) {
        let slack = i64::from(columns[slot].2.slack_ms);
        for (k, b) in rebuild.iter().enumerate() {
            if b.t_ms > last.t_ms {
                break;
            }
            let t = b.t_ms as i64;
            let start = t - i64::from(b.dt_ms);
            for e in compared_events(b) {
                per_column[slot].compared += 1;
                let found = rows_meeting(reference, start - slack + 1, t + slack).any(|i| {
                    compared_events(&reference[i])
                        .iter()
                        .any(|r| same_event(r, e))
                });
                if !found {
                    let ri = row_at(reference, b.t_ms).unwrap_or(reference.len() - 1);
                    let d = Divergence {
                        t_ms: b.t_ms,
                        column: "events".to_string(),
                        reference: events_or_none(&reference[ri]),
                        rebuild: e.to_string(),
                        reference_row: ri,
                        rebuild_row: k,
                    };
                    note(slot, &mut per_column[slot], d);
                }
            }
        }
    }

    let first_divergence = firsts
        .into_iter()
        .enumerate()
        .filter_map(|(slot, d)| d.map(|d| (d.t_ms, columns[slot].0, d)))
        .min_by_key(|(t, order, _)| (*t, *order))
        .map(|(_, _, d)| d);
    Comparison {
        rows_compared,
        compared_ms,
        rows_unmatched,
        first_divergence,
        per_column,
    }
}

fn events_or_none(r: &TraceRow) -> String {
    let e = compared_events(r).join("|");
    if e.is_empty() { "none".to_string() } else { e }
}

/// A plain-text report of a comparison: a summary line, the per-column
/// table and, on a divergence, up to `context` rows of each trace leading
/// up to it (the divergent rows marked with `>`).
pub fn report(
    rebuild: &[TraceRow],
    reference: &[TraceRow],
    cmp: &Comparison,
    context: usize,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "compared {} reference rows over {} ms{}",
        cmp.rows_compared,
        cmp.compared_ms,
        if cmp.rows_unmatched > 0 {
            format!(
                "; {} reference rows run past the end of the rebuild trace",
                cmp.rows_unmatched
            )
        } else {
            String::new()
        }
    );
    match &cmp.first_divergence {
        None => out.push_str("no divergence\n"),
        Some(d) => {
            let _ = writeln!(
                out,
                "first divergence at t_ms {} in column {}: reference {}, rebuild {}",
                d.t_ms, d.column, d.reference, d.rebuild
            );
        }
    }
    let _ = writeln!(
        out,
        "\n{:<14}{:>10}{:>10}{:>12}{:>12}",
        "column", "compared", "diverged", "max_diff", "first_t_ms"
    );
    for s in &cmp.per_column {
        let max = s.max_diff.map_or("-".to_string(), |m| {
            if m.fract() == 0.0 {
                format!("{m:.0}")
            } else {
                format!("{m:.3}")
            }
        });
        let first = s.first_t_ms.map_or("-".to_string(), |t| t.to_string());
        let _ = writeln!(
            out,
            "{:<14}{:>10}{:>10}{:>12}{:>12}",
            s.column, s.compared, s.diverged, max, first
        );
    }
    if let Some(d) = &cmp.first_divergence {
        for (label, rows, at) in [
            ("reference", reference, d.reference_row),
            ("rebuild", rebuild, d.rebuild_row),
        ] {
            let _ = writeln!(out, "\n{label} rows up to the divergence:\n  {HEADER}");
            let from = at.saturating_sub(context);
            for (i, r) in rows.iter().enumerate().take(at + 1).skip(from) {
                let mark = if i == at { '>' } else { ' ' };
                let _ = writeln!(out, "{mark} {}", r.to_csv());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A standing row ending at `t_ms`, `dt_ms` long.
    fn row(tick: u64, t_ms: u64, dt_ms: u32, source: &str) -> TraceRow {
        TraceRow {
            tick,
            t_ms,
            dt_ms,
            x: 100,
            y: 200,
            frac_x: f64::NAN,
            frac_y: f64::NAN,
            state_id: 24,
            physics_type: 1,
            cam_x: 100,
            cam_y: 200,
            held: 0,
            pressed: 0,
            source: source.to_string(),
            activation: "",
            health: 100,
            score: 0,
            lives: 7,
            weapon: "pistol",
            pistol: 10,
            magic: 5,
            dynamite: 3,
            events: String::new(),
            rng: None,
        }
    }

    fn rows(n: u64, source: &str) -> Vec<TraceRow> {
        (0..n)
            .map(|k| row(k, (k + 1) * u64::from(TICK_MS), TICK_MS, source))
            .collect()
    }

    #[test]
    fn traces_that_do_not_overlap_never_pass() {
        let reference = rows(3, "reference");
        let c = compare(&[], &reference, &Tolerances::exact());
        assert_eq!((c.rows_compared, c.rows_unmatched), (0, 3));
        assert!(c.first_divergence.is_none());
        assert!(!c.passed(), "nothing was compared");
        let c = compare(&rows(3, "script"), &reference, &Tolerances::exact());
        assert!(c.passed());
    }

    #[test]
    fn the_level_end_is_matched_by_name() {
        let mut rebuild = rows(10, "script");
        let mut reference = rows(10, "reference");
        rebuild[6].events = "level_complete:1500".into();
        reference[6].events = "level_complete".into();
        assert!(compare(&rebuild, &reference, &Tolerances::exact()).passed());
        // Other kinds still compare their values.
        rebuild[6].events = "checkpoint:10:20".into();
        reference[6].events = "checkpoint:10:21".into();
        let d = compare(&rebuild, &reference, &Tolerances::exact())
            .first_divergence
            .expect("the checkpoints differ");
        assert_eq!(
            (d.column.as_str(), d.reference.as_str()),
            ("events", "checkpoint:10:21")
        );
        assert!(same_event("level_complete", "level_complete:0"));
        assert!(!same_event("level_complete", "game_over"));
        assert!(!same_event("claw_died:5", "claw_died:4"));
    }

    #[test]
    fn an_event_after_the_rebuild_trace_ends_is_a_divergence() {
        let rebuild = rows(10, "script");
        let mut reference = rows(14, "reference");
        reference[12].events = "level_complete".into();
        let c = compare(&rebuild, &reference, &Tolerances::feel());
        assert_eq!((c.rows_compared, c.rows_unmatched), (10, 4));
        assert!(!c.passed());
        let d = c.first_divergence.expect("the level end was not reached");
        assert_eq!(
            (
                d.t_ms,
                d.column.as_str(),
                d.reference.as_str(),
                d.rebuild.as_str()
            ),
            (13 * 18, "events", "level_complete", "the trace has ended")
        );
        assert_eq!(d.rebuild_row, 9);
        // Rows without events past the end are only counted.
        reference[12].events.clear();
        assert!(compare(&rebuild, &reference, &Tolerances::feel()).passed());
    }
}
