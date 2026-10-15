//! The end of a level: the map screen, then the treasure tally.
//!
//! **Map screen.** Odd levels up to 11 add a piece to the treasure map;
//! the others show an amulet gem. On a map level the map loop starts at
//! volume 25 and rises to 80 over 750 ms; the piece waits (5 s on level 1,
//! 2 s on levels 3 to 11), then flies in a straight line from its start to
//! its place on the map in 2 s while the curling sound plays; on arrival the
//! curling stops and the impact sound plays. When the impact has finished,
//! Claw says his line for the level, and when he has finished the tally
//! follows. On a gem level the amulet sound plays, the gem shows for 2 s,
//! then Claw's line. Any skip input goes to the tally at once.
//!
//! **Tally.** Nine rows, one per treasure kind `k`, from `k = 8` at the
//! top (y = 55 + 44 (8 - k)) to `k = 0`. Rows take their turn one at a
//! time from the top: the kind's icon flies in from x = -32 to 160 in 300
//! ms, the row's sound plays and the row shows "count OF total TIMES value
//! EQUALS product", with total = max(collected, the level's count of that
//! kind). Then, every `trunc((25 + trunc(25 r / c)) f)` ms (r the count
//! still to add, c the collected count, f the kind's speed factor) a bounce
//! sound plays and `min(step, c - count)` is added to the count, its value
//! to the product and to both totals at the bottom (the level total from 0,
//! the game total from the score at the level's end minus the level's
//! treasure points). Numbers start with their first digit's hotspot at
//! their position. A row with nothing collected finishes at once; a row
//! that collected every item of its kind plays the cat-and-magnet sound and
//! glitters. After the last row, when every row collected everything (and
//! the level is not a community custom level) "Perfect" shows, its sound
//! plays and Claw gets an extra life if he has fewer than 10. The tally
//! then holds for 60 s, after which every row slides off (25 moves of 25
//! pixels every `18 - 2k` ms, even kinds left and odd kinds right) and the
//! totals slide off after row 1 (25 moves of 25 pixels every 75 ms).
//!
//! The first skip input completes every row at once (its final numbers,
//! without the counting sounds); a skip during the hold ends the hold.
//! The tally's numbers are for show: the points were already added to the
//! score in play.

use std::collections::BTreeMap;

use claw_assets::Palette;
use claw_frame::{AudioCue, SoundChannel, SoundCue};
use claw_render::{Anchor, ImageId, ScreenScene, ScreenSprite};

/// Treasure kinds.
pub const KINDS: usize = 9;

/// One treasure kind in the tally.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreasureKind {
    /// Icon image set.
    pub set: &'static str,
    /// Points per item.
    pub value: u32,
    /// Items added per counting step.
    pub step: u32,
    /// Counting speed factor: the interval is multiplied by it.
    pub factor: f64,
    /// Sound of the row's turn.
    pub sound: &'static str,
}

/// The nine treasure kinds, by kind number.
pub const TREASURES: [TreasureKind; KINDS] = [
    TreasureKind {
        set: "BOOTY_TREASURE_COINS",
        value: 100,
        step: 10,
        factor: 1.0,
        sound: "BOOTY_COIN",
    },
    TreasureKind {
        set: "BOOTY_TREASURE_GOLDBARS",
        value: 500,
        step: 1,
        factor: 4.0,
        sound: "BOOTY_PICKUP1",
    },
    TreasureKind {
        set: "BOOTY_TREASURE_RINGS_RED",
        value: 1500,
        step: 1,
        factor: 3.0,
        sound: "BOOTY_RINGS",
    },
    TreasureKind {
        set: "BOOTY_TREASURE_CHALICES_GREEN",
        value: 2500,
        step: 1,
        factor: 3.0,
        sound: "BOOTY_PICKUP2",
    },
    TreasureKind {
        set: "BOOTY_TREASURE_CROSSES_BLUE",
        value: 5000,
        step: 1,
        factor: 3.0,
        sound: "BOOTY_CROSS",
    },
    TreasureKind {
        set: "BOOTY_TREASURE_SCEPTERS_PURPLE",
        value: 7500,
        step: 1,
        factor: 4.0,
        sound: "BOOTY_SCEPTER",
    },
    TreasureKind {
        set: "BOOTY_TREASURE_GECKOS_RED",
        value: 10000,
        step: 1,
        factor: 4.0,
        sound: "BOOTY_SCEPTER",
    },
    TreasureKind {
        set: "BOOTY_TREASURE_CROWNS_GREEN",
        value: 15000,
        step: 1,
        factor: 6.0,
        sound: "BOOTY_SCEPTER",
    },
    TreasureKind {
        set: "BOOTY_TREASURE_JEWELEDSKULL_BLUE",
        value: 25000,
        step: 1,
        factor: 6.0,
        sound: "BOOTY_SCEPTER",
    },
];

/// Digits of the tally's numbers.
pub const DIGITS: &str = "BOOTY_SCORENUMBERS";
/// OF, TIMES, EQUALS and the two totals' labels (frames 1 to 5).
pub const SYMBOLS: &str = "BOOTY_SYMBOLS";
/// The "Perfect" sign.
pub const PERFECT: &str = "BOOTY_PERFECT";
/// The glitter on a complete row's icon.
pub const GLITTER: &str = "BOOTY_GLITTER";
/// The map loop.
pub const MAP_LOOP: &str = "BOOTY_MAPLOOP";
/// The map piece's flight.
pub const CURLING: &str = "BOOTY_CURLING1";
/// The map piece's arrival.
pub const IMPACT: &str = "BOOTY_IMPACT3";
/// The gem levels' sound.
pub const AMULET: &str = "BOOTY_AMULET";
/// Every counting step.
pub const BOUNCE: &str = "BOOTY_BOUNCE1";
/// A row that collected everything.
pub const COMPLETE: &str = "BOOTY_CATNMAG";
/// A perfect tally.
pub const PERFECT_SOUND: &str = "BOOTY_PERFECT";

/// Icon's final x.
pub const ICON_X: i32 = 160;
/// Icon's start x.
pub const ICON_START_X: i32 = -32;
/// Icon flight, ms.
pub const ICON_FLIGHT_MS: f64 = 300.0;
/// The finished tally's hold, ms.
pub const HOLD_MS: f64 = 60_000.0;
/// Map piece flight, ms.
pub const FLIGHT_MS: f64 = 2000.0;
/// How long a gem shows, ms.
pub const GEM_MS: f64 = 2000.0;
/// Most lives.
pub const MAX_LIVES: i32 = 10;

/// Sound handles of the screen.
pub const MAP_LOOP_VOICE: u32 = 0x7F00_0010;
/// Handle of the curling sound.
pub const CURLING_VOICE: u32 = 0x7F00_0011;

/// What the tally counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootyStats {
    /// The level, 1 to 14.
    pub level: u32,
    /// A community custom level: no map screen and no perfect bonus.
    pub custom: bool,
    /// Items collected in the level, per kind.
    pub collected: [u32; KINDS],
    /// Items the level holds, per kind.
    pub census: [u32; KINDS],
    /// The score at the level's end.
    pub score: u32,
    /// Claw's lives at the level's end.
    pub lives: i32,
}

/// The pictures and sound lengths the screen needs.
#[derive(Debug, Clone, PartialEq)]
pub struct BootyArt {
    /// The map screen's picture and palette.
    pub map: (Option<ImageId>, Palette),
    /// The tally's picture and palette.
    pub tally: (Option<ImageId>, Palette),
    /// Length of sounds by key, ms; a missing sound counts as 0.
    pub sound_ms: BTreeMap<String, u32>,
    /// Width of a digit.
    pub digit_width: i32,
    /// Distance from the game and level totals' labels to their numbers.
    pub total_offsets: (i32, i32),
}

/// The map piece's start and end for piece `k` (levels 1, 3, ... 11).
pub const PIECE_PATHS: [((i32, i32), (i32, i32)); 6] = [
    ((-100, -240), (560, 385)),
    ((740, -240), (109, 316)),
    ((-100, -250), (275, 222)),
    ((740, -250), (316, 363)),
    ((-100, -240), (502, 293)),
    ((-100, 640), (483, 112)),
];

/// Claw's line on the map screen by level.
pub const CLAW_LINES: [&str; 14] = [
    "BOOTY_CLAW_1110004",
    "BOOTY_CLAW_1110023",
    "BOOTY_CLAW_1110005",
    "BOOTY_CLAW_1110024",
    "BOOTY_CLAW_1110006",
    "BOOTY_CLAW_1110025",
    "BOOTY_CLAW_1110007",
    "BOOTY_CLAW_1110026",
    "BOOTY_CLAW_1110008",
    "BOOTY_CLAW_1110027",
    "BOOTY_CLAW_1110009",
    "BOOTY_CLAW_1110028",
    "BOOTY_CLAW_1110002",
    "BOOTY_CLAW_1110003",
];

/// True for the levels that add a map piece.
pub fn is_map_level(level: u32) -> bool {
    level % 2 == 1 && level <= 11
}

/// The map screen's picture of `level`: `SCREENS/<2n-1>MAP` or `AMULET`.
pub fn map_picture(level: u32) -> String {
    let kind = if is_map_level(level) { "MAP" } else { "AMULET" };
    format!("{:03}{kind}", (2 * level).saturating_sub(1))
}

/// The tally's picture of `level`: `SCREENS/<2n>MAP` or `AMULET`, or the
/// custom level picture.
pub fn tally_picture(level: u32, custom: bool) -> String {
    if custom {
        return "030CUSTOM".to_string();
    }
    let kind = if is_map_level(level) { "MAP" } else { "AMULET" };
    format!("{:03}{kind}", 2 * level)
}

/// The image set of `level`'s map piece or gem.
pub fn piece_set(level: u32) -> String {
    if level == 13 {
        "BOOTY_MAPPIECE_LEVEL013_GEMBLUE".to_string()
    } else {
        format!("BOOTY_MAPPIECE_LEVEL{level:03}")
    }
}

/// The counting interval of a row with `remaining` of `collected` still to
/// add and speed factor `factor`, ms.
pub fn count_interval(remaining: u32, collected: u32, factor: f64) -> f64 {
    if collected == 0 {
        return 0.0;
    }
    let base = 25 + (25 * u64::from(remaining) / u64::from(collected)) as i64;
    (base as f64 * factor).trunc()
}

fn cue(key: &str, volume: u8, looped: bool, voice: Option<u32>) -> AudioCue {
    AudioCue::Play(SoundCue {
        key: key.to_string(),
        volume,
        looped,
        channel: SoundChannel::Interface,
        voice,
        ..SoundCue::default()
    })
}

/// Where the screen is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BootyPhase {
    /// The map screen.
    Map,
    /// The tally, rows counting.
    Counting,
    /// The finished tally holding.
    Hold,
    /// The rows sliding off.
    SlideOut,
    /// Finished.
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MapStep {
    Wait,
    Flight,
    Impact,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RowState {
    Waiting,
    FlyIn,
    Counting,
    Finished,
}

/// One row of the tally.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Row {
    /// Kind.
    pub kind: usize,
    /// Collected.
    pub collected: u32,
    /// Total shown: max(collected, the level's count).
    pub total: u32,
    /// Count shown.
    pub count: u32,
    /// Product shown.
    pub product: u64,
    remaining: u32,
    state: RowState,
    timer: f64,
    glitter: bool,
}

impl Row {
    /// True once the row has finished counting.
    pub fn finished(&self) -> bool {
        self.state == RowState::Finished
    }

    /// True once the row shows its numbers.
    pub fn shown(&self) -> bool {
        matches!(self.state, RowState::Counting | RowState::Finished)
    }

    /// The icon's x.
    pub fn icon_x(&self) -> i32 {
        match self.state {
            RowState::Waiting => ICON_START_X,
            RowState::FlyIn => {
                let t = (self.timer / ICON_FLIGHT_MS).clamp(0.0, 1.0);
                ICON_START_X + (f64::from(ICON_X - ICON_START_X) * t).floor() as i32
            }
            _ => ICON_X,
        }
    }

    /// True when the row collected every item of its kind.
    pub fn complete(&self) -> bool {
        self.collected == self.total
    }
}

/// The end-of-level screens.
#[derive(Debug, Clone, PartialEq)]
pub struct BootyScreen {
    stats: BootyStats,
    art: BootyArt,
    phase: BootyPhase,
    elapsed: f64,
    map_step: MapStep,
    step_ms: f64,
    rows: Vec<Row>,
    turn: Option<usize>,
    level_total: u64,
    game_total: u64,
    perfect: bool,
    extra_life: bool,
    loop_fade: Option<(f64, f64, f64, f64)>,
    loop_volume: u8,
    audio: Vec<AudioCue>,
}

impl BootyScreen {
    /// The screens for `stats` with `art`. A community custom level starts
    /// at the tally.
    pub fn new(stats: BootyStats, art: BootyArt) -> BootyScreen {
        let level_points: u64 = (0..KINDS)
            .map(|k| u64::from(stats.collected[k]) * u64::from(TREASURES[k].value))
            .sum();
        let rows = (0..KINDS)
            .rev()
            .map(|k| Row {
                kind: k,
                collected: stats.collected[k],
                total: stats.collected[k].max(stats.census[k]),
                count: 0,
                product: 0,
                remaining: stats.collected[k],
                state: RowState::Waiting,
                timer: 0.0,
                glitter: false,
            })
            .collect();
        let game_total = u64::from(stats.score).saturating_sub(level_points);
        let mut b = BootyScreen {
            phase: if stats.custom {
                BootyPhase::Counting
            } else {
                BootyPhase::Map
            },
            stats,
            art,
            elapsed: 0.0,
            map_step: MapStep::Wait,
            step_ms: 0.0,
            rows,
            turn: None,
            level_total: 0,
            game_total,
            perfect: false,
            extra_life: false,
            loop_fade: None,
            loop_volume: 0,
            audio: Vec::new(),
        };
        if b.phase == BootyPhase::Map {
            if is_map_level(b.stats.level) {
                b.audio.push(cue(MAP_LOOP, 25, true, Some(MAP_LOOP_VOICE)));
                b.loop_volume = 25;
                b.loop_fade = Some((25.0, 80.0, 0.0, 750.0));
            } else {
                b.audio.push(cue(AMULET, 100, false, None));
            }
        } else {
            b.start_tally();
        }
        b
    }

    /// Where the screen is.
    pub fn phase(&self) -> BootyPhase {
        self.phase
    }

    /// The rows, top (kind 8) first.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The level and game totals shown at the bottom.
    pub fn totals(&self) -> (u64, u64) {
        (self.level_total, self.game_total)
    }

    /// True when the tally was perfect.
    pub fn perfect(&self) -> bool {
        self.perfect
    }

    /// True when the perfect tally gave Claw an extra life.
    pub fn extra_life(&self) -> bool {
        self.extra_life
    }

    /// True once the screens have finished.
    pub fn done(&self) -> bool {
        self.phase == BootyPhase::Done
    }

    /// Sounds to play.
    pub fn take_audio(&mut self) -> Vec<AudioCue> {
        std::mem::take(&mut self.audio)
    }

    fn sound_ms(&self, key: &str) -> f64 {
        f64::from(self.art.sound_ms.get(key).copied().unwrap_or(0))
    }

    fn start_tally(&mut self) {
        self.phase = BootyPhase::Counting;
        self.step_ms = 0.0;
        self.start_turn(0);
    }

    fn start_turn(&mut self, i: usize) {
        if i >= self.rows.len() {
            self.turn = None;
            self.finish_counting();
            return;
        }
        self.turn = Some(i);
        self.rows[i].state = RowState::FlyIn;
        self.rows[i].timer = 0.0;
    }

    fn finish_counting(&mut self) {
        self.turn = None;
        let all = self.rows.iter().all(Row::complete);
        if all && !self.stats.custom {
            self.perfect = true;
            self.audio.push(cue(PERFECT_SOUND, 100, false, None));
            if self.stats.lives < MAX_LIVES {
                self.extra_life = true;
            }
        }
        self.phase = BootyPhase::Hold;
        self.step_ms = 0.0;
    }

    fn finish_row(&mut self, i: usize, sounds: bool) {
        let r = &mut self.rows[i];
        let left = r.collected - r.count;
        let value = u64::from(TREASURES[r.kind].value);
        r.count = r.collected;
        r.product = u64::from(r.collected) * value;
        r.remaining = 0;
        r.state = RowState::Finished;
        let added = u64::from(left) * value;
        self.level_total += added;
        self.game_total += added;
        if r.collected > 0 && r.complete() {
            r.glitter = true;
            if sounds {
                self.audio.push(cue(COMPLETE, 100, false, None));
            }
        }
    }

    /// A skip input: Escape, Space, Enter or a gamepad button.
    pub fn skip(&mut self) {
        match self.phase {
            BootyPhase::Map => {
                self.audio.push(AudioCue::Stop {
                    voice: CURLING_VOICE,
                });
                self.start_tally();
            }
            BootyPhase::Counting => {
                for i in 0..self.rows.len() {
                    if !self.rows[i].finished() {
                        self.finish_row(i, false);
                    }
                }
                self.finish_counting();
            }
            BootyPhase::Hold => {
                self.phase = BootyPhase::SlideOut;
                self.step_ms = 0.0;
            }
            BootyPhase::SlideOut | BootyPhase::Done => {}
        }
    }

    /// Lets `dt_ms` pass.
    pub fn update(&mut self, dt_ms: f64) {
        let dt = if dt_ms.is_finite() {
            dt_ms.max(0.0)
        } else {
            0.0
        };
        self.elapsed += dt;
        self.update_loop(dt);
        match self.phase {
            BootyPhase::Map => self.update_map(dt),
            BootyPhase::Counting => self.update_tally(dt),
            BootyPhase::Hold => {
                self.step_ms += dt;
                if self.step_ms >= HOLD_MS {
                    self.phase = BootyPhase::SlideOut;
                    self.step_ms = 0.0;
                }
            }
            BootyPhase::SlideOut => {
                self.step_ms += dt;
                if self.step_ms >= slide_out_ms() {
                    self.phase = BootyPhase::Done;
                    if self.loop_volume > 0 {
                        self.loop_fade = Some((f64::from(self.loop_volume), 0.0, 0.0, 1000.0));
                    }
                }
            }
            BootyPhase::Done => {}
        }
    }

    fn update_loop(&mut self, dt: f64) {
        let Some((from, to, t, len)) = self.loop_fade else {
            return;
        };
        let t = t + dt;
        let v = if t >= len {
            self.loop_fade = None;
            to
        } else {
            self.loop_fade = Some((from, to, t, len));
            from + (to - from) * t / len
        };
        let v = v.round().clamp(0.0, 100.0) as u8;
        if v != self.loop_volume {
            self.loop_volume = v;
            if v == 0 {
                self.audio.push(AudioCue::Stop {
                    voice: MAP_LOOP_VOICE,
                });
            } else {
                self.audio
                    .push(cue(MAP_LOOP, v, true, Some(MAP_LOOP_VOICE)));
            }
        }
    }

    fn wait_ms(&self) -> f64 {
        if !is_map_level(self.stats.level) {
            GEM_MS
        } else if self.stats.level == 1 {
            5000.0
        } else {
            2000.0
        }
    }

    fn update_map(&mut self, dt: f64) {
        self.step_ms += dt;
        loop {
            match self.map_step {
                MapStep::Wait => {
                    let w = self.wait_ms();
                    if self.step_ms < w {
                        return;
                    }
                    self.step_ms -= w;
                    if is_map_level(self.stats.level) {
                        self.map_step = MapStep::Flight;
                        self.audio
                            .push(cue(CURLING, 100, false, Some(CURLING_VOICE)));
                    } else {
                        self.map_step = MapStep::Line;
                        self.say_line();
                    }
                }
                MapStep::Flight => {
                    if self.step_ms < FLIGHT_MS {
                        return;
                    }
                    self.step_ms -= FLIGHT_MS;
                    self.audio.push(AudioCue::Stop {
                        voice: CURLING_VOICE,
                    });
                    self.audio.push(cue(IMPACT, 100, false, None));
                    self.map_step = MapStep::Impact;
                }
                MapStep::Impact => {
                    let w = self.sound_ms(IMPACT);
                    if self.step_ms < w {
                        return;
                    }
                    self.step_ms -= w;
                    self.map_step = MapStep::Line;
                    self.say_line();
                }
                MapStep::Line => {
                    let w = self.line().map_or(0.0, |k| self.sound_ms(k));
                    if self.step_ms < w {
                        return;
                    }
                    self.start_tally();
                    return;
                }
            }
        }
    }

    fn line(&self) -> Option<&'static str> {
        CLAW_LINES
            .get((self.stats.level as usize).wrapping_sub(1))
            .copied()
    }

    fn say_line(&mut self) {
        if let Some(k) = self.line() {
            self.audio.push(cue(k, 100, false, None));
        }
    }

    fn update_tally(&mut self, mut dt: f64) {
        let mut guard = 0;
        while let Some(i) = self.turn {
            guard += 1;
            if guard > 10_000 {
                return;
            }
            let row = self.rows[i];
            match row.state {
                RowState::FlyIn => {
                    let need = ICON_FLIGHT_MS - row.timer;
                    if dt < need {
                        self.rows[i].timer += dt;
                        return;
                    }
                    dt -= need;
                    let kind = TREASURES[row.kind];
                    self.audio.push(cue(kind.sound, 100, false, None));
                    if row.collected == 0 {
                        self.finish_row(i, true);
                        self.start_turn(i + 1);
                        continue;
                    }
                    self.rows[i].state = RowState::Counting;
                    self.rows[i].timer = count_interval(row.remaining, row.collected, kind.factor);
                }
                RowState::Counting => {
                    if dt < row.timer {
                        self.rows[i].timer -= dt;
                        return;
                    }
                    dt -= row.timer;
                    let kind = TREASURES[row.kind];
                    self.audio.push(cue(BOUNCE, 100, false, None));
                    let r = &mut self.rows[i];
                    let inc = kind.step.min(r.collected - r.count);
                    r.count += inc;
                    let added = u64::from(inc) * u64::from(kind.value);
                    r.product += added;
                    r.remaining = r.remaining.saturating_sub(kind.step);
                    self.level_total += added;
                    self.game_total += added;
                    if self.rows[i].remaining == 0 {
                        self.finish_row(i, true);
                        self.start_turn(i + 1);
                    } else {
                        let r = self.rows[i];
                        self.rows[i].timer = count_interval(r.remaining, r.collected, kind.factor);
                    }
                }
                RowState::Waiting | RowState::Finished => {
                    self.start_turn(i + 1);
                }
            }
        }
    }

    /// The map piece's position now.
    pub fn piece_position(&self) -> (i32, i32) {
        let level = self.stats.level;
        if !is_map_level(level) {
            return (0, 0);
        }
        let ((sx, sy), (ex, ey)) = PIECE_PATHS[(level.div_ceil(2) - 1) as usize];
        match self.map_step {
            MapStep::Wait => (sx, sy),
            MapStep::Flight => {
                let t = (self.step_ms / FLIGHT_MS).clamp(0.0, 1.0);
                (
                    sx + (f64::from(ex - sx) * t).floor() as i32,
                    sy + (f64::from(ey - sy) * t).floor() as i32,
                )
            }
            _ => (ex, ey),
        }
    }

    /// A number whose first digit's hotspot is at (x, y), each next digit
    /// a digit's width further right.
    fn digits(&self, out: &mut Vec<ScreenSprite>, value: u64, x: i32, y: i32) {
        let w = self.art.digit_width.max(1);
        for (i, c) in value.to_string().chars().enumerate() {
            let d = c.to_digit(10).unwrap_or(0) as i32;
            out.push(ScreenSprite::new(
                DIGITS,
                d,
                Anchor::TopLeft,
                x + w * i as i32,
                y,
            ));
        }
    }

    /// The picture now.
    pub fn scene(&self) -> ScreenScene {
        let mut sprites = Vec::new();
        if self.phase == BootyPhase::Map {
            let (x, y) = self.piece_position();
            sprites.push(ScreenSprite::new(
                &piece_set(self.stats.level),
                -1,
                Anchor::TopLeft,
                x,
                y,
            ));
            return ScreenScene {
                background: self.art.map.0,
                palette: self.art.map.1.clone(),
                sprites,
                fade: None,
            };
        }
        let slide = if self.phase == BootyPhase::SlideOut || self.phase == BootyPhase::Done {
            Some(self.step_ms)
        } else {
            None
        };
        for row in &self.rows {
            if row.state == RowState::Waiting {
                continue;
            }
            let k = row.kind as i32;
            let y = 55 + 44 * (8 - k);
            let dx = slide.map_or(0, |t| {
                let period = f64::from(18 - 2 * k);
                let moves = ((t / period).floor() as i32).min(25);
                let d = 25 * moves;
                if k % 2 == 0 { -d } else { d }
            });
            let x = row.icon_x() + dx;
            sprites.push(ScreenSprite::new(
                TREASURES[row.kind].set,
                -1,
                Anchor::TopLeft,
                x,
                y,
            ));
            if row.glitter {
                let f = ((self.elapsed / 100.0).floor() as i32 % 16) + 1;
                sprites.push(ScreenSprite::new(GLITTER, f, Anchor::TopLeft, x, y));
            }
            if row.shown() {
                let x = ICON_X + dx;
                self.digits(&mut sprites, u64::from(row.count), x + 40, y);
                sprites.push(ScreenSprite::new(SYMBOLS, 1, Anchor::TopLeft, x + 110, y));
                self.digits(&mut sprites, u64::from(row.total), x + 150, y);
                sprites.push(ScreenSprite::new(SYMBOLS, 2, Anchor::TopLeft, x + 215, y));
                self.digits(
                    &mut sprites,
                    u64::from(TREASURES[row.kind].value),
                    x + 233,
                    y,
                );
                sprites.push(ScreenSprite::new(SYMBOLS, 3, Anchor::TopLeft, x + 299, y));
                self.digits(&mut sprites, row.product, x + 321, y);
            }
        }
        let tdx = slide.map_or(0, |t| {
            let start = 25.0 * 16.0;
            let moves = (((t - start) / 75.0).floor().max(0.0) as i32).min(25);
            25 * moves
        });
        sprites.push(ScreenSprite::new(
            SYMBOLS,
            4,
            Anchor::TopLeft,
            90 + tdx,
            466,
        ));
        self.digits(
            &mut sprites,
            self.game_total,
            90 + tdx + self.art.total_offsets.0,
            466,
        );
        sprites.push(ScreenSprite::new(
            SYMBOLS,
            5,
            Anchor::TopLeft,
            420 - tdx,
            466,
        ));
        self.digits(
            &mut sprites,
            self.level_total,
            420 - tdx + self.art.total_offsets.1,
            466,
        );
        if self.perfect && self.phase == BootyPhase::Hold {
            let f = ((self.elapsed / 200.0).floor() as i32 % 7) + 1;
            sprites.push(ScreenSprite::new(PERFECT, f, Anchor::TopLeft, 320, 240));
        }
        ScreenScene {
            background: self.art.tally.0,
            palette: self.art.tally.1.clone(),
            sprites,
            fade: None,
        }
    }

    /// Every image set the screens draw for `level`.
    pub fn image_sets(level: u32) -> Vec<String> {
        let mut v: Vec<String> = TREASURES.iter().map(|t| t.set.to_string()).collect();
        v.extend([DIGITS, SYMBOLS, PERFECT, GLITTER].map(String::from));
        v.push(piece_set(level));
        v
    }
}

/// How long the slide-out takes, ms.
pub fn slide_out_ms() -> f64 {
    let rows = 25.0 * 18.0;
    let totals = 25.0 * 16.0 + 25.0 * 75.0;
    f64::max(rows, totals)
}
