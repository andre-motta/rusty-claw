//! The search on small levels built in code: a gap, a ladder and a
//! stand-in hazard, a boss room, breaking planks, further games every move
//! is played in, and the same seed giving the same route.

use claw_bot::actions;
use claw_bot::route::{self, spacing_violations};
use claw_bot::search::{BossStage, Intro, boss_end, boss_stage};
use claw_bot::{Goal, SearchConfig, search, search_report, search_report_with};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::break_plank::{self, FALLBACK_MOVE_RECT};
use claw_sim::logic::enemy;
use claw_sim::logic::enemy::crab::CrabValues;
use claw_sim::logic::world_boss::fight;
use claw_sim::tiles::{CLEAR, Descriptor, GROUND};
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, TickInput, TileMap};
use std::sync::Arc;

/// A level from text rows of 64 px tiles (see [`TileMap::from_ascii`]).
fn level(rows: &[String], start: (i32, i32)) -> SimLevel {
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), start)
}

fn sim(level: SimLevel) -> Sim {
    Sim::new(level, SimConfig::default())
}

fn cfg() -> SearchConfig {
    SearchConfig {
        nodes: 4_000,
        threads: 2,
        ..SearchConfig::default()
    }
}

/// Searches, then checks that the route wins when played from the start,
/// with no death and the spacing rule kept.
fn solve(start: &Sim, goal: Goal) -> InputScript {
    let script = search(start, goal.clone(), &cfg()).expect("a route");
    let played = route::play(start, &script, &goal);
    assert!(played.won(), "{played:?}");
    assert!(spacing_violations(&script, 2).is_empty());
    script
}

/// Rows of an open level `w` tiles wide and `h` tall, with a solid floor
/// row at the bottom.
fn room(w: usize, h: usize) -> Vec<String> {
    let mut rows = vec![".".repeat(w); h - 1];
    rows.push("#".repeat(w));
    rows
}

#[test]
fn crosses_a_gap_over_a_pit() {
    // A floor at row 9 with a pit two tiles wide whose bottom is death.
    let mut rows = room(30, 12);
    for r in rows.iter_mut().take(11).skip(9) {
        *r = "#".repeat(30);
    }
    for r in rows.iter_mut().skip(9) {
        r.replace_range(14..16, "..");
    }
    rows[11].replace_range(14..16, "xx");
    let start = sim(level(&rows, (5 * 64, 8 * 64)));
    let script = solve(
        &start,
        Goal::Reach {
            x: 22 * 64,
            y: 9 * 64 - 61,
            radius: 40,
        },
    );
    assert!(script.ticks.len() < 400);
    // The same point on foot: the route goes on until Claw stands there.
    let stand = Goal::Stand {
        x: 22 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let script = solve(&start, stand);
    let mut end = start.clone();
    for t in &script.ticks {
        end.step(*t);
    }
    assert!(claw_bot::search::on_foot(&end), "the route ends on foot");
}

#[test]
fn climbs_a_ladder_to_a_ledge() {
    // A ledge at row 4 over the right half, reached only by the ladder in
    // column 12 (far too high to jump).
    let mut rows = room(30, 12);
    rows[4].replace_range(13..30, &"#".repeat(17));
    for r in rows.iter_mut().take(11).skip(3) {
        r.replace_range(12..13, "H");
    }
    let start = sim(level(&rows, (4 * 64, 9 * 64)));
    solve(
        &start,
        Goal::Reach {
            x: 20 * 64,
            y: 4 * 64 - 61,
            radius: 40,
        },
    );
}

#[test]
fn jumps_a_stand_in_hazard() {
    // Death tiles set into the floor between Claw and the goal.
    let mut rows = room(30, 10);
    rows[9].replace_range(13..15, "xx");
    let start = sim(level(&rows, (5 * 64, 7 * 64)));
    let goal = Goal::Reach {
        x: 22 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let found = search_report(&start, goal, &cfg()).expect("a route");
    assert_eq!(found.stats.deaths, 0);
    // Walking straight into the hazard is a death the search saw and
    // dropped.
    assert!(found.stats.deaths_pruned > 0);
}

#[test]
fn the_same_seed_gives_the_same_route_on_any_thread_count() {
    let mut rows = room(30, 10);
    rows[9].replace_range(13..15, "xx");
    rows[7].replace_range(18..21, "---");
    let start = sim(level(&rows, (5 * 64, 7 * 64)));
    let goal = Goal::Reach {
        x: 19 * 64,
        y: 7 * 64 - 61,
        radius: 40,
    };
    let run = |seed: u64, threads: usize| {
        let c = SearchConfig {
            seed,
            threads,
            ..cfg()
        };
        search_report(&start, goal.clone(), &c).expect("a route")
    };
    let a = run(7, 1);
    let b = run(7, 4);
    assert_eq!(a.script, b.script);
    assert_eq!(a.stats, b.stats);
    assert_eq!(run(7, 1).script, a.script);
}

#[test]
fn an_unreachable_goal_fails_within_the_budget() {
    // The goal is sealed in rock.
    let mut rows = room(20, 10);
    rows[2] = "#".repeat(20);
    rows[3] = "#".repeat(20);
    let start = sim(level(&rows, (3 * 64, 7 * 64)));
    let c = SearchConfig {
        nodes: 300,
        ..cfg()
    };
    let err = search(
        &start,
        Goal::Reach {
            x: 10 * 64,
            y: 64,
            radius: 20,
        },
        &c,
    )
    .unwrap_err();
    assert!(err.stats.expanded <= 300);
}

#[test]
fn a_search_from_part_way_through_a_route_keeps_the_spacing_across_the_join() {
    let rows = room(30, 10);
    let fresh = sim(level(&rows, (5 * 64, 7 * 64)));
    // A prefix that changes its held mask in its very last tick.
    let right = claw_sim::Buttons::RIGHT.0;
    let mut held = vec![0u32; 20];
    held.push(right);
    let prefix = InputScript::from_held(&held);
    let mut start = fresh.clone();
    for &t in &prefix.ticks {
        start.step(t);
    }
    let goal = Goal::Reach {
        x: 12 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let c = SearchConfig {
        start_held: right,
        start_held_for: 1,
        ..cfg()
    };
    let found = search_report(&start, goal.clone(), &c).expect("a route");
    // The route found first keeps the prefix's mask for the missing tick.
    assert_eq!(found.script.ticks[0].held.0, right);
    assert_eq!(found.script.ticks[0].pressed.0, 0);
    let mut whole = prefix.clone();
    whole.ticks.extend(found.script.ticks.iter().copied());
    assert!(spacing_violations(&whole, 2).is_empty());
    // Pressed buttons are the rising edges across the join, so the route
    // written is the route the search played.
    let masks: Vec<u32> = whole.ticks.iter().map(|t| t.held.0).collect();
    assert_eq!(whole, InputScript::from_held(&masks));
    assert!(route::play(&fresh, &whole, &goal).won());
}

#[test]
fn a_route_from_the_level_start_presses_nothing_in_its_first_ticks() {
    // Keys down when play starts count only once released and pressed
    // again, so the search idles first.
    let rows = room(30, 10);
    let start = sim(level(&rows, (5 * 64, 7 * 64)));
    let goal = Goal::Reach {
        x: 12 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let script = solve(&start, goal);
    assert!(script.ticks[..2].iter().all(|t| t.held.0 == 0));
    assert_ne!(script.ticks[2].held.0, 0, "then it goes");
}

/// The route ticks at which Claw lands and at which jump is pressed, when
/// `script` is played from `start`.
fn landings_and_jumps(start: &Sim, script: &InputScript) -> (Vec<usize>, Vec<usize>) {
    let mut sim = start.clone();
    let (mut lands, mut jumps) = (Vec::new(), Vec::new());
    for (k, &t) in script.ticks.iter().enumerate() {
        let air = actions::phase(&sim) == actions::Phase::Air;
        sim.step(t);
        if air && actions::phase(&sim) != actions::Phase::Air {
            lands.push(k);
        }
        if t.pressed.0 & actions::JUMP != 0 {
            jumps.push(k);
        }
    }
    (lands, jumps)
}

#[test]
fn with_a_landing_gap_no_jump_is_pressed_near_a_landing() {
    // Steps up a staircase of ledges: jump after jump.
    let mut rows = room(40, 12);
    for (i, r) in rows.iter_mut().enumerate().take(11).skip(5) {
        let from = 10 + 4 * (10 - i);
        r.replace_range(from..40, &"#".repeat(40 - from));
    }
    let start = sim(level(&rows, (3 * 64, 9 * 64)));
    let goal = Goal::Reach {
        x: 36 * 64,
        y: 5 * 64 - 61,
        radius: 48,
    };
    let c = SearchConfig {
        land_gap: 2,
        ..cfg()
    };
    let found = search_report(&start, goal.clone(), &c).expect("a route");
    assert!(route::play(&start, &found.script, &goal).won());
    let (lands, jumps) = landings_and_jumps(&start, &found.script);
    assert!(lands.len() >= 2 && jumps.len() >= 2, "{lands:?} {jumps:?}");
    for j in &jumps {
        assert!(
            lands.iter().all(|l| l.abs_diff(*j) > 2),
            "jump at {j}, landings {lands:?}"
        );
    }
}

#[test]
fn a_segment_starts_with_claw_settled_and_the_seed_set() {
    let rows = room(30, 10);
    let mut start = sim(level(&rows, (5 * 64, 7 * 64)));
    route::settle_at(&mut start, 15 * 64, 6 * 64, 111_567);
    let o = start.player().unwrap();
    assert_eq!(o.x, 15 * 64);
    assert_eq!(o.y + o.move_rect.bottom, 9 * 64 - 1, "on the floor");
    assert_eq!(o.state, claw_sim::player::state::STAND);
    assert_eq!(start.rng_state(), 111_567);
    // The search goes on from there.
    let goal = Goal::Reach {
        x: 20 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    solve(&start, goal);
}

#[test]
fn with_the_fragility_check_a_route_still_wins_with_any_change_a_tick_off() {
    // A gap over a pit, crossed with room to spare.
    let mut rows = room(30, 12);
    for r in rows.iter_mut().take(11).skip(9) {
        *r = "#".repeat(30);
    }
    for r in rows.iter_mut().skip(9) {
        r.replace_range(14..16, "..");
    }
    rows[11].replace_range(14..16, "xx");
    let start = sim(level(&rows, (5 * 64, 8 * 64)));
    let goal = Goal::Reach {
        x: 22 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let c = SearchConfig {
        fragile_cost: 400,
        ..cfg()
    };
    let found = search_report(&start, goal.clone(), &c).expect("a route");
    assert!(route::play(&start, &found.script, &goal).won());
    // Some moves tried fail with their timing shifted; none is on the
    // route found.
    assert!(found.stats.fragile > 0, "{:?}", found.stats);
    assert_eq!(found.stats.fragile_on_route, 0);
    let m = route::margin(&start, &found.script, &goal, 40);
    assert!(m.sampled > 0);
    assert_eq!((m.early_wins, m.late_wins), (m.sampled, m.sampled), "{m:?}");
}

/// A boss room with no introduction: a flat floor with Raux standing on
/// it (his patrol limits and the gem's landing point in his record), a
/// door far off to the left that leads into the room (it makes the level
/// a boss level; Claw never goes near it), and Claw 400 px left of Raux,
/// every object awake.
fn boss_room() -> Sim {
    let rows = room(80, 16);
    let mut level = level(&rows, (2600, 900));
    let mut raux = ObjectSpec::at("Raux", 3000, 900, 0);
    raux.image_set = "LEVEL_RAUX".into();
    raux.record[enemy::field::X_MIN] = 2700;
    raux.record[enemy::field::X_MAX] = 3300;
    raux.record[enemy::field::SPEED_X] = 2965;
    raux.record[enemy::field::SPEED_X + 1] = 752;
    level.objects.push(raux);
    let mut door = ObjectSpec::at("BossWarp", 300, 900, 1000);
    door.record[55] = 2600;
    door.record[56] = 900;
    level.objects.push(door);
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    Sim::new(
        level,
        SimConfig {
            seed: 3,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    )
}

#[test]
fn a_boss_level_is_read_from_its_door_and_its_boss() {
    let start = boss_room();
    let end = boss_end(&start).expect("a boss level");
    assert_eq!(end.door, (300, 900));
    assert_eq!(end.room, (2600, 900));
    assert_eq!(end.boss, (3000, 900));
    assert_eq!(end.gem, (2965, 752));
    assert!(start.boss().current().is_some(), "Raux is built");
    assert_eq!(boss_stage(&start), BossStage::Fight);
    // A level without a boss is not a boss level.
    let plain = sim(level(&room(30, 10), (5 * 64, 7 * 64)));
    assert_eq!(boss_end(&plain), None);
    assert_eq!(boss_stage(&plain), BossStage::Door);
}

#[test]
fn a_boss_fight_without_an_introduction_is_dropped_when_the_route_must_play_it() {
    // The room has no introduction (no stager): with the introduction
    // asked for, every move in the room leaves the fight flag at 0 past
    // the grace ticks and is dropped, and the search fails.
    let start = boss_room();
    assert_eq!(start.boss().fight(), fight::NONE);
    let c = SearchConfig {
        nodes: 2_000,
        threads: 2,
        intro: Intro::Play,
        ..SearchConfig::default()
    };
    let err = search_report(&start, Goal::LevelComplete, &c).unwrap_err();
    assert!(err.stats.entries_pruned > 0, "{:?}", err.stats);
    assert_eq!(err.reason, "every reachable state was searched");
    // Without the rule nothing is dropped for it.
    let c = SearchConfig {
        nodes: 200,
        intro: Intro::Skip,
        ..c
    };
    let err = search_report(&start, Goal::LevelComplete, &c).unwrap_err();
    assert_eq!(err.stats.entries_pruned, 0);
}

#[test]
fn the_longest_throw_of_each_tuning_charges_in_full() {
    // A throw charges for 3 s at most: 167 ticks at the original speeds,
    // 180 with the default tuning's 16.7 ms a tick, and a few to spare.
    for (tuning, full) in [
        (route::TuningName::Original, 170),
        (route::TuningName::Default, 183),
    ] {
        let rows = room(30, 10);
        let sim = Sim::new(level(&rows, (5 * 64, 7 * 64)), route::sim_config(tuning, 1));
        let holds = actions::throw_holds(&sim);
        assert_eq!(holds[3], full, "{tuning:?}");
        assert_eq!(holds[..3], actions::THROW_HOLDS[..3]);
    }
}

#[test]
fn the_full_jump_of_each_tuning_reaches_the_full_height() {
    // A held jump rises 145 px at both tunings once the hold window is
    // credited in full; the default tuning credits 16.7 ms a tick, so its
    // full jump is held a tick longer.
    for (tuning, full) in [
        (route::TuningName::Original, 14),
        (route::TuningName::Default, 15),
    ] {
        let rows = room(30, 10);
        let mut sim = Sim::new(level(&rows, (5 * 64, 7 * 64)), route::sim_config(tuning, 1));
        for _ in 0..40 {
            sim.step(claw_sim::TickInput::default());
        }
        let holds = actions::jump_holds(&sim);
        assert_eq!(holds[3], full, "{tuning:?}");
        let rise = |hold: usize| {
            let mut s = sim.clone();
            let y0 = s.player().unwrap().y;
            let mut top = y0;
            let jump = actions::Action::Jump {
                dir: 0,
                hold: hold as u8,
                up: false,
            };
            let mut masks = jump.masks();
            masks.extend(std::iter::repeat_n(0, 40));
            let script = InputScript::from_held(&masks);
            for &t in &script.ticks {
                s.step(t);
                top = top.min(s.player().unwrap().y);
            }
            y0 - top
        };
        assert_eq!(rise(full), 145, "{tuning:?}");
        assert!(rise(full - 1) < 145, "{tuning:?}");
    }
}

/// The coarse states of a game in which Claw stands still beside `object`,
/// one after each of `waits` waits of the bot.
fn keys_while_waiting(object: ObjectSpec, waits: usize) -> Vec<u64> {
    let mut l = level(&room(12, 6), (200, 250));
    l.number = 4;
    l.objects = vec![object];
    let mut s = sim(l);
    for _ in 0..30 {
        s.step(Default::default());
    }
    let mut keys = Vec::new();
    for _ in 0..waits {
        for _ in actions::Action::Wait.masks() {
            s.step(Default::default());
        }
        keys.push(claw_bot::search::state_key(&s));
    }
    keys
}

#[test]
fn a_wait_for_a_stepping_stone_is_progress() {
    // A stone that keeps to the clock, up for its first 1.5 s: each wait
    // of Claw beside it brings the stone's sink nearer, so none is the
    // same state as the one before.
    let keys = keys_while_waiting(ObjectSpec::at("SteppingStone2", 400, 300, 0), 10);
    for w in keys.windows(2) {
        assert_ne!(w[0], w[1]);
    }
}

/// A seventh level of 64 px tiles with a rock spring whose surface is at
/// `top`, its middle at `x`: its pictures each last longer than a wait,
/// and the throw comes on the third.
fn rock_level(rows: &[String], start: (i32, i32), x: i32, top: i32) -> SimLevel {
    use claw_sim::logic::springboard;
    let rect = springboard::surface(7);
    let table = springboard::level_table(7).expect("the seventh level has springs");
    let mut l = level(rows, start);
    l.number = 7;
    l.animations.insert(
        table.idle_anim.into(),
        Arc::new(Animation::looping(table.idle_anim, &[(1, 2000)])),
    );
    let mut spring = Animation::once(table.spring_anim, &[(1, 300), (2, 300), (3, 300), (1, 0)]);
    spring.frames[2].event = springboard::LAUNCH_EVENT;
    l.animations
        .insert(table.spring_anim.into(), Arc::new(spring));
    l.images.insert("ROCK".into(), ImageFrames::range(1, 3));
    let mut rock = ObjectSpec::at("SpringBoard", x, top - rect.top, 0);
    rock.image_set = "ROCK".into();
    rock.frame = 1;
    l.objects = vec![rock];
    l
}

#[test]
fn rides_a_springboards_throw_over_water_too_wide_to_jump() {
    // A floor at row 9 broken by water five tiles wide (death at its
    // bottom), with a rock spring standing in the water by the left
    // floor's edge, its top 40 px below the floor's: only the throw
    // carries Claw over, though from above it the gap looks narrow enough
    // to jump. The route rises far higher than a jump.
    let mut rows = room(30, 12);
    for r in rows.iter_mut().take(11).skip(9) {
        *r = "#".repeat(30);
    }
    for r in rows.iter_mut().skip(9) {
        r.replace_range(10..15, ".....");
    }
    rows[11].replace_range(10..15, "xxxxx");
    let start = sim(rock_level(
        &rows,
        (5 * 64, 8 * 64),
        10 * 64 + 30,
        9 * 64 + 40,
    ));
    let script = solve(
        &start,
        Goal::Stand {
            x: 20 * 64,
            y: 9 * 64 - 61,
            radius: 40,
        },
    );
    let mut s = start.clone();
    let mut top = i32::MAX;
    for t in &script.ticks {
        s.step(*t);
        top = top.min(s.player().unwrap().y);
    }
    assert!(9 * 64 - 61 - top > 300, "thrown: the route rose to {top}");
}

#[test]
fn a_wait_on_a_springboard_for_its_throw_is_progress() {
    // Claw lands on a seventh level rock spring (its surface at y 260,
    // 60 px over the floor, under his start) and waits: while the spring
    // he set off runs, each wait brings the throw nearer, even while the
    // rock's picture stays the same, so none is the same state as the one
    // before, up to the throw.
    use claw_sim::logic::springboard;
    let l = rock_level(&room(12, 6), (200, 150), 200, 260);
    let mut s = sim(l);
    s.step(Default::default());
    s.teleport_player(200, 260 - 61 - 10);
    let mut keys = Vec::new();
    for _ in 0..12 {
        for _ in actions::Action::Wait.masks() {
            s.step(Default::default());
        }
        if actions::phase(&s) != actions::Phase::Ground {
            break;
        }
        let spring = s
            .objects()
            .all()
            .find_map(|(_, o)| springboard::data_of(o).map(|_| o.state));
        if spring == Some(springboard::state::CLAW_SPRING) {
            keys.push(claw_bot::search::state_key(&s));
        }
    }
    assert!(keys.len() >= 2, "Claw waits on the running spring");
    for w in keys.windows(2) {
        assert_ne!(w[0], w[1]);
    }
}

#[test]
fn a_wait_over_a_blower_or_beside_a_goo_vent_is_progress_and_far_from_them_is_not() {
    // Claw stands at x 200 on a floor whose top is at y 320. Over a
    // blower's nozzle (the blower 90 px above the floor), or beside a goo
    // vent, each wait is a state of its own: the throw or the end of the
    // eruption comes nearer.
    for object in [
        ObjectSpec::at("GroundBlower", 200, 230, 0),
        ObjectSpec::at("GooVent", 330, 300, 0),
    ] {
        let keys = keys_while_waiting(object.clone(), 10);
        for w in keys.windows(2) {
            assert_ne!(w[0], w[1], "{}", object.logic);
        }
    }
    // The same objects out of reach: every wait is the same state.
    for object in [
        ObjectSpec::at("GroundBlower", 600, 230, 0),
        ObjectSpec::at("GooVent", 700, 300, 0),
    ] {
        let keys = keys_while_waiting(object.clone(), 4);
        for w in keys.windows(2) {
            assert_eq!(w[0], w[1], "{}", object.logic);
        }
    }
}

/// A level numbered 5 (the fifth level's plank geometry) with a floor at
/// y 960 and a row of five breaking planks from x 807 at y 793, 500 ms
/// apart: their one-way floor tiles in row 11, columns 12 to 16, 28 px
/// below the cells' tops. Claw starts above the floor left of the row.
fn plank_level() -> SimLevel {
    let mut rows = vec![".".repeat(30); 15];
    rows.push("#".repeat(30));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut map = TileMap::from_ascii(64, &rows);
    let plank = map.add_descriptor(Descriptor::Double {
        width: 64,
        height: 64,
        outside: CLEAR,
        inside: GROUND,
        rect: [0, 28, 63, 63],
    });
    for c in 12..17 {
        map.set_cell(c, 11, plank);
    }
    let mut level = SimLevel::synthetic(map, (500, 860));
    level.number = 5;
    let mut spec = ObjectSpec::at(break_plank::SPAWNER, 807, 793, 1010);
    spec.record[break_plank::DELAY_FIELD] = 500;
    spec.record[break_plank::WIDTH_FIELD] = 5;
    level.objects.push(spec);
    let mut frames = ImageFrames::range(1, 12);
    frames.boxes.insert(1, FALLBACK_MOVE_RECT);
    level
        .images
        .insert(break_plank::IMAGE_SET.to_string(), frames);
    let idle = Animation::once(break_plank::IDLE_ANIM, &[(1, 0)]);
    let crumble: Vec<(i32, i32)> = (1..=12).map(|i| (i, 75)).collect();
    let crumble = Animation::once(break_plank::CRUMBLE_ANIM, &crumble);
    for a in [idle, crumble] {
        level.animations.insert(a.key.clone(), Arc::new(a));
    }
    level
}

/// Steps `sim` with nothing held for `ticks` ticks.
fn idle(sim: &mut Sim, ticks: usize) {
    for _ in 0..ticks {
        sim.step(TickInput::default());
    }
}

#[test]
fn claw_back_where_he_stood_with_the_planks_broken_is_another_state() {
    let mut s = sim(plank_level());
    idle(&mut s, 60);
    let at = s.player().map(|o| (o.x, o.y)).unwrap();
    let whole = claw_bot::search::state_key(&s);
    // On to the row: the pieces under him wait, crumble and let him fall
    // to the floor below.
    s.teleport_player(807 + 64, 640);
    idle(&mut s, 150);
    assert!(
        s.player().unwrap().y > 800,
        "Claw fell through the broken row"
    );
    // Back where he stood, standing still again.
    s.teleport_player(at.0, at.1 - 40);
    idle(&mut s, 60);
    assert_eq!(s.player().map(|o| (o.x, o.y)), Some(at));
    assert_ne!(
        claw_bot::search::state_key(&s),
        whole,
        "the broken row tells the states apart"
    );
}

#[test]
fn a_move_that_dies_in_a_further_game_is_dropped() {
    // The route's game has a plain floor; a further game has death tiles
    // set into it between Claw and the goal, as a game whose crabs start
    // otherwise can hold a danger the route's own does not.
    let rows = room(30, 10);
    let mut hazard = rows.clone();
    hazard[9].replace_range(13..15, "xx");
    let start = sim(level(&rows, (5 * 64, 7 * 64)));
    let other = sim(level(&hazard, (5 * 64, 7 * 64)));
    let goal = Goal::Reach {
        x: 22 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let plain = search_report(&start, goal.clone(), &cfg()).expect("a route");
    assert!(
        !route::play(&other, &plain.script, &goal).won(),
        "the plain route walks into the hazard of the other game"
    );
    let variants = [(other.clone(), CrabValues::default())];
    let found = search_report_with(&start, &variants, goal.clone(), &cfg()).expect("a route");
    assert!(found.stats.variants_pruned > 0);
    assert!(route::play(&start, &found.script, &goal).won());
    assert!(route::play(&other, &found.script, &goal).won());
    // With no further game, the search is the plain one.
    let again = search_report_with(&start, &[], goal, &cfg()).expect("a route");
    assert_eq!(again.script.ticks, plain.script.ticks);
}
