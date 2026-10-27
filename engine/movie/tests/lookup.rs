//! Where each movie is looked for, on folders made here: the movie folder,
//! the install's files, its `Movies` folder and its archives, in every
//! mode, with names in any case. With the `ffmpeg` feature, a small
//! MPEG program stream made by the `ffmpeg` command (skipped without it)
//! plays from a `Movies` folder.

use std::fs;
use std::path::{Path, PathBuf};

use claw_movie::{FileKind, MovieIndex, MovieMode, Roots, SkipRule, Source, locate_all};

/// A movie archive with `clips` clips of a few bytes each.
fn archive(clips: u16) -> Vec<u8> {
    let mut out = b"FEC".to_vec();
    for v in [1u32, 1, u32::from(clips)] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for n in 1..=clips {
        let data = format!("SMK0 clip {n}");
        let mut record = [0u8; 0x10C];
        record[..4].copy_from_slice(&u32::from(n).to_le_bytes());
        record[0x106..0x108].copy_from_slice(&0x2B8u16.to_le_bytes());
        record[0x108..0x10C].copy_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&record);
        out.extend_from_slice(data.as_bytes());
    }
    out
}

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, b"x").unwrap();
}

fn index(n: i8) -> MovieIndex {
    MovieIndex::new(n).unwrap()
}

/// The candidates as short strings relative to `base`: `up:NAME`,
/// `orig:NAME` or `fec:NAME#ENTRY`.
fn names(sources: &[Source], base: &Path) -> Vec<String> {
    sources
        .iter()
        .map(|s| match s {
            Source::File { path, kind } => {
                let tag = match kind {
                    FileKind::Upscaled => "up",
                    FileKind::Original => "orig",
                };
                format!("{tag}:{}", rel(path, base))
            }
            Source::Archive { path, entry } => format!("fec:{}#{entry}", rel(path, base)),
        })
        .collect()
}

fn rel(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

/// A movie folder and an install with every kind of source for the logo
/// (index -1, archive clip 1) and a few for the intro (index 0, clip 2).
struct Setup {
    _dir: tempfile::TempDir,
    base: PathBuf,
    user: PathBuf,
    install: PathBuf,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_path_buf();
    let user = base.join("upscaled");
    let install = base.join("game");
    for name in [
        "LOGO.avi",
        "logo.MOV",
        "Logo.mp4",
        "LOGO.MKV",
        "LOGO.webm",
        "LOGO",
    ] {
        touch(&user.join(name));
    }
    touch(&user.join("CLAW0.mp4"));
    for name in [
        "Logo.vob",
        "LOGO.avi",
        "LOGO.smk",
        "CLAW0.VOB",
        "readme.txt",
    ] {
        touch(&install.join(name));
    }
    for name in ["logo.VOB", "LOGO.Avi", "CLAW1.avi"] {
        touch(&install.join("mOvIeS").join(name));
    }
    // A file named like the movies folder is no folder.
    touch(&install.join("MOVIES"));
    fs::write(install.join("CLAW.FEC"), archive(12)).unwrap();
    fs::write(install.join("mOvIeS").join("claw.fec"), archive(2)).unwrap();
    Setup {
        _dir: dir,
        base,
        user,
        install,
    }
}

impl Setup {
    fn roots(&self, mode: MovieMode) -> Roots {
        Roots {
            install: self.install.clone(),
            user_folder: Some(self.user.clone()),
            mode,
        }
    }

    fn find(&self, n: i8, mode: MovieMode) -> Vec<String> {
        names(&locate_all(index(n), &self.roots(mode)), &self.base)
    }
}

#[test]
fn upscaled_copies_come_first_then_the_install_files_then_the_archives() {
    let s = setup();
    assert_eq!(
        s.find(-1, MovieMode::Upscaled),
        [
            "up:upscaled/LOGO.MKV",
            "up:upscaled/Logo.mp4",
            "up:upscaled/logo.MOV",
            "up:upscaled/LOGO.avi",
            "orig:game/LOGO.avi",
            "orig:game/Logo.vob",
            "orig:game/mOvIeS/LOGO.Avi",
            "orig:game/mOvIeS/logo.VOB",
            "fec:game/CLAW.FEC#1",
            "fec:game/mOvIeS/claw.fec#1",
        ]
    );
    assert_eq!(
        s.find(0, MovieMode::Upscaled),
        [
            "up:upscaled/CLAW0.mp4",
            "orig:game/CLAW0.VOB",
            "fec:game/CLAW.FEC#2",
            "fec:game/mOvIeS/claw.fec#2",
        ]
    );
}

#[test]
fn original_mode_skips_the_movie_folder() {
    let s = setup();
    assert_eq!(
        s.find(-1, MovieMode::Original),
        [
            "orig:game/LOGO.avi",
            "orig:game/Logo.vob",
            "orig:game/mOvIeS/LOGO.Avi",
            "orig:game/mOvIeS/logo.VOB",
            "fec:game/CLAW.FEC#1",
            "fec:game/mOvIeS/claw.fec#1",
        ]
    );
    // Without a movie folder the upscaled mode is the original one.
    let roots = Roots {
        user_folder: None,
        ..s.roots(MovieMode::Upscaled)
    };
    assert_eq!(
        names(&locate_all(index(-1), &roots), &s.base),
        s.find(-1, MovieMode::Original)
    );
    // A movie folder that does not exist adds nothing.
    let roots = Roots {
        user_folder: Some(s.base.join("nowhere")),
        ..s.roots(MovieMode::Upscaled)
    };
    assert_eq!(
        names(&locate_all(index(-1), &roots), &s.base),
        s.find(-1, MovieMode::Original)
    );
}

#[test]
fn archive_mode_plays_the_archives_only() {
    let s = setup();
    assert_eq!(
        s.find(-1, MovieMode::Archive),
        ["fec:game/CLAW.FEC#1", "fec:game/mOvIeS/claw.fec#1"]
    );
    // The CD folder's archive has two clips: the movie after level 2 is
    // clip 3, only in the install's own archive.
    assert_eq!(s.find(2, MovieMode::Archive), ["fec:game/CLAW.FEC#3"]);
    assert_eq!(
        s.find(2, MovieMode::Original),
        ["orig:game/mOvIeS/CLAW1.avi", "fec:game/CLAW.FEC#3"]
    );
}

#[test]
fn the_attract_clip_is_archive_clip_one_only() {
    let s = setup();
    for mode in [MovieMode::Upscaled, MovieMode::Original, MovieMode::Archive] {
        assert_eq!(
            s.find(-2, mode),
            ["fec:game/CLAW.FEC#1", "fec:game/mOvIeS/claw.fec#1"],
            "{mode:?}"
        );
    }
}

#[test]
fn the_odd_levels_have_no_movie() {
    let s = setup();
    // Even files named for them are not looked at.
    for n in [1, 3, 5, 7, 9, 11] {
        touch(&s.install.join(format!("CLAW{n}.avi")));
    }
    for mode in [MovieMode::Upscaled, MovieMode::Original, MovieMode::Archive] {
        for n in [1, 3, 5, 7, 9, 11] {
            assert!(s.find(n, mode).is_empty(), "index {n} in {mode:?}");
        }
    }
}

#[test]
fn every_movie_finds_its_file_and_its_clip() {
    let dir = tempfile::tempdir().unwrap();
    let install = dir.path();
    fs::write(install.join("CLAW.FEC"), archive(12)).unwrap();
    for stem in [
        "LOGO", "CLAW0", "CLAW1", "CLAW2", "CLAW3", "CLAW4", "CLAW5", "CLAW6", "CLAW7", "CLAW8",
    ] {
        touch(&install.join(format!("{stem}.avi")));
    }
    let roots = Roots {
        install: install.to_path_buf(),
        user_folder: None,
        mode: MovieMode::Original,
    };
    for i in MovieIndex::all() {
        let found = locate_all(i, &roots);
        match (i.stem(), i.fec_entry()) {
            (None, None) => assert!(found.is_empty(), "{i}"),
            (None, Some(entry)) => assert_eq!(
                found,
                [Source::Archive {
                    path: install.join("CLAW.FEC"),
                    entry
                }],
                "{i}"
            ),
            (Some(stem), Some(entry)) => {
                assert_eq!(
                    found,
                    [
                        Source::File {
                            path: install.join(format!("{stem}.avi")),
                            kind: FileKind::Original,
                        },
                        Source::Archive {
                            path: install.join("CLAW.FEC"),
                            entry,
                        },
                    ],
                    "{i}"
                );
                assert_eq!(found[0].skip_rule(), SkipRule::Back);
                assert_eq!(found[1].skip_rule(), SkipRule::AnyInput);
            }
            (Some(_), None) => panic!("{i} has a file but no clip"),
        }
    }
}

#[test]
fn a_malformed_or_short_archive_offers_no_clip() {
    let dir = tempfile::tempdir().unwrap();
    let install = dir.path();
    let mut bad = archive(12);
    let len = bad.len();
    bad.truncate(len - 1);
    fs::write(install.join("CLAW.FEC"), &bad).unwrap();
    let roots = Roots {
        install: install.to_path_buf(),
        user_folder: None,
        mode: MovieMode::Archive,
    };
    assert!(locate_all(MovieIndex::LOGO, &roots).is_empty());
    // A good archive in the Movies folder still plays.
    fs::create_dir(install.join("MOVIES")).unwrap();
    fs::write(install.join("MOVIES").join("Claw.Fec"), archive(1)).unwrap();
    assert_eq!(
        locate_all(MovieIndex::LOGO, &roots),
        [Source::Archive {
            path: install.join("MOVIES").join("Claw.Fec"),
            entry: 1
        }]
    );
    assert!(
        locate_all(MovieIndex::INTRO, &roots).is_empty(),
        "no clip 2"
    );
    // An archive that is a folder, or a missing install, offers nothing.
    let other = tempfile::tempdir().unwrap();
    fs::create_dir(other.path().join("CLAW.FEC")).unwrap();
    let roots = Roots {
        install: other.path().to_path_buf(),
        ..roots
    };
    assert!(locate_all(MovieIndex::LOGO, &roots).is_empty());
    let roots = Roots {
        install: other.path().join("nowhere"),
        ..roots
    };
    assert!(locate_all(MovieIndex::LOGO, &roots).is_empty());
}

#[cfg(feature = "ffmpeg")]
mod playing {
    use std::process::{Command, Stdio};
    use std::time::Duration;

    use super::*;

    /// Makes a one-second MPEG-2 program stream (`.vob`) with MPEG audio
    /// at `path` with the `ffmpeg` command; false when it is not there.
    fn make_vob(path: &Path) -> bool {
        let status = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
            .args(["-f", "lavfi", "-i", "testsrc=size=160x120:rate=25"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:sample_rate=44100"])
            .args(["-t", "1", "-c:v", "mpeg2video", "-c:a", "mp2", "-f", "vob"])
            .arg(path)
            .stdin(Stdio::null())
            .status();
        match status {
            Ok(s) if s.success() => true,
            other => {
                eprintln!("skipped: the ffmpeg command cannot make a test clip ({other:?})");
                false
            }
        }
    }

    // FFmpeg recognises the picture stream of a program stream through
    // its raw MPEG video demuxer, which the pinned build includes for this.
    #[test]
    fn a_program_stream_plays_from_the_movies_folder() {
        let dir = tempfile::tempdir().unwrap();
        let movies = dir.path().join("Movies");
        fs::create_dir(&movies).unwrap();
        if !make_vob(&movies.join("Claw0.vob")) {
            return;
        }
        let roots = Roots {
            install: dir.path().to_path_buf(),
            user_folder: None,
            mode: MovieMode::Original,
        };
        let candidates = locate_all(MovieIndex::INTRO, &roots);
        assert_eq!(
            candidates,
            [Source::File {
                path: movies.join("Claw0.vob"),
                kind: FileKind::Original
            }]
        );
        let (mut p, which) = claw_movie::open_first(&candidates, 48000).unwrap();
        assert_eq!(which, candidates[0]);
        assert_eq!(p.size(), (160, 120));
        let mut audio = p.take_audio().expect("the clip has sound");
        let mut out = vec![0.0f32; 2 * 480];
        let mut loud = false;
        let mut pictures = 0;
        let mut last = None;
        while let Some(f) = p.decoded_frame(Duration::from_secs(10)) {
            assert_eq!((f.width, f.height), (160, 120));
            assert!(last.is_none_or(|l| f.pts > l), "{:?} after {last:?}", f.pts);
            last = Some(f.pts);
            pictures += 1;
            out.fill(0.0);
            audio.render(&mut out, 48000);
            loud |= out.iter().any(|s| s.abs() > 0.01);
        }
        assert_eq!(p.error(), None);
        assert_eq!(pictures, 25);
        assert!(loud, "the sound is not silent");
    }
}
