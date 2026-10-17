//! Claw's full arsenal on synthetic data: every attack type's box, damage
//! and active frame, the shots he launches (read from the projectile
//! requests), ammunition, the weapon toggle and keys, dynamite power,
//! rapid fire, sword shots, lifting and throwing, the hit reaction and the
//! freeze, and the random stream left untouched by firing.

mod common;

use std::sync::Arc;

use claw_frame::{AudioCue, Buttons, TickInput, Weapon};
use claw_sim::anim::Animation;
use claw_sim::logic::keg::{self, Phase};
use claw_sim::logic::projectile::{ProjectileData, ProjectileSpec};
use claw_sim::logic::{self, World};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, draw_flags, flags, types};
use claw_sim::physics::ptype;
use claw_sim::player::attack::{self, kind};
use claw_sim::player::special;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::tuning::{Behaviour, Tuning};
use claw_sim::{ObjectData, Sim, SimConfig, SimLevel};
use common::*;

const ATTACK: u32 = 0x2;
const PROJECTILE: u32 = 0x4;
const TOGGLE: u32 = 0x8;
const LIFT: u32 = 0x20;
const PISTOL: u32 = 0x40;
const MAGIC: u32 = 0x80;
const DYNAMITE: u32 = 0x100;
const TICK_MS: i32 = 18;

/// An animation of (image, duration, event) elements; `once` makes the
/// last element finish it.
fn anim(key: &str, frames: &[(i32, i32, i32)], once: bool) -> Arc<Animation> {
    let pairs: Vec<(i32, i32)> = frames.iter().map(|&(i, d, _)| (i, d)).collect();
    let mut a = if once {
        Animation::once(key, &pairs)
    } else {
        Animation::looping(key, &pairs)
    };
    for (f, &(_, _, event)) in a.frames.iter_mut().zip(frames) {
        f.event = event;
    }
    Arc::new(a)
}

/// A swing: wind-up, the active frame (event 1), follow-through.
const SWING: &[(i32, i32, i32)] = &[(24, 50, 0), (25, 50, 0), (27, 100, 1), (24, 50, 0)];
/// A sword swing: its first frame (1 ms) sends the sword's shot.
const SWORD_SWING: &[(i32, i32, i32)] = &[(24, 1, 5), (25, 50, 0), (27, 100, 1), (24, 50, 0)];
/// A pistol shot: the shot leaves on element 3, later elements allow
/// rapid fire.
const PISTOL_SHOT: &[(i32, i32, i32)] = &[
    (100, 50, 0),
    (101, 50, 0),
    (102, 50, 0),
    (103, 50, 2),
    (104, 50, 3),
    (105, 50, 3),
    (106, 50, 3),
];
/// An empty pistol: clicks with rapid-fire frames, no shot.
const EMPTY_PISTOL: &[(i32, i32, i32)] = &[
    (100, 50, 0),
    (101, 50, 0),
    (102, 50, 0),
    (103, 50, 3),
    (104, 50, 3),
];
/// A single release on element 1.
const RELEASE: &[(i32, i32, i32)] = &[(110, 50, 0), (111, 50, 2), (112, 100, 0)];
/// No event.
const EMPTY: &[(i32, i32, i32)] = &[(110, 50, 0), (112, 100, 0)];
/// The dynamite charge: event 4 frames, the last one very long.
const CHARGE: &[(i32, i32, i32)] = &[(120, 50, 4), (121, 50, 4), (122, 5000, 4)];

/// Claw's animations for every attack and special state.
fn claw_anims() -> Vec<Arc<Animation>> {
    let mut v = Vec::new();
    for key in [
        "CLAW_SWIPE",
        "CLAW_PUNCH",
        "CLAW_UPPERCUT",
        "CLAW_KICK",
        "CLAW_DUCKSWIPE",
        "CLAW_JUMPSWIPE",
    ] {
        v.push(anim(key, SWING, true));
    }
    for key in [
        "CLAW_POWERUPSWIPE",
        "CLAW_DUCKPOWERUPSWIPE",
        "CLAW_JUMPPOWERUPSWIPE",
    ] {
        v.push(anim(key, SWORD_SWING, true));
    }
    for key in ["CLAW_PISTOL", "CLAW_DUCKPISTOL"] {
        v.push(anim(key, PISTOL_SHOT, true));
    }
    for key in ["CLAW_EMPTYPISTOL", "CLAW_DUCKEMPTYPISTOL"] {
        v.push(anim(key, EMPTY_PISTOL, true));
    }
    for key in [
        "CLAW_MAGIC",
        "CLAW_DUCKMAGIC",
        "CLAW_JUMPMAGIC",
        "CLAW_JUMPPISTOL",
        "CLAW_POSTDYNAMITE",
        "CLAW_DUCKPOSTDYNAMITE",
        "CLAW_JUMPDYNAMITE",
    ] {
        v.push(anim(key, RELEASE, true));
    }
    for key in [
        "CLAW_EMPTYMAGIC",
        "CLAW_DUCKEMPTYMAGIC",
        "CLAW_EMPTYJUMPMAGIC",
        "CLAW_EMPTYJUMPPISTOL",
        "CLAW_EMPTYDYNAMITE",
        "CLAW_DUCKEMPTYDYNAMITE",
        "CLAW_EMPTYJUMPDYNAMITE",
    ] {
        v.push(anim(key, EMPTY, true));
    }
    for key in ["CLAW_PREDYNAMITE", "CLAW_DUCKPREDYNAMITE"] {
        v.push(anim(key, CHARGE, true));
    }
    for key in special::HIT_ANIMS {
        v.push(anim(key, &[(200, 100, 1), (201, 200, 0)], true));
    }
    v.push(anim(
        special::LIFT_ANIM,
        &[(40, 100, 0), (42, 100, 0), (43, 100, 0)],
        true,
    ));
    v.push(anim(
        special::LIFT_LOW_ANIM,
        &[(41, 100, 0), (42, 100, 0), (43, 100, 0)],
        true,
    ));
    v.push(anim(special::CARRY_ANIM, &[(43, 1000, 0)], false));
    v.push(anim(
        special::CARRY_WALK_ANIM,
        &[(45, 100, 1), (46, 100, 0)],
        false,
    ));
    v.push(anim(
        special::THROW_ANIM,
        &[(44, 100, 0), (47, 50, 1), (48, 100, 0)],
        true,
    ));
    v.push(anim("CLAW_STAND", &[(11, 110, 0), (12, 110, 0)], false));
    v.push(anim("CLAW_DUCK", &[(67, 250, 0)], false));
    v.push(anim("CLAW_JUMP", &[(271, 55, 0), (272, 55, 0)], false));
    v.push(anim("CLAW_FALL", &[(278, 65, 0), (280, 80, 0)], false));
    v.push(anim("CLAW_LAND", &[(290, 50, 0)], true));
    v
}

fn arena() -> SimLevel {
    let mut l = flat_level();
    for a in claw_anims() {
        l.animations.insert(a.key.clone(), a);
    }
    l
}

fn sim_with_tuning(seed: u32, tuning: Tuning) -> Sim {
    let mut sim = Sim::new(
        arena(),
        SimConfig {
            seed,
            tuning,
            ..SimConfig::default()
        },
    );
    idle(&mut sim, 30);
    sim
}

fn new_sim(seed: u32) -> Sim {
    sim_with_tuning(seed, Tuning::default())
}

fn claw(sim: &Sim) -> &claw_sim::object::Object {
    sim.player().expect("Claw")
}

fn claw_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.type_flags == types::PLAYER)
        .map(|(id, _)| id)
        .expect("Claw")
}

/// Ticks an animation element of `duration` ms is shown: at least one.
fn shown(duration: i32) -> i32 {
    ((duration + TICK_MS - 1) / TICK_MS).max(1)
}

/// The tick, counted from the one the animation starts on, on which
/// element `index` of `frames` comes up.
fn element_tick(frames: &[(i32, i32, i32)], index: usize) -> i32 {
    frames[..index].iter().map(|f| shown(f.1)).sum()
}

/// Every shot request seen so far, in launch order: (creation number,
/// request, drawing flags when it appeared). Real shots fly off and
/// disappear, so the simulation is watched after every tick.
#[derive(Default)]
struct Shots(Vec<(u32, ProjectileSpec, u32)>);

impl Shots {
    fn watch(&mut self, sim: &Sim) {
        let mut new: Vec<(u32, ProjectileSpec, u32)> = sim
            .objects()
            .all()
            .filter_map(|(_, o)| match &o.data {
                ObjectData::Projectile(ProjectileData { spec: Some(s), .. })
                    if !self.0.iter().any(|e| e.0 == o.serial) =>
                {
                    Some((o.serial, *s, o.draw_flags))
                }
                _ => None,
            })
            .collect();
        new.sort_by_key(|e| e.0);
        self.0.extend(new);
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn spec(&self, i: usize) -> ProjectileSpec {
        self.0[i].1
    }
}

/// One tick, recording the shots it launches.
fn go(sim: &mut Sim, log: &mut Shots, input: TickInput) {
    sim.step(input);
    log.watch(sim);
}

/// `n` idle ticks, recording shots.
fn wait(sim: &mut Sim, log: &mut Shots, n: usize) {
    for _ in 0..n {
        go(sim, log, TickInput::default());
    }
}

fn played(sim: &Sim, key: &str) -> bool {
    sim.last_audio()
        .iter()
        .any(|c| matches!(c, AudioCue::Play(s) if s.key == key))
}

fn set(sim: &mut Sim, f: impl FnOnce(&mut World)) {
    sim.with_world(f);
}

/// The first standing blow chosen with `seed` when nothing else draws.
fn rolled_blow(rng: &mut Rng) -> i32 {
    let mut r = rng.below(4);
    if r == 0 || r == 3 {
        r = rng.below(4);
    }
    attack::MELEE_TABLE[r as usize]
}

fn seed_for(blow: i32) -> u32 {
    (1..1000)
        .find(|&s| rolled_blow(&mut Rng::new(s)) == blow)
        .expect("a seed gives the blow")
}

/// Per tick of an attack: (attacking, box live, box, damage).
type Tick = (bool, bool, Rect, i32);

/// Presses `button` on top of `held` and records the attack until it ends.
fn run_attack(sim: &mut Sim, button: u32, held: u32) -> Vec<Tick> {
    let rec = |sim: &Sim| {
        let o = claw(sim);
        (
            sim.claw().attacking,
            o.flags & flags::HARMLESS == 0,
            o.attack_rect,
            o.damage,
        )
    };
    sim.step(TickInput {
        held: Buttons(held | button),
        pressed: Buttons(button),
    });
    let mut out = vec![rec(sim)];
    for _ in 0..80 {
        if !sim.claw().attacking {
            break;
        }
        sim.step(hold(held));
        out.push(rec(sim));
    }
    out
}

fn live_ticks(ticks: &[Tick]) -> Vec<i32> {
    ticks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.0 && t.1)
        .map(|(i, _)| i as i32)
        .collect()
}

fn crouch(sim: &mut Sim) {
    for _ in 0..5 {
        sim.step(hold(DOWN));
    }
    assert_eq!(claw(sim).state, state::CROUCH);
}

fn jump(sim: &mut Sim) {
    sim.step(press(JUMP));
    sim.step(hold(JUMP));
    assert_eq!(claw(sim).state, state::JUMP);
}

fn face_left(sim: &mut Sim) {
    for _ in 0..3 {
        sim.step(hold(LEFT));
    }
    idle(sim, 2);
    assert!(!sim.claw().facing_right);
}

// ------------------------------------------------------------ blows

#[test]
fn every_blow_has_its_box_damage_and_active_frame() {
    // The active element (100 ms) is live on each tick it shows.
    let first = element_tick(SWING, 2);
    let live: Vec<i32> = (first..first + shown(100)).collect();
    let standing = [
        (kind::SWIPE, Rect::new(30, -2, 112, 19), 5),
        (kind::PUNCH, Rect::new(0, -2, 56, 19), 5),
        (kind::UPPERCUT, Rect::new(0, -19, 56, 19), 5),
        (kind::KICK, Rect::new(0, -2, 56, 19), 5),
    ];
    for (blow, rect, damage) in standing {
        let mut sim = new_sim(seed_for(blow));
        let ticks = run_attack(&mut sim, ATTACK, 0);
        assert_eq!(live_ticks(&ticks), live, "type {blow}");
        assert!(
            ticks
                .iter()
                .filter(|t| t.0)
                .all(|t| t.2 == rect && t.3 == damage)
        );
        assert_eq!(claw(&sim).state, state::STAND);
    }
    let mut sim = new_sim(1);
    crouch(&mut sim);
    sim.step(press(ATTACK | DOWN));
    assert_eq!(sim.claw().attack.kind, kind::DUCK_SWIPE);
    assert_eq!(claw(&sim).attack_rect, Rect::new(30, 19, 105, 37));
    assert_eq!(claw(&sim).damage, 2);
    let mut sim = new_sim(1);
    jump(&mut sim);
    sim.step(press(ATTACK | JUMP));
    assert_eq!(sim.claw().attack.kind, kind::JUMP_SWIPE);
    assert_eq!(claw(&sim).attack_rect, Rect::new(30, -2, 112, 19));
    assert_eq!(claw(&sim).damage, 5);
}

#[test]
fn catnip_doubles_blows_and_overrides_replace_damage_and_reach() {
    let catnip = |w: &mut World| {
        w.claw.stats.powerup_kind = attack::CATNIP_POWERUP;
        w.claw.stats.powerup_ms = 20_000;
    };
    let mut sim = new_sim(seed_for(kind::PUNCH));
    set(&mut sim, catnip);
    sim.step(press(ATTACK));
    assert_eq!(claw(&sim).damage, 10);
    let mut sim = new_sim(1);
    set(&mut sim, catnip);
    crouch(&mut sim);
    sim.step(press(ATTACK | DOWN));
    assert_eq!(claw(&sim).damage, 4, "crouching swipe doubled");
    // Strong blows win over catnip.
    let mut sim = new_sim(1);
    set(&mut sim, |w| {
        catnip(w);
        w.claw.attack.overrides.strong_blows = true;
    });
    crouch(&mut sim);
    sim.step(press(ATTACK | DOWN));
    assert_eq!(claw(&sim).damage, attack::STRONG_DUCK_DAMAGE);
    // One hit wins over no damage; long reach widens the box, and facing
    // left mirrors the widened box.
    let mut sim = new_sim(seed_for(kind::KICK));
    face_left(&mut sim);
    set(&mut sim, |w| {
        w.claw.attack.overrides = attack::AttackOverrides {
            one_hit: true,
            no_damage: true,
            long_reach: true,
            ..Default::default()
        };
    });
    // Facing left the same kick comes up from the untouched stream.
    sim.step(press(ATTACK));
    assert_eq!(sim.claw().attack.kind, kind::KICK);
    assert_eq!(claw(&sim).damage, attack::ONE_HIT_DAMAGE);
    assert_eq!(claw(&sim).attack_rect, Rect::new(-376, -2, 0, 19));
    let mut sim = new_sim(seed_for(kind::KICK));
    set(&mut sim, |w| w.claw.attack.overrides.no_damage = true);
    sim.step(press(ATTACK));
    assert_eq!(claw(&sim).damage, 0);
}

#[test]
fn a_jumping_swipe_carries_on_into_the_fall() {
    let long = anim(
        "CLAW_JUMPSWIPE",
        &[(284, 50, 0), (283, 400, 1), (284, 75, 0)],
        true,
    );
    let mut level = arena();
    level.animations.insert(long.key.clone(), long);
    let mut sim = Sim::new(level, SimConfig::default());
    idle(&mut sim, 30);
    jump(&mut sim);
    sim.step(press(ATTACK | JUMP));
    assert_eq!(sim.claw().attack.kind, kind::JUMP_SWIPE);
    let mut fell_attacking = false;
    for _ in 0..40 {
        sim.step(TickInput::default());
        if claw(&sim).state == state::FALL && sim.claw().attacking {
            fell_attacking = true;
            assert_eq!(
                claw(&sim).anim.key(),
                Some("CLAW_JUMPSWIPE"),
                "the swipe keeps playing"
            );
        }
        if !sim.claw().attacking {
            break;
        }
    }
    assert!(
        fell_attacking,
        "the swipe went on after the top of the jump"
    );
    assert!(!sim.claw().attacking);
    if claw(&sim).state == state::FALL {
        assert_eq!(claw(&sim).anim.key(), Some("CLAW_FALL"));
    }
}

#[test]
fn the_fall_animation_comes_back_after_an_air_attack_carried_over_the_top() {
    let mut sim = new_sim(3);
    jump(&mut sim);
    sim.step(press(ATTACK | JUMP));
    assert_eq!(sim.claw().attack.kind, kind::JUMP_SWIPE);
    for _ in 0..40 {
        sim.step(TickInput::default());
        if claw(&sim).state == state::FALL {
            break;
        }
    }
    assert_eq!(claw(&sim).state, state::FALL);
    assert!(sim.claw().attacking, "the swipe carries on into the fall");
    assert_eq!(sim.claw().attack.restore_anim.as_deref(), Some("CLAW_FALL"));
}

#[test]
fn a_hit_at_the_top_of_a_jump_ends_the_air_attack() {
    let mut sim = new_sim(3);
    jump(&mut sim);
    sim.step(press(ATTACK | JUMP));
    assert_eq!(sim.claw().attack.kind, kind::JUMP_SWIPE);
    assert_eq!(claw(&sim).state, state::JUMP);
    // Claw stays at the top of his jump for one tick (his own update is
    // skipped, so it cannot hand over to the fall first) and an enemy shot
    // hits him there.
    let id = claw_id(&sim);
    let (x, y) = (claw(&sim).x, claw(&sim).y);
    set(&mut sim, |w| {
        w.objects.get_mut(id).unwrap().frame_delay = 1;
        w.claw.jump_peak_y = y;
    });
    let mut shot = Object::new(0, x, y, 10, Arc::from("Shot"), LogicKind::Inert);
    shot.flags = flags::WORLD;
    shot.type_flags = types::ENEMY_SHOT;
    shot.attack_rect = Rect::new(-20, -20, 20, 20);
    shot.damage = 5;
    sim.spawn(shot);
    let health = sim.stats().health;
    sim.step(TickInput::default());
    assert_eq!(sim.stats().health, health - 5, "the shot hit");
    assert!(!sim.claw().attacking, "the hit ends the swipe");
    assert_eq!(sim.claw().attack.kind, kind::NONE);
    assert!(claw(&sim).flags & flags::HARMLESS != 0);
}

// ------------------------------------------------------------ shots

#[test]
fn a_pistol_shot_requests_its_bullet_on_the_release_frame() {
    let mut sim = new_sim(3);
    let mut log = Shots::default();
    let id = claw_id(&sim);
    let (x, y, z) = (claw(&sim).x, claw(&sim).y, claw(&sim).z);
    assert_eq!(sim.stats().weapon, Weapon::Pistol);
    let release = element_tick(PISTOL_SHOT, 3);
    go(&mut sim, &mut log, press(PROJECTILE));
    assert_eq!(sim.claw().attack.kind, kind::PISTOL);
    for t in 1..=release {
        assert_eq!(log.len(), 0, "no shot before tick {release}");
        go(&mut sim, &mut log, TickInput::default());
        assert_eq!(log.len(), usize::from(t == release), "tick {t}");
    }
    assert_eq!(
        log.spec(0),
        ProjectileSpec {
            logic: "PistolBullet",
            x: x + 71,
            y: y - 21,
            z: z - 1,
            vx: 700,
            vy: 0,
            damage: 8,
            type_flags: types::PISTOL_SHOT,
            hit_type_flags: types::ENEMY,
            special_flags: 0,
            image_set: "GAME_BULLETS",
            owner: Some(id),
            fuse_ms: None,
            power: 0,
        }
    );
    assert_eq!(sim.stats().pistol, 9, "one shot spent");
    // Only one shot leaves from the release frame.
    wait(&mut sim, &mut log, 20);
    assert_eq!(log.len(), 1);
    assert!(!sim.claw().attacking);
}

#[test]
fn shots_leave_from_their_offsets_facing_either_way() {
    // Facing left: mirrored position and speed; magic is mirrored too.
    let mut sim = new_sim(3);
    let mut log = Shots::default();
    face_left(&mut sim);
    let (x, y, z) = (claw(&sim).x, claw(&sim).y, claw(&sim).z);
    go(&mut sim, &mut log, press(MAGIC));
    assert_eq!(sim.stats().weapon, Weapon::Magic, "the key selects magic");
    assert_eq!(sim.claw().attack.kind, kind::MAGIC, "and fires it");
    wait(&mut sim, &mut log, 10);
    let (_, s, drawn) = log.0[0];
    assert_eq!(
        (s.logic, s.x, s.y, s.z, s.vx, s.vy),
        ("MagicBullet", x - 54, y - 8, z + 1, -700, 0)
    );
    assert_eq!(
        (s.damage, s.type_flags, s.hit_type_flags, s.special_flags),
        (
            25,
            attack::MAGIC_SHOT_TYPE,
            types::ENEMY,
            attack::MAGIC_SHOT_FLAG
        )
    );
    assert!(drawn & draw_flags::MIRROR != 0);
    assert_eq!(sim.stats().magic, 4);
    // Crouching pistol and crouching magic.
    let mut sim = new_sim(3);
    let mut log = Shots::default();
    crouch(&mut sim);
    let (x, y) = (claw(&sim).x, claw(&sim).y);
    go(&mut sim, &mut log, press(PISTOL | DOWN));
    assert_eq!(sim.claw().attack.kind, kind::DUCK_PISTOL);
    for _ in 0..12 {
        go(&mut sim, &mut log, hold(DOWN));
    }
    let s = log.spec(0);
    assert_eq!((s.x, s.y, s.vx), (x + 71, y + 23, 700));
    for _ in 0..40 {
        if !sim.claw().attacking {
            break;
        }
        go(&mut sim, &mut log, hold(DOWN));
    }
    go(&mut sim, &mut log, hold(DOWN));
    go(&mut sim, &mut log, press(MAGIC | DOWN));
    assert_eq!(sim.claw().attack.kind, kind::DUCK_MAGIC);
    for _ in 0..6 {
        go(&mut sim, &mut log, hold(DOWN));
    }
    let s = log.spec(1);
    assert_eq!((s.logic, s.x, s.y), ("MagicBullet", x + 68, y + 24));
    // In the air: pistol and magic.
    let mut sim = new_sim(3);
    let mut log = Shots::default();
    jump(&mut sim);
    go(&mut sim, &mut log, press(PROJECTILE | JUMP));
    assert_eq!(sim.claw().attack.kind, kind::JUMP_PISTOL);
    let x = claw(&sim).x;
    for _ in 0..element_tick(RELEASE, 1) - 1 {
        go(&mut sim, &mut log, hold(JUMP));
    }
    assert_eq!(log.len(), 0);
    go(&mut sim, &mut log, hold(JUMP));
    let (cx, cy) = (claw(&sim).x, claw(&sim).y);
    let s = log.spec(0);
    assert_eq!(s.logic, "PistolBullet");
    assert_eq!(
        (s.x, s.y),
        (cx + 71, cy - 21),
        "from where he is in the air"
    );
    assert_eq!(cx, x, "no steering");
    let mut sim = new_sim(3);
    jump(&mut sim);
    sim.step(press(MAGIC | JUMP));
    assert_eq!(sim.claw().attack.kind, kind::JUMP_MAGIC);
}

#[test]
fn empty_weapons_play_their_empty_animation_and_launch_nothing() {
    let mut sim = new_sim(3);
    let mut log = Shots::default();
    set(&mut sim, |w| {
        w.claw.stats.pistol = 0;
        w.claw.stats.magic = 0;
        w.claw.stats.dynamite = 0;
    });
    for (key, anim) in [
        (PISTOL, "CLAW_EMPTYPISTOL"),
        (MAGIC, "CLAW_EMPTYMAGIC"),
        (DYNAMITE, "CLAW_EMPTYDYNAMITE"),
    ] {
        go(&mut sim, &mut log, press(key));
        assert!(sim.claw().attacking);
        assert_eq!(claw(&sim).anim.key(), Some(anim));
        wait(&mut sim, &mut log, 30);
        assert!(!sim.claw().attacking);
    }
    assert_eq!(log.len(), 0);
    let st = sim.stats();
    assert_eq!((st.pistol, st.magic, st.dynamite), (0, 0, 0));
}

#[test]
fn ammunition_runs_down_and_the_toggle_cycles_with_a_click() {
    let mut sim = new_sim(3);
    let mut log = Shots::default();
    set(&mut sim, |w| {
        w.claw.stats.pistol = 2;
        w.claw.stats.magic = 1;
        w.claw.stats.dynamite = 1;
    });
    for _ in 0..3 {
        go(&mut sim, &mut log, press(PISTOL));
        wait(&mut sim, &mut log, 30);
    }
    assert_eq!(sim.stats().pistol, 0);
    assert_eq!(log.len(), 2, "the third press finds the pistol empty");
    // Toggle: pistol, magic, dynamite, pistol, each with the click.
    let mut seen = Vec::new();
    for _ in 0..3 {
        go(&mut sim, &mut log, press(TOGGLE));
        assert!(played(&sim, attack::TOGGLE_SOUND));
        seen.push(sim.stats().weapon);
    }
    assert_eq!(seen, [Weapon::Magic, Weapon::Dynamite, Weapon::Pistol]);
    // Not while an attack runs: no change, no click.
    go(&mut sim, &mut log, press(MAGIC));
    assert!(sim.claw().attacking);
    go(&mut sim, &mut log, press(TOGGLE));
    assert_eq!(sim.stats().weapon, Weapon::Magic);
    assert!(!played(&sim, attack::TOGGLE_SOUND));
    wait(&mut sim, &mut log, 30);
    assert_eq!(sim.stats().magic, 0);
    // A stick of dynamite is spent at its release.
    go(&mut sim, &mut log, press(DYNAMITE));
    wait(&mut sim, &mut log, 40);
    assert_eq!(sim.stats().dynamite, 0);
    assert_eq!(log.len(), 4);
    assert_eq!(log.spec(3).logic, "Dynamite");
}

fn clicks(sim: &Sim) -> usize {
    sim.last_audio()
        .iter()
        .filter(|c| matches!(c, AudioCue::Play(s) if s.key == attack::TOGGLE_SOUND))
        .count()
}

#[test]
fn the_toggle_clicks_once_in_every_state() {
    // Standing: one click.
    let mut sim = new_sim(3);
    sim.step(press(TOGGLE));
    assert_eq!(sim.stats().weapon, Weapon::Magic);
    assert_eq!(clicks(&sim), 1);
    // Climbing a ladder: the weapon changes with the same click.
    let mut sim = Sim::new(ladder_level(), SimConfig::default());
    idle(&mut sim, 30);
    for &input in &script(&[(UP | JUMP, 108), (UP, 300)]).ticks {
        sim.step(input);
    }
    assert_eq!(claw(&sim).state, state::CLIMB);
    assert_eq!(sim.stats().weapon, Weapon::Pistol);
    sim.step(TickInput {
        held: Buttons(UP | TOGGLE),
        pressed: Buttons(TOGGLE),
    });
    assert_eq!(claw(&sim).state, state::CLIMB);
    assert_eq!(sim.stats().weapon, Weapon::Magic);
    assert_eq!(clicks(&sim), 1);
}

/// Throws dynamite after holding the fire key for `hold_ticks` ticks
/// after the press; returns the request.
fn throw_dynamite(sim: &mut Sim, hold_ticks: usize, extra: u32) -> ProjectileSpec {
    let mut log = Shots::default();
    set(sim, |w| w.claw.stats.weapon = Weapon::Dynamite);
    go(sim, &mut log, press(PROJECTILE | extra));
    for _ in 0..hold_ticks {
        go(sim, &mut log, hold(PROJECTILE | extra));
    }
    for _ in 0..40 {
        if log.len() > 0 {
            break;
        }
        go(sim, &mut log, hold(extra));
    }
    assert_eq!(log.len(), 1, "one stick");
    log.spec(0)
}

#[test]
fn dynamite_power_follows_the_charge() {
    // A tap: the shortest charge, power 0.
    let mut sim = new_sim(3);
    let (x, y, z) = (claw(&sim).x, claw(&sim).y, claw(&sim).z);
    let s = throw_dynamite(&mut sim, 0, 0);
    assert_eq!(
        (s.logic, s.x, s.y, s.z, s.vx, s.vy),
        ("Dynamite", x + 60, y - 8, z + 1, 0, 0)
    );
    assert_eq!(
        (s.damage, s.type_flags, s.power),
        (15, types::PISTOL_SHOT, 0)
    );
    assert_eq!(sim.stats().dynamite, 2);
    // Held 2000 ms: the longest charge, power 200.
    let mut sim = new_sim(3);
    let s = throw_dynamite(&mut sim, (2000 / TICK_MS) as usize + 2, 0);
    assert_eq!(s.power, 200);
    // About halfway, facing left: negative.
    let mut sim = new_sim(3);
    face_left(&mut sim);
    let s = throw_dynamite(&mut sim, 54, 0);
    let charge = 55 * TICK_MS;
    assert_eq!(s.power, -((charge - 50) * 200 / 1950));
    // Crouching.
    let mut sim = new_sim(3);
    crouch(&mut sim);
    let (x, y) = (claw(&sim).x, claw(&sim).y);
    let s = throw_dynamite(&mut sim, 0, DOWN);
    assert_eq!((s.x, s.y, s.power), (x + 60, y + 12, 0));
    // In the air: always the power of a 500 ms charge.
    let mut sim = new_sim(3);
    jump(&mut sim);
    let s = throw_dynamite(&mut sim, 0, JUMP);
    assert_eq!(s.power, 450 * 200 / 1950);
    assert_eq!(s.power, 46);
    assert_eq!(attack::throw_power(500, 50, 2000, false), -46);
    assert_eq!(attack::throw_power(5000, 50, 2000, true), 200);
    assert_eq!(attack::throw_power(0, 50, 50, true), 0);
}

/// Claw's attack count recorded by every projectile now in the level, by
/// creation number.
fn shot_serials(sim: &Sim, seen: &mut Vec<(u32, &'static str, u32)>) {
    for (_, o) in sim.objects().all() {
        if let ObjectData::Projectile(d) = &o.data
            && let Some(s) = d.spec
            && !seen.iter().any(|e| e.0 == o.serial)
        {
            seen.push((o.serial, s.logic, d.attack_serial));
        }
    }
    seen.sort_by_key(|e| e.0);
}

#[test]
fn pistol_and_magic_shots_carry_claws_attack_count() {
    let mut sim = new_sim(3);
    set(&mut sim, |w| w.claw.stats.dynamite = 1);
    let mut seen = Vec::new();
    let mut counts = Vec::new();
    for button in [PISTOL, MAGIC, PISTOL, DYNAMITE] {
        sim.step(press(button));
        counts.push(sim.claw().attack.serial);
        for _ in 0..40 {
            sim.step(TickInput::default());
            shot_serials(&sim, &mut seen);
        }
    }
    let got: Vec<(&str, u32)> = seen.iter().map(|e| (e.1, e.2)).collect();
    assert_eq!(
        got,
        [
            ("PistolBullet", counts[0]),
            ("MagicBullet", counts[1]),
            ("PistolBullet", counts[2]),
            ("Dynamite", 0),
        ]
    );
    assert!(counts.windows(2).all(|c| c[1] == c[0].wrapping_add(1)));
    // A rapid-fire shot counts as an attack of its own.
    let mut sim = new_sim(3);
    let mut seen = Vec::new();
    sim.step(press(PROJECTILE));
    let first = sim.claw().attack.serial;
    for _ in 1..element_tick(PISTOL_SHOT, 4) {
        sim.step(TickInput::default());
        shot_serials(&sim, &mut seen);
    }
    sim.step(press(PROJECTILE));
    for _ in 0..20 {
        sim.step(TickInput::default());
        shot_serials(&sim, &mut seen);
    }
    let got: Vec<u32> = seen.iter().map(|e| e.2).collect();
    assert_eq!(got, [first, first.wrapping_add(1)]);
}

#[test]
fn a_magic_shot_through_an_enemy_hits_it_once() {
    let mut sim = new_sim(3);
    let (x, y) = (claw(&sim).x, claw(&sim).y);
    let cid = claw_id(&sim);
    set(&mut sim, |w| {
        w.objects.get_mut(cid).unwrap().hit_type_flags = 0;
        w.claw.stats.invulnerable_ms = 1_000_000;
    });
    let officer = sim.spawn(logic::new_object("Officer", x + 160, y, 4000));
    idle(&mut sim, 2);
    set(&mut sim, |w| {
        w.objects.get_mut(officer).unwrap().health = 1000
    });
    sim.step(press(MAGIC));
    let mut hits = 0;
    for _ in 0..60 {
        sim.step(TickInput::default());
        hits += sim
            .collision_events()
            .iter()
            .filter(|e| {
                matches!(**e, claw_sim::container::CollisionEvent::Hit { attacker, victim }
                    if victim == officer && sim.objects().get(attacker).is_some_and(|a| a.type_flags != types::PLAYER))
            })
            .count();
    }
    let health = sim.objects().get(officer).unwrap().health;
    assert!(hits >= 1, "the shot reached the officer");
    assert_eq!(health, 1000 - 25, "one hit of the shot's damage");
}

#[test]
fn rapid_fire_rewinds_for_another_shot() {
    let mut sim = new_sim(3);
    let mut log = Shots::default();
    let serial = sim.claw().attack.serial;
    go(&mut sim, &mut log, press(PROJECTILE));
    let refire = element_tick(PISTOL_SHOT, 4);
    for _ in 1..refire {
        go(&mut sim, &mut log, TickInput::default());
    }
    assert_eq!(log.len(), 1);
    // On a rapid-fire frame, a press readies the next shot.
    go(&mut sim, &mut log, press(PROJECTILE));
    assert_eq!(sim.claw().attack.serial, serial.wrapping_add(2));
    wait(&mut sim, &mut log, 8);
    assert_eq!(log.len(), 2, "a second shot left");
    assert_eq!(sim.stats().pistol, 8);
    wait(&mut sim, &mut log, 30);
    assert!(!sim.claw().attacking);
    assert_eq!(log.len(), 2);
    // With the last shot spent, pressing again does not rewind.
    let mut sim = new_sim(3);
    let mut log = Shots::default();
    set(&mut sim, |w| w.claw.stats.pistol = 1);
    go(&mut sim, &mut log, press(PROJECTILE));
    for _ in 1..refire {
        go(&mut sim, &mut log, TickInput::default());
    }
    go(&mut sim, &mut log, press(PROJECTILE));
    wait(&mut sim, &mut log, 30);
    assert_eq!(log.len(), 1);
    assert_eq!(sim.stats().pistol, 0);
}

#[test]
fn a_sword_shot_leaves_once_per_swing() {
    for (code, logic, flag, sound) in attack::SWORD_SHOTS {
        let mut sim = new_sim(3);
        let mut log = Shots::default();
        set(&mut sim, |w| {
            w.claw.stats.powerup_kind = code;
            w.claw.stats.powerup_ms = 30_000;
        });
        let (x, y) = (claw(&sim).x, claw(&sim).y);
        go(&mut sim, &mut log, press(ATTACK));
        assert_eq!(sim.claw().attack.kind, kind::SWIPE);
        assert_eq!(claw(&sim).anim.key(), Some("CLAW_POWERUPSWIPE"));
        // The 1 ms frame is shorter than a tick and still sends the shot.
        assert!(played(&sim, sound), "{sound}");
        wait(&mut sim, &mut log, 30);
        assert_eq!(log.len(), 1, "{logic}: one shot per swing");
        let s = log.spec(0);
        assert_eq!(
            (s.logic, s.x, s.y, s.vx, s.damage, s.special_flags),
            (
                logic,
                x + 95,
                y + 15,
                700,
                25,
                attack::MAGIC_SHOT_FLAG | flag
            )
        );
        go(&mut sim, &mut log, press(ATTACK));
        wait(&mut sim, &mut log, 30);
        assert_eq!(log.len(), 2, "{logic}: a second swing");
    }
    // The plasma sword swings without a shot.
    let mut sim = new_sim(3);
    let mut log = Shots::default();
    set(&mut sim, |w| {
        w.claw.stats.powerup_kind = 0x26BB;
        w.claw.stats.powerup_ms = 30_000;
    });
    go(&mut sim, &mut log, press(ATTACK));
    wait(&mut sim, &mut log, 30);
    assert_eq!(log.len(), 0);
}

/// Removes every shot before it acts, so only Claw's own random draws
/// are compared.
fn remove_shots(sim: &mut Sim) {
    sim.with_world(|w| {
        let ids: Vec<ObjectId> = w
            .objects
            .all()
            .filter(|(_, o)| matches!(o.data, ObjectData::Projectile(_)))
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            if let Some(o) = w.objects.get_mut(id) {
                o.kind = LogicKind::Inert;
            }
            w.destroy(id);
        }
    });
}

#[test]
fn firing_draws_the_line_chance_from_the_game_random_stream() {
    // Same seed; one run fires everything, the other stands still. Each
    // attack that starts with ammunition rolls the chance of Claw's line,
    // one draw of the shared stream as in the original; nothing else in
    // the firing draws.
    let mut quiet = new_sim(11);
    let mut busy = new_sim(11);
    let mut log = Shots::default();
    let presses = [PROJECTILE, MAGIC, DYNAMITE, TOGGLE, PISTOL, PROJECTILE];
    let mut extra = Vec::new();
    for p in presses {
        go(&mut busy, &mut log, press(p));
        remove_shots(&mut busy);
        for _ in 0..40 {
            go(&mut busy, &mut log, TickInput::default());
            remove_shots(&mut busy);
        }
        idle(&mut quiet, 41);
        extra.push(draws_between(quiet.rng().state(), busy.rng().state()));
    }
    assert!(log.len() >= 4, "shots: {}", log.len());
    // The pistol shot and the dynamite throw roll; the magic shot has no
    // line and toggling the weapon starts no attack; the pistol key and the
    // projectile key then fire the pistol again.
    assert_eq!(
        extra,
        [Some(1), Some(1), Some(2), Some(2), Some(3), Some(4)],
        "extra draws of the firing run after each press"
    );
}

/// The number of draws of the shared random stream that lead from state
/// `from` to state `to`, when there are fewer than 1000.
fn draws_between(from: u32, to: u32) -> Option<u32> {
    let mut r = Rng::new(from);
    for n in 0..1000 {
        if r.state() == to {
            return Some(n);
        }
        r.next_u15();
    }
    None
}

// ------------------------------------------------------------ lift and throw

/// A powder keg standing on the floor `dx` px in front of Claw.
fn keg_beside(sim: &mut Sim, dx: i32) -> ObjectId {
    let (x, y) = (claw(sim).x, claw(sim).y);
    // Claw's feet are 60 px below his position; kegs rest on the floor at
    // spawn.
    let feet = keg::move_box(keg::KEG_BOX).bottom;
    let o = logic::new_object("PowderKeg", x + dx, y + 60 - feet, 4000);
    let id = sim.spawn(o);
    sim.step(TickInput::default());
    let o = sim.objects().get(id).expect("keg");
    assert!(o.special_flags & claw_sim::object::special::LIFTABLE != 0);
    id
}

fn keg_phase(sim: &Sim, id: ObjectId) -> Phase {
    keg::data_of(sim.objects().get(id).expect("keg"))
        .expect("keg data")
        .phase
}

#[test]
fn lifting_then_throwing_a_keg_calls_its_hooks() {
    let mut sim = new_sim(3);
    let id = keg_beside(&mut sim, 40);
    let z = claw(&sim).z;
    let top = claw(&sim).move_rect.top;
    sim.step(press(LIFT));
    assert_eq!(claw(&sim).state, state::LIFT);
    assert_eq!(sim.claw().special.lifted, Some(id));
    assert_eq!(keg_phase(&sim, id), Phase::Lifted);
    assert_eq!(sim.objects().get(id).unwrap().z, z + 1);
    assert_eq!(claw(&sim).move_rect.top, top - special::LIFT_RAISE);
    assert_eq!(
        claw(&sim).anim.key(),
        Some(special::LIFT_LOW_ANIM),
        "a keg is lifted from a crouch"
    );
    // The lift ends: Claw carries the keg.
    idle(&mut sim, 20);
    assert!(sim.claw().special.carrying);
    assert_eq!(claw(&sim).anim.key(), Some(special::CARRY_ANIM));
    // Walking with it.
    let x = claw(&sim).x;
    for _ in 0..30 {
        sim.step(hold(RIGHT));
    }
    assert_eq!(claw(&sim).state, state::LIFT);
    assert_eq!(claw(&sim).anim.key(), Some(special::CARRY_WALK_ANIM));
    let walked = claw(&sim).x - x;
    assert!(walked > 0 && walked < 20, "slow carrying walk: {walked} px");
    idle(&mut sim, 2);
    assert_eq!(claw(&sim).anim.key(), Some(special::CARRY_ANIM));
    // Throw, charged for 50 ticks.
    sim.step(press(LIFT));
    assert_eq!(claw(&sim).state, state::THROW);
    assert_eq!(claw(&sim).move_rect.top, top);
    for _ in 0..50 {
        sim.step(hold(LIFT));
    }
    assert_eq!(sim.claw().special.throw_ms, 50 * TICK_MS);
    let (evx, evy) = special::throw_velocity(50 * TICK_MS, true);
    assert_eq!((evx, evy), (200 + 3 * 30, -(600 + 2 * 30)));
    let mut thrown = false;
    for _ in 0..20 {
        sim.step(TickInput::default());
        if keg_phase(&sim, id) == Phase::Thrown {
            thrown = true;
            assert!(played(&sim, special::THROW_SOUND));
            break;
        }
    }
    assert!(thrown, "the keg was thrown");
    assert_eq!(sim.claw().special.lifted, None);
    let d = keg::data_of(sim.objects().get(id).unwrap()).unwrap();
    assert_eq!(d.vx, f64::from(evx));
    assert!(
        d.vy >= f64::from(evy) && d.vy <= f64::from(evy) + 80.0,
        "{}",
        d.vy
    );
    idle(&mut sim, 20);
    assert_eq!(claw(&sim).state, state::STAND);
    assert_eq!(special::throw_velocity(0, false), (-200, -600));
    assert_eq!(special::throw_velocity(9000, true), (500, -800));
}

#[test]
fn a_thrown_keg_kills_the_officer_it_strikes_and_explodes_there() {
    let mut sim = new_sim(3);
    let (x, y) = (claw(&sim).x, claw(&sim).y);
    let cid = claw_id(&sim);
    set(&mut sim, |w| {
        w.objects.get_mut(cid).unwrap().hit_type_flags = 0;
    });
    let officer = sim.spawn(logic::new_object("Officer", x + 300, y, 4000));
    let id = keg_beside(&mut sim, 200);
    idle(&mut sim, 2);
    let health = sim.objects().get(officer).unwrap().health;
    assert!(health > 0 && health < keg::BLAST_DAMAGE, "{health}");
    // Flying level at the officer's chest, from his left.
    let o = sim.objects().get(officer).unwrap();
    let (ox, oy) = (o.x, o.y);
    set(&mut sim, |w| {
        let k = w.objects.get_mut(id).unwrap();
        k.x = ox - 90;
        k.y = oy - 60;
        logic::thrown(w, id, 400, 0);
    });
    // The synthetic level has no explosion animation: the keg blasts and
    // is removed at once when it explodes.
    let mut last = None;
    let mut struck = None;
    for _ in 0..40 {
        let k = sim.objects().get(id).expect("the keg flies");
        last = Some((k.x, k.y));
        sim.step(TickInput::default());
        if struck.is_none()
            && sim.collision_events().iter().any(|e| {
                matches!(*e, claw_sim::container::CollisionEvent::Hit { attacker, victim }
                    if attacker == id && victim == officer)
            })
        {
            struck = sim.objects().get(id).map(|k| (k.x, k.y));
        }
        if sim.objects().get(id).is_none() {
            break;
        }
    }
    let (kx, ky) = struck.expect("the keg struck the officer");
    assert!(sim.objects().get(id).is_none(), "the keg exploded");
    assert_eq!(last, Some((kx, ky)), "it exploded where it struck");
    let o = sim.objects().get(officer).unwrap();
    let d = claw_sim::logic::enemy::data_of(o).unwrap();
    assert!(d.dead, "the officer is killed by the strike");
    assert_eq!(d.death, claw_sim::logic::enemy::DeathKind::Hit);
}

#[test]
fn a_thrown_keg_strikes_every_enemy_it_overlaps_in_that_pass_then_explodes_once() {
    let mut sim = new_sim(3);
    let (x, y) = (claw(&sim).x, claw(&sim).y);
    let cid = claw_id(&sim);
    set(&mut sim, |w| {
        w.objects.get_mut(cid).unwrap().hit_type_flags = 0;
    });
    // Two officers standing together, both in the keg's way.
    let a = sim.spawn(logic::new_object("Officer", x + 300, y, 4000));
    let b = sim.spawn(logic::new_object("Officer", x + 304, y, 4000));
    let id = keg_beside(&mut sim, 200);
    idle(&mut sim, 2);
    for o in [a, b] {
        set(&mut sim, |w| w.objects.get_mut(o).unwrap().health = 100);
    }
    let oy = sim.objects().get(a).unwrap().y;
    set(&mut sim, |w| {
        let k = w.objects.get_mut(id).unwrap();
        k.x = x + 300 - 90;
        k.y = oy - 60;
        logic::thrown(w, id, 400, 0);
    });
    let mut strikes = Vec::new();
    let mut blasts = 0;
    for t in 0..40 {
        sim.step(TickInput::default());
        for e in sim.collision_events() {
            match *e {
                claw_sim::container::CollisionEvent::Hit { attacker, victim }
                    if attacker == id && (victim == a || victim == b) =>
                {
                    strikes.push((t, victim));
                }
                _ => {}
            }
        }
        blasts += sim
            .objects()
            .all()
            .filter(|(_, o)| {
                matches!(&o.data, ObjectData::Effect(d)
                    if matches!(d.request, Some(claw_sim::logic::effect::EffectKind::Explosion { source: Some(s), .. }) if s == id))
            })
            .count();
        if sim.objects().get(id).is_none() {
            break;
        }
    }
    // Each enemy takes the strike once, in the same pass: an enemy's own
    // record of Claw's attacks is all that limits hits, and the keg is a
    // thrown object until it explodes at its next logic call.
    assert_eq!(strikes.len(), 2, "{strikes:?}");
    assert_eq!(strikes[0].0, strikes[1].0, "the same pass: {strikes:?}");
    assert_ne!(strikes[0].1, strikes[1].1);
    assert!(sim.objects().get(id).is_none(), "the keg exploded");
    assert!(blasts <= 1, "one explosion");
    for o in [a, b] {
        let h = sim.objects().get(o).map_or(0, |o| o.health);
        assert!(h <= 100 - keg::BLAST_DAMAGE, "struck: {h}");
    }
}

#[test]
fn a_hit_while_carrying_drops_the_keg() {
    let mut sim = new_sim(3);
    let id = keg_beside(&mut sim, 40);
    sim.step(press(LIFT));
    idle(&mut sim, 20);
    let x = claw(&sim).x;
    set(&mut sim, |w| {
        assert!(w.claw_hit_reaction(x + 50));
    });
    sim.step(TickInput::default());
    assert_eq!(claw(&sim).state, state::HIT);
    assert_eq!(sim.claw().special.lifted, None);
    assert_eq!(keg_phase(&sim, id), Phase::Dropped);
}

#[test]
fn starting_a_throw_winds_up_a_carried_officer() {
    let mut sim = new_sim(3);
    let (x, y) = (claw(&sim).x, claw(&sim).y);
    // The officer's touch does not hurt Claw during the test.
    let cid = claw_id(&sim);
    set(&mut sim, |w| {
        w.claw.stats.invulnerable_ms = 1_000_000;
        w.objects.get_mut(cid).unwrap().hit_type_flags &= !types::ENEMY;
    });
    let id = sim.spawn(logic::new_object("Officer", x + 40, y, 4000));
    idle(&mut sim, 3);
    let o = sim.objects().get(id).expect("officer");
    assert!(o.special_flags & claw_sim::object::special::LIFTABLE != 0);
    sim.step(press(LIFT));
    assert_eq!(sim.claw().special.lifted, Some(id));
    let winding = |sim: &Sim| {
        claw_sim::logic::enemy::data_of(sim.objects().get(id).unwrap())
            .expect("enemy data")
            .winding_up
    };
    idle(&mut sim, 20);
    assert!(sim.claw().special.carrying);
    assert!(!winding(&sim));
    sim.step(press(LIFT));
    assert_eq!(claw(&sim).state, state::THROW);
    assert!(winding(&sim), "the officer is told Claw winds up");
}

/// An officer spawned 40 px in front of Claw and killed where it stands as
/// on spikes; Claw cannot be hurt during the test.
fn officer_body(sim: &mut Sim) -> ObjectId {
    use claw_sim::logic::enemy::{self, DeathKind};
    let (x, y) = (claw(sim).x, claw(sim).y);
    let cid = claw_id(sim);
    set(sim, |w| {
        w.claw.stats.invulnerable_ms = 1_000_000;
        w.objects.get_mut(cid).unwrap().hit_type_flags &= !types::ENEMY;
    });
    let id = sim.spawn(logic::new_object("Officer", x + 40, y, 4000));
    idle(sim, 3);
    set(sim, |w| enemy::die(w, id, DeathKind::Spikes, None));
    idle(sim, 1);
    id
}

fn enemy_state(sim: &Sim, id: ObjectId) -> Option<claw_sim::logic::enemy::EnemyState> {
    sim.objects()
        .get(id)
        .and_then(|o| claw_sim::logic::enemy::EnemyState::from_id(o.state))
}

#[test]
fn claw_lifts_carries_and_throws_an_officers_body() {
    use claw_sim::logic::enemy::{DeathKind, EnemyState, data_of};
    let mut sim = new_sim(3);
    let id = officer_body(&mut sim);
    assert_eq!(enemy_state(&sim, id), Some(EnemyState::Dead));
    sim.step(press(LIFT));
    assert_eq!(sim.claw().special.lifted, Some(id));
    assert_eq!(enemy_state(&sim, id), Some(EnemyState::Lifted));
    idle(&mut sim, 20);
    assert!(sim.claw().special.carrying);
    assert_eq!(enemy_state(&sim, id), Some(EnemyState::Flailing));
    let (c, o) = (claw(&sim), sim.objects().get(id).unwrap());
    assert!(o.y < c.y, "held over Claw's head");
    sim.step(press(LIFT));
    assert_eq!(claw(&sim).state, state::THROW);
    let mut seen = Vec::new();
    for _ in 0..300 {
        sim.step(TickInput::default());
        let Some(s) = enemy_state(&sim, id) else {
            break;
        };
        if seen.last() != Some(&s) {
            seen.push(s);
        }
        if s == EnemyState::Dead {
            let o = sim.objects().get(id).unwrap();
            assert_eq!(data_of(o).unwrap().death, DeathKind::Landing);
            assert_eq!(o.type_flags, types::THROWN);
        }
    }
    assert!(seen.contains(&EnemyState::Thrown), "{seen:?}");
    assert_eq!(seen.last(), Some(&EnemyState::Dead), "{seen:?}");
    assert!(sim.objects().get(id).is_none(), "removed");
}

#[test]
fn claw_never_lifts_a_body_killed_by_the_landing_of_a_throw() {
    use claw_sim::logic::enemy::{self, DeathKind};
    let mut sim = new_sim(3);
    let id = officer_body(&mut sim);
    // As after a throw: a thrown object that died of its landing.
    set(&mut sim, |w| {
        w.objects.get_mut(id).unwrap().type_flags = types::THROWN;
        enemy::die(w, id, DeathKind::Landing, None);
    });
    idle(&mut sim, 1);
    let o = sim.objects().get(id).unwrap();
    assert!(o.special_flags & claw_sim::object::special::LIFTABLE != 0);
    sim.step(press(LIFT));
    assert_eq!(sim.claw().special.lifted, None);
    assert_ne!(claw(&sim).state, state::LIFT);
}

#[test]
fn claw_does_not_target_or_lift_what_is_flagged_for_removal() {
    for flagged in [false, true] {
        // An officer's body in reach, flagged for removal before the pass
        // in which Claw reaches for it (as when it died in goo earlier in
        // that pass).
        let mut sim = new_sim(3);
        let id = officer_body(&mut sim);
        if flagged {
            set(&mut sim, |w| w.destroy(id));
        }
        sim.step(press(LIFT));
        if flagged {
            assert_eq!(sim.claw().special.lifted, None);
            assert_ne!(claw(&sim).state, state::LIFT, "no lift pose");
            assert!(sim.objects().get(id).is_none(), "removed");
        } else {
            assert_eq!(sim.claw().special.lifted, Some(id), "control");
        }
        // The same for a keg.
        let mut sim = new_sim(3);
        let id = keg_beside(&mut sim, 40);
        if flagged {
            set(&mut sim, |w| w.destroy(id));
        }
        sim.step(press(LIFT));
        let lifted = sim.claw().special.lifted;
        assert_eq!(lifted, (!flagged).then_some(id), "flagged: {flagged}");
    }
}

#[test]
fn up_and_attack_lifts_too_and_nothing_out_of_reach_is_lifted() {
    let mut sim = new_sim(3);
    let id = keg_beside(&mut sim, 40);
    sim.step(TickInput {
        held: Buttons(UP | ATTACK),
        pressed: Buttons(ATTACK),
    });
    assert_eq!(claw(&sim).state, state::LIFT);
    assert_eq!(sim.claw().special.lifted, Some(id));
    let mut sim = new_sim(3);
    keg_beside(&mut sim, 120);
    sim.step(press(LIFT));
    assert_eq!(claw(&sim).state, state::STAND);
    assert_eq!(sim.claw().special.lifted, None);
}

// ------------------------------------------------------------ hit and freeze

#[test]
fn a_hit_pushes_claw_away_then_he_stands() {
    let mut sim = new_sim(3);
    let x = claw(&sim).x;
    let before = sim.rng().clone();
    set(&mut sim, |w| {
        assert!(w.claw_hit_reaction(x - 50));
        assert!(!w.claw_hit_reaction(x - 50), "already reeling");
    });
    let mut mirror = before;
    mirror.below(100);
    let push = shown(100);
    for t in 0..push {
        sim.step(TickInput::default());
        assert_eq!(claw(&sim).state, state::HIT);
        assert_eq!(claw(&sim).x, x + 3 * (t + 1), "tick {t}");
    }
    assert_eq!(
        sim.rng().state(),
        mirror.state(),
        "one draw for the animation"
    );
    for _ in 0..30 {
        sim.step(TickInput::default());
        if claw(&sim).state != state::HIT {
            break;
        }
    }
    assert_eq!(claw(&sim).x, x + 3 * push, "the push stops after its frame");
    assert_eq!(claw(&sim).state, state::STAND);
    // From the other side, pushed left.
    let mut sim = new_sim(3);
    set(&mut sim, |w| {
        w.claw_hit_reaction(x + 50);
    });
    sim.step(TickInput::default());
    assert_eq!(claw(&sim).x, x - 3);
}

fn hit_in_the_air(fix: bool) -> Sim {
    let tuning = Tuning {
        behaviour: Behaviour {
            fix_double_jump: fix,
        },
        ..Tuning::default()
    };
    let mut sim = sim_with_tuning(3, tuning);
    jump(&mut sim);
    sim.step(hold(JUMP));
    let x = claw(&sim).x;
    set(&mut sim, |w| {
        w.claw_hit_reaction(x - 50);
    });
    sim.step(TickInput::default());
    assert_eq!(claw(&sim).state, state::HIT);
    assert_eq!(claw(&sim).physics_type, ptype::FALL);
    assert_eq!(claw(&sim).x, x, "no push in the air");
    let y = claw(&sim).y;
    while claw(&sim).state == state::HIT {
        sim.step(TickInput::default());
        if claw(&sim).state == state::HIT {
            assert_eq!(claw(&sim).y, y, "the reaction does not move him");
        }
    }
    sim
}

#[test]
fn a_hit_ending_in_mid_air_lands_then_falls_with_the_fix() {
    let mut sim = hit_in_the_air(true);
    assert_eq!(claw(&sim).state, state::LAND);
    let y = claw(&sim).y;
    sim.step(press(JUMP));
    assert_eq!(claw(&sim).state, state::FALL, "no second jump");
    assert!(claw(&sim).y >= y);
    let mut sim = hit_in_the_air(false);
    assert_eq!(claw(&sim).state, state::STAND, "the original stands");
    sim.step(press(JUMP));
    assert_eq!(claw(&sim).state, state::JUMP);
}

#[test]
fn a_freeze_holds_claw_until_released_or_timed_out() {
    let mut sim = new_sim(3);
    face_left(&mut sim);
    sim.step(press(MAGIC));
    assert!(sim.claw().attacking);
    set(&mut sim, |w| {
        assert!(w.freeze_claw(0));
    });
    sim.step(TickInput::default());
    assert_eq!(claw(&sim).state, state::FREEZE);
    assert!(!sim.claw().attacking, "the attack ended");
    assert!(sim.claw().facing_right, "he faces right");
    let x = claw(&sim).x;
    for _ in 0..60 {
        sim.step(hold(LEFT));
        sim.step(press(JUMP | ATTACK));
    }
    assert_eq!(claw(&sim).state, state::FREEZE);
    assert_eq!(claw(&sim).x, x);
    set(&mut sim, |w| w.thaw_claw());
    sim.step(TickInput::default());
    assert_eq!(claw(&sim).state, state::STAND);
    // A timed freeze.
    let mut sim = new_sim(3);
    set(&mut sim, |w| {
        w.freeze_claw(100);
    });
    let mut frozen = 0;
    for _ in 0..20 {
        sim.step(TickInput::default());
        if claw(&sim).state == state::FREEZE {
            frozen += 1;
        }
    }
    assert_eq!(frozen, shown(100) - 1);
    assert_eq!(claw(&sim).state, state::STAND);
}
