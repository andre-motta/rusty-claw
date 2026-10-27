//! A movie's soundtrack as the audio engine plays it, and the clock it
//! keeps.
//!
//! The decoder writes interleaved stereo `f32` samples at the rate it was
//! opened with into a lock-free ring; [`MovieAudio`] reads them on the
//! audio output thread as a [`MusicSource`] and counts the frames it
//! takes, which is the movie's clock. When the output asks for another
//! rate (a device opened at another rate after the movie started), it
//! follows by linear interpolation; the clock still counts the movie's own
//! frames.

// Only the player, built where FFmpeg is linked, feeds a soundtrack.
#![cfg_attr(not(claw_ffmpeg), allow(dead_code))]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::time::Duration;

use claw_audio::MusicSource;
use rtrb::{Consumer, Producer, RingBuffer};

/// The soundtrack has not been handed out yet.
const UNTAKEN: u8 = 0;
/// The soundtrack is with an audio engine.
const LIVE: u8 = 1;
/// The soundtrack was dropped.
const GONE: u8 = 2;

/// What the decoder, the soundtrack and the player's clock share.
#[derive(Debug)]
pub(crate) struct AudioLink {
    /// The rate the decoder writes at.
    pub rate: u32,
    /// Frames the soundtrack has taken from the ring.
    consumed: AtomicU64,
    state: AtomicU8,
    /// The decoder wrote its last samples.
    ended: AtomicBool,
    /// The soundtrack took every sample after the decoder ended.
    drained: AtomicBool,
    /// When the first sample plays, from the movie's start, in
    /// microseconds.
    start_us: AtomicU64,
    /// The player's clock left the soundtrack for the wall clock.
    wall: AtomicBool,
}

impl AudioLink {
    pub(crate) fn new(rate: u32) -> AudioLink {
        AudioLink {
            rate: rate.max(1),
            consumed: AtomicU64::new(0),
            state: AtomicU8::new(UNTAKEN),
            ended: AtomicBool::new(false),
            drained: AtomicBool::new(false),
            start_us: AtomicU64::new(0),
            wall: AtomicBool::new(false),
        }
    }

    /// The movie time of the next sample the soundtrack plays.
    pub(crate) fn position(&self) -> Duration {
        let frames = self.consumed.load(Ordering::Acquire);
        Duration::from_micros(self.start_us.load(Ordering::Relaxed))
            + Duration::from_secs_f64(frames as f64 / f64::from(self.rate))
    }

    /// The soundtrack is with an audio engine.
    pub(crate) fn live(&self) -> bool {
        self.state.load(Ordering::Acquire) == LIVE
    }

    /// The soundtrack played everything the decoder wrote.
    pub(crate) fn drained(&self) -> bool {
        self.drained.load(Ordering::Acquire)
    }

    pub(crate) fn set_start(&self, start: Duration) {
        self.start_us
            .store(start.as_micros() as u64, Ordering::Relaxed);
    }

    pub(crate) fn set_ended(&self) {
        self.ended.store(true, Ordering::Release);
    }

    pub(crate) fn set_wall(&self) {
        self.wall.store(true, Ordering::Release);
    }

    /// Whether the decoder should wait for room in a full ring rather than
    /// drop samples: only while an engine plays them on the soundtrack's
    /// clock.
    pub(crate) fn worth_waiting(&self) -> bool {
        self.live() && !self.wall.load(Ordering::Acquire)
    }

    fn took(&self, frames: u64) {
        if frames > 0 {
            self.consumed.fetch_add(frames, Ordering::AcqRel);
        }
    }
}

/// A ring of `seconds` of stereo samples at `link.rate`, the decoder's
/// end and the soundtrack.
pub(crate) fn ring(link: &Arc<AudioLink>, seconds: u32) -> (Producer<f32>, MovieAudio) {
    let cap = link.rate as usize * 2 * seconds.max(1) as usize;
    let (tx, rx) = RingBuffer::new(cap);
    let audio = MovieAudio {
        ring: rx,
        link: Arc::clone(link),
        prev: [0.0; 2],
        next: [0.0; 2],
        frac: 1.0,
    };
    (tx, audio)
}

/// A movie's soundtrack: plays the decoded samples and counts them as the
/// movie's clock (see the module documentation). Dropping it tells the
/// player, whose clock then runs on the wall clock.
pub struct MovieAudio {
    ring: Consumer<f32>,
    link: Arc<AudioLink>,
    /// The two source frames being interpolated, and the position between
    /// them, when the output rate differs.
    prev: [f32; 2],
    next: [f32; 2],
    frac: f64,
}

impl std::fmt::Debug for MovieAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieAudio")
            .field("rate", &self.link.rate)
            .field("buffered", &(self.ring.slots() / 2))
            .finish_non_exhaustive()
    }
}

impl MovieAudio {
    /// Marks the soundtrack as handed to an audio engine.
    pub(crate) fn mark_live(&self) {
        self.link.state.store(LIVE, Ordering::Release);
    }

    fn pop_frame(&mut self) -> Option<[f32; 2]> {
        if self.ring.slots() < 2 {
            return None;
        }
        let l = self.ring.pop().ok()?;
        let r = self.ring.pop().ok()?;
        Some([l, r])
    }

    fn note_end(&self) {
        if self.ring.slots() < 2 && self.link.ended.load(Ordering::Acquire) {
            self.link.drained.store(true, Ordering::Release);
        }
    }
}

impl MusicSource for MovieAudio {
    fn render(&mut self, out: &mut [f32], rate: u32) {
        let frames = out.len() / 2;
        if rate == self.link.rate {
            let n = (self.ring.slots() / 2).min(frames);
            if let Ok(chunk) = self.ring.read_chunk(n * 2) {
                let (a, b) = chunk.as_slices();
                out[..a.len()].copy_from_slice(a);
                out[a.len()..a.len() + b.len()].copy_from_slice(b);
                chunk.commit_all();
            }
            self.link.took(n as u64);
        } else {
            let step = f64::from(self.link.rate) / f64::from(rate.max(1));
            let mut took = 0u64;
            'out: for frame in out.chunks_exact_mut(2) {
                while self.frac >= 1.0 {
                    match self.pop_frame() {
                        Some(f) => {
                            self.prev = self.next;
                            self.next = f;
                            self.frac -= 1.0;
                            took += 1;
                        }
                        None => break 'out,
                    }
                }
                let t = self.frac as f32;
                frame[0] = self.prev[0] + (self.next[0] - self.prev[0]) * t;
                frame[1] = self.prev[1] + (self.next[1] - self.prev[1]) * t;
                self.frac += step;
            }
            self.link.took(took);
        }
        self.note_end();
    }
}

impl Drop for MovieAudio {
    fn drop(&mut self) {
        self.link.state.store(GONE, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push(tx: &mut Producer<f32>, frames: &[[f32; 2]]) {
        for f in frames {
            tx.push(f[0]).unwrap();
            tx.push(f[1]).unwrap();
        }
    }

    #[test]
    fn at_its_own_rate_the_soundtrack_plays_its_samples_and_counts_them() {
        let link = Arc::new(AudioLink::new(1000));
        let (mut tx, mut audio) = ring(&link, 1);
        audio.mark_live();
        link.set_start(Duration::from_millis(500));
        push(&mut tx, &[[0.1, -0.1], [0.2, -0.2], [0.3, -0.3]]);
        let mut out = vec![0.0f32; 4];
        audio.render(&mut out, 1000);
        assert_eq!(out, [0.1, -0.1, 0.2, -0.2]);
        assert_eq!(link.position(), Duration::from_millis(502));
        // An underrun plays silence and does not move the clock.
        let mut out = vec![0.0f32; 6];
        audio.render(&mut out, 1000);
        assert_eq!(out, [0.3, -0.3, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(link.position(), Duration::from_millis(503));
        assert!(!link.drained());
        link.set_ended();
        audio.render(&mut out, 1000);
        assert!(link.drained());
    }

    #[test]
    fn at_another_rate_the_soundtrack_is_interpolated_and_the_clock_counts_its_own_frames() {
        let link = Arc::new(AudioLink::new(1000));
        let (mut tx, mut audio) = ring(&link, 1);
        let frames: Vec<[f32; 2]> = (0..100).map(|i| [i as f32, -(i as f32)]).collect();
        push(&mut tx, &frames);
        // Twice the rate: every source frame gives two output frames.
        let mut out = vec![0.0f32; 2 * 100];
        audio.render(&mut out, 2000);
        assert_eq!(link.position(), Duration::from_millis(50));
        assert_eq!(&out[4..10], &[0.0, 0.0, 0.5, -0.5, 1.0, -1.0]);
        for w in out.chunks_exact(2).collect::<Vec<_>>().windows(2) {
            assert!(w[1][0] >= w[0][0], "rising ramp");
        }
    }

    #[test]
    fn dropping_the_soundtrack_tells_the_player() {
        let link = Arc::new(AudioLink::new(48000));
        let (_tx, audio) = ring(&link, 1);
        assert!(!link.live() && !link.worth_waiting());
        audio.mark_live();
        assert!(link.live() && link.worth_waiting());
        link.set_wall();
        assert!(!link.worth_waiting());
        drop(audio);
        assert!(!link.live());
    }
}
