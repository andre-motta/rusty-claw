//! Layered store behaviour on synthetic folders and archives built in-test.

mod common;

use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;

use claw_assets::name::{TYPE_PAL, TYPE_PID};
use claw_assets::rez::RezBuilder;
use claw_assets::store::NoteKind;
use claw_assets::{AssetError, AssetStore, Install, Source};
use common::put;

#[test]
fn folder_mount_is_case_insensitive() {
    let tmp = tempfile::tempdir().unwrap();
    put(tmp.path(), "Level1/Palettes/main.pal", &[1; 768]);
    let mut store = AssetStore::new();
    store.mount(Source::Dir(tmp.path().into())).unwrap();
    for path in [
        "LEVEL1/PALETTES/MAIN.PAL",
        "level1/palettes/main.pal",
        "LEVEL1\\PALETTES\\MAIN.PAL",
        "./level1//PALETTES/Main.Pal",
    ] {
        assert_eq!(store.open(path).unwrap().len(), 768, "{path}");
    }
    // Folder names and file names are stored upper-cased.
    let dir = store.folder("level1").unwrap().unwrap();
    assert_eq!(dir.name(), "LEVEL1");
    let id = store
        .lookup_typed("level1/palettes/main", TYPE_PAL)
        .unwrap()
        .unwrap();
    assert_eq!(store.file(id).name(), "MAIN");
    assert!(store.lookup("LEVEL1/PALETTES/MAIN.XMI").unwrap().is_none());
    assert!(matches!(
        store.open("LEVEL1/NOPE.PAL"),
        Err(AssetError::NotFound(_))
    ));
}

#[test]
fn later_mounts_win_and_folders_merge() {
    let archive = RezBuilder::new()
        .file("LEVEL1/PALETTES/MAIN.PAL", &[1; 768])
        .file("LEVEL1/TILES/ACTION/001.PID", b"from archive")
        .file("GAME/ONLY.TXT", b"archive only")
        .build();
    let tmp = tempfile::tempdir().unwrap();
    put(tmp.path(), "level1/palettes/MAIN.PAL", &[2; 768]);
    put(tmp.path(), "LEVEL1/TILES/ACTION/002.PID", b"from folder");

    let mut store = AssetStore::new();
    store.mount(Source::RezBytes(archive)).unwrap();
    store.mount(Source::Dir(tmp.path().into())).unwrap();

    assert_eq!(
        store.open("LEVEL1/PALETTES/MAIN.PAL").unwrap(),
        vec![2; 768]
    );
    assert_eq!(
        store.open("LEVEL1/TILES/ACTION/001.PID").unwrap(),
        b"from archive"
    );
    assert_eq!(
        store.open("LEVEL1/TILES/ACTION/002.PID").unwrap(),
        b"from folder"
    );
    assert_eq!(store.open("game/only.txt").unwrap(), b"archive only");
    let action = store.folder("LEVEL1/TILES/ACTION").unwrap().unwrap();
    assert_eq!(action.files_of_type(TYPE_PID).count(), 2);
    assert!(store.notes().iter().any(|n| n.kind == NoteKind::Replaced));
}

#[test]
fn archive_on_disk_and_empty_archive() {
    let tmp = tempfile::tempdir().unwrap();
    let path = put(
        tmp.path(),
        "ARCHIVE.REZ",
        &RezBuilder::new().file("A/B.TXT", b"hello").build(),
    );
    let mut store = AssetStore::new();
    store.mount(Source::Rez(path)).unwrap();
    assert_eq!(store.open("a/b.txt").unwrap(), b"hello");

    // An archive with an empty root folder mounts as an empty tree.
    let empty = put(tmp.path(), "EMPTY.REZ", &RezBuilder::new().build());
    let mut store = AssetStore::new();
    store.mount(Source::Rez(empty)).unwrap();
    assert_eq!(store.root().count(), (0, 0));
}

fn archive_with_root_block(block: &[u8], archive_size_extra: usize) -> Vec<u8> {
    let mut data = RezBuilder::new().build();
    let offset = data.len() as u32;
    data.extend_from_slice(block);
    data.extend(std::iter::repeat_n(0u8, archive_size_extra));
    data[0x83..0x87].copy_from_slice(&offset.to_le_bytes());
    data[0x87..0x8B].copy_from_slice(&(block.len() as u32).to_le_bytes());
    data
}

fn dir_entry(name: &str, offset: u32, size: u32) -> Vec<u8> {
    let mut out = Vec::new();
    for v in [1u32, offset, size, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    out
}

#[test]
fn hostile_archives_are_refused() {
    // A folder whose block is the root block itself: a cycle.
    let header_len = RezBuilder::new().build().len() as u32;
    let entry_len = dir_entry("LOOP", 0, 0).len() as u32;
    let block = dir_entry("LOOP", header_len, entry_len);
    let data = archive_with_root_block(&block, 0);
    let mut store = AssetStore::new();
    assert!(store.mount(Source::RezBytes(data)).is_err());

    // A folder block outside the archive.
    let block = dir_entry("FAR", 1 << 30, 16);
    let mut store = AssetStore::new();
    assert!(
        store
            .mount(Source::RezBytes(archive_with_root_block(&block, 0)))
            .is_err()
    );

    // A root block larger than the archive.
    let mut data = RezBuilder::new().file("A.TXT", b"x").build();
    data[0x87..0x8B].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(AssetStore::new().mount(Source::RezBytes(data)).is_err());

    // Bad signature, short header.
    let mut data = RezBuilder::new().build();
    data[0x7E] = 0;
    assert!(AssetStore::new().mount(Source::RezBytes(data)).is_err());
    assert!(
        AssetStore::new()
            .mount(Source::RezBytes(vec![0x0D; 20]))
            .is_err()
    );

    // Names that cannot be addressed are skipped, not mounted.
    let block = [dir_entry("..", 0, 0), dir_entry("A/B", 0, 0)].concat();
    let mut store = AssetStore::new();
    store
        .mount(Source::RezBytes(archive_with_root_block(&block, 0)))
        .unwrap();
    assert_eq!(store.root().count(), (0, 0));
    assert_eq!(
        store
            .notes()
            .iter()
            .filter(|n| n.kind == NoteKind::UnusableName)
            .count(),
        2
    );
}

fn file_entry(name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    // Kind, data offset, data size, time, id, type code, key count.
    for v in [0u32, 0, 0, 0, 0, TYPE_PID, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(&[0, 0]);
    out
}

#[test]
fn overlapping_folder_blocks_are_refused() {
    // One region of 64 file entries, and a root block whose 64 folders each
    // point at a longer prefix of it. Every folder block is new by (offset,
    // size), but together they read the region about 32 times over.
    let files: Vec<Vec<u8>> = (0..64).map(|i| file_entry(&format!("F{i:02}"))).collect();
    let entry_len = files[0].len() as u32;
    let region: Vec<u8> = files.concat();
    let header_len = RezBuilder::new().build().len() as u32;
    let folders = |region_at: u32, count: u32| -> Vec<u8> {
        (0..count)
            .flat_map(|j| dir_entry(&format!("D{j:02}"), region_at, (j + 1) * entry_len))
            .collect()
    };
    let root_len = folders(0, 64).len() as u32;
    let region_at = header_len + root_len;
    let mut block = folders(region_at, 64);
    block.extend_from_slice(&region);
    let mut data = archive_with_root_block(&block, 0);
    data[0x87..0x8B].copy_from_slice(&root_len.to_le_bytes());
    let err = AssetStore::new().mount(Source::RezBytes(data)).unwrap_err();
    assert!(err.to_string().contains("overlap"), "{err}");

    // A few overlapping folders stay within the budget and mount.
    let root_len = folders(0, 2).len() as u32;
    let region_at = header_len + root_len;
    let mut block = folders(region_at, 2);
    block.extend_from_slice(&region);
    let mut data = archive_with_root_block(&block, 0);
    data[0x87..0x8B].copy_from_slice(&root_len.to_le_bytes());
    let mut store = AssetStore::new();
    store.mount(Source::RezBytes(data)).unwrap();
    assert_eq!(store.root().count(), (1 + 2, 2));
}

#[test]
fn deep_archive_nesting_is_refused() {
    let mut builder = RezBuilder::new();
    let deep = vec!["D"; 80].join("/") + "/F.TXT";
    builder.file(&deep, b"x");
    assert!(
        AssetStore::new()
            .mount(Source::RezBytes(builder.build()))
            .is_err()
    );
    let mut builder = RezBuilder::new();
    let ok = vec!["D"; 40].join("/") + "/F.TXT";
    builder.file(&ok, b"x");
    let mut store = AssetStore::new();
    store.mount(Source::RezBytes(builder.build())).unwrap();
    assert_eq!(store.open(&ok).unwrap(), b"x");
}

#[test]
fn logical_paths_cannot_escape() {
    let tmp = tempfile::tempdir().unwrap();
    put(tmp.path(), "outside.txt", b"secret");
    put(tmp.path(), "root/inside.txt", b"ok");
    let mut store = AssetStore::new();
    store.mount(Source::Dir(tmp.path().join("root"))).unwrap();
    assert_eq!(store.open("inside.txt").unwrap(), b"ok");
    for bad in [
        "../outside.txt",
        "a/../../outside.txt",
        "..\\outside.txt",
        "/etc/passwd",
        "\\etc\\passwd",
        "C:\\Windows\\win.ini",
        "c:outside.txt",
        "inside.txt\0",
        "",
    ] {
        let result = store.open(bad);
        assert!(
            matches!(result, Err(AssetError::Path { .. })),
            "{bad:?} gave {result:?}"
        );
    }
}

#[test]
fn links_never_leave_the_root() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = put(tmp.path(), "outside/secret.txt", b"secret");
    let root = tmp.path().join("root");
    put(&root, "real/file.txt", b"inside");
    symlink(&outside, root.join("escape.txt")).unwrap();
    symlink(tmp.path().join("outside"), root.join("escape_dir")).unwrap();
    symlink(&root, root.join("real/loop")).unwrap();
    symlink(root.join("real/file.txt"), root.join("alias.txt")).unwrap();
    symlink(root.join("missing.txt"), root.join("broken.txt")).unwrap();

    let mut store = AssetStore::new();
    store.mount(Source::Dir(root.clone())).unwrap();
    assert!(store.lookup("escape.txt").unwrap().is_none());
    assert!(store.folder("escape_dir").unwrap().is_none());
    assert!(store.folder("real/loop").unwrap().is_none());
    assert!(store.lookup("broken.txt").unwrap().is_none());
    // A link to a file inside the root is an ordinary file.
    assert_eq!(store.open("alias.txt").unwrap(), b"inside");
    let kinds: Vec<NoteKind> = store.notes().iter().map(|n| n.kind).collect();
    assert!(kinds.contains(&NoteKind::LinkEscape));
    assert!(kinds.contains(&NoteKind::LinkedFolderSkipped));
    assert!(kinds.contains(&NoteKind::BrokenLink));

    // A file swapped for an outside link after mounting is refused on read.
    fs::remove_file(root.join("real/file.txt")).unwrap();
    symlink(&outside, root.join("real/file.txt")).unwrap();
    assert!(matches!(
        store.open("real/file.txt"),
        Err(AssetError::Path { .. })
    ));
}

#[test]
fn names_that_are_not_single_byte_are_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    put(tmp.path(), "GOOD.TXT", b"ok");
    put(tmp.path(), "caf\u{e9}.txt", b"latin-1 is fine");
    put(tmp.path(), "euro\u{20ac}.txt", b"skipped");
    let raw = std::ffi::OsStr::from_bytes(b"bad\xff\xfe.txt");
    fs::write(tmp.path().join(raw), b"skipped").unwrap();
    let mut store = AssetStore::new();
    store.mount(Source::Dir(tmp.path().into())).unwrap();
    assert_eq!(store.open("good.txt").unwrap(), b"ok");
    assert_eq!(store.open("CAF\u{e9}.TXT").unwrap(), b"latin-1 is fine");
    assert_eq!(store.root().count().0, 2);
    let skipped = store
        .notes()
        .iter()
        .filter(|n| n.kind == NoteKind::UnencodableName)
        .count();
    assert_eq!(skipped, 2);
}

#[test]
fn case_duplicates_keep_the_first() {
    let tmp = tempfile::tempdir().unwrap();
    put(tmp.path(), "A.TXT", b"upper");
    put(tmp.path(), "a.txt", b"lower");
    put(tmp.path(), "DIR/ONE.TXT", b"1");
    put(tmp.path(), "dir/TWO.TXT", b"2");
    let mut store = AssetStore::new();
    store.mount(Source::Dir(tmp.path().into())).unwrap();
    assert_eq!(store.open("a.txt").unwrap(), b"upper");
    // Folders that differ only by case merge.
    assert_eq!(store.open("dir/one.txt").unwrap(), b"1");
    assert_eq!(store.open("dir/two.txt").unwrap(), b"2");
}

#[test]
fn custom_level_folders_shadow_whole_folders() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("base");
    put(&base, "GAME/A.TXT", b"base");
    put(&base, "LEVEL1/B.TXT", b"base");
    let custom = tmp.path().join("custom");
    let wwd = put(&custom, "Mine.WWD", b"level bytes");
    put(&custom, "mine/GAME/C.TXT", b"custom");
    put(&custom, "mine/LEVEL/D.TXT", b"custom");

    let mut store = AssetStore::new();
    store.mount(Source::Dir(base)).unwrap();
    store.mount(Source::CustomLevel(wwd.clone())).unwrap();
    assert!(store.lookup("GAME/A.TXT").unwrap().is_none());
    assert_eq!(store.open("GAME/C.TXT").unwrap(), b"custom");
    assert_eq!(store.open("LEVEL1/B.TXT").unwrap(), b"base");
    assert_eq!(store.open("LEVEL/D.TXT").unwrap(), b"custom");
    let level = store.custom_level().unwrap();
    assert_eq!(level.name, "Mine");
    assert_eq!(level.wwd_path, wwd);
    assert!(level.folder.is_some());
    assert!(store.notes().iter().any(|n| n.kind == NoteKind::Shadowed));
}

#[test]
fn custom_level_without_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let wwd = put(tmp.path(), "Plain.WWD", b"x");
    let mut store = AssetStore::new();
    store.mount(Source::CustomLevel(wwd)).unwrap();
    assert!(store.custom_level().unwrap().folder.is_none());
    assert!(
        AssetStore::new()
            .mount(Source::CustomLevel(tmp.path().join("Missing.WWD")))
            .is_err()
    );
}

#[test]
fn install_layout_and_custom_level_paths() {
    let tmp = tempfile::tempdir().unwrap();
    common::install(tmp.path());
    put(tmp.path(), "Secret.WWD", b"outside the custom folder");
    let install = Install::open(tmp.path()).unwrap();
    assert!(install.archive().is_none());
    assert!(install.assets_dir().is_some());
    assert_eq!(install.custom_levels().unwrap().len(), 3);
    assert!(install.custom_level_path("my level.wwd").is_ok());
    assert!(install.custom_level_path("Custom/My Level.WWD").is_ok());
    assert!(install.custom_level_path("custom\\MY LEVEL.wwd").is_ok());
    for bad in [
        "../Secret.WWD",
        "Custom/../Secret.WWD",
        "/etc/passwd",
        "C:\\Secret.WWD",
        "My Level/TILES/ACTION/001.PID",
        "Missing.WWD",
        "",
    ] {
        assert!(install.custom_level_path(bad).is_err(), "{bad:?} accepted");
    }
    // A custom level file that links out of the folder is refused.
    symlink(
        tmp.path().join("Secret.WWD"),
        tmp.path().join("Custom/Link.WWD"),
    )
    .unwrap();
    assert!(install.custom_level_path("Link.WWD").is_err());

    let empty = tempfile::tempdir().unwrap();
    assert!(Install::open(empty.path()).is_err());
}

#[test]
fn install_with_archive_and_folder() {
    let tmp = tempfile::tempdir().unwrap();
    put(
        tmp.path(),
        "claw.rez",
        &RezBuilder::new()
            .file("GAME/A.TXT", b"archive")
            .file("GAME/B.TXT", b"archive")
            .build(),
    );
    put(tmp.path(), "ASSETS/GAME/B.TXT", b"folder");
    let install = Install::open(tmp.path()).unwrap();
    assert!(install.archive().is_some());
    let store = install.store().unwrap();
    assert_eq!(store.open("GAME/A.TXT").unwrap(), b"archive");
    assert_eq!(store.open("GAME/B.TXT").unwrap(), b"folder");
}

#[test]
fn install_parts_may_be_links() {
    let tmp = tempfile::tempdir().unwrap();
    let library = tmp.path().join("library");
    put(
        &library,
        "claw.rez",
        &RezBuilder::new().file("GAME/A.TXT", b"archive").build(),
    );
    put(&library, "Assets/GAME/B.TXT", b"folder");
    put(tmp.path(), "secret.txt", b"secret");
    symlink(
        tmp.path().join("secret.txt"),
        library.join("Assets/GAME/ESCAPE.TXT"),
    )
    .unwrap();
    let root = tmp.path().join("install");
    fs::create_dir_all(&root).unwrap();
    symlink(library.join("claw.rez"), root.join("CLAW.REZ")).unwrap();
    symlink(library.join("Assets"), root.join("Assets")).unwrap();

    let install = Install::open(&root).unwrap();
    assert!(install.archive().is_some());
    assert!(install.assets_dir().is_some());
    let store = install.store().unwrap();
    assert_eq!(store.open("GAME/A.TXT").unwrap(), b"archive");
    assert_eq!(store.open("GAME/B.TXT").unwrap(), b"folder");
    // Inside the linked folder, links that leave it are still refused.
    assert!(store.lookup("GAME/ESCAPE.TXT").unwrap().is_none());

    // Broken links count as missing.
    let broken = tmp.path().join("broken");
    fs::create_dir_all(&broken).unwrap();
    symlink(tmp.path().join("nothing"), broken.join("Assets")).unwrap();
    assert!(Install::open(&broken).is_err());
}
