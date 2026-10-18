//! Decoration: still and animated scenery in front of or behind the
//! action.
//!
//! None of it collides, makes a sound of its own or can be stood on.
//!
//! - `DoNothing` and `DoNothingNormal` keep the frame they were placed
//!   with; every call only keeps them out of pair collision.
//! - `BehindCandy` and `FrontCandy` are still pictures. Their first call
//!   gives a depth of 0 the default 990 (behind Claw) or 5100 (in front of
//!   him); later calls do nothing.
//! - `BehindAniCandy` and `FrontAniCandy` loop an animation: the one they
//!   were placed with when the level has it, otherwise `GAME_CYCLE100`,
//!   which shows every image of the set in turn for 100 ms each. They do
//!   not advance in their first call; afterwards each call advances the
//!   animation by one tick.
//! - `AniCycle` and `AniCycleNormal` loop like the animated candy (the
//!   placed animation, else `GAME_CYCLE100`), but keep the depth they
//!   were placed with, even 0. On the second level they turn the arrow
//!   signs.
//! - `GooCoverup` is a still picture of a goo surface laid over a goo
//!   pool: its first call gives a depth of 0 the default 8010, in front of
//!   Claw and the enemies, so whatever sinks into the pool disappears
//!   behind it; later calls do nothing.
//!
//! Animations that step through an image set (next, previous, first,
//! last image) need the set's first and last frame numbers, which the
//! level reports (see [`image_frames`]). [`SpriteAnim`] plays them, and
//! the platforms use it too.

use std::sync::Arc;

use super::World;
use crate::anim::{Animation, FRAME_COUNTS_CALLS};
use crate::level::SimLevel;
use crate::object::{Object, ObjectId, flags};

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "BehindAniCandy",
    "BehindCandy",
    "FrontAniCandy",
    "FrontCandy",
    "DoNothing",
    "DoNothingNormal",
    "AniCycle",
    "AniCycleNormal",
    "GooCoverup",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[CYCLE_ANIM];

/// The loop an animated decoration plays when it was placed without one:
/// every image of its set in turn, 100 ms each.
pub const CYCLE_ANIM: &str = "GAME_CYCLE100";

/// Depth given to a decoration behind the action placed at depth 0.
pub const BEHIND_Z: i32 = 990;
/// Depth given to a decoration in front of the action placed at depth 0.
pub const FRONT_Z: i32 = 5100;
/// Depth given to a goo cover-up placed at depth 0.
pub const GOO_COVERUP_Z: i32 = 8010;

/// State of a decoration after its first call.
pub const SET_UP: i32 = 5;

/// A decoration's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DecorData {
    /// First and last frame numbers of the decoration's image set, when
    /// known (see [`image_frames`]). A value present before the first call
    /// is kept.
    pub frames: Option<(i32, i32)>,
    /// The animation being looped.
    pub anim: SpriteAnim,
}

/// First and last frame numbers of the image set `set` in `level`, which
/// the animations that step through a set need; `None` when the level
/// registers no such set, in which case an object treats its placed frame
/// as the whole set (see [`frames_of`]).
pub fn image_frames(level: &SimLevel, set: &str) -> Option<(i32, i32)> {
    level.image_frames(set)
}

/// The image-set frame range `known`, else the level's for `o`'s set,
/// else `o`'s current frame alone.
pub fn frames_of(level: &SimLevel, o: &Object, known: Option<(i32, i32)>) -> (i32, i32) {
    known
        .or_else(|| image_frames(level, &o.image_set))
        .unwrap_or((o.frame, o.frame))
}

/// Plays an animation on an object's frame within its image set.
///
/// It follows the rules of the shared player: at most one element is
/// applied per call, time left over when a wait ends is dropped, an
/// element flagged to count calls waits that many calls, and a negative
/// duration holds for good. On top of that it knows the image set's first
/// and last frames, so it can play the image operations "next" and
/// "previous" (both wrapping around the set), "first" and "last", and the
/// next-element operations that wait for the set's first or last image.
/// Image operations 6 and 7 (step by several images) leave the frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpriteAnim {
    anim: Option<Arc<Animation>>,
    pending: usize,
    wait: u32,
    wait_calls: bool,
    finished: bool,
}

impl SpriteAnim {
    /// Starts `anim` from its first element (or stops playing with `None`).
    pub fn set(&mut self, anim: Option<Arc<Animation>>) {
        *self = SpriteAnim {
            anim,
            ..SpriteAnim::default()
        };
    }

    /// The animation being played.
    pub fn animation(&self) -> Option<&Arc<Animation>> {
        self.anim.as_ref()
    }

    /// Key of the animation being played.
    pub fn key(&self) -> Option<&str> {
        self.anim.as_deref().map(|a| a.key.as_str())
    }

    /// True once a finishing element has been shown for its whole
    /// duration. Without an animation (or with an empty one) it counts as
    /// finished, so a logic waiting for it never hangs on missing data.
    pub fn is_done(&self) -> bool {
        match &self.anim {
            None => true,
            Some(a) => a.frames.is_empty() || (self.finished && self.wait == 0),
        }
    }

    /// Advances by `ms` on `o`, whose image set runs from frame
    /// `first` to frame `last`: sets its frame and moves it as the applied
    /// element says. Returns the index of the element applied by this
    /// call, if one was.
    pub fn advance(&mut self, ms: u32, o: &mut Object, (first, last): (i32, i32)) -> Option<usize> {
        let anim = self.anim.as_ref()?;
        let n = anim.frames.len();
        if n == 0 {
            return None;
        }
        if self.wait != 0 {
            if self.wait_calls {
                self.wait -= 1;
                return None;
            }
            if ms < self.wait {
                self.wait -= ms;
                return None;
            }
            self.wait = 0;
        }
        if self.finished {
            return None;
        }
        let idx = self.pending.min(n - 1);
        let el = anim.frames[idx];
        let frame = o.frame;
        o.frame = match el.image_op {
            1 if frame >= last => first,
            1 => frame + 1,
            2 if frame <= first => last,
            2 => frame - 1,
            3 => el.image_index,
            4 => first,
            5 => last,
            _ => frame,
        };
        match el.move_op {
            2 => {
                let mirrored = o.draw_flags & crate::object::draw_flags::MIRROR != 0;
                o.x += if mirrored {
                    -el.operand_x
                } else {
                    el.operand_x
                };
                o.y += el.operand_y;
            }
            3 => {
                o.x = el.operand_x;
                o.y = el.operand_y;
            }
            _ => {}
        }
        // A negative duration compares as a huge unsigned wait.
        self.wait = el.duration as u32;
        self.wait_calls = el.flags & FRAME_COUNTS_CALLS != 0;
        let advance_if = |target: i32| {
            if o.frame == target {
                (idx + 1) % n
            } else {
                idx
            }
        };
        self.pending = match el.next_op & 0xFFFF {
            9 => {
                self.finished = true;
                idx
            }
            8 => 0,
            7 => {
                self.wait = 0;
                if n > 1 { 1 } else { 0 }
            }
            6 => idx,
            1 => advance_if(el.image_index),
            2 => advance_if(first),
            3 => advance_if(last),
            4 => advance_if(first + 1),
            5 => advance_if(last - 1),
            _ => (idx + 1) % n,
        };
        Some(idx)
    }
}

/// Advances the [`SpriteAnim`] of `id` (held in `anim`) by the call's
/// frame time ([`World::dt_ms`]) and plays the sounds of the element it
/// applies.
pub(crate) fn advance_sprite(
    w: &mut World,
    id: ObjectId,
    anim: &mut SpriteAnim,
    frames: (i32, i32),
) -> Option<usize> {
    let o = w.objects.get_mut(id)?;
    let applied = anim.advance(w.dt_ms, o, frames)?;
    if let Some(a) = anim.animation()
        && a.sounds_of(applied).iter().any(Option::is_some)
    {
        let a = a.clone();
        w.frame_sound(id, &a, applied);
    }
    Some(applied)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Still,
    Candy { z: i32 },
    AniCandy { z: i32 },
}

/// Depth that means "keep the placed depth" for the animated kinds.
const KEEP_Z: i32 = 0;

fn kind_of(name: &str) -> Kind {
    match name {
        "BehindCandy" => Kind::Candy { z: BEHIND_Z },
        "FrontCandy" => Kind::Candy { z: FRONT_Z },
        "GooCoverup" => Kind::Candy { z: GOO_COVERUP_Z },
        "BehindAniCandy" => Kind::AniCandy { z: BEHIND_Z },
        "FrontAniCandy" => Kind::AniCandy { z: FRONT_Z },
        "AniCycle" | "AniCycleNormal" => Kind::AniCandy { z: KEEP_Z },
        _ => Kind::Still,
    }
}

/// The animation `o` was placed with, when the level has it.
fn placed_animation(level: &SimLevel, o: &Object) -> Option<Arc<Animation>> {
    let (x, y, _) = o.editor;
    level
        .objects
        .iter()
        .find(|s| {
            s.record[0] == o.id && s.record[5] == x && s.record[6] == y && *s.logic == *o.logic
        })
        .filter(|s| !s.anim.is_empty())
        .and_then(|s| level.animations.get(&s.anim))
        .cloned()
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.flags |= flags::NO_COLLIDE;
    let kind = kind_of(&o.logic);
    match kind {
        Kind::Still => {}
        Kind::Candy { z } | Kind::AniCandy { z } if o.state != SET_UP => {
            o.state = SET_UP;
            if o.z == 0 && z != KEEP_Z {
                o.z = z;
                o.flags |= flags::RESORT;
            }
            if matches!(kind, Kind::AniCandy { .. }) {
                let known = match &o.data {
                    super::ObjectData::Decor(d) => d.frames,
                    _ => None,
                };
                let level = w.level;
                let o = w.objects.get(id).expect("checked above");
                let frames = Some(frames_of(level, o, known));
                let anim = placed_animation(level, o)
                    .or_else(|| level.animations.get(CYCLE_ANIM).cloned());
                let mut data = DecorData {
                    frames,
                    anim: SpriteAnim::default(),
                };
                data.anim.set(anim);
                if let Some(o) = w.objects.get_mut(id) {
                    o.data = super::ObjectData::Decor(data);
                }
            }
        }
        Kind::Candy { .. } => {}
        Kind::AniCandy { .. } => {
            let super::ObjectData::Decor(d) = &mut o.data else {
                return;
            };
            let mut anim = std::mem::take(&mut d.anim);
            let frames = d.frames.unwrap_or((o.frame, o.frame));
            advance_sprite(w, id, &mut anim, frames);
            if let Some(o) = w.objects.get_mut(id)
                && let super::ObjectData::Decor(d) = &mut o.data
            {
                d.anim = anim;
            }
        }
    }
}

/// `attacker` hit `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::AnimFrame;
    use crate::object::LogicKind;

    fn element(image_op: i32, next_op: i32, duration: i32) -> AnimFrame {
        AnimFrame {
            image_op,
            next_op,
            duration,
            ..AnimFrame::default()
        }
    }

    fn anim(frames: Vec<AnimFrame>) -> Arc<Animation> {
        Arc::new(Animation {
            key: "T".into(),
            frames,
            ..Animation::default()
        })
    }

    #[test]
    fn next_image_wraps_around_the_set() {
        let mut o = Object::new(0, 0, 0, 0, Arc::from("BehindAniCandy"), LogicKind::Inert);
        o.frame = 3;
        let mut p = SpriteAnim::default();
        p.set(Some(anim(vec![element(1, 0, 100)])));
        let mut seen = Vec::new();
        for _ in 0..24 {
            p.advance(18, &mut o, (1, 4));
            seen.push(o.frame);
        }
        // One image every sixth tick: 100 ms is five waits plus the tick
        // that ends the wait and applies the element again.
        let changes: Vec<i32> = seen.chunks(6).map(|c| c[0]).collect();
        assert_eq!(changes, vec![4, 1, 2, 3]);
        assert!(!p.is_done());
    }

    #[test]
    fn waiting_for_the_last_image_then_finishing() {
        let mut o = Object::new(0, 0, 0, 0, Arc::from("x"), LogicKind::Inert);
        o.frame = 1;
        let mut p = SpriteAnim::default();
        p.set(Some(anim(vec![element(1, 3, 50), element(0, 9, 0)])));
        let mut calls = 0;
        while !p.is_done() {
            p.advance(18, &mut o, (1, 3));
            calls += 1;
            assert!(calls < 100);
        }
        // Frames 2 and 3 three ticks apart, then 50 ms on the last image
        // and the finishing element.
        assert_eq!(o.frame, 3);
        assert_eq!(calls, 1 + 3 + 3);
    }
}
