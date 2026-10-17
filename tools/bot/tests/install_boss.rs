//! The search through the second level's boss room on your own install
//! (skipped without `CLAW_INSTALL`): from beside the door, Claw goes
//! through it, sits through Raux's introduction (or skips it with the
//! jump-in), fights Raux down and takes the gem his defeat releases.
//! Nothing from an install is stored in the repository.

use std::path::PathBuf;

use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_bot::search::{BossStage, Intro, boss_end, boss_stage};
use claw_bot::{Goal, SearchConfig, search_report};
use claw_sim::claw_frame::GameEvent;
use claw_sim::logic::world_boss::fight;
use claw_sim::{InputScript, Sim, SimLevel};

/// Beside the boss room's door, where the reference runs of the room
/// start.
const DOOR_SIDE: (i32, i32) = (20150, 2091);

/// The random state those runs set when Claw has settled there.
const SEGMENT_SEED: u32 = 111_567;

fn level2() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(2)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

/// The game settled beside the door, as a segment of the reference runs
/// starts.
fn beside_the_door(level: SimLevel, tuning: TuningName) -> Sim {
    let mut sim = Sim::new(level, route::sim_config(tuning, SEGMENT_SEED));
    route::settle_at(&mut sim, DOOR_SIDE.0, DOOR_SIDE.1, SEGMENT_SEED);
    sim
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
