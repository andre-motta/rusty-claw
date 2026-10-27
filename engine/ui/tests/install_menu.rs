//! The menus and screens on your own install: the three menu files parse,
//! every image they name exists, every entry lies inside the 640x480
//! canvas on every submenu, every command this game acts on is reachable
//! with the keys, and the title, main menu and end-of-level screens draw.
//! Run with `make test-install MODULE=ui` (needs `CLAW_INSTALL`); skipped
//! otherwise. With `CLAW_MENU_PNG` set to a folder, the pictures are also
//! written there as PNG files for a look.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::ViewConfig;
use claw_render::scene::DEFAULT_MAX_TARGET;
use claw_render::{LevelGraphics, ScreenBuilder, ScreenScene, cpu};
use claw_ui::menu::command::{CommandCatalogue, MenuCommand, MenuKind, Page};
use claw_ui::menu::draw::{self, MenuState};
use claw_ui::menu::layout::CANVAS;
use claw_ui::menu::tree::{MenuTree, image_set_of};
use claw_ui::menu::{MenuAction, MenuInput, Navigator};
use claw_ui::screens::assets::{
    self, BOOTY_SCREENS, MENU_PICTURE, MENU_SCREENS, ScreenAssets, TITLE_PICTURE, TITLE_SCREENS,
};
use claw_ui::screens::booty::{self, BootyArt, BootyScreen, BootyStats};
use claw_ui::screens::title::TitleScreen;

fn install() -> Option<PathBuf> {
    let root = std::env::var_os("CLAW_INSTALL").map(PathBuf::from);
    if root.is_none() {
        eprintln!("skipped: CLAW_INSTALL is not set");
    }
    root
}

fn screen_assets(root: &PathBuf) -> ScreenAssets {
    ScreenAssets::load(Install::open(root).unwrap().store().unwrap())
}

fn screen_graphics(a: &ScreenAssets) -> (LevelGraphics, claw_assets::Palette) {
    let (_, pal) = a.picture(MENU_SCREENS, MENU_PICTURE).expect("menu picture");
    (
        LevelGraphics::for_screens(pal.clone(), Box::new(a.images())),
        pal,
    )
}

/// Every submenu entry of `nav` (by entering each submenu in turn).
fn pages(nav: &Navigator) -> Vec<usize> {
    (0..nav.tree.items.len())
        .filter(|&i| !nav.tree.items[i].children.is_empty())
        .collect()
}

fn check_tree(nav: &mut Navigator, art: &mut dyn claw_ui::menu::Art, what: &str) -> usize {
    draw::measure(nav, art);
    let mut images = 0;
    let kind = nav.kind();
    for item in &nav.tree.items {
        for slot in &item.slots {
            let img =
                image_set_of(kind, slot).unwrap_or_else(|| panic!("{what}: {slot:?} has no set"));
            assert!(
                art.info(&img.set, img.frame).is_some(),
                "{what}: {} frame {} ({slot}) is missing",
                img.set,
                img.frame
            );
            images += 1;
        }
    }
    for p in pages(nav) {
        nav.show(p, 0);
        for &k in nav.entries() {
            let c = nav.tree.items[k].cell;
            assert!(
                c.x >= 0 && c.y >= 0 && c.x + c.w <= CANVAS.0 && c.y + c.h <= CANVAS.1,
                "{what}: entry {} at {c:?} leaves the canvas on {:?}",
                nav.tree.items[k].id,
                nav.page()
            );
        }
    }
    nav.reset();
    images
}

/// Every command reached by moving down through each submenu and choosing
/// every entry, entering submenus as they open.
fn reachable(nav: &mut Navigator) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut todo = vec![Vec::<usize>::new()];
    let mut visited = BTreeSet::new();
    while let Some(path) = todo.pop() {
        // Replay the path: each step is "move down n times, then choose".
        nav.reset();
        let mut ok = true;
        for &n in &path {
            for _ in 0..n {
                nav.input(MenuInput::Down);
            }
            nav.input(MenuInput::Confirm);
            let out = nav.update(400.0, None);
            if !out.iter().any(|a| matches!(a, MenuAction::Command(_))) {
                ok = false;
            }
        }
        if !ok || !visited.insert(nav.page_item()) {
            continue;
        }
        let count = nav.entries().len();
        let page_item = nav.page_item();
        for i in 0..count {
            nav.show(page_item, i);
            nav.input(MenuInput::Confirm);
            for a in nav.update(400.0, None) {
                if let MenuAction::Command(c) = a {
                    seen.insert(format!("{c:?}"));
                }
            }
            if nav.page_item() != page_item
                && nav.tree.items[nav.page_item()].parent == Some(page_item)
            {
                let mut p = path.clone();
                p.push(i);
                todo.push(p);
            }
        }
    }
    seen
}

#[test]
fn the_main_menu_parses_resolves_fits_and_reaches_every_command() {
    let Some(root) = install() else { return };
    let a = screen_assets(&root);
    assert!(a.missing.is_empty(), "missing: {:?}", a.missing);
    let (mut g, _) = screen_graphics(&a);
    let tree = MenuTree::from_ifc(
        a.menu(MenuKind::Main).unwrap(),
        &CommandCatalogue::new(MenuKind::Main),
    );
    assert!(tree.items.len() > 100);
    let mut nav = Navigator::new(tree);
    let images = check_tree(&mut nav, &mut g, "main menu");
    for set in draw::extra_sets(MenuKind::Main) {
        assert!(!g.frame_numbers(set).is_empty(), "{set} is missing");
    }
    eprintln!(
        "main menu: {} entries, {images} images",
        nav.tree.items.len()
    );
    let seen = reachable(&mut nav);
    let mut want: Vec<MenuCommand> = vec![
        MenuCommand::Open(Page::SinglePlayer),
        MenuCommand::Open(Page::Options),
        MenuCommand::Open(Page::QuitConfirm),
        MenuCommand::Open(Page::NewGameLevels),
        MenuCommand::Open(Page::LoadGameLevels),
        MenuCommand::Open(Page::SaveInfo),
        MenuCommand::Open(Page::Audio),
        MenuCommand::Open(Page::Display),
        MenuCommand::Back,
        MenuCommand::Quit,
        MenuCommand::Controls,
    ];
    for t in [
        claw_ui::menu::Toggle::Sound,
        claw_ui::menu::Toggle::Voice,
        claw_ui::menu::Toggle::Ambient,
        claw_ui::menu::Toggle::Music,
        claw_ui::menu::Toggle::HighDetail,
        claw_ui::menu::Toggle::FrontLayer,
        claw_ui::menu::Toggle::MovieInterlace,
    ] {
        want.push(MenuCommand::Toggle(t));
    }
    want.push(MenuCommand::Adjust(claw_ui::menu::Slider::PlayArea));
    // The Replay Movies page and each of its movies.
    want.push(MenuCommand::Open(Page::Movies));
    for entry in 1..=claw_ui::menu::command::MOVIE_ENTRIES {
        want.push(MenuCommand::ReplayMovie(entry));
    }
    for n in 1..=14 {
        want.push(MenuCommand::NewGame { level: n });
        want.push(MenuCommand::Open(Page::LoadEntries { level: n }));
        for entry in 0..3 {
            want.push(MenuCommand::Load { level: n, entry });
        }
    }
    for w in want {
        assert!(seen.contains(&format!("{w:?}")), "{w:?} cannot be reached");
    }
    // Sliders are reachable: the sound slider moves with the arrows.
    let audio = nav.tree.find_page(Page::Audio).unwrap();
    nav.show(audio, 0);
    assert!(matches!(
        nav.input(MenuInput::Right)[..],
        [MenuAction::Slider { .. }]
    ));
}

fn level1(root: &PathBuf) -> (LevelData, LevelGraphics) {
    let store = Install::open(root).unwrap().store().unwrap();
    let level = LevelData::build(&store, LevelSource::Retail(1)).unwrap();
    let g = LevelGraphics::from_level(&level, &store, None);
    (level, g)
}

#[test]
fn the_in_game_and_game_over_menus_parse_resolve_and_fit() {
    let Some(root) = install() else { return };
    let a = screen_assets(&root);
    let (_, mut g) = level1(&root);
    let tree = MenuTree::from_ifc(
        a.menu(MenuKind::InGame).unwrap(),
        &CommandCatalogue::new(MenuKind::InGame),
    );
    let mut nav = Navigator::new(tree);
    check_tree(&mut nav, &mut g, "in-game menu");
    for set in draw::extra_sets(MenuKind::InGame) {
        assert!(!g.frame_numbers(set).is_empty(), "{set} is missing");
    }
    let seen = reachable(&mut nav);
    for w in [
        MenuCommand::Resume,
        MenuCommand::Open(Page::EndLifeConfirm),
        MenuCommand::Open(Page::Options),
        MenuCommand::Open(Page::EndGameConfirm),
        MenuCommand::EndLife,
        MenuCommand::EndGame,
        MenuCommand::Back,
        MenuCommand::Controls,
        MenuCommand::Open(Page::Display),
        MenuCommand::Open(Page::Audio),
        MenuCommand::Adjust(claw_ui::menu::Slider::PlayArea),
    ] {
        assert!(seen.contains(&format!("{w:?}")), "{w:?} cannot be reached");
    }
    // The in-game menu over level 1, centred on a wide picture.
    let mut nav = Navigator::new(MenuTree::from_ifc(
        a.menu(MenuKind::InGame).unwrap(),
        &CommandCatalogue::new(MenuKind::InGame),
    ));
    draw::measure(&mut nav, &mut g);
    let (level, _) = level1(&root);
    let sprites = draw::centred(draw::sprites(&nav, &mut g, &MenuState::default(), 0.0));
    let input = claw_render::FrameInput {
        center: (f64::from(level.start_x), f64::from(level.start_y)),
        sprites: Vec::new(),
        time_ms: 0.0,
        fade: None,
        screen: sprites.clone(),
        transition: None,
    };
    let view = ViewConfig {
        aspect_policy: claw_frame::AspectPolicy::FrozenEdgeClamped,
        ..ViewConfig::default()
    }
    .with_render_size(1708 * 2, 960);
    let mut b = claw_render::SceneBuilder::new(&g, view, DEFAULT_MAX_TARGET);
    let pw = b.composition().presentation.0 as i32;
    let scene = b.build(&mut g, &input);
    assert!(
        scene.sprites.len() >= sprites.len(),
        "every menu sprite draws"
    );
    // The menu sits in the middle of the wide picture.
    let first = scene.sprites[scene.sprites.len() - sprites.len()];
    let k = b.composition().scale as i32;
    assert!(first.pos[0] / k > (pw - 640) / 2 && first.pos[0] / k < (pw + 640) / 2);
    if let Some(dir) = std::env::var_os("CLAW_MENU_PNG").map(PathBuf::from) {
        let c = cpu::compose(&g, &scene);
        let rgba = cpu::present(&scene, &c);
        let (w, h) = scene.comp.output;
        claw_render::pack::write_png(&dir.join("in_game.png"), w, h, &rgba).unwrap();
    }

    let over = MenuTree::from_ifc(
        a.menu(MenuKind::GameOver).unwrap(),
        &CommandCatalogue::new(MenuKind::GameOver),
    );
    let mut nav = Navigator::new(over);
    check_tree(&mut nav, &mut g, "game-over menu");
}

/// Writes `scene` as a PNG into `CLAW_MENU_PNG`, when set.
fn png(name: &str, g: &mut LevelGraphics, scene: &ScreenScene) {
    let Some(dir) = std::env::var_os("CLAW_MENU_PNG").map(PathBuf::from) else {
        return;
    };
    let view = ViewConfig::default().with_render_size(1280, 960);
    let b = ScreenBuilder::new(&view, DEFAULT_MAX_TARGET);
    let s = b.build(g, scene, 0.0);
    let c = cpu::compose(g, &s);
    let rgba = cpu::present(&s, &c);
    let (w, h) = s.comp.output;
    claw_render::pack::write_png(&dir.join(format!("{name}.png")), w, h, &rgba).unwrap();
}

#[test]
fn the_screens_draw_from_the_install() {
    let Some(root) = install() else { return };
    let a = screen_assets(&root);
    let (mut g, pal) = screen_graphics(&a);
    let (menu_pic, _) = a.picture(MENU_SCREENS, MENU_PICTURE).unwrap();
    let bg = g.add_image(&assets::picture_frame(&menu_pic));
    let tree = MenuTree::from_ifc(
        a.menu(MenuKind::Main).unwrap(),
        &CommandCatalogue::new(MenuKind::Main),
    );
    let mut nav = Navigator::new(tree);
    draw::measure(&mut nav, &mut g);
    let sprites = draw::sprites(&nav, &mut g, &MenuState::default(), 0.0);
    assert!(sprites.len() >= 9);
    let scene = ScreenScene {
        background: Some(bg),
        palette: pal,
        sprites,
        fade: None,
    };
    let view = ViewConfig::default().with_render_size(1280, 960);
    let b = ScreenBuilder::new(&view, DEFAULT_MAX_TARGET);
    let s = b.build(&mut g, &scene, 0.0);
    assert_eq!(
        s.sprites.len(),
        scene.sprites.len() + 1,
        "every sprite draws"
    );
    png("main", &mut g, &scene);
    for (name, page) in [
        ("options", Page::Options),
        ("audio", Page::Audio),
        ("new", Page::NewGameLevels),
        ("quit", Page::QuitConfirm),
    ] {
        let p = nav.tree.find_page(page).unwrap();
        nav.show(p, 1);
        let sc = ScreenScene {
            sprites: draw::sprites(&nav, &mut g, &MenuState::default(), 0.0),
            ..scene.clone()
        };
        png(name, &mut g, &sc);
    }

    let (title, title_pal) = a.picture(TITLE_SCREENS, TITLE_PICTURE).unwrap();
    let id = g.add_image(&assets::picture_frame(&title));
    let t = TitleScreen::new(Some(id), title_pal, a.sound_ms("TITLE"));
    assert!(t.left_ms() > 1000.0, "the title sound has a length");
    assert_eq!(b.build(&mut g, &t.scene(), 0.0).sprites.len(), 1);
    png("title", &mut g, &t.scene());

    let mut sound_ms = BTreeMap::new();
    for (k, v) in &a.sound_ms {
        sound_ms.insert(k.clone(), *v);
    }
    for key in [
        booty::IMPACT,
        booty::CLAW_LINES[0],
        booty::MAP_LOOP,
        booty::BOUNCE,
    ] {
        assert!(sound_ms.contains_key(key), "{key} is missing");
    }
    for k in booty::TREASURES {
        assert!(sound_ms.contains_key(k.sound), "{} is missing", k.sound);
    }
    let pic = |name: &str, g: &mut LevelGraphics| {
        let (img, pal) = a
            .picture(BOOTY_SCREENS, name)
            .unwrap_or_else(|| panic!("{name}"));
        (Some(g.add_image(&assets::picture_frame(&img))), pal)
    };
    let art = BootyArt {
        map: pic(&booty::map_picture(1), &mut g),
        tally: pic(&booty::tally_picture(1, false), &mut g),
        sound_ms,
        digit_width: g
            .frame_info(booty::DIGITS, 0)
            .map_or(12, |f| f.width as i32),
        total_offsets: (
            g.frame_info(booty::SYMBOLS, 4)
                .map_or(60, |f| f.width as i32 / 2 + 14),
            g.frame_info(booty::SYMBOLS, 5)
                .map_or(64, |f| f.width as i32 / 2 + 14),
        ),
    };
    let mut collected = [0; 9];
    collected[0] = 23;
    let mut screen = BootyScreen::new(
        BootyStats {
            level: 1,
            custom: false,
            collected,
            census: [40, 3, 0, 0, 0, 0, 0, 0, 0],
            score: 12_345,
            lives: 5,
        },
        art,
    );
    for set in BootyScreen::image_sets(1) {
        assert!(!g.frame_numbers(&set).is_empty(), "{set} is missing");
    }
    let map = screen.scene();
    assert_eq!(b.build(&mut g, &map, 0.0).sprites.len(), 2);
    png("map", &mut g, &map);
    let mut arrived = screen.clone();
    arrived.update(7000.0);
    png("map_end", &mut g, &arrived.scene());
    screen.skip();
    screen.skip();
    let tally = screen.scene();
    let built = b.build(&mut g, &tally, 0.0);
    assert_eq!(
        built.sprites.len(),
        tally.sprites.len() + 1,
        "every tally sprite draws"
    );
    png("tally", &mut g, &tally);
}
