//! Menu trees built from menu files: entries, their looks and their
//! submenus.
//!
//! Every entry names up to eight images, one per look:
//!
//! | Slot | Shown when the entry is | Taken from when empty |
//! | --- | --- | --- |
//! | 0 | normal | (required) |
//! | 1 | pressed, blink off | slot 3 |
//! | 2 | pressed, blink on | slot 1 |
//! | 3 | highlighted, blink off | (required) |
//! | 4 | highlighted, blink on | slot 3 |
//! | 5 | disabled | slot 0 |
//! | 6 | disabled and highlighted, blink off | slot 3 |
//! | 7 | disabled and highlighted, blink on | slot 6 |
//!
//! Empty slots are filled in that order (1, 2, 4, 5, 6, 7), so slot 2
//! takes slot 1 after slot 1 took slot 3. An image name has the form
//! `NNN_<group>_<name>`: the leading number is the frame and the group picks
//! the image set (see [`image_set_of`]). The game's own menus leave slots 1,
//! 2, 4 and 7 empty, so the highlight is a steady swap from slot 0 to slot
//! 3; community menus may fill the others.
//!
//! An entry's label is not drawn. This game's own pages (Video and the
//! controls list) are entries with text instead of images.

use claw_assets::ifc::{IfcNode, SLOTS};

use super::command::{CommandCatalogue, MenuCommand, MenuKind, Page, Slider, Toggle};

/// Index of an entry in [`MenuTree::items`]; the root is 0.
pub type ItemId = usize;

/// An image: a frame of an image set.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemImage {
    /// Image set key.
    pub set: String,
    /// Frame number.
    pub frame: i32,
}

/// A rectangle on the 640x480 menu canvas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Cell {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

/// One menu entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// The command number from the menu file (0 for this game's own
    /// entries).
    pub id: u32,
    /// The label from the menu file.
    pub label: String,
    /// The eight image names after the empty slots were filled.
    pub slots: [String; SLOTS],
    /// Text drawn instead of images, for this game's own entries.
    pub text: Option<String>,
    /// What choosing it does.
    pub command: MenuCommand,
    /// The slider left and right change.
    pub slider: Option<Slider>,
    /// The letter that chooses it.
    pub hotkey: Option<char>,
    /// The entry above, `None` for the root.
    pub parent: Option<ItemId>,
    /// The submenu's entries.
    pub children: Vec<ItemId>,
    /// False when greyed out.
    pub enabled: bool,
    /// Size of the entry: the largest width and height of its images.
    pub size: (i32, i32),
    /// Where it is drawn when its submenu shows.
    pub cell: Cell,
}

impl MenuItem {
    /// The submenu this entry opens.
    pub fn page(&self) -> Page {
        match self.command {
            MenuCommand::Open(p) => p,
            MenuCommand::Controls => Page::Controls,
            MenuCommand::Unavailable(super::command::Unavailable::Multiplayer) => Page::Multiplayer,
            MenuCommand::Unavailable(super::command::Unavailable::Movies) => Page::Movies,
            _ if self.parent.is_none() => Page::Root,
            _ => Page::Other,
        }
    }

    /// The toggle the entry switches, if any.
    pub fn toggle(&self) -> Option<Toggle> {
        match self.command {
            MenuCommand::Toggle(t) => Some(t),
            _ => None,
        }
    }
}

/// Fills the empty slots of an entry from the others.
pub fn fill_slots(mut slots: [String; SLOTS]) -> [String; SLOTS] {
    for (to, from) in [(1, 3), (2, 1), (4, 3), (5, 0), (6, 3), (7, 6)] {
        if slots[to].is_empty() {
            slots[to] = slots[from].clone();
        }
    }
    slots
}

/// The leading number of `name` as C's `atoi` reads it: optional spaces,
/// an optional sign, then digits; 0 when there are none.
pub fn leading_number(name: &str) -> i32 {
    let s = name.trim_start();
    let (neg, rest) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let mut v: i64 = 0;
    for b in rest.bytes().take_while(u8::is_ascii_digit) {
        v = (v * 10 + i64::from(b - b'0')).min(i64::from(i32::MAX));
    }
    (if neg { -v } else { v }) as i32
}

/// Image groups of the main menu: the group in an image name and its set.
pub const MAIN_GROUPS: &[(&str, &str)] = &[
    (
        "_singleplayerloadsub_",
        "MENU_MAIN_SINGLEPLAYER_LOAD_CHECKPOINTS",
    ),
    ("_singleplayernew_", "MENU_MAIN_SINGLEPLAYER_LEVELS"),
    ("_singleplayerload_", "MENU_MAIN_SINGLEPLAYER_LEVELS"),
    ("_singleplayersave_", "MENU_MAIN_SINGLEPLAYER_SAVE"),
    ("_singleplayer_", "MENU_MAIN_SINGLEPLAYER"),
    ("_multiplayergamelobby_", "MENU_MAIN_MULTIPLAYER_GAMELOBBY"),
    ("_multiplayer_", "MENU_MAIN_MULTIPLAYER"),
    ("_replay_", "MENU_MAIN_REPLAYMOVIES"),
    ("_optionsedit_", "MENU_MAIN_OPTIONS_EDIT"),
    ("_optionsdifficulty_", "MENU_MAIN_OPTIONS_DIFFICULTY"),
    ("_optionsaudio_", "MENU_MAIN_OPTIONS_AUDIO"),
    ("_optionsdisplay_", "MENU_MAIN_OPTIONS_DISPLAY"),
    ("_options_", "MENU_MAIN_OPTIONS"),
    ("_quit_", "MENU_MAIN_QUIT"),
    ("_main_", "MENU_MAIN"),
];

/// Image groups of the in-game menu.
pub const IN_GAME_GROUPS: &[(&str, &str)] = &[
    ("_clawicide_", "GAME_INGAMEMENU_MAIN_CLAWICIDE"),
    ("_optionsaudio_", "GAME_INGAMEMENU_MAIN_OPTIONS_AUDIO"),
    ("_optionsdisplay_", "GAME_INGAMEMENU_MAIN_OPTIONS_DISPLAY"),
    ("_options_", "GAME_INGAMEMENU_MAIN_OPTIONS"),
    ("_endgame_", "GAME_INGAMEMENU_MAIN_ENDGAME"),
    ("_main_", "GAME_INGAMEMENU_MAIN"),
];

/// Image groups of the game-over menu.
pub const GAME_OVER_GROUPS: &[(&str, &str)] = &[("_main_", "GAME_GAMEOVERMENU_MAIN")];

/// The image groups of `kind`'s menu.
pub fn groups(kind: MenuKind) -> &'static [(&'static str, &'static str)] {
    match kind {
        MenuKind::Main => MAIN_GROUPS,
        MenuKind::InGame => IN_GAME_GROUPS,
        MenuKind::GameOver => GAME_OVER_GROUPS,
    }
}

/// The image set and frame an image name stands for in `kind`'s menu, or
/// `None` for an empty name or one without a known group. The group is
/// matched ignoring case.
pub fn image_set_of(kind: MenuKind, name: &str) -> Option<ItemImage> {
    if name.is_empty() {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    let set = groups(kind)
        .iter()
        .find(|(g, _)| lower.contains(g))
        .map(|(_, s)| *s)?;
    Some(ItemImage {
        set: set.to_string(),
        frame: leading_number(name),
    })
}

/// A menu: its entries, the root first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuTree {
    /// Which menu.
    pub kind: MenuKind,
    /// Every entry; children follow the order of the file.
    pub items: Vec<MenuItem>,
}

impl MenuTree {
    /// Builds the tree of `root`, giving every entry its meaning from
    /// `catalogue`.
    pub fn from_ifc(root: IfcNode, catalogue: &CommandCatalogue) -> MenuTree {
        let mut tree = MenuTree {
            kind: catalogue.kind(),
            items: Vec::new(),
        };
        let mut stack: Vec<(IfcNode, Option<ItemId>)> = vec![(root, None)];
        while let Some((node, parent)) = stack.pop() {
            let entry = catalogue.entry(node.command);
            let id = tree.items.len();
            let command = if parent.is_none() {
                MenuCommand::Open(Page::Root)
            } else {
                entry.command
            };
            tree.items.push(MenuItem {
                id: node.command,
                label: node.label,
                slots: fill_slots(node.slots),
                text: None,
                command,
                slider: entry.slider,
                hotkey: entry.hotkey,
                parent,
                children: Vec::new(),
                enabled: true,
                size: (0, 0),
                cell: Cell::default(),
            });
            if let Some(p) = parent {
                tree.items[p].children.push(id);
            }
            for child in node.children.into_iter().rev() {
                stack.push((child, Some(id)));
            }
        }
        tree
    }

    /// The entry `id`.
    pub fn item(&self, id: ItemId) -> &MenuItem {
        &self.items[id]
    }

    /// The first entry, depth first, whose command is `command`.
    pub fn find(&self, command: MenuCommand) -> Option<ItemId> {
        self.items.iter().position(|i| i.command == command)
    }

    /// The first entry that opens `page`.
    pub fn find_page(&self, page: Page) -> Option<ItemId> {
        self.items
            .iter()
            .position(|i| !i.children.is_empty() && i.page() == page)
            .or_else(|| (page == Page::Root).then_some(0))
    }

    /// Adds an entry under `parent` that shows `text`; returns its id.
    pub fn add_text(&mut self, parent: ItemId, text: &str, command: MenuCommand) -> ItemId {
        let id = self.items.len();
        self.items.push(MenuItem {
            id: 0,
            label: text.to_string(),
            slots: Default::default(),
            text: Some(text.to_string()),
            command,
            slider: None,
            hotkey: None,
            parent: Some(parent),
            children: Vec::new(),
            enabled: true,
            size: (0, 0),
            cell: Cell::default(),
        });
        self.items[parent].children.push(id);
        id
    }

    /// Removes the entries under `parent` (they stay in the list but are
    /// no longer reachable).
    pub fn clear_children(&mut self, parent: ItemId) {
        let kids = std::mem::take(&mut self.items[parent].children);
        for k in kids {
            self.items[k].parent = None;
        }
    }

    /// Greys out or enables every entry by `rule`.
    pub fn set_enabled_by(&mut self, mut rule: impl FnMut(&MenuItem) -> bool) {
        for i in 0..self.items.len() {
            let on = rule(&self.items[i]);
            self.items[i].enabled = on;
        }
    }

    /// Every image set the tree's images name.
    pub fn image_sets(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .items
            .iter()
            .flat_map(|i| i.slots.iter())
            .filter_map(|s| image_set_of(self.kind, s))
            .map(|i| i.set)
            .collect();
        out.sort();
        out.dedup();
        out
    }
}
