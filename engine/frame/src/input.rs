//! Per-tick input from key events.
//!
//! Input is sampled once per tick, not once per frame. Key-down events
//! between two ticks latch into the next tick's `pressed` set; a frame that
//! runs no tick keeps the latch, so no press is lost at any display rate, and
//! when one frame runs several ticks only the first sees the latched presses,
//! so none is doubled. A tick's `held` set is the keys held now plus the keys
//! pressed since the previous tick, so a tap shorter than one tick still reads
//! as held for exactly one tick. A second press of a key that is already
//! latched waits for the following tick (at most one press per key per tick).

use crate::{Buttons, TickInput};

/// Builds [`TickInput`] values from key events.
#[derive(Debug, Clone, Default)]
pub struct InputLatch {
    held: Buttons,
    latched: Buttons,
    queued: Buttons,
}

impl InputLatch {
    /// No key held, nothing latched.
    pub fn new() -> InputLatch {
        InputLatch::default()
    }

    /// A key-down event for `buttons`. Repeats of a key that is already held
    /// (keyboard auto-repeat) are ignored.
    pub fn press(&mut self, buttons: Buttons) {
        let fresh = Buttons(buttons.0 & !self.held.0);
        self.held |= buttons;
        let again = Buttons(fresh.0 & self.latched.0);
        self.queued |= again;
        self.latched |= Buttons(fresh.0 & !again.0);
    }

    /// A key-up event for `buttons`.
    pub fn release(&mut self, buttons: Buttons) {
        self.held = Buttons(self.held.0 & !buttons.0);
    }

    /// Releases every key, for example when the window loses focus.
    pub fn release_all(&mut self) {
        self.held = Buttons::NONE;
    }

    /// Buttons held right now.
    pub fn held(&self) -> Buttons {
        self.held
    }

    /// The input of the next tick; clears the latch.
    pub fn next_tick(&mut self) -> TickInput {
        let pressed = self.latched;
        let input = TickInput {
            held: self.held | pressed,
            pressed,
        };
        self.latched = self.queued;
        self.queued = Buttons::NONE;
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tap_between_ticks_is_one_press_and_one_held_tick() {
        let mut latch = InputLatch::new();
        latch.press(Buttons::JUMP);
        latch.release(Buttons::JUMP);
        let t = latch.next_tick();
        assert_eq!(t.pressed, Buttons::JUMP);
        assert_eq!(t.held, Buttons::JUMP);
        let t = latch.next_tick();
        assert_eq!(t, TickInput::default());
    }

    #[test]
    fn a_held_key_presses_once() {
        let mut latch = InputLatch::new();
        latch.press(Buttons::RIGHT);
        latch.press(Buttons::RIGHT);
        let t = latch.next_tick();
        assert_eq!(t.pressed, Buttons::RIGHT);
        let t = latch.next_tick();
        assert_eq!(t.pressed, Buttons::NONE);
        assert_eq!(t.held, Buttons::RIGHT);
    }

    #[test]
    fn a_second_press_waits_for_the_next_tick() {
        let mut latch = InputLatch::new();
        latch.press(Buttons::JUMP);
        latch.release(Buttons::JUMP);
        latch.press(Buttons::JUMP);
        latch.release(Buttons::JUMP);
        assert_eq!(latch.next_tick().pressed, Buttons::JUMP);
        let t = latch.next_tick();
        assert_eq!(t.pressed, Buttons::JUMP);
        assert_eq!(t.held, Buttons::JUMP);
        assert_eq!(latch.next_tick(), TickInput::default());
    }
}
