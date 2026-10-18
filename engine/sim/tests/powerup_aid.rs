//! The powerup play-test aid on synthetic levels: a start powerup is given
//! at the level's start and after every respawn, a powerup given with
//! [`Sim::give_powerup`] starts at the next tick boundary as a pickup's
//! does, and scripts naming either are refused as routes but read for
//! replays.

mod common;

use claw_frame::{AudioCue, MusicTrack, TickInput};
use claw_sim::input::{DirectiveKind, ScriptCommand};
use claw_sim::player::death::{CATNIP_GLITTER_LOGIC, powerup};
use claw_sim::player::powerup_aid::{AID_DEFAULT_MS, AID_POWERUPS, AidPowerup};
use claw_sim::player::state;
use claw_sim::{InputScript, Sim, SimConfig};
use common::*;

fn sim_with(start: Option<AidPowerup>) -> Sim {
    pressing_sim(
        flat_level(),
        SimConfig {
            start_powerup: start,
            ..SimConfig::default()
        },
    )
}

fn aid(text: &str) -> AidPowerup {
    AidPowerup::parse(text).unwrap()
}

/// The running powerup and its time left.
fn powerup_now(sim: &Sim) -> (u32, u32) {
    (sim.stats().powerup_kind, sim.stats().powerup_ms)
}

/// The music switches of the last tick (or between-ticks call).
fn music(sim: &Sim) -> Vec<MusicTrack> {
    sim.last_audio()
        .iter()
        .filter_map(|c| match c {
            AudioCue::Music { track, .. } => Some(*track),
            _ => None,
        })
        .collect()
}

fn glitter(sim: &Sim) -> usize {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == CATNIP_GLITTER_LOGIC)
        .count()
}

/// Steps idle until Claw's death has been counted and the respawn waits.
fn die(sim: &mut Sim) {
    sim.end_life();
    for _ in 0..300 {
        sim.step(TickInput::default());
        if sim.respawn_pending() {
            return;
        }
    }
    panic!("Claw did not die");
}

#[test]
fn a_start_powerup_is_given_at_the_level_start() {
    let p = aid("fire:45");
    let mut sim = sim_with(Some(p));
    // Not before time zero: at the boundary before the first tick.
    assert_eq!(powerup_now(&sim), (0, 0));
    assert_eq!(sim.powerup_pending(), Some(p));
    sim.step(TickInput::default());
    assert_eq!(sim.powerup_pending(), None);
    let (kind, ms) = powerup_now(&sim);
    assert_eq!(kind, powerup::FIRE_SWORD);
    assert!((45_000 - 18..=45_000).contains(&ms), "{ms}");
    assert_eq!(music(&sim), [MusicTrack::Powerup]);
}

#[test]
fn without_a_start_powerup_nothing_is_given() {
    let mut sim = sim_with(None);
    assert_eq!(sim.powerup_pending(), None);
    idle(&mut sim, 40);
    assert_eq!(powerup_now(&sim), (0, 0));
}

#[test]
fn the_start_powerup_comes_back_after_every_respawn() {
    let mut sim = sim_with(Some(aid("invisible")));
    idle(&mut sim, 40);
    assert_eq!(sim.stats().powerup_kind, powerup::GHOST);
    for n in 0..2 {
        die(&mut sim);
        // The death ended it.
        assert_eq!(powerup_now(&sim), (0, 0), "death {n}");
        assert!(sim.respawn_now(), "death {n}");
        // Given again with the respawn, between the two ticks (the
        // respawn's update passes count it down as any tick does).
        let (kind, ms) = powerup_now(&sim);
        assert_eq!(kind, powerup::GHOST, "death {n}");
        assert!(
            (AID_DEFAULT_MS - 2 * 18..=AID_DEFAULT_MS).contains(&ms),
            "death {n}: {ms}"
        );
        assert!(music(&sim).ends_with(&[MusicTrack::Powerup]), "death {n}");
        sim.step(TickInput::default());
        assert_eq!(sim.player().unwrap().state, state::STAND, "death {n}");
        assert_eq!(sim.stats().powerup_kind, powerup::GHOST, "death {n}");
    }
}

#[test]
fn a_given_powerup_starts_at_the_next_tick_boundary() {
    let mut sim = sim_with(None);
    idle(&mut sim, 40);
    sim.give_powerup(aid("catnip"));
    assert_eq!(powerup_now(&sim), (0, 0));
    sim.step(TickInput::default());
    assert_eq!(sim.stats().powerup_kind, powerup::CATNIP);
    assert_eq!(music(&sim), [MusicTrack::Powerup]);
    assert_eq!(glitter(&sim), 1, "catnip's sparkles trail Claw");
    // The last call before a tick wins.
    sim.give_powerup(aid("fire"));
    sim.give_powerup(aid("ice:5"));
    sim.step(TickInput::default());
    assert_eq!(sim.stats().powerup_kind, powerup::ICE_SWORD);
    assert!(sim.stats().powerup_ms <= 5000);
    assert_eq!(glitter(&sim), 0, "a new powerup ends catnip's sparkles");
    // None ends the running one and brings the level music back.
    sim.give_powerup(AidPowerup::NONE);
    sim.step(TickInput::default());
    assert_eq!(powerup_now(&sim), (0, 0));
    assert_eq!(music(&sim), [MusicTrack::Level]);
}

#[test]
fn every_aid_powerup_starts_as_a_pickup_starts_it() {
    for (name, kind, _) in AID_POWERUPS {
        let mut sim = sim_with(None);
        idle(&mut sim, 40);
        sim.give_powerup(aid(name));
        sim.step(TickInput::default());
        let (k, ms) = powerup_now(&sim);
        assert_eq!(k, kind, "{name}");
        assert!(ms > AID_DEFAULT_MS - 20, "{name}: {ms}");
        assert_eq!(music(&sim), [MusicTrack::Powerup], "{name}");
        // It runs out as a pickup's does.
        idle(&mut sim, (AID_DEFAULT_MS / 18 + 2) as usize);
        assert_eq!(powerup_now(&sim), (0, 0), "{name}");
    }
}

#[test]
fn a_dying_claw_takes_no_powerup() {
    let mut sim = sim_with(None);
    idle(&mut sim, 40);
    sim.end_life();
    sim.step(TickInput::default());
    assert!(sim.stats().health <= 0);
    sim.give_powerup(aid("fire"));
    sim.step(TickInput::default());
    assert_eq!(powerup_now(&sim), (0, 0));
}

const ROWS: &str = "tick,held_mask,pressed_mask\n0,0,0\n1,0,0\n2,0,0\n";

#[test]
fn a_script_with_the_powerup_aid_replays_but_is_never_a_route() {
    // The start powerup.
    let text = format!("# level 1\n# powerup = fire:30\n{ROWS}");
    let e = InputScript::parse(&text).unwrap_err();
    assert_eq!(e.line, 2);
    assert!(e.message.contains("play-test aid"), "{e}");
    assert!(e.message.contains("powerup = fire:30"), "{e}");
    let s = InputScript::parse_with_aids(&text).unwrap();
    assert_eq!(s.header.powerup, Some(aid("fire")));
    assert!(s.header.any_aid());
    // The play-test key.
    let text = format!("# powerup_key = used\n{ROWS}");
    let e = InputScript::parse(&text).unwrap_err();
    assert!(e.message.contains("powerup_key = used"), "{e}");
    assert!(
        InputScript::parse_with_aids(&text)
            .unwrap()
            .header
            .powerup_key
    );
    // A powerup command alone is the key's.
    let text = format!("{ROWS}@2 command powerup ice:30\n@3 command powerup none\n");
    let e = InputScript::parse(&text).unwrap_err();
    assert_eq!(e.line, 5);
    assert!(e.message.contains("powerup_key = used"), "{e}");
    let s = InputScript::parse_with_aids(&text).unwrap();
    let kinds: Vec<_> = s
        .directives
        .iter()
        .map(|d| (d.tick, d.kind.clone()))
        .collect();
    assert_eq!(
        kinds,
        [
            (
                2,
                DirectiveKind::Command(ScriptCommand::Powerup(aid("ice")))
            ),
            (
                3,
                DirectiveKind::Command(ScriptCommand::Powerup(AidPowerup::NONE))
            ),
        ]
    );
    // Off spelled out is plain play.
    let text = format!("# powerup = none\n# powerup_key = unused\n{ROWS}");
    let s = InputScript::parse(&text).unwrap();
    assert!(!s.header.any_aid());
}

#[test]
fn the_powerup_aid_writes_back_as_it_was_read() {
    let text = format!(
        "# level 2\n# invincible = on\n# powerup = catnip:90\n# powerup_key = used\n{ROWS}\
         @1 command powerup lightning:30\n"
    );
    let s = InputScript::parse_with_aids(&text).unwrap();
    let out = s.to_csv();
    for line in [
        "# invincible = on",
        "# powerup = catnip:90",
        "# powerup_key = used",
        "@1 command powerup lightning:30",
    ] {
        assert!(out.contains(line), "{line} in\n{out}");
    }
    assert_eq!(InputScript::parse_with_aids(&out).unwrap(), s);
    assert_eq!(
        s.header.aid_lines(),
        [
            "invincible = on",
            "powerup = catnip:90",
            "powerup_key = used"
        ]
    );
}

#[test]
fn bad_powerup_lines_are_errors() {
    for bad in [
        "# powerup = sword\n",
        "# powerup = fire:0\n",
        "# powerup\n",
        "# powerup_key = maybe\n",
        "# powerup = fire\n# powerup = ice\n",
        "# powerup_key = used\n# powerup_key = used\n",
    ] {
        let text = format!("{bad}{ROWS}");
        assert!(InputScript::parse_with_aids(&text).is_err(), "{bad}");
    }
    for bad in [
        "@1 command powerup\n",
        "@1 command powerup fire ice\n",
        "@1 command powerup sword\n",
        "@1 command end_life now\n",
    ] {
        let text = format!("{ROWS}{bad}");
        assert!(InputScript::parse_with_aids(&text).is_err(), "{bad}");
    }
}
