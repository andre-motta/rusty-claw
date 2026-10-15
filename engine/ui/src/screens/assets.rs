//! The art and sounds of the standalone screens, read from the install.
//!
//! The screens' registries map:
//!
//! - images: the menu's (`MENU_...`), the dialogs' (`DIALOGS_...`), the
//!   end-of-level screens' (`BOOTY_...`) and the game's (`GAME_...`, for
//!   the bitmap font);
//! - sounds: the menu's and the title's (no prefix) and the end-of-level
//!   screens' (`BOOTY_...`);
//! - animations: the menu's (`MENU_...`) and the end-of-level screens'
//!   (`BOOTY_...`).
//!
//! Menu entries are pictures of the PCX kind as well as the usual sprite
//! images; both are read, and palette index 0 of a picture is transparent.
//! Full-screen pictures (`SCREENS/...`) come with their own palette.

use std::collections::BTreeMap;

use claw_assets::ifc::{self, IfcNode, TYPE_IFC};
use claw_assets::name::{TYPE_PCX, TYPE_PID};
use claw_assets::pcx::{self, IndexedImage};
use claw_assets::pid::{self, FLAG_COLOR_KEY};
use claw_assets::{AssetStore, Frame, Palette, Registries, wav};
use claw_render::ImageSource;
use claw_render::images::{MAX_IMAGE_DIMENSION, SourceFrame};

use crate::menu::command::MenuKind;

/// Where each menu file is.
pub fn menu_file(kind: MenuKind) -> (&'static str, &'static str) {
    match kind {
        MenuKind::Main => ("STATES/MENU", "CLAW"),
        MenuKind::InGame => ("GAME", "INGAME"),
        MenuKind::GameOver => ("GAME", "GAMEOVER"),
    }
}

/// Folder of the menu's pictures.
pub const MENU_SCREENS: &str = "STATES/MENU/SCREENS";
/// The main menu's picture.
pub const MENU_PICTURE: &str = "MENU";
/// Folder of the title's pictures.
pub const TITLE_SCREENS: &str = "STATES/ATTRACT/SCREENS";
/// The title's picture.
pub const TITLE_PICTURE: &str = "TITLE";
/// Folder of the end-of-level pictures.
pub const BOOTY_SCREENS: &str = "STATES/BOOTY/SCREENS";

/// Decodes an image file of the screens: a sprite image or a picture.
pub fn decode_frame(data: &[u8], type_code: u32) -> Option<Frame> {
    if type_code == TYPE_PID {
        let img = pid::decode(data).ok()?;
        return Some(Frame {
            width: img.width,
            height: img.height,
            offset_x: img.offset_x,
            offset_y: img.offset_y,
            flags: img.flags,
            indices: img.pixels,
            opaque: img.opaque,
        });
    }
    if type_code == TYPE_PCX {
        let (img, _) = pcx::decode(data).ok()?;
        let mut f = picture_frame(&img);
        f.flags = FLAG_COLOR_KEY;
        return Some(f);
    }
    None
}

/// A full-screen picture as an opaque frame.
pub fn picture_frame(img: &IndexedImage) -> Frame {
    Frame {
        width: img.width,
        height: img.height,
        offset_x: 0,
        offset_y: 0,
        flags: 0,
        indices: img.pixels.clone(),
        opaque: None,
    }
}

/// Images of the screens' registries, pictures included.
#[derive(Debug, Clone)]
pub struct ScreenImages {
    store: AssetStore,
    registries: Registries,
}

impl ScreenImages {
    /// Images of `registries` read from `store`.
    pub fn new(store: AssetStore, registries: Registries) -> ScreenImages {
        ScreenImages { store, registries }
    }
}

impl ImageSource for ScreenImages {
    fn image_set(&self, key: &str) -> Option<Vec<SourceFrame>> {
        let set = self.registries.images.get(key)?;
        let mut out = Vec::with_capacity(set.frames.len());
        for (&number, &id) in &set.frames {
            let file = self.store.file(id);
            let Ok(data) = self.store.read(id) else {
                continue;
            };
            let Some(frame) = decode_frame(&data, file.type_code()) else {
                continue;
            };
            if frame.width > MAX_IMAGE_DIMENSION || frame.height > MAX_IMAGE_DIMENSION {
                continue;
            }
            out.push(SourceFrame {
                number,
                frame,
                path: Some(file.path().to_string()),
            });
        }
        Some(out)
    }
}

/// The screens' art and sounds.
#[derive(Debug, Clone)]
pub struct ScreenAssets {
    /// The install's assets.
    pub store: AssetStore,
    /// Images, sounds and animations of the screens.
    pub registries: Registries,
    /// Length of every sound in `registries`, ms.
    pub sound_ms: BTreeMap<String, u32>,
    /// What could not be found.
    pub missing: Vec<String>,
}

impl ScreenAssets {
    /// Maps the screens' folders of `store`.
    pub fn load(store: AssetStore) -> ScreenAssets {
        let mut reg = Registries::new();
        let mut missing = Vec::new();
        let maps: [(&str, &str, u8); 10] = [
            ("STATES/MENU/IMAGES", "MENU", 0),
            ("STATES/DIALOGS/IMAGES", "DIALOGS", 0),
            ("STATES/BOOTY/IMAGES", "BOOTY", 0),
            ("GAME/IMAGES", "GAME", 0),
            ("STATES/MENU/SOUNDS", "", 1),
            ("STATES/ATTRACT/SOUNDS", "", 1),
            ("STATES/BOOTY/SOUNDS", "BOOTY", 1),
            ("STATES/MENU/ANIS", "MENU", 2),
            ("STATES/BOOTY/ANIS", "BOOTY", 2),
            ("GAME/ANIS", "GAME", 2),
        ];
        for (path, prefix, what) in maps {
            match store.folder(path) {
                Ok(Some(dir)) => match what {
                    0 => reg.map_images(&store, dir, prefix),
                    1 => reg.map_sounds(&store, dir, prefix),
                    _ => reg.map_anis(&store, dir, prefix),
                },
                _ => missing.push(path.to_string()),
            }
        }
        let mut sound_ms = BTreeMap::new();
        for (key, &id) in &reg.sounds {
            if let Ok(ms) = store.read(id).and_then(|d| wav::duration_ms(&d)) {
                sound_ms.insert(key.clone(), ms);
            }
        }
        ScreenAssets {
            store,
            registries: reg,
            sound_ms,
            missing,
        }
    }

    /// The image source for screen graphics.
    pub fn images(&self) -> ScreenImages {
        ScreenImages::new(self.store.clone(), self.registries.clone())
    }

    /// Reads and parses `kind`'s menu file.
    pub fn menu(&self, kind: MenuKind) -> Result<IfcNode, String> {
        let (dir, name) = menu_file(kind);
        let path = format!("{dir}/{name}");
        let id = self
            .store
            .lookup_typed(&path, TYPE_IFC)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{path}.IFC not found"))?;
        let data = self.store.read(id).map_err(|e| e.to_string())?;
        ifc::parse(&data).map_err(|e| format!("{path}.IFC: {e}"))
    }

    /// The picture `name` in `folder` with its palette.
    pub fn picture(&self, folder: &str, name: &str) -> Option<(IndexedImage, Palette)> {
        let id = self
            .store
            .lookup_typed(&format!("{folder}/{name}"), TYPE_PCX)
            .ok()??;
        let (img, pal) = pcx::decode(&self.store.read(id).ok()?).ok()?;
        Some((img, pal?))
    }

    /// Length of sound `key`, ms.
    pub fn sound_ms(&self, key: &str) -> Option<u32> {
        self.sound_ms.get(key).copied()
    }
}
