//! Music: XMIDI conversion on synthetic sequences, the volume mapping, track
//! selection, the SoundFont lookup, and the synthesizer driven by a tiny bank
//! built in test code.

use std::fs;
use std::path::PathBuf;

use claw_audio::music::convert::{
    BankMode, DIVISION, SmfEvent, TEMPO, XmidiEvent, XmidiKind, presets_used, read_smf,
    xmidi_to_smf,
};
use claw_audio::music::soundfont::{self, Bank, Origin, Places};
use claw_audio::music::{MusicPlayer, Song, SynthMusic, Track, music, select_file};
use claw_audio::{AudioEngine, AudioSettings, Mixer, MusicSource, Output, curve};

fn note(tick: u32, channel: u8, key: u8, velocity: u8, duration: u32) -> XmidiEvent {
    XmidiEvent::new(
        tick,
        XmidiKind::Note {
            channel,
            key,
            velocity,
            duration,
        },
    )
}

fn cc(tick: u32, channel: u8, controller: u8, value: u8) -> XmidiEvent {
    XmidiEvent::new(
        tick,
        XmidiKind::Controller {
            channel,
            controller,
            value,
        },
    )
}

fn program(tick: u32, channel: u8, program: u8) -> XmidiEvent {
    XmidiEvent::new(tick, XmidiKind::Program { channel, program })
}

fn end(tick: u32) -> XmidiEvent {
    XmidiEvent::new(tick, XmidiKind::EndOfTrack)
}

/// The converted file's events, without the leading tempo and the end.
fn body(events: &[XmidiEvent], mode: BankMode) -> Vec<(u32, Vec<u8>)> {
    let (division, all) = read_smf(&xmidi_to_smf(events, mode)).expect("an SMF");
    assert_eq!(division, DIVISION);
    let tempo = TEMPO.to_be_bytes();
    assert_eq!(
        all[0],
        SmfEvent {
            tick: 0,
            bytes: vec![0xFF, 0x51, 3, tempo[1], tempo[2], tempo[3]]
        }
    );
    let last = all.last().unwrap();
    assert_eq!(last.bytes, vec![0xFF, 0x2F, 0]);
    all[1..all.len() - 1]
        .iter()
        .map(|e| (e.tick, e.bytes.clone()))
        .collect()
}

fn end_tick(events: &[XmidiEvent]) -> u32 {
    let (_, all) = read_smf(&xmidi_to_smf(events, BankMode::GeneralMidi)).unwrap();
    all.last().unwrap().tick
}

#[test]
fn delays_and_durations_keep_their_ticks() {
    // One SMF tick is one 120 Hz tick: 60 per quarter at 120 bpm.
    assert_eq!(f64::from(TEMPO) / 1e6 / f64::from(DIVISION), 1.0 / 120.0);
    let ev = [
        note(0, 0, 60, 100, 30),
        note(265, 1, 64, 90, 4180),
        end(4500),
    ];
    assert_eq!(
        body(&ev, BankMode::GeneralMidi),
        vec![
            (0, vec![0x90, 60, 100]),
            (30, vec![0x80, 60, 64]),
            (265, vec![0x91, 64, 90]),
            (4445, vec![0x81, 64, 64]),
        ]
    );
    assert_eq!(end_tick(&ev), 4500);
}

#[test]
fn tempo_metas_are_dropped_and_the_end_is_kept() {
    // Tempo, meta and system exclusive events say nothing at play time.
    let ev = [
        XmidiEvent::new(0, XmidiKind::Other),
        note(10, 0, 60, 100, 5),
        XmidiEvent::new(12, XmidiKind::Other),
        end(1200),
        note(1300, 0, 61, 100, 5),
    ];
    assert_eq!(
        body(&ev, BankMode::GeneralMidi),
        vec![(10, vec![0x90, 60, 100]), (15, vec![0x80, 60, 64])]
    );
    // The loop length is the end of the sequence, not the last note-off.
    assert_eq!(end_tick(&ev), 1200);
    let song = Song::from_xmidi(&ev, BankMode::GeneralMidi).unwrap();
    assert!((song.seconds() - 10.0).abs() < 1e-9);
    // Without an end the last event ends the sequence.
    assert_eq!(end_tick(&[note(7, 0, 60, 1, 3)]), 7);
}

#[test]
fn duplicates_restrikes_and_the_end_cut_notes() {
    let ev = [
        // The same key twice at one tick sounds once, louder and longer.
        note(0, 2, 50, 80, 10),
        note(0, 2, 50, 100, 20),
        // Struck again at 30 while sounding until 40: ends right before.
        note(25, 2, 52, 90, 15),
        note(30, 2, 52, 91, 5),
        // A note-off past the end moves to the end.
        note(90, 3, 40, 70, 500),
        // A zero-length note ends right after its note-on.
        note(95, 4, 41, 60, 0),
        end(100),
    ];
    assert_eq!(
        body(&ev, BankMode::GeneralMidi),
        vec![
            (0, vec![0x92, 50, 100]),
            (20, vec![0x82, 50, 64]),
            (25, vec![0x92, 52, 90]),
            (30, vec![0x82, 52, 64]),
            (30, vec![0x92, 52, 91]),
            (35, vec![0x82, 52, 64]),
            (90, vec![0x93, 40, 70]),
            (95, vec![0x94, 41, 60]),
            (95, vec![0x84, 41, 64]),
            (100, vec![0x83, 40, 64]),
        ]
    );
}

#[test]
fn natural_note_offs_come_first_within_a_tick() {
    let ev = [
        note(0, 0, 60, 100, 10),
        program(10, 1, 5),
        note(10, 1, 62, 100, 10),
        end(20),
    ];
    assert_eq!(
        body(&ev, BankMode::GeneralMidi),
        vec![
            // Channel 1's program is set only after the first note: the
            // loop prologue gives it the power-on program at tick 0.
            (0, vec![0xC1, 0]),
            (0, vec![0x90, 60, 100]),
            (10, vec![0x80, 60, 64]),
            (10, vec![0xC1, 5]),
            (10, vec![0x91, 62, 100]),
            (20, vec![0x81, 62, 64]),
        ]
    );
}

#[test]
fn controllers_and_bank_selects() {
    let ev = [
        cc(0, 0, 0, 1),
        cc(0, 0, 32, 0),
        program(0, 0, 48),
        cc(0, 0, 7, 110),
        cc(0, 0, 10, 30),
        cc(0, 0, 116, 0),
        cc(0, 0, 117, 127),
        cc(0, 0, 1, 5),
        note(0, 0, 60, 100, 4),
        end(4),
    ];
    // General MIDI: bank selects and the sequencer extensions are dropped.
    assert_eq!(
        body(&ev, BankMode::GeneralMidi),
        vec![
            (0, vec![0xC0, 48]),
            (0, vec![0xB0, 7, 110]),
            (0, vec![0xB0, 10, 30]),
            (0, vec![0xB0, 1, 5]),
            (0, vec![0x90, 60, 100]),
            (4, vec![0x80, 60, 64]),
        ]
    );
    // The game's own bank keeps the bank selects.
    let original = body(&ev, BankMode::Original);
    assert_eq!(original[0], (0, vec![0xB0, 0, 1]));
    assert_eq!(original[1], (0, vec![0xB0, 32, 0]));
    assert!(
        original
            .iter()
            .all(|(_, b)| b[0] != 0xB0 || b[1] < 110 || b[1] > 120)
    );
    let used = presets_used(&ev);
    assert_eq!(used.into_iter().collect::<Vec<_>>(), vec![(1, 48)]);
}

#[test]
fn every_loop_pass_starts_like_the_first() {
    // Channel 1 gets its program and volume only after its first note, and
    // channel 0 sets its pan both before and after: only channel 1 needs
    // power-on values at tick 0.
    let ev = [
        cc(0, 0, 10, 20),
        note(0, 0, 60, 100, 10),
        note(5, 1, 62, 100, 10),
        cc(20, 0, 10, 100),
        program(20, 1, 40),
        cc(20, 1, 7, 80),
        XmidiEvent::new(
            20,
            XmidiKind::PitchBend {
                channel: 1,
                value: 0x3000,
            },
        ),
        cc(20, 1, 0, 3),
        end(40),
    ];
    let b = body(&ev, BankMode::GeneralMidi);
    assert_eq!(
        &b[..4],
        &[
            (0, vec![0xE1, 0x00, 0x40]),
            (0, vec![0xB1, 7, 100]),
            (0, vec![0xC1, 0]),
            (0, vec![0xB0, 10, 20]),
        ]
    );
    assert!(b.contains(&(20, vec![0xC1, 40])));
    assert!(b.contains(&(20, vec![0xE1, 0x00, 0x60])));
}

#[test]
fn track_selection_and_fallback() {
    let level1 = ["PLAY", "PLAY0"];
    assert_eq!(
        select_file(Track::Level, BankMode::GeneralMidi, &level1),
        Some("PLAY0")
    );
    assert_eq!(
        select_file(Track::Level, BankMode::Original, &level1),
        Some("PLAY")
    );
    // Falls back to the other arrangement, compares without case.
    assert_eq!(
        select_file(Track::Level, BankMode::GeneralMidi, &["play"]),
        Some("play")
    );
    assert_eq!(
        select_file(Track::Boss, BankMode::Original, &["BOSS0", "PLAY0"]),
        Some("BOSS0")
    );
    assert_eq!(
        select_file(Track::Boss, BankMode::GeneralMidi, &["PLAY0"]),
        None
    );
    assert_eq!(
        select_file(
            Track::Powerup,
            BankMode::GeneralMidi,
            &["MONOLITH", "POWERUP", "POWERUP0"]
        ),
        Some("POWERUP0")
    );
    assert_eq!(
        select_file(Track::Chord, BankMode::GeneralMidi, &["CHORD"]),
        Some("CHORD")
    );
    assert_eq!(Track::Level.name(), "LEVEL");
}

#[test]
fn music_volume_mapping() {
    // 0..=100 to the MIDI master volume 0..=127, then squared into gain.
    for (v, m) in [
        (0u8, 0u8),
        (1, 1),
        (50, 63),
        (79, 100),
        (99, 125),
        (100, 127),
        (120, 127),
    ] {
        assert_eq!(curve::music_master(v), m, "v = {v}");
        let g = (f32::from(m) / 127.0).powi(2);
        assert!((curve::music_gain(v) - g).abs() < 1e-7, "v = {v}");
    }
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn soundfont_lookup_order() {
    let dir = tmp();
    let bank = soundfont::build_test_bank();
    let put = |rel: &str, data: &[u8]| -> PathBuf {
        let p = dir.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, data).unwrap();
        p
    };
    let setting = put("setting/mine.sf2", &bank);
    let env = put("env/env.sf2", &bank);
    let package = dir.path().join("package");
    put("package/zzz.sf2", &bank);
    let data = dir.path().join("data");
    put("data/aaa.sf2", &bank);
    put("data/GeneralUser-GS.sf2", &bank);
    put("data/notes.txt", b"not a bank");
    let system = put("system/FluidR3_GM.sf2", &bank);

    let mut places = Places {
        setting: Some(setting.clone()),
        environment: Some(env.clone()),
        package: Some(package.clone()),
        data_dir: Some(data.clone()),
        system: vec![dir.path().join("missing.sf2"), system.clone()],
    };
    let found = |p: &Places| soundfont::find(p).found.unwrap();
    assert_eq!(found(&places).path, setting);
    assert_eq!(found(&places).origin, Origin::Setting);
    places.setting = None;
    assert_eq!(found(&places).origin, Origin::Environment);
    places.environment = None;
    assert_eq!(found(&places).path, package.join("zzz.sf2"));
    places.package = None;
    // The default file first, then the others by name.
    assert_eq!(found(&places).path, data.join("GeneralUser-GS.sf2"));
    assert_eq!(
        soundfont::folder_banks(&data),
        vec![data.join("GeneralUser-GS.sf2"), data.join("aaa.sf2")]
    );
    places.data_dir = None;
    assert_eq!(found(&places).path, system);
    assert_eq!(found(&places).origin, Origin::System);
    places.system.clear();
    let none = soundfont::find(&places);
    assert!(none.found.is_none());
    let msg = soundfont::missing_message(&none);
    assert!(msg.contains("music is off") && msg.contains("make soundfont"));
    assert!(msg.contains("soundfont setting") && msg.contains("CLAW_SOUNDFONT"));

    // A named file that is no SF2 is refused and the lookup goes on.
    let sf3 = {
        let mut b = bank.clone();
        let ifil = b.windows(4).position(|w| w == b"ifil").unwrap();
        b[ifil + 8] = 3;
        put("setting/compressed.sf3", &b)
    };
    let places = Places {
        setting: Some(sf3.clone()),
        environment: Some(dir.path().join("nowhere.sf2")),
        system: vec![system.clone()],
        ..Places::default()
    };
    let l = soundfont::find(&places);
    assert_eq!(l.found.as_ref().unwrap().path, system);
    assert_eq!(l.refused.len(), 2);
    assert!(l.refused[0].reason.contains("SF3"));
    assert!(soundfont::missing_message(&l).contains("SF3"));
}

#[test]
fn soundfont_checks() {
    let bank = soundfont::build_test_bank();
    assert!(soundfont::check_header(&bank).is_ok());
    assert!(soundfont::check_header(b"RIFF\0\0\0\0WAVE").is_err());
    assert!(soundfont::check_header(&bank[..20]).is_err());
    assert!(Bank::from_bytes(b"nothing").is_err());
    // Corrupted banks are refused, never a crash.
    for cut in [30usize, 200, bank.len() / 2, bank.len() - 10] {
        assert!(Bank::from_bytes(&bank[..cut]).is_err(), "cut at {cut}");
    }
    let b = Bank::from_bytes(&bank).unwrap();
    assert!(b.has_preset(0, 0) && b.has_preset(128, 0) && !b.has_preset(0, 1));
    assert_eq!(
        soundfont::bank_mode(&PathBuf::from("/x/claw.sf2")),
        BankMode::Original
    );
    assert_eq!(
        soundfont::bank_mode(&PathBuf::from("/x/CLAW4.SF2")),
        BankMode::Original
    );
    assert_eq!(
        soundfont::bank_mode(&PathBuf::from("/x/GeneralUser-GS.sf2")),
        BankMode::GeneralMidi
    );
    assert_eq!(
        soundfont::bank_gain(&PathBuf::from("GeneralUser-GS.sf2")),
        soundfont::DEFAULT_GAIN
    );
    assert!(soundfont::bank_gain(&PathBuf::from("FluidR3_GM.sf2")) < soundfont::DEFAULT_GAIN);
}

const RATE: u32 = 48_000;

fn test_bank() -> Bank {
    Bank::from_bytes(&soundfont::build_test_bank()).unwrap()
}

/// A track of `seconds` with one long note on channel 0 (and a drum hit).
fn song(seconds: u32, key: u8) -> Song {
    let ticks = seconds * 120;
    Song::from_xmidi(
        &[
            note(0, 0, key, 120, ticks),
            note(0, 9, 36, 120, 10),
            end(ticks),
        ],
        BankMode::GeneralMidi,
    )
    .unwrap()
}

fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt()
}

/// Renders `ms` of music in 10 ms blocks.
fn render(source: &mut SynthMusic, ms: u32, rate: u32) -> Vec<f32> {
    let block = (rate / 100) as usize;
    let mut out = Vec::new();
    for _ in 0..ms / 10 {
        let mut b = vec![0.0f32; 2 * block];
        source.render(&mut b, rate);
        out.extend_from_slice(&b);
    }
    out
}

fn deck(tracks: &[(Track, Song)]) -> (MusicPlayer, SynthMusic) {
    music(&test_bank(), tracks, RATE).unwrap()
}

#[test]
fn a_tiny_bank_drives_the_synthesizer() {
    let (mut player, mut source) = deck(&[(Track::Level, song(2, 69))]);
    assert_eq!(source.rate(), RATE);
    assert!(
        rms(&render(&mut source, 100, RATE)) == 0.0,
        "silent until played"
    );
    player.play(Track::Level, true);
    let out = render(&mut source, 500, RATE);
    assert!(rms(&out) > 0.01, "the sine sounds: {}", rms(&out));
    assert_eq!(player.sounding(), Some(Track::Level));
    // At another output rate the music is resampled, not silenced.
    let out = render(&mut source, 200, 44_100);
    assert!(rms(&out) > 0.01);
    // Same input, same output.
    let run = || {
        let (mut p, mut s) = deck(&[(Track::Level, song(2, 69))]);
        p.play(Track::Level, true);
        render(&mut s, 300, 44_100)
    };
    assert_eq!(run(), run());
}

#[test]
fn loop_restart_and_stop() {
    // A one-second track: without looping it plays once.
    let (mut player, mut source) = deck(&[(Track::Chord, song(1, 69))]);
    player.play(Track::Chord, false);
    render(&mut source, 500, RATE);
    assert_eq!(player.sounding(), Some(Track::Chord));
    render(&mut source, 1000, RATE);
    assert_eq!(player.sounding(), None, "played once");
    let tail = render(&mut source, 1500, RATE);
    assert!(
        rms(&tail[tail.len() - 4800..]) < 1e-4,
        "silent after the end"
    );

    // Looping, it starts again at its end.
    player.play(Track::Chord, true);
    let out = render(&mut source, 3000, RATE);
    assert_eq!(player.sounding(), Some(Track::Chord));
    let second = &out[2 * 48_000 + 9600..2 * 48_000 + 19200];
    assert!(rms(second) > 0.01, "second pass sounds");

    // Restart plays from the beginning; stop silences.
    player.restart(true);
    render(&mut source, 100, RATE);
    assert_eq!(player.sounding(), Some(Track::Chord));
    player.stop();
    render(&mut source, 500, RATE);
    assert_eq!(player.sounding(), None);
    let out = render(&mut source, 100, RATE);
    assert_eq!(rms(&out), 0.0);
}

#[test]
fn powerup_pauses_the_level_and_the_level_resumes() {
    let (mut player, mut source) =
        deck(&[(Track::Level, song(30, 60)), (Track::Powerup, song(30, 72))]);
    player.play(Track::Level, true);
    render(&mut source, 200, RATE);
    // A repeated level request keeps it going.
    player.play(Track::Level, true);
    assert_eq!(player.current(), Some(Track::Level));
    player.play(Track::Powerup, true);
    render(&mut source, 200, RATE);
    assert_eq!(player.sounding(), Some(Track::Powerup));
    // Powerup again (another powerup) restarts it.
    player.play(Track::Powerup, true);
    render(&mut source, 50, RATE);
    assert_eq!(player.sounding(), Some(Track::Powerup));
    // The level comes back where it was, not from the start.
    player.play(Track::Level, true);
    render(&mut source, 50, RATE);
    assert_eq!(player.sounding(), Some(Track::Level));
    // Unknown tracks change nothing.
    player.play(Track::Boss, true);
    render(&mut source, 50, RATE);
    assert_eq!(player.sounding(), Some(Track::Level));
    assert!(!player.has(Track::Boss));
}

#[test]
fn the_level_resumes_where_it_paused() {
    // Compare a level that pauses for a power-up with one that never did:
    // after the power-up, the resumed level matches the uninterrupted one
    // shifted by nothing, because a paused track does not advance.
    let tracks = || vec![(Track::Level, song(30, 60)), (Track::Powerup, song(30, 72))];
    let (mut a, mut sa) = deck(&tracks());
    a.play(Track::Level, true);
    let mut first = render(&mut sa, 300, RATE);
    a.play(Track::Powerup, true);
    render(&mut sa, 500, RATE);
    a.play(Track::Level, true);
    first.extend(render(&mut sa, 300, RATE));

    let (mut b, mut sb) = deck(&tracks());
    b.play(Track::Level, true);
    let straight = render(&mut sb, 600, RATE);
    // The level's synthesizer renders the same frames in both runs, and the
    // frames it had rendered but not played at the pause are kept for the
    // resume, so the two runs line up exactly.
    assert_eq!(first.len(), straight.len());
    let diff = first
        .iter()
        .zip(&straight)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max);
    assert!(diff < 1e-6, "resume continues the level: {diff}");
}

#[test]
fn the_music_switch() {
    let (mut player, mut source) = deck(&[(Track::Level, song(30, 60))]);
    player.set_enabled(false);
    player.play(Track::Level, true);
    render(&mut source, 100, RATE);
    assert_eq!(player.sounding(), None, "off: nothing plays");
    player.set_enabled(true);
    render(&mut source, 100, RATE);
    assert_eq!(
        player.sounding(),
        Some(Track::Level),
        "on: the level restarts"
    );
    player.set_enabled(false);
    render(&mut source, 100, RATE);
    assert_eq!(player.sounding(), None);
    assert!(!player.enabled());
}

#[test]
fn speed_ramps() {
    let (mut player, mut source) = deck(&[(Track::Chord, song(2, 69))]);
    player.play(Track::Chord, false);
    player.set_speed(200, 0);
    render(&mut source, 1100, RATE);
    assert_eq!(player.sounding(), None, "twice as fast: over after 1 s");
    player.set_speed(100, 500);
    player.play(Track::Chord, false);
    render(&mut source, 1500, RATE);
    assert_eq!(
        player.sounding(),
        Some(Track::Chord),
        "back to normal speed"
    );
}

#[test]
fn music_mixes_at_the_music_volume() {
    let (mut player, source) = deck(&[(Track::Level, song(30, 69))]);
    player.play(Track::Level, true);
    let mut mixer = Mixer::new(RATE);
    mixer.set_settings(&AudioSettings {
        music_volume: 100,
        ..AudioSettings::default()
    });
    mixer.set_music(Some(Box::new(source)));
    let mut loud = vec![0.0f32; 2 * 9600];
    mixer.render(&mut loud);
    mixer.render(&mut loud);
    mixer.set_settings(&AudioSettings::default());
    let mut soft = vec![0.0f32; 2 * 9600];
    mixer.render(&mut soft);
    let ratio = rms(&soft) / rms(&loud);
    let expected = curve::music_gain(50);
    assert!((ratio - expected).abs() < 0.02, "{ratio} vs {expected}");
}

#[test]
fn the_null_output_plays_music_and_works_without_it() {
    let mut engine = AudioEngine::null(AudioSettings::default());
    assert_eq!(engine.output(), Output::Null);
    let (mut player, source) = deck(&[(Track::Level, song(30, 69))]);
    engine.set_music(Some(Box::new(source)));
    player.play(Track::Level, true);
    let start = std::time::Instant::now();
    while player.sounding() != Some(Track::Level) && start.elapsed().as_secs() < 3 {
        engine.poll_device();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(player.sounding(), Some(Track::Level));
    // No music at all: the engine keeps running.
    engine.set_music(None);
    engine.play_ui("NOTHING");
    engine.poll_device();
    assert_eq!(engine.output(), Output::Null);
}
