//! Property tests: readers never panic on arbitrary or corrupted input, and
//! logical paths never escape their root.

mod common;

use claw_assets::rez::RezBuilder;
use claw_assets::wwd::build::{PlaneSpec, level};
use claw_assets::wwd::{PLANE_FLAG_MAIN, TileDescriptor};
use claw_assets::{Ani, AssetError, AssetStore, Clt, Palette, Source, Wwd, name, pid};
use proptest::prelude::*;

fn sample_level(compress: bool) -> Vec<u8> {
    let planes = vec![PlaneSpec {
        name: "Action".into(),
        flags: PLANE_FLAG_MAIN,
        tile_size: (64, 64),
        size: (3, 2),
        move_pct: (100, 100),
        z: 0,
        image_sets: vec!["ACTION".into()],
        tiles: vec![1, 2, -1, 4, 5, 6],
        objects: vec![["A".into(), "B".into(), "C".into(), "D".into()]],
    }];
    let descs = vec![
        TileDescriptor::Single {
            width: 64,
            height: 64,
            attribute: 1,
        },
        TileDescriptor::Mask {
            width: 2,
            height: 2,
            mask: vec![1, 2, 3, 4],
        },
    ];
    level("Level 1", (5, 6), &planes, &descs, compress)
}

fn sample_rez() -> Vec<u8> {
    RezBuilder::new()
        .file("A/B/C.PID", b"one")
        .file("A/D.PAL", &[0; 768])
        .file("E.TXT", b"two")
        .build()
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
    fn readers_accept_any_bytes(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = pid::decode(&data);
        let _ = Palette::parse(&data);
        let _ = Clt::parse(&data);
        let _ = Ani::parse(&data);
        let _ = Wwd::parse(&data);
        let _ = AssetStore::new().mount(Source::RezBytes(data));
    }

    #[test]
    fn corrupted_levels_never_panic(
        compress in any::<bool>(),
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..16),
        cut in any::<usize>(),
    ) {
        let data = mutate(sample_level(compress), &edits);
        let _ = Wwd::parse(&data);
        let len = cut % (data.len() + 1);
        let _ = Wwd::parse(&data[..len]);
    }

    #[test]
    fn corrupted_header_fields_never_panic(
        field in prop::sample::select(vec![0usize, 0x2DC, 0x2E0, 0x2E4, 0x2E8, 0x5F4, 0x5F4 + 0x60,
            0x5F4 + 0x64, 0x5F4 + 0x7C, 0x5F4 + 0x80, 0x5F4 + 0x84, 0x5F4 + 0x88, 0x5F4 + 0x8C]),
        value in any::<u32>(),
    ) {
        let mut data = sample_level(false);
        data[field..field + 4].copy_from_slice(&value.to_le_bytes());
        let _ = Wwd::parse(&data);
    }

    #[test]
    fn corrupted_archives_never_panic(
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..16),
    ) {
        let data = mutate(sample_rez(), &edits);
        let mut store = AssetStore::new();
        if store.mount(Source::RezBytes(data)).is_ok() {
            // Whatever mounted must read back without panicking.
            let mut stack = vec![store.root().clone()];
            while let Some(dir) = stack.pop() {
                for id in dir.files() {
                    let _ = store.read(id);
                }
                stack.extend(dir.dirs().cloned());
            }
        }
    }

    #[test]
    fn corrupted_images_never_panic(
        rle in any::<bool>(),
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..8),
    ) {
        let data = if rle {
            pid::build(8, 2, pid::FLAG_RLE, (0, 0), &[0x80, 8, 1, 2, 3, 4, 5, 6, 7, 8, 0x88])
        } else {
            pid::build(8, 2, 0, (0, 0), &pid::encode_pcx(&[9; 16], 8))
        };
        let _ = pid::decode(&mutate(data, &edits));
    }

    #[test]
    fn logical_paths_stay_inside(segments in proptest::collection::vec(
        prop::sample::select(vec!["..", ".", "", "a", "A", "b", "C:", "c:", "x\0", "/", "\\", "dir"]),
        0..8,
    ), sep in prop::sample::select(vec!["/", "\\"])) {
        let path = segments.join(sep);
        match name::split_logical(&path) {
            Ok(parts) => {
                for p in &parts {
                    prop_assert!(*p != ".." && *p != "." && !p.is_empty());
                    prop_assert!(!p.contains(['/', '\\', '\0']));
                }
                prop_assert!(!path.starts_with(['/', '\\']));
                prop_assert!(!path.split(['/', '\\']).any(|s| s == ".."));
            }
            Err(AssetError::Path { .. }) => {}
            Err(other) => prop_assert!(false, "unexpected error {other}"),
        }
    }
}

#[test]
fn open_never_reaches_outside_the_root() {
    let tmp = tempfile::tempdir().unwrap();
    common::put(tmp.path(), "secret.txt", b"secret");
    common::put(tmp.path(), "root/a/file.txt", b"inside");
    let mut store = AssetStore::new();
    store.mount(Source::Dir(tmp.path().join("root"))).unwrap();
    let pieces = ["..", "a", "secret.txt", "file.txt", "/", "\\", ".", "C:"];
    // Every combination of up to four pieces.
    let mut paths = vec![String::new()];
    for _ in 0..4 {
        let mut next = Vec::new();
        for p in &paths {
            for piece in pieces {
                next.push(format!("{p}{piece}/"));
                next.push(format!("{p}{piece}"));
            }
        }
        paths.extend(next);
        paths.sort();
        paths.dedup();
    }
    for path in paths {
        if let Ok(data) = store.open(&path) {
            assert_eq!(data, b"inside", "{path:?}");
        }
    }
}
