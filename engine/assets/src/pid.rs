//! PID images: 8-bit indexed pictures.
//!
//! A PID is a 32-byte header (flags, size, hotspot offset) followed by a
//! compressed pixel stream, and optionally a 768-byte palette at the very end
//! of the file. Two encodings exist:
//!
//! - Sprite RLE (flag 0x20), read row by row: a control byte with the high
//!   bit set skips `c - 0x80` transparent pixels (so 0x80 skips none), any
//!   other control byte is followed by that many literal indices.
//! - PCX-style RLE (every other file): a byte with both top bits set is a
//!   run of `b & 0x3F` copies of the next byte, any other byte is one literal
//!   pixel. A run longer than the rest of its row carries into the next row.
//!
//! The reader bounds every read, refuses impossible sizes and never writes
//! outside the image.

use crate::bytes::{i32_at, u32_at};
use crate::error::{Result, format_error};
use crate::pal::{PALETTE_SIZE, Palette};

/// Size of the header.
pub const HEADER_SIZE: usize = 0x20;
/// Largest width or height accepted.
pub const MAX_DIMENSION: i32 = 1 << 15;
/// Largest pixel count accepted.
pub const MAX_PIXELS: usize = 1 << 26;

/// The image is drawn with palette index 0 as transparent.
pub const FLAG_COLOR_KEY: u32 = 0x1;
/// The stream is sprite RLE.
pub const FLAG_RLE: u32 = 0x20;
/// A palette is stored in the last 768 bytes.
pub const FLAG_PALETTE: u32 = 0x80;
/// The low byte of header word 6 is the sprite's key value.
pub const FLAG_KEY_BYTE: u32 = 0x100;

const FORMAT: &str = "PID image";

/// A decoded PID image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Horizontal offset of the image relative to its object.
    pub offset_x: i32,
    /// Vertical offset of the image relative to its object.
    pub offset_y: i32,
    /// Header flags.
    pub flags: u32,
    /// Palette indices, row-major, `width * height` bytes. Pixels skipped by
    /// sprite RLE are 0.
    pub pixels: Vec<u8>,
    /// For sprite RLE images, 1 for every pixel the stream wrote and 0 for
    /// skipped ones; `None` for PCX-style images.
    pub opaque: Option<Vec<u8>>,
    /// Embedded palette (flag 0x80).
    pub palette: Option<Palette>,
    /// Key value of a sprite with flag 0x100.
    pub key: Option<u8>,
    /// Stream bytes left after the last row.
    pub trailing: usize,
    /// Literal runs that passed the end of their row (clipped).
    pub row_overruns: u32,
    /// Runs that carried more than a whole row into the next row (clipped).
    pub wide_carries: u32,
}

impl PidImage {
    /// True for sprite RLE images.
    pub fn is_rle(&self) -> bool {
        self.flags & FLAG_RLE != 0
    }
}

/// Decodes a PID file.
pub fn decode(data: &[u8]) -> Result<PidImage> {
    if data.len() < HEADER_SIZE {
        return Err(format_error(FORMAT, "file shorter than its header"));
    }
    let word = |off| u32_at(data, off).unwrap_or(0);
    let flags = word(0x04);
    let width = i32_at(data, 0x08).unwrap_or(0);
    let height = i32_at(data, 0x0C).unwrap_or(0);
    if width <= 0 || height <= 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(format_error(FORMAT, format!("bad size {width}x{height}")));
    }
    let (w, h) = (width as usize, height as usize);
    if w * h > MAX_PIXELS {
        return Err(format_error(
            FORMAT,
            format!("image too large: {width}x{height}"),
        ));
    }
    if flags & FLAG_RLE == 0 && w % 4 != 0 {
        return Err(format_error(
            FORMAT,
            format!("width {width} is not a multiple of 4"),
        ));
    }
    let mut end = data.len();
    let mut palette = None;
    if flags & FLAG_PALETTE != 0 {
        if data.len() < HEADER_SIZE + PALETTE_SIZE {
            return Err(format_error(FORMAT, "too short for its palette"));
        }
        end -= PALETTE_SIZE;
        palette = Some(Palette::parse(&data[end..])?);
    }
    let body = &data[HEADER_SIZE..end];
    let key = (flags & FLAG_KEY_BYTE != 0).then(|| word(0x18) as u8);
    let mut image = PidImage {
        width: w as u32,
        height: h as u32,
        offset_x: i32_at(data, 0x10).unwrap_or(0),
        offset_y: i32_at(data, 0x14).unwrap_or(0),
        flags,
        pixels: Vec::new(),
        opaque: None,
        palette,
        key,
        trailing: 0,
        row_overruns: 0,
        wide_carries: 0,
    };
    let used = if flags & FLAG_RLE != 0 {
        let rle = decode_rle(body, w, h)?;
        image.pixels = rle.pixels;
        image.opaque = Some(rle.opaque);
        image.row_overruns = rle.overruns;
        rle.used
    } else {
        let pcx = decode_pcx(body, w, h)?;
        image.pixels = pcx.pixels;
        image.wide_carries = pcx.wide_carries;
        pcx.used
    };
    image.trailing = body.len() - used;
    Ok(image)
}

struct Rle {
    pixels: Vec<u8>,
    opaque: Vec<u8>,
    used: usize,
    overruns: u32,
}

fn decode_rle(body: &[u8], width: usize, height: usize) -> Result<Rle> {
    let mut pixels = vec![0u8; width * height];
    let mut opaque = vec![0u8; width * height];
    let mut pos = 0usize;
    let mut overruns = 0u32;
    for y in 0..height {
        let row = y * width;
        let mut x = 0usize;
        while x < width {
            let c = *body
                .get(pos)
                .ok_or_else(|| format_error(FORMAT, format!("sprite stream ends at row {y}")))?;
            pos += 1;
            if c & 0x80 != 0 {
                x += usize::from(c - 0x80);
                continue;
            }
            let count = usize::from(c);
            let run = body.get(pos..pos + count).ok_or_else(|| {
                format_error(FORMAT, format!("sprite literal run cut short at row {y}"))
            })?;
            let take = count.min(width - x);
            pixels[row + x..row + x + take].copy_from_slice(&run[..take]);
            opaque[row + x..row + x + take].fill(1);
            if take < count {
                overruns += 1;
            }
            pos += count;
            x += count;
        }
    }
    Ok(Rle {
        pixels,
        opaque,
        used: pos,
        overruns,
    })
}

struct Pcx {
    pixels: Vec<u8>,
    used: usize,
    wide_carries: u32,
}

fn decode_pcx(body: &[u8], width: usize, height: usize) -> Result<Pcx> {
    let mut pixels = vec![0u8; width * height];
    let mut pos = 0usize;
    let mut carry = 0usize;
    let mut value = 0u8;
    let mut wide_carries = 0u32;
    for y in 0..height {
        let row = y * width;
        let mut x = 0usize;
        if carry > 0 {
            if carry > width {
                wide_carries += 1;
            }
            let take = carry.min(width);
            pixels[row..row + take].fill(value);
            x = take;
            carry = 0;
        }
        while x < width {
            let b = *body
                .get(pos)
                .ok_or_else(|| format_error(FORMAT, format!("stream ends at row {y}")))?;
            pos += 1;
            if b & 0xC0 == 0xC0 {
                let mut count = usize::from(b & 0x3F);
                value = *body
                    .get(pos)
                    .ok_or_else(|| format_error(FORMAT, format!("run cut short at row {y}")))?;
                pos += 1;
                let room = width - x;
                if count > room {
                    carry = count - room;
                    count = room;
                }
                pixels[row + x..row + x + count].fill(value);
                x += count;
            } else {
                pixels[row + x] = b;
                x += 1;
            }
        }
    }
    Ok(Pcx {
        pixels,
        used: pos,
        wide_carries,
    })
}

/// Builds PID bytes from a header and an already encoded stream. Meant for
/// tests and tools.
pub fn build(width: i32, height: i32, flags: u32, offset: (i32, i32), stream: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_SIZE + stream.len());
    for v in [10u32, flags, width as u32, height as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in [offset.0, offset.1, 0, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(stream);
    out
}

/// Encodes indices with the PCX-style run-length code, one row at a time
/// (runs never cross a row end). Meant for tests and tools.
pub fn encode_pcx(pixels: &[u8], width: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for row in pixels.chunks(width.max(1)) {
        let mut i = 0;
        while i < row.len() {
            let v = row[i];
            let mut run = 1;
            while i + run < row.len() && row[i + run] == v && run < 63 {
                run += 1;
            }
            if run > 1 || v >= 0xC0 {
                out.push(0xC0 | run as u8);
                out.push(v);
            } else {
                out.push(v);
            }
            i += run;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_rle_with_the_skip_boundary() {
        // Row 0: skip 0 (0x80 means no pixels), then 2 literals, skip 2.
        // Row 1: 4 literals, the first one index 0 (still opaque).
        let stream = [0x80, 2, 5, 6, 0x82, 4, 0, 1, 2, 3];
        let img = decode(&build(4, 2, FLAG_RLE, (1, -2), &stream)).unwrap();
        assert_eq!(img.pixels, vec![5, 6, 0, 0, 0, 1, 2, 3]);
        assert_eq!(img.opaque.unwrap(), vec![1, 1, 0, 0, 1, 1, 1, 1]);
        assert_eq!((img.offset_x, img.offset_y), (1, -2));
        assert_eq!(img.trailing, 0);
        assert_eq!(img.row_overruns, 0);
    }

    #[test]
    fn sprite_rle_skip_of_full_row() {
        // 0xFF skips 127 pixels, more than the row; the row ends there.
        let stream = [0xFF, 0x80 + 3, 1, 9];
        let img = decode(&build(3, 2, FLAG_RLE, (0, 0), &stream)).unwrap();
        assert_eq!(img.pixels, vec![0, 0, 0, 0, 0, 0]);
        assert_eq!(img.trailing, 2);
    }

    #[test]
    fn sprite_rle_overrun_is_clipped() {
        let stream = [3, 1, 2, 3];
        let img = decode(&build(2, 1, FLAG_RLE, (0, 0), &stream)).unwrap();
        assert_eq!(img.pixels, vec![1, 2]);
        assert_eq!(img.row_overruns, 1);
    }

    #[test]
    fn sprite_rle_allows_odd_widths() {
        let img = decode(&build(3, 1, FLAG_RLE, (0, 0), &[3, 1, 2, 3])).unwrap();
        assert_eq!(img.pixels, vec![1, 2, 3]);
    }

    #[test]
    fn sprite_rle_truncated() {
        assert!(decode(&build(4, 2, FLAG_RLE, (0, 0), &[4, 1, 2, 3, 4])).is_err());
        assert!(decode(&build(4, 1, FLAG_RLE, (0, 0), &[4, 1, 2])).is_err());
    }

    #[test]
    fn pcx_round_trip_and_trailing_marker() {
        let pixels: Vec<u8> = (0..64u32)
            .map(|i| if i < 40 { 7 } else { (i * 5) as u8 })
            .collect();
        let mut stream = encode_pcx(&pixels, 8);
        stream.push(0x0C);
        let img = decode(&build(8, 8, 0x2, (0, 0), &stream)).unwrap();
        assert_eq!(img.pixels, pixels);
        assert_eq!(img.trailing, 1);
        assert!(img.opaque.is_none());
    }

    #[test]
    fn pcx_run_carries_into_next_row() {
        // A run of 6 in a 4-wide image fills row 0 and the first 2 of row 1.
        let stream = [0xC6, 9, 1, 2];
        let img = decode(&build(4, 2, 0, (0, 0), &stream)).unwrap();
        assert_eq!(img.pixels, vec![9, 9, 9, 9, 9, 9, 1, 2]);
    }

    #[test]
    fn pcx_wide_carry_is_clipped() {
        // A run of 20 in a 4x3 image: row 0, then a carry of 16 into row 1.
        let stream = [0xD4, 3, 0xC4, 5];
        let img = decode(&build(4, 3, 0, (0, 0), &stream)).unwrap();
        assert_eq!(img.wide_carries, 1);
        assert_eq!(&img.pixels[..8], &[3; 8]);
        assert_eq!(&img.pixels[8..], &[5; 4]);
    }

    #[test]
    fn pcx_rejects_width_not_multiple_of_four() {
        assert!(decode(&build(3, 1, 0, (0, 0), &[1, 2, 3])).is_err());
    }

    #[test]
    fn embedded_palette() {
        let mut data = build(4, 1, FLAG_PALETTE, (0, 0), &encode_pcx(&[1, 2, 3, 4], 4));
        let pal: Vec<u8> = (0..PALETTE_SIZE).map(|i| (i % 7) as u8).collect();
        data.extend_from_slice(&pal);
        let img = decode(&data).unwrap();
        assert_eq!(img.pixels, vec![1, 2, 3, 4]);
        assert_eq!(img.palette.unwrap().color(1), [3, 4, 5]);
        assert!(decode(&build(4, 1, FLAG_PALETTE, (0, 0), &[1, 2, 3, 4])).is_err());
    }

    #[test]
    fn bad_headers() {
        assert!(decode(&[0u8; 10]).is_err());
        assert!(decode(&build(0, 4, 0, (0, 0), &[])).is_err());
        assert!(decode(&build(-4, 4, 0, (0, 0), &[])).is_err());
        assert!(decode(&build(MAX_DIMENSION + 4, 4, 0, (0, 0), &[])).is_err());
        assert!(decode(&build(MAX_DIMENSION, MAX_DIMENSION, 0, (0, 0), &[])).is_err());
    }

    #[test]
    fn key_byte() {
        let mut data = build(1, 1, FLAG_RLE | FLAG_KEY_BYTE, (0, 0), &[1, 4]);
        data[0x18] = 0x2A;
        assert_eq!(decode(&data).unwrap().key, Some(0x2A));
    }
}
