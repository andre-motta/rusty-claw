//! The extra width of a wide display, which shows the level beyond the
//! logic view with the objects there asleep: what happens there is drawing
//! only, and must not look broken.
//!
//! - A body killed by a hit that the game removes as it falls out of the
//!   side of the logic view is reported in the snapshot of that tick, as
//!   the game had it, and a wide presentation draws its fall on until it
//!   leaves the presented area; the game plays the same with any view.
//! - A crate stack, a powder keg, a curse and a pickup the game has not
//!   woken yet are drawn as they look once woken: the column of crates the
//!   stack builds, the keg on the floor, no curse, the pickup with its
//!   glitter; they neither pop in, jump nor vanish when the game wakes
//!   them, and drawing them so changes nothing in the game.

mod common;

use claw_frame::{FallingBodies, Renderable, SimSnapshot, TickInput, ViewConfig};
use claw_sim::activation::ActivationConfig;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, DeathKind, EnemyState};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

/// A view presented at `w` x `h` (the frozen-edge default).
fn presented(w: u32, h: u32) -> ViewConfig {
    ViewConfig::default().with_render_size(w, h)
}

/// The flat floor of the common helpers with `objects`, every object awake
/// from the start, presented as `view`.
fn awake_sim(objects: Vec<ObjectSpec>, view: ViewConfig) -> Sim {
    let mut level = flat_level();
    level.objects = objects;
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    Sim::new(
        level,
        SimConfig {
            seed: 12,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            view,
            ..SimConfig::default()
        },
    )
}

/// A hit of Claw's that kills `id` (one health left), as his blow lands.
fn kill(sim: &mut Sim, id: ObjectId) {
    let mut a = Object::new(0, 0, 0, 0, "TestShot".into(), LogicKind::Inert);
    a.type_flags = types::PLAYER;
    a.damage = 5;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    let shot = sim.spawn(a);
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().health = 1;
        let (x, y) = w.objects.get(id).map(|o| (o.x, o.y)).unwrap();
        let a = w.objects.get_mut(shot).unwrap();
        a.x = x;
        a.y = y;
        a.attack_rect = Rect::new(-5, -5, 5, 5);
        w.claw.attack.serial += 1;
        claw_sim::logic::on_hit(w, id, shot);
    });
}

fn officer(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Officer))
        .map(|(id, _)| id)
        .expect("the officer")
}

/// A digest of the game: the random streams and every object.
fn game(sim: &Sim) -> String {
    let mut s = format!(
        "{} {} {:?}",
        sim.rng().state(),
        sim.sound_rng().state(),
        sim.stats()
    );
    for (id, o) in sim.objects().all() {
        s.push_str(&format!(
            "|{id:?} {} {} {} {} {} {} {}",
            o.x, o.y, o.z, o.state, o.frame, o.flags, o.draw_flags
        ));
    }
    s
}

#[test]
fn a_body_falling_out_of_the_side_falls_on_on_a_wide_display() {
    let wide = presented(1920, 1080);
    let mut rec = ObjectSpec::at("Officer", 3000, 900, 4000);
    rec.image_set = "LEVEL_OFFICER".to_string();
    // The same game at 640x480 alongside.
    let mut sims = [
        awake_sim(vec![rec.clone()], wide),
        awake_sim(vec![rec], ViewConfig::default()),
    ];
    for sim in &mut sims {
        sim.teleport_player(3250, 899);
        idle(sim, 2);
        let id = officer(sim);
        kill(sim, id);
        let o = sim.objects().get(id).unwrap();
        assert_eq!(EnemyState::from_id(o.state), Some(EnemyState::Dead));
    }
    let id = officer(&sims[0]);
    let serial = sims[0].objects().get(id).unwrap().serial;
    let mut bodies = FallingBodies::new();
    let mut removal = None;
    let mut last: Option<(i32, i32)> = None;
    let mut drawn_after = Vec::new();
    for tick in 0..300 {
        let mut snaps: Vec<SimSnapshot> = sims
            .iter_mut()
            .map(|s| s.step(TickInput::default()))
            .collect();
        assert_eq!(
            game(&sims[0]),
            game(&sims[1]),
            "the same game (tick {tick})"
        );
        assert_eq!(snaps[0].removed_bodies, snaps[1].removed_bodies);
        let view = sims[0].view().rect;
        match sims[0].objects().get(id) {
            Some(o) => {
                let d = enemy::data_of(o).unwrap();
                assert_eq!(d.death, DeathKind::Hit);
                assert!(snaps[0].removed_bodies.is_empty());
                last = Some((o.x, o.y));
            }
            None if removal.is_none() => {
                // The tick that removed it reports it, where the tick left
                // it: out of the left side, still inside the view's height.
                assert_eq!(snaps[0].removed_bodies.len(), 1, "tick {tick}");
                let b = &snaps[0].removed_bodies[0];
                assert_eq!(b.id, serial);
                assert_eq!(b.image_set_key, "LEVEL_OFFICER");
                assert_eq!(b.vx, -150.0, "away from Claw");
                assert!(b.vy > 0.0 && b.gravity > 0.0, "falling");
                assert_eq!(b.z, enemy::DEATH_DEPTH);
                assert!(!b.frames.is_empty());
                let (lx, ly) = last.unwrap();
                assert!((b.x - f64::from(lx)).abs() <= 24.0 && (b.y - f64::from(ly)).abs() <= 24.0);
                assert!(b.x < f64::from(view.left), "out of the left side");
                assert!(b.y < f64::from(view.bottom), "not out of the bottom");
                removal = Some(tick);
            }
            None => assert!(snaps[0].removed_bodies.is_empty()),
        }
        let narrow_before = snaps[1].renderables.len();
        bodies.absorb(&wide, &mut snaps[0]);
        let mut narrow = FallingBodies::new();
        narrow.absorb(&ViewConfig::default(), &mut snaps[1]);
        assert_eq!(
            snaps[1].renderables.len(),
            narrow_before,
            "640x480 draws nothing more"
        );
        if removal.is_some() {
            let drawn: Vec<&Renderable> = snaps[0]
                .renderables
                .iter()
                .filter(|r| r.id == serial)
                .collect();
            match drawn.first() {
                Some(r) => drawn_after.push((r.x, r.y)),
                None => break,
            }
        }
    }
    assert!(removal.is_some(), "the body was removed");
    assert!(
        drawn_after.len() >= 3,
        "drawn on for {} ticks",
        drawn_after.len()
    );
    // It falls on to the left, and goes once below the presented area.
    for w in drawn_after.windows(2) {
        assert!(w[1].0 < w[0].0 && w[1].1 > w[0].1, "{w:?}");
    }
    let (_, y) = *drawn_after.last().unwrap();
    let bottom = f64::from(sims[0].view().rect.bottom);
    assert!(
        y < bottom + 120.0 && y > bottom - 120.0,
        "leaves at the bottom ({y})"
    );
    assert!(bodies.is_empty());
    assert!(
        sims[0].objects().get(id).is_none(),
        "never back in the game"
    );
}

/// The objects a level places 600 to 900 px right of Claw's start, beyond
/// the small objects' activation window but inside a 32:9 presentation:
/// a powder keg in the air, a stack of three crates, a curse and a
/// treasure.
fn sleepers() -> SimLevel {
    let mut level = flat_level();
    let mut keg = ObjectSpec::at("PowderKeg", 1600, 800, 0);
    keg.image_set = "LEVEL_POWDERKEG".to_string();
    let mut stack = ObjectSpec::at("BackStackedCrates", 1700, 930, 0);
    stack.image_set = "LEVEL_CRATES".to_string();
    stack.frame = 1;
    stack.record[15] = 33;
    stack.record[35] = 33;
    stack.record[36] = 33;
    let mut curse = ObjectSpec::at("CursePowerup", 1800, 900, 0);
    curse.image_set = "GAME_CURSES_LIFE".to_string();
    let mut treasure = ObjectSpec::at("TreasurePowerup", 1900, 900, 0);
    treasure.image_set = "GAME_TREASURE_RINGS".to_string();
    level.objects = vec![keg, stack, curse, treasure];
    level
}

/// The sprites of `snap` in the sleepers' place, Claw's apart, as (image
/// set, frame, x, y), sorted.
fn sleeper_sprites(snap: &SimSnapshot) -> Vec<(String, i32, i64, i64)> {
    let mut v: Vec<_> = snap
        .renderables
        .iter()
        .filter(|r| (1550.0..=1950.0).contains(&r.x) && r.y < 960.0 && r.image_set_key != "CLAW")
        .map(|r| (r.image_set_key.clone(), r.frame, r.x as i64, r.y as i64))
        .collect();
    v.sort();
    v
}

#[test]
fn sleeping_crates_kegs_and_pickups_are_drawn_as_they_wake() {
    let view = presented(5120, 1440);
    let mut sim = Sim::new(sleepers(), SimConfig::for_view(view));
    let mut plain = Sim::new(sleepers(), SimConfig::default());
    idle(&mut sim, 5);
    idle(&mut plain, 5);
    assert!(
        sim.objects()
            .all()
            .filter(|(_, o)| o.x >= 1600)
            .all(|(_, o)| !o.is_listed()),
        "asleep"
    );
    let asleep = sleeper_sprites(&sim.snapshot());
    let keg = asleep
        .iter()
        .find(|s| s.0 == "LEVEL_POWDERKEG")
        .expect("the keg is drawn");
    assert!(
        keg.3 > 800,
        "on the floor, not where it was placed in the air"
    );
    let crates: Vec<i64> = asleep
        .iter()
        .filter(|s| s.0 == "LEVEL_CRATES")
        .map(|s| s.3)
        .collect();
    assert_eq!(crates, vec![850, 890, 930], "the stack's column");
    assert!(
        !asleep.iter().any(|s| s.0.starts_with("GAME_CURSES")),
        "no curse"
    );
    assert!(asleep.iter().any(|s| s.0 == "GAME_TREASURE_RINGS"));
    assert!(asleep.iter().any(|s| s.0 == "GAME_GLITTER"), "the glitter");
    // Claw walks up: everything wakes, and looks the same.
    for s in [&mut sim, &mut plain] {
        s.teleport_player(1750, 899);
        idle(s, 10);
    }
    assert!(
        sim.objects()
            .all()
            .filter(|(_, o)| o.x >= 1600 && o.x <= 1900)
            .all(|(_, o)| o.is_listed()),
        "awake"
    );
    assert_eq!(sleeper_sprites(&sim.snapshot()), asleep);
    // Drawing them so changed nothing in the game.
    assert_eq!(game(&sim), game(&plain));
}
