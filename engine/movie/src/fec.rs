//! The movie archive (`CLAW.FEC`): a header, then one record and the data
//! of each clip.
//!
//! The layout, all numbers little-endian:
//!
//! | Offset | Size | Field |
//! | --- | --- | --- |
//! | 0 | 3 | `FEC` |
//! | 3 | 4 | major version |
//! | 7 | 4 | minor version |
//! | 11 | 4 | number of clips |
//! | 15 | | each clip: its record, a gap, then its data |
//!
//! A clip's record is 0x10C bytes: its number (counting from 1) at +0, a
//! 16-bit field at +0x106 whose value minus 0x2B8 is the gap between the
//! record's end and the data, and the data's size at +0x108. Clips follow
//! each other with nothing in between. A clip is found by its place in
//! the archive, counting from 1; the record's own number is not used.
//!
//! The clips are Smacker video. Most start with the signature `SMK0`,
//! which is a `SMK2` stream under another label: [`open_entry`] reads it as
//! `SMK2` so that the decoder accepts it, without writing any file.
//!
//! The archive is read as untrusted input: every offset is checked against
//! the file's length, and anything that does not fit is an error, never a
//! panic.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use crate::ReadSeek;
use crate::range::PatchedRange;

/// The archive's first three bytes.
pub const MAGIC: [u8; 3] = *b"FEC";
/// The header's length in bytes.
pub const HEADER_LEN: u64 = 15;
/// A clip record's length in bytes.
pub const RECORD_LEN: u64 = 0x10C;
/// The record field's value that means no gap before the data.
const GAP_BASE: u16 = 0x2B8;
/// Where the gap field is in a record.
const GAP_AT: usize = 0x106;
/// Where the data size is in a record.
const SIZE_AT: usize = 0x108;
/// The signature most clips carry.
pub const SMK0: [u8; 4] = *b"SMK0";
/// The signature the decoder knows; [`SMK0`] clips read as it.
pub const SMK2: [u8; 4] = *b"SMK2";

/// Why an archive could not be read.
#[derive(Debug, thiserror::Error)]
pub enum FecError {
    /// Reading failed.
    #[error("reading the movie archive: {0}")]
    Io(#[from] io::Error),
    /// The archive is not laid out as expected.
    #[error("the movie archive is malformed: {0}")]
    Malformed(String),
    /// The archive could not be read by this build. Kept for callers that
    /// match on it; archives are read in full now and this is not
    /// returned.
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
    /// The first four bytes of its data (zero past the end of a shorter
    /// clip).
    pub signature: [u8; 4],
}

impl FecEntry {
    /// What its first four bytes read as through [`open_entry`]: `SMK2`
    /// in place of `SMK0`, nothing for every other clip.
    pub fn patch(&self) -> Option<[u8; 4]> {
        (self.signature == SMK0).then_some(SMK2)
    }
}

/// The clips of an archive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fec {
    entries: Vec<FecEntry>,
    version: (u32, u32),
}

impl Fec {
    /// Reads the archive's layout from `r`. Fails when any part of it lies
    /// outside the data `r` holds; bytes after the last clip are allowed.
    pub fn parse<R: Read + Seek>(mut r: R) -> Result<Fec, FecError> {
        let file_len = r.seek(SeekFrom::End(0))?;
        if file_len < HEADER_LEN {
            return Err(malformed(format!(
                "{file_len} bytes is shorter than the {HEADER_LEN}-byte header"
            )));
        }
        let mut header = [0u8; HEADER_LEN as usize];
        r.seek(SeekFrom::Start(0))?;
        read_exact(&mut r, &mut header)?;
        if header[..3] != MAGIC {
            return Err(malformed("it does not start with FEC".into()));
        }
        let major = u32_at(&header, 3);
        let minor = u32_at(&header, 7);
        let count = u32_at(&header, 11);
        // Every clip takes at least a record, so a count the file cannot
        // hold is rejected before any walking.
        let room = (file_len - HEADER_LEN) / RECORD_LEN;
        if u64::from(count) > room {
            return Err(malformed(format!(
                "{count} clips do not fit in {file_len} bytes"
            )));
        }
        let count =
            u16::try_from(count).map_err(|_| malformed(format!("{count} clips are too many")))?;

        let mut entries = Vec::with_capacity(usize::from(count));
        let mut at = HEADER_LEN;
        let mut record = [0u8; RECORD_LEN as usize];
        for number in 1..=count {
            let record_end = at
                .checked_add(RECORD_LEN)
                .filter(|&end| end <= file_len)
                .ok_or_else(|| malformed(format!("clip {number}: its record is cut short")))?;
            r.seek(SeekFrom::Start(at))?;
            read_exact(&mut r, &mut record)?;
            let g = u16::from_le_bytes([record[GAP_AT], record[GAP_AT + 1]]);
            let gap = g.checked_sub(GAP_BASE).ok_or_else(|| {
                malformed(format!(
                    "clip {number}: the gap field {g:#x} is below {GAP_BASE:#x}"
                ))
            })?;
            let len = u64::from(u32_at(&record, SIZE_AT));
            let data_offset = record_end
                .checked_add(u64::from(gap))
                .filter(|&start| start <= file_len)
                .ok_or_else(|| malformed(format!("clip {number}: its gap runs past the end")))?;
            let data_end = data_offset
                .checked_add(len)
                .filter(|&end| end <= file_len)
                .ok_or_else(|| {
                    malformed(format!(
                        "clip {number}: {len} bytes at {data_offset} run past the end at {file_len}"
                    ))
                })?;
            let mut signature = [0u8; 4];
            let head = len.min(4) as usize;
            if head > 0 {
                r.seek(SeekFrom::Start(data_offset))?;
                read_exact(&mut r, &mut signature[..head])?;
            }
            entries.push(FecEntry {
                number,
                data_offset,
                len,
                signature,
            });
            at = data_end;
        }
        Ok(Fec {
            entries,
            version: (major, minor),
        })
    }

    /// Reads the layout of the archive at `path`.
    pub fn read(path: &Path) -> Result<Fec, FecError> {
        Fec::parse(File::open(path)?)
    }

    /// Clip `number`, counting from 1.
    pub fn entry(&self, number: u16) -> Option<FecEntry> {
        let i = usize::from(number.checked_sub(1)?);
        self.entries.get(i).copied()
    }

    /// Every clip, in order.
    pub fn entries(&self) -> &[FecEntry] {
        &self.entries
    }

    /// The archive's version, major and minor.
    pub fn version(&self) -> (u32, u32) {
        self.version
    }
}

/// A reader over clip `number` of the archive at `archive`, ready for the
/// player's [`crate::Player::open_reader`]: the clip's bytes alone, with a
/// `SMK0` signature read as `SMK2`. Fails with `NotFound` when the archive
/// or the clip is missing and with `InvalidData` when the archive is
/// malformed.
pub fn open_entry(archive: &Path, number: u16) -> io::Result<Box<dyn ReadSeek + Send>> {
    let mut file = File::open(archive)?;
    let fec = Fec::parse(&mut file).map_err(|e| match e {
        FecError::Io(e) => e,
        other => io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {other}", archive.display()),
        ),
    })?;
    let entry = fec.entry(number).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{} has {} clips, no clip {number}",
                archive.display(),
                fec.entries.len()
            ),
        )
    })?;
    let range = PatchedRange::new(file, entry.data_offset, entry.len, entry.patch())?;
    Ok(Box::new(range))
}

fn malformed(why: String) -> FecError {
    FecError::Malformed(why)
}

/// `read_exact`, with a short read reported as a malformed archive.
fn read_exact<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<(), FecError> {
    r.read_exact(buf).map_err(|e| match e.kind() {
        io::ErrorKind::UnexpectedEof => malformed("the data ends early".into()),
        _ => FecError::Io(e),
    })
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn archive(clips: &[&[u8]]) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        for v in [1u32, 1, clips.len() as u32] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for (i, data) in clips.iter().enumerate() {
            let mut record = [0u8; RECORD_LEN as usize];
            record[..4].copy_from_slice(&(i as u32 + 1).to_le_bytes());
            record[GAP_AT..GAP_AT + 2].copy_from_slice(&GAP_BASE.to_le_bytes());
            record[SIZE_AT..SIZE_AT + 4].copy_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&record);
            out.extend_from_slice(data);
        }
        out
    }

    #[test]
    fn clips_are_found_by_their_place() {
        let fec = Fec::parse(Cursor::new(archive(&[b"SMK0abc", b"SMK2", b"x"]))).unwrap();
        assert_eq!(fec.version(), (1, 1));
        assert_eq!(fec.entries().len(), 3);
        assert_eq!(fec.entry(0), None);
        assert_eq!(fec.entry(4), None);
        let first = fec.entry(1).unwrap();
        assert_eq!((first.data_offset, first.len), (283, 7));
        assert_eq!(first.patch(), Some(SMK2));
        let second = fec.entry(2).unwrap();
        assert_eq!(second.data_offset, 283 + 7 + RECORD_LEN);
        assert_eq!(second.patch(), None);
        assert_eq!(fec.entry(3).unwrap().signature, *b"x\0\0\0");
    }

    #[test]
    fn a_count_past_the_file_is_malformed() {
        let mut bytes = archive(&[b"SMK2"]);
        bytes[11..15].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            Fec::parse(Cursor::new(bytes)),
            Err(FecError::Malformed(_))
        ));
        assert!(matches!(
            Fec::parse(Cursor::new(b"FE".to_vec())),
            Err(FecError::Malformed(_))
        ));
    }
}
