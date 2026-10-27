//! The movie's clock: the soundtrack's position while an audio engine plays
//! it, otherwise the wall clock.
//!
//! The clock starts at the first reading. It follows the soundtrack when
//! one was handed to an audio engine by then; it moves to the wall clock,
//! from the soundtrack's last position and the moment it last moved, when
//! the soundtrack is dropped, has played all its samples, or stands still
//! for more than [`STALL`] (an audio device lost or reopened, or a decoder
//! that fell behind). Once on the wall clock it stays there. A movie
//! without sound, or played without an audio engine, runs on the wall
//! clock from the start.

// Only the player, built with the `ffmpeg` feature, feeds a soundtrack.
#![cfg_attr(not(feature = "ffmpeg"), allow(dead_code))]

use std::cell::Cell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::audio::AudioLink;

/// How long the soundtrack may stand still before the clock leaves it.
pub const STALL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Not read yet.
    Idle,
    /// On the soundtrack, last at `last`, which it reached at `changed`.
    Audio { last: Duration, changed: Instant },
    /// On the wall clock: `base` at `since`.
    Wall { base: Duration, since: Instant },
}

/// A movie's clock (see the module documentation).
#[derive(Debug)]
pub struct Clock {
    link: Option<Arc<AudioLink>>,
    state: Cell<State>,
}

impl Clock {
    /// A clock on the wall clock only.
    pub fn wall() -> Clock {
        Clock {
            link: None,
            state: Cell::new(State::Idle),
        }
    }

    /// Follows `link` from the first reading on, when it is live then. A
    /// clock already on the wall clock stays there.
    pub(crate) fn follow(&mut self, link: Arc<AudioLink>) {
        if self.on_wall() {
            link.set_wall();
        }
        self.link = Some(link);
    }

    /// Whether the clock has left the soundtrack (or never had one).
    pub fn on_wall(&self) -> bool {
        matches!(self.state.get(), State::Wall { .. })
    }

    /// The movie time at `at`.
    pub fn at(&self, at: Instant) -> Duration {
        let live = self.link.as_ref().filter(|l| l.live());
        let next = match (self.state.get(), live) {
            (State::Idle, Some(link)) => State::Audio {
                last: link.position(),
                changed: at,
            },
            (State::Idle, None) => State::Wall {
                base: Duration::ZERO,
                since: at,
            },
            (State::Audio { last, changed }, Some(link)) if !link.drained() => {
                let now = link.position();
                if now != last {
                    State::Audio {
                        last: now,
                        changed: at,
                    }
                } else if at.saturating_duration_since(changed) > STALL {
                    State::Wall {
                        base: last,
                        since: changed,
                    }
                } else {
                    State::Audio { last, changed }
                }
            }
            (State::Audio { last, changed }, _) => State::Wall {
                base: last,
                since: changed,
            },
            (wall @ State::Wall { .. }, _) => wall,
        };
        if let (State::Wall { .. }, Some(link)) = (next, &self.link)
            && !matches!(self.state.get(), State::Wall { .. })
        {
            link.set_wall();
        }
        self.state.set(next);
        match next {
            State::Idle => Duration::ZERO,
            State::Audio { last, .. } => last,
            State::Wall { base, since } => base + at.saturating_duration_since(since),
        }
    }

    /// The movie time now.
    pub fn now(&self) -> Duration {
        self.at(Instant::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::ring;
    use claw_audio::MusicSource;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn without_a_soundtrack_the_clock_is_the_wall_clock_from_its_first_reading() {
        let clock = Clock::wall();
        let t0 = Instant::now() + Duration::from_secs(5);
        assert_eq!(clock.at(t0), Duration::ZERO);
        assert!(clock.on_wall());
        assert_eq!(clock.at(t0 + 40 * MS), 40 * MS);
        assert_eq!(clock.at(t0 + 1000 * MS), 1000 * MS);
    }

    #[test]
    fn a_soundtrack_not_handed_to_an_engine_leaves_the_clock_on_the_wall() {
        let link = Arc::new(AudioLink::new(1000));
        let (_tx, _audio) = ring(&link, 1);
        let mut clock = Clock::wall();
        clock.follow(Arc::clone(&link));
        let t0 = Instant::now();
        assert_eq!(clock.at(t0), Duration::ZERO);
        assert_eq!(clock.at(t0 + 10 * MS), 10 * MS);
        assert!(clock.on_wall());
    }

    #[test]
    fn the_clock_follows_the_soundtrack_and_leaves_it_when_it_stalls() {
        let link = Arc::new(AudioLink::new(1000));
        let (mut tx, mut audio) = ring(&link, 1);
        audio.mark_live();
        for _ in 0..2000 {
            let _ = tx.push(0.0);
        }
        let mut clock = Clock::wall();
        clock.follow(Arc::clone(&link));
        let t0 = Instant::now();
        assert_eq!(clock.at(t0), Duration::ZERO);
        let mut out = vec![0.0f32; 2 * 30];
        audio.render(&mut out, 1000);
        // The soundtrack, not the wall: 30 ms played, whatever the time.
        assert_eq!(clock.at(t0 + 5 * MS), 30 * MS);
        audio.render(&mut out, 1000);
        assert_eq!(clock.at(t0 + 100 * MS), 60 * MS);
        // Standing still up to the limit holds the clock.
        assert_eq!(clock.at(t0 + 300 * MS), 60 * MS);
        assert!(!clock.on_wall());
        // Past it, the wall clock runs on from where the soundtrack stopped.
        assert_eq!(clock.at(t0 + 301 * MS), 261 * MS);
        assert!(clock.on_wall());
        assert!(!link.worth_waiting(), "the decoder stops waiting for it");
        // Even when the soundtrack moves again.
        audio.render(&mut out, 1000);
        assert_eq!(clock.at(t0 + 400 * MS), 360 * MS);
    }

    #[test]
    fn a_dropped_or_finished_soundtrack_hands_over_to_the_wall_at_once() {
        for finish in [false, true] {
            let link = Arc::new(AudioLink::new(1000));
            let (mut tx, mut audio) = ring(&link, 1);
            audio.mark_live();
            for _ in 0..40 {
                let _ = tx.push(0.0);
            }
            let mut clock = Clock::wall();
            clock.follow(Arc::clone(&link));
            let t0 = Instant::now();
            clock.at(t0);
            let mut out = vec![0.0f32; 2 * 20];
            audio.render(&mut out, 1000);
            assert_eq!(clock.at(t0 + 15 * MS), 20 * MS);
            if finish {
                link.set_ended();
                audio.render(&mut out, 1000);
                assert!(link.drained());
            } else {
                drop(audio);
            }
            // From the soundtrack's last position at the time it last moved.
            assert_eq!(clock.at(t0 + 50 * MS), 55 * MS, "finished {finish}");
            assert!(clock.on_wall());
        }
    }
}
