//! A movie held in the background: a small movie made here (an AVI file of
//! 32x24 pictures at 10 a second, raw BGR, with stereo 16-bit sound at
//! 8000 Hz) plays through the window's movie playback with an audio engine
//! on its silent output, which plays the soundtrack in real time. Held, it
//! shows no new picture and its sound stands still; let go, it goes on from
//! where it held rather than jumping ahead by the time it was held. A build
//! without FFmpeg has no movie player, so there the test says it skipped.

use std::thread::sleep;
use std::time::{Duration, Instant};

use claw_audio::AudioSettings;
use claw_game::audio::AudioBridge;
use claw_game::movie::{MovieEnd, MoviePlayback};
use claw_movie::{FileKind, Source};

const W: u32 = 32;
const H: u32 = 24;
const FPS: u32 = 10;
const FRAMES: u32 = 20;
const SOUND_RATE: u32 = 8000;

fn chunk(out: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) -> usize {
    let at = out.len();
    out.extend_from_slice(id);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
    at
}

fn list(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    let mut data = kind.to_vec();
    data.extend_from_slice(body);
    chunk(out, b"LIST", &data);
}

fn le(v: &mut Vec<u8>, words: &[u32]) {
    for w in words {
        v.extend_from_slice(&w.to_le_bytes());
    }
}

/// The test movie as AVI bytes: each picture one grey, its sound a quiet
/// square wave stored right after it.
fn movie() -> Vec<u8> {
    let image = (W * 3 * H) as usize;
    let per_frame = SOUND_RATE / FPS;
    let block = 4u32;
    let mut avih = Vec::new();
    le(
        &mut avih,
        &[
            1_000_000 / FPS,
            0,
            0,
            0x10 | 0x100,
            FRAMES,
            0,
            2,
            image as u32,
            W,
            H,
            0,
            0,
            0,
            0,
        ],
    );
    let mut vstrh = b"vids".to_vec();
    le(
        &mut vstrh,
        &[
            0,
            0,
            0,
            0,
            1,
            FPS,
            0,
            FRAMES,
            image as u32,
            u32::MAX,
            0,
            0,
            0,
        ],
    );
    let mut vstrf = Vec::new();
    le(&mut vstrf, &[40, W, H]);
    vstrf.extend_from_slice(&1u16.to_le_bytes());
    vstrf.extend_from_slice(&24u16.to_le_bytes());
    le(&mut vstrf, &[0, image as u32, 0, 0, 0, 0]);
    let mut astrh = b"auds".to_vec();
    le(
        &mut astrh,
        &[
            0,
            0,
            0,
            0,
            block,
            SOUND_RATE * block,
            0,
            FRAMES * per_frame,
            per_frame * block,
            u32::MAX,
            block,
            0,
            0,
        ],
    );
    let mut astrf = Vec::new();
    astrf.extend_from_slice(&1u16.to_le_bytes());
    astrf.extend_from_slice(&2u16.to_le_bytes());
    le(&mut astrf, &[SOUND_RATE, SOUND_RATE * block]);
    astrf.extend_from_slice(&(block as u16).to_le_bytes());
    astrf.extend_from_slice(&16u16.to_le_bytes());
    astrf.extend_from_slice(&0u16.to_le_bytes());

    let mut vstrl = Vec::new();
    chunk(&mut vstrl, b"strh", &vstrh);
    chunk(&mut vstrl, b"strf", &vstrf);
    let mut astrl = Vec::new();
    chunk(&mut astrl, b"strh", &astrh);
    chunk(&mut astrl, b"strf", &astrf);
    let mut hdrl = Vec::new();
    chunk(&mut hdrl, b"avih", &avih);
    list(&mut hdrl, b"strl", &vstrl);
    list(&mut hdrl, b"strl", &astrl);

    let mut movi = b"movi".to_vec();
    let mut idx = Vec::new();
    for i in 0..FRAMES {
        let pixels = vec![(i * 12) as u8; image];
        let at = chunk(&mut movi, b"00db", &pixels);
        idx.extend_from_slice(b"00db");
        le(&mut idx, &[0x10, at as u32, pixels.len() as u32]);
        let mut sound = Vec::new();
        for n in 0..per_frame {
            let s: i16 = if (n / 20) % 2 == 0 { 2000 } else { -2000 };
            sound.extend_from_slice(&s.to_le_bytes());
            sound.extend_from_slice(&s.to_le_bytes());
        }
        let at = chunk(&mut movi, b"01wb", &sound);
        idx.extend_from_slice(b"01wb");
        le(&mut idx, &[0x10, at as u32, sound.len() as u32]);
    }

    let mut body = b"AVI ".to_vec();
    list(&mut body, b"hdrl", &hdrl);
    list(&mut body, b"movi", &movi[4..]);
    chunk(&mut body, b"idx1", &idx);
    let mut out = Vec::new();
    chunk(&mut out, b"RIFF", &body);
    out
}

/// Runs frames of 10 ms until `until` says stop or `limit` passes; true
/// when `until` said stop.
fn frames_until(
    m: &mut MoviePlayback,
    limit: Duration,
    mut until: impl FnMut(&mut MoviePlayback) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if until(m) {
            return true;
        }
        sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn a_movie_held_in_the_background_goes_on_from_where_it_held() {
    if !claw_movie::available() {
        eprintln!("skipped: built without FFmpeg");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("held.avi");
    std::fs::write(&path, movie()).unwrap();
    let source = Source::File {
        path,
        kind: FileKind::Original,
    };
    let mut audio = AudioBridge::silent(AudioSettings::default());
    let mut m = MoviePlayback::start(&[source], Some(&mut audio)).expect("the movie opens");

    // It plays on its soundtrack's clock, a few pictures in.
    let mut shown = 0;
    assert!(
        frames_until(&mut m, Duration::from_secs(5), |m| {
            shown += usize::from(m.frame().is_some());
            m.clock().is_some_and(|c| c >= Duration::from_millis(500))
        }),
        "the movie plays"
    );
    assert!(shown > 0);
    assert!(m.sound_position().is_some());

    // Held: no picture, no end, and the sound stands still.
    m.hold(true);
    m.hold(true);
    sleep(Duration::from_millis(50));
    let held_at = m.sound_position().unwrap();
    let hold = Duration::from_millis(1500);
    let start = Instant::now();
    while start.elapsed() < hold {
        assert!(m.frame().is_none(), "no picture while held");
        assert!(m.clock().is_none());
        assert_eq!(m.end(), None, "a held movie does not end");
        sleep(Duration::from_millis(10));
    }
    assert_eq!(m.sound_position(), Some(held_at), "its sound held");

    // Let go: it goes on from where it held, not 1.5 s further on (the
    // whole movie is 2 s long).
    m.hold(false);
    m.hold(false);
    assert!(
        frames_until(&mut m, Duration::from_secs(2), |m| m.clock().is_some()),
        "its sound goes on"
    );
    let at = m.clock().unwrap();
    assert!(
        at < held_at + Duration::from_millis(600),
        "it went on at {at:?}, held at {held_at:?}"
    );
    assert!(
        frames_until(&mut m, Duration::from_secs(1), |m| m.frame().is_some()),
        "pictures again"
    );
    assert_eq!(m.end(), None, "it is not over yet");

    // And it plays to its end.
    assert!(frames_until(&mut m, Duration::from_secs(10), |m| {
        let _ = m.frame();
        m.done()
    }));
    assert_eq!(m.end(), Some(MovieEnd::Finished));
    m.stop(Some(&mut audio));
}
