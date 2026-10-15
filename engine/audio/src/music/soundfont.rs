//! Finding, checking and loading the SoundFont that music plays with.
//!
//! Music needs a General MIDI SoundFont (an `.sf2` instrument bank). None is
//! shipped with the source; the default bank is GeneralUser GS, which
//! `make soundfont` fetches into the user data directory and release
//! packages install next to the game. The first usable file in this order
//! wins:
//!
//! 1. the `soundfont` setting ([`crate::AudioSettings::soundfont`]);
//! 2. the `CLAW_SOUNDFONT` environment variable;
//! 3. the installed package's `share/rusty-claw/soundfonts/` folder, next to
//!    the game's `bin/` folder;
//! 4. the user data directory, `$XDG_DATA_HOME/rusty-claw/soundfonts/`
//!    (default `~/.local/share/rusty-claw/soundfonts/`);
//! 5. common system paths, where distributions install FluidR3_GM.
//!
//! In a folder, `GeneralUser-GS.sf2` is taken first and otherwise the first
//! `.sf2` file in name order. A file is usable when it is a RIFF `sfbk` form
//! whose version is 2 (SF2); compressed SF3 banks are refused. With no usable
//! file, music is off and one message says how to get a bank.

use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustysynth::SoundFont;

use super::convert::BankMode;

/// File name of the default bank.
pub const DEFAULT_FILE: &str = "GeneralUser-GS.sf2";

/// Environment variable naming a SoundFont file.
pub const ENV_VAR: &str = "CLAW_SOUNDFONT";

/// Folder below a data directory (user or package) that holds banks.
pub const SUBDIR: &str = "rusty-claw/soundfonts";

/// System paths tried last, in order.
pub const SYSTEM_PATHS: &[&str] = &[
    "/usr/share/soundfonts/default.sf2",
    "/usr/share/soundfonts/FluidR3_GM.sf2",
    "/usr/share/soundfonts/GeneralUser.sf2",
    "/usr/share/sounds/sf2/default-GM.sf2",
    "/usr/share/sounds/sf2/FluidR3_GM.sf2",
];

/// Largest bank loaded, in bytes.
pub const MAX_BANK_BYTES: u64 = 1 << 30;

/// Where a bank was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The `soundfont` setting.
    Setting,
    /// The `CLAW_SOUNDFONT` environment variable.
    Environment,
    /// The installed package's data folder.
    Package,
    /// The user data directory.
    DataDir,
    /// A system path.
    System,
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Origin::Setting => "the soundfont setting",
            Origin::Environment => ENV_VAR,
            Origin::Package => "the installed package",
            Origin::DataDir => "the user data directory",
            Origin::System => "the system",
        })
    }
}

/// A bank chosen by the lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The file.
    pub path: PathBuf,
    /// Where it came from.
    pub origin: Origin,
}

/// The places the lookup searches, so tests can supply their own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Places {
    /// The `soundfont` setting.
    pub setting: Option<PathBuf>,
    /// The `CLAW_SOUNDFONT` value.
    pub environment: Option<PathBuf>,
    /// The installed package's bank folder.
    pub package: Option<PathBuf>,
    /// The user data directory's bank folder.
    pub data_dir: Option<PathBuf>,
    /// System files, in order.
    pub system: Vec<PathBuf>,
}

impl Places {
    /// The real places: the given setting, the environment, the package
    /// folder next to the running program, the user data directory and the
    /// system paths.
    pub fn standard(setting: Option<&Path>) -> Places {
        Places {
            setting: setting.map(Path::to_path_buf),
            environment: std::env::var_os(ENV_VAR)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            package: package_dir(),
            data_dir: data_dir(),
            system: SYSTEM_PATHS.iter().map(PathBuf::from).collect(),
        }
    }
}

/// The user data directory's bank folder:
/// `$XDG_DATA_HOME/rusty-claw/soundfonts`, or
/// `~/.local/share/rusty-claw/soundfonts`.
pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|h| !h.is_empty())
                .map(|h| PathBuf::from(h).join(".local/share"))
        })?;
    Some(base.join(SUBDIR))
}

/// The installed package's bank folder: `../share/rusty-claw/soundfonts`
/// from the folder of the running program.
pub fn package_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let prefix = exe.parent()?.parent()?;
    Some(prefix.join("share").join(SUBDIR))
}

/// Why a file was not used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// The file.
    pub path: PathBuf,
    /// Where it came from.
    pub origin: Origin,
    /// The reason.
    pub reason: String,
}

/// The lookup's answer: the bank to use, if any, and the files refused on
/// the way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lookup {
    /// The first usable bank.
    pub found: Option<Found>,
    /// Files named or present that were not usable.
    pub refused: Vec<Refused>,
}

/// Searches `places` in order and returns the first usable bank.
pub fn find(places: &Places) -> Lookup {
    let mut lookup = Lookup::default();
    let mut candidates: Vec<(PathBuf, Origin, bool)> = Vec::new();
    if let Some(p) = &places.setting {
        candidates.push((p.clone(), Origin::Setting, true));
    }
    if let Some(p) = &places.environment {
        candidates.push((p.clone(), Origin::Environment, true));
    }
    for (dir, origin) in [
        (&places.package, Origin::Package),
        (&places.data_dir, Origin::DataDir),
    ] {
        if let Some(dir) = dir {
            for f in folder_banks(dir) {
                candidates.push((f, origin, false));
            }
        }
    }
    for p in &places.system {
        candidates.push((p.clone(), Origin::System, false));
    }
    for (path, origin, named) in candidates {
        if !named && !path.is_file() {
            continue;
        }
        match check_file(&path) {
            Ok(()) => {
                lookup.found = Some(Found { path, origin });
                return lookup;
            }
            Err(reason) => lookup.refused.push(Refused {
                path,
                origin,
                reason,
            }),
        }
    }
    lookup
}

/// The banks in a folder, in the order the lookup tries them: the default
/// file first, then the other `.sf2` files by name.
pub fn folder_banks(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.to_ascii_lowercase().ends_with(".sf2"))
        .collect();
    names.sort();
    if let Some(i) = names.iter().position(|n| n == DEFAULT_FILE) {
        let d = names.remove(i);
        names.insert(0, d);
    }
    names.into_iter().map(|n| dir.join(n)).collect()
}

/// Checks the start of a bank: a RIFF `sfbk` form whose `ifil` version is
/// 2. Returns why not otherwise.
pub fn check_header(data: &[u8]) -> Result<(), String> {
    if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"sfbk" {
        return Err("not a SoundFont (no RIFF sfbk header)".into());
    }
    // The INFO list comes first; find ifil in it.
    let mut pos = 12;
    while pos + 12 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
            as usize;
        if id == b"LIST" && &data[pos + 8..pos + 12] == b"INFO" {
            let end = (pos + 8).saturating_add(size).min(data.len());
            let mut q = pos + 12;
            while q + 8 <= end {
                let sid = &data[q..q + 4];
                let ssize = u32::from_le_bytes([data[q + 4], data[q + 5], data[q + 6], data[q + 7]])
                    as usize;
                if sid == b"ifil" {
                    let major = data
                        .get(q + 8..q + 10)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]))
                        .ok_or("the SoundFont version is cut short")?;
                    return match major {
                        2 => Ok(()),
                        3 => Err("an SF3 (compressed) bank; only SF2 banks are supported".into()),
                        v => Err(format!("SoundFont version {v} is not supported (SF2 only)")),
                    };
                }
                q = q.saturating_add(8).saturating_add(ssize + (ssize & 1));
            }
            return Err("the SoundFont has no version".into());
        }
        pos = pos.saturating_add(8).saturating_add(size + (size & 1));
    }
    Err("the SoundFont has no INFO list".into())
}

/// Checks a bank file's header.
pub fn check_file(path: &Path) -> Result<(), String> {
    let mut head = Vec::with_capacity(4096);
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(1 << 16)
        .read_to_end(&mut head)
        .map_err(|e| e.to_string())?;
    check_header(&head)
}

/// The bank mode for a file: the game's own instrument bank names
/// (`CLAW.SF2`, `CLAW4.SF2`) select [`BankMode::Original`].
pub fn bank_mode(path: &Path) -> BankMode {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_uppercase();
    if name == "CLAW.SF2" || name == "CLAW4.SF2" {
        BankMode::Original
    } else {
        BankMode::GeneralMidi
    }
}

/// Synthesizer volume of the default bank, keeping its loudest LEVEL music
/// at or below -1 dBFS at full music volume.
pub const DEFAULT_GAIN: f32 = 0.375;

/// Synthesizer volume for a bank, by its file name: measured values for the
/// known General MIDI banks, the default bank's value otherwise. The level
/// is per bank, never per track, so the tracks keep their relative levels.
pub fn bank_gain(path: &Path) -> f32 {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.starts_with("fluidr3") {
        0.29
    } else if name.starts_with("musescore_general") {
        0.41
    } else if name.starts_with("timgm6mb") {
        0.35
    } else {
        DEFAULT_GAIN
    }
}

/// A loaded bank, shared by every track's synthesizer.
#[derive(Clone)]
pub struct Bank {
    /// The instruments.
    pub font: Arc<SoundFont>,
    /// The file it came from.
    pub path: PathBuf,
    /// Arrangement and bank-select handling.
    pub mode: BankMode,
    /// Synthesizer volume (see [`bank_gain`]).
    pub gain: f32,
}

impl fmt::Debug for Bank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bank")
            .field("path", &self.path)
            .field("mode", &self.mode)
            .field("gain", &self.gain)
            .field("presets", &self.font.get_presets().len())
            .finish()
    }
}

impl Bank {
    /// Loads a bank file. Slow for large banks; call it once, off the audio
    /// thread.
    pub fn load(path: &Path) -> Result<Bank, String> {
        let meta = fs::metadata(path).map_err(|e| e.to_string())?;
        if meta.len() > MAX_BANK_BYTES {
            return Err(format!("larger than {MAX_BANK_BYTES} bytes"));
        }
        let data = fs::read(path).map_err(|e| e.to_string())?;
        let mut bank = Bank::from_bytes(&data)?;
        bank.mode = bank_mode(path);
        bank.gain = bank_gain(path);
        bank.path = path.to_path_buf();
        Ok(bank)
    }

    /// Loads a bank from memory, as a General MIDI bank at the default gain.
    pub fn from_bytes(data: &[u8]) -> Result<Bank, String> {
        check_header(data)?;
        // The bank comes from the player's disk; a malformed file must not
        // take the game down, whatever the parser does with it.
        let parsed = std::panic::catch_unwind(|| SoundFont::new(&mut &data[..]))
            .map_err(|_| "the SoundFont is malformed".to_string())?;
        let font = parsed.map_err(|e| format!("the SoundFont is malformed: {e}"))?;
        Ok(Bank {
            font: Arc::new(font),
            path: PathBuf::new(),
            mode: BankMode::GeneralMidi,
            gain: DEFAULT_GAIN,
        })
    }

    /// Whether the bank has the preset (bank, program).
    pub fn has_preset(&self, bank: u8, program: u8) -> bool {
        self.font.get_presets().iter().any(|p| {
            p.get_bank_number() == i32::from(bank) && p.get_patch_number() == i32::from(program)
        })
    }
}

/// The message for the player when no bank is usable: music is off, and how
/// to get a bank.
pub fn missing_message(lookup: &Lookup) -> String {
    let mut msg = String::from("no SoundFont found, music is off. ");
    for r in &lookup.refused {
        msg.push_str(&format!(
            "{} (from {}) was not used: {}. ",
            r.path.display(),
            r.origin,
            r.reason
        ));
    }
    let dir = data_dir().map_or_else(
        || format!("~/.local/share/{SUBDIR}"),
        |d| d.display().to_string(),
    );
    msg.push_str(&format!(
        "Run `make soundfont` to fetch the default bank into {dir}, put an .sf2 file there, \
         or name one with the soundfont setting or {ENV_VAR}."
    ));
    msg
}

/// Builds a tiny SF2 bank in memory: one sine-wave instrument as preset
/// (0, 0) and a click as the percussion preset (128, 0). For tests and
/// tools.
pub fn build_test_bank() -> Vec<u8> {
    fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
        if body.len() & 1 == 1 {
            out.push(0);
        }
        out
    }
    fn list(kind: &[u8; 4], parts: &[Vec<u8>]) -> Vec<u8> {
        let mut body = kind.to_vec();
        for p in parts {
            body.extend_from_slice(p);
        }
        chunk(b"LIST", &body)
    }
    fn name20(name: &str) -> [u8; 20] {
        let mut n = [0u8; 20];
        n[..name.len()].copy_from_slice(name.as_bytes());
        n
    }
    // Samples: a 441 Hz sine at 22050 Hz (50-sample period, 20 periods),
    // then a decaying click, each followed by 46 zero points.
    let mut smpl: Vec<i16> = (0..1000)
        .map(|i| ((i as f32 * std::f32::consts::TAU / 50.0).sin() * 12000.0) as i16)
        .collect();
    smpl.extend(std::iter::repeat_n(0, 46));
    let click_start = smpl.len() as u32;
    smpl.extend((0..400).map(|i| ((400 - i) * 40 * if i % 2 == 0 { 1 } else { -1 }) as i16));
    let click_end = smpl.len() as u32;
    smpl.extend(std::iter::repeat_n(0, 46));
    let smpl_bytes: Vec<u8> = smpl.iter().flat_map(|s| s.to_le_bytes()).collect();

    let info = list(
        b"INFO",
        &[
            chunk(b"ifil", &[2, 0, 1, 0]),
            chunk(b"isng", b"EMU8000\0"),
            chunk(b"INAM", b"Test bank\0"),
        ],
    );
    let sdta = list(b"sdta", &[chunk(b"smpl", &smpl_bytes)]);

    // Presets: Sine (0, 0) -> instrument 0, Click (128, 0) -> instrument 1.
    let mut phdr = Vec::new();
    for (name, preset, bank, bag) in [
        ("Sine", 0u16, 0u16, 0u16),
        ("Click", 0, 128, 1),
        ("EOP", 0, 0, 2),
    ] {
        phdr.extend_from_slice(&name20(name));
        phdr.extend_from_slice(&preset.to_le_bytes());
        phdr.extend_from_slice(&bank.to_le_bytes());
        phdr.extend_from_slice(&bag.to_le_bytes());
        phdr.extend_from_slice(&[0u8; 12]);
    }
    let bags = |n: u16| -> Vec<u8> {
        (0..=n)
            .flat_map(|i| {
                let mut b = i.to_le_bytes().to_vec();
                b.extend_from_slice(&0u16.to_le_bytes());
                b
            })
            .collect()
    };
    let gen_list = |gens: &[(u16, u16)]| -> Vec<u8> {
        let mut out = Vec::new();
        for (op, amount) in gens.iter().chain([(0u16, 0u16)].iter()) {
            out.extend_from_slice(&op.to_le_bytes());
            out.extend_from_slice(&amount.to_le_bytes());
        }
        out
    };
    // Generator 41 is the instrument, 53 the sample, 54 the sample modes.
    let pgen = gen_list(&[(41, 0), (41, 1)]);
    let mut inst = Vec::new();
    for (name, bag) in [("Sine", 0u16), ("Click", 1), ("EOI", 2)] {
        inst.extend_from_slice(&name20(name));
        inst.extend_from_slice(&bag.to_le_bytes());
    }
    let igen = gen_list(&[(54, 1), (53, 0), (53, 1)]);
    // Instrument zone 0 has two generators (modes, sample), zone 1 one.
    let ibag: Vec<u8> = [(0u16, 0u16), (2, 0), (3, 0)]
        .iter()
        .flat_map(|(g, m)| {
            let mut b = g.to_le_bytes().to_vec();
            b.extend_from_slice(&m.to_le_bytes());
            b
        })
        .collect();
    let mut shdr = Vec::new();
    for (name, start, end, ls, le, pitch) in [
        ("Sine", 0u32, 1000u32, 0u32, 1000u32, 69u8),
        ("Click", click_start, click_end, click_start, click_end, 60),
        ("EOS", 0, 0, 0, 0, 0),
    ] {
        shdr.extend_from_slice(&name20(name));
        for v in [start, end, ls, le, 22050] {
            shdr.extend_from_slice(&v.to_le_bytes());
        }
        shdr.push(pitch);
        shdr.push(0);
        shdr.extend_from_slice(&0u16.to_le_bytes());
        shdr.extend_from_slice(&(if name == "EOS" { 0u16 } else { 1u16 }).to_le_bytes());
    }
    let pdta = list(
        b"pdta",
        &[
            chunk(b"phdr", &phdr),
            chunk(b"pbag", &bags(2)),
            chunk(b"pmod", &[0u8; 10]),
            chunk(b"pgen", &pgen),
            chunk(b"inst", &inst),
            chunk(b"ibag", &ibag),
            chunk(b"imod", &[0u8; 10]),
            chunk(b"igen", &igen),
            chunk(b"shdr", &shdr),
        ],
    );
    let mut body = b"sfbk".to_vec();
    body.extend_from_slice(&info);
    body.extend_from_slice(&sdta);
    body.extend_from_slice(&pdta);
    chunk(b"RIFF", &body)
}
