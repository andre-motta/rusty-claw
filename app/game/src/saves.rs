//! Saved games: one file per retail level and save slot.
//!
//! Touching one of a level's two save points saves the game in slot 1 or
//! 2; slot 3 holds the progress a level was started with. A save holds
//! where Claw starts, the progress that carries between levels (score,
//! health, lives, ammunition, the selected weapon and the game's treasure
//! counts) and the level's treasure counts. Saves live in
//! `$XDG_DATA_HOME/rusty-claw/saves/default/` (by default
//! `~/.local/share/rusty-claw/saves/default/`), one folder per profile with
//! a single default profile, as text files in the config file's
//! `key = value` style:
//!
//! ```text
//! # rusty-claw saved game
//! version = 1
//! level = LEVEL1
//! slot = 1
//! x = 5410
//! y = 4032
//! score = 25000
//! health = 85
//! lives = 6
//! pistol = 12
//! magic = 5
//! dynamite = 3
//! weapon = pistol
//! treasure_game = 10, 2, 0, 0, 0, 0, 0, 0, 0
//! treasure_level = 10, 2, 0, 0, 0, 0, 0, 0, 0
//! ```
//!
//! Writes are atomic and keep the previous file as `<name>.bak`, with the
//! permissions of the file they replace. Files are read up to
//! [`MAX_SAVE_BYTES`]. A save whose contents are not usable (too large, not
//! text, damaged, holding another level or slot, or from a newer version of
//! the game) is renamed aside with a timestamp
//! (`LEVEL1-1.save.unreadable-<seconds>` or `.newer-<seconds>`) and
//! reported. A save that cannot be read at all (no permission, a disk
//! error, not a regular file) is reported once per run and left where it
//! is, so it still counts once it can be read again. Saves are never
//! deleted. A second copy of the game running at the same time sees the
//! saves but cannot write them. The store only ever touches its own folder.

use std::collections::HashSet;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use claw_frame::Weapon;
use claw_sim::player::stats::TREASURE_KINDS;
use claw_sim::{Carry, SavePos, StartState};

use crate::settings::{UserDirs, WriteHooks, read_bounded, write_atomic};

/// Version of the save format this game writes.
pub const SAVE_VERSION: u32 = 1;

/// Largest save file read, in bytes.
pub const MAX_SAVE_BYTES: u64 = 16 * 1024;

/// The profile every save belongs to.
pub const DEFAULT_PROFILE: &str = "default";

/// Save slots: 1 and 2 for the save points, 3 for the level start.
pub const SLOTS: std::ops::RangeInclusive<u8> = 1..=3;

/// Extension of save files.
const EXTENSION: &str = "save";

/// The lock file a running game holds.
const LOCK_FILE: &str = ".lock";

/// One saved game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRecord {
    /// Save format version.
    pub version: u32,
    /// The level, by name (`LEVEL1` to `LEVEL14`).
    pub level: String,
    /// The slot, 1 to 3.
    pub slot: u8,
    /// Where Claw starts; 0 or less with `y` means the level's own start.
    pub x: i32,
    /// Where Claw starts.
    pub y: i32,
    /// Progress carried into the level.
    pub carry: Carry,
    /// The level's treasure counts at the save.
    pub treasure_level: [u32; TREASURE_KINDS],
}

/// The level name of retail level `n`: `LEVEL<n>`.
pub fn retail_level_name(n: u32) -> String {
    format!("LEVEL{n}")
}

/// Checks a level name: 1 to 32 ASCII letters, digits, `_` or `-`, so no
/// name can reach outside the saves folder.
pub fn check_level_name(level: &str) -> Result<()> {
    let ok = (1..=32).contains(&level.len())
        && level
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if !ok {
        bail!("{level:?} is not a level name (letters, digits, _ and - only)");
    }
    Ok(())
}

fn check_slot(slot: u8) -> Result<()> {
    if !SLOTS.contains(&slot) {
        bail!("save slot {slot} does not exist (1 to 3)");
    }
    Ok(())
}

fn weapon_name(w: Weapon) -> &'static str {
    match w {
        Weapon::Pistol => "pistol",
        Weapon::Magic => "magic",
        Weapon::Dynamite => "dynamite",
    }
}

fn join_counts(c: &[u32; TREASURE_KINDS]) -> String {
    c.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
}

/// The keys of the progress a level starts with, in the order a save
/// writes them.
pub const CARRY_KEYS: [&str; 8] = [
    "score",
    "health",
    "lives",
    "pistol",
    "magic",
    "dynamite",
    "weapon",
    "treasure_game",
];

/// `carry` as the (key, value) pairs a save writes, in [`CARRY_KEYS`]
/// order.
pub fn carry_fields(carry: &Carry) -> Vec<(&'static str, String)> {
    let c = carry;
    vec![
        ("score", c.score.to_string()),
        ("health", c.health.to_string()),
        ("lives", c.lives.to_string()),
        ("pistol", c.pistol.to_string()),
        ("magic", c.magic.to_string()),
        ("dynamite", c.dynamite.to_string()),
        ("weapon", weapon_name(c.weapon).to_string()),
        ("treasure_game", join_counts(&c.treasure_game)),
    ]
}

/// The progress named by (key, value) pairs in a save's form: every key of
/// [`CARRY_KEYS`] must be there with a value in range; other keys are
/// left alone.
pub fn carry_from_fields(fields: &[(&str, &str)]) -> Result<Carry, SaveError> {
    let bad = |why: String| SaveError::Unreadable(why);
    let get = |k: &str| -> Result<&str, SaveError> {
        fields
            .iter()
            .find(|(f, _)| *f == k)
            .map(|(_, v)| *v)
            .ok_or_else(|| bad(format!("{k} is missing")))
    };
    let int = |k: &str, range: std::ops::RangeInclusive<i64>| -> Result<i64, SaveError> {
        let v = get(k)?;
        match v.parse::<i64>() {
            Ok(n) if range.contains(&n) => Ok(n),
            _ => Err(bad(format!("bad {k} {v:?}"))),
        }
    };
    let i32_range = i64::from(i32::MIN)..=i64::from(i32::MAX);
    let weapon = match get("weapon")? {
        "pistol" => Weapon::Pistol,
        "magic" => Weapon::Magic,
        "dynamite" => Weapon::Dynamite,
        w => return Err(bad(format!("bad weapon {w:?}"))),
    };
    Ok(Carry {
        score: int("score", 0..=i64::from(u32::MAX))? as u32,
        health: int("health", i32_range.clone())? as i32,
        lives: int("lives", i32_range.clone())? as i32,
        pistol: int("pistol", i32_range.clone())? as i32,
        magic: int("magic", i32_range.clone())? as i32,
        dynamite: int("dynamite", i32_range)? as i32,
        weapon,
        treasure_game: counts(fields, "treasure_game")?,
    })
}

/// The nine treasure counts of key `k` in (key, value) pairs.
fn counts(fields: &[(&str, &str)], k: &str) -> Result<[u32; TREASURE_KINDS], SaveError> {
    let bad = |why: String| SaveError::Unreadable(why);
    let v = fields
        .iter()
        .find(|(f, _)| *f == k)
        .map(|(_, v)| *v)
        .ok_or_else(|| bad(format!("{k} is missing")))?;
    let parts: Vec<&str> = v.split(',').map(str::trim).collect();
    if parts.len() != TREASURE_KINDS {
        return Err(bad(format!("{k} needs {TREASURE_KINDS} counts")));
    }
    let mut out = [0u32; TREASURE_KINDS];
    for (o, p) in out.iter_mut().zip(parts) {
        *o = p.parse().map_err(|_| bad(format!("bad {k} {v:?}")))?;
    }
    Ok(out)
}

/// Why a save file could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// The file is damaged or not a save.
    Unreadable(String),
    /// The file comes from a newer version of the game.
    Newer(u32),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Unreadable(why) => write!(f, "unreadable: {why}"),
            SaveError::Newer(v) => write!(
                f,
                "written by a newer version of the game (format {v}, this game reads {SAVE_VERSION})"
            ),
        }
    }
}

impl std::error::Error for SaveError {}

impl SaveRecord {
    /// A save of `level` at the save point `pos` with the progress `carry`
    /// and the level's treasure counts.
    pub fn new(
        level: impl Into<String>,
        pos: SavePos,
        carry: Carry,
        treasure_level: [u32; TREASURE_KINDS],
    ) -> SaveRecord {
        SaveRecord {
            version: SAVE_VERSION,
            level: level.into(),
            slot: pos.slot,
            x: pos.x,
            y: pos.y,
            carry,
            treasure_level,
        }
    }

    /// The save point, when the save has a position (both coordinates
    /// above 0).
    pub fn save_pos(&self) -> Option<SavePos> {
        (self.x > 0 && self.y > 0).then_some(SavePos {
            slot: self.slot,
            x: self.x,
            y: self.y,
        })
    }

    /// How the level starts from this save.
    pub fn start_state(&self) -> StartState {
        StartState {
            carry: Some(self.carry.clone()),
            save: self.save_pos(),
        }
    }

    /// The file contents.
    pub fn to_text(&self) -> String {
        let c = &self.carry;
        format!(
            "# rusty-claw saved game\n\
             version = {}\nlevel = {}\nslot = {}\nx = {}\ny = {}\n\
             score = {}\nhealth = {}\nlives = {}\npistol = {}\nmagic = {}\ndynamite = {}\n\
             weapon = {}\ntreasure_game = {}\ntreasure_level = {}\n",
            self.version,
            self.level,
            self.slot,
            self.x,
            self.y,
            c.score,
            c.health,
            c.lives,
            c.pistol,
            c.magic,
            c.dynamite,
            weapon_name(c.weapon),
            join_counts(&c.treasure_game),
            join_counts(&self.treasure_level),
        )
    }

    /// Parses file contents. The first setting must be the version; every
    /// field must be present once and in range.
    pub fn parse(text: &str) -> Result<SaveRecord, SaveError> {
        let bad = |why: String| SaveError::Unreadable(why);
        let mut fields: Vec<(&str, &str)> = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (k, v) = line
                .split_once('=')
                .ok_or_else(|| bad(format!("line {}: expected key = value", i + 1)))?;
            let (k, v) = (k.trim(), v.trim());
            if fields.iter().any(|(f, _)| *f == k) {
                return Err(bad(format!("line {}: {k} given twice", i + 1)));
            }
            if fields.is_empty() {
                if k != "version" {
                    return Err(bad("the first setting is not the version".into()));
                }
                let version: u32 = v.parse().map_err(|_| bad(format!("bad version {v:?}")))?;
                if version > SAVE_VERSION {
                    return Err(SaveError::Newer(version));
                }
                if version == 0 {
                    return Err(bad("bad version 0".into()));
                }
            }
            fields.push((k, v));
        }
        const KEYS: [&str; 14] = [
            "version",
            "level",
            "slot",
            "x",
            "y",
            "score",
            "health",
            "lives",
            "pistol",
            "magic",
            "dynamite",
            "weapon",
            "treasure_game",
            "treasure_level",
        ];
        if let Some((k, _)) = fields.iter().find(|(k, _)| !KEYS.contains(k)) {
            return Err(bad(format!("unknown setting {k:?}")));
        }
        Self::from_fields(&fields)
    }

    fn from_fields(fields: &[(&str, &str)]) -> Result<SaveRecord, SaveError> {
        let bad = |why: String| SaveError::Unreadable(why);
        let get = |k: &str| -> Result<&str, SaveError> {
            fields
                .iter()
                .find(|(f, _)| *f == k)
                .map(|(_, v)| *v)
                .ok_or_else(|| bad(format!("{k} is missing")))
        };
        let int = |k: &str, range: std::ops::RangeInclusive<i64>| -> Result<i64, SaveError> {
            let v = get(k)?;
            match v.parse::<i64>() {
                Ok(n) if range.contains(&n) => Ok(n),
                _ => Err(bad(format!("bad {k} {v:?}"))),
            }
        };
        let level = get("level")?.to_string();
        check_level_name(&level).map_err(|e| bad(e.to_string()))?;
        let i32_range = i64::from(i32::MIN)..=i64::from(i32::MAX);
        Ok(SaveRecord {
            version: int("version", 1..=i64::from(SAVE_VERSION))? as u32,
            level,
            slot: int("slot", 1..=3)? as u8,
            x: int("x", i32_range.clone())? as i32,
            y: int("y", i32_range)? as i32,
            carry: carry_from_fields(fields)?,
            treasure_level: counts(fields, "treasure_level")?,
        })
    }
}

/// The saved games of one profile, in one folder.
#[derive(Debug)]
pub struct SaveStore {
    dir: PathBuf,
    lock: Option<File>,
    reports: Vec<String>,
    /// Saves reported as unreadable, so each is reported once per run.
    unreadable: HashSet<PathBuf>,
}

impl SaveStore {
    /// Opens the store in `dir`, creating the folder when missing, and
    /// takes its lock. When another running game holds the lock, or the
    /// lock cannot be taken, the store is read-only and says so in
    /// [`SaveStore::reports`].
    pub fn open(dir: impl Into<PathBuf>) -> Result<SaveStore> {
        let dir = dir.into();
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating the saves folder {}", dir.display()))?;
        let mut store = SaveStore {
            dir,
            lock: None,
            reports: Vec::new(),
            unreadable: HashSet::new(),
        };
        let path = store.dir.join(LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path);
        match lock {
            Ok(file) => match file.try_lock() {
                Ok(()) => store.lock = Some(file),
                Err(fs::TryLockError::WouldBlock) => store.report(format!(
                    "another copy of the game is running; saved games are read-only \
                     in this one ({})",
                    store.dir.display()
                )),
                Err(fs::TryLockError::Error(e)) => store.report(format!(
                    "cannot lock {}: {e}; saved games are read-only",
                    path.display()
                )),
            },
            Err(e) => store.report(format!(
                "cannot open {}: {e}; saved games are read-only",
                path.display()
            )),
        }
        Ok(store)
    }

    /// Opens the default profile's store under the user's data folder
    /// (see [`UserDirs::saves_dir`]). A data folder inside the game
    /// install is refused, so the install is never written.
    pub fn open_default(dirs: &UserDirs, install: Option<&Path>) -> Result<SaveStore> {
        let dir = dirs
            .saves_dir()
            .ok_or_else(|| anyhow!("no folder for saved games: set HOME or XDG_DATA_HOME"))?;
        if let Some(install) = install {
            let canon = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            let install = canon(install);
            let mut probe = dir.clone();
            // Compare against the nearest existing ancestor too.
            while !probe.exists() && probe.pop() {}
            if dir.starts_with(&install) || canon(&probe).starts_with(&install) {
                bail!(
                    "the saves folder {} is inside the game install; set XDG_DATA_HOME elsewhere",
                    dir.display()
                );
            }
        }
        SaveStore::open(dir)
    }

    /// The store's folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// True when another running game holds the store: saves can be read
    /// but not written.
    pub fn is_read_only(&self) -> bool {
        self.lock.is_none()
    }

    /// Messages about the store: read-only, saves moved aside.
    pub fn reports(&self) -> &[String] {
        &self.reports
    }

    /// Takes the messages gathered so far.
    pub fn take_reports(&mut self) -> Vec<String> {
        std::mem::take(&mut self.reports)
    }

    fn report(&mut self, message: String) {
        eprintln!("rusty-claw: {message}");
        self.reports.push(message);
    }

    /// The file of `level`'s `slot`.
    pub fn path(&self, level: &str, slot: u8) -> Result<PathBuf> {
        check_level_name(level)?;
        check_slot(slot)?;
        Ok(self.dir.join(format!("{level}-{slot}.{EXTENSION}")))
    }

    /// Writes `record` into its slot, replacing the save there (kept as
    /// the backup). A file in the slot that is not a readable save of this
    /// version is moved aside first.
    pub fn write(&mut self, record: &SaveRecord) -> Result<()> {
        self.write_with(record, &mut WriteHooks::default())
    }

    /// [`SaveStore::write`] with test hooks.
    pub fn write_with(&mut self, record: &SaveRecord, hooks: &mut WriteHooks<'_>) -> Result<()> {
        let path = self.path(&record.level, record.slot)?;
        if self.is_read_only() {
            bail!("saved games are read-only: another copy of the game is running");
        }
        if record.version != SAVE_VERSION {
            bail!("cannot write a save of format {}", record.version);
        }
        if path.exists() {
            // Keep anything there that is not a save of this version.
            let _ = self.load(&path, &record.level, record.slot);
        }
        write_atomic(&path, record.to_text().as_bytes(), hooks)
            .with_context(|| format!("saving the game to {}", path.display()))
    }

    /// Reads `level`'s `slot`: `None` when there is no save. A file whose
    /// contents are not usable, or one from a newer version, is moved
    /// aside, reported and read as no save; a file that cannot be read is
    /// reported, left where it is and read as no save.
    pub fn read(&mut self, level: &str, slot: u8) -> Result<Option<SaveRecord>> {
        let path = self.path(level, slot)?;
        Ok(self.load(&path, level, slot))
    }

    /// Every readable save, by level name and slot. Files whose contents
    /// are not usable are moved aside and reported (see
    /// [`SaveStore::read`]).
    pub fn list(&mut self) -> Result<Vec<SaveRecord>> {
        let mut out = Vec::new();
        let entries =
            fs::read_dir(&self.dir).with_context(|| format!("listing {}", self.dir.display()))?;
        let mut names: Vec<(String, u8)> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(stem) = name.strip_suffix(&format!(".{EXTENSION}")) else {
                continue;
            };
            let Some((level, slot)) = stem.rsplit_once('-') else {
                continue;
            };
            let Ok(slot) = slot.parse::<u8>() else {
                continue;
            };
            if check_level_name(level).is_ok() && SLOTS.contains(&slot) {
                names.push((level.to_string(), slot));
            }
        }
        names.sort();
        for (level, slot) in names {
            if let Some(r) = self.read(&level, slot)? {
                out.push(r);
            }
        }
        Ok(out)
    }

    /// Reads one save file; moves it aside when its contents are not
    /// usable, and only reports it (once per run) when it cannot be read.
    fn load(&mut self, path: &Path, level: &str, slot: u8) -> Option<SaveRecord> {
        let result = match read_bounded(path, MAX_SAVE_BYTES) {
            Ok(None) => return None,
            // Too large: a content problem, read_bounded's only InvalidData.
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                Err(SaveError::Unreadable(e.to_string()))
            }
            Err(e) => {
                if self.unreadable.insert(path.to_path_buf()) {
                    self.report(format!(
                        "the save {} cannot be read ({e}); it is left as it is",
                        path.display()
                    ));
                }
                return None;
            }
            Ok(Some(bytes)) => match String::from_utf8(bytes) {
                Ok(text) => SaveRecord::parse(&text).and_then(|r| {
                    if r.level == level && r.slot == slot {
                        Ok(r)
                    } else {
                        Err(SaveError::Unreadable(format!(
                            "it holds {} slot {}",
                            r.level, r.slot
                        )))
                    }
                }),
                Err(_) => Err(SaveError::Unreadable("not text".into())),
            },
        };
        match result {
            Ok(r) => Some(r),
            Err(e) => {
                self.set_aside(path, &e);
                None
            }
        }
    }

    /// Renames an unusable save aside with a timestamp, never replacing
    /// another file.
    fn set_aside(&mut self, path: &Path, why: &SaveError) {
        if self.is_read_only() {
            self.report(format!("the save {} is {why}", path.display()));
            return;
        }
        let tag = match why {
            SaveError::Unreadable(_) => "unreadable",
            SaveError::Newer(_) => "newer",
        };
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        for n in 0..1000u32 {
            let suffix = if n == 0 {
                format!("{tag}-{secs}")
            } else {
                format!("{tag}-{secs}-{n}")
            };
            let aside = path.with_file_name(format!("{name}.{suffix}"));
            // A hard link never replaces an existing file; the rename
            // fallback is for file systems without links.
            let moved = match fs::hard_link(path, &aside) {
                Ok(()) => fs::remove_file(path).map(|_| true),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
                Err(_) if !aside.exists() => fs::rename(path, &aside).map(|_| true),
                Err(_) => Ok(false),
            };
            match moved {
                Ok(true) => {
                    self.report(format!(
                        "the save {} is {why}; it was kept as {}",
                        path.display(),
                        aside.display()
                    ));
                    return;
                }
                Ok(false) => continue,
                Err(e) => {
                    self.report(format!(
                        "the save {} is {why}, and it could not be moved aside: {e}",
                        path.display()
                    ));
                    return;
                }
            }
        }
        self.report(format!(
            "the save {} is {why}, and no free name was found to keep it aside",
            path.display()
        ));
    }
}
