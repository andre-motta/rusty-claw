//! The second and fourth levels' boss stages on your own install: the
//! door and the stager bind, the door leads into the stager's box, the box
//! has the stated reach, and walking (or, on the fourth level, jumping) in
//! through the door runs the introduction, on the fourth level step for
//! step as in the original at its 18 ms step.
//! Needs `CLAW_INSTALL`; skipped otherwise.

mod common;

use std::path::PathBuf;

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::claw_frame::Buttons;
use claw_sim::claw_frame::output::BossMeter;
use claw_sim::claw_frame::{AudioCue, GameEvent, MusicTrack};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::GameLogic;
use claw_sim::logic::boss_stage::{self, stage_state};
use claw_sim::logic::enemy::{EnemyState, katherine};
use claw_sim::logic::world_boss::fight;
use claw_sim::object::{LogicKind, ObjectId, types};
use claw_sim::physics::ptype;
use claw_sim::player::state;
use claw_sim::rng::Rng;
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

// ----------------------------------------------------------------------
// The fourth level

/// Where the fourth level's boss routes start: under its door, which hangs
/// 123 px above the floor, so Claw jumps into it.
const START4: (i32, i32) = (16107, 1500);
/// Game time from the level's start to the routes' time zero, ms.
const T0_MS: u64 = 828;
/// The random state at the routes' time zero.
const SEED4: u32 = 111_567;
/// The fourth level's door, its target, and its stager.
const DOOR4: (i32, i32) = (16107, 1380);
const TARGET4: (i32, i32) = (18429, 1759);
const STAGER4: (i32, i32) = (18429, 1730);
/// Katherine on her floor.
const KATHERINE: (i32, i32) = (18586, 1769);

fn level4() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(4)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

/// The fourth level at its boss routes' time zero with `tuning`: Claw put
/// under the door on the level's first tick, left there until 828 ms of
/// game time have passed, then the random stream set as the routes have
/// it.
fn at_the_door4(level: SimLevel, tuning: ScriptTuning) -> Sim {
    let mut sim = Sim::new(
        level,
        SimConfig {
            tuning: tuning.tuning(),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(START4.0, START4.1);
    idle(&mut sim, T0_MS.div_ceil(18) as usize);
    sim.with_world(|w| *w.rng = Rng::new(SEED4));
    assert_eq!(
        sim.player().map(|o| (o.x, o.y, o.state)),
        Some((START4.0, START4.1 + 3, state::STAND))
    );
    sim
}

/// The routes' way in: still for 15 ticks, Jump held for 6 from the 15th
/// (pressed on it), then still.
fn jump_into_the_door(t: u64) -> TickInput {
    match t {
        15 => TickInput {
            held: Buttons(JUMP),
            pressed: Buttons(JUMP),
        },
        16..=20 => TickInput {
            held: Buttons(JUMP),
            pressed: Buttons::NONE,
        },
        _ => TickInput::default(),
    }
}

#[test]
fn install_level4_door_and_stager_records_bind_where_stated() {
    let Some(level) = level4() else { return };
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
    assert_eq!((door.record[5], door.record[6]), DOOR4);
    assert_eq!((door.record[55], door.record[56]), TARGET4);
    let stager = records
        .iter()
        .find(|o| o.logic == boss_stage::STAGER)
        .unwrap();
    assert_eq!(
        (stager.record[5], stager.record[6], stager.record[7]),
        (STAGER4.0, STAGER4.1, 1000),
        "below Claw's depth: it runs before him"
    );
    assert!((STAGER4.0 - 31..=STAGER4.0 + 38).contains(&TARGET4.0));
    assert!((STAGER4.1 - 68..=STAGER4.1 + 61).contains(&TARGET4.1));
}

/// The fourth level's introduction at the original's 18 ms step, from the
/// routes' time zero: the step of each event, as the original shows it on
/// the same route (the first step in her clearing is step 23).
#[test]
fn install_level4_the_introduction_runs_step_for_step_as_in_the_original() {
    let Some(level) = level4() else { return };
    let mut sim = at_the_door4(level, ScriptTuning::Original);
    let mut stager_states: Vec<(u64, i32)> = Vec::new();
    let mut seen: std::collections::BTreeMap<&str, u64> = Default::default();
    let note = |seen: &mut std::collections::BTreeMap<&str, u64>, k: &'static str, t: u64| {
        seen.entry(k).or_insert(t);
    };
    let mut her = None;
    for t in 0..620u64 {
        sim.step(jump_into_the_door(t));
        let c = sim.player().unwrap().clone();
        if let Some(at) = checkpoint_in(sim.last_events()) {
            assert_eq!(at, TARGET4, "the checkpoint is where Claw stands");
            note(&mut seen, "trigger", t);
        }
        if boss_music(&sim) {
            note(&mut seen, "music", t);
        }
        if let Some((_, o)) = sim
            .objects()
            .all()
            .find(|(_, o)| &*o.logic == boss_stage::STAGER)
        {
            if stager_states.last().map(|&(_, s)| s) != Some(o.state) {
                stager_states.push((t, o.state));
            }
        } else {
            note(&mut seen, "stager gone", t);
        }
        if her.is_none() {
            her = sim.boss().current();
        }
        let her_state = her
            .and_then(|h| sim.objects().get(h))
            .and_then(|o| EnemyState::from_id(o.state));
        match sim.boss().fight() {
            fight::STAGING => note(&mut seen, "flag 1", t),
            fight::ON => note(&mut seen, "flag 2", t),
            _ => {}
        }
        if c.state == state::FREEZE {
            note(&mut seen, "frozen", t);
        }
        if seen.contains_key("frozen") && c.state == state::STAND {
            note(&mut seen, "thaw", t);
        }
        if sim.camera().current.0 == KATHERINE.0 {
            note(&mut seen, "camera on her", t);
        }
        if seen.contains_key("flag 1") && sim.camera_override().is_none() {
            note(&mut seen, "camera back", t);
        }
        if sim
            .last_audio()
            .iter()
            .any(|a| matches!(a, AudioCue::Play(s) if s.key == boss_stage::ENEMY_LINE))
        {
            note(&mut seen, "her line", t);
        }
        if sim
            .last_audio()
            .iter()
            .any(|a| matches!(a, AudioCue::Play(s) if s.key == boss_stage::CLAW_LINE))
        {
            note(&mut seen, "Claw's line", t);
        }
        match her_state {
            Some(EnemyState::Idle) => note(&mut seen, "her idle", t),
            Some(EnemyState::Home) => note(&mut seen, "her home", t),
            Some(EnemyState::Strike) => note(&mut seen, "her whip", t),
            _ => {}
        }
        if sim.claw().stats.health < 100 {
            note(&mut seen, "Claw hit", t);
        }
        if t == 24 {
            assert_eq!(c.state, state::FREEZE);
            assert_eq!((c.x, c.y), TARGET4, "frozen without moving");
            assert_eq!(
                sim.hud().boss_meter,
                Some(BossMeter {
                    value: 100,
                    max: katherine::METER_MAX
                })
            );
            assert_eq!(sim.camera_override(), Some(KATHERINE));
            let o = sim.objects().get(her.expect("built")).unwrap();
            assert_eq!((o.x, o.y), KATHERINE);
            assert_eq!(her_state, Some(EnemyState::Startup));
        }
        if seen.contains_key("Claw hit") {
            break;
        }
    }
    println!("introduction steps: {seen:?}");
    println!("stager states by step: {stager_states:?}");
    let want: [(&str, u64); 15] = [
        ("trigger", 23),
        ("music", 23),
        ("flag 1", 24),
        ("frozen", 24),
        ("camera on her", 46),
        ("her line", 77),
        ("camera back", 132),
        ("Claw's line", 180),
        ("stager gone", 569),
        ("flag 2", 569),
        ("thaw", 570),
        ("her idle", 570),
        ("her home", 571),
        ("her whip", 580),
        ("Claw hit", 594),
    ];
    for (k, step) in want {
        assert_eq!(seen.get(k), Some(&step), "{k}");
    }
    // The stager's states after each step: 8006 from the step after the
    // trigger, then 8008, 8005, 8007, the banner's 8000, 8001 and 8002,
    // 8004, and gone.
    let after_trigger: Vec<(u64, i32)> = stager_states
        .iter()
        .copied()
        .filter(|&(t, _)| t > 23)
        .collect();
    assert_eq!(
        after_trigger,
        [
            (24, stage_state::BOSS_LINE),
            (77, stage_state::CAMERA_BACK),
            (132, stage_state::CLAW_LINE),
            (180, stage_state::AFTER_CLAW_LINE),
            (282, stage_state::BANNER),
            (294, stage_state::BANNER_DROP),
            (421, stage_state::BANNER_LEAVE),
            (568, stage_state::FIGHT),
        ]
    );
}
