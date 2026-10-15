//! PCX pictures: decoding synthetic files, refusing malformed ones, and
//! never panicking on arbitrary bytes. With `CLAW_INSTALL` set, every PCX
//! file of your own install also decodes.

use std::path::PathBuf;

use claw_assets::name::TYPE_PCX;
use claw_assets::pcx::{self, HEADER_SIZE, IndexedImage, MAX_DIMENSION, PALETTE_MARKER};
use claw_assets::{DirNode, Install, Palette};
use proptest::prelude::*;

fn palette() -> Palette {
    let mut p = Palette::default();
    for (i, c) in p.0.iter_mut().enumerate() {
        *c = [i as u8, (255 - i) as u8, (i * 7 % 256) as u8];
    }
    p
}

fn picture(w: u32, h: u32) -> Vec<u8> {
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            // Long flat stretches (runs) and indices above 0xC0 (escaped).
            if y % 3 == 0 {
                (x / 9) as u8
            } else {
                (0xB0 + (x + y) % 40) as u8
            }
        })
        .collect()
}

#[test]
fn decodes_pixels_and_palette() {
    let (w, h) = (70, 9);
    let pixels = picture(w, h);
    let pal = palette();
    let data = pcx::build(w as u16, h as u16, &pcx::encode(&pixels), Some(&pal));
    let (img, got) = pcx::decode(&data).unwrap();
    assert_eq!(
        img,
        IndexedImage {
            width: w,
            height: h,
            pixels
        }
    );
    assert_eq!(got, Some(pal));
}

#[test]
fn runs_carry_across_row_ends() {
    // 4x3: a run of 6 fills row 0 and half of row 1, then a run of 6 the
    // rest.
    let data = pcx::build(4, 3, &[0xC6, 5, 0xC6, 8], None);
    let (img, _) = pcx::decode(&data).unwrap();
    assert_eq!(img.pixels, vec![5, 5, 5, 5, 5, 5, 8, 8, 8, 8, 8, 8]);
    // A run longer than what is left of the image is cut at its end.
    let (img, _) = pcx::decode(&pcx::build(2, 1, &[0xFF, 3], None)).unwrap();
    assert_eq!(img.pixels, vec![3, 3]);
    // The encoder makes runs that cross rows; they decode the same.
    let pixels = vec![9u8; 4 * 3];
    let stream = pcx::encode(&pixels);
    assert_eq!(stream, vec![0xCC, 9]);
    assert_eq!(
        pcx::decode(&pcx::build(4, 3, &stream, None))
            .unwrap()
            .0
            .pixels,
        pixels
    );
}

#[test]
fn a_zero_length_run_still_takes_its_value_byte() {
    let data = pcx::build(3, 1, &[0xC0, 77, 1, 2, 3], None);
    assert_eq!(pcx::decode(&data).unwrap().0.pixels, vec![1, 2, 3]);
}

#[test]
fn size_comes_from_both_corners() {
    let mut data = pcx::build(1, 1, &[0xC6, 4], None);
    // x 10..12, y 5..6: 3x2.
    data[0x04..0x06].copy_from_slice(&10i16.to_le_bytes());
    data[0x06..0x08].copy_from_slice(&5i16.to_le_bytes());
    data[0x08..0x0A].copy_from_slice(&12i16.to_le_bytes());
    data[0x0A..0x0C].copy_from_slice(&6i16.to_le_bytes());
    let (img, _) = pcx::decode(&data).unwrap();
    assert_eq!((img.width, img.height), (3, 2));
    // The bytes-per-line field is not used.
    data[0x42..0x44].copy_from_slice(&999u16.to_le_bytes());
    assert_eq!(pcx::decode(&data).unwrap().0.pixels, vec![4; 6]);
}

#[test]
fn the_palette_needs_its_marker() {
    let pal = palette();
    let mut data = pcx::build(2, 2, &[0xC4, 1], Some(&pal));
    assert_eq!(pcx::decode(&data).unwrap().1, Some(pal));
    let at = data.len() - 769;
    assert_eq!(data[at], PALETTE_MARKER);
    data[at] = 0;
    assert_eq!(pcx::decode(&data).unwrap().1, None);
}

#[test]
fn malformed_files_are_refused() {
    let ok = pcx::build(4, 2, &[0xC8, 1], None);
    assert!(pcx::decode(&ok).is_ok());
    assert!(pcx::decode(&ok[..HEADER_SIZE - 1]).is_err(), "short header");
    assert!(
        pcx::decode(&ok[..HEADER_SIZE + 1]).is_err(),
        "run without value"
    );
    let mut planes = ok.clone();
    planes[0x41] = 3;
    assert!(pcx::decode(&planes).is_err(), "24-bit image");
    let mut bits = ok.clone();
    bits[0x03] = 4;
    assert!(pcx::decode(&bits).is_err(), "4-bit image");
    let mut empty = ok.clone();
    empty[0x08..0x0A].copy_from_slice(&(-1i16).to_le_bytes());
    assert!(pcx::decode(&empty).is_err(), "negative width");
    let mut huge = ok.clone();
    huge[0x08..0x0A].copy_from_slice(&(MAX_DIMENSION as i16).to_le_bytes());
    assert!(pcx::decode(&huge).is_err(), "too wide");
    // A big declared size with a tiny stream is refused without decoding.
    let big = pcx::build(4000, 4000, &[0xFF, 1, 0xFF, 2], None);
    let err = pcx::decode(&big).unwrap_err();
    assert!(err.to_string().contains("cannot cover"), "{err}");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn arbitrary_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = pcx::decode(&data);
    }

    #[test]
    fn arbitrary_streams_after_a_valid_header_never_panic(
        w in 1u16..300,
        h in 1u16..300,
        stream in proptest::collection::vec(any::<u8>(), 0..4096),
        with_palette in any::<bool>(),
    ) {
        let pal = palette();
        let data = pcx::build(w, h, &stream, with_palette.then_some(&pal));
        if let Ok((img, _)) = pcx::decode(&data) {
            prop_assert_eq!(img.pixels.len(), usize::from(w) * usize::from(h));
        }
    }

    #[test]
    fn round_trips(w in 1u32..120, h in 1u32..40, seed in any::<u32>()) {
        let pixels: Vec<u8> = (0..w * h)
            .map(|i| ((i / (1 + seed % 70)) ^ (seed >> 8)) as u8)
            .collect();
        let data = pcx::build(w as u16, h as u16, &pcx::encode(&pixels), None);
        let (img, pal) = pcx::decode(&data).unwrap();
        prop_assert_eq!(img.pixels, pixels);
        prop_assert_eq!(pal, None);
    }

    #[test]
    fn corrupted_files_never_panic(
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..16),
    ) {
        let pal = palette();
        let mut data = pcx::build(33, 17, &pcx::encode(&picture(33, 17)), Some(&pal));
        let len = data.len();
        for (pos, value) in edits {
            data[pos % len] = value;
        }
        let _ = pcx::decode(&data);
    }
}

fn pcx_files(dir: &DirNode, out: &mut Vec<claw_assets::FileId>) {
    out.extend(dir.files_of_type(TYPE_PCX));
    for sub in dir.dirs() {
        pcx_files(sub, out);
    }
}

#[test]
fn every_install_picture_decodes() {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let mut ids = Vec::new();
    pcx_files(store.root(), &mut ids);
    assert!(!ids.is_empty(), "no PCX pictures in the install");
    for id in &ids {
        let path = store.file(*id).path().to_string();
        let data = store.read(*id).unwrap();
        let (img, pal) = pcx::decode(&data).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(
            img.pixels.len(),
            (img.width * img.height) as usize,
            "{path}"
        );
        assert!(pal.is_some(), "{path}: no palette");
    }
    eprintln!("{} PCX pictures decode", ids.len());
}
