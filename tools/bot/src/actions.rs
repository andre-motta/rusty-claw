//! The macro-actions the search plays: short runs of held buttons, each a
//! few ticks long.
//!
//! Every action is a list of segments, each one mask held for at least
//! [`MIN_SPACING`] ticks, so any chain of actions changes the held mask at
//! most every [`MIN_SPACING`] ticks: a display at 60 Hz or slower never
//! drops a press.

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
        }
        out
    }

    /// A short name for logs.
    pub fn name(self) -> String {
        match self {
            Action::Jump { dir, hold, up } => {
                format!("jump{dir:+}x{hold}{}", if up { "up" } else { "" })
            }
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
        ];
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
