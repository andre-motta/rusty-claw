//! Tests against the user's own install. They read the folder named by the
//! `CLAW_INSTALL` environment variable at run time and skip themselves when
//! it is unset; no install file is ever copied into the repository.
//!
//! The exact counts below are those of the reference install the project is
//! developed against. Set `CLAW_SKIP_COUNT_CHECKS=1` to check another
//! edition, which still requires every file to load.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use claw_assets::level::RETAIL_LEVELS;
use claw_assets::name::{TYPE_ANI, TYPE_CLT, TYPE_PAL, TYPE_PID};
use claw_assets::{
    Ani, AssetStore, Clt, DirNode, FileId, Install, LevelData, LevelSource, Palette, Source, Wwd,
    pid,
};

fn install() -> Option<Install> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    Some(Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install"))
}

fn check_counts() -> bool {
    std::env::var_os("CLAW_SKIP_COUNT_CHECKS").is_none_or(|v| v.is_empty() || v == "0")
}

fn files_of_type(store: &AssetStore, type_code: u32) -> Vec<FileId> {
    let mut out = Vec::new();
    let mut stack: Vec<&DirNode> = vec![store.root()];
    while let Some(dir) = stack.pop() {
        out.extend(dir.files_of_type(type_code));
        stack.extend(dir.dirs());
    }
    out
}

#[test]
fn install_retail_levels_load() {
    let Some(install) = install() else { return };
    let store = install.store().unwrap();
    let mut loaded = 0;
    for n in RETAIL_LEVELS {
        let level = LevelData::build(&store, LevelSource::Retail(n))
            .unwrap_or_else(|e| panic!("LEVEL{n}: {e}"));
        let refs = level.check_references();
        assert!(
            refs.missing_tilesets.is_empty(),
            "LEVEL{n}: {:?}",
            refs.missing_tilesets
        );
        assert!(
            refs.missing_tiles.is_empty(),
            "LEVEL{n}: {:?}",
            refs.missing_tiles
        );
        assert!(level.main_plane.is_some(), "LEVEL{n} has no main plane");
        loaded += 1;
    }
    assert_eq!(loaded, 14);
}

#[test]
fn install_level1_builds_quickly() {
    let Some(install) = install() else { return };
    let started = Instant::now();
    let store = install.store().unwrap();
    let mounted = started.elapsed();
    let level = LevelData::build(&store, LevelSource::Retail(1)).unwrap();
    let total = started.elapsed();
    eprintln!(
        "LEVEL1: mount {:?}, level build {:?}, total {:?}",
        mounted,
        total - mounted,
        total
    );
    assert!(!level.planes.is_empty());
    if !cfg!(debug_assertions) {
        assert!(total < Duration::from_secs(1), "LEVEL1 took {total:?}");
    }
}

#[test]
fn install_every_level_file_parses() {
    let Some(install) = install() else { return };
    let store = install.store().unwrap();
    let mut parsed = 0;
    for n in RETAIL_LEVELS {
        let data = store
            .open(&format!("LEVEL{n}/WORLDS/WORLD.WWD"))
            .unwrap_or_else(|e| panic!("LEVEL{n}: {e}"));
        let wwd = Wwd::parse(&data).unwrap_or_else(|e| panic!("LEVEL{n}: {e}"));
        assert_eq!(wwd.unused_input, 0, "LEVEL{n}");
        parsed += 1;
    }
    assert_eq!(parsed, 14, "WWD 14/14");
}

#[test]
fn install_every_image_decodes() {
    let Some(install) = install() else { return };
    let store = install.store().unwrap();
    let ids = files_of_type(&store, TYPE_PID);
    let mut failures = Vec::new();
    for &id in &ids {
        if let Err(e) = store.read(id).and_then(|d| pid::decode(&d)) {
            failures.push(format!("{}: {e}", store.file(id).path()));
        }
    }
    eprintln!(
        "PID: {} of {} decoded",
        ids.len() - failures.len(),
        ids.len()
    );
    assert!(
        failures.is_empty(),
        "{} failures: {:?}",
        failures.len(),
        &failures[..failures.len().min(10)]
    );
    if check_counts() {
        assert_eq!(ids.len(), 10152);
    }
}

#[test]
fn install_every_animation_parses() {
    let Some(install) = install() else { return };
    let store = install.store().unwrap();
    let ids = files_of_type(&store, TYPE_ANI);
    let mut exact = 0;
    let mut failures = Vec::new();
    for &id in &ids {
        match store.read(id).and_then(|d| Ani::parse(&d)) {
            Ok(ani) if ani.is_exact() => exact += 1,
            Ok(ani) => eprintln!(
                "ANI {}: {} trailing, {} missing",
                store.file(id).path(),
                ani.trailing,
                ani.missing
            ),
            Err(e) => failures.push(format!("{}: {e}", store.file(id).path())),
        }
    }
    eprintln!("ANI: {exact} exact of {} files", ids.len());
    assert!(failures.is_empty(), "{failures:?}");
    if check_counts() {
        assert_eq!(ids.len(), 1143);
        assert_eq!(exact, 1141);
    }
}

#[test]
fn install_palettes_and_tables_parse() {
    let Some(install) = install() else { return };
    let store = install.store().unwrap();
    let pals = files_of_type(&store, TYPE_PAL);
    for &id in &pals {
        Palette::parse(&store.read(id).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", store.file(id).path()));
    }
    let clts = files_of_type(&store, TYPE_CLT);
    for &id in &clts {
        Clt::parse(&store.read(id).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", store.file(id).path()));
    }
    assert!(!pals.is_empty());
    if check_counts() {
        assert_eq!(pals.len(), 17);
        assert_eq!(clts.len(), 43);
    }
}

#[test]
fn install_custom_levels_load_or_fail_cleanly() {
    let Some(install) = install() else { return };
    let base = install.store().unwrap();
    let paths = install.custom_levels().unwrap();
    let mut ok = 0;
    let mut failed = Vec::new();
    for path in &paths {
        let mut store = base.clone();
        let result = store
            .mount(Source::CustomLevel(path.clone()))
            .and_then(|()| LevelData::build(&store, LevelSource::Custom));
        match result {
            Ok(_) => ok += 1,
            Err(e) => failed.push(format!("{}: {e}", path.display())),
        }
    }
    eprintln!("custom levels: {ok} of {} loaded", paths.len());
    for f in failed.iter().take(10) {
        eprintln!("  {f}");
    }
    assert!(ok >= 20, "only {ok} custom levels loaded");
}

#[test]
fn install_paths_cannot_escape() {
    let Some(install) = install() else { return };
    let store = install.store().unwrap();
    for bad in [
        "../CLAW.REZ",
        "LEVEL1/../../CLAW.REZ",
        "/etc/passwd",
        "C:\\CLAW.REZ",
    ] {
        assert!(store.open(bad).is_err(), "{bad:?}");
    }
    for bad in [
        "../Assets/LEVEL1/WORLDS/WORLD.WWD",
        "Custom/../CLAW.REZ",
        "/etc/passwd",
    ] {
        assert!(install.custom_level_path(bad).is_err(), "{bad:?}");
    }
}
