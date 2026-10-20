//! The credits: the parchment unrolls, the credits text rises over it and
//! the credits music loops, until a key or a click goes back to the main
//! menu.
//!
//! **Opening.** After the last level's tally (the flow fades the tally out
//! over [`FADE_OUT_MS`]) the screen stays black for [`BLACK_MS`], then the
//! parchment unrolls from a narrow strip at the centre to the full screen
//! in [`UNROLL_MS`]; the delayed-unroll sound plays as the black starts.
//! Opened from the main menu (after the menu has rolled up) the parchment
//! unrolls at once, with the unroll sound.
//!
//! **Text.** The text file of the install is read whole and word-wrapped
//! in the bitmap font to a column of [`TEXT_WIDTH`] pixels starting at x
//! [`TEXT_LEFT`], one line every [`LINE_HEIGHT`] pixels. Its top starts at
//! the bottom edge of the screen and rises [`SCROLL_PX_PER_S`] pixels a
//! second (accumulated exactly, drawn at the whole pixel); when its last
//! line has left the top it starts again from the bottom. The text is
//! white. The music starts, looping, on the first frame of the text.
//!
//! **Leaving.** Escape, Space or Enter, any newly pressed input other than
//! the four directions, and a click outside the top-left [`CORNER`] x
//! [`CORNER`] square go back to the main menu. Inputs while the screen
//! opens are ignored.
//!
//! **The corner.** A click in the top-left corner switches to the
//! developer's tune (played once), shows [`MONOLITH_LINE`] in white at the
//! centre for [`MESSAGE_MS`] and gives the text a new random colour every
//! [`COLOUR_MS`]; the tune's end or a second corner click brings the text
//! back to white and the credits music back from its start.
//!
//! The credits run on real time; no level runs while they show.

use claw_assets::pcx::IndexedImage;
use claw_assets::{Frame, Palette};
use claw_frame::{AudioCue, SoundChannel, SoundCue};
use claw_render::{Anchor, ImageId, ScreenScene, ScreenSprite};

use crate::hud::{FONT, FONT_GLYPHS, glyph};
use crate::menu::draw::Art;

/// The credits text in the install.
pub const CREDITS_TEXT: &str = "STATES/CREDITS/CREDITS.TXT";
/// Folder of the credits picture.
pub const CREDITS_SCREENS: &str = "STATES/CREDITS/SCREENS";
/// The parchment picture.
pub const CREDITS_PICTURE: &str = "CREDITS";
/// How fast the text rises, pixels per second.
pub const SCROLL_PX_PER_S: f64 = 25.0;
/// Left edge of the text column.
pub const TEXT_LEFT: i32 = 50;
/// Width of the text column.
pub const TEXT_WIDTH: i32 = 540;
/// Distance between two lines of text.
pub const LINE_HEIGHT: i32 = 16;
/// Width of the screen.
pub const SCREEN_WIDTH: i32 = 640;
/// Height of the screen.
pub const SCREEN_HEIGHT: i32 = 480;
/// The tally's fade to black before the credits, ms.
pub const FADE_OUT_MS: f64 = 500.0;
/// The black between the tally and the unrolling parchment, ms.
pub const BLACK_MS: f64 = 500.0;
/// How long the parchment takes to unroll, ms.
pub const UNROLL_MS: f64 = 700.0;
/// Size of the top-left corner whose clicks toggle the developer's tune.
pub const CORNER: i32 = 100;
/// How long the developer's line shows, ms.
pub const MESSAGE_MS: f64 = 3000.0;
/// How often the text changes colour while the developer's tune plays, ms.
pub const COLOUR_MS: f64 = 300.0;
/// The developer's line.
pub const MONOLITH_LINE: &str = "Now is the time at Monolith when we dance";
/// The sound of the unroll after the black, as the black starts.
pub const DELAYED_UNROLL_SOUND: &str = "CREDITS_DELAYUNROLL";
/// The sound of the unroll from the main menu.
pub const UNROLL_SOUND: &str = "CREDITS_UNROLL";
/// The font set drawn in the text's colour (see [`recolour`]).
pub const TEXT_FONT: &str = "CREDITS_TEXTFONT";
/// The font set drawn in white for the developer's line.
pub const MESSAGE_FONT: &str = "CREDITS_MESSAGEFONT";
/// The parchment cut into vertical strips, for the unroll.
pub const STRIPS: &str = "CREDITS_PARCHMENT";
/// Width of a strip of the parchment.
pub const STRIP_WIDTH: u32 = 16;
/// The text's colour.
pub const WHITE: [u8; 3] = [255, 255, 255];
/// Width of a space when the font has no space frame.
pub const DEFAULT_SPACE: i32 = 6;
/// Tab stops, in characters.
const TAB_STOP: usize = 8;

/// Where the credits were opened from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsEntry {
    /// After the last level's tally: black, then the unroll.
    AfterGame,
    /// From the main menu: the unroll at once.
    FromMenu,
}

/// What shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsPhase {
    /// Black, before the unroll.
    Black,
    /// The parchment unrolling.
    Unroll,
    /// The text rising over the parchment.
    Text,
}

/// The music the credits ask for: the track to start, which stops the one
/// playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsTune {
    /// The credits music, looping.
    Credits,
    /// The developer's tune, once.
    Monolith,
}

/// What a click did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// Ignored (the screen is still opening, or already leaving).
    Ignored,
    /// The corner: the developer's tune started or stopped.
    Corner,
    /// Anywhere else: back to the main menu.
    Leave,
}

/// The parchment prepared for the screen: the picture, its strips and its
/// palette with two entries set aside for the text.
#[derive(Debug, Clone, PartialEq)]
pub struct Parchment {
    /// The whole picture.
    pub picture: Frame,
    /// The picture in vertical strips of [`STRIP_WIDTH`], numbered from 1
    /// left to right.
    pub strips: Vec<(i32, Frame)>,
    /// The picture's palette; entries `text_index` and `message_index` are
    /// white.
    pub palette: Palette,
    /// The palette entry the text is drawn in.
    pub text_index: u8,
    /// The palette entry the developer's line is drawn in.
    pub message_index: u8,
}

/// Prepares the parchment `img` in `palette`: the two entries the picture
/// uses least (index 0 aside) are set aside for the text and the
/// developer's line, their pixels drawn with the nearest other colour.
pub fn prepare_parchment(img: &IndexedImage, palette: &Palette) -> Parchment {
    let mut count = [0usize; 256];
    for &p in &img.pixels {
        count[usize::from(p)] += 1;
    }
    let mut order: Vec<u8> = (1..=255u8).collect();
    order.sort_by_key(|&i| (count[usize::from(i)], i));
    let (text_index, message_index) = (order[0], order[1]);
    let free = |i: u8| i != text_index && i != message_index;
    let nearest = |i: u8| -> u8 {
        let [r, g, b] = palette.0[usize::from(i)].map(i32::from);
        (0..=255u8)
            .filter(|&j| free(j))
            .min_by_key(|&j| {
                let [r2, g2, b2] = palette.0[usize::from(j)].map(i32::from);
                ((r - r2).pow(2) + (g - g2).pow(2) + (b - b2).pow(2), j)
            })
            .unwrap_or(0)
    };
    let mut pixels = img.pixels.clone();
    for set_aside in [text_index, message_index] {
        if count[usize::from(set_aside)] > 0 {
            let to = nearest(set_aside);
            for p in pixels.iter_mut().filter(|p| **p == set_aside) {
                *p = to;
            }
        }
    }
    let mut pal = palette.clone();
    if count[0] == 0 {
        // What shows where nothing is drawn (around the unrolling
        // parchment) is black.
        pal.0[0] = [0, 0, 0];
    }
    pal.0[usize::from(text_index)] = WHITE;
    pal.0[usize::from(message_index)] = WHITE;
    let picture = Frame {
        width: img.width,
        height: img.height,
        offset_x: 0,
        offset_y: 0,
        flags: 0,
        indices: pixels,
        opaque: None,
    };
    let strips = strips_of(&picture);
    Parchment {
        picture,
        strips,
        palette: pal,
        text_index,
        message_index,
    }
}

/// `picture` cut into vertical strips of [`STRIP_WIDTH`] (the last one
/// narrower when the width is not a multiple), numbered from 1.
pub fn strips_of(picture: &Frame) -> Vec<(i32, Frame)> {
    let (w, h) = (picture.width as usize, picture.height as usize);
    let step = STRIP_WIDTH as usize;
    let mut out = Vec::new();
    let mut x0 = 0;
    while x0 < w {
        let sw = step.min(w - x0);
        let mut indices = Vec::with_capacity(sw * h);
        for row in 0..h {
            let start = row * w + x0;
            indices.extend_from_slice(&picture.indices[start..start + sw]);
        }
        out.push((
            out.len() as i32 + 1,
            Frame {
                width: sw as u32,
                height: h as u32,
                offset_x: 0,
                offset_y: 0,
                flags: 0,
                indices,
                opaque: None,
            },
        ));
        x0 += sw;
    }
    out
}

/// `frame` with every visible pixel in palette entry `index`: the pixels
/// its mask marks, or, without a mask, every pixel other than index 0.
pub fn recolour(frame: &Frame, index: u8) -> Frame {
    let mut f = frame.clone();
    for (i, p) in f.indices.iter_mut().enumerate() {
        let visible = match &frame.opaque {
            Some(mask) => mask.get(i).is_some_and(|&m| m != 0),
            None => *p != 0,
        };
        if visible {
            *p = index;
        }
    }
    f
}

/// Sizes of the bitmap font, for laying out the text.
#[derive(Debug, Clone, PartialEq)]
pub struct FontMetrics {
    /// Per frame number: the advance and the hotspot's distance from the
    /// glyph's left edge; `None` for a frame the font lacks.
    glyphs: Vec<Option<(i32, i32)>>,
    /// Advance of a space.
    space: i32,
    /// Distance from a line's top to the glyphs' hotspots.
    rise: i32,
}

impl FontMetrics {
    /// The metrics of the bitmap font in `art`.
    pub fn from_art(art: &mut dyn Art) -> FontMetrics {
        let mut glyphs = Vec::new();
        let mut rise = 0;
        let mut space = None;
        for n in 0..=FONT_GLYPHS {
            let info = art.info(FONT, n).filter(|f| f.width > 0);
            if n == 0 {
                space = info.map(|f| f.width as i32);
                glyphs.push(None);
                continue;
            }
            glyphs.push(info.map(|f| {
                let (half_w, half_h) = ((f.width >> 1) as i32, (f.height >> 1) as i32);
                rise = rise.max(half_h - f.offset_y);
                (f.width as i32, half_w - f.offset_x)
            }));
        }
        FontMetrics {
            glyphs,
            space: space.unwrap_or(DEFAULT_SPACE),
            rise,
        }
    }

    /// Metrics where every glyph is `width` wide with its hotspot at its
    /// centre, and a space is `width` wide (tests).
    pub fn uniform(width: i32) -> FontMetrics {
        FontMetrics {
            glyphs: (0..=FONT_GLYPHS)
                .map(|n| (n > 0).then_some((width, width / 2)))
                .collect(),
            space: width,
            rise: 4,
        }
    }

    fn glyph(&self, c: char) -> Option<(i32, i32, i32)> {
        let n = glyph(c);
        self.glyphs
            .get(n as usize)
            .copied()
            .flatten()
            .map(|(adv, dx)| (n, adv, dx))
    }

    fn advance(&self, c: char) -> i32 {
        self.glyph(c).map_or(self.space, |(_, adv, _)| adv)
    }

    /// Width of `text`. Characters the font lacks are as wide as a space.
    pub fn width(&self, text: &str) -> i32 {
        text.chars().map(|c| self.advance(c)).sum()
    }

    /// The glyphs of `text` as (frame, hotspot x from the line's left).
    fn line(&self, text: &str) -> Vec<(i32, i32)> {
        let mut pen = 0;
        let mut out = Vec::new();
        for c in text.chars() {
            match self.glyph(c) {
                Some((n, adv, dx)) => {
                    out.push((n, pen + dx));
                    pen += adv;
                }
                None => pen += self.space,
            }
        }
        out
    }
}

/// Expands tabs to the next tab stop.
fn expand_tabs(line: &str) -> String {
    let mut out = String::new();
    for c in line.chars() {
        if c == '\t' {
            let n = TAB_STOP - out.chars().count() % TAB_STOP;
            out.extend(std::iter::repeat_n(' ', n));
        } else {
            out.push(c);
        }
    }
    out
}

/// `text` word-wrapped to `width` pixels: lines break at spaces, a word
/// wider than the column breaks where it reaches the edge, and empty lines
/// stay.
pub fn wrap(text: &str, metrics: &FontMetrics, width: i32) -> Vec<String> {
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let raw = expand_tabs(raw.trim_end_matches('\r'));
        if raw.trim().is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line: Option<String> = None;
        for word in raw.split(' ') {
            let candidate = match &line {
                None => word.to_string(),
                Some(l) => format!("{l} {word}"),
            };
            if metrics.width(&candidate) <= width {
                line = Some(candidate);
                continue;
            }
            if let Some(l) = line.take()
                && !l.trim().is_empty()
            {
                out.push(l);
            }
            // The word starts the next line; one too wide breaks.
            let mut piece = String::new();
            for c in word.chars() {
                let mut next = piece.clone();
                next.push(c);
                if metrics.width(&next) > width && !piece.is_empty() {
                    out.push(std::mem::take(&mut piece));
                    next = c.to_string();
                }
                piece = next;
            }
            line = (!piece.is_empty()).then_some(piece);
        }
        if let Some(l) = line {
            out.push(l);
        }
    }
    // A text ending with a line end has no line after it.
    if text.ends_with('\n') && out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out
}

/// A small random stream for the text's colours.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Colours(u32);

impl Colours {
    fn next(&mut self) -> u8 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 24) as u8
    }

    fn colour(&mut self) -> [u8; 3] {
        [self.next(), self.next(), self.next()]
    }
}

/// The developer's tune and its line, while they run.
#[derive(Debug, Clone, PartialEq)]
struct Monolith {
    /// Time left of the tune, ms.
    tune_left: f64,
    /// Time left of the line, ms.
    message_left: f64,
    /// Time left until the next colour, ms.
    colour_left: f64,
}

/// The art of the credits.
#[derive(Debug, Clone, PartialEq)]
pub struct CreditsArt {
    /// The parchment, whole.
    pub picture: Option<ImageId>,
    /// Number of strips of [`STRIPS`].
    pub strips: i32,
    /// The parchment's palette.
    pub palette: Palette,
    /// The palette entry of the text.
    pub text_index: u8,
    /// The palette entry of the developer's line.
    pub message_index: u8,
    /// The font's sizes.
    pub metrics: FontMetrics,
    /// Length of the developer's tune, ms, when it can play.
    pub tune_ms: Option<f64>,
}

impl CreditsArt {
    /// Art with no pictures, white text in entries 1 and 2 of a black
    /// palette, and `metrics` (tests, and an install without the picture).
    pub fn plain(metrics: FontMetrics) -> CreditsArt {
        let mut palette = Palette::default();
        palette.0[1] = WHITE;
        palette.0[2] = WHITE;
        CreditsArt {
            picture: None,
            strips: 0,
            palette,
            text_index: 1,
            message_index: 2,
            metrics,
            tune_ms: None,
        }
    }
}

/// The credits screen.
#[derive(Debug, Clone, PartialEq)]
pub struct CreditsScreen {
    art: CreditsArt,
    entry: CreditsEntry,
    lines: Vec<Vec<(i32, i32)>>,
    message: Vec<(i32, i32)>,
    message_width: i32,
    phase: CreditsPhase,
    /// Time in the current phase, ms.
    elapsed: f64,
    /// How far the text has risen, pixels.
    offset: f64,
    /// Times the text started again from the bottom.
    loops: u32,
    music_started: bool,
    monolith: Option<Monolith>,
    colour: [u8; 3],
    colours: Colours,
    leaving: bool,
    audio: Vec<AudioCue>,
    music: Vec<CreditsTune>,
}

impl CreditsScreen {
    /// The credits showing `text` with `art`, opened from `entry`.
    pub fn new(entry: CreditsEntry, text: &str, art: CreditsArt) -> CreditsScreen {
        let lines = wrap(text, &art.metrics, TEXT_WIDTH)
            .iter()
            .map(|l| art.metrics.line(l))
            .collect();
        let message = art.metrics.line(MONOLITH_LINE);
        let message_width = art.metrics.width(MONOLITH_LINE);
        let (phase, sound) = match entry {
            CreditsEntry::AfterGame => (CreditsPhase::Black, DELAYED_UNROLL_SOUND),
            CreditsEntry::FromMenu => (CreditsPhase::Unroll, UNROLL_SOUND),
        };
        let audio = vec![AudioCue::Play(SoundCue {
            key: sound.to_string(),
            channel: SoundChannel::Interface,
            ..SoundCue::default()
        })];
        CreditsScreen {
            art,
            entry,
            lines,
            message,
            message_width,
            phase,
            elapsed: 0.0,
            offset: 0.0,
            loops: 0,
            music_started: false,
            monolith: None,
            colour: WHITE,
            colours: Colours(0x5EED_C0DE),
            leaving: false,
            audio,
            music: Vec::new(),
        }
    }

    /// Where the credits were opened from.
    pub fn entry(&self) -> CreditsEntry {
        self.entry
    }

    /// What shows.
    pub fn phase(&self) -> CreditsPhase {
        self.phase
    }

    /// Lines of text after wrapping.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Height of the text, pixels.
    pub fn text_height(&self) -> i32 {
        self.lines.len() as i32 * LINE_HEIGHT
    }

    /// Where the top of the text is drawn now.
    pub fn text_top(&self) -> i32 {
        SCREEN_HEIGHT - self.offset as i32
    }

    /// Times the text started again from the bottom.
    pub fn loops(&self) -> u32 {
        self.loops
    }

    /// The text's colour now.
    pub fn colour(&self) -> [u8; 3] {
        self.colour
    }

    /// True while the developer's tune plays.
    pub fn monolith(&self) -> bool {
        self.monolith.is_some()
    }

    /// True while the developer's line shows.
    pub fn message_shown(&self) -> bool {
        self.monolith.as_ref().is_some_and(|m| m.message_left > 0.0)
    }

    /// True once an input asked to leave for the main menu.
    pub fn leaving(&self) -> bool {
        self.leaving
    }

    /// Brightness of the picture: black during the black before the unroll.
    pub fn brightness(&self) -> f64 {
        if self.phase == CreditsPhase::Black {
            0.0
        } else {
            1.0
        }
    }

    /// Sounds to play.
    pub fn take_audio(&mut self) -> Vec<AudioCue> {
        std::mem::take(&mut self.audio)
    }

    /// Music to start, in order.
    pub fn take_music(&mut self) -> Vec<CreditsTune> {
        std::mem::take(&mut self.music)
    }

    /// Lets `dt_ms` pass.
    pub fn update(&mut self, dt_ms: f64) {
        let mut dt = if dt_ms.is_finite() {
            dt_ms.max(0.0)
        } else {
            0.0
        };
        // The opening, then what is left of the frame scrolls the text.
        while dt > 0.0 && self.phase != CreditsPhase::Text {
            let len = match self.phase {
                CreditsPhase::Black => BLACK_MS,
                _ => UNROLL_MS,
            };
            let step = dt.min(len - self.elapsed);
            self.elapsed += step;
            dt -= step;
            if self.elapsed >= len {
                self.elapsed = 0.0;
                self.phase = match self.phase {
                    CreditsPhase::Black => CreditsPhase::Unroll,
                    _ => CreditsPhase::Text,
                };
            }
        }
        if self.phase != CreditsPhase::Text {
            return;
        }
        if !self.music_started {
            self.music_started = true;
            self.music.push(CreditsTune::Credits);
        }
        self.elapsed += dt;
        self.offset += dt * SCROLL_PX_PER_S / 1000.0;
        let bottom = SCREEN_HEIGHT + self.text_height() - self.offset as i32;
        if bottom < 0 {
            self.offset = 0.0;
            self.loops += 1;
        }
        self.update_monolith(dt);
    }

    fn update_monolith(&mut self, dt: f64) {
        let Some(m) = self.monolith.as_mut() else {
            return;
        };
        m.tune_left -= dt;
        m.message_left = (m.message_left - dt).max(0.0);
        m.colour_left -= dt;
        let new_colour = m.colour_left <= 0.0;
        if new_colour {
            m.colour_left = COLOUR_MS;
        }
        let ended = m.tune_left <= 0.0;
        if new_colour && !ended {
            self.colour = self.colours.colour();
        }
        if ended {
            self.leave_monolith();
        }
    }

    fn enter_monolith(&mut self) {
        self.monolith = Some(Monolith {
            // Without the tune, the extra lasts as long as its line.
            tune_left: self.art.tune_ms.unwrap_or(MESSAGE_MS),
            message_left: MESSAGE_MS,
            colour_left: COLOUR_MS,
        });
        self.colour = self.colours.colour();
        self.music.push(CreditsTune::Monolith);
    }

    fn leave_monolith(&mut self) {
        self.monolith = None;
        self.colour = WHITE;
        self.music.push(CreditsTune::Credits);
    }

    /// A newly pressed input that leaves the credits: Escape, Space, Enter
    /// or any input other than the four directions. Returns true when the
    /// screen takes it (once the text shows).
    pub fn press(&mut self) -> bool {
        if self.phase != CreditsPhase::Text || self.leaving {
            return false;
        }
        self.leaving = true;
        true
    }

    /// A click at (x, y) of the 640x480 screen.
    pub fn click(&mut self, x: i32, y: i32) -> Click {
        if self.phase != CreditsPhase::Text || self.leaving {
            return Click::Ignored;
        }
        if (0..CORNER).contains(&x) && (0..CORNER).contains(&y) {
            if self.monolith.is_some() {
                self.leave_monolith();
            } else {
                self.enter_monolith();
            }
            return Click::Corner;
        }
        self.leaving = true;
        Click::Leave
    }

    /// The picture.
    pub fn scene(&self) -> ScreenScene {
        let mut palette = self.art.palette.clone();
        palette.0[usize::from(self.art.text_index)] = self.colour;
        palette.0[usize::from(self.art.message_index)] = WHITE;
        let mut sprites = Vec::new();
        let background = match self.phase {
            CreditsPhase::Black => None,
            CreditsPhase::Unroll => {
                sprites.extend(self.unroll_sprites());
                None
            }
            CreditsPhase::Text => {
                self.text_sprites(&mut sprites);
                self.art.picture
            }
        };
        ScreenScene {
            background,
            palette,
            sprites,
            fade: None,
        }
    }

    /// Half the width of the parchment shown while it unrolls.
    pub fn unrolled_half_width(&self) -> i32 {
        match self.phase {
            CreditsPhase::Black => 0,
            CreditsPhase::Unroll => {
                (f64::from(SCREEN_WIDTH / 2) * (self.elapsed / UNROLL_MS).clamp(0.0, 1.0)) as i32
            }
            CreditsPhase::Text => SCREEN_WIDTH / 2,
        }
    }

    fn unroll_sprites(&self) -> Vec<ScreenSprite> {
        let half = self.unrolled_half_width();
        let (lo, hi) = (SCREEN_WIDTH / 2 - half, SCREEN_WIDTH / 2 + half);
        let w = STRIP_WIDTH as i32;
        (1..=self.art.strips)
            .filter_map(|n| {
                let left = (n - 1) * w;
                (half > 0 && left < hi && left + w > lo).then(|| {
                    ScreenSprite::new(STRIPS, n, Anchor::TopLeft, left + w / 2, SCREEN_HEIGHT / 2)
                })
            })
            .collect()
    }

    fn text_sprites(&self, sprites: &mut Vec<ScreenSprite>) {
        let top = self.text_top();
        let rise = self.art.metrics.rise;
        for (i, line) in self.lines.iter().enumerate() {
            let y = top + i as i32 * LINE_HEIGHT;
            if y + LINE_HEIGHT <= 0 || y >= SCREEN_HEIGHT {
                continue;
            }
            for &(frame, dx) in line {
                sprites.push(ScreenSprite::new(
                    TEXT_FONT,
                    frame,
                    Anchor::TopLeft,
                    TEXT_LEFT + dx,
                    y + rise,
                ));
            }
        }
        if self.message_shown() {
            let left = (SCREEN_WIDTH - self.message_width) / 2;
            let y = (SCREEN_HEIGHT - LINE_HEIGHT) / 2;
            for &(frame, dx) in &self.message {
                sprites.push(ScreenSprite::new(
                    MESSAGE_FONT,
                    frame,
                    Anchor::TopLeft,
                    left + dx,
                    y + rise,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(entry: CreditsEntry, text: &str) -> CreditsScreen {
        CreditsScreen::new(entry, text, CreditsArt::plain(FontMetrics::uniform(10)))
    }

    /// A screen whose text shows, `lines` lines long.
    fn shown(lines: usize) -> CreditsScreen {
        let text: Vec<String> = (0..lines).map(|i| format!("LINE {i}")).collect();
        let mut s = screen(CreditsEntry::FromMenu, &text.join("\r\n"));
        s.update(UNROLL_MS);
        assert_eq!(s.phase(), CreditsPhase::Text);
        s
    }

    #[test]
    fn after_the_game_black_then_the_unroll_then_the_text() {
        let mut s = screen(CreditsEntry::AfterGame, "A");
        let audio = s.take_audio();
        assert!(matches!(&audio[..], [AudioCue::Play(c)] if c.key == DELAYED_UNROLL_SOUND));
        assert_eq!(s.phase(), CreditsPhase::Black);
        assert_eq!(s.brightness(), 0.0);
        s.update(BLACK_MS - 1.0);
        assert_eq!(s.phase(), CreditsPhase::Black);
        s.update(1.0);
        assert_eq!(s.phase(), CreditsPhase::Unroll);
        assert_eq!(s.brightness(), 1.0);
        assert!(s.take_music().is_empty(), "no music before the text");
        s.update(UNROLL_MS / 2.0);
        assert_eq!(s.unrolled_half_width(), 160);
        s.update(UNROLL_MS / 2.0);
        assert_eq!(s.phase(), CreditsPhase::Text);
        s.update(1.0);
        assert_eq!(s.take_music(), vec![CreditsTune::Credits]);
        s.update(1000.0);
        assert!(s.take_music().is_empty(), "the music starts once");
    }

    #[test]
    fn from_the_menu_the_parchment_unrolls_at_once() {
        let mut s = screen(CreditsEntry::FromMenu, "A");
        let audio = s.take_audio();
        assert!(matches!(&audio[..], [AudioCue::Play(c)] if c.key == UNROLL_SOUND));
        assert_eq!(s.phase(), CreditsPhase::Unroll);
        s.update(UNROLL_MS);
        assert_eq!(s.phase(), CreditsPhase::Text);
    }

    #[test]
    fn the_unroll_widens_from_the_centre() {
        let mut art = CreditsArt::plain(FontMetrics::uniform(10));
        art.strips = 40;
        let mut s = CreditsScreen::new(CreditsEntry::FromMenu, "", art);
        assert!(s.scene().sprites.is_empty(), "nothing at the start");
        s.update(UNROLL_MS * 0.1);
        let narrow = s.scene().sprites;
        assert!(!narrow.is_empty());
        assert!(narrow.iter().all(|sp| (sp.x - 320).abs() < 64));
        s.update(UNROLL_MS * 0.8);
        let wide = s.scene().sprites;
        assert!(wide.len() > narrow.len() && wide.len() <= 40);
    }

    #[test]
    fn the_text_rises_25_pixels_a_second_from_the_bottom() {
        let mut s = shown(10);
        assert_eq!(s.text_top(), 480);
        s.update(1000.0);
        assert_eq!(s.text_top(), 455);
        // Accumulated across frames, drawn at the whole pixel.
        for _ in 0..26 {
            s.update(16.0);
        }
        assert_eq!(s.text_top(), 445);
        let sprites = s.scene().sprites;
        assert!(sprites.iter().all(|sp| sp.key == TEXT_FONT));
        let first = sprites.iter().map(|sp| sp.y).min().unwrap();
        assert_eq!(first, 445 + 4);
        assert!(sprites.iter().all(|sp| sp.x >= TEXT_LEFT));
    }

    #[test]
    fn the_text_starts_again_when_its_last_line_has_left() {
        let mut s = shown(4);
        let height = s.text_height();
        assert_eq!(height, 64);
        // The bottom passes above the top edge after (480 + 64) px, plus
        // the pixel that makes it negative.
        let pass_ms = f64::from(480 + height) * 1000.0 / SCROLL_PX_PER_S;
        s.update(pass_ms);
        assert_eq!(s.loops(), 0);
        assert_eq!(s.text_top(), -64);
        assert!(s.scene().sprites.is_empty(), "the text has gone");
        s.update(1000.0 / SCROLL_PX_PER_S);
        assert_eq!(s.loops(), 1);
        assert_eq!(s.text_top(), 480);
    }

    #[test]
    fn inputs_leave_once_the_text_shows() {
        let mut s = screen(CreditsEntry::AfterGame, "A");
        assert!(!s.press(), "ignored in the black");
        s.update(BLACK_MS + 1.0);
        assert!(!s.press(), "ignored while unrolling");
        assert_eq!(s.click(300, 300), Click::Ignored);
        s.update(UNROLL_MS);
        assert!(s.press());
        assert!(s.leaving());
        assert!(!s.press(), "once");
    }

    #[test]
    fn a_click_outside_the_corner_leaves() {
        let mut s = shown(3);
        assert_eq!(s.click(100, 50), Click::Leave);
        assert!(s.leaving());
        let mut s = shown(3);
        assert_eq!(s.click(50, 100), Click::Leave);
    }

    #[test]
    fn the_corner_plays_the_tune_flashes_the_line_and_colours_the_text() {
        let mut art = CreditsArt::plain(FontMetrics::uniform(10));
        art.tune_ms = Some(10_000.0);
        let mut s = CreditsScreen::new(CreditsEntry::FromMenu, "A\nB", art);
        s.update(UNROLL_MS + 1.0);
        s.take_music();
        assert_eq!(s.click(99, 99), Click::Corner);
        assert!(!s.leaving());
        assert!(s.monolith() && s.message_shown());
        assert_eq!(s.take_music(), vec![CreditsTune::Monolith]);
        let first = s.colour();
        let msg = s
            .scene()
            .sprites
            .iter()
            .filter(|sp| sp.key == MESSAGE_FONT)
            .count();
        assert_eq!(msg, MONOLITH_LINE.chars().filter(|c| *c != ' ').count());
        // A new colour every 300 ms.
        s.update(COLOUR_MS - 1.0);
        assert_eq!(s.colour(), first);
        s.update(1.0);
        assert_ne!(s.colour(), first);
        let pal = s.scene().palette;
        assert_eq!(pal.0[1], s.colour(), "the text's entry");
        assert_eq!(pal.0[2], WHITE, "the line stays white");
        // The line shows for 3 s.
        s.update(MESSAGE_MS);
        assert!(s.monolith() && !s.message_shown());
        // The tune's end brings the credits back.
        s.update(10_000.0);
        assert!(!s.monolith());
        assert_eq!(s.colour(), WHITE);
        assert_eq!(s.take_music(), vec![CreditsTune::Credits]);
    }

    #[test]
    fn a_second_corner_click_ends_the_extra() {
        let mut s = shown(2);
        s.take_music();
        assert_eq!(s.click(0, 0), Click::Corner);
        assert_eq!(s.click(10, 10), Click::Corner);
        assert!(!s.monolith());
        assert_eq!(s.colour(), WHITE);
        assert_eq!(
            s.take_music(),
            vec![CreditsTune::Monolith, CreditsTune::Credits]
        );
    }

    #[test]
    fn without_the_tune_the_extra_lasts_as_long_as_its_line() {
        let mut s = shown(2);
        s.click(5, 5);
        s.update(MESSAGE_MS - 1.0);
        assert!(s.monolith());
        s.update(1.0);
        assert!(!s.monolith());
    }

    #[test]
    fn wrapping_breaks_at_spaces_and_keeps_empty_lines() {
        let m = FontMetrics::uniform(10);
        // 54 characters fit in 540 px.
        let long = format!("{} {}", "A".repeat(30), "B".repeat(30));
        assert_eq!(
            wrap(&format!("ONE\r\n\r\n{long}\r\n"), &m, 540),
            vec![
                "ONE".to_string(),
                String::new(),
                "A".repeat(30),
                "B".repeat(30)
            ]
        );
        // A word wider than the column breaks at the edge.
        assert_eq!(
            wrap(&"C".repeat(60), &m, 540),
            vec!["C".repeat(54), "C".repeat(6)]
        );
        // Tabs reach the next stop.
        assert_eq!(wrap("A\tB", &m, 540), vec!["A       B".to_string()]);
        assert_eq!(wrap("WORD", &m, 540), vec!["WORD".to_string()]);
    }

    #[test]
    fn the_parchment_sets_aside_its_least_used_entries() {
        let mut palette = Palette::default();
        for i in 0..256 {
            palette.0[i] = [i as u8, i as u8, i as u8];
        }
        // Every entry used once, entry 7 twice more and 0 never: 1 and 2
        // are the least used.
        let mut pixels: Vec<u8> = (0..=255u8).collect();
        pixels.extend([7, 7, 0, 0]);
        let img = IndexedImage {
            width: 26,
            height: 10,
            pixels,
        };
        let p = prepare_parchment(&img, &palette);
        assert_eq!((p.text_index, p.message_index), (1, 2));
        assert_eq!(p.palette.0[1], WHITE);
        assert_eq!(p.palette.0[2], WHITE);
        assert!(!p.picture.indices.contains(&1) && !p.picture.indices.contains(&2));
        // Their pixels take the nearest colour left.
        assert_eq!(p.picture.indices[1], 0);
        assert_eq!(p.picture.indices[2], 3);
        assert_eq!(p.strips.len(), 2);
        assert_eq!((p.strips[0].1.width, p.strips[1].1.width), (16, 10));
        assert_eq!(p.strips[1].1.indices[0], p.picture.indices[16]);
        assert_eq!(p.strips[1].1.indices[10], p.picture.indices[26 + 16]);
    }

    #[test]
    fn recolouring_paints_the_visible_pixels() {
        let f = Frame {
            width: 3,
            height: 1,
            offset_x: 0,
            offset_y: 0,
            flags: 0,
            indices: vec![0, 5, 9],
            opaque: None,
        };
        assert_eq!(recolour(&f, 200).indices, vec![0, 200, 200]);
        let masked = Frame {
            opaque: Some(vec![1, 0, 1]),
            ..f
        };
        assert_eq!(recolour(&masked, 200).indices, vec![200, 5, 200]);
    }
}
