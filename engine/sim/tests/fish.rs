//! The Fish on a synthetic pool with the eleventh level's animation
//! timings: how it is built (on its record's height, which is its swim
//! line, its box from its first picture for everything, never lifted),
//! its swim (90 px/s, turning exactly on its limits), the leap at a Claw
//! above its line while swimming towards him (80 draws in 101, 44 ticks of
//! arc 155 px high, back on its line within its range, the leaving
//! splash, the bite on picture 202 and the splash back in), the dash at a
//! Claw level with or below it (90 in 101, 180 px/s to where he stood),
//! the leap against a wall that turns it back into its pool, the flop on
//! dry ground (sliding back at 60 px/s, a flop every 250 ms facing by the
//! toggle the game shares, back in below its line), its touch (10) and the
//! hit that kills it at once.

use std::sync::Arc;

use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::fish::{self, FishData};
use claw_sim::logic::enemy::{self, EnemyState, field};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{LogicKind, Object, ObjectId, flags, types};
use claw_sim::physics::ptype;
use claw_sim::rng::Rng;
use claw_sim::tiles::TileMap;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};

/// An animation of (image, duration) elements; `once` makes the last
/// element finish it.
fn anim(key: &str, frames: &[(i32, i32)], once: bool) -> Arc<Animation> {
    Arc::new(if once {
        Animation::once(key, frames)
    } else {
        Animation::looping(key, frames)
    })
}

fn fish_anims() -> Vec<Arc<Animation>> {
    vec![
        anim(
            fish::SWIM_ANIM,
            &[(1, 150), (2, 150), (3, 150), (2, 150), (1, 150), (200, 150)],
            false,
        ),
        // Picture 202 on the 20th call, done on the 48th.
        anim(
            fish::STRIKE_ANIM,
            &[
                (4, 150),
                (201, 180),
                (202, 120),
                (200, 150),
                (5, 200),
                (6, 0),
            ],
            true,
        ),
        anim(fish::FLOP_ANIM, &[(5, 130), (6, 130)], false),
        anim(fish::DEATH_ANIM, &[(950, 0)], true),
    ]
}

fn claw_anims() -> Vec<Arc<Animation>> {
    let mut v = Vec::new();
    for key in claw_sim::player::special::HIT_ANIMS {
        v.push(anim(key, &[(200, 150), (201, 200)], true));
    }
    v.push(anim("CLAW_STAND", &[(11, 110), (12, 110)], false));
    v
}

/// The pool: a fish's line and range.
#[derive(Debug, Clone, Copy)]
struct Pool {
    at: i32,
    line: i32,
    range: (i32, i32),
}

/// The floor's top is 960 (Claw stands at 899); a line at 900 lies just
/// below Claw, one at 880 above him.
const POOL: Pool = Pool {
    at: 3000,
    line: 900,
    range: (2700, 3300),
};

/// A level from `map` with a fish in `pool`, every object awake.
fn level_with(map: TileMap, pool: Pool) -> SimLevel {
    let mut level = SimLevel::synthetic(map, (1000, 600));
    level.number = 11;
    for a in fish_anims().into_iter().chain(claw_anims()) {
        level.animations.insert(a.key.clone(), a);
    }
    let mut o = ObjectSpec::at("Fish", pool.at, pool.line, 5000);
    o.record[field::X_MIN] = pool.range.0;
    o.record[field::X_MAX] = pool.range.1;
    level.objects = vec![o];
    level
}

/// The flat floor with a fish in `pool`.
fn arena(pool: Pool) -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    level_with(TileMap::from_ascii(64, &rows), pool)
}

/// `level` after its first tick with Claw far away, then put at `claw_x`
/// on the floor, tough enough to take any number of touches; the fish's
/// id.
fn with_claw(level: SimLevel, seed: u32, claw_x: i32) -> (Sim, ObjectId) {
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(FAR, 899);
    sim.with_world(|w| w.claw.stats.health = 10_000);
    sim.step(TickInput::default());
    sim.teleport_player(claw_x, 899);
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Fish))
        .map(|(id, _)| id)
        .expect("a fish");
    (sim, id)
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn fdata(sim: &Sim, id: ObjectId) -> FishData {
    match &obj(sim, id).data {
        ObjectData::Fish(d) => d.clone(),
        d => panic!("fish data: {d:?}"),
    }
}

/// A random stream whose next draw of 0..`n` is `want`.
fn stream_drawing(n: u32, want: u32) -> Rng {
    (1..100_000)
        .map(Rng::new)
        .find(|r| r.clone().below(n) == want)
        .expect("a seed")
}

/// True when `key` played on the last tick.
fn played(sim: &Sim, key: &str) -> bool {
    sim.last_audio()
        .iter()
        .any(|a| matches!(a, AudioCue::Play(s) if s.key == key))
}

/// Where Claw waits while the level is set up: out of every range box.
const FAR: i32 = 5000;

/// Runs the shared pre-tick (Claw's side and the range mode), then the
/// swim step with the next draw of 0..101 being `r`.
fn swim_with(sim: &mut Sim, id: ObjectId, r: u32) {
    sim.with_world(|w| {
        enemy::pre_tick(w, id, &fish::CLASS);
        *w.rng = stream_drawing(101, r);
        fish::swim_tick(w, id);
    });
}

/// The fish's (x, y, state) on each of `n` ticks.
fn run(sim: &mut Sim, id: ObjectId, n: usize) -> Vec<(i32, i32, Option<EnemyState>)> {
    (0..n)
        .map(|_| {
            sim.step(TickInput::default());
            let o = obj(sim, id);
            (o.x, o.y, state_of(sim, id))
        })
        .collect()
}

#[test]
fn it_is_built_on_its_line_swimming_left() {
    let (sim, id) = with_claw(arena(POOL), 1, 3600);
    let o = obj(&sim, id);
    assert_eq!(o.y, POOL.line, "not dropped to the floor");
    assert_eq!(o.damage, fish::DAMAGE);
    assert_eq!(o.hit_type_flags, fish::HIT_BY);
    assert_eq!(o.type_flags, types::ENEMY);
    for r in [o.move_rect, o.hit_rect, o.attack_rect] {
        assert_eq!(r, fish::RECT);
    }
    assert_eq!(o.physics_type, ptype::DIRECT);
    let f = fdata(&sim, id);
    assert_eq!((f.line, f.home), (POOL.line, POOL.range));
    assert_eq!(f.enemy.ranges, fish::RANGES);
    assert_eq!(f.enemy.vx, -f64::from(fish::SWIM_SPEED));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Swim));
    assert!(!enemy::faces_right(o));
    assert!(enemy::class(GameLogic::Fish).unwrap().carry.is_none());
    // A record's own damage counts.
    let mut level = arena(POOL);
    level.objects[0].record[16] = 25;
    let (sim, id) = with_claw(level, 1, 3600);
    assert_eq!(obj(&sim, id).damage, 25);
}

#[test]
fn it_swims_at_90_px_a_second_and_turns_exactly_on_its_limits() {
    let pool = Pool { at: 2710, ..POOL };
    let (mut sim, id) = with_claw(arena(pool), 2, FAR);
    let rows = run(&mut sim, id, 20);
    // 1.62 px a tick to the left, held on its limit, then right.
    let on_limit = rows.iter().position(|r| r.0 == 2700).unwrap();
    assert!(
        rows[..on_limit]
            .windows(2)
            .all(|p| (1..=2).contains(&(p[0].0 - p[1].0))),
        "{rows:?}"
    );
    assert!(rows[on_limit + 2].0 > 2700, "{rows:?}");
    assert!(enemy::faces_right(obj(&sim, id)));
    assert_eq!(fdata(&sim, id).enemy.vx, f64::from(fish::SWIM_SPEED));
    assert!(rows.iter().all(|r| r.1 == pool.line));
}

#[test]
fn swimming_towards_a_claw_above_its_line_it_leaps() {
    // Claw 80 px to its left, a pixel above its line.
    let (mut sim, id) = with_claw(arena(POOL), 3, POOL.at - 80);
    // 80 and above: nothing.
    swim_with(&mut sim, id, 80);
    assert_eq!(fdata(&sim, id).enemy.mode, 3);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Swim));
    swim_with(&mut sim, id, 79);
    assert_eq!(state_of(&sim, id), Some(EnemyState::JumpStrike));
    let d = fdata(&sim, id);
    assert!((d.enemy.vx + 325.68).abs() < 0.01, "{}", d.enemy.vx);
    assert!((d.enemy.vy + 786.3).abs() < 0.1, "{}", d.enemy.vy);
    assert_eq!(d.enemy.ay, fish::LEAP_GRAVITY);
    assert_eq!(obj(&sim, id).physics_type, ptype::FLY);
    let x0 = obj(&sim, id).x;
    let mut rows = Vec::new();
    let mut sounds = Vec::new();
    for _ in 0..60 {
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        rows.push((o.x, o.y, state_of(&sim, id)));
        sounds.push((
            played(&sim, fish::CHOMP_SOUND),
            played(&sim, fish::IN_SOUND),
        ));
        if state_of(&sim, id) == Some(EnemyState::Swim) {
            break;
        }
    }
    // Entered here between two ticks, it moves from the next: 44 ticks of
    // flight (in play the tick that enters it is the first), back on its
    // line on the tick after them, 155 px up at the top, about 256 px on.
    let leap = rows
        .iter()
        .take_while(|r| r.2 == Some(EnemyState::JumpStrike))
        .count();
    assert_eq!(leap, fish::LEAP_TICKS as usize);
    let top = rows.iter().map(|r| r.1).min().unwrap();
    assert_eq!(POOL.line - top, 155);
    let back = rows.last().unwrap();
    assert_eq!((back.1, back.2), (POOL.line, Some(EnemyState::Swim)));
    assert!((x0 - back.0 - 256).abs() <= 4, "{}", x0 - back.0);
    // The bite on the strike's 20th call (its picture 202), once; the
    // splash back in on the last tick.
    let bites: Vec<usize> = (0..sounds.len()).filter(|&k| sounds[k].0).collect();
    assert_eq!(bites, [19]);
    assert!(sounds.last().unwrap().1);
    // It swims on the way it went.
    assert_eq!(fdata(&sim, id).enemy.vx, -f64::from(fish::SWIM_SPEED));
}

#[test]
fn swimming_away_it_does_not_leap() {
    let (mut sim, id) = with_claw(arena(POOL), 4, POOL.at + 80);
    sim.with_world(|w| enemy::pre_tick(w, id, &fish::CLASS));
    assert_eq!(fdata(&sim, id).enemy.mode, 3);
    let before = sim.rng_state();
    sim.with_world(|w| fish::swim_tick(w, id));
    assert_eq!(sim.rng_state(), before, "no draw");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Swim));
}

#[test]
fn towards_a_claw_level_or_below_it_dashes_to_where_he_stood() {
    let pool = Pool { line: 880, ..POOL };
    let (mut sim, id) = with_claw(arena(pool), 5, pool.at - 80);
    swim_with(&mut sim, id, 90);
    assert_eq!(fdata(&sim, id).enemy.mode, 3);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Swim));
    swim_with(&mut sim, id, 89);
    assert_eq!(state_of(&sim, id), Some(fish::DASH));
    let claw_x = sim.player().unwrap().x;
    assert_eq!(fdata(&sim, id).target, claw_x);
    let rows = run(&mut sim, id, 40);
    let dash = rows.iter().take_while(|r| r.2 == Some(fish::DASH)).count();
    // 3.24 px a tick on its line, to Claw's x of the dash's start.
    assert!(rows[..dash].iter().all(|r| r.1 == pool.line));
    let end = rows[dash];
    assert_eq!(end.2, Some(EnemyState::Swim));
    assert!(end.0 <= claw_x, "{end:?}");
    assert!((rows[0].0 - rows[9].0 - 29).abs() <= 1, "{rows:?}");
    // Claw out of its home: no draw.
    let pool = Pool {
        line: 880,
        range: (2950, 3300),
        ..POOL
    };
    let (mut sim, id) = with_claw(arena(pool), 5, 2930);
    sim.with_world(|w| enemy::pre_tick(w, id, &fish::CLASS));
    let before = sim.rng_state();
    sim.with_world(|w| fish::swim_tick(w, id));
    assert_eq!(sim.rng_state(), before);
}

#[test]
fn a_wall_in_its_leap_turns_it_back_into_its_pool() {
    // A wall from the top to the floor at x 3008 to 3071.
    let mut rows: Vec<String> = vec![".".repeat(200); 15];
    for r in rows.iter_mut() {
        r.replace_range(47..48, "#");
    }
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let pool = Pool {
        at: 3200,
        line: 900,
        range: (3080, 3700),
    };
    let (mut sim, id) = with_claw(level_with(TileMap::from_ascii(64, &rows), pool), 6, 3120);
    swim_with(&mut sim, id, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::JumpStrike));
    let mut xs = vec![obj(&sim, id).x];
    for _ in 0..60 {
        sim.step(TickInput::default());
        xs.push(obj(&sim, id).x);
        if state_of(&sim, id) != Some(EnemyState::JumpStrike) {
            break;
        }
    }
    // Its box meets the wall: from then it moves right at half speed and
    // comes down within its range, back in the water.
    let turn = xs.windows(2).position(|p| p[1] > p[0]).expect("a turn");
    let after: Vec<i32> = xs[turn..xs.len() - 1]
        .windows(2)
        .map(|p| p[1] - p[0])
        .collect();
    assert!(after.iter().all(|&d| (2..=3).contains(&d)), "{xs:?}");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Swim));
    let x = obj(&sim, id).x;
    assert!((pool.range.0..=pool.range.1).contains(&x));
}

#[test]
fn a_leap_onto_dry_ground_leaves_it_flopping_back_to_the_water() {
    // Near its left limit, leaping left: it comes down on the floor left
    // of its range.
    let pool = Pool { at: 2760, ..POOL };
    let (mut sim, id) = with_claw(arena(pool), 7, pool.at - 80);
    swim_with(&mut sim, id, 0);
    let mut flop_at = None;
    for k in 0..120 {
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::OutOfWater) {
            flop_at = Some(k);
            break;
        }
    }
    let flop_at = flop_at.expect("a flop");
    assert!(flop_at > fish::LEAP_TICKS as usize);
    assert!(played(&sim, fish::FLOP_SOUND));
    assert_eq!(fdata(&sim, id).enemy.vx, f64::from(fish::FLOP_SPEED));
    let x0 = obj(&sim, id).x;
    assert!(x0 < pool.range.0, "{x0}");
    // A flop every 250 ms (14 ticks) facing by the shared toggle, which
    // flips at every flop; then back in once within its range below its
    // line.
    let mut facings = Vec::new();
    let mut toggles = Vec::new();
    let mut n = 0;
    while state_of(&sim, id) == Some(EnemyState::OutOfWater) && n < 400 {
        sim.step(TickInput::default());
        n += 1;
        if played(&sim, fish::FLOP_SOUND) {
            facings.push(enemy::faces_right(obj(&sim, id)));
            toggles.push(sim.with_world(|w| w.fish_flop()));
        }
    }
    assert!(facings.len() >= 2, "{facings:?}");
    // The toggle starts at 1: the first flop faces left and leaves it at 0.
    assert_eq!((facings[0], toggles[0]), (false, 0));
    assert_eq!((facings[1], toggles[1]), (true, 1));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Swim));
    assert!(played(&sim, fish::IN_SOUND));
    assert_eq!(obj(&sim, id).y, pool.line);
    assert!(obj(&sim, id).x >= pool.range.0);
}

#[test]
fn its_touch_costs_claw_10() {
    // Claw within its 50 px box: it only swims, and swims into him.
    let (mut sim, id) = with_claw(arena(POOL), 8, POOL.at - 30);
    sim.with_world(|w| w.claw.stats.health = 100);
    sim.step(TickInput::default());
    assert_eq!(fdata(&sim, id).enemy.mode, 4);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Swim));
    assert_eq!(sim.claw().stats.health, 90);
}

/// An attacker of type `kind`, at the fish.
fn attacker(sim: &mut Sim, kind: u32) -> ObjectId {
    sim.with_world(|w| {
        let mut o = logic::new_object("TestBlow", POOL.at, POOL.line, 0);
        o.kind = LogicKind::Inert;
        o.type_flags = kind;
        o.damage = 1;
        o.flags |= flags::NO_COLLIDE;
        w.spawn(o)
    })
}

#[test]
fn any_hit_kills_it_at_once() {
    for kind in [
        types::PLAYER,
        types::PISTOL_SHOT,
        types::LEVEL_SHOT,
        0x200_0000,
    ] {
        let (mut sim, id) = with_claw(arena(POOL), 9, 3600);
        let a = attacker(&mut sim, kind);
        sim.with_world(|w| fish::on_hit(w, id, a));
        assert_eq!(state_of(&sim, id), Some(EnemyState::Dead), "{kind:x}");
        let used = obj(&sim, a).flags & flags::DESTROY != 0;
        assert_eq!(used, kind == types::PISTOL_SHOT, "{kind:x}");
        // Dead, it ignores every later hit: a shot is not used up.
        let b = attacker(&mut sim, types::PISTOL_SHOT);
        let before = sim.rng_state();
        sim.with_world(|w| fish::on_hit(w, id, b));
        assert_eq!(sim.rng_state(), before);
        assert_eq!(obj(&sim, b).flags & flags::DESTROY, 0);
    }
}
