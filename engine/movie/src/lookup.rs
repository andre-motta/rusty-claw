//! Finding a movie's candidates in preference order.

use std::fs;
use std::path::{Path, PathBuf};

use crate::index::MovieIndex;
use crate::source::{FileKind, Roots, Source};

/// The extensions of the install's movie files, in preference order.
const ORIGINAL_EXTENSIONS: [&str; 2] = ["avi", "vob"];

/// Every place movie `index` can play from under `roots`, best first. A
/// player tries them in order (see [`crate::open_first`]).
///
/// The install's own files come from its root folder: `STEM.avi`, then
/// `STEM.vob`, where STEM is [`MovieIndex::stem`] and the name matches in
/// any case.
pub fn locate_all(index: MovieIndex, roots: &Roots) -> Vec<Source> {
    let Some(stem) = index.stem() else {
        return Vec::new();
    };
    find_files(&roots.install, stem, &ORIGINAL_EXTENSIONS)
        .into_iter()
        .map(|path| Source::File {
            path,
            kind: FileKind::Original,
        })
        .collect()
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
                    Source::Archive { .. } => panic!("no archive in this item"),
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
}
