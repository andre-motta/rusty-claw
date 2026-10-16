//! This game's own entries beside the art entries of the menus, on your own
//! install (skipped without `CLAW_INSTALL`): on the Options and Display
//! pages of the main and in-game menus, the added entry (Gameplay, Video)
//! has a row of its own with at least the art rows' gap to its neighbours,
//! letters at least half as tall as the art entries, the art entries before
//! it where they are without it, and the Down key reaches it. Its magnified
//! text and the markers draw whole at every presentation. With
//! `CLAW_MENU_PNG` set to a folder, the pages are also written there as PNG
//! files for a look.

use std::path::{Path, PathBuf};

use claw_assets::Install;
use claw_frame::{AspectPolicy, ViewConfig};
use claw_game::flow::{Flow, Start};
use claw_game::load::{load_graphics, load_level};
use claw_game::settings::{LevelChoice, Settings, UserDirs};
use claw_render::scene::DEFAULT_MAX_TARGET;
use claw_render::{
    FrameInput, LevelGraphics, Scene, SceneBuilder, ScreenBuilder, ScreenScene, cpu,
};
use claw_ui::hud::FONT;
use claw_ui::menu::command::{CommandCatalogue, MenuKind, Page};
use claw_ui::menu::draw::{self, Art, FontArt, MenuState};
use claw_ui::menu::layout::CANVAS;
use claw_ui::menu::tree::Cell;
use claw_ui::menu::{MenuInput, MenuTree, Navigator};
use claw_ui::screens::assets::{MENU_PICTURE, MENU_SCREENS, ScreenAssets, picture_frame};
use tempfile::TempDir;

fn install() -> Option<PathBuf> {
    let root = std::env::var_os("CLAW_INSTALL").map(PathBuf::from);
    if root.is_none() {
        eprintln!("skipped: CLAW_INSTALL is not set");
    }
    root
}

fn settings(install: &Path, tmp: &Path) -> Settings {
    let home = tmp.to_path_buf();
    let dirs = UserDirs::from_lookup(move |k| match k {
        "HOME" => Some(home.join("home").into()),
        "XDG_DATA_HOME" => Some(home.join("data").into()),
        "XDG_CONFIG_HOME" => Some(home.join("config").into()),
        _ => None,
    });
    let mut s = Settings::new(install, LevelChoice::Retail(1));
    s.config_path = dirs.config_file();
    s
}

/// The page's entries as drawn: (entry, box, shows text).
fn boxes(nav: &Navigator, art: &mut dyn Art) -> Vec<(usize, Cell, bool)> {
    draw::entry_boxes(nav, art, &MenuState::default())
        .into_iter()
        .map(|(k, b)| (k, b, nav.tree.items[k].shows_text))
        .collect()
}

/// Checks the added entry of `page` in `nav` against the same page of
/// `plain` (the menu without this game's entries); returns the entry.
fn check_page(
    nav: &mut Navigator,
    plain: &mut Navigator,
    art: &mut dyn Art,
    page: Page,
    what: &str,
) -> usize {
    let p = nav.tree.find_page(page).expect("page");
    nav.show(p, 0);
    let drawn = boxes(nav, art);
    assert_eq!(
        drawn.len(),
        nav.entries().len(),
        "{what}: every entry draws"
    );
    let i = drawn
        .iter()
        .position(|d| d.2)
        .unwrap_or_else(|| panic!("{what}: no added entry"));
    let (added, b, _) = drawn[i];
    assert!(
        i > 0 && i + 1 < drawn.len(),
        "{what}: added entry has neighbours"
    );
    for (_, c, _) in &drawn {
        assert!(
            c.x >= 0 && c.y >= 0 && c.x + c.w <= CANVAS.0 && c.y + c.h <= CANVAS.1,
            "{what}: {c:?} leaves the canvas"
        );
    }
    // The gap between two art entries.
    let art_gap = drawn
        .windows(2)
        .filter(|w| !w[0].2 && !w[1].2)
        .map(|w| w[1].1.y - (w[0].1.y + w[0].1.h))
        .min()
        .expect("two art entries");
    let art_h = drawn.iter().filter(|d| !d.2).map(|d| d.1.h).max().unwrap();
    let above = drawn[i - 1].1;
    let below = drawn[i + 1].1;
    let gap_above = b.y - (above.y + above.h);
    let gap_below = below.y - (b.y + b.h);
    eprintln!(
        "{what}: added entry {b:?} (zoom {}), gaps {gap_above} and {gap_below}, art gap \
         {art_gap}, art height {art_h}",
        nav.tree.items[added].zoom
    );
    assert!(gap_above >= 0 && gap_below >= 0, "{what}: entries overlap");
    assert!(
        gap_above >= art_gap && gap_below >= art_gap,
        "{what}: gaps {gap_above}, {gap_below} under the art gap {art_gap}"
    );
    assert!(
        2 * b.h >= art_h,
        "{what}: letters {} high beside art {art_h} high",
        b.h
    );
    assert!(nav.tree.items[added].zoom >= 2, "{what}: text is magnified");
    let centre = |c: Cell| c.x + c.w / 2;
    assert!(
        (centre(b) - CANVAS.0 / 2).abs() <= 1,
        "{what}: added entry is centred"
    );
    // The art entries before it stay where they are without it.
    let q = plain.tree.find_page(page).expect("plain page");
    plain.show(q, 0);
    let before = boxes(plain, art);
    for (n, d) in drawn[..i].iter().enumerate() {
        assert_eq!(d.1, before[n].1, "{what}: art entry {n} moved");
    }
    // Down reaches it.
    nav.show(p, 0);
    let mut presses = 0;
    while nav.selected() != Some(added) && presses < drawn.len() {
        nav.input(MenuInput::Down);
        presses += 1;
    }
    assert_eq!(nav.selected(), Some(added), "{what}: Down never reaches it");
    assert_eq!(presses, i, "{what}: Down presses");
    // Highlighted, it draws its magnified letters and the markers.
    let picture = draw::picture(nav, art, &MenuState::default(), 0.0);
    let letters = nav.tree.items[added]
        .text
        .as_deref()
        .unwrap()
        .chars()
        .filter(|c| !c.is_whitespace())
        .count();
    let zoom = nav.tree.items[added].zoom;
    assert_eq!(
        picture
            .iter()
            .filter(|m| m.sprite.key == FONT && m.zoom == zoom)
            .count(),
        letters,
        "{what}: magnified letters"
    );
    let set = draw::coin_set(nav.kind());
    let coins: Vec<_> = picture.iter().filter(|m| m.sprite.key == set).collect();
    assert_eq!(coins.len(), 2, "{what}: markers");
    for c in coins {
        assert!(
            c.sprite.y >= b.y && c.sprite.y <= b.y + b.h,
            "{what}: marker at {} beside {b:?}",
            c.sprite.y
        );
    }
    added
}

/// The number of sprites `picture` adds to `scene`.
fn added_sprites(scene: &mut Scene, g: &mut LevelGraphics, picture: &[draw::MenuSprite]) -> usize {
    let before = scene.sprites.len();
    draw::add_to_scene(scene, g, picture);
    scene.sprites.len() - before
}

fn write_png(name: &str, g: &LevelGraphics, scene: &Scene) {
    let Some(dir) = std::env::var_os("CLAW_MENU_PNG").map(PathBuf::from) else {
        return;
    };
    let c = cpu::compose(g, scene);
    let rgba = cpu::present(scene, &c);
    let (w, h) = scene.comp.output;
    claw_render::pack::write_png(&dir.join(format!("{name}.png")), w, h, &rgba).unwrap();
}

#[test]
fn added_entries_read_as_menu_entries_beside_the_art() {
    let Some(root) = install() else { return };
    let tmp = TempDir::new().unwrap();
    let store = Install::open(&root).unwrap().store().unwrap();
    let assets = ScreenAssets::load(store);
    let (menu_pic, pal) = assets.picture(MENU_SCREENS, MENU_PICTURE).unwrap();
    let mut g = LevelGraphics::for_screens(pal.clone(), Box::new(assets.images()));
    let bg = g.add_image(&picture_frame(&menu_pic));
    let font = FontArt::from_art(&mut g);
    let main = MenuTree::from_ifc(
        assets.menu(MenuKind::Main).unwrap(),
        &CommandCatalogue::new(MenuKind::Main),
    );
    let in_game = MenuTree::from_ifc(
        assets.menu(MenuKind::InGame).unwrap(),
        &CommandCatalogue::new(MenuKind::InGame),
    );
    let mut flow = Flow::new(
        settings(&root, tmp.path()),
        None,
        Some(main.clone()),
        font,
        Start::Title,
    );
    flow.measure(MenuKind::Main, &mut g);
    let bundle = load_level(&root, &LevelChoice::Retail(1)).expect("level 1");
    let mut lg = load_graphics(&bundle, None).expect("graphics");
    flow.set_in_game_menu(Some(in_game.clone()));
    flow.measure(MenuKind::InGame, &mut lg);

    // The main menu, on its own picture at two output sizes.
    let mut nav = flow.main_menu().unwrap().clone();
    let mut plain = Navigator::new(main);
    draw::measure(&mut plain, &mut g);
    for (page, name) in [(Page::Options, "options"), (Page::Display, "display")] {
        let what = format!("main menu {name}");
        check_page(&mut nav, &mut plain, &mut g, page, &what);
        let picture = draw::picture(&nav, &mut g, &MenuState::default(), 0.0);
        for res in [(640, 480), (1920, 1080)] {
            let view = ViewConfig::default().with_render_size(res.0, res.1);
            let b = ScreenBuilder::new(&view, DEFAULT_MAX_TARGET);
            let screen = ScreenScene {
                background: Some(bg),
                palette: pal.clone(),
                sprites: Vec::new(),
                fade: None,
            };
            let mut scene = b.build(&mut g, &screen, 0.0);
            assert_eq!(
                added_sprites(&mut scene, &mut g, &picture),
                picture.len(),
                "{what} at {res:?}: every sprite draws"
            );
            if res.0 == 640 {
                write_png(&format!("main_{name}"), &g, &scene);
            }
        }
    }

    // The in-game menu, centred over level 1 at several presentations.
    let mut nav = flow.in_game_menu().unwrap().clone();
    let mut plain = Navigator::new(in_game);
    draw::measure(&mut plain, &mut lg);
    let level = &bundle.level;
    for (page, name) in [(Page::Options, "options"), (Page::Display, "display")] {
        let what = format!("in-game menu {name}");
        let added = check_page(&mut nav, &mut plain, &mut lg, page, &what);
        let p = nav.tree.find_page(page).unwrap();
        let index = nav.entries().iter().position(|&k| k == added).unwrap();
        nav.show(p, index);
        let picture =
            draw::centred_picture(draw::picture(&nav, &mut lg, &MenuState::default(), 0.0));
        for (policy, res) in [
            (AspectPolicy::default(), (640, 480)),
            (AspectPolicy::default(), (1920, 1080)),
            (AspectPolicy::FrozenEdgeClamped, (1708 * 2, 960)),
        ] {
            let view = ViewConfig {
                aspect_policy: policy,
                ..ViewConfig::default()
            }
            .with_render_size(res.0, res.1);
            let mut b = SceneBuilder::new(&lg, view, DEFAULT_MAX_TARGET);
            let input = FrameInput {
                center: (f64::from(level.start_x), f64::from(level.start_y)),
                sprites: Vec::new(),
                time_ms: 0.0,
                fade: None,
                screen: Vec::new(),
                transition: None,
            };
            let mut scene = b.build(&mut lg, &input);
            assert_eq!(
                added_sprites(&mut scene, &mut lg, &picture),
                picture.len(),
                "{what} at {res:?}: every sprite draws"
            );
            if res == (640, 480) {
                write_png(&format!("in_game_{name}"), &lg, &scene);
            }
        }
    }

    // This game's own text pages fit the canvas, magnified except the
    // controls list.
    for (page, name, zoom) in [
        (Page::Gameplay, "gameplay", 2),
        (Page::Video, "video", 2),
        (Page::Controls, "controls", 1),
    ] {
        let p = nav.tree.find_page(page).unwrap();
        nav.show(p, 0);
        let drawn = boxes(&nav, &mut lg);
        assert_eq!(
            drawn.len(),
            nav.entries().len(),
            "{name}: every entry draws"
        );
        for w in drawn.windows(2) {
            assert!(w[0].1.y + w[0].1.h <= w[1].1.y, "{name}: rows overlap");
        }
        for (k, c, _) in &drawn {
            assert_eq!(nav.tree.items[*k].zoom, zoom, "{name}: zoom");
            assert!(
                c.x >= 0 && c.y >= 0 && c.x + c.w <= CANVAS.0 && c.y + c.h <= CANVAS.1,
                "{name}: {c:?} leaves the canvas"
            );
        }
        let picture =
            draw::centred_picture(draw::picture(&nav, &mut lg, &MenuState::default(), 0.0));
        let view = ViewConfig::default().with_render_size(640, 480);
        let mut b = SceneBuilder::new(&lg, view, DEFAULT_MAX_TARGET);
        let input = FrameInput {
            center: (f64::from(level.start_x), f64::from(level.start_y)),
            sprites: Vec::new(),
            time_ms: 0.0,
            fade: None,
            screen: Vec::new(),
            transition: None,
        };
        let mut scene = b.build(&mut lg, &input);
        assert_eq!(
            added_sprites(&mut scene, &mut lg, &picture),
            picture.len(),
            "{name}: every sprite draws"
        );
        write_png(&format!("in_game_{name}"), &lg, &scene);
    }
}
