//! Menus and standalone screens on synthetic menu trees and art: the
//! command catalogue, the slot and image rules, layout, navigation with
//! held keys, sliders, drawing, the title, the end-of-level tally and the
//! game-over fade.

use std::collections::BTreeMap;

use claw_assets::Palette;
use claw_assets::ifc::IfcNode;
use claw_frame::AudioCue;
use claw_render::FrameInfo;
use claw_ui::menu::command::{CommandCatalogue, MenuCommand, MenuKind, Page, Slider, Toggle};
use claw_ui::menu::draw::{self, Art, MenuState, Switches};
use claw_ui::menu::layout;
use claw_ui::menu::nav::{BLINK_MS, Look, PRESS_MS, REPEAT_FIRST_MS, REPEAT_NEXT_MS};
use claw_ui::menu::tree::{self, MenuTree};
use claw_ui::menu::{MenuAction, MenuInput, Navigator, Unavailable, VideoSetting};
use claw_ui::screens::booty::{self, BootyArt, BootyPhase, BootyScreen, BootyStats};
use claw_ui::screens::gameover::{self, GameOverScreen};
use claw_ui::screens::title::{self, TitleScreen};

/// Every image is `w` x 30 with no offsets, where `w` is 100 plus the
/// frame number; sets named `MISSING` have nothing.
#[derive(Default)]
struct FakeArt {
    sizes: BTreeMap<(String, i32), FrameInfo>,
}

impl Art for FakeArt {
    fn info(&mut self, set: &str, frame: i32) -> Option<FrameInfo> {
        if set.contains("MISSING") {
            return None;
        }
        if let Some(f) = self.sizes.get(&(set.to_string(), frame)) {
            return Some(*f);
        }
        Some(FrameInfo {
            width: 100 + frame.max(0) as u32,
            height: 30,
            offset_x: 0,
            offset_y: 0,
        })
    }

    fn frames(&mut self, set: &str) -> Vec<i32> {
        if set.contains("MISSING") {
            Vec::new()
        } else {
            (1..=4).collect()
        }
    }
}

fn node(command: u32, normal: &str, hi: &str, children: Vec<IfcNode>) -> IfcNode {
    let mut n = IfcNode {
        command,
        label: format!("{command}"),
        children,
        ..IfcNode::default()
    };
    n.slots[0] = normal.to_string();
    n.slots[3] = hi.to_string();
    n
}

fn leaf(command: u32, normal: &str, hi: &str) -> IfcNode {
    node(command, normal, hi, vec![])
}

/// A main menu shaped like the game's: single player (new game list, load
/// list with entries, back), multiplayer, options (audio with sliders,
/// display), quit.
fn main_menu() -> IfcNode {
    let new_list: Vec<IfcNode> = (1..=14)
        .map(|n| {
            leaf(
                11100 + n,
                &format!("{:03}_singleplayernew_x", 3 * n - 2),
                &format!("{:03}_singleplayernew_x", 3 * n - 1),
            )
        })
        .chain([leaf(
            11115,
            "043_singleplayernew_back",
            "044_singleplayernew_back",
        )])
        .collect();
    let load_list: Vec<IfcNode> = (1..=14)
        .map(|n| {
            node(
                11200 + n,
                &format!("{:03}_singleplayerload_x", 3 * n - 2),
                &format!("{:03}_singleplayerload_x", 3 * n - 1),
                (1..=4)
                    .map(|k| {
                        leaf(
                            112_000 + 10 * n + k,
                            "001_singleplayerloadsub_a",
                            "002_singleplayerloadsub_a",
                        )
                    })
                    .collect(),
            )
        })
        .chain([leaf(
            11215,
            "043_singleplayerload_back",
            "044_singleplayerload_back",
        )])
        .collect();
    let mut sound = leaf(13401, "001_optionsaudio_sound", "003_optionsaudio_sound");
    sound.slots[5] = "031_optionsaudio_sound".into();
    sound.slots[6] = "031_optionsaudio_sound".into();
    let mut voice = leaf(13402, "005_optionsaudio_voice", "007_optionsaudio_voice");
    voice.slots[5] = "032_optionsaudio_voice".into();
    voice.slots[6] = "032_optionsaudio_voice".into();
    node(
        10000,
        "001_main_bogus",
        "002_main_bogus",
        vec![
            node(
                10001,
                "001_main_single",
                "002_main_single",
                vec![
                    node(
                        11001,
                        "001_singleplayer_new",
                        "002_singleplayer_new",
                        new_list,
                    ),
                    node(
                        11002,
                        "003_singleplayer_load",
                        "004_singleplayer_load",
                        load_list,
                    ),
                    leaf(11006, "012_singleplayer_back", "013_singleplayer_back"),
                ],
            ),
            node(
                10002,
                "003_main_multi",
                "004_main_multi",
                vec![leaf(
                    12001,
                    "001_multiplayer_racing",
                    "002_multiplayer_racing",
                )],
            ),
            node(
                10004,
                "008_main_options",
                "009_main_options",
                vec![
                    node(
                        13004,
                        "007_options_audio",
                        "008_options_audio",
                        vec![
                            sound,
                            voice,
                            leaf(13404, "013_optionsaudio_music", "015_optionsaudio_music"),
                            leaf(13405, "017_optionsaudio_back", "018_optionsaudio_back"),
                        ],
                    ),
                    node(
                        13005,
                        "010_options_display",
                        "011_options_display",
                        vec![
                            leaf(
                                13503,
                                "009_optionsdisplay_front",
                                "011_optionsdisplay_front",
                            ),
                            leaf(13504, "013_optionsdisplay_area", "014_optionsdisplay_area"),
                            leaf(13506, "019_optionsdisplay_back", "020_optionsdisplay_back"),
                        ],
                    ),
                    leaf(13006, "012_options_back", "013_options_back"),
                ],
            ),
            node(
                10006,
                "012_main_quit",
                "013_main_quit",
                vec![
                    leaf(14001, "001_quit_yes", "002_quit_yes"),
                    leaf(14002, "003_quit_no", "004_quit_no"),
                ],
            ),
        ],
    )
}

fn in_game_menu() -> IfcNode {
    node(
        20000,
        "001_main_bogus",
        "002_main_bogus",
        vec![
            leaf(20001, "001_main_return", "002_main_return"),
            node(
                20002,
                "003_main_clawicide",
                "004_main_clawicide",
                vec![
                    leaf(22001, "001_clawicide_yes", "002_clawicide_yes"),
                    leaf(22002, "003_clawicide_no", "004_clawicide_no"),
                ],
            ),
            node(
                20003,
                "005_main_options",
                "006_main_options",
                vec![leaf(23004, "007_options_back", "008_options_back")],
            ),
            leaf(20004, "007_main_help", "008_main_help"),
            node(
                20005,
                "009_main_quit",
                "010_main_quit",
                vec![
                    leaf(24001, "001_endgame_yes", "002_endgame_yes"),
                    leaf(24002, "003_endgame_no", "004_endgame_no"),
                ],
            ),
        ],
    )
}

fn nav(root: IfcNode, kind: MenuKind) -> Navigator {
    let tree = MenuTree::from_ifc(root, &CommandCatalogue::new(kind));
    let mut n = Navigator::new(tree);
    draw::measure(&mut n, &mut FakeArt::default());
    n
}

/// Runs a press to its end and returns what it did.
fn finish(n: &mut Navigator) -> Vec<MenuAction> {
    n.update(PRESS_MS, None)
}

fn choose(n: &mut Navigator) -> Vec<MenuAction> {
    assert!(n.input(MenuInput::Confirm).is_empty());
    assert!(n.pressing());
    finish(n)
}

fn selected_id(n: &Navigator) -> u32 {
    n.tree.items[n.selected().unwrap()].id
}

#[test]
fn every_in_scope_command_maps_to_its_action() {
    use MenuCommand as C;
    let main = CommandCatalogue::new(MenuKind::Main);
    let cases: Vec<(u32, MenuCommand)> = vec![
        (10001, C::Open(Page::SinglePlayer)),
        (10004, C::Open(Page::Options)),
        (10006, C::Open(Page::QuitConfirm)),
        (11001, C::Open(Page::NewGameLevels)),
        (11002, C::Open(Page::LoadGameLevels)),
        (11004, C::Open(Page::SaveInfo)),
        (13004, C::Open(Page::Audio)),
        (13005, C::Open(Page::Display)),
        (11006, C::Back),
        (11115, C::Back),
        (11215, C::Back),
        (11401, C::Back),
        (13006, C::Back),
        (13405, C::Back),
        (13506, C::Back),
        (14002, C::Back),
        (14001, C::Quit),
        (13003, C::Controls),
        (13401, C::Toggle(Toggle::Sound)),
        (13402, C::Toggle(Toggle::Voice)),
        (13403, C::Toggle(Toggle::Ambient)),
        (13404, C::Toggle(Toggle::Music)),
        (13502, C::Toggle(Toggle::HighDetail)),
        (13503, C::Toggle(Toggle::FrontLayer)),
        (13505, C::Toggle(Toggle::MovieInterlace)),
        (13504, C::Adjust(Slider::PlayArea)),
        (10002, C::Unavailable(Unavailable::Multiplayer)),
        (10003, C::Unavailable(Unavailable::Movies)),
        (10005, C::Unavailable(Unavailable::Help)),
        (10007, C::Unavailable(Unavailable::Credits)),
        (11003, C::Unavailable(Unavailable::CustomLevels)),
        (11005, C::Unavailable(Unavailable::UploadScores)),
        (13001, C::Unavailable(Unavailable::Players)),
        (15001, C::Unavailable(Unavailable::Movies)),
        (15010, C::Unavailable(Unavailable::Movies)),
        (15011, C::Back),
        (13101, C::Nothing),
        (13201, C::Nothing),
        (99999, C::Nothing),
    ];
    for (id, want) in cases {
        assert_eq!(main.entry(id).command, want, "main {id}");
    }
    for n in 1..=14u32 {
        assert_eq!(main.entry(11100 + n).command, C::NewGame { level: n });
        assert_eq!(
            main.entry(11200 + n).command,
            C::Open(Page::LoadEntries { level: n })
        );
        for entry in 0..3u8 {
            let id = 112_000 + 10 * n + u32::from(entry) + 1;
            assert_eq!(main.entry(id).command, C::Load { level: n, entry }, "{id}");
        }
        assert_eq!(main.entry(112_000 + 10 * n + 4).command, C::Back);
    }
    assert_eq!(main.entry(13401).slider, Some(Slider::SoundVolume));
    assert_eq!(main.entry(13404).slider, Some(Slider::MusicVolume));
    assert_eq!(main.entry(13504).slider, Some(Slider::PlayArea));
    assert_eq!(main.entry(13402).slider, None);

    let game = CommandCatalogue::new(MenuKind::InGame);
    let cases: Vec<(u32, MenuCommand)> = vec![
        (20001, C::Resume),
        (20002, C::Open(Page::EndLifeConfirm)),
        (20003, C::Open(Page::Options)),
        (20004, C::Unavailable(Unavailable::Help)),
        (20005, C::Open(Page::EndGameConfirm)),
        (22001, C::EndLife),
        (22002, C::Back),
        (23001, C::Controls),
        (23002, C::Open(Page::Display)),
        (23003, C::Open(Page::Audio)),
        (23004, C::Back),
        (23201, C::Toggle(Toggle::Sound)),
        (23202, C::Toggle(Toggle::Voice)),
        (23203, C::Toggle(Toggle::Ambient)),
        (23204, C::Toggle(Toggle::Music)),
        (23205, C::Back),
        (23301, C::Toggle(Toggle::FrontLayer)),
        (23302, C::Adjust(Slider::PlayArea)),
        (23303, C::Toggle(Toggle::MovieInterlace)),
        (23304, C::Back),
        (24001, C::EndGame),
        (24002, C::Back),
    ];
    for (id, want) in cases {
        assert_eq!(game.entry(id).command, want, "in game {id}");
    }
    assert_eq!(game.entry(23201).slider, Some(Slider::SoundVolume));
    assert_eq!(game.entry(23204).slider, Some(Slider::MusicVolume));
    assert_eq!(game.entry(23302).slider, Some(Slider::PlayArea));
    let over = CommandCatalogue::new(MenuKind::GameOver);
    assert_eq!(over.entry(30001).command, C::Nothing);
}

#[test]
fn hotkeys_follow_the_table() {
    let main = CommandCatalogue::new(MenuKind::Main);
    let keys: String = [10001, 10002, 10003, 10004, 10007, 10005, 10006]
        .iter()
        .map(|&id| main.entry(id).hotkey.unwrap())
        .collect();
    assert_eq!(keys, "SMROCHQ");
    let levels: String = (11101..=11114)
        .map(|id| main.entry(id).hotkey.unwrap())
        .collect();
    assert_eq!(levels, "RAFWOLDSPCVUIT");
    assert_eq!(main.entry(112_011).hotkey, None);
    assert_eq!(main.entry(112_041).hotkey, Some('S'));
    assert_eq!(main.entry(112_012).hotkey, Some('1'));
    assert_eq!(main.entry(112_013).hotkey, Some('2'));
    assert_eq!(main.entry(112_014).hotkey, Some('B'));
    assert_eq!(main.entry(14001).hotkey, Some('Y'));
    let game = CommandCatalogue::new(MenuKind::InGame);
    let keys: String = (20001..=20005)
        .map(|id| game.entry(id).hotkey.unwrap())
        .collect();
    assert_eq!(keys, "RLOHE");
}

#[test]
fn empty_slots_are_filled_in_order() {
    let mut s: [String; 8] = Default::default();
    s[0] = "a".into();
    s[3] = "d".into();
    let f = tree::fill_slots(s.clone());
    assert_eq!(
        f,
        ["a", "d", "d", "d", "d", "a", "d", "d"].map(String::from)
    );
    s[1] = "b".into();
    s[6] = "g".into();
    let f = tree::fill_slots(s);
    assert_eq!(
        f,
        ["a", "b", "b", "d", "d", "a", "g", "g"].map(String::from)
    );
}

#[test]
fn image_names_pick_a_set_and_frame() {
    let img = |k, n| tree::image_set_of(k, n).map(|i| (i.set, i.frame));
    let s = |v: &str, f| Some((v.to_string(), f));
    assert_eq!(img(MenuKind::Main, "001_main_single"), s("MENU_MAIN", 1));
    assert_eq!(
        img(MenuKind::Main, "043_singleplayernew_back"),
        s("MENU_MAIN_SINGLEPLAYER_LEVELS", 43)
    );
    assert_eq!(
        img(MenuKind::Main, "009_singleplayerloadsub_back"),
        s("MENU_MAIN_SINGLEPLAYER_LOAD_CHECKPOINTS", 9)
    );
    assert_eq!(
        img(MenuKind::Main, "012_singleplayer_previousmenu"),
        s("MENU_MAIN_SINGLEPLAYER", 12)
    );
    assert_eq!(
        img(MenuKind::Main, "031_OPTIONSAUDIO_sound"),
        s("MENU_MAIN_OPTIONS_AUDIO", 31)
    );
    assert_eq!(
        img(MenuKind::Main, "004_replay_intro"),
        s("MENU_MAIN_REPLAYMOVIES", 4)
    );
    assert_eq!(img(MenuKind::Main, "nothing"), None);
    assert_eq!(img(MenuKind::Main, ""), None);
    assert_eq!(
        img(MenuKind::InGame, "003_clawicide_no"),
        s("GAME_INGAMEMENU_MAIN_CLAWICIDE", 3)
    );
    assert_eq!(
        img(MenuKind::InGame, "015_optionsaudio_music"),
        s("GAME_INGAMEMENU_MAIN_OPTIONS_AUDIO", 15)
    );
    assert_eq!(
        img(MenuKind::GameOver, "003_main_no"),
        s("GAME_GAMEOVERMENU_MAIN", 3)
    );
    assert_eq!(tree::leading_number("  -12x"), -12);
    assert_eq!(tree::leading_number("x12"), 0);
    assert_eq!(tree::leading_number("99999999999"), i32::MAX);
}

#[test]
fn the_root_spreads_over_its_band_and_lists_form_two_columns() {
    let mut n = nav(main_menu(), MenuKind::Main);
    // Root: four entries, 30 high, over 215..474.
    let ys: Vec<i32> = n
        .entries()
        .iter()
        .map(|&k| n.tree.items[k].cell.y)
        .collect();
    let gap = (474 - 215 - 4 * 30) / 3;
    assert_eq!(
        ys,
        vec![
            215,
            215 + 30 + gap,
            215 + 2 * (30 + gap),
            215 + 3 * (30 + gap)
        ]
    );
    // Each entry is as wide as its widest image, centred.
    let single = n.entries()[0];
    assert_eq!(n.tree.items[single].size, (102, 30));
    assert_eq!(n.tree.items[single].cell.x, 320 - 102 / 2);
    // New game levels: two columns of seven, the back entry in the band.
    choose(&mut n);
    assert_eq!(n.page(), Page::SinglePlayer);
    choose(&mut n);
    assert_eq!(n.page(), Page::NewGameLevels);
    let cells: Vec<_> = n.entries().iter().map(|&k| n.tree.items[k].cell).collect();
    assert_eq!((cells[0].x, cells[0].y), (37, 213));
    assert_eq!((cells[6].x, cells[6].y), (37, 213 + 6 * 28));
    assert_eq!((cells[7].x, cells[7].y), (308, 213));
    assert_eq!((cells[13].x, cells[13].y), (308, 213 + 6 * 28));
    let back = cells[14];
    let all = layout::stacked(
        &n.entries()
            .iter()
            .map(|&k| n.tree.items[k].size)
            .collect::<Vec<_>>(),
    );
    assert_eq!(back.x, all[14].x);
    let mut spread = all.clone();
    layout::spread(&mut spread, 220, 442);
    assert_eq!(back.y, spread[14].y);
}

#[test]
fn stacking_and_bands_follow_the_formulas() {
    let cells = layout::stacked(&[(100, 30), (50, 20)]);
    assert_eq!((cells[0].x, cells[0].y), (270, 240 - 25 - 10));
    assert_eq!((cells[1].x, cells[1].y), (295, 240 - 25 - 10 + 40));
    let mut one = layout::stacked(&[(10, 10)]);
    layout::spread(&mut one, 300, 200);
    assert_eq!(one[0].y, 200);
    // Negative gaps truncate toward zero.
    let mut tight = layout::stacked(&[(10, 100), (10, 100)]);
    layout::spread(&mut tight, 0, 150);
    assert_eq!((tight[0].y, tight[1].y), (0, 50));
    assert_eq!(layout::in_game_band(Page::Root), Some((145, 335)));
    assert_eq!(layout::in_game_band(Page::EndLifeConfirm), Some((268, 335)));
    assert_eq!(layout::in_game_band(Page::Options), Some((164, 316)));
    assert_eq!(layout::main_band(Page::QuitConfirm), Some((294, 368)));
}

#[test]
fn up_and_down_wrap_without_skipping_disabled_entries() {
    let mut n = nav(main_menu(), MenuKind::Main);
    n.tree.set_enabled_by(|i| i.id != 10002);
    assert_eq!(selected_id(&n), 10001);
    assert_eq!(n.input(MenuInput::Down), vec![MenuAction::Moved]);
    assert_eq!(selected_id(&n), 10002, "disabled entries are not skipped");
    assert_eq!(n.look(n.selected().unwrap()).slot(), 7, "blink phase on");
    n.input(MenuInput::Up);
    n.input(MenuInput::Up);
    assert_eq!(selected_id(&n), 10006, "wraps to the last entry");
    n.input(MenuInput::Down);
    assert_eq!(selected_id(&n), 10001, "and back to the first");
}

#[test]
fn a_disabled_entry_reports_itself_and_opens_nothing() {
    let mut n = nav(main_menu(), MenuKind::Main);
    n.tree.set_enabled_by(|i| i.id != 10002);
    n.input(MenuInput::Down);
    let out = choose(&mut n);
    assert_eq!(
        out,
        vec![MenuAction::Disabled(MenuCommand::Unavailable(
            Unavailable::Multiplayer
        ))]
    );
    assert_eq!(n.page(), Page::Root);
}

#[test]
fn a_press_takes_300_ms_and_blocks_other_input() {
    let mut n = nav(main_menu(), MenuKind::Main);
    n.input(MenuInput::Confirm);
    let item = n.selected().unwrap();
    assert!(n.look(item).pressed);
    assert_eq!(n.look(item).slot(), 2, "pressed in the blink phase");
    assert!(n.input(MenuInput::Down).is_empty(), "ignored while pressed");
    assert!(n.update(PRESS_MS - 1.0, None).is_empty());
    let out = n.update(1.0, None);
    assert_eq!(
        out,
        vec![MenuAction::Command(MenuCommand::Open(Page::SinglePlayer))]
    );
    assert_eq!(n.page(), Page::SinglePlayer);
    assert_eq!(selected_id(&n), 11001, "the submenu's first entry");
}

#[test]
fn back_entries_and_escape_return_to_the_entry_they_came_from() {
    let mut n = nav(main_menu(), MenuKind::Main);
    n.input(MenuInput::Down);
    n.input(MenuInput::Down);
    choose(&mut n); // options
    choose(&mut n); // audio
    assert_eq!(n.page(), Page::Audio);
    assert_eq!(n.input(MenuInput::Back), vec![MenuAction::Back]);
    assert_eq!(n.page(), Page::Options);
    assert_eq!(selected_id(&n), 13004);
    n.input(MenuInput::Up);
    assert_eq!(selected_id(&n), 13006);
    assert_eq!(choose(&mut n), vec![MenuAction::Command(MenuCommand::Back)]);
    assert_eq!(n.page(), Page::Root);
    assert_eq!(selected_id(&n), 10004);
    // Escape at the top highlights quit and asks.
    assert!(n.input(MenuInput::Back).is_empty());
    assert_eq!(selected_id(&n), 10006);
    finish(&mut n);
    assert_eq!(n.page(), Page::QuitConfirm);
    assert_eq!(choose(&mut n), vec![MenuAction::Command(MenuCommand::Quit)]);
}

#[test]
fn escape_in_game_resumes_from_any_submenu() {
    let mut n = nav(in_game_menu(), MenuKind::InGame);
    n.input(MenuInput::Down);
    choose(&mut n);
    assert_eq!(n.page(), Page::EndLifeConfirm);
    assert_eq!(
        n.input(MenuInput::Back),
        vec![MenuAction::Command(MenuCommand::Resume)]
    );
}

#[test]
fn held_directions_repeat_after_350_ms_then_every_30_ms() {
    let mut n = nav(main_menu(), MenuKind::Main);
    let held = Some(MenuInput::Down);
    assert_eq!(n.input(MenuInput::Down), vec![MenuAction::Moved]);
    let mut moves = 1;
    let mut at = Vec::new();
    let mut t = 0.0;
    // 1 ms frames for 500 ms.
    for _ in 0..500 {
        t += 1.0;
        let out = n.update(1.0, held);
        if !out.is_empty() {
            moves += out.len();
            at.push(t);
        }
    }
    assert_eq!(at[0], REPEAT_FIRST_MS);
    assert_eq!(at[1], REPEAT_FIRST_MS + REPEAT_NEXT_MS);
    assert_eq!(at[2], REPEAT_FIRST_MS + 2.0 * REPEAT_NEXT_MS);
    assert_eq!(
        moves,
        1 + 1 + ((500.0 - REPEAT_FIRST_MS) / REPEAT_NEXT_MS) as usize
    );
    // Releasing stops it; holding again without a press does not repeat.
    assert!(n.update(1.0, None).is_empty());
    assert!(n.update(1000.0, held).is_empty());
    // Long frames repeat as often as the time allows, up to a limit.
    n.input(MenuInput::Up);
    let out = n.update(REPEAT_FIRST_MS + 3.0 * REPEAT_NEXT_MS, Some(MenuInput::Up));
    assert_eq!(out.len(), 4);
    // A press restarts the delay.
    let mut n = nav(main_menu(), MenuKind::Main);
    n.input(MenuInput::Down);
    n.update(300.0, held);
    n.input(MenuInput::Down);
    assert!(n.update(300.0, held).is_empty());
    assert_eq!(n.update(50.0, held).len(), 1);
}

#[test]
fn the_highlight_blinks_every_400_ms() {
    let mut n = nav(main_menu(), MenuKind::Main);
    let item = n.selected().unwrap();
    assert!(n.look(item).blink);
    n.update(BLINK_MS - 1.0, None);
    assert!(n.look(item).blink);
    n.update(1.0, None);
    assert!(!n.look(item).blink);
    assert_eq!(n.look(item).slot(), 3);
    let other = n.entries()[1];
    assert_eq!(n.look(other).slot(), 0);
    let l = Look {
        highlighted: false,
        pressed: false,
        disabled: true,
        blink: false,
    };
    assert_eq!(l.slot(), 5);
}

#[test]
fn sliders_step_and_stop_at_their_limits() {
    let mut n = nav(main_menu(), MenuKind::Main);
    n.input(MenuInput::Down);
    n.input(MenuInput::Down);
    choose(&mut n);
    choose(&mut n);
    assert_eq!(selected_id(&n), 13401);
    assert_eq!(
        n.input(MenuInput::Right),
        vec![MenuAction::Slider {
            slider: Slider::SoundVolume,
            delta: 8
        }]
    );
    assert_eq!(
        n.input(MenuInput::Left),
        vec![MenuAction::Slider {
            slider: Slider::SoundVolume,
            delta: -8
        }]
    );
    n.input(MenuInput::Down);
    assert!(n.input(MenuInput::Right).is_empty(), "voice is no slider");
    n.input(MenuInput::Down);
    assert_eq!(
        n.input(MenuInput::Left),
        vec![MenuAction::Slider {
            slider: Slider::MusicVolume,
            delta: -10
        }]
    );
    let s = Slider::SoundVolume;
    assert_eq!(s.adjust(80, 8), 80);
    assert_eq!(s.adjust(72, 8), 80);
    assert_eq!(s.adjust(4, -8), 0);
    assert_eq!(s.adjust(0, -8), 0);
    assert_eq!(
        s.adjust(90, 8),
        90,
        "a louder setting is not lowered by raising"
    );
    assert_eq!(s.adjust(90, -8), 80);
    let m = Slider::MusicVolume;
    assert_eq!(m.adjust(47, 10), 60, "rounded to the nearest step first");
    assert_eq!(m.adjust(44, -10), 30);
    assert_eq!(m.adjust(100, 10), 100);
    assert_eq!(m.adjust(0, -10), 0);
    let a = Slider::PlayArea;
    assert_eq!(a.adjust(10, 1), 10);
    assert_eq!(a.adjust(1, -1), 1);
    assert_eq!(a.adjust(5, 1), 6);
}

#[test]
fn left_and_right_jump_columns_in_the_level_lists() {
    let mut n = nav(main_menu(), MenuKind::Main);
    choose(&mut n);
    choose(&mut n);
    assert_eq!(n.page(), Page::NewGameLevels);
    assert_eq!(n.input(MenuInput::Right), vec![MenuAction::Moved]);
    assert_eq!(selected_id(&n), 11108);
    assert!(n.input(MenuInput::Right).is_empty(), "past the back entry");
    n.input(MenuInput::Left);
    assert_eq!(selected_id(&n), 11101);
    n.tree.set_enabled_by(|i| i.id != 11108);
    assert!(
        n.input(MenuInput::Right).is_empty(),
        "a greyed-out level is not a target"
    );
}

#[test]
fn left_and_right_step_text_entries() {
    let mut n = nav(main_menu(), MenuKind::Main);
    let display = n.tree.find_page(Page::Display).unwrap();
    let page = n
        .tree
        .add_text(display, "CONTROLS", MenuCommand::Open(Page::Video));
    n.tree
        .add_text(page, "STICK DEAD ZONE 66", MenuCommand::PadDeadZone);
    let back = n.tree.add_text(page, "BACK", MenuCommand::Back);
    n.show(page, 0);
    assert_eq!(
        n.input(MenuInput::Right),
        vec![MenuAction::Step {
            command: MenuCommand::PadDeadZone,
            delta: 1
        }]
    );
    assert_eq!(
        n.input(MenuInput::Left),
        vec![MenuAction::Step {
            command: MenuCommand::PadDeadZone,
            delta: -1
        }]
    );
    n.tree
        .set_enabled_by(|i| i.command != MenuCommand::PadDeadZone);
    assert!(
        n.input(MenuInput::Right).is_empty(),
        "a greyed-out entry does not step"
    );
    n.show(page, 1);
    assert_eq!(n.selected(), Some(back));
}

#[test]
fn hotkeys_choose_enabled_entries() {
    let mut n = nav(main_menu(), MenuKind::Main);
    assert_eq!(n.hotkey('o'), vec![MenuAction::Moved]);
    assert!(n.pressing());
    finish(&mut n);
    assert_eq!(n.page(), Page::Options);
    n.input(MenuInput::Back);
    n.tree.set_enabled_by(|i| i.id != 10002);
    assert_eq!(n.hotkey('M'), vec![MenuAction::Moved]);
    assert!(!n.pressing(), "a greyed-out entry is only highlighted");
    assert!(n.hotkey('z').is_empty());
}

#[test]
fn every_main_page_is_reachable_and_leads_to_its_commands() {
    let mut n = nav(main_menu(), MenuKind::Main);
    // New game at level 9: single player, new game, right column, two down.
    choose(&mut n);
    choose(&mut n);
    n.input(MenuInput::Right);
    n.input(MenuInput::Down);
    assert_eq!(
        choose(&mut n),
        vec![MenuAction::Command(MenuCommand::NewGame { level: 9 })]
    );
    // Load level 2, save point 2.
    let mut n = nav(main_menu(), MenuKind::Main);
    choose(&mut n);
    n.input(MenuInput::Down);
    choose(&mut n);
    n.input(MenuInput::Down);
    choose(&mut n);
    assert_eq!(n.page(), Page::LoadEntries { level: 2 });
    n.input(MenuInput::Down);
    n.input(MenuInput::Down);
    assert_eq!(
        choose(&mut n),
        vec![MenuAction::Command(MenuCommand::Load {
            level: 2,
            entry: 2
        })]
    );
}

#[test]
fn drawing_shows_switch_states_knobs_coins_and_headings() {
    let mut art = FakeArt::default();
    let mut n = nav(main_menu(), MenuKind::Main);
    let state = MenuState::default();
    let s = draw::sprites(&n, &mut art, &state, 0.0);
    assert_eq!(
        (s[0].key.as_str(), s[0].frame, s[0].x, s[0].y),
        ("MENU_MAIN", 14, 320, 180)
    );
    // The highlighted entry shows slot 3 (frame 2), the others slot 0.
    assert_eq!(s[1].frame, 2);
    assert_eq!(s[2].frame, 3);
    // Two coins at the extent's sides, 17 below the highlighted entry's top.
    let coins: Vec<_> = s.iter().filter(|x| x.key == "MENU_COINS").collect();
    assert_eq!(coins.len(), 2);
    let cell = n.tree.items[n.selected().unwrap()].cell;
    assert_eq!(coins[0].y, cell.y + 17);
    assert!(coins[0].x < cell.x && coins[1].x > cell.x + cell.w);
    // The coins cycle every 100 ms.
    let later = draw::sprites(&n, &mut art, &state, 250.0);
    let c2: Vec<_> = later.iter().filter(|x| x.key == "MENU_COINS").collect();
    assert_eq!((coins[0].frame, c2[0].frame), (1, 3));

    n.input(MenuInput::Down);
    n.input(MenuInput::Down);
    choose(&mut n);
    choose(&mut n);
    let mut off = state;
    off.switches.sound = false;
    off.switches.voice = false;
    off.sound_volume = 40;
    let s = draw::sprites(&n, &mut art, &off, 0.0);
    let frames: Vec<i32> = s
        .iter()
        .filter(|x| x.key == "MENU_MAIN_OPTIONS_AUDIO")
        .map(|x| x.frame)
        .collect();
    // Heading 19, sound highlighted (3 -> 4 while off), voice normal (5
    // -> 6), music 13, back 17, then the sound and music knobs.
    assert_eq!(frames, vec![19, 4, 6, 13, 17, 42, 41]);
    let knob = s.iter().find(|x| x.frame == 42).unwrap();
    let sound = &n.tree.items[n.entries()[0]];
    let left = sound.cell.x + (sound.cell.w - 104) / 2;
    assert_eq!(knob.x, draw::SOUND_KNOB_X[5] + left);
    // Greyed-out sound shows frame 31, never shifted.
    n.tree.set_enabled_by(|i| i.id != 13401);
    let s = draw::sprites(&n, &mut art, &off, 0.0);
    assert!(s.iter().any(|x| x.frame == 31));
    assert!(
        !s.iter().any(|x| x.frame == 42),
        "no knob on a greyed-out slider"
    );
    // Knob positions.
    assert_eq!(
        draw::knob_x(Slider::MusicVolume, 50, 10),
        draw::SOUND_KNOB_X[5] - 8 + 10
    );
    assert_eq!(draw::knob_x(Slider::PlayArea, 10, 0), draw::AREA_KNOB_X[9]);
    assert_eq!(draw::knob_x(Slider::PlayArea, 1, 0), draw::AREA_KNOB_X[0]);
    let mut sw = Switches::default();
    assert!(!sw.shifted(Toggle::FrontLayer));
    sw.set(Toggle::FrontLayer, false);
    assert!(sw.shifted(Toggle::FrontLayer));
    assert!(
        sw.shifted(Toggle::HighDetail),
        "high detail shows its other frame when on"
    );
}

#[test]
fn in_game_confirmations_show_their_question() {
    let mut art = FakeArt::default();
    let mut n = nav(in_game_menu(), MenuKind::InGame);
    n.input(MenuInput::Down);
    choose(&mut n);
    let mut state = MenuState::default();
    let s = draw::sprites(&n, &mut art, &state, 0.0);
    assert!(s.iter().any(|x| x.key == "GAME_INGAMEMENU_MAIN_CLAWICIDE"
        && x.frame == 5
        && (x.x, x.y) == (320, 202)));
    state.lives = 1;
    let s = draw::sprites(&n, &mut art, &state, 0.0);
    assert!(s.iter().any(|x| x.frame == 6 && x.y == 202));
    assert!(s.iter().any(|x| x.key == "GAME_TREASURE_COINS"));
}

#[test]
fn text_entries_measure_and_draw_with_the_font() {
    let mut art = FakeArt::default();
    let tree = MenuTree::from_ifc(main_menu(), &CommandCatalogue::new(MenuKind::Main));
    let mut n = Navigator::new(tree);
    let display = n.tree.find_page(Page::Display).unwrap();
    let video = n
        .tree
        .add_text(display, "VIDEO", MenuCommand::Open(Page::Video));
    n.tree.add_text(
        video,
        "ASPECT FILL",
        MenuCommand::Video(VideoSetting::Aspect),
    );
    n.tree.add_text(video, "BACK", MenuCommand::Back);
    draw::measure(&mut n, &mut art);
    let (w, h) = draw::text_size(&mut art, "AB");
    assert_eq!((w, h), (101 + 102, 30));
    assert_eq!(n.tree.items[video].size, draw::text_size(&mut art, "VIDEO"));
    n.show(video, 0);
    assert_eq!(n.page(), Page::Video);
    let s = draw::sprites(&n, &mut art, &MenuState::default(), 0.0);
    assert_eq!(s.iter().filter(|x| x.key == "GAME_FONT").count(), 10 + 4);
}

fn stats(collected: [u32; 9], census: [u32; 9]) -> BootyStats {
    BootyStats {
        level: 1,
        custom: false,
        collected,
        census,
        score: 50_000,
        lives: 4,
    }
}

fn art() -> BootyArt {
    let mut sound_ms = BTreeMap::new();
    sound_ms.insert(booty::IMPACT.to_string(), 400);
    sound_ms.insert(booty::CLAW_LINES[0].to_string(), 1200);
    BootyArt {
        map: (Some(1), Palette::default()),
        tally: (Some(2), Palette::default()),
        sound_ms,
        digit_width: 12,
        total_offsets: (60, 64),
    }
}

fn keys(audio: &[AudioCue]) -> Vec<String> {
    audio
        .iter()
        .filter_map(|a| match a {
            AudioCue::Play(c) if c.key != booty::MAP_LOOP => Some(c.key.clone()),
            _ => None,
        })
        .collect()
}

/// Runs `b` in 1 ms steps until `until` or `limit` ms, returning the time.
fn run_until(b: &mut BootyScreen, limit: f64, until: impl Fn(&BootyScreen) -> bool) -> f64 {
    let mut t = 0.0;
    while t < limit && !until(b) {
        b.update(1.0);
        t += 1.0;
    }
    t
}

#[test]
fn the_map_screen_flies_the_piece_then_claw_speaks() {
    let mut b = BootyScreen::new(stats([0; 9], [0; 9]), art());
    assert_eq!(b.phase(), BootyPhase::Map);
    let start = b.take_audio();
    assert!(
        matches!(&start[0], AudioCue::Play(c) if c.key == booty::MAP_LOOP && c.volume == 25 && c.looped)
    );
    assert_eq!(b.piece_position(), (-100, -240));
    b.update(4999.0);
    assert!(keys(&b.take_audio()).is_empty());
    b.update(1.0);
    assert_eq!(keys(&b.take_audio()), vec![booty::CURLING]);
    b.update(1000.0);
    assert_eq!(b.piece_position(), (-100 + 330, -240 + 312));
    b.update(1000.0);
    let a = b.take_audio();
    assert!(a.contains(&AudioCue::Stop {
        voice: booty::CURLING_VOICE
    }));
    assert_eq!(keys(&a), vec![booty::IMPACT]);
    assert_eq!(b.piece_position(), (560, 385));
    b.update(400.0);
    assert_eq!(keys(&b.take_audio()), vec![booty::CLAW_LINES[0]]);
    b.update(1199.0);
    assert_eq!(b.phase(), BootyPhase::Map);
    b.update(1.0);
    assert_eq!(b.phase(), BootyPhase::Counting);
    // The loop rose to 80 meanwhile.
    let mut b = BootyScreen::new(stats([0; 9], [0; 9]), art());
    b.update(750.0);
    let a = b.take_audio();
    assert!(
        matches!(a.last(), Some(AudioCue::Play(c)) if c.key == booty::MAP_LOOP && c.volume == 80)
    );
}

#[test]
fn the_tally_counts_with_the_original_timing() {
    // 37 coins of 40, 2 gold bars of 2, nothing else.
    let mut collected = [0; 9];
    collected[0] = 37;
    collected[1] = 2;
    let mut census = [0; 9];
    census[0] = 40;
    census[1] = 2;
    census[4] = 1;
    let mut s = stats(collected, census);
    s.custom = true; // straight to the tally
    s.score = 100_000;
    let mut b = BootyScreen::new(s, art());
    assert_eq!(b.phase(), BootyPhase::Counting);
    let level_points = 37 * 100 + 2 * 500;
    assert_eq!(b.totals(), (0, 100_000 - level_points));
    // Rows 8..2 have nothing: 300 ms each to fly in, then they finish.
    let t = run_until(&mut b, 10_000.0, |b| b.rows()[7].shown());
    assert_eq!(t, 7.0 * 300.0 + 300.0);
    assert_eq!(b.rows()[4].total, 1, "the total is the level's count");
    // Gold bars: interval trunc((25 + trunc(25 r / c)) * 4) = 200 then 150.
    assert_eq!(booty::count_interval(2, 2, 4.0), 200.0);
    assert_eq!(booty::count_interval(1, 2, 4.0), 148.0);
    b.take_audio();
    let t = run_until(&mut b, 10_000.0, |b| b.rows()[7].finished());
    assert_eq!(t, 200.0 + 148.0);
    let a = keys(&b.take_audio());
    assert_eq!(a, vec![booty::BOUNCE, booty::BOUNCE, booty::COMPLETE]);
    // Coins: steps of 10 then 7, products 1000 2000 3000 3700.
    let mut products = Vec::new();
    let mut last = 0;
    run_until(&mut b, 10_000.0, |b| {
        let p = b.rows()[8].product;
        p == 3700
    });
    let _ = &mut last;
    products.push(b.rows()[8].product);
    assert_eq!(b.rows()[8].count, 37);
    assert_eq!(b.totals(), (level_points, 100_000));
    assert!(!b.perfect(), "coins are missing");
    assert_eq!(b.phase(), BootyPhase::Hold);
    assert_eq!(booty::count_interval(37, 37, 1.0), 50.0);
    assert_eq!(booty::count_interval(27, 37, 1.0), 43.0);
}

#[test]
fn coins_count_in_steps_of_ten() {
    let mut collected = [0; 9];
    collected[0] = 37;
    let mut s = stats(collected, [0; 9]);
    s.custom = true;
    let mut b = BootyScreen::new(s, art());
    let mut seen = Vec::new();
    for _ in 0..20_000 {
        b.update(1.0);
        let p = b.rows()[8].product;
        if seen.last() != Some(&p) {
            seen.push(p);
        }
        if b.rows()[8].finished() {
            break;
        }
    }
    assert_eq!(seen, vec![0, 1000, 2000, 3000, 3700]);
}

#[test]
fn a_perfect_tally_gives_a_life_and_holds_then_slides_off() {
    let mut collected = [0; 9];
    collected[3] = 1;
    let mut b = BootyScreen::new(stats(collected, collected), art());
    b.skip();
    assert_eq!(b.phase(), BootyPhase::Counting);
    run_until(&mut b, 100_000.0, |b| b.phase() == BootyPhase::Hold);
    assert!(b.perfect());
    assert!(b.extra_life());
    assert!(keys(&b.take_audio()).contains(&booty::PERFECT_SOUND.to_string()));
    let t = run_until(&mut b, 100_000.0, |b| b.phase() != BootyPhase::Hold);
    assert_eq!(t, 60_000.0);
    let t = run_until(&mut b, 100_000.0, |b| b.done());
    assert_eq!(t, booty::slide_out_ms());
    assert_eq!(booty::slide_out_ms(), 400.0 + 25.0 * 75.0);
    // No extra life at 10 lives, and none on a custom level.
    let mut s = stats(collected, collected);
    s.lives = 10;
    let mut b = BootyScreen::new(s, art());
    b.skip();
    b.skip();
    assert!(b.perfect() && !b.extra_life());
    let mut s = stats(collected, collected);
    s.custom = true;
    let mut b = BootyScreen::new(s, art());
    b.skip();
    assert!(!b.perfect());
}

#[test]
fn skipping_completes_the_counting_then_ends_the_hold() {
    let mut collected = [0; 9];
    collected[0] = 55;
    collected[8] = 1;
    let mut s = stats(collected, [0; 9]);
    s.custom = true;
    s.score = 40_000;
    let mut b = BootyScreen::new(s, art());
    b.update(200.0);
    b.take_audio();
    b.skip();
    assert_eq!(b.phase(), BootyPhase::Hold);
    assert!(
        b.rows()
            .iter()
            .all(|r| r.finished() && r.count == r.collected)
    );
    assert_eq!(b.rows()[8].product, 5500);
    assert_eq!(b.totals(), (5500 + 25_000, 40_000));
    assert!(
        keys(&b.take_audio()).is_empty(),
        "no counting sounds when skipped"
    );
    b.skip();
    assert_eq!(b.phase(), BootyPhase::SlideOut);
    b.update(booty::slide_out_ms());
    assert!(b.done());
}

#[test]
fn the_tally_draws_rows_and_totals() {
    let mut collected = [0; 9];
    collected[0] = 12;
    let mut s = stats(collected, [0; 9]);
    s.custom = true;
    let mut b = BootyScreen::new(s, art());
    b.skip();
    let scene = b.scene();
    assert_eq!(scene.background, Some(2));
    let icons: Vec<_> = scene
        .sprites
        .iter()
        .filter(|x| x.key.starts_with("BOOTY_TREASURE"))
        .collect();
    assert_eq!(icons.len(), 9);
    assert_eq!((icons[0].x, icons[0].y), (160, 55));
    assert_eq!((icons[8].x, icons[8].y), (160, 55 + 44 * 8));
    // Coins row: 12 OF 12 TIMES 100 EQUALS 1200.
    let row: Vec<i32> = scene
        .sprites
        .iter()
        .filter(|x| x.key == booty::DIGITS && x.y == 55 + 44 * 8)
        .map(|x| x.frame)
        .collect();
    assert_eq!(row, vec![1, 2, 1, 2, 1, 0, 0, 1, 2, 0, 0]);
    // During the slide-out even rows go left and odd rows right.
    b.skip();
    b.update(100.0);
    let scene = b.scene();
    let x_of = |k: usize| {
        scene
            .sprites
            .iter()
            .find(|x| x.key == booty::TREASURES[k].set)
            .unwrap()
            .x
    };
    assert!(x_of(0) < 160 && x_of(1) > 160);
    assert_eq!(x_of(8), 160 - 25 * 25, "row 8 moved its 25 steps of 2 ms");
    assert_eq!(booty::map_picture(1), "001MAP");
    assert_eq!(booty::tally_picture(1, false), "002MAP");
    assert_eq!(booty::map_picture(2), "003AMULET");
    assert_eq!(booty::tally_picture(4, true), "030CUSTOM");
}

#[test]
fn the_title_lasts_its_sound_or_a_key() {
    let mut t = TitleScreen::new(Some(3), Palette::default(), Some(2500));
    let a = t.take_audio();
    assert!(matches!(&a[0], AudioCue::Play(c) if c.key == title::TITLE_SOUND));
    t.update(2499.0);
    assert!(!t.done());
    t.update(1.0);
    assert!(t.done());
    let mut t = TitleScreen::new(None, Palette::default(), None);
    assert!(t.take_audio().is_empty());
    assert_eq!(t.left_ms(), f64::from(title::DEFAULT_TITLE_MS));
    t.skip();
    assert!(t.done());
    assert_eq!(t.scene().background, None);
}

#[test]
fn game_over_holds_then_fades() {
    let mut g = GameOverScreen::new();
    g.skip();
    assert!(!g.done(), "inputs right away are ignored");
    g.update(gameover::HOLD_MS);
    assert_eq!(g.brightness(), 1.0);
    g.update(gameover::FADE_MS / 2.0);
    assert!((g.brightness() - 0.5).abs() < 1e-9);
    g.update(gameover::FADE_MS / 2.0);
    assert!(g.done());
    let mut g = GameOverScreen::new();
    g.update(gameover::GUARD_MS);
    g.skip();
    assert!(g.done());
}
