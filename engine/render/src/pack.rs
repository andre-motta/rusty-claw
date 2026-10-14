//! Optional image packs: higher-resolution RGBA replacements for the
//! original indexed images.
//!
//! A pack is a folder with a `pack.txt` manifest:
//!
//! ```text
//! # Comments start with '#'.
//! scale 2
//! LEVEL1/TILES/ACTION/012.PID = tiles/action_012.png
//! CLAW/IMAGES/STAND/FRAME001.PID = claw/stand_1.png
//! ```
//!
//! - `scale N` (1 to 8) is the ratio of pack pixels to game pixels, the same
//!   for every image of the pack.
//! - Every other line maps the logical path of an original image, as the
//!   asset store names it (case-insensitive, `/` or `\`), to a PNG file
//!   inside the pack folder.
//! - A replacement must be exactly `scale` times the original's size; one
//!   that is not, or that cannot be read, is skipped with a warning and the
//!   original is drawn instead.
//!
//! A pack changes appearance only. Placement, size, offsets and coverage
//! (which pixels are drawn at all) always come from the original image, so
//! the simulation never sees a pack. Replacements are drawn for sprites and
//! tiles in the normal draw mode; the table-based modes keep the original
//! indexed image, and palette fades darken replacement colours by the same
//! amount.

use std::collections::HashMap;
use std::fs;
use std::io::BufWriter;
use std::path::{Component, Path, PathBuf};

/// File name of the manifest inside a pack folder.
pub const MANIFEST: &str = "pack.txt";

/// Largest scale a pack may declare.
pub const MAX_SCALE: u32 = 8;

/// Largest width or height of a replacement image.
pub const MAX_DIMENSION: u32 = 1 << 14;

/// Errors reading a pack.
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    /// The folder or manifest could not be read.
    #[error("{path}: {source}")]
    Io {
        /// The path involved.
        path: PathBuf,
        /// The cause.
        source: std::io::Error,
    },
    /// The manifest is malformed.
    #[error("{MANIFEST} line {line}: {reason}")]
    Manifest {
        /// 1-based line number.
        line: usize,
        /// What is wrong.
        reason: String,
    },
    /// A PNG could not be written.
    #[error("writing {path}: {reason}")]
    Write {
        /// The path involved.
        path: PathBuf,
        /// What went wrong.
        reason: String,
    },
}

/// Normalises a logical image path to a pack key: upper case, `/`
/// separators, no leading separator.
pub fn key(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .to_ascii_uppercase()
}

/// A loaded pack manifest. Images are read when first needed.
#[derive(Debug, Clone)]
pub struct HdPack {
    root: PathBuf,
    scale: u32,
    entries: HashMap<String, PathBuf>,
    /// Problems met so far: bad entries, unreadable or mis-sized images.
    pub warnings: Vec<String>,
}

impl HdPack {
    /// Reads the manifest of the pack in `dir`.
    pub fn load(dir: &Path) -> Result<HdPack, PackError> {
        let root = fs::canonicalize(dir).map_err(|source| PackError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let manifest = root.join(MANIFEST);
        let text = fs::read_to_string(&manifest).map_err(|source| PackError::Io {
            path: manifest.clone(),
            source,
        })?;
        let mut scale = None;
        let mut entries = HashMap::new();
        let mut warnings = Vec::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("scale") {
                if !rest.starts_with(char::is_whitespace) {
                    return Err(PackError::Manifest {
                        line: n + 1,
                        reason: format!("unknown setting {line:?}"),
                    });
                }
                let v: u32 = rest.trim().parse().map_err(|_| PackError::Manifest {
                    line: n + 1,
                    reason: format!("bad scale {:?}", rest.trim()),
                })?;
                if !(1..=MAX_SCALE).contains(&v) {
                    return Err(PackError::Manifest {
                        line: n + 1,
                        reason: format!("scale {v} is outside 1 to {MAX_SCALE}"),
                    });
                }
                scale = Some(v);
                continue;
            }
            let Some((from, to)) = line.split_once('=') else {
                return Err(PackError::Manifest {
                    line: n + 1,
                    reason: "expected 'ORIGINAL/PATH = file.png'".into(),
                });
            };
            let (from, to) = (from.trim(), to.trim());
            match confined(&root, to) {
                Some(file) if !from.is_empty() => {
                    entries.insert(key(from), file);
                }
                _ => warnings.push(format!(
                    "line {}: {to:?} is not a file inside the pack, skipped",
                    n + 1
                )),
            }
        }
        let scale = scale.ok_or(PackError::Manifest {
            line: 0,
            reason: "no 'scale N' line".into(),
        })?;
        Ok(HdPack {
            root,
            scale,
            entries,
            warnings,
        })
    }

    /// The pack folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Pack pixels per game pixel.
    pub fn scale(&self) -> u32 {
        self.scale
    }

    /// Number of replacements listed.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the pack lists no replacement.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The replacement for the original image at `path` of `width` x
    /// `height` game pixels, as packed RGBA (red in the low byte), row-major,
    /// `scale` times the size. `None`, with a warning, when the replacement
    /// is missing a file, unreadable or the wrong size.
    pub fn image_for(&mut self, path: &str, width: u32, height: u32) -> Option<Vec<u32>> {
        let file = self.entries.get(&key(path))?.clone();
        let want = (width * self.scale, height * self.scale);
        match read_png(&file) {
            Ok((w, h, px)) if (w, h) == want => Some(px),
            Ok((w, h, _)) => {
                self.warnings.push(format!(
                    "{}: {w}x{h}, expected {}x{} for {path}; using the original",
                    file.display(),
                    want.0,
                    want.1
                ));
                None
            }
            Err(e) => {
                self.warnings
                    .push(format!("{}: {e}; using the original", file.display()));
                None
            }
        }
    }
}

/// Resolves a manifest file name inside `root`, refusing absolute paths,
/// parent references and anything that resolves outside the folder.
fn confined(root: &Path, name: &str) -> Option<PathBuf> {
    let rel = Path::new(name);
    if name.is_empty()
        || rel
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    let real = fs::canonicalize(root.join(rel)).ok()?;
    (real.starts_with(root) && real.is_file()).then_some(real)
}

/// Reads a PNG as packed RGBA (red in the low byte).
pub fn read_png(path: &Path) -> Result<(u32, u32, Vec<u32>), String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let (w, h) = {
        let info = reader.info();
        (info.width, info.height)
    };
    if w == 0 || h == 0 || w > MAX_DIMENSION || h > MAX_DIMENSION {
        return Err(format!("unsupported size {w}x{h}"));
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "image too large".to_string())?;
    let mut buf = vec![0u8; size];
    let frame = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let data = &buf[..frame.buffer_size()];
    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err("indexed PNG was not expanded".into()),
    };
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h as usize {
        let row = &data[y * frame.line_size..];
        for x in 0..w as usize {
            let p = &row[x * channels..x * channels + channels];
            let (r, g, b, a) = match channels {
                1 => (p[0], p[0], p[0], 255),
                2 => (p[0], p[0], p[0], p[1]),
                3 => (p[0], p[1], p[2], 255),
                _ => (p[0], p[1], p[2], p[3]),
            };
            out.push(u32::from_le_bytes([r, g, b, a]));
        }
    }
    Ok((w, h, out))
}

/// Writes RGBA bytes (four per pixel, row-major) as a PNG.
pub fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), PackError> {
    let err = |reason: String| PackError::Write {
        path: path.to_path_buf(),
        reason,
    };
    let file = fs::File::create(path).map_err(|e| err(e.to_string()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| err(e.to_string()))?;
    writer
        .write_image_data(rgba)
        .map_err(|e| err(e.to_string()))?;
    writer.finish().map_err(|e| err(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_and_confinement() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join("img")).unwrap();
        write_png(&root.join("img/a.png"), 4, 2, &[200u8; 32]).unwrap();
        write_png(&root.join("img/b.png"), 3, 3, &[9u8; 36]).unwrap();
        fs::write(
            root.join(MANIFEST),
            "# test\nscale 2\nlevel1\\tiles\\a.pid = img/a.png\nX/B.PID = img/b.png\n\
             X/C.PID = ../outside.png\nX/D.PID = /etc/hostname\nX/E.PID = img/missing.png\n",
        )
        .unwrap();
        let mut pack = HdPack::load(root).unwrap();
        assert_eq!(pack.scale(), 2);
        assert_eq!(pack.len(), 2);
        assert_eq!(pack.warnings.len(), 3);
        let a = pack.image_for("LEVEL1/TILES/A.PID", 2, 1).unwrap();
        assert_eq!(a.len(), 8);
        assert_eq!(a[0], u32::from_le_bytes([200, 200, 200, 200]));
        assert!(pack.image_for("X/B.PID", 2, 2).is_none());
        assert_eq!(pack.warnings.len(), 4);
        assert!(pack.image_for("NOT/LISTED.PID", 1, 1).is_none());
    }

    #[test]
    fn manifest_errors() {
        let dir = tempfile::tempdir().unwrap();
        for bad in [
            "a = b.png\n",
            "scale 0\n",
            "scale nine\n",
            "scaled 2\n",
            "scale 2\nnothing\n",
        ] {
            fs::write(dir.path().join(MANIFEST), bad).unwrap();
            assert!(HdPack::load(dir.path()).is_err(), "{bad:?}");
        }
        assert!(HdPack::load(&dir.path().join("none")).is_err());
    }
}
