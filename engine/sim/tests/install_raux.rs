//! The second level's boss fight on your own install, from the door to the
//! level's end: the introduction in the original's order and timing, Raux
//! fought down with the health bar falling blow by blow, the gem his defeat
//! releases, and the end of the level when Claw collects it, with every
//! picture drawn on the way among those loaded with the level; then the
//! boss room's stretches of the reference store played against their
//! reference traces of the same route, in the same trace format.
//!
//! Needs `CLAW_INSTALL`, and `CLAW_REFERENCE` for the comparison; each
//! test skips itself when what it needs is unset. Nothing from an install
//! or a reference store is kept in the repository.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::claw_frame::output::BossMeter;
use claw_sim::claw_frame::{AudioCue, Buttons, GameEvent, SimSnapshot};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::boss_stage;
use claw_sim::logic::enemy::EnemyState;
use claw_sim::logic::pickup;
use claw_sim::logic::world_boss::{AMULET_SOUND, fight};
use claw_sim::object::ObjectId;
use claw_sim::player::special::BOSS_END;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Comparison, Tolerances, TraceRow};
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// Where the boss room's stretches start: next to the door, where the
/// reference routes of the boss room start too.
const START: (i32, i32) = (20150, 2091);
/// Raux's place, standing in his room.
const RAUX_AT: (i32, i32) = (23970, 3955);
/// Where the scripted fight below leaves Raux when he falls, on his
/// room's floor 77 px right of his place. A change here means the fight
/// played differently (a blow, a step or a jump of his came at another
/// time), which the gem's flight from wherever he fell would not show.
const RAUX_FELL_AT: (i32, i32) = (24047, 3955);
/// The gem's target, from Raux's level record.
const GEM_TARGET: (i32, i32) = (23935, 3800);
const ATTACK: u32 = 0x2;

fn install_store() -> Option<claw_assets::AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    Some(Install::open(&root).unwrap().store().unwrap())
}

fn level2() -> Option<(SimLevel, LevelData)> {
    let store = install_store()?;
    let data = LevelData::build(&store, LevelSource::Retail(2)).unwrap();
    Some((SimLevel::from_level(&data, &store).unwrap(), data))
}

/// Moves Claw to (x, y), lets him and the camera settle, then starts the
/// shared random stream from `seed`: the next tick is the stretch's time
/// zero, as the comparison tool plays a stretch of a level.
fn settle_at(sim: &mut Sim, (x, y): (i32, i32), seed: u32) {
    sim.teleport_player(x, y);
    let mut still = 0;
    let mut last = None;
    for _ in 0..222 {
        sim.step(TickInput::default());
        let now = sim
            .player()
            .map(|o| (o.x, o.y, o.state, sim.camera().current));
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still >= 10 {
            break;
        }
    }
    sim.with_world(|w| *w.rng = Rng::new(seed));
}

fn find(sim: &Sim, logic: &str) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
}

fn raux_state(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    sim.objects()
        .get(id)
        .and_then(|o| EnemyState::from_id(o.state))
}

fn plays(sim: &Sim, key: &str) -> bool {
    sim.last_audio()
        .iter()
        .any(|c| matches!(c, AudioCue::Play(s) if s.key == key))
}

fn music_cue(sim: &Sim) -> bool {
    sim.last_audio()
        .iter()
        .any(|c| matches!(c, AudioCue::Music { .. }))
}

fn level_complete(sim: &Sim) -> bool {
    sim.last_events()
        .iter()
        .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
}

fn input(held: u32, pressed: u32) -> TickInput {
    TickInput {
        held: Buttons(held),
        pressed: Buttons(pressed),
    }
}

/// The sets a game drawing this level loads before play, as the game
/// does: the placed objects' sets, Claw's, the sets the logics switch
/// objects to while the level runs, and those of the first picture.
fn loaded_sets(data: &LevelData, first: &SimSnapshot) -> BTreeSet<String> {
    data.objects
        .iter()
        .map(|o| o.image_set.clone())
        .chain(
            claw_sim::level::RUNTIME_IMAGE_SETS
                .iter()
                .map(|s| s.to_string()),
        )
        .chain(
            claw_sim::logic::runtime_image_sets()
                .into_iter()
                .map(str::to_string),
        )
        .chain(first.renderables.iter().map(|r| r.image_set_key.clone()))
        .filter(|k| !k.is_empty())
        .collect()
}

/// Steps the game and counts the pictures drawn that the game would not
/// have loaded with the level.
struct Drawn {
    loaded: BTreeSet<String>,
    later: BTreeSet<String>,
    music_cues: usize,
}

impl Drawn {
    /// One tick with `i`, its keys pressed in the tick they go down as on
    /// a keyboard (a walk from standing starts on the press).
    fn step(&mut self, sim: &mut Sim, i: TickInput) {
        let down = i.held.0 & !sim.input().held.0;
        let i = TickInput {
            held: i.held,
            pressed: Buttons(i.pressed.0 | down),
        };
        let snap = sim.step(i);
        for r in &snap.renderables {
            if !r.image_set_key.is_empty() && !self.loaded.contains(&r.image_set_key) {
                self.later.insert(r.image_set_key.clone());
            }
        }
        if music_cue(sim) {
            self.music_cues += 1;
        }
    }
}

/// What one tick of the fight is decided from.
struct Seen {
    claw_x: i32,
    claw_state: i32,
    facing_right: bool,
    raux: Option<(i32, EnemyState)>,
    pickup_x: Option<i32>,
}

fn seen(sim: &Sim, raux: ObjectId) -> Seen {
    let p = sim.player().unwrap();
    Seen {
        claw_x: p.x,
        claw_state: p.state,
        facing_right: sim.claw().facing_right,
        raux: sim
            .objects()
            .get(raux)
            .and_then(|o| EnemyState::from_id(o.state).map(|s| (o.x, s))),
        pickup_x: find(sim, pickup::END_OF_LEVEL_LOGIC)
            .and_then(|id| sim.objects().get(id))
            .map(|o| o.x),
    }
}

/// The scripted player of the fight, decided each tick from what is on
/// screen: walk at Raux, turn to him and swing every eighth tick once
/// within 60 px; once the gem has landed, walk under it and jump with Jump
/// held.
fn fight_input(s: &Seen, k: u64) -> TickInput {
    let toward = |dx: i32| if dx > 0 { RIGHT } else { LEFT };
    if let Some(gx) = s.pickup_x {
        if !matches!(
            s.claw_state,
            state::STAND | state::MOVE_RIGHT | state::JUMP | state::FALL
        ) {
            return TickInput::default();
        }
        let dx = gx - s.claw_x;
        if dx.abs() > 4 {
            return input(toward(dx), 0);
        }
        let press = if s.claw_state == state::STAND {
            JUMP
        } else {
            0
        };
        return input(JUMP, press);
    }
    match s.raux {
        Some((rx, st)) if st != EnemyState::Dead && s.claw_state != BOSS_END => {
            let dx = rx - s.claw_x;
            if dx.abs() > 60 {
                input(toward(dx), 0)
            } else if (dx > 0) != s.facing_right {
                input(toward(dx), toward(dx))
            } else if k.is_multiple_of(8) {
                input(ATTACK, ATTACK)
            } else {
                TickInput::default()
            }
        }
        _ => TickInput::default(),
    }
}

/// The ticks from a boss's defeat at `from` until the gem it releases
/// lands: it flies on the slowest arc to [`GEM_TARGET`] from the tick
/// after the defeat, one tick of constant acceleration at a time, and
/// lands in the first tick that finds it moving down below the target.
fn gem_flight_ticks(from: (i32, i32)) -> u64 {
    let mut pos = (f64::from(from.0), f64::from(from.1));
    let mut vel = pickup::end_gem_launch(pos, GEM_TARGET);
    let dt = 0.018;
    let g = pickup::END_GEM_GRAVITY;
    let mut k = 1;
    while !(vel.1 > 0.0 && pos.1 > f64::from(GEM_TARGET.1)) {
        pos.0 += vel.0 * dt;
        pos.1 += vel.1 * dt + 0.5 * g * dt * dt;
        vel.1 += g * dt;
        k += 1;
        assert!(k < 1000);
    }
    k
}

/// The original game's steps of the introduction on this level, counted
/// from the stretch's time zero: his line, the camera back to Claw,
/// Claw's line, the banner, its landing, the flag at 2, and Claw and Raux
/// moving again.
const INTRODUCTION: [u64; 7] = [95, 258, 306, 421, 548, 696, 697];

#[test]
fn install_level2_the_staged_fight_ends_the_level_through_the_gem() {
    let Some((level, data)) = level2() else {
        return;
    };
    // The original's speeds at its 18 ms step, as in the reference runs
    // the steps below come from.
    let cfg = SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level, cfg);
    settle_at(&mut sim, START, 1);
    let mut d = Drawn {
        loaded: loaded_sets(&data, &sim.snapshot()),
        later: BTreeSet::new(),
        music_cues: 0,
    };

    // The walk through the door: 15 ticks still, then Right held, as the
    // reference routes do.
    for _ in 0..15 {
        d.step(&mut sim, TickInput::default());
    }
    let mut t = 15u64;
    let mut door = None;
    let mut trigger = None;
    while trigger.is_none() {
        assert!(t < 200, "the trigger fires once Claw is through the door");
        d.step(&mut sim, input(RIGHT, if t == 15 { RIGHT } else { 0 }));
        if door.is_none() && music_cue(&sim) {
            door = Some(t);
        }
        if sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::Checkpoint { .. }))
        {
            trigger = Some(t);
        }
        t += 1;
    }
    let door = door.expect("the boss music with the door");
    let trigger = trigger.unwrap();
    // The original: the door touched on step 40 takes Claw into the room
    // before step 41, the first step there, which has the boss music (the
    // stager woke with the room) and the trigger.
    assert_eq!((door, trigger), (41, 41));
    let music_before = d.music_cues;
    let raux = sim.boss().current().expect("Raux is built in his room");
    assert_eq!(&*sim.objects().get(raux).unwrap().logic, "Raux");

    // The introduction, tick by tick from the trigger: the flag goes 0, 1,
    // 2; Claw is frozen, the camera visits Raux, both speak, the banner
    // drops and leaves, and both start to move together.
    let mut flags = vec![sim.boss().fight()];
    let mut seen_at: [Option<u64>; 7] = [None; 7];
    let mut cam_on_raux = None;
    let mut k = 1u64;
    loop {
        assert!(k < 800, "the introduction ends");
        d.step(&mut sim, TickInput::default());
        let f = sim.boss().fight();
        if flags.last() != Some(&f) {
            flags.push(f);
        }
        if k == 1 {
            assert_eq!(f, fight::STAGING);
            assert_eq!(sim.player().unwrap().state, state::FREEZE);
            assert_eq!(
                sim.hud().boss_meter,
                Some(BossMeter {
                    value: 100,
                    max: 100
                })
            );
            assert_eq!(sim.camera_override(), Some(RAUX_AT));
        }
        if cam_on_raux.is_none() && sim.camera().current.0 == RAUX_AT.0 {
            cam_on_raux = Some(k + trigger);
        }
        let banner = sim.hud().boss_banner;
        let now = [
            plays(&sim, boss_stage::ENEMY_LINE),
            k > 1 && sim.camera_override().is_none(),
            plays(&sim, boss_stage::CLAW_LINE),
            banner.is_some(),
            banner.is_some_and(|(_, y)| y == 160),
            f == fight::ON,
            f == fight::ON && sim.player().unwrap().state == state::STAND,
        ];
        for (at, now) in seen_at.iter_mut().zip(now) {
            if now && at.is_none() {
                *at = Some(k + trigger);
            }
        }
        if seen_at[3] == Some(k + trigger) {
            assert_eq!(banner, Some((320, -220)));
        }
        if seen_at[6].is_some() {
            assert_eq!(raux_state(&sim, raux), Some(EnemyState::Idle));
            break;
        }
        assert_ne!(sim.player().unwrap().state, state::STAND, "tick {k}");
        k += 1;
    }
    println!(
        "introduction: camera on Raux at {cam_on_raux:?} (the original: 87); his line, camera \
         back, Claw's line, banner, landing, flag 2, thaw at {seen_at:?} (the original: \
         {INTRODUCTION:?})"
    );
    assert_eq!(flags, vec![fight::NONE, fight::STAGING, fight::ON]);
    assert_eq!(seen_at, INTRODUCTION.map(Some));
    assert_eq!(
        cam_on_raux,
        Some(87),
        "the camera reached Raux as in the original"
    );

    // The fight, blow by blow: the bar follows Raux's health against the
    // full health it copied, until his death.
    let mut healths = vec![100];
    let mut bar = vec![100];
    let mut k = 0u64;
    let body = loop {
        assert!(k < 6000, "Raux falls");
        let s = seen(&sim, raux);
        d.step(&mut sim, fight_input(&s, k));
        // Claw is kept on his feet: this test is about Raux.
        sim.with_world(|w| {
            if w.claw.stats.health < 50 {
                w.claw.stats.health = 100;
            }
        });
        let o = sim.objects().get(raux).unwrap().clone();
        if healths.last() != Some(&o.health) {
            healths.push(o.health);
        }
        if let Some(m) = sim.hud().boss_meter {
            assert_eq!(m.max, 100);
            if bar.last() != Some(&m.value) {
                bar.push(m.value);
            }
        }
        if raux_state(&sim, raux) == Some(EnemyState::Dead) {
            break o;
        }
        k += 1;
    };
    println!(
        "Raux fell after {k} ticks of the fight: health {healths:?} at {:?}",
        (body.x, body.y)
    );
    assert!(healths.len() >= 3, "several blows");
    assert!(body.health <= 0);
    assert_eq!(
        (body.x, body.y),
        RAUX_FELL_AT,
        "Raux falls where the scripted fight left him before"
    );
    // The bar showed every health he had while he lived.
    assert_eq!(bar[..healths.len() - 1], healths[..healths.len() - 1]);

    // His defeat: Claw held at once, the gem out of him and its sound.
    assert_eq!(sim.player().unwrap().state, BOSS_END);
    assert_eq!(sim.boss().defeated(), Some(raux));
    assert!(plays(&sim, AMULET_SOUND));
    let gem = find(&sim, pickup::END_GEM).expect("the gem");
    let g = sim.objects().get(gem).unwrap();
    assert_eq!((g.x, g.y), (body.x, body.y), "out of Raux");
    let held_at = sim.player().unwrap().x;

    // The gem's flight: the camera follows it until it lands; then Claw is
    // let go and it is the end-of-level pickup. Raux's body leaves on the
    // way, and the bar the tick after.
    let mut k = 1u64;
    let mut gone = None;
    let mut bar_gone = None;
    loop {
        assert!(k < 400, "the gem lands");
        let before = sim.objects().get(gem).map(|o| (o.x, o.y)).unwrap();
        d.step(&mut sim, TickInput::default());
        if gone.is_none() && sim.boss().current().is_none() {
            gone = Some(k);
        }
        if bar_gone.is_none() && sim.hud().boss_meter.is_none() {
            bar_gone = Some(k);
        }
        let o = sim.objects().get(gem).unwrap();
        if &*o.logic == pickup::END_OF_LEVEL_LOGIC {
            break;
        }
        assert_eq!(sim.player().unwrap().state, BOSS_END);
        assert_eq!(sim.player().unwrap().x, held_at);
        assert_eq!(sim.camera_override(), Some(before), "the camera on the gem");
        k += 1;
    }
    let o = sim.objects().get(gem).unwrap();
    println!(
        "the gem landed {k} ticks after Raux fell (the original: 101 from his place), at {:?}; \
         his body went after {gone:?} ticks, the bar after {bar_gone:?}",
        (o.x, o.y)
    );
    // As in the original: the gem lands 101 ticks after the defeat when
    // Raux falls at his place (here, the ticks of the same arc from where
    // he fell), 1 px short of its target; the body leaves after 50 ticks
    // (out of view) and the bar on the tick after.
    assert_eq!(gem_flight_ticks(RAUX_AT), 101);
    assert_eq!(k, gem_flight_ticks((body.x, body.y)));
    assert_eq!((o.x, o.y), (GEM_TARGET.0 - 1, GEM_TARGET.1));
    assert_eq!((gone, bar_gone), (Some(50), Some(51)));
    assert_eq!(sim.player().unwrap().state, state::STAND);
    assert_eq!(sim.camera_override(), None);

    // Claw jumps into it and the level ends.
    let mut k = 0u64;
    loop {
        assert!(k < 600, "Claw collects the gem");
        let s = seen(&sim, raux);
        d.step(&mut sim, fight_input(&s, k));
        if level_complete(&sim) {
            break;
        }
        k += 1;
    }
    assert_eq!(d.music_cues, music_before, "the boss music to the end");
    assert_eq!(sim.boss().fight(), fight::ON);
    assert!(
        d.later.is_empty(),
        "pictures loaded while drawing: {:?}",
        d.later
    );
}

// ----------------------------------------------------------------------
// The reference store

/// The boss room's reference routes played without help, with the clocks
/// each was recorded at: the entry with the introduction, the jump-in
/// entry that skips it, a jump one tick too late, and the skip followed by
/// blows.
const STORES: &[(&str, &[&str])] = &[
    ("boss-intro-normal", &["c18", "c60"]),
    ("boss-intro-skip-c18", &["c18"]),
    ("boss-intro-skip", &["c60"]),
    ("boss-intro-skip-late-c18", &["c18"]),
    ("boss-intro-skip-late", &["c60"]),
    ("boss-intro-skip-fight-c18", &["c18"]),
    ("boss-intro-skip-fight", &["c60"]),
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. What they come from:
///
/// - c18: the door takes Claw into the room between two steps, as in the
///   original, so his position, the camera, the trigger, the
///   introduction's flag and the jump-in's landing all come on the
///   original's steps.
/// - c18, 774 ms (rng): the enemies' rolls come on frame pulse ticks, and
///   the pulse is in another phase after the stretch's start than in the
///   original, so Raux draws his first roll a tick earlier. His choices in
///   the fight then differ, which is where Claw's state and health leave
///   the trace (2214 to 24822 ms), and once a blow pushes Claw back into
///   the stager's box after the skip, the introduction runs in the middle
///   of the fight (32148 ms). Played from the level's start, where the
///   pulse is in the original's phase, the whole staging and fight follow
///   the original step for step (the second level's bot route).
/// - c60 (the default tuning played tick by tick): the first arena tick
///   walks 5 px instead of 4, so the trigger's checkpoint is 1 px further
///   (667 ms); the camera's pan and the introduction's delays count 18 ms
///   ticks against frames of 16 and 17 ms, so the flag reaches 2 about
///   350 ms later (12200 ms) and the pan drifts (1267 ms); the jump-in's
///   arc is up to 9 px lower after the timing slack (933 ms), and Claw
///   still lands short of every range box, so Raux stays in his start-up
///   state for the whole run as in the original.
const KNOWN: &[Known] = &[
    (
        "boss-intro-normal",
        "c18",
        &[
            ("rng", 774),
            ("cam_x", 9882),
            ("cam_y", 9900),
            ("state_id", 24822),
            ("health", 24822),
            ("x", 24840),
        ],
    ),
    (
        "boss-intro-normal",
        "c60",
        &[
            ("events", 667),
            ("cam_x", 1267),
            ("boss_fight", 12200),
            ("state_id", 12217),
            ("health", 26167),
            ("x", 26217),
        ],
    ),
    (
        "boss-intro-skip-c18",
        "c18",
        &[
            ("rng", 774),
            ("state_id", 22428),
            ("health", 22428),
            ("x", 22446),
            ("cam_x", 22446),
            ("events", 32130),
            ("cam_y", 32148),
            ("boss_fight", 32148),
        ],
    ),
    ("boss-intro-skip", "c60", &[("y", 933), ("cam_y", 933)]),
    (
        "boss-intro-skip-late-c18",
        "c18",
        &[
            ("rng", 774),
            ("cam_x", 9882),
            ("cam_y", 9900),
            ("state_id", 24822),
            ("health", 24822),
            ("x", 24840),
        ],
    ),
    (
        "boss-intro-skip-late",
        "c60",
        &[
            ("events", 667),
            ("cam_x", 1267),
            ("boss_fight", 12200),
            ("state_id", 12217),
            ("health", 26167),
            ("x", 26217),
        ],
    ),
    (
        "boss-intro-skip-fight-c18",
        "c18",
        &[
            ("rng", 774),
            ("state_id", 2214),
            ("physics_type", 2214),
            ("health", 2214),
            ("x", 2232),
            ("cam_x", 2232),
            ("boss_health", 2736),
        ],
    ),
    (
        "boss-intro-skip-fight",
        "c60",
        &[
            ("y", 933),
            ("cam_y", 933),
            ("state_id", 3633),
            ("health", 3633),
            ("x", 3683),
            ("cam_x", 3683),
        ],
    ),
];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
}

fn meta_value<'a>(meta: &'a str, key: &str) -> Option<&'a str> {
    meta.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().trim_matches('"'))
}

/// Replays the store's route from [`START`] with the clock's tuning and the
/// store's random state at time zero, for as long as the reference runs,
/// and compares it with the clock's first reference trace at the gate's
/// tolerances (exact for c18, feel for c60), as `claw-replay --compare`
/// does.
fn compare_store(level: &SimLevel, dir: &Path, clock: &str) -> Option<Comparison> {
    let meta = std::fs::read_to_string(dir.join("meta.toml")).ok()?;
    let seed: u32 = meta_value(&meta, "rng_at_t0")?.parse().ok()?;
    let route = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let script = InputScript::parse(&route).unwrap();
    let reference =
        trace::parse(&std::fs::read_to_string(dir.join(format!("trace-{clock}-r1.csv"))).ok()?)
            .unwrap();
    let (tuning, tol) = if clock == "c18" {
        (ScriptTuning::Original, Tolerances::exact())
    } else {
        (ScriptTuning::Default, Tolerances::feel())
    };
    let cfg = SimConfig {
        tuning: tuning.tuning(),
        seed,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level.clone(), cfg);
    settle_at(&mut sim, START, seed);
    let end_ms = reference.last().map_or(0, |r| r.t_ms);
    let mut rows = Vec::new();
    let mut tick = 0usize;
    while (tick as u64) * 18 < end_ms + 18 {
        sim.step(script.ticks.get(tick).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, tick as u64, "script"));
        tick += 1;
    }
    let cmp = trace::compare(&rows, &reference, &tol);
    print!("{}", trace::report(&rows, &reference, &cmp, 3));
    Some(cmp)
}

#[test]
fn install_level2_boss_room_against_the_reference_traces() {
    let Some((level, _)) = level2() else { return };
    let Some(root) = std::env::var_os("CLAW_REFERENCE").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_REFERENCE is not set");
        return;
    };
    let mut failures = Vec::new();
    for &(store, clocks) in STORES {
        let dir = root.join("level2").join(store);
        for &clock in clocks {
            println!("== level2/{store} {clock}");
            let Some(cmp) = compare_store(&level, &dir, clock) else {
                println!("skipped: no reference trace");
                continue;
            };
            assert!(cmp.rows_compared > 0);
            let list = known(store, clock);
            for c in &cmp.per_column {
                let Some(first) = c.first_t_ms else { continue };
                println!("first {store} {clock} {} {first}", c.column);
                match list.iter().find(|(name, _)| *name == c.column) {
                    Some(&(_, at)) if first >= at => {}
                    _ => failures.push(format!("{store} {clock}: {} at {first} ms", c.column)),
                }
            }
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}
