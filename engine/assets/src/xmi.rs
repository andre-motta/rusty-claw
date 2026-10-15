//! XMIDI music (to be filled in by the music reader).
//!
//! The game's music tracks are XMIDI files: an IFF container holding one or
//! more sequences, each with its instrument list and a stream of MIDI
//! events timed in 120 Hz ticks. The reader will turn them into
//! `Xmi { sequences }`, each sequence a list of timed events, for
//! conversion to standard MIDI and playback through a General MIDI
//! synthesizer. Like every reader in this crate it will be bounded, never
//! panic and treat its input as untrusted.
