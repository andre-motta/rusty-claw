//! Best-first search over macro-actions in the headless simulation.
//!
//! Each node is the game after a chain of [`Action`]s from the start. The
//! search expands the node with the lowest priority: the path distance from
//! Claw to the current target on the coarse graph ([`crate::field`]), plus
//! a small cost per tick and a larger one per point of health lost (in the
//! flight of a throw, less his height over the springboard or blower that
//! threw him: the throw rises away from the target before it carries him
//! on; on an elevator or a path elevator, the least of the distances from
//! the places its way takes him, each with the ride there). A child
//! in which Claw dies is dropped. Children whose coarse state (Claw's cell,
//! state, facing and health, and the platforms and enemies around him) was
//! seen before are dropped too.
//!
//! Games are only ever cloned and stepped: [`Sim::clone`] is the savestate
//! and [`Sim::step`] the only way forward. A game takes megabytes, so only
//! the most promising nodes keep theirs; any other is rebuilt by stepping
//! the actions from its nearest ancestor that kept one.
//!
//! Waypoints split the search into segments: a hint point reached, or a
//! checkpoint touched well ahead of where the segment started, becomes the
//! root of a fresh search. The segment it ended is set aside: when the new
//! segment runs out of states (its root was a dead end), the search goes
//! back to the segment before and carries on there without that root.
//!
//! A fresh game starts with [`MIN_SPACING`] idle ticks: keys already down
//! when play starts count only once released and pressed again, so the
//! route's first press comes after tick 0, even played a tick early. With [`SearchConfig::land_gap`] set, a jump is
//! never pressed within that many ticks of a landing: at 60 frames per
//! second the original may land a frame from where the game does, and a
//! press on its landing frame is lost.
//!
//! With [`SearchConfig::fragile_cost`] set, each child is also played
//! with its timing shifted by a tick (it starts a tick late, and each
//! change of the held mask inside it comes a tick early or late) and with
//! the shared random stream a draw or two further on (so the enemies
//! decide otherwise). Besides, every move is also played in two shadow
//! games that follow the route from a slightly different start (see
//! [`Shadow`]): a tick ahead, and with the random stream a draw further
//! on. When a variant or a shadow dies, loses more health or ends
//! somewhere else (another floor, another phase, or more than
//! [`FRAGILE_PX`] away), the child is fragile and costs that much more,
//! and the shadow starts again from the child. Routes then keep to moves
//! with room to spare, so they still play the same when the timing slips
//! by a tick somewhere before.
//!
//! A level that ends with a boss ([`boss_end`]) is searched in three
//! stages ([`BossStage`]): to the boss room's door, the fight, and the gem
//! the boss's defeat releases. The distance the search is guided by is the
//! path distance to the door, then the boss's health and Claw's distance
//! to him (none while Claw carries him or he flies from Claw's throw, and,
//! for a boss no blow hurts ([`drowns`]), plus his distance from the water
//! beside him, where a throw drowns him), then the path distance to where
//! the gem lands; each stage costs
//! less than any state of the stage before, and the first state of a new
//! stage becomes the root of a fresh segment, as a waypoint does. Through
//! the boss's introduction and the gem's flight Claw is held, so the time
//! is part of the coarse state there, and so it is while a boss Claw threw
//! cannot be lifted again yet. A throw may land a boss to be drowned
//! anywhere, a peg reaching past a drop's edge included: his bounces there
//! play as in the original game, so the search keeps no landing out. With
//! [`SearchConfig::intro`] the search keeps to routes that play the
//! introduction (the normal entry) or to routes that skip it (the jump-in
//! entry).
//!
//! Marrow's fight on the tenth level runs in rounds, and is guided round
//! by round ([`marrow_guide`]): while the pit is open between Claw and
//! Marrow, by the parrot's hits (three close the slabs into a bridge) and
//! how near Claw is to its dive, then by Marrow's health and Claw's
//! distance to him; a round whose whistle is due counts as the next
//! one's start. While the parrot patrols, the stretch of the room it is
//! over and the time make the coarse state, so a wait for its dive is
//! progress. When Marrow lands from the parrot's carry, the step after
//! decides what he does by where Claw is in his range boxes, built at
//! the height he hung from; a child that leaves Claw within
//! [`MARROW_BOX_CLEAR`] px of an edge of those boxes, or of the boxes
//! built where Marrow landed, is dropped ([`on_marrow_box_edge`]): a
//! pixel there decides the landing's outcome.
//!
//! On the eleventh level, a Siren's scream that touches Claw takes his
//! controls for two seconds ([`actions::locked`]): no key acts until the
//! lockout has run down, so the time is part of the coarse state while it
//! runs, and only the wait and the walks are tried. Near a toggle peg
//! ([`TOGGLE_NEAR`]), where the peg is in its cycle, to a tenth of a
//! second, is part of the coarse state too: its picture stays the same
//! for most of a second out or in the wall, and a jump lands on it only
//! when it is timed to that cycle. Beside a laser ([`LASER_NEAR`]),
//! whether its beam hurts, its picture and the time are part of it, as
//! beside a goo vent, so a wait for the beam to stop is progress.
//!
//! A route can be made to win under several sets of the values the
//! hermit crabs start with ([`CrabValues`], which the original game does
//! not fix): [`SearchConfig::crabs`] is the set the route plays under,
//! and [`search_report_with`] takes further games, each started under
//! another set, that play every move too. A child in which Claw dies in
//! one of them is dropped, and so is one that meets the goal while one of
//! them has not won the level by the end of the same move (or, for a goal
//! point, has not come within twice the goal's distance of it, on his
//! feet when the goal asks it). The
//! health Claw loses in them counts in a node's priority as his own does
//! (the most any of them loses in each move), the priority's distance is
//! that of the game furthest from the target, so none is left behind, and
//! how far each one's Claw is from the route's own (across plus up or
//! down, times [`SearchConfig::drift_cost`]) adds to it, so they keep
//! together; a node becomes a segment root only with each of them within
//! [`TOGETHER_PX`] of it.
//!
//! The search is deterministic. Its budget counts expanded nodes, never
//! time; ties go by a hash of the seed and the node's state; and the
//! children of a node may be played on several threads, but they are
//! gathered in the order of the actions, so the thread count never changes
//! the result.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};
use std::time::{Duration, Instant};

use claw_sim::claw_frame::GameEvent;
use claw_sim::logic::enemy::EnemyState;
use claw_sim::logic::enemy::crab::CrabValues;
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{
    ObjectData, boss_stage, enemy, floor_spike, goo_vent, laser, path_elevator, pickup, platform,
    springboard,
};
use claw_sim::object::special;
use claw_sim::player::special::BOSS_END;
use claw_sim::player::state;
use claw_sim::tiles::{DEATH, GROUND, SOLID};
use claw_sim::{Buttons, InputScript, Sim, SimLevel, TickInput};

use crate::actions::{self, Action, Arena, MARROW_PIT, MIN_SPACING, parrot_dives, parrot_waits};
use crate::field::{BLOWER_HALF_WIDTH, DistanceField, Grid, Reach, UNREACHABLE};

/// What ends the search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Goal {
    /// The level is won: a tick reports the level complete. The search is
    /// guided toward the level's end object.
    LevelComplete,
    /// Claw's position comes within `radius` px of (`x`, `y`).
    Reach {
        /// Target x.
        x: i32,
        /// Target y.
        y: i32,
        /// Distance that counts as there, px.
        radius: i32,
    },
    /// Claw comes within `radius` px of (`x`, `y`) on his feet, on a
    /// ladder or on a rope, never in the middle of a jump or a fall: a
    /// stretch of a route that ends there leaves him where he can stay.
    Stand {
        /// Target x.
        x: i32,
        /// Target y.
        y: i32,
        /// Distance that counts as there, px.
        radius: i32,
    },
}

impl Goal {
    /// True when `sim` meets this goal by Claw's place alone (the level
    /// complete goal is met by its event).
    pub fn met_at(&self, sim: &Sim) -> bool {
        match *self {
            Goal::LevelComplete => false,
            Goal::Reach { x, y, radius } => near(sim, x, y, radius),
            Goal::Stand { x, y, radius } => near(sim, x, y, radius) && on_foot(sim),
        }
    }
}

/// Search settings.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchConfig {
    /// Budget: nodes expanded at most.
    pub nodes: u64,
    /// Breaks ties between nodes of equal priority.
    pub seed: u64,
    /// Waypoints visited in order before the goal, px. Written from level
    /// geometry, never from recorded play.
    pub hints: Vec<(i32, i32)>,
    /// Distance at which a hint counts as reached, px.
    pub hint_radius: i32,
    /// A hint counts as reached only with Claw on his feet, on a ladder
    /// or on a rope, within twice [`hint_radius`](Self::hint_radius) of it
    /// (a point in the air above a floor counts from the floor), never in
    /// the middle of a jump or a fall: the next segment then starts from
    /// where he can stay.
    pub hint_on_foot: bool,
    /// The hints, by index into [`hints`](Self::hints), that count as
    /// reached only on foot as [`hint_on_foot`](Self::hint_on_foot) makes
    /// every hint count; the others count anywhere near them.
    pub foot_hints: Vec<usize>,
    /// Threads that play the children of a node (1 plays them in turn).
    /// The result does not depend on it.
    pub threads: usize,
    /// Games kept in memory at most, besides the segment root (a node's
    /// shadows count as games of their own).
    pub cached_sims: usize,
    /// A safety stop: give up after this much wall time. It ends the
    /// search with a failure, never with a different route.
    pub wall_limit: Option<Duration>,
    /// Longest route, ticks.
    pub max_ticks: u64,
    /// Priority cost of one tick, px.
    pub tick_cost: i64,
    /// Priority cost of one point of health lost, px.
    pub health_cost: i64,
    /// Progress toward the target a checkpoint must show over the segment
    /// root to become a new root, px.
    pub checkpoint_progress: u32,
    /// What the coarse graph assumes Claw can reach.
    pub reach: Reach,
    /// Print progress every this many expanded nodes (0: never).
    pub log_every: u64,
    /// The mask held in the tick before the start, when the start is the
    /// game part way through another route (0 for a fresh game).
    pub start_held: u32,
    /// Ticks [`start_held`](Self::start_held) has been held since it last
    /// changed (`usize::MAX` when it never did). When this is under
    /// [`MIN_SPACING`], the route found first holds it for the ticks
    /// missing, so the spacing holds across the join.
    pub start_held_for: usize,
    /// Fewest ticks between a press of jump and a landing of Claw, either
    /// way round (0: no rule). A child that breaks it is dropped.
    pub land_gap: u64,
    /// Priority cost of a fragile child, px (0: children are not checked
    /// for fragility).
    pub fragile_cost: i64,
    /// With the fragility check on, also play every move in the shadow
    /// games (see [`Shadow`]).
    pub shadows: bool,
    /// How a boss level's route enters the boss room (see [`Intro`]).
    pub intro: Intro,
    /// The values the crabs start with in the route's game and its
    /// shadows (none: the plain default).
    pub crabs: CrabValues,
    /// Priority cost of each px Claw in a game under further crab values
    /// is from the route's own Claw (across plus up or down), px (see
    /// [`search_report_with`]).
    pub drift_cost: i64,
}

/// How a route enters a boss room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Intro {
    /// Either way.
    #[default]
    Any,
    /// The normal way: the boss's introduction plays. A move that leaves
    /// Claw in the room for more than [`INTRO_GRACE`] ticks with the
    /// introduction not started is dropped.
    Play,
    /// The jump-in: the introduction never starts. A move in which it
    /// starts is dropped.
    Skip,
}

impl Intro {
    /// Reads `any`, `play` or `skip`.
    pub fn parse(s: &str) -> Option<Intro> {
        match s {
            "any" => Some(Intro::Any),
            "play" => Some(Intro::Play),
            "skip" => Some(Intro::Skip),
            _ => None,
        }
    }

    /// The name [`Intro::parse`] reads.
    pub fn name(self) -> &'static str {
        match self {
            Intro::Any => "any",
            Intro::Play => "play",
            Intro::Skip => "skip",
        }
    }
}

/// Ticks Claw may be in a boss room before the introduction starts, with
/// [`Intro::Play`].
pub const INTRO_GRACE: u64 = 4;

impl Default for SearchConfig {
    fn default() -> SearchConfig {
        SearchConfig {
            nodes: 200_000,
            seed: 1,
            hints: Vec::new(),
            hint_radius: 48,
            hint_on_foot: false,
            foot_hints: Vec::new(),
            threads: 1,
            cached_sims: 512,
            wall_limit: None,
            max_ticks: 60_000,
            tick_cost: 1,
            health_cost: 12,
            checkpoint_progress: 320,
            reach: Reach::default(),
            log_every: 0,
            start_held: 0,
            start_held_for: usize::MAX,
            land_gap: 0,
            fragile_cost: 0,
            shadows: true,
            intro: Intro::Any,
            crabs: CrabValues::default(),
            drift_cost: 1,
        }
    }
}

/// How far across the end of a child played with shifted timing may be from
/// its end as found, px, before the child counts as fragile.
pub const FRAGILE_PX: i32 = 32;

/// How far up or down, px.
pub const FRAGILE_DY: i32 = 16;

/// A shadow game: the route played from a slightly different start, to
/// see whether a move still works when what came before went a little
/// differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shadow {
    /// One tick ahead: the last held mask kept for one more tick, so Claw
    /// is a tick further along and the level's clock, its enemies and the
    /// random stream are a tick on.
    Ahead,
    /// The shared random stream one draw further on, so the enemies decide
    /// otherwise.
    Draw,
}

/// The shadow games every move is also played in.
pub const SHADOWS: [Shadow; 2] = [Shadow::Ahead, Shadow::Draw];

impl Shadow {
    /// `main` started over as this shadow; `last` is the mask it held last.
    fn start(self, main: &Sim, last: u32, crabs: &CrabValues) -> Sim {
        let mut sim = main.clone();
        match self {
            Shadow::Ahead => {
                step(&mut sim, input(last, last), crabs);
            }
            Shadow::Draw => {
                let mut r = claw_sim::rng::Rng::new(sim.rng_state());
                r.next_u15();
                sim.set_rng_state(r.state());
            }
        }
        sim
    }
}

/// One tick of `sim`, the crabs it builds given their `crabs` values.
fn step(sim: &mut Sim, t: TickInput, crabs: &CrabValues) {
    sim.step(t);
    crabs.apply(sim);
}

/// The crab values the games of a node play under: the route's own (its
/// game and the shadows), and each further game's.
#[derive(Debug, Clone, Copy)]
struct Values<'a> {
    main: &'a CrabValues,
    variants: &'a [CrabValues],
}

/// The games of a node: the route as found, its shadows (none when the
/// fragility check is off) and the games under further crab values.
#[derive(Debug, Clone)]
struct Worlds {
    main: Sim,
    shadows: Vec<Sim>,
    variants: Vec<Sim>,
}

impl Worlds {
    /// `main` with fresh shadows when `shadows` is set, and `variants`.
    fn new(main: Sim, last: u32, shadows: bool, variants: Vec<Sim>, crabs: &CrabValues) -> Worlds {
        let shadows = if shadows {
            SHADOWS
                .iter()
                .map(|s| s.start(&main, last, crabs))
                .collect()
        } else {
            Vec::new()
        };
        Worlds {
            main,
            shadows,
            variants,
        }
    }

    /// Games kept for this node.
    fn games(&self) -> usize {
        1 + self.shadows.len() + self.variants.len()
    }

    fn step(&mut self, t: TickInput, values: Values) {
        step(&mut self.main, t, values.main);
        for s in &mut self.shadows {
            step(s, t, values.main);
        }
        for (s, crabs) in self.variants.iter_mut().zip(values.variants) {
            step(s, t, crabs);
        }
    }

    /// Starts the shadows of the bits of `reseed` over from the main game.
    fn reseed(&mut self, reseed: u8, last: u32, crabs: &CrabValues) {
        for (i, kind) in SHADOWS.iter().enumerate() {
            if reseed & (1 << i) != 0
                && let Some(s) = self.shadows.get_mut(i)
            {
                *s = kind.start(&self.main, last, crabs);
            }
        }
    }
}

/// What a search did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchStats {
    /// Nodes expanded.
    pub expanded: u64,
    /// Children played.
    pub generated: u64,
    /// Children dropped because Claw died in them.
    pub deaths_pruned: u64,
    /// Children dropped because a jump was pressed too close to a landing
    /// (see [`SearchConfig::land_gap`]).
    pub landings_pruned: u32,
    /// Children dropped because they entered a boss room the other way
    /// (see [`SearchConfig::intro`]).
    pub entries_pruned: u32,
    /// Children dropped because they left Claw on an edge of Marrow's
    /// range boxes as he landed (see [`on_marrow_box_edge`]).
    pub box_edges_pruned: u32,
    /// Children dropped because Claw died in a game under further crab
    /// values, or it did not meet the goal there (see
    /// [`search_report_with`]).
    pub variants_pruned: u64,
    /// Children found fragile (see [`SearchConfig::fragile_cost`]).
    pub fragile: u64,
    /// Fragile moves along the route found.
    pub fragile_on_route: u32,
    /// Children dropped as already seen.
    pub duplicates: u64,
    /// Games rebuilt by stepping from an ancestor.
    pub rebuilt: u64,
    /// Segment roots taken (hints reached and checkpoints passed).
    pub segments: u32,
    /// Segments given up as dead ends.
    pub backed_out: u32,
    /// Ticks of the route found (0 without one).
    pub ticks: u64,
    /// Health Claw lost along the route found.
    pub health_lost: i32,
    /// Deaths along the route found (always 0: deaths are pruned).
    pub deaths: u32,
    /// Least distance to the target seen, px.
    pub best_distance: u32,
    /// Where Claw was in the node closest to the target.
    pub best_at: (i32, i32),
}

/// A route found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The route, one input per tick from the start. Its first ticks hold
    /// [`SearchConfig::start_held`] when that mask was held for fewer than
    /// [`MIN_SPACING`] ticks; pressed buttons are the rising edges from
    /// that mask.
    pub script: InputScript,
    /// What the search did.
    pub stats: SearchStats,
    /// The tick (0-based, within the route) in which the goal was met.
    pub done_at: u64,
}

/// Why a search found nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFailure {
    /// What ran out or went wrong.
    pub reason: String,
    /// What the search did (boxed: the failure stays small to pass
    /// back).
    pub stats: Box<SearchStats>,
}

impl std::fmt::Display for SearchFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} after {} nodes (closest {} px, at {:?})",
            self.reason, self.stats.expanded, self.stats.best_distance, self.stats.best_at
        )
    }
}

impl std::error::Error for SearchFailure {}

/// Searches for a route from `start` that meets `goal`.
pub fn search(start: &Sim, goal: Goal, cfg: &SearchConfig) -> Result<InputScript, SearchFailure> {
    search_report(start, goal, cfg).map(|f| f.script)
}

/// [`search`], with what the search did.
pub fn search_report(start: &Sim, goal: Goal, cfg: &SearchConfig) -> Result<Found, SearchFailure> {
    search_report_with(start, &[], goal, cfg)
}

/// [`search_report`], with further games that every move is also played
/// in: each the game at the start under another set of crab values (with
/// the crabs built so far given that set's), stepped with that set. A
/// child in which Claw dies in one of them is dropped, and so is one that
/// meets the goal while one of them does not come as far (see
/// [`played_under`]), so the route found wins under every set.
pub fn search_report_with(
    start: &Sim,
    variants: &[(Sim, CrabValues)],
    goal: Goal,
    cfg: &SearchConfig,
) -> Result<Found, SearchFailure> {
    let fail = |reason: String| SearchFailure {
        reason,
        stats: Box::default(),
    };
    // Keep the start mask held until it has lasted the spacing, so the
    // first change the search makes is far enough from the last one
    // before the start. A fresh game, whose keys are all latched, idles
    // for its first ticks, so the first press comes after tick 0 even a
    // tick early.
    let mut lead_len = MIN_SPACING.saturating_sub(cfg.start_held_for);
    if !start.latched().is_empty() && cfg.start_held == 0 {
        lead_len = lead_len.max(MIN_SPACING);
    }
    let lead = vec![cfg.start_held; lead_len];
    let mut start = start.clone();
    let mut variant_sims: Vec<Sim> = variants.iter().map(|(s, _)| s.clone()).collect();
    let variant_crabs: Vec<CrabValues> = variants.iter().map(|(_, c)| c.clone()).collect();
    for &held in &lead {
        step(&mut start, input(held, cfg.start_held), &cfg.crabs);
        for (s, crabs) in variant_sims.iter_mut().zip(&variant_crabs) {
            step(s, input(held, cfg.start_held), crabs);
        }
    }
    let start = &start;
    let grid = Grid::build(start.level(), cfg.reach);
    // The fields are walked by Claw's feet; a point he is to reach is his
    // middle.
    let feet = start.player().map_or(60, |o| o.move_rect.bottom);
    let mut targets: Vec<Target> = cfg
        .hints
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| Target {
            field: grid.field_to(x, y + feet, cfg.hint_radius),
            point: Some((x, y, cfg.hint_radius)),
            on_foot: cfg.hint_on_foot || cfg.foot_hints.contains(&i),
        })
        .collect();
    // Claw takes a pickup with any part of his body: his feet may be up to
    // his height below it.
    let pickup_field =
        |(x, y): (i32, i32)| grid.field_to_box(x - 40, y - 30, x + 40, y + 2 * feet + 20);
    let mut boss = None;
    let field = match goal {
        Goal::LevelComplete => match (end_object(start), boss_end(start)) {
            (Some(at), _) => pickup_field(at),
            (None, Some(end)) => {
                boss = Some(BossGoal {
                    end,
                    gem: pickup_field(end.gem),
                });
                // The door is entered by touching it.
                let (x, y) = end.door;
                grid.field_to_box(x - 24, y - 40, x + 24, y + feet + 40)
            }
            (None, None) => return Err(fail("the level has no end object to head for".into())),
        },
        Goal::Reach { x, y, radius } | Goal::Stand { x, y, radius } => {
            grid.field_to(x, y + feet, radius.max(8))
        }
    };
    targets.push(Target {
        field,
        point: None,
        on_foot: false,
    });
    let arena = Arena {
        door: boss.as_ref().map(|b| b.end.door),
        boss_side: boss
            .as_ref()
            .map_or(0, |b| if b.end.boss.0 < b.end.room.0 { -1 } else { 1 }),
    };
    let mut s = Search {
        cfg,
        goal,
        boss,
        arena,
        grid,
        targets,
        target: 0,
        nodes: Vec::new(),
        open: BinaryHeap::new(),
        seen: HashSet::new(),
        cache: HashMap::new(),
        cache_order: BTreeSet::new(),
        root: 0,
        root_sim: Worlds::new(
            start.clone(),
            cfg.start_held,
            cfg.fragile_cost > 0 && cfg.shadows,
            variant_sims,
            &cfg.crabs,
        ),
        variant_crabs,
        lead,
        earlier: Vec::new(),
        stats: SearchStats {
            best_distance: UNREACHABLE,
            ..SearchStats::default()
        },
        began: Instant::now(),
    };
    s.run(start)
}

/// True in the flight of a throw: Claw is in the air, and his last jump
/// was planned higher than a jump of his own rises (a springboard's or a
/// blower's throw).
pub fn thrown(sim: &Sim) -> bool {
    let Some(c) = sim.player() else {
        return false;
    };
    let claw = sim.claw();
    actions::phase(sim) == actions::Phase::Air
        && c.state != state::BEARHUG
        && claw.jump_start_y - claw.jump_peak_y > Reach::default().jump_height
}

/// The position of the elevator or path elevator Claw stands on, if he
/// stands on one.
pub fn ridden(sim: &Sim) -> Option<(i32, i32)> {
    let c = sim.player()?;
    let e = sim.objects().get(c.object_below?)?;
    let lift = path_elevator::NAMES.contains(&&*e.logic)
        || (platform::NAMES.contains(&&*e.logic) && !e.logic.contains("Peg"));
    (lift && on_foot(sim)).then_some((e.x, e.y))
}

/// The bosses whose defeat ends a level, by logic name.
pub fn is_boss(logic: &str) -> bool {
    enemy::raux::NAMES.contains(&logic)
        || enemy::katherine::NAMES.contains(&logic)
        || enemy::wolvington::NAMES.contains(&logic)
        || enemy::gabriel::NAMES.contains(&logic)
        || enemy::marrow::NAMES.contains(&logic)
}

/// The bosses whose fall the search guides into the water: blows cannot
/// hurt them, and Claw lifts them and throws them in (see
/// [`BossEnd::water`]).
pub fn drowns(logic: &str) -> bool {
    enemy::gabriel::NAMES.contains(&logic)
}

/// How far either side of a boss the water he can be thrown into is
/// looked for, px.
pub const WATER_SEARCH: i32 = 3000;

/// Record fields of the boss room's door: where it leads, x and y.
const DOOR_TO: [usize; 2] = [55, 56];

/// Record fields of a boss: where the gem his defeat releases lands, x and
/// y.
const GEM_TO: [usize; 2] = [enemy::field::SPEED_X, enemy::field::SPEED_X + 1];

/// How a boss level ends: through the door into the boss room, past the
/// boss, and at the gem his defeat releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BossEnd {
    /// The door into the boss room.
    pub door: (i32, i32),
    /// Where the door leads.
    pub room: (i32, i32),
    /// Where the boss is placed.
    pub boss: (i32, i32),
    /// Where the gem lands.
    pub gem: (i32, i32),
    /// For a boss the search drowns ([`drowns`]), the x of the nearest
    /// column beside him where nothing below his feet holds a body up
    /// before the level's death tiles.
    pub water: Option<i32>,
}

/// The boss end of the level `sim` plays, when it has a boss room's door
/// and a boss.
pub fn boss_end(sim: &Sim) -> Option<BossEnd> {
    let objects = &sim.level().objects;
    let door = objects.iter().find(|o| o.logic == boss_stage::WARP)?;
    let boss = objects.iter().find(|o| is_boss(&o.logic))?;
    let at = |r: &[i32], f: [usize; 2]| (r[f[0]], r[f[1]]);
    let place = at(&boss.record, [5, 6]);
    Some(BossEnd {
        door: at(&door.record, [5, 6]),
        room: at(&door.record, DOOR_TO),
        boss: place,
        gem: at(&boss.record, GEM_TO),
        water: drowns(&boss.logic)
            .then(|| water_beside(sim.level(), place))
            .flatten(),
    })
}

/// The x of the column nearest `at` (within [`WATER_SEARCH`], either
/// side, in steps of a tile's width) in which a body falling from `at`'s
/// height meets a death tile before any solid or ground tile.
pub fn water_beside(level: &SimLevel, at: (i32, i32)) -> Option<i32> {
    let map = &level.map;
    let falls_in = |x: i32| {
        let mut y = at.1;
        while y < map.height() {
            match map.attr(x, y) {
                DEATH => return true,
                SOLID | GROUND => return false,
                _ => y += WATER_STEP,
            }
        }
        false
    };
    (0..=WATER_SEARCH / WATER_STEP)
        .flat_map(|k| [at.0 - k * WATER_STEP, at.0 + k * WATER_STEP])
        .find(|&x| x >= 0 && x < map.width() && falls_in(x))
}

/// The step of [`water_beside`]'s look, px.
const WATER_STEP: i32 = 16;

/// Where a boss level's route is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BossStage {
    /// On the way to the boss room's door.
    Door,
    /// In the boss room with the boss in play.
    Fight,
    /// The boss is defeated: on to the gem.
    Gem,
}

/// The stage of a boss level `sim` is at.
pub fn boss_stage(sim: &Sim) -> BossStage {
    let b = sim.boss();
    if b.defeated().is_some() {
        BossStage::Gem
    } else if b.current().is_some() || b.fight() != fight::NONE {
        BossStage::Fight
    } else {
        BossStage::Door
    }
}

/// How finely the search tells apart the moments of a floor spike's
/// cycle, ms.
pub const SPIKE_STEP_MS: i64 = 100;

/// How near a toggle peg, across and up or down, Claw must be for a wait
/// for its cycle to count as progress, px: a jump's width and more, so a
/// jump timed to land on it as it slides out is tried at every moment of
/// its cycle.
pub const TOGGLE_NEAR: (u32, u32) = (400, 300);

/// How near a laser, across and up or down, Claw must be for a wait for
/// its beam to stop to count as progress, px: its box and a run up to it.
pub const LASER_NEAR: (u32, u32) = (250, 150);

/// How near a goo vent, across and up or down, Claw must be for a wait
/// beside it to count as progress, px.
pub const VENT_NEAR: (u32, u32) = (200, 150);

/// How far above a ground blower Claw may be and still count as over its
/// nozzle for a wait there, px (standing, he is about 30 px below it).
pub const BLOWER_NEAR_Y: u32 = 120;

/// The distance of the gem stage's start, px: any state of the fight is
/// guided as at least this far from the goal.
pub const FIGHT_BASE: u32 = 4000;

/// What one point of the boss's health counts for in the distance, px.
pub const BOSS_HEALTH_PX: u32 = 40;

/// Most the distance from Claw to the boss counts for, px.
pub const BOSS_NEAR_CAP: u32 = 1000;

/// The boss's most health the guide counts, points.
pub const BOSS_HEALTH_CAP: u32 = 200;

/// Most the distance from a boss to be drowned to the water counts for,
/// px.
pub const WATER_CAP: u32 = 1500;

/// The distance of the fight's start, px: any state on the way to the
/// door is guided as at least this far.
pub const DOOR_BASE: u32 =
    FIGHT_BASE + BOSS_HEALTH_CAP * BOSS_HEALTH_PX + BOSS_NEAR_CAP + WATER_CAP;

/// The boss in play, its health and where it is.
fn boss_in_play(sim: &Sim) -> Option<(i32, i32, i32)> {
    let id = sim.boss().current()?;
    let o = sim.objects().get(id)?;
    Some((o.health, o.x, o.y))
}

/// True while Claw carries the boss in play or the boss flies from his
/// throw: wherever it lands, Claw is as near him as a throw takes him.
pub fn boss_carried(sim: &Sim) -> bool {
    sim.boss()
        .current()
        .and_then(|id| sim.objects().get(id))
        .and_then(|o| EnemyState::from_id(o.state))
        .is_some_and(|s| {
            matches!(
                s,
                EnemyState::Lifted | EnemyState::Flailing | EnemyState::Thrown
            )
        })
}

/// True while the boss in play gets up from a throw and cannot be lifted
/// again yet: the time it has left counts in the coarse state, so a wait
/// for it is progress.
fn boss_guarded(sim: &Sim) -> bool {
    sim.boss()
        .current()
        .and_then(|id| sim.objects().get(id))
        .is_some_and(|o| {
            drowns(&o.logic)
                && o.special_flags & special::LIFTABLE == 0
                && enemy::data_of(o).is_some_and(|d| d.may_lift && !d.dead)
        })
}

/// Marrow's health in one round of his fight: the round ends with his
/// whistle, after [`enemy::marrow::PHASE_DAMAGE`] points.
const MARROW_ROUND_HEALTH: i32 = enemy::marrow::PHASE_DAMAGE;

/// What one point of Marrow's health counts for within a round, px.
pub const MARROW_HEALTH_PX: u32 = 30;

/// Most Claw's distance to Marrow counts for in Marrow's fight, px.
pub const MARROW_NEAR_CAP: u32 = 600;

/// What each hit the parrot still needs before it perches counts for, px.
pub const PARROT_HIT_PX: u32 = 200;

/// Most Claw's distance to the parrot counts for, px.
pub const PARROT_NEAR_CAP: u32 = 200;

/// The hits that send the parrot to its perch.
const PARROT_HITS: u32 = 3;

/// What one round of Marrow's fight counts for, px: more than any state
/// within a round.
pub const MARROW_ROUND_PX: u32 = MARROW_ROUND_HEALTH as u32 * MARROW_HEALTH_PX
    + MARROW_NEAR_CAP
    + PARROT_HITS * PARROT_HIT_PX
    + PARROT_NEAR_CAP
    + 50;

/// How far across from Marrow Claw must be for Marrow to lose sight of
/// him (off the screen), px: waiting for the parrot further away than
/// this is better, up to half of [`PARROT_NEAR_CAP`].
const MARROW_SIGHT_X: u32 = 480;

/// The parrot's coarse place on its patrol, in the search's state: the
/// x shift (128 px stretches of the room).
const PARROT_WAIT_SHIFT: i32 = 7;

/// What the guide of Marrow's fight ([`marrow_guide`]) reads from the
/// game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarrowFight {
    /// Marrow's health.
    pub health: i32,
    /// The health from which he counts [`enemy::marrow::PHASE_DAMAGE`]
    /// before his whistle (0 before the slabs or the parrot first wrote
    /// it).
    pub phase_health: i32,
    /// His round is spent: his whistle is due, he whistles, or the parrot
    /// flies to him or carries him.
    pub spent: bool,
    /// Claw can reach him: the slabs close or are closed, or the two are
    /// on the same side of the pit.
    pub met: bool,
    /// Claw's distance to him, px (across less a sword's reach, plus up or
    /// down).
    pub near_marrow: u32,
    /// The parrot's hits since it last left its perch.
    pub parrot_hits: u32,
    /// Claw's distance to the parrot while it dives at him or flinches,
    /// px (across plus up or down); otherwise how near he waits for its
    /// next dive, [`PARROT_NEAR_CAP`] less up to half of it for waiting
    /// out of Marrow's sight.
    pub near_parrot: u32,
}

/// The guide of Marrow's fight, past [`FIGHT_BASE`].
///
/// The fight runs in rounds of [`MARROW_ROUND_HEALTH`] points of his
/// health. In each, with the pit open between Claw and him, the parrot
/// must be hit three times, so it perches and the slabs close into a
/// bridge; then Claw crosses and hits him until he whistles, and the
/// parrot carries him across while the pit opens again. So a round is
/// guided by the parrot's hits and Claw's distance to it while Claw cannot
/// reach Marrow, then by Marrow's health and Claw's distance to him; a
/// spent round counts as the start of the next one. Each round left
/// counts for more than any state within a round, so every step of the
/// loop is nearer than the one before.
pub fn marrow_guide(f: &MarrowFight) -> u32 {
    let round = MARROW_ROUND_HEALTH;
    let health = f.health.max(0);
    // The health at which this round ends: his whistle, 25 below the phase
    // health once the slabs or the parrot wrote it (before that, as if
    // each round started at a multiple of 25); a spent round counts as the
    // next one, which ends 25 below his health now.
    let end = if f.spent {
        health - round
    } else if f.phase_health > 0 {
        f.phase_health - round
    } else {
        (health - 1).max(0) / round * round
    }
    .max(0);
    // Rounds after this one, and the health left in it.
    let later = (end + round - 1) as u32 / round as u32;
    let left = (health - end).clamp(0, round) as u32;
    let base = later * MARROW_ROUND_PX;
    if f.met && !f.spent {
        return base + left * MARROW_HEALTH_PX + f.near_marrow.min(MARROW_NEAR_CAP);
    }
    // The pit is open between them: the parrot's hits, then its flight
    // home (its hits stay counted until it leaves the perch again).
    let (hits, near) = if f.spent {
        (0, f.near_parrot.min(PARROT_NEAR_CAP))
    } else {
        (
            f.parrot_hits.min(PARROT_HITS),
            f.near_parrot.min(PARROT_NEAR_CAP),
        )
    };
    base + round as u32 * MARROW_HEALTH_PX
        + MARROW_NEAR_CAP
        + (PARROT_HITS - hits) * PARROT_HIT_PX
        + near
}

/// What Marrow's fight in `sim` is at, while his link is up.
fn marrow_fight(sim: &Sim) -> Option<MarrowFight> {
    let link = sim.marrow_link()?;
    let objects = sim.objects();
    let m = objects.get(link.marrow)?;
    let c = sim.player()?;
    let parrot = objects.all().find_map(|(_, o)| match &o.data {
        ObjectData::Parrot(d) => Some((o, d)),
        _ => None,
    });
    let parrot_state = parrot.and_then(|(o, _)| EnemyState::from_id(o.state));
    let health = m.health.max(0);
    let spent = (link.phase_health > 0 && health <= link.phase_health - MARROW_ROUND_HEALTH)
        || EnemyState::from_id(m.state) == Some(EnemyState::WaitForParrot)
        || matches!(
            parrot_state,
            Some(EnemyState::FlyToMarrow | EnemyState::CarryMarrow)
        );
    let side = |x: i32| {
        if x < MARROW_PIT.0 {
            -1
        } else if x > MARROW_PIT.1 {
            1
        } else {
            0
        }
    };
    Some(MarrowFight {
        health,
        phase_health: link.phase_health,
        spent,
        met: !link.open_request || (side(c.x) != 0 && side(c.x) == side(m.x)),
        near_marrow: (m.x - c.x).unsigned_abs().saturating_sub(60) + (m.y - c.y).unsigned_abs(),
        parrot_hits: parrot.map_or(0, |(_, d)| d.hits.max(0) as u32),
        // Only a parrot diving at Claw (or flinching from his hit) comes
        // near: on its patrol it keeps to the top of the room, and Claw
        // waits for its dive (and for its carry of Marrow to end) far
        // from Marrow, out of his sight, but under the parrot's patrol,
        // which it dives from.
        near_parrot: match parrot {
            Some((o, _)) if parrot_dives(o) => {
                (o.x - c.x).unsigned_abs() + (o.y - c.y).unsigned_abs()
            }
            Some((_, d)) if (d.enemy.bounds.0..=d.enemy.bounds.1).contains(&c.x) => {
                PARROT_NEAR_CAP
                    - (m.x - c.x)
                        .unsigned_abs()
                        .saturating_sub(MARROW_SIGHT_X)
                        .min(PARROT_NEAR_CAP / 2)
            }
            _ => PARROT_NEAR_CAP,
        },
    })
}

/// The boss end's guide: the gem's field, besides the door's, which is the
/// goal target.
struct BossGoal {
    end: BossEnd,
    gem: DistanceField,
}

/// Where the level's end object is: the map piece or gem placed in the
/// level.
pub fn end_object(sim: &Sim) -> Option<(i32, i32)> {
    let level = sim.level();
    let set = pickup::map_piece_set(level.number);
    level
        .objects
        .iter()
        .find(|o| o.image_set == set || o.logic == "EndOfLevelPowerup")
        .map(|o| (o.record[5], o.record[6]))
}

struct Target {
    field: DistanceField,
    /// The hint point and its radius; `None` for the goal.
    point: Option<(i32, i32, i32)>,
    /// The hint counts as reached only on foot.
    on_foot: bool,
}

#[derive(Debug, Clone, Copy)]
struct Node {
    parent: Option<u32>,
    action: Option<Action>,
    ticks: u64,
    last: u32,
    /// Route tick of Claw's last landing and of the last press of jump.
    timing: Timing,
    lost: i32,
    /// Health lost along the path in the further game that lost the most
    /// in each move (see [`search_report_with`]).
    variant_lost: i32,
    /// Fragile children along the path from the start.
    fragile: u32,
    /// The shadows started over from this node's game, one bit each.
    reseed: u8,
    h: u32,
    prio: i64,
    tie: u64,
}

/// Route ticks of Claw's last landing and of the last press of jump, and
/// of his first tick in a boss room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Timing {
    landed: Option<u64>,
    jumped: Option<u64>,
    arrived: Option<u64>,
}

/// How a child came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Alive,
    Dead,
    /// A jump was pressed too close to a landing.
    TooClose,
    /// A boss room was entered the other way (see [`SearchConfig::intro`]).
    WrongEntry,
    /// Claw was left on an edge of Marrow's range boxes as he landed (see
    /// [`on_marrow_box_edge`]).
    BoxEdge,
    /// Claw died in a game under further crab values, or the goal was met
    /// while it was not met there (see [`search_report_with`]).
    VariantLost,
    /// The goal was met in the tick of this index within the action.
    Done(usize),
}

struct Child {
    action: Action,
    worlds: Worlds,
    reseed: u8,
    outcome: Outcome,
    lost: i32,
    /// The most health a further game lost in the move.
    variant_lost: i32,
    last: u32,
    timing: Timing,
    fragile: bool,
    checkpoint: bool,
    hint: bool,
}

type OpenEntry = Reverse<(i64, u64, u32)>;

struct Search<'a> {
    cfg: &'a SearchConfig,
    /// The crab values of each further game (see [`search_report_with`]).
    variant_crabs: Vec<CrabValues>,
    goal: Goal,
    /// The boss end the goal follows, on a level that ends with a boss.
    boss: Option<BossGoal>,
    arena: Arena,
    /// The coarse graph the targets' fields were walked on.
    grid: Grid,
    targets: Vec<Target>,
    target: usize,
    nodes: Vec<Node>,
    open: BinaryHeap<OpenEntry>,
    seen: HashSet<u64>,
    cache: HashMap<u32, Worlds>,
    cache_order: BTreeSet<(i64, u64, u32)>,
    root: u32,
    root_sim: Worlds,
    /// Ticks held at [`SearchConfig::start_held`] before the root.
    lead: Vec<u32>,
    /// The segments set aside by each commit, the latest last.
    earlier: Vec<Segment>,
    stats: SearchStats,
    began: Instant,
}

/// A segment of the search set aside when a waypoint started a new one.
struct Segment {
    target: usize,
    open: BinaryHeap<OpenEntry>,
    seen: HashSet<u64>,
    root: u32,
    root_sim: Worlds,
}

impl Search<'_> {
    fn run(&mut self, start: &Sim) -> Result<Found, SearchFailure> {
        let h = self.distance(start);
        let root = Node {
            parent: None,
            action: None,
            ticks: self.lead.len() as u64,
            last: self.cfg.start_held,
            timing: Timing::default(),
            lost: 0,
            variant_lost: 0,
            fragile: 0,
            reseed: 0,
            h,
            prio: 0,
            tie: 0,
        };
        self.nodes.push(root);
        self.seen.insert(state_key(start));
        self.open.push(Reverse((0, 0, 0)));
        loop {
            let Some(Reverse((_, _, id))) = self.open.pop() else {
                // A dead end: back to the segment before, if any.
                if !self.back_out() {
                    break;
                }
                continue;
            };
            if self.stats.expanded >= self.cfg.nodes {
                return Err(self.failure("the node budget ran out"));
            }
            if let Some(limit) = self.cfg.wall_limit
                && self.began.elapsed() > limit
            {
                return Err(self.failure("the wall-time safety stop was reached"));
            }
            self.stats.expanded += 1;
            if self.cfg.log_every > 0 && self.stats.expanded.is_multiple_of(self.cfg.log_every) {
                self.log(id);
            }
            let parent = self.sim_of(id);
            let node = self.nodes[id as usize];
            let choices = if self.boss.is_some() {
                actions::choices_in(&parent.main, &self.arena)
            } else {
                actions::choices(&parent.main)
            };
            let stage = self.boss.as_ref().map(|_| boss_stage(&parent.main));
            let children = self.play_children(&parent, &node, &choices);
            self.keep(id, parent);
            for child in children {
                self.stats.generated += 1;
                match child.outcome {
                    Outcome::Dead => {
                        self.stats.deaths_pruned += 1;
                        continue;
                    }
                    Outcome::TooClose => {
                        self.stats.landings_pruned += 1;
                        continue;
                    }
                    Outcome::WrongEntry => {
                        self.stats.entries_pruned += 1;
                        continue;
                    }
                    Outcome::BoxEdge => {
                        self.stats.box_edges_pruned += 1;
                        continue;
                    }
                    Outcome::VariantLost => {
                        self.stats.variants_pruned += 1;
                        continue;
                    }
                    Outcome::Done(_) => return Ok(self.found(id, &child)),
                    Outcome::Alive => {}
                }
                let ticks = node.ticks + child.action.masks().len() as u64;
                if ticks > self.cfg.max_ticks {
                    continue;
                }
                let key = state_key(&child.worlds.main);
                if !self.seen.insert(key) {
                    self.stats.duplicates += 1;
                    continue;
                }
                let h = self.distance(&child.worlds.main);
                // The search is guided by the game furthest from the target:
                // a further game left behind is steered on too.
                let guide = child
                    .worlds
                    .variants
                    .iter()
                    .map(|v| self.distance(v))
                    .fold(h, u32::max);
                // A further game whose Claw drifts away from the route's
                // own will not play the route's later moves the same way.
                let drift: i64 = child
                    .worlds
                    .variants
                    .iter()
                    .map(|v| apart(&child.worlds.main, v))
                    .sum::<i64>()
                    * self.cfg.drift_cost;
                let lost = node.lost + child.lost;
                let variant_lost = node.variant_lost + child.variant_lost;
                let fragile = node.fragile + u32::from(child.fragile);
                self.stats.fragile += u64::from(child.fragile);
                let prio = i64::from(guide.min(UNREACHABLE / 2))
                    + drift
                    + self.cfg.tick_cost * ticks as i64
                    + self.cfg.health_cost * i64::from(lost + variant_lost)
                    + self.cfg.fragile_cost * i64::from(fragile);
                let cid = self.nodes.len() as u32;
                let tie = mix(self.cfg.seed ^ key);
                self.nodes.push(Node {
                    parent: Some(id),
                    action: Some(child.action),
                    ticks,
                    last: child.last,
                    timing: child.timing,
                    lost,
                    variant_lost,
                    fragile,
                    reseed: child.reseed,
                    h,
                    prio,
                    tie,
                });
                if h < self.stats.best_distance {
                    self.stats.best_distance = h;
                    if let Some(o) = child.worlds.main.player() {
                        self.stats.best_at = (o.x, o.y);
                    }
                }
                let ahead = self.nodes[self.root as usize].h.saturating_sub(h);
                let hint = child.hint && self.targets[self.target].point.is_some();
                // A boss level's next stage starts a fresh segment.
                let staged = stage.is_some_and(|s| boss_stage(&child.worlds.main) > s);
                // A segment starts only where the further games are with
                // the route's own: a root they strayed from would leave them
                // to follow moves made for another place.
                let together = child
                    .worlds
                    .variants
                    .iter()
                    .all(|v| apart(&child.worlds.main, v) <= TOGETHER_PX);
                if together
                    && (hint
                        || staged
                        || (child.checkpoint && ahead >= self.cfg.checkpoint_progress))
                {
                    self.commit(cid, child.worlds, hint);
                    break;
                }
                self.open.push(Reverse((prio, tie, cid)));
                self.cache_put(cid, child.worlds);
            }
        }
        Err(self.failure("every reachable state was searched"))
    }

    /// Makes `id` the root of a fresh segment; with `hint`, the next
    /// target becomes the current one. The segment it ends is set aside
    /// (see [`Search::back_out`]).
    fn commit(&mut self, id: u32, worlds: Worlds, hint: bool) {
        self.stats.segments += 1;
        let earlier = Segment {
            target: self.target,
            open: std::mem::take(&mut self.open),
            seen: std::mem::take(&mut self.seen),
            root: self.root,
            root_sim: self.root_sim.clone(),
        };
        self.earlier.push(earlier);
        if hint && self.target + 1 < self.targets.len() {
            self.target += 1;
        }
        self.cache.clear();
        self.cache_order.clear();
        self.seen.insert(state_key(&worlds.main));
        let h = self.distance(&worlds.main);
        let n = &mut self.nodes[id as usize];
        n.h = h;
        self.root = id;
        self.root_sim = worlds;
        self.open.push(Reverse((0, 0, id)));
        if self.cfg.log_every > 0 {
            let (x, y) = self.root_sim.main.player().map_or((0, 0), |o| (o.x, o.y));
            eprintln!(
                "claw-bot: segment {} at ({x}, {y}), tick {}, {h} px to go, {} nodes",
                self.stats.segments, self.nodes[id as usize].ticks, self.stats.expanded
            );
        }
    }

    /// Goes back to the segment set aside by the last commit, whose root
    /// led nowhere; false when there is none. That root stays in the
    /// segment's seen states, so it is never taken again.
    fn back_out(&mut self) -> bool {
        let Some(seg) = self.earlier.pop() else {
            return false;
        };
        self.stats.backed_out += 1;
        self.target = seg.target;
        self.open = seg.open;
        self.seen = seg.seen;
        self.root = seg.root;
        self.root_sim = seg.root_sim;
        self.cache.clear();
        self.cache_order.clear();
        if self.cfg.log_every > 0 {
            eprintln!(
                "claw-bot: segment {} was a dead end, back to the one before, {} nodes",
                self.earlier.len() + 1,
                self.stats.expanded
            );
        }
        true
    }

    fn distance(&self, sim: &Sim) -> u32 {
        let Some(o) = sim.player() else {
            return UNREACHABLE;
        };
        let feet = o.y + o.move_rect.bottom - 2;
        let target = &self.targets[self.target];
        let mut here = target.field.at(o.x, feet);
        // In the flight of a throw, the height Claw has over the
        // springboard or blower that threw him is a way on he did not have
        // to climb: it takes off from the distance, so the rise does not
        // count against the throw, and steering toward the target does.
        if thrown(sim)
            && here != UNREACHABLE
            && let Some((_, base)) = self.grid.throw_base(o.x, feet)
        {
            here = here.saturating_sub(base.saturating_sub(feet).max(0) as u32);
        }
        // Riding an elevator or a path elevator, Claw is as near as a
        // place of its way and the ride there take him: waiting on it is
        // the way on, though it carries him away from the target for a
        // while.
        if let Some((ex, ey)) = ridden(sim)
            && let Some((ride, at)) = self.grid.ride_of(ex, ey)
        {
            for (k, &(px, py)) in ride.points.iter().enumerate() {
                let there = target.field.at(px + o.x - ex, py + feet - ey);
                here = here.min(there.saturating_add(ride.travel(at, k)));
            }
        }
        let Some(boss) = self.boss.as_ref().filter(|_| target.point.is_none()) else {
            return here;
        };
        match boss_stage(sim) {
            BossStage::Door => DOOR_BASE.saturating_add(here),
            BossStage::Fight => {
                if let Some(f) = marrow_fight(sim) {
                    return FIGHT_BASE + marrow_guide(&f);
                }
                let (health, bx, by) = boss_in_play(sim).unwrap_or((
                    BOSS_HEALTH_CAP as i32,
                    boss.end.boss.0,
                    boss.end.boss.1,
                ));
                let health = (health.max(0) as u32).min(BOSS_HEALTH_CAP);
                // Near enough to strike counts as there, and so does the
                // boss in Claw's hands or in the flight of his throw.
                let near = if boss_carried(sim) {
                    0
                } else {
                    ((bx - o.x).unsigned_abs().saturating_sub(60) + (by - o.y).unsigned_abs())
                        .min(BOSS_NEAR_CAP)
                };
                // A boss to be drowned is as far as he is from the water.
                let wet = boss
                    .end
                    .water
                    .map_or(0, |wx| (bx - wx).unsigned_abs().min(WATER_CAP));
                FIGHT_BASE + health * BOSS_HEALTH_PX + near + wet
            }
            BossStage::Gem => boss.gem.at(o.x, feet).min(FIGHT_BASE - 1),
        }
    }

    /// The games of node `id`, from the cache or rebuilt from its nearest
    /// ancestor that has them.
    fn sim_of(&mut self, id: u32) -> Worlds {
        if id == self.root {
            return self.root_sim.clone();
        }
        if let Some(w) = self.cache.remove(&id) {
            let n = self.nodes[id as usize];
            self.cache_order.remove(&(n.prio, n.tie, id));
            return w;
        }
        self.stats.rebuilt += 1;
        let mut path = Vec::new();
        let mut cur = id;
        while cur != self.root && !self.cache.contains_key(&cur) {
            path.push(cur);
            cur = self.nodes[cur as usize]
                .parent
                .expect("a non-root node has a parent");
        }
        let mut w = if cur == self.root {
            self.root_sim.clone()
        } else {
            self.cache[&cur].clone()
        };
        let mut prev = self.nodes[cur as usize].last;
        let values = self.values();
        for n in path.into_iter().rev() {
            let node = self.nodes[n as usize];
            for held in node.action.expect("a non-root node has an action").masks() {
                w.step(input(held, prev), values);
                prev = held;
            }
            w.reseed(node.reseed, prev, values.main);
        }
        w
    }

    /// The crab values the games play under.
    fn values(&self) -> Values<'_> {
        Values {
            main: &self.cfg.crabs,
            variants: &self.variant_crabs,
        }
    }

    /// Keeps the game of an expanded node, as a base for rebuilding its
    /// descendants.
    fn keep(&mut self, id: u32, worlds: Worlds) {
        if id != self.root {
            self.cache_put(id, worlds);
        }
    }

    fn cache_put(&mut self, id: u32, worlds: Worlds) {
        let n = self.nodes[id as usize];
        // Each node keeps its shadows and further games too: the budget
        // counts games.
        let games = worlds.games();
        self.cache.insert(id, worlds);
        self.cache_order.insert((n.prio, n.tie, id));
        while self.cache.len() > (self.cfg.cached_sims / games).max(1) {
            let Some(worst) = self.cache_order.pop_last() else {
                break;
            };
            self.cache.remove(&worst.2);
        }
    }

    fn play_children(&self, parent: &Worlds, node: &Node, choices: &[Action]) -> Vec<Child> {
        let threads = self.cfg.threads.max(1).min(choices.len());
        let from = From {
            last: node.last,
            tick: node.ticks,
            timing: node.timing,
            land_gap: self.cfg.land_gap,
            hint_on_foot: self.targets[self.target].on_foot,
            check_fragile: self.cfg.fragile_cost > 0,
            intro: if self.boss.is_some() {
                self.cfg.intro
            } else {
                Intro::Any
            },
        };
        let goal = &self.goal;
        let point = self.targets[self.target].point;
        let values = self.values();
        if threads <= 1 {
            return choices
                .iter()
                .map(|&a| play(parent, from, a, goal, point, values))
                .collect();
        }
        let mut slots: Vec<Option<Child>> = (0..choices.len()).map(|_| None).collect();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    scope.spawn(move || {
                        choices
                            .iter()
                            .enumerate()
                            .skip(t)
                            .step_by(threads)
                            .map(|(i, &a)| (i, play(parent, from, a, goal, point, values)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for h in handles {
                for (i, c) in h.join().expect("a search thread panicked") {
                    slots[i] = Some(c);
                }
            }
        });
        slots
            .into_iter()
            .map(|c| c.expect("every child played"))
            .collect()
    }

    fn found(&mut self, parent: u32, child: &Child) -> Found {
        let mut chain = vec![child.action];
        let mut cur = parent;
        while let Some(p) = self.nodes[cur as usize].parent {
            chain.push(self.nodes[cur as usize].action.expect("action"));
            cur = p;
        }
        let mut held = self.lead.clone();
        for a in chain.iter().rev() {
            held.extend(a.masks());
        }
        let before = self.nodes[parent as usize].ticks;
        let done_at = match child.outcome {
            Outcome::Done(i) => before + i as u64,
            _ => held.len() as u64 - 1,
        };
        self.stats.ticks = held.len() as u64;
        self.stats.health_lost = self.nodes[parent as usize].lost + child.lost;
        self.stats.fragile_on_route =
            self.nodes[parent as usize].fragile + u32::from(child.fragile);
        let mut prev = self.cfg.start_held;
        let ticks = held
            .iter()
            .map(|&h| {
                let t = input(h, prev);
                prev = h;
                t
            })
            .collect();
        Found {
            script: InputScript {
                ticks,
                ..InputScript::default()
            },
            stats: self.stats.clone(),
            done_at,
        }
    }

    fn failure(&self, reason: &str) -> SearchFailure {
        SearchFailure {
            reason: reason.to_string(),
            stats: Box::new(self.stats.clone()),
        }
    }

    fn log(&self, id: u32) {
        let n = self.nodes[id as usize];
        eprintln!(
            "claw-bot: {} nodes, {} open, {} kept, {} rebuilt, {} lost in further games, node at tick {} with {} px to go, closest {} px at {:?}, {:.0} s",
            self.stats.expanded,
            self.open.len(),
            self.cache.len(),
            self.stats.rebuilt,
            self.stats.variants_pruned,
            n.ticks,
            n.h,
            self.stats.best_distance,
            self.stats.best_at,
            self.began.elapsed().as_secs_f64()
        );
    }
}

fn input(held: u32, prev: u32) -> TickInput {
    TickInput {
        held: Buttons(held),
        pressed: Buttons(held & !prev),
    }
}

fn near(sim: &Sim, x: i32, y: i32, r: i32) -> bool {
    sim.player().is_some_and(|o| {
        let (dx, dy) = (i64::from(o.x - x), i64::from(o.y - y));
        dx * dx + dy * dy <= i64::from(r) * i64::from(r)
    })
}

/// How far Claw is kept from an edge of Marrow's range boxes as he
/// lands, px (see [`on_marrow_box_edge`]).
pub const MARROW_BOX_CLEAR: i64 = 8;

/// True when Marrow has just been dropped onto a ledge (his range boxes
/// still at the height he hung from until his next movement step) and
/// Claw stands within [`MARROW_BOX_CLEAR`] px of an edge of one of his
/// range boxes, built from that height or from where he landed: the
/// step after the landing tests Claw against those boxes before Claw
/// moves, and a pixel either way there decides whether Marrow shoots or
/// runs at him.
pub fn on_marrow_box_edge(sim: &Sim) -> bool {
    let Some(link) = sim.marrow_link() else {
        return false;
    };
    let Some(m) = sim.objects().get(link.marrow) else {
        return false;
    };
    let Some(d) = enemy::data_of(m) else {
        return false;
    };
    let Some(kept) = d.range_from else {
        return false;
    };
    let Some(c) = sim.player() else {
        return false;
    };
    let claw = (i64::from(c.x), i64::from(c.y));
    let landed = (f64::from(m.x), f64::from(m.y));
    [kept, landed].into_iter().any(|centre| {
        (0..5).any(|i| near_box_edge(enemy::range_box(d, i, centre), claw, MARROW_BOX_CLEAR))
    })
}

/// True when `point` lies within `margin` of an edge of `rect` (left,
/// top, right, bottom; the right and bottom edges outside it): on either
/// side of the edge, along the edge's length widened by `margin`.
pub fn near_box_edge(
    (left, top, right, bottom): (i64, i64, i64, i64),
    (x, y): (i64, i64),
    margin: i64,
) -> bool {
    let across = x > left - margin && x < right + margin;
    let down = y > top - margin && y < bottom + margin;
    let near_x = (x - left).abs() < margin || (x - right).abs() < margin;
    let near_y = (y - top).abs() < margin || (y - bottom).abs() < margin;
    (near_x && down) || (near_y && across)
}

/// True when Claw is dying or dead.
pub fn dying(sim: &Sim) -> bool {
    sim.player()
        .is_none_or(|o| matches!(o.state, state::DEATH | state::DEATH_FALL))
        || sim.stats().health <= 0
}

/// Where a child starts: the parent's last held mask, the route tick of
/// its first tick, its landing and jump timing, and the gap kept between
/// them.
#[derive(Debug, Clone, Copy)]
struct From {
    last: u32,
    tick: u64,
    timing: Timing,
    land_gap: u64,
    hint_on_foot: bool,
    check_fragile: bool,
    intro: Intro,
}

/// True when route ticks `a` and `b` are at most `gap` ticks apart (the
/// rule keeps a press and a landing more than `gap` ticks apart); never
/// with a gap of 0.
fn within(a: Option<u64>, b: u64, gap: u64) -> bool {
    gap > 0 && a.is_some_and(|a| a.abs_diff(b) <= gap)
}

/// True when Claw is on his feet, on a ladder or on a rope.
pub fn on_foot(sim: &Sim) -> bool {
    matches!(
        actions::phase(sim),
        actions::Phase::Ground | actions::Phase::Climb | actions::Phase::Swing
    )
}

/// Plays `action` on a copy of the games of `parent`, which start as
/// `from` says.
fn play(
    parents: &Worlds,
    from: From,
    action: Action,
    goal: &Goal,
    hint: Option<(i32, i32, i32)>,
    values: Values,
) -> Child {
    let parent = &parents.main;
    let mut sim = parent.clone();
    let lives = sim.stats().lives;
    let mut prev = from.last;
    let mut timing = from.timing;
    let mut lost = 0;
    let mut checkpoint = false;
    let mut reached = false;
    let mut outcome = Outcome::Alive;
    for (i, held) in action.masks().into_iter().enumerate() {
        let tick = from.tick + i as u64;
        let health = sim.stats().health;
        let airborne = actions::phase(&sim) == actions::Phase::Air;
        let jump = held & !prev & actions::JUMP != 0;
        step(&mut sim, input(held, prev), values.main);
        prev = held;
        if jump {
            if within(timing.landed, tick, from.land_gap) {
                outcome = Outcome::TooClose;
                break;
            }
            timing.jumped = Some(tick);
        }
        if airborne && actions::phase(&sim) != actions::Phase::Air {
            if within(timing.jumped, tick, from.land_gap) {
                outcome = Outcome::TooClose;
                break;
            }
            timing.landed = Some(tick);
        }
        lost += (health - sim.stats().health).max(0);
        let mut died = false;
        let mut done = false;
        for e in sim.last_events() {
            match e {
                GameEvent::LevelComplete { .. } => done = *goal == Goal::LevelComplete,
                GameEvent::ClawDied { .. } | GameEvent::GameOver => died = true,
                GameEvent::Checkpoint { .. } | GameEvent::SavePoint { .. } => checkpoint = true,
                _ => {}
            }
        }
        done |= goal.met_at(&sim);
        if let Some((x, y, r)) = hint {
            reached |= if from.hint_on_foot {
                near(&sim, x, y, 2 * r) && on_foot(&sim)
            } else {
                near(&sim, x, y, r)
            };
        }
        if died || dying(&sim) || sim.stats().lives < lives {
            outcome = Outcome::Dead;
            break;
        }
        if on_marrow_box_edge(&sim) {
            outcome = Outcome::BoxEdge;
            break;
        }
        if from.intro != Intro::Any {
            let boss = sim.boss();
            if timing.arrived.is_none() && boss.current().is_some() {
                timing.arrived = Some(tick);
            }
            let wrong = match from.intro {
                Intro::Skip => boss.fight() != fight::NONE,
                Intro::Play => {
                    boss.fight() == fight::NONE
                        && boss.defeated().is_none()
                        && timing.arrived.is_some_and(|a| tick >= a + INTRO_GRACE)
                }
                Intro::Any => false,
            };
            if wrong {
                outcome = Outcome::WrongEntry;
                break;
            }
        }
        if done {
            outcome = Outcome::Done(i);
            break;
        }
    }
    let mut variants = Vec::with_capacity(parents.variants.len());
    let mut variant_lost = 0;
    if matches!(outcome, Outcome::Alive | Outcome::Done(_)) {
        for (v, crabs) in parents.variants.iter().zip(values.variants) {
            match played_under(v, from.last, &action.masks(), goal, crabs) {
                Some((s, done, lost)) if done || outcome == Outcome::Alive => {
                    variant_lost = variant_lost.max(lost);
                    variants.push(s);
                }
                _ => {
                    outcome = Outcome::VariantLost;
                    break;
                }
            }
        }
    }
    let mut shadows = Vec::with_capacity(parents.shadows.len());
    let mut reseed = 0u8;
    if outcome == Outcome::Alive {
        for (i, (shadow, kind)) in parents.shadows.iter().zip(SHADOWS).enumerate() {
            match played_on(shadow, from.last, &action.masks(), &sim, values.main) {
                Some(s) => shadows.push(s),
                None => {
                    reseed |= 1 << i;
                    shadows.push(kind.start(&sim, prev, values.main));
                }
            }
        }
    }
    let fragile = from.check_fragile
        && outcome == Outcome::Alive
        && (reseed != 0
            || shifted(action, from.last)
                .iter()
                .any(|masks| !holds_up(parent, from.last, masks, &sim, values.main))
            || (1..=2).any(|draws| {
                let mut other = parent.clone();
                let mut r = claw_sim::rng::Rng::new(other.rng_state());
                for _ in 0..draws {
                    r.next_u15();
                }
                other.set_rng_state(r.state());
                !holds_up(&other, from.last, &action.masks(), &sim, values.main)
            }));
    Child {
        action,
        worlds: Worlds {
            main: sim,
            shadows,
            variants,
        },
        reseed,
        outcome,
        lost,
        variant_lost,
        last: prev,
        timing,
        fragile,
        checkpoint,
        hint: reached,
    }
}

/// The held masks of `action` with its timing shifted by a tick, after a
/// tick that held `last`: the action starting a tick late (the mask before
/// it kept one tick more), and each change inside it a tick early and a
/// tick late. Every variant is as long as the action.
fn shifted(action: Action, last: u32) -> Vec<Vec<u32>> {
    let m = action.masks();
    let mut out = Vec::new();
    if m.first().is_some_and(|&f| f != last) {
        let mut v = vec![last];
        v.extend_from_slice(&m[1..]);
        out.push(v);
    }
    for c in 1..m.len() {
        if m[c] != m[c - 1] {
            let mut early = m.clone();
            early[c - 1] = m[c];
            out.push(early);
            let mut late = m.clone();
            late[c] = m[c - 1];
            out.push(late);
        }
    }
    out
}

/// True when `masks` played from `parent` (whose last mask was `last`)
/// leave Claw alive, with no less health, and near where `nominal` has
/// him: in the same phase, within [`FRAGILE_DY`] up or down (at any
/// height when both are in the flight of a throw) and [`FRAGILE_PX`]
/// across.
fn holds_up(parent: &Sim, last: u32, masks: &[u32], nominal: &Sim, crabs: &CrabValues) -> bool {
    played_on(parent, last, masks, nominal, crabs).is_some()
}

/// `masks` played in the game `parent` under further crab values `crabs`
/// (whose last mask was `last`): the game after them, whether it came as
/// far as `goal` asks and the health Claw lost, or `None` when he died. A level complete goal
/// is met when the level was won (in these masks or before them; play
/// stops there); a goal point when Claw is within twice its distance of
/// it at some tick (on his feet when the goal asks it), so a game that
/// strays a little from the route's own still counts.
fn played_under(
    parent: &Sim,
    last: u32,
    masks: &[u32],
    goal: &Goal,
    crabs: &CrabValues,
) -> Option<(Sim, bool, i32)> {
    let mut sim = parent.clone();
    let lives = sim.stats().lives;
    let mut lost = 0;
    let mut prev = last;
    // A game whose level ended in an earlier move stays won.
    let mut done = (*goal == Goal::LevelComplete && sim.level_ended()) || near_goal(&sim, goal);
    for &held in masks {
        if done {
            break;
        }
        let health = sim.stats().health;
        step(&mut sim, input(held, prev), crabs);
        lost += (health - sim.stats().health).max(0);
        prev = held;
        for e in sim.last_events() {
            match e {
                GameEvent::LevelComplete { .. } => done |= *goal == Goal::LevelComplete,
                GameEvent::ClawDied { .. } | GameEvent::GameOver => return None,
                _ => {}
            }
        }
        if dying(&sim) || sim.stats().lives < lives {
            return None;
        }
        done |= near_goal(&sim, goal);
    }
    Some((sim, done, lost))
}

/// How far Claw in a game under further crab values may be from the
/// route's own Claw (across plus up or down) for a node to become a
/// segment root, px.
pub const TOGETHER_PX: i64 = 2 * FRAGILE_PX as i64;

/// How far apart Claw is in two games, across plus up or down, px (a
/// large number when one has no Claw).
fn apart(a: &Sim, b: &Sim) -> i64 {
    match (a.player(), b.player()) {
        (Some(p), Some(q)) => i64::from((p.x - q.x).abs() + (p.y - q.y).abs()),
        _ => i64::from(UNREACHABLE / 2),
    }
}

/// True when Claw is within twice a goal point's distance of it, and on
/// his feet, a ladder or a rope there when the goal asks it (never for the
/// level complete goal).
fn near_goal(sim: &Sim, goal: &Goal) -> bool {
    match *goal {
        Goal::LevelComplete => false,
        Goal::Reach { x, y, radius } => near(sim, x, y, 2 * radius),
        Goal::Stand { x, y, radius } => near(sim, x, y, 2 * radius) && on_foot(sim),
    }
}

/// `masks` played from `parent` as [`holds_up`] plays them: the game
/// after them when it holds up, else `None`.
fn played_on(
    parent: &Sim,
    last: u32,
    masks: &[u32],
    nominal: &Sim,
    crabs: &CrabValues,
) -> Option<Sim> {
    let mut sim = parent.clone();
    let lives = sim.stats().lives;
    let mut prev = last;
    for &held in masks {
        step(&mut sim, input(held, prev), crabs);
        prev = held;
        if dying(&sim) || sim.stats().lives < lives {
            return None;
        }
    }
    if sim.stats().health < nominal.stats().health {
        return None;
    }
    // Both in the flight of a throw, a tick early or late leaves Claw
    // at another height on the same throw (he rises and falls fast in
    // it): only the distance across counts.
    let in_throw = thrown(&sim) && thrown(nominal);
    let near = match (sim.player(), nominal.player()) {
        (Some(a), Some(b)) => {
            actions::phase(&sim) == actions::phase(nominal)
                && (in_throw || (a.y - b.y).abs() <= FRAGILE_DY)
                && (a.x - b.x).abs() <= FRAGILE_PX
        }
        _ => false,
    };
    near.then_some(sim)
}

/// The coarse state of a game the search treats as one: Claw's position to
/// 8 px, his state, facing, running and attacking, his health to 10
/// points, and the platforms (position to 16 px and state) and enemies
/// (alive or not) within about a screen of him, and whether each piece of
/// a breaking plank row near him is whole, waiting or crumbling. Springboards and stepping
/// stones count with where they are in their cycles (state and frame, the
/// time while a springboard runs the spring Claw set off, and
/// a stone that keeps to the clock its time in its cycle to a tenth of a
/// second), so a wait on a springboard for its throw, or beside a stone
/// for it to come up, is progress.
///
/// The flight of a throw counts apart from an ordinary jump or fall
/// through the same place.
///
/// The key is a fixed hash of those values (64-bit FNV-1a), so it is the
/// same with every toolchain and on every machine, and so are the routes
/// the search finds.
pub fn state_key(sim: &Sim) -> u64 {
    let mut k = Fold::default();
    let Some(c) = sim.player() else {
        return 0;
    };
    let claw = sim.claw();
    k.add(&[i64::from(c.x >> 3), i64::from(c.y >> 3), i64::from(c.state)]);
    k.add(&[
        i64::from(claw.facing_right),
        i64::from(claw.attacking),
        i64::from(claw.running_time == 0),
    ]);
    k.add(&[
        i64::from(sim.stats().health / 10),
        i64::from(sim.stats().lives),
    ]);
    // In the flight of a throw (a springboard's or a blower's), Claw is
    // in another state than an ordinary jump or fall through the same
    // place: he goes far higher, or comes down far faster.
    if thrown(sim) {
        k.add(&[12]);
    }
    // Struck, or in another state that takes no steering, Claw waits for
    // it to end: a wait through it is progress, so the time tells the
    // states apart (they end soon, so this never grows far). A Siren's
    // scream that has taken his controls is such a state too, whatever
    // he is doing when it touches him.
    if actions::phase(sim) == actions::Phase::Busy || actions::locked(sim) {
        k.add(&[
            6,
            i64::from(c.frame),
            (sim.tick() / actions::STEP as u64) as i64,
        ]);
    }
    let objects = sim.objects();
    let clock = sim.tick() as i64 * i64::from(claw_sim::tuning::TICK_MS);
    for id in objects.listed() {
        let Some(o) = objects.get(id) else { continue };
        if o.x.abs_diff(c.x) > 700 || o.y.abs_diff(c.y) > 500 {
            continue;
        }
        if let ObjectData::SpringBoard(d) = &o.data {
            // A wait on a springboard for its throw is progress: where it
            // is in its cycle and, while a spring Claw set off runs (a
            // picture of it may last longer than a wait), the time.
            let into = if o.state == springboard::state::CLAW_SPRING {
                (sim.tick() / actions::STEP as u64) as i64
            } else {
                0
            };
            k.add(&[
                7,
                i64::from(id),
                i64::from(o.state),
                i64::from(o.frame),
                i64::from(d.armed),
                into,
            ]);
        } else if let ObjectData::SteppingStone(d) = &o.data {
            // A wait for a stone to come up is progress: where it is in
            // its sink and rise and, for a stone that keeps to the clock,
            // in its cycle, to a tenth of a second.
            let into = if d.period > 0 {
                (clock - i64::from(d.phase)).rem_euclid(i64::from(d.period)) / SPIKE_STEP_MS
            } else {
                0
            };
            k.add(&[
                8,
                i64::from(id),
                i64::from(o.state),
                i64::from(o.frame),
                into,
            ]);
        } else if let ObjectData::BreakPlank(d) = &o.data
            && d.pieces.is_empty()
        {
            // A piece of a breaking plank row: whole, waiting to give way,
            // or crumbling and gone decides whether Claw can stand there,
            // so Claw back where he stood with the row broken behind him
            // is another state.
            k.add(&[9, i64::from(id), i64::from(o.state)]);
        } else if o.type_flags & platform::PLATFORM != 0
            || matches!(&o.data, ObjectData::Platform(d) if d.period > 0)
        {
            // A wait near a toggle peg for the moment of its cycle that a
            // jump onto it needs is progress: where it is in its cycle, to
            // a tenth of a second (its picture alone stays the same for
            // most of a second, out or in the wall). A peg in the wall is
            // no platform until it slides out again, but counts here all
            // the same.
            let into = match &o.data {
                ObjectData::Platform(d)
                    if d.period > 0
                        && o.x.abs_diff(c.x) <= TOGGLE_NEAR.0
                        && o.y.abs_diff(c.y) <= TOGGLE_NEAR.1 =>
                {
                    (clock - i64::from(d.phase)).rem_euclid(i64::from(d.period)) / SPIKE_STEP_MS
                }
                _ => 0,
            };
            k.add(&[
                1,
                i64::from(id),
                i64::from(o.x >> 4),
                i64::from(o.y >> 4),
                i64::from(o.state),
                into,
            ]);
        } else if let ObjectData::Parrot(d) = &o.data {
            // Marrow's parrot: its hits decide when the bridge closes. On
            // its patrol or its perch only the stretch of the room it is
            // over tells a wait for its dive apart (finer, every way Claw
            // could walk about while it flies would be a state of its
            // own); diving at him, flinching or carrying, where it is.
            let (x, y) = if parrot_waits(o) {
                (o.x >> PARROT_WAIT_SHIFT, 0)
            } else {
                (o.x >> 4, o.y >> 4)
            };
            k.add(&[
                14,
                i64::from(id),
                i64::from(d.hits),
                i64::from(o.state),
                i64::from(x),
                i64::from(y),
            ]);
        } else if let Some(d) = enemy::data_of(o) {
            // An enemy's health too: a shot or a blow that hurts without
            // killing is progress, so an enemy that takes several (one
            // that parries some of them, as the eleventh level's Mercat
            // does Claw's pistol shots) can be worn down from where Claw
            // stands.
            k.add(&[2, i64::from(id), i64::from(d.dead), i64::from(o.health)]);
        } else if let ObjectData::Rope(r) = &o.data {
            // Where the rope is in its swing decides where a grab or a
            // release takes Claw.
            k.add(&[4, i64::from(id), i64::from(r.frame / 2)]);
        } else if let ObjectData::GroundBlower(_) = &o.data {
            // A wait over a blower's nozzle for its throw is progress: the
            // jet's picture and, since the last picture lasts a long while,
            // the time.
            if o.x.abs_diff(c.x) <= BLOWER_HALF_WIDTH.unsigned_abs()
                && o.y.abs_diff(c.y) <= BLOWER_NEAR_Y
            {
                k.add(&[
                    10,
                    i64::from(id),
                    i64::from(o.frame),
                    (sim.tick() / actions::STEP as u64) as i64,
                ]);
            }
        } else if let ObjectData::GooVent(_) = &o.data {
            // A wait beside a goo vent for its eruption to end is progress:
            // whether it is dangerous, its picture and the time.
            if o.x.abs_diff(c.x) <= VENT_NEAR.0 && o.y.abs_diff(c.y) <= VENT_NEAR.1 {
                k.add(&[
                    11,
                    i64::from(id),
                    i64::from(goo_vent::dangerous(o)),
                    i64::from(o.frame),
                    (sim.tick() / actions::STEP as u64) as i64,
                ]);
            }
        } else if let ObjectData::Laser(_) = &o.data {
            // A wait beside a laser for its beam to stop is progress, as
            // beside a goo vent: whether it hurts, its picture and the
            // time.
            if o.x.abs_diff(c.x) <= LASER_NEAR.0 && o.y.abs_diff(c.y) <= LASER_NEAR.1 {
                k.add(&[
                    17,
                    i64::from(id),
                    i64::from(laser::dangerous(o)),
                    i64::from(o.frame),
                    (sim.tick() / actions::STEP as u64) as i64,
                ]);
            }
        } else if let ObjectData::FloorSpike(d) = &o.data {
            // A wait for a spike to sink is progress: where the spike is
            // in its cycle, to a tenth of a second.
            let into = (clock - i64::from(d.phase)).rem_euclid(i64::from(d.period.max(1)));
            k.add(&[
                5,
                i64::from(id),
                i64::from(floor_spike::dangerous(o)),
                into / SPIKE_STEP_MS,
            ]);
        }
    }
    // In a boss fight: the flag, the boss's health, place and state, and,
    // while Claw is held (the introduction, the gem's flight), the time.
    let boss = sim.boss();
    if boss.fight() != fight::NONE || boss.current().is_some() || boss.defeated().is_some() {
        k.add(&[
            3,
            i64::from(boss.fight()),
            i64::from(boss.defeated().is_some()),
        ]);
        if let Some((health, x, y)) = boss_in_play(sim) {
            let st = boss
                .current()
                .and_then(|id| objects.get(id))
                .map_or(0, |o| o.state);
            k.add(&[
                i64::from(health),
                i64::from(x >> 3),
                i64::from(y >> 3),
                i64::from(st),
            ]);
        }
        if let Some(l) = sim.marrow_link() {
            k.add(&[
                15,
                i64::from(l.open_request),
                i64::from(l.parrot_call),
                i64::from(l.phase_health),
            ]);
            // Waiting for the parrot's dive (the only move then), the time
            // tells the waits apart: a wait is progress.
            if actions::parrot_patrols(sim) {
                k.add(&[16, (sim.tick() / actions::STEP as u64) as i64]);
            }
        }
        if matches!(c.state, state::FREEZE | BOSS_END) {
            k.add(&[4, sim.tick() as i64]);
        }
        // Getting up from a throw, the boss cannot be lifted again for a
        // while: the time tells a wait for it apart.
        if boss_guarded(sim) {
            k.add(&[13, (sim.tick() / actions::STEP as u64) as i64]);
        }
    }
    k.0
}

/// A running 64-bit FNV-1a hash of integers, each taken as its eight
/// little-endian bytes.
struct Fold(u64);

impl Default for Fold {
    fn default() -> Fold {
        Fold(0xcbf2_9ce4_8422_2325)
    }
}

impl Fold {
    fn add(&mut self, values: &[i64]) {
        for &v in values {
            for b in v.to_le_bytes() {
                self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
}

/// A 64-bit mix of `v` (splitmix64's finaliser).
fn mix(v: u64) -> u64 {
    let mut z = v.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
