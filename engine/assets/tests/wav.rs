//! WAV sounds: the chunk walk, sample conversion and length, on synthetic
//! files built here, plus property tests that the reader never panics.
//! `install_wavs_parse` reads every sound of your own install when
//! `CLAW_INSTALL` is set.

use std::path::PathBuf;

use claw_assets::name::TYPE_WAV;
use claw_assets::wav::{self, build};
use claw_assets::{DirNode, Install};
use proptest::prelude::*;

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() & 1 == 1 {
        out.push(0);
    }
    out
}

fn fmt_body(channels: u16, rate: u32, bits: u16) -> Vec<u8> {
    let block = channels * (bits / 8);
    let mut b = Vec::new();
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    b.extend_from_slice(&block.to_le_bytes());
    b.extend_from_slice(&bits.to_le_bytes());
    b
}

fn riff(chunks: &[Vec<u8>], riff_size: Option<u32>) -> Vec<u8> {
    let body: Vec<u8> = chunks.concat();
    let mut out = b"RIFF".to_vec();
    let size = riff_size.unwrap_or(4 + body.len() as u32);
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(&body);
    out
}

#[test]
fn reads_the_samples_and_their_length() {
    let file = build(1, 22050, 8, &[128; 22050]);
    let w = wav::parse(&file).unwrap();
    assert_eq!(w.samples.len(), 22050);
    assert!(w.samples.iter().all(|&s| s == 0));
    assert_eq!(w.duration_ms(), 1000);
    assert_eq!(wav::duration_ms(&file).unwrap(), 1000);

    let stereo16 = build(2, 11025, 16, &[0; 11025]);
    assert_eq!(wav::duration_ms(&stereo16).unwrap(), 250);
    assert_eq!(wav::parse(&stereo16).unwrap().duration_ms(), 249);
}

#[test]
fn skips_other_chunks_and_takes_the_last_format() {
    let file = riff(
        &[
            chunk(b"fmt ", &fmt_body(1, 8000, 16)),
            chunk(b"fact", &[1, 2, 3]),
            chunk(b"fmt ", &fmt_body(1, 11025, 8)),
            chunk(b"data", &[0, 255]),
            chunk(b"LIST", &[9; 6]),
        ],
        None,
    );
    let w = wav::parse(&file).unwrap();
    assert_eq!((w.rate, w.bits), (11025, 8));
    assert_eq!(w.samples, vec![-128 << 8, 127 << 8]);
}

#[test]
fn a_riff_size_past_the_end_or_short_of_a_pad_byte_is_accepted() {
    let mut file = build(1, 22050, 8, &[1, 2, 3]);
    assert_eq!(file.pop(), Some(0), "pad byte");
    assert_eq!(wav::parse(&file).unwrap().samples.len(), 3);
    let big = riff(
        &[chunk(b"fmt ", &fmt_body(1, 22050, 8)), chunk(b"data", &[7])],
        Some(4000),
    );
    assert_eq!(wav::parse(&big).unwrap().samples.len(), 1);
}

#[test]
fn rejects_what_cannot_be_played() {
    let fmt = chunk(b"fmt ", &fmt_body(1, 22050, 8));
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", Vec::new()),
        ("not riff", b"RIFX\0\0\0\0WAVE".to_vec()),
        ("no data", riff(std::slice::from_ref(&fmt), None)),
        (
            "data first",
            riff(&[chunk(b"data", &[0]), fmt.clone()], None),
        ),
        (
            "short fmt",
            riff(&[chunk(b"fmt ", &[1, 0, 1, 0]), chunk(b"data", &[0])], None),
        ),
        ("data past the end", {
            let mut f = riff(&[fmt.clone(), chunk(b"data", &[0; 8])], None);
            f.truncate(f.len() - 4);
            f
        }),
        ("not pcm", {
            let mut body = fmt_body(1, 22050, 8);
            body[0] = 2;
            riff(&[chunk(b"fmt ", &body), chunk(b"data", &[0])], None)
        }),
        (
            "24 bits",
            riff(
                &[
                    chunk(b"fmt ", &fmt_body(1, 22050, 24)),
                    chunk(b"data", &[0]),
                ],
                None,
            ),
        ),
        (
            "no channels",
            riff(
                &[chunk(b"fmt ", &fmt_body(0, 22050, 8)), chunk(b"data", &[0])],
                None,
            ),
        ),
    ];
    for (name, file) in cases {
        assert!(wav::parse(&file).is_err(), "{name}");
        assert!(wav::duration_ms(&file).is_err(), "{name}");
    }
}

fn mutate(mut data: Vec<u8>, edits: &[(usize, u8)]) -> Vec<u8> {
    for &(pos, value) in edits {
        if !data.is_empty() {
            let i = pos % data.len();
            data[i] = value;
        }
    }
    data
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..256)) {
        let _ = wav::parse(&data);
        let _ = wav::duration_ms(&data);
    }

    #[test]
    fn corrupted_files_never_panic(
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..8),
        cut in any::<usize>(),
    ) {
        let base = riff(
            &[
                chunk(b"fmt ", &fmt_body(2, 22050, 16)),
                chunk(b"fact", &[0; 4]),
                chunk(b"data", &[3; 40]),
            ],
            None,
        );
        let mut data = mutate(base, &edits);
        data.truncate(cut % (data.len() + 1));
        if let Ok(w) = wav::parse(&data) {
            prop_assert!(w.samples.len() <= data.len());
        }
        let _ = wav::duration_ms(&data);
    }
}

fn sounds(dir: &DirNode, out: &mut Vec<claw_assets::FileId>) {
    for sub in dir.dirs() {
        sounds(sub, out);
    }
    out.extend(dir.files_of_type(TYPE_WAV));
}

#[test]
fn install_wavs_parse() {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return;
    };
    let store = Install::open(PathBuf::from(root))
        .and_then(|i| i.store())
        .expect("mounting the install");
    let mut ids = Vec::new();
    sounds(store.root(), &mut ids);
    assert!(!ids.is_empty(), "the install has sounds");
    for id in ids {
        let path = store.file(id).path().to_string();
        let data = store.read(id).expect("readable");
        let w = wav::parse(&data).unwrap_or_else(|e| panic!("{path}: {e}"));
        let ms = wav::duration_ms(&data).unwrap();
        assert!(
            w.samples.len() >= usize::from(w.channels) || ms == 0,
            "{path}"
        );
        assert!(ms.abs_diff(w.duration_ms()) <= 1, "{path}");
    }
}
