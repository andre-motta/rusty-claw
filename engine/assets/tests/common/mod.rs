//! Synthetic installs built in test code. No file here comes from a real
//! install; every byte is generated.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use claw_assets::ani::{self, AniFrame};
use claw_assets::pid;
use claw_assets::wwd::build::{PlaneSpec, level};
use claw_assets::wwd::{PLANE_FLAG_MAIN, TILE_EMPTY, TileDescriptor};

/// Writes `data` at `root/rel`, creating folders.
pub fn put(root: &Path, rel: &str, data: &[u8]) -> PathBuf {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, data).unwrap();
    path
}

/// A 4x4 PCX-style image filled with one index.
pub fn tile(index: u8) -> Vec<u8> {
    pid::build(4, 4, 0x2, (0, 0), &pid::encode_pcx(&[index; 16], 4))
}

/// A palette whose entry i is (i, i, i), or (i, 0, 0) when `red`.
pub fn palette(red: bool) -> Vec<u8> {
    (0..256u32)
        .flat_map(|i| {
            let v = i as u8;
            if red { [v, 0, 0] } else { [v, v, v] }
        })
        .collect()
}

/// A level whose action plane uses tiles 1 and 2 of `ACTION` and places one
/// rat.
pub fn world(name: &str, extra_tile: Option<i32>) -> Vec<u8> {
    let mut tiles = vec![1, 2, TILE_EMPTY, 1];
    if let Some(t) = extra_tile {
        tiles[2] = t;
    }
    let planes = vec![
        PlaneSpec {
            name: "Background".into(),
            flags: 0x4,
            tile_size: (4, 4),
            size: (1, 1),
            move_pct: (50, 50),
            z: -999,
            image_sets: vec!["BACK".into()],
            tiles: vec![1],
            objects: vec![],
        },
        PlaneSpec {
            name: "Action".into(),
            flags: PLANE_FLAG_MAIN,
            tile_size: (4, 4),
            size: (2, 2),
            move_pct: (100, 100),
            z: 0,
            image_sets: vec!["ACTION".into()],
            tiles,
            objects: vec![[
                "Rat".into(),
                "Rat".into(),
                "LEVEL_RAT".into(),
                "LEVEL_RAT_WALK".into(),
            ]],
        },
    ];
    let descs = vec![
        TileDescriptor::Single {
            width: 4,
            height: 4,
            attribute: 0,
        };
        3
    ];
    level(name, (10, 20), &planes, &descs, true)
}

/// Builds a small install: `Assets` with LEVEL1, CLAW and GAME, and a
/// `Custom` folder with one level and its folder.
pub fn install(root: &Path) {
    let a = |rel: &str, data: &[u8]| put(root, &format!("Assets/{rel}"), data);
    a("LEVEL1/PALETTES/MAIN.PAL", &palette(false));
    a("LEVEL1/TILES/ACTION/001.PID", &tile(1));
    a("LEVEL1/TILES/ACTION/002.PID", &tile(2));
    a("LEVEL1/TILES/BACK/001.PID", &tile(3));
    a("level1/sounds/drip.wav", b"RIFF");
    a("LEVEL1/IMAGES/RAT/FRAME001.PID", &tile(4));
    a("LEVEL1/IMAGES/RAT/FRAME002.PID", &tile(5));
    let walk = ani::build(
        "LEVEL_RAT",
        &[AniFrame {
            image_index: 1,
            duration: 100,
            ..AniFrame::default()
        }],
    );
    a("LEVEL1/ANIS/RAT/WALK.ANI", &walk);
    a("LEVEL1/WORLDS/WORLD.WWD", &world("Level 1", None));
    a("LEVEL1/MUSIC/PLAY.XMI", b"FORM");
    a("LEVEL1/MUSIC/PLAY0.XMI", b"FORM");
    a("CLAW/SOUNDS/HIT.WAV", b"RIFF");
    a("CLAW/IMAGES/STAND/001.PID", &tile(6));
    a("CLAW/ANIS/STAND.ANI", &ani::build("CLAW_STAND", &[]));
    a("GAME/SOUNDS/COIN.WAV", b"RIFF");
    a("GAME/IMAGES/COIN/001.PID", &tile(7));
    a("GAME/ANIS/SPIN.ANI", &ani::build("GAME_COIN", &[]));
    a("GAME/MUSIC/MONOLITH.XMI", b"FORM");

    let c = |rel: &str, data: &[u8]| put(root, &format!("Custom/{rel}"), data);
    c("My Level.WWD", &world("Remix of level 1", Some(3)));
    c("My Level/TILES/ACTION/003.PID", &tile(8));
    c("My Level/TILES/ACTION/001.PID", &tile(9));
    c("My Level/IMAGES/SPLASH/001.PID", &tile(10));
    c("My Level/PALETTES/MAIN.PAL", &palette(true));
    c("My Level/MUSIC/LEVEL.XMI", b"FORM");
    c("My Level/LOGICS/Boss.lua", b"error('never run')");
    c("Broken.WWD", b"not a level");
    c("No Number.WWD", &world("Untitled", None));
}
