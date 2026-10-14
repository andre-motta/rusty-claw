//! ANI animations: an image-set name and an ordered list of frames.
//!
//! Layout: a 32-byte header (flags at +0x08, frame count at +0x0C, name
//! length at +0x10), the image-set name (not NUL-terminated), then one
//! 20-byte record per frame. A frame with flag 0x2 is followed by a
//! NUL-terminated list of sound names.
//!
//! Bytes after the last frame are accepted. A final frame record that is cut
//! short is accepted too and its missing fields read as 0.

use crate::bytes::{i16_at, latin1, slice, to_usize, u16_at, u32_at};
use crate::error::{Result, format_error};

/// Size of the header.
pub const HEADER_SIZE: usize = 0x20;
/// Size of one frame record.
pub const FRAME_SIZE: usize = 20;
/// Frame flag: the duration counts 22 ms units.
pub const FRAME_FLAG_UNITS: u16 = 0x1;
/// Frame flag: a sound list follows the frame record.
pub const FRAME_FLAG_SOUNDS: u16 = 0x2;
/// Duration used for a frame whose duration is 0 or negative, and the unit
/// of frames with [`FRAME_FLAG_UNITS`].
pub const DURATION_UNIT_MS: i32 = 22;

const FORMAT: &str = "ANI animation";

/// One animation frame. Fields without a known meaning keep their raw
/// values so later work can name them without a format change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AniFrame {
    /// Frame flags.
    pub flags: u16,
    /// Raw field at +0x02.
    pub field1: i16,
    /// Raw field at +0x04.
    pub field2: i16,
    /// Raw field at +0x06.
    pub field3: i16,
    /// Frame number of the image inside the image set.
    pub image_index: i16,
    /// Display time as stored (milliseconds, or 22 ms units with flag 0x1).
    pub duration: i16,
    /// Raw field at +0x0C.
    pub field6: i16,
    /// Raw field at +0x0E.
    pub field7: i16,
    /// Raw field at +0x10.
    pub field8: i16,
    /// Raw field at +0x12.
    pub field9: u16,
    /// Sound names triggered when the frame starts.
    pub sounds: Vec<String>,
}

impl AniFrame {
    /// Display time in milliseconds.
    pub fn duration_ms(&self) -> i32 {
        let d = i32::from(self.duration);
        if d <= 0 {
            DURATION_UNIT_MS
        } else if self.flags & FRAME_FLAG_UNITS != 0 {
            d * DURATION_UNIT_MS
        } else {
            d
        }
    }
}

/// A parsed animation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ani {
    /// Header flags.
    pub flags: u32,
    /// Image-set name, possibly empty.
    pub image_set: String,
    /// Frames in order.
    pub frames: Vec<AniFrame>,
    /// Bytes after the last frame.
    pub trailing: usize,
    /// Bytes missing from a final frame record that was cut short.
    pub missing: usize,
}

impl Ani {
    /// Total duration of one pass in milliseconds.
    pub fn total_ms(&self) -> i64 {
        self.frames.iter().map(|f| i64::from(f.duration_ms())).sum()
    }

    /// True when the file ends exactly after its last frame.
    pub fn is_exact(&self) -> bool {
        self.trailing == 0 && self.missing == 0
    }

    /// Reads an animation.
    pub fn parse(data: &[u8]) -> Result<Ani> {
        if data.len() < HEADER_SIZE {
            return Err(format_error(FORMAT, "file shorter than its header"));
        }
        let flags = u32_at(data, 0x08).unwrap_or(0);
        let frame_count = u32_at(data, 0x0C).unwrap_or(0) as i32;
        let name_len = to_usize(u32_at(data, 0x10).unwrap_or(0));
        let name = slice(data, HEADER_SIZE, name_len)
            .ok_or_else(|| format_error(FORMAT, "image-set name runs past the end"))?;
        let mut ani = Ani {
            flags,
            image_set: latin1(name),
            ..Ani::default()
        };
        let mut pos = HEADER_SIZE + name_len;
        let count = usize::try_from(frame_count).unwrap_or(0);
        for i in 0..count {
            let available = data.len() - pos;
            if available >= FRAME_SIZE {
                let frame = read_frame(&data[pos..pos + FRAME_SIZE]);
                pos += FRAME_SIZE;
                let frame = if frame.flags & FRAME_FLAG_SOUNDS != 0 {
                    let rest = &data[pos..];
                    let len = rest.iter().position(|&b| b == 0).ok_or_else(|| {
                        format_error(FORMAT, format!("unterminated sound list in frame {i}"))
                    })?;
                    pos += len + 1;
                    AniFrame {
                        sounds: split_sounds(&rest[..len]),
                        ..frame
                    }
                } else {
                    frame
                };
                ani.frames.push(frame);
            } else if i + 1 == count && available > 0 {
                let mut record = [0u8; FRAME_SIZE];
                record[..available].copy_from_slice(&data[pos..]);
                ani.frames.push(read_frame(&record));
                ani.missing = FRAME_SIZE - available;
                pos = data.len();
            } else {
                return Err(format_error(
                    FORMAT,
                    format!("declares {count} frames, file ends in frame {i}"),
                ));
            }
        }
        ani.trailing = data.len() - pos;
        Ok(ani)
    }
}

fn read_frame(record: &[u8]) -> AniFrame {
    let s = |off| i16_at(record, off).unwrap_or(0);
    AniFrame {
        flags: u16_at(record, 0x00).unwrap_or(0),
        field1: s(0x02),
        field2: s(0x04),
        field3: s(0x06),
        image_index: s(0x08),
        duration: s(0x0A),
        field6: s(0x0C),
        field7: s(0x0E),
        field8: s(0x10),
        field9: u16_at(record, 0x12).unwrap_or(0),
        sounds: Vec::new(),
    }
}

/// Splits a sound list at control characters, spaces, `!` and every byte
/// from 0x80 up; empty names are dropped.
fn split_sounds(raw: &[u8]) -> Vec<String> {
    raw.split(|&b| (b as i8) <= 0x21)
        .filter(|s| !s.is_empty())
        .map(latin1)
        .collect()
}

/// Builds ANI bytes. Meant for tests and tools.
pub fn build(image_set: &str, frames: &[AniFrame]) -> Vec<u8> {
    let mut out = Vec::new();
    for v in [
        HEADER_SIZE as u32,
        0,
        0,
        frames.len() as u32,
        image_set.len() as u32,
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(image_set.as_bytes());
    for f in frames {
        out.extend_from_slice(&f.flags.to_le_bytes());
        for v in [
            f.field1,
            f.field2,
            f.field3,
            f.image_index,
            f.duration,
            f.field6,
            f.field7,
            f.field8,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&f.field9.to_le_bytes());
        if f.flags & FRAME_FLAG_SOUNDS != 0 {
            out.extend_from_slice(f.sounds.join(" ").as_bytes());
            out.push(0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(image_index: i16, duration: i16) -> AniFrame {
        AniFrame {
            image_index,
            duration,
            field1: 3,
            ..AniFrame::default()
        }
    }

    #[test]
    fn round_trip_with_sounds() {
        let mut with_sound = frame(2, 5);
        with_sound.flags = FRAME_FLAG_SOUNDS | FRAME_FLAG_UNITS;
        with_sound.sounds = vec!["LEVEL_DRIP".into(), "GAME_HIT".into()];
        let frames = vec![frame(1, 100), with_sound.clone(), frame(3, 0)];
        let ani = Ani::parse(&build("LEVEL_RAT", &frames)).unwrap();
        assert_eq!(ani.image_set, "LEVEL_RAT");
        assert_eq!(ani.frames, frames);
        assert!(ani.is_exact());
        assert_eq!(ani.total_ms(), 100 + 5 * 22 + 22);
    }

    #[test]
    fn sound_list_separators() {
        assert_eq!(
            split_sounds(b"A!B  C\x01D\xE9E"),
            vec!["A", "B", "C", "D", "E"]
        );
    }

    #[test]
    fn trailing_bytes_and_short_final_frame() {
        let mut data = build("", &[frame(1, 10)]);
        data.push(0);
        let ani = Ani::parse(&data).unwrap();
        assert_eq!(ani.trailing, 1);

        let data = build("", &[frame(1, 10), frame(17, -1)]);
        let short = &data[..data.len() - 6];
        let ani = Ani::parse(short).unwrap();
        assert_eq!(ani.missing, 6);
        assert_eq!(ani.frames[1].image_index, 17);
        assert_eq!(ani.frames[1].field7, 0);
        assert_eq!(ani.frames[1].duration_ms(), 22);
    }

    #[test]
    fn malformed() {
        assert!(Ani::parse(&[0u8; 8]).is_err());
        let data = build("", &[frame(1, 10), frame(2, 10)]);
        // Two frames declared, only the first and none of the second.
        assert!(Ani::parse(&data[..HEADER_SIZE + FRAME_SIZE]).is_err());
        // Name longer than the file.
        let mut data = build("AB", &[]);
        data[0x10] = 0xFF;
        assert!(Ani::parse(&data).is_err());
        // Unterminated sound list.
        let mut f = frame(1, 1);
        f.flags = FRAME_FLAG_SOUNDS;
        let mut data = build("", &[f]);
        data.pop();
        assert!(Ani::parse(&data).is_err());
        // Negative and huge counts.
        let mut data = build("", &[]);
        data[0x0C..0x10].copy_from_slice(&(-5i32).to_le_bytes());
        assert_eq!(Ani::parse(&data).unwrap().frames.len(), 0);
        data[0x0C..0x10].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(Ani::parse(&data).is_err());
    }
}
