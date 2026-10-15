//! Sound objects on the first level of the user's own install. The tests
//! read the folder named by the `CLAW_INSTALL` environment variable at run
//! time and skip themselves when it is unset; nothing from an install is
//! stored in the repository. Objects are found through the level's own
//! records.

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::claw_frame::{AudioCue, SoundChannel, SoundCue};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::sound;
use claw_sim::logic::sound_object::{self, AmbientKind};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

fn level1(store: &AssetStore) -> (LevelData, Arc<SimLevel>) {
    let data = LevelData::build(store, LevelSource::Retail(1)).unwrap();
    let level = SimLevel::from_level(&data, store).unwrap();
    (data, Arc::new(level))
}

const TRIGGERS: [&str; 3] = ["SoundTrigger", "BigSoundTrigger", "SmallSoundTrigger"];
const DIALOG: [&str; 2] = ["ClawDialogSmallSoundTrigger", "ClawDialogBigSoundTrigger"];

fn records<'a>(l: &'a SimLevel, logics: &'a [&str]) -> impl Iterator<Item = &'a ObjectSpec> {
    l.objects
        .iter()
        .filter(move |o| logics.contains(&o.logic.as_str()))
}

fn plays(cues: &[AudioCue]) -> impl Iterator<Item = &SoundCue> {
    cues.iter().filter_map(|c| match c {
        AudioCue::Play(s) => Some(s),
        _ => None,
    })
}

/// The cues of a fresh run in which Claw is put at (x, y) after two ticks
/// and left there for `n` more. A trigger at the start point counts as
/// entered when Claw appears in it.
fn cues_at(l: &Arc<SimLevel>, x: i32, y: i32, n: usize) -> Vec<AudioCue> {
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    let mut cues: Vec<AudioCue> = (0..2)
        .flat_map(|_| sim.step(TickInput::default()).audio)
        .collect();
    sim.teleport_player(x, y);
    cues.extend((0..n).flat_map(|_| sim.step(TickInput::default()).audio));
    cues
}

#[test]
fn install_level1_each_trigger_plays_when_claw_enters() {
    let Some(store) = store() else { return };
    let (_, l) = level1(&store);
    let mut checked = 0;
    for (logics, channel) in [
        (&TRIGGERS[..], SoundChannel::Effect),
        (&DIALOG[..], SoundChannel::Voice),
    ] {
        for r in records(&l, logics) {
            let (x, y) = (r.record[5], r.record[6]);
            assert!(l.sounds.contains_key(&r.anim), "{} {}", r.logic, r.anim);
            let cues = cues_at(&l, x, y, 8);
            let want = SoundCue {
                key: r.anim.clone(),
                channel,
                ..SoundCue::default()
            };
            assert!(
                plays(&cues).any(|s| *s == want),
                "{} at ({x}, {y}) did not play {}: {cues:?}",
                r.logic,
                r.anim
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 25 + 21 + 7 + 12 + 1);
}

#[test]
fn install_level1_global_loops_start_at_level_load() {
    let Some(store) = store() else { return };
    let (_, l) = level1(&store);
    let globals: Vec<&ObjectSpec> = records(&l, &["GlobalAmbientSound"]).collect();
    assert_eq!(globals.len(), 4);
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    let first = sim.step(TickInput::default()).audio;
    let m = sound_object::manager_of(sim.objects()).expect("an ambient manager");
    let mut heard: BTreeSet<String> = BTreeSet::new();
    for g in &globals {
        let entry = m
            .entries
            .iter()
            .find(|e| e.key == g.anim && e.owner.is_none())
            .unwrap_or_else(|| panic!("{} is registered", g.anim));
        match entry.kind {
            AmbientKind::Area { area, .. } => {
                assert!(!area.is_set(), "{} plays everywhere", g.anim);
                // Heard at once, at the record's percent.
                let want = sound_object::ambient_volume(100, g.record[16]) as u8;
                assert!(
                    plays(&first).any(|s| s.key == g.anim
                        && s.looped
                        && s.channel == SoundChannel::Ambient
                        && s.volume == want
                        && s.voice.is_some()),
                    "{} starts in the first tick: {first:?}",
                    g.anim
                );
                heard.insert(g.anim.clone());
            }
            AmbientKind::Timed { on_phase, .. } => assert!(on_phase, "{}", g.anim),
            AmbientKind::Spot { .. } => panic!("{} is not a spot", g.anim),
        }
    }
    assert!(!heard.is_empty());
    // The timed loops come in after two off periods (at most two minutes).
    for _ in 0..ticks(121_000) {
        for s in plays(&sim.step(TickInput::default()).audio) {
            if s.channel == SoundChannel::Ambient {
                heard.insert(s.key.clone());
            }
        }
    }
    for g in &globals {
        assert!(heard.contains(&g.anim), "{} was never heard", g.anim);
    }
}

#[test]
fn install_level1_walk_emits_only_registered_sounds() {
    let Some(store) = store() else { return };
    let (data, l) = level1(&store);
    // The exclamation mark of Claw's lines is an image set of the level.
    assert!(
        data.registries
            .images
            .get(sound::EXCLAMATION_IMAGE_SET)
            .is_some_and(|s| s.frames.contains_key(&sound::EXCLAMATION_FRAME))
    );
    assert!(
        l.image_sets()
            .contains(&sound::EXCLAMATION_IMAGE_SET.to_string())
    );
    let walk = script(&[
        (RIGHT, 6000),
        (RIGHT | JUMP, 700),
        (RIGHT, 9000),
        (LEFT, 4000),
        (RIGHT | JUMP, 700),
        (RIGHT, 12000),
        (0, 1000),
        (RIGHT | JUMP, 700),
        (RIGHT, 20000),
    ]);
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    let mut keys = BTreeSet::new();
    let mut voices = BTreeSet::new();
    let mut live = BTreeSet::new();
    for &input in walk.ticks.iter().take(3000) {
        for c in sim.step(input).audio {
            match c {
                AudioCue::Play(s) => {
                    if let Some(v) = s.voice {
                        voices.insert(v);
                        live.insert(v);
                    }
                    keys.insert(s.key);
                }
                AudioCue::Stop { voice } => {
                    assert!(live.remove(&voice), "stop of a voice that is not playing");
                }
                _ => {}
            }
        }
    }
    assert_eq!(sim.tick(), 3000);
    for k in &keys {
        assert!(l.sounds.contains_key(k), "{k} is not a sound of the level");
    }
    assert!(
        keys.iter().any(|k| k.starts_with("LEVEL_TRIGGER_")),
        "{keys:?}"
    );
    assert!(
        keys.iter().any(|k| k.starts_with("LEVEL_AMBIENT_")),
        "{keys:?}"
    );
    println!(
        "LEVEL1 walk: {} sound keys, {} looping voices: {keys:?}",
        keys.len(),
        voices.len()
    );
}
