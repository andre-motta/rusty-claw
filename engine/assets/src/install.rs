//! The user's own game install: where its assets and custom levels are.
//!
//! An install root holds an `Assets` folder (the extracted asset tree), an
//! optional `CLAW.REZ` archive and an optional `Custom` folder with community
//! custom levels. Names are matched case-insensitively, and the archive and
//! `Assets` may be symbolic links. A missing or empty archive is fine; an
//! install with neither an archive nor `Assets` is not.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AssetError, Result, io_error};
use crate::name::{fold, split_logical};
use crate::store::{AssetStore, Source, find_child};

/// File name of the archive at the install root.
pub const ARCHIVE_NAME: &str = "CLAW.REZ";
/// Folder name of the extracted asset tree.
pub const ASSETS_DIR: &str = "Assets";
/// Folder name of the community custom levels.
pub const CUSTOM_DIR: &str = "Custom";

/// A located install.
#[derive(Debug, Clone)]
pub struct Install {
    root: PathBuf,
    archive: Option<PathBuf>,
    assets: Option<PathBuf>,
    custom: Option<PathBuf>,
}

impl Install {
    /// Locates the parts of an install below `root`.
    pub fn open(root: impl AsRef<Path>) -> Result<Install> {
        let given = root.as_ref();
        let root = fs::canonicalize(given).map_err(|e| io_error(given, e))?;
        if !root.is_dir() {
            return Err(AssetError::Invalid(format!(
                "{} is not a folder",
                given.display()
            )));
        }
        let archive = find_part(&root, ARCHIVE_NAME, false);
        let assets = find_part(&root, ASSETS_DIR, true);
        let custom = find_child(&root, CUSTOM_DIR, true);
        if archive.is_none() && assets.is_none() {
            return Err(AssetError::Invalid(format!(
                "{} has neither an {ASSETS_DIR} folder nor {ARCHIVE_NAME}",
                given.display()
            )));
        }
        Ok(Install {
            root,
            archive,
            assets,
            custom,
        })
    }

    /// The install root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The archive, if present.
    pub fn archive(&self) -> Option<&Path> {
        self.archive.as_deref()
    }

    /// The extracted asset folder, if present.
    pub fn assets_dir(&self) -> Option<&Path> {
        self.assets.as_deref()
    }

    /// The custom level folder, if present.
    pub fn custom_dir(&self) -> Option<&Path> {
        self.custom.as_deref()
    }

    /// A store with the archive (if any) and then the asset folder mounted,
    /// so files in the folder win over the archive.
    pub fn store(&self) -> Result<AssetStore> {
        let mut store = AssetStore::new();
        if let Some(archive) = &self.archive {
            store.mount(Source::Rez(archive.clone()))?;
        }
        if let Some(assets) = &self.assets {
            store.mount(Source::Dir(assets.clone()))?;
        }
        Ok(store)
    }

    /// Resolves a custom level file name, such as `My Level.WWD` or
    /// `Custom/My Level.WWD`, to its path inside the custom level folder.
    ///
    /// The name is matched case-insensitively and must name a file directly
    /// in that folder; any path that would leave it is rejected.
    pub fn custom_level_path(&self, name: &str) -> Result<PathBuf> {
        let custom = self
            .custom
            .as_ref()
            .ok_or_else(|| AssetError::NotFound(format!("{CUSTOM_DIR} folder")))?;
        let mut segs = split_logical(name)?;
        if segs.len() > 1 && fold(segs[0]) == fold(CUSTOM_DIR) {
            segs.remove(0);
        }
        let [file] = segs.as_slice() else {
            return Err(AssetError::Path {
                path: name.to_string(),
                reason: "must name a file directly in the custom level folder",
            });
        };
        let path = find_child(custom, file, false)
            .ok_or_else(|| AssetError::NotFound(format!("{CUSTOM_DIR}/{file}")))?;
        let real = fs::canonicalize(&path).map_err(|e| io_error(&path, e))?;
        let custom_real = fs::canonicalize(custom).map_err(|e| io_error(custom, e))?;
        if !real.starts_with(&custom_real) {
            return Err(AssetError::Path {
                path: name.to_string(),
                reason: "resolves outside the custom level folder",
            });
        }
        Ok(path)
    }

    /// Every `.WWD` file directly in the custom level folder, sorted by name.
    pub fn custom_levels(&self) -> Result<Vec<PathBuf>> {
        let Some(custom) = &self.custom else {
            return Ok(Vec::new());
        };
        let mut out: Vec<PathBuf> = fs::read_dir(custom)
            .map_err(|e| io_error(custom, e))?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| fold(n).ends_with(".wwd"))
            })
            .map(|e| e.path())
            .collect();
        out.sort();
        Ok(out)
    }
}

/// Finds the archive or the asset folder at the install root by name, any
/// case. Unlike lookups inside a mount, a symbolic link is followed here:
/// the install root is the user's own choice, and its parts may live
/// elsewhere (for example in another game library). What a mounted folder
/// may read is still confined to the folder the link resolves to.
fn find_part(root: &Path, name: &str, want_dir: bool) -> Option<PathBuf> {
    let want = fold(name);
    let mut found: Vec<(std::ffi::OsString, PathBuf)> = fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| fold(n) == want))
        .filter(|e| {
            fs::metadata(e.path()).is_ok_and(|m| if want_dir { m.is_dir() } else { m.is_file() })
        })
        .map(|e| (e.file_name(), e.path()))
        .collect();
    found.sort();
    found.into_iter().next().map(|(_, p)| p)
}
