//! The framework the second level's families build on: the families that
//! are declared but not built yet stay inert placeholders, the enemy state
//! numbers keep their blocks, the boss fight state of the world, the
//! camera point a logic sets, the immediate freeze of a boss fight's
//! introduction, the boss columns of traces and the death tile hook.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_frame::output::BossMeter;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, DeathKind, EnemyState};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{self, GameLogic, ObjectData};
use claw_sim::object::{LogicKind, ObjectId, draw_flags};
use claw_sim::player::state;
use claw_sim::registry::Registry;
use claw_sim::trace::{self, TraceRow};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

const ATTACK: u32 = 0x2;

/// The families declared for the second level, with the names they claim.
const DECLARED: [(GameLogic, &[&str]); 4] = [
    (GameLogic::PunkRat, &["PunkRat"]),
    (
        GameLogic::Cannon,
        &["TowerCannonLeft", "TowerCannonRight", "GroundCannon"],
    ),
    (GameLogic::Raux, &["Raux"]),
    (
        GameLogic::BossStage,
        &[
            "BossWarp",
            "BossStager",
            "BossStagerAttack",
            "BossHealthMeter",
        ],
    ),
];

fn claw(sim: &Sim) -> &claw_sim::object::Object {
    sim.player().expect("Claw")
}

/// The flat level with `names` placed beside Claw.
fn level_with(names: &[&str]) -> SimLevel {
    let mut level = flat_level();
    for (i, name) in names.iter().enumerate() {
        level
            .objects
            .push(ObjectSpec::at(name, 1100 + 40 * i as i32, 900, 4000));
    }
    level
}

#[test]
fn every_name_belongs_to_one_family() {
    let mut seen = std::collections::BTreeSet::new();
    for g in logic::ALL {
        for &n in logic::names(g) {
            assert!(seen.insert(n), "{n} is claimed twice");
            assert_eq!(logic::kind_for(n), Some(LogicKind::Game(g)), "{n}");
            assert!(Registry::shared().get(n).is_some(), "{n} is registered");
        }
    }
    for (g, names) in DECLARED {
        assert_eq!(logic::names(g), names, "{g:?}");
    }
}

#[test]
fn declared_families_stay_inert_placeholders_until_built() {
    // The families not built yet (each one leaves this list as it is built).
    let unbuilt = DECLARED.iter().filter(|(g, _)| !logic::built(*g));
    let names: Vec<&str> = unbuilt
        .clone()
        .flat_map(|(_, n)| n.iter().copied())
        .collect();
    for &(g, list) in unbuilt {
        assert!(!logic::built(g), "{g:?} is not built yet");
        for &n in list {
            assert_eq!(logic::runs(n), None, "{n}");
            assert_eq!(Registry::shared().get(n).unwrap().kind, LogicKind::Inert);
            assert_eq!(logic::new_object(n, 0, 0, 0).kind, LogicKind::Inert);
        }
    }
    // The built families run their names.
    assert_eq!(
        logic::runs("Officer"),
        Some(LogicKind::Game(GameLogic::Officer))
    );
    // Placed in a level, they are drawn placeholders that never collide,
    // and the level load reports them as not implemented.
    let mut sim = Sim::new(level_with(&names), SimConfig::default());
    idle(&mut sim, 5);
    let d = sim.diagnostics();
    for n in &names {
        assert_eq!(d.inert_logics.get(*n), Some(&1), "{n} is reported");
    }
    let placed: Vec<_> = sim
        .objects()
        .all()
        .filter(|(_, o)| o.record.is_some())
        .collect();
    assert_eq!(placed.len(), names.len());
    for (_, o) in placed {
        assert_eq!(o.kind, LogicKind::Inert, "{}", o.logic);
        assert_eq!(o.state, 0, "{} never runs", o.logic);
        assert_eq!(o.data, ObjectData::None);
    }
    assert!(sim.collision_events().is_empty());
}

/// Every enemy state with its fixed number and name.
const STATES: [(EnemyState, i32, &str); 103] = [
    (EnemyState::Idle, 2000, "Idle"),
    (EnemyState::Home, 2001, "Home"),
    (EnemyState::Strike, 2002, "Strike"),
    (EnemyState::Pace, 2003, "Pace"),
    (EnemyState::HitLow, 2004, "HitLow"),
    (EnemyState::HitHigh, 2005, "HitHigh"),
    (EnemyState::Dead, 2006, "Dead"),
    (EnemyState::Pause, 2007, "Pause"),
    (EnemyState::Falling, 2008, "Falling"),
    (EnemyState::Advance, 2009, "Advance"),
    (EnemyState::FastAdvance, 2010, "FastAdvance"),
    (EnemyState::JumpAtClaw, 2011, "JumpAtClaw"),
    (EnemyState::Jump, 2012, "Jump"),
    (EnemyState::Fire, 2013, "Fire"),
    (EnemyState::MoveLeft, 2014, "MoveLeft"),
    (EnemyState::MoveRight, 2015, "MoveRight"),
    (EnemyState::Throw, 2016, "Throw"),
    (EnemyState::Lifted, 2017, "Lifted"),
    (EnemyState::Flailing, 2018, "Flailing"),
    (EnemyState::Thrown, 2019, "Thrown"),
    (EnemyState::Bounce, 2020, "Bounce"),
    (EnemyState::Dropped, 2021, "Dropped"),
    (EnemyState::Startup, 2022, "Startup"),
    (EnemyState::JumpBack, 2030, "JumpBack"),
    (EnemyState::MoveBack, 2031, "MoveBack"),
    (EnemyState::MoveAhead, 2032, "MoveAhead"),
    (EnemyState::BlockHigh, 2033, "BlockHigh"),
    (EnemyState::BlockLow, 2034, "BlockLow"),
    (EnemyState::HumanIdle, 2035, "HumanIdle"),
    (EnemyState::JumpOffSpringboard, 2036, "JumpOffSpringboard"),
    (EnemyState::Recoil, 2050, "Recoil"),
    (EnemyState::VertFiring, 2060, "VertFiring"),
    (EnemyState::Rotate, 2061, "Rotate"),
    (EnemyState::JumpOver, 2070, "JumpOver"),
    (EnemyState::Flip, 2120, "Flip"),
    (EnemyState::FlipLand, 2121, "FlipLand"),
    (EnemyState::Idle2, 2140, "Idle2"),
    (EnemyState::Block, 2141, "Block"),
    (EnemyState::Withdraw, 2142, "Withdraw"),
    (EnemyState::GenericHit, 2143, "GenericHit"),
    (EnemyState::MagicVolley, 2160, "MagicVolley"),
    (EnemyState::MagicShot, 2161, "MagicShot"),
    (EnemyState::Uppercut, 2162, "Uppercut"),
    (EnemyState::SwordThrust, 2163, "SwordThrust"),
    (EnemyState::Sprint, 2164, "Sprint"),
    (EnemyState::StartHug, 2180, "StartHug"),
    (EnemyState::Hug, 2181, "Hug"),
    (EnemyState::Release, 2182, "Release"),
    (EnemyState::GabrielAttack, 2183, "GabrielAttack"),
    (EnemyState::Regroup, 2190, "Regroup"),
    (EnemyState::Hide, 2191, "Hide"),
    (EnemyState::Defense, 2192, "Defense"),
    (EnemyState::Attack, 2193, "Attack"),
    (EnemyState::MovingAttack, 2194, "MovingAttack"),
    (EnemyState::Snip, 2195, "Snip"),
    (EnemyState::Tumble, 2196, "Tumble"),
    (EnemyState::Nudge, 2197, "Nudge"),
    (EnemyState::Launch, 2198, "Launch"),
    (EnemyState::MultiBomb, 2200, "MultiBomb"),
    (EnemyState::ThugAttack, 2201, "ThugAttack"),
    (EnemyState::CannonAttack, 2202, "CannonAttack"),
    (EnemyState::CannonHit, 2203, "CannonHit"),
    (EnemyState::Blocked, 2204, "Blocked"),
    (EnemyState::DeadAct1, 2205, "DeadAct1"),
    (EnemyState::DeadAct2, 2206, "DeadAct2"),
    (EnemyState::DeadAct3, 2207, "DeadAct3"),
    (EnemyState::DeadAct4, 2208, "DeadAct4"),
    (EnemyState::RollForward, 2220, "RollForward"),
    (EnemyState::RollBackward, 2221, "RollBackward"),
    (EnemyState::HoldHook, 2222, "HoldHook"),
    (EnemyState::WaitForParrot, 2240, "WaitForParrot"),
    (EnemyState::FlyAbout, 2241, "FlyAbout"),
    (EnemyState::StrikeSetup, 2242, "StrikeSetup"),
    (EnemyState::FlyToIdle, 2243, "FlyToIdle"),
    (EnemyState::FlyToMarrow, 2244, "FlyToMarrow"),
    (EnemyState::CarryMarrow, 2245, "CarryMarrow"),
    (EnemyState::Swim, 2270, "Swim"),
    (EnemyState::JumpStrike, 2271, "JumpStrike"),
    (EnemyState::OutOfWater, 2272, "OutOfWater"),
    (EnemyState::Singing, 2273, "Singing"),
    (EnemyState::HighPunch, 2280, "HighPunch"),
    (EnemyState::LowPunch, 2281, "LowPunch"),
    (EnemyState::SuperFrenzy, 2282, "SuperFrenzy"),
    (EnemyState::StalactiteHit, 2283, "StalactiteHit"),
    (EnemyState::Scared, 2284, "Scared"),
    (EnemyState::StartGrab, 2290, "StartGrab"),
    (EnemyState::Squeeze, 2291, "Squeeze"),
    (EnemyState::Respawn, 2292, "Respawn"),
    (EnemyState::KnifeAttack, 2300, "KnifeAttack"),
    (EnemyState::KickAttack, 2301, "KickAttack"),
    (EnemyState::SlideAttack, 2302, "SlideAttack"),
    (EnemyState::GunAttack, 2303, "GunAttack"),
    (EnemyState::WindAttack, 2304, "WindAttack"),
    (EnemyState::JumpUp, 2305, "JumpUp"),
    (EnemyState::JumpDown, 2306, "JumpDown"),
    (EnemyState::PhaseOut, 2310, "PhaseOut"),
    (EnemyState::PhaseIn, 2311, "PhaseIn"),
    (EnemyState::LungeAttack, 2320, "LungeAttack"),
    (EnemyState::FrontFlip, 2321, "FrontFlip"),
    (EnemyState::BackFlip, 2322, "BackFlip"),
    (EnemyState::MagicAttack, 2330, "MagicAttack"),
    (EnemyState::MoveToPosition, 2331, "MoveToPosition"),
    (EnemyState::Wait, 2332, "Wait"),
];

/// The blocks of the ninth to fourteenth levels, whose free numbers their
/// families may still take.
const OPEN_BLOCKS: [std::ops::RangeInclusive<i32>; 6] = [
    2220..=2239,
    2240..=2259,
    2260..=2279,
    2280..=2299,
    2300..=2319,
    2320..=2339,
];

/// Whether `id` lies in a block whose families may still add states.
fn in_open_block(id: i32) -> bool {
    OPEN_BLOCKS.iter().any(|b| b.contains(&id))
}

#[test]
fn every_state_number_is_pinned_and_gaps_are_no_state() {
    // Every state is listed here, except new ones in the ninth to
    // fourteenth levels' blocks.
    let listed: Vec<EnemyState> = STATES.iter().map(|s| s.0).collect();
    for s in EnemyState::ALL {
        assert!(
            listed.contains(&s) || in_open_block(s as i32),
            "{} is listed",
            s.name()
        );
    }
    let all: Vec<EnemyState> = EnemyState::ALL
        .into_iter()
        .filter(|s| listed.contains(s))
        .collect();
    assert_eq!(all, listed, "in number order");
    for &(s, id, name) in &STATES {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s), "{name}");
    }
    // The blocks: bosses 2022-2029, human states 2030-2049, the punk rat
    // 2050-2059, cannons 2060-2069, Raux 2070-2089, the fourth level
    // 2120-2139, the fifth level 2140-2159, the sixth level 2160-2179,
    // the seventh level 2180-2199, the eighth level 2200-2219, the ninth
    // level 2220-2239, the tenth level 2240-2259, the eleventh level
    // 2260-2279, the twelfth level 2280-2299, the thirteenth level
    // 2300-2319, the fourteenth level 2320-2339.
    let blocks: [(i32, i32); 17] = [
        (2000, 2021),
        (2022, 2029),
        (2030, 2049),
        (2050, 2059),
        (2060, 2069),
        (2070, 2089),
        (2120, 2139),
        (2140, 2159),
        (2160, 2179),
        (2180, 2199),
        (2200, 2219),
        (2220, 2239),
        (2240, 2259),
        (2260, 2279),
        (2280, 2299),
        (2300, 2319),
        (2320, 2339),
    ];
    for (_, id, name) in STATES {
        assert!(
            blocks.iter().any(|&(lo, hi)| (lo..=hi).contains(&id)),
            "{name} is in a block"
        );
    }
    let known: Vec<i32> = STATES.iter().map(|s| s.1).collect();
    for id in 1990..2360 {
        if !known.contains(&id) && !in_open_block(id) {
            assert_eq!(EnemyState::from_id(id), None, "{id} is a gap");
        }
    }
    for id in [i32::MIN, -1, 0, 1000, 5009, i32::MAX] {
        assert_eq!(EnemyState::from_id(id), None);
    }
}

#[test]
fn the_boss_state_starts_cleared_and_round_trips() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    let b = sim.boss();
    assert_eq!(b.fight(), fight::NONE);
    assert_eq!(b.current(), None);
    assert_eq!(b.meter(), None);
    assert_eq!(b.banner(), None);
    assert_eq!(b.defeated(), None);
    assert_eq!(sim.camera_override(), None);
    let hud = sim.snapshot().hud;
    assert_eq!((hud.boss_meter, hud.boss_banner), (None, None));
    sim.with_world(|w| {
        assert_eq!(w.boss_fight(), 0);
        assert_eq!(w.current_boss(), None);
        assert_eq!(w.boss_meter(), None);
    });

    let claw_id = sim.with_world(|w| w.claw_id.unwrap());
    let boss: ObjectId = 7;
    let meter = BossMeter {
        value: 95,
        max: 100,
    };
    sim.with_world(|w| {
        w.set_boss_fight(fight::STAGING);
        w.set_current_boss(Some(boss));
        w.set_boss_meter(Some(meter));
        w.set_boss_banner(Some((320, -220)));
        w.set_checkpoint(1234, 567);
        assert_eq!(w.boss_fight(), 1);
        assert_eq!(w.current_boss(), Some(boss));
        assert_eq!(w.boss_meter(), Some(meter));
    });
    let snap = sim.step(TickInput::default());
    assert_eq!(snap.hud.boss_meter, Some(meter));
    assert_eq!(snap.hud.boss_banner, Some((320, -220)));
    assert_eq!(sim.boss().fight(), fight::STAGING);
    assert_eq!(sim.boss().current(), Some(boss));
    assert_eq!(sim.claw().checkpoint, Some((1234, 567)));
    // The defeat is kept once.
    sim.with_world(|w| {
        w.boss_defeated(boss);
        w.boss_defeated(claw_id);
    });
    assert_eq!(sim.boss().defeated(), Some(boss));
    sim.with_world(|w| {
        w.set_boss_fight(fight::ON);
        w.set_current_boss(None);
        w.set_boss_meter(None);
        w.set_boss_banner(None);
    });
    let snap = sim.step(TickInput::default());
    assert_eq!(sim.boss().fight(), fight::ON);
    assert_eq!(sim.boss().current(), None);
    assert_eq!((snap.hud.boss_meter, snap.hud.boss_banner), (None, None));
    // A clone keeps it; a level load starts it cleared again.
    assert_eq!(sim.clone().boss(), sim.boss());
    let fresh = Sim::new(flat_level(), SimConfig::default());
    assert_eq!(fresh.boss(), &logic::BossState::default());
}

#[test]
fn a_logics_camera_point_is_honoured_and_released() {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    idle(&mut sim, 30);
    let follow = sim.camera().current;
    let point = (follow.0 + 600, follow.1 - 200);
    sim.with_world(|w| w.set_camera_override(Some(point)));
    assert_eq!(sim.camera_override(), Some(point), "handed over at once");
    let mut previous = sim.camera().current;
    for _ in 0..10 {
        sim.step(TickInput::default());
        let now = sim.camera().current;
        assert!(now.0 > previous.0, "heads right for the point");
        assert!(now.1 < previous.1, "and up");
        previous = now;
    }
    assert_eq!(sim.camera().target, point);
    for _ in 0..200 {
        sim.step(TickInput::default());
    }
    assert_eq!(sim.camera().current, point, "reaches the point");
    // Released, the camera follows Claw again.
    sim.with_world(|w| w.set_camera_override(None));
    assert_eq!(sim.camera_override(), None);
    for _ in 0..200 {
        sim.step(TickInput::default());
    }
    assert_eq!(sim.camera().current, follow, "back on Claw");
    // A later write wins: the scripted point set by the tools replaces a
    // logic's, and Claw's respawn releases it as it does his code's own.
    sim.with_world(|w| w.set_camera_override(Some(point)));
    sim.set_camera_point(Some((1, 2)));
    assert_eq!(sim.camera_override(), Some((1, 2)));
    sim.with_world(|w| w.set_camera_override(Some(point)));
    sim.respawn();
    assert_eq!(sim.camera_override(), None);
}

#[test]
fn freeze_claw_now_takes_effect_in_the_same_tick_until_the_fight_starts() {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    idle(&mut sim, 30);
    // Walking left with an attack running.
    sim.step(press(LEFT));
    sim.step(hold(LEFT));
    // An attack running, as a blow leaves it.
    sim.with_world(|w| w.claw.attacking = true);
    assert!(!sim.claw().facing_right);
    let ticks = sim.tick();
    let rng = sim.rng_state();
    assert!(sim.with_world(|w| w.freeze_claw_now()));
    // No logic call of Claw's was needed.
    assert_eq!(sim.tick(), ticks);
    assert_eq!(claw(&sim).state, state::FREEZE);
    assert!(!sim.claw().attacking, "the attack ended");
    assert!(sim.claw().facing_right, "he faces right");
    assert_eq!(claw(&sim).draw_flags & draw_flags::MIRROR, 0);
    assert_eq!(sim.rng_state(), rng, "no random draw");
    // Already frozen: nothing changes.
    assert!(!sim.with_world(|w| w.freeze_claw_now()));
    let x = claw(&sim).x;
    for _ in 0..40 {
        sim.step(hold(LEFT));
        sim.step(press(JUMP | ATTACK));
        sim.with_world(|w| w.set_boss_fight(fight::STAGING));
    }
    assert_eq!(claw(&sim).state, state::FREEZE);
    assert_eq!(claw(&sim).x, x, "he stays put");
    // The fight starts: his next logic call sees it and he stands.
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    assert_eq!(claw(&sim).state, state::FREEZE);
    sim.step(TickInput::default());
    assert_eq!(claw(&sim).state, state::STAND);
    sim.step(hold(RIGHT));
    sim.step(hold(RIGHT));
    assert!(claw(&sim).x > x, "he moves again");
    // A freeze without the boss flag is not ended by it.
    sim.with_world(|w| w.freeze_claw(0));
    idle(&mut sim, 5);
    assert_eq!(claw(&sim).state, state::FREEZE);
}

#[test]
fn freeze_claw_now_takes_over_a_waiting_freeze() {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    idle(&mut sim, 30);
    sim.with_world(|w| {
        assert!(w.freeze_claw(0));
        assert!(w.freeze_claw_now(), "the waiting freeze starts now");
        w.set_boss_fight(fight::ON);
    });
    sim.step(TickInput::default());
    assert_eq!(claw(&sim).state, state::STAND, "the boss freeze ended it");
}

#[test]
fn traces_carry_the_boss_columns() {
    assert!(trace::HEADER.ends_with(",rng,boss_fight,boss_health"));
    let mut level = flat_level();
    level
        .objects
        .push(ObjectSpec::at("Officer", 1300, 900, 4000));
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    let row = TraceRow::capture(&sim, 0, "script");
    assert_eq!((row.boss_fight, row.boss_health), (Some(0), None));
    assert!(row.to_csv().ends_with(",0,"), "{}", row.to_csv());
    // A current boss shows his health.
    let officer = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "Officer")
        .map(|(id, _)| id)
        .unwrap();
    sim.with_world(|w| {
        w.set_boss_fight(fight::ON);
        w.set_current_boss(Some(officer));
    });
    sim.step(TickInput::default());
    let health = sim.objects().get(officer).unwrap().health;
    let row = TraceRow::capture(&sim, 1, "script");
    assert_eq!((row.boss_fight, row.boss_health), (Some(2), Some(health)));
    // Parsed back, with and without the columns.
    let text = trace::to_csv(std::slice::from_ref(&row));
    let parsed = trace::parse(&text).unwrap();
    assert_eq!(
        (parsed[0].boss_fight, parsed[0].boss_health),
        (Some(2), Some(health))
    );
    let old = text
        .replace(",boss_fight,boss_health", "")
        .replace(&format!(",2,{health}\n"), "\n");
    let parsed = trace::parse(&old).unwrap();
    assert_eq!(parsed[0].rng, row.rng);
    assert_eq!((parsed[0].boss_fight, parsed[0].boss_health), (None, None));
    // Empty cells, and cells missing at the end of a row.
    let empty = trace::parse(&format!(
        "{}\n0,18,18,1,2,,,24,1,0,0,0,0,reference,,100,0,7,pistol,10,5,3,,5,,\n0,36,18,1,2,,,24,1,0,0,0,0,reference,,100,0,7,pistol,10,5,3,,5\n",
        trace::HEADER
    ))
    .unwrap();
    for r in &empty {
        assert_eq!((r.boss_fight, r.boss_health), (None, None));
    }
    assert!(
        trace::parse(&format!(
            "{}\n0,18,18,1,2,,,24,1,0,0,0,0,reference,,100,0,7,pistol,10,5,3,,5,x,",
            trace::HEADER
        ))
        .is_err()
    );
}

#[test]
fn the_boss_columns_are_compared_when_both_traces_have_them() {
    let mut level = flat_level();
    level
        .objects
        .push(ObjectSpec::at("Officer", 1300, 900, 4000));
    let mut sim = Sim::new(level, SimConfig::default());
    let mut rows = Vec::new();
    for k in 0..5 {
        sim.step(TickInput::default());
        rows.push(TraceRow::capture(&sim, k, "script"));
    }
    let tol = claw_sim::Tolerances::exact();
    assert!(trace::compare(&rows, &rows, &tol).passed());
    let mut reference = rows.clone();
    reference[3].boss_fight = Some(1);
    let d = trace::compare(&rows, &reference, &tol)
        .first_divergence
        .expect("the flag differs");
    assert_eq!(d.column, "boss_fight");
    // A reference without the columns compares as before.
    for r in &mut reference {
        r.boss_fight = None;
        r.boss_health = None;
    }
    assert!(trace::compare(&rows, &reference, &tol).passed());
    let mut reference = rows.clone();
    reference[2].boss_health = Some(50);
    rows[2].boss_health = Some(40);
    let d = trace::compare(&rows, &reference, &tol)
        .first_divergence
        .expect("the health differs");
    assert_eq!(d.column, "boss_health");
}

/// Two officers on the flat level and the first one's id.
fn officer_sim() -> (Sim, ObjectId) {
    let mut level = flat_level();
    level
        .objects
        .push(ObjectSpec::at("Officer", 1300, 900, 4000));
    let mut sim = Sim::new(level, SimConfig::default());
    idle(&mut sim, 3);
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "Officer")
        .map(|(id, _)| id)
        .unwrap();
    assert!(
        enemy::data_of(sim.objects().get(id).unwrap()).is_some(),
        "built"
    );
    (sim, id)
}

#[test]
fn a_death_tile_goes_through_the_goo_hook_unchanged() {
    let (sim, id) = officer_sim();
    let mut by_die = sim.clone();
    by_die.with_world(|w| enemy::die(w, id, DeathKind::Spikes, None));
    let mut by_hook = sim.clone();
    by_hook.with_world(|w| enemy::goo::enter_goo(w, id));
    let mut direct = sim.clone();
    direct.with_world(|w| enemy::die_of(w, id, DeathKind::Spikes, None));
    for s in [&by_die, &by_hook] {
        let o = s.objects().get(id).unwrap();
        let d = direct.objects().get(id).unwrap();
        assert_eq!(o.state, EnemyState::Dead as i32);
        assert_ne!(o.draw_flags & draw_flags::FLASH, 0);
        assert_eq!(
            (o.state, o.flags, o.draw_flags),
            (d.state, d.flags, d.draw_flags)
        );
        assert_eq!(o.data, d.data);
        assert_eq!(enemy::data_of(o).unwrap().death, DeathKind::Spikes);
        assert_eq!(s.rng_state(), direct.rng_state());
    }
}

#[test]
fn enemy_data_is_reached_inside_a_familys_own_data() {
    let (mut sim, id) = officer_sim();
    let plain = sim.objects().get(id).unwrap().data.clone();
    let enemy_data = plain.enemy().cloned().expect("enemy data");
    // Wrapped in Raux's own data, the shared behaviour still finds it.
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.data = ObjectData::Raux(enemy::raux::RauxData {
            enemy: enemy_data.clone(),
        });
        assert_eq!(enemy::data(w, id), Some(&enemy_data));
        enemy::data_mut(w, id).unwrap().roll = 77;
        let seen = enemy::with_data(w, id, |_, d| {
            d.roll += 1;
            d.roll
        });
        assert_eq!(seen, Some(78));
        assert!(matches!(
            w.objects.get(id).unwrap().data,
            ObjectData::Raux(_)
        ));
    });
    let o = sim.objects().get(id).unwrap();
    assert_eq!(enemy::data_of(o).unwrap().roll, 78);
    let wrapped = ObjectData::PunkRat(enemy::punkrat::PunkRatData {
        enemy: enemy_data.clone(),
    });
    assert_eq!(wrapped.enemy(), Some(&enemy_data));
    // Other data holds none, and is left in place.
    assert_eq!(ObjectData::None.enemy(), None);
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    let claw_id = sim.with_world(|w| w.claw_id.unwrap());
    sim.with_world(|w| {
        assert_eq!(enemy::with_data(w, claw_id, |_, _| ()), None);
    });
    // The new classes point into their files.
    assert_eq!(enemy::class(GameLogic::PunkRat).unwrap().name, "PunkRat");
    assert_eq!(enemy::class(GameLogic::Raux).unwrap().name, "Raux");
    assert!(enemy::class(GameLogic::Cannon).is_none());
    assert!(enemy::class(GameLogic::BossStage).is_none());
}

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

#[test]
fn install_level2_declared_families_load_as_reported_placeholders() {
    let Some(store) = store() else { return };
    let data = LevelData::build(&store, LevelSource::Retail(2)).unwrap();
    let level = SimLevel::from_level(&data, &store).unwrap();
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    sim.step(TickInput::default());
    let d = sim.diagnostics();
    for name in [
        "PunkRat",
        "TowerCannonLeft",
        "TowerCannonRight",
        "Raux",
        "BossStager",
        "BossWarp",
    ] {
        let placed = level.objects.iter().filter(|o| o.logic == name).count();
        assert!(placed > 0, "LEVEL2 places {name}");
        // Reported until its family is built.
        let reported = logic::runs(name).is_none().then_some(&placed);
        assert_eq!(d.inert_logics.get(name), reported, "{name}");
    }
    assert_eq!(sim.boss(), &logic::BossState::default());
}
