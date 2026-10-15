//! The engine on the silent output: requests flow through the queue to the
//! mixer without a device.

use std::sync::Arc;
use std::time::{Duration, Instant};

use claw_audio::{
    AudioEngine, AudioSettings, Channel, MusicSource, Output, Pcm, PlayRequest, SoundBank,
};

fn bank(keys: &[&str], frames: usize) -> SoundBank {
    let mut b = SoundBank::new();
    for (k, key) in keys.iter().enumerate() {
        let samples: Vec<i16> = (0..frames).map(|i| ((i * (k + 1)) % 200) as i16).collect();
        b.insert(*key, Arc::new(Pcm::mono(22050, samples)));
    }
    b
}

/// Polls until `done` holds, for at most 3 s.
fn wait(engine: &mut AudioEngine, done: impl Fn(&AudioEngine) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        engine.poll_device();
        if done(engine) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn null_output_runs_a_scripted_request_list() {
    let settings = AudioSettings {
        min_replay_gap_ms: 0,
        ..AudioSettings::default()
    };
    let mut engine = AudioEngine::null(settings.clone());
    assert_eq!(engine.output(), Output::Null);
    engine.set_bank(bank(&["GAME_CLICK", "CLAW_HIT", "LEVEL_AMBIENT"], 22050));

    let mut ambient = PlayRequest::new("LEVEL_AMBIENT", Channel::Ambient);
    ambient.looped = true;
    ambient.handle = Some(1);
    let mut hit = PlayRequest::new("CLAW_HIT", Channel::Effect);
    hit.pan = -40;
    hit.pitch = 20;
    hit.volume = 70;

    engine.play(&ambient);
    engine.play(&hit);
    engine.play_ui("GAME_CLICK");
    engine.play(&PlayRequest::new("NOT_REGISTERED", Channel::Effect));
    assert!(wait(&mut engine, |e| e.stats().voices_started == 3));
    assert!(wait(&mut engine, |e| e.stats().active_voices == 3));

    engine.pause_game();
    engine.play_ui("GAME_CLICK");
    engine.stop(1);
    engine.resume_game();
    assert!(wait(&mut engine, |e| e.stats().voices_started == 4));

    // The one-shots end after their 1 s length; the loop was stopped.
    assert!(wait(&mut engine, |e| e.stats().active_voices == 0));

    // Switching ambient off rejects ambient plays.
    engine.set_settings(AudioSettings {
        ambient: false,
        ..settings.clone()
    });
    engine.play(&ambient);
    engine.play(&hit);
    assert!(wait(&mut engine, |e| e.stats().voices_started == 5));

    engine.stop_all();
    // Bank, 3 plays, pause, play, stop, resume, settings, 2 plays, stop all.
    assert!(wait(&mut engine, |e| e.stats().commands == 12));
    assert!(wait(&mut engine, |e| e.stats().active_voices == 0));
    assert_eq!(engine.stats().dropped, 0);
    assert_eq!(engine.output(), Output::Null);
}

#[test]
fn bank_swap_keeps_old_voices_playing() {
    let mut engine = AudioEngine::null(AudioSettings::default());
    engine.set_bank(bank(&["A"], 22050 * 2));
    let mut a = PlayRequest::new("A", Channel::Effect);
    a.looped = true;
    a.handle = Some(5);
    engine.play(&a);
    assert!(wait(&mut engine, |e| e.stats().active_voices == 1));
    engine.set_bank(bank(&["B"], 100));
    engine.play(&PlayRequest::new("A", Channel::Effect));
    engine.play(&PlayRequest::new("B", Channel::Effect));
    assert!(wait(&mut engine, |e| e.stats().voices_started == 2));
    assert!(engine.bank().unwrap().get("A").is_none());
    engine.stop(5);
    assert!(wait(&mut engine, |e| e.stats().active_voices == 0));
}

struct Silence;

impl MusicSource for Silence {
    fn render(&mut self, _out: &mut [f32], _rate: u32) {}
}

#[test]
fn music_and_shutdown() {
    let mut engine = AudioEngine::null(AudioSettings::default());
    engine.set_music(Some(Box::new(Silence)));
    engine.set_music(Some(Box::new(Silence)));
    engine.set_music(None);
    let start = engine.stats().commands;
    assert!(wait(&mut engine, |e| e.stats().commands >= start.max(3)));
    drop(engine);
}

#[test]
fn audio_off_plays_nothing() {
    let mut engine = AudioEngine::null(AudioSettings {
        enabled: false,
        ..AudioSettings::default()
    });
    engine.set_bank(bank(&["A"], 100));
    engine.play_ui("A");
    assert!(wait(&mut engine, |e| e.stats().commands >= 1));
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(engine.stats().voices_started, 0);
}
