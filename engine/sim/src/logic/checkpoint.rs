//! Checkpoint flags and the two save points of a level.
//!
//! A flag waits until Claw touches it. Then it becomes his respawn point,
//! [`RESPAWN_RISE`] pixels above the flag's position, the game reports
//! [`GameEvent::Checkpoint`] with that point, and the flag rises
//! (`GAME_CHECKPOINT_RISE`) and then waves for good
//! (`GAME_CHECKPOINT_WAVE`). Each flag fires once.
//!
//! The two save points (`FirstSuperCheckpoint` and `SecondSuperCheckpoint`)
//! are flags whose touch box reaches [`SAVE_POINT_REACH`] pixels higher,
//! and which also save the game: the game reports
//! [`GameEvent::SavePoint`] with their slot (1 or 2) and shows the
//! game-saved message ([`MESSAGE_SAVED`]). After a cheat the game is not
//! saved and the not-saved message ([`MESSAGE_NOT_SAVED`]) shows instead.
//! Every message shows for [`MESSAGE_MS`].
//!
//! A level started from a save point has that save point's flag removed
//! (see [`crate::player::progress`]).

use claw_frame::GameEvent;

use super::World;
use crate::logic::ObjectData;
use crate::object::{ObjectId, Rect, engine_state, flags, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "Checkpoint",
    "FirstSuperCheckpoint",
    "SecondSuperCheckpoint",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["GAME_CHECKPOINTFLAG", "GAME_SUPERCHECKPOINT"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["GAME_CHECKPOINT"];

/// Animation of a flag rising when touched.
pub const RISE_ANIM: &str = "GAME_CHECKPOINT_RISE";
/// Animation of a raised flag.
pub const WAVE_ANIM: &str = "GAME_CHECKPOINT_WAVE";

/// The respawn point is this far above the flag's position, px.
pub const RESPAWN_RISE: i32 = 32;
/// A save point's touch box reaches this much higher than a flag's, px.
pub const SAVE_POINT_REACH: i32 = 64;
/// Touch box of a checkpoint flag, relative to its position: the box of
/// the lowered flag's picture.
pub const FLAG_BOX: Rect = Rect::new(-30, -49, -3, 62);
/// Touch box of a save point's flag before its extra reach.
pub const SAVE_POINT_BOX: Rect = Rect::new(-30, -59, -3, 62);

/// Message id of "Your game has been saved".
pub const MESSAGE_SAVED: u32 = 1;
/// Message id of "Your game was not saved because you cheated!".
pub const MESSAGE_NOT_SAVED: u32 = 2;
/// How long a message shows, ms.
pub const MESSAGE_MS: u32 = 5000;

/// Logic states of a flag.
pub mod flag_state {
    /// Raised and waving.
    pub const WAVING: i32 = 1;
    /// Waiting to be touched.
    pub const WAITING: i32 = 5;
    /// Rising.
    pub const RISING: i32 = 6;
}

/// A checkpoint's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckpointData {
    /// Save slot: 1 or 2 for a save point, 0 for a plain flag.
    pub slot: u8,
}

/// The save slot of a flag running `logic`, 0 for a plain flag.
pub fn slot_of(logic: &str) -> u8 {
    match logic {
        "FirstSuperCheckpoint" => 1,
        "SecondSuperCheckpoint" => 2,
        _ => 0,
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.flags & flags::DESTROY != 0 {
        return;
    }
    match o.state {
        engine_state::INIT => {
            let slot = slot_of(&o.logic);
            o.flags |= flags::SAFE;
            o.type_flags = types::CHECKPOINT;
            o.attack_type_flags = types::PLAYER;
            o.attack_rect = if slot == 0 {
                FLAG_BOX
            } else {
                let mut r = SAVE_POINT_BOX;
                r.top -= SAVE_POINT_REACH;
                r
            };
            o.attack_handler = true;
            o.data = ObjectData::Checkpoint(CheckpointData { slot });
            o.state = flag_state::WAITING;
        }
        flag_state::RISING => {
            w.advance_anim(id);
            let done = w.objects.get(id).is_some_and(|o| o.anim.is_done());
            if done {
                w.set_anim(id, WAVE_ANIM);
                if let Some(o) = w.objects.get_mut(id) {
                    o.state = flag_state::WAVING;
                }
            }
        }
        flag_state::WAVING => {
            w.advance_anim(id);
        }
        _ => {}
    }
}

/// `attacker` hit `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`: Claw touched the flag.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    if Some(victim) != w.claw_id {
        return;
    }
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state != flag_state::WAITING {
        return;
    }
    let point = (o.x, o.y - RESPAWN_RISE);
    let slot = slot_of(&o.logic);
    o.flags |= flags::NO_COLLIDE;
    o.state = flag_state::RISING;
    w.set_anim(id, RISE_ANIM);
    w.claw.checkpoint = Some(point);
    w.event(GameEvent::Checkpoint {
        x: point.0,
        y: point.1,
    });
    if slot == 0 {
        return;
    }
    if w.claw.stats.cheated {
        w.event(GameEvent::Message {
            id: MESSAGE_NOT_SAVED,
            ms: MESSAGE_MS,
        });
    } else {
        w.event(GameEvent::SavePoint { slot });
        w.event(GameEvent::Message {
            id: MESSAGE_SAVED,
            ms: MESSAGE_MS,
        });
    }
}
