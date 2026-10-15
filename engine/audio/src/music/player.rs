//! Playing the game's music tracks: the synthesizer on the audio thread and
//! the game-side handle that switches tracks.
//!
//! [`music`] builds a pair from a loaded [`Bank`] and the level's converted
//! tracks:
//!
//! - [`SynthMusic`], a [`MusicSource`] the mixer renders on the audio
//!   thread. It owns one synthesizer per track, so a paused track keeps its
//!   place, and renders the current one. It never allocates or blocks.
//! - [`MusicPlayer`], kept by the game, which sends it commands through a
//!   lock-free queue and applies the game's switching rules:
//!   - one track plays at a time; starting a track stops the one playing;
//!   - the power-up track pauses the level track, and asking for the level
//!     track again resumes it where it was instead of restarting it;
//!   - the Music switch: off stops the music, on restarts the current track
//!     from the start, looping.
//!
//! A looping track restarts from its first tick the moment it reaches its
//! end; a track started without looping plays once.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use rtrb::{Consumer, Producer, RingBuffer};
use rustysynth::{MidiFile, MidiFileSequencer, Synthesizer, SynthesizerSettings};

use super::convert::{BankMode, XmidiEvent, xmidi_to_smf};
use super::soundfont::Bank;
use crate::mixer::MusicSource;

/// A named music track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Track {
    /// The level's music.
    Level,
    /// The power-up music.
    Powerup,
    /// The level's boss music.
    Boss,
    /// The music of the level's map piece.
    Monolith,
    /// The main menu's chord, played after a music volume change.
    Chord,
    /// The credits music.
    Credits,
}

impl Track {
    /// Every track.
    pub const ALL: [Track; 6] = [
        Track::Level,
        Track::Powerup,
        Track::Boss,
        Track::Monolith,
        Track::Chord,
        Track::Credits,
    ];

    fn index(self) -> usize {
        self as usize
    }

    /// The track's name in the game's music registry.
    pub fn name(self) -> &'static str {
        match self {
            Track::Level => "LEVEL",
            Track::Powerup => "POWERUP",
            Track::Boss => "BOSS",
            Track::Monolith => "MONOLITH",
            Track::Chord => "CHORD",
            Track::Credits => "CREDITS",
        }
    }

    /// The music file names that can supply the track, most preferred first.
    /// With a General MIDI bank the General MIDI arrangement (the name ending
    /// in 0) comes first and the other one is the fallback; with the game's
    /// own instrument bank it is the other way round.
    pub fn file_names(self, mode: BankMode) -> &'static [&'static str] {
        let gm = mode == BankMode::GeneralMidi;
        match self {
            Track::Level if gm => &["PLAY0", "PLAY"],
            Track::Level => &["PLAY", "PLAY0"],
            Track::Powerup if gm => &["POWERUP0", "POWERUP"],
            Track::Powerup => &["POWERUP", "POWERUP0"],
            Track::Boss if gm => &["BOSS0", "BOSS"],
            Track::Boss => &["BOSS", "BOSS0"],
            Track::Monolith => &["MONOLITH"],
            Track::Chord => &["CHORD"],
            Track::Credits => &["PLAY"],
        }
    }
}

/// Picks the file for `track` among the names `available` (compared without
/// case): the first of [`Track::file_names`] present.
pub fn select_file<'a>(track: Track, mode: BankMode, available: &[&'a str]) -> Option<&'a str> {
    track.file_names(mode).iter().find_map(|want| {
        available
            .iter()
            .copied()
            .find(|have| have.eq_ignore_ascii_case(want))
    })
}

/// A converted track, ready for the synthesizer.
#[derive(Debug, Clone)]
pub struct Song {
    midi: Arc<MidiFile>,
    seconds: f64,
}

impl Song {
    /// Converts an XMIDI sequence (see [`super::convert`]).
    pub fn from_xmidi(events: &[XmidiEvent], mode: BankMode) -> Result<Song, String> {
        Song::from_smf(&xmidi_to_smf(events, mode))
    }

    /// Reads a Standard MIDI File.
    pub fn from_smf(smf: &[u8]) -> Result<Song, String> {
        let midi = MidiFile::new(&mut &smf[..]).map_err(|e| e.to_string())?;
        let seconds = midi.get_length();
        Ok(Song {
            midi: Arc::new(midi),
            seconds,
        })
    }

    /// Length of one pass in seconds.
    pub fn seconds(&self) -> f64 {
        self.seconds
    }
}

/// Rate of the synthesizers when the caller has no better one.
pub const DEFAULT_RATE: u32 = 48_000;

/// Frames the synthesizer renders per step.
const BLOCK: usize = 512;

/// Commands queued between the handle and the source.
const QUEUE: usize = 64;

/// Marker for "no track" in the shared status.
const NONE: u8 = u8::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy)]
enum Cmd {
    /// Stop the playing track (a paused one stays paused) and start `track`
    /// from its first tick.
    Start(Track, bool),
    /// Pause the current track.
    Pause,
    /// Stop the playing track and resume `track` where it was paused.
    Resume(Track),
    /// Stop everything.
    StopAll,
    /// Ramp the speed to `target` (1.0 is normal) over `ms`.
    Speed(f64, u32),
}

/// What the audio thread reports back.
#[derive(Debug)]
struct Status {
    /// The track sounding now, or [`NONE`].
    sounding: AtomicU8,
}

struct Slot {
    seq: MidiFileSequencer,
    song: Arc<MidiFile>,
    state: State,
    /// Rendered frames not played yet, kept across a pause so the track
    /// resumes exactly where it stopped.
    left: Box<[f32]>,
    right: Box<[f32]>,
    pos: usize,
}

/// The music synthesizer, rendered by the mixer on the audio thread. See the
/// module documentation.
pub struct SynthMusic {
    slots: [Option<Box<Slot>>; 6],
    current: Option<usize>,
    commands: Consumer<Cmd>,
    status: Arc<Status>,
    rate: u32,
    /// Resampler: the fraction between the previous and the next
    /// synthesizer frame.
    phase: f64,
    prev: (f32, f32),
    next: (f32, f32),
    speed: f64,
    speed_from: f64,
    speed_to: f64,
    ramp_frames: u64,
    ramp_done: u64,
}

impl fmt::Debug for SynthMusic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SynthMusic")
            .field("rate", &self.rate)
            .field("current", &self.current)
            .field("speed", &self.speed)
            .finish()
    }
}

/// Builds the synthesizer and its handle for `songs` with `bank`, rendering
/// at `rate` Hz (16000..=192000; output at another rate is resampled).
/// Allocates; call it off the audio thread.
pub fn music(
    bank: &Bank,
    songs: &[(Track, Song)],
    rate: u32,
) -> Result<(MusicPlayer, SynthMusic), String> {
    let rate = if (16_000..=192_000).contains(&rate) {
        rate
    } else {
        DEFAULT_RATE
    };
    let settings = SynthesizerSettings::new(rate as i32);
    let mut slots: [Option<Box<Slot>>; 6] = Default::default();
    let mut loaded = [false; 6];
    for (track, song) in songs {
        let mut synth = Synthesizer::new(&bank.font, &settings).map_err(|e| e.to_string())?;
        synth.set_master_volume(bank.gain);
        slots[track.index()] = Some(Box::new(Slot {
            seq: MidiFileSequencer::new(synth),
            song: Arc::clone(&song.midi),
            state: State::Idle,
            left: vec![0.0; BLOCK].into_boxed_slice(),
            right: vec![0.0; BLOCK].into_boxed_slice(),
            pos: BLOCK,
        }));
        loaded[track.index()] = true;
    }
    let (tx, rx) = RingBuffer::new(QUEUE);
    let status = Arc::new(Status {
        sounding: AtomicU8::new(NONE),
    });
    let source = SynthMusic {
        slots,
        current: None,
        commands: rx,
        status: Arc::clone(&status),
        rate,
        phase: 0.0,
        prev: (0.0, 0.0),
        next: (0.0, 0.0),
        speed: 1.0,
        speed_from: 1.0,
        speed_to: 1.0,
        ramp_frames: 0,
        ramp_done: 0,
    };
    let player = MusicPlayer {
        commands: tx,
        status,
        loaded,
        states: [State::Idle; 6],
        current: None,
        enabled: true,
        dropped: 0,
    };
    Ok((player, source))
}

impl SynthMusic {
    /// Synthesizer rate in Hz.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    fn stop_slot(&mut self, i: usize) {
        if let Some(s) = self.slots[i].as_mut() {
            s.seq.stop();
            s.state = State::Idle;
        }
    }

    fn switched(&mut self) {
        self.phase = 0.0;
        self.prev = (0.0, 0.0);
        self.next = (0.0, 0.0);
    }

    fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Start(track, looped) => {
                let i = track.index();
                if self.slots[i].is_none() {
                    return;
                }
                if let Some(c) = self.current
                    && c != i
                    && self.slots[c]
                        .as_ref()
                        .is_some_and(|s| s.state == State::Playing)
                {
                    self.stop_slot(c);
                }
                if let Some(s) = self.slots[i].as_mut() {
                    let song = Arc::clone(&s.song);
                    s.seq.play(&song, looped);
                    s.state = State::Playing;
                    s.pos = BLOCK;
                }
                self.current = Some(i);
                self.switched();
            }
            Cmd::Pause => {
                if let Some(c) = self.current
                    && let Some(s) = self.slots[c].as_mut()
                    && s.state == State::Playing
                {
                    s.state = State::Paused;
                }
            }
            Cmd::Resume(track) => {
                let i = track.index();
                if self.slots[i]
                    .as_ref()
                    .is_none_or(|s| s.state != State::Paused)
                {
                    return;
                }
                if let Some(c) = self.current
                    && c != i
                    && self.slots[c]
                        .as_ref()
                        .is_some_and(|s| s.state == State::Playing)
                {
                    self.stop_slot(c);
                }
                if let Some(s) = self.slots[i].as_mut() {
                    s.state = State::Playing;
                }
                self.current = Some(i);
                self.switched();
            }
            Cmd::StopAll => {
                for i in 0..self.slots.len() {
                    self.stop_slot(i);
                }
                self.current = None;
            }
            Cmd::Speed(target, ms) => {
                self.speed_from = self.speed;
                self.speed_to = target.max(0.0);
                self.ramp_frames = u64::from(ms) * u64::from(self.rate) / 1000;
                self.ramp_done = 0;
                if self.ramp_frames == 0 {
                    self.speed = self.speed_to;
                }
            }
        }
    }

    fn playing_slot(&mut self) -> Option<&mut Slot> {
        let c = self.current?;
        self.slots[c]
            .as_deref_mut()
            .filter(|s| s.state == State::Playing)
    }

    /// Advances the speed ramp by `frames` synthesizer frames.
    fn step_speed(&mut self, frames: usize) {
        if self.ramp_done < self.ramp_frames {
            self.ramp_done = (self.ramp_done + frames as u64).min(self.ramp_frames);
            let t = self.ramp_done as f64 / self.ramp_frames as f64;
            self.speed = self.speed_from + (self.speed_to - self.speed_from) * t;
        }
    }

    /// The next frame of the playing track, rendering a block when its
    /// buffer is used up; silence when nothing plays.
    fn next_frame(&mut self) -> (f32, f32) {
        let Some(c) = self.current else {
            return (0.0, 0.0);
        };
        let refill = match self.slots[c].as_deref() {
            Some(s) if s.state == State::Playing => s.pos >= BLOCK,
            _ => return (0.0, 0.0),
        };
        if refill {
            self.step_speed(BLOCK);
        }
        let speed = self.speed;
        let Some(s) = self.slots[c].as_deref_mut() else {
            return (0.0, 0.0);
        };
        if refill {
            if s.seq.get_speed() != speed {
                s.seq.set_speed(speed);
            }
            s.seq.render(&mut s.left, &mut s.right);
            s.pos = 0;
        }
        let f = (s.left[s.pos], s.right[s.pos]);
        s.pos += 1;
        f
    }

    fn publish(&self) {
        let sounding = self
            .current
            .and_then(|c| self.slots[c].as_ref().map(|s| (c, s)))
            .filter(|(_, s)| s.state == State::Playing && !s.seq.end_of_sequence())
            .map_or(NONE, |(c, _)| c as u8);
        self.status.sounding.store(sounding, Ordering::Release);
    }
}

impl MusicSource for SynthMusic {
    fn render(&mut self, out: &mut [f32], rate: u32) {
        while let Ok(cmd) = self.commands.pop() {
            self.apply(cmd);
        }
        if self.playing_slot().is_some() {
            if rate == self.rate {
                for frame in out.chunks_exact_mut(2) {
                    let (l, r) = self.next_frame();
                    frame[0] += l;
                    frame[1] += r;
                }
            } else {
                let step = f64::from(self.rate) / f64::from(rate.max(1));
                for frame in out.chunks_exact_mut(2) {
                    self.phase += step;
                    while self.phase >= 1.0 {
                        self.phase -= 1.0;
                        self.prev = self.next;
                        self.next = self.next_frame();
                    }
                    let t = self.phase as f32;
                    frame[0] += self.prev.0 + (self.next.0 - self.prev.0) * t;
                    frame[1] += self.prev.1 + (self.next.1 - self.prev.1) * t;
                }
            }
        }
        self.publish();
    }
}

/// The game's handle on the music: see the module documentation.
pub struct MusicPlayer {
    commands: Producer<Cmd>,
    status: Arc<Status>,
    loaded: [bool; 6],
    states: [State; 6],
    current: Option<Track>,
    enabled: bool,
    dropped: u64,
}

impl fmt::Debug for MusicPlayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MusicPlayer")
            .field("current", &self.current)
            .field("enabled", &self.enabled)
            .field("sounding", &self.sounding())
            .finish()
    }
}

impl MusicPlayer {
    fn send(&mut self, cmd: Cmd) {
        if self.commands.push(cmd).is_err() {
            self.dropped += 1;
        }
    }

    /// Whether `track` was loaded.
    pub fn has(&self, track: Track) -> bool {
        self.loaded[track.index()]
    }

    /// The current track: the one playing, or the one the Music switch
    /// restarts.
    pub fn current(&self) -> Option<Track> {
        self.current
    }

    /// The track the synthesizer is playing now; `None` when nothing plays,
    /// a track without looping has ended, or the commands have not reached
    /// the audio thread yet.
    pub fn sounding(&self) -> Option<Track> {
        Track::ALL
            .get(usize::from(self.status.sounding.load(Ordering::Acquire)))
            .copied()
    }

    /// Whether the Music switch is on.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Commands lost because the queue was full.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Starts `track`, following the game's rules:
    ///
    /// - the level track resumes where it was when it is paused, and keeps
    ///   playing when it already is; otherwise it starts;
    /// - the power-up track pauses the level track when that is playing, and
    ///   always starts from the beginning;
    /// - any other track starts from the beginning.
    ///
    /// Starting a track stops the one playing. Nothing happens for a track
    /// that was not loaded or while the Music switch is off.
    pub fn play(&mut self, track: Track, looped: bool) {
        if !self.enabled || !self.has(track) {
            return;
        }
        let i = track.index();
        match track {
            Track::Level if self.states[i] == State::Paused => {
                self.stop_playing_except(i);
                self.send(Cmd::Resume(track));
            }
            Track::Level if self.current == Some(track) && self.states[i] == State::Playing => {
                return;
            }
            Track::Powerup => {
                let level = Track::Level.index();
                if self.current == Some(Track::Level) && self.states[level] == State::Playing {
                    self.send(Cmd::Pause);
                    self.states[level] = State::Paused;
                }
                self.stop_playing_except(i);
                self.send(Cmd::Start(track, looped));
            }
            _ => {
                self.stop_playing_except(i);
                self.send(Cmd::Start(track, looped));
            }
        }
        self.states[i] = State::Playing;
        self.current = Some(track);
    }

    /// Marks the playing track other than `keep` as stopped (the source does
    /// the same when the next command arrives).
    fn stop_playing_except(&mut self, keep: usize) {
        for (i, s) in self.states.iter_mut().enumerate() {
            if i != keep && *s == State::Playing {
                *s = State::Idle;
            }
        }
    }

    /// Restarts the current track from its beginning (the level track when
    /// none was played yet).
    pub fn restart(&mut self, looped: bool) {
        if !self.enabled {
            return;
        }
        let track = self.current.unwrap_or(Track::Level);
        if !self.has(track) {
            return;
        }
        self.stop_playing_except(track.index());
        self.send(Cmd::Start(track, looped));
        self.states[track.index()] = State::Playing;
        self.current = Some(track);
    }

    /// Stops all music. The current track is kept for the Music switch.
    pub fn stop(&mut self) {
        self.send(Cmd::StopAll);
        self.states = [State::Idle; 6];
    }

    /// The Music switch. Off stops the music; on restarts the current track
    /// (or the level track) from the beginning, looping.
    pub fn set_enabled(&mut self, on: bool) {
        if on == self.enabled {
            return;
        }
        if on {
            self.enabled = true;
            self.restart(true);
        } else {
            self.stop();
            self.enabled = false;
        }
    }

    /// Changes the playback speed to `percent` (100 is normal) over
    /// `ramp_ms`.
    pub fn set_speed(&mut self, percent: u16, ramp_ms: u32) {
        self.send(Cmd::Speed(f64::from(percent) / 100.0, ramp_ms));
    }
}
