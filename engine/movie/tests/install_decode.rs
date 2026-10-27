//! The install's logo movie decodes in full: every picture, with sound.
//! Reads the install named by `CLAW_INSTALL` and skips without it.
#![cfg(claw_ffmpeg)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use claw_movie::{MovieIndex, Roots, Source, locate_all};

const OUT_RATE: u32 = 48000;

fn install() -> Option<PathBuf> {
    let dir = std::env::var_os("CLAW_INSTALL").map(PathBuf::from);
    if dir.is_none() {
        eprintln!("skipped: set CLAW_INSTALL to your own install (make test-install)");
    }
    dir
}

#[test]
fn the_logo_decodes_in_full_with_its_sound() {
    let Some(install) = install() else { return };
    let roots = Roots {
        install,
        ..Roots::default()
    };
    let candidates = locate_all(MovieIndex::LOGO, &roots);
    let Some(Source::File { path, .. }) = candidates.first() else {
        panic!("no logo movie in the install's folder: {candidates:?}");
    };
    let t = Instant::now();
    let (mut p, _) = claw_movie::open_first(&candidates, OUT_RATE).unwrap();
    assert_eq!(p.size(), (640, 480), "{}", path.display());
    assert!((p.display_aspect() - 4.0 / 3.0).abs() < 1e-3);
    let mut audio = p.take_audio().expect("the logo has sound");
    let mut pictures = 0usize;
    let mut last = Duration::ZERO;
    let mut loud = false;
    let mut out = vec![0.0f32; 2 * 480];
    let deadline = Instant::now() + Duration::from_secs(120);
    while !p.finished() {
        assert!(Instant::now() < deadline, "the logo never finished");
        while let Some(f) = p.decoded_frame(Duration::from_millis(1)) {
            assert!(pictures == 0 || f.pts > last, "{:?} after {last:?}", f.pts);
            assert_eq!((f.width, f.height), (640, 480));
            last = f.pts;
            pictures += 1;
        }
        out.fill(0.0);
        audio.render(&mut out, OUT_RATE);
        loud |= out.iter().any(|s| s.abs() > 0.01);
    }
    let sound = p.audio_position().unwrap();
    eprintln!(
        "{}: {pictures} pictures, last at {last:?}, sound {sound:?}, decoded in {:?}",
        path.display(),
        t.elapsed()
    );
    assert_eq!(pictures, 366);
    let length = last + Duration::from_secs_f64(1.0 / 15.0);
    assert!(
        length.abs_diff(Duration::from_millis(24_400)) < Duration::from_millis(100),
        "{length:?}"
    );
    assert!(loud, "the sound is not silent");
    assert!(sound > Duration::from_secs(20), "sound {sound:?}");
    assert_eq!(p.error(), None);
}
