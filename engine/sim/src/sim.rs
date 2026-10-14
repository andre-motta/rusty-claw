//! The simulation: one level, stepped one 18 ms tick at a time.

use std::collections::BTreeMap;
use std::sync::Arc;

use claw_frame::{AspectPolicy, DrawMode, Point, Renderable, SimSnapshot, TickInput, ViewConfig};

use crate::activation::{Activation, ActivationConfig, ActivationMode, Centres};
use crate::camera::{Camera, Followed};
use crate::container::{CollisionEvent, Container};
use crate::level::{ObjectSpec, SimLevel};
use crate::object::{LogicKind, Object, ObjectId, Rect, draw_flags, flags};
use crate::physics::Platform;
use crate::player::{self, Claw, Ctx};
use crate::registry::{CAMERA_LOGIC, PLAYER_LOGIC, REG_ALWAYS, REG_CLASS_1, REG_CLASS_2, Registry};
use crate::rng::Rng;
use crate::tuning::{TICK_MS, Tuning};
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
    /// Seed of the game's random stream.
    pub seed: u32,
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
    tick: u64,
    input: TickInput,
    events: Vec<CollisionEvent>,
    diagnostics: Diagnostics,
}

/// What runs an object's logic: everything but the object container.
struct Runner<'a> {
    cfg: &'a SimConfig,
    level: &'a SimLevel,
    claw: &'a mut Claw,
    camera: &'a mut Camera,
    camera_point: &'a mut (i32, i32),
    camera_override: &'a mut Option<(i32, i32)>,
    rng: &'a mut Rng,
    input: TickInput,
    now_ms: u64,
    player: Option<ObjectId>,
}

impl Runner<'_> {
    fn plane(&self) -> PlaneGeometry {
        self.level.plane()
    }

    fn run(&mut self, objects: &mut Container, id: ObjectId) {
        let Some(kind) = objects.get(id).map(|o| o.kind) else {
            return;
        };
        match kind {
            LogicKind::Player => {
                let platforms = platforms(objects);
                let centre = PlaneView::new(self.plane(), *self.camera_point, (640, 480)).centre;
                let Some(o) = objects.get_mut(id) else { return };
                let mut ctx = Ctx {
                    o,
                    p: self.claw,
                    map: &self.level.map,
                    platforms: &platforms,
                    input: self.input,
                    t: &self.cfg.tuning,
                    rng: self.rng,
                    anims: &self.level.animations,
                    camera_override: self.camera_override,
                    plane_centre: centre,
                };
                player::tick(&mut ctx);
            }
            LogicKind::Camera => {
                let followed = self.player.and_then(|p| objects.get(p)).map(|o| Followed {
                    x: o.x,
                    y: o.y,
                    state: o.state,
                    physics_type: o.physics_type,
                    look_down_time: self.claw.look_down_time,
                });
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
        }
    }
}

/// Platform objects in list order.
fn platforms(objects: &Container) -> Vec<Platform> {
    let mut out = Vec::new();
    let mut cur = objects.head();
    while let Some(id) = cur {
        let o = objects.get(id).expect("listed object exists");
        if o.type_flags & 0x80 != 0 && o.hit_rect.is_set() {
            out.push(Platform {
                id,
                x: o.x,
                y: o.y,
                hit_rect: o.hit_rect,
                move_x: 0,
                move_y: 0,
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
    /// start point, dropped onto the floor below) and the camera, and wakes
    /// the objects around the start.
    pub fn new(level: impl Into<Arc<SimLevel>>, cfg: SimConfig) -> Sim {
        let level: Arc<SimLevel> = level.into();
        let plane = level.plane();
        let activation = Activation::new(cfg.activation.clone(), plane);
        let mut sim = Sim {
            rng: Rng::new(cfg.seed),
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
            events: Vec::new(),
            diagnostics: Diagnostics::default(),
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
        sim
    }

    fn load_record(&mut self, spec: &ObjectSpec, plane: PlaneGeometry) {
        let r = &spec.record;
        let (x, y) = (r[5], r[6]);
        if spec.logic.is_empty() || x < 0 || y < 0 || x >= plane.width || y >= plane.height {
            self.diagnostics.dropped_records += 1;
            return;
        }
        let (name, reg_flags) = match self.registry.get(&spec.logic) {
            Some(e) => (e.name.clone(), e.flags),
            None => {
                *self
                    .diagnostics
                    .unknown_logics
                    .entry(spec.logic.clone())
                    .or_default() += 1;
                (Arc::from(spec.logic.as_str()), 0)
            }
        };
        // Level objects run as placeholders until their logics exist; Claw
        // and the camera are created by the simulation itself, so a record
        // naming them gets a placeholder too.
        let mut o = Object::new(r[0], x, y, r[7], name, LogicKind::Inert);
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

    fn runner(&mut self) -> (Runner<'_>, &mut Container) {
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
                input: self.input,
                now_ms,
                player: self.player,
            },
            &mut self.objects,
        )
    }

    fn run_logic(&mut self, id: ObjectId) {
        let (mut runner, objects) = self.runner();
        runner.run(objects, id);
    }

    /// Advances one tick with `input` and returns the snapshot.
    pub fn step(&mut self, input: TickInput) -> SimSnapshot {
        self.input = input;
        let c = self.centres();
        self.activation.enter(&mut self.objects, c);
        self.update_pass();
        self.events = self.objects.pair_collision();
        let c = self.centres();
        let mut notify = |_: &mut Container, _: ObjectId, _: i32| {};
        self.activation.leave(&mut self.objects, c, &mut notify);
        self.tick += 1;
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

    fn update_pass(&mut self) {
        let mut destroy = Vec::new();
        let mut resort = Vec::new();
        let (mut runner, objects) = self.runner();
        let mut cur = objects.head();
        while let Some(id) = cur {
            cur = objects.next_of(id);
            let run = {
                let Some(o) = objects.get_mut(id) else {
                    continue;
                };
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
        objects.finish_pass(&destroy, &resort, &mut |_, _| {});
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
            palette_fade: None,
            camera_teleported: self.camera.teleported,
        }
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

    /// The input of the last tick.
    pub fn input(&self) -> TickInput {
        self.input
    }

    /// The random stream.
    pub fn rng(&self) -> &Rng {
        &self.rng
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
        let plane = self.level.plane();
        let centre = PlaneView::new(plane, self.camera_point, (640, 480)).centre;
        let pf = platforms(&self.objects);
        let Some(o) = self.objects.get_mut(id) else {
            return;
        };
        let mut ctx = Ctx {
            o,
            p: &mut self.claw,
            map: &self.level.map,
            platforms: &pf,
            input: self.input,
            t: &self.cfg.tuning,
            rng: &mut self.rng,
            anims: &self.level.animations,
            camera_override: &mut self.camera_override,
            plane_centre: centre,
        };
        player::transition(&mut ctx, state);
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
