//! Turning button events into per-tick input, and recorded input scripts.
//!
//! The simulation reads one [`TickInput`] per 18 ms tick: the buttons held
//! during the tick and the buttons pressed since the previous tick. Events
//! from the window system arrive at display rate with a timestamp; the
//! [`InputLatch`] assigns each event to the tick whose time span contains
//! it, so the same events give the same ticks at any display rate. A press
//! and release that both happen inside one tick still reach that tick as
//! pressed and held.

use std::collections::VecDeque;

use claw_frame::{Buttons, TickInput};

use crate::tuning::{PlayAids, TICK_MS};

/// Length of a tick in microseconds.
pub const TICK_US: u64 = TICK_MS as u64 * 1000;

/// Collects timestamped button events and hands out tick inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputLatch {
    events: VecDeque<(u64, Buttons, bool)>,
    down: Buttons,
}

impl InputLatch {
    /// An empty latch with nothing held.
    pub fn new() -> InputLatch {
        InputLatch::default()
    }

    /// Records that `buttons` went down (`down`) or up at `at_us`
    /// microseconds of simulation time. Events must arrive in time order.
    pub fn event(&mut self, at_us: u64, buttons: Buttons, down: bool) {
        self.events.push_back((at_us, buttons, down));
    }

    /// The input of tick `tick`, which spans `[tick * 18 ms, (tick + 1) *
    /// 18 ms)`: consumes the events before the tick's end. A button counts
    /// as held when it is down at the end of the span or was pressed inside
    /// it.
    pub fn take(&mut self, tick: u64) -> TickInput {
        let end = (tick + 1) * TICK_US;
        let mut pressed = Buttons::NONE;
        while let Some(&(at, b, down)) = self.events.front() {
            if at >= end {
                break;
            }
            self.events.pop_front();
            if down {
                pressed |= Buttons(b.0 & !self.down.0);
                self.down |= b;
            } else {
                self.down = Buttons(self.down.0 & !b.0);
            }
        }
        // Buttons down at the end of the tick, plus any pressed during it
        // (a press released again inside the tick still counts).
        TickInput {
            held: self.down | pressed,
            pressed,
        }
    }

    /// Buttons down right now.
    pub fn down(&self) -> Buttons {
        self.down
    }
}

/// A recorded input script, also called a route: one [`TickInput`] per
/// tick, optional header keys and optional directives.
///
/// The text form is CSV with the columns `tick,held_mask,pressed_mask` (a
/// header line, one row per tick, masks in hex with `0x` or decimal). Row
/// `k` covers game time `[18k, 18k + 18)` ms from the level's first logic
/// step, the route's time zero. Two further kinds of line are allowed:
///
/// - Header keys, comment lines naming how the route is meant to be played:
///   `# level N`, `# tuning default|original` and `# seed N` (the state of
///   the shared random stream at time zero), and the play-test aids it was
///   played with, `# invincible = on|off` and `# infinite_lives = on|off`
///   (see [`PlayAids`]; off when absent). An `=` between the key and the
///   value is accepted. The words `level`, `tuning`, `seed`, `invincible`
///   and `infinite_lives` are reserved: a `#` line whose first word is one
///   of them is read as that header key and must hold a valid value, so a
///   comment never starts with them. Every other line starting with `#` is
///   a comment.
/// - Directives, lines starting with `@`, which take effect at the tick
///   boundary before row `<tick>`:
///   - `@<tick> pause <wall_ms> [held=<mask>]`;
///   - `@<tick> menu <help|ingame|pause> <wall_ms> [keys=<list>]`, the list
///     being key names separated by commas;
///   - `@<tick> command end_life`.
///
/// A directive kind this reader does not know is an error, never skipped, so
/// a route can never be played differently from how it was written.
///
/// A script played with a play-test aid on is not a route: it says nothing
/// about how the level plays. [`InputScript::parse`] refuses it, so routes,
/// the bot and the comparisons with reference traces only ever take plain
/// play; the game's replays read it with [`InputScript::parse_with_aids`]
/// and play it with the aids it names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputScript {
    /// Inputs by tick.
    pub ticks: Vec<TickInput>,
    /// Header keys.
    pub header: ScriptHeader,
    /// Directives in tick order (directives of the same tick keep the order
    /// they were written in).
    pub directives: Vec<Directive>,
}

/// The header keys of an input script; `None` when a key is absent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScriptHeader {
    /// `# level N`: the level the route plays.
    pub level: Option<u32>,
    /// `# tuning default|original`: the movement tuning the route is for.
    pub tuning: Option<ScriptTuning>,
    /// `# seed N`: the state of the shared random stream at time zero.
    pub seed: Option<u32>,
    /// `# invincible = on` and `# infinite_lives = on`: the play-test aids
    /// the script was played with; both off when it names none.
    pub aids: PlayAids,
}

/// The movement tuning named by a script's `# tuning` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptTuning {
    /// The default tuning, matching the original played at 60 frames per
    /// second.
    Default,
    /// The original game's movement numbers at the fixed 18 ms step
    /// ([`crate::Tuning::original_speeds`]).
    Original,
}

impl ScriptTuning {
    /// The name used in scripts and on command lines.
    pub fn name(self) -> &'static str {
        match self {
            ScriptTuning::Default => "default",
            ScriptTuning::Original => "original",
        }
    }

    /// The tuning of a name, `default` or `original`.
    pub fn from_name(name: &str) -> Option<ScriptTuning> {
        match name {
            "default" => Some(ScriptTuning::Default),
            "original" => Some(ScriptTuning::Original),
            _ => None,
        }
    }

    /// The movement constants this names.
    pub fn tuning(self) -> crate::Tuning {
        match self {
            ScriptTuning::Default => crate::Tuning::default(),
            ScriptTuning::Original => crate::Tuning::original_speeds(),
        }
    }
}

/// One `@` line of an input script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    /// The row the directive takes effect before (the boundary between rows
    /// `tick - 1` and `tick`); at most the number of rows.
    pub tick: usize,
    /// What happens there.
    pub kind: DirectiveKind,
}

/// What a directive does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectiveKind {
    /// `pause <wall_ms> [held=<mask>]`: play is paused for `wall_ms`
    /// milliseconds of wall time, with `held` the buttons held during the
    /// pause.
    Pause {
        /// Wall time paused, ms.
        wall_ms: u32,
        /// Buttons held during the pause, when given.
        held: Option<u32>,
    },
    /// `menu <screen> <wall_ms> [keys=<list>]`: a menu screen is open for
    /// `wall_ms` milliseconds of wall time, with the keys pressed in it.
    Menu {
        /// The screen.
        screen: MenuScreen,
        /// Wall time on the screen, ms.
        wall_ms: u32,
        /// Key names pressed on the screen, in order.
        keys: Vec<String>,
    },
    /// `command <name>`: a game command.
    Command(ScriptCommand),
}

/// A menu screen a directive opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuScreen {
    /// `help`: the help screen.
    Help,
    /// `ingame`: the in-game menu.
    InGame,
    /// `pause`: the pause screen.
    Pause,
}

impl MenuScreen {
    /// The name used in scripts.
    pub fn name(self) -> &'static str {
        match self {
            MenuScreen::Help => "help",
            MenuScreen::InGame => "ingame",
            MenuScreen::Pause => "pause",
        }
    }

    fn from_name(name: &str) -> Option<MenuScreen> {
        match name {
            "help" => Some(MenuScreen::Help),
            "ingame" => Some(MenuScreen::InGame),
            "pause" => Some(MenuScreen::Pause),
            _ => None,
        }
    }
}

/// A game command a directive gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptCommand {
    /// `end_life`: Claw gives up the current life.
    EndLife,
}

impl ScriptCommand {
    /// The name used in scripts.
    pub fn name(self) -> &'static str {
        match self {
            ScriptCommand::EndLife => "end_life",
        }
    }

    fn from_name(name: &str) -> Option<ScriptCommand> {
        match name {
            "end_life" => Some(ScriptCommand::EndLife),
            _ => None,
        }
    }
}

impl Directive {
    /// The directive as a script line (no line end).
    pub fn to_line(&self) -> String {
        let mut s = format!("@{} ", self.tick);
        match &self.kind {
            DirectiveKind::Pause { wall_ms, held } => {
                s.push_str(&format!("pause {wall_ms}"));
                if let Some(h) = held {
                    s.push_str(&format!(" held=0x{h:x}"));
                }
            }
            DirectiveKind::Menu {
                screen,
                wall_ms,
                keys,
            } => {
                s.push_str(&format!("menu {} {wall_ms}", screen.name()));
                if !keys.is_empty() {
                    s.push_str(&format!(" keys={}", keys.join(",")));
                }
            }
            DirectiveKind::Command(c) => s.push_str(&format!("command {}", c.name())),
        }
        s
    }

    /// Parses one `@` line (without its line number).
    fn parse(line: &str) -> Result<Directive, String> {
        let mut words = line.split_whitespace();
        let at = words.next().unwrap_or_default();
        let tick: usize = at
            .strip_prefix('@')
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| format!("bad directive tick {at:?}, expected @<tick>"))?;
        let kind = words
            .next()
            .ok_or_else(|| format!("directive {at} has no kind"))?;
        let rest: Vec<&str> = words.collect();
        let wall_ms = |w: Option<&&str>| -> Result<u32, String> {
            let w = w.ok_or_else(|| format!("{kind} directive needs a wall time in ms"))?;
            w.parse()
                .map_err(|_| format!("bad wall time {w:?} in {kind} directive"))
        };
        let kind = match kind {
            "pause" => {
                let wall_ms = wall_ms(rest.first())?;
                let mut held = None;
                for opt in &rest[1..] {
                    match opt.split_once('=') {
                        Some(("held", v)) if held.is_none() => {
                            held =
                                Some(parse_mask(v).ok_or_else(|| format!("bad held mask {v:?}"))?);
                        }
                        _ => return Err(format!("unexpected {opt:?} in pause directive")),
                    }
                }
                DirectiveKind::Pause { wall_ms, held }
            }
            "menu" => {
                let screen = rest
                    .first()
                    .ok_or("menu directive needs a screen: help, ingame or pause")?;
                let screen = MenuScreen::from_name(screen).ok_or_else(|| {
                    format!("unknown menu screen {screen:?}, expected help, ingame or pause")
                })?;
                let wall_ms = wall_ms(rest.get(1))?;
                let mut keys = None;
                for opt in rest.iter().skip(2) {
                    match opt.split_once('=') {
                        Some(("keys", v)) if keys.is_none() => {
                            let list: Vec<String> = v.split(',').map(str::to_string).collect();
                            if let Some(bad) = list.iter().find(|k| !is_key_name(k)) {
                                return Err(format!("bad key name {bad:?} in menu directive"));
                            }
                            keys = Some(list);
                        }
                        _ => return Err(format!("unexpected {opt:?} in menu directive")),
                    }
                }
                DirectiveKind::Menu {
                    screen,
                    wall_ms,
                    keys: keys.unwrap_or_default(),
                }
            }
            "command" => {
                let name = rest
                    .first()
                    .ok_or("command directive needs a command name")?;
                let c = ScriptCommand::from_name(name)
                    .ok_or_else(|| format!("unknown command {name:?}, expected end_life"))?;
                if let Some(extra) = rest.get(1) {
                    return Err(format!("unexpected {extra:?} in command directive"));
                }
                DirectiveKind::Command(c)
            }
            other => {
                return Err(format!(
                    "unknown directive kind {other:?}, expected pause, menu or command"
                ));
            }
        };
        Ok(Directive { tick, kind })
    }
}

fn is_key_name(k: &str) -> bool {
    !k.is_empty()
        && k.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// A held-mask change that comes too soon after the previous one (see
/// [`InputScript::spacing_warnings`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpacingWarning {
    /// The tick whose held mask differs from the tick before.
    pub tick: usize,
    /// Ticks since the previous change.
    pub gap: usize,
}

/// A problem in an input script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    /// 1-based line number.
    pub line: usize,
    /// What is wrong.
    pub message: String,
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ScriptError {}

fn parse_mask(s: &str) -> Option<u32> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

/// The message of [`InputScript::parse`] refusing a script played with
/// `aids` on.
pub fn aids_refusal(aids: PlayAids) -> String {
    let names: Vec<String> = aids.names().iter().map(|n| format!("{n} = on")).collect();
    format!(
        "the script was played with a play-test aid on ({}); routes, the bot and comparisons \
         with reference traces take plain play only (replay it in the game, or with \
         claw-replay without --compare)",
        names.join(", ")
    )
}

/// Header key names.
const HEADER_KEYS: [&str; 5] = [
    "level",
    "tuning",
    "seed",
    PlayAids::NAMES[0],
    PlayAids::NAMES[1],
];

impl ScriptHeader {
    /// Reads a `#` comment line into the header if it is a header key:
    /// `Ok(true)` when it was one, `Ok(false)` for a plain comment.
    /// `aids_seen` marks the aid keys ([`PlayAids::NAMES`]) read so far.
    fn read(&mut self, comment: &str, aids_seen: &mut [bool; 2]) -> Result<bool, String> {
        let mut words = comment.split_whitespace();
        let Some(key) = words.next() else {
            return Ok(false);
        };
        // `# level 3`, `# level = 3`, and `# level=3` all name the key.
        let (key, glued) = match key.split_once('=') {
            Some((k, v)) => (k, Some(v)),
            None => (key, None),
        };
        if !HEADER_KEYS.contains(&key) {
            return Ok(false);
        }
        let mut rest: Vec<&str> = glued.filter(|v| !v.is_empty()).into_iter().collect();
        rest.extend(words);
        if glued.is_none()
            && let Some(first) = rest.first().copied()
            && let Some(v) = first.strip_prefix('=')
        {
            if v.is_empty() {
                rest.remove(0);
            } else {
                rest[0] = v;
            }
        }
        let [value] = rest[..] else {
            return Err(format!("header key {key} needs exactly one value"));
        };
        let dup = || format!("header key {key} is given twice");
        if let Some(i) = PlayAids::NAMES.iter().position(|n| *n == key) {
            if aids_seen[i] {
                return Err(dup());
            }
            aids_seen[i] = true;
            let on = match value {
                "on" => true,
                "off" => false,
                _ => return Err(format!("bad {key} {value:?} in header, expected on or off")),
            };
            if let Some(switch) = self.aids.switch_mut(key) {
                *switch = on;
            }
            return Ok(true);
        }
        match key {
            "level" => {
                if self.level.is_some() {
                    return Err(dup());
                }
                self.level = Some(
                    value
                        .parse()
                        .map_err(|_| format!("bad level {value:?} in header"))?,
                );
            }
            "tuning" => {
                if self.tuning.is_some() {
                    return Err(dup());
                }
                self.tuning = Some(ScriptTuning::from_name(value).ok_or_else(|| {
                    format!("bad tuning {value:?} in header, expected default or original")
                })?);
            }
            _ => {
                if self.seed.is_some() {
                    return Err(dup());
                }
                self.seed = Some(
                    value
                        .parse()
                        .map_err(|_| format!("bad seed {value:?} in header"))?,
                );
            }
        }
        Ok(true)
    }

    /// The header as `#` lines, each ending in a line end.
    fn to_lines(self) -> String {
        let mut out = String::new();
        if let Some(level) = self.level {
            out.push_str(&format!("# level {level}\n"));
        }
        if let Some(tuning) = self.tuning {
            out.push_str(&format!("# tuning {}\n", tuning.name()));
        }
        if let Some(seed) = self.seed {
            out.push_str(&format!("# seed {seed}\n"));
        }
        for name in self.aids.names() {
            out.push_str(&format!("# {name} = on\n"));
        }
        out
    }
}

impl InputScript {
    /// Parses a route: the text form described on [`InputScript`], played
    /// with no play-test aid. Ticks must start at 0 and increase by one;
    /// directives may sit anywhere, but name a tick no greater than the
    /// number of rows.
    ///
    /// A script that names an aid on (`# invincible = on`,
    /// `# infinite_lives = on`) is refused: routes, the bot and the
    /// comparisons with reference traces take plain play only. The game's
    /// replays read such a recording with [`InputScript::parse_with_aids`].
    pub fn parse(text: &str) -> Result<InputScript, ScriptError> {
        let (script, line) = InputScript::read_text(text)?;
        if let Some(line) = line {
            return Err(ScriptError {
                line,
                message: aids_refusal(script.header.aids),
            });
        }
        Ok(script)
    }

    /// Parses the text form described on [`InputScript`] like
    /// [`InputScript::parse`], also when the script was played with a
    /// play-test aid on ([`ScriptHeader::aids`] then names it): for
    /// replaying a recording as it was played.
    pub fn parse_with_aids(text: &str) -> Result<InputScript, ScriptError> {
        InputScript::read_text(text).map(|(script, _)| script)
    }

    /// Parses the text form; with the script, the line of the first header
    /// key that turns an aid on, if any.
    fn read_text(text: &str) -> Result<(InputScript, Option<usize>), ScriptError> {
        let mut ticks = Vec::new();
        let mut header = ScriptHeader::default();
        let mut directives: Vec<(usize, Directive)> = Vec::new();
        let mut header_seen = false;
        let mut aids_seen = [false; 2];
        let mut aid_line = None;
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim();
            let err = |message: String| ScriptError {
                line: i + 1,
                message,
            };
            if line.is_empty() {
                continue;
            }
            if let Some(comment) = line.strip_prefix('#') {
                let aids = header.aids;
                header.read(comment, &mut aids_seen).map_err(err)?;
                if header.aids != aids && header.aids.any() {
                    aid_line.get_or_insert(i + 1);
                }
                continue;
            }
            if line.starts_with('@') {
                directives.push((i + 1, Directive::parse(line).map_err(err)?));
                continue;
            }
            if !header_seen {
                header_seen = true;
                if line.split(',').next().map(str::trim) == Some("tick") {
                    continue;
                }
            }
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() < 3 {
                return Err(err(format!("expected 3 columns, found {}", cols.len())));
            }
            let tick: usize = cols[0]
                .trim()
                .parse()
                .map_err(|_| err(format!("bad tick {:?}", cols[0])))?;
            if tick != ticks.len() {
                return Err(err(format!("expected tick {}, found {tick}", ticks.len())));
            }
            let held = parse_mask(cols[1]).ok_or_else(|| err(format!("bad mask {:?}", cols[1])))?;
            let pressed =
                parse_mask(cols[2]).ok_or_else(|| err(format!("bad mask {:?}", cols[2])))?;
            ticks.push(TickInput {
                held: Buttons(held),
                pressed: Buttons(pressed),
            });
        }
        if let Some((line, d)) = directives.iter().find(|(_, d)| d.tick > ticks.len()) {
            return Err(ScriptError {
                line: *line,
                message: format!(
                    "directive at tick {} is past the last row (the script has {} rows)",
                    d.tick,
                    ticks.len()
                ),
            });
        }
        // Stable: directives of one tick keep their written order.
        directives.sort_by_key(|(_, d)| d.tick);
        let script = InputScript {
            ticks,
            header,
            directives: directives.into_iter().map(|(_, d)| d).collect(),
        };
        Ok((script, aid_line))
    }

    /// A script from held masks per tick; pressed is the rising edge.
    pub fn from_held(held: &[u32]) -> InputScript {
        let mut prev = 0;
        let ticks = held
            .iter()
            .map(|&h| {
                let t = TickInput {
                    held: Buttons(h),
                    pressed: Buttons(h & !prev),
                };
                prev = h;
                t
            })
            .collect();
        InputScript {
            ticks,
            ..InputScript::default()
        }
    }

    /// The script's directives in tick order.
    pub fn directives(&self) -> &[Directive] {
        &self.directives
    }

    /// Writes the script back as text: the header keys, the column line,
    /// then the rows with each directive just before the row it names.
    /// [`InputScript::parse`] reads it back unchanged.
    pub fn to_csv(&self) -> String {
        let mut out = self.header.to_lines();
        out.push_str("tick,held_mask,pressed_mask\n");
        let mut directives: Vec<&Directive> = self.directives.iter().collect();
        directives.sort_by_key(|d| d.tick);
        let mut next = directives.into_iter().peekable();
        for (i, t) in self.ticks.iter().enumerate() {
            while let Some(d) = next.next_if(|d| d.tick <= i) {
                out.push_str(&d.to_line());
                out.push('\n');
            }
            out.push_str(&format!("{i},0x{:x},0x{:x}\n", t.held.0, t.pressed.0));
        }
        for d in next {
            out.push_str(&d.to_line());
            out.push('\n');
        }
        out
    }

    /// Held-mask changes that come fewer than `min_ticks` ticks after the
    /// previous change (the mask before the first tick counts as nothing
    /// held). A consumer that samples the route at another rate can miss
    /// such a short press or release; routes keep changes at least two
    /// ticks apart.
    pub fn spacing_warnings(&self, min_ticks: usize) -> Vec<SpacingWarning> {
        let mut out = Vec::new();
        let mut prev_mask = 0u32;
        let mut prev_change: Option<usize> = None;
        for (k, t) in self.ticks.iter().enumerate() {
            if t.held.0 == prev_mask {
                continue;
            }
            if let Some(p) = prev_change
                && k - p < min_ticks
            {
                out.push(SpacingWarning {
                    tick: k,
                    gap: k - p,
                });
            }
            prev_mask = t.held.0;
            prev_change = Some(k);
        }
        out
    }

    /// Button events of the script: each change of the held mask at the
    /// start of its tick, in microseconds.
    pub fn events(&self) -> Vec<(u64, Buttons, bool)> {
        let mut out = Vec::new();
        let mut prev = 0u32;
        for (k, t) in self.ticks.iter().enumerate() {
            let at = k as u64 * TICK_US;
            let h = t.held.0;
            let up = prev & !h;
            let down = h & !prev;
            if up != 0 {
                out.push((at, Buttons(up), false));
            }
            if down != 0 {
                out.push((at, Buttons(down), true));
            }
            prev = h;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comments_hex_and_decimal() {
        let s = InputScript::parse(
            "tick,held_mask,pressed_mask\n# note\n0,0x2000000,0x2000000\n1,33554432,0\n",
        )
        .unwrap();
        assert_eq!(s.ticks.len(), 2);
        assert_eq!(s.ticks[1].held, Buttons::RIGHT);
        assert_eq!(s.ticks[0].pressed, Buttons::RIGHT);
        assert!(InputScript::parse("tick,a,b\n1,0,0\n").is_err());
        assert!(InputScript::parse("tick,a,b\n0,zz,0\n").is_err());
    }

    #[test]
    fn short_presses_latch_into_their_tick() {
        let mut latch = InputLatch::new();
        latch.event(2_000, Buttons::JUMP, true);
        latch.event(5_000, Buttons::JUMP, false);
        let t0 = latch.take(0);
        assert_eq!(t0.held, Buttons::JUMP);
        assert_eq!(t0.pressed, Buttons::JUMP);
        let t1 = latch.take(1);
        assert!(t1.held.is_empty() && t1.pressed.is_empty());
    }

    #[test]
    fn held_buttons_press_once() {
        let mut latch = InputLatch::new();
        latch.event(0, Buttons::RIGHT, true);
        assert_eq!(latch.take(0).pressed, Buttons::RIGHT);
        let t = latch.take(1);
        assert_eq!(t.held, Buttons::RIGHT);
        assert!(t.pressed.is_empty());
        latch.event(40_000, Buttons::RIGHT, false);
        assert!(latch.take(2).held.is_empty());
    }

    #[test]
    fn script_events_round_trip_through_the_latch() {
        let s = InputScript::from_held(&[0, 1, 1, 0, 0x2000000, 0x2000001, 0]);
        let mut latch = InputLatch::new();
        for (at, b, down) in s.events() {
            latch.event(at, b, down);
        }
        let back: Vec<TickInput> = (0..s.ticks.len() as u64).map(|k| latch.take(k)).collect();
        assert_eq!(back, s.ticks);
        assert_eq!(InputScript::parse(&s.to_csv()).unwrap(), s);
    }
}
