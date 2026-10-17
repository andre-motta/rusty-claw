//! Sound and music: the bridge between the game's sound cues and the audio
//! engine.
//!
//! When a level loads, [`AudioBridge::load_level`] decodes every sound the
//! level registers (its own and the shared ones) into samples and hands
//! them to the engine as one sound bank, so nothing is read or decoded
//! while the game runs. It also converts the level's music tracks (level,
//! boss, power-up, map piece, and the menu chord) and starts the level
//! music, as the game does once a level has loaded. Each frame the window
//! passes the sound cues of every tick it ran to [`AudioBridge::apply`],
//! which plays them.
//!
//! Sound rules:
//!
//! - each cue's channel plays at its configured volume: effects and the
//!   interface at the sound volume, speech a little louder, ambient loops a
//!   little softer (see [`claw_audio::channel_volume`]); the Voice and
//!   Ambient switches silence their channels;
//! - a looping cue with a handle that is already playing updates its volume
//!   and balance; such loops come back at once when their switch (or sound
//!   as a whole) is turned on again;
//! - pausing the game (or losing focus) stops every effect, speech and
//!   ambient sound, holds the music where it is and keeps interface sounds
//!   playing; resuming starts the live ambient loops again at their current
//!   level, while the effects and lines the pause stopped stay stopped;
//! - leaving a level ([`AudioBridge::leave_level`]) ends its ambient loops
//!   for good, paused or not;
//! - the music can also be held on its own ([`AudioBridge::hold_music`]):
//!   it stops where it is while every sound plays on, and carries on from
//!   there when let go. Play running with the pause key's flag still set
//!   after a help screen holds it this way.
//!
//! Music rules: one track at a time, looping as the cue asks; the power-up
//! track pauses the level track, which resumes where it was when the level
//! track is asked for again; the Music switch pauses the current track and,
//! turned on, restarts it. While the game is paused the switch is only
//! recorded: the resume pauses or restarts the current track when the
//! switch changed, and otherwise lets the music carry on. The music stops
//! when the level is won ([`AudioBridge::events`]).
//!
//! Boss fights: the boss track starts when the game asks for it (as soon
//! as the stage of a boss level wakes, intro skipped or not) and plays on
//! through the boss's defeat and the level's end; nothing switches back to
//! the level track. While the boss fight flag is set (from the
//! introduction's start, see [`AudioBridge::set_boss_fight`]) a power-up
//! neither swaps in its track nor, when it ends, the level track: the cues
//! are dropped, as the game skips its power-up music then. After an intro
//! skip the flag stays clear, so a power-up swaps the boss track out as
//! anywhere else in the level. Music needs a General
//! MIDI SoundFont (see [`claw_audio::music::soundfont`]); without one,
//! music is off, one message says how to get a bank, and sound effects are
//! unaffected.
//!
//! Only the window's normal play opens an audio device. The headless
//! benchmark plays through a silent output (with music, so its mixing and
//! synthesis run); replays and the window benchmark create no bridge.

use std::collections::BTreeMap;
use std::time::Instant;

use claw_assets::name::TYPE_XMI;
use claw_assets::xmi::{self, XmiEventKind};
use claw_assets::{AssetStore, FileId, Registries, wav};
use claw_audio::music::player::DEFAULT_RATE;
use claw_audio::music::soundfont::{self, Places};
use claw_audio::music::{
    Bank, BankMode, MusicPlayer, Song, Track, XmidiEvent, XmidiKind, convert, music, select_file,
};
use claw_audio::{AudioEngine, AudioSettings, Channel, Pcm, PlayRequest, SoundBank};
use claw_frame::{AudioCue, GameEvent, MusicTrack, SoundChannel, SoundCue};

/// The part of the level in view, for placing sounds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewInfo {
    /// Left edge in level pixels.
    pub left: i32,
    /// Top edge in level pixels.
    pub top: i32,
    /// Width in level pixels.
    pub width: u32,
    /// Height in level pixels.
    pub height: u32,
}

/// What loading a level's sounds and music did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BankReport {
    /// Sounds the level registers.
    pub registered: usize,
    /// Sounds decoded into the bank.
    pub decoded: usize,
    /// Sounds that could not be read or decoded, with the reason.
    pub failed: Vec<(String, String)>,
    /// Decoded samples, in bytes.
    pub bytes: usize,
    /// Music tracks ready to play (0 when music is off).
    pub music_tracks: usize,
    /// Music tracks that could not be read or converted, with the reason.
    pub music_failed: Vec<(String, String)>,
    /// Time spent, ms.
    pub ms: f64,
}

/// Decodes a WAV sound file into samples.
pub fn decode_sound(data: &[u8]) -> Result<Pcm, String> {
    let w = wav::parse(data).map_err(|e| e.to_string())?;
    let channels = u8::try_from(w.channels)
        .ok()
        .filter(|c| matches!(c, 1 | 2))
        .ok_or_else(|| format!("{} channels", w.channels))?;
    let pcm = Pcm {
        rate: w.rate,
        channels,
        samples: w.samples.into(),
    };
    if !pcm.is_playable() {
        return Err("no samples".to_string());
    }
    Ok(pcm)
}

/// Decodes every sound in `registries` from `store` into a bank.
pub fn decode_bank(store: &AssetStore, registries: &Registries) -> (SoundBank, BankReport) {
    let t = Instant::now();
    let mut bank = SoundBank::new();
    let mut report = BankReport {
        registered: registries.sounds.len(),
        ..BankReport::default()
    };
    for (key, &file) in &registries.sounds {
        let pcm = store
            .read(file)
            .map_err(|e| e.to_string())
            .and_then(|data| decode_sound(&data));
        match pcm {
            Ok(pcm) => {
                report.bytes += pcm.samples.len() * 2;
                if bank.insert(key.clone(), pcm.into()) {
                    report.decoded += 1;
                }
            }
            Err(e) => report.failed.push((key.clone(), e)),
        }
    }
    report.ms = t.elapsed().as_secs_f64() * 1000.0;
    (bank, report)
}

/// The engine channel of a cue's channel.
pub fn channel(c: SoundChannel) -> Channel {
    match c {
        SoundChannel::Effect => Channel::Effect,
        SoundChannel::Voice => Channel::Voice,
        SoundChannel::Ambient => Channel::Ambient,
        SoundChannel::Interface => Channel::Interface,
    }
}

/// The engine request of a sound cue.
pub fn request(cue: &SoundCue) -> PlayRequest {
    PlayRequest {
        key: cue.key.clone(),
        volume: cue.volume,
        pan: cue.pan,
        pitch: cue.pitch,
        looped: cue.looped,
        channel: channel(cue.channel),
        handle: cue.voice,
    }
}

/// The music track of a cue's track.
pub fn track(t: MusicTrack) -> Track {
    match t {
        MusicTrack::Level => Track::Level,
        MusicTrack::Powerup => Track::Powerup,
        MusicTrack::Boss => Track::Boss,
        MusicTrack::Chord => Track::Chord,
    }
}

/// The events of an XMIDI sequence as the music converter takes them.
pub fn xmidi_events(seq: &xmi::XmiSequence) -> Vec<XmidiEvent> {
    seq.events
        .iter()
        .map(|e| {
            let kind = match e.kind {
                XmiEventKind::Note {
                    channel,
                    key,
                    velocity,
                    duration,
                } => XmidiKind::Note {
                    channel,
                    key,
                    velocity,
                    duration,
                },
                XmiEventKind::KeyPressure {
                    channel,
                    key,
                    pressure,
                } => XmidiKind::KeyPressure {
                    channel,
                    key,
                    pressure,
                },
                XmiEventKind::Controller {
                    channel,
                    controller,
                    value,
                } => XmidiKind::Controller {
                    channel,
                    controller,
                    value,
                },
                XmiEventKind::Program { channel, program } => {
                    XmidiKind::Program { channel, program }
                }
                XmiEventKind::ChannelPressure { channel, pressure } => {
                    XmidiKind::ChannelPressure { channel, pressure }
                }
                XmiEventKind::PitchBend { channel, value } => {
                    XmidiKind::PitchBend { channel, value }
                }
                XmiEventKind::EndOfTrack => XmidiKind::EndOfTrack,
                XmiEventKind::NoteOff { .. }
                | XmiEventKind::Tempo { .. }
                | XmiEventKind::Meta { .. }
                | XmiEventKind::SysEx { .. } => XmidiKind::Other,
            };
            XmidiEvent::new(e.time_120hz, kind)
        })
        .collect()
}

/// Reads an XMIDI file's first sequence (the only one the game plays) as
/// converter events.
pub fn read_music(data: &[u8]) -> Result<Vec<XmidiEvent>, String> {
    let x = xmi::parse(data).map_err(|e| e.to_string())?;
    let seq = x.sequences.first().ok_or("no sequence")?;
    Ok(xmidi_events(seq))
}

/// Folders searched for the menu chord, in order.
pub const MENU_MUSIC_DIRS: &[&str] = &["STATES/MENU/MUSIC", "GAME/MUSIC"];

/// The music file of `track`: for the chord, the menu's music folder; for
/// the other tracks, the level's registered file, or the arrangement next to
/// it that suits the bank (see [`Track::file_names`]).
pub fn music_file(
    store: &AssetStore,
    registries: &Registries,
    track: Track,
    mode: BankMode,
) -> Option<FileId> {
    let pick = |dir: &str| -> Option<FileId> {
        let node = store.folder(dir).ok().flatten()?;
        let ids: Vec<FileId> = node.files_of_type(TYPE_XMI).collect();
        let names: Vec<&str> = ids.iter().map(|&id| store.file(id).name()).collect();
        let name = select_file(track, mode, &names)?;
        ids.iter()
            .copied()
            .find(|&id| store.file(id).name() == name)
    };
    if track == Track::Chord {
        return MENU_MUSIC_DIRS.iter().find_map(|d| pick(d));
    }
    let id = *registries.music.get(track.name())?;
    let path = store.file(id).path();
    path.rsplit_once('/')
        .and_then(|(dir, _)| pick(dir))
        .or(Some(id))
}

/// Finds and loads the SoundFont for `settings`, logging one line either
/// way. `None` means music is off.
pub fn load_soundfont(settings: &AudioSettings) -> Option<Bank> {
    let lookup = soundfont::find(&Places::standard(settings.soundfont.as_deref()));
    let Some(found) = lookup.found.as_ref() else {
        eprintln!("rusty-claw: {}", soundfont::missing_message(&lookup));
        return None;
    };
    let t = Instant::now();
    match Bank::load(&found.path) {
        Ok(bank) => {
            eprintln!(
                "rusty-claw: music plays with {} (from {}), loaded in {:.0} ms",
                found.path.display(),
                found.origin,
                t.elapsed().as_secs_f64() * 1000.0
            );
            Some(bank)
        }
        Err(e) => {
            eprintln!(
                "rusty-claw: the SoundFont {} cannot be used ({e}); music is off. \
                 Run `make soundfont` or name another bank with the soundfont setting.",
                found.path.display()
            );
            None
        }
    }
}

/// Whether `settings` let `channel` sound.
fn channel_on(settings: &AudioSettings, channel: Channel) -> bool {
    settings.enabled
        && match channel {
            Channel::Voice => settings.voice,
            Channel::Ambient => settings.ambient,
            Channel::Effect | Channel::Interface => true,
        }
}

/// Plays the game's sound cues and music.
#[derive(Debug)]
pub struct AudioBridge {
    engine: AudioEngine,
    settings: AudioSettings,
    paused: bool,
    font: Option<Bank>,
    music: Option<MusicPlayer>,
    /// Looping sounds started with a handle and not stopped, by handle.
    loops: BTreeMap<u32, PlayRequest>,
    /// The boss fight flag is set: power-up music is held off.
    boss_fight: bool,
}

impl AudioBridge {
    /// Starts the audio engine on the default output device (or silently,
    /// with one message, when there is none or sound is off) and looks for
    /// the SoundFont.
    pub fn new(settings: AudioSettings) -> AudioBridge {
        let font = load_soundfont(&settings);
        AudioBridge::with_engine(AudioEngine::new(settings.clone()), settings, font)
    }

    /// A bridge that never opens a device and has no music (tests).
    pub fn silent(settings: AudioSettings) -> AudioBridge {
        AudioBridge::silent_with_music(settings, None)
    }

    /// A bridge that never opens a device and plays music with `font`
    /// (tests).
    pub fn silent_with_music(settings: AudioSettings, font: Option<Bank>) -> AudioBridge {
        AudioBridge::with_engine(AudioEngine::null(settings.clone()), settings, font)
    }

    fn with_engine(
        engine: AudioEngine,
        settings: AudioSettings,
        font: Option<Bank>,
    ) -> AudioBridge {
        AudioBridge {
            engine,
            settings,
            paused: false,
            font,
            music: None,
            loops: BTreeMap::new(),
            boss_fight: false,
        }
    }

    /// The engine.
    pub fn engine(&self) -> &AudioEngine {
        &self.engine
    }

    /// The music player, when music is loaded.
    pub fn music(&self) -> Option<&MusicPlayer> {
        self.music.as_ref()
    }

    /// Whether a SoundFont is loaded, so music can play.
    pub fn has_soundfont(&self) -> bool {
        self.font.is_some()
    }

    /// Stops what plays and loads the sounds and music of a new level, then
    /// starts the level music.
    pub fn load_level(&mut self, store: &AssetStore, registries: &Registries) -> BankReport {
        self.engine.stop_all();
        self.loops.clear();
        self.boss_fight = false;
        let (bank, mut report) = decode_bank(store, registries);
        self.engine.set_bank(bank);
        let t = Instant::now();
        let tracks = [
            Track::Level,
            Track::Boss,
            Track::Powerup,
            Track::Monolith,
            Track::Chord,
        ];
        let (count, failed) = self.load_music(store, registries, &tracks);
        report.music_tracks = count;
        report.music_failed = failed;
        report.ms += t.elapsed().as_secs_f64() * 1000.0;
        if let Some(m) = self.music.as_mut() {
            m.play(Track::Level, true);
        }
        report
    }

    /// Stops what plays and replaces the sounds with `bank` (tests and
    /// tools; a level's sounds come with [`AudioBridge::load_level`]).
    pub fn set_sounds(&mut self, bank: SoundBank) {
        self.engine.stop_all();
        self.loops.clear();
        self.engine.set_bank(bank);
    }

    /// Loads the menu's music (the chord played after a music volume
    /// change), replacing any level music.
    pub fn load_menu_music(&mut self, store: &AssetStore) -> BankReport {
        let (count, failed) = self.load_music(store, &Registries::new(), &[Track::Chord]);
        BankReport {
            music_tracks: count,
            music_failed: failed,
            ..BankReport::default()
        }
    }

    /// Converts `tracks` and hands the synthesizer to the engine. Returns
    /// the tracks loaded and the failures, which are also logged.
    fn load_music(
        &mut self,
        store: &AssetStore,
        registries: &Registries,
        tracks: &[Track],
    ) -> (usize, Vec<(String, String)>) {
        let enabled = self.settings.music;
        self.music = None;
        self.engine.set_music(None);
        let Some(font) = self.font.as_ref() else {
            return (0, Vec::new());
        };
        let mut songs = Vec::new();
        let mut failed = Vec::new();
        let mut missing = std::collections::BTreeSet::new();
        for &t in tracks {
            let Some(id) = music_file(store, registries, t, font.mode) else {
                continue;
            };
            let events = store
                .read(id)
                .map_err(|e| e.to_string())
                .and_then(|data| read_music(&data));
            match events.and_then(|ev| {
                if font.mode == BankMode::Original {
                    missing.extend(
                        convert::presets_used(&ev)
                            .into_iter()
                            .filter(|&(b, p)| !font.has_preset(b, p)),
                    );
                }
                Song::from_xmidi(&ev, font.mode)
            }) {
                Ok(song) => songs.push((t, song)),
                Err(e) => failed.push((t.name().to_string(), e)),
            }
        }
        for (name, why) in &failed {
            eprintln!("rusty-claw: music {name} left out: {why}");
        }
        if !missing.is_empty() {
            let list: Vec<String> = missing.iter().map(|(b, p)| format!("{b}:{p}")).collect();
            eprintln!(
                "rusty-claw: the bank lacks {} instruments the music selects (bank:program {}); \
                 they play with the nearest sound it has",
                list.len(),
                list.join(" ")
            );
        }
        if songs.is_empty() {
            return (0, failed);
        }
        let rate = match self.engine.stats().device_rate {
            0 => DEFAULT_RATE,
            r => r,
        };
        match music(font, &songs, rate) {
            Ok((mut player, source)) => {
                player.set_enabled(enabled);
                self.engine.set_music(Some(Box::new(source)));
                self.music = Some(player);
                (songs.len(), failed)
            }
            Err(e) => {
                eprintln!("rusty-claw: music cannot start: {e}");
                failed.push(("music".to_string(), e));
                (0, failed)
            }
        }
    }

    /// Plays the cues of the ticks run since the last frame, in order, and
    /// keeps the output device going.
    pub fn apply(&mut self, cues: &[AudioCue], view: &ViewInfo) {
        let _ = view;
        for cue in cues {
            match cue {
                AudioCue::Play(s) => {
                    let r = request(s);
                    if let Some(h) = r.handle {
                        if r.looped {
                            self.loops.insert(h, r.clone());
                        } else {
                            self.loops.remove(&h);
                        }
                    }
                    self.engine.play(&r);
                }
                AudioCue::Stop { voice } => {
                    self.loops.remove(voice);
                    self.engine.stop(*voice);
                }
                AudioCue::Music { track: t, looped } => {
                    if self.holds_off(*t) {
                        continue;
                    }
                    if let Some(m) = self.music.as_mut() {
                        m.play(track(*t), *looped);
                    }
                }
                AudioCue::StopMusic => self.stop_music(),
            }
        }
        self.engine.poll_device();
    }

    /// Tells the bridge whether the boss fight flag is set (1 or 2): from
    /// the start of a boss's introduction to the end of the level. Call it
    /// with the simulation's flag before [`AudioBridge::apply`] passes the
    /// ticks' cues. While it is set, the power-up track and the level track
    /// a power-up's end asks for are held off, so the boss track plays on.
    /// A new level clears it.
    pub fn set_boss_fight(&mut self, on: bool) {
        self.boss_fight = on;
    }

    /// True while the boss fight flag is set (see
    /// [`AudioBridge::set_boss_fight`]).
    pub fn is_boss_fight(&self) -> bool {
        self.boss_fight
    }

    /// Whether a cue for `t` is dropped: the power-up music (its start and
    /// the level track at its end) during a boss fight.
    fn holds_off(&self, t: MusicTrack) -> bool {
        self.boss_fight && matches!(t, MusicTrack::Powerup | MusicTrack::Level)
    }

    /// Acts on the game events that concern sound: the music stops when the
    /// level is won (the end-of-level screen chooses its own).
    pub fn events(&mut self, events: &[GameEvent]) {
        for e in events {
            if let GameEvent::LevelComplete { .. } = e {
                self.stop_music();
            }
        }
    }

    /// The settings in force.
    pub fn settings(&self) -> &AudioSettings {
        &self.settings
    }

    /// Applies new settings: volumes and switches at once; sound off closes
    /// the output device. Loops silenced by a switch come back when it is
    /// turned on again, and the Music switch pauses or restarts the current
    /// track; while the game is paused both wait for the resume. A different
    /// SoundFont takes effect from the next level.
    pub fn set_settings(&mut self, settings: AudioSettings) {
        let before = self.settings.clone();
        let soundfont_changed = settings.soundfont != before.soundfont;
        self.engine.set_settings(settings.clone());
        self.settings = settings;
        if !self.paused {
            // While paused, the resume starts the loops again.
            for r in self.loops.values() {
                if channel_on(&self.settings, r.channel) && !channel_on(&before, r.channel) {
                    self.engine.play(r);
                }
            }
            if let Some(m) = self.music.as_mut() {
                m.set_enabled(self.settings.music);
            }
        }
        if soundfont_changed {
            self.font = load_soundfont(&self.settings);
        }
    }

    /// Pauses the game's sounds: every effect, speech and ambient sound
    /// stops (loops included) and the music holds. Only the ambient loops
    /// come back at the resume.
    pub fn pause(&mut self) {
        if !self.paused {
            self.paused = true;
            self.engine.pause_game();
            self.loops.retain(|_, r| r.channel == Channel::Ambient);
        }
    }

    /// Resumes the game's sounds: the music carries on where it was (or
    /// follows a Music switch changed during the pause) and the live
    /// ambient loops start again at their current level.
    pub fn resume(&mut self) {
        if self.paused {
            self.paused = false;
            self.engine.resume_game();
            for r in self.loops.values() {
                if channel_on(&self.settings, r.channel) {
                    self.engine.play(r);
                }
            }
            if let Some(m) = self.music.as_mut() {
                m.set_enabled(self.settings.music);
            }
        }
    }

    /// Ends the ambient loops of the level being left: they stop now and a
    /// later [`AudioBridge::resume`] does not start them again, so the
    /// level's loops never play over the menus or the end-of-level screens.
    pub fn leave_level(&mut self) {
        for &h in self.loops.keys() {
            self.engine.stop(h);
        }
        self.loops.clear();
    }

    /// The looping sounds of the level in play that are still live (the
    /// ones a resume or a switch brings back).
    pub fn live_loops(&self) -> usize {
        self.loops.len()
    }

    /// True while the game's sounds are paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Holds the music where it is (`true`) or lets it carry on from there
    /// (`false`). Unlike [`AudioBridge::pause`] it leaves every sound as it
    /// is, and the two are independent: the music plays only when neither
    /// holds it. Music cues that arrive while it is held still switch
    /// tracks; the track chosen sounds once the hold ends.
    pub fn hold_music(&mut self, on: bool) {
        self.engine.hold_music(on);
    }

    /// True while [`AudioBridge::hold_music`] holds the music.
    pub fn is_music_held(&self) -> bool {
        self.engine.music_held()
    }

    /// Plays an interface sound.
    pub fn play_ui(&mut self, key: &str) {
        self.engine.play_ui(key);
    }

    /// Plays the menu chord once, as the main menu does after a music volume
    /// change. Needs [`AudioBridge::load_menu_music`] or a loaded level.
    pub fn play_menu_music(&mut self) {
        if let Some(m) = self.music.as_mut() {
            m.play(Track::Chord, false);
        }
    }

    /// Restarts the current track from its beginning, looping, as the game
    /// does when its in-game menu closes.
    pub fn restart_music(&mut self) {
        if let Some(m) = self.music.as_mut() {
            m.restart(true);
        }
    }

    /// Stops the music.
    pub fn stop_music(&mut self) {
        if let Some(m) = self.music.as_mut() {
            m.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    use claw_assets::Source;
    use claw_assets::xmi::{Timbre, XmiEvent};

    #[test]
    fn sounds_decode_from_wav_files() {
        let mono = wav::build(1, 11025, 16, &[1, 0, 2, 0, 3, 0]);
        let p = decode_sound(&mono).unwrap();
        assert_eq!((p.rate, p.channels), (11025, 1));
        assert_eq!(&p.samples[..], &[1, 2, 3]);
        let eight = wav::build(2, 22050, 8, &[128, 255, 0, 128]);
        let p = decode_sound(&eight).unwrap();
        assert_eq!((p.channels, p.frames()), (2, 2));
        assert!(decode_sound(b"not a sound").is_err());
        assert!(decode_sound(&wav::build(3, 8000, 16, &[0; 12])).is_err());
        assert!(decode_sound(&wav::build(1, 8000, 16, &[])).is_err());
    }

    #[test]
    fn cues_map_to_requests() {
        let cue = SoundCue {
            key: "GAME_HIT1".into(),
            volume: 70,
            pan: -20,
            pitch: 5,
            looped: true,
            channel: SoundChannel::Voice,
            voice: Some(9),
        };
        assert_eq!(
            request(&cue),
            PlayRequest {
                key: "GAME_HIT1".into(),
                volume: 70,
                pan: -20,
                pitch: 5,
                looped: true,
                channel: Channel::Voice,
                handle: Some(9),
            }
        );
        for (c, e) in [
            (SoundChannel::Effect, Channel::Effect),
            (SoundChannel::Voice, Channel::Voice),
            (SoundChannel::Ambient, Channel::Ambient),
            (SoundChannel::Interface, Channel::Interface),
        ] {
            assert_eq!(channel(c), e);
        }
        for (t, e) in [
            (MusicTrack::Level, Track::Level),
            (MusicTrack::Powerup, Track::Powerup),
            (MusicTrack::Boss, Track::Boss),
            (MusicTrack::Chord, Track::Chord),
        ] {
            assert_eq!(track(t), e);
        }
    }

    /// Polls the engine until `done`, for at most 3 s.
    fn wait(bridge: &mut AudioBridge, done: impl Fn(&AudioBridge) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            bridge.apply(&[], &ViewInfo::default());
            if done(bridge) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn play(key: &str, channel: SoundChannel, voice: Option<u32>, looped: bool) -> AudioCue {
        AudioCue::Play(SoundCue {
            key: key.into(),
            volume: 100,
            looped,
            channel,
            voice,
            ..SoundCue::default()
        })
    }

    #[test]
    fn the_bridge_plays_and_pauses_without_a_device_or_music() {
        let mut bridge = AudioBridge::silent(AudioSettings::default());
        assert!(!bridge.has_soundfont());
        let mut bank = SoundBank::new();
        bank.insert("A", Pcm::mono(22050, vec![0i16; 100]).into());
        bridge.engine.set_bank(bank);
        let cues = vec![
            play("A", SoundChannel::Effect, None, false),
            play("MISSING", SoundChannel::Effect, None, false),
            AudioCue::Stop { voice: 3 },
            AudioCue::Music {
                track: MusicTrack::Level,
                looped: true,
            },
            AudioCue::StopMusic,
        ];
        bridge.apply(&cues, &ViewInfo::default());
        bridge.events(&[GameEvent::LevelComplete { after_ms: 0 }]);
        bridge.pause();
        bridge.pause();
        assert!(bridge.is_paused());
        bridge.resume();
        assert!(!bridge.is_paused());
        bridge.play_ui("A");
        bridge.play_menu_music();
        bridge.restart_music();
        bridge.stop_music();
        assert!(bridge.music().is_none());
        assert_eq!(bridge.engine().output(), claw_audio::Output::Null);
    }

    #[test]
    fn switches_gate_channels_and_bring_loops_back() {
        let settings = AudioSettings::default();
        let mut bridge = AudioBridge::silent(settings.clone());
        let mut bank = SoundBank::new();
        for k in ["AMB", "LINE", "HIT"] {
            bank.insert(k, Pcm::mono(22050, vec![100i16; 22050 * 4]).into());
        }
        bridge.engine.set_bank(bank);
        bridge.apply(
            &[
                play("AMB", SoundChannel::Ambient, Some(7), true),
                play("LINE", SoundChannel::Voice, None, false),
            ],
            &ViewInfo::default(),
        );
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 2));
        // An update of the live loop does not add a voice.
        bridge.apply(
            &[play("AMB", SoundChannel::Ambient, Some(7), true)],
            &ViewInfo::default(),
        );
        assert!(wait(&mut bridge, |b| b.engine().stats().commands >= 4));
        assert_eq!(bridge.engine().stats().active_voices, 2);

        // Ambient and Voice off silence their channels; effects still play.
        bridge.set_settings(AudioSettings {
            ambient: false,
            voice: false,
            ..settings.clone()
        });
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 0));
        bridge.apply(
            &[
                play("LINE", SoundChannel::Voice, None, false),
                play("HIT", SoundChannel::Effect, None, false),
            ],
            &ViewInfo::default(),
        );
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 1));

        // Ambient on: the live loop comes back at once.
        bridge.set_settings(AudioSettings {
            voice: false,
            ..settings.clone()
        });
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 2));
        // A stopped loop does not.
        bridge.apply(&[AudioCue::Stop { voice: 7 }], &ViewInfo::default());
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 1));
        bridge.set_settings(AudioSettings {
            enabled: false,
            ..settings.clone()
        });
        bridge.set_settings(settings);
        bridge.apply(&[], &ViewInfo::default());
        std::thread::sleep(Duration::from_millis(30));
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 0));
    }

    #[test]
    fn a_pause_ends_game_sounds_and_the_resume_brings_back_ambient_loops() {
        let settings = AudioSettings::default();
        let mut bridge = AudioBridge::silent(settings.clone());
        let mut bank = SoundBank::new();
        for k in ["AMB", "SAW", "LINE"] {
            bank.insert(k, Pcm::mono(22050, vec![100i16; 22050 * 4]).into());
        }
        bridge.engine.set_bank(bank);
        bridge.apply(
            &[
                play("AMB", SoundChannel::Ambient, Some(7), true),
                play("SAW", SoundChannel::Effect, Some(3), true),
                play("LINE", SoundChannel::Voice, None, false),
            ],
            &ViewInfo::default(),
        );
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 3));
        // Every game sound ends at the pause, the looping effect included.
        bridge.pause();
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 0));
        // Interface sounds still play.
        bridge.play_ui("LINE");
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 1));
        // The resume starts the ambient loop again, and only it.
        bridge.resume();
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 2));
        // The interface sound and the ambient loop.
        let started = bridge.engine().stats().voices_started;
        assert_eq!(started, 5, "three plays, the interface sound, the loop");
        // The effect loop does not come back with a switch either.
        bridge.set_settings(AudioSettings {
            enabled: false,
            ..settings.clone()
        });
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 0));
        bridge.set_settings(settings);
        assert!(wait(&mut bridge, |b| b.engine().stats().voices_started == 6));
        std::thread::sleep(Duration::from_millis(30));
        bridge.apply(&[], &ViewInfo::default());
        assert_eq!(bridge.engine().stats().voices_started, 6);
    }

    #[test]
    fn leaving_a_level_ends_its_ambient_loops() {
        let settings = AudioSettings::default();
        let mut bridge = AudioBridge::silent(settings.clone());
        let mut bank = SoundBank::new();
        for k in ["AMB", "UI"] {
            bank.insert(k, Pcm::mono(22050, vec![100i16; 22050 * 4]).into());
        }
        bridge.engine.set_bank(bank);
        let amb = play("AMB", SoundChannel::Ambient, Some(7), true);
        bridge.apply(std::slice::from_ref(&amb), &ViewInfo::default());
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 1));
        assert_eq!(bridge.live_loops(), 1);

        // Left while paused (the in-game menu, the game-over fade): the
        // resume that follows starts nothing.
        bridge.pause();
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 0));
        bridge.leave_level();
        assert_eq!(bridge.live_loops(), 0);
        bridge.resume();
        // An interface sound behind the resume: once it plays, the resume
        // has been carried out too.
        bridge.play_ui("UI");
        assert!(wait(&mut bridge, |b| b.engine().stats().voices_started == 2));
        std::thread::sleep(Duration::from_millis(30));
        bridge.apply(&[], &ViewInfo::default());
        assert_eq!(bridge.engine().stats().voices_started, 2);
        assert_eq!(bridge.engine().stats().active_voices, 1, "only UI");

        // Left while playing (a level won): the loop stops at once, and a
        // switch turned off and on does not bring it back.
        bridge.set_sounds(SoundBank::new());
        let mut bank = SoundBank::new();
        bank.insert("AMB", Pcm::mono(22050, vec![100i16; 22050 * 4]).into());
        bridge.engine.set_bank(bank);
        bridge.apply(&[amb], &ViewInfo::default());
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 1));
        bridge.leave_level();
        assert!(wait(&mut bridge, |b| b.engine().stats().active_voices == 0));
        bridge.set_settings(AudioSettings {
            ambient: false,
            ..settings.clone()
        });
        bridge.set_settings(settings);
        std::thread::sleep(Duration::from_millis(30));
        bridge.apply(&[], &ViewInfo::default());
        assert_eq!(bridge.engine().stats().active_voices, 0);
        assert_eq!(bridge.engine().stats().voices_started, 3);
    }

    #[test]
    fn the_music_switch_waits_for_the_resume() {
        let (_dir, store, reg) = music_store();
        let mut bridge =
            AudioBridge::silent_with_music(AudioSettings::default(), Some(test_bank()));
        bridge.load_level(&store, &reg);
        let sounding = |b: &AudioBridge| b.music().and_then(|m| m.sounding());
        let enabled = |b: &AudioBridge| b.music().is_some_and(|m| m.enabled());
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Level)));
        let off = AudioSettings {
            music: false,
            ..AudioSettings::default()
        };

        // Off during a pause: recorded, then applied by the resume.
        bridge.pause();
        bridge.set_settings(off.clone());
        assert!(enabled(&bridge), "only recorded while paused");
        bridge.resume();
        assert!(!enabled(&bridge));
        assert!(wait(&mut bridge, |b| sounding(b).is_none()));

        // On during a pause: the resume restarts the current track.
        bridge.pause();
        bridge.set_settings(AudioSettings::default());
        assert!(!enabled(&bridge));
        bridge.resume();
        assert!(enabled(&bridge));
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Level)));

        // Off and on again during a pause: no change at the resume.
        bridge.pause();
        bridge.set_settings(off);
        bridge.set_settings(AudioSettings::default());
        bridge.resume();
        assert!(enabled(&bridge));
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Level)));
    }

    fn note(time: u32, channel: u8, key: u8, duration: u32) -> XmiEvent {
        XmiEvent {
            time_120hz: time,
            kind: XmiEventKind::Note {
                channel,
                key,
                velocity: 120,
                duration,
            },
        }
    }

    fn track_file(key: u8, seconds: u32) -> Vec<u8> {
        let ticks = seconds * 120;
        let events = vec![
            note(0, 0, key, ticks),
            XmiEvent {
                time_120hz: ticks,
                kind: XmiEventKind::EndOfTrack,
            },
        ];
        xmi::build(&[(vec![Timbre::default()], xmi::encode_events(&events))])
    }

    /// A store with LEVEL1 music (both arrangements), the power-up track and
    /// the menu chord, all synthetic.
    fn music_store() -> (tempfile::TempDir, AssetStore, Registries) {
        let dir = tempfile::tempdir().unwrap();
        let put = |rel: &str, data: &[u8]| {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, data).unwrap();
        };
        put("LEVEL1/MUSIC/PLAY.XMI", &track_file(50, 30));
        put("LEVEL1/MUSIC/PLAY0.XMI", &track_file(60, 30));
        put("GAME/MUSIC/POWERUP0.XMI", &track_file(72, 30));
        put("STATES/MENU/MUSIC/CHORD.XMI", &track_file(64, 2));
        let mut store = AssetStore::new();
        store.mount(Source::Dir(dir.path().to_path_buf())).unwrap();
        let mut registries = Registries::new();
        let level = store.lookup("LEVEL1/MUSIC/PLAY0.XMI").unwrap().unwrap();
        let powerup = store.lookup("GAME/MUSIC/POWERUP0.XMI").unwrap().unwrap();
        registries.music.insert("LEVEL".into(), level);
        registries.music.insert("POWERUP".into(), powerup);
        (dir, store, registries)
    }

    fn test_bank() -> Bank {
        Bank::from_bytes(&soundfont::build_test_bank()).unwrap()
    }

    #[test]
    fn music_files_follow_the_bank() {
        let (_dir, store, reg) = music_store();
        let name =
            |t, mode| music_file(&store, &reg, t, mode).map(|id| store.file(id).name().to_string());
        assert_eq!(
            name(Track::Level, BankMode::GeneralMidi).as_deref(),
            Some("PLAY0")
        );
        assert_eq!(
            name(Track::Level, BankMode::Original).as_deref(),
            Some("PLAY")
        );
        assert_eq!(
            name(Track::Powerup, BankMode::Original).as_deref(),
            Some("POWERUP0"),
            "falls back to the arrangement present"
        );
        assert_eq!(
            name(Track::Chord, BankMode::GeneralMidi).as_deref(),
            Some("CHORD")
        );
        assert_eq!(name(Track::Boss, BankMode::GeneralMidi), None);
    }

    #[test]
    fn level_music_starts_switches_and_stops() {
        let (_dir, store, reg) = music_store();
        let mut bridge =
            AudioBridge::silent_with_music(AudioSettings::default(), Some(test_bank()));
        let report = bridge.load_level(&store, &reg);
        assert_eq!(report.music_tracks, 3, "{report:?}");
        assert!(report.music_failed.is_empty());
        let sounding = |b: &AudioBridge| b.music().and_then(|m| m.sounding());
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Level)));

        let cue = |t, looped| AudioCue::Music { track: t, looped };
        bridge.apply(&[cue(MusicTrack::Powerup, true)], &ViewInfo::default());
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Powerup)));
        bridge.apply(&[cue(MusicTrack::Level, true)], &ViewInfo::default());
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Level)));

        // The Music switch.
        bridge.set_settings(AudioSettings {
            music: false,
            ..AudioSettings::default()
        });
        assert!(wait(&mut bridge, |b| sounding(b).is_none()));
        bridge.apply(&[cue(MusicTrack::Powerup, true)], &ViewInfo::default());
        std::thread::sleep(Duration::from_millis(30));
        assert!(wait(&mut bridge, |b| sounding(b).is_none()));
        bridge.set_settings(AudioSettings::default());
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Level)));

        // The level is won: the music stops.
        bridge.events(&[GameEvent::LevelComplete { after_ms: 3000 }]);
        assert!(wait(&mut bridge, |b| sounding(b).is_none()));

        // The menu chord plays once.
        bridge.load_menu_music(&store);
        bridge.play_menu_music();
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Chord)));
        assert!(wait(&mut bridge, |b| sounding(b).is_none()));
    }
    /// A store with LEVEL2 music: the level track, the boss track and the
    /// power-up track, all synthetic.
    fn boss_store() -> (tempfile::TempDir, AssetStore, Registries) {
        let dir = tempfile::tempdir().unwrap();
        let put = |rel: &str, data: &[u8]| {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, data).unwrap();
        };
        put("LEVEL2/MUSIC/PLAY0.XMI", &track_file(50, 30));
        put("LEVEL2/MUSIC/BOSS0.XMI", &track_file(55, 30));
        put("GAME/MUSIC/POWERUP0.XMI", &track_file(72, 30));
        let mut store = AssetStore::new();
        store.mount(Source::Dir(dir.path().to_path_buf())).unwrap();
        let mut registries = Registries::new();
        for (key, path) in [
            ("LEVEL", "LEVEL2/MUSIC/PLAY0.XMI"),
            ("BOSS", "LEVEL2/MUSIC/BOSS0.XMI"),
            ("POWERUP", "GAME/MUSIC/POWERUP0.XMI"),
        ] {
            let id = store.lookup(path).unwrap().unwrap();
            registries.music.insert(key.into(), id);
        }
        (dir, store, registries)
    }

    #[test]
    fn the_boss_track_plays_on_and_holds_off_the_power_up_music() {
        let (_dir, store, reg) = boss_store();
        let mut bridge =
            AudioBridge::silent_with_music(AudioSettings::default(), Some(test_bank()));
        let report = bridge.load_level(&store, &reg);
        assert_eq!(report.music_tracks, 3, "{report:?}");
        let sounding = |b: &AudioBridge| b.music().and_then(|m| m.sounding());
        let cue = |t, looped| AudioCue::Music { track: t, looped };
        let view = ViewInfo::default();
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Level)));

        // The stage wakes: the boss track, before any introduction.
        bridge.apply(&[cue(MusicTrack::Boss, true)], &view);
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Boss)));
        assert!(!bridge.is_boss_fight());

        // The introduction starts: a power-up and its end change nothing.
        bridge.set_boss_fight(true);
        bridge.apply(
            &[cue(MusicTrack::Powerup, true), cue(MusicTrack::Level, true)],
            &view,
        );
        std::thread::sleep(Duration::from_millis(30));
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Boss)));
        assert_eq!(bridge.music().unwrap().current(), Some(Track::Boss));
        // The level is won: the music stops, and the next level clears the
        // flag.
        bridge.events(&[GameEvent::LevelComplete { after_ms: 3000 }]);
        assert!(wait(&mut bridge, |b| sounding(b).is_none()));
        bridge.load_level(&store, &reg);
        assert!(!bridge.is_boss_fight());
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Level)));

        // After an intro skip the flag stays clear: a power-up swaps the
        // boss track out, and its end brings the level track.
        bridge.apply(&[cue(MusicTrack::Boss, true)], &view);
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Boss)));
        bridge.apply(&[cue(MusicTrack::Powerup, true)], &view);
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Powerup)));
        bridge.apply(&[cue(MusicTrack::Level, true)], &view);
        assert!(wait(&mut bridge, |b| sounding(b) == Some(Track::Level)));
    }
}
