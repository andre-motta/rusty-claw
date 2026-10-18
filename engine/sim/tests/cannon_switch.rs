//! The props of Gabriel's fight in a synthetic room, with Gabriel there
//! asleep (no fight flag): the cannon switch that flips on his lever
//! order, the button that only Claw's own blows press, and his cannon,
//! which fires along the dock or, turned up by the button, straight up,
//! turns back down after an upward shot, and keeps its direction of fire
//! through its reset.

use std::sync::Arc;

use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::cannon_switch::{self, button, cannon};
use claw_sim::logic::enemy::{EnemyState, gabriel};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, draw_flags, flags, types};
use claw_sim::tiles::TileMap;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};

/// Where the cannon is, as on his level (its record mirrored).
const CANNON_AT: (i32, i32) = (3203, 5184);
const SWITCH_AT: (i32, i32) = (3277, 5022);
const BUTTON_AT: (i32, i32) = (2961, 4779);
const GABRIEL_AT: (i32, i32) = (3223, 5068);

fn anim(key: &str, frames: &[(i32, i32)], once: bool, event_on_first: bool) -> Arc<Animation> {
    let mut a = if once {
        Animation::once(key, frames)
    } else {
        Animation::looping(key, frames)
    };
    if event_on_first && let Some(f) = a.frames.first_mut() {
        f.event = 1;
    }
    Arc::new(a)
}

fn anims() -> Vec<Arc<Animation>> {
    let turn: Vec<(i32, i32)> = (1..=19).map(|i| (i, 25)).collect();
    let back: Vec<(i32, i32)> = (1..=19).rev().map(|i| (i, 25)).collect();
    vec![
        anim(cannon::FIRE_ANIM, &[(20, 100), (21, 100)], true, true),
        anim(cannon::VERT_FIRE_ANIM, &[(30, 100), (31, 100)], true, true),
        anim(cannon::RISE_ANIM, &turn, true, false),
        anim(cannon::REST_ANIM, &back, true, false),
        anim(
            cannon_switch::SWITCH_ANIM,
            &[(1, 100), (2, 100), (3, 100), (2, 100)],
            true,
            false,
        ),
        anim(button::IDLE_ANIM, &[(1, 200), (3, 200)], false, false),
        anim(gabriel::IDLE1_ANIM, &[(100, 2000)], true, false),
        anim(gabriel::IDLE2_ANIM, &[(150, 1500)], true, false),
    ]
}

/// The room, in 20 px tiles: the dock floor at y 5280 and Gabriel's deck,
/// a one-way floor at y 5100 from x 3180 eastwards; his props at their
/// places in his level's order (button, switch, cannon), then Gabriel.
fn room(with_gabriel: bool) -> SimLevel {
    let (cols, rows) = (220, 270);
    let mut lines: Vec<String> = vec![".".repeat(cols); rows];
    lines[255].replace_range(159..cols, &"=".repeat(cols - 159));
    for line in lines.iter_mut().skip(264) {
        *line = "#".repeat(cols);
    }
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(20, &lines), (2800, 5219));
    level.number = 8;
    for a in anims() {
        level.animations.insert(a.key.clone(), a);
    }
    // The button's three lamps.
    level
        .images
        .insert(button::IMAGE_SET.into(), ImageFrames::range(1, 3));
    let b = ObjectSpec::at(button::NAMES[0], BUTTON_AT.0, BUTTON_AT.1, 1000);
    let s = ObjectSpec::at(cannon_switch::NAMES[0], SWITCH_AT.0, SWITCH_AT.1, 1000);
    let mut c = ObjectSpec::at(cannon::NAMES[0], CANNON_AT.0, CANNON_AT.1, 1000);
    c.record[11] = draw_flags::MIRROR as i32;
    level.objects = vec![b, s, c];
    if with_gabriel {
        let mut g = ObjectSpec::at("Gabriel", GABRIEL_AT.0, GABRIEL_AT.1, 0);
        g.image_set = gabriel::IMAGE_SET.into();
        level.objects.push(g);
    }
    level
}

fn arena(with_gabriel: bool) -> Sim {
    let level = room(with_gabriel);
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed: 3,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(2800, 5219);
    idle(&mut sim, 2);
    sim
}

fn idle(sim: &mut Sim, ticks: usize) {
    for _ in 0..ticks {
        sim.step(TickInput::default());
    }
}

fn find(sim: &Sim, kind: GameLogic) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(kind))
        .map(|(id, _)| id)
        .expect("the prop exists")
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn link(sim: &mut Sim) -> gabriel::GabrielLink {
    sim.with_world(|w| w.gabriel_link().cloned())
        .expect("his link")
}

fn set_link(sim: &mut Sim, f: impl FnOnce(&mut gabriel::GabrielLink)) {
    sim.with_world(|w| f(w.gabriel_link_mut().expect("his link")));
}

fn cannon_data(sim: &Sim) -> cannon::CannonData {
    match &obj(sim, find(sim, GameLogic::GabrielCannon)).data {
        ObjectData::GabrielCannon(d) => d.clone(),
        _ => panic!("the cannon's data"),
    }
}

fn cannon_state(sim: &Sim) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, find(sim, GameLogic::GabrielCannon)).state)
}

fn balls(sim: &Sim) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic::cannon::BALL_LOGIC && o.flags & flags::DESTROY == 0)
        .map(|(id, _)| id)
        .collect()
}

fn sounds(sim: &Sim) -> Vec<String> {
    sim.last_audio()
        .iter()
        .filter_map(|c| match c {
            AudioCue::Play(s) => Some(s.key.clone()),
            _ => None,
        })
        .collect()
}

/// Ticks until `f` holds, at most `limit`.
fn until(sim: &mut Sim, limit: usize, mut f: impl FnMut(&Sim) -> bool) -> usize {
    for k in 1..=limit {
        sim.step(TickInput::default());
        if f(sim) {
            return k;
        }
    }
    panic!("not within {limit} ticks");
}

/// A blow of Claw's on the button.
fn strike_button(sim: &mut Sim) {
    let b = find(sim, GameLogic::CannonButton);
    sim.with_world(|w| {
        let claw = w.claw_id.expect("Claw");
        w.claw.attack.serial += 1;
        logic::on_hit(w, b, claw);
    });
}

// ----------------------------------------------------------------------
// The switch

#[test]
fn the_switch_flips_on_his_lever_order_and_the_cannon_fires_it() {
    let mut sim = arena(true);
    let s = find(&sim, GameLogic::CannonSwitch);
    assert_eq!(obj(&sim, s).state, cannon_switch::state::IDLE);
    assert!(obj(&sim, s).flags & flags::NO_COLLIDE != 0);
    // His reset is acknowledged by the cannon's first wait.
    assert!(!link(&mut sim).wait_for_cannon);
    set_link(&mut sim, |l| l.fire_order = true);
    sim.step(TickInput::default());
    // The switch saw the order before the cannon fired and cleared it.
    assert_eq!(obj(&sim, s).state, cannon_switch::state::SWITCH);
    assert!(sounds(&sim).contains(&cannon_switch::SWITCH_SOUND.to_string()));
    assert_eq!(cannon_state(&sim), Some(EnemyState::Fire));
    assert!(!link(&mut sim).fire_order);
    let n = until(&mut sim, 60, |s2| {
        obj(s2, s).state == cannon_switch::state::IDLE
    });
    assert_eq!(n as u32, cannon_switch::SWITCH_TICKS);
}

#[test]
fn without_gabriel_nothing_moves() {
    let mut sim = arena(false);
    let c = find(&sim, GameLogic::GabrielCannon);
    let before = (obj(&sim, c).state, obj(&sim, c).frame);
    idle(&mut sim, 50);
    assert_eq!((obj(&sim, c).state, obj(&sim, c).frame), before);
    assert!(balls(&sim).is_empty());
}

// ----------------------------------------------------------------------
// The button

#[test]
fn only_claws_blows_press_the_button() {
    let mut sim = arena(true);
    let b = find(&sim, GameLogic::CannonButton);
    let o = obj(&sim, b);
    assert_eq!(o.type_flags, button::TYPE);
    assert_eq!(o.hit_type_flags, button::HIT_MASK);
    assert_eq!(o.hit_rect, button::HIT_RECT);
    strike_button(&mut sim);
    assert!(link(&mut sim).button);
    // Its press sound plays once, from the next tick its pressed lamp.
    let pressed_sound = sim.with_world(|w| match &w.objects.get(b).unwrap().data {
        ObjectData::CannonButton(d) => d.latch,
        _ => false,
    });
    assert!(pressed_sound);
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, b).frame, button::PRESSED_FRAME);
    // Held pressed for as long as the flag stays set.
    idle(&mut sim, 30);
    assert_eq!(obj(&sim, b).frame, button::PRESSED_FRAME);
}

#[test]
fn once_he_is_defeated_the_button_blinks_and_ignores_blows() {
    let mut sim = arena(true);
    let b = find(&sim, GameLogic::CannonButton);
    set_link(&mut sim, |l| l.defeated = true);
    strike_button(&mut sim);
    assert!(!link(&mut sim).button);
    let mut frames = Vec::new();
    for _ in 0..30 {
        sim.step(TickInput::default());
        frames.push(obj(&sim, b).frame);
    }
    assert!(frames.contains(&1) && frames.contains(&3), "{frames:?}");
}

// ----------------------------------------------------------------------
// The cannon

#[test]
fn the_cannon_is_built_facing_left_and_fires_along_the_dock() {
    let mut sim = arena(true);
    let c = find(&sim, GameLogic::GabrielCannon);
    let o = obj(&sim, c);
    assert_eq!(cannon_state(&sim), Some(EnemyState::Home));
    assert_eq!((o.speed_x, o.speed_y), (-cannon::SPEED, 0));
    assert_eq!(o.frame, cannon::REST_FRAME);
    set_link(&mut sim, |l| l.fire_order = true);
    sim.step(TickInput::default());
    assert_eq!(cannon_state(&sim), Some(EnemyState::Fire));
    // The ball leaves on the next tick, from under the bow.
    until(&mut sim, 5, |s| !balls(s).is_empty());
    let ball = obj(&sim, balls(&sim)[0]);
    let (along, _) = cannon::muzzles(true);
    assert_eq!(
        (ball.x, ball.y),
        (CANNON_AT.0 + along.0, CANNON_AT.1 + along.1)
    );
    assert_eq!((ball.speed_x, ball.speed_y), (-cannon::SPEED, 0));
    assert_eq!(ball.type_flags, types::ENEMY_SHOT);
    until(&mut sim, 30, |s| cannon_state(s) == Some(EnemyState::Home));
    assert!(cannon_data(&sim).fired && !cannon_data(&sim).raised);
}

#[test]
fn the_button_turns_it_up_and_its_next_shot_goes_up_then_it_turns_down() {
    let mut sim = arena(true);
    strike_button(&mut sim);
    sim.step(TickInput::default());
    assert_eq!(cannon_state(&sim), Some(EnemyState::Rotate));
    assert!(sounds(&sim).contains(&cannon::RISE_SOUND.to_string()));
    let n = until(&mut sim, 60, |s| cannon_state(s) == Some(EnemyState::Home));
    assert_eq!(n as u32, cannon::ROTATE_TICKS);
    let c = find(&sim, GameLogic::GabrielCannon);
    assert_eq!(obj(&sim, c).frame, cannon::RAISED_FRAME);
    assert_eq!(
        (obj(&sim, c).speed_x, obj(&sim, c).speed_y),
        cannon::ball_velocity(true)
    );
    // Raised, it waits for his order.
    idle(&mut sim, 20);
    assert_eq!(cannon_state(&sim), Some(EnemyState::Home));
    set_link(&mut sim, |l| l.fire_order = true);
    sim.step(TickInput::default());
    assert_eq!(cannon_state(&sim), Some(EnemyState::VertFiring));
    until(&mut sim, 5, |s| !balls(s).is_empty());
    let ball = obj(&sim, balls(&sim)[0]);
    let (_, up) = cannon::muzzles(true);
    assert_eq!((ball.x, ball.y), (CANNON_AT.0 + up.0, CANNON_AT.1 + up.1));
    assert_eq!((ball.speed_x, ball.speed_y), (0, -cannon::SPEED));
    // The upward ball flies on through the ground above it.
    let y0 = ball.y;
    let id = balls(&sim)[0];
    idle(&mut sim, 5);
    assert!(obj(&sim, id).y < y0 - 30, "it rises");
    // Home again, then it turns down and lets the button go.
    until(&mut sim, 40, |s| {
        cannon_state(s) == Some(EnemyState::Rotate) && !cannon_data(s).raised
    });
    assert!(!link(&mut sim).button);
    assert!(sounds(&sim).contains(&cannon::DROP_SOUND.to_string()));
    let n = until(&mut sim, 60, |s| cannon_state(s) == Some(EnemyState::Home));
    assert_eq!(n as u32, cannon::ROTATE_TICKS);
    assert_eq!(obj(&sim, c).frame, 1, "barrel down");
    set_link(&mut sim, |l| l.fire_order = true);
    sim.step(TickInput::default());
    assert_eq!(
        cannon_state(&sim),
        Some(EnemyState::Fire),
        "along the dock again"
    );
}

#[test]
fn its_reset_keeps_it_firing_upwards() {
    let mut sim = arena(true);
    strike_button(&mut sim);
    until(&mut sim, 60, |s| {
        cannon_state(s) == Some(EnemyState::Home) && cannon_data(s).raised
    });
    // His start of fight again (Claw's respawn, a late introduction): the
    // cannon resets, the button goes, the barrel shows down.
    set_link(&mut sim, |l| l.wait_for_cannon = true);
    sim.step(TickInput::default());
    let c = find(&sim, GameLogic::GabrielCannon);
    assert_eq!(obj(&sim, c).frame, cannon::REST_FRAME);
    let l = link(&mut sim);
    assert!(!l.wait_for_cannon && !l.button && !l.fire_order);
    assert!(!cannon_data(&sim).raised);
    // Yet every order after it fires upwards, and it never turns down.
    for _ in 0..2 {
        set_link(&mut sim, |l| l.fire_order = true);
        sim.step(TickInput::default());
        assert_eq!(cannon_state(&sim), Some(EnemyState::VertFiring));
        until(&mut sim, 40, |s| cannon_state(s) == Some(EnemyState::Home));
        idle(&mut sim, 5);
        assert_eq!(cannon_state(&sim), Some(EnemyState::Home), "no turn");
    }
}

#[test]
fn an_upward_ball_meets_gabriel_as_a_cannon_hit() {
    let mut sim = arena(true);
    let g = find(&sim, GameLogic::Gabriel);
    let ball = sim.with_world(|w| {
        let mut o = logic::new_object(logic::cannon::BALL_LOGIC, 0, 0, 0);
        o.kind = LogicKind::Inert;
        o.type_flags = types::ENEMY_SHOT;
        o.attack_rect = Rect::new(-5, -5, 5, 5);
        o.flags |= flags::NO_COLLIDE;
        w.spawn(o)
    });
    sim.with_world(|w| logic::on_hit(w, g, ball));
    assert_eq!(
        EnemyState::from_id(obj(&sim, g).state),
        Some(EnemyState::CannonHit)
    );
}
