//! Moving through a menu: the highlight, choosing, submenus, sliders and
//! held keys.
//!
//! - Up and Down move the highlight and wrap around; greyed-out entries
//!   are not skipped.
//! - Choosing (Enter, Space, a pad button) presses the highlighted entry
//!   for [`PRESS_MS`]; nothing else moves meanwhile. When the press ends an
//!   entry with a submenu opens it (its first entry highlighted), a back
//!   entry shows the submenu above, and the entry's command is reported:
//!   [`MenuAction::Command`] for an entry that is enabled,
//!   [`MenuAction::Disabled`] for one that is greyed out.
//! - Left and Right change the highlighted entry's slider by one step,
//!   and on the main menu's level lists jump to the other column (seven
//!   entries along) when that entry exists and is enabled.
//! - Escape: in the main menu, the submenu above (at the top, the quit
//!   entry is highlighted and chosen, which asks whether to quit); in the
//!   in-game menu, back to the game from any submenu.
//! - A letter chooses the entry it is the hotkey of, unless it is greyed
//!   out.
//! - A direction held after being pressed repeats [`REPEAT_FIRST_MS`]
//!   after the press, then every [`REPEAT_NEXT_MS`].
//! - The highlighted entry blinks every [`BLINK_MS`] (visible only when a
//!   menu gives the blink its own images).

use super::command::{MenuCommand, MenuKind, Page, Slider};
use super::layout::{self, is_level_list};
use super::tree::{ItemId, MenuTree};

/// How long an entry stays pressed before its command runs, ms.
pub const PRESS_MS: f64 = 300.0;
/// Blink period of the highlighted entry, ms.
pub const BLINK_MS: f64 = 400.0;
/// Delay before a held direction repeats, ms.
pub const REPEAT_FIRST_MS: f64 = 350.0;
/// Interval of the repeats that follow, ms.
pub const REPEAT_NEXT_MS: f64 = 30.0;
/// Most repeats one update makes, however long it was.
pub const MAX_REPEATS: u32 = 8;
/// How far Left and Right jump in a level list.
pub const COLUMN_JUMP: usize = 7;

/// A menu input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuInput {
    /// Up.
    Up,
    /// Down.
    Down,
    /// Left, or decrease.
    Left,
    /// Right, or increase.
    Right,
    /// Choose.
    Confirm,
    /// Escape.
    Back,
}

impl MenuInput {
    /// True for the four directions.
    pub fn is_direction(self) -> bool {
        matches!(
            self,
            MenuInput::Up | MenuInput::Down | MenuInput::Left | MenuInput::Right
        )
    }
}

/// What an input did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// An enabled entry was chosen. A submenu it opens, or the submenu a
    /// back entry returns to, is already showing.
    Command(MenuCommand),
    /// A greyed-out entry was chosen.
    Disabled(MenuCommand),
    /// Left or Right on a slider: change it by `delta`.
    Slider {
        /// Which slider.
        slider: Slider,
        /// The change, one step up or down.
        delta: i32,
    },
    /// Escape showed the submenu above.
    Back,
    /// The highlight moved.
    Moved,
}

/// How an entry looks now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    /// Highlighted.
    pub highlighted: bool,
    /// Pressed.
    pub pressed: bool,
    /// Greyed out.
    pub disabled: bool,
    /// The blink phase.
    pub blink: bool,
}

impl Look {
    /// The image slot shown for this look.
    pub fn slot(self) -> usize {
        match (self.disabled, self.highlighted, self.pressed, self.blink) {
            (true, true, _, false) => 6,
            (true, true, _, true) => 7,
            (true, false, _, _) => 5,
            (false, true, true, false) => 1,
            (false, true, true, true) => 2,
            (false, true, false, false) => 3,
            (false, true, false, true) => 4,
            (false, false, _, _) => 0,
        }
    }
}

/// A menu and where the player is in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Navigator {
    /// The menu.
    pub tree: MenuTree,
    page: ItemId,
    sel: usize,
    press: Option<f64>,
    blink_ms: f64,
    blink: bool,
    held: Option<MenuInput>,
    delay: f64,
}

impl Navigator {
    /// A navigator at the top of `tree`, its first entry highlighted.
    pub fn new(tree: MenuTree) -> Navigator {
        let mut n = Navigator {
            tree,
            page: 0,
            sel: 0,
            press: None,
            blink_ms: 0.0,
            blink: true,
            held: None,
            delay: 0.0,
        };
        n.relayout();
        n
    }

    /// The menu's kind.
    pub fn kind(&self) -> MenuKind {
        self.tree.kind
    }

    /// The entry whose submenu shows.
    pub fn page_item(&self) -> ItemId {
        self.page
    }

    /// The submenu that shows.
    pub fn page(&self) -> Page {
        self.tree.items[self.page].page()
    }

    /// The entries of the submenu that shows.
    pub fn entries(&self) -> &[ItemId] {
        &self.tree.items[self.page].children
    }

    /// The highlighted entry.
    pub fn selected(&self) -> Option<ItemId> {
        self.entries().get(self.sel).copied()
    }

    /// True while an entry is pressed.
    pub fn pressing(&self) -> bool {
        self.press.is_some()
    }

    /// How `item` looks now.
    pub fn look(&self, item: ItemId) -> Look {
        let highlighted = self.selected() == Some(item);
        Look {
            highlighted,
            pressed: highlighted && self.press.is_some(),
            disabled: !self.tree.items[item].enabled,
            blink: highlighted && self.blink,
        }
    }

    /// Lays out the submenu that shows again (after sizes or entries
    /// changed).
    pub fn relayout(&mut self) {
        layout::layout(&mut self.tree, self.page);
        let n = self.entries().len();
        if n == 0 {
            self.sel = 0;
        } else if self.sel >= n {
            self.sel = n - 1;
        }
    }

    /// Shows `page_item`'s submenu with entry `index` highlighted.
    pub fn show(&mut self, page_item: ItemId, index: usize) {
        if page_item >= self.tree.items.len() {
            return;
        }
        self.page = page_item;
        self.sel = index;
        self.press = None;
        self.relayout();
        self.restart_blink();
    }

    /// Back to the top, its first entry highlighted.
    pub fn reset(&mut self) {
        self.held = None;
        self.show(0, 0);
    }

    /// Highlights `item` if it is in the submenu that shows.
    pub fn select(&mut self, item: ItemId) -> bool {
        match self.entries().iter().position(|&k| k == item) {
            Some(i) => {
                let moved = i != self.sel;
                self.sel = i;
                if moved {
                    self.restart_blink();
                }
                moved
            }
            None => false,
        }
    }

    fn restart_blink(&mut self) {
        self.blink = true;
        self.blink_ms = 0.0;
    }

    fn step(&mut self, forward: bool) -> Vec<MenuAction> {
        let n = self.entries().len();
        if n == 0 {
            return Vec::new();
        }
        self.sel = if forward {
            (self.sel + 1) % n
        } else {
            (self.sel + n - 1) % n
        };
        self.restart_blink();
        vec![MenuAction::Moved]
    }

    fn sideways(&mut self, right: bool) -> Vec<MenuAction> {
        let Some(item) = self.selected() else {
            return Vec::new();
        };
        if let Some(slider) = self.tree.items[item].slider {
            let s = slider.step();
            return vec![MenuAction::Slider {
                slider,
                delta: if right { s } else { -s },
            }];
        }
        if self.tree.kind == MenuKind::Main && is_level_list(self.page()) {
            let target = if right {
                self.sel.checked_add(COLUMN_JUMP)
            } else {
                self.sel.checked_sub(COLUMN_JUMP)
            };
            if let Some(t) = target
                && let Some(&k) = self.entries().get(t)
                && self.tree.items[k].enabled
                && layout::list_level(self.tree.items[k].command).is_some()
            {
                self.sel = t;
                self.restart_blink();
                return vec![MenuAction::Moved];
            }
        }
        Vec::new()
    }

    fn activate(&mut self) {
        if self.selected().is_some() {
            self.press = Some(PRESS_MS);
        }
    }

    /// Shows the submenu above; false at the top.
    pub fn go_up(&mut self) -> bool {
        let Some(parent) = self.tree.items[self.page].parent else {
            return false;
        };
        let from = self.page;
        let index = self.tree.items[parent]
            .children
            .iter()
            .position(|&k| k == from)
            .unwrap_or(0);
        self.show(parent, index);
        true
    }

    fn act(&mut self, input: MenuInput) -> Vec<MenuAction> {
        match input {
            MenuInput::Up => self.step(false),
            MenuInput::Down => self.step(true),
            MenuInput::Left => self.sideways(false),
            MenuInput::Right => self.sideways(true),
            MenuInput::Confirm => {
                self.activate();
                Vec::new()
            }
            MenuInput::Back => match self.tree.kind {
                MenuKind::InGame => vec![MenuAction::Command(MenuCommand::Resume)],
                MenuKind::GameOver => Vec::new(),
                MenuKind::Main => {
                    if self.go_up() {
                        return vec![MenuAction::Back];
                    }
                    let quit = self.tree.find_page(Page::QuitConfirm);
                    if let Some(q) = quit {
                        self.select(q);
                        self.activate();
                    }
                    Vec::new()
                }
            },
        }
    }

    /// A new press of `input`.
    pub fn input(&mut self, input: MenuInput) -> Vec<MenuAction> {
        if self.press.is_some() {
            return Vec::new();
        }
        if input.is_direction() {
            self.held = Some(input);
            self.delay = REPEAT_FIRST_MS;
        }
        self.act(input)
    }

    /// The letter key `letter` went down.
    pub fn hotkey(&mut self, letter: char) -> Vec<MenuAction> {
        if self.press.is_some() {
            return Vec::new();
        }
        let up = letter.to_ascii_uppercase();
        let found = self
            .entries()
            .iter()
            .copied()
            .find(|&k| self.tree.items[k].hotkey == Some(up));
        let Some(item) = found else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if self.select(item) {
            out.push(MenuAction::Moved);
        }
        if self.tree.items[item].enabled {
            self.activate();
        }
        out
    }

    /// Lets `dt_ms` pass with the direction `held` held down (or none):
    /// runs a finished press, blinks and repeats a held direction.
    pub fn update(&mut self, dt_ms: f64, held: Option<MenuInput>) -> Vec<MenuAction> {
        let dt = if dt_ms.is_finite() {
            dt_ms.max(0.0)
        } else {
            0.0
        };
        let mut out = Vec::new();
        self.blink_ms += dt;
        while self.blink_ms >= BLINK_MS {
            self.blink_ms -= BLINK_MS;
            self.blink = !self.blink;
        }
        if let Some(left) = self.press {
            let left = left - dt;
            if left > 0.0 {
                self.press = Some(left);
                return out;
            }
            self.press = None;
            out.extend(self.finish_press());
            self.held = None;
            return out;
        }
        if held.is_none() || held != self.held {
            self.held = None;
            return out;
        }
        self.delay -= dt;
        let mut n = 0;
        while self.delay <= 0.0 && n < MAX_REPEATS {
            if let Some(d) = self.held {
                out.extend(self.act(d));
            }
            self.delay += REPEAT_NEXT_MS;
            n += 1;
        }
        if self.delay <= 0.0 {
            self.delay = REPEAT_NEXT_MS;
        }
        out
    }

    fn finish_press(&mut self) -> Vec<MenuAction> {
        let Some(item) = self.selected() else {
            return Vec::new();
        };
        let it = &self.tree.items[item];
        let (enabled, command, has_children) = (it.enabled, it.command, !it.children.is_empty());
        if !enabled {
            return vec![MenuAction::Disabled(command)];
        }
        if has_children {
            self.show(item, 0);
        } else if command == MenuCommand::Back {
            self.go_up();
        }
        vec![MenuAction::Command(command)]
    }
}
