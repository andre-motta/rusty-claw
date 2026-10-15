//! The first level's sounds decode for the audio engine (skipped without
//! `CLAW_INSTALL`): every sound the level registers, its own and the shared
//! ones, becomes playable samples, and a bridge plays the level's cues
//! without opening a device.

use claw_assets::{Install, LevelData, LevelSource};
use claw_audio::AudioSettings;
use claw_frame::{AudioCue, SoundCue};
use claw_game::audio::{AudioBridge, ViewInfo, decode_bank};

#[test]
fn every_first_level_sound_decodes() {
    let Some(path) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let install = Install::open(path).expect("install opens");
    let store = install.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(1)).expect("level 1 loads");
    let (bank, report) = decode_bank(&store, &level.registries);
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert_eq!(report.decoded, level.registries.sounds.len());
    assert_eq!(bank.len(), level.registries.sounds.len());
    for prefix in ["CLAW_", "GAME_", "LEVEL_"] {
        assert!(
            level
                .registries
                .sounds
                .keys()
                .any(|k| k.starts_with(prefix)),
            "{prefix} sounds are registered"
        );
    }
    for key in level.registries.sounds.keys() {
        let pcm = bank.get(key).expect("in the bank");
        assert!(pcm.is_playable(), "{key}");
    }
    eprintln!(
        "{} sounds, {:.1} MB, decoded in {:.0} ms",
        report.decoded,
        report.bytes as f64 / 1_048_576.0,
        report.ms
    );
    // A silent bridge loads the same bank and takes the level's cues.
    let mut bridge = AudioBridge::silent(AudioSettings::default());
    let r = bridge.load_level(&store, &level.registries);
    assert_eq!(r.decoded, report.decoded);
    let cues: Vec<AudioCue> = level
        .registries
        .sounds
        .keys()
        .take(20)
        .map(|k| {
            AudioCue::Play(SoundCue {
                key: k.clone(),
                ..SoundCue::default()
            })
        })
        .collect();
    bridge.apply(&cues, &ViewInfo::default());
    assert_eq!(
        bridge.engine().bank().map(|b| b.len()),
        Some(report.decoded)
    );
}
