//! PCX pictures: full-screen images such as loading and menu screens.
//!
//! The game reads 256-colour PCX files: a 128-byte header, run-length
//! encoded pixels and a 768-byte palette at the end of the file.
//!
//! - The size is `xmax - xmin + 1` by `ymax - ymin + 1` from the header.
//!   Only 8-bit images with one colour plane are read.
//! - The pixels are one run-length stream of `width * height` bytes, row by
//!   row and `width` bytes per row (the header's bytes-per-line field is not
//!   used). A byte with both top bits set is a run: the next byte repeats
//!   `byte & 0x3F` times (so `0xC0` repeats it zero times); any other byte is
//!   one pixel. A run that is longer than what is left of its row carries on
//!   at the start of the next row.
//! - The palette is the last 768 bytes of the file, 256 (red, green, blue)
//!   triples, as the game reads it: whenever at least 768 bytes follow the
//!   pixel stream, the last 768 are the palette. The marker byte `0x0C`
//!   that normally sits in front of it is not checked. A file that ends
//!   sooner has no palette.
//!
//! Like every reader in this crate it treats its input as untrusted: the
//! size is bounded, a stream too short for the image is refused before
//! anything is allocated, and nothing panics.

use crate::bytes::{i16_at, u8_at};
use crate::error::{Result, format_error};
use crate::pal::{PALETTE_SIZE, Palette};

const FORMAT: &str = "PCX image";

/// Size of the header.
pub const HEADER_SIZE: usize = 128;
/// The byte in front of the palette at the end of the file.
pub const PALETTE_MARKER: u8 = 0x0C;
/// Largest width or height accepted.
pub const MAX_DIMENSION: u32 = 1 << 14;
/// Largest number of pixels accepted.
pub const MAX_PIXELS: u64 = 1 << 26;
/// Longest run one run byte encodes.
pub const MAX_RUN: usize = 0x3F;

/// An image of palette indices.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexedImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// One palette index per pixel, row by row.
    pub pixels: Vec<u8>,
}

impl IndexedImage {
    /// Palette index at (x, y), or `None` outside the image.
    pub fn index_at(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.pixels
            .get(y as usize * self.width as usize + x as usize)
            .copied()
    }
}

/// Decodes a PCX file into its pixels and, when the file carries one, its
/// palette.
pub fn decode(data: &[u8]) -> Result<(IndexedImage, Option<Palette>)> {
    if data.len() < HEADER_SIZE {
        return Err(format_error(FORMAT, "file shorter than its header"));
    }
    let bits = u8_at(data, 0x03).unwrap_or(0);
    let planes = u8_at(data, 0x41).unwrap_or(0);
    if bits != 8 || planes != 1 {
        return Err(format_error(
            FORMAT,
            format!(
                "{bits} bits per pixel in {planes} planes; only 8-bit, one-plane images are read"
            ),
        ));
    }
    let coord = |off| i32::from(i16_at(data, off).unwrap_or(0));
    let (xmin, ymin, xmax, ymax) = (coord(0x04), coord(0x06), coord(0x08), coord(0x0A));
    let width = xmax - xmin + 1;
    let height = ymax - ymin + 1;
    if width <= 0 || height <= 0 {
        return Err(format_error(
            FORMAT,
            format!("empty size {width}x{height} (x {xmin}..{xmax}, y {ymin}..{ymax})"),
        ));
    }
    let (width, height) = (width as u32, height as u32);
    let count = u64::from(width) * u64::from(height);
    if width > MAX_DIMENSION || height > MAX_DIMENSION || count > MAX_PIXELS {
        return Err(format_error(
            FORMAT,
            format!("size {width}x{height} is larger than accepted"),
        ));
    }
    let count = count as usize;
    let stream = &data[HEADER_SIZE..];
    // Two bytes give at most 63 pixels: refuse a stream that cannot cover
    // the image before allocating it.
    let reach = (stream.len() / 2) * MAX_RUN + (stream.len() % 2);
    if reach < count {
        return Err(format_error(
            FORMAT,
            format!(
                "{} bytes of pixel data cannot cover {width}x{height}",
                stream.len()
            ),
        ));
    }
    let mut pixels = vec![0u8; count];
    let mut out = 0usize;
    let mut pos = 0usize;
    while out < count {
        let Some(&b) = stream.get(pos) else {
            return Err(format_error(
                FORMAT,
                format!("pixel data ends after {out} of {count} pixels"),
            ));
        };
        pos += 1;
        if b & 0xC0 == 0xC0 {
            let Some(&value) = stream.get(pos) else {
                return Err(format_error(
                    FORMAT,
                    format!("run without a value after {out} of {count} pixels"),
                ));
            };
            pos += 1;
            let n = usize::from(b & 0x3F).min(count - out);
            pixels[out..out + n].fill(value);
            out += n;
        } else {
            pixels[out] = b;
            out += 1;
        }
    }
    let stream_end = HEADER_SIZE + pos;
    let palette = data
        .len()
        .checked_sub(PALETTE_SIZE)
        .filter(|&at| at >= stream_end)
        .and_then(|at| Palette::parse(&data[at..]).ok());
    Ok((
        IndexedImage {
            width,
            height,
            pixels,
        },
        palette,
    ))
}

/// Builds PCX bytes: a version 5, 8-bit, one-plane header for `width` x
/// `height`, the given encoded `stream` and, when given, the marker and the
/// palette. Meant for tests and tools.
pub fn build(width: u16, height: u16, stream: &[u8], palette: Option<&Palette>) -> Vec<u8> {
    let mut out = vec![0u8; HEADER_SIZE];
    out[0] = 0x0A;
    out[1] = 5;
    out[2] = 1;
    out[3] = 8;
    out[0x08..0x0A].copy_from_slice(&width.wrapping_sub(1).to_le_bytes());
    out[0x0A..0x0C].copy_from_slice(&height.wrapping_sub(1).to_le_bytes());
    out[0x41] = 1;
    out[0x42..0x44].copy_from_slice(&width.to_le_bytes());
    out.extend_from_slice(stream);
    if let Some(p) = palette {
        out.push(PALETTE_MARKER);
        for c in &p.0 {
            out.extend_from_slice(c);
        }
    }
    out
}

/// Encodes indices as one run-length stream over the whole image, so runs
/// may cross row ends. Meant for tests and tools.
pub fn encode(pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < pixels.len() {
        let v = pixels[i];
        let mut run = 1;
        while i + run < pixels.len() && pixels[i + run] == v && run < MAX_RUN {
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_and_run_bytes() {
        // 4x1: literal 7, a run of three 9s.
        let (img, pal) = decode(&build(4, 1, &[7, 0xC3, 9], None)).unwrap();
        assert_eq!((img.width, img.height), (4, 1));
        assert_eq!(img.pixels, vec![7, 9, 9, 9]);
        assert_eq!(pal, None);
        assert_eq!(img.index_at(3, 0), Some(9));
        assert_eq!(img.index_at(4, 0), None);
    }

    #[test]
    fn short_streams_are_refused() {
        assert!(decode(&build(4, 1, &[7, 0xC3], None)).is_err());
        assert!(decode(&build(4, 1, &[7], None)).is_err());
        assert!(decode(&[0; 10]).is_err());
    }

    #[test]
    fn the_palette_is_the_last_768_bytes_after_the_stream() {
        let pal = Palette(std::array::from_fn(|i| [i as u8, 1, 2]));
        let with = build(4, 1, &[7, 0xC3, 9], Some(&pal));
        assert_eq!(decode(&with).unwrap().1, Some(pal.clone()));
        // The marker is not checked.
        let mut other = with.clone();
        other[with.len() - PALETTE_SIZE - 1] = 0x55;
        assert_eq!(decode(&other).unwrap().1, Some(pal.clone()));
        // No marker at all: the 768 bytes right after the stream.
        let mut bare = build(4, 1, &[7, 0xC3, 9], None);
        for c in &pal.0 {
            bare.extend_from_slice(c);
        }
        assert_eq!(decode(&bare).unwrap().1, Some(pal));
        // Fewer than 768 bytes after the stream: no palette, whatever the
        // byte 769 from the end holds.
        let mut short = build(1000, 1, &encode(&[PALETTE_MARKER; 1000]), None);
        short.extend_from_slice(&[0; 767]);
        assert_eq!(decode(&short).unwrap().1, None);
    }
}
