//! Boss fights: what every boss level shares across its logics.
//!
//! A boss level ends with a staged fight. Several logics take part (the
//! stage that introduces the boss, the boss himself, his health bar and
//! the gem he leaves), and they talk through the level-wide state kept
//! here:
//!
//! - the fight flag ([`World::boss_fight`]): 0 when the level starts, 1
//!   while the introduction runs and 2 once the fight is on; it never goes
//!   back to 0 within a level;
//! - the current boss ([`World::current_boss`]), set when the boss is
//!   built and cleared when his body is removed;
//! - the scripted camera point ([`World::set_camera_override`]): the
//!   camera heads for it instead of following Claw, exactly as for the
//!   point Claw's own code sets;
//! - the boss's health bar and name banner in the status bar
//!   ([`World::set_boss_meter`], [`World::set_boss_banner`]);
//! - the boss's defeat ([`World::boss_defeated`]).
//!
//! Claw can also be frozen at once for the introduction
//! ([`World::freeze_claw_now`], in [`crate::player::special`]): he stands
//! still until the flag reaches 2.
//!
//! Everything here starts cleared when a level loads. The state lives in
//! the tick's [`Outbox`](super::Outbox), so Claw's code sees it too
//! ([`Outbox::boss`](super::Outbox::boss)).

use claw_frame::output::BossMeter;

use crate::logic::World;
use crate::object::ObjectId;

/// Values of the boss fight flag.
pub mod fight {
    /// No fight yet: the level's value until the introduction starts.
    pub const NONE: u8 = 0;
    /// The introduction runs.
    pub const STAGING: u8 = 1;
    /// The fight is on.
    pub const ON: u8 = 2;
}

/// The level-wide boss fight state (see the module documentation).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BossState {
    /// The fight flag, one of [`fight`].
    pub(crate) fight: u8,
    /// The current boss.
    pub(crate) current: Option<ObjectId>,
    /// The health bar.
    pub(crate) meter: Option<BossMeter>,
    /// The name banner's centre on the 640x480 view.
    pub(crate) banner: Option<(i32, i32)>,
    /// A change of the scripted camera point not yet handed to the camera:
    /// `Some(None)` releases the camera.
    pub(crate) camera: Option<Option<(i32, i32)>>,
    /// The boss whose defeat was reported first.
    pub(crate) defeated: Option<ObjectId>,
}

impl BossState {
    /// The fight flag, one of [`fight`].
    pub fn fight(&self) -> u8 {
        self.fight
    }

    /// The current boss.
    pub fn current(&self) -> Option<ObjectId> {
        self.current
    }

    /// The health bar.
    pub fn meter(&self) -> Option<BossMeter> {
        self.meter
    }

    /// The name banner's centre on the 640x480 view.
    pub fn banner(&self) -> Option<(i32, i32)> {
        self.banner
    }

    /// The boss whose defeat was reported first (see
    /// [`World::boss_defeated`]).
    pub fn defeated(&self) -> Option<ObjectId> {
        self.defeated
    }

    /// Takes the change of the scripted camera point asked for since the
    /// last call: `Some(point)` sets the camera's point (`None` releases
    /// it), `None` when nothing was asked.
    pub(crate) fn take_camera(&mut self) -> Option<Option<(i32, i32)>> {
        self.camera.take()
    }
}

impl World<'_> {
    /// The boss fight flag: [`fight::NONE`], [`fight::STAGING`] or
    /// [`fight::ON`].
    pub fn boss_fight(&self) -> u8 {
        self.out.boss.fight
    }

    /// Sets the boss fight flag.
    pub fn set_boss_fight(&mut self, value: u8) {
        self.out.boss.fight = value;
    }

    /// The current boss, if one is built.
    pub fn current_boss(&self) -> Option<ObjectId> {
        self.out.boss.current
    }

    /// Sets (or clears) the current boss.
    pub fn set_current_boss(&mut self, id: Option<ObjectId>) {
        self.out.boss.current = id;
    }

    /// Points the camera at a scripted point, or gives it back to Claw with
    /// `None`. The simulation hands the point to the camera as soon as this
    /// logic call returns, exactly as it does the point Claw's own code
    /// sets: the camera heads for it from its next call, which is in this
    /// tick when the camera has not run yet. Claw's start and respawn
    /// release it.
    pub fn set_camera_override(&mut self, point: Option<(i32, i32)>) {
        self.out.boss.camera = Some(point);
    }

    /// Shows the boss's health bar in the status bar, or hides it with
    /// `None`.
    pub fn set_boss_meter(&mut self, meter: Option<BossMeter>) {
        self.out.boss.meter = meter;
    }

    /// The boss's health bar shown now.
    pub fn boss_meter(&self) -> Option<BossMeter> {
        self.out.boss.meter
    }

    /// Shows the boss's name banner centred at `centre` on the 640x480
    /// view, or hides it with `None`.
    pub fn set_boss_banner(&mut self, centre: Option<(i32, i32)>) {
        self.out.boss.banner = centre;
    }

    /// Sets the checkpoint Claw respawns at.
    pub fn set_checkpoint(&mut self, x: i32, y: i32) {
        self.claw.checkpoint = Some((x, y));
    }

    /// The boss `id` was defeated. Only the first call of a level counts;
    /// it is kept in [`BossState::defeated`]. What the defeat then sets off
    /// (the gem and the level end) is not built yet.
    pub fn boss_defeated(&mut self, id: ObjectId) {
        let boss = &mut self.out.boss;
        if boss.defeated.is_none() {
            boss.defeated = Some(id);
        }
    }
}
