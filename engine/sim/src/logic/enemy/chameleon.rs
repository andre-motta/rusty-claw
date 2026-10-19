//! The Chameleon, the green lizard of the thirteenth and fourteenth
//! levels.
//!
//! It is a one-hit human enemy that decides on every tick, with no wait:
//! with Claw 200 to 500 px away it runs at him ([`FAST_ADVANCE_SPEED`])
//! or tries to phase; closer, it lashes its tongue ([`TONGUE`], for
//! [`DAMAGE`]) when its strike timer allows, tries to phase, or waits a
//! tick; farther away it paces. It cannot be lifted, carries no loot and
//! has [`HEALTH`] health, so every accepted hit kills it.
//!
//! Its trick is the phase. It fades out where it stands over 26 calls,
//! untouchable and harmless meanwhile, drawn with the brightness blend on
//! its phase picture ([`PHASE_FRAME`]); then it jumps, hidden, to a
//! random x inside its patrol limits (never inside Claw's own columns),
//! waits hidden for 0 to 1500 ms, and fades back in over 26 calls. A new
//! phase cannot start for [`PHASE_COOLDOWN_MS`] after one ends. The fade
//! moves by [`FADE_STEP`] on every call (its 5 ms timer always runs out
//! between two steps), so it takes 26 calls at either clock; only the
//! hidden wait is in milliseconds, and so in whole steps or frames.
//!
//! # The rules
//!
//! - Built: health 1 and damage 10 (both times the difficulty factors,
//!   the record's values unused), smarts 99, not liftable, not going by
//!   sight, the shared human range boxes, its picture's box as its hit and
//!   movement box, the 2 x 2 contact box at its origin, run speed 180 and
//!   walk speed 60; placed on its patrol limit when outside it, then
//!   dropped to the floor; then it goes Home.
//! - Going Home: the shared human go Home (stopped), its idle pose (not
//!   advanced), drawn and back in pair collision, and 3 times in 100 its
//!   roam hiss. This is the only step that shows it and makes it
//!   touchable again after a phase.
//! - Home, every tick: it faces Claw and advances its pose; then by range
//!   mode: 0 and 1, a draw of 0..101 below 66 runs at Claw, otherwise it
//!   tries to phase; 2 to 4, a draw of 0..101 below 40 lashes the tongue
//!   when the strike timer has run out (then a new timer of 500 ms plus a
//!   draw of 0..=500), 40 to 79 tries to phase, otherwise nothing; 5 and 6,
//!   3 times in 100 the roam hiss, then pacing.
//! - Trying to phase: while the cooldown runs it only goes Home again;
//!   otherwise it stops, fades out ([`EnemyState::PhaseOut`]), out of pair
//!   collision, showing [`PHASE_FRAME`] drawn with the blend.
//! - Fading out: the amount drops by 10 each call; below 0 the jump: the
//!   target x drawn from the patrol limits ([`phase_target`]), the hidden
//!   wait drawn ([`HIDDEN_WAIT_SPREAD`]), and, when the target lies
//!   outside Claw's movement box columns, the target becomes its position
//!   ([`super::EnemyFields::teleport_to`]): the movement step of the same
//!   tick moves it there through the tiles as far as they let it, and a
//!   wall in the way makes the target its patrol limit on that side, where
//!   it stays. Fading in ([`EnemyState::PhaseIn`]) then follows.
//! - Fading in: nothing while the hidden wait runs; then drawn again, the
//!   amount rises by 10 each call; above 255 normal drawing, the cooldown
//!   and Home.
//! - The tongue: the shared strike with its strike picture's event (the
//!   fourth call) and the tongue sound; its tongue box is live on that
//!   call only; it goes Home on the sixteenth.
//! - Hits: the shared human hit rule, which kills it in the same hit.
//! - The fourteenth level only: while not fading or dead, a close frontal
//!   attack of Claw's (range modes 3 and 4, Claw attacking and facing it)
//!   tries to phase 3 times in 100 ([`FRONTAL_PHASE_CHANCE`]).
//!
//! The pre-tick's out-of-range rule may end a phase with the shared human
//! go idle (Claw out of every range box, the hidden lizard counting as
//! off screen): it then idles and paces untouchable, and unseen when it was
//! hidden, until its next go Home.
//!
//! # Patrol limits and the phase
//!
//! The target is drawn with 32-bit wrapping integer arithmetic on the
//! limits, an unset limit counting as [`UNSET_LIMITS`]: with both unset the
//! span wraps to -2 and the target is the low limit or one more. Such a
//! target is far left of the level: the movement step moves the lizard
//! toward it until a wall stops it (the target then becomes its left
//! limit, and the next phase draws from there) or until its floor ends
//! (it falls). On the thirteenth level the unbounded lizard at (2427,
//! 2416) so stays where it is phase after phase, its left limit creeping
//! up by a few pixels each time, and the one at (1782, 1900) is carried
//! toward the end of its ledge and falls off it.
//!
//! # State numbers
//!
//! It decides in `Home`, strikes in `Strike`, runs and walks in
//! `FastAdvance` and `Advance`, paces in `Pace`, idles in `Idle` and fades
//! in the thirteenth level's new names `PhaseOut` and `PhaseIn` ([`STATES`],
//! 2310 and 2311 in [`super::redtail::STATE_BLOCK`]).
//!
//! # Random draws, in order
//!
//! - building: going Home's;
//! - the pre-tick: the shared roll (frame pulse ticks only); on the
//!   fourteenth level with the frontal rule's conditions one of 0..100;
//! - going Home: one of 0..100;
//! - Home: modes 0 and 1, one of 0..101, then a refused phase's go Home
//!   draw; modes 2 to 4, one of 0..101, then one of 0..501 after a strike
//!   entry, or a refused phase's go Home draw; modes 5 and 6, one of 0..100,
//!   then the pacing start's;
//! - the jump: the target's draw, then one of 0..1501;
//! - fading in and the tongue: none, then going Home's at their end;
//! - the shared states and hits: theirs.
//!
//! # Defaults taken
//!
//! - The hidden lizard counts as off screen for its range mode, as an
//!   object that was not drawn.
//! - The two unbounded lizards of the thirteenth level's start: the
//!   original's runs show the first staying in place with the target
//!   written into its left limit, and the second moved 70 px left off its
//!   ledge; both follow from the movement step taking the target as the
//!   position it starts from, with a wall's or a ledge's limit set to it,
//!   which is what this build does.
//!
//! Its data ([`ChameleonData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use claw_frame::{DrawMode, Renderable};

use super::human::{self, HumanAnims, HumanData};
use super::states::{self, enter_strike, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, EnemyData, EnemyFields, EnemyState, HUMAN_HIT_BY,
    Handler, Timer, build, data, data_mut, exact_x, face_claw, human_fields, obj_mut, play, rand,
    set_anim, set_state,
};
use crate::logic::{ObjectData, World};
use crate::object::{Object, ObjectId, Rect, draw_flags, flags};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Chameleon"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_CHAMELEON";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_CHAMELEON_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Its new state names, in number order from 2310 (see
/// [`super::redtail::STATE_BLOCK`]).
pub const STATES: [EnemyState; 2] = [EnemyState::PhaseOut, EnemyState::PhaseIn];

/// Its health, before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 1;
/// Damage of touching it and of its tongue, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Its smarts at the default factor.
pub const SMARTS: i32 = 99;

/// Its hit and movement box: its first picture's.
pub const BODY_RECT: Rect = Rect::new(-27, -30, 64, 25);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// The tongue's box, facing left.
pub const TONGUE: Rect = Rect::new(-127, -32, -16, -8);
/// The five range boxes (half width, half height), outermost first: the
/// shared human ones.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (150, 40), (100, 40)];

/// Running speed towards Claw, px/s.
pub const FAST_ADVANCE_SPEED: f64 = 180.0;
/// Walking speed towards Claw, px/s.
pub const ADVANCE_SPEED: f64 = 60.0;

/// The picture shown and blended while it fades.
pub const PHASE_FRAME: i32 = 901;
/// The blend amount at the start of a fade out (the sprite whole).
pub const FULL_AMOUNT: i32 = 255;
/// What the amount moves by on each call of a fade.
pub const FADE_STEP: i32 = 10;
/// The fade's own timer, ms: shorter than any step, so the amount moves
/// on every call.
pub const FADE_MS: u64 = 5;
/// The hidden wait after the jump is a draw of 0..this, ms (0 to 1500).
pub const HIDDEN_WAIT_SPREAD: u32 = 1501;
/// No new phase for this long after one ends, ms.
pub const PHASE_COOLDOWN_MS: u64 = 1000;
/// The strike timer after a tongue: this plus a draw of 0..=500, ms.
pub const STRIKE_WAIT_MS: u64 = 500;
/// The spread of the draw added to [`STRIKE_WAIT_MS`].
pub const STRIKE_WAIT_SPREAD: u32 = 501;
/// Chance in 101 that a decision in range modes 0 and 1 runs at Claw.
pub const RUN_CHANCE: i32 = 66;
/// Chance in 101 that a decision in range modes 2 to 4 strikes.
pub const STRIKE_CHANCE: i32 = 40;
/// Below this draw of 0..101 (and from [`STRIKE_CHANCE`]) a decision in
/// range modes 2 to 4 tries to phase.
pub const PHASE_BELOW: i32 = 80;
/// Chance in 100 of the roam hiss on going Home and on starting to pace.
pub const ROAM_CHANCE: i32 = 3;
/// Chance in 100, per tick, of a phase on a close frontal attack (the
/// fourteenth level only).
pub const FRONTAL_PHASE_CHANCE: i32 = 3;
/// The level whose Chameleons phase on a close frontal attack.
pub const FRONTAL_PHASE_LEVEL: u32 = 14;
/// The patrol limits an unset limit counts as for the phase's target
/// (min, max).
pub const UNSET_LIMITS: (i32, i32) = (-2_147_483_647, 2_147_483_646);

/// Its decision and idle pose.
pub const IDLE_ANIM: &str = "LEVEL_CHAMELEON_IDLE";
/// Running, walking and pacing.
pub const RUN_ANIM: &str = "LEVEL_CHAMELEON_FASTADVANCE";
/// The tongue.
pub const STRIKE_ANIM: &str = "LEVEL_CHAMELEON_STRIKE1";
/// A hit (never seen: the hit kills).
pub const HIT_ANIM: &str = "LEVEL_CHAMELEON_HIT";
/// A jump (never reached on the thirteenth level).
pub const JUMP_ANIM: &str = "LEVEL_CHAMELEON_JUMP";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_CHAMELEON_KILLFALL";
/// The tongue's sound.
pub const TONGUE_SOUND: &str = "LEVEL_CHAMELEON_TONGUESTRIKE";
/// Its death sound.
pub const DEATH_SOUND: &str = "LEVEL_CHAMELEON_CHAMELEONDEATH";
/// Its roam hiss.
pub const ROAM_SOUND: &str = "LEVEL_CHAMELEON_CHAMELEONROAM2";

/// Its animations of the shared states.
pub const ANIMS: ClassAnims = ClassAnims {
    advance: RUN_ANIM,
    fast_advance: RUN_ANIM,
    pace: RUN_ANIM,
    walk: "",
    hit_low: HIT_ANIM,
    hit_high: HIT_ANIM,
    home: IDLE_ANIM,
    jump: JUMP_ANIM,
    fall: DEATH_ANIM,
    lifted: "",
    lifted2: "",
    bounce: "",
};

/// The hit and strike sounds of the shared states.
const SOUNDS: ClassSounds = ClassSounds {
    strike: TONGUE_SOUND,
    hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
    hit_high: "GAME_SOLHITHI",
};

/// The Chameleon class.
pub static CLASS: Class = Class {
    name: "Chameleon",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home,
    go_idle: human::go_idle,
    carry: None,
    pick_lift_voice: super::nothing,
    lift_roll: false,
    anims: ANIMS,
    sounds: SOUNDS,
};

/// How the shared phase picture ([`PHASE_FRAME`]) of the level's
/// Chameleons is drawn: the blend amount and mode live on the picture,
/// not on a lizard, so two lizards fading at once write the same picture
/// and the last writer of a tick sets how both are drawn. It lasts the
/// whole level and starts as [`PhaseFrame::default`] leaves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseFrame {
    /// Drawn with the brightness blend (else normally).
    pub blend: bool,
    /// The blend amount: 0 shows what is behind, 255 the picture.
    pub amount: u8,
}

impl Default for PhaseFrame {
    /// Normal drawing, the amount at the pictures' default.
    fn default() -> PhaseFrame {
        PhaseFrame {
            blend: false,
            amount: DrawMode::DEFAULT_COLOR,
        }
    }
}

/// A Chameleon's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChameleonData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// The tongue's timer: no new tongue before it runs out.
    pub strike_wait: Timer,
    /// The fade's own timer ([`FADE_MS`]).
    pub fade: Timer,
    /// No new phase before it runs out ([`PHASE_COOLDOWN_MS`]).
    pub cooldown: Timer,
    /// The hidden wait after the jump.
    pub hidden: Timer,
    /// The blend amount of its fade.
    pub amount: i32,
}

impl World<'_> {
    /// How the Chameleons' shared phase picture is drawn (see
    /// [`PhaseFrame`]).
    pub fn chameleon_frame(&self) -> PhaseFrame {
        self.out.chameleon
    }

    /// Sets how the Chameleons' shared phase picture is drawn: blended
    /// (or not) at `amount` (clamped to 0..=255). The lizards' fades set
    /// it; a test may put it in step with a recorded run.
    pub fn set_chameleon_phase(&mut self, blend: bool, amount: i32) {
        self.set_phase_blend(blend);
        self.set_phase_amount(amount);
    }

    fn set_phase_amount(&mut self, amount: i32) {
        self.out.chameleon.amount = amount.clamp(0, 255) as u8;
    }

    fn set_phase_blend(&mut self, blend: bool) {
        self.out.chameleon.blend = blend;
    }
}

/// Draws a lizard showing the shared phase picture as the picture says
/// (see [`PhaseFrame`]); any other sprite is left as it is.
pub fn apply_phase_look(frame: PhaseFrame, o: &Object, r: &mut Renderable) {
    if frame.blend && o.frame == PHASE_FRAME && o.image_set == IMAGE_SET {
        r.draw_mode = DrawMode::BLEND;
        r.draw_color = Some(frame.amount);
    }
}

/// The Chameleon data of `id`.
pub fn chameleon_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a ChameleonData> {
    match &w.objects.get(id)?.data {
        ObjectData::Chameleon(d) => Some(d),
        _ => None,
    }
}

/// The Chameleon data of `id`, mutable.
pub fn chameleon_data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut ChameleonData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Chameleon(d) => Some(d),
        _ => None,
    }
}

/// The x a phase sends a lizard with patrol limits `bounds` to, given the
/// draw `r` of 0..32767 (see the module documentation): with
/// `n = max - min + 1` in wrapping 32-bit arithmetic, `r % n + min`, or
/// for `n = 0` the low limit when `r` is odd and the high one otherwise.
/// Unset limits count as [`UNSET_LIMITS`].
pub fn phase_target(bounds: (i32, i32), r: i32) -> i32 {
    let lo = if bounds.0 == super::UNBOUNDED.0 {
        UNSET_LIMITS.0
    } else {
        bounds.0
    };
    let hi = if bounds.1 == super::UNBOUNDED.1 {
        UNSET_LIMITS.1
    } else {
        bounds.1
    };
    let n = hi.wrapping_sub(lo).wrapping_add(1);
    if n == 0 {
        if r & 1 != 0 { lo } else { hi }
    } else {
        (r % n).wrapping_add(lo)
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Home => Some(home_tick),
        EnemyState::PhaseOut => Some(phase_out_tick),
        EnemyState::PhaseIn => Some(phase_in_tick),
        EnemyState::FastAdvance => Some(fast_advance_tick),
        EnemyState::Advance => Some(advance_tick),
        _ => None,
    }
}

/// One logic call: the shared human logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    human::tick(w, id);
}

/// `attacker` hit `id`: the shared human hit rule.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    super::on_hit(w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building

/// Builds a Chameleon (see the module documentation). Draws: going
/// Home's.
pub fn construct(w: &mut World, id: ObjectId) {
    let fields = EnemyFields {
        ranges: RANGES,
        contact_rect: CONTACT,
        strike_rect: TONGUE,
        strike_anim: STRIKE_ANIM,
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        smarts: super::smarts(SMARTS),
        may_lift: false,
        human: HumanData {
            anims: HumanAnims {
                jump_back: JUMP_ANIM,
                idle: IDLE_ANIM,
                ..HumanAnims::default()
            },
            ..HumanData::default()
        },
        ..human_fields(w, id)
    };
    let body = Body {
        image_set: IMAGE_SET,
        move_rect: BODY_RECT,
        hit_rect: BODY_RECT,
        attack_rect: CONTACT,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::Chameleon(ChameleonData {
            enemy,
            ..ChameleonData::default()
        });
    }
    go_home(w, id);
}

// ----------------------------------------------------------------------
// Pre-tick

/// Its own pre-tick step, after the shared ones: on the fourteenth level
/// only, while not fading or dead, a close frontal attack of Claw's tries
/// to phase 3 times in 100. Draws: with the rule's conditions, one of
/// 0..100.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    if w.level_number() != FRONTAL_PHASE_LEVEL {
        return;
    }
    if matches!(
        super::state(w, id),
        Some(EnemyState::PhaseOut | EnemyState::PhaseIn | EnemyState::Dead)
    ) {
        return;
    }
    let Some((right, mode)) = data(w, id).map(|d| (d.claw_right, d.mode)) else {
        return;
    };
    let facing = w.claw.attacking && w.claw.facing_right != right;
    if facing && matches!(mode, 3 | 4) && rand(w, 100) < FRONTAL_PHASE_CHANCE {
        try_phase(w, id);
    }
}

// ----------------------------------------------------------------------
// Home

/// Goes Home: stopped, Home, its idle pose (not advanced), drawn and back
/// in pair collision, and 3 times in 100 its roam hiss. It leaves the
/// busy flag as it is. Draws: one of 0..100.
pub fn go_home(w: &mut World, id: ObjectId) {
    states::go_home(w, id);
    set_anim(w, id, IDLE_ANIM);
    let o = obj_mut(w, id);
    o.draw_flags &= !draw_flags::HIDDEN;
    o.flags &= !flags::NO_COLLIDE;
    if rand(w, 100) < ROAM_CHANCE {
        play(w, ROAM_SOUND);
    }
}

/// Home (see the module documentation). Draws: by mode, as the module
/// documentation lists.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    w.advance_anim(id);
    let now = w.now_ms;
    match data(w, id).map_or(6, |d| d.mode) {
        0 | 1 => {
            if rand(w, 101) < RUN_CHANCE {
                enter_fast_advance(w, id);
            } else {
                try_phase(w, id);
            }
        }
        2..=4 => {
            let r = rand(w, 101);
            if r < STRIKE_CHANCE {
                if chameleon_data(w, id).is_some_and(|c| c.strike_wait.done(now)) {
                    enter_strike(w, id);
                    let wait = STRIKE_WAIT_MS + rand(w, STRIKE_WAIT_SPREAD) as u64;
                    if let Some(c) = chameleon_data_mut(w, id) {
                        c.strike_wait = Timer::new(now, wait);
                    }
                }
            } else if r < PHASE_BELOW {
                try_phase(w, id);
            }
        }
        _ => {
            if rand(w, 100) < ROAM_CHANCE {
                play(w, ROAM_SOUND);
            }
            start_pacing(w, id);
        }
    }
}

// ----------------------------------------------------------------------
// Running and walking at Claw

fn at_limit(w: &World, id: ObjectId, limit: i32) -> bool {
    exact_x(w, id) == f64::from(limit)
}

fn toward_claw(w: &World, id: ObjectId, speed: f64) -> f64 {
    if data(w, id).is_some_and(|d| d.claw_right) {
        speed
    } else {
        -speed
    }
}

fn set_vx(w: &mut World, id: ObjectId, vx: f64) {
    if let Some(d) = data_mut(w, id) {
        d.vx = vx;
    }
}

fn mode(w: &World, id: ObjectId) -> u8 {
    data(w, id).map_or(6, |d| d.mode)
}

/// Starts running at Claw at [`FAST_ADVANCE_SPEED`], facing him.
pub fn enter_fast_advance(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let vx = toward_claw(w, id, FAST_ADVANCE_SPEED);
    set_vx(w, id, vx);
    set_state(w, id, EnemyState::FastAdvance);
    set_anim(w, id, RUN_ANIM);
}

/// FastAdvance, the shared rule at its own speed: exactly on the limit on
/// Claw's side it goes idle (and carries on); it faces Claw; then by mode
/// it keeps running at him (0, 1, 5), walks (2), goes Home (3, 4) or paces
/// (6). Draws: one, discarded; then the idle's or the pacing start's.
pub fn fast_advance_tick(w: &mut World, id: ObjectId) {
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let right = data(w, id).is_some_and(|d| d.claw_right);
    if (at_limit(w, id, min) && !right) || (at_limit(w, id, max) && right) {
        human::go_idle(w, id);
    }
    face_claw(w, id);
    w.advance_anim(id);
    let _ = w.rng.next_u15();
    match mode(w, id) {
        0 | 1 | 5 => {
            let vx = toward_claw(w, id, FAST_ADVANCE_SPEED);
            set_vx(w, id, vx);
        }
        2 => enter_advance(w, id),
        3 | 4 => go_home(w, id),
        _ => start_pacing(w, id),
    }
}

/// Starts walking at Claw at [`ADVANCE_SPEED`], facing him.
pub fn enter_advance(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let vx = toward_claw(w, id, ADVANCE_SPEED);
    set_vx(w, id, vx);
    set_state(w, id, EnemyState::Advance);
    set_anim(w, id, RUN_ANIM);
}

/// Advance, the shared rule at its own speeds: exactly on either limit it
/// goes idle (and carries on); it faces Claw and walks at him; then by
/// mode it runs (0), keeps walking (1, 2) or goes Home (3 to 6).
pub fn advance_tick(w: &mut World, id: ObjectId) {
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    if at_limit(w, id, min) || at_limit(w, id, max) {
        human::go_idle(w, id);
    }
    face_claw(w, id);
    let vx = toward_claw(w, id, ADVANCE_SPEED);
    set_vx(w, id, vx);
    w.advance_anim(id);
    match mode(w, id) {
        0 => enter_fast_advance(w, id),
        1 | 2 => {}
        _ => go_home(w, id),
    }
}

// ----------------------------------------------------------------------
// The phase

/// Tries to phase: while the cooldown runs it only goes Home; otherwise it
/// stops and starts fading out, out of pair collision, showing the phase
/// picture with the blend (whose amount keeps its last value for this
/// tick's drawing). Draws: going Home's when refused.
pub fn try_phase(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if !chameleon_data(w, id).is_some_and(|c| c.cooldown.done(now)) {
        go_home(w, id);
        return;
    }
    set_vx(w, id, 0.0);
    set_state(w, id, EnemyState::PhaseOut);
    let has_frame = w
        .image_frames(IMAGE_SET)
        .is_none_or(|(lo, hi)| (lo..=hi).contains(&PHASE_FRAME));
    let o = obj_mut(w, id);
    o.flags |= flags::NO_COLLIDE;
    if has_frame {
        o.frame = PHASE_FRAME;
    }
    if let Some(c) = chameleon_data_mut(w, id) {
        c.amount = FULL_AMOUNT;
    }
    w.set_phase_blend(true);
}

/// Steps the fade's amount by `step` when its own timer has run out (on
/// every call: see [`FADE_MS`]); returns the amount.
fn fade_step(w: &mut World, id: ObjectId, step: i32) -> i32 {
    let now = w.now_ms;
    let Some(c) = chameleon_data_mut(w, id) else {
        return 0;
    };
    if c.fade.done(now) {
        c.amount += step;
        c.fade = Timer::new(now, FADE_MS);
    }
    c.amount
}

/// Fading out (see the module documentation). Draws: on the jump, the
/// target's and the hidden wait's.
pub fn phase_out_tick(w: &mut World, id: ObjectId) {
    let amount = fade_step(w, id, -FADE_STEP);
    if amount < 0 {
        w.set_phase_amount(0);
        jump(w, id);
    } else {
        w.set_phase_amount(amount);
    }
}

/// The jump at the end of the fade out: hidden, the target and the
/// hidden wait drawn, and the target taken as its position unless it lies
/// in Claw's columns. Draws: the target's (one of 0..32767), then one of
/// 0..1501.
fn jump(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::PhaseIn);
    obj_mut(w, id).draw_flags |= draw_flags::HIDDEN;
    let bounds = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let target = phase_target(bounds, w.rng.next_u15() as i32);
    let wait = rand(w, HIDDEN_WAIT_SPREAD) as u64;
    let now = w.now_ms;
    if let Some(c) = chameleon_data_mut(w, id) {
        c.amount = 0;
        c.hidden = Timer::new(now, wait);
    }
    let columns = w
        .claw_object()
        .map(|c| (c.x + c.move_rect.left, c.x + c.move_rect.right));
    let clear = columns.is_none_or(|(l, r)| target < l || target > r);
    if clear && let Some(d) = data_mut(w, id) {
        d.teleport_to = Some(target);
    }
}

/// Fading in (see the module documentation). Draws: going Home's at its
/// end.
pub fn phase_in_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if !chameleon_data(w, id).is_some_and(|c| c.hidden.done(now)) {
        return;
    }
    obj_mut(w, id).draw_flags &= !draw_flags::HIDDEN;
    let amount = fade_step(w, id, FADE_STEP);
    if amount > FULL_AMOUNT {
        w.set_phase_amount(FULL_AMOUNT);
        w.set_phase_blend(false);
        if let Some(c) = chameleon_data_mut(w, id) {
            c.cooldown = Timer::new(now, PHASE_COOLDOWN_MS);
        }
        go_home(w, id);
    } else {
        w.set_phase_amount(amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_states_follow_redtails_in_the_thirteenth_levels_block() {
        let block = super::super::redtail::STATE_BLOCK;
        assert_eq!(STATES.map(|s| s as i32), [2310, 2311]);
        for s in STATES {
            assert!(block.contains(&(s as i32)), "{}", s.name());
            assert!(handler(s).is_some(), "{}", s.name());
        }
        assert!(
            super::super::redtail::STATES
                .iter()
                .all(|s| (*s as i32) < STATES[0] as i32)
        );
        assert!(CLASS.human, "a human enemy");
        assert!(CLASS.carry.is_none(), "never lifted");
        assert!(handler(EnemyState::Strike).is_none(), "the shared tongue");
        assert!(handler(EnemyState::Idle).is_none(), "the shared idle");
    }

    #[test]
    fn a_bounded_target_lies_in_its_limits() {
        // A 47 px range: the draw modulo 48 from its left limit.
        assert_eq!(phase_target((4237, 4284), 0), 4237);
        assert_eq!(phase_target((4237, 4284), 47), 4284);
        assert_eq!(phase_target((4237, 4284), 48), 4237);
        assert_eq!(phase_target((4237, 4284), 32767), 4237 + 32767 % 48);
    }

    #[test]
    fn unset_limits_wrap_far_left() {
        let unset = super::super::UNBOUNDED;
        // The span wraps to -2: the low limit or one more.
        assert_eq!(phase_target(unset, 0), -2_147_483_647);
        assert_eq!(phase_target(unset, 1), -2_147_483_646);
        assert_eq!(phase_target(unset, 12345), -2_147_483_646);
        // A left limit learned at the target: the span wraps to -3, -4,
        // -6 as the limit creeps up (the original's runs: -2147483646 to
        // -2147483645, to -2147483643, to -2147483642).
        assert_eq!(phase_target((-2_147_483_646, unset.1), 1), -2_147_483_645);
        assert_eq!(phase_target((-2_147_483_645, unset.1), 2), -2_147_483_643);
        assert_eq!(phase_target((-2_147_483_643, unset.1), 1), -2_147_483_642);
    }

    #[test]
    fn a_fade_takes_26_calls_each_way() {
        // Out: 255 down by 10 until below 0.
        let out = (1..).find(|k| FULL_AMOUNT - FADE_STEP * k < 0).unwrap();
        assert_eq!(out, 26);
        // In: from 0 up by 10 until above 255.
        let back = (1..).find(|k| FADE_STEP * k > FULL_AMOUNT).unwrap();
        assert_eq!(back, 26);
        // The fade's timer runs out between any two steps.
        const { assert!(FADE_MS < 16) };
        // The hidden wait is at most 84 calls at the 18 ms step.
        assert_eq!((HIDDEN_WAIT_SPREAD - 1).div_ceil(18), 84);
    }

    #[test]
    fn the_phase_picture_is_blended_only_while_a_fade_runs() {
        let mut o = Object::new(
            1,
            0,
            0,
            0,
            "Chameleon".into(),
            crate::object::LogicKind::Inert,
        );
        o.image_set = IMAGE_SET.to_string();
        o.frame = PHASE_FRAME;
        let r = Renderable {
            id: 1,
            image_set_key: IMAGE_SET.to_string(),
            frame: PHASE_FRAME,
            x: 0.0,
            y: 0.0,
            z: 0,
            mirror: false,
            invert: false,
            flash: false,
            draw_mode: DrawMode::NORMAL,
            draw_color: None,
            teleported: false,
        };
        let mut a = r.clone();
        apply_phase_look(PhaseFrame::default(), &o, &mut a);
        assert_eq!((a.draw_mode, a.draw_color), (DrawMode::NORMAL, None));
        let fading = PhaseFrame {
            blend: true,
            amount: 125,
        };
        let mut b = r.clone();
        apply_phase_look(fading, &o, &mut b);
        assert_eq!((b.draw_mode, b.draw_color), (DrawMode::BLEND, Some(125)));
        // Another picture of the set is drawn as it is.
        o.frame = 101;
        let mut c = r.clone();
        apply_phase_look(fading, &o, &mut c);
        assert_eq!(c.draw_mode, DrawMode::NORMAL);
    }

    #[test]
    fn its_art_is_its_own() {
        for key in [
            IDLE_ANIM,
            RUN_ANIM,
            STRIKE_ANIM,
            HIT_ANIM,
            JUMP_ANIM,
            DEATH_ANIM,
            TONGUE_SOUND,
            DEATH_SOUND,
            ROAM_SOUND,
        ] {
            assert!(key.starts_with(ANIM_PREFIXES[0]), "{key}");
        }
        // The tongue reaches 127 px ahead of it; one hit kills it.
        assert_eq!(-TONGUE.left, 127);
        assert_eq!(HEALTH, 1);
    }
}
