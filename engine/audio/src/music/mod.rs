//! Music: XMIDI conversion, the SoundFont and the synthesizer.
//!
//! - [`convert`] turns the game's XMIDI sequences into Standard MIDI Files.
//! - [`soundfont`] finds, checks and loads the General MIDI bank.
//! - [`player`] plays converted tracks through a synthesizer on the audio
//!   thread ([`SynthMusic`]) and switches them from the game
//!   ([`MusicPlayer`]).

pub mod convert;
pub mod player;
pub mod soundfont;

pub use convert::{BankMode, XmidiEvent, XmidiKind, xmidi_to_smf};
pub use player::{MusicPlayer, Song, SynthMusic, Track, music, select_file};
pub use soundfont::Bank;
