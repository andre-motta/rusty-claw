//! Level model and registry rules on a synthetic install built in-test.

mod common;

use claw_assets::registry::RegistryEventKind;
use claw_assets::{AssetStore, Install, LevelData, LevelSource, Registries, Source};
use common::put;

fn store_for(root: &std::path::Path) -> (Install, AssetStore) {
    let install = Install::open(root).unwrap();
    let store = install.store().unwrap();
    (install, store)
}

#[test]
fn retail_level_builds() {
    let tmp = tempfile::tempdir().unwrap();
    common::install(tmp.path());
    let (_, store) = store_for(tmp.path());
    let level = LevelData::build(&store, LevelSource::Retail(1)).unwrap();
    assert_eq!(level.level, 1);
    assert!(level.custom_name.is_none());
    assert_eq!((level.start_x, level.start_y), (10, 20));
    assert_eq!(level.planes.len(), 2);
    assert_eq!(level.main_plane, Some(1));
    assert_eq!(level.objects.len(), 1);
    assert_eq!(level.tile_attributes.len(), 3);
    assert_eq!(level.palette.color(7), [7, 7, 7]);

    let reg = &level.registries;
    let keys: Vec<&str> = reg.images.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        ["ACTION", "BACK", "CLAW_STAND", "GAME_COIN", "LEVEL_RAT"]
    );
    assert_eq!(
        reg.images["ACTION"]
            .frames
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(reg.images["LEVEL_RAT"].frames.len(), 2);
    let sounds: Vec<&str> = reg.sounds.keys().map(String::as_str).collect();
    assert_eq!(sounds, ["CLAW_HIT", "GAME_COIN", "LEVEL_DRIP"]);
    let anis: Vec<&str> = reg.anis.keys().map(String::as_str).collect();
    assert_eq!(anis, ["CLAW_STAND", "GAME_SPIN", "LEVEL_RAT_WALK"]);
    // The 0 variant is preferred.
    assert_eq!(store.file(reg.music["LEVEL"]).name(), "PLAY0");
    assert!(reg.music.contains_key("MONOLITH"));

    let refs = level.check_references();
    assert_eq!(refs.tilesets, 2);
    assert!(refs.missing_tilesets.is_empty());
    assert!(refs.missing_tiles.is_empty());
    assert!(refs.missing_object_image_sets.is_empty());
    assert!(refs.missing_object_anims.is_empty());

    let set = reg.load_image_set(&store, "LEVEL_RAT").unwrap();
    assert_eq!(set.frames[&2].indices, vec![5; 16]);
    assert!(set.skipped.is_empty());
}

#[test]
fn custom_level_builds_on_its_retail_level() {
    let tmp = tempfile::tempdir().unwrap();
    common::install(tmp.path());
    let (install, mut store) = store_for(tmp.path());
    let path = install.custom_level_path("My Level.WWD").unwrap();
    store.mount(Source::CustomLevel(path)).unwrap();
    let level = LevelData::build(&store, LevelSource::Custom).unwrap();
    assert_eq!(level.level, 1);
    assert_eq!(level.custom_name.as_deref(), Some("My Level"));
    // The folder's palette replaces the level palette.
    assert_eq!(level.palette.color(7), [7, 0, 0]);
    let reg = &level.registries;
    // Custom tiles join ACTION; the retail frame 1 is kept.
    let action = &reg.images["ACTION"];
    assert_eq!(action.frames.keys().copied().collect::<Vec<_>>(), [1, 2, 3]);
    let frame1 = store.file(action.frames[&1]);
    assert!(frame1.path().starts_with("LEVEL1/"), "{}", frame1.path());
    assert!(
        reg.events
            .iter()
            .any(|e| e.kind == RegistryEventKind::FrameKept)
    );
    assert!(reg.images.contains_key("CUSTOM_SPLASH"));
    assert_eq!(store.file(reg.music["LEVEL"]).path(), "MUSIC/LEVEL.XMI");
    assert!(level.check_references().missing_tiles.is_empty());
    // Script files are only data in the tree.
    assert!(store.folder("LOGICS").unwrap().is_some());
}

#[test]
fn malformed_custom_levels_fail_cleanly() {
    let tmp = tempfile::tempdir().unwrap();
    common::install(tmp.path());
    let (install, base) = store_for(tmp.path());
    for name in ["Broken.WWD", "No Number.WWD"] {
        let mut store = base.clone();
        store
            .mount(Source::CustomLevel(
                install.custom_level_path(name).unwrap(),
            ))
            .unwrap();
        assert!(
            LevelData::build(&store, LevelSource::Custom).is_err(),
            "{name}"
        );
    }
    // A custom level that names a level outside 1..14.
    put(
        tmp.path(),
        "Custom/Far.WWD",
        &common::world("Level 15", None),
    );
    let mut store = base.clone();
    store
        .mount(Source::CustomLevel(
            install.custom_level_path("Far.WWD").unwrap(),
        ))
        .unwrap();
    assert!(LevelData::build(&store, LevelSource::Custom).is_err());
    // No custom level mounted.
    assert!(LevelData::build(&base, LevelSource::Custom).is_err());
}

#[test]
fn missing_retail_parts_fail_cleanly() {
    let tmp = tempfile::tempdir().unwrap();
    common::install(tmp.path());
    let (_, store) = store_for(tmp.path());
    assert!(LevelData::build(&store, LevelSource::Retail(2)).is_err());
    assert!(LevelData::build(&store, LevelSource::Retail(0)).is_err());
    assert!(LevelData::build(&store, LevelSource::Retail(15)).is_err());
    std::fs::remove_dir_all(tmp.path().join("Assets/CLAW/ANIS")).unwrap();
    let (_, store) = store_for(tmp.path());
    assert!(LevelData::build(&store, LevelSource::Retail(1)).is_err());
}

#[test]
fn registry_collision_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();
    // Sounds: A/B.WAV (subfolders first) and A_B.WAV share the key X_A_B.
    put(r, "SOUNDS/A/B.WAV", b"first");
    put(r, "SOUNDS/A_B.WAV", b"second");
    // Animations: same shape; the later one replaces.
    put(r, "ANIS/A/B.ANI", b"first");
    put(r, "ANIS/A_B.ANI", b"second");
    // Images: two files with frame number 1; the first in folder order wins.
    put(r, "IMAGES/SET/001.PID", &common::tile(1));
    put(r, "IMAGES/SET/FRAME1.PID", &common::tile(2));
    put(r, "IMAGES/SET/NOTES.TXT", b"not an image");
    put(r, "IMAGES/EMPTY/README.TXT", b"no frames");
    let mut store = AssetStore::new();
    store.mount(Source::Dir(r.into())).unwrap();
    let mut reg = Registries::new();
    reg.map_sounds(&store, store.folder("SOUNDS").unwrap().unwrap(), "X");
    reg.map_anis(&store, store.folder("ANIS").unwrap().unwrap(), "X");
    reg.map_images(&store, store.folder("IMAGES").unwrap().unwrap(), "X");
    assert_eq!(store.read(reg.sounds["X_A_B"]).unwrap(), b"first");
    assert_eq!(store.read(reg.anis["X_A_B"]).unwrap(), b"second");
    let set = &reg.images["X_SET"];
    assert_eq!(set.frames.len(), 1);
    assert_eq!(store.file(set.frames[&1]).name(), "001");
    assert!(!reg.images.contains_key("X_EMPTY"));
    let kinds: Vec<RegistryEventKind> = reg.events.iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&RegistryEventKind::SoundKept));
    assert!(kinds.contains(&RegistryEventKind::AnimationReplaced));
    assert!(kinds.contains(&RegistryEventKind::FrameKept));
}
