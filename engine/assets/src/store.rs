//! The layered asset store.
//!
//! An [`AssetStore`] merges any number of sources into one tree of folders
//! and files. Sources are mounted in order and a later mount wins: a file
//! replaces an earlier file with the same folder, name and type, and folders
//! present in both merge entry by entry. A community custom level is the
//! exception: each of its top-level folders replaces (shadows) the earlier
//! folder of the same name as a whole.
//!
//! Lookups are case-insensitive, accept `/` or `\`, and are confined to the
//! store: a logical path can never name anything outside the mounted sources.
//! A folder mount never follows a symbolic link that leaves its root and
//! never descends through a linked folder.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::bytes::{slice, to_usize};
use crate::error::{AssetError, Result, format_error, io_error};
use crate::name::{fold, is_single_byte, split_filename, split_logical, type_ext, upper};
use crate::rez::{self, RezEntry};

/// Largest single file the store reads.
pub const MAX_FILE_SIZE: u64 = 256 << 20;
/// Deepest folder nesting a folder mount descends into.
pub const MAX_SCAN_DEPTH: usize = 64;
/// Most entries one folder mount accepts.
pub const MAX_SCAN_ENTRIES: usize = 1 << 21;

/// A source to mount.
#[derive(Debug, Clone)]
pub enum Source {
    /// A REZ archive on disk.
    Rez(PathBuf),
    /// A REZ archive already in memory.
    RezBytes(Vec<u8>),
    /// An extracted folder tree mounted at the root.
    Dir(PathBuf),
    /// A community custom level: the path of its `.WWD` file. The folder of
    /// the same name next to it, if present, is mounted at the root and
    /// shadows same-named top-level folders.
    CustomLevel(PathBuf),
}

/// Handle of a file in an [`AssetStore`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(u32);

#[derive(Debug, Clone)]
enum Location {
    Disk(PathBuf),
    Archive { archive: usize, offset: u64 },
}

/// A file in the store.
#[derive(Debug, Clone)]
pub struct FileEntry {
    name: String,
    type_code: u32,
    size: u64,
    mount: usize,
    path: String,
    location: Location,
}

impl FileEntry {
    /// Name without extension, as stored (upper case for folder mounts,
    /// as spelled for archives).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Type code packed from the extension.
    pub fn type_code(&self) -> u32 {
        self.type_code
    }

    /// Extension text.
    pub fn ext(&self) -> String {
        type_ext(self.type_code)
    }

    /// Size in bytes when mounted.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Logical path, for messages.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Index of the mount that supplied this file.
    pub fn mount(&self) -> usize {
        self.mount
    }
}

/// A folder in the store.
#[derive(Debug, Clone, Default)]
pub struct DirNode {
    name: String,
    path: String,
    dirs: BTreeMap<String, DirNode>,
    files: BTreeMap<(u32, String), FileId>,
}

impl DirNode {
    fn new(name: String, parent_path: &str) -> Self {
        let path = if parent_path.is_empty() {
            name.clone()
        } else {
            format!("{parent_path}/{name}")
        };
        DirNode {
            name,
            path,
            ..DirNode::default()
        }
    }

    /// Folder name as stored.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Logical path of the folder.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Subfolder by name, any case.
    pub fn subdir(&self, name: &str) -> Option<&DirNode> {
        self.dirs.get(&fold(name))
    }

    /// File by name without extension (any case) and type code.
    pub fn file(&self, name: &str, type_code: u32) -> Option<FileId> {
        self.files.get(&(type_code, fold(name))).copied()
    }

    /// Subfolders in a stable order.
    pub fn dirs(&self) -> impl Iterator<Item = &DirNode> {
        self.dirs.values()
    }

    /// Files in a stable order (by type, then name).
    pub fn files(&self) -> impl Iterator<Item = FileId> + '_ {
        self.files.values().copied()
    }

    /// Files of one type in a stable order.
    pub fn files_of_type(&self, type_code: u32) -> impl Iterator<Item = FileId> + '_ {
        self.files
            .range((type_code, String::new())..)
            .take_while(move |((t, _), _)| *t == type_code)
            .map(|(_, id)| *id)
    }

    /// Number of files and folders below this folder, recursively.
    pub fn count(&self) -> (usize, usize) {
        let mut files = self.files.len();
        let mut dirs = self.dirs.len();
        for d in self.dirs.values() {
            let (f, n) = d.count();
            files += f;
            dirs += n;
        }
        (files, dirs)
    }

    fn ensure_dir(&mut self, name: &str) -> &mut DirNode {
        let parent = self.path.clone();
        self.dirs
            .entry(fold(name))
            .or_insert_with(|| DirNode::new(name.to_string(), &parent))
    }
}

/// Something a mount skipped or replaced, kept for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// What happened.
    pub kind: NoteKind,
    /// The path involved.
    pub detail: String,
}

/// Kinds of [`Note`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteKind {
    /// A name that is not single-byte text was skipped.
    UnencodableName,
    /// A symbolic link that leaves the mount root was skipped.
    LinkEscape,
    /// A symbolic link to a folder was not descended into.
    LinkedFolderSkipped,
    /// A symbolic link whose target does not exist was skipped.
    BrokenLink,
    /// A folder could not be listed.
    Unreadable,
    /// Two names in one folder differ only by case; the first one was kept.
    CaseDuplicate,
    /// An archive repeats a file in one folder; the first one was kept.
    DuplicateFile,
    /// An archive name that cannot be addressed (empty, `.`, `..` or with a
    /// separator) was skipped.
    UnusableName,
    /// Folder nesting deeper than [`MAX_SCAN_DEPTH`] was not scanned.
    TooDeep,
    /// The scan stopped at [`MAX_SCAN_ENTRIES`].
    TooManyEntries,
    /// A later mount replaced a file.
    Replaced,
    /// A custom level folder shadowed an earlier folder.
    Shadowed,
}

#[derive(Debug, Clone)]
enum Archive {
    File(PathBuf),
    Bytes(Arc<[u8]>),
}

#[derive(Debug, Clone)]
struct MountInfo {
    label: String,
    /// Canonical root of a folder mount, used to re-check confinement when a
    /// file is read.
    root: Option<PathBuf>,
}

/// A mounted community custom level.
#[derive(Debug, Clone)]
pub struct CustomLevel {
    /// Path of the level's `.WWD` file.
    pub wwd_path: PathBuf,
    /// Level name: the file name without its extension.
    pub name: String,
    /// The level's own folder, if it has one.
    pub folder: Option<PathBuf>,
    /// The level folder's own tree (empty without a folder).
    pub tree: DirNode,
}

/// The layered asset store. See the module documentation.
#[derive(Debug, Clone, Default)]
pub struct AssetStore {
    root: DirNode,
    files: Vec<FileEntry>,
    mounts: Vec<MountInfo>,
    archives: Vec<Archive>,
    notes: Vec<Note>,
    custom: Option<CustomLevel>,
}

impl AssetStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mounts a source over everything mounted so far.
    pub fn mount(&mut self, source: Source) -> Result<()> {
        let index = self.mounts.len();
        match source {
            Source::Rez(path) => {
                let size = fs::metadata(&path).map_err(|e| io_error(&path, e))?.len();
                let archive = self.archives.len();
                self.archives.push(Archive::File(path.clone()));
                let tree = self.build_rez(archive, size, index)?;
                self.mounts.push(MountInfo {
                    label: format!("archive {}", path.display()),
                    root: None,
                });
                self.merge(tree, false);
            }
            Source::RezBytes(data) => {
                let size = data.len() as u64;
                let archive = self.archives.len();
                self.archives.push(Archive::Bytes(data.into()));
                let tree = self.build_rez(archive, size, index)?;
                self.mounts.push(MountInfo {
                    label: "archive in memory".to_string(),
                    root: None,
                });
                self.merge(tree, false);
            }
            Source::Dir(path) => {
                let root = canonical_dir(&path)?;
                let tree = self.build_dir(&root, index);
                self.mounts.push(MountInfo {
                    label: format!("folder {}", path.display()),
                    root: Some(root),
                });
                self.merge(tree, false);
            }
            Source::CustomLevel(wwd_path) => {
                let meta = fs::metadata(&wwd_path).map_err(|e| io_error(&wwd_path, e))?;
                if !meta.is_file() {
                    return Err(AssetError::NotFound(wwd_path.display().to_string()));
                }
                let file_name = wwd_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| AssetError::Invalid("custom level name is not text".into()))?;
                let name = match file_name.rfind('.') {
                    Some(dot) if dot > 0 => file_name[..dot].to_string(),
                    _ => file_name.to_string(),
                };
                let parent = wwd_path.parent().unwrap_or(Path::new("."));
                let folder = find_child(parent, &name, true);
                let (tree, root) = match &folder {
                    Some(dir) => {
                        let root = canonical_dir(dir)?;
                        (self.build_dir(&root, index), Some(root))
                    }
                    None => (DirNode::default(), None),
                };
                self.mounts.push(MountInfo {
                    label: format!("custom level {name}"),
                    root,
                });
                self.custom = Some(CustomLevel {
                    wwd_path,
                    name,
                    folder,
                    tree: tree.clone(),
                });
                self.merge(tree, true);
            }
        }
        Ok(())
    }

    /// The root folder.
    pub fn root(&self) -> &DirNode {
        &self.root
    }

    /// The most recently mounted custom level, if any.
    pub fn custom_level(&self) -> Option<&CustomLevel> {
        self.custom.as_ref()
    }

    /// Notes recorded while mounting: skipped entries and replacements.
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// Label of a mount, for messages.
    pub fn mount_label(&self, mount: usize) -> Option<&str> {
        self.mounts.get(mount).map(|m| m.label.as_str())
    }

    /// The entry behind a file handle.
    ///
    /// # Panics
    ///
    /// Panics when `id` comes from another store.
    pub fn file(&self, id: FileId) -> &FileEntry {
        &self.files[id.0 as usize]
    }

    /// Folder at a logical path; the empty path is the root.
    pub fn folder(&self, path: &str) -> Result<Option<&DirNode>> {
        let mut node = &self.root;
        for seg in split_logical(path)? {
            match node.subdir(seg) {
                Some(next) => node = next,
                None => return Ok(None),
            }
        }
        Ok(Some(node))
    }

    /// File at a logical path such as `LEVEL1/PALETTES/MAIN.PAL`.
    pub fn lookup(&self, path: &str) -> Result<Option<FileId>> {
        let segs = split_logical(path)?;
        let Some((leaf, dirs)) = segs.split_last() else {
            return Err(AssetError::Path {
                path: path.to_string(),
                reason: "empty path",
            });
        };
        if !is_single_byte(leaf) {
            return Ok(None);
        }
        let (name, type_code) = split_filename(leaf);
        Ok(self.walk(dirs).and_then(|d| d.file(&name, type_code)))
    }

    /// File at a logical path without extension plus an explicit type code,
    /// such as (`LEVEL1/PALETTES/MAIN`, PAL).
    pub fn lookup_typed(&self, path: &str, type_code: u32) -> Result<Option<FileId>> {
        let segs = split_logical(path)?;
        let Some((leaf, dirs)) = segs.split_last() else {
            return Err(AssetError::Path {
                path: path.to_string(),
                reason: "empty path",
            });
        };
        Ok(self
            .walk(dirs)
            .and_then(|d| d.file(&upper(leaf), type_code)))
    }

    fn walk(&self, dirs: &[&str]) -> Option<&DirNode> {
        let mut node = &self.root;
        for seg in dirs {
            node = node.subdir(seg)?;
        }
        Some(node)
    }

    /// Reads the file at a logical path.
    pub fn open(&self, path: &str) -> Result<Vec<u8>> {
        match self.lookup(path)? {
            Some(id) => self.read(id),
            None => Err(AssetError::NotFound(path.to_string())),
        }
    }

    /// Reads a file by handle.
    pub fn read(&self, id: FileId) -> Result<Vec<u8>> {
        let entry = self.file(id);
        if entry.size > MAX_FILE_SIZE {
            return Err(AssetError::Invalid(format!(
                "{} is larger than {MAX_FILE_SIZE} bytes",
                entry.path
            )));
        }
        match &entry.location {
            Location::Disk(real) => {
                let root = self.mounts.get(entry.mount).and_then(|m| m.root.as_ref());
                read_confined(real, root)
            }
            Location::Archive { archive, offset } => {
                read_archive(&self.archives[*archive], *offset, entry.size)
            }
        }
    }

    fn push_file(&mut self, entry: FileEntry) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(entry);
        id
    }

    fn note(&mut self, kind: NoteKind, detail: impl Into<String>) {
        self.notes.push(Note {
            kind,
            detail: detail.into(),
        });
    }

    fn merge(&mut self, src: DirNode, shadow: bool) {
        let mut root = std::mem::take(&mut self.root);
        self.merge_into(&mut root, src, shadow);
        self.root = root;
    }

    fn merge_into(&mut self, dst: &mut DirNode, src: DirNode, shadow: bool) {
        for (key, id) in src.files {
            if let Some(old) = dst.files.insert(key, id) {
                let detail = format!(
                    "{} from {} replaced by {}",
                    self.file(id).path,
                    self.label_of(old),
                    self.label_of(id)
                );
                self.note(NoteKind::Replaced, detail);
            }
        }
        for (key, sub) in src.dirs {
            if shadow || !dst.dirs.contains_key(&key) {
                if dst.dirs.contains_key(&key) {
                    self.note(NoteKind::Shadowed, sub.path.clone());
                }
                let mut sub = sub;
                rebase(&mut sub, &dst.path);
                dst.dirs.insert(key, sub);
            } else if let Some(existing) = dst.dirs.get_mut(&key) {
                self.merge_into(existing, sub, false);
            }
        }
    }

    fn label_of(&self, id: FileId) -> String {
        self.mount_label(self.file(id).mount)
            .unwrap_or("?")
            .to_string()
    }

    fn build_rez(&mut self, archive: usize, size: u64, mount: usize) -> Result<DirNode> {
        let source = self.archives[archive].clone();
        let head = read_archive(&source, 0, size.min(rez::HEADER_SIZE as u64))?;
        let header = rez::parse_header(&head)?;
        let mut root = DirNode::default();
        if header.dir_size != 0 {
            let mut state = RezWalk {
                source: &source,
                archive,
                size,
                mount,
                seen: HashSet::new(),
                entries: 0,
            };
            let mut files = Vec::new();
            let mut notes = Vec::new();
            state.block(
                &mut root,
                header.dir_offset,
                header.dir_size,
                0,
                &mut files,
                &mut notes,
                self.files.len(),
            )?;
            self.files.extend(files);
            self.notes.extend(notes);
        }
        Ok(root)
    }

    fn build_dir(&mut self, root: &Path, mount: usize) -> DirNode {
        let mut tree = DirNode::default();
        let mut scan = DirScan {
            root,
            mount,
            entries: 0,
            stopped: false,
        };
        scan.scan(self, root, &mut tree, 0);
        tree
    }
}

/// Gives a subtree moved under a new parent its logical paths.
fn rebase(node: &mut DirNode, parent_path: &str) {
    node.path = if parent_path.is_empty() {
        node.name.clone()
    } else {
        format!("{parent_path}/{}", node.name)
    };
    let path = node.path.clone();
    for sub in node.dirs.values_mut() {
        rebase(sub, &path);
    }
}

struct RezWalk<'a> {
    source: &'a Archive,
    archive: usize,
    size: u64,
    mount: usize,
    seen: HashSet<(u32, u32)>,
    entries: usize,
}

impl RezWalk<'_> {
    #[allow(clippy::too_many_arguments)]
    fn block(
        &mut self,
        node: &mut DirNode,
        offset: u32,
        size: u32,
        depth: usize,
        files: &mut Vec<FileEntry>,
        notes: &mut Vec<Note>,
        first_id: usize,
    ) -> Result<()> {
        const FORMAT: &str = "REZ archive";
        if depth > rez::MAX_DEPTH {
            return Err(format_error(FORMAT, "folders nested too deeply"));
        }
        if !self.seen.insert((offset, size)) {
            return Err(format_error(FORMAT, "a folder block is reached twice"));
        }
        if to_usize(size) > rez::MAX_BLOCK_SIZE {
            return Err(format_error(FORMAT, "folder block too large"));
        }
        if u64::from(offset) + u64::from(size) > self.size {
            return Err(format_error(
                FORMAT,
                "folder block lies outside the archive",
            ));
        }
        let block = read_archive(self.source, u64::from(offset), u64::from(size))?;
        let entries = rez::parse_block(&block, self.size)?;
        self.entries += entries.len();
        if self.entries > rez::MAX_ENTRIES {
            return Err(format_error(FORMAT, "too many entries"));
        }
        // A repeated folder entry updates the location of the earlier one.
        let mut pending: Vec<(String, u32, u32)> = Vec::new();
        for entry in entries {
            match entry {
                RezEntry::Directory {
                    name, offset, size, ..
                } => {
                    if !usable_archive_name(&name) {
                        notes.push(Note {
                            kind: NoteKind::UnusableName,
                            detail: format!("{}/{name}", node.path),
                        });
                        continue;
                    }
                    node.ensure_dir(&name);
                    let key = fold(&name);
                    pending.retain(|(k, _, _)| *k != key);
                    pending.push((key, offset, size));
                }
                RezEntry::File {
                    name,
                    type_code,
                    offset,
                    size,
                    ..
                } => {
                    let path = join_path(&node.path, &display_name(&name, type_code));
                    if !usable_archive_name(&name) {
                        notes.push(Note {
                            kind: NoteKind::UnusableName,
                            detail: path,
                        });
                        continue;
                    }
                    let key = (type_code, fold(&name));
                    if node.files.contains_key(&key) {
                        notes.push(Note {
                            kind: NoteKind::DuplicateFile,
                            detail: path,
                        });
                        continue;
                    }
                    let id = FileId((first_id + files.len()) as u32);
                    files.push(FileEntry {
                        name,
                        type_code,
                        size: u64::from(size),
                        mount: self.mount,
                        path,
                        location: Location::Archive {
                            archive: self.archive,
                            offset: u64::from(offset),
                        },
                    });
                    node.files.insert(key, id);
                }
            }
        }
        for (key, offset, size) in pending {
            if offset == 0 {
                continue;
            }
            if let Some(child) = node.dirs.get_mut(&key) {
                self.block(child, offset, size, depth + 1, files, notes, first_id)?;
            }
        }
        Ok(())
    }
}

fn usable_archive_name(name: &str) -> bool {
    !(name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']))
}

fn display_name(name: &str, type_code: u32) -> String {
    if type_code == 0 {
        name.to_string()
    } else {
        format!("{name}.{}", type_ext(type_code))
    }
}

fn join_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

struct DirScan<'a> {
    root: &'a Path,
    mount: usize,
    entries: usize,
    stopped: bool,
}

impl DirScan<'_> {
    fn scan(&mut self, store: &mut AssetStore, real: &Path, node: &mut DirNode, depth: usize) {
        let listing = match fs::read_dir(real) {
            Ok(listing) => listing,
            Err(e) => {
                store.note(NoteKind::Unreadable, format!("{}: {e}", real.display()));
                return;
            }
        };
        let mut entries: Vec<fs::DirEntry> = listing.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if self.stopped {
                return;
            }
            self.entries += 1;
            if self.entries > MAX_SCAN_ENTRIES {
                store.note(NoteKind::TooManyEntries, real.display().to_string());
                self.stopped = true;
                return;
            }
            let entry_path = entry.path();
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                store.note(NoteKind::UnencodableName, entry_path.display().to_string());
                continue;
            };
            if !is_single_byte(&name) {
                store.note(NoteKind::UnencodableName, entry_path.display().to_string());
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                store.note(NoteKind::Unreadable, entry_path.display().to_string());
                continue;
            };
            let (is_dir, is_file) = if file_type.is_symlink() {
                match fs::canonicalize(&entry_path) {
                    Err(_) => {
                        store.note(NoteKind::BrokenLink, entry_path.display().to_string());
                        continue;
                    }
                    Ok(target) if !target.starts_with(self.root) => {
                        store.note(NoteKind::LinkEscape, entry_path.display().to_string());
                        continue;
                    }
                    Ok(target) => {
                        if target.is_dir() {
                            store.note(
                                NoteKind::LinkedFolderSkipped,
                                entry_path.display().to_string(),
                            );
                            continue;
                        }
                        (false, target.is_file())
                    }
                }
            } else {
                (file_type.is_dir(), file_type.is_file())
            };
            if is_dir {
                if depth + 1 > MAX_SCAN_DEPTH {
                    store.note(NoteKind::TooDeep, entry_path.display().to_string());
                    continue;
                }
                let child = node.ensure_dir(&upper(&name));
                self.scan(store, &entry_path, child, depth + 1);
            } else if is_file {
                let (stem, type_code) = split_filename(&name);
                let key = (type_code, fold(&stem));
                let path = join_path(&node.path, &name);
                if node.files.contains_key(&key) {
                    store.note(NoteKind::CaseDuplicate, path);
                    continue;
                }
                let size = match fs::metadata(&entry_path) {
                    Ok(meta) => meta.len(),
                    Err(e) => {
                        store.note(NoteKind::Unreadable, format!("{path}: {e}"));
                        continue;
                    }
                };
                let id = store.push_file(FileEntry {
                    name: stem,
                    type_code,
                    size,
                    mount: self.mount,
                    path,
                    location: Location::Disk(entry_path),
                });
                node.files.insert(key, id);
            }
        }
    }
}

fn canonical_dir(path: &Path) -> Result<PathBuf> {
    let real = fs::canonicalize(path).map_err(|e| io_error(path, e))?;
    if !real.is_dir() {
        return Err(AssetError::Invalid(format!(
            "{} is not a folder",
            path.display()
        )));
    }
    Ok(real)
}

/// Finds a direct child of `parent` by name, any case. Symbolic links are
/// not followed.
pub(crate) fn find_child(parent: &Path, name: &str, want_dir: bool) -> Option<PathBuf> {
    let want = fold(name);
    let mut found: Vec<(std::ffi::OsString, PathBuf)> = fs::read_dir(parent)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let Ok(t) = e.file_type() else { return false };
            let kind_ok = if want_dir { t.is_dir() } else { t.is_file() };
            kind_ok && e.file_name().to_str().is_some_and(|n| fold(n) == want)
        })
        .map(|e| (e.file_name(), e.path()))
        .collect();
    found.sort();
    found.into_iter().next().map(|(_, p)| p)
}

fn read_confined(real: &Path, root: Option<&PathBuf>) -> Result<Vec<u8>> {
    if let Some(root) = root {
        let resolved = fs::canonicalize(real).map_err(|e| io_error(real, e))?;
        if !resolved.starts_with(root) {
            return Err(AssetError::Path {
                path: real.display().to_string(),
                reason: "resolves outside its mount",
            });
        }
    }
    let file = fs::File::open(real).map_err(|e| io_error(real, e))?;
    let mut data = Vec::new();
    file.take(MAX_FILE_SIZE + 1)
        .read_to_end(&mut data)
        .map_err(|e| io_error(real, e))?;
    if data.len() as u64 > MAX_FILE_SIZE {
        return Err(AssetError::Invalid(format!(
            "{} is larger than {MAX_FILE_SIZE} bytes",
            real.display()
        )));
    }
    Ok(data)
}

fn read_archive(archive: &Archive, offset: u64, size: u64) -> Result<Vec<u8>> {
    match archive {
        Archive::Bytes(data) => {
            let start = usize::try_from(offset).ok();
            let len = usize::try_from(size).ok();
            start
                .zip(len)
                .and_then(|(s, l)| slice(data, s, l))
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format_error("REZ archive", "read outside the archive"))
        }
        Archive::File(path) => {
            let mut file = fs::File::open(path).map_err(|e| io_error(path, e))?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| io_error(path, e))?;
            let mut data = Vec::new();
            file.take(size)
                .read_to_end(&mut data)
                .map_err(|e| io_error(path, e))?;
            if data.len() as u64 != size {
                return Err(format_error("REZ archive", "short read"));
            }
            Ok(data)
        }
    }
}
