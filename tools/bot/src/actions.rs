//! The macro-actions the search plays: short runs of held buttons, each a
//! few ticks long.
//!
//! Every action is a list of segments, each one mask held for at least
//! [`MIN_SPACING`] ticks, so any chain of actions changes the held mask at
//! most every [`MIN_SPACING`] ticks: a display at 60 Hz or slower never
//! drops a press.
//!
//! A boss level adds two kinds of macro ([`choices_in`]):
//!
//! - the fight's, offered while a boss is in play: a short wait to time a
//!   blow against the boss's own attacks, a blow struck after turning to
//!   either side, and jumps to leap clear (the retreat) or over him; the
//!   walks are the approach;
//! - the door's, offered on the ground near the boss room's door: walk
//!   toward the door for a few ticks, then jump on through it, so that one
//!   of them jumps on Claw's first tick in the room (the jump-in entry,
//!   which skips the boss's introduction).

use claw_sim::player::state;
use claw_sim::{Buttons, Sim};

/// Fewest ticks between two changes of the held mask in a route.
pub const MIN_SPACING: usize = 2;

/// Length of the basic actions (walk, wait, climb), ticks.
pub const STEP: usize = 6;

/// Jump.
pub const JUMP: u32 = Buttons::JUMP.0;
/// Attack.
pub const ATTACK: u32 = Buttons::ATTACK.0;
/// Fire the selected weapon.
pub const FIRE: u32 = Buttons::PROJECTILE.0;
/// Left.
pub const LEFT: u32 = Buttons::LEFT.0;
/// Right.
pub const RIGHT: u32 = Buttons::RIGHT.0;
/// Up.
pub const UP: u32 = Buttons::UP.0;
/// Down.
pub const DOWN: u32 = Buttons::DOWN.0;

/// Ticks the jump button is held for, from a tap to a full jump.
pub const JUMP_HOLDS: [usize; 4] = [2, 5, 9, 14];

/// Length of the fight's short wait, ticks.
pub const SHORT: usize = 3;

/// Ticks the jump of a door entry holds jump (with the direction).
pub const ENTRY_JUMP: usize = 12;

/// Ticks a door entry walks before it jumps: one entry each.
pub const ENTRY_WALKS: std::ops::RangeInclusive<u8> = 2..=8;

/// How near the door, across, Claw must stand for the door entries, px.
pub const DOOR_REACH: i32 = 240;

/// One macro-action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    /// Hold nothing.
    Wait,
    /// Walk (and, kept up, run) left.
    Left,
    /// Walk (and, kept up, run) right.
    Right,
    /// Jump, holding the button for `hold` ticks, steering `dir` (-1, 0
    /// or 1) all along, and with `up` holding Up to catch a ladder.
    Jump {
        /// Steering: -1 left, 0 none, 1 right.
        dir: i8,
        /// Ticks the jump button is held.
        hold: u8,
        /// Up is held too.
        up: bool,
    },
    /// Strike once with the sword.
    Attack,
    /// Crouch and strike low.
    LowAttack,
    /// Fire the selected weapon once.
    Fire,
    /// Crouch (and step down onto a ladder below).
    Crouch,
    /// Climb up (and step onto a ladder above).
    Up,
    /// Climb down.
    Down,
    /// Hold nothing for a moment ([`SHORT`] ticks), to time a blow.
    Short,
    /// Turn to `dir` (-1 left, 1 right) and strike once with the sword.
    Strike {
        /// The side to strike: -1 left, 1 right.
        dir: i8,
    },
    /// Walk `dir` for `walk` ticks, then jump on in that direction (jump
    /// held [`ENTRY_JUMP`] ticks): through a door, a jump on the first tick
    /// beyond it.
    Entry {
        /// Direction: -1 left, 1 right.
        dir: i8,
        /// Ticks walked before the jump.
        walk: u8,
    },
}

impl Action {
    /// The held masks of the action, one per tick.
    pub fn masks(self) -> Vec<u32> {
        let mut out = Vec::new();
        let mut seg = |mask: u32, n: usize| out.extend(std::iter::repeat_n(mask, n));
        match self {
            Action::Wait => seg(0, STEP),
            Action::Left => seg(LEFT, STEP),
            Action::Right => seg(RIGHT, STEP),
            Action::Jump { dir, hold, up } => {
                let d = dir_mask(dir) | if up { UP } else { 0 };
                seg(JUMP | d, usize::from(hold));
                seg(d, MIN_SPACING);
            }
            Action::Attack => {
                seg(ATTACK, MIN_SPACING);
                seg(0, STEP);
            }
            Action::LowAttack => {
                seg(DOWN | ATTACK, MIN_SPACING);
                seg(DOWN, STEP);
            }
            Action::Fire => {
                seg(FIRE, MIN_SPACING);
                seg(0, STEP);
            }
            Action::Crouch => seg(DOWN, STEP),
            Action::Up => seg(UP, STEP),
            Action::Down => seg(DOWN, STEP),
            Action::Short => seg(0, SHORT),
            Action::Strike { dir } => {
                seg(dir_mask(dir), MIN_SPACING);
                seg(ATTACK, MIN_SPACING);
                seg(0, STEP - MIN_SPACING);
            }
            Action::Entry { dir, walk } => {
                let d = dir_mask(dir);
                seg(d, usize::from(walk).max(MIN_SPACING));
                seg(d | JUMP, ENTRY_JUMP);
                seg(d, MIN_SPACING);
            }
        }
        out
    }

    /// A short name for logs.
    pub fn name(self) -> String {
        match self {
            Action::Jump { dir, hold, up } => {
                format!("jump{dir:+}x{hold}{}", if up { "up" } else { "" })
            }
            Action::Strike { dir } => format!("strike{dir:+}"),
            Action::Entry { dir, walk } => format!("entry{dir:+}x{walk}"),
            other => format!("{other:?}").to_lowercase(),
        }
    }
}

/// The steering mask of a direction.
pub fn dir_mask(dir: i8) -> u32 {
    match dir.signum() {
        -1 => LEFT,
        1 => RIGHT,
        _ => 0,
    }
}

/// Where Claw is, for choosing actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// On his feet.
    Ground,
    /// Jumping or falling.
    Air,
    /// On a ladder.
    Climb,
    /// Busy: struck, attacking, lifting or in another state that takes no
    /// steering.
    Busy,
}

/// Claw's phase in `sim`.
pub fn phase(sim: &Sim) -> Phase {
    let Some(o) = sim.player() else {
        return Phase::Busy;
    };
    if sim.claw().attacking {
        return Phase::Busy;
    }
    match o.state {
        state::JUMP | state::FALL => Phase::Air,
        state::CLIMB => Phase::Climb,
        state::STAND
        | state::MOVE_LEFT
        | state::MOVE_RIGHT
        | state::LAND
        | state::CROUCH
        | state::LOOK_UP
        | state::ON_EDGE => Phase::Ground,
        _ => Phase::Busy,
    }
}

/// What the actions of a boss level depend on besides the game.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Arena {
    /// The boss room's door, when the level has one.
    pub door: Option<(i32, i32)>,
}

/// The actions worth trying in `sim`, in a fixed order: [`choices`], with
/// the fight's macros instead while a boss is in play and the door
/// entries near the door (see the module documentation).
pub fn choices_in(sim: &Sim, arena: &Arena) -> Vec<Action> {
    let boss = sim.boss();
    if boss.current().is_some() && boss.defeated().is_none() {
        return fight_choices(sim);
    }
    let mut out = choices(sim);
    if let (Some((dx, dy)), Some(o), Phase::Ground) = (arena.door, sim.player(), phase(sim)) {
        let across = dx - o.x;
        if across.abs() <= DOOR_REACH && (dy - o.y).abs() <= DOOR_REACH {
            let dir = if across >= 0 { 1 } else { -1 };
            out.extend(ENTRY_WALKS.map(|walk| Action::Entry { dir, walk }));
        }
    }
    out
}

/// The actions of a boss fight: the walks (the approach), the waits, the
/// blows, and jumps to leap clear or over.
fn fight_choices(sim: &Sim) -> Vec<Action> {
    let mut out = vec![Action::Wait, Action::Left, Action::Right];
    match phase(sim) {
        Phase::Ground => {
            out.extend([
                Action::Short,
                Action::Attack,
                Action::Strike { dir: -1 },
                Action::Strike { dir: 1 },
            ]);
            for dir in [1i8, -1] {
                for hold in [JUMP_HOLDS[1], JUMP_HOLDS[3]] {
                    out.push(Action::Jump {
                        dir,
                        hold: hold as u8,
                        up: false,
                    });
                }
            }
            out.push(Action::Jump {
                dir: 0,
                hold: JUMP_HOLDS[3] as u8,
                up: false,
            });
        }
        Phase::Air => out.push(Action::Attack),
        Phase::Climb | Phase::Busy => {}
    }
    out
}

/// The actions worth trying in `sim`, in a fixed order.
pub fn choices(sim: &Sim) -> Vec<Action> {
    let mut out = vec![Action::Wait, Action::Left, Action::Right];
    match phase(sim) {
        Phase::Ground => {
            for dir in [1i8, -1, 0] {
                for hold in JUMP_HOLDS {
                    out.push(Action::Jump {
                        dir,
                        hold: hold as u8,
                        up: false,
                    });
                }
            }
            // Jumps that catch a ladder overhead.
            for dir in [0i8, 1, -1] {
                out.push(Action::Jump {
                    dir,
                    hold: JUMP_HOLDS[3] as u8,
                    up: true,
                });
            }
            out.extend([
                Action::Attack,
                Action::LowAttack,
                Action::Crouch,
                Action::Up,
            ]);
            let stats = sim.stats();
            if stats.pistol > 0 {
                out.push(Action::Fire);
            }
        }
        Phase::Air => out.extend([Action::Up, Action::Attack]),
        Phase::Climb => {
            out.extend([Action::Up, Action::Down]);
            for dir in [1i8, -1] {
                for hold in [JUMP_HOLDS[1], JUMP_HOLDS[3]] {
                    out.push(Action::Jump {
                        dir,
                        hold: hold as u8,
                        up: false,
                    });
                }
            }
        }
        Phase::Busy => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_keeps_each_mask_for_at_least_the_spacing() {
        let mut all = vec![
            Action::Wait,
            Action::Left,
            Action::Right,
            Action::Attack,
            Action::LowAttack,
            Action::Fire,
            Action::Crouch,
            Action::Up,
            Action::Down,
            Action::Short,
            Action::Strike { dir: -1 },
            Action::Strike { dir: 1 },
        ];
        for dir in [-1i8, 1] {
            for walk in ENTRY_WALKS {
                all.push(Action::Entry { dir, walk });
            }
        }
        for dir in [-1, 0, 1] {
            for hold in JUMP_HOLDS {
                for up in [false, true] {
                    all.push(Action::Jump {
                        dir,
                        hold: hold as u8,
                        up,
                    });
                }
            }
        }
        for a in all {
            let m = a.masks();
            let mut run = 1;
            for w in m.windows(2) {
                if w[0] == w[1] {
                    run += 1;
                } else {
                    assert!(run >= MIN_SPACING, "{a:?}");
                    run = 1;
                }
            }
            assert!(run >= MIN_SPACING, "{a:?}");
            // The jump button is never held at the end of an action, so
            // the next jump is a fresh press.
            assert_eq!(m.last().unwrap() & JUMP, 0, "{a:?}");
        }
    }
}
