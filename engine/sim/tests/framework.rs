//! The framework inputs the level and player code build on: the attack
//! gate of the standing state, the level number, the attempt counter, the
//! image sets' frame ranges and boxes, the screen shake request, the
//! sound-only random stream and the kind of a kill, driven through the
//! public simulation API.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use claw_assets::level::RETAIL_LEVELS;
use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::anim::Animation;
use claw_sim::level::{ImageFrames, frame_box};
use claw_sim::object::{LogicKind, Object, Rect, draw_flags, engine_state, flags, types};
use claw_sim::player::state;
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const ATTACK: u32 = 0x2;

fn claw_state(sim: &Sim) -> i32 {
    sim.player().expect("Claw").state
}

/// Key of a blow animation that holds its first image for good.
const HELD_BLOW: &str = "TEST_HELD_BLOW";

/// Puts Claw in a running attack, as starting a blow does, playing
/// [`HELD_BLOW`]; `interruptible` says whether its active part is over.
fn force_attack(sim: &mut Sim, interruptible: bool) {
    sim.with_world(|w| {
        let id = w.claw_id.unwrap();
        assert!(w.set_anim(id, HELD_BLOW));
        w.claw.attacking = true;
        w.claw.attack.interruptible = interruptible;
    });
}

#[test]
fn a_running_blow_holds_the_standing_input_until_it_can_be_interrupted() {
    let mut level = flat_level();
    let held = Arc::new(Animation::looping(HELD_BLOW, &[(1, 100_000)]));
    level.animations.insert(held.key.clone(), held);
    let mut sim = Sim::new(level, SimConfig::default());
    idle(&mut sim, 30);
    assert_eq!(claw_state(&sim), state::STAND);
    let x = sim.player().unwrap().x;
    force_attack(&mut sim, false);
    sim.step(press(RIGHT));
    for _ in 0..4 {
        sim.step(hold(RIGHT));
    }
    sim.step(press(JUMP));
    sim.step(hold(LEFT | JUMP));
    assert_eq!(claw_state(&sim), state::STAND, "no walk or jump starts");
    assert_eq!(sim.player().unwrap().x, x, "Claw does not move");
    assert!(sim.claw().attacking, "the blow is not cancelled");
    // Once the active part is over, input is read again, but a walk starts
    // only on a press: Right held from before does not start it.
    force_attack(&mut sim, true);
    sim.step(hold(RIGHT));
    assert_eq!(claw_state(&sim), state::STAND);
    assert!(sim.claw().attacking);
    // A press does, and ends the blow.
    sim.step(TickInput::default());
    sim.step(press(RIGHT));
    assert_eq!(claw_state(&sim), state::MOVE_RIGHT);
    assert!(!sim.claw().attacking);
}

#[test]
fn without_a_blow_the_standing_input_is_read_as_before() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    idle(&mut sim, 30);
    sim.step(press(RIGHT));
    assert_eq!(claw_state(&sim), state::MOVE_RIGHT);
}

/// An animation of (image, duration, event) elements; the last element
/// finishes it.
fn blow(key: &str, frames: &[(i32, i32, i32)]) -> Arc<Animation> {
    let pairs: Vec<(i32, i32)> = frames.iter().map(|&(i, d, _)| (i, d)).collect();
    let mut a = Animation::once(key, &pairs);
    for (f, &(_, _, event)) in a.frames.iter_mut().zip(frames) {
        f.event = event;
    }
    Arc::new(a)
}

/// Holding Right and Attack through a running blow does not cut it short
/// before its strike box goes live.
#[test]
fn holding_right_and_attack_through_a_blow_lets_it_strike() {
    let mut level = flat_level();
    let strike = [(1, 25, 0), (2, 25, 0), (3, 125, 1), (2, 35, 0), (1, 25, 0)];
    for key in [
        "CLAW_SWIPE",
        "CLAW_PUNCH",
        "CLAW_UPPERCUT",
        "CLAW_KICK",
        "CLAW_DUCKSWIPE",
        "CLAW_JUMPSWIPE",
    ] {
        let a = blow(key, &strike);
        level.animations.insert(a.key.clone(), a);
    }
    let stand = Arc::new(Animation::looping("CLAW_STAND", &[(11, 110), (12, 110)]));
    level.animations.insert(stand.key.clone(), stand);
    let mut sim = Sim::new(level, SimConfig::default());
    idle(&mut sim, 30);
    for _ in 0..5 {
        sim.step(key_down(&sim, RIGHT));
    }
    assert_eq!(claw_state(&sim), state::MOVE_RIGHT);
    sim.step(TickInput {
        held: Buttons(RIGHT | ATTACK),
        pressed: Buttons(ATTACK),
    });
    assert!(sim.claw().attacking, "the blow starts");
    let mut struck = false;
    for _ in 0..60 {
        if !sim.claw().attacking {
            break;
        }
        struck |= sim.player().unwrap().flags & flags::HARMLESS == 0;
        sim.step(hold(RIGHT | ATTACK));
    }
    assert!(
        struck,
        "the strike box goes live before holding Right ends the blow"
    );
}

#[test]
fn the_level_number_and_the_attempt_counter_reach_the_logics() {
    let mut level = flat_level();
    assert_eq!(level.number, 1, "a synthetic level counts as the first");
    level.number = 7;
    let mut sim = Sim::new(level, SimConfig::default());
    assert_eq!(sim.with_world(|w| w.level_number()), 7);
    assert_eq!(sim.stats().attempt, 0);
    sim.with_world(|w| {
        assert_eq!(w.attempt(), 0);
        w.claw.stats.count_attempt();
        assert_eq!(w.attempt(), 1);
    });
    assert_eq!(sim.stats().attempt, 1);
}

#[test]
fn image_ranges_and_frame_boxes_reach_the_logics() {
    let mut level = flat_level();
    let mut set = ImageFrames::range(1, 4);
    set.boxes.insert(2, frame_box(20, 30, 4, -6));
    level.images.insert("TEST_SET".into(), set);
    let mut sim = Sim::new(level, SimConfig::default());
    sim.with_world(|w| {
        assert_eq!(w.image_frames("TEST_SET"), Some((1, 4)));
        assert_eq!(w.image_frames("NO_SUCH_SET"), None);
        assert_eq!(w.frame_box("TEST_SET", 2), Some(Rect::new(-6, -21, 13, 8)));
        assert_eq!(w.frame_box("TEST_SET", 3), None);
        let id = w.claw_id.unwrap();
        let mut o = w.objects.get(id).unwrap().clone();
        o.image_set = "TEST_SET".into();
        o.frame = 2;
        o.x = 100;
        o.y = 200;
        assert_eq!(w.draw_box(&o), Some(Rect::new(94, 179, 113, 208)));
        o.draw_flags |= draw_flags::MIRROR;
        assert_eq!(w.draw_box(&o), Some(Rect::new(87, 179, 106, 208)));
    });
}

#[test]
fn a_shake_request_jolts_the_snapshot_camera_for_its_length() {
    let mut still = Sim::new(flat_level(), SimConfig::default());
    idle(&mut still, 30);
    let mut shaken = still.clone();
    shaken.with_world(|w| w.shake(500));
    let mut replay = shaken.clone();
    let now = shaken.tick() * 18;
    let cameras = |s: &mut Sim| {
        (0..40)
            .map(|_| s.step(TickInput::default()).camera)
            .collect::<Vec<_>>()
    };
    let (a, b, c) = (
        cameras(&mut still),
        cameras(&mut shaken),
        cameras(&mut replay),
    );
    assert_eq!(shaken.camera().shake_until_ms, now + 500);
    assert!(a != b, "the shake shows in the snapshot camera");
    assert_eq!(
        b, c,
        "the same request from the same state shakes the same way"
    );
    assert_eq!(a[39], b[39], "after the shake the camera is back on Claw");
}

#[test]
fn the_sound_stream_is_seeded_per_level_and_apart_from_the_game_stream() {
    let a = Sim::new(flat_level(), SimConfig::default());
    let mut other = flat_level();
    other.number = 2;
    let b = Sim::new(other, SimConfig::default());
    assert_eq!(a.rng(), b.rng(), "the game stream depends on the seed only");
    assert_ne!(a.sound_rng(), b.sound_rng());
    assert_ne!(a.sound_rng(), a.rng());
    let c = Sim::new(flat_level(), SimConfig::default());
    assert_eq!(a.sound_rng(), c.sound_rng());
}

/// An undrawn object with no logic of its own at (x, 900).
fn bare(name: &str, x: i32) -> Object {
    let mut o = Object::new(0, x, 900, 10, name.into(), LogicKind::Inert);
    o.flags = flags::WORLD;
    o
}

#[test]
fn a_survived_kill_leaves_no_kind_for_a_later_death_by_hit_damage() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    idle(&mut sim, 2);
    let mut target = bare("TestTarget", 1300);
    target.flags |= flags::AUTO_HIT_DAMAGE;
    target.hit_type_flags = types::THROWN;
    target.hit_rect = Rect::new(-5, -5, 5, 5);
    target.health = 20;
    let id = sim.spawn(target);
    assert!(sim.with_world(|w| w.kill(id, 2)));
    let o = sim.objects().get(id).unwrap();
    assert_eq!((o.state, o.kill_kind), (engine_state::KILLED, 2));
    // Its logic lets it live through that kind of kill.
    sim.with_world(|w| w.objects.get_mut(id).unwrap().state = 1);
    idle(&mut sim, 1);
    assert_eq!(sim.objects().get(id).unwrap().state, 1);
    // Later something thrown at it takes the rest of its health.
    let mut blow = bare("TestBlow", 1300);
    blow.type_flags = types::THROWN;
    blow.attack_rect = Rect::new(-5, -5, 5, 5);
    blow.damage = 30;
    sim.spawn(blow);
    idle(&mut sim, 1);
    let o = sim.objects().get(id).unwrap();
    assert!(o.health <= 0);
    assert_eq!(
        (o.state, o.kill_kind),
        (engine_state::KILLED, 0),
        "an ordinary kill, not the old kind"
    );
}

// ------------------------------------------------------------- install

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

fn level(store: &AssetStore, n: u32) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(n)).unwrap();
    SimLevel::from_level(&data, store).unwrap()
}

#[test]
fn install_levels_know_their_number() {
    let Some(store) = store() else { return };
    for n in RETAIL_LEVELS {
        assert_eq!(level(&store, n).number, n);
    }
}

#[test]
fn install_level1_animated_decorations_know_their_frames_without_help() {
    let Some(store) = store() else { return };
    let l = level(&store, 1);
    let candy: Vec<_> = l
        .objects
        .iter()
        .filter(|o| o.logic == "BehindAniCandy")
        .collect();
    assert!(!candy.is_empty());
    let mut animated = 0;
    for o in &candy {
        // A record may name a set the level does not register; it shows
        // nothing either way.
        let Some((first, last)) = l.image_frames(&o.image_set) else {
            continue;
        };
        assert!(first <= o.frame && o.frame <= last, "{}", o.image_set);
        let b = l
            .frame_box(&o.image_set, o.frame)
            .unwrap_or_else(|| panic!("{} frame {} has a box", o.image_set, o.frame));
        assert!(b.left <= b.right && b.top <= b.bottom);
        if last > first {
            animated += 1;
        }
    }
    assert!(
        animated * 2 > candy.len(),
        "most of the {} animated decorations have several frames, {animated} do",
        candy.len()
    );
    // Claw's own set is there too, and a Sim hands the ranges to logics.
    let claw = l.image_frames("CLAW").expect("Claw's frames");
    let mut sim = Sim::new(l, SimConfig::default());
    assert_eq!(sim.with_world(|w| w.image_frames("CLAW")), Some(claw));
}
