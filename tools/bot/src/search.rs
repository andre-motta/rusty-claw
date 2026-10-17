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
//! root of a fresh search, and everything before it is dropped.
//!
//! The search is deterministic. Its budget counts expanded nodes, never
//! time; ties go by a hash of the seed and the node's state; and the
//! children of a node may be played on several threads, but they are
//! gathered in the order of the actions, so the thread count never changes
//! the result.

use std::cmp::Reverse;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
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
    /// Games kept in memory at most, besides the segment root.
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
    /// Children dropped as already seen.
    pub duplicates: u64,
    /// Games rebuilt by stepping from an ancestor.
    pub rebuilt: u64,
    /// Segment roots taken (hints reached and checkpoints passed).
    pub segments: u32,
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
    // before the start.
    let lead = vec![cfg.start_held; MIN_SPACING.saturating_sub(cfg.start_held_for)];
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
        root_sim: start.clone(),
        lead,
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
    lost: i32,
    h: u32,
    prio: i64,
    tie: u64,
}

/// How a child came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Alive,
    Dead,
    /// The goal was met in the tick of this index within the action.
    Done(usize),
}

struct Child {
    action: Action,
    sim: Sim,
    outcome: Outcome,
    lost: i32,
    last: u32,
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
    cache: HashMap<u32, Sim>,
    cache_order: BTreeSet<(i64, u64, u32)>,
    root: u32,
    root_sim: Sim,
    /// Ticks held at [`SearchConfig::start_held`] before the root.
    lead: Vec<u32>,
    stats: SearchStats,
    began: Instant,
}

impl Search<'_> {
    fn run(&mut self, start: &Sim) -> Result<Found, SearchFailure> {
        let h = self.distance(start);
        let root = Node {
            parent: None,
            action: None,
            ticks: self.lead.len() as u64,
            last: self.cfg.start_held,
            lost: 0,
            h,
            prio: 0,
            tie: 0,
        };
        self.nodes.push(root);
        self.seen.insert(state_key(start));
        self.open.push(Reverse((0, 0, 0)));
        while let Some(Reverse((_, _, id))) = self.open.pop() {
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
            let choices = actions::choices(&parent);
            let children = self.play_children(&parent, node.last, &choices);
            self.keep(id, parent);
            for child in children {
                self.stats.generated += 1;
                match child.outcome {
                    Outcome::Dead => {
                        self.stats.deaths_pruned += 1;
                        continue;
                    }
                    Outcome::Done(_) => return Ok(self.found(id, &child)),
                    Outcome::Alive => {}
                }
                let ticks = node.ticks + child.action.masks().len() as u64;
                if ticks > self.cfg.max_ticks {
                    continue;
                }
                let key = state_key(&child.sim);
                if !self.seen.insert(key) {
                    self.stats.duplicates += 1;
                    continue;
                }
                let h = self.distance(&child.sim);
                let lost = node.lost + child.lost;
                let prio = i64::from(h.min(UNREACHABLE / 2))
                    + self.cfg.tick_cost * ticks as i64
                    + self.cfg.health_cost * i64::from(lost);
                let cid = self.nodes.len() as u32;
                let tie = mix(self.cfg.seed ^ key);
                self.nodes.push(Node {
                    parent: Some(id),
                    action: Some(child.action),
                    ticks,
                    last: child.last,
                    lost,
                    h,
                    prio,
                    tie,
                });
                if h < self.stats.best_distance {
                    self.stats.best_distance = h;
                    if let Some(o) = child.sim.player() {
                        self.stats.best_at = (o.x, o.y);
                    }
                }
                let ahead = self.nodes[self.root as usize].h.saturating_sub(h);
                let hint = child.hint && self.targets[self.target].point.is_some();
                if hint || (child.checkpoint && ahead >= self.cfg.checkpoint_progress) {
                    self.commit(cid, child.sim, hint);
                    break;
                }
                self.open.push(Reverse((prio, tie, cid)));
                self.cache_put(cid, child.sim);
            }
        }
        Err(self.failure("every reachable state was searched"))
    }

    /// Makes `id` the root of a fresh segment; with `hint`, the next
    /// target becomes the current one.
    fn commit(&mut self, id: u32, sim: Sim, hint: bool) {
        self.stats.segments += 1;
        if hint && self.target + 1 < self.targets.len() {
            self.target += 1;
        }
        self.open.clear();
        self.seen.clear();
        self.cache.clear();
        self.cache_order.clear();
        self.seen.insert(state_key(&sim));
        let h = self.distance(&sim);
        let n = &mut self.nodes[id as usize];
        n.h = h;
        self.root = id;
        self.root_sim = sim;
        self.open.push(Reverse((0, 0, id)));
        if self.cfg.log_every > 0 {
            let (x, y) = self.root_sim.player().map_or((0, 0), |o| (o.x, o.y));
            eprintln!(
                "claw-bot: segment {} at ({x}, {y}), tick {}, {h} px to go, {} nodes",
                self.stats.segments, self.nodes[id as usize].ticks, self.stats.expanded
            );
        }
    }

    fn distance(&self, sim: &Sim) -> u32 {
        let Some(o) = sim.player() else {
            return UNREACHABLE;
        };
        self.targets[self.target]
            .field
            .at(o.x, o.y + o.move_rect.bottom - 2)
    }

    /// The game of node `id`, from the cache or rebuilt from its nearest
    /// ancestor that has one.
    fn sim_of(&mut self, id: u32) -> Sim {
        if id == self.root {
            return self.root_sim.clone();
        }
        if let Some(sim) = self.cache.remove(&id) {
            let n = self.nodes[id as usize];
            self.cache_order.remove(&(n.prio, n.tie, id));
            return sim;
        }
        self.stats.rebuilt += 1;
        let mut path = Vec::new();
        let mut cur = id;
        while cur != self.root && !self.cache.contains_key(&cur) {
            let n = self.nodes[cur as usize];
            path.push(n.action.expect("a non-root node has an action"));
            cur = n.parent.expect("a non-root node has a parent");
        }
        let mut sim = if cur == self.root {
            self.root_sim.clone()
        } else {
            self.cache[&cur].clone()
        };
        let mut prev = self.nodes[cur as usize].last;
        for a in path.into_iter().rev() {
            for held in a.masks() {
                sim.step(input(held, prev));
                prev = held;
            }
        }
        sim
    }

    /// Keeps the game of an expanded node, as a base for rebuilding its
    /// descendants.
    fn keep(&mut self, id: u32, sim: Sim) {
        if id != self.root {
            self.cache_put(id, sim);
        }
    }

    fn cache_put(&mut self, id: u32, sim: Sim) {
        let n = self.nodes[id as usize];
        self.cache.insert(id, sim);
        self.cache_order.insert((n.prio, n.tie, id));
        while self.cache.len() > self.cfg.cached_sims {
            let Some(worst) = self.cache_order.pop_last() else {
                break;
            };
            self.cache.remove(&worst.2);
        }
    }

    fn play_children(&self, parent: &Sim, last: u32, choices: &[Action]) -> Vec<Child> {
        let threads = self.cfg.threads.max(1).min(choices.len());
        let goal = &self.goal;
        let point = self.targets[self.target].point;
        if threads <= 1 {
            return choices
                .iter()
                .map(|&a| play(parent, last, a, goal, point))
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
                            .map(|(i, &a)| (i, play(parent, last, a, goal, point)))
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

/// Plays `action` on a copy of `parent`, whose last held mask was `last`.
fn play(
    parent: &Sim,
    last: u32,
    action: Action,
    goal: &Goal,
    hint: Option<(i32, i32, i32)>,
) -> Child {
    let mut sim = parent.clone();
    let lives = sim.stats().lives;
    let mut prev = last;
    let mut lost = 0;
    let mut checkpoint = false;
    let mut reached = false;
    let mut outcome = Outcome::Alive;
    for (i, held) in action.masks().into_iter().enumerate() {
        let health = sim.stats().health;
        sim.step(input(held, prev));
        prev = held;
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
    Child {
        action,
        sim,
        outcome,
        lost,
        last: prev,
        checkpoint,
        hint: reached,
    }
}

/// The coarse state of a game the search treats as one: Claw's position to
/// 8 px, his state, facing, running and attacking, his health to 10
/// points, and the platforms (position to 16 px and state) and enemies
/// (alive or not) within about a screen of him.
pub fn state_key(sim: &Sim) -> u64 {
    let mut h = DefaultHasher::new();
    let Some(c) = sim.player() else {
        return 0;
    };
    let claw = sim.claw();
    (c.x >> 3, c.y >> 3, c.state).hash(&mut h);
    (claw.facing_right, claw.attacking, claw.running_time == 0).hash(&mut h);
    (sim.stats().health / 10, sim.stats().lives).hash(&mut h);
    let objects = sim.objects();
    for id in objects.listed() {
        let Some(o) = objects.get(id) else { continue };
        if o.x.abs_diff(c.x) > 700 || o.y.abs_diff(c.y) > 500 {
            continue;
        }
        if o.type_flags & platform::PLATFORM != 0 {
            (id, o.x >> 4, o.y >> 4, o.state).hash(&mut h);
        } else if let Some(d) = enemy::data_of(o) {
            (id, d.dead).hash(&mut h);
        }
    }
    h.finish()
}

/// A 64-bit mix of `v` (splitmix64's finaliser).
fn mix(v: u64) -> u64 {
    let mut z = v.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
