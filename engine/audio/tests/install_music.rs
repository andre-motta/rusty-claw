//! Music against the user's own install. The tests read the folder named by
//! `CLAW_INSTALL` at run time and skip themselves when it is unset; no
//! install file is ever copied into the repository. The rendering test also
//! needs a SoundFont named by `CLAW_SOUNDFONT` and skips without one.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use claw_assets::name::TYPE_XMI;
use claw_assets::xmi::{self, XmiEventKind, XmiSequence};
use claw_assets::{AssetStore, DirNode, FileId, Install, LevelData, LevelSource};
use claw_audio::music::convert::{BankMode, XmidiEvent, XmidiKind, read_smf, xmidi_to_smf};
use claw_audio::music::{Bank, Song, Track, music, select_file};
use claw_audio::{Mixer, MusicSource};

fn install() -> Option<Install> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    Some(Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install"))
}

fn music_files(store: &AssetStore) -> Vec<FileId> {
    let mut out = Vec::new();
    let mut stack: Vec<&DirNode> = vec![store.root()];
    while let Some(dir) = stack.pop() {
        out.extend(dir.files_of_type(TYPE_XMI));
        stack.extend(dir.dirs());
    }
    out
}

fn events(seq: &XmiSequence) -> Vec<XmidiEvent> {
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
                _ => XmidiKind::Other,
            };
            XmidiEvent::new(e.time_120hz, kind)
        })
        .collect()
}

#[test]
fn every_install_xmi_parses_and_converts() {
    let Some(install) = install() else {
        return;
    };
    let store = install.store().expect("mounting the install");
    let files = music_files(&store);
    assert!(!files.is_empty(), "the install has music");
    let mut seconds = 0.0;
    for id in &files {
        let path = store.file(*id).path().to_string();
        let data = store.read(*id).expect("readable");
        let x = xmi::parse(&data).unwrap_or_else(|e| panic!("{path}: {e}"));
        let seq = &x.sequences[0];
        assert!(seq.has_end(), "{path}");
        let ev = events(seq);
        for mode in [BankMode::GeneralMidi, BankMode::Original] {
            let smf = xmidi_to_smf(&ev, mode);
            let (_, back) = read_smf(&smf).unwrap_or_else(|| panic!("{path}: unreadable SMF"));
            // The converted file ends exactly at the sequence's end, and
            // every note-on has its note-off.
            assert_eq!(back.last().unwrap().tick, seq.end_time_120hz(), "{path}");
            let ons = back.iter().filter(|e| e.bytes[0] & 0xF0 == 0x90).count();
            let offs = back.iter().filter(|e| e.bytes[0] & 0xF0 == 0x80).count();
            assert_eq!(ons, offs, "{path}");
            let song = Song::from_smf(&smf).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert!((song.seconds() - seq.seconds()).abs() < 1e-6, "{path}");
        }
        seconds += seq.seconds();
    }
    eprintln!(
        "{} music files parse and convert, {:.0} s of music",
        files.len(),
        seconds
    );

    // Community custom levels: their music is read without a crash; the
    // files that do not follow the format are reported.
    let mut custom = Vec::new();
    if let Some(dir) = install.custom_dir() {
        walk(dir, &mut custom);
    }
    let mut good = 0;
    for path in &custom {
        let data = fs::read(path).unwrap();
        if let Ok(x) = xmi::parse(&data) {
            let _ = xmidi_to_smf(&events(&x.sequences[0]), BankMode::GeneralMidi);
            good += 1;
        }
    }
    eprintln!(
        "{good} of {} community level music files read",
        custom.len()
    );
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("xmi"))
        {
            out.push(p);
        }
    }
}

#[test]
fn level1_selects_the_general_midi_arrangement() {
    let Some(install) = install() else {
        return;
    };
    let store = install.store().expect("mounting the install");
    let level = LevelData::build(&store, LevelSource::Retail(1)).expect("LEVEL1 loads");
    let id = level.registries.music["LEVEL"];
    assert_eq!(store.file(id).name(), "PLAY0");
    let dir = store
        .folder("LEVEL1/MUSIC")
        .unwrap()
        .expect("LEVEL1 has music");
    let names: Vec<&str> = dir
        .files_of_type(TYPE_XMI)
        .map(|id| store.file(id).name())
        .collect();
    assert_eq!(
        select_file(Track::Level, BankMode::GeneralMidi, &names),
        Some("PLAY0")
    );
    assert_eq!(
        select_file(Track::Level, BankMode::Original, &names),
        Some("PLAY")
    );
    assert_eq!(
        select_file(Track::Boss, BankMode::GeneralMidi, &names),
        None
    );
}

#[test]
fn level_music_renders_with_a_soundfont() {
    let Some(install) = install() else {
        return;
    };
    let Some(sf) = std::env::var_os("CLAW_SOUNDFONT") else {
        eprintln!("CLAW_SOUNDFONT is not set; skipping the render");
        return;
    };
    let t = Instant::now();
    let bank = Bank::load(Path::new(&sf)).expect("the SoundFont loads");
    let load_ms = t.elapsed().as_secs_f64() * 1000.0;
    let store = install.store().expect("mounting the install");
    let level = LevelData::build(&store, LevelSource::Retail(1)).expect("LEVEL1 loads");
    let data = store.read(level.registries.music["LEVEL"]).unwrap();
    let x = xmi::parse(&data).unwrap();
    let t = Instant::now();
    let song = Song::from_xmidi(&events(&x.sequences[0]), bank.mode).unwrap();
    let convert_ms = t.elapsed().as_secs_f64() * 1000.0;
    let rate = 48_000;
    let (mut player, source) = music(&bank, &[(Track::Level, song)], rate).unwrap();
    player.play(Track::Level, true);

    // Through the mixer at the default music volume, in 10 ms blocks like an
    // audio callback.
    let mut mixer = Mixer::new(rate);
    mixer.set_music(Some(Box::new(source)));
    let block = (rate / 100) as usize;
    let mut buf = vec![0.0f32; 2 * block];
    let mut sum = 0.0f64;
    let mut peak = 0.0f32;
    for _ in 0..200 {
        mixer.render(&mut buf);
        for s in &buf {
            sum += f64::from(s * s);
            peak = peak.max(s.abs());
        }
    }
    let rms = (sum / (200 * buf.len()) as f64).sqrt();
    eprintln!("2 s of LEVEL1 music: rms {rms:.4}, peak {peak:.3}");
    assert!(rms > 1e-3, "the level music is silent");

    // Synthesis cost per second of audio, in the same blocks.
    let seconds = 30;
    let t = Instant::now();
    for _ in 0..seconds * 100 {
        mixer.render(&mut buf);
    }
    let cpu = t.elapsed().as_secs_f64() / f64::from(seconds);
    eprintln!(
        "music synthesis: {cpu:.4} CPU s per audio s ({:.1} percent of a core), \
         {:.4} at the Deck proxy x3 ({:.1} percent); bank load {load_ms:.0} ms, \
         LEVEL1 conversion {convert_ms:.1} ms",
        cpu * 100.0,
        cpu * 3.0,
        cpu * 300.0
    );

    // The raw synthesizer at full music volume keeps headroom.
    let (mut player, mut source) = music(
        &bank,
        &[(
            Track::Level,
            Song::from_xmidi(&events(&x.sequences[0]), bank.mode).unwrap(),
        )],
        rate,
    )
    .unwrap();
    player.play(Track::Level, true);
    let mut full_peak = 0.0f32;
    for _ in 0..6000 {
        buf.fill(0.0);
        source.render(&mut buf, rate);
        full_peak = buf.iter().fold(full_peak, |m, s| m.max(s.abs()));
    }
    eprintln!(
        "first 60 s at full music volume: peak {:.1} dBFS",
        20.0 * full_peak.max(1e-9).log10()
    );
}
