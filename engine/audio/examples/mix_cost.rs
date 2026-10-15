//! Offline mix cost: 32 voices rendered for one second of 48 kHz stereo, in
//! device-sized blocks. Run with `cargo run --release -p claw-audio --example
//! mix_cost`.

use std::sync::Arc;
use std::time::Instant;

use claw_audio::{AudioSettings, Channel, Mixer, Pcm, PlayRequest, SoundBank};

const RATE: u32 = 48000;
const BLOCK: usize = 512;
const VOICES: usize = 32;
const RUNS: usize = 20;

fn main() {
    let mut bank = SoundBank::new();
    for k in 0..VOICES {
        let samples: Vec<i16> = (0..22050 + k * 100)
            .map(|i| (((i * (k + 7)) % 6000) as i32 - 3000) as i16)
            .collect();
        let pcm = if k % 4 == 0 {
            Pcm {
                rate: 22050,
                channels: 2,
                samples: samples.into(),
            }
        } else {
            Pcm::mono(if k % 2 == 0 { 22050 } else { 11025 }, samples)
        };
        bank.insert(format!("S{k}"), Arc::new(pcm));
    }
    let settings = AudioSettings {
        min_replay_gap_ms: 0,
        ..AudioSettings::default()
    };
    let mut out = vec![0.0f32; 2 * BLOCK];
    let mut best = f64::MAX;
    let mut checksum = 0.0f64;
    for _ in 0..RUNS {
        let mut mixer = Mixer::new(RATE);
        mixer.set_settings(&settings);
        for k in 0..VOICES {
            let mut r = PlayRequest::new(format!("S{k}"), Channel::Effect);
            r.looped = true;
            r.pitch = (k as i16 * 7) - 100;
            r.pan = (k as i8 * 6) - 96;
            r.volume = 40 + k as u8;
            mixer.play(&r, &bank).expect("voice");
        }
        let start = Instant::now();
        let mut frames = 0usize;
        while frames < RATE as usize {
            mixer.render(&mut out);
            frames += BLOCK;
        }
        let secs = start.elapsed().as_secs_f64();
        best = best.min(secs);
        checksum += out.iter().map(|s| f64::from(*s)).sum::<f64>();
        assert_eq!(mixer.active_voices(), VOICES);
    }
    let block_us = BLOCK as f64 * 1e6 / f64::from(RATE);
    let per_block_us = best * 1e6 / (RATE as f64 / BLOCK as f64);
    println!(
        "{VOICES} voices, 1 s at {RATE} Hz: best {:.3} ms ({:.0}x real time); \
         {:.1} us per {BLOCK}-frame block of {:.0} us (checksum {checksum:.3})",
        best * 1e3,
        1.0 / best,
        per_block_us,
        block_us,
    );
}
