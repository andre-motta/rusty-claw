//! Sound effects, the output device and the music input.
//!
//! The crate plays decoded samples ([`Pcm`]) by name from a [`SoundBank`] the
//! way the game does:
//!
//! - volume: a request's volume (0..=100) is taken relative to the configured
//!   sound volume, then goes through the game's curve, 10 dB per halving
//!   ([`curve::attenuation`]);
//! - pan: the same curve read backwards attenuates the opposite channel
//!   ([`curve::pan_value`]);
//! - pitch: a percent change of the sample's rate, clamped to 101..=99999 Hz
//!   with the game's unsigned comparison ([`curve::pitch_frequency`]);
//! - a per-sound replay throttle, a voice pool that reuses free voices, loops
//!   with handles, and channel switches for voice and ambient sounds.
//!
//! Two product choices differ from the game and are documented where they
//! live: a voice cap ([`AudioSettings::voice_cap`]) and a provisional replay
//! gap ([`settings::PROVISIONAL_MIN_REPLAY_GAP_MS`]).
//!
//! [`Mixer`] is the deterministic, allocation-free mixer and can be driven
//! offline. [`AudioEngine`] runs it on an output device on its own thread, falls
//! back to a silent output when there is no device, and takes music through
//! [`MusicSource`].
//!
//! The crate knows nothing about game files: callers decode WAV data into
//! [`Pcm`] and map game cues to [`PlayRequest`]s.

#![deny(unsafe_code)]

pub mod bank;
pub mod curve;
mod device;
mod engine;
pub mod mixer;
pub mod settings;
mod state;

#[cfg(test)]
mod alloc_tests;

pub use bank::{Pcm, SoundBank, SoundId};
pub use engine::{AudioEngine, AudioStats, Output};
pub use mixer::{Mixer, MusicSource, PlayParams, VoiceId};
pub use settings::{AudioSettings, Channel, PlayRequest};
