//! Synthetic logics that prove the framework: a spawner, a mover, a hitter
//! and a data holder, bound to families for the length of a test through
//! [`install`], and run by real simulation ticks.

use std::cell::RefCell;
use std::collections::BTreeMap;

use claw_frame::{AudioCue, Buttons, GameEvent, SoundChannel, SoundCue, TickInput};

use super::effect::EffectKind;
use super::pickup::PickupData;
use super::projectile::ProjectileSpec;
use super::*;
use crate::anim::{AnimFrame, Animation, FRAME_SOUND_IN_VIEW};
use crate::container::CollisionEvent;
use crate::level::{ImageFrames, ObjectSpec, SimLevel};
use crate::object::{Rect, engine_state, types};
use crate::physics::ptype;
use crate::sim::{Sim, SimConfig};
use crate::tiles::TileMap;
use crate::trace::{self, TraceRow};

/// Replacement handlers of one family.
#[derive(Clone, Copy)]
pub(super) struct Hooks {
    pub tick: fn(&mut World, ObjectId),
    pub on_hit: fn(&mut World, ObjectId, ObjectId),
    pub on_attack: fn(&mut World, ObjectId, ObjectId),
}

thread_local! {
    static HOOKS: RefCell<BTreeMap<GameLogic, Hooks>> = const { RefCell::new(BTreeMap::new()) };
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// The test handlers bound to `g` on this thread.
pub(super) fn hooks(g: GameLogic) -> Option<Hooks> {
    HOOKS.with(|h| h.borrow().get(&g).copied())
}

fn install(g: GameLogic, tick: fn(&mut World, ObjectId)) {
    install_all(g, tick, |_, _, _| {}, |_, _, _| {});
}

fn install_all(
    g: GameLogic,
    tick: fn(&mut World, ObjectId),
    on_hit: fn(&mut World, ObjectId, ObjectId),
    on_attack: fn(&mut World, ObjectId, ObjectId),
) {
    HOOKS.with(|h| {
        h.borrow_mut().insert(
            g,
            Hooks {
                tick,
                on_hit,
                on_attack,
            },
        )
    });
}

fn log(line: String) {
    LOG.with(|l| l.borrow_mut().push(line));
}

fn take_log() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

/// A long flat floor (feet rest on y = 959) with `objects` placed on it;
/// Claw starts on it at x = 1000, y = 899. Each object's id is its x.
/// Objects of the small activation window must stay within 288 px of
/// y = 600 to be live from the start.
fn level(objects: &[(&str, i32, i32, i32)]) -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    for &(name, x, y, z) in objects {
        let mut spec = ObjectSpec::at(name, x, y, z);
        spec.record[0] = x;
        l.objects.push(spec);
    }
    l
}

fn find(sim: &Sim, logic: &str) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .collect()
}

fn idle(sim: &mut Sim, n: usize) {
    for _ in 0..n {
        sim.step(TickInput::default());
    }
}

// ------------------------------------------------------------- spawning

fn spawner(w: &mut World, id: ObjectId) {
    let (x, y, z, state) = {
        let o = w.objects.get(id).unwrap();
        (o.x, o.y, o.z, o.state)
    };
    log(format!("spawner {}", w.tick));
    if state != 0 {
        return;
    }
    w.objects.get_mut(id).unwrap().state = 1;
    let mut ids = Vec::new();
    for i in 0..3 {
        let mut child = new_object("HitBurst", x + i, y, z);
        child.id = 100 + i;
        if i == 2 {
            child.flags |= flags::RUN_AT_CREATION;
        }
        ids.push(w.spawn(child));
    }
    // A spawn asked to go before it is listed never runs.
    let doomed = w.spawn(new_object("HitBurst", x, y, z));
    w.destroy(doomed);
    assert!(w.objects.get(ids[0]).is_some(), "exists at once");
    assert!(!w.objects.get(ids[0]).unwrap().is_listed(), "listed later");
    log(format!("spawned {ids:?}"));
}

fn child(w: &mut World, id: ObjectId) {
    let o = w.objects.get(id).unwrap();
    log(format!("child {} at {}", o.id, w.tick));
}

#[test]
fn spawns_join_the_list_at_the_end_of_the_pass_in_order() {
    install(GameLogic::Decor, spawner);
    install(GameLogic::Effect, child);
    let mut sim = Sim::new(
        level(&[("FrontCandy", 1100, 800, 500)]),
        SimConfig::default(),
    );
    let spawner_id = find(&sim, "FrontCandy")[0];
    // The spawner's first call runs while the level loads.
    let log0 = take_log();
    let ids: Vec<ObjectId> = find(&sim, "HitBurst");
    assert_eq!(ids.len(), 3, "the destroyed spawn is gone: {ids:?}");
    assert_eq!(
        log0,
        vec![
            "spawner 0".to_string(),
            format!("spawned {ids:?}"),
            // Only the child asking to run at creation runs then.
            "child 102 at 0".to_string(),
        ]
    );
    // Equal depth keeps spawn order, after the spawner.
    let listed = sim.objects().listed();
    let pos = |id| listed.iter().position(|&x| x == id).unwrap();
    assert!(pos(spawner_id) < pos(ids[0]));
    assert!(pos(ids[0]) < pos(ids[1]) && pos(ids[1]) < pos(ids[2]));
    sim.step(TickInput::default());
    assert_eq!(
        take_log(),
        vec![
            "spawner 0",
            "child 100 at 0",
            "child 101 at 0",
            "child 102 at 0"
        ]
    );
}

#[test]
fn cross_family_stubs_leave_placeholders_that_record_the_request() {
    let mut sim = Sim::new(level(&[]), SimConfig::default());
    let spec = ProjectileSpec {
        logic: "PistolBullet",
        x: 1050,
        y: 880,
        z: 4000,
        vx: 500,
        damage: 8,
        owner: Some(0),
        ..ProjectileSpec::default()
    };
    let (p, e, d, curse) = sim.with_world(|w| {
        let p = projectile::spawn(w, spec).unwrap();
        let e = effect::spawn(w, EffectKind::Splash, 990, 900).unwrap();
        let d = pickup::spawn_drop(w, 1000, 900, 33).unwrap();
        let curse = pickup::curse_effect(w, 1);
        sound::play(w, "GAME_TEST", SoundChannel::Interface);
        (p, e, d, curse)
    });
    assert!(!curse);
    assert_eq!(sim.treasure_census(), [0; 9]);
    // Projectiles and effects, placeholders or filled in, record the
    // request they were spawned with.
    match &sim.objects().get(p).unwrap().data {
        ObjectData::Projectile(data) => assert_eq!(data.spec, Some(spec)),
        other => panic!("projectile data expected, got {other:?}"),
    }
    match &sim.objects().get(e).unwrap().data {
        ObjectData::Effect(data) => assert_eq!(data.request, Some(EffectKind::Splash)),
        other => panic!("effect data expected, got {other:?}"),
    }
    // Drops are real: a bouncing goodie that remembers its code.
    let o = sim.objects().get(d).unwrap();
    assert!(o.is_listed());
    assert_eq!(o.kind, LogicKind::Game(GameLogic::Pickup));
    assert_eq!(&*o.logic, pickup::BOUNCING_GOODIE);
    assert!(matches!(
        o.data,
        ObjectData::Pickup(PickupData {
            drop_code: Some(33),
            ..
        })
    ));
    // The sound goes out with the next tick, centred.
    let snap = sim.step(TickInput::default());
    assert_eq!(
        snap.audio,
        vec![AudioCue::Play(SoundCue {
            key: "GAME_TEST".into(),
            channel: SoundChannel::Interface,
            ..SoundCue::default()
        })]
    );
    assert!(sim.step(TickInput::default()).audio.is_empty());
}

// ------------------------------------------------------- moving and data

fn mover(w: &mut World, id: ObjectId) {
    let (x, y) = {
        let o = w.objects.get_mut(id).unwrap();
        o.physics_type = ptype::DIRECT;
        o.type_flags = 0x80;
        o.hit_rect = Rect::new(-32, -8, 32, 8);
        o.platform_move = (3, 0);
        (o.x, o.y)
    };
    w.physics(id, x + 3, y, 0);
}

fn holder(w: &mut World, id: ObjectId) {
    let o = w.objects.get_mut(id).unwrap();
    let n = match o.data {
        ObjectData::Pickup(PickupData {
            drop_code: Some(n), ..
        }) => n + 1,
        _ => 1,
    };
    o.data = ObjectData::Pickup(PickupData {
        drop_code: Some(n),
        ..PickupData::default()
    });
    if n % 10 == 0 {
        w.event(GameEvent::Message {
            id: n as u32,
            ms: 100,
        });
    }
}

#[test]
fn logics_move_objects_and_keep_their_data() {
    install(GameLogic::Platform, mover);
    install(GameLogic::Pickup, holder);
    let mut sim = Sim::new(
        level(&[
            ("Elevator", 900, 700, 10),
            ("TreasurePowerup", 1050, 880, 10),
        ]),
        SimConfig::default(),
    );
    let (lift, gem) = (find(&sim, "Elevator")[0], find(&sim, "TreasurePowerup")[0]);
    let mut events = Vec::new();
    for _ in 0..20 {
        events.extend(sim.step(TickInput::default()).events);
    }
    // 21 calls: the first while the level loads, then one per tick.
    assert_eq!(sim.objects().get(lift).unwrap().x, 963);
    let p = crate::sim::platforms(sim.objects());
    assert_eq!((p[0].id, p[0].move_x, p[0].move_y), (lift, 3, 0));
    assert_eq!(
        sim.objects().get(gem).unwrap().data,
        ObjectData::Pickup(PickupData {
            drop_code: Some(21),
            ..PickupData::default()
        })
    );
    assert_eq!(
        events,
        vec![
            GameEvent::Message { id: 10, ms: 100 },
            GameEvent::Message { id: 20, ms: 100 }
        ]
    );
}

fn stopper(w: &mut World, id: ObjectId) {
    let moved = w.objects.get(id).unwrap().x < 906;
    if moved {
        mover(w, id);
    }
}

#[test]
fn a_platform_that_stops_carries_no_one() {
    install(GameLogic::Platform, stopper);
    let mut sim = Sim::new(level(&[("Elevator", 900, 700, 10)]), SimConfig::default());
    let lift = find(&sim, "Elevator")[0];
    // It moved once while the level loaded, and moves once more.
    assert_eq!(sim.objects().get(lift).unwrap().x, 903);
    sim.step(TickInput::default());
    let p = crate::sim::platforms(sim.objects());
    assert_eq!((p[0].id, p[0].move_x), (lift, 3));
    sim.step(TickInput::default());
    let p = crate::sim::platforms(sim.objects());
    assert_eq!((p[0].id, p[0].move_x, p[0].move_y), (lift, 0, 0));
    assert_eq!(sim.objects().get(lift).unwrap().x, 906);
}

fn still_platform(w: &mut World, id: ObjectId) {
    let o = w.objects.get_mut(id).unwrap();
    o.type_flags = 0x80;
    o.hit_rect = Rect::new(-32, -8, 32, 8);
}

fn rider(w: &mut World, id: ObjectId) {
    let (x, y) = {
        let o = w.objects.get(id).unwrap();
        (o.x, o.y)
    };
    if w.objects.get(id).unwrap().object_below.is_some() {
        w.physics(id, x, y, crate::physics::opt::PLATFORMS);
    }
}

#[test]
fn a_rider_keeps_a_platform_that_no_longer_updates() {
    install(GameLogic::Platform, still_platform);
    install(GameLogic::Officer, rider);
    let mut sim = Sim::new(
        level(&[("Elevator", 900, 700, 10), ("Officer", 900, 631, 20)]),
        SimConfig::default(),
    );
    let (lift, officer) = (find(&sim, "Elevator")[0], find(&sim, "Officer")[0]);
    // The officer stands on the platform's surface (y 692), and the
    // platform leaves the objects that update.
    sim.with_world(|w| {
        let o = w.objects.get_mut(officer).unwrap();
        o.move_rect = Rect::new(-20, -60, 20, 60);
        o.physics_type = ptype::STAND;
        o.flags |= crate::object::flags::ON_ELEVATOR;
        o.object_below = Some(lift);
        w.objects.unlink(lift);
    });
    idle(&mut sim, 5);
    let o = sim.objects().get(officer).unwrap();
    assert_eq!((o.physics_type, o.y), (ptype::STAND, 631));
    assert!(crate::sim::platforms(sim.objects()).is_empty());
}

// ------------------------------------------- the ledge guard and a teleport

/// An enemy guarded against stepping off ledges that teleports, on its
/// first call, from x 1000 to x 650, where its left edge hangs over the
/// gap left of the floor.
fn teleporter(w: &mut World, id: ObjectId) {
    if w.objects.get(id).unwrap().data.enemy().is_none() {
        let o = w.objects.get_mut(id).unwrap();
        o.move_rect = Rect::new(-20, -60, 20, 60);
        o.physics_type = ptype::STAND;
        o.data = ObjectData::Enemy(enemy::EnemyData::from(enemy::EnemyFields {
            bounds: (600, 1600),
            ledge_guard: true,
            teleport_to: Some(650),
            ..enemy::EnemyFields::default()
        }));
    }
    enemy::motion::integrate(w, id);
}

#[test]
fn a_teleport_is_not_a_step_for_the_ledge_guard() {
    install(GameLogic::Officer, teleporter);
    // The floor starts at x 640.
    let mut rows = vec![".".repeat(200); 15];
    for _ in 0..2 {
        rows.push(format!("{}{}", ".".repeat(10), "#".repeat(190)));
    }
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    let mut spec = ObjectSpec::at("Officer", 1000, 899, 20);
    spec.record[0] = 1000;
    l.objects.push(spec);
    let sim = Sim::new(l, SimConfig::default());
    let officer = find(&sim, "Officer")[0];
    // Standing still where the teleport put it, it made no step: its
    // patrol range is the one it had, though no floor is under its left
    // edge.
    let o = sim.objects().get(officer).unwrap();
    assert_eq!((o.x, o.y), (650, 899));
    assert_eq!(o.data.enemy().unwrap().bounds, (600, 1600));
}

// --------------------------------------------------------------- hitting

fn hitter(w: &mut World, id: ObjectId) {
    let o = w.objects.get_mut(id).unwrap();
    if o.state == 0 {
        o.state = 1;
        o.type_flags = types::ENEMY;
        o.attack_type_flags = types::TREASURE;
        o.attack_rect = Rect::new(-40, -40, 40, 40);
        o.hit_rect = Rect::new(-10, -10, 10, 10);
        o.attack_handler = true;
        o.damage = 10;
    }
}

fn victim(w: &mut World, id: ObjectId) {
    let o = w.objects.get_mut(id).unwrap();
    if o.state == engine_state::KILLED {
        log(format!("victim {} killed", o.id));
        o.state = 2;
    }
    if o.state == 0 {
        o.state = 1;
        o.type_flags = types::TREASURE;
        o.hit_type_flags = types::ENEMY;
        o.hit_rect = Rect::new(-10, -10, 10, 10);
        if o.id == 1160 {
            o.flags |= flags::AUTO_HIT_DAMAGE;
            o.health = 15;
        } else {
            o.hit_handler = true;
        }
    }
}

fn hitter_attacks(w: &mut World, id: ObjectId, victim: ObjectId) {
    let v = w.objects.get(victim).unwrap().id;
    log(format!(
        "attack {} -> {v} at {}",
        w.objects.get(id).unwrap().id,
        w.tick
    ));
}

fn victim_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    assert_eq!(w.objects.get(id).unwrap().hit_ref, Some(attacker));
    let a = w.objects.get(attacker).unwrap().id;
    log(format!(
        "hit {} by {a} at {}",
        w.objects.get(id).unwrap().id,
        w.tick
    ));
}

#[test]
fn collisions_call_the_handlers_in_event_order() {
    install_all(GameLogic::Officer, hitter, |_, _, _| {}, hitter_attacks);
    install_all(GameLogic::Pickup, victim, victim_hit, |_, _, _| {});
    // Two victims around one hitter, one taking damage directly; Claw is
    // far away.
    let mut sim = Sim::new(
        level(&[
            ("TreasurePowerup", 1140, 870, 30),
            ("Officer", 1150, 870, 20),
            ("TreasurePowerup", 1160, 870, 10),
        ]),
        SimConfig::default(),
    );
    take_log();
    sim.step(TickInput::default());
    let events = sim.collision_events().to_vec();
    let id_of = |o: ObjectId| sim.objects().get(o).unwrap().id;
    let mut want = Vec::new();
    for e in &events {
        match *e {
            CollisionEvent::Attack { attacker, victim } => {
                want.push(format!(
                    "attack {} -> {} at 0",
                    id_of(attacker),
                    id_of(victim)
                ));
            }
            CollisionEvent::Hit { attacker, victim } => {
                want.push(format!("hit {} by {} at 0", id_of(victim), id_of(attacker)));
            }
            CollisionEvent::Damage { .. } => {}
        }
    }
    // The hitter attacks both victims; one takes the damage directly, the
    // other through its hit handler.
    assert_eq!(events.len(), 4, "{events:?}");
    assert_eq!(want.len(), 3);
    assert_eq!(take_log(), want);
    // The victim with automatic damage (15 health) loses 10 per tick and
    // sees the killed state on its next call.
    sim.step(TickInput::default());
    assert!(!take_log().iter().any(|l| l.contains("killed")));
    sim.step(TickInput::default());
    let got = take_log();
    assert!(got.contains(&"victim 1160 killed".to_string()), "{got:?}");
}

#[test]
fn enemies_hurt_claw_with_a_window_between_hits() {
    install(GameLogic::Officer, hitter);
    let mut sim = Sim::new(level(&[("Officer", 1000, 899, 20)]), SimConfig::default());
    let mut health = Vec::new();
    for _ in 0..60 {
        sim.step(TickInput::default());
        health.push(sim.stats().health);
    }
    // Hit on the first tick the hitter is set up (its first call), then
    // again once the 850 ms window is over.
    let first = health.iter().position(|&h| h == 90).unwrap();
    let second = health.iter().position(|&h| h == 80).unwrap();
    assert_eq!(second - first, 48, "{health:?}");
    assert_eq!(sim.snapshot().hud.health, sim.stats().health);
    // A harmless touch does nothing.
    let mut sim = Sim::new(level(&[("Officer", 1000, 899, 20)]), SimConfig::default());
    let officer = find(&sim, "Officer")[0];
    sim.step(TickInput::default());
    sim.with_world(|w| {
        w.objects.get_mut(officer).unwrap().special_flags = crate::object::special::HARMLESS_TOUCH;
    });
    let before = sim.stats().health;
    idle(&mut sim, 60);
    assert_eq!(sim.stats().health, before);
}

#[test]
fn without_level_logics_objects_stay_inert_and_collisions_run_no_handler() {
    install(GameLogic::Officer, hitter);
    let cfg = SimConfig {
        level_logics: false,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level(&[("Officer", 1000, 899, 20)]), cfg);
    let officer = find(&sim, "Officer")[0];
    assert_eq!(sim.objects().get(officer).unwrap().kind, LogicKind::Inert);
    // Even an object set up to hurt Claw leaves him alone.
    sim.with_world(|w| {
        let o = w.objects.get_mut(officer).unwrap();
        o.type_flags = types::ENEMY;
        o.attack_rect = Rect::new(-40, -40, 40, 40);
        o.damage = 10;
    });
    idle(&mut sim, 10);
    assert!(!sim.collision_events().is_empty());
    assert_eq!(sim.stats().health, 100);
    assert!(sim.diagnostics().inert_logics.is_empty());
}

// --------------------------------------------------------- notifications

fn notified(w: &mut World, id: ObjectId) {
    let o = w.objects.get_mut(id).unwrap();
    match o.state {
        0 => {
            o.flags |= flags::NOTIFY_DESTROY | flags::NOTIFY_SLEEP;
            o.state = 1;
        }
        engine_state::DESTROYING => log(format!("{} destroying", o.id)),
        engine_state::SLEEPING => {
            log(format!("{} sleeping", o.id));
            o.state = 7;
        }
        _ => {
            if o.id == 1100 && w.tick == 3 {
                w.destroy(id);
            }
        }
    }
}

#[test]
fn destroy_and_sleep_reach_the_logic() {
    install(GameLogic::Checkpoint, notified);
    let mut sim = Sim::new(
        level(&[("Checkpoint", 1100, 800, 5), ("Checkpoint", 1150, 800, 5)]),
        SimConfig::default(),
    );
    let ids = find(&sim, "Checkpoint");
    idle(&mut sim, 5);
    assert_eq!(take_log(), vec!["1100 destroying"]);
    assert!(sim.objects().get(ids[0]).is_none());
    // Moving the view far away puts the other one to sleep; it gets one
    // call in the sleeping state, and its own state comes back after it.
    sim.teleport_player(10000, 899);
    let mut ticks = 0;
    while sim.objects().get(ids[1]).unwrap().is_listed() {
        sim.step(TickInput::default());
        ticks += 1;
        assert!(ticks < 300, "the camera follows Claw away");
    }
    idle(&mut sim, 3);
    assert_eq!(take_log(), vec!["1150 sleeping"]);
    let o = sim.objects().get(ids[1]).unwrap();
    assert!(!o.is_listed());
    assert_eq!(o.state, 7, "a state set by the call itself stays");
}

// ----------------------------------------------------------- frame sounds

fn sounding(w: &mut World, id: ObjectId) {
    if w.objects.get(id).unwrap().state == 0 {
        w.objects.get_mut(id).unwrap().state = 1;
        assert!(w.set_anim(id, "TEST_SOUNDS"));
        assert!(!w.set_anim(id, "NO_SUCH_ANIMATION"));
    }
    w.advance_anim(id);
}

fn sound_anim() -> Animation {
    let mut a = Animation::looping("TEST_SOUNDS", &[(1, 1), (2, 1), (3, 1), (4, 1)]);
    a.frames[3] = AnimFrame {
        flags: FRAME_SOUND_IN_VIEW,
        ..a.frames[3]
    };
    a.frame_sounds = vec![
        vec![Some("ONE".into())],
        vec![Some("TWO_A".into()), Some("TWO_B".into())],
        vec![None],
        vec![Some("IN_VIEW".into())],
    ];
    a
}

fn played(audio: &[AudioCue]) -> Vec<String> {
    audio
        .iter()
        .map(|c| match c {
            AudioCue::Play(s) => s.key.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn animation_frames_play_their_sounds() {
    install(GameLogic::Decor, sounding);
    let mut l = level(&[("FrontCandy", 1050, 800, 5), ("FrontCandy", 1350, 800, 5)]);
    let a = sound_anim();
    l.animations.insert(a.key.clone(), std::sync::Arc::new(a));
    let mut sim = Sim::new(l, SimConfig::default());
    let mut heard = Vec::new();
    let game_rng = sim.rng().clone();
    let mut rng = sim.sound_rng().clone();
    for _ in 0..4 {
        heard.push(played(&sim.step(TickInput::default()).audio));
    }
    // The choice comes from the sound-only stream: the game's stream is
    // untouched.
    assert_eq!(sim.rng(), &game_rng);
    // Both objects are live (the second is outside the view but inside
    // its activation window); the in-view sound plays for the first only.
    // The first element is applied by the first call, while the level
    // loads; its sounds go out with the first tick.
    let two = |r: &mut crate::rng::Rng| ["TWO_A", "TWO_B"][r.below(2) as usize].to_string();
    let (a, b) = (two(&mut rng), two(&mut rng));
    assert_eq!(heard[0], vec!["ONE".to_string(), "ONE".to_string(), a, b]);
    assert!(heard[1].is_empty(), "an unregistered name plays nothing");
    assert_eq!(heard[2], vec!["IN_VIEW"]);
    assert_eq!(heard[3], vec!["ONE", "ONE"]);
}

#[test]
fn claws_own_frames_play_their_sounds() {
    let mut l = level(&[]);
    let mut stand = Animation::looping("CLAW_STAND", &[(11, 1)]);
    stand.frame_sounds = vec![vec![Some("STEP".into())]];
    l.animations
        .insert(stand.key.clone(), std::sync::Arc::new(stand));
    let mut sim = Sim::new(l, SimConfig::default());
    // The level start applies the stand animation's first element too.
    assert_eq!(
        played(&sim.step(TickInput::default()).audio),
        vec!["STEP", "STEP"]
    );
    for _ in 0..3 {
        assert_eq!(played(&sim.step(TickInput::default()).audio), vec!["STEP"]);
    }
    assert_eq!(played(sim.last_audio()), vec!["STEP"]);
}

fn cycler(w: &mut World, id: ObjectId) {
    if w.objects.get(id).unwrap().state == 0 {
        let o = w.objects.get_mut(id).unwrap();
        o.state = 1;
        o.image_set = "TEST_CYCLE".into();
        o.frame = 3;
        assert!(w.set_anim(id, "TEST_NEXT"));
    }
    w.advance_anim(id);
    let o = w.objects.get(id).unwrap();
    log(format!("{}", o.frame));
}

#[test]
fn animations_step_within_the_image_set_of_their_object() {
    install(GameLogic::Decor, cycler);
    let mut l = level(&[("FrontCandy", 1050, 800, 5)]);
    let next = Animation::looping("TEST_NEXT", &[(0, 1)]);
    let mut next = next;
    next.frames[0].image_op = 1;
    l.animations
        .insert(next.key.clone(), std::sync::Arc::new(next));
    l.images
        .insert("TEST_CYCLE".into(), ImageFrames::range(2, 4));
    let mut sim = Sim::new(l, SimConfig::default());
    // The first call, while the level loads, steps from 3 to 4.
    assert_eq!(take_log(), vec!["4"]);
    idle(&mut sim, 5);
    assert_eq!(take_log(), vec!["2", "3", "4", "2", "3"]);
}

#[test]
fn in_view_frame_sounds_follow_the_draw_box() {
    install(GameLogic::Decor, sounding);
    let mut l = level(&[
        ("FrontCandy", 1050, 800, 5),
        ("FrontCandy", 1350, 800, 5),
        ("FrontCandy", 1340, 800, 5),
    ]);
    // The second object is past the right edge of the view, but its wide
    // image reaches into it; the third has no known image.
    l.objects[1].image_set = "WIDE".into();
    let mut wide = ImageFrames::range(1, 4);
    for f in 1..=4 {
        wide.boxes.insert(f, Rect::new(-100, -10, 100, 10));
    }
    l.images.insert("WIDE".into(), wide);
    let a = sound_anim();
    l.animations.insert(a.key.clone(), std::sync::Arc::new(a));
    let mut sim = Sim::new(l, SimConfig::default());
    let view = sim.view().rect;
    assert!(!view.contains(1340, 800) && !view.contains(1350, 800));
    let mut heard = Vec::new();
    for _ in 0..4 {
        heard.push(played(&sim.step(TickInput::default()).audio));
    }
    // The load's sounds go out with the first tick.
    assert_eq!(heard[0][..3], ["ONE", "ONE", "ONE"]);
    assert_eq!(heard[2], vec!["IN_VIEW", "IN_VIEW"]);
}

fn shaker(w: &mut World, id: ObjectId) {
    let o = w.objects.get_mut(id).unwrap();
    if o.state == 0 {
        o.state = 1;
        w.shake(300);
        w.shake(500);
        w.shake(100);
        assert_eq!(w.out.pending_shake(), Some(500), "the longest is kept");
    }
}

#[test]
fn a_shake_asked_for_by_a_logic_starts_in_its_tick() {
    install(GameLogic::Decor, shaker);
    // One object runs before the camera, the other after it.
    for z in [5, 20_000] {
        let mut sim = Sim::new(level(&[("FrontCandy", 1050, 800, z)]), SimConfig::default());
        idle(&mut sim, 3);
        assert_eq!(sim.camera().shake_until_ms, 500, "z {z}");
    }
}

fn killable(w: &mut World, id: ObjectId) {
    let o = w.objects.get(id).unwrap();
    log(format!("{} {}", o.state, o.kill_kind));
}

#[test]
fn a_kill_reaches_the_logic_with_its_kind() {
    install(GameLogic::Decor, killable);
    let mut sim = Sim::new(level(&[("FrontCandy", 1050, 800, 5)]), SimConfig::default());
    idle(&mut sim, 1);
    let id = find(&sim, "FrontCandy")[0];
    assert!(sim.with_world(|w| w.kill(id, 2)));
    assert!(!sim.with_world(|w| w.kill(9999, 2)));
    idle(&mut sim, 1);
    // The calls while the level loads and in the first tick, then the kill.
    assert_eq!(take_log(), vec!["0 0", "0 0", "28 2"]);
}

// ------------------------------------------------------------- cloning

#[test]
fn a_cloned_running_simulation_reproduces_its_trace() {
    install(GameLogic::Decor, spawner);
    install(GameLogic::Effect, child);
    install(GameLogic::Pickup, holder);
    install(GameLogic::Platform, mover);
    let mut sim = Sim::new(
        level(&[
            ("FrontCandy", 1100, 800, 500),
            ("TreasurePowerup", 1050, 880, 10),
            ("Elevator", 900, 700, 10),
        ]),
        SimConfig::default(),
    );
    let input = |i: usize| {
        let held = if i % 50 < 30 {
            Buttons::RIGHT
        } else {
            Buttons::LEFT | Buttons::JUMP
        };
        TickInput {
            held,
            pressed: if i % 50 == 30 { held } else { Buttons::NONE },
        }
    };
    for i in 0..30 {
        sim.step(input(i));
    }
    let mut copy = sim.clone();
    let run = |s: &mut Sim| {
        let mut rows = Vec::new();
        let mut outputs = Vec::new();
        for i in 30..200 {
            let snap = s.step(input(i));
            rows.push(TraceRow::capture(s, i as u64, "script"));
            outputs.push((snap.audio, snap.events, snap.renderables));
        }
        let objects: Vec<String> = s.objects().all().map(|o| format!("{o:?}")).collect();
        (trace::to_csv(&rows), outputs, objects)
    };
    let a = run(&mut sim);
    take_log();
    let b = run(&mut copy);
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
    assert_eq!(a.2, b.2);
}
