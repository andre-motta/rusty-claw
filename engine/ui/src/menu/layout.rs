//! Where the entries of a submenu sit on the 640x480 menu canvas.
//!
//! Every submenu is first stacked and centred: for entries of sizes `(w_i,
//! h_i)`, `x_i = 320 - w_i / 2`, `y_0 = 240 - (sum of h_i) / 2 - 10 (n -
//! 1)` and `y_(i+1) = y_i + h_i + 10`. The main and in-game menus then
//! spread the entries over a vertical band `[top, bottom]` of their
//! submenu: the first entry at `top`, each next one at
//! `y_(i+1) = y_i + h_i + (bottom - top - sum of h_i) / (n - 1)` (a single
//! entry at `min(top, bottom)`). The level lists of the main menu are two
//! columns of seven, cell left edges at x 37 and 308, rows 28 pixels apart
//! from y 213; their back entry keeps its band position. Divisions round
//! toward zero.

use super::command::{MenuCommand, MenuKind, Page};
use super::tree::{Cell, ItemId, MenuTree};

/// Size of the menu canvas.
pub const CANVAS: (i32, i32) = (640, 480);
/// Gap between stacked entries.
pub const GAP: i32 = 10;
/// Centre row the in-game bands are measured from.
pub const IN_GAME_CENTER: i32 = CANVAS.1 / 2;
/// Left edges of the level list columns.
pub const GRID_COLUMNS: [i32; 2] = [37, 308];
/// Top of the level lists.
pub const GRID_TOP: i32 = 213;
/// Row pitch of the level lists.
pub const GRID_PITCH: i32 = 28;
/// Levels in a level list.
pub const GRID_LEVELS: i32 = 14;

/// Stacks entries of `sizes` centred on the canvas.
pub fn stacked(sizes: &[(i32, i32)]) -> Vec<Cell> {
    let n = sizes.len() as i32;
    let total: i32 = sizes.iter().map(|s| s.1).sum();
    let mut y = CANVAS.1 / 2 - total / 2 - GAP * (n - 1).max(0);
    sizes
        .iter()
        .map(|&(w, h)| {
            let c = Cell {
                x: CANVAS.0 / 2 - w / 2,
                y,
                w,
                h,
            };
            y += h + GAP;
            c
        })
        .collect()
}

/// Spreads `cells` over the band `[top, bottom]`, keeping their x.
pub fn spread(cells: &mut [Cell], top: i32, bottom: i32) {
    let n = cells.len() as i32;
    if n == 0 {
        return;
    }
    if n == 1 {
        cells[0].y = top.min(bottom);
        return;
    }
    let total: i32 = cells.iter().map(|c| c.h).sum();
    let gap = (bottom - top - total) / (n - 1);
    let mut y = top;
    for c in cells.iter_mut() {
        c.y = y;
        y += c.h + gap;
    }
}

/// The band of a main-menu submenu.
pub fn main_band(page: Page) -> Option<(i32, i32)> {
    Some(match page {
        Page::Root => (215, 474),
        Page::SinglePlayer | Page::NewGameLevels | Page::LoadGameLevels => (220, 442),
        Page::Multiplayer | Page::Options | Page::Audio | Page::Display => (220, 405),
        Page::Movies => (200, 474),
        Page::QuitConfirm => (294, 368),
        Page::LoadEntries { .. } => (220, 368),
        Page::SaveInfo => (319, 356),
        Page::Video => (190, 440),
        Page::Controls => (70, 440),
        _ => return None,
    })
}

/// The band of an in-game submenu.
pub fn in_game_band(page: Page) -> Option<(i32, i32)> {
    let c = IN_GAME_CENTER;
    Some(match page {
        Page::Root | Page::Audio => (c - 95, c + 95),
        Page::EndLifeConfirm | Page::EndGameConfirm => (c + 38 - 10, c + 95),
        Page::Options | Page::Display => (c - 76, c + 76),
        Page::Video => (c - 110, c + 120),
        Page::Controls => (70, 440),
        _ => return None,
    })
}

/// The level of a level-list entry, 1 to 14.
pub fn list_level(command: MenuCommand) -> Option<u32> {
    match command {
        MenuCommand::NewGame { level } | MenuCommand::Open(Page::LoadEntries { level }) => {
            Some(level)
        }
        _ => None,
    }
}

/// True for the two level lists.
pub fn is_level_list(page: Page) -> bool {
    matches!(page, Page::NewGameLevels | Page::LoadGameLevels)
}

/// Lays out the entries of `page_item`'s submenu in `tree`.
pub fn layout(tree: &mut MenuTree, page_item: ItemId) {
    let kids = tree.items[page_item].children.clone();
    let sizes: Vec<(i32, i32)> = kids.iter().map(|&k| tree.items[k].size).collect();
    let mut cells = stacked(&sizes);
    let page = tree.items[page_item].page();
    let band = match tree.kind {
        MenuKind::Main => main_band(page),
        MenuKind::InGame => in_game_band(page),
        MenuKind::GameOver => None,
    };
    if let Some((top, bottom)) = band {
        spread(&mut cells, top, bottom);
    }
    if tree.kind == MenuKind::Main && is_level_list(page) {
        let half = GRID_LEVELS / 2;
        for (cell, &k) in cells.iter_mut().zip(&kids) {
            let Some(level) = list_level(tree.items[k].command) else {
                continue;
            };
            let i = level as i32 - 1;
            if !(0..GRID_LEVELS).contains(&i) {
                continue;
            }
            cell.x = GRID_COLUMNS[usize::from(i >= half)];
            cell.y = (i % half) * GRID_PITCH + GRID_TOP;
        }
    }
    for (cell, k) in cells.into_iter().zip(kids) {
        tree.items[k].cell = cell;
    }
}
