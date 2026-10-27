//! A byte range of a larger reader, read as a file of its own, with its
//! first bytes optionally replaced.

use std::io::{self, Read, Seek, SeekFrom};

/// `[offset, offset + len)` of `inner` as a reader of its own: reads stop
/// at the range's end, seeks are relative to the range (`SeekFrom::End` to
/// its end), and the range's first four bytes read as `patch` when one is
/// given.
#[derive(Debug)]
pub struct PatchedRange<R: Read + Seek> {
    inner: R,
    offset: u64,
    len: u64,
    patch: Option<[u8; 4]>,
    pos: u64,
}

impl<R: Read + Seek> PatchedRange<R> {
    /// The range `[offset, offset + len)` of `inner`, starting at its first
    /// byte.
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
        };
        range.inner.seek(SeekFrom::Start(offset))?;
        range.pos = 0;
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
        let want = (buf.len() as u64).min(left) as usize;
        if want == 0 {
            return Ok(0);
        }
        self.inner.seek(SeekFrom::Start(self.offset + self.pos))?;
        let n = self.inner.read(&mut buf[..want])?;
        if let Some(patch) = self.patch {
            for (i, b) in buf[..n].iter_mut().enumerate() {
                let at = self.pos + i as u64;
                if at < 4 {
                    *b = patch[at as usize];
                }
            }
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
        self.pos = to;
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
}
