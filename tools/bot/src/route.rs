//! Route files: the input script with its header lines, the ROUTE.json
//! report, playing a route back and its timing margin.

use claw_sim::claw_frame::GameEvent;
use claw_sim::{InputScript, Sim, SimConfig};

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
        out
    }

    /// Reads the header keys from a route's comment lines; a key that is
    /// missing takes its default (level 1, default tuning, seed 1, fix
    /// off).
    pub fn parse(text: &str) -> Header {
        let mut h = Header {
            level: 1,
            tuning: TuningName::Default,
            seed: 1,
            mid_air_hit_fix: false,
            hints: None,
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
                _ => {}
            }
        }
        h
    }
}

/// The route file: header lines, then the script.
pub fn route_csv(header: &Header, script: &InputScript) -> String {
    format!("{}{}", header.lines(), script.to_csv())
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
    let mut sim = start.clone();
    let mut out = Played {
        done_at: None,
        died: false,
        health_lost: 0,
    };
    let lives = sim.stats().lives;
    for (i, &t) in script.ticks.iter().enumerate() {
        let health = sim.stats().health;
        sim.step(t);
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
        if let Goal::Reach { x, y, radius } = *goal
            && sim.player().is_some_and(|o| {
                let (dx, dy) = (i64::from(o.x - x), i64::from(o.y - y));
                dx * dx + dy * dy <= i64::from(radius) * i64::from(radius)
            })
        {
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
/// mask, each played one tick early and one tick late on its own.
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
        let early_won = play(start, &InputScript::from_held(&early), goal).won();
        let late_won = play(start, &InputScript::from_held(&late), goal).won();
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
    seconds: f64,
) -> String {
    let fragile: Vec<String> = margin.fragile.iter().map(|t| t.to_string()).collect();
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
            "  \"duplicates\": {},\n",
            "  \"rebuilt\": {},\n",
            "  \"segments\": {},\n",
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
        stats.duplicates,
        stats.rebuilt,
        stats.segments,
        stats.ticks,
        done_at,
        stats.deaths,
        stats.health_lost,
        MIN_SPACING,
        margin.sampled,
        margin.early_wins,
        margin.late_wins,
        fragile.join(", "),
        seconds
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use claw_sim::input::ScriptTuning;

    #[test]
    fn headers_round_trip_and_the_script_still_parses() {
        let h = Header {
            level: 3,
            tuning: TuningName::Original,
            seed: 42,
            mid_air_hit_fix: false,
            hints: Some("level3".into()),
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
    }

    #[test]
    fn spacing_is_measured_between_changes() {
        let ok = InputScript::from_held(&[1, 1, 0, 0, 2, 2]);
        assert!(spacing_violations(&ok, 2).is_empty());
        let bad = InputScript::from_held(&[1, 1, 0, 2, 2]);
        assert_eq!(spacing_violations(&bad, 2), vec![(3, 1)]);
    }
}
