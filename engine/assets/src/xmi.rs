//! XMIDI music: the IFF container and its event streams.
//!
//! The game's music tracks are XMIDI files, an extended MIDI format. A file
//! is an IFF container (chunk sizes big-endian, odd chunks padded to even):
//!
//! ```text
//! FORM <size> XDIR
//!     INFO <2> u16 sequence count (little-endian)
//! CAT  <size> XMID
//!     FORM <size> XMID            one per sequence
//!         TIMB <size> u16 count, count x (u8 patch, u8 bank)
//!         EVNT <size> event stream
//! ```
//!
//! A bare `FORM XMID` (one sequence, no directory) is accepted too. The
//! reader tolerates a sequence count that disagrees with the catalogue and a
//! `TIMB` list shorter than its count, and skips unknown chunks; anything
//! that runs past its container is an error.
//!
//! The event stream differs from a standard MIDI track in three ways:
//!
//! - time: one or more bytes below `0x80` at an event boundary are a delay,
//!   and consecutive delay bytes add up; time runs at 120 ticks per second;
//! - a note-on carries its own length as a variable-length quantity after
//!   the velocity, so a stream has no note-off events of its own;
//! - there is no running status: every event starts with its status byte.
//!
//! Decoding stops at the end-of-track meta event; bytes after it are
//! counted, not read. Every count and length is checked against the input,
//! and malformed data returns an error instead of panicking.

use crate::error::{Result, format_error};

const FORMAT: &str = "XMI music";

/// Most sequences one file may hold.
pub const MAX_SEQUENCES: usize = 256;

/// Most events one sequence may hold.
pub const MAX_EVENTS: usize = 1 << 22;

/// Ticks per second of the XMIDI clock.
pub const TICKS_PER_SECOND: u32 = 120;

/// Largest value a variable-length quantity of four bytes holds.
pub const MAX_VLQ: u32 = 0x0FFF_FFFF;

/// One entry of a sequence's instrument list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Timbre {
    /// Program number.
    pub patch: u8,
    /// Bank number.
    pub bank: u8,
}

/// What one event does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmiEventKind {
    /// A note that sounds for `duration` ticks.
    Note {
        /// MIDI channel 0..=15.
        channel: u8,
        /// Key 0..=127.
        key: u8,
        /// Velocity 0..=127.
        velocity: u8,
        /// Length in ticks.
        duration: u32,
    },
    /// An explicit note-off.
    NoteOff {
        /// MIDI channel 0..=15.
        channel: u8,
        /// Key 0..=127.
        key: u8,
        /// Release velocity 0..=127.
        velocity: u8,
    },
    /// Polyphonic key pressure.
    KeyPressure {
        /// MIDI channel 0..=15.
        channel: u8,
        /// Key 0..=127.
        key: u8,
        /// Pressure 0..=127.
        pressure: u8,
    },
    /// A controller change.
    Controller {
        /// MIDI channel 0..=15.
        channel: u8,
        /// Controller number 0..=127.
        controller: u8,
        /// Value 0..=127.
        value: u8,
    },
    /// A program change.
    Program {
        /// MIDI channel 0..=15.
        channel: u8,
        /// Program 0..=127.
        program: u8,
    },
    /// Channel pressure.
    ChannelPressure {
        /// MIDI channel 0..=15.
        channel: u8,
        /// Pressure 0..=127.
        pressure: u8,
    },
    /// Pitch bend, 0..=0x3FFF with 0x2000 centred.
    PitchBend {
        /// MIDI channel 0..=15.
        channel: u8,
        /// 14-bit bend value.
        value: u16,
    },
    /// A tempo meta event (microseconds per quarter note).
    Tempo {
        /// Microseconds per quarter note.
        micros_per_quarter: u32,
    },
    /// The end-of-track meta event.
    EndOfTrack,
    /// Any other meta event.
    Meta {
        /// Meta type 0..=127.
        kind: u8,
        /// Its data.
        data: Vec<u8>,
    },
    /// A system exclusive message (status `0xF0` or `0xF7`).
    SysEx {
        /// The status byte.
        status: u8,
        /// Its data.
        data: Vec<u8>,
    },
}

/// One event at an absolute time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmiEvent {
    /// Absolute time in 120 Hz ticks: the sum of every delay before it.
    pub time_120hz: u32,
    /// What it does.
    pub kind: XmiEventKind,
}

/// One sequence: its instrument list and its decoded events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XmiSequence {
    /// The `TIMB` instrument list (empty when the sequence has none).
    pub timbres: Vec<Timbre>,
    /// Events in stream order, ending with [`XmiEventKind::EndOfTrack`] when
    /// the stream has one.
    pub events: Vec<XmiEvent>,
    /// Bytes of the event stream after the end-of-track event.
    pub trailing: usize,
}

impl XmiSequence {
    /// Time of the end-of-track event, or of the last event when there is
    /// none (0 for an empty stream).
    pub fn end_time_120hz(&self) -> u32 {
        self.events
            .iter()
            .find(|e| e.kind == XmiEventKind::EndOfTrack)
            .or(self.events.last())
            .map_or(0, |e| e.time_120hz)
    }

    /// Whether the stream ends with an end-of-track event.
    pub fn has_end(&self) -> bool {
        self.events
            .last()
            .is_some_and(|e| e.kind == XmiEventKind::EndOfTrack)
    }

    /// Length in seconds at 120 ticks per second.
    pub fn seconds(&self) -> f64 {
        f64::from(self.end_time_120hz()) / f64::from(TICKS_PER_SECOND)
    }
}

/// A parsed file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Xmi {
    /// Sequences in file order. The game only ever plays the first.
    pub sequences: Vec<XmiSequence>,
}

/// Parses an XMIDI file and decodes every sequence's events.
pub fn parse(data: &[u8]) -> Result<Xmi> {
    let (id, body, end) = chunk(data, 0, data.len())?;
    if id != *b"FORM" {
        return Err(format_error(FORMAT, "the file does not start with FORM"));
    }
    match form_type(data, body, end)? {
        b"XMID" => Ok(Xmi {
            sequences: vec![sequence(data, body + 4, end)?],
        }),
        b"XDIR" => {
            let next = padded(end);
            if next >= data.len() {
                return Err(format_error(
                    FORMAT,
                    "FORM XDIR is not followed by CAT XMID",
                ));
            }
            let (cat, cbody, cend) = chunk(data, next, data.len())?;
            if cat != *b"CAT " || form_type(data, cbody, cend)? != b"XMID" {
                return Err(format_error(
                    FORMAT,
                    "FORM XDIR is not followed by CAT XMID",
                ));
            }
            let mut sequences = Vec::new();
            let mut pos = cbody + 4;
            while pos < cend {
                if cend - pos < 8 {
                    // Padding at the end of the catalogue.
                    break;
                }
                let (id, fbody, fend) = chunk(data, pos, cend)?;
                if id == *b"FORM" && form_type(data, fbody, fend)? == b"XMID" {
                    if sequences.len() == MAX_SEQUENCES {
                        return Err(format_error(
                            FORMAT,
                            format!("more than {MAX_SEQUENCES} sequences"),
                        ));
                    }
                    sequences.push(sequence(data, fbody + 4, fend)?);
                }
                pos = padded(fend);
            }
            if sequences.is_empty() {
                return Err(format_error(FORMAT, "CAT XMID holds no sequence"));
            }
            Ok(Xmi { sequences })
        }
        other => Err(format_error(
            FORMAT,
            format!("unexpected form type {:?}", String::from_utf8_lossy(other)),
        )),
    }
}

/// Reads a chunk header at `pos`: its id, the start of its body and the end
/// of its body, which must lie within `limit`.
fn chunk(data: &[u8], pos: usize, limit: usize) -> Result<([u8; 4], usize, usize)> {
    let limit = limit.min(data.len());
    let header = pos
        .checked_add(8)
        .filter(|&h| h <= limit)
        .and_then(|h| data.get(pos..h))
        .ok_or_else(|| {
            format_error(
                FORMAT,
                format!("chunk header at {pos} runs past its container"),
            )
        })?;
    let id = [header[0], header[1], header[2], header[3]];
    let size = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
    let body = pos + 8;
    let end = body
        .checked_add(size)
        .filter(|&e| e <= limit)
        .ok_or_else(|| {
            format_error(
                FORMAT,
                format!(
                    "chunk {:?} at {pos} runs past its container",
                    String::from_utf8_lossy(&id)
                ),
            )
        })?;
    Ok((id, body, end))
}

fn padded(end: usize) -> usize {
    end.saturating_add(end & 1)
}

fn form_type(data: &[u8], body: usize, end: usize) -> Result<&[u8]> {
    if end < body + 4 {
        return Err(format_error(FORMAT, format!("form at {body} has no type")));
    }
    Ok(&data[body..body + 4])
}

/// Reads the chunks of one `FORM XMID` body.
fn sequence(data: &[u8], start: usize, end: usize) -> Result<XmiSequence> {
    let mut timbres = None;
    let mut evnt = None;
    let mut pos = start;
    while pos < end {
        if end - pos < 8 {
            break;
        }
        let (id, body, cend) = chunk(data, pos, end)?;
        match &id {
            b"TIMB" if timbres.is_none() => timbres = Some(timbre_list(&data[body..cend])),
            b"EVNT" if evnt.is_none() => evnt = Some(&data[body..cend]),
            _ => {}
        }
        pos = padded(cend);
    }
    let evnt =
        evnt.ok_or_else(|| format_error(FORMAT, format!("sequence at {start} has no EVNT chunk")))?;
    let (events, trailing) = decode_events(evnt)?;
    Ok(XmiSequence {
        timbres: timbres.unwrap_or_default(),
        events,
        trailing,
    })
}

fn timbre_list(body: &[u8]) -> Vec<Timbre> {
    let Some(count) = body
        .get(..2)
        .map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])))
    else {
        return Vec::new();
    };
    body[2..]
        .chunks_exact(2)
        .take(count)
        .map(|p| Timbre {
            patch: p[0],
            bank: p[1],
        })
        .collect()
}

/// Reads a standard MIDI variable-length quantity (at most four bytes).
fn read_vlq(data: &[u8], pos: &mut usize) -> Result<u32> {
    let start = *pos;
    let mut value = 0u32;
    for _ in 0..4 {
        let b = *data.get(*pos).ok_or_else(|| {
            format_error(
                FORMAT,
                format!("length at {start} runs past the event stream"),
            )
        })?;
        *pos += 1;
        value = (value << 7) | u32::from(b & 0x7F);
        if b & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(format_error(
        FORMAT,
        format!("length at {start} is longer than four bytes"),
    ))
}

fn data_bytes<const N: usize>(data: &[u8], pos: &mut usize, start: usize) -> Result<[u8; N]> {
    let bytes = data.get(*pos..*pos + N).ok_or_else(|| {
        format_error(
            FORMAT,
            format!("event at {start} runs past the event stream"),
        )
    })?;
    let mut out = [0u8; N];
    out.copy_from_slice(bytes);
    if out.iter().any(|&b| b >= 0x80) {
        return Err(format_error(
            FORMAT,
            format!("event at {start} has a data byte of 0x80 or more"),
        ));
    }
    *pos += N;
    Ok(out)
}

fn sized<'a>(data: &'a [u8], pos: &mut usize, start: usize) -> Result<&'a [u8]> {
    let len = read_vlq(data, pos)? as usize;
    let body = pos
        .checked_add(len)
        .and_then(|e| data.get(*pos..e))
        .ok_or_else(|| {
            format_error(
                FORMAT,
                format!("event at {start} runs past the event stream"),
            )
        })?;
    *pos += len;
    Ok(body)
}

/// Decodes one event stream into events at absolute times. Returns the events
/// and the number of bytes after the end-of-track event.
pub fn decode_events(data: &[u8]) -> Result<(Vec<XmiEvent>, usize)> {
    let mut events = Vec::new();
    let mut pos = 0;
    let mut time: u32 = 0;
    while pos < data.len() {
        let b = data[pos];
        if b < 0x80 {
            time = time
                .checked_add(u32::from(b))
                .ok_or_else(|| format_error(FORMAT, format!("time overflows at {pos}")))?;
            pos += 1;
            continue;
        }
        if events.len() == MAX_EVENTS {
            return Err(format_error(
                FORMAT,
                format!("more than {MAX_EVENTS} events"),
            ));
        }
        let start = pos;
        pos += 1;
        let channel = b & 0x0F;
        let kind = match b & 0xF0 {
            0x80 => {
                let [key, velocity] = data_bytes(data, &mut pos, start)?;
                XmiEventKind::NoteOff {
                    channel,
                    key,
                    velocity,
                }
            }
            0x90 => {
                let [key, velocity] = data_bytes(data, &mut pos, start)?;
                let duration = read_vlq(data, &mut pos)?;
                XmiEventKind::Note {
                    channel,
                    key,
                    velocity,
                    duration,
                }
            }
            0xA0 => {
                let [key, pressure] = data_bytes(data, &mut pos, start)?;
                XmiEventKind::KeyPressure {
                    channel,
                    key,
                    pressure,
                }
            }
            0xB0 => {
                let [controller, value] = data_bytes(data, &mut pos, start)?;
                XmiEventKind::Controller {
                    channel,
                    controller,
                    value,
                }
            }
            0xC0 => {
                let [program] = data_bytes(data, &mut pos, start)?;
                XmiEventKind::Program { channel, program }
            }
            0xD0 => {
                let [pressure] = data_bytes(data, &mut pos, start)?;
                XmiEventKind::ChannelPressure { channel, pressure }
            }
            0xE0 => {
                let [lo, hi] = data_bytes(data, &mut pos, start)?;
                XmiEventKind::PitchBend {
                    channel,
                    value: u16::from(lo) | (u16::from(hi) << 7),
                }
            }
            _ => match b {
                0xFF => {
                    let kind = *data.get(pos).ok_or_else(|| {
                        format_error(FORMAT, format!("meta event at {start} has no type"))
                    })?;
                    if kind >= 0x80 {
                        return Err(format_error(
                            FORMAT,
                            format!("meta event at {start} has type {kind:#04x}"),
                        ));
                    }
                    pos += 1;
                    let body = sized(data, &mut pos, start)?;
                    match (kind, body) {
                        (0x2F, _) => XmiEventKind::EndOfTrack,
                        (0x51, [a, b, c]) => XmiEventKind::Tempo {
                            micros_per_quarter: u32::from_be_bytes([0, *a, *b, *c]),
                        },
                        _ => XmiEventKind::Meta {
                            kind,
                            data: body.to_vec(),
                        },
                    }
                }
                0xF0 | 0xF7 => XmiEventKind::SysEx {
                    status: b,
                    data: sized(data, &mut pos, start)?.to_vec(),
                },
                _ => {
                    return Err(format_error(
                        FORMAT,
                        format!("system message {b:#04x} at {start} cannot be decoded"),
                    ));
                }
            },
        };
        let end = kind == XmiEventKind::EndOfTrack;
        events.push(XmiEvent {
            time_120hz: time,
            kind,
        });
        if end {
            return Ok((events, data.len() - pos));
        }
    }
    Ok((events, 0))
}

/// Writes a variable-length quantity; values above [`MAX_VLQ`] are written
/// as [`MAX_VLQ`].
pub fn encode_vlq(value: u32, out: &mut Vec<u8>) {
    let value = value.min(MAX_VLQ);
    let mut bytes = [0u8; 4];
    let mut n = 0;
    let mut v = value;
    loop {
        bytes[n] = (v & 0x7F) as u8;
        n += 1;
        v >>= 7;
        if v == 0 {
            break;
        }
    }
    for i in (0..n).rev() {
        out.push(if i > 0 { bytes[i] | 0x80 } else { bytes[i] });
    }
}

/// Encodes events (in time order) as an event stream: delays as runs of
/// `0x7F` bytes plus a remainder, then each event. For tests and tools.
pub fn encode_events(events: &[XmiEvent]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut now = 0u32;
    for e in events {
        let mut delay = e.time_120hz.saturating_sub(now);
        now = now.max(e.time_120hz);
        while delay > 0 {
            let step = delay.min(0x7F);
            out.push(step as u8);
            delay -= step;
        }
        match &e.kind {
            XmiEventKind::Note {
                channel,
                key,
                velocity,
                duration,
            } => {
                out.extend_from_slice(&[0x90 | channel & 0x0F, key & 0x7F, velocity & 0x7F]);
                encode_vlq(*duration, &mut out);
            }
            XmiEventKind::NoteOff {
                channel,
                key,
                velocity,
            } => out.extend_from_slice(&[0x80 | channel & 0x0F, key & 0x7F, velocity & 0x7F]),
            XmiEventKind::KeyPressure {
                channel,
                key,
                pressure,
            } => out.extend_from_slice(&[0xA0 | channel & 0x0F, key & 0x7F, pressure & 0x7F]),
            XmiEventKind::Controller {
                channel,
                controller,
                value,
            } => out.extend_from_slice(&[0xB0 | channel & 0x0F, controller & 0x7F, value & 0x7F]),
            XmiEventKind::Program { channel, program } => {
                out.extend_from_slice(&[0xC0 | channel & 0x0F, program & 0x7F])
            }
            XmiEventKind::ChannelPressure { channel, pressure } => {
                out.extend_from_slice(&[0xD0 | channel & 0x0F, pressure & 0x7F])
            }
            XmiEventKind::PitchBend { channel, value } => out.extend_from_slice(&[
                0xE0 | channel & 0x0F,
                (value & 0x7F) as u8,
                ((value >> 7) & 0x7F) as u8,
            ]),
            XmiEventKind::Tempo { micros_per_quarter } => {
                let b = micros_per_quarter.to_be_bytes();
                out.extend_from_slice(&[0xFF, 0x51, 3, b[1], b[2], b[3]]);
            }
            XmiEventKind::EndOfTrack => out.extend_from_slice(&[0xFF, 0x2F, 0]),
            XmiEventKind::Meta { kind, data } => {
                out.extend_from_slice(&[0xFF, kind & 0x7F]);
                encode_vlq(data.len() as u32, &mut out);
                out.extend_from_slice(data);
            }
            XmiEventKind::SysEx { status, data } => {
                out.push(if *status == 0xF7 { 0xF7 } else { 0xF0 });
                encode_vlq(data.len() as u32, &mut out);
                out.extend_from_slice(data);
            }
        }
    }
    out
}

fn iff(out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
    if body.len() & 1 == 1 {
        out.push(0);
    }
}

/// Builds an XMIDI file from (instrument list, event stream) pairs, with the
/// directory form and the catalogue. For tests and tools.
pub fn build(sequences: &[(Vec<Timbre>, Vec<u8>)]) -> Vec<u8> {
    let mut info = Vec::new();
    iff(&mut info, b"INFO", &(sequences.len() as u16).to_le_bytes());
    let mut xdir = b"XDIR".to_vec();
    xdir.extend_from_slice(&info);
    let mut cat = b"XMID".to_vec();
    for (timbres, evnt) in sequences {
        let mut timb = (timbres.len() as u16).to_le_bytes().to_vec();
        for t in timbres {
            timb.extend_from_slice(&[t.patch, t.bank]);
        }
        let mut form = b"XMID".to_vec();
        iff(&mut form, b"TIMB", &timb);
        iff(&mut form, b"EVNT", evnt);
        iff(&mut cat, b"FORM", &form);
    }
    let mut out = Vec::new();
    iff(&mut out, b"FORM", &xdir);
    iff(&mut out, b"CAT ", &cat);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vlq_round_trip() {
        for v in [0u32, 1, 0x7F, 0x80, 0x3FFF, 0x4000, 0x1F_FFFF, MAX_VLQ] {
            let mut out = Vec::new();
            encode_vlq(v, &mut out);
            let mut pos = 0;
            assert_eq!(read_vlq(&out, &mut pos).unwrap(), v);
            assert_eq!(pos, out.len());
        }
        let mut pos = 0;
        assert!(read_vlq(&[0x80, 0x80, 0x80, 0x80, 0], &mut pos).is_err());
        let mut pos = 0;
        assert!(read_vlq(&[0x81], &mut pos).is_err());
    }

    #[test]
    fn delays_add_up() {
        let (events, trailing) =
            decode_events(&[0x7F, 0x7F, 3, 0xC0, 5, 0xFF, 0x2F, 0, 0xAA]).unwrap();
        assert_eq!(events[0].time_120hz, 257);
        assert_eq!(trailing, 1);
        assert_eq!(events[1].kind, XmiEventKind::EndOfTrack);
    }
}
