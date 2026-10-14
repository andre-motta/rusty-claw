//! REZ archives: one file holding a directory tree and the bytes of every
//! file in it.
//!
//! Layout: a 0xA8-byte header, then directory blocks and file data anywhere
//! in the file. The header names the root directory block; a directory block
//! is a run of entries, each either a subdirectory (with the location of its
//! own block) or a file (with the location of its bytes). All integers are
//! little-endian.
//!
//! Archives are untrusted input. The reader checks every range against the
//! archive size, bounds names, entry counts and nesting depth, refuses a
//! directory block that is reached twice, and caps the directory bytes read
//! in total.

use crate::bytes::{cstr_field, latin1, slice, to_usize, u8_at, u32_at};
use crate::error::{Result, format_error};
use crate::name::MAX_NAME_LEN;

/// Size of the archive header.
pub const HEADER_SIZE: usize = 0xA8;
/// The only archive version accepted.
pub const VERSION: u32 = 1;
/// Deepest directory nesting accepted.
pub const MAX_DEPTH: usize = 64;
/// Most entries accepted in one archive.
pub const MAX_ENTRIES: usize = 1 << 20;
/// Largest directory block accepted.
pub const MAX_BLOCK_SIZE: usize = 64 << 20;
/// Most directory-block bytes read while walking one archive, as a multiple
/// of the archive size. Blocks of a well-formed archive do not overlap, so
/// together they are smaller than the archive; folders that point into the
/// same region again and again exceed this long before the walk gets slow.
pub const MAX_DIRECTORY_READ_FACTOR: u64 = 2;

const ENTRY_DIRECTORY: u32 = 1;
const FORMAT: &str = "REZ archive";

/// The archive header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RezHeader {
    /// Archive title with trailing spaces removed.
    pub title: String,
    /// Format version; always [`VERSION`] for an accepted archive.
    pub version: u32,
    /// File offset of the root directory block.
    pub dir_offset: u32,
    /// Size of the root directory block in bytes.
    pub dir_size: u32,
    /// Time stamp of the root directory.
    pub dir_time: u32,
    /// End of the data written so far.
    pub next_write: u32,
}

/// One entry of a directory block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RezEntry {
    /// A subdirectory and the location of its block. An offset of 0 means an
    /// empty folder.
    Directory {
        /// Folder name as stored.
        name: String,
        /// Offset of the folder's directory block.
        offset: u32,
        /// Size of the folder's directory block.
        size: u32,
        /// Time stamp.
        time: u32,
    },
    /// A file and the location of its bytes.
    File {
        /// File name without extension, as stored.
        name: String,
        /// Type code packed from the extension.
        type_code: u32,
        /// Offset of the file's bytes.
        offset: u32,
        /// Size of the file in bytes.
        size: u32,
        /// Time stamp.
        time: u32,
        /// Numeric id.
        id: u32,
    },
}

/// Parses the archive header and checks its signature and version.
pub fn parse_header(data: &[u8]) -> Result<RezHeader> {
    if data.len() < HEADER_SIZE {
        return Err(format_error(
            FORMAT,
            format!("header is {} bytes, need {HEADER_SIZE}", data.len()),
        ));
    }
    let signature = u8_at(data, 0) == Some(0x0D)
        && u8_at(data, 0x3F) == Some(0x0A)
        && u8_at(data, 0x7E) == Some(0x1A);
    if !signature {
        return Err(format_error(FORMAT, "signature bytes missing"));
    }
    let field = |off: usize| u32_at(data, off).unwrap_or(0);
    let version = field(0x7F);
    if version != VERSION {
        return Err(format_error(
            FORMAT,
            format!("version {version}, expected {VERSION}"),
        ));
    }
    let title = cstr_field(&data[0x40..0x7C])
        .trim_end_matches(' ')
        .to_string();
    Ok(RezHeader {
        title,
        version,
        dir_offset: field(0x83),
        dir_size: field(0x87),
        dir_time: field(0x8B),
        next_write: field(0x8F),
    })
}

fn read_name<'a>(block: &'a [u8], pos: usize, what: &str) -> Result<(&'a [u8], usize)> {
    let rest = block
        .get(pos..)
        .ok_or_else(|| format_error(FORMAT, format!("{what} starts past the block end")))?;
    let len = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| format_error(FORMAT, format!("unterminated {what}")))?;
    if len > MAX_NAME_LEN {
        return Err(format_error(
            FORMAT,
            format!("{what} longer than {MAX_NAME_LEN} bytes"),
        ));
    }
    Ok((&rest[..len], pos + len + 1))
}

/// Parses one directory block. `archive_size` bounds every location the
/// entries point at.
pub fn parse_block(block: &[u8], archive_size: u64) -> Result<Vec<RezEntry>> {
    let mut entries = Vec::new();
    let mut pos = 0usize;
    while pos < block.len() {
        let head = slice(block, pos, 16).ok_or_else(|| {
            format_error(FORMAT, format!("truncated entry at block offset {pos}"))
        })?;
        let word = |i: usize| u32_at(head, i * 4).unwrap_or(0);
        let (kind, offset, size, time) = (word(0), word(1), word(2), word(3));
        let in_range = u64::from(offset) + u64::from(size) <= archive_size;
        let entry = if kind == ENTRY_DIRECTORY {
            let (name, next) = read_name(block, pos + 16, "folder name")?;
            pos = next;
            if offset != 0 && !in_range {
                return Err(format_error(
                    FORMAT,
                    "folder block lies outside the archive",
                ));
            }
            RezEntry::Directory {
                name: latin1(name),
                offset,
                size,
                time,
            }
        } else {
            let tail = slice(block, pos + 16, 12).ok_or_else(|| {
                format_error(
                    FORMAT,
                    format!("truncated file entry at block offset {pos}"),
                )
            })?;
            let id = u32_at(tail, 0).unwrap_or(0);
            let type_code = u32_at(tail, 4).unwrap_or(0);
            let key_count = to_usize(u32_at(tail, 8).unwrap_or(0));
            let (name, next) = read_name(block, pos + 28, "file name")?;
            let (_comment, next) = read_name(block, next, "comment")?;
            let keys_len = key_count
                .checked_mul(4)
                .filter(|&n| n <= block.len().saturating_sub(next))
                .ok_or_else(|| format_error(FORMAT, "key count overruns the block"))?;
            pos = next + keys_len;
            if !in_range {
                return Err(format_error(FORMAT, "file data lies outside the archive"));
            }
            RezEntry::File {
                name: latin1(name),
                type_code,
                offset,
                size,
                time,
                id,
            }
        };
        entries.push(entry);
        if entries.len() > MAX_ENTRIES {
            return Err(format_error(FORMAT, "too many entries in one block"));
        }
    }
    Ok(entries)
}

/// Builds archive bytes from a tree description. Used by tests and tools to
/// produce archives in the layout [`parse_header`] and [`parse_block`] read.
#[derive(Debug, Default, Clone)]
pub struct RezBuilder {
    root: BuildDir,
}

#[derive(Debug, Default, Clone)]
struct BuildDir {
    dirs: Vec<(String, BuildDir)>,
    files: Vec<(String, u32, Vec<u8>)>,
}

impl RezBuilder {
    /// An empty archive.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file at `path` (folders separated by `/`, the last segment is
    /// `NAME.EXT`).
    pub fn file(&mut self, path: &str, data: &[u8]) -> &mut Self {
        let mut segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let leaf = segs.pop().unwrap_or("");
        let mut node = &mut self.root;
        for seg in segs {
            let idx = match node.dirs.iter().position(|(n, _)| n == seg) {
                Some(i) => i,
                None => {
                    node.dirs.push((seg.to_string(), BuildDir::default()));
                    node.dirs.len() - 1
                }
            };
            node = &mut node.dirs[idx].1;
        }
        let (name, ext) = leaf.rsplit_once('.').unwrap_or((leaf, ""));
        node.files.push((
            name.to_string(),
            crate::name::pack_type(ext.as_bytes()),
            data.to_vec(),
        ));
        self
    }

    /// Serialises the archive.
    pub fn build(&self) -> Vec<u8> {
        let mut out = vec![0u8; HEADER_SIZE];
        out[0] = 0x0D;
        out[0x3F] = 0x0A;
        out[0x7E] = 0x1A;
        let title = b"Resource File";
        out[0x40..0x40 + title.len()].copy_from_slice(title);
        out[0x7F..0x83].copy_from_slice(&VERSION.to_le_bytes());
        let (offset, size) = Self::write_dir(&self.root, &mut out);
        out[0x83..0x87].copy_from_slice(&offset.to_le_bytes());
        out[0x87..0x8B].copy_from_slice(&size.to_le_bytes());
        let end = out.len() as u32;
        out[0x8F..0x93].copy_from_slice(&end.to_le_bytes());
        out
    }

    fn write_dir(dir: &BuildDir, out: &mut Vec<u8>) -> (u32, u32) {
        let mut block = Vec::new();
        for (name, sub) in &dir.dirs {
            let (offset, size) = Self::write_dir(sub, out);
            for v in [ENTRY_DIRECTORY, offset, size, 0] {
                block.extend_from_slice(&v.to_le_bytes());
            }
            block.extend_from_slice(name.as_bytes());
            block.push(0);
        }
        for (name, type_code, data) in &dir.files {
            let offset = out.len() as u32;
            out.extend_from_slice(data);
            for v in [0, offset, data.len() as u32, 0, 0, *type_code, 0] {
                block.extend_from_slice(&v.to_le_bytes());
            }
            block.extend_from_slice(name.as_bytes());
            block.extend_from_slice(&[0, 0]);
        }
        let offset = out.len() as u32;
        out.extend_from_slice(&block);
        (offset, block.len() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        RezBuilder::new()
            .file("LEVEL1/PALETTES/MAIN.PAL", &[7u8; 768])
            .file("LEVEL1/TILES/ACTION/001.PID", b"abc")
            .file("README", b"x")
            .build()
    }

    #[test]
    fn round_trip() {
        let data = sample();
        let header = parse_header(&data).unwrap();
        assert_eq!(header.version, 1);
        assert_eq!(header.title, "Resource File");
        let block = slice(&data, header.dir_offset as usize, header.dir_size as usize).unwrap();
        let entries = parse_block(block, data.len() as u64).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(matches!(&entries[0], RezEntry::Directory { name, .. } if name == "LEVEL1"));
        assert!(
            matches!(&entries[1], RezEntry::File { name, type_code: 0, size: 1, .. } if name == "README")
        );
    }

    #[test]
    fn header_checks() {
        let mut data = sample();
        assert!(parse_header(&data[..HEADER_SIZE - 1]).is_err());
        data[0x7F] = 2;
        assert!(parse_header(&data).is_err());
        let mut data = sample();
        data[0] = 0;
        assert!(parse_header(&data).is_err());
    }

    #[test]
    fn block_checks() {
        // A file entry pointing past the archive.
        let mut block = Vec::new();
        for v in [0u32, 100, 100, 0, 0, 0, 0] {
            block.extend_from_slice(&v.to_le_bytes());
        }
        block.extend_from_slice(b"A\0\0");
        assert!(parse_block(&block, 150).is_err());
        assert!(parse_block(&block, 200).is_ok());
        // Unterminated name.
        assert!(parse_block(&block[..block.len() - 2], 200).is_err());
        // Key count far larger than the block.
        let mut keys = block.clone();
        keys[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_block(&keys, 200).is_err());
        // Truncated entry head.
        assert!(parse_block(&block[..10], 200).is_err());
    }
}
