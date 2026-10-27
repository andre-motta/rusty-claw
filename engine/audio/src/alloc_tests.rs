//! Proves that playing, stopping and rendering never touch the heap.
//!
//! A counting global allocator records allocations and frees made on the
//! current thread while counting is switched on.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use crate::bank::{Pcm, SoundBank};
use crate::mixer::{Mixer, MusicSource, PlayParams};
use crate::settings::{AudioSettings, Channel, MixSettings, PlayRequest};
use crate::state::{AudioState, Command, Garbage, Shared};

struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

fn note() {
    let _ = ON.try_with(|on| {
        if on.get() {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        }
    });
}

// SAFETY: every call forwards to the system allocator unchanged; the counter
// only touches const-initialised thread locals, which never allocate.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note();
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note();
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Heap operations made by `f` on this thread.
fn heap_ops(f: impl FnOnce()) -> u64 {
    COUNT.with(|c| c.set(0));
    ON.with(|on| on.set(true));
    f();
    ON.with(|on| on.set(false));
    COUNT.with(|c| c.get())
}

struct Saw(f32);

impl MusicSource for Saw {
    fn render(&mut self, out: &mut [f32], _rate: u32) {
        for s in out.iter_mut() {
            self.0 = (self.0 + 0.001) % 0.2;
            *s = self.0;
        }
    }
}

fn test_bank() -> SoundBank {
    let mut bank = SoundBank::new();
    for k in 0..40 {
        let samples: Vec<i16> = (0..2000 + k * 50)
            .map(|i| ((i * (k + 3)) % 4000) as i16 - 2000)
            .collect();
        let pcm = if k % 3 == 0 {
            Pcm {
                rate: 22050,
                channels: 2,
                samples: samples.into(),
            }
        } else {
            Pcm::mono(11025, samples)
        };
        bank.insert(format!("S{k}"), Arc::new(pcm));
    }
    bank
}

fn params(k: usize) -> PlayParams {
    PlayParams {
        volume: 20 + (k as u8 % 80),
        pan: (k as i8 % 100) - 50,
        pitch: (k as i16 % 60) - 30,
        looped: k.is_multiple_of(4),
        channel: if k.is_multiple_of(5) {
            Channel::Interface
        } else {
            Channel::Effect
        },
        handle: Some(k as u32 % 7),
    }
}

#[test]
fn mixer_render_does_not_allocate() {
    let bank = test_bank();
    let mut mixer = Mixer::new(48000);
    mixer.set_settings(&AudioSettings {
        min_replay_gap_ms: 0,
        voice_cap: 32,
        ..AudioSettings::default()
    });
    mixer.set_music(Some(Box::new(Saw(0.0))));
    mixer.set_movie(Some(Box::new(Saw(0.05))));
    let mut out = vec![0.0f32; 2 * 4096];
    let ids: Vec<_> = (0..40)
        .map(|k| bank.id(&format!("S{k}")).unwrap())
        .collect();
    let req = PlayRequest::new("S1", Channel::Effect);

    let ops = heap_ops(|| {
        for (k, id) in ids.iter().enumerate() {
            mixer.play_sound(*id, params(k), &bank);
        }
        // Playing by name looks the name up without allocating.
        mixer.play(&req, &bank);
        for round in 0..50 {
            mixer.render(&mut out[..2 * (64 + round * 37)]);
            if round == 10 {
                mixer.pause_game();
            }
            if round == 20 {
                mixer.resume_game();
                mixer.stop(3);
            }
            if round == 30 {
                mixer.set_out_rate(44100);
            }
        }
        mixer.stop_all();
        mixer.render(&mut out);
    });
    assert_eq!(ops, 0, "heap operations during play and render");
}

#[test]
fn output_side_does_not_allocate() {
    let (mut tx, rx) = rtrb::RingBuffer::new(256);
    let (gtx, mut grx) = rtrb::RingBuffer::new(4);
    let shared = Arc::new(Shared::default());
    let mut state = AudioState::new(Mixer::new(48000), rx, gtx, shared);
    let bank = Arc::new(test_bank());
    let keep = Arc::clone(&bank);
    let ids: Vec<_> = (0..40)
        .map(|k| bank.id(&format!("S{k}")).unwrap())
        .collect();
    let mix = MixSettings::from(&AudioSettings {
        min_replay_gap_ms: 0,
        ..AudioSettings::default()
    });
    // Everything the game side would send, built before counting.
    let mut cmds = vec![
        Command::Settings(mix),
        Command::Bank { epoch: 1, bank },
        Command::Music(Some(Box::new(Saw(0.0)))),
        Command::Movie(Some(Box::new(Saw(0.05)))),
    ];
    for (k, id) in ids.iter().enumerate() {
        cmds.push(Command::Play {
            epoch: 1,
            id: *id,
            params: params(k),
        });
    }
    cmds.push(Command::PauseGame);
    cmds.push(Command::ResumeGame);
    cmds.push(Command::HoldMusic(true));
    cmds.push(Command::HoldMusic(false));
    cmds.push(Command::Stop(2));
    cmds.push(Command::Music(Some(Box::new(Saw(0.1)))));
    cmds.push(Command::Movie(None));
    for c in cmds {
        assert!(tx.push(c).is_ok());
    }
    let mut device = vec![0i16; 6 * 1500];
    let mut device_f = vec![0.0f32; 700];

    let ops = heap_ops(|| {
        state.fill(&mut device, 6);
        state.fill(&mut device_f, 1);
        state.run_silent(3000);
        state.fill(&mut device, 6);
    });
    assert_eq!(ops, 0, "heap operations on the output side");
    // The replaced music source and the cleared movie soundtrack came back
    // to be dropped here.
    assert!(matches!(grx.pop(), Ok(Garbage::Music(_))));
    assert!(matches!(grx.pop(), Ok(Garbage::Movie(_))));
    assert!(grx.pop().is_err());
    drop(keep);
}

#[test]
fn a_cleared_movie_comes_back_to_be_dropped() {
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A soundtrack that notes on which thread it was dropped.
    struct Noted(Arc<AtomicBool>);
    impl MusicSource for Noted {
        fn render(&mut self, out: &mut [f32], _rate: u32) {
            out.fill(0.1);
        }
    }
    impl Drop for Noted {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }

    let (mut tx, rx) = rtrb::RingBuffer::new(8);
    let (gtx, mut grx) = rtrb::RingBuffer::new(4);
    let shared = Arc::new(Shared::default());
    let mut state = AudioState::new(Mixer::new(48000), rx, gtx, shared);
    let dropped = Arc::new(AtomicBool::new(false));
    assert!(
        tx.push(Command::Movie(Some(Box::new(Noted(Arc::clone(&dropped))))))
            .is_ok()
    );
    state.run_silent(100);
    assert!(state.mixer.has_movie());
    assert!(tx.push(Command::Movie(None)).is_ok());
    state.run_silent(100);
    assert!(!state.mixer.has_movie());
    assert!(
        !dropped.load(Ordering::Relaxed),
        "not dropped on the output side"
    );
    let back = grx.pop();
    assert!(matches!(back, Ok(Garbage::Movie(_))));
    drop(back);
    assert!(dropped.load(Ordering::Relaxed));
}

#[test]
fn music_render_does_not_allocate() {
    use crate::music::convert::{BankMode, XmidiEvent, XmidiKind};
    use crate::music::{Bank, Song, Track, music, soundfont};

    let bank = Bank::from_bytes(&soundfont::build_test_bank()).unwrap();
    let mut events = Vec::new();
    for k in 0..40u32 {
        events.push(XmidiEvent::new(
            k * 6,
            XmidiKind::Note {
                channel: (k % 16) as u8,
                key: 40 + (k % 40) as u8,
                velocity: 100,
                duration: 30,
            },
        ));
        events.push(XmidiEvent::new(
            k * 6 + 1,
            XmidiKind::Controller {
                channel: (k % 16) as u8,
                controller: 7,
                value: (k * 3 % 128) as u8,
            },
        ));
    }
    events.push(XmidiEvent::new(260, XmidiKind::EndOfTrack));
    let song = Song::from_xmidi(&events, BankMode::GeneralMidi).unwrap();
    let (mut player, mut source) = music(
        &bank,
        &[(Track::Level, song.clone()), (Track::Powerup, song)],
        48000,
    )
    .unwrap();
    let mut out = vec![0.0f32; 2 * 1024];

    let ops = heap_ops(|| {
        player.play(Track::Level, true);
        for round in 0..200 {
            let n = 64 + (round * 37) % 900;
            let rate = if round % 3 == 0 { 44100 } else { 48000 };
            source.render(&mut out[..2 * n], rate);
            match round {
                50 => player.play(Track::Powerup, true),
                80 => player.set_speed(150, 500),
                120 => player.play(Track::Level, true),
                150 => player.stop(),
                160 => player.restart(false),
                _ => {}
            }
        }
    });
    assert_eq!(ops, 0, "heap operations while synthesizing");
}

#[test]
fn counter_sees_heap_operations() {
    let ops = heap_ops(|| {
        let v = std::hint::black_box(vec![1u8; 64]);
        drop(v);
    });
    assert_eq!(ops, 2);
}
