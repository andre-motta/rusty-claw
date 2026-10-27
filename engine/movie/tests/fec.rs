//! The movie archive on archives built here: the walk, every way an archive
//! can be malformed, the `SMK0` clips read as `SMK2`, the byte ranges the
//! clips are read through, and (with the `ffmpeg` feature) a small movie
//! played from inside an archive.

use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use claw_movie::PatchedRange;
use claw_movie::fec::{self, Fec, FecEntry, FecError, HEADER_LEN, RECORD_LEN, SMK0, SMK2};

const GAP_BASE: u16 = 0x2B8;

/// One clip of a test archive: its data, the gap before it and the size its
/// record states (the data's length unless set).
struct Clip<'a> {
    data: &'a [u8],
    gap: u16,
    size: Option<u32>,
    number: u32,
}

fn clip(data: &[u8]) -> Clip<'_> {
    Clip {
        data,
        gap: 0,
        size: None,
        number: 0,
    }
}

fn build(clips: &[Clip<'_>]) -> Vec<u8> {
    let mut out = b"FEC".to_vec();
    for v in [1u32, 1, clips.len() as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for (i, c) in clips.iter().enumerate() {
        let mut record = [0u8; RECORD_LEN as usize];
        let number = if c.number == 0 {
            i as u32 + 1
        } else {
            c.number
        };
        record[..4].copy_from_slice(&number.to_le_bytes());
        let g = GAP_BASE.wrapping_add(c.gap);
        record[0x106..0x108].copy_from_slice(&g.to_le_bytes());
        let size = c.size.unwrap_or(c.data.len() as u32);
        record[0x108..0x10C].copy_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&record);
        out.extend(std::iter::repeat_n(0xEEu8, usize::from(c.gap)));
        out.extend_from_slice(c.data);
    }
    out
}

fn parse(bytes: &[u8]) -> Result<Fec, FecError> {
    Fec::parse(Cursor::new(bytes))
}

fn malformed(bytes: &[u8]) -> String {
    match parse(bytes) {
        Err(FecError::Malformed(why)) => why,
        other => panic!("expected a malformed archive, got {other:?}"),
    }
}

#[test]
fn the_walk_finds_every_clip_after_its_record_and_gap() {
    let a = b"SMK0 first clip".as_slice();
    let b = b"SMK2 second".as_slice();
    let c = b"RIFF third".as_slice();
    let bytes = build(&[
        clip(a),
        Clip { gap: 7, ..clip(b) },
        Clip {
            number: 99,
            ..clip(c)
        },
    ]);
    let fec = parse(&bytes).unwrap();
    assert_eq!(fec.version(), (1, 1));
    let first = HEADER_LEN + RECORD_LEN;
    let second = first + a.len() as u64 + RECORD_LEN + 7;
    let third = second + b.len() as u64 + RECORD_LEN;
    assert_eq!(
        fec.entries(),
        &[
            FecEntry {
                number: 1,
                data_offset: first,
                len: a.len() as u64,
                signature: SMK0,
            },
            FecEntry {
                number: 2,
                data_offset: second,
                len: b.len() as u64,
                signature: SMK2,
            },
            FecEntry {
                number: 3,
                data_offset: third,
                len: c.len() as u64,
                signature: *b"RIFF",
            },
        ]
    );
    assert_eq!(first, 283, "the first clip starts right after its record");
    assert_eq!(third + c.len() as u64, bytes.len() as u64);
    // Clips are found by their place; the record's own number is not used.
    assert_eq!(fec.entry(3).unwrap().signature, *b"RIFF");
    assert_eq!(fec.entry(0), None);
    assert_eq!(fec.entry(4), None);
}

#[test]
fn empty_short_and_trailing_archives() {
    let fec = parse(&build(&[])).unwrap();
    assert!(fec.entries().is_empty());
    let fec = parse(&build(&[clip(b""), clip(b"SM")])).unwrap();
    assert_eq!(fec.entry(1).unwrap().len, 0);
    assert_eq!(fec.entry(1).unwrap().signature, [0; 4]);
    assert_eq!(fec.entry(2).unwrap().signature, *b"SM\0\0");
    let mut bytes = build(&[clip(b"SMK2")]);
    bytes.extend_from_slice(b"anything after the last clip");
    assert_eq!(parse(&bytes).unwrap().entries().len(), 1);
}

#[test]
fn a_wrong_header_is_malformed() {
    malformed(b"");
    malformed(b"FEC");
    malformed(&build(&[])[..14]);
    let mut bytes = build(&[clip(b"SMK2")]);
    bytes[0] = b'X';
    malformed(&bytes);
}

#[test]
fn a_truncated_record_is_malformed() {
    let bytes = build(&[clip(b"SMK2 data"), clip(b"SMK0 more")]);
    let second_record = (HEADER_LEN + RECORD_LEN) as usize + 9;
    for cut in [16, 200, second_record + 1, second_record + 0x10B] {
        let why = malformed(&bytes[..cut]);
        assert!(!why.is_empty());
    }
    // A count larger than the clips present.
    let mut bytes = build(&[clip(b"SMK2")]);
    bytes[11..15].copy_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 100]);
    malformed(&bytes);
}

#[test]
fn an_oversized_clip_is_malformed() {
    let bytes = build(&[Clip {
        size: Some(1000),
        ..clip(b"SMK2 short")
    }]);
    assert!(malformed(&bytes).contains("past the end"));
    let bytes = build(&[Clip {
        size: Some(u32::MAX),
        ..clip(b"SMK2")
    }]);
    malformed(&bytes);
    // One byte too many in the last clip.
    let data = b"SMK2 exact".as_slice();
    let bytes = build(&[Clip {
        size: Some(data.len() as u32 + 1),
        ..clip(data)
    }]);
    malformed(&bytes);
}

#[test]
fn a_bad_gap_is_malformed() {
    let mut bytes = build(&[clip(b"SMK2")]);
    let field = (HEADER_LEN + 0x106) as usize;
    bytes[field..field + 2].copy_from_slice(&(GAP_BASE - 1).to_le_bytes());
    assert!(malformed(&bytes).contains("gap"));
    bytes[field..field + 2].copy_from_slice(&0u16.to_le_bytes());
    malformed(&bytes);
    // A gap that runs past the end of the file.
    bytes[field..field + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    malformed(&bytes);
}

#[test]
fn a_bad_clip_spoils_the_whole_archive() {
    let mut bytes = build(&[clip(b"SMK0 good"), clip(b"SMK0 bad")]);
    let len = bytes.len();
    bytes.truncate(len - 2);
    malformed(&bytes);
}

fn write_archive(dir: &Path, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join("CLAW.FEC");
    std::fs::write(&path, bytes).unwrap();
    path
}

fn read_all(mut r: impl Read) -> Vec<u8> {
    let mut out = Vec::new();
    r.read_to_end(&mut out).unwrap();
    out
}

#[test]
fn smk0_clips_read_as_smk2_and_others_are_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let relabelled = b"SMK0\x40\x01\x00\x00rest of the stream".as_slice();
    let plain = b"SMK2\x40\x01\x00\x00another stream".as_slice();
    let other = b"RIFF....AVI ".as_slice();
    let path = write_archive(
        dir.path(),
        &build(&[
            clip(relabelled),
            Clip {
                gap: 3,
                ..clip(plain)
            },
            clip(other),
        ]),
    );
    let before = std::fs::read(&path).unwrap();

    let mut want = relabelled.to_vec();
    want[..4].copy_from_slice(b"SMK2");
    assert_eq!(read_all(fec::open_entry(&path, 1).unwrap()), want);
    assert_eq!(read_all(fec::open_entry(&path, 2).unwrap()), plain);
    assert_eq!(read_all(fec::open_entry(&path, 3).unwrap()), other);

    // Seeking inside a clip: its end, then back to its start.
    let mut r = fec::open_entry(&path, 1).unwrap();
    assert_eq!(r.seek(SeekFrom::End(0)).unwrap(), relabelled.len() as u64);
    assert_eq!(r.seek(SeekFrom::Start(0)).unwrap(), 0);
    let mut head = [0u8; 6];
    r.read_exact(&mut head).unwrap();
    assert_eq!(&head, b"SMK2\x40\x01");

    assert_eq!(std::fs::read(&path).unwrap(), before, "nothing is written");
}

#[test]
fn opening_a_missing_or_malformed_clip_fails() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_archive(dir.path(), &build(&[clip(b"SMK2")]));
    for n in [0, 2, u16::MAX] {
        let err = fec::open_entry(&path, n).err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound, "{err}");
    }
    let err = fec::open_entry(&dir.path().join("none.fec"), 1)
        .err()
        .unwrap();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    let bad = write_archive(dir.path(), b"FEC not an archive");
    let err = fec::open_entry(&bad, 1).err().unwrap();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData, "{err}");
    assert!(Fec::read(&bad).is_err());
}

/// A small fast generator, so the property test below repeats exactly.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Checks what a parse that succeeded promises: every clip lies inside the
/// bytes, in order, numbered from 1.
fn check_parsed(bytes: &[u8]) {
    if let Ok(fec) = parse(bytes) {
        let mut end = HEADER_LEN;
        for (i, e) in fec.entries().iter().enumerate() {
            assert_eq!(usize::from(e.number), i + 1);
            assert!(e.data_offset >= end + RECORD_LEN);
            end = e.data_offset + e.len;
            assert!(end <= bytes.len() as u64);
        }
    }
}

#[test]
fn parsing_never_panics_on_random_or_damaged_archives() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    // Random bytes, some of them starting like an archive.
    for _ in 0..4000 {
        let len = rng.below(700);
        let mut bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        if len >= 3 && rng.below(2) == 0 {
            bytes[..3].copy_from_slice(b"FEC");
        }
        check_parsed(&bytes);
    }
    // Valid archives with damage: flipped bytes, overwritten fields, cuts.
    let data: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
    let valid = build(&[
        clip(&data[..40]),
        Clip {
            gap: 5,
            ..clip(&data[..120])
        },
        clip(&data[..3]),
        clip(&data),
    ]);
    let fields: Vec<usize> = {
        let mut at = HEADER_LEN as usize;
        let mut v = vec![3, 7, 11];
        for len in [40usize, 120 + 5, 3, 300] {
            v.extend([at, at + 0x106, at + 0x108]);
            at += RECORD_LEN as usize + len;
        }
        v
    };
    for _ in 0..20000 {
        let mut bytes = valid.clone();
        for _ in 0..1 + rng.below(4) {
            match rng.below(4) {
                0 => {
                    let i = rng.below(bytes.len());
                    bytes[i] ^= 1 << rng.below(8);
                }
                1 => {
                    let at = fields[rng.below(fields.len())];
                    let word = rng.next().to_le_bytes();
                    let n = [1, 2, 4][rng.below(3)];
                    let n = n.min(bytes.len().saturating_sub(at));
                    if n > 0 {
                        bytes[at..at + n].copy_from_slice(&word[..n]);
                    }
                }
                2 => {
                    let at = rng.below(bytes.len() + 1);
                    bytes.truncate(at);
                }
                _ => {
                    let extra = rng.below(64);
                    bytes.extend((0..extra).map(|_| rng.next() as u8));
                }
            }
            if bytes.is_empty() {
                break;
            }
        }
        check_parsed(&bytes);
    }
}

#[test]
fn patched_ranges_read_and_seek_within_their_bounds() {
    let data: Vec<u8> = (0..=255u8).collect();
    let mut r = PatchedRange::new(Cursor::new(data.clone()), 100, 50, Some(*b"SMK2")).unwrap();
    assert_eq!(r.len(), 50);
    assert!(!r.is_empty());

    // A read larger than the range stops at its end.
    let mut big = vec![0u8; 500];
    let mut got = 0;
    loop {
        let n = r.read(&mut big[got..]).unwrap();
        if n == 0 {
            break;
        }
        got += n;
    }
    assert_eq!(got, 50);
    assert_eq!(&big[..4], b"SMK2");
    assert_eq!(&big[4..50], &data[104..150]);
    assert_eq!(r.read(&mut big).unwrap(), 0, "nothing after the end");

    // Seeks are relative to the range.
    assert_eq!(r.seek(SeekFrom::End(0)).unwrap(), 50);
    assert_eq!(r.seek(SeekFrom::End(-50)).unwrap(), 0);
    assert_eq!(r.seek(SeekFrom::Current(3)).unwrap(), 3);
    let mut two = [0u8; 2];
    r.read_exact(&mut two).unwrap();
    assert_eq!(two, [b'2', data[104]], "the patch ends at byte 4");
    assert_eq!(r.stream_position().unwrap(), 5);
    assert!(r.seek(SeekFrom::End(-51)).is_err());
    assert!(r.seek(SeekFrom::Current(-6)).is_err());
    assert_eq!(
        r.stream_position().unwrap(),
        5,
        "a failed seek moves nothing"
    );

    // Past the end: allowed, and reads nothing.
    assert_eq!(r.seek(SeekFrom::Start(80)).unwrap(), 80);
    assert_eq!(r.read(&mut two).unwrap(), 0);
    assert_eq!(r.seek(SeekFrom::End(10)).unwrap(), 60);
    assert_eq!(r.read(&mut two).unwrap(), 0);

    // Every start and size of read sees the same bytes.
    let mut want = data[100..150].to_vec();
    want[..4].copy_from_slice(b"SMK2");
    for start in 0..8u64 {
        for size in 1..7usize {
            r.seek(SeekFrom::Start(start)).unwrap();
            let mut seen = Vec::new();
            let mut buf = vec![0u8; size];
            loop {
                let n = r.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                seen.extend_from_slice(&buf[..n]);
            }
            assert_eq!(seen, want[start as usize..], "start {start} size {size}");
        }
    }
}

#[test]
fn a_range_without_a_patch_or_past_its_source() {
    let data: Vec<u8> = (0..20u8).collect();
    let r = PatchedRange::new(Cursor::new(data.clone()), 5, 10, None).unwrap();
    assert_eq!(read_all(r), &data[5..15]);
    // The source ends before the range does: reading stops there.
    let r = PatchedRange::new(Cursor::new(data.clone()), 15, 10, Some(*b"ABCD")).unwrap();
    assert_eq!(read_all(r), [b'A', b'B', b'C', b'D', 19]);
    let r = PatchedRange::new(Cursor::new(data.clone()), 0, 0, Some(*b"ABCD")).unwrap();
    assert!(r.is_empty());
    assert!(read_all(r).is_empty());
    assert!(PatchedRange::new(Cursor::new(data), u64::MAX, 2, None).is_err());
}

#[cfg(feature = "ffmpeg")]
mod playing {
    //! A small movie made here (a 32x24 raw BGR picture at 10 pictures a
    //! second, each with its own colour) plays the same from its own file
    //! and from inside an archive.

    use std::io::Cursor;
    use std::time::Duration;

    use claw_movie::{Player, Source, VideoFrame};

    use super::{build, clip, fec};

    const W: u32 = 32;
    const H: u32 = 24;
    const FRAMES: u32 = 12;

    fn chunk(out: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) -> usize {
        let at = out.len();
        out.extend_from_slice(id);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        if data.len() % 2 == 1 {
            out.push(0);
        }
        at
    }

    fn list(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        let mut data = kind.to_vec();
        data.extend_from_slice(body);
        chunk(out, b"LIST", &data);
    }

    fn le(v: &mut Vec<u8>, words: &[u32]) {
        for w in words {
            v.extend_from_slice(&w.to_le_bytes());
        }
    }

    /// The test movie as AVI bytes.
    fn movie() -> Vec<u8> {
        let image = (W * H * 3) as usize;
        let mut avih = Vec::new();
        le(
            &mut avih,
            &[
                100_000,
                0,
                0,
                0x10,
                FRAMES,
                0,
                1,
                image as u32,
                W,
                H,
                0,
                0,
                0,
                0,
            ],
        );
        let mut strh = b"vids".to_vec();
        le(
            &mut strh,
            &[
                0,
                0,
                0,
                0,
                1,
                10,
                0,
                FRAMES,
                image as u32,
                u32::MAX,
                0,
                0,
                0,
            ],
        );
        let mut strf = Vec::new();
        le(&mut strf, &[40, W, H]);
        strf.extend_from_slice(&1u16.to_le_bytes());
        strf.extend_from_slice(&24u16.to_le_bytes());
        le(&mut strf, &[0, image as u32, 0, 0, 0, 0]);
        let mut strl = Vec::new();
        chunk(&mut strl, b"strh", &strh);
        chunk(&mut strl, b"strf", &strf);
        let mut hdrl = Vec::new();
        chunk(&mut hdrl, b"avih", &avih);
        list(&mut hdrl, b"strl", &strl);

        let mut movi = b"movi".to_vec();
        let mut idx = Vec::new();
        for i in 0..FRAMES {
            let (b, g, r) = ((i * 20) as u8, (255 - i * 15) as u8, (i * 7 + 40) as u8);
            let mut pixels = Vec::with_capacity(image);
            for y in 0..H {
                for x in 0..W {
                    pixels.extend_from_slice(&[b ^ x as u8, g, r ^ y as u8]);
                }
            }
            let at = chunk(&mut movi, b"00db", &pixels);
            idx.extend_from_slice(b"00db");
            le(&mut idx, &[0x10, at as u32, pixels.len() as u32]);
        }
        let mut body = b"AVI ".to_vec();
        list(&mut body, b"hdrl", &hdrl);
        list(&mut body, b"movi", &movi[4..]);
        chunk(&mut body, b"idx1", &idx);
        let mut out = Vec::new();
        chunk(&mut out, b"RIFF", &body);
        out
    }

    fn pictures(mut p: Player) -> Vec<VideoFrame> {
        assert_eq!(p.size(), (W, H));
        let mut out = Vec::new();
        while let Some(f) = p.decoded_frame(Duration::from_secs(10)) {
            out.push(f);
        }
        assert_eq!(p.error(), None);
        out
    }

    #[test]
    fn a_movie_inside_an_archive_plays_as_its_own_file() {
        let avi = movie();
        let plain = pictures(
            Player::open_reader(Box::new(Cursor::new(avi.clone())), "a.avi", 48000).unwrap(),
        );
        assert_eq!(plain.len(), FRAMES as usize);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("CLAW.FEC");
        std::fs::write(&path, build(&[clip(b"SMK0 not this one"), clip(&avi)])).unwrap();
        let wrapped = pictures(
            Player::open_reader(fec::open_entry(&path, 2).unwrap(), "CLAW2.smk", 48000).unwrap(),
        );
        assert!(wrapped == plain, "the pictures differ");

        let source = Source::Archive {
            path: path.clone(),
            entry: 2,
        };
        let through_source = pictures(Player::open(&source, 48000).unwrap());
        assert!(through_source == plain, "the pictures differ");
        assert!(Player::open(&Source::Archive { path, entry: 3 }, 48000).is_err());
    }
}
