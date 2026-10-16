//! Menus: trees from the game's menu files, what each entry does, where it
//! sits, moving through them and what they show.
//!
//! - [`command`]: the command catalogue, numbers to [`MenuCommand`]s;
//! - [`tree`]: [`MenuTree`], the entries with their images and submenus;
//! - [`layout`]: where the entries of a submenu sit;
//! - [`nav`]: [`Navigator`], the highlight, choosing, sliders and held
//!   keys;
//! - [`draw`]: the screen sprites of a menu;
//! - [`decor`]: the main menu's sparkles.
//!
//! ```
//! use claw_assets::ifc::IfcNode;
//! use claw_ui::menu::{CommandCatalogue, MenuKind, MenuTree, Navigator};
//!
//! let mut root = IfcNode { command: 10000, ..IfcNode::default() };
//! root.children.push(IfcNode { command: 10006, ..IfcNode::default() });
//! let tree = MenuTree::from_ifc(root, &CommandCatalogue::new(MenuKind::Main));
//! let nav = Navigator::new(tree);
//! assert_eq!(nav.entries().len(), 1);
//! ```

pub mod command;
pub mod decor;
pub mod draw;
pub mod layout;
pub mod nav;
pub mod tree;

pub use command::{
    CatalogueEntry, CommandCatalogue, GameplaySetting, MenuCommand, MenuKind, Page, Slider, Toggle,
    Unavailable, VideoSetting,
};
pub use draw::{Art, FontArt, MenuState, Switches};
pub use nav::{Look, MenuAction, MenuInput, Navigator};
pub use tree::{Cell, ItemId, ItemImage, MenuItem, MenuTree};
