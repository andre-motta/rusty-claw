//! The boss fight's stage: everything around a boss that is not the boss
//! himself. Every boss level uses the same pieces; the boss only waits for
//! the fight flag and reports his defeat (see [`super::world_boss`]).
//!
//! # BossWarp
//!
//! The door into the boss's room: a warp pickup that stays after use. On
//! its first call it shows `GAME_BOSSWARP` (or `GAME_POWERUP` when the
//! level lacks it), plays its animation from then on, and touches Claw
//! with its picture's box grown by 6 px left and right, 10 px up and 2 px
//! down. Claw's side does the move (see [`crate::player::hit`]): to the
//! record's (speed x, speed y), a warp taken in a jump turning into a fall.
//! The move ends with the warp's own set-up, in no game time (see
//! [`crate::Sim::step`]): the camera jumps to Claw and the objects around
//! him wake and run their first call, so the stager next to the target is
//! armed before Claw's first tick there. A placed BossWarp at depth 0 is
//! drawn at depth 1000.
//!
//! # BossStager
//!
//! The trigger next to where the warp drops Claw, and the introduction it
//! plays. On its first call, which comes when it wakes (as soon as Claw is
//! near; through the door, in the door's set-up), it switches the music to
//! the level's boss track, whatever the trigger does later. It is hidden, nothing acts on it, and it touches
//! Claw with the box [`TRIGGER_BOX`] around its position: with Claw's
//! standing box facing right, Claw's x within 31 px left and 38 px right of
//! it and his y within 68 px above and 61 px below.
//!
//! The touch (BossStagerAttack) accepts Claw only while he stands or walks
//! (physics type 1 or 2); otherwise nothing changes and the trigger stays
//! armed for the next collision pass. Accepted, it makes Claw's position
//! his checkpoint (with [`GameEvent::Checkpoint`], as a flag does), adds
//! [`CHECKPOINT_BONUS_MS`] to the checkpoint time, and fires once. This is
//! the only check on Claw: a jump pressed on his first step in the room
//! carries him out of the box in the air, so the introduction never runs
//! (the intro skip). Pressed one tick later it is too late: the trigger
//! fired on that first tick. The fight then has no freeze, no health bar and no
//! checkpoint, and the flag stays 0 for the rest of the level.
//!
//! The introduction, in the stager's later calls (each step's delay is the
//! time before its next call, so a delay of D ms is `ceil(D / 18)` ticks):
//!
//! | State | What it does | Delay | Next |
//! | --- | --- | --- | --- |
//! | 8003 | flag 1; Claw frozen at once; the health bar; the camera to the boss | 950 | 8006 |
//! | 8006 | the boss says `LEVEL_STAGING_ENEMY` | its length + 100 | 8008 |
//! | 8008 | the camera back to Claw | 850 | 8005 |
//! | 8005 | Claw says `LEVEL_STAGING_CLAW` | its length + 100 | 8007 |
//! | 8007 | the camera back to Claw (again) | 200 | 8000 |
//! | 8000 | the name banner at (320, -220), `GAME_SDPT1` | - | 8001 |
//! | 8001 | the banner drops 200 px/s; at y 160: a 750 ms shake, `GAME_SDPT2`, `LEVEL_DEFEAT` | 1500 at the end | 8002 |
//! | 8002 | the banner leaves right at 600 px/s; at x 960 it is hidden | - | 8004 |
//! | 8004 | flag 2; the stager is removed | - | - |
//!
//! Claw's own next call sees the flag at 2 and he stands again. The banner
//! moves in whole pixels per tick on the 640x480 view and is shown through
//! [`World::set_boss_banner`]. The shake draws from the shared random
//! stream; the stager itself draws nothing. The stager runs in the update
//! order of its depth: one placed above Claw's depth (4000) runs after him,
//! so the freeze lands after his logic of that tick; one placed below runs
//! before him until the banner moves it to depth 8000.
//!
//! [`BossStageData::claw_first`] swaps the two lines (Claw speaks first and
//! the camera visits the boss between them); no level uses it.
//!
//! # BossHealthMeter
//!
//! Made by the stager in state 8003 and run at once: it copies the current
//! boss's health as the full bar ([`World::set_boss_meter`]) and every tick
//! reports his health against it. The tick after the current boss is gone
//! (his body removed, not his death) it hides the bar and removes itself.
//! It is not a level object: it never goes dormant.

use claw_frame::output::BossMeter;
use claw_frame::{AudioCue, GameEvent, MusicTrack, SoundChannel};

use super::world_boss::fight;
use super::{ObjectData, World, new_object, pickup, sound};
use crate::level::runtime_image_set;
use crate::object::{ObjectId, Rect, draw_flags, engine_state, flags, types};
use crate::physics::ptype;
use crate::tuning::TICK_MS;

/// Logic name of the boss room's door.
pub const WARP: &str = "BossWarp";
/// Logic name of the introduction's trigger.
pub const STAGER: &str = "BossStager";
/// Name of the trigger's touch handler.
pub const STAGER_ATTACK: &str = "BossStagerAttack";
/// Logic name of the boss's health bar.
pub const METER: &str = "BossHealthMeter";

/// Logic names this family runs.
pub const NAMES: &[&str] = &[WARP, STAGER, STAGER_ATTACK, METER];

/// The door's picture.
pub const WARP_IMAGE: &str = "GAME_BOSSWARP";
/// The door's picture on a level without [`WARP_IMAGE`].
pub const WARP_FALLBACK_IMAGE: &str = "GAME_POWERUP";
/// The door's animation.
pub const WARP_ANIM: &str = "GAME_BOSSWARP";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[WARP_IMAGE, WARP_FALLBACK_IMAGE];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[WARP_ANIM];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// Depth of a placed door whose record gives depth 0.
pub const WARP_Z: i32 = 1000;
/// The trigger's touch box around the stager.
pub const TRIGGER_BOX: Rect = Rect::new(-16, -16, 16, 16);
/// What the stager is, for collisions (the same bit as a checkpoint flag,
/// which nothing that hits Claw carries).
pub const STAGER_TYPE: u32 = types::CHECKPOINT;
/// Time the accepted trigger adds to the checkpoint time, ms.
pub const CHECKPOINT_BONUS_MS: i32 = 120_000;

/// Sound key of the boss's line.
pub const ENEMY_LINE: &str = "LEVEL_STAGING_ENEMY";
/// Sound key of Claw's line.
pub const CLAW_LINE: &str = "LEVEL_STAGING_CLAW";
/// Sound of the banner's appearance.
pub const BANNER_SOUND: &str = "GAME_SDPT1";
/// Sound of the banner's landing.
pub const THUD_SOUND: &str = "GAME_SDPT2";
/// Played right after [`THUD_SOUND`].
pub const DEFEAT_SOUND: &str = "LEVEL_DEFEAT";
/// The banner's image set, drawn by the status bar.
pub const BANNER_IMAGE: &str = "LEVEL_BOSSNAME";

/// Delay after the start of the introduction, ms.
pub const START_DELAY_MS: u32 = 950;
/// Added to a line's length for the delay after it, ms.
pub const LINE_PAD_MS: u32 = 100;
/// Delay after the camera is given back to Claw, ms.
pub const CAMERA_BACK_DELAY_MS: u32 = 850;
/// Delay before the banner, ms.
pub const BANNER_DELAY_MS: u32 = 200;
/// Delay before the banner, ms, with the lines swapped.
pub const SWAPPED_BANNER_DELAY_MS: u32 = 500;
/// Delay of the banner after its landing, ms.
pub const LANDED_DELAY_MS: u32 = 1500;
/// The shake of the banner's landing, ms.
pub const THUD_SHAKE_MS: u32 = 750;
/// The banner's falling speed, px/s.
pub const BANNER_DROP_SPEED: u32 = 200;
/// The banner's leaving speed, px/s.
pub const BANNER_LEAVE_SPEED: u32 = 600;
/// The view the banner moves on.
pub const BANNER_VIEW: (i32, i32) = (640, 480);
/// The banner's start: centred, above the view.
pub const BANNER_START: (i32, i32) = (BANNER_VIEW.0 / 2, -220);
/// The banner lands at this y: a third of the view's height.
pub const BANNER_LAND_Y: i32 = BANNER_VIEW.1 / 3;
/// The banner is hidden at this x: one and a half view widths.
pub const BANNER_GONE_X: i32 = BANNER_VIEW.0 * 3 / 2;
/// Depth the banner moves the stager to.
pub const BANNER_Z: i32 = 8000;
/// Depth of the health bar.
pub const METER_Z: i32 = 9988;

/// Logic states of the stage's pieces.
pub mod stage_state {
    /// First call.
    pub const INIT: i32 = 0;
    /// The stager waits for Claw; the door and the health bar run.
    pub const ARMED: i32 = 5;
    /// The banner appears.
    pub const BANNER: i32 = 8000;
    /// The banner drops.
    pub const BANNER_DROP: i32 = 8001;
    /// The banner leaves.
    pub const BANNER_LEAVE: i32 = 8002;
    /// The introduction starts: Claw is frozen.
    pub const START: i32 = 8003;
    /// The fight starts.
    pub const FIGHT: i32 = 8004;
    /// Claw's line.
    pub const CLAW_LINE: i32 = 8005;
    /// The boss's line.
    pub const BOSS_LINE: i32 = 8006;
    /// After Claw's line.
    pub const AFTER_CLAW_LINE: i32 = 8007;
    /// The camera comes back to Claw.
    pub const CAMERA_BACK: i32 = 8008;
}

/// The data of a piece of the stage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BossStageData {
    /// The health bar: the health a full bar stands for.
    pub meter_max: i32,
    /// The stager: the banner's centre on the 640x480 view while it shows.
    pub banner: Option<(i32, i32)>,
    /// The stager: Claw says his line first and the camera visits the boss
    /// between the lines. Off on every level.
    pub claw_first: bool,
}

fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut BossStageData> {
    let o = w.objects.get_mut(id)?;
    if !matches!(o.data, ObjectData::BossStage(_)) {
        o.data = ObjectData::BossStage(BossStageData::default());
    }
    match &mut o.data {
        ObjectData::BossStage(d) => Some(d),
        _ => None,
    }
}

/// The stage data of `id`, if it has any.
pub fn data_of<'a>(w: &'a World, id: ObjectId) -> Option<&'a BossStageData> {
    match &w.objects.get(id)?.data {
        ObjectData::BossStage(d) => Some(d),
        _ => None,
    }
}

/// Ticks of game time a delay of `ms` lasts: the logic runs again this
/// many ticks after the one that set it.
pub const fn delay_ticks(ms: u32) -> u32 {
    if ms == 0 { 1 } else { ms.div_ceil(TICK_MS) }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    if o.flags & flags::DESTROY != 0
        || matches!(o.state, engine_state::DESTROYING | engine_state::SLEEPING)
    {
        return;
    }
    let logic = o.logic.clone();
    match &*logic {
        WARP => warp_tick(w, id),
        STAGER => stager_tick(w, id),
        METER => meter_tick(w, id),
        _ => {}
    }
}

/// `attacker` hit `id`. Nothing hits the stage.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`: the stager's touch on Claw.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let is_stager = w.objects.get(id).is_some_and(|o| &*o.logic == STAGER);
    if is_stager {
        stager_attack(w, id, victim);
    }
}

// ------------------------------------------------------------ the door

fn warp_tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    match o.state {
        stage_state::INIT => warp_init(w, id),
        stage_state::ARMED => {
            w.advance_anim(id);
        }
        _ => {}
    }
}

fn warp_init(w: &mut World, id: ObjectId) {
    let image = if w.image_frames(WARP_IMAGE).is_some() {
        WARP_IMAGE
    } else {
        WARP_FALLBACK_IMAGE
    };
    if let Some(o) = w.objects.get_mut(id) {
        if o.z == 0 {
            o.z = WARP_Z;
            o.flags |= flags::RESORT;
        }
        o.image_set = runtime_image_set(image);
    }
    w.set_anim(id, WARP_ANIM);
    let picture = pickup::picture_box(w, w.objects.get(id).expect("checked above"));
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.type_flags = types::SPECIAL;
    o.attack_rect = pickup::padded(picture);
    let r = o
        .record
        .get_or_insert_with(|| Box::new([0; claw_assets::wwd::OBJECT_INTS]));
    r[crate::player::hit::POINTS_FIELD] = pickup::WARP;
    // Any non-zero value keeps the door after use.
    r[crate::player::hit::SMARTS_FIELD] = 9999;
    o.data = ObjectData::BossStage(BossStageData::default());
    o.state = stage_state::ARMED;
}

// ------------------------------------------------------------ the stager

fn stager_init(w: &mut World, id: ObjectId) {
    w.cue(AudioCue::Music {
        track: MusicTrack::Boss,
        looped: true,
    });
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.draw_flags |= draw_flags::HIDDEN;
    o.flags |= flags::SAFE;
    o.attack_rect = TRIGGER_BOX;
    o.type_flags = STAGER_TYPE;
    o.attack_type_flags = types::PLAYER;
    o.attack_handler = true;
    o.state = stage_state::ARMED;
    data_mut(w, id);
}

/// The trigger's touch: accepts Claw only while he stands or walks.
fn stager_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    if Some(victim) != w.claw_id {
        return;
    }
    let Some(o) = w.objects.get(id) else { return };
    if o.state != stage_state::ARMED || o.flags & flags::NO_COLLIDE != 0 {
        return;
    }
    let Some(claw) = w.objects.get(victim) else {
        return;
    };
    if !matches!(claw.physics_type, ptype::STAND | ptype::WALK) {
        return;
    }
    let (x, y) = (claw.x, claw.y);
    w.set_checkpoint(x, y);
    w.event(GameEvent::Checkpoint { x, y });
    w.add_checkpoint_time(CHECKPOINT_BONUS_MS);
    if let Some(o) = w.objects.get_mut(id) {
        o.flags |= flags::NO_COLLIDE;
        o.state = stage_state::START;
    }
}

fn set_delay(w: &mut World, id: ObjectId, ms: u32, next: i32) {
    if let Some(o) = w.objects.get_mut(id) {
        o.time_delay = ms;
        o.state = next;
    }
}

fn boss_point(w: &World) -> Option<(i32, i32)> {
    w.current_boss()
        .and_then(|b| w.objects.get(b))
        .map(|b| (b.x, b.y))
}

fn line_delay(w: &World, key: &str) -> u32 {
    w.sound_ms(key).unwrap_or(0) + LINE_PAD_MS
}

fn show_banner(w: &mut World, id: ObjectId, banner: Option<(i32, i32)>) {
    if let Some(d) = data_mut(w, id) {
        d.banner = banner;
    }
    w.set_boss_banner(banner);
}

fn stager_tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    let state = o.state;
    let claw_first = data_of(w, id).is_some_and(|d| d.claw_first);
    match state {
        stage_state::INIT => stager_init(w, id),
        stage_state::START => {
            w.set_boss_fight(fight::STAGING);
            w.freeze_claw_now();
            spawn_meter(w);
            if !claw_first && let Some(p) = boss_point(w) {
                w.set_camera_override(Some(p));
            }
            let next = if claw_first {
                stage_state::CLAW_LINE
            } else {
                stage_state::BOSS_LINE
            };
            set_delay(w, id, START_DELAY_MS, next);
        }
        stage_state::BOSS_LINE => {
            let delay = match w.current_boss() {
                Some(boss) => {
                    sound::enemy_say(w, boss, ENEMY_LINE, true);
                    line_delay(w, ENEMY_LINE)
                }
                None => 0,
            };
            set_delay(w, id, delay, stage_state::CAMERA_BACK);
        }
        stage_state::CAMERA_BACK => {
            w.set_camera_override(None);
            if claw_first {
                set_delay(w, id, SWAPPED_BANNER_DELAY_MS, stage_state::BANNER);
            } else {
                set_delay(w, id, CAMERA_BACK_DELAY_MS, stage_state::CLAW_LINE);
            }
        }
        stage_state::CLAW_LINE => {
            sound::claw_say(w, CLAW_LINE, true);
            let delay = line_delay(w, CLAW_LINE);
            set_delay(w, id, delay, stage_state::AFTER_CLAW_LINE);
        }
        stage_state::AFTER_CLAW_LINE => {
            if claw_first {
                if let Some(p) = boss_point(w) {
                    w.set_camera_override(Some(p));
                }
                set_delay(w, id, CAMERA_BACK_DELAY_MS, stage_state::BOSS_LINE);
            } else {
                w.set_camera_override(None);
                set_delay(w, id, BANNER_DELAY_MS, stage_state::BANNER);
            }
        }
        stage_state::BANNER => {
            if let Some(o) = w.objects.get_mut(id) {
                // A screen piece from now on: it never goes dormant.
                o.flags &= !flags::WORLD;
                if o.z != BANNER_Z {
                    o.z = BANNER_Z;
                    o.flags |= flags::RESORT;
                }
                o.state = stage_state::BANNER_DROP;
            }
            show_banner(w, id, Some(BANNER_START));
            sound::play(w, BANNER_SOUND, SoundChannel::Effect);
        }
        stage_state::BANNER_DROP => {
            let (x, mut y) = data_of(w, id)
                .and_then(|d| d.banner)
                .unwrap_or(BANNER_START);
            y += (TICK_MS * BANNER_DROP_SPEED / 1000) as i32;
            if y >= BANNER_LAND_Y {
                y = BANNER_LAND_Y;
                w.shake(THUD_SHAKE_MS);
                sound::play(w, THUD_SOUND, SoundChannel::Effect);
                sound::play(w, DEFEAT_SOUND, SoundChannel::Effect);
                set_delay(w, id, LANDED_DELAY_MS, stage_state::BANNER_LEAVE);
            }
            show_banner(w, id, Some((x, y)));
        }
        stage_state::BANNER_LEAVE => {
            let (mut x, y) = data_of(w, id)
                .and_then(|d| d.banner)
                .unwrap_or((BANNER_START.0, BANNER_LAND_Y));
            x += (TICK_MS * BANNER_LEAVE_SPEED / 1000) as i32;
            if x >= BANNER_GONE_X {
                if let Some(d) = data_mut(w, id) {
                    d.banner = Some((x, y));
                }
                w.set_boss_banner(None);
                if let Some(o) = w.objects.get_mut(id) {
                    o.state = stage_state::FIGHT;
                }
            } else {
                show_banner(w, id, Some((x, y)));
            }
        }
        stage_state::FIGHT => {
            w.set_boss_fight(fight::ON);
            w.destroy(id);
        }
        _ => {}
    }
}

// ------------------------------------------------------------ the health bar

fn spawn_meter(w: &mut World) -> ObjectId {
    let mut o = new_object(METER, 0, 0, 0);
    // A screen piece: not a level object, so it never goes dormant.
    o.flags = flags::RUN_AT_CREATION | flags::NO_COLLIDE;
    o.draw_flags = draw_flags::HIDDEN;
    o.data = ObjectData::BossStage(BossStageData::default());
    w.spawn(o)
}

fn boss_health(w: &World) -> Option<i32> {
    w.current_boss()
        .and_then(|b| w.objects.get(b))
        .map(|b| b.health)
}

fn meter_tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    match o.state {
        stage_state::INIT => {
            let max = boss_health(w).unwrap_or(0);
            if let Some(o) = w.objects.get_mut(id) {
                if o.z != METER_Z {
                    o.z = METER_Z;
                    o.flags |= flags::RESORT;
                }
                o.state = stage_state::ARMED;
            }
            if let Some(d) = data_mut(w, id) {
                d.meter_max = max;
            }
            if let Some(value) = boss_health(w) {
                w.set_boss_meter(Some(BossMeter { value, max }));
            }
        }
        stage_state::ARMED => match boss_health(w) {
            Some(value) => {
                let max = data_of(w, id).map_or(0, |d| d.meter_max);
                w.set_boss_meter(Some(BossMeter { value, max }));
            }
            None => {
                w.set_boss_meter(None);
                w.destroy(id);
            }
        },
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_last_whole_ticks() {
        assert_eq!(delay_ticks(START_DELAY_MS), 53);
        assert_eq!(delay_ticks(2820 + LINE_PAD_MS), 163);
        assert_eq!(delay_ticks(CAMERA_BACK_DELAY_MS), 48);
        assert_eq!(delay_ticks(1750 + LINE_PAD_MS), 103);
        assert_eq!(delay_ticks(BANNER_DELAY_MS), 12);
        assert_eq!(delay_ticks(LANDED_DELAY_MS), 84);
        assert_eq!(delay_ticks(18), 1);
    }

    #[test]
    fn the_banner_moves_in_whole_pixels() {
        assert_eq!(TICK_MS * BANNER_DROP_SPEED / 1000, 3);
        assert_eq!(TICK_MS * BANNER_LEAVE_SPEED / 1000, 10);
        assert_eq!(
            (BANNER_START, BANNER_LAND_Y, BANNER_GONE_X),
            ((320, -220), 160, 960)
        );
    }
}
