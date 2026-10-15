//! Decoded samples and the named sound bank.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// A decoded sample: signed 16-bit frames at its own rate, interleaved when it
/// has two channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcm {
    /// Playback rate in Hz.
    pub rate: u32,
    /// 1 (mono) or 2 (stereo). Other counts are rejected at play time.
    pub channels: u8,
    /// Interleaved samples.
    pub samples: Arc<[i16]>,
}

impl Pcm {
    /// A mono sample.
    pub fn mono(rate: u32, samples: impl Into<Arc<[i16]>>) -> Pcm {
        Pcm {
            rate,
            channels: 1,
            samples: samples.into(),
        }
    }

    /// Number of frames.
    pub fn frames(&self) -> usize {
        match self.channels {
            0 => 0,
            c => self.samples.len() / usize::from(c),
        }
    }

    /// Whether the mixer can play it: one or two channels, a non-zero rate and
    /// at least one frame.
    pub fn is_playable(&self) -> bool {
        matches!(self.channels, 1 | 2) && self.rate > 0 && self.frames() > 0
    }

    /// Length in milliseconds at its own rate.
    pub fn duration_ms(&self) -> u64 {
        if self.rate == 0 {
            return 0;
        }
        self.frames() as u64 * 1000 / u64::from(self.rate)
    }
}

/// Index of a sound inside one [`SoundBank`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SoundId(pub u32);

/// Marker for "never played" in the throttle clock.
const NEVER: u64 = u64::MAX;

#[derive(Debug)]
struct Entry {
    key: String,
    pcm: Arc<Pcm>,
    /// Mixer time in ms of the last accepted play, or [`NEVER`].
    last_play: AtomicU64,
}

/// Sounds by name. The first insert of a name wins; later inserts of the same
/// name are ignored, as the game's sound registration does.
///
/// Names are compared exactly; callers normalise them before inserting and
/// playing.
///
/// The bank also holds the per-sound replay throttle clock, so the throttle
/// starts fresh with every new bank.
#[derive(Debug, Default)]
pub struct SoundBank {
    index: HashMap<String, u32>,
    entries: Vec<Entry>,
}

impl SoundBank {
    /// An empty bank.
    pub fn new() -> SoundBank {
        SoundBank::default()
    }

    /// Adds a sound under `key`. Returns false, keeping the first sound, when
    /// the name is already present.
    pub fn insert(&mut self, key: impl Into<String>, pcm: Arc<Pcm>) -> bool {
        let key = key.into();
        if self.index.contains_key(&key) {
            return false;
        }
        let id = self.entries.len() as u32;
        self.index.insert(key.clone(), id);
        self.entries.push(Entry {
            key,
            pcm,
            last_play: AtomicU64::new(NEVER),
        });
        true
    }

    /// The sound registered under `key`.
    pub fn get(&self, key: &str) -> Option<&Arc<Pcm>> {
        self.id(key).map(|id| &self.entries[id.0 as usize].pcm)
    }

    /// Index of the sound registered under `key`.
    pub fn id(&self, key: &str) -> Option<SoundId> {
        self.index.get(key).copied().map(SoundId)
    }

    /// The sound at `id`.
    pub fn by_id(&self, id: SoundId) -> Option<&Arc<Pcm>> {
        self.entries.get(id.0 as usize).map(|e| &e.pcm)
    }

    /// Name of the sound at `id`.
    pub fn key(&self, id: SoundId) -> Option<&str> {
        self.entries.get(id.0 as usize).map(|e| e.key.as_str())
    }

    /// Number of sounds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the bank has no sounds.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Replay throttle: returns false when the sound at `id` was accepted less
    /// than `gap_ms` ago (compared as unsigned 32-bit milliseconds, as the
    /// game does); otherwise records `now_ms` as its last play and returns
    /// true. A sound never played before always passes.
    pub(crate) fn throttle_pass(&self, id: SoundId, now_ms: u64, gap_ms: u32) -> bool {
        let Some(entry) = self.entries.get(id.0 as usize) else {
            return false;
        };
        let last = entry.last_play.load(Ordering::Relaxed);
        if last != NEVER && (now_ms as u32).wrapping_sub(last as u32) < gap_ms {
            return false;
        }
        entry.last_play.store(now_ms, Ordering::Relaxed);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm(n: usize) -> Arc<Pcm> {
        Arc::new(Pcm::mono(22050, vec![0i16; n]))
    }

    #[test]
    fn first_insert_wins() {
        let mut bank = SoundBank::new();
        assert!(bank.insert("GAME_CLICK", pcm(10)));
        assert!(!bank.insert("GAME_CLICK", pcm(20)));
        assert_eq!(bank.get("GAME_CLICK").unwrap().frames(), 10);
        assert_eq!(bank.len(), 1);
        assert_eq!(bank.id("GAME_CLICK"), Some(SoundId(0)));
        assert_eq!(bank.key(SoundId(0)), Some("GAME_CLICK"));
        assert!(bank.get("game_click").is_none());
    }

    #[test]
    fn pcm_shape() {
        let p = Pcm {
            rate: 1000,
            channels: 2,
            samples: vec![0i16; 2000].into(),
        };
        assert_eq!(p.frames(), 1000);
        assert_eq!(p.duration_ms(), 1000);
        assert!(p.is_playable());
        let bad = Pcm {
            rate: 1000,
            channels: 3,
            samples: vec![0i16; 3].into(),
        };
        assert!(!bad.is_playable());
        assert!(!Pcm::mono(0, vec![1i16]).is_playable());
        assert!(!Pcm::mono(8000, Vec::<i16>::new()).is_playable());
    }

    #[test]
    fn throttle_clock() {
        let mut bank = SoundBank::new();
        bank.insert("A", pcm(1));
        let a = SoundId(0);
        assert!(bank.throttle_pass(a, 0, 50));
        assert!(!bank.throttle_pass(a, 49, 50));
        assert!(bank.throttle_pass(a, 50, 50));
        assert!(!bank.throttle_pass(a, 60, 50));
        assert!(bank.throttle_pass(a, 60, 0));
        assert!(!bank.throttle_pass(SoundId(9), 0, 0));
    }
}
