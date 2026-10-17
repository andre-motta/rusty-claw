//! The third level's thieves with the user's own game data: every placed
//! thief runs, the animations and sounds they play ship with the level,
//! the knife's and the arrow's boxes come from their first frames, and the
//! throw and shot schedule at the 18 ms step.
//!
//! These tests read the user's own install from the folder named by
//! `CLAW_INSTALL` at run time and skip themselves when it is unset; nothing
//! from an install is stored in the repository.

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, cutthroat, robberthief};
use claw_sim::logic::projectile::{self, ProjectileSpec};
use claw_sim::logic::{self, GameLogic, ObjectData};
use claw_sim::object::{LogicKind, ObjectId, types};
use claw_sim::{Sim, SimConfig, SimLevel};
use common::*;

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

fn level3() -> Option<SimLevel> {
    let store = store()?;
    let data = LevelData::build(&store, LevelSource::Retail(3)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

/// A simulation of `level` with every object awake from the start.
fn awake(level: SimLevel) -> Sim {
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.with_world(|w| w.claw.stats.health = 10_000);
    sim
}

/// The flat floor of the common helpers carrying the third level's
/// animations, sounds and image frames, with `logic` placed at x 1250
/// (Claw at 1000).
fn arena(l3: &SimLevel, logic: &str) -> SimLevel {
    let mut l = flat_level();
    l.number = 3;
    l.animations = l3.animations.clone();
    l.sounds = l3.sounds.clone();
    l.images = l3.images.clone();
    let mut o = ObjectSpec::at(logic, 1250, 900, 4000);
    o.image_set = format!("LEVEL_{}", logic.to_uppercase());
    l.objects = vec![o];
    l
}

fn thieves(sim: &Sim) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| {
            matches!(
                o.kind,
                LogicKind::Game(GameLogic::CutThroat | GameLogic::RobberThief)
            )
        })
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn install_every_thief_of_the_third_level_runs() {
    let Some(level) = level3() else { return };
    let placed = |name: &str| level.objects.iter().filter(|o| o.logic == name).count();
    assert_eq!(placed("CutThroat"), 34);
    assert_eq!(placed("RobberThief"), 35);
    assert!(logic::runs("CutThroat").is_some());
    assert!(logic::runs("RobberThief").is_some());
    let mut sim = awake(level);
    for _ in 0..3 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    assert!(!d.inert_logics.contains_key("CutThroat"));
    assert!(!d.inert_logics.contains_key("RobberThief"));
    let ids = thieves(&sim);
    assert_eq!(ids.len(), 69);
    for id in ids {
        let o = sim.objects().get(id).unwrap();
        assert!(EnemyState::from_id(o.state).is_some(), "built");
        assert_eq!(o.health, 7, "7 health on the third level");
        assert!(enemy::data_of(o).is_some());
    }
}

#[test]
fn install_the_thieves_animations_and_sounds_ship_with_the_third_level() {
    let Some(level) = level3() else { return };
    let mut anims: BTreeSet<&str> = BTreeSet::new();
    let mut sounds: BTreeSet<&str> = BTreeSet::new();
    for class in [&cutthroat::CLASS, &robberthief::CLASS] {
        let a = class.anims;
        anims.extend([
            a.fast_advance,
            a.pace,
            a.hit_low,
            a.hit_high,
            a.home,
            a.jump,
            a.fall,
            a.lifted,
            a.lifted2,
            a.bounce,
        ]);
        sounds.extend(class.sounds.hit_low);
        sounds.insert(class.sounds.hit_high);
    }
    anims.extend(cutthroat::STAND);
    anims.extend([
        cutthroat::STRIKE_ANIM,
        cutthroat::THROW_ANIM,
        cutthroat::THROW_LOW_ANIM,
        cutthroat::DEATH_ANIM,
        robberthief::IDLE_ANIM,
        robberthief::SHOOT_ANIM,
        robberthief::SHOOT_LOW_ANIM,
        robberthief::DEATH_ANIM,
    ]);
    sounds.extend(cutthroat::IDLE_LINES);
    sounds.extend(robberthief::IDLE_LINES);
    sounds.extend([
        cutthroat::ALERT,
        cutthroat::DEATH_SOUND,
        cutthroat::LIFTED_LINE,
        cutthroat::THROW_SOUND,
        cutthroat::THROW_LOW_SOUND,
        robberthief::LIFTED_LINE,
        robberthief::SHOT_SOUND,
        robberthief::SHOT_LOW_SOUND,
    ]);
    for k in anims {
        assert!(level.animations.contains_key(k), "animation {k}");
    }
    for k in sounds {
        assert!(level.sounds.contains_key(k), "sound {k}");
    }
}

#[test]
fn install_knife_and_arrow_boxes_come_from_their_first_frame() {
    let Some(level) = level3() else { return };
    let mut sim = awake(level);
    for image in [cutthroat::KNIFE_IMAGE, robberthief::ARROW_IMAGE] {
        let want = projectile::image_frame_box(image).unwrap();
        for vx in [400, -400] {
            let id = sim
                .with_world(|w| {
                    projectile::spawn(
                        w,
                        ProjectileSpec {
                            logic: "Bullet",
                            x: 2000,
                            y: 2000,
                            z: 1,
                            vx,
                            damage: 10,
                            type_flags: types::ENEMY_SHOT,
                            image_set: image,
                            ..ProjectileSpec::default()
                        },
                    )
                })
                .unwrap();
            let o = sim.objects().get(id).unwrap();
            let r = if vx < 0 { want.mirrored() } else { want };
            assert_eq!(o.move_rect, r, "{image} {vx}");
            assert_eq!(o.attack_rect, r, "{image} {vx}");
            sim.with_world(|w| {
                w.objects.remove(id);
            });
        }
    }
}

/// What a thief at x 1250 did over `n` ticks with `input`: the ticks (from
/// 0) on which he entered `Fire`, on which a shot appeared and on which he
/// was Home again after a shot, and the firing animations he played;
/// shots are removed as they appear.
struct Schedule {
    fire: Vec<usize>,
    shot: Vec<usize>,
    home: Vec<usize>,
    anims: BTreeSet<String>,
}

fn schedule(l3: &SimLevel, logic: &str, n: usize, input: TickInput) -> Schedule {
    let mut sim = awake(arena(l3, logic));
    // Keys count from the first tick (Claw crouches at once).
    sim.unlatch_keys();
    let mut fire = Vec::new();
    let mut shot = Vec::new();
    let mut home = Vec::new();
    let mut seen = BTreeSet::new();
    let mut fired = BTreeSet::new();
    let mut was = None;
    for t in 0..n {
        sim.step(input);
        let id = thieves(&sim)[0];
        let s = EnemyState::from_id(sim.objects().get(id).unwrap().state);
        if s == Some(EnemyState::Fire) && was != s {
            fire.push(t);
            let o = sim.objects().get(id).unwrap();
            fired.insert(o.anim.key().unwrap_or("").to_string());
        }
        if was == Some(EnemyState::Fire) && s == Some(EnemyState::Home) {
            home.push(t);
        }
        was = s;
        let new: Vec<ObjectId> = sim
            .objects()
            .all()
            .filter(|(id, o)| matches!(o.data, ObjectData::Projectile(_)) && seen.insert(*id))
            .map(|(id, _)| id)
            .collect();
        if !new.is_empty() {
            shot.push(t);
        }
        sim.with_world(|w| {
            for id in new {
                w.objects.remove(id);
            }
        });
    }
    Schedule {
        fire,
        shot,
        home,
        anims: fired,
    }
}

#[test]
fn install_the_cutthroat_throws_on_the_level_three_schedule() {
    let Some(l3) = level3() else { return };
    for (input, anim, release, done) in [
        (TickInput::default(), cutthroat::THROW_ANIM, 32, 38),
        (hold(DOWN), cutthroat::THROW_LOW_ANIM, 33, 39),
    ] {
        let Schedule {
            fire,
            shot,
            home,
            anims,
        } = schedule(&l3, "CutThroat", 400, input);
        assert_eq!(anims, BTreeSet::from([anim.to_string()]));
        assert!(fire.len() >= 3, "{fire:?}");
        assert!(shot.len() + 1 >= fire.len(), "a knife per throw: {shot:?}");
        for (i, h) in fire.iter().enumerate().take(shot.len()) {
            assert_eq!(
                shot[i] - h,
                release,
                "knife leaves on the throw's shot frame"
            );
            assert_eq!(home[i] - h, done, "Home when the throw ends");
        }
        for p in fire.windows(2) {
            assert_eq!(p[1] - p[0], 112, "2000 ms, rounded up to whole ticks");
        }
    }
}

#[test]
fn install_the_robber_thief_shoots_on_the_level_three_schedule() {
    let Some(l3) = level3() else { return };
    for (input, anim) in [
        (TickInput::default(), robberthief::SHOOT_ANIM),
        (hold(DOWN), robberthief::SHOOT_LOW_ANIM),
    ] {
        let Schedule {
            fire,
            shot,
            home,
            anims,
        } = schedule(&l3, "RobberThief", 500, input);
        assert_eq!(anims, BTreeSet::from([anim.to_string()]));
        assert!(fire.len() >= 4, "{fire:?}");
        assert!(shot.len() + 1 >= fire.len(), "an arrow per shot: {shot:?}");
        for (i, h) in fire.iter().enumerate().take(shot.len()) {
            assert_eq!(shot[i] - h, 40, "arrow leaves on the shot frame");
            assert_eq!(home[i] - h, 61, "Home when the shot ends");
        }
        for p in fire.windows(2) {
            assert!((84..=87).contains(&(p[1] - p[0])), "{fire:?}");
        }
    }
}
