//! WAV sounds: a RIFF container with a `fmt ` chunk describing
//! uncompressed PCM and a `data` chunk holding the samples.
//!
//! The reader walks the chunks the way the game does: the file starts with
//! `RIFF`, a 32-bit size and `WAVE`; chunks follow as a 4-byte id and a
//! 32-bit size, each skipped by its size rounded up to an even number; a
//! later `fmt ` chunk replaces an earlier one; the first `data` chunk ends
//! the walk and needs a `fmt ` chunk before it. The walk stops at the end
//! the RIFF size names or at the end of the buffer, whichever comes first.
//!
//! On top of the game's walk the reader refuses a `fmt ` chunk shorter than
//! 16 bytes, a `data` body that runs past the end of the buffer, a format
//! other than PCM, sample sizes other than 8 and 16 bits, and zero channels
//! or sample rate. 8-bit samples are unsigned and become signed 16-bit
//! values; 16-bit samples are signed little-endian.

use crate::bytes::{slice, to_usize, u16_at, u32_at};
use crate::error::{Result, format_error};

const FORMAT: &str = "WAV sound";

/// Format tag of uncompressed PCM.
pub const FORMAT_PCM: u16 = 1;

/// Size of the fields of a `fmt ` chunk the reader uses.
pub const FMT_MIN_SIZE: usize = 16;

/// A decoded sound.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wav {
    /// Channels, interleaved in `samples`.
    pub channels: u16,
    /// Sample frames per second.
    pub rate: u32,
    /// Bits per sample in the file (8 or 16).
    pub bits: u16,
    /// Samples as signed 16-bit values, channels interleaved; a partial
    /// sample frame at the end of the data is dropped.
    pub samples: Vec<i16>,
}

impl Wav {
    /// Length in milliseconds at the sound's own rate.
    pub fn duration_ms(&self) -> u32 {
        let frames = self.samples.len() as u64 / u64::from(self.channels.max(1));
        let ms = frames * 1000 / u64::from(self.rate.max(1));
        u32::try_from(ms).unwrap_or(u32::MAX)
    }
}

/// The format fields and the sample bytes of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Chunks<'a> {
    tag: u16,
    channels: u16,
    rate: u32,
    byte_rate: u32,
    block_align: u16,
    bits: u16,
    data: &'a [u8],
}

fn walk(buf: &[u8]) -> Result<Chunks<'_>> {
    if slice(buf, 0, 4) != Some(b"RIFF") || slice(buf, 8, 4) != Some(b"WAVE") {
        return Err(format_error(FORMAT, "not a RIFF WAVE file"));
    }
    let riff_size = u32_at(buf, 4).unwrap_or(0);
    let end = (8u64 + u64::from(riff_size)).min(buf.len() as u64) as usize;
    let mut pos = 12usize;
    let mut fmt: Option<&[u8]> = None;
    while pos < end {
        let (Some(id), Some(size)) = (slice(buf, pos, 4), u32_at(buf, pos + 4)) else {
            break;
        };
        let body = pos + 8;
        if id == b"data" {
            let Some(fmt) = fmt else {
                return Err(format_error(FORMAT, "data chunk before any fmt chunk"));
            };
            let data = slice(buf, body, to_usize(size))
                .ok_or_else(|| format_error(FORMAT, "data chunk runs past the end"))?;
            return read_fmt(fmt, data);
        }
        if id == b"fmt " {
            let start = body.min(buf.len());
            let len = to_usize(size).min(buf.len() - start);
            fmt = Some(&buf[start..start + len]);
        }
        let skip = u64::from(size) + u64::from(size & 1);
        pos = match usize::try_from(body as u64 + skip) {
            Ok(p) => p,
            Err(_) => break,
        };
    }
    Err(format_error(FORMAT, "no data chunk"))
}

fn read_fmt<'a>(fmt: &[u8], data: &'a [u8]) -> Result<Chunks<'a>> {
    if fmt.len() < FMT_MIN_SIZE {
        return Err(format_error(FORMAT, "fmt chunk shorter than 16 bytes"));
    }
    let c = Chunks {
        tag: u16_at(fmt, 0).unwrap_or(0),
        channels: u16_at(fmt, 2).unwrap_or(0),
        rate: u32_at(fmt, 4).unwrap_or(0),
        byte_rate: u32_at(fmt, 8).unwrap_or(0),
        block_align: u16_at(fmt, 12).unwrap_or(0),
        bits: u16_at(fmt, 14).unwrap_or(0),
        data,
    };
    if c.tag != FORMAT_PCM {
        return Err(format_error(FORMAT, format!("format {} is not PCM", c.tag)));
    }
    if c.bits != 8 && c.bits != 16 {
        return Err(format_error(FORMAT, format!("{} bits per sample", c.bits)));
    }
    if c.channels == 0 || c.rate == 0 {
        return Err(format_error(FORMAT, "no channels or a zero sample rate"));
    }
    Ok(c)
}

/// Reads and decodes a sound.
pub fn parse(buf: &[u8]) -> Result<Wav> {
    let c = walk(buf)?;
    let bytes = usize::from(c.bits / 8);
    let frame = bytes * usize::from(c.channels);
    let whole = c.data.len() / frame * frame;
    let data = &c.data[..whole];
    let samples = if c.bits == 8 {
        data.iter().map(|&b| (i16::from(b) - 128) << 8).collect()
    } else {
        data.chunks_exact(2)
            .map(|p| i16::from_le_bytes([p[0], p[1]]))
            .collect()
    };
    Ok(Wav {
        channels: c.channels,
        rate: c.rate,
        bits: c.bits,
        samples,
    })
}

/// Length of a sound in milliseconds, without decoding its samples: the
/// size of the sample data over the bytes played per second (the format's
/// own figure, or the rate times the block size when that is 0).
pub fn duration_ms(buf: &[u8]) -> Result<u32> {
    let c = walk(buf)?;
    let per_second = if c.byte_rate != 0 {
        u64::from(c.byte_rate)
    } else {
        let block = if c.block_align != 0 {
            u64::from(c.block_align)
        } else {
            u64::from(c.bits / 8) * u64::from(c.channels)
        };
        u64::from(c.rate) * block
    };
    if per_second == 0 {
        return Err(format_error(FORMAT, "no byte rate"));
    }
    let ms = c.data.len() as u64 * 1000 / per_second;
    Ok(u32::try_from(ms).unwrap_or(u32::MAX))
}

/// Builds a PCM sound file from raw sample bytes, for tests and tools.
pub fn build(channels: u16, rate: u32, bits: u16, data: &[u8]) -> Vec<u8> {
    let block = channels * (bits / 8);
    let mut out = Vec::with_capacity(44 + data.len() + 1);
    let riff = 4 + 8 + 16 + 8 + data.len() + (data.len() & 1);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(riff as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&FORMAT_PCM.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    out.extend_from_slice(&block.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() & 1 == 1 {
        out.push(0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_bit_mono() {
        let file = build(1, 22050, 8, &[128, 255, 0, 129]);
        let w = parse(&file).unwrap();
        assert_eq!((w.channels, w.rate, w.bits), (1, 22050, 8));
        assert_eq!(w.samples, vec![0, 127 << 8, -128 << 8, 1 << 8]);
    }

    #[test]
    fn sixteen_bit_stereo() {
        let mut data = Vec::new();
        for v in [1i16, -1, 300, -300, 7] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        let w = parse(&build(2, 44100, 16, &data)).unwrap();
        // The fifth sample is half a frame and is dropped.
        assert_eq!(w.samples, vec![1, -1, 300, -300]);
    }
}
