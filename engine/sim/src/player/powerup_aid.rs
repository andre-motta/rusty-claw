//! The powerup play-test aid: any powerup on demand, for trying each one's
//! look and effect without looking for its pickup.
//!
//! A powerup is named by a short name ([`AID_POWERUPS`]): `invisible`,
//! `invulnerable`, `catnip`, `fire`, `ice` or `lightning`, optionally with its length in whole seconds (`fire:45`). Without one it
//! runs for [`AID_DEFAULT_MS`], the length of a powerup pickup that names
//! none (the larger catnip's for catnip). `none` names no powerup: giving
//! it ends the running one.
//!
//! The simulation gives one through [`crate::Sim::give_powerup`] at the
//! boundary before the next tick, or at every level start and respawn
//! with [`crate::SimConfig::start_powerup`]. Either way it starts as a
//! pickup starts it ([`super::death::set_powerup`]): the powerup tune, the
//! voice line, the look and the timer on the status bar.
//!
//! Like the other play-test aids, a run with it says nothing about how a
//! level plays: recordings name it (see [`crate::input::ScriptHeader`]),
//! and routes and the comparisons with reference traces refuse them.

use std::fmt;

use super::death::{POWERUP_MAX_MS, powerup};
use super::hit::POWERUP_DEFAULT_MS;

/// Length of a powerup the aid gives without a length, ms: the length of
/// a powerup pickup that names none (and of the larger catnip).
pub const AID_DEFAULT_MS: u32 = POWERUP_DEFAULT_MS;

/// The name that gives no powerup (ends the running one).
pub const NONE_NAME: &str = "none";

/// The powerups the aid gives, in the order the play-test key steps
/// through them: short name, powerup code and the name shown on screen.
/// The plasma sword is left out: the game has its code and pickup, but
/// taking it changes nothing.
pub const AID_POWERUPS: [(&str, u32, &str); 6] = [
    ("invisible", powerup::GHOST, "INVISIBLE"),
    ("invulnerable", powerup::VADER, "INVULNERABLE"),
    ("catnip", powerup::CATNIP, "CATNIP"),
    ("fire", powerup::FIRE_SWORD, "FIRE SWORD"),
    ("ice", powerup::ICE_SWORD, "ICE SWORD"),
    ("lightning", powerup::LIGHTNING_SWORD, "LIGHTNING SWORD"),
];

/// A powerup the aid gives and for how long; [`AidPowerup::NONE`] ends
/// the running one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AidPowerup {
    /// The powerup code ([`powerup`]), 0 for none.
    pub kind: u32,
    /// Its length, ms (0 for none).
    pub ms: u32,
}

impl AidPowerup {
    /// No powerup: giving it ends the running one.
    pub const NONE: AidPowerup = AidPowerup { kind: 0, ms: 0 };

    /// The powerup with code `kind` for its default length; `None` for a
    /// code the aid does not give.
    pub fn of_kind(kind: u32) -> Option<AidPowerup> {
        AID_POWERUPS
            .iter()
            .any(|(_, k, _)| *k == kind)
            .then_some(AidPowerup {
                kind,
                ms: AID_DEFAULT_MS,
            })
    }

    /// True for [`AidPowerup::NONE`] (or any entry with no code or length).
    pub fn is_none(self) -> bool {
        self.kind == 0 || self.ms == 0
    }

    /// Reads `NAME[:SECONDS]`: a name of [`AID_POWERUPS`] (any case) and
    /// an optional length of 1 to 999 whole seconds, or `none`.
    pub fn parse(text: &str) -> Result<AidPowerup, String> {
        let text = text.trim();
        let (name, secs) = match text.split_once(':') {
            Some((n, s)) => (n, Some(s)),
            None => (text, None),
        };
        let name = name.trim().to_ascii_lowercase();
        if name == NONE_NAME {
            return match secs {
                None => Ok(AidPowerup::NONE),
                Some(_) => Err("none takes no length".into()),
            };
        }
        let Some(&(_, kind, _)) = AID_POWERUPS.iter().find(|(n, _, _)| *n == name) else {
            return Err(format!(
                "unknown powerup {name:?}, expected one of {}",
                names_list()
            ));
        };
        let max_s = POWERUP_MAX_MS / 1000;
        let ms = match secs {
            None => AID_DEFAULT_MS,
            Some(s) => {
                let n: u32 = s
                    .trim()
                    .parse()
                    .map_err(|_| format!("bad powerup length {s:?}, expected whole seconds"))?;
                if !(1..=max_s).contains(&n) {
                    return Err(format!(
                        "powerup length {n} out of range, expected 1 to {max_s} seconds"
                    ));
                }
                n * 1000
            }
        };
        Ok(AidPowerup { kind, ms })
    }

    /// The short name (`fire`), or `none`.
    pub fn name(self) -> &'static str {
        AID_POWERUPS
            .iter()
            .find(|(_, k, _)| *k == self.kind && !self.is_none())
            .map_or(NONE_NAME, |(n, _, _)| n)
    }

    /// The name shown on screen (`FIRE SWORD`), or `NO POWERUP`.
    pub fn label(self) -> &'static str {
        AID_POWERUPS
            .iter()
            .find(|(_, k, _)| *k == self.kind && !self.is_none())
            .map_or("NO POWERUP", |(_, _, l)| l)
    }

    /// The powerup the play-test key gives after `last`, the one it gave
    /// before (or the one the level started with): the next of
    /// [`AID_POWERUPS`] in order, none after the last, and the first again
    /// after none.
    pub fn next_in_cycle(last: AidPowerup) -> AidPowerup {
        if last.is_none() {
            return AidPowerup::of_kind(AID_POWERUPS[0].1).unwrap_or(AidPowerup::NONE);
        }
        AID_POWERUPS
            .iter()
            .position(|(_, k, _)| *k == last.kind)
            .and_then(|i| AID_POWERUPS.get(i + 1))
            .and_then(|(_, k, _)| AidPowerup::of_kind(*k))
            .unwrap_or(AidPowerup::NONE)
    }
}

/// `NAME:SECONDS` (`fire:30`), or `none`: the form [`AidPowerup::parse`]
/// reads.
impl fmt::Display for AidPowerup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_none() {
            return f.write_str(NONE_NAME);
        }
        write!(f, "{}:{}", self.name(), self.ms / 1000)
    }
}

/// The names [`AidPowerup::parse`] takes, for messages.
pub fn names_list() -> String {
    let names: Vec<&str> = AID_POWERUPS.iter().map(|(n, _, _)| *n).collect();
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_lengths_read_back() {
        for (name, kind, _) in AID_POWERUPS {
            let p = AidPowerup::parse(name).unwrap();
            assert_eq!(p, AidPowerup { kind, ms: 30_000 });
            assert_eq!(AidPowerup::parse(&p.to_string()).unwrap(), p);
        }
        let p = AidPowerup::parse("Fire:45").unwrap();
        assert_eq!(
            (p.kind, p.ms, p.to_string()),
            (powerup::FIRE_SWORD, 45_000, "fire:45".into())
        );
        assert_eq!(AidPowerup::parse("none").unwrap(), AidPowerup::NONE);
        for bad in [
            "",
            "sword",
            "fire:",
            "fire:0",
            "fire:1000",
            "fire:2.5",
            "none:3",
        ] {
            assert!(AidPowerup::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_cycle_goes_through_every_powerup_then_none() {
        let mut p = AidPowerup::NONE;
        let mut seen = Vec::new();
        for _ in 0..8 {
            p = AidPowerup::next_in_cycle(p);
            seen.push(p.name());
        }
        assert_eq!(
            seen,
            [
                "invisible",
                "invulnerable",
                "catnip",
                "fire",
                "ice",
                "lightning",
                "none",
                "invisible"
            ]
        );
    }
}
