//! The search through the second, sixth and eighth levels' boss rooms on
//! your own install (skipped without `CLAW_INSTALL`): from beside the
//! door, Claw goes through it, sits through the boss's introduction (or
//! skips it with the jump-in), fights Raux or Wolvington down, or lifts
//! Gabriel and throws him into the water, and takes the gem his defeat
//! releases. Nothing from an install is stored in the repository.

use std::path::PathBuf;

use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_bot::search::{BossStage, Intro, boss_end, boss_stage};
use claw_bot::{Goal, SearchConfig, search_report};
use claw_sim::claw_frame::GameEvent;
use claw_sim::logic::world_boss::fight;
use claw_sim::player::powerup_aid::AidPowerup;
use claw_sim::{Buttons, InputScript, Sim, SimLevel};

/// Beside the boss room's door, where the reference runs of the room
/// start.
const DOOR_SIDE: (i32, i32) = (20150, 2091);

/// Under the sixth level's boss room door, where the reference runs of
/// that room start (a jump takes Claw into the door).
const DOOR_UNDER_6: (i32, i32) = (30685, 4175);

/// Right of the eighth level's boss room door, where the reference runs of
/// that room start (a walk left takes Claw into the door).
const DOOR_RIGHT_8: (i32, i32) = (31700, 5200);

/// Further right on the floor beside the eighth level's door, where the
/// search has room to time its jump-in.
const FLOOR_RIGHT_8: (i32, i32) = (31800, 5200);

/// Where the eighth level's dock ends: the water Gabriel is thrown into
/// lies left of it.
const DOCK_END_8: i32 = 42471;

/// The random state those runs set when Claw has settled there.
const SEGMENT_SEED: u32 = 111_567;

fn retail(n: u32) -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(n)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

fn level2() -> Option<SimLevel> {
    retail(2)
}

/// The game settled at `at`, as a segment of the reference runs starts.
fn settled(level: SimLevel, tuning: TuningName, at: (i32, i32)) -> Sim {
    let mut sim = Sim::new(level, route::sim_config(tuning, SEGMENT_SEED));
    route::settle_at(&mut sim, at.0, at.1, SEGMENT_SEED);
    sim
}

/// The second level's game settled beside the door.
fn beside_the_door(level: SimLevel, tuning: TuningName) -> Sim {
    settled(level, tuning, DOOR_SIDE)
}

/// What a route did in the boss room: the fight flag's values in order,
/// the tick of Raux's defeat and the tick the level was won.
fn play(start: &Sim, script: &InputScript) -> (Vec<u8>, Option<usize>, Option<usize>) {
    let mut sim = start.clone();
    let mut flags = vec![sim.boss().fight()];
    let (mut defeated, mut won) = (None, None);
    for (i, &t) in script.ticks.iter().enumerate() {
        sim.step(t);
        if flags.last() != Some(&sim.boss().fight()) {
            flags.push(sim.boss().fight());
        }
        if defeated.is_none() && sim.boss().defeated().is_some() {
            defeated = Some(i);
        }
        if sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
        {
            won = Some(i);
            break;
        }
    }
    (flags, defeated, won)
}

fn cfg(intro: Intro) -> SearchConfig {
    SearchConfig {
        nodes: 20_000,
        threads: 4,
        seed: u64::from(SEGMENT_SEED),
        intro,
        ..SearchConfig::default()
    }
}

#[test]
fn install_level2_the_search_sits_through_the_introduction_and_beats_raux() {
    let Some(level) = level2() else { return };
    let start = beside_the_door(level, TuningName::Original);
    let end = boss_end(&start).expect("the second level ends with a boss");
    assert_eq!(boss_stage(&start), BossStage::Door);
    let found = search_report(&start, Goal::LevelComplete, &cfg(Intro::Play)).expect("a route");
    let (flags, defeated, won) = play(&start, &found.script);
    println!(
        "introduction played: {} ticks, Raux defeated at {defeated:?}, won at {won:?}, {} nodes, door {:?}",
        found.script.ticks.len(),
        found.stats.expanded,
        end.door
    );
    assert_eq!(flags, vec![fight::NONE, fight::STAGING, fight::ON]);
    assert!(defeated.is_some() && won.is_some() && defeated < won);
    assert!(route::play(&start, &found.script, &Goal::LevelComplete).won());
    // The door, the fight and the gem each started a fresh segment.
    assert!(found.stats.segments >= 2, "{:?}", found.stats);
}

#[test]
fn install_level2_the_search_finds_the_jump_in_that_skips_the_introduction() {
    let Some(level) = level2() else { return };
    let start = beside_the_door(level, TuningName::Default);
    let found = search_report(&start, Goal::LevelComplete, &cfg(Intro::Skip)).expect("a route");
    let (flags, defeated, won) = play(&start, &found.script);
    println!(
        "jump-in: {} ticks, Raux defeated at {defeated:?}, won at {won:?}, {} nodes, {} entries dropped",
        found.script.ticks.len(),
        found.stats.expanded,
        found.stats.entries_pruned
    );
    assert_eq!(flags, vec![fight::NONE], "the introduction never starts");
    assert!(defeated.is_some() && won.is_some() && defeated < won);
    assert!(found.stats.entries_pruned > 0, "walking in starts it");
}

#[test]
fn install_level6_the_search_sits_through_the_introduction_and_beats_wolvington() {
    let Some(level) = retail(6) else { return };
    let start = settled(level, TuningName::Original, DOOR_UNDER_6);
    let end = boss_end(&start).expect("the sixth level ends with a boss");
    assert_eq!(end.gem, (34909, 4330));
    let found = search_report(&start, Goal::LevelComplete, &cfg(Intro::Play)).expect("a route");
    let (flags, defeated, won) = play(&start, &found.script);
    println!(
        "introduction played: {} ticks, Wolvington defeated at {defeated:?}, won at {won:?}, {} nodes, {} health lost",
        found.script.ticks.len(),
        found.stats.expanded,
        found.stats.health_lost
    );
    assert_eq!(flags, vec![fight::NONE, fight::STAGING, fight::ON]);
    assert!(defeated.is_some() && won.is_some() && defeated < won);
}

#[test]
fn install_level6_the_search_finds_the_jump_in_and_beats_the_dormant_wolvington() {
    let Some(level) = retail(6) else { return };
    let start = settled(level, TuningName::Original, DOOR_UNDER_6);
    let end = boss_end(&start).expect("the sixth level ends with a boss");
    let found = search_report(&start, Goal::LevelComplete, &cfg(Intro::Skip)).expect("a route");
    let (flags, defeated, won) = play(&start, &found.script);
    println!(
        "jump-in: {} ticks, Wolvington defeated at {defeated:?}, won at {won:?}, {} nodes, {} entries dropped",
        found.script.ticks.len(),
        found.stats.expanded,
        found.stats.entries_pruned
    );
    assert_eq!(flags, vec![fight::NONE], "the introduction never starts");
    assert!(defeated.is_some() && won.is_some() && defeated < won);
    // The jump is pressed on the first tick in the room, toward
    // Wolvington, and held 12 ticks, as the original's players make it.
    let mut sim = start.clone();
    let first = found
        .script
        .ticks
        .iter()
        .position(|&t| {
            sim.step(t);
            sim.player().is_some_and(|o| o.x > end.room.0 - 100)
        })
        .expect("the room");
    let held = |i: usize| found.script.ticks[i].held;
    assert!(found.script.ticks[first].pressed.contains(Buttons::JUMP));
    for i in first..first + 12 {
        assert!(held(i).contains(Buttons::JUMP | Buttons::RIGHT), "tick {i}");
    }
    assert!(!held(first + 12).contains(Buttons::JUMP));
}

#[test]
fn install_level8_gabriel_is_read_with_the_water_beside_his_dock() {
    let Some(level) = retail(8) else { return };
    let start = settled(level, TuningName::Original, DOOR_RIGHT_8);
    let end = boss_end(&start).expect("the eighth level ends with a boss");
    assert_eq!(end.room, (43058, 5212));
    assert_eq!(end.boss, (43223, 5068));
    assert_eq!(end.gem, (43036, 5028));
    assert_eq!(end.water, Some(DOCK_END_8));
}

#[test]
fn install_level8_the_search_lifts_the_dormant_gabriel_and_drowns_him() {
    let Some(level) = retail(8) else { return };
    // With Catnip, as the level's route brings him there, the jump-in
    // lands Claw on Gabriel's deck.
    let mut start = settled(level, TuningName::Original, FLOOR_RIGHT_8);
    start.give_powerup(AidPowerup::parse("catnip:40").unwrap());
    start.step(Default::default());
    let found = search_report(
        &start,
        Goal::LevelComplete,
        &SearchConfig {
            threads: 8,
            ..cfg(Intro::Skip)
        },
    )
    .expect("a route");
    let (flags, defeated, won) = play(&start, &found.script);
    println!(
        "jump-in: {} ticks, Gabriel drowned at {defeated:?}, won at {won:?}, {} nodes, {} health lost",
        found.script.ticks.len(),
        found.stats.expanded,
        found.stats.health_lost
    );
    assert_eq!(flags, vec![fight::NONE], "the introduction never starts");
    assert!(defeated.is_some() && won.is_some() && defeated < won);
}
