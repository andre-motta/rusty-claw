//! The player on a small movie made here: a 64x48 picture at 10 pictures a
//! second (raw BGR) with stereo 16-bit sound at 8000 Hz, in an AVI file
//! written by this test. Every picture has its own two colours, top and
//! bottom, and the sound is a 440 Hz tone, opposite on the two sides.
#![cfg(feature = "ffmpeg")]

use std::f32::consts::TAU;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use claw_movie::{FileKind, FrameLayout, MovieError, Player, Source, VideoFrame};

const W: u32 = 64;
const H: u32 = 48;
const FPS: u32 = 10;
const FRAMES: u32 = 20;
const SOUND_RATE: u32 = 8000;
const OUT_RATE: u32 = 48000;

fn top(i: u32) -> [u8; 3] {
    [(i * 10) as u8, 200, 50]
}

fn bottom(i: u32) -> [u8; 3] {
    [30, (i * 12) as u8, 220]
}

fn tone(n: u32) -> f32 {
    0.5 * (TAU * 440.0 * n as f32 / SOUND_RATE as f32).sin()
}

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

/// The test movie as AVI bytes.
fn movie() -> Vec<u8> {
    movie_with(FRAMES, 0)
}

/// A test movie of `frames` pictures as AVI bytes, each picture's sound
/// stored `audio_delay` pictures after it in the file.
fn movie_with(frames: u32, audio_delay: u32) -> Vec<u8> {
    let row = (W * 3) as usize;
    let image = row * H as usize;
    let per_frame = SOUND_RATE / FPS;
    // Headers.
    let mut avih = Vec::new();
    le(
        &mut avih,
        &[
            1_000_000 / FPS,
            0,
            0,
            0x10 | 0x100,
            frames,
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
            frames,
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
    let block = 4u32;
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
            frames * per_frame,
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

    // The pictures and the sound, interleaved, and their index.
    let mut movi = b"movi".to_vec();
    let mut idx = Vec::new();
    let sound_chunk = |i: u32| {
        let mut sound = Vec::new();
        for n in i * per_frame..(i + 1) * per_frame {
            let s = (tone(n) * 32767.0) as i16;
            sound.extend_from_slice(&s.to_le_bytes());
            sound.extend_from_slice(&(-s).to_le_bytes());
        }
        sound
    };
    let mut next_sound = 0;
    for i in 0..frames {
        let mut pixels = Vec::with_capacity(image);
        // Bottom-up rows of blue, green, red.
        for y in (0..H).rev() {
            let [r, g, b] = if y < H / 2 { top(i) } else { bottom(i) };
            for _ in 0..W {
                pixels.extend_from_slice(&[b, g, r]);
            }
        }
        let at = chunk(&mut movi, b"00db", &pixels);
        idx.extend_from_slice(b"00db");
        le(&mut idx, &[0x10, at as u32, pixels.len() as u32]);
        // The sound of the picture `audio_delay` pictures back, and all
        // that is left after the last picture.
        let upto = if i + 1 == frames {
            frames
        } else {
            (i + 1).saturating_sub(audio_delay)
        };
        while next_sound < upto {
            let sound = sound_chunk(next_sound);
            let at = chunk(&mut movi, b"01wb", &sound);
            idx.extend_from_slice(b"01wb");
            le(&mut idx, &[0x10, at as u32, sound.len() as u32]);
            next_sound += 1;
        }
    }

    let mut body = b"AVI ".to_vec();
    list(&mut body, b"hdrl", &hdrl);
    list(&mut body, b"movi", &movi[4..]);
    chunk(&mut body, b"idx1", &idx);
    let mut out = Vec::new();
    chunk(&mut out, b"RIFF", &body);
    out
}

fn open_cursor() -> Player {
    Player::open_reader(Box::new(Cursor::new(movie())), "test.avi", OUT_RATE).unwrap()
}

fn check_picture(i: u32, f: &VideoFrame) {
    assert_eq!((f.width, f.height), (W, H));
    assert_eq!(f.layout, FrameLayout::Rgba);
    let [r, g, b] = top(i);
    for (x, y) in [(0, 0), (W - 1, 0), (W / 2, H / 2 - 1)] {
        assert_eq!(f.rgba(x, y), Some([r, g, b, 255]), "picture {i} at {x},{y}");
    }
    let [r, g, b] = bottom(i);
    for (x, y) in [(0, H / 2), (W - 1, H - 1)] {
        assert_eq!(f.rgba(x, y), Some([r, g, b, 255]), "picture {i} at {x},{y}");
    }
    let want = Duration::from_millis(u64::from(i) * 1000 / u64::from(FPS));
    assert!(
        f.pts.abs_diff(want) < Duration::from_millis(1),
        "picture {i} at {:?}",
        f.pts
    );
}

/// Reads every picture and plays the soundtrack by hand, 10 ms at a time,
/// until the movie is over; checks the pictures, the sound and the clock.
fn play_through(mut p: Player) {
    assert_eq!(p.size(), (W, H));
    assert!((p.display_aspect() - 4.0 / 3.0).abs() < 1e-6);
    let mut audio = p.take_audio().expect("the movie has sound");
    assert!(
        p.take_audio().is_none(),
        "the soundtrack is handed out once"
    );

    // Let the decoder get ahead (it stops a few pictures ahead), then play
    // 100 ms: the clock is the sound.
    std::thread::sleep(Duration::from_millis(300));
    let mut out = vec![0.0f32; 2 * (OUT_RATE as usize / 10)];
    audio.render(&mut out, OUT_RATE);
    assert_eq!(p.audio_position(), Some(Duration::from_millis(100)));
    assert_eq!(p.clock(), Duration::from_millis(100));

    let mut peak = 0.0f32;
    let mut opposite = true;
    let mut pictures = Vec::new();
    let mut out = vec![0.0f32; 2 * (OUT_RATE as usize / 100)];
    let deadline = Instant::now() + Duration::from_secs(20);
    while !p.finished() {
        assert!(Instant::now() < deadline, "the movie never finished");
        while let Some(f) = p.decoded_frame(Duration::from_millis(1)) {
            pictures.push(f);
        }
        out.fill(0.0);
        audio.render(&mut out, OUT_RATE);
        for lr in out.chunks_exact(2) {
            peak = peak.max(lr[0].abs());
            opposite &= (lr[0] + lr[1]).abs() < 1e-3;
        }
    }
    assert_eq!(pictures.len(), FRAMES as usize);
    for (i, f) in pictures.iter().enumerate() {
        check_picture(i as u32, f);
    }
    assert!(pictures.windows(2).all(|w| w[0].pts < w[1].pts));
    let video = Duration::from_secs_f64(f64::from(FRAMES) / f64::from(FPS));
    let sound = p.audio_position().unwrap();
    assert!(
        sound.abs_diff(video) < Duration::from_millis(50),
        "sound {sound:?} for pictures of {video:?}"
    );
    assert!((0.45..0.55).contains(&peak), "peak {peak}");
    assert!(opposite, "the two sides stay opposite");
    assert!(p.clock() >= video);
    assert!(p.next_frame().is_none(), "nothing left");
    assert_eq!(p.error(), None);
}

#[test]
fn the_movie_decodes_exactly_from_memory() {
    play_through(open_cursor());
}

#[test]
fn the_movie_decodes_exactly_from_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TEST.avi");
    std::fs::write(&path, movie()).unwrap();
    let source = Source::File {
        path,
        kind: FileKind::Original,
    };
    play_through(Player::open(&source, OUT_RATE).unwrap());
    let (p, which) = claw_movie::open_first(std::slice::from_ref(&source), OUT_RATE).unwrap();
    assert_eq!(which, source);
    drop(p);
}

#[test]
fn without_an_engine_the_movie_plays_on_the_wall_clock() {
    let mut p = open_cursor();
    let start = Instant::now();
    let mut shown = Vec::new();
    while !p.finished() {
        assert!(start.elapsed() < Duration::from_secs(10), "never finished");
        if let Some(f) = p.next_frame() {
            shown.push((f.pts, start.elapsed()));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let took = start.elapsed();
    let video = Duration::from_secs(2);
    assert!(took >= video, "over after {took:?}");
    assert!(
        took < video + Duration::from_millis(500),
        "over after {took:?}"
    );
    assert!(shown.len() >= FRAMES as usize - 2, "{} shown", shown.len());
    // Each picture shows once its time has come.
    for (pts, at) in shown {
        assert!(
            at + Duration::from_millis(5) >= pts,
            "{pts:?} shown at {at:?}"
        );
    }
}

/// Sound stored well behind its pictures (1.2 s here, more than the
/// picture queue's usual few pictures) still reaches the audio engine in
/// time: the decoder reads on past a full picture queue while the sound is
/// starving, so the movie's clock follows the sound to the end and never
/// falls to the wall clock (which would leave the sound behind the
/// pictures by the gap for the rest of the movie).
#[test]
fn sound_stored_behind_its_pictures_keeps_the_movie_on_the_sound_clock() {
    const LONG: u32 = 30;
    const DELAY: u32 = 12;
    let mut p = Player::open_reader(
        Box::new(Cursor::new(movie_with(LONG, DELAY))),
        "late-sound.avi",
        OUT_RATE,
    )
    .unwrap();
    let mut audio = p.take_audio().expect("the movie has sound");
    // An audio engine: 10 ms of sound every 10 ms, on a thread of its own.
    let stop = Arc::new(AtomicBool::new(false));
    let engine = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut out = vec![0.0f32; 2 * (OUT_RATE as usize / 100)];
            while !stop.load(Ordering::Relaxed) {
                out.fill(0.0);
                audio.render(&mut out, OUT_RATE);
                std::thread::sleep(Duration::from_millis(10));
            }
        })
    };
    let video = Duration::from_secs_f64(f64::from(LONG) / f64::from(FPS));
    let start = Instant::now();
    let mut shown = Vec::new();
    let mut worst = Duration::ZERO;
    while !p.finished() {
        assert!(start.elapsed() < Duration::from_secs(15), "never finished");
        let clock = p.clock();
        let sound = p.audio_position().unwrap();
        // Up to the soundtrack's last moments (where it hands over to the
        // wall clock, as every finished soundtrack does), the clock is the
        // sound's position, give or take one 10 ms block the engine plays
        // between the two readings.
        if sound + Duration::from_millis(100) < video {
            worst = worst.max(clock.abs_diff(sound));
        }
        if let Some(f) = p.next_frame() {
            shown.push(f.pts);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    stop.store(true, Ordering::Relaxed);
    engine.join().unwrap();
    assert!(
        worst < Duration::from_millis(60),
        "the clock left the sound by {worst:?}"
    );
    let sound = p.audio_position().unwrap();
    assert!(
        sound.abs_diff(video) < Duration::from_millis(50),
        "the whole soundtrack played: {sound:?} of {video:?}"
    );
    assert!(shown.len() >= LONG as usize - 3, "{} shown", shown.len());
    assert!(shown.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(p.error(), None);
}

#[test]
fn late_pictures_are_dropped() {
    let mut p = open_cursor();
    let t0 = p.clock();
    assert_eq!(t0, Duration::ZERO);
    std::thread::sleep(Duration::from_millis(550));
    let first = p.next_frame().expect("pictures are due").pts;
    assert!(
        first >= Duration::from_millis(200),
        "the oldest pictures were skipped, not shown: {first:?}"
    );
    let mut last = first;
    let deadline = Instant::now() + Duration::from_secs(2);
    while last < Duration::from_millis(500) {
        assert!(Instant::now() < deadline);
        if let Some(f) = p.next_frame() {
            assert!(f.pts > last);
            last = f.pts;
        }
    }
}

#[test]
fn dropping_a_player_stops_its_thread_at_once() {
    for take_sound in [false, true] {
        let mut p = open_cursor();
        let audio = take_sound.then(|| p.take_audio().unwrap());
        // The decoder fills its queue and waits.
        std::thread::sleep(Duration::from_millis(100));
        let t = Instant::now();
        drop(p);
        let took = t.elapsed();
        assert!(took < Duration::from_millis(100), "joined in {took:?}");
        drop(audio);
    }
}

#[test]
fn bytes_that_are_no_movie_do_not_open() {
    let err = Player::open_reader(Box::new(Cursor::new(vec![7u8; 4096])), "x.avi", OUT_RATE)
        .err()
        .unwrap();
    assert!(matches!(err, MovieError::Open(_)), "{err:?}");
    let mut half = movie();
    half.truncate(300);
    assert!(Player::open_reader(Box::new(Cursor::new(half)), "x.avi", OUT_RATE).is_err());
}
