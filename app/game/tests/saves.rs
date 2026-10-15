//! Saved games: round trip, backups, damaged and newer files kept aside,
//! level names that cannot leave the folder, and the second-instance lock.

use std::fs;
use std::path::Path;

use claw_frame::Weapon;
use claw_game::saves::{
    MAX_SAVE_BYTES, SAVE_VERSION, SaveError, SaveRecord, SaveStore, check_level_name,
    retail_level_name,
};
use claw_game::settings::{WriteHooks, WriteStep, backup_path};
use claw_sim::{Carry, SavePos};

fn record(level: &str, slot: u8, score: u32) -> SaveRecord {
    SaveRecord::new(
        level,
        SavePos {
            slot,
            x: 5410,
            y: 4032,
        },
        Carry {
            score,
            health: 85,
            lives: 6,
            pistol: 12,
            magic: 5,
            dynamite: 3,
            weapon: Weapon::Magic,
            treasure_game: [10, 2, 0, 1, 0, 0, 0, 0, 7],
        },
        [3, 1, 0, 0, 0, 0, 0, 0, 2],
    )
}

/// Files in `dir` whose names start with `prefix`.
fn named(dir: &Path, prefix: &str) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(prefix))
        .collect();
    v.sort();
    v
}

#[test]
fn a_record_round_trips() {
    let r = record("LEVEL1", 2, 25_000);
    assert_eq!(r.version, SAVE_VERSION);
    let text = r.to_text();
    assert!(text.starts_with("# rusty-claw saved game\nversion = 1\n"));
    assert_eq!(SaveRecord::parse(&text), Ok(r.clone()));
    let start = r.start_state();
    assert_eq!(start.carry, Some(r.carry.clone()));
    assert_eq!(
        start.save,
        Some(SavePos {
            slot: 2,
            x: 5410,
            y: 4032
        })
    );
    // Slot 3, the level start, has no position.
    let mut start_of_level = r.clone();
    start_of_level.slot = 3;
    start_of_level.x = 0;
    start_of_level.y = 0;
    assert_eq!(start_of_level.start_state().save, None);
    assert_eq!(retail_level_name(14), "LEVEL14");

    let dir = tempfile::tempdir().unwrap();
    let mut store = SaveStore::open(dir.path()).unwrap();
    assert!(!store.is_read_only());
    assert_eq!(store.read("LEVEL1", 2).unwrap(), None);
    store.write(&r).unwrap();
    store.write(&record("LEVEL1", 1, 10)).unwrap();
    store.write(&record("LEVEL3", 1, 99)).unwrap();
    assert_eq!(store.read("LEVEL1", 2).unwrap(), Some(r.clone()));
    let all = store.list().unwrap();
    let keys: Vec<(String, u8)> = all.iter().map(|r| (r.level.clone(), r.slot)).collect();
    assert_eq!(
        keys,
        vec![
            ("LEVEL1".into(), 1),
            ("LEVEL1".into(), 2),
            ("LEVEL3".into(), 1)
        ]
    );
    assert!(store.reports().is_empty());
}

#[test]
fn overwriting_a_slot_keeps_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SaveStore::open(dir.path()).unwrap();
    let first = record("LEVEL1", 1, 100);
    let second = record("LEVEL1", 1, 200);
    store.write(&first).unwrap();
    store.write(&second).unwrap();
    let path = store.path("LEVEL1", 1).unwrap();
    assert_eq!(store.read("LEVEL1", 1).unwrap(), Some(second));
    assert_eq!(
        SaveRecord::parse(&fs::read_to_string(backup_path(&path)).unwrap()),
        Ok(first)
    );
}

#[test]
fn an_interrupted_save_leaves_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SaveStore::open(dir.path()).unwrap();
    let old = record("LEVEL2", 1, 1);
    store.write(&old).unwrap();
    for step in [
        WriteStep::Temp,
        WriteStep::Data,
        WriteStep::Flush,
        WriteStep::Backup,
        WriteStep::Rename,
    ] {
        let mut hooks = WriteHooks {
            fail_at: Some(step),
            ..WriteHooks::default()
        };
        assert!(
            store
                .write_with(&record("LEVEL2", 1, 2), &mut hooks)
                .is_err()
        );
        assert_eq!(
            store.read("LEVEL2", 1).unwrap(),
            Some(old.clone()),
            "{step:?}"
        );
    }
    assert!(named(dir.path(), ".LEVEL2").is_empty());
}

#[test]
fn damaged_and_newer_saves_are_moved_aside_never_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SaveStore::open(dir.path()).unwrap();
    let good = record("LEVEL1", 1, 5).to_text();
    let cases: Vec<(u8, Vec<u8>, &str)> = vec![
        (1, b"\0\x01garbage".to_vec(), "unreadable"),
        (2, good.as_bytes()[..good.len() / 2].to_vec(), "unreadable"),
        (
            3,
            good.replace("version = 1", "version = 2").into_bytes(),
            "newer",
        ),
    ];
    for (slot, bytes, tag) in &cases {
        let path = store.path("LEVEL1", *slot).unwrap();
        fs::write(&path, bytes).unwrap();
        assert_eq!(store.read("LEVEL1", *slot).unwrap(), None);
        assert!(!path.exists());
        let aside = named(dir.path(), &format!("LEVEL1-{slot}.save.{tag}-"));
        assert_eq!(aside.len(), 1, "{aside:?}");
        assert_eq!(&fs::read(dir.path().join(&aside[0])).unwrap(), bytes);
    }
    assert_eq!(store.take_reports().len(), 3);
    // A second damaged file in the same second gets its own name.
    let path = store.path("LEVEL1", 1).unwrap();
    fs::write(&path, b"again").unwrap();
    assert_eq!(store.list().unwrap(), vec![]);
    assert_eq!(named(dir.path(), "LEVEL1-1.save.unreadable-").len(), 2);
    // A save holding another slot, or too large a file, is kept aside too.
    fs::write(&path, record("LEVEL1", 2, 1).to_text()).unwrap();
    assert_eq!(store.read("LEVEL1", 1).unwrap(), None);
    let big = vec![b'#'; MAX_SAVE_BYTES as usize + 1];
    fs::write(&path, &big).unwrap();
    assert_eq!(store.read("LEVEL1", 1).unwrap(), None);
    assert_eq!(named(dir.path(), "LEVEL1-1.save.unreadable-").len(), 4);
    // Writing over a damaged slot keeps the damaged file aside first.
    fs::write(&path, b"damaged").unwrap();
    store.write(&record("LEVEL1", 1, 9)).unwrap();
    assert_eq!(named(dir.path(), "LEVEL1-1.save.unreadable-").len(), 5);
    assert!(!backup_path(&path).exists());
    assert_eq!(
        store.read("LEVEL1", 1).unwrap(),
        Some(record("LEVEL1", 1, 9))
    );
}

#[cfg(unix)]
#[test]
fn a_save_that_cannot_be_read_is_left_where_it_is() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mut store = SaveStore::open(dir.path()).unwrap();
    let saved = record("LEVEL1", 1, 5);
    store.write(&saved).unwrap();
    let path = store.path("LEVEL1", 1).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::File::open(&path).is_ok() {
        // Permissions do not stop this user (for example root).
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        eprintln!("skipped: unreadable files can be read here");
        return;
    }
    let _ = store.take_reports();
    assert_eq!(store.read("LEVEL1", 1).unwrap(), None);
    assert!(path.exists(), "not moved aside");
    assert!(named(dir.path(), "LEVEL1-1.save.unreadable-").is_empty());
    let reports = store.take_reports();
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(reports[0].contains("cannot be read"), "{reports:?}");
    // Building the save lists again does not repeat the notice.
    assert_eq!(store.read("LEVEL1", 1).unwrap(), None);
    store.list().unwrap();
    store.list().unwrap();
    assert_eq!(store.take_reports(), Vec::<String>::new());
    // Writing to the slot fails rather than replacing it unseen.
    assert!(store.write(&record("LEVEL1", 1, 9)).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    // Readable again: the save counts.
    assert_eq!(store.read("LEVEL1", 1).unwrap(), Some(saved));
}

#[test]
fn parse_errors_name_the_problem() {
    let good = record("LEVEL1", 1, 5).to_text();
    assert_eq!(
        SaveRecord::parse(&good.replace("version = 1", "version = 7")),
        Err(SaveError::Newer(7))
    );
    for bad in [
        String::new(),
        good.replace("version = 1\n", ""),
        good.replace("slot = 1", "slot = 4"),
        good.replace("weapon = magic", "weapon = cannon"),
        good.replace("lives = 6", "lives = 6\nlives = 7"),
        good.replace("health = 85", "health = 99999999999"),
        good.replace(
            "treasure_level = 3, 1, 0, 0, 0, 0, 0, 0, 2",
            "treasure_level = 1",
        ),
        good.replace("level = LEVEL1", "level = ../x"),
        good + "cheats = on\n",
    ] {
        assert!(
            matches!(SaveRecord::parse(&bad), Err(SaveError::Unreadable(_))),
            "{bad}"
        );
    }
}

#[test]
fn level_names_cannot_leave_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SaveStore::open(dir.path().join("saves")).unwrap();
    for bad in [
        "",
        ".",
        "..",
        "../LEVEL1",
        "a/b",
        "a\\b",
        "/etc/passwd",
        "LEVEL1\0",
        "C:x",
        "LEVEL 1",
        &"L".repeat(33),
    ] {
        assert!(check_level_name(bad).is_err(), "{bad:?}");
        assert!(store.path(bad, 1).is_err(), "{bad:?}");
        assert!(store.read(bad, 1).is_err(), "{bad:?}");
        let mut r = record("LEVEL1", 1, 0);
        r.level = bad.to_string();
        assert!(store.write(&r).is_err(), "{bad:?}");
    }
    for slot in [0, 4, 255] {
        assert!(store.read("LEVEL1", slot).is_err());
    }
    // Nothing was created next to the saves folder.
    assert_eq!(named(dir.path(), ""), vec!["saves".to_string()]);
    // Files that are not saves are never read.
    fs::write(dir.path().join("saves").join("CLAW.USR"), b"x").unwrap();
    assert_eq!(store.list().unwrap(), vec![]);
    assert!(dir.path().join("saves").join("CLAW.USR").exists());
}

#[test]
fn a_second_running_game_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut first = SaveStore::open(dir.path()).unwrap();
    first.write(&record("LEVEL1", 1, 1)).unwrap();
    let mut second = SaveStore::open(dir.path()).unwrap();
    assert!(!first.is_read_only());
    assert!(second.is_read_only());
    assert!(second.reports()[0].contains("read-only"));
    assert_eq!(
        second.read("LEVEL1", 1).unwrap(),
        Some(record("LEVEL1", 1, 1))
    );
    assert!(second.write(&record("LEVEL1", 1, 2)).is_err());
    // It does not move damaged files either.
    let path = second.path("LEVEL1", 2).unwrap();
    fs::write(&path, b"damaged").unwrap();
    assert_eq!(second.read("LEVEL1", 2).unwrap(), None);
    assert!(path.exists());
    drop(first);
    let mut third = SaveStore::open(dir.path()).unwrap();
    assert!(!third.is_read_only());
    third.write(&record("LEVEL1", 1, 3)).unwrap();
    assert_eq!(
        second.read("LEVEL1", 1).unwrap(),
        Some(record("LEVEL1", 1, 3))
    );
}
