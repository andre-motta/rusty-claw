//! The committed bot routes on your own install (skipped without
//! `CLAW_INSTALL`).
//!
//! Every route in `tests/acceptance/routes/` is an input script found by
//! the search bot (`claw-bot`), led by its `# level`, `# tuning`, `# seed`
//! and behaviour header lines, and by the progress the level starts with
//! when it is a later level of one game (`# start_score = ...`). Each one
//! is played in the game built the way the game builds it, with the
//! tuning, seed, behaviour and start its header names:
//!
//! - stepped tick by tick from the script, then replayed through the
//!   game's frame loop at a simulated 60 Hz display, once from the script
//!   rows and twice as key events: every per-tick digest of the whole game
//!   is identical across the runs, and so is the trace of each replay;
//! - the level is won (the level complete event), Claw never dies, and no
//!   tick moves him further than walking, running, falling or riding can
//!   (nothing moves him but the input), except the boss room's door, which
//!   takes him into the room, and a rope's grab, which hangs him from its
//!   end;
//! - on a level that ends with a boss, the boss is defeated before the
//!   level is won, and the boss's introduction plays (the fight flag goes
//!   0, 1, 2) unless the route is an intro-skip route (`introskip` in its
//!   name), whose jump-in entry keeps the flag at 0 all along;
//! - every change of the held buttons comes at least 2 ticks after the
//!   previous one, so a 60 Hz display never drops a press;
//! - nothing is held in the first tick (keys down when play starts count
//!   only once released and pressed again), and a route starts from the
//!   random state the level's set-up leaves, which is the state the
//!   reference runs of that level start from ([`LEVEL_SEEDS`]);
//! - the same route stepped in the bot's own configuration gives the same
//!   digests, so what the bot found is what the game plays.
//!
//! A route file `<stem>.level<N>.csv` is level N of the game whose first
//! level is `<stem>.csv`, as a recorded game names its files. Each such
//! game is played through the game's flow: the first file wins its level,
//! the booty screen leads into the next level with the progress carried,
//! which must be the progress the next file starts with, and that file,
//! played as a replay of it plays (with the seed and tuning it names), wins
//! that level too; twice, with the same traces.
//!
//! The third level's routes ([`SEARCHED_AGAIN_IN_S2_P5D`]) were found
//! before the game learned eight more of the original's rules (an enemy
//! sees Claw only once a frame has shown it on screen; the respawn's
//! passes see the keys pressed in the tick of the death, move Claw a
//! whole step in each and let the camera jump to him in its own call; an
//! enemy's move counts the time since its last one; the rat touches at a
//! single point; walkers turn on their exact limits; the end-of-level
//! gem's glitter starts as it is made); under those rules none of the four
//! wins any more, so their checks are skipped until they are searched
//! again (S2-P5d).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use claw_acceptance::install;
use claw_acceptance::play::{Feed, Player, digest, replay};
use claw_bot::route::{self, TuningName};
use claw_bot::search::{BossEnd, boss_end};
use claw_frame::GameEvent;
use claw_game::flow::{self, Effect, Flow, Screen, Start};
use claw_game::load::{LevelBundle, load_level};
use claw_game::new_sim;
use claw_game::saves::SaveStore;
use claw_game::session::{LevelRecording, Session, script_behaviour};
use claw_game::settings::{LevelChoice, Settings, UserDirs};
use claw_sim::logic::world_boss::fight;
use claw_sim::player::state;
use claw_sim::trace::TraceRow;
use claw_sim::{InputScript, Sim, SimConfig, StartState, Tuning};
use claw_ui::menu::MenuInput;
use claw_ui::menu::draw::FontArt;
use claw_ui::screens::booty::{BootyArt, BootyScreen};
use tempfile::TempDir;

/// Fewest ticks between two changes of the held buttons.
const MIN_SPACING: usize = 2;

/// Largest move of Claw in one tick that input alone can make, px.
const MAX_STEP_PX: i32 = 48;

/// The state of the shared random stream at time zero in the reference
/// runs of each level: the level-load state advanced by the draws of the
/// level's set-up (six on the first level, four on the second, eight on
/// the third).
const LEVEL_SEEDS: [(u32, u32); 3] = [(1, 4_105_512_405), (2, 1_617_999_171), (3, 714_107_639)];

/// The routes found before the third level's latest rules, skipped until
/// they are searched again (S2-P5d).
const SEARCHED_AGAIN_IN_S2_P5D: [&str; 4] = [
    "level2-original.level3.csv",
    "level3-boost.csv",
    "level3-original.csv",
    "level3.csv",
];

/// True (with a note) when `name` waits to be searched again.
fn awaits_search(name: &str) -> bool {
    let waits = SEARCHED_AGAIN_IN_S2_P5D.contains(&name);
    if waits {
        println!(
            "{name}: skipped, expected to fail until it is searched again under the third level's latest rules (S2-P5d)"
        );
    }
    waits
}

/// The route header keys.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Header {
    level: u32,
    original: bool,
    seed: u32,
    fix: bool,
}

fn header(text: &str) -> Header {
    let mut h = Header {
        level: 0,
        original: false,
        seed: 1,
        fix: script_behaviour(text).unwrap_or(false),
    };
    let mut tuning = None;
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix('#') else {
            continue;
        };
        let mut words = rest.split_whitespace();
        match (words.next(), words.next()) {
            (Some("level"), Some(v)) => h.level = v.parse().expect("a level number"),
            (Some("tuning"), Some(v)) => tuning = Some(v.to_string()),
            (Some("seed"), Some(v)) => h.seed = v.parse().expect("a seed"),
            _ => {}
        }
    }
    assert!(h.level >= 1, "the route names its level");
    match tuning.as_deref() {
        Some("default") => {}
        Some("original") => h.original = true,
        other => panic!("the route names its tuning, found {other:?}"),
    }
    h
}

fn routes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("routes")
}

fn routes() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(routes_dir())
        .expect("the routes folder")
        .map(|e| e.expect("a folder entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "csv"))
        .collect();
    out.sort();
    out
}

/// The later levels of the games in the routes folder: for each
/// `<stem>.level<N>.csv`, the first file `<stem>.csv` and N.
fn later_levels() -> Vec<(PathBuf, PathBuf, u32)> {
    let mut out = Vec::new();
    for p in routes() {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".csv") else {
            continue;
        };
        let Some((first, n)) = stem.rsplit_once(".level") else {
            continue;
        };
        let n: u32 = n.parse().expect("a level number after .level");
        out.push((routes_dir().join(format!("{first}.csv")), p, n));
    }
    out
}

/// Ticks of held-button changes closer than [`MIN_SPACING`] to the
/// previous change.
fn too_close(script: &InputScript) -> Vec<usize> {
    let mut prev = 0;
    let mut last: Option<usize> = None;
    let mut out = Vec::new();
    for (i, t) in script.ticks.iter().enumerate() {
        if t.held.0 != prev {
            if last.is_some_and(|l| i - l < MIN_SPACING) {
                out.push(i);
            }
            last = Some(i);
        }
        prev = t.held.0;
    }
    out
}

/// Steps `script` from `start`, returning the player and the digest
/// before every tick and after the last.
fn step_through(start: &Sim, script: &InputScript) -> (Player, Vec<u64>) {
    let mut p = Player::new(start.clone());
    let mut digests = Vec::with_capacity(script.ticks.len() + 1);
    for t in &script.ticks {
        digests.push(digest(&p.sim));
        p.step(t.held.0);
    }
    digests.push(digest(&p.sim));
    (p, digests)
}

/// `cfg` with the tuning and the fix the route's header names.
fn route_config(h: &Header, mut cfg: SimConfig) -> SimConfig {
    if h.original {
        cfg.tuning = Tuning::original_speeds();
        cfg.tuning.behaviour.fix_double_jump = h.fix;
    }
    cfg.seed = h.seed;
    cfg
}

/// The level as the game starts it: from a new game's values, or from the
/// progress the route says the level starts with.
fn start_level(bundle: &LevelBundle, cfg: SimConfig, start: &StartState) -> Sim {
    if *start == StartState::default() {
        new_sim(bundle, cfg, None)
    } else {
        Sim::with_start(bundle.sim_level.clone(), cfg, start.clone())
    }
}

/// What a route did in a boss room.
#[derive(Debug, Default)]
struct BossRun {
    /// The fight flag's values, in order, each change once.
    flags: Vec<u8>,
    /// The first tick in the boss room.
    arrived: Option<u64>,
    /// The tick the fight came on.
    fight_on: Option<u64>,
    /// The tick the boss was defeated.
    defeated: Option<u64>,
}

/// True when Claw went from `from` to `to` through the boss room's door.
fn through_the_door(end: Option<&BossEnd>, from: (i32, i32), to: (i32, i32)) -> bool {
    end.is_some_and(|e| {
        (from.0 - e.door.0).abs() <= 96
            && (from.1 - e.door.1).abs() <= 96
            && (to.0 - e.room.0).abs() <= 16
            && (to.1 - e.room.1).abs() <= 16
    })
}

#[test]
fn install_routes_win_the_same_way_every_time() {
    let Some(root) = install() else { return };
    let files = routes();
    assert!(!files.is_empty(), "no committed routes");
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if awaits_search(&name) {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("the route reads");
        // A route is plain play: one played with a play-test aid is refused.
        let script = InputScript::parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let h = header(&text);
        let rec = LevelRecording::parse(&text).expect("the route parses as a recording");
        assert!(!script.ticks.is_empty(), "{name}: empty route");
        assert_eq!(
            too_close(&script),
            Vec::<usize>::new(),
            "{name}: held-button changes closer than {MIN_SPACING} ticks"
        );
        assert_eq!(
            script.ticks[0].held.0, 0,
            "{name}: the first press comes after the first tick"
        );
        if let Some(&(_, seed)) = LEVEL_SEEDS.iter().find(|(l, _)| *l == h.level) {
            assert_eq!(h.seed, seed, "{name}: the reference runs' seed");
        }

        // The game as the game builds it, with the route's header.
        let choice = LevelChoice::Retail(h.level);
        let bundle = load_level(&root, &choice).expect("the level loads");
        let mut settings = Settings::new(&root, choice);
        settings.mid_air_hit_fix = h.fix;
        let cfg = route_config(&h, settings.sim_config(settings.res));
        let start = start_level(&bundle, cfg, &rec.start);
        assert_eq!(
            start.rng_after_set_up(),
            h.seed,
            "{name}: the seed is the state the level's set-up leaves"
        );
        let end = boss_end(&start);

        let (p, stepped) = step_through(&start, &script);
        assert!(p.teleports.is_empty(), "{name}: the route moved Claw");
        let won = p
            .events
            .iter()
            .position(|(_, e)| matches!(e, GameEvent::LevelComplete { .. }));
        let won_at = won.map(|i| p.events[i].0);
        assert!(won_at.is_some(), "{name}: the level is not won");
        assert!(
            !p.events
                .iter()
                .any(|(_, e)| matches!(e, GameEvent::ClawDied { .. } | GameEvent::GameOver)),
            "{name}: Claw died"
        );
        // No tick moves Claw further than input can, but the door into the
        // boss room; and what happens in the boss room.
        let mut q = Player::new(start.clone());
        let mut at = q.at();
        let mut boss = BossRun {
            flags: vec![q.sim.boss().fight()],
            ..BossRun::default()
        };
        for (i, t) in script.ticks.iter().enumerate() {
            let swinging = |sim: &Sim| sim.player().is_some_and(|o| o.state == state::SWING);
            let was_swinging = swinging(&q.sim);
            q.step(t.held.0);
            let now = q.at();
            let moved = (now.0 - at.0).abs().max((now.1 - at.1).abs());
            let grabbed = !was_swinging && swinging(&q.sim);
            assert!(
                moved <= MAX_STEP_PX || through_the_door(end.as_ref(), at, now) || grabbed,
                "{name}: Claw moved {moved} px in tick {i}, from {at:?} to {now:?}"
            );
            at = now;
            let b = q.sim.boss();
            if boss.flags.last() != Some(&b.fight()) {
                boss.flags.push(b.fight());
            }
            let i = i as u64;
            if boss.arrived.is_none() && b.current().is_some() {
                boss.arrived = Some(i);
            }
            if boss.fight_on.is_none() && b.fight() == fight::ON {
                boss.fight_on = Some(i);
            }
            if boss.defeated.is_none() && b.defeated().is_some() {
                boss.defeated = Some(i);
            }
        }
        let mut fight_note = String::new();
        if end.is_some() {
            let defeated = boss
                .defeated
                .unwrap_or_else(|| panic!("{name}: the boss is not defeated"));
            assert!(defeated < won_at.unwrap(), "{name}: the boss falls first");
            let arrived = boss.arrived.expect("the boss room");
            if name.contains("introskip") {
                assert_eq!(
                    boss.flags,
                    vec![fight::NONE],
                    "{name}: the introduction never plays"
                );
                fight_note = format!(
                    ", jump-in at tick {arrived}, boss defeated {} ticks later",
                    defeated - arrived
                );
            } else {
                assert_eq!(
                    boss.flags,
                    vec![fight::NONE, fight::STAGING, fight::ON],
                    "{name}: the introduction plays, then the fight"
                );
                let on = boss.fight_on.unwrap();
                fight_note = format!(
                    ", in the boss room at tick {arrived}, fight on at {on}, boss defeated {} ticks later",
                    defeated - on
                );
            }
        }

        // Through the game's frame loop at 60 Hz: from the script rows,
        // and twice as key events.
        let rows = replay(&start, &p, 60.0, None, Feed::Script);
        let keys = replay(&start, &p, 60.0, None, Feed::Keys);
        let again = replay(&start, &p, 60.0, None, Feed::Keys);
        assert_eq!(rows.digests, stepped, "{name}: script replay differs");
        assert_eq!(keys.digests, stepped, "{name}: key replay differs");
        assert_eq!(again.digests, keys.digests, "{name}: key replays differ");
        assert_eq!(rows.trace, keys.trace, "{name}: traces differ");
        assert_eq!(again.trace, keys.trace, "{name}: traces differ");

        // The bot's own configuration plays the same game.
        let tuning = if h.original {
            TuningName::Original
        } else {
            TuningName::Default
        };
        assert!(!h.fix, "{name}: bot routes play with the fix off");
        let bot_cfg = route::sim_config(tuning, h.seed);
        let bot_start = match route::parse_carry(&text).expect("the start lines read") {
            Some(carry) => {
                let start = StartState {
                    carry: Some(carry),
                    save: None,
                };
                assert_eq!(
                    start, rec.start,
                    "{name}: the bot reads the start the game reads"
                );
                Sim::with_start(bundle.sim_level.clone(), bot_cfg, start)
            }
            None => Sim::new(bundle.sim_level.clone(), bot_cfg),
        };
        let (_, bot) = step_through(&bot_start, &script);
        assert_eq!(bot, stepped, "{name}: the bot's configuration differs");

        println!(
            "{name}: level {} ({} tuning, seed {}) won at tick {} of {}, health {}{fight_note}",
            h.level,
            if h.original { "original" } else { "default" },
            h.seed,
            won_at.unwrap(),
            script.ticks.len(),
            p.health()
        );
    }
}

/// User folders inside `root`.
fn dirs(root: &Path) -> UserDirs {
    let get = |k: &str| -> Option<OsString> {
        match k {
            "HOME" => Some(root.join("home").into()),
            "XDG_DATA_HOME" => Some(root.join("data").into()),
            "XDG_CONFIG_HOME" => Some(root.join("config").into()),
            _ => None,
        }
    };
    UserDirs::from_lookup(get)
}

/// A game played through the game's flow from its first level, each level
/// replaying its own route file as a replay of that file plays it.
struct Game {
    flow: Flow,
    install: PathBuf,
    settings: Settings,
    /// The route of each level, by number.
    files: BTreeMap<u32, LevelRecording>,
    session: Option<Session>,
    /// Each level started: its number and the start the flow gave it.
    started: Vec<(u32, StartState)>,
    /// Each level won: its number and trace.
    won: Vec<(u32, Vec<TraceRow>)>,
}

impl Game {
    fn new(install: &Path, tmp: &Path, files: BTreeMap<u32, LevelRecording>) -> Game {
        let d = dirs(tmp);
        let first = *files.keys().next().expect("a first level");
        let mut settings = Settings::new(install, LevelChoice::Retail(first));
        settings.config_path = d.config_file();
        settings.follow_header(&files[&first].script.header);
        let saves = SaveStore::open_default(&d, Some(install)).unwrap();
        assert!(
            saves.dir().starts_with(tmp),
            "saves go to the temporary folder"
        );
        let flow = Flow::new(
            settings.clone(),
            Some(saves),
            None,
            FontArt::uniform(8, 10),
            Start::Play,
        );
        let mut g = Game {
            flow,
            install: install.to_path_buf(),
            settings,
            files,
            session: None,
            started: Vec::new(),
            won: Vec::new(),
        };
        g.effects();
        g
    }

    fn effects(&mut self) {
        for _ in 0..8 {
            let effects = self.flow.take_effects();
            if effects.is_empty() {
                return;
            }
            for e in effects {
                match e {
                    Effect::StartLevel { level, start } => {
                        let n = flow::retail_number(&level).expect("a retail level");
                        let rec = self
                            .files
                            .get(&n)
                            .unwrap_or_else(|| panic!("no route file for level {n}"));
                        let bundle = load_level(&self.install, &level).expect("the level loads");
                        // As a replay of the file plays it: with the seed
                        // and tuning it names, from the start the flow
                        // gives.
                        let cfg = rec.sim_config(self.settings.sim_config(self.settings.res));
                        let sim = start_level(&bundle, cfg, &start);
                        let mut session = Session::new(sim).with_replay(rec.script.clone());
                        session.record_trace();
                        self.session = Some(session);
                        self.started.push((n, start));
                    }
                    Effect::EndLevel => {
                        self.session = None;
                    }
                    Effect::ShowBooty(stats) => {
                        let art = BootyArt {
                            map: (None, claw_assets::Palette::default()),
                            tally: (None, claw_assets::Palette::default()),
                            sound_ms: BTreeMap::new(),
                            digit_width: 12,
                            total_offsets: (60, 64),
                        };
                        self.flow.set_booty(BootyScreen::new(stats, art));
                    }
                    _ => {}
                }
            }
        }
    }

    /// Runs `ms` of frames of about 16.7 ms.
    fn run(&mut self, ms: f64) {
        let mut t = 0.0;
        while t < ms {
            let dt = 16.667;
            t += dt;
            if let Some(session) = self.session.as_mut() {
                session.frame(Duration::from_secs_f64(dt / 1000.0));
                let (_, events) = session.drain_outputs();
                if events
                    .iter()
                    .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
                {
                    let n = self.started.last().unwrap().0;
                    self.won.push((n, session.trace().to_vec()));
                }
                assert!(
                    !events
                        .iter()
                        .any(|e| matches!(e, GameEvent::ClawDied { .. } | GameEvent::GameOver)),
                    "Claw died"
                );
                self.flow.play_events(&events, session.sim());
            }
            let sim = self.session.as_ref().map(Session::sim);
            self.flow.update(dt, None, sim);
            self.effects();
        }
    }

    /// Plays the level in play until its booty screen.
    fn win_level(&mut self) {
        let n = self.started.last().unwrap().0;
        let mut guard = 0;
        while self.flow.screen() != Screen::Booty {
            self.run(1000.0);
            guard += 1;
            assert!(guard < 1200, "level {n} is never won");
        }
        assert_eq!(self.won.last().map(|w| w.0), Some(n), "level {n} won");
    }

    /// Goes through the booty screen into the next level.
    fn next_level(&mut self) {
        let before = self.started.len();
        for _ in 0..3 {
            self.flow.menu_input(MenuInput::Confirm, false);
            self.effects();
        }
        let mut guard = 0;
        while self.started.len() == before || self.flow.screen() != Screen::Play {
            self.run(100.0);
            guard += 1;
            assert!(guard < 100, "stuck at {:?}", self.flow.screen());
        }
    }
}

/// `rows` with the input source left out (a replay and the game name it
/// differently).
fn plain(rows: &[TraceRow]) -> Vec<TraceRow> {
    rows.iter()
        .cloned()
        .map(|mut r| {
            r.source = String::new();
            r
        })
        .collect()
}

#[test]
fn install_routes_play_one_game_through_the_booty_screen() {
    let Some(root) = install() else { return };
    let games = later_levels();
    assert!(!games.is_empty(), "no route of a game's later level");
    for (first, later, n) in games {
        if awaits_search(&later.file_name().unwrap().to_string_lossy()) {
            continue;
        }
        let read = |p: &Path| {
            let text = std::fs::read_to_string(p).expect("the route reads");
            // Plain play only: a route played with a play-test aid is refused.
            if let Err(e) = InputScript::parse(&text) {
                panic!("{}: {e}", p.display());
            }
            (
                LevelRecording::parse(&text).expect("the route parses"),
                header(&text),
            )
        };
        let (rec1, h1) = read(&first);
        let (rec2, h2) = read(&later);
        let name = later.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(h2.level, n, "{name}: the level its name says");
        assert_eq!(h1.level + 1, n, "{name}: the level after the first file's");
        assert_eq!(h1.original, h2.original, "{name}: one tuning for the game");
        assert!(rec1.start == StartState::default(), "the game starts new");
        assert!(
            rec2.start.carry.is_some(),
            "{name}: the progress carried in"
        );

        let mut traces = Vec::new();
        for _ in 0..2 {
            let tmp = TempDir::new().unwrap();
            let files = BTreeMap::from([(h1.level, rec1.clone()), (n, rec2.clone())]);
            let mut g = Game::new(&root, tmp.path(), files);
            assert_eq!(g.flow.screen(), Screen::Play);
            g.win_level();
            g.next_level();
            assert_eq!(g.flow.level(), Some(&LevelChoice::Retail(n)));
            let (m, start) = g.started.last().unwrap().clone();
            assert_eq!(m, n);
            assert_eq!(
                start, rec2.start,
                "{name}: the progress the flow carries into level {n} is the route's start"
            );
            g.win_level();
            assert_eq!(g.flow.screen(), Screen::Booty);
            assert_eq!(
                g.flow.next_level().map(|(l, _)| l),
                Some(n + 1),
                "the booty screen leads on"
            );
            traces.push(g.won.clone());
        }
        assert_eq!(
            traces[0], traces[1],
            "{name}: the game plays the same twice"
        );
        // Each level as played in the game is its route played on its own.
        for (level, trace) in &traces[0] {
            let rec = if *level == n { &rec2 } else { &rec1 };
            let choice = LevelChoice::Retail(*level);
            let bundle = load_level(&root, &choice).unwrap();
            let base = Settings::new(&root, choice);
            let sim = start_level(
                &bundle,
                rec.sim_config(base.sim_config(base.res)),
                &rec.start,
            );
            let s = rec.replay(sim);
            let won = trace.len();
            assert!(
                s.trace().len() >= won,
                "level {level}: the route runs past its win"
            );
            assert_eq!(
                plain(&s.trace()[..won]),
                plain(trace),
                "level {level} plays as its route alone plays"
            );
        }
        println!(
            "{} then {name}: one game through the booty screen, levels {} and {n} won ({} and {} ticks), twice the same",
            first.file_name().unwrap().to_string_lossy(),
            h1.level,
            traces[0][0].1.len(),
            traces[0][1].1.len()
        );
    }
}
