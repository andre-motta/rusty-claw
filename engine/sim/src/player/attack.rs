//! Claw's attacks: choosing one from the pressed buttons, running it on its
//! animation, and cancelling it.
//!
//! The host states call [`select`] once per tick in the game's order: Jump,
//! Fall and Crouch before their own update, Walk after its movement (an
//! attack started there stops the walk), and Stand before its update. While
//! an attack runs, the hosts call [`update`] in place of their plain
//! animation step.
//!
//! Nothing starts an attack yet: [`select`] returns false, so [`update`]
//! only runs when another part of the game sets [`super::Claw::attacking`].

use super::Ctx;

/// Event value of the stand animation that asks for the attack's frame
/// hook.
pub const STAND_FRAME_EVENT: i32 = 99;

/// The running attack.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttackData {
    /// Attack type, 0 when none runs.
    pub kind: i32,
    /// The attack's active part is over: the host state may leave it (crouch
    /// to stand, stand to crouch).
    pub interruptible: bool,
    /// Animation to go back to when the attack ends.
    pub restore_anim: Option<String>,
}

/// Starts an attack when a button asks for one; true when one started.
pub fn select(c: &mut Ctx) -> bool {
    let _ = c;
    false
}

/// One tick of the running attack, in place of the host's animation step.
/// Until attacks are filled in it advances the animation.
pub fn update(c: &mut Ctx) {
    c.advance_anim();
}

/// Ends the running attack.
pub fn cancel(c: &mut Ctx) {
    c.p.attacking = false;
}

/// True while a jump attack runs that survives hitting a ceiling.
pub fn air_attack_running(c: &Ctx) -> bool {
    let _ = c;
    false
}

/// Called when the stand animation reports [`STAND_FRAME_EVENT`].
pub fn frame_event(c: &mut Ctx, event: i32) {
    let _ = (c, event);
}
