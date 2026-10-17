//! The boss fight's stage on a synthetic arena, with a stand-in boss (an
//! inert object made the current boss): the introduction's order and
//! timing, the trigger's one check on Claw, the jump-in entry that skips
//! the introduction, the boss music, the health bar and the door.

mod common;

use std::sync::Arc;

use claw_sim::claw_frame::output::BossMeter;
use claw_sim::claw_frame::{AudioCue, Buttons, GameEvent, MusicTrack, SoundChannel};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::boss_stage::{self, stage_state};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{GameLogic, ObjectData};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::physics::ptype;
use claw_sim::player::death::powerup;
use claw_sim::player::hit::PickupFields;
use claw_sim::player::special::BOSS_END;
use claw_sim::player::state;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// Where the stager stands: 47 px below Claw's feet line, as on the
/// second level, so a standing Claw at its x is inside its box.
const STAGER_X: i32 = 3000;
/// The stand-in boss, 320 px right of the stager.
const BOSS_X: i32 = STAGER_X + 320;
/// Lengths of the two lines, ms (those of the second level).
const ENEMY_LINE_MS: u32 = 2820;
const CLAW_LINE_MS: u32 = 1750;
const STAND_IN: &str = "StandInBoss";

/// Claw's y standing on the flat level's floor.
fn floor_y() -> i32 {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    idle(&mut sim, 40);
    sim.player().unwrap().y
}

/// The flat level with the stager (at depth `stager_z`), a stand-in boss
/// and the two staging lines; Claw starts `start_dx` px left of the
/// stager.
fn arena(start_dx: i32, stager_z: i32) -> (SimLevel, i32) {
    let y = floor_y();
    let mut level = flat_level();
    level.start = (STAGER_X - start_dx, y);
    level.objects.push(ObjectSpec::at(
        boss_stage::STAGER,
        STAGER_X,
        y + 47,
        stager_z,
    ));
    level
        .objects
        .push(ObjectSpec::at(STAND_IN, BOSS_X, y + 8, 4500));
    level
        .sounds
        .insert(boss_stage::ENEMY_LINE.into(), Some(ENEMY_LINE_MS));
    level
        .sounds
        .insert(boss_stage::CLAW_LINE.into(), Some(CLAW_LINE_MS));
    for key in [
        boss_stage::BANNER_SOUND,
        boss_stage::THUD_SOUND,
        boss_stage::DEFEAT_SOUND,
    ] {
        level.sounds.insert(key.into(), Some(500));
    }
    (level, y)
}

fn find(sim: &Sim, logic: &str) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
}

/// A sim on the arena whose stand-in boss is the current boss with 100
/// health.
fn arena_sim(start_dx: i32, stager_z: i32) -> (Sim, ObjectId, ObjectId) {
    let (level, _) = arena(start_dx, stager_z);
    let mut sim = pressing_sim(level, SimConfig::default());
    idle(&mut sim, 30);
    let boss = find(&sim, STAND_IN).expect("the stand-in boss");
    sim.with_world(|w| {
        w.objects.get_mut(boss).unwrap().health = 100;
        w.set_current_boss(Some(boss));
    });
    let stager = find(&sim, boss_stage::STAGER).expect("the stager");
    (sim, boss, stager)
}

fn checkpoint_in(events: &[GameEvent]) -> Option<(i32, i32)> {
    events.iter().find_map(|e| match *e {
        GameEvent::Checkpoint { x, y } => Some((x, y)),
        _ => None,
    })
}

fn plays(audio: &[AudioCue], key: &str, channel: SoundChannel) -> bool {
    audio
        .iter()
        .any(|c| matches!(c, AudioCue::Play(s) if s.key == key && s.channel == channel))
}

fn boss_music(audio: &[AudioCue]) -> bool {
    audio.iter().any(|c| {
        matches!(
            c,
            AudioCue::Music {
                track: MusicTrack::Boss,
                looped: true
            }
        )
    })
}

/// Everything the introduction changes, after one tick.
#[derive(Debug, Clone, PartialEq)]
struct Look {
    tick: u64,
    x: i32,
    claw_state: i32,
    physics: i32,
    flag: u8,
    camera_point: Option<(i32, i32)>,
    meter: Option<BossMeter>,
    banner: Option<(i32, i32)>,
    stager: bool,
    checkpoint: Option<(i32, i32)>,
    audio: Vec<AudioCue>,
}

fn look(sim: &Sim, stager: ObjectId) -> Look {
    let claw = sim.player().unwrap();
    let hud = sim.hud();
    Look {
        tick: sim.tick(),
        x: claw.x,
        claw_state: claw.state,
        physics: claw.physics_type,
        flag: sim.boss().fight(),
        camera_point: sim.camera_override(),
        meter: hud.boss_meter,
        banner: hud.boss_banner,
        stager: sim.objects().get(stager).is_some(),
        checkpoint: checkpoint_in(sim.last_events()),
        audio: sim.last_audio().to_vec(),
    }
}

#[test]
fn the_stager_is_built_and_wakes_with_the_boss_music() {
    let (level, _) = arena(200, 8000);
    let mut sim = pressing_sim(level, SimConfig::default());
    let stager = find(&sim, boss_stage::STAGER).unwrap();
    let o = sim.objects().get(stager).unwrap();
    assert_eq!(o.kind, LogicKind::Game(GameLogic::BossStage));
    assert!(
        !sim.diagnostics()
            .inert_logics
            .contains_key(boss_stage::STAGER)
    );
    // Its first call came while the level loaded: the boss music is in the
    // first tick's cues, and the stager is a hidden, armed trigger.
    let snap = sim.step(TickInput::default());
    assert!(boss_music(&snap.audio), "{:?}", snap.audio);
    let o = sim.objects().get(stager).unwrap();
    assert_eq!(o.state, stage_state::ARMED);
    assert_eq!(o.attack_rect, boss_stage::TRIGGER_BOX);
    assert_eq!(o.attack_type_flags, types::PLAYER);
    assert!(matches!(o.data, ObjectData::BossStage(_)));
    assert!(snap.renderables.iter().all(|r| r.id != o.serial), "hidden");
    // Nothing else happens until Claw comes.
    idle(&mut sim, 50);
    assert_eq!(sim.boss().fight(), fight::NONE);
    assert!(!boss_music(sim.last_audio()));
}

#[test]
fn the_introduction_freezes_pans_speaks_and_thaws_at_its_ticks() {
    let (mut sim, boss, stager) = arena_sim(150, 8000);
    let boss_at = {
        let b = sim.objects().get(boss).unwrap();
        (b.x, b.y)
    };
    let mut trace: Vec<Look> = Vec::new();
    // Walk right into the box and keep the key held all along.
    let mut n = None;
    for _ in 0..200 {
        sim.step(hold(RIGHT));
        let l = look(&sim, stager);
        let fired = l.checkpoint.is_some();
        trace.push(l);
        if fired {
            n = Some(trace.len() - 1);
            break;
        }
    }
    let n = n.expect("the trigger fired");
    for _ in 0..700 {
        sim.step(hold(RIGHT));
        trace.push(look(&sim, stager));
    }
    let at = |k: usize| &trace[n + k];
    // N: the trigger accepted the walking Claw; nothing else yet.
    let trig = at(0);
    assert_eq!(trig.physics, ptype::WALK);
    assert_eq!(trig.checkpoint, Some((trig.x, sim.player().unwrap().y)));
    assert_eq!(sim.claw().checkpoint, trig.checkpoint);
    assert_eq!(sim.boss().checkpoint_time(), 120_000);
    assert_eq!(trig.flag, fight::NONE);
    assert!(trig.x >= STAGER_X - 31 && trace[n - 1].x < STAGER_X - 31);
    // N+1: flag 1, frozen after one more step of his own, the bar, the
    // camera on the boss.
    let start = at(1);
    assert_eq!(start.flag, fight::STAGING);
    assert_eq!(start.claw_state, state::FREEZE);
    assert!(start.x > trig.x, "Claw moved once more before the freeze");
    assert_eq!(
        start.meter,
        Some(BossMeter {
            value: 100,
            max: 100
        })
    );
    assert_eq!(start.camera_point, Some(boss_at));
    let frozen_x = start.x;
    // The boss's line 53 ticks later, Claw's after the camera is back.
    let line = |k: usize, key: &str| plays(&at(k).audio, key, SoundChannel::Voice);
    assert!(line(54, boss_stage::ENEMY_LINE));
    let enemy_ticks = (ENEMY_LINE_MS + 100).div_ceil(18) as usize;
    let back = 54 + enemy_ticks;
    assert_eq!(
        back, 217,
        "the second level's step 258 from the trigger at 41"
    );
    assert_eq!(at(back - 1).camera_point, Some(boss_at));
    assert_eq!(at(back).camera_point, None);
    let claw_line = back + 48;
    assert_eq!(claw_line, 265);
    assert!(line(claw_line, boss_stage::CLAW_LINE));
    let banner = claw_line + (CLAW_LINE_MS + 100).div_ceil(18) as usize + 12;
    assert_eq!(banner, 380);
    assert_eq!(at(banner - 1).banner, None);
    assert_eq!(at(banner).banner, Some((320, -220)));
    assert!(plays(
        &at(banner).audio,
        boss_stage::BANNER_SOUND,
        SoundChannel::Effect
    ));
    // 127 drop steps of 3 px to y 160, the thud, 84 ticks, then 64 steps
    // of 10 px to x 960, where it is hidden.
    let thud = banner + 127;
    assert_eq!(at(thud - 1).banner, Some((320, 158)));
    assert_eq!(at(thud).banner, Some((320, 160)));
    let thud_audio = &at(thud).audio;
    assert!(plays(
        thud_audio,
        boss_stage::THUD_SOUND,
        SoundChannel::Effect
    ));
    assert!(plays(
        thud_audio,
        boss_stage::DEFEAT_SOUND,
        SoundChannel::Effect
    ));
    assert_eq!(at(thud + 84).banner, Some((330, 160)));
    let gone = thud + 84 + 63;
    assert_eq!(gone, 654);
    assert_eq!(at(gone - 1).banner, Some((950, 160)));
    assert_eq!(at(gone).banner, None);
    assert_eq!(at(gone).flag, fight::STAGING);
    assert!(at(gone).stager);
    // Flag 2 and the stager gone on the next tick; Claw stands on the one
    // after, 656 ticks after the trigger.
    assert_eq!(at(gone + 1).flag, fight::ON);
    assert!(!at(gone + 1).stager);
    assert_eq!(at(gone + 1).claw_state, state::FREEZE);
    assert_eq!(at(gone + 2).claw_state, state::STAND);
    // The flag went 0, 1, 2 at exactly those ticks, and Claw never moved
    // while frozen, though Right was held all along.
    for (k, l) in trace.iter().enumerate().skip(n) {
        let want = if k <= n {
            fight::NONE
        } else if k <= n + gone {
            fight::STAGING
        } else {
            fight::ON
        };
        assert_eq!(l.flag, want, "tick {}", l.tick);
        if (n + 1..=n + gone + 1).contains(&k) {
            assert_eq!((l.x, l.claw_state), (frozen_x, state::FREEZE));
            assert_eq!(
                l.meter,
                Some(BossMeter {
                    value: 100,
                    max: 100
                })
            );
        }
    }
    // Held Right moves him again.
    assert!(trace.last().unwrap().x > frozen_x);
    // The checkpoint fired once.
    assert_eq!(trace.iter().filter(|l| l.checkpoint.is_some()).count(), 1);
}

#[test]
fn the_camera_pans_to_the_boss_and_back_at_the_point_and_follow_speeds() {
    let (mut sim, boss, _) = arena_sim(150, 8000);
    let boss_x = sim.objects().get(boss).unwrap().x;
    while sim.boss().fight() == fight::NONE {
        sim.step(hold(RIGHT));
    }
    let mut xs = vec![sim.camera().current.0];
    for _ in 0..260 {
        sim.step(TickInput::default());
        xs.push(sim.camera().current.0);
    }
    // 400 px/s toward the boss: 7 px a tick, until it is there.
    let steps: Vec<i32> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(steps[..20].iter().all(|&d| d == 7), "{:?}", &steps[..20]);
    assert!(xs.contains(&boss_x));
    // Released (216 ticks after the start), back at 500 px/s: 9 px a tick.
    let back = xs.iter().rposition(|&x| x == boss_x).unwrap();
    assert_eq!(steps[back], -9, "{:?}", &steps[back - 2..back + 3]);
}

#[test]
fn the_trigger_accepts_claw_only_while_he_stands_or_walks() {
    let (sim, _, stager) = arena_sim(150, 8000);
    let y = sim.player().unwrap().y;
    // Falling into the box: no trigger until he lands.
    let mut s = sim.clone();
    s.teleport_player(STAGER_X, y - 60);
    s.set_player_state(state::FALL);
    let mut fired_at = None;
    for _ in 0..40 {
        s.step(TickInput::default());
        let l = look(&s, stager);
        if l.checkpoint.is_some() {
            assert!(matches!(l.physics, ptype::STAND | ptype::WALK), "{l:?}");
            fired_at = Some(l);
            break;
        }
        assert!(!matches!(l.physics, ptype::STAND | ptype::WALK), "{l:?}");
        assert_eq!(l.flag, fight::NONE);
    }
    let l = fired_at.expect("fired once he landed");
    assert_eq!(l.checkpoint.unwrap().1, y);
    // Crouching in the box: no trigger; standing up fires it.
    let mut s = sim.clone();
    for _ in 0..10 {
        s.step(hold(DOWN));
    }
    assert_eq!(s.player().unwrap().physics_type, ptype::DUCK);
    s.teleport_player(STAGER_X, y);
    for _ in 0..20 {
        s.step(hold(DOWN));
        assert_eq!(look(&s, stager).checkpoint, None);
    }
    s.step(TickInput::default());
    assert!(look(&s, stager).checkpoint.is_some());
    // Standing still in the box fires it at once.
    let mut s = sim.clone();
    s.teleport_player(STAGER_X, y);
    s.step(TickInput::default());
    let l = look(&s, stager);
    assert_eq!(l.physics, ptype::STAND);
    assert_eq!(l.checkpoint, Some((STAGER_X, y)));
}

#[test]
fn the_box_reaches_31_px_left_and_38_px_right_of_the_stager() {
    let (sim, _, stager) = arena_sim(150, 8000);
    let y = sim.player().unwrap().y;
    assert!(sim.claw().facing_right);
    for (x, inside) in [
        (STAGER_X - 32, false),
        (STAGER_X - 31, true),
        (STAGER_X + 38, true),
        (STAGER_X + 39, false),
    ] {
        let mut s = sim.clone();
        s.teleport_player(x, y);
        s.step(TickInput::default());
        assert_eq!(s.player().unwrap().x, x);
        assert_eq!(look(&s, stager).checkpoint.is_some(), inside, "x {x}");
    }
}

#[test]
fn a_jump_on_the_first_tick_in_the_box_skips_the_introduction() {
    let (sim, _, stager) = arena_sim(150, 8000);
    let y = sim.player().unwrap().y;
    // Arriving in the box as the door leaves Claw there, Right held, with
    // Jump pressed on that first tick: his logic runs before the collision
    // pass and he is already in the air.
    let mut s = sim.clone();
    s.step(hold(RIGHT));
    s.teleport_player(STAGER_X + 4, y);
    s.step(press(RIGHT | JUMP));
    assert_eq!(s.player().unwrap().physics_type, ptype::JUMP);
    for _ in 0..80 {
        s.step(hold(RIGHT));
        let l = look(&s, stager);
        assert_eq!(l.checkpoint, None);
        assert_eq!(l.flag, fight::NONE);
        assert_ne!(l.claw_state, state::FREEZE);
        assert_eq!((l.meter, l.banner, l.camera_point), (None, None, None));
    }
    // He landed right of the box and walks on; the trigger stays armed and
    // nothing of the introduction happened.
    assert!(s.player().unwrap().x > STAGER_X + 38);
    let o = s.objects().get(stager).unwrap();
    assert_eq!(o.state, stage_state::ARMED);
    assert_eq!(s.boss().checkpoint_time(), 0);
    assert_eq!(s.claw().checkpoint, None);
    // Walking back into the box later runs the whole introduction then.
    let mut back = s.clone();
    let mut fired = false;
    for _ in 0..200 {
        back.step(hold(LEFT));
        if look(&back, stager).checkpoint.is_some() {
            fired = true;
            break;
        }
    }
    assert!(fired);
    back.step(TickInput::default());
    assert_eq!(back.boss().fight(), fight::STAGING);

    // One tick late is too late: the trigger fired on the first tick.
    let mut s = sim.clone();
    s.step(hold(RIGHT));
    s.teleport_player(STAGER_X + 4, y);
    s.step(hold(RIGHT));
    assert!(look(&s, stager).checkpoint.is_some());
    s.step(press(RIGHT | JUMP));
    assert_eq!(s.boss().fight(), fight::STAGING);
    assert_eq!(s.player().unwrap().state, state::FREEZE);
}

#[test]
fn a_stager_below_claws_depth_freezes_him_before_his_own_logic() {
    let (mut sim, _, stager) = arena_sim(150, 1000);
    let mut last_x = sim.player().unwrap().x;
    loop {
        sim.step(hold(RIGHT));
        if look(&sim, stager).checkpoint.is_some() {
            break;
        }
        last_x = sim.player().unwrap().x;
    }
    let trig_x = sim.player().unwrap().x;
    assert!(trig_x > last_x);
    sim.step(hold(RIGHT));
    // The stager ran first in this tick: Claw did not move again.
    assert_eq!(sim.player().unwrap().state, state::FREEZE);
    assert_eq!(sim.player().unwrap().x, trig_x);
    // The banner moves the stager after Claw.
    for _ in 0..400 {
        sim.step(TickInput::default());
    }
    assert_eq!(sim.objects().get(stager).unwrap().z, 8000);
}

#[test]
fn the_meter_falls_with_the_boss_and_goes_the_tick_after_he_is_gone() {
    let (mut sim, boss, _) = arena_sim(150, 8000);
    while sim.boss().fight() == fight::NONE {
        sim.step(hold(RIGHT));
    }
    let meter = find(&sim, boss_stage::METER).expect("the meter");
    assert!(sim.objects().get(meter).unwrap().is_listed());
    let mut seen = Vec::new();
    for health in (0..=100).rev().step_by(5) {
        sim.with_world(|w| w.objects.get_mut(boss).unwrap().health = health);
        let snap = sim.step(TickInput::default());
        seen.push(snap.hud.boss_meter.unwrap());
    }
    for (i, m) in seen.iter().enumerate() {
        assert_eq!(m.max, 100, "the maximum stays the one copied");
        assert_eq!(m.value, 100 - 5 * i as i32);
    }
    // Health above the maximum moves only the value.
    sim.with_world(|w| w.objects.get_mut(boss).unwrap().health = 150);
    let snap = sim.step(TickInput::default());
    assert_eq!(
        snap.hud.boss_meter,
        Some(BossMeter {
            value: 150,
            max: 100
        })
    );
    // The boss's body goes: the bar stays for that tick and goes the next.
    sim.with_world(|w| w.set_current_boss(None));
    let snap = sim.step(TickInput::default());
    assert_eq!(snap.hud.boss_meter, None);
    assert!(sim.objects().get(meter).is_none());
}

#[test]
fn the_meter_stays_while_claw_walks_far_from_where_it_was_made() {
    let (mut sim, boss, _) = arena_sim(150, 8000);
    while sim.boss().fight() != fight::ON {
        sim.step(hold(RIGHT));
    }
    for _ in 0..400 {
        sim.step(hold(LEFT));
    }
    assert!(sim.player().unwrap().x < STAGER_X - 1500);
    assert_eq!(
        sim.hud().boss_meter,
        Some(BossMeter {
            value: sim.objects().get(boss).map_or(0, |b| b.health),
            max: 100
        })
    );
}

#[test]
fn freeze_and_release_at_the_boss_end() {
    let (mut sim, _, _) = arena_sim(600, 8000);
    // Walking right with an attack: held at once, as he is.
    sim.step(hold(RIGHT));
    sim.step(hold(RIGHT));
    let before = sim.player().unwrap().clone();
    assert!(sim.with_world(|w| w.freeze_claw_boss_end()));
    let o = sim.player().unwrap();
    assert_eq!(o.state, BOSS_END);
    assert_eq!(o.physics_type, before.physics_type);
    for _ in 0..30 {
        sim.step(hold(RIGHT));
        sim.step(press(JUMP));
    }
    assert_eq!(sim.player().unwrap().state, BOSS_END);
    assert_eq!(sim.player().unwrap().x, before.x);
    // Released: he stands, and a held direction moves him at once.
    assert!(sim.with_world(|w| w.release_claw_boss_end()));
    let o = sim.player().unwrap();
    assert_eq!((o.state, o.physics_type), (state::STAND, ptype::STAND));
    assert!(!sim.with_world(|w| w.release_claw_boss_end()), "only once");
    sim.step(hold(RIGHT));
    sim.step(hold(RIGHT));
    assert!(sim.player().unwrap().x > before.x);
}

#[test]
fn the_boss_freeze_ends_a_timed_freeze_only_with_the_fight() {
    let (mut sim, _, _) = arena_sim(600, 8000);
    sim.with_world(|w| assert!(w.freeze_claw(100)));
    sim.step(TickInput::default());
    assert_eq!(sim.player().unwrap().state, state::FREEZE);
    // Frozen already: the boss freeze takes over and drops the time.
    assert!(!sim.with_world(|w| w.freeze_claw_now()));
    sim.with_world(|w| w.set_boss_fight(fight::STAGING));
    idle(&mut sim, 20);
    assert_eq!(sim.player().unwrap().state, state::FREEZE);
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    sim.step(TickInput::default());
    assert_eq!(sim.player().unwrap().state, state::STAND);
}

#[test]
fn the_door_is_a_warp_that_stays() {
    let y = floor_y();
    let mut level = flat_level();
    level.start = (1000, y);
    let mut door = ObjectSpec::at(boss_stage::WARP, 1150, y, 0);
    door.record[55] = 5000;
    door.record[56] = y;
    level.objects.push(door);
    let mut sim = pressing_sim(level, SimConfig::default());
    idle(&mut sim, 5);
    let id = find(&sim, boss_stage::WARP).unwrap();
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.kind, LogicKind::Game(GameLogic::BossStage));
    assert_eq!(o.state, stage_state::ARMED);
    assert_eq!(o.z, boss_stage::WARP_Z);
    assert_eq!(o.type_flags, types::SPECIAL);
    assert_eq!(o.image_set, boss_stage::WARP_FALLBACK_IMAGE, "no art here");
    let mut taken = None;
    for k in 0..60 {
        sim.step(hold(RIGHT));
        if sim.player().unwrap().x >= 5000 {
            taken = Some(k);
            break;
        }
    }
    assert!(taken.is_some(), "the door moved Claw");
    assert_eq!(sim.player().unwrap().y, y);
    assert!(sim.objects().get(id).is_some(), "the door stays");
}

/// Where the door leads in [`door_arena`]: 4 px right of a stager placed
/// far from the start, as on the second level.
const FAR_STAGER_X: i32 = 8000;

/// The flat level with a door at x 1150 leading next to a stager (depth
/// 8000) at [`FAR_STAGER_X`], too far from the start to be awake; Claw
/// starts at x 1000.
fn door_arena() -> SimLevel {
    let y = floor_y();
    let mut level = flat_level();
    level.start = (1000, y);
    let mut door = ObjectSpec::at(boss_stage::WARP, 1150, y, 0);
    door.record[55] = FAR_STAGER_X + 4;
    door.record[56] = y;
    level.objects.push(door);
    level.objects.push(ObjectSpec::at(
        boss_stage::STAGER,
        FAR_STAGER_X,
        y + 47,
        8000,
    ));
    level
}

/// Right held, with Jump pressed too.
fn jump_right() -> TickInput {
    TickInput {
        held: Buttons(RIGHT | JUMP),
        pressed: Buttons(JUMP),
    }
}

#[test]
fn the_door_wakes_the_stager_for_claws_first_tick_at_the_target() {
    let mut sim = pressing_sim(door_arena(), SimConfig::default());
    idle(&mut sim, 5);
    let stager = find(&sim, boss_stage::STAGER).unwrap();
    assert_eq!(
        sim.objects().get(stager).unwrap().state,
        stage_state::INIT,
        "asleep while Claw is far"
    );
    let mut door_tick = None;
    for _ in 0..60 {
        sim.step(hold(RIGHT));
        if sim.player().unwrap().x >= FAR_STAGER_X {
            door_tick = Some(sim.tick());
            break;
        }
        assert!(!boss_music(sim.last_audio()));
    }
    assert!(door_tick.is_some(), "the door moved Claw");
    // The door's set-up ends its tick: the camera is on Claw, and the
    // stager woke, asked for the boss music and is armed.
    let o = sim.player().unwrap();
    assert_eq!(sim.camera().current, (o.x, o.y));
    assert!(boss_music(sim.last_audio()));
    assert_eq!(sim.objects().get(stager).unwrap().state, stage_state::ARMED);
    let at_door = sim.clone();

    // Claw's first tick at the target: walking in the box, the trigger
    // fires.
    sim.step(hold(RIGHT));
    let l = look(&sim, stager);
    assert_eq!(l.physics, ptype::WALK);
    assert!(l.checkpoint.is_some());
    assert!(!boss_music(&l.audio));

    // Jump pressed on that tick: the introduction never runs.
    let mut s = at_door.clone();
    s.step(jump_right());
    assert_eq!(s.player().unwrap().physics_type, ptype::JUMP);
    for _ in 0..80 {
        let l = look(&s, stager);
        assert_eq!((l.checkpoint, l.flag), (None, fight::NONE));
        assert_ne!(l.claw_state, state::FREEZE);
        s.step(hold(RIGHT));
    }
    assert!(s.player().unwrap().x > FAR_STAGER_X + 38);
    assert_eq!(s.objects().get(stager).unwrap().state, stage_state::ARMED);

    // Pressed on the next tick: too late, the introduction runs.
    let mut s = at_door.clone();
    s.step(hold(RIGHT));
    assert!(look(&s, stager).checkpoint.is_some());
    s.step(jump_right());
    assert_eq!(s.boss().fight(), fight::STAGING);
    assert_eq!(s.player().unwrap().state, state::FREEZE);
}

/// A special pickup giving powerup `code` for `ms`, touching Claw.
fn give_powerup(sim: &mut Sim, code: u32, ms: i32) {
    let (x, y) = {
        let o = sim.player().unwrap();
        (o.x, o.y)
    };
    let mut o = Object::new(0, x, y, 10, Arc::from("Toucher"), LogicKind::Inert);
    o.flags = flags::WORLD;
    o.type_flags = types::SPECIAL;
    o.attack_rect = Rect::new(-20, -20, 20, 20);
    PickupFields {
        kind: 53,
        points: code as i32,
        smarts: ms,
        ..PickupFields::default()
    }
    .store(&mut o);
    sim.spawn(o);
}

fn music(audio: &[AudioCue]) -> Vec<MusicTrack> {
    audio
        .iter()
        .filter_map(|c| match c {
            AudioCue::Music { track, .. } => Some(*track),
            _ => None,
        })
        .collect()
}

/// Gives the ghost powerup for 200 ms and runs until it is over; the
/// music cues of those ticks.
fn powerup_music(sim: &mut Sim) -> Vec<MusicTrack> {
    give_powerup(sim, powerup::GHOST, 200);
    let mut cues = Vec::new();
    sim.step(TickInput::default());
    assert_eq!(sim.stats().powerup_kind, powerup::GHOST);
    cues.extend(music(sim.last_audio()));
    for _ in 0..30 {
        sim.step(TickInput::default());
        cues.extend(music(sim.last_audio()));
    }
    assert_eq!(sim.stats().powerup_kind, 0, "the powerup ran out");
    cues
}

#[test]
fn a_powerup_leaves_the_boss_music_alone_while_the_flag_is_set() {
    // During the introduction and the fight: no music change at all.
    let (mut sim, _, _) = arena_sim(150, 8000);
    while sim.boss().fight() == fight::NONE {
        sim.step(hold(RIGHT));
    }
    assert_eq!(powerup_music(&mut sim), vec![]);
    while sim.boss().fight() != fight::ON {
        sim.step(TickInput::default());
    }
    idle(&mut sim, 2);
    assert_eq!(sim.player().unwrap().state, state::STAND);
    assert_eq!(powerup_music(&mut sim), vec![]);
    // With the flag at 0 (as after an intro skip): the powerup's tune, then
    // the level's.
    let (mut sim, _, _) = arena_sim(600, 8000);
    assert_eq!(sim.boss().fight(), fight::NONE);
    assert_eq!(
        powerup_music(&mut sim),
        vec![MusicTrack::Powerup, MusicTrack::Level]
    );
}
