//! Route files: the input script with its header lines, the ROUTE.json
//! report, playing a route back and its timing margin.
//!
//! A route may hold help screens and pauses between two ticks (`@` lines,
//! as a recorded game holds them): a stretch written by hand that throws a
//! stick of dynamite and opens the help screen while it burns, so that it
//! goes off at Claw's side in mid-air. They are played here as the game's
//! own replay plays them ([`play_directives`]).

use claw_sim::claw_frame::GameEvent;
use claw_sim::input::{Directive, DirectiveKind, ScriptCommand};
use claw_sim::logic::enemy::crab::CrabValues;
use claw_sim::{Carry, InputScript, Sim, SimConfig};

use crate::actions::MIN_SPACING;
use crate::search::{Goal, SearchStats, dying};

/// The movement tuning a route was found with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuningName {
    /// The 60 fps feel tuning the game plays with.
    Default,
    /// The original's numbers at a fixed 18 ms step.
    Original,
}

impl TuningName {
    /// The name in a route header.
    pub fn name(self) -> &'static str {
        match self {
            TuningName::Default => "default",
            TuningName::Original => "original",
        }
    }

    /// Reads a name.
    pub fn parse(s: &str) -> Option<TuningName> {
        match s {
            "default" => Some(TuningName::Default),
            "original" => Some(TuningName::Original),
            _ => None,
        }
    }

    /// The tuning itself.
    pub fn tuning(self) -> claw_sim::Tuning {
        match self {
            TuningName::Default => claw_sim::Tuning::default(),
            TuningName::Original => claw_sim::Tuning::original_speeds(),
        }
    }
}

/// The game the bot searches: the game's own simulation settings with
/// `tuning`, the random stream seeded with `seed` and the mid-air hit
/// jump fix off (as the original plays).
pub fn sim_config(tuning: TuningName, seed: u32) -> SimConfig {
    let mut tuning = tuning.tuning();
    tuning.behaviour.fix_double_jump = false;
    SimConfig {
        tuning,
        seed,
        ..SimConfig::default()
    }
}

/// Most ticks [`settle_at`] waits for Claw and the camera to come to rest.
pub const SETTLE_TICKS: usize = 222;

/// Places Claw at (`x`, `y`) in `sim` and steps it with nothing held until
/// Claw and the camera have not moved for 10 ticks (at most
/// [`SETTLE_TICKS`]), then sets the shared random stream to `seed`: the
/// start of a segment, planned from the level-start world as the
/// reference runs of a segment start (they place Claw, wait until he
/// stands, and set the random state at the segment's time zero).
pub fn settle_at(sim: &mut Sim, x: i32, y: i32, seed: u32) {
    settle_at_with(sim, x, y, seed, &CrabValues::default());
}

/// [`settle_at`], the crabs built while Claw settles given their `crabs`
/// values.
pub fn settle_at_with(sim: &mut Sim, x: i32, y: i32, seed: u32, crabs: &CrabValues) {
    sim.teleport_player(x, y);
    let mut still = 0;
    let mut last = None;
    for _ in 0..SETTLE_TICKS {
        sim.step(Default::default());
        crabs.apply(sim);
        let now = sim
            .player()
            .map(|o| (o.x, o.y, o.state, sim.camera().current));
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still >= 10 {
            break;
        }
    }
    sim.set_rng_state(seed);
}

/// What a route header says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Level number.
    pub level: u32,
    /// Movement tuning.
    pub tuning: TuningName,
    /// Seed of the game's random stream at the start.
    pub seed: u32,
    /// The mid-air hit jump fix (off plays as the original).
    pub mid_air_hit_fix: bool,
    /// The hints file the search used, if any.
    pub hints: Option<String>,
    /// Where Claw was placed and settled before the route, when the route
    /// is a segment (see [`settle_at`]).
    pub start: Option<(i32, i32)>,
    /// The progress the level starts with, when it is not a new game's:
    /// the level after another in one game (see [`carry_lines`]).
    pub carry: Option<Carry>,
    /// The values the crabs start with in the route's game (`# crab ...`
    /// lines; see [`CrabValues`]).
    pub crabs: CrabValues,
    /// The further sets of crab values the route was made to win under
    /// too (`# crab_variant N: ...` lines).
    pub crab_variants: Vec<CrabValues>,
}

pub use claw_sim::player::progress::{CARRY_KEYS, START_PREFIX};

/// The comment lines of the progress a level starts with, as a recording
/// of a game's later level writes them (see
/// [`claw_sim::player::progress::start_lines`]).
pub fn carry_lines(c: &Carry) -> String {
    claw_sim::player::progress::start_lines(c)
}

/// The progress a level starts with, read from the `# start_...` comment
/// lines of `text` (see [`claw_sim::player::progress::parse_start_lines`]).
pub fn parse_carry(text: &str) -> Result<Option<Carry>, String> {
    claw_sim::player::progress::parse_start_lines(text)
}

impl Header {
    /// The header's comment lines, each ending in a newline.
    pub fn lines(&self) -> String {
        let mut out = format!(
            "# level {}\n# tuning {}\n# seed {}\n# mid_air_hit_fix = {}\n",
            self.level,
            self.tuning.name(),
            self.seed,
            if self.mid_air_hit_fix { "on" } else { "off" }
        );
        if let Some(h) = &self.hints {
            out.push_str(&format!("# hints {h}\n"));
        }
        if let Some((x, y)) = self.start {
            out.push_str(&format!("# start {x},{y}\n"));
        }
        if let Some(c) = &self.carry {
            out.push_str(&carry_lines(c));
        }
        out.push_str(&self.crabs.header_lines());
        for (i, v) in self.crab_variants.iter().enumerate() {
            out.push_str(&v.variant_lines(i + 1));
        }
        out
    }

    /// Reads the header keys from a route's comment lines; a key that is
    /// missing takes its default (level 1, default tuning, seed 1, fix
    /// off, a new game's progress, also when the start lines are
    /// unreadable, and no crab values, also when their lines are
    /// unreadable: [`crab_values`] reads them with their errors).
    pub fn parse(text: &str) -> Header {
        let mut h = Header {
            level: 1,
            tuning: TuningName::Default,
            seed: 1,
            mid_air_hit_fix: false,
            hints: None,
            start: None,
            carry: parse_carry(text).ok().flatten(),
            crabs: CrabValues::from_header(text).unwrap_or_default(),
            crab_variants: CrabValues::variants_from_header(text).unwrap_or_default(),
        };
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix('#') else {
                continue;
            };
            let rest = rest.trim();
            if let Some((k, v)) = rest.split_once('=') {
                if k.trim() == "mid_air_hit_fix" {
                    h.mid_air_hit_fix = v.trim() == "on";
                }
                continue;
            }
            let mut words = rest.split_whitespace();
            match (words.next(), words.next()) {
                (Some("level"), Some(v)) => h.level = v.parse().unwrap_or(h.level),
                (Some("tuning"), Some(v)) => h.tuning = TuningName::parse(v).unwrap_or(h.tuning),
                (Some("seed"), Some(v)) => h.seed = v.parse().unwrap_or(h.seed),
                (Some("hints"), Some(v)) => h.hints = Some(v.to_string()),
                (Some("start"), Some(v)) => {
                    h.start = v
                        .split_once(',')
                        .and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)));
                }
                _ => {}
            }
        }
        h
    }
}

/// The crab values of a route's header: its own and the further sets
/// it was made to win under.
pub fn crab_values(text: &str) -> Result<(CrabValues, Vec<CrabValues>), String> {
    Ok((
        CrabValues::from_header(text).map_err(|e| format!("the crab values: {e}"))?,
        CrabValues::variants_from_header(text)
            .map_err(|e| format!("the further crab values: {e}"))?,
    ))
}

/// The route file: header lines, then the script.
pub fn route_csv(header: &Header, script: &InputScript) -> String {
    format!("{}{}", header.lines(), script.to_csv())
}

/// Plays the directives of `script` due at the boundary before tick
/// `tick` on `sim`, as the game's replay does: when play comes back from a
/// help screen, the time it was up reaches the dynamite fuse; when it comes
/// back from any screen, the keys down count as already pressed (the
/// game's original rule for keys); and as play resumes after a pause or a
/// screen, the game's clock is reset. The in-game menu's end of a life
/// ends it.
pub fn play_directives(sim: &mut Sim, script: &InputScript, tick: usize) {
    for d in script.directives.iter().filter(|d| d.tick == tick) {
        play_directive(sim, d);
    }
}

fn play_directive(sim: &mut Sim, d: &Directive) {
    match &d.kind {
        DirectiveKind::Menu {
            screen, wall_ms, ..
        } => {
            if *screen == claw_sim::input::MenuScreen::Help {
                sim.pass_menu_time(*wall_ms);
            }
            sim.restart_idle_wait();
            sim.latch_keys();
        }
        DirectiveKind::Pause { .. } => sim.restart_idle_wait(),
        DirectiveKind::Command(ScriptCommand::EndLife) => sim.end_life(),
        DirectiveKind::Command(ScriptCommand::Powerup(p)) => sim.give_powerup(*p),
    }
}

/// Ticks at which the held mask changes (tick 0 counts when anything is
/// held from the start).
pub fn changes(script: &InputScript) -> Vec<usize> {
    let mut prev = 0;
    let mut out = Vec::new();
    for (i, t) in script.ticks.iter().enumerate() {
        if t.held.0 != prev {
            out.push(i);
        }
        prev = t.held.0;
    }
    out
}

/// Changes of the held mask closer than `min` ticks to the previous
/// change: (tick, gap).
pub fn spacing_violations(script: &InputScript, min: usize) -> Vec<(usize, usize)> {
    changes(script)
        .windows(2)
        .filter(|w| w[1] - w[0] < min)
        .map(|w| (w[1], w[1] - w[0]))
        .collect()
}

/// How a route played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Played {
    /// The tick in which the goal was met, if it was.
    pub done_at: Option<u64>,
    /// Claw died.
    pub died: bool,
    /// Health lost.
    pub health_lost: i32,
}

impl Played {
    /// The goal was met with no death before it.
    pub fn won(&self) -> bool {
        self.done_at.is_some() && !self.died
    }
}

/// Plays `script` from `start` (stepping a copy) and reports when `goal`
/// was met.
pub fn play(start: &Sim, script: &InputScript, goal: &Goal) -> Played {
    play_with(start, script, goal, &CrabValues::default())
}

/// [`play`], the crabs built along the way given their `crabs` values.
pub fn play_with(start: &Sim, script: &InputScript, goal: &Goal, crabs: &CrabValues) -> Played {
    let mut sim = start.clone();
    let mut out = Played {
        done_at: None,
        died: false,
        health_lost: 0,
    };
    let lives = sim.stats().lives;
    for (i, &t) in script.ticks.iter().enumerate() {
        play_directives(&mut sim, script, i);
        let health = sim.stats().health;
        sim.step(t);
        crabs.apply(&mut sim);
        out.health_lost += (health - sim.stats().health).max(0);
        for e in sim.last_events() {
            match e {
                GameEvent::LevelComplete { .. } if *goal == Goal::LevelComplete => {
                    out.done_at.get_or_insert(i as u64);
                }
                GameEvent::ClawDied { .. } | GameEvent::GameOver => out.died = true,
                _ => {}
            }
        }
        if goal.met_at(&sim) {
            out.done_at.get_or_insert(i as u64);
        }
        if dying(&sim) || sim.stats().lives < lives {
            out.died = true;
        }
        if out.done_at.is_some() || out.died {
            break;
        }
    }
    out
}

/// How much the route's timing can slip: sampled changes of the held
/// mask, each played one tick early and one tick late on its own (the
/// route's help screens and pauses staying where they are).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Margin {
    /// Changes sampled.
    pub sampled: usize,
    /// Of the sampled changes played a tick early, the ones that still win.
    pub early_wins: usize,
    /// Of the sampled changes played a tick late, the ones that still win.
    pub late_wins: usize,
    /// Ticks of the sampled changes whose shift lost the route.
    pub fragile: Vec<usize>,
}

/// The [`Margin`] of `script` from `start`, sampling at most `samples`
/// changes spread evenly over the route.
pub fn margin(start: &Sim, script: &InputScript, goal: &Goal, samples: usize) -> Margin {
    margin_with(start, script, goal, samples, &CrabValues::default())
}

/// [`margin`], the crabs built along the way given their `crabs` values.
pub fn margin_with(
    start: &Sim,
    script: &InputScript,
    goal: &Goal,
    samples: usize,
    crabs: &CrabValues,
) -> Margin {
    let held: Vec<u32> = script.ticks.iter().map(|t| t.held.0).collect();
    let all: Vec<usize> = changes(script).into_iter().filter(|&c| c > 0).collect();
    let picked: Vec<usize> = if all.len() <= samples {
        all
    } else {
        (0..samples).map(|k| all[k * all.len() / samples]).collect()
    };
    let mut m = Margin {
        sampled: picked.len(),
        ..Margin::default()
    };
    for &c in &picked {
        // A tick early: the new mask starts at c - 1.
        let mut early = held.clone();
        early[c - 1] = held[c];
        // A tick late: the old mask runs through c.
        let mut late = held.clone();
        late[c] = held[c - 1];
        let shifted = |held: &[u32]| InputScript {
            directives: script.directives.clone(),
            ..InputScript::from_held(held)
        };
        let early_won = play_with(start, &shifted(&early), goal, crabs).won();
        let late_won = play_with(start, &shifted(&late), goal, crabs).won();
        m.early_wins += usize::from(early_won);
        m.late_wins += usize::from(late_won);
        if !early_won || !late_won {
            m.fragile.push(c);
        }
    }
    m
}

/// The ROUTE.json report.
pub fn report_json(
    header: &Header,
    stats: &SearchStats,
    done_at: u64,
    margin: &Margin,
    variants: &[(Played, Margin)],
    seconds: f64,
) -> String {
    let fragile: Vec<String> = margin.fragile.iter().map(|t| t.to_string()).collect();
    let variants: Vec<String> = variants
        .iter()
        .map(|(p, m)| {
            format!(
                "{{\"done_at_tick\": {}, \"health_lost\": {}, \"margin_sampled\": {}, \"early_wins\": {}, \"late_wins\": {}}}",
                p.done_at.map_or("null".to_string(), |t| t.to_string()),
                p.health_lost,
                m.sampled,
                m.early_wins,
                m.late_wins
            )
        })
        .collect();
    format!(
        concat!(
            "{{\n",
            "  \"level\": {},\n",
            "  \"tuning\": \"{}\",\n",
            "  \"seed\": {},\n",
            "  \"hints\": {},\n",
            "  \"nodes\": {},\n",
            "  \"generated\": {},\n",
            "  \"deaths_pruned\": {},\n",
            "  \"landings_pruned\": {},\n",
            "  \"entries_pruned\": {},\n",
            "  \"crab_variants_pruned\": {},\n",
            "  \"fragile_children\": {},\n",
            "  \"fragile_on_route\": {},\n",
            "  \"duplicates\": {},\n",
            "  \"rebuilt\": {},\n",
            "  \"segments\": {},\n",
            "  \"segments_backed_out\": {},\n",
            "  \"ticks\": {},\n",
            "  \"done_at_tick\": {},\n",
            "  \"deaths\": {},\n",
            "  \"health_lost\": {},\n",
            "  \"min_spacing_ticks\": {},\n",
            "  \"margin\": {{\n",
            "    \"note\": \"informational: each sampled change of the held mask shifted by one tick on its own\",\n",
            "    \"sampled\": {},\n",
            "    \"early_wins\": {},\n",
            "    \"late_wins\": {},\n",
            "    \"fragile_ticks\": [{}]\n",
            "  }},\n",
            "  \"crab_variants\": [{}],\n",
            "  \"search_seconds\": {:.1}\n",
            "}}\n"
        ),
        header.level,
        header.tuning.name(),
        header.seed,
        header
            .hints
            .as_ref()
            .map_or("null".to_string(), |h| format!("\"{h}\"")),
        stats.expanded,
        stats.generated,
        stats.deaths_pruned,
        stats.landings_pruned,
        stats.entries_pruned,
        stats.variants_pruned,
        stats.fragile,
        stats.fragile_on_route,
        stats.duplicates,
        stats.rebuilt,
        stats.segments,
        stats.backed_out,
        stats.ticks,
        done_at,
        stats.deaths,
        stats.health_lost,
        MIN_SPACING,
        margin.sampled,
        margin.early_wins,
        margin.late_wins,
        fragile.join(", "),
        variants.join(", "),
        seconds
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use claw_sim::claw_frame::Weapon;
    use claw_sim::input::ScriptTuning;

    #[test]
    fn headers_round_trip_and_the_script_still_parses() {
        let h = Header {
            level: 3,
            tuning: TuningName::Original,
            seed: 42,
            mid_air_hit_fix: false,
            hints: Some("level3".into()),
            start: Some((10355, 2051)),
            carry: None,
            crabs: CrabValues::parse("crab 101 = 2, 300, 40\nnest_crab 1304 1 = 0, 0, 636097\n")
                .unwrap(),
            crab_variants: vec![
                CrabValues::parse("crab 101 = 2, 382, 382\n").unwrap(),
                CrabValues::parse("nest_crab * * = 1, 1000000, 1000000\n").unwrap(),
            ],
        };
        let s = InputScript::from_held(&[0, 0, 1, 1, 0, 0]);
        let text = route_csv(&h, &s);
        assert_eq!(Header::parse(&text), h);
        let parsed = InputScript::parse(&text).unwrap();
        assert_eq!(parsed.ticks, s.ticks);
        assert!(parsed.directives.is_empty());
        assert_eq!(parsed.header.level, Some(3));
        assert_eq!(parsed.header.tuning, Some(ScriptTuning::Original));
        assert_eq!(parsed.header.seed, Some(42));
        assert!(text.contains("# mid_air_hit_fix = off\n"));
        assert!(text.contains("# crab 101 = 2, 300, 40\n# nest_crab 1304 1 = 0, 0, 636097\n"));
        assert!(text.contains("# crab_variant 2: nest_crab * * = 1, 1000000, 1000000\n"));
        assert_eq!(
            crab_values(&text),
            Ok((h.crabs.clone(), h.crab_variants.clone()))
        );
        let e = crab_values("# crab 1 = 2\n").unwrap_err();
        assert!(e.contains("the crab values"), "{e}");
    }

    #[test]
    fn the_carried_progress_round_trips_through_the_start_lines() {
        let carry = Carry {
            score: 25_000,
            health: 85,
            lives: 8,
            pistol: 12,
            magic: 5,
            dynamite: 3,
            weapon: Weapon::Magic,
            treasure_game: [10, 2, 0, 0, 0, 1, 0, 0, 0],
        };
        let lines = carry_lines(&carry);
        assert!(lines.starts_with("# start_score = 25000\n# start_health = 85\n"));
        assert!(lines.ends_with("# start_treasure_game = 10, 2, 0, 0, 0, 1, 0, 0, 0\n"));
        assert_eq!(parse_carry(&lines), Ok(Some(carry.clone())));
        let h = Header {
            level: 2,
            tuning: TuningName::Original,
            seed: 1_617_999_171,
            mid_air_hit_fix: false,
            hints: None,
            start: None,
            carry: Some(carry),
            crabs: CrabValues::default(),
            crab_variants: Vec::new(),
        };
        let text = route_csv(&h, &InputScript::from_held(&[0, 0]));
        assert_eq!(Header::parse(&text), h);
        assert_eq!(InputScript::parse(&text).unwrap().ticks.len(), 2);
        // No start lines: a new game's progress; other start keys are not
        // progress.
        assert_eq!(parse_carry("# level 2\n# start_x = 5\n0,0,0\n"), Ok(None));
        let e = parse_carry("# start_score = 1\n").unwrap_err();
        assert!(e.contains("start_health is missing"), "{e}");
        let e = parse_carry(&lines.replace("magic\n", "sword\n")).unwrap_err();
        assert!(e.contains("start_weapon"), "{e}");
    }

    #[test]
    fn spacing_is_measured_between_changes() {
        let ok = InputScript::from_held(&[1, 1, 0, 0, 2, 2]);
        assert!(spacing_violations(&ok, 2).is_empty());
        let bad = InputScript::from_held(&[1, 1, 0, 2, 2]);
        assert_eq!(spacing_violations(&bad, 2), vec![(3, 1)]);
    }
}
