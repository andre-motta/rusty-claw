//! Finding a movie's candidates in preference order.

use std::fs;
use std::path::{Path, PathBuf};

use crate::fec::Fec;
use crate::index::MovieIndex;
use crate::source::{FileKind, MovieMode, Roots, Source};

/// The extensions of the copies in the player's movie folder, in
/// preference order.
const UPSCALED_EXTENSIONS: [&str; 4] = ["mkv", "mp4", "mov", "avi"];
/// The extensions of the install's movie files, in preference order.
const ORIGINAL_EXTENSIONS: [&str; 2] = ["avi", "vob"];
/// The install's folder of movies copied from the CD.
const MOVIES_FOLDER: &str = "Movies";
/// The movie archive's name, without and with its extension.
const ARCHIVE_STEM: &str = "CLAW";
const ARCHIVE_EXTENSION: [&str; 1] = ["fec"];

/// Every place movie `index` can play from under `roots`, best first. A
/// player tries them in order (see [`crate::open_first`]).
///
/// With STEM the movie's [`MovieIndex::stem`] and names matched in any
/// case, the order is:
///
/// 1. In [`MovieMode::Upscaled`] only: the movie folder's `STEM.mkv`,
///    `STEM.mp4`, `STEM.mov` and `STEM.avi`.
/// 2. Unless in [`MovieMode::Archive`]: the install's `STEM.avi` and
///    `STEM.vob`, then the same in its `Movies` folder.
/// 3. The movie's archive clip ([`MovieIndex::fec_entry`]) in the
///    install's `CLAW.FEC`, then in `Movies/CLAW.FEC`. An archive is only
///    offered when it reads cleanly and holds that clip.
///
/// A movie without a stem or a clip has none of those candidates; the odd
/// indices 1 to 11 have neither and return nothing.
pub fn locate_all(index: MovieIndex, roots: &Roots) -> Vec<Source> {
    let mut out: Vec<Source> = Vec::new();
    let movies = movies_folders(&roots.install);
    let mut add_files = |dir: &Path, stem: &str, extensions: &[&str], kind: FileKind| {
        for path in find_files(dir, stem, extensions) {
            if !out.iter().any(|s| source_path(s) == path) {
                out.push(Source::File { path, kind });
            }
        }
    };
    if let Some(stem) = index.stem() {
        if roots.mode == MovieMode::Upscaled
            && let Some(folder) = &roots.user_folder
        {
            add_files(folder, stem, &UPSCALED_EXTENSIONS, FileKind::Upscaled);
        }
        if roots.mode != MovieMode::Archive {
            for dir in std::iter::once(&roots.install).chain(&movies) {
                add_files(dir, stem, &ORIGINAL_EXTENSIONS, FileKind::Original);
            }
        }
    }
    if let Some(entry) = index.fec_entry() {
        for dir in std::iter::once(&roots.install).chain(&movies) {
            for path in find_files(dir, ARCHIVE_STEM, &ARCHIVE_EXTENSION) {
                let holds = Fec::read(&path).is_ok_and(|fec| fec.entry(entry).is_some());
                if holds && !out.iter().any(|s| source_path(s) == path) {
                    out.push(Source::Archive { path, entry });
                }
            }
        }
    }
    out
}

fn source_path(source: &Source) -> &Path {
    match source {
        Source::File { path, .. } | Source::Archive { path, .. } => path,
    }
}

/// The folders of `install` named `Movies` in any case, by name.
fn movies_folders(install: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(install) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            let path = entry.path();
            (name.eq_ignore_ascii_case(MOVIES_FOLDER) && path.is_dir()).then_some((name, path))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, path)| path).collect()
}

/// The files of `dir` named `stem` with one of `extensions`, in the order
/// of `extensions`, matching names in any case.
fn find_files(dir: &Path, stem: &str, extensions: &[&str]) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(usize, String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some((s, ext)) = name.rsplit_once('.') else {
            continue;
        };
        if !s.eq_ignore_ascii_case(stem) {
            continue;
        }
        let Some(rank) = extensions.iter().position(|e| e.eq_ignore_ascii_case(ext)) else {
            continue;
        };
        let path = entry.path();
        if path.is_file() {
            found.push((rank, name.to_string(), path));
        }
    }
    found.sort();
    found.into_iter().map(|(_, _, path)| path).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_install_root_is_searched_in_any_case() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "logo.AVI",
            "Logo.vob",
            "CLAW0.avi",
            "CLAW1.txt",
            "claw8.vob",
        ] {
            fs::write(dir.path().join(name), b"x").unwrap();
        }
        fs::create_dir(dir.path().join("CLAW2.avi")).unwrap();
        let roots = Roots {
            install: dir.path().to_path_buf(),
            ..Roots::default()
        };
        let names = |i: i8| -> Vec<String> {
            locate_all(MovieIndex::new(i).unwrap(), &roots)
                .iter()
                .map(|s| match s {
                    Source::File { path, kind } => {
                        assert_eq!(*kind, FileKind::Original);
                        path.file_name().unwrap().to_string_lossy().into_owned()
                    }
                    Source::Archive { .. } => panic!("no archive here"),
                })
                .collect()
        };
        assert_eq!(names(-1), ["logo.AVI", "Logo.vob"]);
        assert_eq!(names(0), ["CLAW0.avi"]);
        assert!(names(2).is_empty(), "a text file is no movie");
        assert!(names(4).is_empty(), "a folder is no movie");
        assert_eq!(names(14), ["claw8.vob"]);
        assert!(names(-2).is_empty() && names(1).is_empty());
        let missing = Roots {
            install: dir.path().join("nowhere"),
            ..Roots::default()
        };
        assert!(locate_all(MovieIndex::LOGO, &missing).is_empty());
    }

    #[test]
    fn a_folder_that_is_also_the_install_lists_each_file_once() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("LOGO.avi"), b"x").unwrap();
        let roots = Roots {
            install: dir.path().to_path_buf(),
            user_folder: Some(dir.path().to_path_buf()),
            mode: MovieMode::Upscaled,
        };
        assert_eq!(locate_all(MovieIndex::LOGO, &roots).len(), 1);
    }
}
