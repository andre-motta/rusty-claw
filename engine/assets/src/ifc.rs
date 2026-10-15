//! Menu tree files (to be filled in by the menu reader).
//!
//! The game describes each of its menus (the main menu, the pause menu and
//! the game-over menu) in a small binary file: a tree of nodes, each with a
//! label, up to eight image names for its looks, the command number the
//! game runs when the entry is chosen, and child nodes for submenus. The
//! reader will turn a file into that tree for the menu screens. Like every
//! reader in this crate it will be bounded, never panic and treat its input
//! as untrusted; nothing in these files is ever executed.
