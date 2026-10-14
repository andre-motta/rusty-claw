//! Decoded image sets: the frames of one registry key, by frame number.

use std::collections::BTreeMap;

use crate::error::AssetError;
use crate::name::TYPE_PID;
use crate::pid;
use crate::registry::ImageSetRef;
use crate::store::AssetStore;

/// One decoded frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Horizontal offset of the image relative to its object.
    pub offset_x: i32,
    /// Vertical offset of the image relative to its object.
    pub offset_y: i32,
    /// Image flags (see [`pid`]).
    pub flags: u32,
    /// Palette indices, row-major.
    pub indices: Vec<u8>,
    /// For sprite RLE frames, 1 where the image has a pixel and 0 where it
    /// is transparent.
    pub opaque: Option<Vec<u8>>,
}

/// An image set with its frames decoded.
#[derive(Debug, Clone, Default)]
pub struct ImageSet {
    /// Registry key.
    pub key: String,
    /// Frames by number.
    pub frames: BTreeMap<i32, Frame>,
    /// Frames that could not be decoded, with the reason.
    pub skipped: Vec<(i32, String)>,
}

impl ImageSet {
    /// Decodes every frame of a registry image set. A frame that cannot be
    /// read or decoded is listed in `skipped` instead of failing the set.
    pub fn load(store: &AssetStore, set: &ImageSetRef) -> ImageSet {
        let mut out = ImageSet {
            key: set.key.clone(),
            ..ImageSet::default()
        };
        for (&index, &id) in &set.frames {
            let file = store.file(id);
            if file.type_code() != TYPE_PID {
                out.skipped.push((
                    index,
                    format!("{}: {} images are not supported", file.path(), file.ext()),
                ));
                continue;
            }
            match store.read(id).and_then(|data| pid::decode(&data)) {
                Ok(img) => {
                    out.frames.insert(
                        index,
                        Frame {
                            width: img.width,
                            height: img.height,
                            offset_x: img.offset_x,
                            offset_y: img.offset_y,
                            flags: img.flags,
                            indices: img.pixels,
                            opaque: img.opaque,
                        },
                    );
                }
                Err(e) => out.skipped.push((index, format!("{}: {e}", file.path()))),
            }
        }
        out
    }

    /// Decodes one frame of a registry image set.
    pub fn load_frame(
        store: &AssetStore,
        set: &ImageSetRef,
        index: i32,
    ) -> Result<Frame, AssetError> {
        let id = *set
            .frames
            .get(&index)
            .ok_or_else(|| AssetError::NotFound(format!("{} frame {index}", set.key)))?;
        let img = pid::decode(&store.read(id)?)?;
        Ok(Frame {
            width: img.width,
            height: img.height,
            offset_x: img.offset_x,
            offset_y: img.offset_y,
            flags: img.flags,
            indices: img.pixels,
            opaque: img.opaque,
        })
    }
}
