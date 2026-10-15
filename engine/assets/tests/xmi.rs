//! XMIDI music: the container and event decoding on synthetic files built
//! here, plus property tests that the reader never panics.
//! `install_xmis_parse` reads every music file of your own install when
//! `CLAW_INSTALL` is set.

use std::path::PathBuf;

use claw_assets::name::TYPE_XMI;
use claw_assets::xmi::{self, Timbre, XmiEvent, XmiEventKind};
use claw_assets::{DirNode, FileId, Install};
use proptest::prelude::*;

fn ev(time_120hz: u32, kind: XmiEventKind) -> XmiEvent {
    XmiEvent { time_120hz, kind }
}

fn note(time: u32, channel: u8, key: u8, velocity: u8, duration: u32) -> XmiEvent {
    ev(
        time,
        XmiEventKind::Note {
            channel,
            key,
            velocity,
            duration,
        },
    )
}

fn sample_events() -> Vec<XmiEvent> {
    vec![
        ev(
            0,
            XmiEventKind::Tempo {
                micros_per_quarter: 600_000,
            },
        ),
        ev(
            0,
            XmiEventKind::Program {
                channel: 0,
                program: 48,
            },
        ),
        ev(
            0,
            XmiEventKind::Controller {
                channel: 0,
                controller: 7,
                value: 100,
            },
        ),
        note(0, 0, 60, 100, 30),
        note(0, 9, 36, 127, 0),
        ev(
            12,
            XmiEventKind::PitchBend {
                channel: 0,
                value: 0x2345,
            },
        ),
        note(300, 1, 64, 90, 4180),
        ev(
            301,
            XmiEventKind::Meta {
                kind: 0x58,
                data: vec![4, 2, 24, 8],
            },
        ),
        ev(
            302,
            XmiEventKind::SysEx {
                status: 0xF0,
                data: vec![0x41, 0x10, 0xF7],
            },
        ),
        ev(4480, XmiEventKind::EndOfTrack),
    ]
}

fn timbres() -> Vec<Timbre> {
    vec![
        Timbre { patch: 48, bank: 0 },
        Timbre {
            patch: 36,
            bank: 127,
        },
    ]
}

#[test]
fn events_round_trip_through_the_container() {
    let events = sample_events();
    let file = xmi::build(&[(timbres(), xmi::encode_events(&events))]);
    let x = xmi::parse(&file).unwrap();
    assert_eq!(x.sequences.len(), 1);
    let s = &x.sequences[0];
    assert_eq!(s.timbres, timbres());
    assert_eq!(s.events, events);
    assert_eq!(s.end_time_120hz(), 4480);
    assert!(s.has_end());
    assert_eq!(s.trailing, 0);
    assert!((s.seconds() - 4480.0 / 120.0).abs() < 1e-9);
}

#[test]
fn long_delays_and_long_durations() {
    // 265 ticks is two 0x7F bytes plus 11; a long note needs a two-byte length.
    let events = vec![
        note(265, 2, 70, 80, 4180),
        ev(4500, XmiEventKind::EndOfTrack),
    ];
    let stream = xmi::encode_events(&events);
    assert_eq!(&stream[..3], &[0x7F, 0x7F, 11]);
    let (decoded, trailing) = xmi::decode_events(&stream).unwrap();
    assert_eq!(decoded, events);
    assert_eq!(trailing, 0);
}

#[test]
fn bytes_after_the_end_are_counted_not_read() {
    let mut stream = xmi::encode_events(&[ev(5, XmiEventKind::EndOfTrack)]);
    stream.extend_from_slice(&[0xF4, 0x00]);
    let (events, trailing) = xmi::decode_events(&stream).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(trailing, 2);
}

#[test]
fn a_stream_without_an_end_ends_at_its_last_event() {
    let stream = xmi::encode_events(&[note(0, 0, 60, 100, 10), note(40, 0, 62, 100, 10)]);
    let x = xmi::parse(&xmi::build(&[(vec![], stream)])).unwrap();
    assert!(!x.sequences[0].has_end());
    assert_eq!(x.sequences[0].end_time_120hz(), 40);
}

#[test]
fn several_sequences_and_a_bare_form() {
    let a = xmi::encode_events(&[note(0, 0, 60, 100, 10), ev(10, XmiEventKind::EndOfTrack)]);
    let b = xmi::encode_events(&[ev(99, XmiEventKind::EndOfTrack)]);
    let x = xmi::parse(&xmi::build(&[(vec![], a.clone()), (timbres(), b)])).unwrap();
    assert_eq!(x.sequences.len(), 2);
    assert_eq!(x.sequences[1].end_time_120hz(), 99);

    // FORM XMID on its own, with an unknown chunk before the events.
    let mut form = b"XMID".to_vec();
    form.extend_from_slice(b"RBRN");
    form.extend_from_slice(&3u32.to_be_bytes());
    form.extend_from_slice(&[1, 2, 3, 0]);
    form.extend_from_slice(b"EVNT");
    form.extend_from_slice(&(a.len() as u32).to_be_bytes());
    form.extend_from_slice(&a);
    if a.len() % 2 == 1 {
        form.push(0);
    }
    let mut file = b"FORM".to_vec();
    file.extend_from_slice(&(form.len() as u32).to_be_bytes());
    file.extend_from_slice(&form);
    let x = xmi::parse(&file).unwrap();
    assert_eq!(x.sequences.len(), 1);
    assert!(x.sequences[0].timbres.is_empty());
    assert_eq!(x.sequences[0].end_time_120hz(), 10);
}

#[test]
fn tolerated_shapes() {
    let stream = xmi::encode_events(&[ev(1, XmiEventKind::EndOfTrack)]);
    let mut file = xmi::build(&[(timbres(), stream)]);
    // INFO says 3 sequences; the catalogue holds one.
    let info = file.windows(4).position(|w| w == b"INFO").unwrap();
    file[info + 8] = 3;
    assert_eq!(xmi::parse(&file).unwrap().sequences.len(), 1);
    // A TIMB count larger than its entries keeps the entries present.
    let timb = file.windows(4).position(|w| w == b"TIMB").unwrap();
    file[timb + 8] = 9;
    assert_eq!(xmi::parse(&file).unwrap().sequences[0].timbres, timbres());
}

#[test]
fn malformed_files_are_errors() {
    let good = xmi::build(&[(
        timbres(),
        xmi::encode_events(&[note(0, 0, 60, 100, 10), ev(10, XmiEventKind::EndOfTrack)]),
    )]);
    assert!(xmi::parse(&good).is_ok());
    assert!(xmi::parse(b"").is_err());
    assert!(xmi::parse(b"RIFF\0\0\0\x04WAVE").is_err());
    assert!(xmi::parse(&good[..good.len() - 3]).is_err());
    // No catalogue after the directory.
    let xdir_end = 8 + u32::from_be_bytes(good[4..8].try_into().unwrap()) as usize;
    assert!(xmi::parse(&good[..xdir_end]).is_err());
    // A sequence without events.
    let mut no_evnt = good.clone();
    let evnt = no_evnt.windows(4).position(|w| w == b"EVNT").unwrap();
    no_evnt[evnt..evnt + 4].copy_from_slice(b"JUNK");
    assert!(xmi::parse(&no_evnt).is_err());

    // Event streams.
    for bad in [
        &[0x90, 60][..],                                 // note without velocity
        &[0x90, 60, 100][..],                            // note without length
        &[0x90, 60, 0x80, 1][..],                        // data byte with the top bit
        &[0xB0, 7][..],                                  // controller without value
        &[0xFF][..],                                     // meta without type
        &[0xFF, 0x80, 0][..],                            // meta type with the top bit
        &[0xFF, 0x51, 5, 1][..],                         // meta longer than the stream
        &[0xF0, 4, 1][..],                               // sysex longer than the stream
        &[0xF4][..],                                     // undecodable system message
        &[0x90, 1, 1, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F][..], // length over four bytes
    ] {
        assert!(xmi::decode_events(bad).is_err(), "{bad:x?}");
    }
}

#[test]
fn many_delay_bytes_add_up() {
    let mut stream = vec![0x7F; 1000];
    stream.extend_from_slice(&[0xFF, 0x2F, 0]);
    let (events, _) = xmi::decode_events(&stream).unwrap();
    assert_eq!(events[0].time_120hz, 127_000);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..256)) {
        let _ = xmi::parse(&data);
        let _ = xmi::decode_events(&data);
    }

    #[test]
    fn corrupted_files_never_panic(
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..8),
        cut in any::<usize>(),
    ) {
        let mut data = xmi::build(&[
            (timbres(), xmi::encode_events(&sample_events())),
            (vec![], xmi::encode_events(&[note(3, 1, 2, 3, 4)])),
        ]);
        for &(pos, value) in &edits {
            let i = pos % data.len();
            data[i] = value;
        }
        data.truncate(cut % (data.len() + 1));
        if let Ok(x) = xmi::parse(&data) {
            for s in &x.sequences {
                prop_assert!(s.events.len() <= data.len());
            }
        }
    }

    #[test]
    fn encoded_events_decode_back(
        raw in proptest::collection::vec((0u32..400, 0u8..16, 0u8..128, 0u8..128, 0u32..5000), 0..64),
    ) {
        let mut time = 0;
        let mut events: Vec<XmiEvent> = raw
            .iter()
            .map(|&(dt, ch, a, b, d)| {
                time += dt;
                if d % 3 == 0 {
                    ev(time, XmiEventKind::Controller { channel: ch, controller: a, value: b })
                } else {
                    note(time, ch, a, b, d)
                }
            })
            .collect();
        events.push(ev(time + 1, XmiEventKind::EndOfTrack));
        let (decoded, trailing) = xmi::decode_events(&xmi::encode_events(&events)).unwrap();
        prop_assert_eq!(decoded, events);
        prop_assert_eq!(trailing, 0);
    }
}

fn music(dir: &DirNode, out: &mut Vec<FileId>) {
    for sub in dir.dirs() {
        music(sub, out);
    }
    out.extend(dir.files_of_type(TYPE_XMI));
}

#[test]
fn install_xmis_parse() {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return;
    };
    let store = Install::open(PathBuf::from(root))
        .and_then(|i| i.store())
        .expect("mounting the install");
    let mut ids = Vec::new();
    music(store.root(), &mut ids);
    assert!(!ids.is_empty(), "the install has music");
    for id in ids {
        let path = store.file(id).path().to_string();
        let data = store.read(id).expect("readable");
        let x = xmi::parse(&data).unwrap_or_else(|e| panic!("{path}: {e}"));
        let s = &x.sequences[0];
        assert!(s.has_end(), "{path}: no end of track");
        assert!(
            s.events.iter().all(|e| e.time_120hz <= s.end_time_120hz()),
            "{path}"
        );
    }
}
