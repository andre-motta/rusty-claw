//! Playing a level from a test: a [`Player`] steps a simulation with held
//! buttons chosen tick by tick, may move Claw between scripted segments,
//! and records both, so that the same game can be replayed through the
//! game's own frame loop at any display rate ([`replay`]).
//!
//! Positions always come from the level's own records, found by logic name
//! at run time; nothing from an install is stored here.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use claw_frame::{AudioCue, Buttons, GameEvent, SimulatedRefresh, TickInput};
use claw_game::Session;
use claw_sim::container::CollisionEvent;
use claw_sim::logic::enemy::{self, DeathKind, EnemyState};
use claw_sim::logic::pickup;
use claw_sim::object::{LogicKind, Object, ObjectId, special as sflags, types};
use claw_sim::player::state;
use claw_sim::tiles::{DEATH, GROUND, SOLID};
use claw_sim::trace::{self, TraceRow};
use claw_sim::{GameLogic, InputScript, Sim, SimLevel};

/// Jump.
pub const JUMP: u32 = 0x1;
/// Attack (sword, fists and feet).
pub const ATTACK: u32 = 0x2;
/// Fire the selected weapon.
pub const FIRE: u32 = 0x4;
/// Lift and throw.
pub const LIFT: u32 = 0x20;
/// Select the pistol.
pub const PISTOL: u32 = 0x40;
/// Select magic.
pub const MAGIC: u32 = 0x80;
/// Select dynamite.
pub const DYNAMITE: u32 = 0x100;
/// Left.
pub const LEFT: u32 = 0x100_0000;
/// Right.
pub const RIGHT: u32 = 0x200_0000;
/// Up.
pub const UP: u32 = 0x400_0000;
/// Down.
pub const DOWN: u32 = 0x800_0000;

/// Where the level placed an object: its record's x, y and z.
pub type Placed = (i32, i32, i32);

/// One enemy's end, as the route saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kill {
    /// The enemy's logic name.
    pub logic: String,
    /// Where the level placed it.
    pub placed: (i32, i32, i32),
    /// What hit it last: Claw's own blow (`CaptainClaw`), or the logic of
    /// the shot or explosion.
    pub by: String,
    /// How it died; `None` when it was removed without being seen dead.
    pub death: Option<DeathKind>,
    /// The tick it died on.
    pub tick: u64,
}

/// A blow that took Claw's health.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wound {
    /// The logic of what hit him.
    pub by: String,
    /// Health lost.
    pub lost: i32,
    /// The tick.
    pub tick: u64,
}

/// A game being played from a test, with everything it did recorded.
#[derive(Debug, Clone)]
pub struct Player {
    /// The game.
    pub sim: Sim,
    /// Buttons held at each tick.
    pub held: Vec<u32>,
    /// Claw moved to (x, y) before the tick of that index.
    pub teleports: Vec<(u64, i32, i32)>,
    /// Game events, with their tick.
    pub events: Vec<(u64, GameEvent)>,
    /// Enemies seen dying.
    pub kills: Vec<Kill>,
    /// Blows that took Claw's health.
    pub wounds: Vec<Wound>,
    /// Sound keys played, with their count.
    pub sounds: BTreeMap<String, usize>,
    /// Treasure Claw took: its kind (powerup code) and points, by the
    /// pickup's id.
    pub treasure: BTreeMap<ObjectId, (i32, i32)>,
    /// The logic name of every object seen so far, by id (slots are never
    /// reused, so an id names one object for the whole game).
    names: BTreeMap<ObjectId, Arc<str>>,
    /// Enemies alive after the last tick.
    alive: BTreeMap<ObjectId, (Arc<str>, Placed)>,
    prev: u32,
}

impl Player {
    /// Starts playing `sim`.
    pub fn new(sim: Sim) -> Player {
        let mut p = Player {
            sim,
            held: Vec::new(),
            teleports: Vec::new(),
            events: Vec::new(),
            kills: Vec::new(),
            wounds: Vec::new(),
            sounds: BTreeMap::new(),
            treasure: BTreeMap::new(),
            names: BTreeMap::new(),
            alive: BTreeMap::new(),
            prev: 0,
        };
        p.look();
        p
    }

    /// Ticks run.
    pub fn tick(&self) -> u64 {
        self.held.len() as u64
    }

    /// Claw's object.
    pub fn claw(&self) -> &Object {
        self.sim.player().expect("Claw")
    }

    /// Claw's position.
    pub fn at(&self) -> (i32, i32) {
        let c = self.claw();
        (c.x, c.y)
    }

    /// Claw's state.
    pub fn state(&self) -> i32 {
        self.claw().state
    }

    /// Claw's health.
    pub fn health(&self) -> i32 {
        self.sim.stats().health
    }

    /// The object `id`, if it is still there.
    pub fn obj(&self, id: ObjectId) -> Option<&Object> {
        self.sim.objects().get(id)
    }

    /// Runs one tick with `held` held (a button is pressed on the first
    /// tick it is held).
    pub fn step(&mut self, held: u32) {
        let pressed = held & !self.prev;
        self.prev = held;
        self.held.push(held);
        let tick = self.sim.tick();
        let health = self.sim.stats().health;
        let snap = self.sim.step(TickInput {
            held: Buttons(held),
            pressed: Buttons(pressed),
        });
        for e in snap.events {
            self.events.push((tick, e));
        }
        for a in &snap.audio {
            if let AudioCue::Play(s) = a {
                *self.sounds.entry(s.key.clone()).or_default() += 1;
            }
        }
        let lost = health - self.sim.stats().health;
        if lost > 0 {
            let by = self.attacker_of_claw();
            self.wounds.push(Wound { by, lost, tick });
        }
        self.look();
    }

    /// Runs `n` ticks with `held` held.
    pub fn hold(&mut self, held: u32, n: usize) {
        for _ in 0..n {
            self.step(held);
        }
    }

    /// Runs `n` ticks with nothing held.
    pub fn idle(&mut self, n: usize) {
        self.hold(0, n);
    }

    /// Moves Claw to (x, y) before the next tick: the start of a scripted
    /// segment.
    pub fn teleport(&mut self, x: i32, y: i32) {
        self.teleports.push((self.tick(), x, y));
        self.sim.teleport_player(x, y);
    }

    /// Idles until Claw has stood still on his feet (standing, not in a
    /// hit reaction or a fall) for five ticks, at most `limit` ticks.
    pub fn settle(&mut self, limit: usize) {
        let mut still = 0;
        let mut last = None;
        for _ in 0..limit {
            self.step(0);
            let now = (self.at(), self.state());
            still = if Some(now) == last && now.1 == state::STAND {
                still + 1
            } else {
                0
            };
            last = Some(now);
            if still >= 5 {
                break;
            }
        }
    }

    /// Runs ticks with `input` choosing the buttons from the game, until it
    /// returns `None` (done) or `limit` ticks ran; true when it finished.
    pub fn until(&mut self, limit: usize, mut input: impl FnMut(&Player) -> Option<u32>) -> bool {
        for _ in 0..limit {
            match input(self) {
                Some(held) => self.step(held),
                None => return true,
            }
        }
        input(self).is_none()
    }

    /// The logic name of whatever hit Claw in the last tick.
    fn attacker_of_claw(&self) -> String {
        let claw = claw_id(&self.sim);
        self.sim
            .collision_events()
            .iter()
            .filter_map(|e| match *e {
                CollisionEvent::Hit { attacker, victim }
                | CollisionEvent::Damage { attacker, victim }
                | CollisionEvent::Attack { attacker, victim }
                    if Some(victim) == claw && Some(attacker) != claw =>
                {
                    Some(attacker)
                }
                _ => None,
            })
            .filter_map(|id| self.names.get(&id).map(|n| n.to_string()))
            .next()
            .unwrap_or_else(|| "level".to_string())
    }

    /// Notes new objects, treasure Claw took and enemies that died in the
    /// last tick.
    fn look(&mut self) {
        for (id, o) in self.sim.objects().all() {
            if o.type_flags == types::TREASURE
                && o.special_flags & sflags::FLYING_TO_HUD != 0
                && !self.treasure.contains_key(&id)
                && let Some(d) = pickup::data_of(o)
            {
                self.treasure.insert(id, (d.kind, d.points));
            }
        }
        let from = self.names.keys().next_back().map_or(0, |&k| k + 1);
        for (id, o) in self.sim.objects().all() {
            if id >= from {
                self.names.insert(id, o.logic.clone());
            }
        }
        let tick = self.sim.tick();
        let mut gone = Vec::new();
        for (&id, (logic, placed)) in &self.alive {
            let dead = match self.sim.objects().get(id) {
                None => Some(None),
                Some(o) => enemy::data_of(o).filter(|d| d.dead).map(|d| Some(d.death)),
            };
            if let Some(death) = dead {
                let by = self
                    .sim
                    .collision_events()
                    .iter()
                    .rev()
                    .find_map(|e| match *e {
                        CollisionEvent::Hit { attacker, victim }
                        | CollisionEvent::Damage { attacker, victim }
                            if victim == id =>
                        {
                            self.names.get(&attacker).map(|n| n.to_string())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();
                gone.push((id, logic.to_string(), *placed, by, death));
            }
        }
        for (id, logic, placed, by, death) in gone {
            self.alive.remove(&id);
            self.kills.push(Kill {
                logic,
                placed,
                by,
                death,
                tick,
            });
        }
        for (id, o) in self.sim.objects().all() {
            if is_enemy(o)
                && !self.alive.contains_key(&id)
                && enemy::data_of(o).is_some_and(|d| !d.dead)
                && !self.kills.iter().any(|k| k.placed == o.editor)
            {
                self.alive.insert(id, (o.logic.clone(), o.editor));
            }
        }
    }

    /// The recorded input as a script.
    pub fn script(&self) -> InputScript {
        InputScript::from_held(&self.held)
    }
}

/// Claw's id in a simulation.
pub fn claw_id(sim: &Sim) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
}

/// True for an officer, a soldier or a rat.
pub fn is_enemy(o: &Object) -> bool {
    matches!(
        o.kind,
        LogicKind::Game(GameLogic::Officer | GameLogic::Soldier | GameLogic::Rat)
    )
}

/// The level's records running `logic`: (x, y, z).
pub fn records(level: &SimLevel, logic: &str) -> Vec<(i32, i32, i32)> {
    level
        .objects
        .iter()
        .filter(|o| o.logic == logic)
        .map(|o| (o.record[5], o.record[6], o.record[7]))
        .collect()
}

/// The object placed by the record at `placed`, if it exists.
pub fn placed_object(sim: &Sim, logic: &str, placed: (i32, i32, i32)) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic && o.editor == placed)
        .map(|(id, _)| id)
}

/// How a fight with an enemy stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fight {
    /// It is alive.
    Alive,
    /// Seen dead of a hit.
    Killed,
    /// Removed, or dead some other way.
    Lost,
}

/// How the fight with the enemy `id` stands. Only an enemy seen dead of a
/// hit counts as killed.
pub fn fight_state(sim: &Sim, id: ObjectId) -> Fight {
    let Some(o) = sim.objects().get(id) else {
        return Fight::Lost;
    };
    match EnemyState::from_id(o.state) {
        Some(EnemyState::Dead) => match enemy::data_of(o) {
            Some(d) if d.dead && d.death == DeathKind::Hit => Fight::Killed,
            _ => Fight::Lost,
        },
        _ => Fight::Alive,
    }
}

/// The bottom of an object's movement box.
pub fn feet(o: &Object) -> i32 {
    o.y + o.move_rect.bottom
}

/// True when Claw's whole body at his position is out of solid tiles.
pub fn room_for_claw(sim: &Sim) -> bool {
    let c = sim.player().expect("Claw");
    let map = &sim.level().map;
    (c.y + c.move_rect.top..=c.y + c.move_rect.bottom)
        .step_by(8)
        .all(|y| {
            [c.x + c.move_rect.left + 2, c.x, c.x + c.move_rect.right - 2]
                .iter()
                .all(|&x| map.attr(x, y) != SOLID)
        })
}

/// The floor under (x, y): the lowest clear row above ground or solid
/// tiles, searching down from y for `depth` px.
pub fn floor_under(level: &SimLevel, x: i32, y: i32, depth: i32) -> Option<i32> {
    let map = &level.map;
    (y..y + depth)
        .find(|&yy| matches!(map.attr(x, yy + 1), SOLID | GROUND) && map.attr(x, yy) != SOLID)
}

/// Moves Claw to stand at x on the floor under (x, y - 100) (searching 300
/// px down) and lets him settle; false when there is no floor there.
pub fn stand_at(p: &mut Player, x: i32, y: i32) -> bool {
    let Some(floor) = floor_under(p.sim.level(), x, y - 100, 300) else {
        return false;
    };
    let bottom = p.claw().move_rect.bottom;
    p.teleport(x, floor - bottom - 4);
    p.settle(60);
    p.state() == state::STAND && (feet(p.claw()) - floor).abs() <= 2
}

/// True when the floor under Claw's feet reaches `half` px to either side
/// of him without a gap, and no death tile lies in that stretch: a blow
/// that pushes him back leaves him standing.
pub fn firm_footing(sim: &Sim, half: i32) -> bool {
    let c = sim.player().expect("Claw");
    let map = &sim.level().map;
    let y = feet(c);
    (c.x - half..=c.x + half).step_by(4).all(|x| {
        (y - 4..=y + 6).any(|yy| matches!(map.attr(x, yy), SOLID | GROUND))
            && (y - 64..=y + 32).all(|yy| map.attr(x, yy) != DEATH)
    })
}

/// A hash of every object of the game (position, state, frame, health and
/// flags), Claw's progress and both random streams.
pub fn digest(sim: &Sim) -> u64 {
    let mut h = DefaultHasher::new();
    sim.rng().state().hash(&mut h);
    sim.sound_rng().state().hash(&mut h);
    format!("{:?}", sim.stats()).hash(&mut h);
    sim.objects().len().hash(&mut h);
    for (id, o) in sim.objects().all() {
        (id, o.x, o.y, o.z, o.state, o.frame, o.health, o.flags).hash(&mut h);
        (o.draw_flags, o.type_flags, o.image_set.as_str()).hash(&mut h);
    }
    h.finish()
}

/// How a replay feeds its input to the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feed {
    /// The recorded script, one row per tick.
    Script,
    /// Key events at the script's changes, a millisecond into their tick,
    /// handled while the frame that contains them runs.
    Keys,
}

/// One replay: the trace rows (with the input source column made the same
/// for every feed) and the digest of the game before every tick and after
/// the last one.
#[derive(Debug, Clone, PartialEq)]
pub struct Replay {
    /// One CSV row per tick.
    pub trace: String,
    /// Digests, one per tick plus the final one.
    pub digests: Vec<u64>,
}

/// Replays `player`'s recording from `start` (the game as it was before
/// the first recorded tick) through the game's frame loop at a simulated
/// `hz` display, with optional jitter, feeding the input as `feed`. The
/// recorded moves of Claw happen before their ticks.
pub fn replay(start: &Sim, player: &Player, hz: f64, jitter: Option<u64>, feed: Feed) -> Replay {
    let script = player.script();
    let n = script.ticks.len();
    let mut session = Session::new(start.clone());
    if feed == Feed::Script {
        session = session.with_replay(script.clone());
    }
    session.record_trace();
    let digests = Rc::new(RefCell::new(Vec::with_capacity(n + 1)));
    let seen = digests.clone();
    let teleports = player.teleports.clone();
    let mut next = 0;
    session.set_before_tick(Box::new(move |tick, sim: &mut Sim| {
        while let Some(&(at, x, y)) = teleports.get(next) {
            if at != tick {
                break;
            }
            sim.teleport_player(x, y);
            next += 1;
        }
        seen.borrow_mut().push(digest(sim));
    }));
    let mut refresh = SimulatedRefresh::new(hz);
    if let Some(seed) = jitter {
        refresh = refresh.with_jitter(0.25, seed);
    }
    let events = script.events();
    let mut next_event = 0;
    let mut clock = 0u64;
    // One tick past the recording (with no input), so that the game
    // after the last recorded tick is seen too.
    while (session.ticks() as usize) <= n {
        let dt = refresh.next().unwrap_or_default();
        let end = clock + dt.as_micros() as u64;
        if feed == Feed::Keys {
            while let Some(&(at, b, down)) = events.get(next_event) {
                let happened = at + 1_000;
                if happened > end {
                    break;
                }
                let stamp = session.stamp(Duration::from_micros(happened - clock));
                session.key(stamp, b, down);
                next_event += 1;
            }
        }
        session.frame(dt);
        clock = end;
    }
    let mut digests = digests.borrow().clone();
    digests.truncate(n + 1);
    let rows: Vec<TraceRow> = session.trace()[..n].to_vec();
    let trace = trace::to_csv(&rows)
        .replace(",script,", ",input,")
        .replace(",keyboard,", ",input,");
    Replay { trace, digests }
}

/// What Claw fights with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    /// His sword, fists and feet (the attack button).
    Sword,
    /// The pistol.
    Pistol,
    /// Magic.
    Magic,
    /// Dynamite.
    Dynamite,
}

impl Arm {
    /// The key that selects and fires this weapon (the attack button for
    /// the sword).
    pub fn key(self) -> u32 {
        match self {
            Arm::Sword => ATTACK,
            Arm::Pistol => PISTOL,
            Arm::Magic => MAGIC,
            Arm::Dynamite => DYNAMITE,
        }
    }

    /// The logic of the shot this weapon leaves, `CaptainClaw` for the
    /// sword (Claw's own blow).
    pub fn shot(self) -> &'static str {
        match self {
            Arm::Sword => "CaptainClaw",
            Arm::Pistol => "PistolBullet",
            Arm::Magic => "MagicBullet",
            Arm::Dynamite => "Dynamite",
        }
    }
}

/// Puts Claw beside the enemy placed at `placed`, standing at the height of
/// its feet with room for his body, at the first of `offsets` (from its
/// record) that works, with a clear line to it at hand height when
/// `clear_line`. Each try runs on a copy of the game, so only the one that
/// works is played. Returns the enemy.
pub fn place_beside(
    p: &mut Player,
    logic: &str,
    placed: (i32, i32, i32),
    offsets: &[i32],
    clear_line: bool,
) -> Option<ObjectId> {
    let (x, y, _) = placed;
    let rat = logic == "Rat";
    for &dx in offsets {
        let mut trial = p.clone();
        trial.teleport(x + dx, if rat { y - 40 } else { y - 30 });
        trial.settle(60);
        let Some(id) = placed_object(&trial.sim, logic, placed) else {
            continue;
        };
        if fight_state(&trial.sim, id) != Fight::Alive {
            continue;
        }
        let (c, o) = (trial.claw(), trial.obj(id).unwrap());
        if c.state != state::STAND
            || (feet(c) - feet(o)).abs() > 12
            || !room_for_claw(&trial.sim)
            || !firm_footing(&trial.sim, 48)
            || trial.health() <= 0
        {
            continue;
        }
        if clear_line {
            let map = &trial.sim.level().map;
            let hand = c.y - 21;
            let (a, b) = (c.x.min(o.x), c.x.max(o.x));
            if (a..=b).step_by(8).any(|xx| map.attr(xx, hand) == SOLID) {
                continue;
            }
        }
        *p = trial;
        return Some(id);
    }
    None
}

/// Fights the enemy `id` with `arm` until it is seen dead of a hit, it is
/// lost some other way, Claw dies or `limit` ticks ran. With the sword Claw
/// walks up to it and strikes; with a weapon he faces it and fires, walking
/// closer only when it is far; against a rat he crouches. Returns the ticks
/// the fight took when it was killed.
pub fn fight(p: &mut Player, id: ObjectId, arm: Arm, limit: usize) -> Option<usize> {
    let rat = p.obj(id).is_some_and(|o| &*o.logic == "Rat");
    let reach = if arm == Arm::Sword { 55 } else { 420 };
    let mut since = 10;
    let start = p.tick();
    let done = p.until(limit, |p| {
        if fight_state(&p.sim, id) != Fight::Alive || p.health() <= 0 {
            return None;
        }
        let (c, o) = (p.claw(), p.obj(id)?);
        let dx = o.x - c.x;
        let toward = if dx > 0 { RIGHT } else { LEFT };
        let facing = p.sim.claw().facing_right == (dx > 0);
        let busy = p.sim.claw().attacking;
        Some(if (dx.abs() > reach || !facing) && !busy {
            since = 10;
            toward
        } else if !busy && since > 2 {
            since = 0;
            // Crouching against a rat, which is too low for a standing
            // blow or shot.
            if rat { DOWN | arm.key() } else { arm.key() }
        } else {
            since += 1;
            if rat { DOWN } else { 0 }
        })
    });
    let _ = done;
    (fight_state(&p.sim, id) == Fight::Killed).then(|| (p.tick() - start) as usize)
}

/// Strikes at the enemy `id` from the air with `arm`: Claw is put beside
/// it, `rise` px above its position and facing it, and attacks as he falls
/// past; tried from both sides. Each try runs on a copy of the game and
/// only one that kills it (seen dead of a hit) with Claw unhurt by the
/// level is played. Returns true when it was killed.
pub fn strike_from_the_air(p: &mut Player, id: ObjectId, arm: Arm, rises: &[i32]) -> bool {
    let reach = if arm == Arm::Sword { 55 } else { 110 };
    for &rise in rises {
        for side in [-1, 1] {
            let Some(o) = p.obj(id) else { return false };
            if fight_state(&p.sim, id) != Fight::Alive {
                return fight_state(&p.sim, id) == Fight::Killed;
            }
            let (x, y) = (o.x, o.y);
            let mut q = p.clone();
            q.teleport(x + side * reach, y - rise);
            let toward = if side < 0 { RIGHT } else { LEFT };
            q.step(toward);
            q.step(toward | arm.key());
            for _ in 0..24 {
                if fight_state(&q.sim, id) != Fight::Alive {
                    break;
                }
                q.step(0);
            }
            if fight_state(&q.sim, id) == Fight::Killed
                && q.health() > 0
                && !matches!(q.state(), state::DEATH | state::DEATH_FALL)
            {
                *p = q;
                return true;
            }
        }
    }
    false
}
