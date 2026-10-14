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

use crate::tuning::TICK_MS;

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

/// A recorded input script: one [`TickInput`] per tick.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputScript {
    /// Inputs by tick.
    pub ticks: Vec<TickInput>,
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

impl InputScript {
    /// Parses CSV text with the columns `tick,held_mask,pressed_mask` (a
    /// header line, one row per tick, masks in hex with `0x` or decimal).
    /// Lines starting with `#` are comments. Ticks must start at 0 and
    /// increase by one.
    pub fn parse(text: &str) -> Result<InputScript, ScriptError> {
        let mut ticks = Vec::new();
        let mut header_seen = false;
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let err = |message: String| ScriptError {
                line: i + 1,
                message,
            };
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
        Ok(InputScript { ticks })
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
        InputScript { ticks }
    }

    /// Writes the script back as CSV.
    pub fn to_csv(&self) -> String {
        let mut out = String::from("tick,held_mask,pressed_mask\n");
        for (i, t) in self.ticks.iter().enumerate() {
            out.push_str(&format!("{i},0x{:x},0x{:x}\n", t.held.0, t.pressed.0));
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
