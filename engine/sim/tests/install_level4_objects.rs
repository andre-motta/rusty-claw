//! The fourth level's springy ferns and stepping stones on your own
//! install, against the original's runs of the same short routes.
//!
//! Each route starts Claw at a fixed point of the level (beside the first
//! fern, or on the floor right of the first row of clock stones), lets him
//! settle, waits until the level clock reaches the time the original's run
//! started at, and starts the shared random stream from the run's state.
//! It is then played tick by tick and compared with the reference trace
//! of the same route at the 18 ms step, row by row, exactly: Claw's
//! position and state, and every other column of the trace too (health,
//! score, the camera, the random stream, the events), except that the
//! original's camera is still catching up with Claw on the first five rows
//! of a fern route. Played this way, from the original's level clock, the
//! level's timed sounds draw from the random stream and the floor spikes
//! by the fern strike on the original's ticks:
//!
//! - standing on the fern: thrown 21 ticks after landing, to 450 px, over
//!   and over;
//! - Jump held from the jump onto the fern through every throw: the same;
//! - walking left and right on the fern: thrown from the walk, the same;
//! - walking off the fern before the throw: no throw.
//!
//! On the row of clock stones, Claw stands still while the stones beside
//! him sink and rise; their clock edges fall on the ticks the original's
//! did (a quarter period apart for the three variants), and none of them
//! makes a sound.
//!
//! The routes are written below; when a store holds its own copy of a
//! route, the two must agree. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE`
//! for the comparisons; each test skips itself when what it needs is unset.
//! Nothing from an install or a reference store is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::claw_frame::AudioCue;
use claw_sim::input::ScriptTuning;
use claw_sim::logic::stepping_stone::{self, RISE_SOUND, SINK_SOUND, state as stone_state};
use claw_sim::logic::{platform, springboard};
use claw_sim::object::{ObjectId, Rect};
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace;
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// One stretch of a route: from `from` to `to` (inclusive) `held` is down,
/// pressed on the stretch's first row when `press` is set.
#[derive(Debug, Clone, Copy)]
struct Span {
    from: usize,
    to: usize,
    held: u32,
    press: bool,
}

const fn span(from: usize, to: usize, held: u32, press: bool) -> Span {
    Span {
        from,
        to,
        held,
        press,
    }
}

/// A route of the stores: its name, where Claw starts, how many rows it
/// has and its stretches (rows not covered hold nothing).
struct Route {
    store: &'static str,
    start: (i32, i32),
    rows: usize,
    spans: &'static [Span],
}

/// Under the first fern: Jump held for ten rows takes Claw up through it,
/// and he lands on it.
const ONTO_FERN: [Span; 2] = [span(15, 15, JUMP, true), span(16, 24, JUMP, false)];

const FERN: (i32, i32) = (1891, 4767);
const STONES: (i32, i32) = (15900, 1500);

/// Walking on the fern from row 45: Right and Left in turns of 4 rows for
/// 40 rows, then Right held in the air for 60 rows.
const WALK_ON_FERN: [Span; 12] = [
    ONTO_FERN[0],
    ONTO_FERN[1],
    span(45, 48, RIGHT, true),
    span(49, 52, LEFT, true),
    span(53, 56, RIGHT, true),
    span(57, 60, LEFT, true),
    span(61, 64, RIGHT, true),
    span(65, 68, LEFT, true),
    span(69, 72, RIGHT, true),
    span(73, 76, LEFT, true),
    span(77, 80, RIGHT, true),
    span(81, 84, LEFT, true),
];

const ROUTES: [Route; 5] = [
    Route {
        store: "spring-stand",
        start: FERN,
        rows: 900,
        spans: &ONTO_FERN,
    },
    Route {
        store: "spring-hold-jump",
        start: FERN,
        rows: 900,
        spans: &[span(15, 15, JUMP, true), span(16, 899, JUMP, false)],
    },
    Route {
        store: "spring-walk",
        start: FERN,
        rows: 500,
        spans: &WALK_ON_FERN,
    },
    Route {
        store: "spring-walk-off",
        start: FERN,
        rows: 400,
        spans: &[
            ONTO_FERN[0],
            ONTO_FERN[1],
            span(45, 45, LEFT, true),
            span(46, 84, LEFT, false),
        ],
    },
    Route {
        store: "stones-row",
        start: STONES,
        rows: 700,
        spans: &[],
    },
];

/// The last stretch of the walk: Right held in the air from row 85.
const WALK_TAIL: Span = span(85, 144, RIGHT, true);

/// The game time at the routes' time zero in the original, ms.
const T0_MS: u64 = 828;
/// The shared random stream's state at the routes' time zero.
const SEED: u32 = 111_567;

fn route_ticks(r: &Route) -> Vec<TickInput> {
    let mut held = vec![0u32; r.rows];
    let mut pressed = vec![0u32; r.rows];
    let tail = if r.store == "spring-walk" {
        Some(WALK_TAIL)
    } else {
        None
    };
    for s in r.spans.iter().chain(tail.iter()) {
        for h in &mut held[s.from..=s.to] {
            *h |= s.held;
        }
        if s.press {
            pressed[s.from] |= s.held;
        }
    }
    held.iter()
        .zip(&pressed)
        .map(|(&h, &p)| TickInput {
            held: Buttons(h),
            pressed: Buttons(p),
        })
        .collect()
}

/// The rows of a store's own route file: (held, pressed) by row.
fn store_route(dir: &Path) -> Option<Vec<(u32, u32)>> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).unwrap();
    Some(
        text.lines()
            .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
            .map(|l| {
                let f: Vec<&str> = l.split(',').collect();
                (hex(f[1]), hex(f[2]))
            })
            .collect(),
    )
}

fn level4() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(4)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

fn reference_root() -> Option<PathBuf> {
    let root = std::env::var_os("CLAW_REFERENCE").map(PathBuf::from);
    if root.is_none() {
        eprintln!("skipped: CLAW_REFERENCE is not set");
    }
    root
}

/// The game at a route's time zero: Claw placed at `start` and left to
/// settle with the original's speeds, then idle until the level clock is
/// the original's at time zero, and the random stream started from
/// [`SEED`].
fn at_time_zero(level: &SimLevel, start: (i32, i32)) -> Sim {
    let cfg = SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        seed: SEED,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level.clone(), cfg);
    sim.teleport_player(start.0, start.1);
    let mut still = 0;
    let mut last = None;
    for _ in 0..40 {
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
    assert!(still >= 10, "Claw settles at {start:?}");
    assert!(sim.tick() * TICK <= T0_MS, "settled before time zero");
    while sim.tick() * TICK < T0_MS {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(SEED));
    sim
}

/// Claw's position and state on one row.
fn claw(sim: &Sim) -> (i32, i32, i32) {
    let o = sim.player().expect("Claw");
    (o.x, o.y, o.state)
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let Some(rows) = store_route(&root.join("level4").join(r.store)) else {
            println!("{}: no route in the store", r.store);
            continue;
        };
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours, rows, "{}", r.store);
    }
}

#[test]
fn install_level4_fern_and_stone_routes_follow_the_reference_traces() {
    let Some(level) = level4() else { return };
    let Some(root) = reference_root() else { return };
    let mut failures = Vec::new();
    for r in &ROUTES {
        let dir = root.join("level4").join(r.store);
        let Ok(text) = std::fs::read_to_string(dir.join("trace-c18-r1.csv")) else {
            println!("{}: no reference trace", r.store);
            continue;
        };
        let reference = trace::parse(&text).unwrap();
        assert_eq!(
            (reference[0].x, reference[0].y),
            {
                let sim = at_time_zero(&level, r.start);
                let (x, y, _) = claw(&sim);
                (x, y)
            },
            "{}: Claw settles where the original's did",
            r.store
        );
        let mut sim = at_time_zero(&level, r.start);
        let ticks = route_ticks(r);
        let mut first = None;
        let mut rows = Vec::with_capacity(reference.len());
        for (k, want) in reference.iter().enumerate() {
            sim.step(ticks.get(k).copied().unwrap_or_default());
            rows.push(trace::TraceRow::capture(&sim, k as u64, "script"));
            let got = claw(&sim);
            if got != (want.x, want.y, want.state_id) && first.is_none() {
                first = Some((k, got, (want.x, want.y, want.state_id)));
            }
        }
        println!(
            "{}: {} rows, first divergence {first:?}",
            r.store,
            reference.len()
        );
        if let Some(f) = first {
            failures.push(format!(
                "{}: row {} game {:?} original {:?}",
                r.store, f.0, f.1, f.2
            ));
        }
        failures.extend(other_columns(r.store, &rows, &reference));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Rows at the start of a fern route on which the original's camera is
/// still catching up with Claw after he was put down: it reaches him on
/// the sixth row, where the game's already stands.
const CAMERA_CATCH_UP_ROWS: usize = 5;

/// Every other column of the trace, exactly, on the same clock: the
/// camera, health, score, the random stream and the events. Only the
/// camera's x may differ, and only on the first
/// [`CAMERA_CATCH_UP_ROWS`] rows of a fern route.
fn other_columns(
    store: &str,
    rows: &[trace::TraceRow],
    reference: &[trace::TraceRow],
) -> Vec<String> {
    let cmp = trace::compare(rows, reference, &trace::Tolerances::exact());
    let mut failures = Vec::new();
    if cmp.rows_compared != reference.len() {
        failures.push(format!(
            "{store}: {} of {} rows compared",
            cmp.rows_compared,
            reference.len()
        ));
    }
    for c in cmp.per_column.iter().filter(|c| c.diverged > 0) {
        let catch_up = store != "stones-row"
            && c.column == "cam_x"
            && c.diverged <= CAMERA_CATCH_UP_ROWS
            && rows
                .iter()
                .zip(reference)
                .skip(CAMERA_CATCH_UP_ROWS)
                .all(|(a, b)| a.cam_x == b.cam_x);
        if !catch_up {
            failures.push(format!(
                "{store}: {} leaves the exact tolerance on {} rows from {:?} ms",
                c.column, c.diverged, c.first_t_ms
            ));
        }
    }
    failures
}

/// The rows on which `id` stores `s`, over `rows` rows of `sim` from time
/// zero with no key held; also whether any stone sound was cued.
fn stone_edges(sim: &mut Sim, ids: &[ObjectId], rows: usize) -> (Vec<Vec<(usize, i32)>>, bool) {
    let mut seen = vec![Vec::new(); ids.len()];
    let mut last: Vec<i32> = ids
        .iter()
        .map(|&id| sim.objects().get(id).map_or(-1, |o| o.state))
        .collect();
    let mut sound = false;
    for k in 0..rows {
        sim.step(TickInput::default());
        sound |= sim.last_audio().iter().any(|c| {
            matches!(c, AudioCue::Play(s) if s.key == SINK_SOUND || s.key == RISE_SOUND
                || s.key.starts_with("LEVEL_STONE"))
        });
        for (i, &id) in ids.iter().enumerate() {
            let s = sim.objects().get(id).map_or(-1, |o| o.state);
            if s != last[i] {
                seen[i].push((k, s));
                last[i] = s;
            }
        }
    }
    (seen, sound)
}

#[test]
fn install_level4_clock_stones_keep_the_originals_beat_and_stay_silent() {
    let Some(level) = level4() else { return };
    let mut sim = at_time_zero(&level, STONES);
    let find = |id: i32| {
        sim.objects()
            .all()
            .find(|(_, o)| o.id == id)
            .map(|(oid, o)| (oid, o.logic.to_string()))
            .unwrap_or_else(|| panic!("record {id}"))
    };
    let stones = [find(2050), find(2051), find(2052)];
    assert_eq!(
        stones.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>(),
        ["SteppingStone3", "SteppingStone4", "SteppingStone"]
    );
    let ids: Vec<ObjectId> = stones.iter().map(|(id, _)| *id).collect();
    let (edges, sound) = stone_edges(&mut sim, &ids, 700);
    assert!(!sound, "the stones are silent");
    let rows_of = |i: usize, s: i32| -> Vec<usize> {
        edges[i]
            .iter()
            .filter(|(_, st)| *st == s)
            .map(|(k, _)| *k)
            .collect()
    };
    for (i, (_, name)) in stones.iter().enumerate() {
        println!(
            "{name}: sink {:?} rise {:?}",
            rows_of(i, stone_state::clock::SINK_START),
            rows_of(i, stone_state::clock::RISE_START)
        );
    }
    // The rises the original's run shows, a quarter period apart.
    assert!(rows_of(0, stone_state::clock::RISE_START).contains(&203));
    assert!(rows_of(1, stone_state::clock::RISE_START).contains(&245));
    assert!(rows_of(2, stone_state::clock::RISE_START).contains(&287));
    // The fourth variant's first sink.
    assert_eq!(
        rows_of(1, stone_state::clock::SINK_START).first(),
        Some(&34)
    );
    // Every cycle after the first: the edges 166 or 167 rows apart, the
    // sink 17 rows long and the rise 18, at both edges.
    for (i, changes) in edges.iter().enumerate() {
        for s in [
            stone_state::clock::SINK_START,
            stone_state::clock::RISE_START,
        ] {
            let rows = rows_of(i, s);
            for pair in rows.windows(2).skip(1) {
                assert!(
                    matches!(pair[1] - pair[0], 166 | 167),
                    "stone {i} state {s}: {rows:?}"
                );
            }
        }
        for &(k, s) in changes {
            let after = |st: i32| changes.iter().find(|(j, x)| *j > k && *x == st);
            if s == stone_state::clock::SINK_START
                && let Some(&(j, _)) = after(stone_state::clock::UNDER)
            {
                assert_eq!(j - k, 17, "stone {i}: under 17 calls after the edge");
            }
            if s == stone_state::clock::RISE_START
                && let Some(&(j, _)) = after(stone_state::clock::UP)
            {
                assert_eq!(j - k, 18, "stone {i}: up 18 calls after the edge");
            }
        }
    }
}

#[test]
fn install_level4_surfaces_of_every_fern_and_stone() {
    let Some(level) = level4() else { return };
    let ferns = level
        .objects
        .iter()
        .filter(|o| o.logic == "SpringBoard")
        .count();
    let stones = level
        .objects
        .iter()
        .filter(|o| stepping_stone::NAMES.contains(&o.logic.as_str()))
        .count();
    assert_eq!((ferns, stones), (47, 36));
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: claw_sim::ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    idle(&mut sim, 2);
    let mut seen = (0, 0);
    let mut heights = std::collections::BTreeMap::new();
    for (_, o) in sim.objects().all() {
        if &*o.logic == "SpringBoard" {
            seen.0 += 1;
            assert_eq!(o.hit_rect, Rect::new(-32, 10, 24, 64));
            assert_eq!(o.type_flags, platform::PLATFORM);
            assert_eq!(o.state, springboard::state::IDLE);
            let d = springboard::data_of(o).expect("springboard data");
            *heights.entry(d.height).or_insert(0) += 1;
        } else if stepping_stone::NAMES.contains(&&*o.logic) {
            seen.1 += 1;
            assert_eq!(o.hit_rect, Rect::new(-16, -1, 16, 64));
        }
    }
    assert_eq!(seen, (47, 36));
    assert_eq!(
        heights.into_iter().collect::<Vec<_>>(),
        [(300, 2), (350, 4), (450, 40), (650, 1)]
    );
    // The fern's animations come with the level.
    let lv = sim.level();
    for key in ["LEVEL_SPRINGYFERN_IDLE", "LEVEL_SPRINGYFERN_SPRING"] {
        assert!(lv.animations.contains_key(key), "{key}");
    }
    assert!(lv.animations.contains_key(platform::FORWARD_ANIM));
    assert!(lv.animations.contains_key(platform::BACKWARD_ANIM));
    // The stones' sound keys name no sound of the level.
    for key in [SINK_SOUND, RISE_SOUND] {
        assert!(!lv.sounds.contains_key(key), "{key}");
    }
    assert_eq!(state::JUMP, 4);
}

fn cues(sim: &Sim, key: &str) -> usize {
    sim.last_audio()
        .iter()
        .filter(|c| matches!(c, AudioCue::Play(s) if s.key == key))
        .count()
}

#[test]
fn install_level4_standing_on_the_first_fern() {
    let Some(level) = level4() else { return };
    let mut sim = at_time_zero(&level, FERN);
    let ticks = route_ticks(&ROUTES[0]);
    let fern = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "SpringBoard" && o.x == 1895 && o.y == 4769)
        .map(|(id, _)| id)
        .expect("the first fern");
    let mut rows = Vec::new();
    let mut springs = Vec::new();
    let mut fern_states = Vec::new();
    for (k, t) in ticks.iter().enumerate().take(260) {
        sim.step(*t);
        rows.push(claw(&sim));
        if cues(&sim, "LEVEL_SPRINGYFERN_SPRING") > 0 {
            springs.push(k);
        }
        fern_states.push(sim.objects().get(fern).unwrap().state);
    }
    let landings: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].2 == state::STAND && rows[k - 1].2 != state::STAND)
        .collect();
    // The jump onto the fern on row 15 is Claw's own.
    let throws: Vec<usize> = (16..rows.len())
        .filter(|&k| rows[k].2 == state::JUMP && rows[k - 1].2 == state::STAND)
        .collect();
    // Lands on the fern on row 43 and every 86 rows after, standing at
    // y 4718; the spring sound on L + 18, the throw on L + 21.
    assert_eq!(landings, [43, 129, 215]);
    assert!(landings.iter().all(|&l| rows[l].1 == 4718));
    assert_eq!(springs, [61, 147, 233]);
    assert_eq!(throws, [64, 150, 236]);
    // The first rising step is 25 px; the apex is exactly 450 px above the
    // fern, 28 rows after the throw.
    assert_eq!(rows[64].1, 4718 - 25);
    let apex = (64..129).min_by_key(|&k| (rows[k].1, k)).unwrap();
    assert_eq!((apex, rows[apex].1), (92, 4718 - 450));
    // The fern sees Claw on L + 1 and is idle again on L + 49.
    assert_eq!(fern_states[43], springboard::state::IDLE);
    assert_eq!(fern_states[44], springboard::state::CLAW_SPRING);
    let idle_again = (45..129)
        .find(|&k| fern_states[k] == springboard::state::IDLE)
        .unwrap();
    assert_eq!(idle_again, 43 + 49);
}

#[test]
fn install_level4_the_ferns_live_at_the_start_rustle_together() {
    let Some(level) = level4() else { return };
    let cfg = SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level, cfg);
    let mut rustles = Vec::new();
    for k in 0..700 {
        sim.step(TickInput::default());
        let n = cues(&sim, "LEVEL_SPRINGYFERN_RUSTLE");
        if n > 0 {
            rustles.push((k, n));
        }
    }
    // Four voices on the same tick, every 254 ticks (4572 ms).
    assert_eq!(rustles, [(112, 4), (366, 4), (620, 4)]);
}
