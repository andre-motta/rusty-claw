//! A byte range of a larger reader, read as a file of its own, with its
//! first bytes optionally replaced. Nothing is ever written: the
//! replacement happens as the bytes are read.

use std::io::{self, Read, Seek, SeekFrom};

/// `[offset, offset + len)` of `inner` as a reader of its own: reads stop
/// at the range's end, seeks are relative to the range (`SeekFrom::End` to
/// its end), and the range's first four bytes read as `patch` when one is
/// given, whatever the size and start of each read.
///
/// Seeking past the end is allowed, as for a file, and reads there return
/// nothing; seeking before the start is an error.
#[derive(Debug)]
pub struct PatchedRange<R: Read + Seek> {
    inner: R,
    offset: u64,
    len: u64,
    patch: Option<[u8; 4]>,
    /// The position in the range.
    pos: u64,
    /// Whether `inner` is at `offset + pos`, so a read needs no seek.
    in_place: bool,
}

impl<R: Read + Seek> PatchedRange<R> {
    /// The range `[offset, offset + len)` of `inner`, starting at its first
    /// byte. Fails when the range's end does not fit in 64 bits or `inner`
    /// cannot seek to its start.
    pub fn new(inner: R, offset: u64, len: u64, patch: Option<[u8; 4]>) -> io::Result<Self> {
        offset
            .checked_add(len)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range too long"))?;
        let mut range = PatchedRange {
            inner,
            offset,
            len,
            patch,
            pos: 0,
            in_place: false,
        };
        range.inner.seek(SeekFrom::Start(offset))?;
        range.in_place = true;
        Ok(range)
    }

    /// The range's length.
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Whether the range is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<R: Read + Seek> Read for PatchedRange<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.len.saturating_sub(self.pos);
        let want = usize::try_from(left).map_or(buf.len(), |left| left.min(buf.len()));
        if want == 0 {
            return Ok(0);
        }
        if !self.in_place {
            // pos < len here, and offset + len fits (checked in `new`).
            self.inner.seek(SeekFrom::Start(self.offset + self.pos))?;
            self.in_place = true;
        }
        let n = match self.inner.read(&mut buf[..want]) {
            Ok(n) => n,
            Err(e) => {
                self.in_place = false;
                return Err(e);
            }
        };
        if let Some(patch) = self.patch
            && self.pos < 4
        {
            let from = self.pos as usize;
            let to = (from + n).min(4);
            buf[..to - from].copy_from_slice(&patch[from..to]);
        }
        self.pos += n as u64;
        Ok(n)
    }
}

impl<R: Read + Seek> Seek for PatchedRange<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let to = match pos {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
            SeekFrom::End(d) => self.len.checked_add_signed(d),
        };
        let to = to.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "seek before the range's start")
        })?;
        if to != self.pos {
            self.pos = to;
            self.in_place = false;
        }
        Ok(to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn a_range_reads_as_its_own_file_with_its_signature_replaced() {
        let data: Vec<u8> = (0..100u8).collect();
        let mut r = PatchedRange::new(Cursor::new(data), 10, 20, Some(*b"ABCD")).unwrap();
        let mut all = Vec::new();
        r.read_to_end(&mut all).unwrap();
        let mut want: Vec<u8> = (10..30u8).collect();
        want[..4].copy_from_slice(b"ABCD");
        assert_eq!(all, want);
        assert_eq!(r.seek(SeekFrom::End(-2)).unwrap(), 18);
        let mut two = [0u8; 8];
        assert_eq!(r.read(&mut two).unwrap(), 2);
        assert_eq!(&two[..2], &[28, 29]);
        r.seek(SeekFrom::Start(2)).unwrap();
        let mut b = [0u8; 3];
        r.read_exact(&mut b).unwrap();
        assert_eq!(&b, &[b'C', b'D', 14]);
        assert!(r.seek(SeekFrom::Current(-10)).is_err());
    }

    #[test]
    fn single_byte_reads_see_the_patch_too() {
        let data: Vec<u8> = (0..10u8).collect();
        let mut r = PatchedRange::new(Cursor::new(data), 3, 6, Some(*b"WXYZ")).unwrap();
        let mut got = Vec::new();
        let mut b = [0u8; 1];
        while r.read(&mut b).unwrap() == 1 {
            got.push(b[0]);
        }
        assert_eq!(got, [b'W', b'X', b'Y', b'Z', 7, 8]);
    }

    #[test]
    fn a_range_shorter_than_the_patch_gets_only_its_part() {
        let mut r = PatchedRange::new(Cursor::new(vec![1u8, 2, 3]), 1, 2, Some(*b"ABCD")).unwrap();
        let mut all = Vec::new();
        r.read_to_end(&mut all).unwrap();
        assert_eq!(all, b"AB");
    }
}
