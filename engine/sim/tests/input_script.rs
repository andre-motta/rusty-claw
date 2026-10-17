//! Input scripts as routes: header keys, directives, the round trip
//! through text, and the spacing check.

use claw_sim::input::{
    Directive, DirectiveKind, InputScript, MenuScreen, ScriptCommand, ScriptTuning, SpacingWarning,
};
use claw_sim::{Buttons, Tuning};

const RIGHT: u32 = 0x200_0000;
const JUMP: u32 = 0x1;

fn parse(text: &str) -> InputScript {
    InputScript::parse(text).unwrap_or_else(|e| panic!("{e}\n{text}"))
}

fn parse_err(text: &str) -> (usize, String) {
    let e = InputScript::parse(text).expect_err(text);
    (e.line, e.message)
}

#[test]
fn scripts_without_keys_or_directives_parse_as_before() {
    let text = "# mid_air_hit_fix = on\n# a note\ntick,held_mask,pressed_mask\n\
                0,0x2000000,0x2000000\n1,33554432,0\n2,0x0,0x0\n";
    let s = parse(text);
    assert_eq!(s.ticks.len(), 3);
    assert_eq!(s.ticks[1].held, Buttons::RIGHT);
    assert_eq!(s.header, Default::default(), "plain comments are no keys");
    assert!(s.directives().is_empty());
    assert_eq!(s, InputScript::from_held(&[RIGHT, RIGHT, 0]));
    // Without a column line, too.
    assert_eq!(parse("0,0,0\n1,1,1\n").ticks.len(), 2);
}

#[test]
fn header_keys_are_read_in_every_spelling() {
    let s = parse(
        "# level 3\n# tuning original\n# seed 4294967295\ntick,held_mask,pressed_mask\n0,0,0\n",
    );
    assert_eq!(s.header.level, Some(3));
    assert_eq!(s.header.tuning, Some(ScriptTuning::Original));
    assert_eq!(s.header.seed, Some(u32::MAX));
    for text in [
        "# level = 7\n",
        "#level=7\n",
        "#  level =7\n",
        "# level= 7\n",
    ] {
        assert_eq!(parse(text).header.level, Some(7), "{text:?}");
    }
    assert_eq!(
        parse("# tuning default\n").header.tuning,
        Some(ScriptTuning::Default)
    );
    // Keys are whole words: these are comments.
    let s = parse("# levels 3\n# seeded by hand\n# Level 3\n");
    assert_eq!(s.header, Default::default());
}

#[test]
fn bad_header_keys_are_errors() {
    assert_eq!(parse_err("# level x\n").0, 1);
    assert!(
        parse_err("# tuning fast\n")
            .1
            .contains("default or original")
    );
    assert!(parse_err("# seed -1\n").1.contains("bad seed"));
    assert!(parse_err("# seed 1 2\n").1.contains("exactly one value"));
    assert!(parse_err("# level\n").1.contains("exactly one value"));
    let (line, msg) = parse_err("# level 1\ntick,held_mask,pressed_mask\n# level 1\n");
    assert_eq!(line, 3);
    assert!(msg.contains("twice"), "{msg}");
}

#[test]
fn the_tuning_key_names_the_tuning() {
    assert_eq!(ScriptTuning::Original.tuning(), Tuning::original_speeds());
    assert_eq!(ScriptTuning::Default.tuning(), Tuning::default());
    for t in [ScriptTuning::Default, ScriptTuning::Original] {
        assert_eq!(ScriptTuning::from_name(t.name()), Some(t));
    }
    assert_eq!(ScriptTuning::from_name("Original"), None);
}

#[test]
fn directives_parse_with_their_options() {
    let s = parse(
        "tick,held_mask,pressed_mask\n0,0,0\n\
         @1 pause 1500\n\
         @1 pause 250 held=0x1\n\
         1,0,0\n\
         @2 menu help 4000\n\
         @2 menu ingame 900 keys=down,down,enter\n\
         @2 menu pause 18\n\
         @2 command end_life\n",
    );
    assert_eq!(s.ticks.len(), 2);
    let kinds: Vec<(usize, DirectiveKind)> = s
        .directives()
        .iter()
        .map(|d| (d.tick, d.kind.clone()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (
                1,
                DirectiveKind::Pause {
                    wall_ms: 1500,
                    held: None
                }
            ),
            (
                1,
                DirectiveKind::Pause {
                    wall_ms: 250,
                    held: Some(JUMP)
                }
            ),
            (
                2,
                DirectiveKind::Menu {
                    screen: MenuScreen::Help,
                    wall_ms: 4000,
                    keys: vec![]
                }
            ),
            (
                2,
                DirectiveKind::Menu {
                    screen: MenuScreen::InGame,
                    wall_ms: 900,
                    keys: vec!["down".into(), "down".into(), "enter".into()]
                }
            ),
            (
                2,
                DirectiveKind::Menu {
                    screen: MenuScreen::Pause,
                    wall_ms: 18,
                    keys: vec![]
                }
            ),
            (2, DirectiveKind::Command(ScriptCommand::EndLife)),
        ]
    );
}

#[test]
fn directives_are_kept_in_tick_order() {
    // Written out of order, read in tick order; one tick keeps its order.
    let s = parse("0,0,0\n1,0,0\n@2 command end_life\n@0 pause 5\n@2 pause 7\n@0 pause 6\n");
    let order: Vec<String> = s.directives().iter().map(Directive::to_line).collect();
    assert_eq!(
        order,
        [
            "@0 pause 5",
            "@0 pause 6",
            "@2 command end_life",
            "@2 pause 7"
        ]
    );
}

#[test]
fn directive_errors_name_the_line() {
    let cases: [(&str, &str); 14] = [
        ("@1 jump 5", "unknown directive kind"),
        ("@x pause 5", "bad directive tick"),
        ("@ 1 pause 5", "bad directive tick"),
        ("@1", "no kind"),
        ("@1 pause", "needs a wall time"),
        ("@1 pause -4", "bad wall time"),
        ("@1 pause 5 held=zz", "bad held mask"),
        ("@1 pause 5 held=1 held=2", "unexpected"),
        ("@1 pause 5 keys=a", "unexpected"),
        ("@1 menu shop 5", "unknown menu screen"),
        ("@1 menu help", "needs a wall time"),
        ("@1 menu help 5 keys=Down", "bad key name"),
        ("@1 command quit", "unknown command"),
        ("@1 command end_life now", "unexpected"),
    ];
    for (line, want) in cases {
        let text = format!("tick,held_mask,pressed_mask\n0,0,0\n1,0,0\n{line}\n");
        let (n, msg) = parse_err(&text);
        assert_eq!(n, 4, "{line}: {msg}");
        assert!(msg.contains(want), "{line}: {msg}");
    }
    let (n, msg) = parse_err("tick,held_mask,pressed_mask\n@3 pause 5\n0,0,0\n1,0,0\n");
    assert_eq!(n, 2);
    assert!(msg.contains("past the last row"), "{msg}");
    // A directive right after the last row is fine.
    assert_eq!(
        parse("0,0,0\n@1 command end_life\n").directives()[0].tick,
        1
    );
}

#[test]
fn scripts_round_trip_through_text() {
    let mut s = InputScript::from_held(&[0, RIGHT, RIGHT, RIGHT | JUMP, 0]);
    s.header.level = Some(1);
    s.header.tuning = Some(ScriptTuning::Original);
    s.header.seed = Some(12_345);
    s.directives = vec![
        Directive {
            tick: 0,
            kind: DirectiveKind::Pause {
                wall_ms: 100,
                held: Some(RIGHT),
            },
        },
        Directive {
            tick: 3,
            kind: DirectiveKind::Menu {
                screen: MenuScreen::InGame,
                wall_ms: 2000,
                keys: vec!["down".into(), "enter".into()],
            },
        },
        Directive {
            tick: 3,
            kind: DirectiveKind::Command(ScriptCommand::EndLife),
        },
        Directive {
            tick: 5,
            kind: DirectiveKind::Pause {
                wall_ms: 9,
                held: None,
            },
        },
    ];
    let text = s.to_csv();
    assert!(
        text.starts_with(
            "# level 1\n# tuning original\n# seed 12345\ntick,held_mask,pressed_mask\n"
        ),
        "{text}"
    );
    assert!(text.contains("@0 pause 100 held=0x2000000\n0,"), "{text}");
    assert!(
        text.contains("@3 menu ingame 2000 keys=down,enter\n@3 command end_life\n3,"),
        "{text}"
    );
    assert!(text.ends_with("4,0x0,0x0\n@5 pause 9\n"), "{text}");
    assert_eq!(parse(&text), s);
    assert_eq!(parse(&text).to_csv(), text);
    // A plain script writes no header keys.
    let plain = InputScript::from_held(&[1, 0]);
    assert!(plain.to_csv().starts_with("tick,"));
    assert_eq!(parse(&plain.to_csv()), plain);
}

#[test]
fn spacing_warnings_find_changes_too_close_together() {
    // Changes at ticks 2 (press), 3 (release, too soon), 6 (press) and 8.
    let s = InputScript::from_held(&[0, 0, RIGHT, 0, 0, 0, JUMP, JUMP, 0]);
    assert_eq!(
        s.spacing_warnings(2),
        vec![SpacingWarning { tick: 3, gap: 1 }]
    );
    assert_eq!(
        s.spacing_warnings(3),
        vec![
            SpacingWarning { tick: 3, gap: 1 },
            SpacingWarning { tick: 8, gap: 2 }
        ]
    );
    assert!(s.spacing_warnings(1).is_empty());
    // The first change is measured from nothing: no warning at tick 0.
    assert!(
        InputScript::from_held(&[RIGHT, RIGHT, 0])
            .spacing_warnings(2)
            .is_empty()
    );
    // Changing from one held set to another counts as one change.
    let s = InputScript::from_held(&[RIGHT, JUMP, JUMP]);
    assert_eq!(
        s.spacing_warnings(2),
        vec![SpacingWarning { tick: 1, gap: 1 }]
    );
}

// ------------------------------------------------------- play-test aids

#[test]
fn the_aids_round_trip_through_the_header() {
    use claw_sim::tuning::PlayAids;
    let text = "# level 2\n# invincible = on\n# infinite_lives=on\n\
                tick,held_mask,pressed_mask\n0,0,0\n1,0x1,0x1\n";
    let s = InputScript::parse_with_aids(text).unwrap();
    assert_eq!(
        s.header.aids,
        PlayAids {
            invincible: true,
            infinite_lives: true
        }
    );
    assert_eq!(s.header.level, Some(2));
    let csv = s.to_csv();
    assert!(csv.contains("# invincible = on\n"), "{csv}");
    assert!(csv.contains("# infinite_lives = on\n"), "{csv}");
    assert_eq!(InputScript::parse_with_aids(&csv).unwrap(), s);
    // One aid alone, and an aid said to be off.
    let one =
        InputScript::parse_with_aids("# infinite_lives on\n# invincible = off\n0,0,0\n").unwrap();
    assert_eq!(
        one.header.aids,
        PlayAids {
            invincible: false,
            infinite_lives: true
        }
    );
    let csv = one.to_csv();
    assert!(!csv.contains("invincible"), "only aids that are on: {csv}");
    assert_eq!(InputScript::parse_with_aids(&csv).unwrap(), one);
}

#[test]
fn a_script_without_aids_writes_no_aid_line() {
    let s = parse("# level 1\n# seed 7\ntick,held_mask,pressed_mask\n0,0,0\n");
    let csv = s.to_csv();
    assert!(!csv.contains("invincible") && !csv.contains("infinite_lives"));
    assert_eq!(
        csv,
        "# level 1\n# seed 7\ntick,held_mask,pressed_mask\n0,0x0,0x0\n"
    );
    // An aid said to be off is plain play.
    let off = parse("# invincible = off\n# infinite_lives = off\n0,0,0\n");
    assert!(!off.header.aids.any());
    assert_eq!(off, InputScript::parse_with_aids("0,0,0\n").unwrap());
}

#[test]
fn routes_refuse_a_script_played_with_an_aid() {
    for (text, line, name) in [
        (
            "# level 1\n# invincible = on\n0,0,0\n",
            2,
            "invincible = on",
        ),
        ("0,0,0\n# infinite_lives = on\n", 2, "infinite_lives = on"),
        (
            "# invincible = on\n# infinite_lives = on\n0,0,0\n",
            1,
            "invincible = on, infinite_lives = on",
        ),
    ] {
        let (l, message) = parse_err(text);
        assert_eq!(l, line, "{text}");
        assert!(message.contains("play-test aid"), "{message}");
        assert!(message.contains(name), "{message}");
        assert!(
            InputScript::parse_with_aids(text).is_ok(),
            "a replay reads it: {text}"
        );
    }
}

#[test]
fn aid_keys_need_on_or_off_once() {
    for text in [
        "# invincible\n0,0,0\n",
        "# invincible = yes\n0,0,0\n",
        "# infinite_lives = on off\n0,0,0\n",
        "# invincible = off\n# invincible = on\n0,0,0\n",
    ] {
        assert!(InputScript::parse_with_aids(text).is_err(), "{text}");
    }
}
