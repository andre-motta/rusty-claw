//! The mid-air hit jump on the first level of your own install (skipped
//! without `CLAW_INSTALL`), with the level's own animations: a soldier's
//! musket ball or Claw's own dynamite hits him in the air, and once the
//! hit reaction is over a jump press starts a second jump by default, as
//! in the original, and not with the mid-air hit jump fix on. The hit holds
//! Claw for 22 ticks and the jump is taken on the one tick after the stand,
//! as in the original at its 18 ms step; Claw's own stick of dynamite goes
//! off 70 ticks after the throw and hurts 9 ticks later, or at the return
//! from a help screen long enough to burn its fuse.

use std::sync::Arc;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{Buttons, TickInput};
use claw_sim::logic::enemy::soldier;
use claw_sim::logic::projectile::{self, ProjectileSpec};
use claw_sim::object::{ObjectId, types};
use claw_sim::physics::ptype;
use claw_sim::player::state;
use claw_sim::tuning::FuseClock;
use claw_sim::{Sim, SimConfig, SimLevel};

const JUMP: u32 = 0x1;

fn level1() -> Option<Arc<SimLevel>> {
    let Some(path) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let install = Install::open(path).expect("install opens");
    let store = install.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(1)).expect("level 1 loads");
    Some(Arc::new(
        SimLevel::from_level(&level, &store).expect("level 1 prepares"),
    ))
}

fn press(mask: u32) -> TickInput {
    TickInput {
        held: Buttons(mask),
        pressed: Buttons(mask),
    }
}

fn hold(mask: u32) -> TickInput {
    TickInput {
        held: Buttons(mask),
        pressed: Buttons::NONE,
    }
}

/// Claw settled at the level's start, with the fix on or off.
fn settled(level: &Arc<SimLevel>, fix: bool) -> Sim {
    let mut cfg = SimConfig::default();
    cfg.tuning.behaviour.fix_double_jump = fix;
    let mut sim = Sim::new(level.clone(), cfg);
    for _ in 0..60 {
        sim.step(TickInput::default());
    }
    assert_eq!(sim.player().unwrap().state, state::STAND);
    sim
}

fn launch(sim: &mut Sim, spec: ProjectileSpec) -> ObjectId {
    sim.with_world(|w| projectile::spawn(w, spec))
        .expect("launched")
}

/// A soldier's musket ball flying at Claw from just in front of him.
fn musket_ball(x: i32, y: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: soldier::BALL_LOGIC,
        x: x + 20,
        y,
        z: 300,
        vx: -soldier::BALL_SPEED,
        damage: soldier::BALL_DAMAGE,
        type_flags: types::ENEMY_SHOT,
        image_set: soldier::BALL_IMAGE,
        ..ProjectileSpec::default()
    }
}

/// Claw's own stick of dynamite, dropped beside him.
fn dynamite(x: i32, y: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: "Dynamite",
        x: x + 20,
        y: y + 40,
        z: 101,
        damage: 15,
        type_flags: types::PISTOL_SHOT,
        hit_type_flags: types::ENEMY,
        image_set: "GAME_DYNAMITELIT",
        ..ProjectileSpec::default()
    }
}

/// Runs until the hit reaction that has just started is over; Claw must
/// have stayed in the air.
fn through_the_reaction(sim: &mut Sim, ground: i32) {
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::HIT);
    assert_eq!(o.physics_type, ptype::FALL, "hit in the air");
    for _ in 0..200 {
        if sim.player().unwrap().state != state::HIT {
            break;
        }
        sim.step(TickInput::default());
    }
    let o = sim.player().unwrap();
    assert_ne!(o.state, state::HIT, "the reaction ends");
    assert!(o.y < ground - 10, "still in the air: {} over {ground}", o.y);
}

/// Jumps, and near the top an enemy's shot hits Claw.
fn shot_in_mid_air(level: &Arc<SimLevel>, fix: bool) -> Sim {
    let mut sim = settled(level, fix);
    let ground = sim.player().unwrap().y;
    sim.step(press(JUMP));
    for _ in 0..8 {
        sim.step(hold(JUMP));
    }
    let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
    let health = sim.stats().health;
    launch(&mut sim, musket_ball(x, y));
    for _ in 0..10 {
        sim.step(TickInput::default());
        if sim.stats().health < health {
            break;
        }
    }
    assert_eq!(sim.stats().health, health - soldier::BALL_DAMAGE);
    through_the_reaction(&mut sim, ground);
    sim
}

/// Drops a stick of dynamite beside Claw and jumps just before it goes
/// off, so the blast catches him in the air.
fn own_dynamite_in_mid_air(level: &Arc<SimLevel>, fix: bool) -> Sim {
    // When the blast hurts a Claw who stands still.
    let mut dry = settled(level, fix);
    let (x, y) = dry.player().map(|o| (o.x, o.y)).unwrap();
    let health = dry.stats().health;
    launch(&mut dry, dynamite(x, y));
    let at = (1..300)
        .find(|_| {
            dry.step(TickInput::default());
            dry.stats().health < health
        })
        .expect("the dynamite goes off");
    let mut sim = settled(level, fix);
    let ground = sim.player().unwrap().y;
    launch(&mut sim, dynamite(x, y));
    for n in 1..at {
        sim.step(if n == at - 3 {
            press(JUMP)
        } else {
            TickInput::default()
        });
    }
    sim.step(TickInput::default());
    assert!(sim.stats().health < health, "the blast hit");
    through_the_reaction(&mut sim, ground);
    sim
}

fn jumps_again(sim: &mut Sim) {
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::STAND, "standing in mid-air");
    let y = o.y;
    sim.step(press(JUMP));
    sim.step(hold(JUMP));
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::JUMP, "a second jump");
    assert!(o.y < y, "he rises: {} from {y}", o.y);
}

fn cannot_jump_again(sim: &mut Sim) {
    let y = sim.player().unwrap().y;
    sim.step(press(JUMP));
    sim.step(hold(JUMP));
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::FALL, "no second jump");
    assert!(o.y >= y, "he falls: {} from {y}", o.y);
}

#[test]
fn install_a_soldiers_shot_in_mid_air_allows_a_second_jump_unless_fixed() {
    let Some(level) = level1() else { return };
    jumps_again(&mut shot_in_mid_air(&level, false));
    cannot_jump_again(&mut shot_in_mid_air(&level, true));
}

#[test]
fn install_his_own_dynamite_in_mid_air_allows_a_second_jump_unless_fixed() {
    let Some(level) = level1() else { return };
    jumps_again(&mut own_dynamite_in_mid_air(&level, false));
    cannot_jump_again(&mut own_dynamite_in_mid_air(&level, true));
}

/// A hit in the air from `source` (0 an enemy's shot, 1 Claw's own
/// dynamite), with `input(t)` played on tick `t` counted from the hit tick
/// (0); Claw's (state, y) after each tick from the hit tick on.
fn after_a_hit(
    level: &Arc<SimLevel>,
    source: u8,
    input: &dyn Fn(u64) -> TickInput,
) -> Vec<(i32, i32)> {
    let mut sim = settled(level, false);
    let health = sim.stats().health;
    let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
    let mut jump_at = None;
    if source == 0 {
        sim.step(press(JUMP));
        for _ in 0..8 {
            sim.step(hold(JUMP));
        }
        let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
        launch(&mut sim, musket_ball(x, y));
    } else {
        // When the blast hurts a Claw who stands still; he jumps 3 ticks
        // before it.
        let mut dry = settled(level, false);
        launch(&mut dry, dynamite(x, y));
        let at = (1..300)
            .find(|_| {
                dry.step(TickInput::default());
                dry.stats().health < health
            })
            .expect("the dynamite goes off");
        launch(&mut sim, dynamite(x, y));
        jump_at = Some(sim.tick() + at as u64 - 3);
    }
    let mut hit = None;
    let mut rows = Vec::new();
    for _ in 0..400 {
        let n = sim.tick();
        let i = match hit {
            Some(h) => input(n - h),
            None if Some(n) == jump_at => press(JUMP),
            None => TickInput::default(),
        };
        sim.step(i);
        if hit.is_none() && sim.stats().health < health {
            hit = Some(n);
            let o = sim.player().unwrap();
            assert_eq!(o.physics_type, ptype::FALL, "hit in the air");
        }
        if hit.is_some() {
            let o = sim.player().unwrap();
            rows.push((o.state, o.y));
            if rows.len() > 30 {
                break;
            }
        }
    }
    assert!(hit.is_some(), "the hit came");
    rows
}

#[test]
fn install_the_stand_window_after_a_hit_in_the_air_is_the_tick_after_the_stand() {
    let Some(level) = level1() else { return };
    for source in [0, 1] {
        let rows = after_a_hit(&level, source, &|_| TickInput::default());
        let y = rows[0].1;
        for (t, &(s, ry)) in rows.iter().enumerate().take(22) {
            assert_eq!((s, ry), (state::HIT, y), "source {source}: tick h+{t}");
        }
        assert_eq!(
            rows[22],
            (state::STAND, y),
            "source {source}: stand on h+22"
        );
        assert_eq!(rows[23].0, state::FALL, "source {source}: fall from h+23");
        let rows = after_a_hit(&level, source, &|t| {
            if t == 23 { press(JUMP) } else { hold(0) }
        });
        assert_eq!(rows[23].0, state::JUMP, "source {source}: jump on h+23");
        assert!(rows[30].1 < y - 40, "source {source}: a full jump");
        for at in (19..=22).chain(24..=26) {
            let rows = after_a_hit(&level, source, &|t| {
                if t == at { press(JUMP) } else { hold(0) }
            });
            assert!(
                rows.iter().all(|&(s, _)| s != state::JUMP),
                "source {source}: a press on h+{at} launched"
            );
        }
    }
}

/// Claw throws a stick of dynamite at the level's start (a tap of the
/// dynamite key); with `help = Some((after, ms))` a help screen of `ms`
/// is up at the boundary `after` ticks after the throw tick. Returns the
/// throw tick, the tick the stick went off in (`None` when it went off at
/// the return from help) and the tick its blast hurt Claw.
fn throw_and_wait(
    level: &Arc<SimLevel>,
    clock: FuseClock,
    help: Option<(u64, u32)>,
) -> (u64, Option<u64>, u64) {
    let cfg = SimConfig {
        fuse_clock: clock,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level.clone(), cfg);
    for _ in 0..60 {
        sim.step(TickInput::default());
    }
    let sticks = sim.stats().dynamite;
    let health = sim.stats().health;
    let mut throw = None;
    let mut went_off = None;
    for i in 0..400 {
        if let (Some(t), Some((after, ms))) = (throw, help)
            && sim.tick() == t + after
        {
            sim.pass_menu_time(ms);
        }
        let live = |sim: &Sim| {
            sim.objects()
                .all()
                .filter_map(|(_, o)| projectile::data(o))
                .any(|d| d.phase == projectile::Phase::Exploding)
        };
        let before = live(&sim);
        let n = sim.tick();
        sim.step(if i == 0 {
            press(Buttons::DYNAMITE.0)
        } else {
            TickInput::default()
        });
        if throw.is_none() && sim.stats().dynamite < sticks {
            throw = Some(n);
        }
        if !before && went_off.is_none() && live(&sim) {
            went_off = Some(n);
        }
        if sim.stats().health < health {
            return (throw.expect("thrown"), went_off, n);
        }
    }
    panic!("the blast never hurt Claw");
}

#[test]
fn install_his_dynamite_goes_off_70_ticks_after_the_throw_tick_or_at_the_return_from_help() {
    let Some(level) = level1() else { return };
    let (t, went_off, hurt) = throw_and_wait(&level, FuseClock::Original, None);
    assert_eq!(went_off, Some(t + 70), "lit in the throw tick");
    assert_eq!(hurt, t + 79, "hurts 9 ticks after it goes off");
    // Help after 18 ticks of fuse (324 ms) and up for 918 ms: 1242 ms, it
    // goes off on its own tick; for 936 ms (1260 ms) it goes off at the
    // return and hurts on the 8th tick after it.
    let (t, went_off, hurt) = throw_and_wait(&level, FuseClock::Original, Some((19, 918)));
    assert_eq!((went_off, hurt), (Some(t + 70), t + 79));
    let (t, went_off, hurt) = throw_and_wait(&level, FuseClock::Original, Some((19, 936)));
    assert_eq!((went_off, hurt), (None, t + 19 + 8));
    // With the fuse clock frozen help freezes the fuse.
    let (t, went_off, hurt) = throw_and_wait(&level, FuseClock::Frozen, Some((19, 2000)));
    assert_eq!((went_off, hurt), (Some(t + 70), t + 79));
}
