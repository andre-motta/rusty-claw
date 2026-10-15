//! Opt-in device smoke test: runs only with CLAW_AUDIO_SMOKE=1.
//!
//! It opens the default output device, plays 32 quiet looping tones for two
//! seconds, prints the callback cost against the buffer length, and checks
//! that a device loss and turning audio off and on again switch between the
//! device and the silent output.

use std::sync::Arc;
use std::time::{Duration, Instant};

use claw_audio::{AudioEngine, AudioSettings, Channel, Output, Pcm, PlayRequest, SoundBank};

#[test]
fn device_smoke() {
    if std::env::var("CLAW_AUDIO_SMOKE").as_deref() != Ok("1") {
        eprintln!("skipped: set CLAW_AUDIO_SMOKE=1 to open the output device");
        return;
    }
    let mut engine = AudioEngine::new(AudioSettings::default());
    let start = Instant::now();
    while engine.output() != Output::Device && start.elapsed() < Duration::from_secs(3) {
        engine.poll_device();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(engine.output(), Output::Device, "no output device opened");

    let mut bank = SoundBank::new();
    for k in 0..32u32 {
        let period = 40 + k as usize;
        let samples: Vec<i16> = (0..period * 50)
            .map(|i| {
                let t = (i % period) as f32 / period as f32;
                ((t * std::f32::consts::TAU).sin() * 8000.0) as i16
            })
            .collect();
        bank.insert(format!("TONE{k}"), Arc::new(Pcm::mono(22050, samples)));
    }
    engine.set_bank(bank);
    for k in 0..32u32 {
        let mut r = PlayRequest::new(format!("TONE{k}"), Channel::Effect);
        r.volume = 2;
        r.looped = true;
        r.handle = Some(1);
        r.pitch = (k as i16) - 16;
        r.pan = ((k as i32 * 6) - 96) as i8;
        engine.play(&r);
    }
    let run = Instant::now();
    while run.elapsed() < Duration::from_secs(2) {
        engine.poll_device();
        std::thread::sleep(Duration::from_millis(16));
    }
    let s = engine.stats();

    // A device failure falls back to the silent output and the device comes
    // back within the retry delay, with the voices still playing.
    engine.simulate_device_loss();
    let lost = Instant::now();
    while engine.output() == Output::Device && lost.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(engine.output(), Output::Null, "loss not noticed");
    let back = Instant::now();
    while engine.output() != Output::Device && back.elapsed() < Duration::from_secs(5) {
        engine.poll_device();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(engine.output(), Output::Device, "device not reopened");
    eprintln!(
        "reopened {:.0} ms after the loss, {} voices playing",
        lost.elapsed().as_secs_f64() * 1e3,
        engine.stats().active_voices
    );
    assert_eq!(engine.stats().active_voices, 32);
    engine.stop(1);

    // Turning audio off closes the device; turning it on opens it again.
    let on = engine.settings().clone();
    engine.set_settings(AudioSettings {
        enabled: false,
        ..on.clone()
    });
    let off = Instant::now();
    while engine.output() == Output::Device && off.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(engine.output(), Output::Null, "device kept open while off");
    engine.set_settings(on);
    let again = Instant::now();
    while engine.output() != Output::Device && again.elapsed() < Duration::from_secs(5) {
        engine.poll_device();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(engine.output(), Output::Device, "device not opened again");
    let buffer_us = s.buffer_frames as f64 * 1e6 / f64::from(s.device_rate.max(1));
    eprintln!(
        "device {} Hz, buffer {} frames ({:.0} us), {} callbacks, \
         callback mean {:.1} us, max {:.1} us, headroom at max {:.1} percent, \
         {} voices playing",
        s.device_rate,
        s.buffer_frames,
        buffer_us,
        s.callbacks,
        s.mean_callback_us,
        s.max_callback_us,
        100.0 * (1.0 - s.max_callback_us / buffer_us.max(1.0)),
        s.active_voices,
    );
    assert!(s.callbacks > 0);
    assert_eq!(s.voices_started, 32);
}
