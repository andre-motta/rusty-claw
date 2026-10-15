//! Animation playback.
//!
//! An animation is a list of elements. Each element sets the image, may
//! move its owner, waits for its duration and then names the next element.
//! The player applies at most one element per call and drops any time left
//! over when a wait expires, so every element is shown for at least one
//! tick and a very short element still fires its event at the 18 ms step.
//!
//! Some operations depend on the owner's image set: "next image" and
//! "previous image" wrap around the set, "first" and "last" image jump to
//! its ends, and some next-element operations wait for one of its ends.
//! [`AnimPlayer::advance_in`] plays them with the set's frame range; without
//! it, "next" and "previous" step without wrapping, "first" and "last"
//! leave the image and those next-element operations advance at once.

use std::sync::Arc;

use claw_assets::Ani;

/// One animation element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimFrame {
    /// Element flags; bit 0x1 makes the duration count calls instead of
    /// milliseconds.
    pub flags: u16,
    /// Image operation: 1 next image, 2 previous image (both wrapping
    /// around the image set), 3 the frame `image_index`, 4 the set's first
    /// image, 5 its last image.
    pub image_op: i32,
    /// Next-element operation: 0 next, 9 finished, 8 restart, 7 restart at
    /// the second element, 6 stay; 1 to 5 go to the next element only once
    /// the frame is `image_index` (1), the set's first image (2), its last
    /// (3), the first plus one (4) or the last minus one (5), and stay
    /// otherwise.
    pub next_op: i32,
    /// Move operation (2 adds the operands to the owner's position, 3 sets
    /// it).
    pub move_op: i32,
    /// Frame number in the image set.
    pub image_index: i32,
    /// Duration in milliseconds (or calls with flag 0x1).
    pub duration: i32,
    /// Event value reported to the owner's logic.
    pub event: i32,
    /// First move operand.
    pub operand_x: i32,
    /// Second move operand.
    pub operand_y: i32,
}

/// Element flag: the duration counts calls.
pub const FRAME_COUNTS_CALLS: u16 = 0x1;
/// Element flag: the element's sound is placed at its owner's position.
pub const FRAME_SOUND_POSITIONAL: u16 = 0x4;
/// Element flag: the element's sound plays only while its owner is in
/// view.
pub const FRAME_SOUND_IN_VIEW: u16 = 0x8;

/// An animation.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Animation {
    /// Registry key, for example `CLAW_STAND`.
    pub key: String,
    /// Image set the frames come from, when the file names one.
    pub image_set: String,
    /// Elements in order.
    pub frames: Vec<AnimFrame>,
    /// Sounds of each element, by index (missing entries have none). When
    /// an element has several, one is chosen at random each time it is
    /// applied. `None` stands for a name no sound is registered under: it
    /// can be chosen and then plays nothing.
    pub frame_sounds: Vec<Vec<Option<String>>>,
}

impl Animation {
    /// Converts a loaded animation file.
    pub fn from_ani(key: &str, ani: &Ani) -> Animation {
        Animation {
            key: key.to_string(),
            image_set: ani.image_set.clone(),
            frames: ani
                .frames
                .iter()
                .map(|f| AnimFrame {
                    flags: f.flags,
                    image_op: i32::from(f.field1),
                    next_op: i32::from(f.field2),
                    move_op: i32::from(f.field3),
                    image_index: i32::from(f.image_index),
                    duration: i32::from(f.duration),
                    event: i32::from(f.field6),
                    operand_x: i32::from(f.field7),
                    operand_y: i32::from(f.field8),
                })
                .collect(),
            frame_sounds: ani
                .frames
                .iter()
                .map(|f| f.sounds.iter().cloned().map(Some).collect())
                .collect(),
        }
    }

    /// The sounds of element `index`.
    pub fn sounds_of(&self, index: usize) -> &[Option<String>] {
        self.frame_sounds.get(index).map_or(&[], Vec::as_slice)
    }

    /// A simple looping animation of the given (frame, duration) pairs.
    pub fn looping(key: &str, frames: &[(i32, i32)]) -> Animation {
        Animation {
            key: key.to_string(),
            image_set: String::new(),
            frames: frames
                .iter()
                .map(|&(image_index, duration)| AnimFrame {
                    image_op: 3,
                    image_index,
                    duration,
                    ..AnimFrame::default()
                })
                .collect(),
            frame_sounds: Vec::new(),
        }
    }

    /// A one-shot animation: the last element finishes it.
    pub fn once(key: &str, frames: &[(i32, i32)]) -> Animation {
        let mut a = Animation::looping(key, frames);
        if let Some(last) = a.frames.last_mut() {
            last.next_op = 9;
        }
        a
    }
}

/// What a call to [`AnimPlayer::advance`] changed on the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimEffect {
    /// New frame number, if the image changed.
    pub frame: Option<i32>,
    /// Position change (move operation 2).
    pub moved_by: Option<(i32, i32)>,
    /// New position (move operation 3).
    pub moved_to: Option<(i32, i32)>,
    /// Event value reported by this call (-1 without an animation).
    pub event: i32,
    /// Index of the element applied by this call, if one was; its sounds
    /// play now.
    pub applied: Option<usize>,
}

/// Plays one animation for one object.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnimPlayer {
    anim: Option<Arc<Animation>>,
    pending: usize,
    wait: u32,
    wait_calls: bool,
    finished: bool,
    pending_event: i32,
    reported_event: i32,
    speed: f32,
}

impl AnimPlayer {
    /// Starts `anim` from its first element.
    pub fn set(&mut self, anim: Arc<Animation>) {
        let event = anim.frames.first().map_or(0, |f| f.event);
        self.anim = Some(anim);
        self.pending = 0;
        self.wait = 0;
        self.wait_calls = false;
        self.finished = false;
        self.pending_event = event;
        self.reported_event = event;
        self.speed = 1.0;
    }

    /// Key of the current animation.
    pub fn key(&self) -> Option<&str> {
        self.anim.as_deref().map(|a| a.key.as_str())
    }

    /// The current animation.
    pub fn animation(&self) -> Option<&Arc<Animation>> {
        self.anim.as_ref()
    }

    /// True once a finishing element has been shown for its whole duration.
    /// Without an animation the player counts as finished, so a state that
    /// waits for its animation never hangs on missing data.
    pub fn is_done(&self) -> bool {
        match &self.anim {
            None => true,
            Some(a) => a.frames.is_empty() || (self.finished && self.wait == 0),
        }
    }

    /// Sets the playback speed factor (1.0 is normal).
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    /// Advances by `ms`. `frame` is the owner's current frame number and
    /// `mirrored` whether the owner faces left (mirroring move operation 2).
    /// The image set's frame range is not known: see [`AnimPlayer::advance_in`].
    pub fn advance(&mut self, ms: u32, frame: i32, mirrored: bool) -> AnimEffect {
        self.advance_in(ms, frame, mirrored, None)
    }

    /// Advances by `ms` like [`AnimPlayer::advance`], with `frames`, the
    /// first and last frame numbers of the owner's image set when known:
    /// "next image" past the last frame wraps to the first, "previous
    /// image" before the first wraps to the last, and the operations that
    /// name the set's ends use them.
    pub fn advance_in(
        &mut self,
        ms: u32,
        frame: i32,
        mirrored: bool,
        frames: Option<(i32, i32)>,
    ) -> AnimEffect {
        let mut out = AnimEffect {
            event: -1,
            ..AnimEffect::default()
        };
        let Some(anim) = self.anim.clone() else {
            return out;
        };
        if anim.frames.is_empty() {
            return out;
        }
        if self.wait != 0 {
            if self.wait_calls {
                self.wait -= 1;
                out.event = self.reported_event;
                return out;
            }
            if ms < self.wait {
                self.wait -= ms;
                out.event = self.reported_event;
                return out;
            }
            self.wait = 0;
        }
        if self.finished {
            out.event = self.pending_event;
            return out;
        }
        let idx = self.pending.min(anim.frames.len() - 1);
        let el = anim.frames[idx];
        out.applied = Some(idx);
        let new_frame = match (el.image_op, frames) {
            (1, Some((first, last))) => Some(if frame >= last { first } else { frame + 1 }),
            (1, None) => Some(frame + 1),
            (2, Some((first, last))) => Some(if frame <= first { last } else { frame - 1 }),
            (2, None) => Some(frame - 1),
            (3, _) => Some(el.image_index),
            (4, Some((first, _))) => Some(first),
            (5, Some((_, last))) => Some(last),
            _ => None,
        };
        let shown = new_frame.unwrap_or(frame);
        out.frame = new_frame;
        match el.move_op {
            2 => {
                let dx = if mirrored {
                    -el.operand_x
                } else {
                    el.operand_x
                };
                out.moved_by = Some((dx, el.operand_y));
            }
            3 => out.moved_to = Some((el.operand_x, el.operand_y)),
            _ => {}
        }
        // A negative duration compares as a huge unsigned wait: held for
        // practically ever.
        let mut wait = el.duration as u32;
        if self.speed != 1.0 {
            wait = (f64::from(el.duration) * f64::from(self.speed)) as i64 as u32;
        }
        self.wait = wait;
        self.wait_calls = el.flags & FRAME_COUNTS_CALLS != 0;
        let n = anim.frames.len();
        let next = match el.next_op & 0xFFFF {
            9 => {
                self.finished = true;
                idx
            }
            8 => {
                self.finished = false;
                self.speed = 1.0;
                0
            }
            7 => {
                self.finished = false;
                self.wait = 0;
                if n > 1 { 1 } else { 0 }
            }
            6 => idx,
            op @ 1..=5 => {
                let wanted = match (op, frames) {
                    (1, _) => Some(el.image_index),
                    (2, Some((first, _))) => Some(first),
                    (3, Some((_, last))) => Some(last),
                    (4, Some((first, _))) => Some(first + 1),
                    (5, Some((_, last))) => Some(last - 1),
                    _ => None,
                };
                match wanted {
                    Some(f) if shown != f => idx,
                    _ => (idx + 1) % n,
                }
            }
            _ => (idx + 1) % n,
        };
        self.pending = next;
        self.reported_event = self.pending_event;
        self.pending_event = anim.frames[next].event;
        out.event = if self.wait != 0 {
            self.reported_event
        } else {
            self.pending_event
        };
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_elements_last_one_call_and_time_is_not_carried() {
        let anim = Arc::new(Animation::looping("A", &[(1, 1), (2, 30), (3, 10)]));
        let mut p = AnimPlayer::default();
        p.set(anim);
        let mut frame = 0;
        let mut seen = Vec::new();
        for _ in 0..6 {
            if let Some(f) = p.advance(18, frame, false).frame {
                frame = f;
            }
            seen.push(frame);
        }
        // 1 ms element: one call. 30 ms element: applied, then 18 < 30,
        // then the wait expires and the next element is applied.
        assert_eq!(seen, vec![1, 2, 2, 3, 1, 2]);
    }

    #[test]
    fn one_shot_finishes_after_its_last_wait() {
        let anim = Arc::new(Animation::once("L", &[(65, 100), (65, 300)]));
        let mut p = AnimPlayer::default();
        p.set(anim);
        let mut ticks = 0;
        while !p.is_done() {
            p.advance(18, 65, false);
            ticks += 1;
            assert!(ticks < 100);
        }
        // 100 ms: applied + 5 waits, then 300 ms: applied + 16 waits, then
        // one call that ends the wait.
        assert_eq!(ticks, 1 + 6 + 17);
    }

    fn frames_seen(anim: Animation, start: i32, frames: Option<(i32, i32)>, n: usize) -> Vec<i32> {
        let mut p = AnimPlayer::default();
        p.set(Arc::new(anim));
        let mut frame = start;
        (0..n)
            .map(|_| {
                if let Some(f) = p.advance_in(18, frame, false, frames).frame {
                    frame = f;
                }
                frame
            })
            .collect()
    }

    fn element(image_op: i32, next_op: i32, duration: i32) -> AnimFrame {
        AnimFrame {
            image_op,
            next_op,
            duration,
            ..AnimFrame::default()
        }
    }

    #[test]
    fn next_and_previous_wrap_around_the_image_set() {
        let cycle = Animation {
            key: "CYCLE".into(),
            frames: vec![element(1, 0, 1)],
            ..Animation::default()
        };
        assert_eq!(
            frames_seen(cycle.clone(), 3, Some((2, 4)), 5),
            vec![4, 2, 3, 4, 2]
        );
        // Without the range the frame keeps counting.
        assert_eq!(frames_seen(cycle, 3, None, 3), vec![4, 5, 6]);
        let back = Animation {
            key: "BACK".into(),
            frames: vec![element(2, 0, 1)],
            ..Animation::default()
        };
        assert_eq!(
            frames_seen(back.clone(), 3, Some((2, 4)), 4),
            vec![2, 4, 3, 2]
        );
        assert_eq!(frames_seen(back, 3, None, 2), vec![2, 1]);
    }

    #[test]
    fn set_ends_and_waiting_for_them() {
        // Jump to the last image, step back until the first, then finish:
        // the shape of a backward slide.
        let backward = Animation {
            key: "BACKWARD".into(),
            frames: vec![element(5, 0, 1), element(2, 2, 1), element(0, 9, 1)],
            ..Animation::default()
        };
        let mut p = AnimPlayer::default();
        p.set(Arc::new(backward.clone()));
        let mut frame = 1;
        let mut seen = Vec::new();
        while !p.is_done() {
            if let Some(f) = p.advance_in(18, frame, false, Some((1, 4))).frame {
                frame = f;
            }
            seen.push(frame);
            assert!(seen.len() < 20);
        }
        assert_eq!(seen, vec![4, 3, 2, 1, 1, 1]);
        // Without the range "last" leaves the image and the wait for the
        // first image does not wait.
        assert_eq!(frames_seen(backward, 7, None, 3), vec![7, 6, 6]);
        let forward = Animation {
            key: "FORWARD".into(),
            frames: vec![element(4, 0, 1), element(1, 5, 1), element(0, 9, 1)],
            ..Animation::default()
        };
        assert_eq!(
            frames_seen(forward, 9, Some((10, 14)), 5),
            vec![10, 11, 12, 13, 13]
        );
    }

    #[test]
    fn missing_animation_counts_as_done() {
        let p = AnimPlayer::default();
        assert!(p.is_done());
    }
}
