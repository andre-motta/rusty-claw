//! How Claw is drawn while a powerup runs, on a synthetic level:
//! see-through while invisible, a colour-cycling silhouette while
//! invulnerable, and normal again when the powerup ends, is replaced or he
//! dies. The play-test invincibility aid does not change his look.

mod common;

use std::sync::Arc;

use claw_frame::{DrawMode, Renderable, SimSnapshot};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::player::death::powerup;
use claw_sim::player::hit::PickupFields;
use claw_sim::player::look::{SILHOUETTE_STEP_MS, next_silhouette_color};
use claw_sim::tiles::TileMap;
use claw_sim::tuning::{PlayAids, TICK_MS};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// A flat floor with Claw standing on it.
fn level() -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600))
}

fn sim_with(aids: PlayAids) -> Sim {
    pressing_sim(
        level(),
        SimConfig {
            aids,
            ..SimConfig::default()
        },
    )
}

/// An object of kind `type_flags` with `f`, touching Claw.
fn toucher(sim: &mut Sim, type_flags: u32, f: PickupFields) -> ObjectId {
    let (x, y) = {
        let o = sim.player().unwrap();
        (o.x, o.y)
    };
    let mut o = Object::new(0, x, y, 10, Arc::from("Toucher"), LogicKind::Inert);
    o.flags = flags::WORLD;
    o.type_flags = type_flags;
    o.attack_rect = Rect::new(-20, -20, 20, 20);
    f.store(&mut o);
    sim.spawn(o)
}

/// Touches a powerup pickup of `code` lasting `ms` and steps the tick
/// that takes it; returns that tick's snapshot.
fn take(sim: &mut Sim, code: u32, ms: i32) -> SimSnapshot {
    toucher(
        sim,
        types::SPECIAL,
        PickupFields {
            kind: 53,
            points: code as i32,
            smarts: ms,
            ..PickupFields::default()
        },
    );
    sim.step(TickInput::default())
}

/// Claw's sprite in `snap`.
fn claw_sprite<'a>(sim: &Sim, snap: &'a SimSnapshot) -> &'a Renderable {
    let serial = sim.player().unwrap().serial;
    snap.renderables
        .iter()
        .find(|r| r.id == serial)
        .expect("Claw is drawn")
}

fn look(sim: &mut Sim) -> (DrawMode, Option<u8>) {
    let snap = sim.step(TickInput::default());
    let r = claw_sprite(sim, &snap);
    (r.draw_mode, r.draw_color)
}

const NORMAL: (DrawMode, Option<u8>) = (DrawMode::NORMAL, None);
const GHOST: (DrawMode, Option<u8>) = (DrawMode::GHOST, None);

fn silhouette(c: u8) -> (DrawMode, Option<u8>) {
    (DrawMode::SHADOW, Some(c))
}

#[test]
fn invisible_claw_is_see_through_until_the_timer_ends() {
    let mut sim = sim_with(PlayAids::NONE);
    assert_eq!(look(&mut sim), NORMAL);
    // The tick that takes the powerup still shows him as before; the look
    // follows from the next tick.
    let snap = take(&mut sim, powerup::GHOST, 500);
    assert_eq!(sim.stats().powerup_kind, powerup::GHOST);
    let r = claw_sprite(&sim, &snap);
    assert_eq!((r.draw_mode, r.draw_color), NORMAL);
    let mut ticks = 0;
    while sim.stats().powerup_kind == powerup::GHOST {
        let l = look(&mut sim);
        if sim.stats().powerup_kind == powerup::GHOST {
            assert_eq!(l, GHOST, "tick {ticks}");
            ticks += 1;
        } else {
            assert_eq!(l, NORMAL, "the tick the timer runs out");
        }
    }
    // 500 ms of 18 ms ticks: the pickup tick takes nothing off, the 28th
    // tick after it ends the powerup.
    assert_eq!(ticks, 500 / TICK_MS as usize);
    assert_eq!(look(&mut sim), NORMAL);
    // Only Claw changes: nothing else in the scene took the look.
    let snap = take(&mut sim, powerup::GHOST, 500);
    let snap2 = sim.step(TickInput::default());
    let serial = sim.player().unwrap().serial;
    assert!(
        snap.renderables
            .iter()
            .chain(&snap2.renderables)
            .filter(|r| r.id != serial)
            .all(|r| r.draw_mode == DrawMode::NORMAL && r.draw_color.is_none())
    );
}

#[test]
fn invulnerable_claw_is_a_silhouette_cycling_colours_every_100_ms() {
    let mut sim = sim_with(PlayAids::NONE);
    take(&mut sim, powerup::VADER, 2000);
    let mut seen = Vec::new();
    while sim.stats().powerup_kind == powerup::VADER {
        let l = look(&mut sim);
        if sim.stats().powerup_kind == powerup::VADER {
            seen.push(l);
        } else {
            assert_eq!(l, NORMAL, "back to normal when the timer ends");
        }
    }
    // The first invulnerable tick steps at once; then the colour holds for
    // as many ticks as it takes the 100 ms step timer to run out at 18 ms a
    // tick (six).
    let per_step = SILHOUETTE_STEP_MS.div_ceil(TICK_MS) as usize;
    assert_eq!(per_step, 6);
    let mut want = Vec::new();
    let mut c = 0;
    while want.len() < seen.len() {
        c = next_silhouette_color(c);
        want.extend(std::iter::repeat_n(silhouette(c), per_step));
    }
    want.truncate(seen.len());
    assert_eq!(seen, want);
    assert_eq!(
        want.iter()
            .step_by(per_step)
            .take(6)
            .map(|l| l.1.unwrap())
            .collect::<Vec<_>>(),
        [0x65, 0x34, 0x35, 0x36, 0x37, 0x65]
    );
    // A later invulnerability goes on from the colour and timer the last
    // one stopped at.
    let last = *seen.last().unwrap();
    let left = per_step - seen.len() % per_step;
    take(&mut sim, powerup::VADER, 2000);
    for _ in 0..left {
        assert_eq!(look(&mut sim), last);
    }
    let next = next_silhouette_color(last.1.unwrap());
    assert_eq!(look(&mut sim), silhouette(next));
}

#[test]
fn a_new_powerup_or_death_ends_the_look() {
    let mut sim = sim_with(PlayAids::NONE);
    take(&mut sim, powerup::GHOST, 30_000);
    assert_eq!(look(&mut sim), GHOST);
    // Invisibility replaced by invulnerability: normal on the tick that
    // takes it, the silhouette from the next.
    let snap = take(&mut sim, powerup::VADER, 30_000);
    let r = claw_sprite(&sim, &snap);
    assert_eq!((r.draw_mode, r.draw_color), NORMAL);
    assert_eq!(look(&mut sim).0, DrawMode::SHADOW);
    // A sword replaces it: normal again and stays so.
    let snap = take(&mut sim, powerup::FIRE_SWORD, 30_000);
    let r = claw_sprite(&sim, &snap);
    assert_eq!((r.draw_mode, r.draw_color), NORMAL);
    for _ in 0..10 {
        assert_eq!(look(&mut sim), NORMAL);
    }
    // Death drops the powerup and the look.
    take(&mut sim, powerup::GHOST, 30_000);
    assert_eq!(look(&mut sim), GHOST);
    sim.with_world(|w| w.claw.stats.health = 1);
    let id = toucher(&mut sim, types::ENEMY, PickupFields::default());
    sim.with_world(|w| w.objects.get_mut(id).unwrap().damage = 5);
    let snap = sim.step(TickInput::default());
    assert_eq!(sim.stats().powerup_kind, 0);
    let r = claw_sprite(&sim, &snap);
    assert_eq!((r.draw_mode, r.draw_color), NORMAL);
}

#[test]
fn the_invincibility_aid_leaves_the_look_alone() {
    let mut sim = sim_with(PlayAids {
        invincible: true,
        ..PlayAids::NONE
    });
    let id = toucher(&mut sim, types::ENEMY, PickupFields::default());
    sim.with_world(|w| w.objects.get_mut(id).unwrap().damage = 10);
    for _ in 0..120 {
        assert_eq!(look(&mut sim), NORMAL);
    }
    assert_eq!(sim.stats().powerup_kind, 0);
    // With the aid on, invulnerability still shows its own look.
    sim.with_world(|w| w.destroy(id));
    take(&mut sim, powerup::VADER, 1000);
    assert_eq!(look(&mut sim), silhouette(0x65));
}
