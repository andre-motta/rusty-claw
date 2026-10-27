//! Palette renderer: indexed images composed on the GPU, with the palette
//! applied in the final pass.
//!
//! The game draws 8-bit palette indices, never colours, and its fades and
//! translucency effects work on those indices. This renderer keeps that
//! model on the GPU so every effect stays exact at any output resolution:
//!
//! - [`images`]: tiles and sprite frames as index images, and the graphics
//!   of one level ([`LevelGraphics`]);
//! - [`view`]: which part of each plane is visible for a camera position;
//! - [`scene`]: one frame as an ordered list of plane and sprite draws, with
//!   the composition and output geometry for a [`ViewConfig`];
//! - [`screen`]: the screen layer (sprites pinned to the picture's edges,
//!   such as the status bar), standalone 640x480 screens and screen
//!   transitions;
//! - [`effects`] and [`palette`]: draw modes, lookup tables and fades;
//! - [`gpu`]: the Vulkan renderer;
//! - [`cpu`]: a pixel-exact reference of everything the shaders do;
//! - [`movie`]: movie pictures, drawn centred at their display aspect;
//! - [`pack`]: optional higher-resolution replacement images.
//!
//! [`ViewConfig`]: claw_frame::ViewConfig

#![forbid(unsafe_code)]

pub mod cpu;
pub mod effects;
pub mod gpu;
pub mod images;
pub mod movie;
pub mod pack;
pub mod palette;
pub mod scene;
pub mod screen;
pub mod view;

pub use gpu::{Gpu, GpuError, Offscreen, Renderer};
pub use images::{FrameInfo, ImageSource, LevelGraphics, MemoryImages, StoreImages};
pub use movie::{MovieDrawError, MoviePicture, MoviePresenter, PictureLayout, movie_rect};
pub use pack::HdPack;
pub use scene::{Composition, FrameInput, Scene, SceneBuilder, SpriteInput};
pub use screen::{Anchor, ImageId, ScreenBuilder, ScreenScene, ScreenSprite};
pub use view::{PlaneView, WorldView};
pub use wgpu;
