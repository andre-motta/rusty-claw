//! Every movie of the install is found, every clip of its movie archive
//! plays, and a movie whose file is missing plays from the archive.
//! Reads the install named by `CLAW_INSTALL` and skips without it.
#![cfg(feature = "ffmpeg")]

use std::path::{Path, PathBuf};
use std::time::Duration;

use claw_movie::fec::{Fec, SMK0, SMK2};
use claw_movie::{FileKind, MovieIndex, MovieMode, Player, Roots, Source, locate_all};

const OUT_RATE: u32 = 48000;

fn install() -> Option<PathBuf> {
    let dir = std::env::var_os("CLAW_INSTALL").map(PathBuf::from);
    if dir.is_none() {
        eprintln!("skipped: set CLAW_INSTALL to your own install (make test-install)");
    }
    dir
}

/// The install's movie archive, in any case.
fn archive(install: &Path) -> PathBuf {
    std::fs::read_dir(install)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case("CLAW.FEC"))
        })
        .expect("the install has a movie archive")
}

/// Plays `source` to the end, reading every picture, and returns how many
/// there were.
fn count_pictures(source: &Source) -> usize {
    let mut p =
        Player::open(source, OUT_RATE).unwrap_or_else(|e| panic!("{}: {e}", source.describe()));
    assert_eq!(p.size(), (320, 240), "{}", source.describe());
    assert!((p.display_aspect() - 4.0 / 3.0).abs() < 1e-3);
    let mut pictures = 0;
    let mut last = None;
    while let Some(f) = p.decoded_frame(Duration::from_secs(10)) {
        assert_eq!((f.width, f.height), (320, 240));
        assert!(last.is_none_or(|l| f.pts > l), "{}", source.describe());
        last = Some(f.pts);
        pictures += 1;
    }
    assert_eq!(p.error(), None, "{}", source.describe());
    pictures
}

#[test]
fn every_movie_is_found() {
    let Some(install) = install() else { return };
    let fec = archive(&install);
    for mode in [MovieMode::Upscaled, MovieMode::Original] {
        let roots = Roots {
            install: install.clone(),
            user_folder: None,
            mode,
        };
        for i in MovieIndex::all() {
            let found = locate_all(i, &roots);
            match (i.stem(), i.fec_entry()) {
                (None, None) => assert!(found.is_empty(), "{i}: {found:?}"),
                (None, Some(entry)) => assert_eq!(
                    found,
                    [Source::Archive {
                        path: fec.clone(),
                        entry
                    }],
                    "{i}"
                ),
                (Some(stem), Some(entry)) => {
                    let Some(Source::File { path, kind }) = found.first() else {
                        panic!("{i}: no file first: {found:?}");
                    };
                    assert_eq!(*kind, FileKind::Original);
                    let name = path.file_name().unwrap().to_string_lossy();
                    assert!(
                        name.eq_ignore_ascii_case(&format!("{stem}.avi")),
                        "{i}: {name}"
                    );
                    assert_eq!(
                        found.last(),
                        Some(&Source::Archive {
                            path: fec.clone(),
                            entry
                        }),
                        "{i}"
                    );
                }
                (Some(_), None) => panic!("{i} has a file but no clip"),
            }
        }
    }
    let roots = Roots {
        install,
        user_folder: None,
        mode: MovieMode::Archive,
    };
    assert_eq!(
        locate_all(MovieIndex::new(14).unwrap(), &roots),
        [Source::Archive {
            path: fec,
            entry: 10
        }]
    );
}

#[test]
fn every_archive_clip_plays() {
    let Some(install) = install() else { return };
    let path = archive(&install);
    let fec = Fec::read(&path).unwrap();
    assert_eq!(fec.version(), (1, 1));
    assert_eq!(fec.entries().len(), 12);
    for e in fec.entries() {
        let want = if e.number <= 10 { SMK0 } else { SMK2 };
        assert_eq!(e.signature, want, "clip {}", e.number);
    }
    for number in 1..=12u16 {
        let source = Source::Archive {
            path: path.clone(),
            entry: number,
        };
        let pictures = count_pictures(&source);
        eprintln!("clip {number}: {pictures} pictures");
        let want = if number == 11 { 723 } else { 52 };
        assert_eq!(pictures, want, "clip {number}");
    }
}

#[cfg(unix)]
#[test]
fn a_movie_without_its_file_plays_from_the_archive() {
    let Some(install) = install() else { return };
    // The install without its movie files: a folder of links to
    // everything else.
    let view = tempfile::tempdir().unwrap();
    for entry in std::fs::read_dir(&install).unwrap().flatten() {
        let name = entry.file_name();
        let lower = name.to_string_lossy().to_ascii_lowercase();
        if lower.ends_with(".avi") || lower.ends_with(".vob") {
            continue;
        }
        std::os::unix::fs::symlink(entry.path(), view.path().join(&name)).unwrap();
    }
    let fec = archive(view.path());
    let roots = Roots {
        install: view.path().to_path_buf(),
        user_folder: None,
        mode: MovieMode::Upscaled,
    };
    let after_level_2 = MovieIndex::after_level(2).unwrap();
    let candidates = locate_all(after_level_2, &roots);
    let want = Source::Archive {
        path: fec,
        entry: 3,
    };
    assert_eq!(candidates, std::slice::from_ref(&want));
    let (p, which) = claw_movie::open_first(&candidates, OUT_RATE).unwrap();
    assert_eq!(which, want);
    drop(p);
    assert_eq!(count_pictures(&want), 52);
}
