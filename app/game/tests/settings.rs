//! Config write-back: line-preserving, atomic, merging and refusing what it
//! cannot understand. Every test works in a temporary folder that stands in
//! for HOME and the XDG folders.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use claw_game::keymap::{Action, Command, Keymap, PadButton};
use claw_game::saves::{SaveRecord, SaveStore};
use claw_game::settings::{
    Args, ConfigFile, FolderRules, Settings, UserDirs, WriteHooks, WriteStep, backup_path,
    write_atomic,
};
use claw_sim::{Buttons, Carry, SavePos};

/// A realistic hand-edited config: comments, alignment, quotes, blank
/// lines and a duplicate key.
const REALISTIC: &str = "\
# rusty-claw settings
#   edited by hand

install = \"/games/claw # retail\"   # my install
level = 3
res = \"2560x1440\"         # window size
   fullscreen=false
aspect = \"4:3\"            # fill, centered or 4:3
scale = integer

# audio
audio = on                # on or off
vsync = true
vsync = false             # the last one wins
";

/// HOME, XDG_CONFIG_HOME and XDG_DATA_HOME inside `root`.
fn user_dirs(root: &Path) -> UserDirs {
    let home = root.join("home");
    let config = root.join("xdg-config");
    let data = root.join("xdg-data");
    UserDirs::from_lookup(move |k| match k {
        "HOME" => Some(OsString::from(&home)),
        "XDG_CONFIG_HOME" => Some(OsString::from(&config)),
        "XDG_DATA_HOME" => Some(OsString::from(&data)),
        _ => None,
    })
}

fn config_in(root: &Path, text: &str) -> PathBuf {
    let path = user_dirs(root).config_file().unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

/// Every file under `root`, relative.
fn files(root: &Path) -> BTreeSet<PathBuf> {
    let mut out = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(p.strip_prefix(root).unwrap().to_path_buf());
            }
        }
    }
    out
}

#[test]
fn user_dirs_follow_the_xdg_rules() {
    let d = UserDirs::from_lookup(|k| match k {
        "HOME" => Some("/h".into()),
        "XDG_CONFIG_HOME" => Some("relative/ignored".into()),
        _ => None,
    });
    assert_eq!(
        d.config_file(),
        Some(PathBuf::from("/h/.config/rusty-claw/config.toml"))
    );
    assert_eq!(
        d.saves_dir(),
        Some(PathBuf::from("/h/.local/share/rusty-claw/saves/default"))
    );
    let d = UserDirs::from_lookup(|k| match k {
        "XDG_CONFIG_HOME" => Some("/c".into()),
        "XDG_DATA_HOME" => Some("/d".into()),
        _ => None,
    });
    assert_eq!(
        d.config_file(),
        Some(PathBuf::from("/c/rusty-claw/config.toml"))
    );
    assert_eq!(
        d.saves_dir(),
        Some(PathBuf::from("/d/rusty-claw/saves/default"))
    );
    assert_eq!(UserDirs::from_lookup(|_| None).config_file(), None);
}

/// A lookup of the variables `vars` and no other.
fn vars(vars: Vec<(&'static str, OsString)>) -> impl Fn(&str) -> Option<OsString> {
    move |k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone())
}

#[test]
fn user_dirs_take_the_windows_folders_when_xdg_is_unset() {
    // Absolute on every system.
    let base = std::env::temp_dir().join("user-dirs");
    let at = |p: &str| base.join(p).into_os_string();
    let windows = |v| UserDirs::from_lookup_with(FolderRules::Windows, vars(v));
    let d = windows(vec![
        ("HOME", at("home")),
        ("APPDATA", at("roaming")),
        ("LOCALAPPDATA", at("local")),
    ]);
    assert_eq!(
        d.config_file(),
        Some(base.join("roaming").join("rusty-claw").join("config.toml"))
    );
    assert_eq!(
        d.saves_dir(),
        Some(
            base.join("local")
                .join("rusty-claw")
                .join("saves")
                .join("default")
        )
    );
    // The XDG variables come first.
    let d = windows(vec![
        ("APPDATA", at("roaming")),
        ("LOCALAPPDATA", at("local")),
        ("XDG_CONFIG_HOME", at("c")),
        ("XDG_DATA_HOME", at("d")),
    ]);
    assert_eq!(d.config_home, Some(base.join("c")));
    assert_eq!(d.data_home, Some(base.join("d")));
    // An empty or relative folder variable falls back to HOME's places.
    let d = windows(vec![
        ("HOME", at("home")),
        ("APPDATA", "".into()),
        ("LOCALAPPDATA", "relative".into()),
    ]);
    assert_eq!(d.config_home, Some(base.join("home").join(".config")));
    assert_eq!(d.data_home, Some(base.join("home").join(".local/share")));
    assert_eq!(windows(Vec::new()), UserDirs::default());
    // The XDG rules never look at the Windows folders.
    let d = UserDirs::from_lookup_with(
        FolderRules::Xdg,
        vars(vec![
            ("HOME", at("home")),
            ("APPDATA", at("roaming")),
            ("LOCALAPPDATA", at("local")),
        ]),
    );
    assert_eq!(d.config_home, Some(base.join("home").join(".config")));
    assert_eq!(d.data_home, Some(base.join("home").join(".local/share")));
    // The game follows the rules of the system it was built for.
    assert_eq!(FolderRules::NATIVE == FolderRules::Windows, cfg!(windows));
}

#[test]
fn update_keeps_every_other_line_byte_for_byte() {
    let root = tempfile::tempdir().unwrap();
    let path = config_in(root.path(), REALISTIC);
    ConfigFile::update_in_place(
        &path,
        &[
            ("res", "1920x1080"),
            ("fullscreen", "true"),
            ("vsync", "true"),
            ("audio", "off"),
            ("sound_volume", "65"),
            ("key_jump", "KeyX, Space"),
        ],
    )
    .unwrap();
    let want = "\
# rusty-claw settings
#   edited by hand

install = \"/games/claw # retail\"   # my install
level = 3
res = \"1920x1080\"         # window size
   fullscreen=true
aspect = \"4:3\"            # fill, centered or 4:3
scale = integer

# audio
audio = off                # on or off
vsync = true
vsync = true             # the last one wins
sound_volume = 65
key_jump = \"KeyX, Space\"
";
    assert_eq!(fs::read_to_string(&path).unwrap(), want);
    assert_eq!(fs::read_to_string(backup_path(&path)).unwrap(), REALISTIC);
    // The result loads, with the new values.
    let file = ConfigFile::load(&path).unwrap();
    let s = Settings::resolve(&Args::default(), Some(&file)).unwrap();
    assert_eq!(s.res, (1920, 1080));
    assert!(s.fullscreen && s.vsync && !s.audio);
    assert_eq!(s.sound_volume, 65);
    assert_eq!(
        s.keymap.keys(Action::Button(Buttons::JUMP)),
        vec![claw_game::keymap::parse_key("KeyX").unwrap(), winit_space()]
    );
    // Updating to the same values changes nothing.
    ConfigFile::update_in_place(&path, &[("res", "1920x1080")]).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), want);
}

fn winit_space() -> winit::keyboard::KeyCode {
    winit::keyboard::KeyCode::Space
}

#[test]
fn line_endings_and_a_missing_last_newline_are_kept() {
    let root = tempfile::tempdir().unwrap();
    let path = config_in(root.path(), "# c\r\nres = 640x480\r\nvsync = on");
    ConfigFile::update_in_place(&path, &[("res", "800x600"), ("gamepad", "off")]).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "# c\r\nres = 800x600\r\nvsync = on\r\ngamepad = off\r\n"
    );
    let path = config_in(root.path(), "vsync = on");
    ConfigFile::update_in_place(&path, &[("vsync", "off")]).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "vsync = off");
}

#[test]
fn a_missing_file_and_folder_are_created() {
    let root = tempfile::tempdir().unwrap();
    let path = user_dirs(root.path()).config_file().unwrap();
    ConfigFile::update_in_place(&path, &[("music_volume", "30"), ("soundfont", "/a b.sf2")])
        .unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "music_volume = 30\nsoundfont = \"/a b.sf2\"\n"
    );
    assert!(!backup_path(&path).exists());
}

#[test]
fn every_interrupted_write_leaves_a_whole_file() {
    let root = tempfile::tempdir().unwrap();
    let path = config_in(root.path(), REALISTIC);
    let dir = path.parent().unwrap().to_path_buf();
    for step in [
        WriteStep::Temp,
        WriteStep::Data,
        WriteStep::Flush,
        WriteStep::Backup,
        WriteStep::Rename,
        WriteStep::SyncDir,
    ] {
        fs::write(&path, REALISTIC).unwrap();
        let mut hooks = WriteHooks {
            fail_at: Some(step),
            ..WriteHooks::default()
        };
        let r = ConfigFile::update_in_place_with(&path, &[("res", "800x600")], &mut hooks);
        let now = fs::read_to_string(&path).unwrap();
        if step == WriteStep::SyncDir {
            // Failing after the rename: the new file is whole and in place,
            // so the write counts as done (with a warning).
            assert!(r.is_ok(), "{step:?}: {r:?}");
            assert!(now.contains("res = \"800x600\""), "{step:?}");
        } else {
            assert!(r.is_err(), "{step:?}");
            assert_eq!(now, REALISTIC, "{step:?}");
        }
        // No temporary file is left behind.
        let names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names.iter().all(|n| !n.contains(".tmp-")),
            "{step:?}: {names:?}"
        );
    }
}

#[test]
fn a_temporary_file_left_by_a_crash_does_no_harm() {
    let root = tempfile::tempdir().unwrap();
    let path = config_in(root.path(), REALISTIC);
    let stray = path.with_file_name(".config.toml.tmp-1-0");
    fs::write(&stray, "garbage = 1\n\0\0").unwrap();
    ConfigFile::update_in_place(&path, &[("scale", "fit")]).unwrap();
    let now = fs::read_to_string(&path).unwrap();
    assert!(now.contains("scale = fit\n"));
    assert_eq!(now.replace("scale = fit", "scale = integer"), REALISTIC);
    // Not ours to remove: it is left as it was.
    assert_eq!(fs::read(&stray).unwrap(), b"garbage = 1\n\0\0");
}

#[test]
fn a_change_made_on_disk_meanwhile_is_merged() {
    let root = tempfile::tempdir().unwrap();
    let path = config_in(root.path(), REALISTIC);
    let mut calls = 0;
    let mut edit = |p: &Path| {
        calls += 1;
        if calls == 1 {
            // Someone edits the file between our read and our rename.
            let text = fs::read_to_string(p).unwrap();
            fs::write(p, text.replace("level = 3", "level = 7  # changed")).unwrap();
        }
    };
    ConfigFile::update_in_place_with(
        &path,
        &[("res", "800x600")],
        &mut WriteHooks {
            before_commit: Some(&mut edit),
            ..WriteHooks::default()
        },
    )
    .unwrap();
    assert_eq!(calls, 2);
    let now = fs::read_to_string(&path).unwrap();
    assert!(now.contains("level = 7  # changed\n"), "{now}");
    assert!(
        now.contains("res = \"800x600\"         # window size\n"),
        "{now}"
    );
    // The backup is the file as it was just before the write.
    assert!(
        fs::read_to_string(backup_path(&path))
            .unwrap()
            .contains("level = 7")
    );
}

#[test]
fn a_file_that_does_not_parse_is_never_written() {
    let root = tempfile::tempdir().unwrap();
    for bad in [
        "install = /i\nzoom = 3\n".as_bytes(),
        b"just words\n",
        b"res = big\n",
        b"key_jump = NoSuchKey\n",
        b"key_jump = Escape\n",
        b"install = \"/i\xff\"\n",
    ] {
        let path = config_in(root.path(), "");
        fs::write(&path, bad).unwrap();
        let _ = fs::remove_file(backup_path(&path));
        let r = ConfigFile::update_in_place(&path, &[("vsync", "off")]);
        assert!(r.is_err(), "{:?}", String::from_utf8_lossy(bad));
        assert_eq!(fs::read(&path).unwrap(), bad);
        assert!(!backup_path(&path).exists());
    }
    // Values that would not load, or cannot be written, are refused too.
    let path = config_in(root.path(), REALISTIC);
    for (k, v) in [
        ("sound_volume", "200"),
        ("pad_dead_zone", "2"),
        ("install", "a\"b"),
        ("res", "1x1\nzoom = 2"),
        ("zoom", "2"),
        ("fps_cap", "0"),
    ] {
        assert!(
            ConfigFile::update_in_place(&path, &[(k, v)]).is_err(),
            "{k}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), REALISTIC);
    }
}

#[cfg(unix)]
#[test]
#[cfg(unix)]
fn links_and_permissions_are_kept() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = tempfile::tempdir().unwrap();
    let real = root.path().join("dotfiles").join("claw.toml");
    fs::create_dir_all(real.parent().unwrap()).unwrap();
    fs::write(&real, "vsync = on\n").unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
    let path = user_dirs(root.path()).config_file().unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    symlink(&real, &path).unwrap();
    ConfigFile::update_in_place(&path, &[("vsync", "off")]).unwrap();
    assert!(
        fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_to_string(&real).unwrap(), "vsync = off\n");
    assert_eq!(
        fs::metadata(&real).unwrap().permissions().mode() & 0o777,
        0o600
    );
    // The backup of a private file is private too.
    let bak = claw_game::settings::backup_path(&real);
    assert_eq!(fs::read_to_string(&bak).unwrap(), "vsync = on\n");
    assert_eq!(
        fs::metadata(&bak).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn new_keys_reach_audio_and_input() {
    let file = ConfigFile::parse(
        "install = /i\nsound_volume = 40\nmusic_volume = 0\nvoice = off\nambient = false\n\
         soundfont = \"/sf/gm.sf2\"\ngamepad = off\npad_dead_zone = 0.5\n\
         key_attack = KeyX\nkey_lift = ControlLeft\npad_jump = West\npad_projectile = South\n",
        "cfg",
    )
    .unwrap();
    let s = Settings::resolve(&Args::default(), Some(&file)).unwrap();
    let a = s.audio_settings();
    assert_eq!((a.sound_volume, a.music_volume), (40, 0));
    assert!(!a.voice && !a.ambient && a.enabled);
    assert_eq!(a.soundfont, Some(PathBuf::from("/sf/gm.sf2")));
    assert!(!s.pad.enabled);
    assert_eq!(s.pad.dead_zone, 0.5);
    let attack = Action::Button(Buttons::ATTACK);
    let k = |n| claw_game::keymap::parse_key(n).unwrap();
    assert_eq!(s.keymap.keys(attack), vec![k("KeyX")]);
    assert_eq!(
        s.keymap.keys(Action::Button(Buttons::LIFT)),
        vec![k("ControlLeft")]
    );
    assert_eq!(
        s.pad.map.action(PadButton::West),
        Some(Action::Button(Buttons::JUMP))
    );
    assert_eq!(
        s.pad.map.action(PadButton::South),
        Some(Action::Button(Buttons::PROJECTILE))
    );
    // Defaults when nothing is set.
    let s = Settings::resolve(
        &Args::try_parse_from(["x", "--install", "/i"]).unwrap(),
        None,
    )
    .unwrap();
    assert_eq!((s.sound_volume, s.music_volume), (80, 50));
    assert!(s.voice && s.ambient && s.pad.enabled);
    assert_eq!(s.pad.dead_zone, 0.34);
    assert_eq!(s.keymap, Keymap::default());
    for bad in [
        "sound_volume = 101",
        "music_volume = -1",
        "voice = maybe",
        "gamepad = 2",
        "pad_dead_zone = 0.01",
        "pad_jump = Z",
        "key_nothing = KeyA",
        "key_jump = KeyA\nkey_attack = KeyA",
    ] {
        let file = ConfigFile::parse(&format!("install = /i\n{bad}\n"), "cfg");
        let r = file.and_then(|f| Settings::resolve(&Args::default(), Some(&f)));
        assert!(r.is_err(), "{bad}");
    }
    // The quit command keeps Escape unless it is moved too.
    let file =
        ConfigFile::parse("install = /i\nkey_pause = Escape\nkey_quit = F12\n", "cfg").unwrap();
    let s = Settings::resolve(&Args::default(), Some(&file)).unwrap();
    assert_eq!(
        s.keymap.action(k("Escape")),
        Some(Action::Command(Command::Pause))
    );
}

#[test]
fn command_line_values_are_never_written_back() {
    let root = tempfile::tempdir().unwrap();
    let path = config_in(root.path(), "install = /i\nres = 640x480\naudio = on\n");
    let args = Args::try_parse_from([
        "rusty-claw",
        "--config",
        path.to_str().unwrap(),
        "--res",
        "3840x2160",
        "--no-audio",
        "--fullscreen",
    ])
    .unwrap();
    let mut s = Settings::from_args(&args).unwrap();
    assert_eq!(s.config_path.as_deref(), Some(path.as_path()));
    assert_eq!(s.res, (3840, 2160));
    // The options screen changed the volume; the run's overrides stay
    // out of the file.
    s.sound_volume = 30;
    let written = s
        .write_back(&["res", "audio", "fullscreen", "sound_volume"])
        .unwrap();
    assert_eq!(written, vec!["sound_volume".to_string()]);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "install = /i\nres = 640x480\naudio = on\nsound_volume = 30\n"
    );
    // A value the player changes in the game is written, even if the
    // command line set it at start-up.
    s.res = (1920, 1080);
    s.changed_in_game("res");
    s.write_back(&["res"]).unwrap();
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("res = 1920x1080\n")
    );
    // Bindings are written by name.
    let mut s = s.clone();
    s.keymap
        .rebind(
            Action::Button(Buttons::JUMP),
            None,
            claw_game::keymap::parse_key("KeyW").unwrap(),
        )
        .unwrap();
    s.write_back(&["key_jump", "pad_pause", "gamepad", "pad_dead_zone"])
        .unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("key_jump = \"Space, KeyW\"\n"), "{text}");
    assert!(text.contains("pad_pause = Start\n"), "{text}");
    assert!(
        text.contains("gamepad = on\npad_dead_zone = 0.34\n"),
        "{text}"
    );
    let reread = Settings::from_args(
        &Args::try_parse_from(["rusty-claw", "--config", path.to_str().unwrap()]).unwrap(),
    )
    .unwrap();
    assert_eq!(reread.keymap, s.keymap);
    assert_eq!(reread.res, (1920, 1080));
}

#[test]
fn nothing_is_written_outside_the_config_and_saves_folders() {
    let root = tempfile::tempdir().unwrap();
    let dirs = user_dirs(root.path());
    fs::create_dir_all(root.path().join("home")).unwrap();
    let install = root.path().join("install");
    fs::create_dir_all(&install).unwrap();
    fs::write(install.join("CLAW.USR"), b"original profile").unwrap();
    let before = files(root.path());

    let mut s = Settings::new(&install, Default::default());
    s.config_path = dirs.config_file();
    s.write_back(&["install", "res", "sound_volume", "key_jump"])
        .unwrap();
    s.write_back(&["res"]).unwrap();
    let mut store = SaveStore::open_default(&dirs, Some(&install)).unwrap();
    let rec = SaveRecord::new(
        "LEVEL1",
        SavePos {
            slot: 1,
            x: 100,
            y: 200,
        },
        Carry::default(),
        [0; 9],
    );
    store.write(&rec).unwrap();
    store.write(&rec).unwrap();
    assert_eq!(store.read("LEVEL1", 1).unwrap(), Some(rec));
    drop(store);

    let config_dir = dirs.config_file().unwrap().parent().unwrap().to_path_buf();
    let saves_dir = dirs.saves_dir().unwrap();
    for f in files(root.path()).difference(&before) {
        let abs = root.path().join(f);
        assert!(
            abs.starts_with(&config_dir) || abs.starts_with(&saves_dir),
            "written outside: {}",
            f.display()
        );
    }
    assert_eq!(
        fs::read(install.join("CLAW.USR")).unwrap(),
        b"original profile"
    );
    // A data folder inside the install is refused.
    let inside = UserDirs {
        config_home: None,
        data_home: Some(install.join("data")),
    };
    assert!(SaveStore::open_default(&inside, Some(&install)).is_err());
    assert!(!install.join("data").exists());
}

#[test]
fn atomic_writes_keep_one_backup() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("f.txt");
    for (i, text) in ["one", "two", "three"].iter().enumerate() {
        write_atomic(&path, text.as_bytes(), &mut WriteHooks::default()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), *text);
        if i > 0 {
            assert_eq!(
                fs::read_to_string(backup_path(&path)).unwrap(),
                ["one", "two"][i - 1]
            );
        }
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}

#[test]
fn the_status_bar_placement_is_a_setting() {
    let parse = |text: &str| ConfigFile::parse(text, "test").and_then(|f| f.check().map(|_| f));
    let resolve = |text: &str| {
        let file = parse(text).unwrap();
        Settings::resolve(
            &Args::try_parse_from(["x", "--install", "/i"]).unwrap(),
            Some(&file),
        )
        .unwrap()
    };
    assert!(!resolve("").status_bar_centred, "at the edges by default");
    assert!(resolve("status_bar = centred\n").status_bar_centred);
    assert!(resolve("status_bar = \"centered\"\n").status_bar_centred);
    assert!(!resolve("status_bar = edges\n").status_bar_centred);
    assert!(parse("status_bar = middle\n").is_err());
    let mut s = resolve("");
    assert_eq!(s.config_value("status_bar").as_deref(), Some("edges"));
    s.status_bar_centred = true;
    assert_eq!(s.config_value("status_bar").as_deref(), Some("centred"));
}

#[test]
fn the_mid_air_hit_fix_is_a_setting_pinned_for_replays_and_benchmarks() {
    let resolve = |text: &str, args: &[&str]| {
        let file = ConfigFile::parse(text, "test").unwrap();
        file.check().unwrap();
        let mut argv = vec!["x", "--install", "/i"];
        argv.extend_from_slice(args);
        Settings::resolve(&Args::try_parse_from(argv).unwrap(), Some(&file)).unwrap()
    };
    let fix = |s: &Settings| s.sim_config((1280, 960)).tuning.behaviour.fix_double_jump;
    // Play: off by default, from the file, the command line winning.
    let s = resolve("", &[]);
    assert!(!s.mid_air_hit_fix && !fix(&s), "the original by default");
    assert_eq!(s.config_value("mid_air_hit_fix").as_deref(), Some("off"));
    let s = resolve("mid_air_hit_fix = on\n", &[]);
    assert!(s.mid_air_hit_fix && fix(&s));
    assert_eq!(s.config_value("mid_air_hit_fix").as_deref(), Some("on"));
    assert!(!resolve("mid_air_hit_fix = on\n", &["--mid-air-hit-fix", "off"]).mid_air_hit_fix);
    let s = resolve("", &["--mid-air-hit-fix", "on"]);
    assert!(s.mid_air_hit_fix);
    assert!(s.from_command_line.contains("mid_air_hit_fix"));
    assert!(
        ConfigFile::parse("mid_air_hit_fix = maybe\n", "test")
            .unwrap()
            .check()
            .is_err()
    );
    // The widened view keeps it too.
    let s = resolve("mid_air_hit_fix = on\n", &["--widen-activation"]);
    assert!(fix(&s));
    // Replays and benchmarks ignore the file; the command line still counts.
    for mode in [
        &["--replay", "r.csv"][..],
        &["--replay", "r.csv", "--trace", "t.csv"],
        &["--bench", "5"],
        &["--bench", "5", "--headless"],
    ] {
        let s = resolve("mid_air_hit_fix = on\n", mode);
        assert!(!s.mid_air_hit_fix && !fix(&s), "{mode:?}");
        let mut on: Vec<&str> = mode.to_vec();
        on.extend(["--mid-air-hit-fix", "on"]);
        assert!(resolve("", &on).mid_air_hit_fix, "{mode:?}");
    }
    // A replay follows its recording unless the command line says.
    let mut s = resolve("mid_air_hit_fix = on\n", &["--replay", "r.csv"]);
    s.follow_script(None);
    assert!(!s.mid_air_hit_fix);
    s.follow_script(Some(true));
    assert!(s.mid_air_hit_fix && fix(&s));
    let mut s = resolve("", &["--replay", "r.csv", "--mid-air-hit-fix", "off"]);
    s.follow_script(Some(true));
    assert!(!s.mid_air_hit_fix);
}

#[test]
fn recordings_carry_the_mid_air_hit_fix() {
    use claw_game::session::{behaviour_comment, recording_csv, script_behaviour};
    use claw_sim::{InputScript, TickInput};
    let script = InputScript {
        ticks: vec![
            TickInput {
                held: Buttons(1),
                pressed: Buttons(1),
            },
            TickInput::default(),
        ],
        ..InputScript::default()
    };
    for fix in [false, true] {
        let csv = recording_csv(&script, fix);
        assert!(csv.starts_with(&behaviour_comment(fix)), "{csv}");
        assert_eq!(script_behaviour(&csv), Some(fix));
        assert_eq!(
            InputScript::parse(&csv).unwrap(),
            script,
            "still a plain script"
        );
    }
    assert_eq!(behaviour_comment(true), "# mid_air_hit_fix = on");
    assert_eq!(script_behaviour(&script.to_csv()), None);
    assert_eq!(
        script_behaviour(
            "#mid_air_hit_fix=off\n# mid_air_hit_fix = on\ntick,held_mask,pressed_mask\n"
        ),
        Some(true),
        "the last line counts"
    );
    assert_eq!(script_behaviour("# mid_air_hit_fix = sometimes\n"), None);
    assert_eq!(script_behaviour("# a note = on\n"), None);
}

#[test]
fn the_smooth_rope_swing_is_a_drawing_setting_on_by_default() {
    let parse = |text: &str| ConfigFile::parse(text, "test").and_then(|f| f.check().map(|_| f));
    let resolve = |text: &str, cli: &[&str]| {
        let file = parse(text).unwrap();
        let args = ["x", "--install", "/i"].iter().chain(cli);
        Settings::resolve(&Args::try_parse_from(args).unwrap(), Some(&file)).unwrap()
    };
    assert!(resolve("", &[]).smooth_rope, "on by default");
    assert!(!resolve("smooth_rope = off\n", &[]).smooth_rope);
    assert!(resolve("smooth_rope = on\n", &[]).smooth_rope);
    assert!(parse("smooth_rope = sometimes\n").is_err());
    // The command line wins and is not written back.
    let s = resolve("smooth_rope = on\n", &["--smooth-rope", "off"]);
    assert!(!s.smooth_rope);
    assert!(s.from_command_line.contains("smooth_rope"));
    assert!(resolve("smooth_rope = off\n", &["--smooth-rope", "on"]).smooth_rope);
    let mut s = resolve("", &[]);
    assert_eq!(s.config_value("smooth_rope").as_deref(), Some("on"));
    s.smooth_rope = false;
    assert_eq!(s.config_value("smooth_rope").as_deref(), Some("off"));
    // Saved with the other settings and read back on the next run.
    let root = tempfile::tempdir().unwrap();
    let path = config_in(root.path(), REALISTIC);
    let file = ConfigFile::load(&path).unwrap();
    let mut s = Settings::resolve(
        &Args::try_parse_from(["x", "--install", "/i"]).unwrap(),
        Some(&file),
    )
    .unwrap();
    s.config_path = Some(path.clone());
    s.smooth_rope = false;
    assert_eq!(s.write_back(&["smooth_rope"]).unwrap(), vec!["smooth_rope"]);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.starts_with(REALISTIC), "{text}");
    assert!(text.lines().any(|l| l == "smooth_rope = off"), "{text}");
    let again = Settings::resolve(
        &Args::try_parse_from(["x", "--install", "/i"]).unwrap(),
        Some(&ConfigFile::load(&path).unwrap()),
    )
    .unwrap();
    assert!(!again.smooth_rope);
    // Drawing only: the game is set up the same either way.
    assert_eq!(
        again.sim_config((1280, 960)),
        resolve("", &[]).sim_config((1280, 960))
    );
}

#[test]
fn the_front_layer_is_a_setting_that_survives_a_reload() {
    let parse = |text: &str| ConfigFile::parse(text, "test").and_then(|f| f.check().map(|_| f));
    let resolve = |text: &str| {
        let file = parse(text).unwrap();
        Settings::resolve(
            &Args::try_parse_from(["x", "--install", "/i"]).unwrap(),
            Some(&file),
        )
        .unwrap()
    };
    assert!(resolve("").front_layer, "on by default");
    assert!(!resolve("front_layer = off\n").front_layer);
    assert!(resolve("front_layer = on\n").front_layer);
    assert!(parse("front_layer = sometimes\n").is_err());
    let mut s = resolve("");
    assert_eq!(s.config_value("front_layer").as_deref(), Some("on"));
    s.front_layer = false;
    assert_eq!(s.config_value("front_layer").as_deref(), Some("off"));
    // Written back into a hand-edited file, keeping its other lines, and
    // read back on the next run.
    let root = tempfile::tempdir().unwrap();
    let path = config_in(root.path(), REALISTIC);
    let file = ConfigFile::load(&path).unwrap();
    let mut s = Settings::resolve(
        &Args::try_parse_from(["x", "--install", "/i"]).unwrap(),
        Some(&file),
    )
    .unwrap();
    s.config_path = Some(path.clone());
    s.front_layer = false;
    assert_eq!(s.write_back(&["front_layer"]).unwrap(), vec!["front_layer"]);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.starts_with(REALISTIC), "{text}");
    assert!(text.lines().any(|l| l == "front_layer = off"), "{text}");
    let again = Settings::resolve(
        &Args::try_parse_from(["x", "--install", "/i"]).unwrap(),
        Some(&ConfigFile::load(&path).unwrap()),
    )
    .unwrap();
    assert!(!again.front_layer);
    // The front layer changes drawing only.
    assert_eq!(
        again.sim_config((1280, 960)),
        resolve("").sim_config((1280, 960))
    );
}

#[test]
fn a_replay_follows_the_tuning_and_seed_of_its_script() {
    use claw_sim::input::ScriptTuning;
    use claw_sim::{InputScript, Tuning};
    let args = Args::try_parse_from(["x", "--install", "/i", "--replay", "r.csv"]).unwrap();
    let mut s = Settings::resolve(&args, None).unwrap();
    let plain = s.sim_config((1280, 960));
    assert_eq!(plain.tuning, Tuning::default());
    let script =
        InputScript::parse("# tuning original\n# seed 1234\ntick,held_mask,pressed_mask\n0,0,0\n")
            .unwrap();
    s.follow_header(&script.header);
    s.follow_script(Some(true));
    let cfg = s.sim_config((1280, 960));
    let mut want = Tuning::original_speeds();
    want.behaviour.fix_double_jump = true;
    assert_eq!(cfg.tuning, want);
    assert_eq!(cfg.seed, 1234);
    assert_eq!(s.tuning, Some(ScriptTuning::Original));
    // A script without the keys leaves them alone.
    let bare = InputScript::parse("tick,held_mask,pressed_mask\n0,0,0\n").unwrap();
    s.follow_header(&bare.header);
    assert_eq!(s.sim_config((1280, 960)).seed, 1234);
    let mut fresh = Settings::resolve(&args, None).unwrap();
    fresh.follow_header(&bare.header);
    assert_eq!(fresh.sim_config((1280, 960)), plain);
}

#[test]
fn the_play_test_aids_are_settings_off_by_default_and_off_for_replays() {
    use claw_sim::tuning::PlayAids;
    let resolve = |text: &str, args: &[&str]| {
        let file = ConfigFile::parse(text, "test").unwrap();
        file.check().unwrap();
        let mut argv = vec!["x", "--install", "/i"];
        argv.extend_from_slice(args);
        Settings::resolve(&Args::try_parse_from(argv).unwrap(), Some(&file)).unwrap()
    };
    let aids = |s: &Settings| s.sim_config((1280, 960)).aids;
    let both = PlayAids {
        invincible: true,
        infinite_lives: true,
    };
    // Off by default; from the file; written back as on or off.
    let s = resolve("", &[]);
    assert_eq!((s.aids, aids(&s)), (PlayAids::NONE, PlayAids::NONE));
    assert_eq!(s.config_value("invincible").as_deref(), Some("off"));
    assert_eq!(s.config_value("infinite_lives").as_deref(), Some("off"));
    let s = resolve("invincible = on\ninfinite_lives = on\n", &[]);
    assert_eq!(aids(&s), both);
    assert_eq!(s.config_value("invincible").as_deref(), Some("on"));
    assert!(
        ConfigFile::parse("invincible = maybe\n", "test")
            .unwrap()
            .check()
            .is_err()
    );
    // A flag turns an aid on for the run, and is not written back.
    let s = resolve("", &["--invincible"]);
    assert!(s.aids.invincible && !s.aids.infinite_lives);
    assert!(
        s.write_back_values(&["invincible", "infinite_lives"])
            == [("infinite_lives".to_string(), "off".to_string())]
    );
    let s = resolve("", &["--infinite-lives", "--invincible"]);
    assert_eq!(aids(&s), both);
    assert!(
        s.write_back_values(&["invincible", "infinite_lives"])
            .is_empty()
    );
    // Replays and benchmarks ignore the file; the command line still counts.
    for mode in [
        &["--replay", "r.csv"][..],
        &["--replay", "r.csv", "--trace", "t.csv"],
        &["--bench", "5"],
        &["--bench", "5", "--headless"],
    ] {
        let s = resolve("invincible = on\ninfinite_lives = on\n", mode);
        assert_eq!(aids(&s), PlayAids::NONE, "{mode:?}");
        let mut on: Vec<&str> = mode.to_vec();
        on.push("--invincible");
        assert!(resolve("", &on).aids.invincible, "{mode:?}");
    }
    // A replay plays with the aids its header names, with those of the
    // command line.
    let header = |text: &str| claw_sim::InputScript::parse_with_aids(text).unwrap().header;
    let mut s = resolve("", &["--replay", "r.csv", "--infinite-lives"]);
    s.follow_header(&header("# invincible = on\n0,0,0\n"));
    assert_eq!(aids(&s), both);
    let mut s = resolve("invincible = on\n", &["--replay", "r.csv"]);
    s.follow_header(&header("0,0,0\n"));
    assert_eq!(aids(&s), PlayAids::NONE);
}
