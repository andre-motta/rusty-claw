//! What a menu shows: its entries, the submenu's heading, the coin markers
//! beside the highlighted entry and the slider knobs, as screen sprites on
//! the 640x480 menu canvas (anchored to its top-left corner).
//!
//! - Each entry's image is centred in its cell. Switch entries show their
//!   state by frame: in the audio sets the frame is one higher while the
//!   setting is off (sound, speech, ambient, music), except frames 31 and
//!   36; in the display sets one higher while the front layer or
//!   interlaced movies are off, or while high detail is on.
//! - Main menu headings sit at (320, 180), the quit question at (320, 220)
//!   and the saving note at (320, 230).
//! - The coins flank the highlighted entry 30 pixels outside the
//!   submenu's widest image, 17 pixels below the entry's top (on the level
//!   lists 18 pixels outside the entry itself, 14 below its top), cycling
//!   a frame every 100 ms.
//! - A slider's knob sits at `T[step] + image left` (the music knob 8
//!   pixels further left), 14 pixels below the image top, with `T_sound =
//!   [328, 346, 366, 386, 406, 426, 446, 466, 486, 506, 524]` (sound volume
//!   / 8, music volume / 10) and `T_area = [348, 366, 386, 406, 426, 446,
//!   466, 486, 506, 526]` (indexed by `0.9 * size` rounded down).
//! - The in-game confirmations show their question at (320, 202): the
//!   lose-a-life question (frame 6 instead of 5 with fewer than 2 lives)
//!   or the end-the-game question.
//! - Text entries use the game's bitmap font, magnified by their zoom (see
//!   [`super::layout`]) and centred in their cell; the coins beside one sit
//!   at the middle of its cell. A magnified sprite is a [`MenuSprite`] with
//!   a zoom above 1: it keeps the top-left corner of its box and grows right
//!   and down, a whole number of pixels per font pixel.
//! - An entry shows images ([`Face::Image`]) or its text ([`Face::Text`]);
//!   this game's own entries show their text unless they name images the
//!   art has.

use std::collections::BTreeMap;

use claw_render::scene::Item;
use claw_render::{Anchor, FrameInfo, LevelGraphics, Scene, ScreenSprite};

use super::command::{MenuCommand, MenuKind, Page, Slider, Toggle};
use super::layout::is_level_list;
use super::nav::Navigator;
use super::tree::{Cell, ItemId, ItemImage, MenuItem, image_set_of};
use crate::hud::{FONT, FONT_GLYPHS, glyph};

/// Where menu art comes from: sizes and frame lists of image sets.
pub trait Art {
    /// Size and hotspot offsets of `frame` of `set` (-1: its first frame).
    fn info(&mut self, set: &str, frame: i32) -> Option<FrameInfo>;
    /// The frame numbers of `set`, ascending.
    fn frames(&mut self, set: &str) -> Vec<i32>;
}

impl Art for LevelGraphics {
    fn info(&mut self, set: &str, frame: i32) -> Option<FrameInfo> {
        self.frame_info(set, frame)
    }

    fn frames(&mut self, set: &str) -> Vec<i32> {
        self.frame_numbers(set)
    }
}

/// The settings switch entries show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Switches {
    /// Sound on.
    pub sound: bool,
    /// Speech on.
    pub voice: bool,
    /// Ambient sounds on.
    pub ambient: bool,
    /// Music on.
    pub music: bool,
    /// High detail on.
    pub high_detail: bool,
    /// Front layer on.
    pub front_layer: bool,
    /// Interlaced movies on.
    pub movie_interlace: bool,
}

impl Default for Switches {
    fn default() -> Self {
        Switches {
            sound: true,
            voice: true,
            ambient: true,
            music: true,
            high_detail: true,
            front_layer: true,
            movie_interlace: false,
        }
    }
}

impl Switches {
    /// The state of `t`.
    pub fn get(&self, t: Toggle) -> bool {
        match t {
            Toggle::Sound => self.sound,
            Toggle::Voice => self.voice,
            Toggle::Ambient => self.ambient,
            Toggle::Music => self.music,
            Toggle::HighDetail => self.high_detail,
            Toggle::FrontLayer => self.front_layer,
            Toggle::MovieInterlace => self.movie_interlace,
        }
    }

    /// Sets `t`.
    pub fn set(&mut self, t: Toggle, on: bool) {
        match t {
            Toggle::Sound => self.sound = on,
            Toggle::Voice => self.voice = on,
            Toggle::Ambient => self.ambient = on,
            Toggle::Music => self.music = on,
            Toggle::HighDetail => self.high_detail = on,
            Toggle::FrontLayer => self.front_layer = on,
            Toggle::MovieInterlace => self.movie_interlace = on,
        }
    }

    /// True when `t`'s entry shows the frame one higher.
    pub fn shifted(&self, t: Toggle) -> bool {
        match t {
            Toggle::HighDetail => self.high_detail,
            _ => !self.get(t),
        }
    }
}

/// What a menu shows besides its entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuState {
    /// The switches.
    pub switches: Switches,
    /// Sound volume, 0 to 100.
    pub sound_volume: i32,
    /// Music volume, 0 to 100.
    pub music_volume: i32,
    /// Play area size, 1 to 10.
    pub play_area: i32,
    /// Claw's lives, for the lose-a-life question.
    pub lives: i32,
}

impl Default for MenuState {
    fn default() -> Self {
        MenuState {
            switches: Switches::default(),
            sound_volume: 80,
            music_volume: 50,
            play_area: 10,
            lives: 6,
        }
    }
}

impl MenuState {
    /// The value of `s`.
    pub fn value(&self, s: Slider) -> i32 {
        match s {
            Slider::SoundVolume => self.sound_volume,
            Slider::MusicVolume => self.music_volume,
            Slider::PlayArea => self.play_area,
        }
    }

    /// Sets `s`.
    pub fn set_value(&mut self, s: Slider, v: i32) {
        match s {
            Slider::SoundVolume => self.sound_volume = v,
            Slider::MusicVolume => self.music_volume = v,
            Slider::PlayArea => self.play_area = v,
        }
    }
}

/// Knob x of the sound and music sliders by step.
pub const SOUND_KNOB_X: [i32; 11] = [328, 346, 366, 386, 406, 426, 446, 466, 486, 506, 524];
/// Knob x of the play area slider by step.
pub const AREA_KNOB_X: [i32; 10] = [348, 366, 386, 406, 426, 446, 466, 486, 506, 526];

/// The image an entry shows in `slot`, with switch entries shifted to
/// their state.
pub fn shown_image(
    kind: MenuKind,
    item: &MenuItem,
    slot: usize,
    switches: &Switches,
) -> Option<ItemImage> {
    let mut img = image_set_of(kind, item.slots.get(slot)?)?;
    if let Some(t) = item.toggle()
        && switches.shifted(t)
    {
        let audio = matches!(
            t,
            Toggle::Sound | Toggle::Voice | Toggle::Ambient | Toggle::Music
        );
        if !(audio && (img.frame == 31 || img.frame == 36)) {
            img.frame += 1;
        }
    }
    Some(img)
}

/// What an entry shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Face {
    /// An image of its own.
    Image(ItemImage),
    /// Its text in the menu font, `zoom` times larger.
    Text {
        /// The text.
        text: String,
        /// Whole magnification.
        zoom: u32,
    },
}

/// What `item` shows in `slot`: its text when it shows text (see
/// [`MenuItem::shows_text`]), its image for the slot otherwise.
pub fn face(kind: MenuKind, item: &MenuItem, slot: usize, switches: &Switches) -> Option<Face> {
    if item.shows_text {
        return item.text.as_ref().map(|t| Face::Text {
            text: t.clone(),
            zoom: item.zoom.max(1),
        });
    }
    shown_image(kind, item, slot, switches).map(Face::Image)
}

/// A screen sprite drawn `zoom` times larger in each direction, growing
/// right and down from the top-left corner of its box.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuSprite {
    /// The sprite, placed for its unmagnified box.
    pub sprite: ScreenSprite,
    /// Whole magnification, 1 for none.
    pub zoom: u32,
}

impl MenuSprite {
    /// `sprite` unmagnified.
    pub fn plain(sprite: ScreenSprite) -> MenuSprite {
        MenuSprite { sprite, zoom: 1 }
    }
}

/// Width and height of `text` in the bitmap font.
pub fn text_size(art: &mut dyn Art, text: &str) -> (i32, i32) {
    let mut w = 0;
    let mut h = 0;
    for c in text.chars() {
        if let Some(f) = art.info(FONT, glyph(c)) {
            w += f.width as i32;
            h = h.max(f.height as i32);
        }
    }
    (w, h)
}

/// `text` in the bitmap font with its box's top-left corner at (left, top)
/// from `anchor`.
pub fn text_sprites(
    art: &mut dyn Art,
    text: &str,
    anchor: Anchor,
    left: i32,
    top: i32,
) -> Vec<ScreenSprite> {
    text_picture(art, text, anchor, left, top, 1)
        .into_iter()
        .map(|m| m.sprite)
        .collect()
}

/// `text` in the bitmap font `zoom` times larger, with its box's top-left
/// corner at (left, top) from `anchor`; the box is [`text_size`] times
/// `zoom`.
pub fn text_picture(
    art: &mut dyn Art,
    text: &str,
    anchor: Anchor,
    left: i32,
    top: i32,
    zoom: u32,
) -> Vec<MenuSprite> {
    let z = zoom.max(1) as i32;
    let glyphs: Vec<(i32, FrameInfo)> = text
        .chars()
        .map(|c| {
            let n = glyph(c);
            (n, art.info(FONT, n).unwrap_or_default())
        })
        .collect();
    let rise = glyphs
        .iter()
        .filter(|(_, f)| f.width > 0)
        .map(|(_, f)| (f.height >> 1) as i32 - f.offset_y)
        .max()
        .unwrap_or(0);
    let mut pen = 0;
    let mut out = Vec::new();
    for (n, f) in glyphs {
        if f.width > 0 && n != 0 {
            // The glyph's box, unmagnified, from the text's top-left corner.
            let (half_w, half_h) = ((f.width >> 1) as i32, (f.height >> 1) as i32);
            let box_top = rise - half_h + f.offset_y;
            let x = left + pen * z + half_w - f.offset_x;
            let y = top + box_top * z + half_h - f.offset_y;
            out.push(MenuSprite {
                sprite: ScreenSprite::new(FONT, n, anchor, x, y),
                zoom: z as u32,
            });
        }
        pen += f.width as i32;
    }
    out
}

/// Measures every entry of `nav`'s menu (the largest of its images, or
/// its unmagnified text when it has text and none of its images is found)
/// and lays the submenu out again.
pub fn measure(nav: &mut Navigator, art: &mut dyn Art) {
    let kind = nav.tree.kind;
    for i in 0..nav.tree.items.len() {
        let item = &nav.tree.items[i];
        let mut size = (0, 0);
        let mut found = false;
        for slot in &item.slots {
            if let Some(img) = image_set_of(kind, slot)
                && let Some(f) = art.info(&img.set, img.frame)
            {
                size.0 = size.0.max(f.width as i32);
                size.1 = size.1.max(f.height as i32);
                found = true;
            }
        }
        let shows_text = item.text.is_some() && !found;
        if shows_text && let Some(t) = item.text.clone() {
            size = text_size(art, &t);
        }
        let item = &mut nav.tree.items[i];
        item.size = size;
        item.shows_text = shows_text;
    }
    nav.relayout();
}

/// Measures the entries that show text again (after their text changed)
/// and lays the submenu out again.
pub fn remeasure_text(nav: &mut Navigator, art: &mut dyn Art) {
    for i in 0..nav.tree.items.len() {
        if nav.tree.items[i].shows_text
            && let Some(t) = nav.tree.items[i].text.clone()
        {
            nav.tree.items[i].size = text_size(art, &t);
        }
    }
    nav.relayout();
}

/// The bitmap font's glyph sizes, kept apart from the rest of the art so
/// text can be measured without it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FontArt {
    glyphs: BTreeMap<i32, FrameInfo>,
}

impl FontArt {
    /// The glyph sizes of the font in `art`.
    pub fn from_art(art: &mut dyn Art) -> FontArt {
        let mut glyphs = BTreeMap::new();
        for n in 0..=FONT_GLYPHS {
            if let Some(f) = art.info(FONT, n) {
                glyphs.insert(n, f);
            }
        }
        FontArt { glyphs }
    }

    /// Glyphs all `width` x `height` (tests).
    pub fn uniform(width: u32, height: u32) -> FontArt {
        let glyphs = (0..=FONT_GLYPHS)
            .map(|n| {
                (
                    n,
                    FrameInfo {
                        width,
                        height,
                        offset_x: 0,
                        offset_y: 0,
                    },
                )
            })
            .collect();
        FontArt { glyphs }
    }
}

impl Art for FontArt {
    fn info(&mut self, set: &str, frame: i32) -> Option<FrameInfo> {
        if set == FONT {
            self.glyphs.get(&frame).copied()
        } else {
            None
        }
    }

    fn frames(&mut self, set: &str) -> Vec<i32> {
        if set == FONT {
            self.glyphs.keys().copied().collect()
        } else {
            Vec::new()
        }
    }
}

/// The heading of a main-menu submenu: image set, frame and position.
pub fn heading(page: Page) -> Option<(&'static str, i32, (i32, i32))> {
    let top = (320, 180);
    Some(match page {
        Page::Root => ("MENU_MAIN", 14, top),
        Page::SinglePlayer => ("MENU_MAIN_SINGLEPLAYER", 14, top),
        Page::Multiplayer => ("MENU_MAIN_MULTIPLAYER", 10, top),
        Page::Movies => ("MENU_MAIN_REPLAYMOVIES", 33, top),
        Page::Options => ("MENU_MAIN_OPTIONS", 14, top),
        Page::QuitConfirm => ("MENU_MAIN_QUIT", 5, (320, 220)),
        Page::NewGameLevels => ("MENU_MAIN_SINGLEPLAYER_NEW", 1, top),
        Page::LoadGameLevels => ("MENU_MAIN_SINGLEPLAYER_LOAD", 1, top),
        Page::SaveInfo => ("MENU_MAIN_SINGLEPLAYER_SAVE", 3, (320, 230)),
        Page::LoadEntries { level } => (
            "MENU_MAIN_SINGLEPLAYER_LOAD_CHECKPOINTS",
            level as i32 + 10,
            top,
        ),
        Page::Audio => ("MENU_MAIN_OPTIONS_AUDIO", 19, top),
        Page::Display => ("MENU_MAIN_OPTIONS_DISPLAY", 21, top),
        _ => return None,
    })
}

/// The coin marker set of `kind`'s menu.
pub fn coin_set(kind: MenuKind) -> &'static str {
    match kind {
        MenuKind::Main => "MENU_COINS",
        _ => "GAME_TREASURE_COINS",
    }
}

/// The knob set and frame of slider `s` in `kind`'s menu.
pub fn knob(kind: MenuKind, s: Slider) -> (&'static str, i32) {
    match kind {
        MenuKind::Main => (
            "MENU_MAIN_OPTIONS_AUDIO",
            match s {
                Slider::SoundVolume => 42,
                Slider::MusicVolume => 41,
                Slider::PlayArea => 45,
            },
        ),
        _ => (
            "GAME_INGAMEMENU_MAIN_OPTIONS_AUDIO",
            match s {
                Slider::SoundVolume => 42,
                Slider::MusicVolume => 43,
                Slider::PlayArea => 45,
            },
        ),
    }
}

/// The knob's x from the slider's image left.
pub fn knob_x(s: Slider, value: i32, image_left: i32) -> i32 {
    match s {
        Slider::SoundVolume => SOUND_KNOB_X[(value.max(0) / 8).min(10) as usize] + image_left,
        Slider::MusicVolume => SOUND_KNOB_X[(value.max(0) / 10).min(10) as usize] - 8 + image_left,
        Slider::PlayArea => {
            let i = (f64::from(value.clamp(0, 10)) * 0.9).floor() as usize;
            AREA_KNOB_X[i.min(9)] + image_left
        }
    }
}

/// Every image set a menu of `kind` draws besides its entries.
pub fn extra_sets(kind: MenuKind) -> Vec<&'static str> {
    let mut v = vec![coin_set(kind), FONT];
    match kind {
        MenuKind::Main => v.extend([
            "MENU_MAIN",
            "MENU_MAIN_SINGLEPLAYER",
            "MENU_MAIN_MULTIPLAYER",
            "MENU_MAIN_REPLAYMOVIES",
            "MENU_MAIN_OPTIONS",
            "MENU_MAIN_QUIT",
            "MENU_MAIN_SINGLEPLAYER_NEW",
            "MENU_MAIN_SINGLEPLAYER_LOAD",
            "MENU_MAIN_SINGLEPLAYER_SAVE",
            "MENU_MAIN_SINGLEPLAYER_LOAD_CHECKPOINTS",
            "MENU_MAIN_OPTIONS_AUDIO",
            "MENU_MAIN_OPTIONS_DISPLAY",
        ]),
        MenuKind::InGame => v.extend([
            "GAME_INGAMEMENU_MAIN_OPTIONS_AUDIO",
            "GAME_INGAMEMENU_MAIN_CLAWICIDE",
            "GAME_INGAMEMENU_MAIN_ENDGAME",
        ]),
        MenuKind::GameOver => {}
    }
    v
}

struct Placed {
    left: i32,
    top: i32,
    right: i32,
}

fn place(
    art: &mut dyn Art,
    img: &ItemImage,
    cell: super::tree::Cell,
) -> Option<(ScreenSprite, Placed)> {
    let f = art.info(&img.set, img.frame)?;
    let (w, h) = (f.width as i32, f.height as i32);
    let left = cell.x + (cell.w - w) / 2;
    let top = cell.y + (cell.h - h) / 2;
    let s = ScreenSprite::new(
        &img.set,
        img.frame,
        Anchor::TopLeft,
        left + (w >> 1) - f.offset_x,
        top + (h >> 1) - f.offset_y,
    );
    Some((
        s,
        Placed {
            left,
            top,
            right: left + w,
        },
    ))
}

/// One entry as drawn: its sprites and the box they cover.
struct Drawn {
    sprites: Vec<MenuSprite>,
    bounds: Cell,
}

/// `face` drawn centred in `cell`.
fn draw_face(art: &mut dyn Art, face: &Face, cell: Cell) -> Option<Drawn> {
    match face {
        Face::Image(img) => {
            let (s, p) = place(art, img, cell)?;
            let h = art.info(&img.set, img.frame).map_or(0, |f| f.height as i32);
            Some(Drawn {
                sprites: vec![MenuSprite::plain(s)],
                bounds: Cell {
                    x: p.left,
                    y: p.top,
                    w: p.right - p.left,
                    h,
                },
            })
        }
        Face::Text { text, zoom } => {
            let z = (*zoom).max(1) as i32;
            let (w, h) = text_size(art, text);
            let (w, h) = (w * z, h * z);
            let left = cell.x + (cell.w - w) / 2;
            let top = cell.y + (cell.h - h) / 2;
            Some(Drawn {
                sprites: text_picture(art, text, Anchor::TopLeft, left, top, *zoom),
                bounds: Cell {
                    x: left,
                    y: top,
                    w,
                    h,
                },
            })
        }
    }
}

/// The box each entry of `nav`'s submenu covers as drawn now, in entry
/// order (entries that draw nothing are left out).
pub fn entry_boxes(nav: &Navigator, art: &mut dyn Art, state: &MenuState) -> Vec<(ItemId, Cell)> {
    let kind = nav.kind();
    let mut out = Vec::new();
    for &k in nav.entries() {
        let item = &nav.tree.items[k];
        let Some(face) = face(kind, item, nav.look(k).slot(), &state.switches) else {
            continue;
        };
        if let Some(d) = draw_face(art, &face, item.cell) {
            out.push((k, d.bounds));
        }
    }
    out
}

/// Everything `nav`'s menu shows at `time_ms` that is drawn unmagnified,
/// on the 640x480 canvas: [`picture`] without the magnified text.
pub fn sprites(
    nav: &Navigator,
    art: &mut dyn Art,
    state: &MenuState,
    time_ms: f64,
) -> Vec<ScreenSprite> {
    picture(nav, art, state, time_ms)
        .into_iter()
        .filter(|m| m.zoom <= 1)
        .map(|m| m.sprite)
        .collect()
}

/// Everything `nav`'s menu shows at `time_ms`, on the 640x480 canvas, in
/// drawing order.
pub fn picture(
    nav: &Navigator,
    art: &mut dyn Art,
    state: &MenuState,
    time_ms: f64,
) -> Vec<MenuSprite> {
    let kind = nav.kind();
    let page = nav.page();
    let mut out = Vec::new();
    if kind == MenuKind::Main
        && let Some((set, frame, (x, y))) = heading(page)
    {
        out.push(MenuSprite::plain(ScreenSprite::new(
            set,
            frame,
            Anchor::TopLeft,
            x,
            y,
        )));
    }
    if kind == MenuKind::InGame {
        let prompt = match page {
            Page::EndLifeConfirm => Some((
                "GAME_INGAMEMENU_MAIN_CLAWICIDE",
                if state.lives < 2 { 6 } else { 5 },
            )),
            Page::EndGameConfirm => Some(("GAME_INGAMEMENU_MAIN_ENDGAME", 5)),
            _ => None,
        };
        if let Some((set, frame)) = prompt {
            out.push(MenuSprite::plain(ScreenSprite::new(
                set,
                frame,
                Anchor::TopLeft,
                320,
                202,
            )));
        }
    }
    let mut extent: Option<(i32, i32)> = None;
    let mut placed: Vec<(ItemId, Placed)> = Vec::new();
    for &k in nav.entries() {
        let item = &nav.tree.items[k];
        let Some(face) = face(kind, item, nav.look(k).slot(), &state.switches) else {
            continue;
        };
        let Some(d) = draw_face(art, &face, item.cell) else {
            continue;
        };
        let b = d.bounds;
        extent = Some(extent.map_or((b.x, b.x + b.w), |(a, r)| (a.min(b.x), r.max(b.x + b.w))));
        out.extend(d.sprites);
        placed.push((
            k,
            Placed {
                left: b.x,
                top: b.y,
                right: b.x + b.w,
            },
        ));
    }
    // Slider knobs.
    for (k, p) in &placed {
        let item = &nav.tree.items[*k];
        if let Some(s) = item.slider
            && item.enabled
        {
            let (set, frame) = knob(kind, s);
            out.push(MenuSprite::plain(ScreenSprite::new(
                set,
                frame,
                Anchor::TopLeft,
                knob_x(s, state.value(s), p.left),
                p.top + 14,
            )));
        }
    }
    // Coin markers.
    if let (Some(sel), Some((ext_l, ext_r))) = (nav.selected(), extent)
        && let Some((_, p)) = placed.iter().find(|(k, _)| *k == sel)
    {
        let set = coin_set(kind);
        let frames = art.frames(set);
        let frame = if frames.is_empty() {
            -1
        } else {
            let i = (time_ms.max(0.0) / 100.0).floor() as usize % frames.len();
            frames[i]
        };
        let item = &nav.tree.items[sel];
        let cell = item.cell;
        let (l, r, dy) = if kind == MenuKind::Main && is_list(page) {
            (p.left - 18, p.right + 18, 14)
        } else if item.shows_text {
            (ext_l - 30, ext_r + 30, cell.h / 2)
        } else {
            (ext_l - 30, ext_r + 30, 17)
        };
        for x in [l, r] {
            out.push(MenuSprite::plain(ScreenSprite::new(
                set,
                frame,
                Anchor::TopLeft,
                x,
                cell.y + dy,
            )));
        }
    }
    out
}

/// Adds `sprites` to the end of `scene` (built from `graphics`), each
/// magnified by its zoom on top of the scene's own scale; sprites that fall
/// outside the target are left out.
pub fn add_to_scene(scene: &mut Scene, graphics: &mut LevelGraphics, sprites: &[MenuSprite]) {
    let k = scene.comp.scale.max(1);
    let (tw, th) = (
        i64::from(scene.comp.target.0),
        i64::from(scene.comp.target.1),
    );
    for m in sprites {
        let Some(mut p) =
            claw_render::screen::sprite_params(graphics, &m.sprite, scene.comp.presentation, k)
        else {
            continue;
        };
        p.scale = k * m.zoom.max(1);
        let (x0, y0, x1, y1) = p.quad();
        if x1 <= 0 || y1 <= 0 || x0 >= tw || y0 >= th {
            continue;
        }
        scene.items.push(Item::Sprite(scene.sprites.len() as u32));
        scene.sprites.push(p);
    }
}

fn is_list(page: Page) -> bool {
    is_level_list(page) || matches!(page, Page::LoadEntries { .. })
}

/// Moves canvas sprites (anchored to the top-left of the 640x480 menu
/// canvas) onto the centre of a picture of any width, for menus drawn over
/// a level.
pub fn centred(sprites: Vec<ScreenSprite>) -> Vec<ScreenSprite> {
    sprites.into_iter().map(centre).collect()
}

/// [`centred`] for a [`picture`].
pub fn centred_picture(sprites: Vec<MenuSprite>) -> Vec<MenuSprite> {
    sprites
        .into_iter()
        .map(|m| MenuSprite {
            sprite: centre(m.sprite),
            zoom: m.zoom,
        })
        .collect()
}

fn centre(mut s: ScreenSprite) -> ScreenSprite {
    s.anchor = Anchor::Center;
    s.x -= CANVAS_CENTER.0;
    s.y -= CANVAS_CENTER.1;
    s
}

/// The centre pixel of the 640x480 canvas, as [`Anchor::Center`] places it.
pub const CANVAS_CENTER: (i32, i32) = (319, 239);

/// True when choosing `command` should wait for the select sound (the
/// game plays the sound to its end before starting or loading a game).
pub fn waits_for_select(command: MenuCommand) -> bool {
    matches!(
        command,
        MenuCommand::NewGame { .. } | MenuCommand::Load { .. } | MenuCommand::Quit
    )
}

#[cfg(test)]
mod tests {
    use claw_assets::{Frame, Palette};
    use claw_frame::ViewConfig;
    use claw_render::scene::DEFAULT_MAX_TARGET;
    use claw_render::{MemoryImages, ScreenBuilder, ScreenScene};

    use super::*;

    #[test]
    fn magnified_text_keeps_its_top_left_corner_and_grows() {
        let mut font = FontArt::uniform(8, 7);
        let one = text_picture(&mut font, "AB", Anchor::TopLeft, 100, 50, 1);
        let three = text_picture(&mut font, "AB", Anchor::TopLeft, 100, 50, 3);
        assert_eq!(one.len(), 2);
        assert!(three.iter().all(|m| m.zoom == 3));
        // Unmagnified, the glyph boxes start at (100, 50) and (108, 50);
        // three times larger, the second starts one magnified width on.
        let corner = |m: &MenuSprite| (m.sprite.x - 4, m.sprite.y - 3);
        assert_eq!(corner(&one[0]), (100, 50));
        assert_eq!(corner(&one[1]), (108, 50));
        assert_eq!(corner(&three[0]), (100, 50));
        assert_eq!(corner(&three[1]), (124, 50));
        assert_eq!(
            text_sprites(&mut font, "AB", Anchor::TopLeft, 100, 50),
            one.into_iter().map(|m| m.sprite).collect::<Vec<_>>()
        );
    }

    #[test]
    fn magnified_sprites_draw_larger_on_the_scene() {
        let mut images = MemoryImages::default();
        images.insert(
            FONT,
            glyph('A'),
            Frame {
                width: 8,
                height: 7,
                offset_x: 0,
                offset_y: 0,
                flags: 0,
                indices: vec![1; 56],
                opaque: None,
            },
        );
        let mut g = LevelGraphics::for_screens(Palette::default(), Box::new(images));
        let view = ViewConfig::default().with_render_size(1280, 960);
        let b = ScreenBuilder::new(&view, DEFAULT_MAX_TARGET);
        let blank = ScreenScene {
            background: None,
            palette: Palette::default(),
            sprites: Vec::new(),
            fade: None,
        };
        let mut scene = b.build(&mut g, &blank, 0.0);
        let k = scene.comp.scale;
        let mut font = FontArt::uniform(8, 7);
        let mut sprites = text_picture(&mut font, "A", Anchor::TopLeft, 100, 50, 3);
        // One sprite far outside the picture is left out.
        sprites.extend(text_picture(&mut font, "A", Anchor::TopLeft, 9000, 50, 1));
        add_to_scene(&mut scene, &mut g, &sprites);
        assert_eq!(scene.sprites.len(), 1);
        assert_eq!(scene.items, vec![Item::Sprite(0)]);
        let p = scene.sprites[0];
        assert_eq!(p.scale, 3 * k);
        let k = i64::from(k);
        assert_eq!(p.quad(), (100 * k, 50 * k, 124 * k, 71 * k));
    }
}
