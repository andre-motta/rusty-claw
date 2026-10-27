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
//! - a per-sound replay throttle (its gap is 0 by default, as in the game), a
//!   voice pool that reuses free voices, loops with handles, channel volumes
//!   for effects, speech, ambient loops and the interface
//!   ([`settings::channel_volume`]), and the voice, ambient and music
//!   switches.
//!
//! One product choice differs from the game and is documented where it
//! lives: a voice cap ([`AudioSettings::voice_cap`]).
//!
//! [`Mixer`] is the deterministic, allocation-free mixer and can be driven
//! offline. [`AudioEngine`] runs it on an output device on its own thread, falls
//! back to a silent output when there is no device or audio is off, and takes
//! music through [`MusicSource`], as well as a movie's soundtrack
//! ([`AudioEngine::set_movie`]), mixed at the sound volume.
//!
//! [`music`] plays the game's music: it converts XMIDI sequences to Standard
//! MIDI Files, finds and loads a General MIDI SoundFont, and synthesizes the
//! tracks on the audio thread ([`music::SynthMusic`]) under the game's
//! switching rules ([`music::MusicPlayer`]).
//!
//! The crate knows nothing about game files: callers decode WAV data into
//! [`Pcm`], XMIDI files into [`music::XmidiEvent`]s, and map game cues to
//! [`PlayRequest`]s.

#![deny(unsafe_code)]

pub mod bank;
pub mod curve;
mod device;
mod engine;
pub mod mixer;
pub mod music;
pub mod settings;
mod state;

#[cfg(test)]
mod alloc_tests;

pub use bank::{Pcm, SoundBank, SoundId};
pub use engine::{AudioEngine, AudioStats, Output};
pub use mixer::{Mixer, MusicSource, PlayParams, VoiceId};
pub use settings::{AudioSettings, Channel, PlayRequest, channel_volume};
