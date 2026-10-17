//! The simulation: one level, stepped one 18 ms tick at a time.

use std::collections::BTreeMap;
use std::sync::Arc;

use claw_frame::{
    AspectPolicy, AudioCue, Buttons, DrawMode, GameEvent, HudState, Point, PowerupHud, Renderable,
    SimSnapshot, TickInput, ViewConfig,
};

use crate::activation::{Activation, ActivationConfig, ActivationMode, Centres};
use crate::camera::{Camera, Followed};
use crate::container::{CollisionEvent, Container};
use crate::level::{ObjectSpec, SimLevel};
use crate::logic::{self, Outbox, World};
use crate::object::{LogicKind, Object, ObjectId, Rect, draw_flags, flags};
use crate::physics::Platform;
use crate::player::stats::{Carry, PlayerStats, StartState, TREASURE_KINDS};
use crate::player::{self, Claw, Ctx};
use crate::registry::{CAMERA_LOGIC, PLAYER_LOGIC, REG_ALWAYS, REG_CLASS_1, REG_CLASS_2, Registry};
use crate::rng::Rng;
use crate::tuning::{FuseClock, TICK_MS, Tuning};
use crate::view::{PlaneGeometry, PlaneView};

/// Simulation settings.
#[derive(Debug, Clone, PartialEq)]
pub struct SimConfig {
    /// The gameplay view and its presentation.
    pub view: ViewConfig,
    /// Activation windows.
    pub activation: ActivationConfig,
    /// Tuning data.
    pub tuning: Tuning,
    /// State of the game's shared random stream at time zero, when the
    /// first tick starts (the level's objects are set up before that from
    /// [`LEVEL_LOAD_SEED`]).
    pub seed: u32,
    /// Run the level objects' logics. Off, every level object stays an
    /// inert placeholder and collisions run no handler, as in the first
    /// playable version; the movement calibration replays use that.
    pub level_logics: bool,
    /// Whether the time a help screen was up reaches the dynamite fuse
    /// (see [`Sim::pass_menu_time`]); the original's way by default.
    pub fuse_clock: FuseClock,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig::for_view(ViewConfig::default())
    }
}

/// Width in logic pixels of a presentation of `view`'s shape at the logic
/// view's height, rounded up to an even number and at least the logic
/// width.
pub fn presentation_width(view: &ViewConfig) -> u32 {
    if view.render_height == 0 {
        return view.logic_width;
    }
    let w = u64::from(view.render_width) * u64::from(view.logic_height);
    let w = w.div_ceil(u64::from(view.render_height));
    let w = u32::try_from(w).unwrap_or(u32::MAX).max(view.logic_width);
    w.div_ceil(2) * 2
}

impl SimConfig {
    /// Settings for `view`: the activation mode follows its aspect policy.
    pub fn for_view(view: ViewConfig) -> SimConfig {
        let activation = match view.aspect_policy {
            AspectPolicy::Pillarbox => ActivationConfig::original(),
            AspectPolicy::FrozenEdgeClamped | AspectPolicy::FrozenEdgeCentered => {
                ActivationConfig::frozen_edge()
            }
            AspectPolicy::WidenedActivation => {
                // The widened window size comes from the view's activation
                // margin, the same value the presentation is drawn with.
                let (mx, my) = view.activation_margin();
                ActivationConfig::widened((
                    view.logic_width.saturating_add(mx.saturating_mul(2)),
                    view.logic_height.saturating_add(my.saturating_mul(2)),
                ))
            }
        };
        SimConfig {
            view,
            activation,
            tuning: Tuning::default(),
            seed: 1,
            level_logics: true,
            fuse_clock: FuseClock::Original,
        }
    }
}

/// What the simulation noticed while loading and running.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics {
    /// Logic names that are not registered, with their record counts;
    /// those objects run as inert placeholders.
    pub unknown_logics: BTreeMap<String, usize>,
    /// Level records dropped: empty logic name or position outside the
    /// main plane.
    pub dropped_records: usize,
    /// Registered logic names no family runs yet, with their record
    /// counts (only with level logics on); those objects run as inert
    /// placeholders.
    pub inert_logics: BTreeMap<String, usize>,
}

/// One level being simulated.
#[derive(Debug, Clone)]
pub struct Sim {
    cfg: SimConfig,
    level: Arc<SimLevel>,
    registry: Registry,
    objects: Container,
    activation: Activation,
    player: Option<ObjectId>,
    camera_obj: Option<ObjectId>,
    claw: Claw,
    camera: Camera,
    camera_point: (i32, i32),
    camera_override: Option<(i32, i32)>,
    rng: Rng,
    sound_rng: Rng,
    tick: u64,
    input: TickInput,
    /// Keys that count as already pressed: neither held nor pressed for
    /// Claw until released at a tick (see [`Sim::latch_keys`]).
    latched: Buttons,
    /// The input Claw's logic sees this tick: [`Sim::input`] without the
    /// latched keys.
    claw_input: TickInput,
    /// The frame pulse countdown, ms (see [`PULSE_MS`]).
    pulse_left: u32,
    /// The state the level's set-up left the random stream in, before
    /// time zero set it to the seed.
    set_up_rng: u32,
    events: Vec<CollisionEvent>,
    diagnostics: Diagnostics,
    out: Outbox,
    last_audio: Vec<AudioCue>,
    last_events: Vec<GameEvent>,
    /// [`Sim::end_life`] was called: Claw dies at the start of the next
    /// tick.
    end_life: bool,
}

/// What runs an object's logic: everything but the object container and
/// the activation index.
struct Runner<'a> {
    cfg: &'a SimConfig,
    level: &'a SimLevel,
    claw: &'a mut Claw,
    camera: &'a mut Camera,
    camera_point: &'a mut (i32, i32),
    camera_override: &'a mut Option<(i32, i32)>,
    rng: &'a mut Rng,
    sound_rng: &'a mut Rng,
    out: &'a mut Outbox,
    input: TickInput,
    tick: u64,
    now_ms: u64,
    pulse: bool,
    player: Option<ObjectId>,
}

impl Runner<'_> {
    fn plane(&self) -> PlaneGeometry {
        self.level.plane()
    }

    /// The logic view in level pixels.
    fn view(&self) -> Rect {
        let v = &self.cfg.view;
        PlaneView::new(
            self.plane(),
            *self.camera_point,
            (v.logic_width, v.logic_height),
        )
        .rect
    }

    fn world<'b>(&'b mut self, objects: &'b mut Container) -> World<'b> {
        let view = self.view();
        World {
            objects,
            claw_id: self.player,
            claw: &mut *self.claw,
            level: self.level,
            rng: &mut *self.rng,
            sound_rng: &mut *self.sound_rng,
            tuning: &self.cfg.tuning,
            tick: self.tick,
            now_ms: self.now_ms,
            pulse: self.pulse,
            view,
            out: &mut *self.out,
        }
    }

    /// Runs `f` on Claw's object `id` with the player context, then carries
    /// out the voice requests it made; `None` when the object does not
    /// exist.
    fn with_claw<R>(
        &mut self,
        objects: &mut Container,
        id: ObjectId,
        f: impl FnOnce(&mut Ctx) -> R,
    ) -> Option<R> {
        let platforms = platforms(objects);
        let centre = PlaneView::new(self.plane(), *self.camera_point, (640, 480)).centre;
        let view = self.view();
        let mut held = Object::new(0, 0, 0, 0, Arc::from(""), LogicKind::Inert);
        if !objects.swap_payload(id, &mut held) {
            return None;
        }
        let r = {
            let mut ctx = Ctx {
                o: &mut held,
                p: &mut *self.claw,
                id,
                objects: &mut *objects,
                level: self.level,
                platforms: &platforms,
                input: self.input,
                t: &self.cfg.tuning,
                rng: &mut *self.rng,
                sound_rng: &mut *self.sound_rng,
                camera_override: &mut *self.camera_override,
                plane_centre: centre,
                out: &mut *self.out,
                tick: self.tick,
                now_ms: self.now_ms,
                pulse: self.pulse,
                view,
                level_logics: self.cfg.level_logics,
            };
            f(&mut ctx)
        };
        objects.swap_payload(id, &mut held);
        self.say_requested(objects);
        Some(r)
    }

    /// Carries out the voice requests made since the last call, in request
    /// order.
    fn say_requested(&mut self, objects: &mut Container) {
        let voice = self.out.take_voice();
        if !voice.is_empty() {
            let mut w = self.world(objects);
            for v in voice {
                logic::sound::claw_say(&mut w, &v.key, v.force);
            }
        }
    }

    /// Starts the screen shake asked for since the last call, timed from
    /// the current tick.
    fn apply_shake(&mut self) {
        if let Some(ms) = self.out.take_shake() {
            self.camera.start_shake(self.now_ms, u64::from(ms));
        }
    }

    fn run(&mut self, objects: &mut Container, id: ObjectId) {
        let Some(kind) = objects.get(id).map(|o| o.kind) else {
            return;
        };
        match kind {
            LogicKind::Player => {
                self.with_claw(objects, id, player::tick);
            }
            LogicKind::Camera => {
                let followed = self.player.and_then(|p| objects.get(p)).map(|o| Followed {
                    x: o.x,
                    y: o.y,
                    state: o.state,
                    physics_type: o.physics_type,
                    look_down_time: self.claw.look_down_time,
                });
                self.apply_shake();
                self.camera.tick(
                    followed.as_ref(),
                    *self.camera_override,
                    &self.cfg.tuning.camera,
                    self.rng,
                    self.now_ms,
                );
                *self.camera_point = self.camera.current;
            }
            LogicKind::Inert => {}
            LogicKind::Game(_) => {
                let mut w = self.world(objects);
                logic::tick(&mut w, id);
            }
        }
    }

    /// The last call of an object being destroyed or put to sleep; its
    /// state already says which.
    fn notify(&mut self, objects: &mut Container, id: ObjectId) {
        if matches!(objects.get(id).map(|o| o.kind), Some(LogicKind::Game(_))) {
            let mut w = self.world(objects);
            logic::tick(&mut w, id);
        }
    }

    /// Calls the handlers of one pair-collision event.
    fn collision(&mut self, objects: &mut Container, event: CollisionEvent) {
        let kind = |objects: &Container, id: ObjectId| objects.get(id).map(|o| o.kind);
        match event {
            CollisionEvent::Attack { attacker, victim } => {
                if let Some(LogicKind::Game(_)) = kind(objects, attacker) {
                    let mut w = self.world(objects);
                    logic::on_attack(&mut w, attacker, victim);
                }
            }
            CollisionEvent::Hit { attacker, victim } => match kind(objects, victim) {
                Some(LogicKind::Game(_)) => {
                    let mut w = self.world(objects);
                    logic::on_hit(&mut w, victim, attacker);
                }
                Some(LogicKind::Player) => {
                    self.with_claw(objects, victim, |c| player::hit::claw_hit(c, attacker));
                }
                _ => {}
            },
            CollisionEvent::Damage { .. } => {}
        }
    }

    /// Lists the objects spawned during the pass, in spawn order: those
    /// already asked to be destroyed are removed instead, and those with
    /// [`flags::RUN_AT_CREATION`] run their logic once at once.
    fn list_spawned(&mut self, objects: &mut Container) {
        while self.out.has_spawned() {
            for id in self.out.take_spawned() {
                let Some(o) = objects.get(id) else { continue };
                if o.flags & flags::DESTROY != 0 {
                    objects.remove(id);
                    continue;
                }
                let run_now = o.flags & flags::RUN_AT_CREATION != 0;
                objects.insert(id);
                if run_now {
                    self.run(objects, id);
                }
            }
        }
    }
}

/// State the shared random stream is set to when a level starts loading;
/// the level's objects are set up from it. Time zero then starts from
/// [`SimConfig::seed`].
pub const LEVEL_LOAD_SEED: u32 = 0x1_B3CF;

/// Period of the frame pulse, ms: a countdown fed the 18 ms of each tick
/// that reaches 0 on every third tick. Enemies make their per-tick roll and
/// a few random choices only on a pulse tick.
pub const PULSE_MS: u32 = 33;

/// The frame pulse countdown when play starts, ms: the first pulse comes in
/// the sixth tick (the tick ending at 108 ms), then every third tick.
pub const PULSE_START_MS: u32 = 100;

/// Seed of the sound-only random stream of a level: it depends on the
/// game seed and the level number only, so it restarts with each level.
pub fn sound_seed(seed: u32, level_number: u32) -> u32 {
    seed.wrapping_add(level_number.wrapping_mul(7919))
        .rotate_left(16)
}

/// Platform objects in list order, with the move each made this tick.
pub(crate) fn platforms(objects: &Container) -> Vec<Platform> {
    let mut out = Vec::new();
    let mut cur = objects.head();
    while let Some(id) = cur {
        let Some(o) = objects.get(id) else { break };
        if o.type_flags & 0x80 != 0 && o.hit_rect.is_set() {
            out.push(Platform {
                id,
                x: o.x,
                y: o.y,
                hit_rect: o.hit_rect,
                move_x: o.platform_move.0,
                move_y: o.platform_move.1,
            });
        }
        cur = o.next;
    }
    out
}

fn unset_if_empty(r: Rect) -> Rect {
    if r.left == 0 && r.right == 0 {
        Rect::UNSET
    } else {
        r
    }
}

impl Sim {
    /// Loads `level`: places the level objects dormant, creates Claw (who
    /// starts at the main plane's view centre, seeded with the level's
    /// start point, dropped onto the floor below) and the camera, wakes
    /// the objects around the start and runs their first logic call (their
    /// set-up), drawing from the shared random stream started at
    /// [`LEVEL_LOAD_SEED`]. The stream is then set to [`SimConfig::seed`]
    /// for time zero. Every key starts latched (see [`Sim::latch_keys`]).
    pub fn new(level: impl Into<Arc<SimLevel>>, cfg: SimConfig) -> Sim {
        let level: Arc<SimLevel> = level.into();
        let plane = level.plane();
        let activation = Activation::new(cfg.activation.clone(), plane);
        let mut sim = Sim {
            rng: Rng::new(LEVEL_LOAD_SEED),
            sound_rng: Rng::new(sound_seed(cfg.seed, level.number)),
            cfg,
            registry: Registry::standard(),
            objects: Container::new(),
            activation,
            player: None,
            camera_obj: None,
            claw: Claw::default(),
            camera: Camera::default(),
            camera_point: level.start,
            camera_override: None,
            tick: 0,
            input: TickInput::default(),
            latched: Buttons(u32::MAX),
            claw_input: TickInput::default(),
            pulse_left: PULSE_START_MS,
            set_up_rng: LEVEL_LOAD_SEED,
            events: Vec::new(),
            diagnostics: Diagnostics::default(),
            out: Outbox::new(),
            last_audio: Vec::new(),
            last_events: Vec::new(),
            end_life: false,
            level,
        };
        let level = sim.level.clone();
        for spec in &level.objects {
            sim.load_record(spec, plane);
        }
        let mut claw = Object::new(0, 320, 350, 100, Arc::from(PLAYER_LOGIC), LogicKind::Player);
        claw.flags = flags::WORLD | flags::ALWAYS_ACTIVE;
        claw.move_res_x = sim.cfg.tuning.physics.move_res;
        claw.move_res_y = sim.cfg.tuning.physics.move_res;
        let claw_id = sim.objects.add(claw);
        sim.objects.insert(claw_id);
        sim.player = Some(claw_id);
        sim.run_logic(claw_id);
        let mut cam = Object::new(
            0,
            i32::MIN,
            0,
            9999,
            Arc::from(CAMERA_LOGIC),
            LogicKind::Camera,
        );
        cam.flags = 0;
        let cam_id = sim.objects.add(cam);
        sim.objects.insert(cam_id);
        sim.camera_obj = Some(cam_id);
        sim.camera_override = None;
        sim.activation.wake_always_active(&mut sim.objects);
        let c = sim.centres();
        sim.activation.enter(&mut sim.objects, c);
        // The objects around the start are set up while the level loads,
        // in list order, so their set-up draws come before time zero and
        // their first update in the first tick.
        sim.set_up_pass();
        sim.set_up_rng = sim.rng.state();
        sim.rng = Rng::new(sim.cfg.seed);
        sim
    }

    fn load_record(&mut self, spec: &ObjectSpec, plane: PlaneGeometry) {
        let r = &spec.record;
        let (x, y) = (r[5], r[6]);
        if spec.logic.is_empty() || x < 0 || y < 0 || x >= plane.width || y >= plane.height {
            self.diagnostics.dropped_records += 1;
            return;
        }
        let (name, reg_flags, kind) = match self.registry.get(&spec.logic) {
            Some(e) => (e.name.clone(), e.flags, e.kind),
            None => {
                *self
                    .diagnostics
                    .unknown_logics
                    .entry(spec.logic.clone())
                    .or_default() += 1;
                (Arc::from(spec.logic.as_str()), 0, LogicKind::Inert)
            }
        };
        // Level objects run their family's logic when level logics are on
        // and a family runs them; otherwise they are placeholders. Claw and
        // the camera are created by the simulation itself, so a record
        // naming them gets a placeholder too.
        let kind = match kind {
            LogicKind::Game(_) if self.cfg.level_logics => kind,
            _ => LogicKind::Inert,
        };
        if self.cfg.level_logics
            && kind == LogicKind::Inert
            && self.registry.get(&spec.logic).is_some()
        {
            *self
                .diagnostics
                .inert_logics
                .entry(spec.logic.clone())
                .or_default() += 1;
        }
        let mut o = Object::new(r[0], x, y, r[7], name, kind);
        o.reg_flags = reg_flags;
        o.flags = flags::WORLD;
        if reg_flags & REG_CLASS_1 != 0 {
            o.flags |= flags::CLASS_1;
        } else if reg_flags & REG_CLASS_2 != 0 {
            o.flags |= flags::CLASS_2;
        }
        o.flags |= r[10] as u32;
        if reg_flags & REG_ALWAYS != 0 {
            // The always-active logic keeps itself awake once it runs.
            o.flags |= flags::ALWAYS_ACTIVE;
        }
        o.draw_flags = r[11] as u32;
        o.image_set = spec.image_set.clone();
        o.frame = spec.frame;
        o.damage = r[16];
        o.health = r[18];
        o.move_rect = unset_if_empty(Rect::new(r[19], r[20], r[21], r[22]));
        o.hit_rect = unset_if_empty(Rect::new(r[23], r[24], r[25], r[26]));
        o.attack_rect = unset_if_empty(Rect::new(r[27], r[28], r[29], r[30]));
        o.speed_x = r[55];
        o.speed_y = r[56];
        o.type_flags = r[67] as u32;
        o.hit_type_flags = r[68] as u32;
        o.move_res_x = if r[69] != 0 {
            r[69]
        } else {
            self.cfg.tuning.physics.move_res
        };
        o.move_res_y = if r[70] != 0 {
            r[70]
        } else {
            self.cfg.tuning.physics.move_res
        };
        o.record = Some(Box::new(*r));
        let id = self.objects.add(o);
        let obj = self.objects.get(id).expect("just added");
        self.activation.file(obj, id);
    }

    fn centres(&self) -> Centres {
        Centres::new(&self.cfg.activation, self.level.plane(), self.camera_point)
    }

    fn parts(&mut self) -> (Runner<'_>, &mut Container, &mut Activation) {
        let now_ms = self.tick * u64::from(TICK_MS);
        (
            Runner {
                cfg: &self.cfg,
                level: &self.level,
                claw: &mut self.claw,
                camera: &mut self.camera,
                camera_point: &mut self.camera_point,
                camera_override: &mut self.camera_override,
                rng: &mut self.rng,
                sound_rng: &mut self.sound_rng,
                out: &mut self.out,
                input: self.claw_input,
                tick: self.tick,
                now_ms,
                pulse: self.pulse_left == 0,
                player: self.player,
            },
            &mut self.objects,
            &mut self.activation,
        )
    }

    fn run_logic(&mut self, id: ObjectId) {
        let (mut runner, objects, _) = self.parts();
        runner.run(objects, id);
        runner.list_spawned(objects);
    }

    /// Advances one tick with `input` and returns the snapshot.
    pub fn step(&mut self, input: TickInput) -> SimSnapshot {
        self.input = input;
        // A latched key stays latched until a tick finds it up.
        self.latched = Buttons(self.latched.0 & input.held.0);
        self.claw_input = TickInput {
            held: Buttons(input.held.0 & !self.latched.0),
            pressed: Buttons(input.pressed.0 & !self.latched.0),
        };
        self.pulse_left = match self.pulse_left {
            0 => PULSE_MS,
            left if left <= TICK_MS => 0,
            left => left - TICK_MS,
        };
        if std::mem::take(&mut self.end_life) {
            self.apply_end_life();
        }
        let c = self.centres();
        self.activation.enter(&mut self.objects, c);
        self.update_pass();
        self.collision_pass();
        let c = self.centres();
        {
            let (mut runner, objects, activation) = self.parts();
            activation.leave(objects, c, &mut |objects, id, _| runner.notify(objects, id));
            // A line asked for outside Claw's handler, or while his handler
            // did not run, is said in this tick rather than left waiting.
            runner.say_requested(objects);
            runner.list_spawned(objects);
            // A shake asked for after the camera ran starts from this tick
            // too; the camera applies it from its next call.
            runner.apply_shake();
        }
        self.tick += 1;
        let (audio, events) = self.out.take_tick();
        self.last_audio = audio;
        self.last_events = events;
        let snapshot = self.snapshot();
        // Jumps are reported once, in the snapshot of the tick they happen
        // in (or the next one, for moves made between ticks).
        for id in self.objects.listed() {
            if let Some(o) = self.objects.get_mut(id) {
                o.teleported = false;
            }
        }
        snapshot
    }

    /// Gives up Claw's life now, before any logic of the tick (see
    /// [`Sim::end_life`]).
    fn apply_end_life(&mut self) {
        let Some(id) = self.player else { return };
        let (mut runner, objects, _) = self.parts();
        runner.with_claw(objects, id, player::death::end_life);
        runner.list_spawned(objects);
    }

    fn update_pass(&mut self) {
        self.pass(false);
    }

    /// The level load's set-up: the first logic call of every level object
    /// in the live list that has not had one, in list order, before time
    /// zero.
    fn set_up_pass(&mut self) {
        self.pass(true);
    }

    /// One walk over the live list running the objects' logics; with
    /// `set_up`, only the first call of the level objects not set up yet.
    fn pass(&mut self, set_up: bool) {
        let mut destroy = Vec::new();
        let mut resort = Vec::new();
        let (mut runner, objects, _) = self.parts();
        let mut cur = objects.head();
        while let Some(id) = cur {
            cur = objects.next_of(id);
            let run = {
                let Some(o) = objects.get_mut(id) else {
                    continue;
                };
                if set_up
                    && !(matches!(o.kind, LogicKind::Game(_))
                        && o.state == crate::object::engine_state::INIT)
                {
                    continue;
                }
                // Only the move made in this turn is published, so a platform
                // that stops or skips a tick stops carrying its riders.
                o.platform_move = (0, 0);
                if o.time_delay != 0 && TICK_MS < o.time_delay {
                    o.time_delay -= TICK_MS;
                    false
                } else {
                    o.time_delay = 0;
                    if o.frame_delay != 0 {
                        o.frame_delay -= 1;
                        false
                    } else {
                        true
                    }
                }
            };
            if run {
                runner.run(objects, id);
            }
            if let Some(o) = objects.get(id) {
                if o.flags & flags::DESTROY != 0 {
                    destroy.push(id);
                } else if o.flags & flags::RESORT != 0 {
                    resort.push(id);
                }
            }
        }
        objects.finish_pass(&destroy, &resort, &mut |objects, id| {
            runner.notify(objects, id);
        });
        runner.list_spawned(objects);
    }

    /// Pair collision; with level logics on, each event calls its handlers
    /// as it happens.
    fn collision_pass(&mut self) {
        let logics = self.cfg.level_logics;
        let mut events = Vec::new();
        {
            let (mut runner, objects, _) = self.parts();
            objects.pair_collision_with(&mut |objects, event| {
                events.push(event);
                if logics {
                    runner.collision(objects, event);
                }
            });
            runner.list_spawned(objects);
        }
        self.events = events;
    }

    /// The snapshot of the current state.
    pub fn snapshot(&self) -> SimSnapshot {
        let plane = self.level.plane();
        let view = &self.cfg.view;
        let rect = PlaneView::new(
            plane,
            self.camera_point,
            (view.logic_width, view.logic_height),
        )
        .rect;
        let mut renderables: Vec<Renderable> = Vec::new();
        for id in self.objects.listed() {
            let o = self.objects.get(id).expect("listed object exists");
            if let Some(r) = renderable(o) {
                renderables.push(r);
            }
        }
        let extra = (presentation_width(view).saturating_sub(view.logic_width) / 2) as i32;
        // Dormant objects are drawn only in the extra width of a wider
        // presentation (plus a margin for sprites that reach into it); a
        // presentation as wide as the logic view draws exactly what the
        // original does.
        if self.cfg.activation.mode != ActivationMode::Original && extra > 0 {
            let margin = 256;
            let area = Rect::new(
                rect.left - extra - margin,
                rect.top - margin,
                rect.right + extra + margin,
                rect.bottom + margin,
            );
            for (_, o) in self.objects.all() {
                if o.is_listed() || o.flags & flags::WORLD == 0 || !area.contains(o.x, o.y) {
                    continue;
                }
                if let Some(r) = renderable(o) {
                    let at = renderables
                        .iter()
                        .position(|x| x.z > r.z)
                        .unwrap_or(renderables.len());
                    renderables.insert(at, r);
                }
            }
        }
        SimSnapshot {
            tick: self.tick,
            camera: Point {
                x: f64::from(rect.left),
                y: f64::from(rect.top),
            },
            renderables,
            palette_fade: self.out.fade(),
            camera_teleported: self.camera.teleported,
            hud: self.hud(),
            audio: self.last_audio.clone(),
            events: self.last_events.clone(),
            transition: self.out.transition(),
        }
    }

    /// The screen transition in progress, as [`Sim::snapshot`] reports it,
    /// without building a snapshot.
    pub fn transition(&self) -> Option<claw_frame::ScreenTransition> {
        self.out.transition()
    }

    /// The status bar values.
    pub fn hud(&self) -> HudState {
        let s = &self.claw.stats;
        HudState {
            score: s.score,
            health: s.health,
            lives: s.lives,
            weapon: s.weapon,
            pistol_ammo: s.pistol,
            magic_ammo: s.magic,
            dynamite_ammo: s.dynamite,
            powerup: (s.powerup_kind != 0 && s.powerup_ms > 0).then_some(PowerupHud {
                kind: s.powerup_kind,
                remaining_ms: s.powerup_ms,
            }),
            timer_ms: None,
        }
    }

    /// Starts `level` like [`Sim::new`], then applies `start`: carried
    /// progress and a save point to begin at.
    pub fn with_start(level: impl Into<Arc<SimLevel>>, cfg: SimConfig, start: StartState) -> Sim {
        let mut sim = Sim::new(level, cfg);
        if let Some(id) = sim.player {
            let (mut runner, objects, _) = sim.parts();
            runner.with_claw(objects, id, |c| player::progress::apply_start(c, &start));
            runner.list_spawned(objects);
        }
        sim
    }

    /// What Claw hands to the next level.
    pub fn carry(&self) -> Carry {
        player::progress::carry(&self.claw)
    }

    /// Claw's health, score, lives, ammunition, weapon, powerup and
    /// treasures.
    pub fn stats(&self) -> &PlayerStats {
        &self.claw.stats
    }

    /// Treasures the level holds, per kind.
    pub fn treasure_census(&self) -> [u32; TREASURE_KINDS] {
        logic::pickup::census(&self.level)
    }

    /// Sound and music cues of the last tick.
    pub fn last_audio(&self) -> &[AudioCue] {
        &self.last_audio
    }

    /// Game events of the last tick.
    pub fn last_events(&self) -> &[GameEvent] {
        &self.last_events
    }

    /// Ticks run so far.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Settings.
    pub fn config(&self) -> &SimConfig {
        &self.cfg
    }

    /// The level.
    pub fn level(&self) -> &SimLevel {
        &self.level
    }

    /// Claw's object.
    pub fn player(&self) -> Option<&Object> {
        self.player.and_then(|id| self.objects.get(id))
    }

    /// Claw's data.
    pub fn claw(&self) -> &Claw {
        &self.claw
    }

    /// The camera.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// The point the view is centred on (before the plane clamp).
    pub fn camera_point(&self) -> (i32, i32) {
        self.camera_point
    }

    /// The main plane's view for the logic view.
    pub fn view(&self) -> PlaneView {
        let v = &self.cfg.view;
        PlaneView::new(
            self.level.plane(),
            self.camera_point,
            (v.logic_width, v.logic_height),
        )
    }

    /// All objects.
    pub fn objects(&self) -> &Container {
        &self.objects
    }

    /// The activation index.
    pub fn activation(&self) -> &Activation {
        &self.activation
    }

    /// Pair-collision effects of the last tick.
    pub fn collision_events(&self) -> &[CollisionEvent] {
        &self.events
    }

    /// Load and run diagnostics.
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    /// The input of the last tick, as given to [`Sim::step`].
    pub fn input(&self) -> TickInput {
        self.input
    }

    /// The input Claw's logic saw in the last tick: [`Sim::input`] without
    /// the latched keys (see [`Sim::latch_keys`]).
    pub fn claw_input(&self) -> TickInput {
        self.claw_input
    }

    /// Every key down at the next tick counts as already pressed: it is
    /// neither held nor pressed for Claw until a tick finds it released, and
    /// counts again from its next press. Play starts this way (a key held
    /// when the level starts does nothing until it is released and pressed
    /// again), and so does the return from a screen over play.
    pub fn latch_keys(&mut self) {
        self.latched = Buttons(u32::MAX);
    }

    /// The keys latched now (all bits set after [`Sim::latch_keys`], until
    /// the next tick clears the ones that are up).
    pub fn latched(&self) -> Buttons {
        self.latched
    }

    /// Lifts the latch: keys down at the next tick count at once, as if
    /// none had been down when play started (tools and tests that press
    /// from the first tick).
    pub fn unlatch_keys(&mut self) {
        self.latched = Buttons::NONE;
    }

    /// True when the last tick was a frame pulse tick (see [`PULSE_MS`]).
    pub fn pulse(&self) -> bool {
        self.pulse_left == 0
    }

    /// The random stream.
    pub fn rng(&self) -> &Rng {
        &self.rng
    }

    /// The state of the shared random stream: after a step, the state the
    /// next tick starts from. Traces record it in their `rng` column.
    pub fn rng_state(&self) -> u32 {
        self.rng.state()
    }

    /// The state the level's set-up left the shared random stream in:
    /// [`LEVEL_LOAD_SEED`] advanced by every draw the objects made while
    /// the level loaded. In the original this is the stream's state at
    /// time zero; [`SimConfig::seed`] sets the state the first tick
    /// starts from instead.
    pub fn rng_after_set_up(&self) -> u32 {
        self.set_up_rng
    }

    /// Sets the state of the shared random stream (tools that start a
    /// stretch of play at a settled point, as the reference runs of a
    /// segment do: the stream is set at the segment's time zero).
    pub fn set_rng_state(&mut self, state: u32) {
        self.rng = Rng::new(state);
    }

    /// The sound-only random stream.
    pub fn sound_rng(&self) -> &Rng {
        &self.sound_rng
    }

    /// Points the camera at a scripted point, or back at Claw with `None`.
    pub fn set_camera_point(&mut self, point: Option<(i32, i32)>) {
        self.camera_override = point;
    }

    /// Shakes the camera for `ms` milliseconds.
    pub fn start_shake(&mut self, ms: u64) {
        let now = self.tick * u64::from(TICK_MS);
        self.camera.start_shake(now, ms);
    }

    /// Sets the checkpoint Claw respawns at.
    pub fn set_checkpoint(&mut self, point: Option<(i32, i32)>) {
        self.claw.checkpoint = point;
    }

    /// Puts Claw back at the checkpoint (or the spawn point) at once, as
    /// the death and restart paths do.
    pub fn respawn(&mut self) {
        let Some(id) = self.player else { return };
        if let Some(o) = self.objects.get_mut(id) {
            o.state = player::state::RESPAWN;
        }
        self.run_logic(id);
    }

    /// Ends Claw's current life, as the in-game menu's end life entry does.
    ///
    /// The death happens at the start of the next tick, before any logic
    /// of that tick runs: when Claw's health is above 0 he dies the timer
    /// death from whatever he is doing (standing, climbing, carrying
    /// something, which he lets go of), falls off the screen and loses a
    /// life as for any other death, so the game ends after the last life.
    /// While he is already dying it does nothing. Calling it again before
    /// the next tick changes nothing.
    pub fn end_life(&mut self) {
        self.end_life = true;
    }

    /// True when [`Sim::end_life`] waits for the next tick.
    pub fn end_life_pending(&self) -> bool {
        self.end_life
    }

    /// Lets `ms` of a help screen's time reach the dynamite fuse, at the
    /// boundary before the next tick, when play comes back from the help
    /// screen (with [`SimConfig::fuse_clock`] at its original default).
    ///
    /// The game time then counts as the game time of the last tick run
    /// plus `ms`, for the fuse only: every lit stick of dynamite whose fuse
    /// has burnt by then goes off at once, as if it had gone off in the
    /// last tick before the screen (its blast hurts in the 8th tick after
    /// the screen, against the 9th after a stick that goes off in a tick).
    /// Nothing else sees that time and the game time stays as it was; no
    /// other object runs and nothing is drawn from the random stream. With
    /// [`FuseClock::Frozen`] it does nothing, and the pause and the
    /// in-game menu never call it: their time never reaches play.
    pub fn pass_menu_time(&mut self, ms: u32) {
        if self.cfg.fuse_clock == FuseClock::Frozen || ms == 0 {
            return;
        }
        let at = self.tick.saturating_sub(1) * u64::from(TICK_MS) + u64::from(ms);
        let (mut runner, objects, _) = self.parts();
        {
            let mut w = runner.world(objects);
            logic::projectile::pass_fuse_time(&mut w, at);
        }
        runner.list_spawned(objects);
    }

    /// Moves Claw to (x, y) at once (tools and tests); the move is drawn
    /// without interpolation.
    pub fn teleport_player(&mut self, x: i32, y: i32) {
        let Some(id) = self.player else { return };
        if let Some(o) = self.objects.get_mut(id) {
            o.x = x;
            o.y = y;
            o.teleported = true;
        }
    }

    /// Forces Claw into `state` through the state table's exit and entry
    /// handlers (tools and tests).
    pub fn set_player_state(&mut self, state: i32) {
        let Some(id) = self.player else { return };
        let (mut runner, objects, _) = self.parts();
        runner.with_claw(objects, id, |c| player::transition(c, state));
        runner.list_spawned(objects);
    }

    /// Runs `f` with a [`World`] over this level (tools and tests): the
    /// same context the level logics get. Objects it spawns join the live
    /// list when it returns; cues and events it adds go into the next
    /// tick's snapshot.
    pub fn with_world<R>(&mut self, f: impl FnOnce(&mut World) -> R) -> R {
        let (mut runner, objects, _) = self.parts();
        let r = {
            let mut w = runner.world(objects);
            f(&mut w)
        };
        runner.list_spawned(objects);
        r
    }

    /// Adds an object at run time (live at once when `flags` lacks
    /// [`flags::UNLISTED`]).
    pub fn spawn(&mut self, obj: Object) -> ObjectId {
        let id = self.objects.add(obj);
        self.objects.insert(id);
        id
    }
}

fn renderable(o: &Object) -> Option<Renderable> {
    if o.image_set.is_empty() || o.draw_flags & draw_flags::HIDDEN != 0 {
        return None;
    }
    Some(Renderable {
        id: o.serial,
        image_set_key: o.image_set.clone(),
        frame: o.frame,
        x: f64::from(o.x),
        y: f64::from(o.y),
        z: o.z,
        mirror: o.draw_flags & draw_flags::MIRROR != 0,
        invert: o.draw_flags & draw_flags::INVERT != 0,
        flash: o.draw_flags & draw_flags::FLASH != 0,
        draw_mode: DrawMode::NORMAL,
        teleported: o.teleported,
    })
}

#[cfg(test)]
mod tests {
    use claw_frame::{SoundChannel, SoundCue};

    use super::*;
    use crate::tiles::TileMap;

    fn flat() -> SimLevel {
        let mut rows = vec![".".repeat(60); 15];
        rows.push("#".repeat(60));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600))
    }

    /// Runs `f` on Claw's handler context, as his logic gets it.
    fn with_ctx<R>(sim: &mut Sim, f: impl FnOnce(&mut Ctx) -> R) -> R {
        let id = sim.player.unwrap();
        let (mut runner, objects, _) = sim.parts();
        let r = runner.with_claw(objects, id, f).unwrap();
        runner.list_spawned(objects);
        r
    }

    #[test]
    fn claws_code_sees_the_logics_switch_the_level_number_and_attempts() {
        let mut level = flat();
        level.number = 4;
        for logics in [true, false] {
            let cfg = SimConfig {
                level_logics: logics,
                ..SimConfig::default()
            };
            let mut sim = Sim::new(level.clone(), cfg);
            sim.claw.stats.count_attempt();
            let seen = with_ctx(&mut sim, |c| {
                (c.level_logics, c.level_number(), c.attempt())
            });
            assert_eq!(seen, (logics, 4, 1));
        }
    }

    #[test]
    fn lines_claws_code_asks_for_are_said_after_his_handler() {
        let mut level = flat();
        level.sounds.insert("CLAW_LINE".into(), Some(900));
        level.sounds.insert("CLAW_OTHER".into(), Some(900));
        let mut sim = Sim::new(level, SimConfig::default());
        with_ctx(&mut sim, |c| {
            c.say("CLAW_LINE", false);
            c.say("NOT_REGISTERED", false);
            // Refused: the first line still runs.
            c.say("CLAW_OTHER", false);
            assert_eq!(c.out.voice_requests().len(), 3);
            assert!(c.out.audio().is_empty(), "nothing is said during the call");
        });
        assert!(sim.with_world(|w| logic::sound::claw_is_speaking(w)));
        let snap = sim.step(TickInput::default());
        let voice: Vec<&SoundCue> = snap
            .audio
            .iter()
            .filter_map(|c| match c {
                AudioCue::Play(s) if s.channel == SoundChannel::Voice => Some(s),
                _ => None,
            })
            .collect();
        assert_eq!(voice.len(), 1);
        assert_eq!(voice[0].key, "CLAW_LINE");
        // A forced line is said over the running one.
        with_ctx(&mut sim, |c| c.say("CLAW_OTHER", true));
        let snap = sim.step(TickInput::default());
        assert!(snap.audio.iter().any(
            |c| matches!(c, AudioCue::Play(s) if s.key == "CLAW_OTHER" && s.channel == SoundChannel::Voice)
        ));
    }

    #[test]
    fn a_line_left_waiting_is_said_by_the_end_of_the_tick() {
        let mut level = flat();
        level.sounds.insert("CLAW_LINE".into(), Some(900));
        let mut sim = Sim::new(level, SimConfig::default());
        idle_ticks(&mut sim, 2);
        let claw = sim.player.unwrap();
        // Asked for outside Claw's handler, in a tick his handler skips.
        sim.with_world(|w| {
            w.out.say("CLAW_LINE", false);
            w.objects.get_mut(claw).unwrap().frame_delay = 1;
        });
        assert_eq!(sim.out.voice_requests().len(), 1);
        let snap = sim.step(TickInput::default());
        assert!(
            snap.audio.iter().any(|c| matches!(
                c,
                AudioCue::Play(s) if s.key == "CLAW_LINE" && s.channel == SoundChannel::Voice
            )),
            "the line is said in the tick it was left waiting in"
        );
        assert!(sim.out.voice_requests().is_empty(), "nothing is left over");
        assert!(sim.clone().out.voice_requests().is_empty());
    }

    fn idle_ticks(sim: &mut Sim, n: usize) {
        for _ in 0..n {
            sim.step(TickInput::default());
        }
    }

    #[test]
    fn the_sound_seed_depends_on_the_seed_and_the_level() {
        assert_eq!(sound_seed(1, 1), sound_seed(1, 1));
        assert_ne!(sound_seed(1, 1), sound_seed(1, 2));
        assert_ne!(sound_seed(1, 1), sound_seed(2, 1));
        assert_ne!(sound_seed(1, 1), 1);
    }
}
