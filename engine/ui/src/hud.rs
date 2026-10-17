//! The status bar and the in-game messages, as screen sprites.
//!
//! The status bar is drawn in the level palette after the world, so fades
//! and transitions act on it like on the rest of the picture. Its pieces,
//! in drawing order, with positions as hotspots in game pixels from the
//! edges of the picture (`L`, `T` the left and top edges, `R` the right
//! edge, inclusive):
//!
//! | Piece | Image set | Frame | Position |
//! | --- | --- | --- | --- |
//! | treasure chest | `GAME_INTERFACE_TREASURECHEST` | its animation, by time | `(L + cx - ox, T + 15)` |
//! | score, 8 digits | `GAME_INTERFACE_SCORENUMBERS` | the digit | `(L + 36 + 11 i, T + 15)` |
//! | heart | `GAME_INTERFACE_HEALTHHEART` | 1, 2, 3, one step per `300 - 2 * health` ms | `(R + 3 - ox - cx, T + 15)` |
//! | health, 3 digits | `GAME_INTERFACE_HEALTHNUMBERS` | the digit | `(R - 52 + 11 i, T + 14)` |
//! | weapon | `GAME_INTERFACE_WEAPONS_PISTOL`, `_MAGIC`, `_DYNAMITE` | its animation, by time since the weapon changed | `(R + 1 - ox - cx, T + 46)` |
//! | ammunition, 2 digits | `GAME_INTERFACE_SMALLNUMBERS` | the digit | `(R - 42 + 8 i, T + 46)` |
//! | Claw's head | `GAME_INTERFACE_LIVESHEAD` | its animation, by time | `(R + 3 - ox - cx, T + 72)` |
//! | spare lives, 1 digit | `GAME_INTERFACE_SMALLNUMBERS` | the digit | `(R - 32, T + 73)` |
//! | powerup stopwatch | `GAME_INTERFACE_STOPWATCH` | every frame in turn, 100 ms each | `(L + cx - ox, T + 48)` |
//! | powerup seconds, 3 digits | `GAME_INTERFACE_SCORENUMBERS` | the digit | `(L + 36 + 11 i, T + 49)` |
//!
//! `cx` is half the width (rounded down) and `ox` the horizontal hotspot
//! offset of the set's first frame (for the weapon, of the pistol set), read
//! from the loaded art, so the chest and stopwatch start at the left edge
//! and the heart, weapon and head end at the right edge. The heart and the
//! head end two pixels past the right edge and are cut there, as in the
//! original game.
//!
//! Numbers are shown zero-padded to their digit count, keeping only the
//! low-order digits of a longer number: the score modulo 10^8, health
//! (never below 0) modulo 1000, the current weapon's ammunition modulo 100,
//! the spare lives (lives - 1, never below 0) modulo 10 and the powerup's
//! whole seconds modulo 1000. The heart steps faster as health grows: every
//! 300 ms at 0 health and every 100 ms at 100 or more. The stopwatch and
//! its digits show only while a powerup has time left. The animations run
//! on the frame's simulation time.
//!
//! During a boss fight two more pieces show, both on the original 640x480
//! view and drawn before the status bar (above the playfield, below the
//! bar):
//!
//! | Piece | Image set | Position |
//! | --- | --- | --- |
//! | the boss's name banner ([`HudState::boss_banner`]) | `LEVEL_BOSSNAME` | its centre `(x, y)` on the 640x480 view, centred in a larger picture |
//! | the boss's health bar ([`HudState::boss_meter`]) | `GAME_BOSSBAR` | `((W - 1) / 2 + 1, H - 33)`: (320, 447) at 640x480 |
//!
//! The health bar's level is a 226 x 7 px band from 18 px right of the
//! bar frame's left edge and 11 px below its top: `pct = trunc(value * 100
//! / max)`, limited to 0..=100; at 100 the band is all full colour (palette
//! index 0x34), at 0 or less all empty colour (0x7E), otherwise empty with
//! its left `trunc(0.01 * pct * 226)` px full ([`boss_meter_layout`]). The
//! band is drawn right after the bar's frame, as solid sprites the status
//! bar makes itself when its art loads ([`BOSS_BAND_FULL`],
//! [`BOSS_BAND_EMPTY`]): pieces 7 px high and 1, 2, 4 ... 128 px wide, so
//! each part of the band takes at most eight of them.
//!
//! Messages ("Your game has been saved") are one line of the game's bitmap
//! font, centred on the picture's centre column with its top 110 pixels
//! below the top edge. One message shows at a time; a new one replaces it
//! and restarts its time. Messages age only while the game runs, and the
//! caller hides them during screen transitions.

use claw_assets::{Ani, AssetStore, Frame, Registries};
use claw_frame::output::BossMeter;
use claw_frame::{DrawMode, GameEvent, HudState, Weapon};
use claw_render::{Anchor, FrameInfo, LevelGraphics, ScreenSprite};

/// The treasure chest beside the score.
pub const CHEST: &str = "GAME_INTERFACE_TREASURECHEST";
/// Score and powerup-second digits.
pub const SCORE_DIGITS: &str = "GAME_INTERFACE_SCORENUMBERS";
/// The health heart.
pub const HEART: &str = "GAME_INTERFACE_HEALTHHEART";
/// Health digits.
pub const HEALTH_DIGITS: &str = "GAME_INTERFACE_HEALTHNUMBERS";
/// The pistol icon.
pub const PISTOL: &str = "GAME_INTERFACE_WEAPONS_PISTOL";
/// The magic icon.
pub const MAGIC: &str = "GAME_INTERFACE_WEAPONS_MAGIC";
/// The dynamite icon.
pub const DYNAMITE: &str = "GAME_INTERFACE_WEAPONS_DYNAMITE";
/// Ammunition and lives digits.
pub const SMALL_DIGITS: &str = "GAME_INTERFACE_SMALLNUMBERS";
/// Claw's head beside the lives.
pub const HEAD: &str = "GAME_INTERFACE_LIVESHEAD";
/// The powerup stopwatch.
pub const STOPWATCH: &str = "GAME_INTERFACE_STOPWATCH";
/// The countdown clock (shown when [`HudState::timer_ms`] is set).
pub const TIMER: &str = "GAME_INTERFACE_TIME";
/// The game's bitmap font.
pub const FONT: &str = "GAME_FONT";

/// Every image set the status bar and the messages draw. Preload them with
/// the level so nothing is read or decoded while frames are drawn.
pub const IMAGE_SETS: &[&str] = &[
    CHEST,
    SCORE_DIGITS,
    HEART,
    HEALTH_DIGITS,
    PISTOL,
    MAGIC,
    DYNAMITE,
    SMALL_DIGITS,
    HEAD,
    STOPWATCH,
    TIMER,
    FONT,
];

/// The boss's health bar frame.
pub const BOSS_BAR: &str = "GAME_BOSSBAR";
/// The boss's name banner, the level's own art.
pub const BOSS_NAME: &str = "LEVEL_BOSSNAME";

/// The image sets of a boss fight's pieces. They are preloaded with the
/// status bar's, but only boss levels have a banner, so a missing one is
/// not reported.
pub const BOSS_IMAGE_SETS: &[&str] = &[BOSS_BAR, BOSS_NAME];

/// The health bar's full colour, as solid pieces made by
/// [`HudArt::load`]: frame `w` is `w` px wide and [`BOSS_FILL_HEIGHT`] high
/// for every power of two `w` up to [`BOSS_BAND_PIECE_MAX`], with its
/// hotspot on its top-left pixel.
pub const BOSS_BAND_FULL: &str = "STATUSBAR_BOSS_BAND_FULL";
/// The health bar's empty colour, made like [`BOSS_BAND_FULL`].
pub const BOSS_BAND_EMPTY: &str = "STATUSBAR_BOSS_BAND_EMPTY";
/// Width of the widest band piece.
pub const BOSS_BAND_PIECE_MAX: i32 = 128;

/// The original view the banner moves on.
pub const BOSS_VIEW: (i32, i32) = (640, 480);
/// The health bar's hotspot row is `H - 33` in a picture `H` rows high:
/// 32 rows above its last row (447 at 480).
pub const BOSS_BAR_RISE: i32 = 33;
/// The level band starts this far right of the bar frame's left edge.
pub const BOSS_FILL_LEFT: i32 = 18;
/// The level band starts this far below the bar frame's top edge.
pub const BOSS_FILL_TOP: i32 = 11;
/// Width of the level band.
pub const BOSS_FILL_WIDTH: i32 = 226;
/// Height of the level band.
pub const BOSS_FILL_HEIGHT: i32 = 7;
/// Palette index of the band's full part.
pub const BOSS_FULL_INDEX: u8 = 0x34;
/// Palette index of the band's empty part.
pub const BOSS_EMPTY_INDEX: u8 = 0x7E;

/// The chest animation.
pub const CHEST_ANIMATION: &str = "GAME_INTERFACE_CHEST";
/// The pistol icon animation.
pub const PISTOL_ANIMATION: &str = "GAME_INTERFACE_PISTOL";
/// The magic icon animation.
pub const MAGIC_ANIMATION: &str = "GAME_INTERFACE_MAGIC";
/// The dynamite icon animation.
pub const DYNAMITE_ANIMATION: &str = "GAME_INTERFACE_DYNAMITE";
/// The head animation.
pub const HEAD_ANIMATION: &str = "GAME_INTERFACE_LIVES";
/// The animation that steps the stopwatch: its first frame's length is the
/// step.
pub const STOPWATCH_ANIMATION: &str = "GAME_CYCLE100";

/// Every animation the status bar reads.
pub const ANIMATIONS: &[&str] = &[
    CHEST_ANIMATION,
    PISTOL_ANIMATION,
    MAGIC_ANIMATION,
    DYNAMITE_ANIMATION,
    HEAD_ANIMATION,
    STOPWATCH_ANIMATION,
];

/// Stopwatch step when its animation is missing, ms.
pub const DEFAULT_STOPWATCH_STEP_MS: u32 = 100;

/// Top of the message line below the picture's top edge.
pub const MESSAGE_TOP: i32 = 110;

/// Highest frame number of the bitmap font.
pub const FONT_GLYPHS: i32 = 54;

/// Message id: the game was saved.
pub const MESSAGE_SAVED: u32 = 1;
/// Message id: the game was not saved because cheats were used.
pub const MESSAGE_NOT_SAVED: u32 = 2;
/// Message ids from this one up are notices of the game itself, outside
/// play (display mode, settings).
pub const FIRST_APP_MESSAGE: u32 = 100;

/// The text of a message id from the simulation, if it has one.
pub fn message_text(id: u32) -> Option<&'static str> {
    match id {
        MESSAGE_SAVED => Some("Your game has been saved"),
        MESSAGE_NOT_SAVED => Some("Your game was not saved because you cheated!"),
        _ => None,
    }
}

/// An animation as frame numbers and their lengths in ms.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Animation {
    /// (frame number, length in ms) in order.
    pub frames: Vec<(i32, u32)>,
}

impl Animation {
    /// The frames of an animation file for an image set with the frame
    /// numbers `set` (in order). An animation frame that names no frame of
    /// the set (a pause) keeps showing the frame before it, or the set's
    /// first frame at the start.
    pub fn from_ani(ani: &Ani, set: &[i32]) -> Animation {
        let mut shown = set.first().copied().unwrap_or(-1);
        Animation {
            frames: ani
                .frames
                .iter()
                .map(|f| {
                    let image = i32::from(f.image_index);
                    if set.contains(&image) {
                        shown = image;
                    }
                    (shown, f.duration_ms().max(1) as u32)
                })
                .collect(),
        }
    }

    /// The same frame length for every frame number in `frames`.
    pub fn cycle(frames: &[i32], step_ms: u32) -> Animation {
        Animation {
            frames: frames.iter().map(|&f| (f, step_ms.max(1))).collect(),
        }
    }

    /// Length of one pass, ms.
    pub fn total_ms(&self) -> u64 {
        self.frames.iter().map(|&(_, ms)| u64::from(ms)).sum()
    }

    /// The frame shown `ms` after the animation started, looping; `None`
    /// for an empty animation.
    pub fn frame_at(&self, ms: f64) -> Option<i32> {
        let total = self.total_ms();
        if total == 0 {
            return None;
        }
        let ms = if ms.is_finite() { ms.max(0.0) } else { 0.0 };
        let mut t = (ms.floor() as u64) % total;
        for &(frame, len) in &self.frames {
            if t < u64::from(len) {
                return Some(frame);
            }
            t -= u64::from(len);
        }
        self.frames.last().map(|&(f, _)| f)
    }
}

/// The art the status bar needs, measured from the loaded frames.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HudArt {
    /// Chest hotspot from the left edge.
    pub chest_x: i32,
    /// Heart hotspot from the right edge.
    pub heart_x: i32,
    /// Weapon hotspot from the right edge.
    pub weapon_x: i32,
    /// Head hotspot from the right edge.
    pub head_x: i32,
    /// Stopwatch hotspot from the left edge.
    pub stopwatch_x: i32,
    /// Countdown clock hotspot from the left edge.
    pub timer_x: i32,
    /// Chest animation.
    pub chest: Animation,
    /// Pistol, magic and dynamite animations.
    pub weapons: [Animation; 3],
    /// Head animation.
    pub head: Animation,
    /// Heart frame numbers.
    pub heart_frames: Vec<i32>,
    /// Stopwatch animation.
    pub stopwatch: Animation,
    /// Font glyph sizes by frame number, 0 to [`FONT_GLYPHS`].
    pub glyphs: Vec<Option<FrameInfo>>,
    /// Size and hotspot of the boss's health bar frame, when the art has
    /// it.
    pub boss_bar: Option<FrameInfo>,
    /// Image sets and animations that were not found.
    pub missing: Vec<String>,
}

/// The band pieces in palette index `index`, by frame number (the width).
fn band_pieces(index: u8) -> Vec<(i32, Frame)> {
    let h = BOSS_FILL_HEIGHT as u32;
    std::iter::successors(Some(1), |&w| (w < BOSS_BAND_PIECE_MAX).then_some(w * 2))
        .map(|w| {
            let frame = Frame {
                width: w as u32,
                height: h,
                // The hotspot on the top-left pixel.
                offset_x: w >> 1,
                offset_y: (h >> 1) as i32,
                flags: 0,
                indices: vec![index; (w as u32 * h) as usize],
                opaque: None,
            };
            (w, frame)
        })
        .collect()
}

fn left_x(info: Option<FrameInfo>) -> i32 {
    info.map_or(0, |f| (f.width >> 1) as i32 - f.offset_x)
}

fn right_x(info: Option<FrameInfo>, pad: i32) -> i32 {
    info.map_or(0, |f| pad - f.offset_x - (f.width >> 1) as i32)
}

impl HudArt {
    /// Measures the status bar art in `graphics` (preloading its image
    /// sets) with animations from `animation`.
    pub fn load(graphics: &mut LevelGraphics, animation: impl Fn(&str) -> Option<Ani>) -> HudArt {
        graphics.preload(IMAGE_SETS);
        graphics.preload(BOSS_IMAGE_SETS);
        graphics.insert_set(BOSS_BAND_FULL, &band_pieces(BOSS_FULL_INDEX));
        graphics.insert_set(BOSS_BAND_EMPTY, &band_pieces(BOSS_EMPTY_INDEX));
        let boss_bar = graphics.frame_info(BOSS_BAR, -1);
        let mut missing = Vec::new();
        for key in IMAGE_SETS {
            if graphics.frame_numbers(key).is_empty() {
                missing.push((*key).to_string());
            }
        }
        let mut anim = |key: &str, set: &str| match animation(key) {
            Some(a) => Animation::from_ani(&a, &graphics.frame_numbers(set)),
            None => {
                missing.push(key.to_string());
                Animation::default()
            }
        };
        let chest = anim(CHEST_ANIMATION, CHEST);
        let weapons = [
            anim(PISTOL_ANIMATION, PISTOL),
            anim(MAGIC_ANIMATION, MAGIC),
            anim(DYNAMITE_ANIMATION, DYNAMITE),
        ];
        let head = anim(HEAD_ANIMATION, HEAD);
        let step = match animation(STOPWATCH_ANIMATION) {
            Some(a) => a
                .frames
                .first()
                .map_or(DEFAULT_STOPWATCH_STEP_MS, |f| f.duration_ms().max(1) as u32),
            None => {
                missing.push(STOPWATCH_ANIMATION.to_string());
                DEFAULT_STOPWATCH_STEP_MS
            }
        };
        let mut first = |key: &str| graphics.frame_info(key, -1);
        let (chest_info, heart_info, pistol_info) = (first(CHEST), first(HEART), first(PISTOL));
        let (head_info, watch_info, timer_info) = (first(HEAD), first(STOPWATCH), first(TIMER));
        let stopwatch = Animation::cycle(&graphics.frame_numbers(STOPWATCH), step);
        let heart_frames = graphics.frame_numbers(HEART);
        let glyphs = (0..=FONT_GLYPHS)
            .map(|n| graphics.frame_info(FONT, n))
            .collect();
        HudArt {
            chest_x: left_x(chest_info),
            heart_x: right_x(heart_info, 3),
            weapon_x: right_x(pistol_info, 1),
            head_x: right_x(head_info, 3),
            stopwatch_x: left_x(watch_info),
            timer_x: left_x(timer_info),
            chest,
            weapons,
            head,
            heart_frames,
            stopwatch,
            glyphs,
            boss_bar,
            missing,
        }
    }

    /// [`HudArt::load`] with animations read from `store` through the
    /// level's `registries`.
    pub fn from_store(
        graphics: &mut LevelGraphics,
        store: &AssetStore,
        registries: &Registries,
    ) -> HudArt {
        HudArt::load(graphics, |key| {
            let id = *registries.anis.get(key)?;
            Ani::parse(&store.read(id).ok()?).ok()
        })
    }
}

/// The last `count` decimal digits of `value`, zero-padded, most
/// significant first.
pub fn digits(value: u64, count: usize) -> Vec<i32> {
    let mut out = vec![0; count];
    let mut v = value;
    for d in out.iter_mut().rev() {
        *d = (v % 10) as i32;
        v /= 10;
    }
    out
}

/// How long each heart frame shows at `health`, ms.
pub fn heart_step_ms(health: i32) -> u32 {
    (300 - 2 * health.clamp(0, 100)) as u32
}

/// The heart frame shown at `time_ms` with `health`, from `frames` (the
/// set's frame numbers in order); -1 (the first frame) when there are
/// none.
pub fn heart_frame(frames: &[i32], health: i32, time_ms: f64) -> i32 {
    if frames.is_empty() {
        return -1;
    }
    let t = if time_ms.is_finite() {
        time_ms.max(0.0)
    } else {
        0.0
    };
    let step = (t / f64::from(heart_step_ms(health))).floor() as u64;
    frames[(step % frames.len() as u64) as usize]
}

/// The ammunition the status bar shows for the selected weapon.
pub fn ammo(hud: &HudState) -> i32 {
    match hud.weapon {
        Weapon::Pistol => hud.pistol_ammo,
        Weapon::Magic => hud.magic_ammo,
        Weapon::Dynamite => hud.dynamite_ammo,
    }
}

fn weapon_index(w: Weapon) -> usize {
    match w {
        Weapon::Pistol => 0,
        Weapon::Magic => 1,
        Weapon::Dynamite => 2,
    }
}

fn weapon_key(w: Weapon) -> &'static str {
    [PISTOL, MAGIC, DYNAMITE][weapon_index(w)]
}

fn sprite(key: &str, frame: i32, anchor: Anchor, x: i32, y: i32) -> ScreenSprite {
    ScreenSprite {
        key: key.to_string(),
        frame,
        x,
        y,
        anchor,
        mode: DrawMode::NORMAL,
    }
}

fn digit_row(
    out: &mut Vec<ScreenSprite>,
    key: &str,
    value: u64,
    count: usize,
    anchor: Anchor,
    (x, y): (i32, i32),
    spacing: i32,
) {
    for (i, d) in digits(value, count).into_iter().enumerate() {
        out.push(sprite(key, d, anchor, x + spacing * i as i32, y));
    }
}

/// Where the status bar is placed in the picture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Placement {
    /// Keep the status bar inside the centred 640-pixel-wide area of a
    /// wider picture, as on the original screen, instead of at the
    /// picture's edges.
    pub safe_area: bool,
}

impl Placement {
    /// Extra offsets of left-anchored and right-anchored pieces for a
    /// presentation `presentation`.
    fn insets(self, presentation: (u32, u32)) -> (i32, i32) {
        let extra = presentation.0.saturating_sub(640) as i32;
        if !self.safe_area || extra == 0 {
            return (0, 0);
        }
        let left = extra / 2;
        (left, -(extra - left))
    }
}

/// The status bar for `hud` at simulation time `time_ms` in a picture of
/// `presentation` game pixels, with the weapon animation timed from the
/// start of the game. [`Hud`] times it from the last weapon change.
pub fn layout(
    art: &HudArt,
    hud: &HudState,
    presentation: (u32, u32),
    time_ms: f64,
) -> Vec<ScreenSprite> {
    layout_with(
        art,
        hud,
        presentation,
        time_ms,
        time_ms,
        Placement::default(),
    )
}

/// [`layout`] with the weapon animation `weapon_ms` into its loop and a
/// placement.
pub fn layout_with(
    art: &HudArt,
    hud: &HudState,
    presentation: (u32, u32),
    time_ms: f64,
    weapon_ms: f64,
    placement: Placement,
) -> Vec<ScreenSprite> {
    let (l, r) = placement.insets(presentation);
    let (tl, tr) = (Anchor::TopLeft, Anchor::TopRight);
    let mut out = Vec::with_capacity(24);
    if let Some(centre) = hud.boss_banner {
        out.push(boss_banner_sprite(centre, presentation));
    }
    if let Some(m) = hud.boss_meter {
        let meter = boss_meter_layout(art, m, presentation);
        out.push(meter.bar);
        out.extend(meter.fills.iter().flat_map(band_sprites));
    }
    let chest = art.chest.frame_at(time_ms).unwrap_or(-1);
    out.push(sprite(CHEST, chest, tl, l + art.chest_x, 15));
    digit_row(
        &mut out,
        SCORE_DIGITS,
        u64::from(hud.score),
        8,
        tl,
        (l + 36, 15),
        11,
    );
    let heart = heart_frame(&art.heart_frames, hud.health, time_ms);
    out.push(sprite(HEART, heart, tr, r + art.heart_x, 15));
    digit_row(
        &mut out,
        HEALTH_DIGITS,
        hud.health.max(0) as u64,
        3,
        tr,
        (r - 52, 14),
        11,
    );
    let w = hud.weapon;
    let icon = art.weapons[weapon_index(w)]
        .frame_at(weapon_ms)
        .unwrap_or(-1);
    out.push(sprite(weapon_key(w), icon, tr, r + art.weapon_x, 46));
    digit_row(
        &mut out,
        SMALL_DIGITS,
        ammo(hud).max(0) as u64,
        2,
        tr,
        (r - 42, 46),
        8,
    );
    let head = art.head.frame_at(time_ms).unwrap_or(-1);
    out.push(sprite(HEAD, head, tr, r + art.head_x, 72));
    let spare = (hud.lives - 1).max(0) as u64;
    digit_row(&mut out, SMALL_DIGITS, spare, 1, tr, (r - 32, 73), 8);
    if let Some(p) = hud.powerup.filter(|p| p.remaining_ms > 0) {
        let watch = art.stopwatch.frame_at(time_ms).unwrap_or(-1);
        out.push(sprite(STOPWATCH, watch, tl, l + art.stopwatch_x, 48));
        digit_row(
            &mut out,
            SCORE_DIGITS,
            u64::from(p.remaining_ms / 1000),
            3,
            tl,
            (l + 36, 49),
            11,
        );
    } else if let Some(ms) = hud.timer_ms {
        out.push(sprite(TIMER, 1, tl, l + art.timer_x, 48));
        digit_row(
            &mut out,
            SCORE_DIGITS,
            u64::from(ms / 1000),
            3,
            tl,
            (l + 36, 49),
            11,
        );
    }
    out
}

/// A solid rectangle in the picture, in game pixels from its top-left
/// corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FillRect {
    /// Left edge.
    pub left: i32,
    /// Top edge.
    pub top: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Palette index.
    pub index: u8,
}

/// The boss's health bar: its frame and the level band over it.
#[derive(Debug, Clone, PartialEq)]
pub struct BossMeterLayout {
    /// The bar's frame.
    pub bar: ScreenSprite,
    /// The level band: the full part, then the empty part, each left out
    /// when it has no width.
    pub fills: Vec<FillRect>,
}

/// The band piece sprites that cover `fill`: the widest pieces first, left
/// to right ([`BOSS_BAND_FULL`] or [`BOSS_BAND_EMPTY`] by its index; other
/// indices draw nothing).
pub fn band_sprites(fill: &FillRect) -> Vec<ScreenSprite> {
    let key = match fill.index {
        BOSS_FULL_INDEX => BOSS_BAND_FULL,
        BOSS_EMPTY_INDEX => BOSS_BAND_EMPTY,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut left = fill.left;
    let mut rest = fill.width as i32;
    let mut piece = BOSS_BAND_PIECE_MAX;
    while rest > 0 {
        while piece > rest {
            piece >>= 1;
        }
        out.push(sprite(key, piece, Anchor::TopLeft, left, fill.top));
        left += piece;
        rest -= piece;
    }
    out
}

/// The percentage of the bar that shows full: `trunc(value * 100 / max)`
/// limited to 0..=100 (0 without a maximum).
pub fn boss_percent(m: BossMeter) -> i32 {
    if m.max <= 0 {
        return 0;
    }
    let pct = f64::from(m.value) * 100.0 / f64::from(m.max);
    (pct.trunc() as i64).clamp(0, 100) as i32
}

/// Width of the band's full part at `pct`: all of it at 100, none at 0 or
/// less, otherwise `trunc(0.01 * pct * 226)` px.
pub fn boss_full_width(pct: i32) -> i32 {
    if pct >= 100 {
        BOSS_FILL_WIDTH
    } else if pct <= 0 {
        0
    } else {
        (0.01 * f64::from(pct) * f64::from(BOSS_FILL_WIDTH)) as i32
    }
}

/// The offset that centres the original 640x480 view in a picture of
/// `presentation` game pixels.
fn view_offset(presentation: (u32, u32)) -> (i32, i32) {
    (
        (presentation.0 as i32 - BOSS_VIEW.0) / 2,
        (presentation.1 as i32 - BOSS_VIEW.1) / 2,
    )
}

/// The boss's name banner centred at `centre` on the original 640x480
/// view, in a picture of `presentation` game pixels.
pub fn boss_banner_sprite(centre: (i32, i32), presentation: (u32, u32)) -> ScreenSprite {
    let (dx, dy) = view_offset(presentation);
    sprite(BOSS_NAME, -1, Anchor::TopLeft, centre.0 + dx, centre.1 + dy)
}

/// The boss's health bar showing `m` in a picture of `presentation` game
/// pixels: the frame centred near the bottom edge, and the level band
/// measured from the frame's edges.
pub fn boss_meter_layout(art: &HudArt, m: BossMeter, presentation: (u32, u32)) -> BossMeterLayout {
    let (w, h) = (presentation.0 as i32, presentation.1 as i32);
    let (x, y) = ((w - 1) / 2 + 1, h - BOSS_BAR_RISE);
    let bar = sprite(BOSS_BAR, -1, Anchor::TopLeft, x, y);
    let info = art.boss_bar.unwrap_or(FrameInfo {
        width: 260,
        height: 28,
        offset_x: 0,
        offset_y: 0,
    });
    let left = x - (info.width >> 1) as i32 + info.offset_x + BOSS_FILL_LEFT;
    let top = y - (info.height >> 1) as i32 + info.offset_y + BOSS_FILL_TOP;
    let full = boss_full_width(boss_percent(m));
    let rect = |left: i32, width: i32, index: u8| FillRect {
        left,
        top,
        width: width as u32,
        height: BOSS_FILL_HEIGHT as u32,
        index,
    };
    let mut fills = Vec::with_capacity(2);
    if full > 0 {
        fills.push(rect(left, full, BOSS_FULL_INDEX));
    }
    if full < BOSS_FILL_WIDTH {
        fills.push(rect(left + full, BOSS_FILL_WIDTH - full, BOSS_EMPTY_INDEX));
    }
    BossMeterLayout { bar, fills }
}

/// The font frame of a character: space 0, letters 1 to 26 (either case),
/// digits 27 to 36, then `, . ! ? = < > ( ) [ ] @ # $ % ^ & *` as 37 to
/// 54; anything else shows as a space.
pub fn glyph(c: char) -> i32 {
    const PUNCTUATION: &str = ",.!?=<>()[]@#$%^&*";
    match c {
        'A'..='Z' => c as i32 - 'A' as i32 + 1,
        'a'..='z' => c as i32 - 'a' as i32 + 1,
        '0'..='9' => c as i32 - '0' as i32 + 27,
        _ => PUNCTUATION
            .chars()
            .position(|p| p == c)
            .map_or(0, |i| 37 + i as i32),
    }
}

/// One line of text in the bitmap font, centred on the picture's centre
/// column with the top of its tallest glyph [`MESSAGE_TOP`] pixels below
/// the top edge. Each glyph advances the line by its own width.
pub fn text_layout(art: &HudArt, text: &str) -> Vec<ScreenSprite> {
    let glyphs: Vec<(i32, FrameInfo)> = text
        .chars()
        .map(|c| {
            let n = glyph(c);
            let info = art
                .glyphs
                .get(n as usize)
                .copied()
                .flatten()
                .unwrap_or_default();
            (n, info)
        })
        .collect();
    let width: i32 = glyphs.iter().map(|(_, f)| f.width as i32).sum();
    let rise = glyphs
        .iter()
        .filter(|(_, f)| f.width > 0)
        .map(|(_, f)| (f.height >> 1) as i32 - f.offset_y)
        .max()
        .unwrap_or(0);
    let y = MESSAGE_TOP + rise;
    let mut pen = -(width / 2);
    let mut out = Vec::with_capacity(glyphs.len());
    for (n, f) in glyphs {
        if f.width > 0 && n != 0 {
            let x = pen + (f.width >> 1) as i32 - f.offset_x;
            out.push(sprite(FONT, n, Anchor::TopCenter, x, y));
        }
        pen += f.width as i32;
    }
    out
}

/// The message on screen, if any.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Messages {
    text: Option<String>,
    left_ms: f64,
}

impl Messages {
    /// Shows `text` for `ms`, replacing any message on screen.
    pub fn show(&mut self, text: impl Into<String>, ms: u32) {
        let text = text.into();
        if text.is_empty() || ms == 0 {
            return;
        }
        self.text = Some(text);
        self.left_ms = f64::from(ms);
    }

    /// Shows the message of a [`GameEvent::Message`] with a known text;
    /// true when it did.
    pub fn event(&mut self, event: &GameEvent) -> bool {
        if let GameEvent::Message { id, ms } = *event
            && let Some(text) = message_text(id)
        {
            self.show(text, ms);
            return true;
        }
        false
    }

    /// Lets `ms` of game time pass.
    pub fn advance(&mut self, ms: f64) {
        if self.text.is_none() || !ms.is_finite() || ms <= 0.0 {
            return;
        }
        self.left_ms -= ms;
        if self.left_ms <= 0.0 {
            self.clear();
        }
    }

    /// Removes the message on screen.
    pub fn clear(&mut self) {
        self.text = None;
        self.left_ms = 0.0;
    }

    /// The text on screen.
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// Time the message stays on screen, ms.
    pub fn left_ms(&self) -> f64 {
        self.left_ms
    }
}

/// The status bar and messages of a game in progress: the art, the time of
/// the last weapon change and the message on screen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Hud {
    art: HudArt,
    weapon: Option<(Weapon, f64)>,
    /// The message on screen.
    pub messages: Messages,
    /// Where the status bar is placed.
    pub placement: Placement,
}

impl Hud {
    /// A status bar drawn with `art`.
    pub fn new(art: HudArt) -> Hud {
        Hud {
            art,
            ..Hud::default()
        }
    }

    /// The art.
    pub fn art(&self) -> &HudArt {
        &self.art
    }

    /// The status bar for `hud` at simulation time `time_ms`; the weapon
    /// animation restarts whenever the weapon changes (or time runs
    /// backwards, as on a level restart).
    pub fn layout(
        &mut self,
        hud: &HudState,
        presentation: (u32, u32),
        time_ms: f64,
    ) -> Vec<ScreenSprite> {
        let since = match self.weapon {
            Some((w, at)) if w == hud.weapon && at <= time_ms => at,
            _ => {
                self.weapon = Some((hud.weapon, time_ms));
                time_ms
            }
        };
        layout_with(
            &self.art,
            hud,
            presentation,
            time_ms,
            time_ms - since,
            self.placement,
        )
    }

    /// The message on screen as sprites (empty when there is none).
    pub fn message_layout(&self) -> Vec<ScreenSprite> {
        self.messages
            .text()
            .map(|t| text_layout(&self.art, t))
            .unwrap_or_default()
    }
}
