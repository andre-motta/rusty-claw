//! The second level on your own install (skipped without `CLAW_INSTALL`):
//! what the slice that builds it must hold before it is play-tested.
//!
//! - The census of objects left inert: none of the level's enemies, cannons
//!   or boss objects (PunkRat, TowerCannonLeft, TowerCannonRight, Raux,
//!   BossStager, BossWarp) is a placeholder, and every logic that still is
//!   one is listed below with the reason it may stay so.
//! - The bot's original-speeds route through the level, played tick by
//!   tick: the door takes Claw into Raux's room between two ticks, the
//!   stager fires on his first tick there, the fight flag goes 0, 1, 2 on
//!   the original's steps (1 on the next tick with Claw frozen and the
//!   boss meter shown, 2 after 654 more, Claw and Raux free on the tick
//!   after), the meter falls with every blow and never rises, Raux's
//!   defeat comes before the level ends, and the level ends.
//! - The same route through the game's flow: the end-of-level screen is
//!   the second level's, with the amulet's gem in its socket, and it leads
//!   into the third level with the progress carried.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use claw_acceptance::install;
use claw_assets::Install;
use claw_frame::GameEvent;
use claw_game::flow::{self, Effect, Flow, Screen, Start};
use claw_game::load::load_level;
use claw_game::saves::SaveStore;
use claw_game::session::{LevelRecording, Session};
use claw_game::settings::{LevelChoice, Settings, UserDirs};
use claw_render::ScreenScene;
use claw_sim::logic::world_boss::fight;
use claw_sim::player::state;
use claw_sim::{InputScript, Sim, SimConfig, StartState, TickInput};
use claw_ui::menu::MenuInput;
use claw_ui::menu::draw::FontArt;
use claw_ui::screens::assets::ScreenAssets;
use claw_ui::screens::booty::{self, BootyArt, BootyScreen, BootyStats};
use tempfile::TempDir;

/// The level's objects that must run their own logic.
const BUILT: [&str; 6] = [
    "PunkRat",
    "TowerCannonLeft",
    "TowerCannonRight",
    "Raux",
    "BossStager",
    "BossWarp",
];

/// Logics the second level still leaves inert, each with the reason it may
/// stay so: none. A logic that becomes inert, or one that runs again, is
/// reported so this list is kept exact.
const INERT: &[(&str, &str)] = &[];

/// Steps from the fight flag's 1 to its 2: the introduction's length in the
/// original at one 18 ms step per frame.
const INTRODUCTION_STEPS: u64 = 654;

fn routes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("routes")
}

#[test]
fn install_level2_leaves_no_enemy_cannon_or_boss_object_inert() {
    let Some(root) = install() else { return };
    let bundle = load_level(&root, &LevelChoice::Retail(2)).expect("level 2 loads");
    let mut sim = Sim::new(bundle.sim_level.clone(), SimConfig::default());
    sim.step(TickInput::default());
    let d = sim.diagnostics();
    println!("LEVEL2 inert census: {:?}", d.inert_logics);
    assert!(
        d.unknown_logics.is_empty(),
        "unknown logics: {:?}",
        d.unknown_logics
    );
    for name in BUILT {
        assert!(
            bundle.sim_level.objects.iter().any(|o| *o.logic == *name),
            "the level places {name}"
        );
        assert!(
            !d.inert_logics.contains_key(name),
            "{name} is left inert ({} objects)",
            d.inert_logics[name]
        );
    }
    let listed: Vec<&str> = INERT.iter().map(|(n, _)| *n).collect();
    let found: Vec<&str> = d.inert_logics.keys().map(String::as_str).collect();
    assert_eq!(
        found, listed,
        "the logics the level leaves inert are the listed ones"
    );
}

/// What the route shows of the boss fight, tick by tick.
#[derive(Debug, Default)]
struct Staging {
    /// The first tick in the room.
    arrived: Option<u64>,
    /// The tick the stager fired (its checkpoint).
    trigger: Option<u64>,
    /// The ticks the flag changed, with the new value.
    flags: Vec<(u64, u8)>,
    /// The first tick Claw was free again after the introduction.
    thawed: Option<u64>,
    /// The meter's values, each change once.
    meter: Vec<i32>,
    /// The tick of Raux's defeat.
    defeated: Option<u64>,
    /// The tick the level ended.
    won: Option<u64>,
}

#[test]
fn install_level2_route_stages_raux_and_ends_the_level() {
    let Some(root) = install() else { return };
    let path = routes_dir().join("level2-original.csv");
    let text = std::fs::read_to_string(&path).expect("the route reads");
    let rec = LevelRecording::parse(&text).expect("the route parses");
    let script = InputScript::parse(&text).expect("the route parses");
    let bundle = load_level(&root, &LevelChoice::Retail(2)).expect("level 2 loads");
    let settings = Settings::new(&root, LevelChoice::Retail(2));
    let cfg = rec.sim_config(settings.sim_config(settings.res));
    let mut sim = Sim::new(bundle.sim_level.clone(), cfg);
    let mut s = Staging::default();
    let mut flag = sim.boss().fight();
    assert_eq!(flag, fight::NONE);
    for (i, t) in script.ticks.iter().enumerate() {
        let i = i as u64;
        sim.step(*t);
        let b = sim.boss();
        if s.arrived.is_none() && b.current().is_some() {
            s.arrived = Some(i);
        }
        if s.trigger.is_none()
            && sim
                .last_events()
                .iter()
                .any(|e| matches!(e, GameEvent::Checkpoint { .. }))
            && s.arrived.is_some()
        {
            s.trigger = Some(i);
        }
        if b.fight() != flag {
            flag = b.fight();
            s.flags.push((i, flag));
        }
        let frozen = sim.player().unwrap().state == state::FREEZE;
        if flag == fight::ON && s.thawed.is_none() && !frozen {
            s.thawed = Some(i);
        }
        if let Some(m) = sim.hud().boss_meter
            && s.meter.last() != Some(&m.value)
        {
            s.meter.push(m.value);
        }
        if s.defeated.is_none() && b.defeated().is_some() {
            s.defeated = Some(i);
        }
        if sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
        {
            s.won = Some(i);
            break;
        }
    }
    println!("{s:?}");
    let arrived = s.arrived.expect("Claw reaches Raux's room");
    // The stager fires on Claw's first tick in the room.
    assert_eq!(
        s.trigger,
        Some(arrived),
        "the trigger on the first room tick"
    );
    // The flag goes 0, 1, 2 on the original's steps: 1 on the next tick,
    // with Claw frozen and the meter up; 2 after the introduction; both
    // free on the tick after.
    let flags: Vec<u8> = s.flags.iter().map(|(_, f)| *f).collect();
    assert_eq!(flags, [fight::STAGING, fight::ON], "the flag goes 0, 1, 2");
    let (staging, on) = (s.flags[0].0, s.flags[1].0);
    assert_eq!(staging, arrived + 1, "flag 1 the tick after the trigger");
    assert_eq!(
        on - staging,
        INTRODUCTION_STEPS,
        "the introduction's length"
    );
    assert_eq!(s.thawed, Some(on + 1), "Claw moves again the tick after");
    // The meter shows Raux's full health, then falls with every blow and
    // never rises, to nothing at his defeat.
    assert_eq!(s.meter.first(), Some(&100), "the meter starts full");
    assert!(
        s.meter.windows(2).all(|w| w[1] < w[0]),
        "the meter only falls: {:?}",
        s.meter
    );
    assert!(s.meter.len() >= 3, "several blows: {:?}", s.meter);
    let defeated = s.defeated.expect("Raux is defeated");
    let won = s.won.expect("the level ends");
    assert!(
        defeated > on && won > defeated,
        "Raux falls, then the level ends"
    );
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

/// The second level played through the game's flow from its route, and
/// what the flow showed and started.
struct Game {
    flow: Flow,
    root: PathBuf,
    settings: Settings,
    rec: LevelRecording,
    assets: ScreenAssets,
    session: Option<Session>,
    /// Each level started: its number and the start the flow gave it.
    started: Vec<(u32, StartState)>,
    /// The end-of-level screen's figures and its first scene.
    booty: Option<(BootyStats, ScreenScene)>,
}

impl Game {
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
                        let bundle = load_level(&self.root, &level).expect("the level loads");
                        let cfg = self
                            .rec
                            .sim_config(self.settings.sim_config(self.settings.res));
                        let sim = Sim::with_start(bundle.sim_level.clone(), cfg, start.clone());
                        let session = Session::new(sim);
                        self.session = Some(if n == 2 {
                            session.with_replay(self.rec.script.clone())
                        } else {
                            session
                        });
                        self.started.push((n, start));
                    }
                    Effect::EndLevel => self.session = None,
                    Effect::ShowBooty(stats) => {
                        let art = BootyArt {
                            map: (None, claw_assets::Palette::default()),
                            tally: (None, claw_assets::Palette::default()),
                            sound_ms: self.assets.sound_ms.clone(),
                            digit_width: 12,
                            total_offsets: (60, 64),
                        };
                        let frames = booty::gem_frames_from(&self.assets, stats.level);
                        let screen = BootyScreen::new(stats.clone(), art).with_gem_frames(frames);
                        self.booty = Some((stats, screen.scene()));
                        self.flow.set_booty(screen);
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
            if let Some(s) = self.session.as_mut() {
                s.frame(Duration::from_secs_f64(dt / 1000.0));
                let (_, events) = s.drain_outputs();
                assert!(
                    !events
                        .iter()
                        .any(|e| matches!(e, GameEvent::ClawDied { .. } | GameEvent::GameOver)),
                    "Claw died"
                );
                self.flow.play_events(&events, s.sim());
            }
            let sim = self.session.as_ref().map(Session::sim);
            self.flow.update(dt, None, sim);
            self.effects();
        }
    }
}

#[test]
fn install_level2_ends_on_the_gem_screen_and_leads_into_level3() {
    let Some(root) = install() else { return };
    let path = routes_dir().join("level2-original.csv");
    let text = std::fs::read_to_string(&path).expect("the route reads");
    let rec = LevelRecording::parse(&text).expect("the route parses");
    let tmp = TempDir::new().unwrap();
    let d = dirs(tmp.path());
    let mut settings = Settings::new(&root, LevelChoice::Retail(2));
    settings.config_path = d.config_file();
    settings.follow_header(&rec.script.header);
    let saves = SaveStore::open_default(&d, Some(&root)).unwrap();
    let flow = Flow::new(
        settings.clone(),
        Some(saves),
        None,
        FontArt::uniform(8, 10),
        Start::Play,
    );
    let assets = ScreenAssets::load(Install::open(&root).unwrap().store().unwrap());
    let mut g = Game {
        flow,
        root: root.clone(),
        settings,
        rec,
        assets,
        session: None,
        started: Vec::new(),
        booty: None,
    };
    g.effects();
    assert_eq!(g.flow.screen(), Screen::Play);
    // Play the level to its end-of-level screen.
    let mut guard = 0;
    while g.flow.screen() != Screen::Booty {
        g.run(1000.0);
        guard += 1;
        assert!(guard < 1200, "the level is never won");
    }
    // The second level's screen: the amulet, with its gem in the socket.
    let (stats, scene) = g.booty.clone().expect("the end-of-level screen");
    assert_eq!(stats.level, 2);
    assert_eq!(scene.sprites.len(), 1, "the gem");
    assert_eq!(
        (scene.sprites[0].x, scene.sprites[0].y),
        booty::gem_spot(2),
        "in its socket"
    );
    assert_eq!(g.flow.next_level().map(|(l, _)| l), Some(3), "it leads on");
    // Through the screen into the third level, with the progress carried.
    for _ in 0..3 {
        g.flow.menu_input(MenuInput::Confirm, false);
        g.effects();
    }
    let mut guard = 0;
    while g.started.len() < 2 || g.flow.screen() != Screen::Play {
        g.run(100.0);
        guard += 1;
        assert!(guard < 200, "stuck at {:?}", g.flow.screen());
    }
    assert_eq!(g.flow.level(), Some(&LevelChoice::Retail(3)));
    let (n, start) = g.started.last().unwrap().clone();
    assert_eq!(n, 3, "the third level starts");
    let carry = start.carry.expect("with the progress carried");
    assert_eq!(carry.score, stats.score, "the score carried");
    // It plays.
    let before = g.session.as_ref().expect("the third level in play").ticks();
    g.run(500.0);
    let after = g.session.as_ref().expect("the third level in play").ticks();
    assert!(after > before, "the third level runs");
}
