//! Name-keyed registries of image sets, sounds, animations and music.
//!
//! Objects in a level do not name files; they name registry keys such as
//! `LEVEL_RAT`. The registries are built by walking folders: every folder of
//! images becomes one image set keyed by a prefix plus the folder path below
//! the mapped folder joined with `_`, and every sound or animation file
//! becomes one entry keyed the same way with its file name appended.
//!
//! Collision rules: within an image set the first file with a given frame
//! number is kept; the first sound registered under a key is kept; a later
//! animation replaces an earlier one with the same key. Registry keys are
//! compared exactly (case-sensitive); folder sources store upper-case names,
//! so their keys are upper case.

use std::collections::BTreeMap;

use crate::image_set::ImageSet;
use crate::name::{TYPE_ANI, TYPE_BMP, TYPE_PCX, TYPE_PID, TYPE_WAV, frame_index, registry_key};
use crate::store::{AssetStore, DirNode, FileId};

/// Type codes of files that can be image-set frames.
pub const IMAGE_TYPES: [u32; 3] = [TYPE_PID, TYPE_PCX, TYPE_BMP];

/// An image set in a registry: frame numbers mapped to files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageSetRef {
    /// Registry key.
    pub key: String,
    /// Frame number to file.
    pub frames: BTreeMap<i32, FileId>,
}

/// A collision met while building the registries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEvent {
    /// What happened.
    pub kind: RegistryEventKind,
    /// The key involved.
    pub key: String,
    /// Paths involved.
    pub detail: String,
}

/// Kinds of [`RegistryEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryEventKind {
    /// A frame number already present in an image set; the earlier file was
    /// kept.
    FrameKept,
    /// A sound key already present; the earlier sound was kept.
    SoundKept,
    /// An animation key already present; the later animation replaced it.
    AnimationReplaced,
}

/// The registries of one level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registries {
    /// Image sets by key.
    pub images: BTreeMap<String, ImageSetRef>,
    /// Sounds by key.
    pub sounds: BTreeMap<String, FileId>,
    /// Animations by key.
    pub anis: BTreeMap<String, FileId>,
    /// Music tracks by key.
    pub music: BTreeMap<String, FileId>,
    /// Collisions met while mapping.
    pub events: Vec<RegistryEvent>,
}

impl Registries {
    /// Empty registries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Maps a folder of images. Subfolders are mapped first, each with its
    /// key as the new prefix; then, when `prefix` is not empty, the folder's
    /// own image files become frames of the set named `prefix`.
    pub fn map_images(&mut self, store: &AssetStore, folder: &DirNode, prefix: &str) {
        for sub in folder.dirs() {
            self.map_images(store, sub, &registry_key(prefix, sub.name()));
        }
        if prefix.is_empty() {
            return;
        }
        let mut set = self.images.remove(prefix).unwrap_or_else(|| ImageSetRef {
            key: prefix.to_string(),
            frames: BTreeMap::new(),
        });
        for id in folder.files() {
            let file = store.file(id);
            if !IMAGE_TYPES.contains(&file.type_code()) {
                continue;
            }
            let index = frame_index(file.name());
            if let Some(&kept) = set.frames.get(&index) {
                self.events.push(RegistryEvent {
                    kind: RegistryEventKind::FrameKept,
                    key: prefix.to_string(),
                    detail: format!(
                        "frame {index}: {} kept, {} skipped",
                        store.file(kept).path(),
                        file.path()
                    ),
                });
                continue;
            }
            set.frames.insert(index, id);
        }
        if !set.frames.is_empty() {
            self.images.insert(prefix.to_string(), set);
        }
    }

    /// Maps a folder of sounds; the first sound registered under a key wins.
    pub fn map_sounds(&mut self, store: &AssetStore, folder: &DirNode, prefix: &str) {
        let mut table = std::mem::take(&mut self.sounds);
        self.map_files(store, folder, prefix, TYPE_WAV, &mut table, true);
        self.sounds = table;
    }

    /// Maps a folder of animations; a later animation replaces an earlier one
    /// with the same key.
    pub fn map_anis(&mut self, store: &AssetStore, folder: &DirNode, prefix: &str) {
        let mut table = std::mem::take(&mut self.anis);
        self.map_files(store, folder, prefix, TYPE_ANI, &mut table, false);
        self.anis = table;
    }

    fn map_files(
        &mut self,
        store: &AssetStore,
        folder: &DirNode,
        prefix: &str,
        type_code: u32,
        table: &mut BTreeMap<String, FileId>,
        first_wins: bool,
    ) {
        for sub in folder.dirs() {
            let key = registry_key(prefix, sub.name());
            self.map_files(store, sub, &key, type_code, table, first_wins);
        }
        for id in folder.files_of_type(type_code) {
            let file = store.file(id);
            let key = registry_key(prefix, file.name());
            if let Some(&old) = table.get(&key) {
                let (kind, detail) = if first_wins {
                    (
                        RegistryEventKind::SoundKept,
                        format!("{} kept, {} skipped", store.file(old).path(), file.path()),
                    )
                } else {
                    (
                        RegistryEventKind::AnimationReplaced,
                        format!("{} replaced by {}", store.file(old).path(), file.path()),
                    )
                };
                self.events.push(RegistryEvent {
                    kind,
                    key: key.clone(),
                    detail,
                });
                if first_wins {
                    continue;
                }
            }
            table.insert(key, id);
        }
    }

    /// Decodes the image set registered under `key`.
    pub fn load_image_set(&self, store: &AssetStore, key: &str) -> Option<ImageSet> {
        self.images.get(key).map(|set| ImageSet::load(store, set))
    }
}
