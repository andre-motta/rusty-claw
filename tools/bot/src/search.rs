//! Best-first search over macro-actions in the headless simulation.
//!
//! Each node is the game after a chain of [`Action`]s from the start. The
//! search expands the node with the lowest priority: the path distance from
//! Claw to the current target on the coarse graph ([`crate::field`]), plus
//! a small cost per tick and a larger one per point of health lost. A child
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
//! The search is deterministic. Its budget counts expanded nodes, never
//! time; ties go by a hash of the seed and the node's state; and the
//! children of a node may be played on several threads, but they are
//! gathered in the order of the actions, so the thread count never changes
//! the result.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};
use std::time::{Duration, Instant};

use claw_sim::claw_frame::GameEvent;
use claw_sim::logic::{enemy, pickup, platform};
use claw_sim::player::state;
use claw_sim::{Buttons, InputScript, Sim, TickInput};

use crate::actions::{self, Action, MIN_SPACING};
use crate::field::{DistanceField, Grid, Reach, UNREACHABLE};

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
}

impl Default for SearchConfig {
    fn default() -> SearchConfig {
        SearchConfig {
            nodes: 200_000,
            seed: 1,
            hints: Vec::new(),
            hint_radius: 48,
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
    fn start(self, main: &Sim, last: u32) -> Sim {
        let mut sim = main.clone();
        match self {
            Shadow::Ahead => {
                sim.step(input(last, last));
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

/// The games of a node: the route as found and its shadows (none when the
/// fragility check is off).
#[derive(Debug, Clone)]
struct Worlds {
    main: Sim,
    shadows: Vec<Sim>,
}

impl Worlds {
    /// `main` with fresh shadows when `shadows` is set.
    fn new(main: Sim, last: u32, shadows: bool) -> Worlds {
        let shadows = if shadows {
            SHADOWS.iter().map(|s| s.start(&main, last)).collect()
        } else {
            Vec::new()
        };
        Worlds { main, shadows }
    }

    fn step(&mut self, input: TickInput) {
        self.main.step(input);
        for s in &mut self.shadows {
            s.step(input);
        }
    }

    /// Starts the shadows of the bits of `reseed` over from the main game.
    fn reseed(&mut self, reseed: u8, last: u32) {
        for (i, kind) in SHADOWS.iter().enumerate() {
            if reseed & (1 << i) != 0
                && let Some(s) = self.shadows.get_mut(i)
            {
                *s = kind.start(&self.main, last);
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
    pub landings_pruned: u64,
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
    /// What the search did.
    pub stats: SearchStats,
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
    let fail = |reason: String| SearchFailure {
        reason,
        stats: SearchStats::default(),
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
    for &held in &lead {
        start.step(input(held, cfg.start_held));
    }
    let start = &start;
    let grid = Grid::build(start.level(), cfg.reach);
    // The fields are walked by Claw's feet; a point he is to reach is his
    // middle.
    let feet = start.player().map_or(60, |o| o.move_rect.bottom);
    let mut targets: Vec<Target> = cfg
        .hints
        .iter()
        .map(|&(x, y)| Target {
            field: grid.field_to(x, y + feet, cfg.hint_radius),
            point: Some((x, y, cfg.hint_radius)),
        })
        .collect();
    let field = match goal {
        Goal::LevelComplete => {
            let (x, y) = end_object(start)
                .ok_or_else(|| fail("the level has no end object to head for".into()))?;
            // Claw takes it with any part of his body: his feet may be up
            // to his height below it.
            grid.field_to_box(x - 40, y - 30, x + 40, y + 2 * feet + 20)
        }
        Goal::Reach { x, y, radius } => grid.field_to(x, y + feet, radius.max(8)),
    };
    targets.push(Target { field, point: None });
    let mut s = Search {
        cfg,
        goal,
        targets,
        target: 0,
        nodes: Vec::new(),
        open: BinaryHeap::new(),
        seen: HashSet::new(),
        cache: HashMap::new(),
        cache_order: BTreeSet::new(),
        root: 0,
        root_sim: Worlds::new(start.clone(), cfg.start_held, cfg.fragile_cost > 0),
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
    /// Fragile children along the path from the start.
    fragile: u32,
    /// The shadows started over from this node's game, one bit each.
    reseed: u8,
    h: u32,
    prio: i64,
    tie: u64,
}

/// Route ticks of Claw's last landing and of the last press of jump.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Timing {
    landed: Option<u64>,
    jumped: Option<u64>,
}

/// How a child came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Alive,
    Dead,
    /// A jump was pressed too close to a landing.
    TooClose,
    /// The goal was met in the tick of this index within the action.
    Done(usize),
}

struct Child {
    action: Action,
    worlds: Worlds,
    reseed: u8,
    outcome: Outcome,
    lost: i32,
    last: u32,
    timing: Timing,
    fragile: bool,
    checkpoint: bool,
    hint: bool,
}

type OpenEntry = Reverse<(i64, u64, u32)>;

struct Search<'a> {
    cfg: &'a SearchConfig,
    goal: Goal,
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
            let choices = actions::choices(&parent.main);
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
                let lost = node.lost + child.lost;
                let fragile = node.fragile + u32::from(child.fragile);
                self.stats.fragile += u64::from(child.fragile);
                let prio = i64::from(h.min(UNREACHABLE / 2))
                    + self.cfg.tick_cost * ticks as i64
                    + self.cfg.health_cost * i64::from(lost)
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
                if hint || (child.checkpoint && ahead >= self.cfg.checkpoint_progress) {
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
        self.targets[self.target]
            .field
            .at(o.x, o.y + o.move_rect.bottom - 2)
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
        for n in path.into_iter().rev() {
            let node = self.nodes[n as usize];
            for held in node.action.expect("a non-root node has an action").masks() {
                w.step(input(held, prev));
                prev = held;
            }
            w.reseed(node.reseed, prev);
        }
        w
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
        // Each node keeps its shadows too: the budget counts games.
        let games = 1 + worlds.shadows.len();
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
            check_fragile: self.cfg.fragile_cost > 0,
        };
        let goal = &self.goal;
        let point = self.targets[self.target].point;
        if threads <= 1 {
            return choices
                .iter()
                .map(|&a| play(parent, from, a, goal, point))
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
                            .map(|(i, &a)| (i, play(parent, from, a, goal, point)))
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
            stats: self.stats.clone(),
        }
    }

    fn log(&self, id: u32) {
        let n = self.nodes[id as usize];
        eprintln!(
            "claw-bot: {} nodes, {} open, {} kept, {} rebuilt, node at tick {} with {} px to go, closest {} px at {:?}, {:.0} s",
            self.stats.expanded,
            self.open.len(),
            self.cache.len(),
            self.stats.rebuilt,
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
    check_fragile: bool,
}

/// True when route ticks `a` and `b` are at most `gap` ticks apart (the
/// rule keeps a press and a landing more than `gap` ticks apart); never
/// with a gap of 0.
fn within(a: Option<u64>, b: u64, gap: u64) -> bool {
    gap > 0 && a.is_some_and(|a| a.abs_diff(b) <= gap)
}

/// Plays `action` on a copy of the games of `parent`, which start as
/// `from` says.
fn play(
    parents: &Worlds,
    from: From,
    action: Action,
    goal: &Goal,
    hint: Option<(i32, i32, i32)>,
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
        sim.step(input(held, prev));
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
        if let Goal::Reach { x, y, radius } = *goal {
            done |= near(&sim, x, y, radius);
        }
        if let Some((x, y, r)) = hint {
            reached |= near(&sim, x, y, r);
        }
        if died || dying(&sim) || sim.stats().lives < lives {
            outcome = Outcome::Dead;
            break;
        }
        if done {
            outcome = Outcome::Done(i);
            break;
        }
    }
    let mut shadows = Vec::with_capacity(parents.shadows.len());
    let mut reseed = 0u8;
    if outcome == Outcome::Alive {
        for (i, (shadow, kind)) in parents.shadows.iter().zip(SHADOWS).enumerate() {
            match played_on(shadow, from.last, &action.masks(), &sim) {
                Some(s) => shadows.push(s),
                None => {
                    reseed |= 1 << i;
                    shadows.push(kind.start(&sim, prev));
                }
            }
        }
    }
    let fragile = from.check_fragile
        && outcome == Outcome::Alive
        && (reseed != 0
            || shifted(action, from.last)
                .iter()
                .any(|masks| !holds_up(parent, from.last, masks, &sim))
            || (1..=2).any(|draws| {
                let mut other = parent.clone();
                let mut r = claw_sim::rng::Rng::new(other.rng_state());
                for _ in 0..draws {
                    r.next_u15();
                }
                other.set_rng_state(r.state());
                !holds_up(&other, from.last, &action.masks(), &sim)
            }));
    Child {
        action,
        worlds: Worlds { main: sim, shadows },
        reseed,
        outcome,
        lost,
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
/// him: in the same phase, within [`FRAGILE_DY`] up or down and
/// [`FRAGILE_PX`] across.
fn holds_up(parent: &Sim, last: u32, masks: &[u32], nominal: &Sim) -> bool {
    played_on(parent, last, masks, nominal).is_some()
}

/// `masks` played from `parent` as [`holds_up`] plays them: the game
/// after them when it holds up, else `None`.
fn played_on(parent: &Sim, last: u32, masks: &[u32], nominal: &Sim) -> Option<Sim> {
    let mut sim = parent.clone();
    let lives = sim.stats().lives;
    let mut prev = last;
    for &held in masks {
        sim.step(input(held, prev));
        prev = held;
        if dying(&sim) || sim.stats().lives < lives {
            return None;
        }
    }
    if sim.stats().health < nominal.stats().health {
        return None;
    }
    let near = match (sim.player(), nominal.player()) {
        (Some(a), Some(b)) => {
            actions::phase(&sim) == actions::phase(nominal)
                && (a.y - b.y).abs() <= FRAGILE_DY
                && (a.x - b.x).abs() <= FRAGILE_PX
        }
        _ => false,
    };
    near.then_some(sim)
}

/// The coarse state of a game the search treats as one: Claw's position to
/// 8 px, his state, facing, running and attacking, his health to 10
/// points, and the platforms (position to 16 px and state) and enemies
/// (alive or not) within about a screen of him.
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
    let objects = sim.objects();
    for id in objects.listed() {
        let Some(o) = objects.get(id) else { continue };
        if o.x.abs_diff(c.x) > 700 || o.y.abs_diff(c.y) > 500 {
            continue;
        }
        if o.type_flags & platform::PLATFORM != 0 {
            k.add(&[
                1,
                i64::from(id),
                i64::from(o.x >> 4),
                i64::from(o.y >> 4),
                i64::from(o.state),
            ]);
        } else if let Some(d) = enemy::data_of(o) {
            k.add(&[2, i64::from(id), i64::from(d.dead)]);
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
