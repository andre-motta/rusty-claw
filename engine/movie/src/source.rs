//! Where a movie comes from, which input skips it, and where to look for
//! movies.

use std::path::PathBuf;

/// What a movie file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileKind {
    /// A copy the player made, for example an upscaled one, from the movie
    /// folder.
    Upscaled,
    /// A movie file of the install.
    Original,
}

/// Which input skips a movie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkipRule {
    /// Escape, or a pad's Back or Start button (the movie files).
    Back,
    /// Any key, mouse click or pad button (the archive's clips).
    AnyInput,
}

/// One place a movie can play from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    /// A movie file.
    File {
        /// The file.
        path: PathBuf,
        /// What it is.
        kind: FileKind,
    },
    /// An entry of the movie archive (`CLAW.FEC`).
    Archive {
        /// The archive.
        path: PathBuf,
        /// The entry, counting from 1.
        entry: u16,
    },
}

impl Source {
    /// Which input skips the movie: Escape (or a pad's Back or Start) for a
    /// file, any input for an archive clip.
    pub fn skip_rule(&self) -> SkipRule {
        match self {
            Source::File { .. } => SkipRule::Back,
            Source::Archive { .. } => SkipRule::AnyInput,
        }
    }

    /// A short description for messages.
    pub fn describe(&self) -> String {
        match self {
            Source::File { path, .. } => path.display().to_string(),
            Source::Archive { path, entry } => format!("{} entry {entry}", path.display()),
        }
    }
}

/// Which kinds of source a run plays.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum MovieMode {
    /// Copies in the movie folder first, then everything
    /// [`MovieMode::Original`] plays.
    #[default]
    Upscaled,
    /// The install's movie files, then its movie archive.
    Original,
    /// The install's movie archive only.
    Archive,
}

/// Where to look for movies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roots {
    /// The game install.
    pub install: PathBuf,
    /// The player's movie folder, if any.
    pub user_folder: Option<PathBuf>,
    /// Which kinds of source to play.
    pub mode: MovieMode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_skip_on_back_and_archive_clips_on_any_input() {
        let file = Source::File {
            path: "/game/LOGO.avi".into(),
            kind: FileKind::Original,
        };
        let clip = Source::Archive {
            path: "/game/CLAW.FEC".into(),
            entry: 3,
        };
        assert_eq!(file.skip_rule(), SkipRule::Back);
        assert_eq!(clip.skip_rule(), SkipRule::AnyInput);
        assert_eq!(file.describe(), "/game/LOGO.avi");
        assert_eq!(clip.describe(), "/game/CLAW.FEC entry 3");
    }
}
