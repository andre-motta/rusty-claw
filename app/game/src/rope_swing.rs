//! Smooth rope swing: drawing Claw and the rope he hangs from between the
//! rope's frames. Drawing only.
//!
//! A swinging rope is an animation of 120 frames that runs at a whole
//! number of frames per second (40 for the usual half swing of 1.5 s, from
//! 34 to 50 on the levels), and Claw hangs at a fixed place below the end
//! of the frame shown. A frame lasts longer than an 18 ms tick, so the end
//! of the rope, Claw and the camera that follows him stay put on some
//! ticks and move on the others: drawn at a 60 Hz refresh, they move on
//! about two frames in three, a stepped look the original has too.
//!
//! With the smooth rope swing setting on (the default), the frames drawn
//! between two ticks of a swing place Claw on a continuous path instead:
//! the rope's frame position runs on smoothly between the positions of the
//! two ticks (in step with the ordinary interpolation between ticks), and
//! Claw hangs below the end interpolated at that position, half a frame
//! behind it. The rope shows the frame of the tick whose frame position is
//! drawn, which is always the frame nearest to the end Claw hangs from, so
//! he stays on the rope's end and never strays more than half a frame from
//! where the game has him. A camera that follows Claw moves with him.
//!
//! Only what is drawn changes: the game, its traces, recordings and
//! replays are the same with the setting on or off. A tick that the
//! smoothing cannot place exactly from the game's own state (the tick Claw
//! grabs or lets go, a respawn, a rope that does not move) is drawn as
//! the ordinary interpolation draws it, as is everything with the setting
//! off.

use claw_frame::{InterpolatedFrame, Point, SimSnapshot};
use claw_sim::logic::rope;
use claw_sim::player::{state, swing};
use claw_sim::view::{PlaneGeometry, view_origin};
use claw_sim::{ObjectData, Sim};

/// Claw hanging from a rope after one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Hang {
    /// The tick.
    tick: u64,
    /// The rope's drawing id.
    rope: u32,
    /// Claw's drawing id.
    claw: u32,
    /// The rope's position.
    rope_at: (i32, i32),
    /// The frame position the tick's rope frame comes from.
    shown: f64,
    /// Claw faces right.
    facing_right: bool,
    /// The view is centred on Claw.
    camera_on_claw: bool,
}

/// The swing of the last two ticks, read from the game after each tick.
#[derive(Debug, Clone, Default)]
pub struct SwingTrack {
    older: Option<Hang>,
    newer: Option<Hang>,
    /// The rope Claw hung from after the last tick (its drawing id and
    /// tick) and its frame position then, which the next tick shows.
    next: Option<(u64, u32, f64)>,
}

/// Where the lower end of a rope is at frame position `at`, relative to the
/// rope: the ends of the two frames around it, blended. Whole positions
/// give the end of their frame exactly.
pub fn end_at(at: f64) -> (f64, f64) {
    let k = at.floor();
    let t = at - k;
    let frames = i64::from(rope::FRAMES);
    let frame = |k: i64| (k.rem_euclid(frames) + 1) as i32;
    let k = k as i64;
    let (e0, _, _) = rope::end_of_frame(frame(k));
    let (e1, _, _) = rope::end_of_frame(frame(k + 1));
    let lerp = |a: i32, b: i32| f64::from(a) + (f64::from(b) - f64::from(a)) * t;
    (lerp(e0.0, e1.0), lerp(e0.1, e1.1))
}

impl SwingTrack {
    /// Forgets the swing (a respawn or a level start).
    pub fn clear(&mut self) {
        *self = SwingTrack::default();
    }

    /// Reads the swing from `sim` after its tick `tick`.
    pub fn after_tick(&mut self, sim: &Sim, tick: u64) {
        let next = self.next.take();
        self.older = self.newer.take();
        let Some(claw) = sim.player() else {
            return;
        };
        if claw.state != state::SWING {
            return;
        }
        let Some(r) = sim.claw().rope.and_then(|id| sim.objects().get(id)) else {
            return;
        };
        let ObjectData::Rope(d) = &r.data else {
            return;
        };
        self.next = Some((tick, r.serial, d.position));
        let Some((_, _, shown)) = next.filter(|&(t, id, _)| t + 1 == tick && id == r.serial) else {
            return;
        };
        let facing_right = sim.claw().facing_right;
        let end = (r.x + d.end.0, r.y + d.end.1);
        // The smoothing places Claw from the rope's own frame position: only
        // when that gives this tick's frame, and Claw where the game hangs
        // him from it.
        if !shown.is_finite()
            || shown < 0.0
            || rope::frame_at(shown) != d.frame
            || r.frame != d.frame
            || swing::hang_point(end, facing_right) != (claw.x, claw.y)
        {
            return;
        }
        self.newer = Some(Hang {
            tick,
            rope: r.serial,
            claw: claw.serial,
            rope_at: (r.x, r.y),
            shown,
            facing_right,
            camera_on_claw: sim.camera_point() == (claw.x, claw.y),
        });
    }

    fn at(&self, tick: u64) -> Option<Hang> {
        [self.older, self.newer]
            .into_iter()
            .flatten()
            .find(|h| h.tick == tick)
    }

    /// Redraws Claw, his rope and the view in `frame`, drawn `alpha` of
    /// the way from `prev` to `curr`, on the smooth swing when Claw hangs
    /// from a moving rope in both ticks; `plane` and `view` place the view
    /// (the main plane and the logic view's size). Returns whether it did.
    pub fn smooth<'a>(
        &self,
        frame: &mut InterpolatedFrame<'a>,
        prev: &'a SimSnapshot,
        curr: &'a SimSnapshot,
        alpha: f64,
        plane: PlaneGeometry,
        view: (u32, u32),
    ) -> bool {
        let (Some(a), Some(b)) = (self.at(prev.tick), self.at(curr.tick)) else {
            return false;
        };
        if b.tick != a.tick + 1 || (a.rope, a.claw, a.rope_at) != (b.rope, b.claw, b.rope_at) {
            return false;
        }
        // Less than a frame per tick, as on every rope of the game.
        let step = b.shown - a.shown;
        if step.is_nan() || step <= 0.0 || step >= 1.0 {
            return false;
        }
        let at = a.shown + step * alpha.clamp(0.0, 1.0);
        // The tick whose frame shows: the frame position drawn is past the
        // start of the newer tick's frame, or still in the older one's.
        let (hang, snap) = if at.floor() >= b.shown.floor() {
            (b, curr)
        } else {
            (a, prev)
        };
        let find = |id: u32| snap.renderables.iter().find(|r| r.id == id);
        let (Some(rope_image), Some(claw_image)) = (find(b.rope), find(b.claw)) else {
            return false;
        };
        let end = end_at(at - 0.5);
        let side = if hang.facing_right {
            swing::HANG_X
        } else {
            -swing::HANG_X
        };
        let x = f64::from(b.rope_at.0) + end.0 + f64::from(side);
        let y = f64::from(b.rope_at.1) + end.1 + f64::from(swing::HANG_Y);
        for r in &mut frame.renderables {
            if r.source.id == b.rope {
                r.source = rope_image;
                r.x = f64::from(b.rope_at.0);
                r.y = f64::from(b.rope_at.1);
            } else if r.source.id == b.claw {
                r.source = claw_image;
                r.x = x;
                r.y = y;
            }
        }
        if a.camera_on_claw && b.camera_on_claw {
            let (cx, cy) = view_origin(plane, (x, y), view);
            frame.camera = Point { x: cx, y: cy };
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_positions_give_their_frame_ends() {
        for frame in 1..=rope::FRAMES {
            let (e, _, _) = rope::end_of_frame(frame);
            let at = f64::from(frame - 1);
            assert_eq!(end_at(at), (f64::from(e.0), f64::from(e.1)), "{frame}");
            assert_eq!(
                end_at(at + f64::from(rope::FRAMES)),
                end_at(at),
                "a whole swing later"
            );
        }
    }

    #[test]
    fn between_frames_the_end_is_blended_and_wraps_round() {
        // Frames 25 and 26: (-12, 87) and (-2, 87).
        assert_eq!(end_at(24.25), (-9.5, 87.0));
        // The last frame of the way back (-139, 26) to the first of the
        // next swing (-140, 27).
        assert_eq!(end_at(119.5), (-139.5, 26.5));
        assert_eq!(end_at(-0.5), end_at(119.5));
    }
}
