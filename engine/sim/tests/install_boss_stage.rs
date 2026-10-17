//! The second level's boss stage on your own install: the door and the
//! stager bind, the door leads into the stager's box, the box has the
//! stated reach, and walking in through the door runs the introduction.
//! Needs `CLAW_INSTALL`; skipped otherwise.

mod common;

use std::path::PathBuf;

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::claw_frame::Buttons;
use claw_sim::claw_frame::output::BossMeter;
use claw_sim::claw_frame::{AudioCue, GameEvent, MusicTrack};
use claw_sim::logic::GameLogic;
use claw_sim::logic::boss_stage::{self, stage_state};
use claw_sim::logic::world_boss::fight;
use claw_sim::object::{LogicKind, ObjectId, types};
use claw_sim::physics::ptype;
use claw_sim::player::state;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// Where the stretch of play starts: next to the door, as the routes that
/// compare the boss entry do.
const START: (i32, i32) = (20150, 2091);
/// The door's record and its target.
const DOOR: (i32, i32) = (20298, 2112);
const TARGET: (i32, i32) = (23646, 3947);
/// The stager's record.
const STAGER: (i32, i32) = (23642, 3994);

fn level2() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(2)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

fn find(sim: &Sim, logic: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("LEVEL2 places {logic}"))
}

/// Moves Claw to (x, y) and lets him and the camera settle, as the
/// comparison routes' start does.
fn settle_at(sim: &mut Sim, x: i32, y: i32) {
    sim.teleport_player(x, y);
    let mut still = 0;
    let mut last = None;
    for _ in 0..400 {
        sim.step(TickInput::default());
        let now = sim
            .player()
            .map(|o| (o.x, o.y, o.state, sim.camera().current));
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still >= 10 {
            break;
        }
    }
}

fn checkpoint_in(events: &[GameEvent]) -> Option<(i32, i32)> {
    events.iter().find_map(|e| match *e {
        GameEvent::Checkpoint { x, y } => Some((x, y)),
        _ => None,
    })
}

#[test]
fn install_level2_door_and_stager_records_bind_where_stated() {
    let Some(level) = level2() else { return };
    let records: Vec<_> = level
        .objects
        .iter()
        .filter(|o| boss_stage::NAMES.contains(&o.logic.as_str()))
        .collect();
    assert_eq!(records.len(), 2, "one door and one stager");
    let door = records
        .iter()
        .find(|o| o.logic == boss_stage::WARP)
        .unwrap();
    assert_eq!((door.record[5], door.record[6]), DOOR);
    assert_eq!((door.record[55], door.record[56]), TARGET);
    let stager = records
        .iter()
        .find(|o| o.logic == boss_stage::STAGER)
        .unwrap();
    assert_eq!(
        (stager.record[5], stager.record[6], stager.record[7]),
        (STAGER.0, STAGER.1, 8000)
    );
    // The target lies inside the box.
    assert!((STAGER.0 - 31..=STAGER.0 + 38).contains(&TARGET.0));
    assert!((STAGER.1 - 68..=STAGER.1 + 61).contains(&TARGET.1));

    let mut sim = pressing_sim(level, SimConfig::default());
    sim.step(TickInput::default());
    let d = sim.diagnostics();
    for name in [boss_stage::WARP, boss_stage::STAGER] {
        assert_eq!(d.inert_logics.get(name), None, "{name} runs");
    }
    settle_at(&mut sim, START.0, START.1);
    let door = find(&sim, boss_stage::WARP);
    let o = sim.objects().get(door).unwrap();
    assert_eq!(o.kind, LogicKind::Game(GameLogic::BossStage));
    assert_eq!(o.state, stage_state::ARMED);
    assert_eq!(o.type_flags, types::SPECIAL);
    assert_eq!(o.image_set, boss_stage::WARP_IMAGE);
    assert!(o.attack_rect.is_set());
}

#[test]
fn install_level2_stager_box_reaches_31_left_38_right() {
    let Some(level) = level2() else { return };
    let mut sim = pressing_sim(level, SimConfig::default());
    // Settle on the arena floor right of the box (left of it is a drop),
    // facing right.
    settle_at(&mut sim, STAGER.0 + 120, TARGET.1);
    let y = sim.player().unwrap().y;
    assert_eq!(y, TARGET.1, "the arena floor");
    let stager = find(&sim, boss_stage::STAGER);
    assert_eq!(sim.objects().get(stager).unwrap().state, stage_state::ARMED);
    assert!(sim.claw().facing_right);
    for (x, inside) in [
        (STAGER.0 - 32, false),
        (STAGER.0 - 31, true),
        (STAGER.0 + 38, true),
        (STAGER.0 + 39, false),
    ] {
        let mut s = sim.clone();
        s.teleport_player(x, y);
        s.step(TickInput::default());
        assert_eq!(s.player().unwrap().physics_type, ptype::STAND);
        let fired = checkpoint_in(s.last_events());
        assert_eq!(fired.is_some(), inside, "x {x}");
        if inside {
            assert_eq!(fired, Some((x, y)));
            s.step(TickInput::default());
            assert_eq!(s.boss().fight(), fight::STAGING);
            assert_eq!(s.player().unwrap().state, state::FREEZE);
        }
    }
}

/// The door's tick: Claw walks right from [`START`] (15 empty ticks, then
/// Right held, as the reference route does) until he touches the door; the
/// sim is left at the end of that tick, with Claw still at the door: the
/// door takes him to its target between that tick and the next.
fn walk_to_the_door(sim: &mut Sim) -> u64 {
    settle_at(sim, START.0, START.1);
    idle(sim, 15);
    for k in 15..200u64 {
        let before = sim.clone();
        sim.step(key_down(sim, RIGHT));
        let o = sim.player().unwrap();
        if o.x > DOOR.0 + 1000 {
            // The tick after the door's: at the target, walking on.
            assert!(
                (TARGET.0..=TARGET.0 + 9).contains(&o.x) && o.y == TARGET.1,
                "the door's target: {:?}",
                (o.x, o.y)
            );
            *sim = before;
            let o = sim.player().unwrap();
            assert!((o.x - DOOR.0).abs() < 200, "at the door: {}", o.x);
            return k - 1;
        }
    }
    panic!("the door never moved Claw");
}

fn boss_music(sim: &Sim) -> bool {
    sim.last_audio().iter().any(|c| {
        matches!(
            c,
            AudioCue::Music {
                track: MusicTrack::Boss,
                ..
            }
        )
    })
}

/// Right held, with Jump pressed too.
fn jump_right() -> TickInput {
    TickInput {
        held: Buttons(RIGHT | JUMP),
        pressed: Buttons(JUMP),
    }
}

#[test]
fn install_level2_walking_through_the_door_runs_the_introduction() {
    let Some(level) = level2() else { return };
    let mut sim = Sim::new(level, SimConfig::default());
    let door = walk_to_the_door(&mut sim);
    assert!(!boss_music(&sim), "no boss music before the door's move");
    // The door's move between that tick and the next, which takes no game
    // time: the camera jumps to Claw, the stager wakes there, switches to
    // the boss music and is armed for Claw's first tick at the target.
    // That tick: still walking, inside the box, and the trigger fires in
    // its collision pass.
    sim.step(key_down(&sim, RIGHT));
    assert!(boss_music(&sim), "the boss music with the door's move");
    let o = sim.player().unwrap();
    let at = (o.x, o.y);
    assert!(
        (sim.camera().current.0 - at.0).abs() <= 9,
        "the camera jumped to Claw"
    );
    println!(
        "door at route tick {door}, the trigger at {} {at:?} (the original game: the door on \
         step 40, the arena and the trigger on step 41 at (23650, 3947))",
        door + 1
    );
    assert_eq!(o.physics_type, ptype::WALK);
    assert!((TARGET.0 + 1..=TARGET.0 + 9).contains(&at.0), "{at:?}");
    assert_eq!(at.1, TARGET.1);
    assert_eq!(checkpoint_in(sim.last_events()), Some(at));
    assert_eq!(sim.boss().checkpoint_time(), 120_000);
    assert_eq!(sim.claw().checkpoint, Some(at));
    // The next tick: the flag, the freeze and the camera on the boss's
    // point; the music was asked for once.
    sim.step(key_down(&sim, RIGHT));
    assert!(!boss_music(&sim), "asked for once");
    assert_eq!(sim.boss().fight(), fight::STAGING);
    assert_eq!(sim.player().unwrap().state, state::FREEZE);
    if sim.boss().current().is_none() {
        assert_eq!(sim.hud().boss_meter, None);
    } else {
        assert!(matches!(sim.hud().boss_meter, Some(BossMeter { .. })));
    }
    // It ends with the fight on and Claw standing.
    let mut on = None;
    for k in 0..1200u64 {
        sim.step(TickInput::default());
        if sim.boss().fight() == fight::ON && on.is_none() {
            on = Some(k);
        }
        if on.is_some() && sim.player().unwrap().state == state::STAND {
            break;
        }
    }
    let on = on.expect("the fight started");
    println!("flag 2 {} ticks after the freeze", on + 1);
    assert_eq!(sim.player().unwrap().state, state::STAND);
}

#[test]
fn install_level2_jump_on_the_first_arena_tick_skips_the_introduction() {
    let Some(level) = level2() else { return };
    let mut at_door = Sim::new(level, SimConfig::default());
    let door = walk_to_the_door(&mut at_door);

    // Jump pressed on the first tick at the target: Claw leaves the box in
    // the air, and nothing of the introduction runs.
    let mut sim = at_door.clone();
    sim.step(jump_right());
    assert_eq!(sim.player().unwrap().physics_type, ptype::JUMP);
    let mut landed = None;
    for k in 0..300u64 {
        assert_eq!(
            checkpoint_in(sim.last_events()),
            None,
            "tick {}",
            door + 1 + k
        );
        if landed.is_none() && sim.player().unwrap().physics_type == ptype::WALK {
            landed = Some(sim.player().unwrap().x);
        }
        sim.step(key_down(&sim, RIGHT));
    }
    let landed = landed.expect("Claw landed");
    println!("skip: landed walking at x {landed} (the original game: 23790 at c18)");
    assert!(landed > STAGER.0 + 38, "out of the box");
    assert_eq!(sim.boss().fight(), fight::NONE);
    assert_ne!(sim.player().unwrap().state, state::FREEZE);
    assert_eq!(sim.hud().boss_meter, None);
    assert_eq!(sim.boss().checkpoint_time(), 0);
    let stager = find(&sim, boss_stage::STAGER);
    assert_eq!(
        sim.objects().get(stager).unwrap().state,
        stage_state::ARMED,
        "the trigger stays armed"
    );

    // Pressed one tick later it is too late: the trigger fired on the first
    // tick, and the introduction runs.
    let mut sim = at_door.clone();
    sim.step(key_down(&sim, RIGHT));
    assert!(checkpoint_in(sim.last_events()).is_some());
    sim.step(jump_right());
    assert_eq!(sim.boss().fight(), fight::STAGING);
    assert_eq!(sim.player().unwrap().state, state::FREEZE);
}
