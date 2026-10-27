//! The movie archive (`CLAW.FEC`): a header, then one record and the data
//! of each clip.
//!
//! Reading the archive arrives in a later change; for now [`Fec::parse`]
//! and [`open_entry`] report that clips cannot be read yet, and the lookup
//! offers no archive candidates.

use std::io::{self, Read, Seek};
use std::path::Path;

use crate::ReadSeek;

/// Why an archive could not be read.
#[derive(Debug, thiserror::Error)]
pub enum FecError {
    /// Reading failed.
    #[error("reading the movie archive: {0}")]
    Io(#[from] io::Error),
    /// The archive is not laid out as expected.
    #[error("the movie archive is malformed: {0}")]
    Malformed(String),
    /// This build cannot read movie archives yet.
    #[error("movie archives cannot be read yet")]
    Unsupported,
}

/// One clip of the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FecEntry {
    /// Its number, counting from 1.
    pub number: u16,
    /// Where its data starts in the archive.
    pub data_offset: u64,
    /// Its length in bytes.
    pub len: u64,
    /// The first four bytes of its data.
    pub signature: [u8; 4],
}

/// The clips of an archive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fec {
    entries: Vec<FecEntry>,
}

impl Fec {
    /// Reads the archive's layout from `r`.
    pub fn parse<R: Read + Seek>(r: R) -> Result<Fec, FecError> {
        let _ = r;
        Err(FecError::Unsupported)
    }

    /// Clip `number`, counting from 1.
    pub fn entry(&self, number: u16) -> Option<FecEntry> {
        self.entries.iter().copied().find(|e| e.number == number)
    }
}

/// A reader over clip `number` of the archive at `archive`, ready for the
/// player's [`crate::Player::open_reader`].
pub fn open_entry(archive: &Path, number: u16) -> io::Result<Box<dyn ReadSeek + Send>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!(
            "{} entry {number}: movie archives cannot be read yet",
            archive.display()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archives_are_not_read_yet() {
        assert!(matches!(
            Fec::parse(io::Cursor::new(vec![0u8; 64])),
            Err(FecError::Unsupported)
        ));
        let err = open_entry(Path::new("CLAW.FEC"), 3).err().unwrap();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
        assert_eq!(Fec::default().entry(1), None);
    }
}
