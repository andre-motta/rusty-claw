//! XMIDI sequences to Standard MIDI Files.
//!
//! The game's music is XMIDI: MIDI events timed by a 120 Hz clock, where a
//! note carries its own length instead of a separate note-off. The
//! synthesizer plays Standard MIDI Files (SMF), so each track is converted
//! once, when it loads, by these rules:
//!
//! - Timebase: the file gets 60 ticks per quarter note and one tempo of
//!   500,000 microseconds per quarter note, so one SMF tick is exactly one
//!   120 Hz tick and every time is copied unchanged. The sequence's own
//!   tempo events are dropped: the 120 Hz times already include the tempo.
//! - Notes: each note becomes a note-on and a note-off (velocity 64) at its
//!   time plus its length. Notes of the same key on the same channel at the
//!   same time sound once, with the higher velocity and the longer length. A
//!   key struck again on its channel while it still sounds first ends the
//!   earlier note, right before the new note-on. A note-off past the end of
//!   the sequence moves to the end.
//! - Order within a tick: note-offs of notes that end naturally, then the
//!   other events in sequence order; a zero-length note's note-off follows
//!   its note-on.
//! - Controllers and programs are copied in order, except the sequencer
//!   extension controllers 110 to 120 and, with a General MIDI bank, the bank
//!   selects (controllers 0 and 32), which would pick a General MIDI bank's
//!   variation sounds. Explicit note-offs, meta events (other than the end)
//!   and system exclusive messages are dropped; so is the instrument list,
//!   since the synthesizer holds the whole bank in memory.
//! - The track ends exactly at the sequence's end, which is the musical loop
//!   length, not at the last note-off.
//! - Every pass of a loop starts like the first: a channel setting that the
//!   sequence changes after its first note without setting it before gets
//!   its General MIDI power-on value at time 0.

use std::collections::{BTreeSet, HashMap};

/// Ticks per quarter note of the converted file.
pub const DIVISION: u16 = 60;

/// Microseconds per quarter note of the converted file: with
/// [`DIVISION`], one tick is 1/120 s.
pub const TEMPO: u32 = 500_000;

/// Velocity written on note-offs.
pub const NOTE_OFF_VELOCITY: u8 = 64;

/// Which bank the music plays with, which decides the bank selects and the
/// arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BankMode {
    /// A General MIDI bank: the General MIDI arrangements, bank selects
    /// dropped.
    #[default]
    GeneralMidi,
    /// The game's own instrument bank supplied by the player: the other
    /// arrangements, bank selects kept. Best effort.
    Original,
}

/// An XMIDI event as the converter takes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmidiKind {
    /// A note sounding for `duration` ticks.
    Note {
        /// Channel 0..=15.
        channel: u8,
        /// Key.
        key: u8,
        /// Velocity.
        velocity: u8,
        /// Length in ticks.
        duration: u32,
    },
    /// Polyphonic key pressure.
    KeyPressure {
        /// Channel 0..=15.
        channel: u8,
        /// Key.
        key: u8,
        /// Pressure.
        pressure: u8,
    },
    /// A controller change.
    Controller {
        /// Channel 0..=15.
        channel: u8,
        /// Controller number.
        controller: u8,
        /// Value.
        value: u8,
    },
    /// A program change.
    Program {
        /// Channel 0..=15.
        channel: u8,
        /// Program.
        program: u8,
    },
    /// Channel pressure.
    ChannelPressure {
        /// Channel 0..=15.
        channel: u8,
        /// Pressure.
        pressure: u8,
    },
    /// Pitch bend, 0..=0x3FFF.
    PitchBend {
        /// Channel 0..=15.
        channel: u8,
        /// 14-bit value.
        value: u16,
    },
    /// The end of the sequence.
    EndOfTrack,
    /// Anything the conversion drops: explicit note-offs, tempo and other
    /// meta events, system exclusive messages.
    Other,
}

/// An event at an absolute time in 120 Hz ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XmidiEvent {
    /// Absolute time in 120 Hz ticks.
    pub tick: u32,
    /// The event.
    pub kind: XmidiKind,
}

impl XmidiEvent {
    /// An event at `tick`.
    pub fn new(tick: u32, kind: XmidiKind) -> XmidiEvent {
        XmidiEvent { tick, kind }
    }
}

/// A channel setting the loop prologue can restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Setting {
    Bend,
    Controller(u8),
    Program,
}

impl Setting {
    /// The General MIDI power-on value, for the settings that have one.
    fn default_message(self, channel: u8) -> Option<[u8; 3]> {
        let ch = channel & 0x0F;
        Some(match self {
            Setting::Program => [0xC0 | ch, 0, 0],
            Setting::Bend => [0xE0 | ch, 0x00, 0x40],
            Setting::Controller(cc) => {
                let value = match cc {
                    1 => 0,
                    7 => 100,
                    10 => 64,
                    11 => 127,
                    64 => 0,
                    91 => 40,
                    93 => 0,
                    _ => return None,
                };
                [0xB0 | ch, cc, value]
            }
        })
    }
}

/// Whether the conversion keeps a controller.
fn keeps_controller(controller: u8, mode: BankMode) -> bool {
    let extension = (110..=120).contains(&controller);
    let bank_select = matches!(controller, 0 | 32);
    !extension && (mode == BankMode::Original || !bank_select)
}

/// A copied channel event: its bytes, their length, and the channel setting
/// it changes.
type Copied = ([u8; 3], usize, Option<(u8, Setting)>);

/// The channel event of `kind` the conversion copies.
fn copied(kind: &XmidiKind, mode: BankMode) -> Option<Copied> {
    Some(match *kind {
        XmidiKind::KeyPressure {
            channel,
            key,
            pressure,
        } => (
            [0xA0 | channel & 0x0F, key & 0x7F, pressure & 0x7F],
            3,
            None,
        ),
        XmidiKind::Controller {
            channel,
            controller,
            value,
        } => {
            if !keeps_controller(controller, mode) {
                return None;
            }
            (
                [0xB0 | channel & 0x0F, controller & 0x7F, value & 0x7F],
                3,
                Some((channel & 0x0F, Setting::Controller(controller & 0x7F))),
            )
        }
        XmidiKind::Program { channel, program } => (
            [0xC0 | channel & 0x0F, program & 0x7F, 0],
            2,
            Some((channel & 0x0F, Setting::Program)),
        ),
        XmidiKind::ChannelPressure { channel, pressure } => {
            ([0xD0 | channel & 0x0F, pressure & 0x7F, 0], 2, None)
        }
        XmidiKind::PitchBend { channel, value } => (
            [
                0xE0 | channel & 0x0F,
                (value & 0x7F) as u8,
                ((value >> 7) & 0x7F) as u8,
            ],
            3,
            Some((channel & 0x0F, Setting::Bend)),
        ),
        XmidiKind::Note { .. } | XmidiKind::EndOfTrack | XmidiKind::Other => return None,
    })
}

/// The events up to and including the end of the sequence, and its end
/// time: the end-of-track event's time, or the last event's.
fn until_end(events: &[XmidiEvent]) -> (&[XmidiEvent], u32) {
    match events.iter().position(|e| e.kind == XmidiKind::EndOfTrack) {
        Some(i) => (&events[..i], events[i].tick),
        None => (events, events.iter().map(|e| e.tick).max().unwrap_or(0)),
    }
}

/// One planned note.
struct Planned {
    tick: u32,
    seq: usize,
    channel: u8,
    key: u8,
    velocity: u8,
    duration: u32,
}

/// Order of an output event within its tick: natural note-offs first (by
/// sequence position), then everything else by its position key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Order(u32, u8, i64);

/// Latest time the converted file can hold: the largest four-byte
/// variable-length quantity.
pub const MAX_TICK: u32 = 0x0FFF_FFFF;

/// Converts an XMIDI sequence to a Standard MIDI File (format 0, one track),
/// following the module rules.
pub fn xmidi_to_smf(events: &[XmidiEvent], mode: BankMode) -> Vec<u8> {
    let (events, end) = until_end(events);
    let end = end.min(MAX_TICK);
    let mut out: Vec<(Order, [u8; 3], usize)> = Vec::with_capacity(events.len() * 2 + 32);

    // Notes: merge same-tick duplicates, then end each one naturally, at a
    // re-strike or at the end of the sequence.
    let mut notes: Vec<Planned> = Vec::new();
    let mut at: HashMap<(u32, u8, u8), usize> = HashMap::new();
    for (seq, e) in events.iter().enumerate() {
        if let XmidiKind::Note {
            channel,
            key,
            velocity,
            duration,
        } = e.kind
        {
            let (channel, key, velocity) = (channel & 0x0F, key & 0x7F, velocity & 0x7F);
            let tick = e.tick.min(MAX_TICK);
            match at.get(&(tick, channel, key)) {
                Some(&i) => {
                    let n = &mut notes[i];
                    n.velocity = n.velocity.max(velocity);
                    n.duration = n.duration.max(duration);
                }
                None => {
                    at.insert((tick, channel, key), notes.len());
                    notes.push(Planned {
                        tick,
                        seq,
                        channel,
                        key,
                        velocity,
                        duration,
                    });
                }
            }
        }
    }
    let mut by_key: HashMap<(u8, u8), Vec<usize>> = HashMap::new();
    for (i, n) in notes.iter().enumerate() {
        by_key.entry((n.channel, n.key)).or_default().push(i);
    }
    let mut first_note = None;
    for list in by_key.values() {
        for (j, &i) in list.iter().enumerate() {
            let n = &notes[i];
            first_note = Some(first_note.map_or(n.tick, |f: u32| f.min(n.tick)));
            let pos = n.seq as i64 * 4;
            out.push((
                Order(n.tick, 1, pos),
                [0x90 | n.channel, n.key, n.velocity],
                3,
            ));
            let off = [0x80 | n.channel, n.key, NOTE_OFF_VELOCITY];
            let natural = n.tick.saturating_add(n.duration);
            let next = list.get(j + 1).map(|&k| &notes[k]);
            let order = match next {
                // Struck again while sounding: end right before the new note.
                Some(m) if m.tick < natural => Order(m.tick, 1, m.seq as i64 * 4 - 1),
                _ => {
                    let t = natural.min(end).max(n.tick);
                    if t == n.tick {
                        Order(t, 1, pos + 1)
                    } else {
                        Order(t, 0, n.seq as i64)
                    }
                }
            };
            out.push((order, off, 3));
        }
    }

    // Channel events, and the settings changed before and after the first
    // note for the loop prologue.
    let first = first_note.unwrap_or(end);
    let mut early: BTreeSet<(u8, Setting)> = BTreeSet::new();
    let mut late: BTreeSet<(u8, Setting)> = BTreeSet::new();
    for (seq, e) in events.iter().enumerate() {
        if let Some((bytes, len, setting)) = copied(&e.kind, mode) {
            let tick = e.tick.min(end);
            out.push((Order(tick, 1, seq as i64 * 4), bytes, len));
            if let Some(s) = setting {
                if e.tick <= first {
                    early.insert(s);
                } else {
                    late.insert(s);
                }
            }
        }
    }
    let prologue: Vec<[u8; 3]> = late
        .difference(&early)
        .filter_map(|&(ch, s)| s.default_message(ch))
        .collect();
    let n = prologue.len() as i64;
    for (i, msg) in prologue.into_iter().enumerate() {
        let len = if msg[0] & 0xF0 == 0xC0 { 2 } else { 3 };
        out.push((Order(0, 1, (i as i64 - n) * 4 - 4), msg, len));
    }

    out.sort_by_key(|e| e.0);
    encode(&out, end)
}

fn vlq(value: u32, out: &mut Vec<u8>) {
    let value = value.min(MAX_TICK);
    let mut started = false;
    for shift in [21u32, 14, 7] {
        let b = ((value >> shift) & 0x7F) as u8;
        if b != 0 || started {
            out.push(b | 0x80);
            started = true;
        }
    }
    out.push((value & 0x7F) as u8);
}

fn encode(events: &[(Order, [u8; 3], usize)], end: u32) -> Vec<u8> {
    let mut track = Vec::with_capacity(events.len() * 4 + 16);
    track.extend_from_slice(&[0x00, 0xFF, 0x51, 0x03]);
    track.extend_from_slice(&TEMPO.to_be_bytes()[1..]);
    let mut last = 0u32;
    for (order, bytes, len) in events {
        vlq(order.0 - last, &mut track);
        last = order.0;
        track.extend_from_slice(&bytes[..*len]);
    }
    vlq(end.saturating_sub(last), &mut track);
    track.extend_from_slice(&[0xFF, 0x2F, 0x00]);

    let mut out = Vec::with_capacity(track.len() + 22);
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&DIVISION.to_be_bytes());
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(track.len() as u32).to_be_bytes());
    out.extend_from_slice(&track);
    out
}

/// The (bank, program) pairs the melodic channels of a sequence select,
/// following bank select (controller 0) and program changes in order; the
/// percussion channel (9) is left out. For checking a bank's coverage.
pub fn presets_used(events: &[XmidiEvent]) -> BTreeSet<(u8, u8)> {
    let (events, _) = until_end(events);
    let mut bank = [0u8; 16];
    let mut program = [0u8; 16];
    let mut sounding = [false; 16];
    let mut used = BTreeSet::new();
    for e in events {
        match e.kind {
            XmidiKind::Controller {
                channel,
                controller: 0,
                value,
            } => bank[usize::from(channel & 0x0F)] = value,
            XmidiKind::Program {
                channel,
                program: p,
            } => {
                program[usize::from(channel & 0x0F)] = p;
                sounding[usize::from(channel & 0x0F)] = false;
            }
            XmidiKind::Note { channel, .. } => {
                let ch = usize::from(channel & 0x0F);
                if ch != 9 && !sounding[ch] {
                    used.insert((bank[ch], program[ch]));
                    sounding[ch] = true;
                }
            }
            _ => {}
        }
    }
    used
}

/// A decoded SMF event, from [`read_smf`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmfEvent {
    /// Absolute tick.
    pub tick: u32,
    /// Status and data bytes (meta events: `0xFF`, type, data).
    pub bytes: Vec<u8>,
}

/// Reads back a format 0 file written by [`xmidi_to_smf`]: its division and
/// its events. For tests and tools; returns `None` on anything else.
pub fn read_smf(data: &[u8]) -> Option<(u16, Vec<SmfEvent>)> {
    if data.get(..4)? != b"MThd" || data.get(14..18)? != b"MTrk" {
        return None;
    }
    let division = u16::from_be_bytes([data[12], data[13]]);
    let len = u32::from_be_bytes(data.get(18..22)?.try_into().ok()?) as usize;
    let track = data.get(22..22usize.checked_add(len)?)?;
    let mut pos = 0;
    let mut tick = 0u32;
    let mut out = Vec::new();
    while pos < track.len() {
        let mut delta = 0u32;
        loop {
            let b = *track.get(pos)?;
            pos += 1;
            delta = (delta << 7) | u32::from(b & 0x7F);
            if b & 0x80 == 0 {
                break;
            }
        }
        tick = tick.checked_add(delta)?;
        let status = *track.get(pos)?;
        let len = match status & 0xF0 {
            0xC0 | 0xD0 => 2,
            0x80..=0xE0 => 3,
            _ if status == 0xFF => 3 + usize::from(*track.get(pos + 2)?),
            _ => return None,
        };
        out.push(SmfEvent {
            tick,
            bytes: track.get(pos..pos + len)?.to_vec(),
        });
        pos += len;
    }
    Some((division, out))
}
