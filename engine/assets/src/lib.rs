//! Asset layer: a layered virtual filesystem over the user's own install,
//! readers for the game's file formats, and the level model.
//!
//! ```no_run
//! use claw_assets::{Install, LevelData, LevelSource};
//!
//! let install = Install::open("/path/to/your/install")?;
//! let store = install.store()?;
//! let level = LevelData::build(&store, LevelSource::Retail(1))?;
//! println!("{} planes, {} objects", level.planes.len(), level.objects.len());
//! # Ok::<(), claw_assets::AssetError>(())
//! ```
//!
//! Every reader treats its input as untrusted: sizes, offsets and counts are
//! checked, inflate is bounded, and malformed data returns an
//! [`AssetError`] instead of panicking. Nothing read through this crate is
//! ever executed.

#![forbid(unsafe_code)]

mod bytes;
mod error;

pub mod ani;
pub mod clt;
pub mod image_set;
pub mod install;
pub mod level;
pub mod name;
pub mod pal;
pub mod pid;
pub mod registry;
pub mod rez;
pub mod store;
pub mod wwd;

pub use ani::{Ani, AniFrame};
pub use clt::{Clt, CltKind};
pub use error::{AssetError, Result};
pub use image_set::{Frame, ImageSet};
pub use install::Install;
pub use level::{LevelData, LevelSource, References};
pub use pal::Palette;
pub use pid::PidImage;
pub use registry::{ImageSetRef, Registries};
pub use store::{AssetStore, DirNode, FileEntry, FileId, Source};
pub use wwd::{ObjectRecord, Plane, TileDescriptor, Wwd};
