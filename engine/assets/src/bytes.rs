//! Bounds-checked little-endian reads. Every accessor returns `None` instead
//! of panicking when the requested range is outside the buffer.

pub(crate) fn slice(buf: &[u8], offset: usize, len: usize) -> Option<&[u8]> {
    let end = offset.checked_add(len)?;
    buf.get(offset..end)
}

pub(crate) fn u8_at(buf: &[u8], offset: usize) -> Option<u8> {
    buf.get(offset).copied()
}

pub(crate) fn u16_at(buf: &[u8], offset: usize) -> Option<u16> {
    let b = slice(buf, offset, 2)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

pub(crate) fn i16_at(buf: &[u8], offset: usize) -> Option<i16> {
    u16_at(buf, offset).map(|v| v as i16)
}

pub(crate) fn u32_at(buf: &[u8], offset: usize) -> Option<u32> {
    let b = slice(buf, offset, 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub(crate) fn i32_at(buf: &[u8], offset: usize) -> Option<i32> {
    u32_at(buf, offset).map(|v| v as i32)
}

/// Converts a `u32` offset or count from a file into `usize`.
pub(crate) fn to_usize(v: u32) -> usize {
    // usize is at least 32 bits on every supported target.
    v as usize
}

/// Decodes single-byte text: every byte becomes the character with the same
/// code point.
pub(crate) fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// Decodes a fixed-size, NUL-terminated text field.
pub(crate) fn cstr_field(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    latin1(&bytes[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_bounded() {
        let buf = [1u8, 2, 3, 4, 5];
        assert_eq!(u32_at(&buf, 0), Some(0x0403_0201));
        assert_eq!(u32_at(&buf, 2), None);
        assert_eq!(u16_at(&buf, 4), None);
        assert_eq!(slice(&buf, usize::MAX, 2), None);
        assert_eq!(u8_at(&buf, 5), None);
        assert_eq!(i16_at(&[0xFF, 0xFF], 0), Some(-1));
    }

    #[test]
    fn text_fields() {
        assert_eq!(cstr_field(b"AB\0CD"), "AB");
        assert_eq!(cstr_field(b"ABC"), "ABC");
        assert_eq!(latin1(&[0x41, 0xE9]), "A\u{e9}");
    }
}
