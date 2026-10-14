//! CLT colour lookup tables: a u32 byte count followed by that many table
//! bytes, kept verbatim.
//!
//! Two shapes occur. A pair table (65536 bytes) maps a pair of palette
//! indices to a result index, entry `a * 256 + b`. A level table (16384
//! bytes) maps a palette index at one of 64 levels to a result index, entry
//! `index * 64 + level`, where level 32 is close to identity. Other sizes are
//! accepted and kept as they are.

use crate::bytes::{to_usize, u32_at};
use crate::error::{Result, format_error};

/// Size of a pair table.
pub const PAIR_TABLE_SIZE: usize = 256 * 256;
/// Size of a level table.
pub const LEVEL_TABLE_SIZE: usize = 256 * 64;

/// Shape of a table, from its size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CltKind {
    /// 256 x 256 index pairs.
    Pair,
    /// 256 indices x 64 levels.
    Level,
    /// Any other size.
    Other,
}

/// A colour lookup table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clt {
    /// The table bytes.
    pub table: Vec<u8>,
    /// Bytes after the table, which are ignored.
    pub trailing: usize,
}

impl Clt {
    /// Reads a table.
    pub fn parse(data: &[u8]) -> Result<Clt> {
        let size = u32_at(data, 0)
            .map(to_usize)
            .ok_or_else(|| format_error("CLT", "file shorter than 4 bytes"))?;
        let body = &data[4..];
        let table = body.get(..size).ok_or_else(|| {
            format_error(
                "CLT",
                format!("declares {size} table bytes, file holds {}", body.len()),
            )
        })?;
        Ok(Clt {
            table: table.to_vec(),
            trailing: body.len() - size,
        })
    }

    /// Shape of the table.
    pub fn kind(&self) -> CltKind {
        match self.table.len() {
            PAIR_TABLE_SIZE => CltKind::Pair,
            LEVEL_TABLE_SIZE => CltKind::Level,
            _ => CltKind::Other,
        }
    }

    /// Result index for the pair (a, b) of a pair table.
    pub fn pair(&self, a: u8, b: u8) -> Option<u8> {
        (self.kind() == CltKind::Pair).then(|| self.table[usize::from(a) * 256 + usize::from(b)])
    }

    /// Result index for `index` at `level` (0..64) of a level table.
    pub fn level(&self, index: u8, level: u8) -> Option<u8> {
        (self.kind() == CltKind::Level && level < 64)
            .then(|| self.table[usize::from(index) * 64 + usize::from(level)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(table: &[u8], extra: &[u8]) -> Vec<u8> {
        let mut out = (table.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(table);
        out.extend_from_slice(extra);
        out
    }

    #[test]
    fn pair_and_level_tables() {
        let pair: Vec<u8> = (0..PAIR_TABLE_SIZE).map(|i| (i / 256) as u8).collect();
        let clt = Clt::parse(&build(&pair, &[])).unwrap();
        assert_eq!(clt.kind(), CltKind::Pair);
        assert_eq!(clt.pair(7, 9), Some(7));
        assert_eq!(clt.level(7, 9), None);

        let level: Vec<u8> = (0..LEVEL_TABLE_SIZE).map(|i| (i % 64) as u8).collect();
        let clt = Clt::parse(&build(&level, b"zz")).unwrap();
        assert_eq!(clt.kind(), CltKind::Level);
        assert_eq!(clt.level(3, 32), Some(32));
        assert_eq!(clt.level(3, 64), None);
        assert_eq!(clt.trailing, 2);
    }

    #[test]
    fn other_sizes_are_kept() {
        let clt = Clt::parse(&build(&[1, 2, 3], &[])).unwrap();
        assert_eq!(clt.kind(), CltKind::Other);
        assert_eq!(clt.table, vec![1, 2, 3]);
    }

    #[test]
    fn rejects_short_files() {
        assert!(Clt::parse(&[1, 0]).is_err());
        let mut data = build(&[1, 2, 3], &[]);
        data.truncate(5);
        assert!(Clt::parse(&data).is_err());
        assert!(Clt::parse(&u32::MAX.to_le_bytes()).is_err());
    }
}
