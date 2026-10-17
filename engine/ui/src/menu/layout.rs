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
//!
//! This game's own text entries are drawn with the menu font magnified by a
//! whole factor (their zoom), so they stay sharp:
//!
//! - on a page of art entries (Options and Display) the zoom is the largest
//!   that keeps the letters within two thirds of the art rows' height (the
//!   height of the art's capitals), and each text entry gets a row as tall
//!   as the art rows with its text centred in it. The art entries keep the
//!   places they have without the text entries; each text row is inserted
//!   with the same gap as between the art rows, so the entries after it
//!   move down (the band grows by those rows, never past the canvas);
//! - on a page of text entries only, the zoom is 2 ([`TEXT_PAGE_ZOOM`]),
//!   or 1 on the controls list, whose rows sit on its panel;
//! - either zoom drops while the widest text would leave less than
//!   [`MARKER_ROOM`] at each side of the canvas for the markers, or while
//!   the text rows of a text page would not fit its band.

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
/// Zoom of the text on this game's own text pages.
pub const TEXT_PAGE_ZOOM: u32 = 2;
/// Room kept at each side of a text entry for the markers beside it.
pub const MARKER_ROOM: i32 = 48;
/// Share of an art row's height the magnified letters may take.
pub const ART_LETTER_SHARE: (i32, i32) = (2, 3);

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
        Page::Video | Page::Gameplay => (190, 440),
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
        Page::Video | Page::Gameplay => (c - 110, c + 120),
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

/// The zoom of text entries `text` wide and high (unmagnified, the widest
/// and the tallest) on `page`, beside art rows `art_h` high (0 for a page
/// of text entries only, `rows` of them in `band`).
pub fn text_zoom(
    page: Page,
    art_h: i32,
    text: (i32, i32),
    rows: i32,
    band: Option<(i32, i32)>,
) -> u32 {
    let (w, h) = text;
    if h <= 0 {
        return 1;
    }
    let mut zoom = if art_h > 0 {
        (art_h * ART_LETTER_SHARE.0 / ART_LETTER_SHARE.1 / h).max(1) as u32
    } else if page == Page::Controls {
        1
    } else {
        TEXT_PAGE_ZOOM
    };
    let fits = |z: i32| {
        let wide = w * z <= CANVAS.0 - 2 * MARKER_ROOM;
        let tall = art_h > 0 || band.is_none_or(|(top, bottom)| rows * h * z <= bottom - top);
        wide && tall
    };
    while zoom > 1 && !fits(zoom as i32) {
        zoom -= 1;
    }
    zoom
}

/// Lays out the entries of `page_item`'s submenu in `tree`.
pub fn layout(tree: &mut MenuTree, page_item: ItemId) {
    let kids = tree.items[page_item].children.clone();
    let page = tree.items[page_item].page();
    let band = match tree.kind {
        MenuKind::Main => main_band(page),
        MenuKind::InGame => in_game_band(page),
        MenuKind::GameOver => None,
    };
    let items = &tree.items;
    let art: Vec<(i32, i32)> = kids
        .iter()
        .filter(|&&k| !items[k].shows_text)
        .map(|&k| items[k].size)
        .collect();
    let texts: Vec<(i32, i32)> = kids
        .iter()
        .filter(|&&k| items[k].shows_text)
        .map(|&k| items[k].size)
        .collect();
    let art_h = art.iter().map(|s| s.1).max().unwrap_or(0);
    let widest = (
        texts.iter().map(|s| s.0).max().unwrap_or(0),
        texts.iter().map(|s| s.1).max().unwrap_or(0),
    );
    let zoom = text_zoom(page, art_h, widest, texts.len() as i32, band);
    let mut sizes = Vec::with_capacity(kids.len());
    for &k in &kids {
        let item = &mut tree.items[k];
        if item.shows_text {
            item.zoom = zoom;
            let z = zoom as i32;
            sizes.push((item.size.0 * z, (item.size.1 * z).max(art_h)));
        } else {
            item.zoom = 1;
            sizes.push(item.size);
        }
    }
    let mut cells = stacked(&sizes);
    if let Some((top, mut bottom)) = band {
        if !art.is_empty() && !texts.is_empty() {
            // Room for the text rows at the art rows' own gap.
            let n = art.len() as i32;
            let sum: i32 = art.iter().map(|s| s.1).sum();
            let gap = if n > 1 {
                ((bottom - top - sum) / (n - 1)).max(0)
            } else {
                GAP
            };
            let rows: i32 = kids
                .iter()
                .zip(&sizes)
                .filter(|&(&k, _)| tree.items[k].shows_text)
                .map(|(_, s)| s.1 + gap)
                .sum();
            bottom = (bottom + rows).min(CANVAS.1);
        }
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

#[cfg(test)]
mod tests {
    use claw_assets::ifc::IfcNode;

    use super::*;
    use crate::menu::command::CommandCatalogue;

    /// A menu whose `page` holds entries of `art` sizes (images) and, when
    /// given, a text entry of `text` size before the last one.
    fn page_of(
        kind: MenuKind,
        page: Page,
        art: &[(i32, i32)],
        text: Option<(i32, i32)>,
    ) -> (MenuTree, ItemId) {
        let root = IfcNode {
            command: 10000,
            ..IfcNode::default()
        };
        let mut tree = MenuTree::from_ifc(root, &CommandCatalogue::new(kind));
        let p = tree.add_text(0, "PAGE", MenuCommand::Open(page));
        for &size in art {
            let k = tree.add_text(p, "", MenuCommand::Back);
            tree.items[k].text = None;
            tree.items[k].shows_text = false;
            tree.items[k].size = size;
        }
        if let Some(size) = text {
            let k = tree.add_text(p, "ADDED", MenuCommand::Back);
            tree.items[k].size = size;
            let n = tree.items[p].children.len();
            tree.items[p].children.swap(n - 1, n - 2);
        }
        layout(&mut tree, p);
        (tree, p)
    }

    fn cells(tree: &MenuTree, page: ItemId) -> Vec<Cell> {
        tree.items[page]
            .children
            .iter()
            .map(|&k| tree.items[k].cell)
            .collect()
    }

    #[test]
    fn an_added_row_keeps_the_art_rows_and_their_gap() {
        let art = [(200, 35), (160, 35), (120, 35), (108, 35)];
        let (plain, p) = page_of(MenuKind::InGame, Page::Options, &art, None);
        let (tree, q) = page_of(MenuKind::InGame, Page::Options, &art, Some((64, 7)));
        let before = cells(&plain, p);
        let after = cells(&tree, q);
        assert_eq!(&after[..3], &before[..3], "the art rows stay");
        let gap = before[1].y - (before[0].y + before[0].h);
        assert_eq!(gap, 4);
        let added = tree.items[tree.items[q].children[3]].clone();
        assert_eq!(added.zoom, 3, "letters within two thirds of the rows");
        assert_eq!(
            added.cell,
            Cell {
                x: 320 - 96,
                y: before[2].y + 35 + gap,
                w: 192,
                h: 35
            }
        );
        assert_eq!(after[4].y, added.cell.y + 35 + gap, "back moves down");
    }

    #[test]
    fn an_added_row_never_pushes_entries_off_the_canvas() {
        let art = [(100, 35); 8];
        let (tree, q) = page_of(MenuKind::Main, Page::Options, &art, Some((64, 7)));
        let last = *cells(&tree, q).last().unwrap();
        assert!(last.y + last.h <= CANVAS.1);
    }

    #[test]
    fn text_pages_magnify_twice_unless_they_cannot_fit() {
        let band = Some((100, 300));
        assert_eq!(text_zoom(Page::Video, 0, (144, 7), 9, band), 2);
        assert_eq!(text_zoom(Page::Controls, 0, (144, 7), 9, band), 1);
        // Too wide for the markers at twice the size.
        assert_eq!(text_zoom(Page::Video, 0, (300, 7), 9, band), 1);
        // Too many rows for the band at twice the size.
        assert_eq!(text_zoom(Page::Video, 0, (100, 7), 15, band), 1);
        // Beside art rows: as large as fits two thirds of a row.
        assert_eq!(text_zoom(Page::Options, 35, (64, 7), 1, band), 3);
        assert_eq!(text_zoom(Page::Options, 35, (200, 7), 1, band), 2);
        assert_eq!(text_zoom(Page::Options, 35, (64, 0), 1, band), 1);
    }
}
