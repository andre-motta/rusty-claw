//! The credits' background movie: a movie that loops, muted, behind the
//! rising credits text, in place of the parchment.
//!
//! **Where it comes from.** The movie folder's copy named `CREDITS`
//! (`.mkv`, `.mp4`, `.mov` or `.avi`), then the long clip of the movie
//! archive (`CLAW.FEC`, clip 11): see [`claw_movie::MovieIndex::CREDITS`].
//! The same clip plays whether the credits follow the last level or the
//! main menu: the archive's clip 12, which the game could also take after
//! the last level, holds only the short placeholder of its clips 1 to 10.
//! The window looks for it at start-up and puts it on the flow's shelf
//! when `credits_movie` is on ([`found`]).
//!
//! **Playing.** [`CreditsMovie`] plays it on the wall clock with its sound
//! left out (the credits music plays as before), and when it ends goes on
//! with the same source opened again, so it loops. Opening a source takes
//! the window thread about 14 ms for a 1080p H.264 file and its first
//! picture about 10 ms more to decode (1.5 ms and under 1 ms for the
//! archive's clip; release build), so the loop point would hitch: each
//! pass opens the next one ahead, on a helper thread, once it shows its
//! first picture; the next pass decodes its first pictures and waits
//! (its clock starts when it is first read). The pass that ended is let
//! go on a helper thread too, which leaves well under a millisecond at
//! the loop point. While the window is in the
//! background it holds like every movie ([`CreditsMovie::hold`]). A source
//! that cannot open again, a stream that ends (on an error or not) before
//! any picture, or a picture that cannot be shown ends it
//! ([`CreditsMovie::poll`]); the credits then go on over the parchment. A
//! pass that showed pictures and then stops on a decoding error (a damaged
//! tail) loops like one that played to its end ([`after_pass`]).
//!
//! **Drawing.** The movie presenter draws one picture, so the text goes
//! into the movie's picture before it is uploaded: [`Canvas`] holds the
//! newest picture, enlarged by a whole factor until it is at least
//! [`TEXT_SIZE`]'s height (nearest pixel), and [`TextMask`] the credits'
//! text (and any notice) as the 640x480 screen draws it, without its
//! background. [`Canvas::overlay`] paints the text's pixels into the
//! 4:3 middle of the picture, where the credits' screen shows on the
//! display, at the picture's own resolution; the presenter then draws the
//! whole full screen at the movie's own shape. The text and the picture are
//! put together again only when either changes.

use std::thread::JoinHandle;

use claw_assets::Palette;
use claw_frame::ViewConfig;
use claw_movie::{MovieIndex, Roots, Source, locate_all};
use claw_render::cpu::{self, Composed};
use claw_render::{LevelGraphics, MoviePicture, PictureLayout, Scene, ScreenBuilder, ScreenScene};
use claw_ui::menu::draw::MenuSprite;

use crate::movie::{MovieEnd, MoviePlayback};
use crate::settings::Settings;

/// Size of the credits' screen in game pixels: the text's coordinates.
pub const TEXT_SIZE: (u32, u32) = (640, 480);

/// The largest whole factor a small movie picture is enlarged by.
const MAX_ENLARGE: u32 = 4;

/// Shape of the credits' screen, width over height.
const SCREEN_ASPECT: f32 = 4.0 / 3.0;

/// Whether this build can play the credits movie and `roots` hold one.
pub fn found(roots: &Roots) -> bool {
    claw_movie::available() && !locate_all(MovieIndex::CREDITS, roots).is_empty()
}

/// Whether the credits movie goes on the flow's shelf: `credits_movie` is
/// on and it is [`found`] under `roots`. (With movies off the window hands
/// the flow no shelf at all.)
pub fn on_shelf(settings: &Settings, roots: &Roots) -> bool {
    settings.credits_movie && found(roots)
}

/// A builder for the credits' text at [`TEXT_SIZE`], one game pixel per
/// pixel, for [`TextMask::from_scene`].
pub fn text_builder() -> ScreenBuilder {
    ScreenBuilder::new(&ViewConfig::default(), TEXT_SIZE.0)
}

/// The credits' text as the 640x480 screen draws it: a colour where the
/// text (or a notice) draws, nothing elsewhere (palette entry 0 counts as
/// nothing, so the movie shows there).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMask {
    /// One entry per game pixel, row by row: `None` where nothing draws.
    pixels: Vec<Option<[u8; 3]>>,
    /// Rows with at least one pixel drawn.
    rows: Vec<bool>,
}

impl Default for TextMask {
    fn default() -> TextMask {
        TextMask::empty()
    }
}

impl TextMask {
    /// No text at all.
    pub fn empty() -> TextMask {
        let (w, h) = TEXT_SIZE;
        TextMask {
            pixels: vec![None; (w * h) as usize],
            rows: vec![false; h as usize],
        }
    }

    /// The mask of a composed 640x480 screen in `palette` (anything of
    /// another size reads as empty).
    pub fn from_composed(c: &Composed, palette: &Palette) -> TextMask {
        let mut mask = TextMask::empty();
        let (w, h) = TEXT_SIZE;
        if (c.width, c.height) != (w, h) {
            return mask;
        }
        for (i, (&index, &hd)) in c.index.iter().zip(&c.hd).enumerate() {
            let colour = if hd >> 24 != 0 {
                let b = hd.to_le_bytes();
                Some([b[0], b[1], b[2]])
            } else if index != 0 {
                Some(palette.color(index))
            } else {
                None
            };
            if colour.is_some() {
                mask.pixels[i] = colour;
                mask.rows[i / w as usize] = true;
            }
        }
        mask
    }

    /// The mask of `scene`, a screen built by [`text_builder`] (one game
    /// pixel per pixel).
    pub fn from_scene(graphics: &LevelGraphics, scene: &Scene) -> TextMask {
        TextMask::from_composed(&cpu::compose(graphics, scene), &scene.palette)
    }

    /// The colour drawn at game pixel (x, y), if any.
    pub fn at(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        let (w, h) = TEXT_SIZE;
        if x >= w || y >= h {
            return None;
        }
        self.pixels[(y * w + x) as usize]
    }

    /// Sets game pixel (x, y) to `colour` (tests).
    pub fn set(&mut self, x: u32, y: u32, colour: Option<[u8; 3]>) {
        let (w, h) = TEXT_SIZE;
        if x < w && y < h {
            self.pixels[(y * w + x) as usize] = colour;
            let row = (y * w) as usize..((y + 1) * w) as usize;
            self.rows[y as usize] = self.pixels[row].iter().any(Option::is_some);
        }
    }

    /// Whether nothing draws.
    pub fn is_empty(&self) -> bool {
        !self.rows.iter().any(|&r| r)
    }
}

/// A movie picture the credits' text is painted into, with its planes
/// packed (each row right after the previous).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canvas {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// How the planes are laid out.
    pub layout: PictureLayout,
    /// The planes ([`PictureLayout::Rgba`] uses the first only).
    pub planes: [Vec<u8>; 3],
}

/// The plane sizes of a `w` x `h` picture of `layout`.
fn plane_sizes(layout: PictureLayout, w: u32, h: u32) -> [(u32, u32); 3] {
    match layout {
        PictureLayout::Rgba => [(w, h), (0, 0), (0, 0)],
        PictureLayout::I420 { .. } => {
            let c = (w.div_ceil(2), h.div_ceil(2));
            [(w, h), c, c]
        }
    }
}

/// Bytes per pixel of plane `n` of `layout`.
fn plane_bytes(layout: PictureLayout, n: usize) -> usize {
    match layout {
        PictureLayout::Rgba if n == 0 => 4,
        PictureLayout::Rgba => 0,
        PictureLayout::I420 { .. } => 1,
    }
}

impl Canvas {
    /// A black picture of the credits' screen's size, shown until the
    /// movie's first picture is due.
    pub fn black() -> Canvas {
        let (w, h) = TEXT_SIZE;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Canvas {
            width: w,
            height: h,
            layout: PictureLayout::Rgba,
            planes: [rgba, Vec::new(), Vec::new()],
        }
    }

    /// The whole factor a picture `height` pixels high is enlarged by: the
    /// smallest that makes it at least [`TEXT_SIZE`]'s height, at most
    /// [`MAX_ENLARGE`].
    pub fn factor(height: u32) -> u32 {
        TEXT_SIZE.1.div_ceil(height.max(1)).clamp(1, MAX_ENLARGE)
    }

    /// `picture`, enlarged by [`Canvas::factor`] (each pixel repeated), or
    /// `None` when its planes do not hold what it claims.
    pub fn from_picture(picture: &MoviePicture) -> Option<Canvas> {
        let (w, h) = (picture.width, picture.height);
        if w == 0 || h == 0 {
            return None;
        }
        let k = Canvas::factor(h);
        let layout = picture.layout;
        let (cw, ch) = (w * k, h * k);
        let src = plane_sizes(layout, w, h);
        let dst = plane_sizes(layout, cw, ch);
        let mut planes: [Vec<u8>; 3] = Default::default();
        for n in 0..3 {
            let bpp = plane_bytes(layout, n);
            let (sw, sh) = src[n];
            let (dw, dh) = dst[n];
            if bpp == 0 || dw == 0 {
                continue;
            }
            let stride = picture.strides[n];
            let data = picture.planes[n];
            let row_len = sw as usize * bpp;
            if stride < row_len || data.len() < stride * (sh as usize - 1) + row_len {
                return None;
            }
            // Where each pixel of the enlarged plane comes from: luma and
            // RGBA pixels by the factor; a colour sample by the luma
            // sample it covers.
            let map = |d: u32, s: u32| -> usize {
                let v = if n == 0 { d / k } else { (2 * d / k) / 2 };
                v.min(s - 1) as usize
            };
            let mut out = Vec::with_capacity(dw as usize * dh as usize * bpp);
            if k == 1 {
                // The same size: whole rows.
                for y in 0..sh as usize {
                    out.extend_from_slice(&data[y * stride..y * stride + row_len]);
                }
            } else {
                let cols: Vec<usize> = (0..dw).map(|x| map(x, sw)).collect();
                for y in 0..dh {
                    let row = &data[map(y, sh) * stride..];
                    for &x in &cols {
                        out.extend_from_slice(&row[x * bpp..(x + 1) * bpp]);
                    }
                }
            }
            planes[n] = out;
        }
        Some(Canvas {
            width: cw,
            height: ch,
            layout,
            planes,
        })
    }

    /// The picture as the presenter takes it.
    pub fn picture(&self) -> MoviePicture<'_> {
        let sizes = plane_sizes(self.layout, self.width, self.height);
        let stride = |n: usize| sizes[n].0 as usize * plane_bytes(self.layout, n);
        MoviePicture {
            width: self.width,
            height: self.height,
            layout: self.layout,
            planes: [&self.planes[0], &self.planes[1], &self.planes[2]],
            strides: [stride(0), stride(1), stride(2)],
        }
    }

    /// Where the credits' 640x480 screen lies in the picture when the
    /// picture shows at `aspect` (width over height): the largest 4:3
    /// area, centred, as (x, y, width, height) in pixels.
    pub fn text_area(&self, aspect: f32) -> (u32, u32, u32, u32) {
        let (w, h) = (self.width, self.height);
        let a = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            w as f32 / h.max(1) as f32
        };
        if a >= SCREEN_ASPECT {
            let tw = ((w as f32 * SCREEN_ASPECT / a).round() as u32).clamp(1, w);
            ((w - tw) / 2, 0, tw, h)
        } else {
            let th = ((h as f32 * a / SCREEN_ASPECT).round() as u32).clamp(1, h);
            (0, (h - th) / 2, w, th)
        }
    }

    /// Paints `mask` into the picture's 4:3 middle (see
    /// [`Canvas::text_area`]), each picture pixel taking the text pixel
    /// it covers.
    pub fn overlay(&mut self, mask: &TextMask, aspect: f32) {
        if mask.is_empty() {
            return;
        }
        let (x0, y0, tw, th) = self.text_area(aspect);
        let (gw, gh) = (u64::from(TEXT_SIZE.0), u64::from(TEXT_SIZE.1));
        // Picture column x shows game column x * gw / tw, so game column gx
        // starts at picture column ceil(gx * tw / gw).
        let start = |gx: usize| x0 as usize + (gx as u64 * u64::from(tw)).div_ceil(gw) as usize;
        let width = self.width as usize;
        let cw = self.width.div_ceil(2) as usize;
        let layout = self.layout;
        let [p0, p1, p2] = &mut self.planes;
        // The runs of one colour of the game row last painted, as picture
        // columns.
        let mut runs: Vec<(usize, usize, [u8; 3])> = Vec::new();
        let mut runs_of = None;
        for y in 0..th {
            let gy = (u64::from(y) * gh / u64::from(th)) as usize;
            if !mask.rows[gy] {
                continue;
            }
            if runs_of != Some(gy) {
                runs_of = Some(gy);
                runs.clear();
                let row = &mask.pixels[gy * TEXT_SIZE.0 as usize..][..TEXT_SIZE.0 as usize];
                let mut gx = 0;
                while gx < row.len() {
                    let Some(rgb) = row[gx] else {
                        gx += 1;
                        continue;
                    };
                    let from = gx;
                    while gx < row.len() && row[gx] == Some(rgb) {
                        gx += 1;
                    }
                    let colour = match layout {
                        PictureLayout::Rgba => rgb,
                        PictureLayout::I420 {
                            bt709, full_range, ..
                        } => rgb_to_yuv(rgb, bt709, full_range),
                    };
                    runs.push((start(from), start(gx), colour));
                }
            }
            let py = (y0 + y) as usize;
            for &(from, to, c) in &runs {
                match layout {
                    PictureLayout::Rgba => {
                        for px in
                            p0[(py * width + from) * 4..(py * width + to) * 4].chunks_exact_mut(4)
                        {
                            px.copy_from_slice(&[c[0], c[1], c[2], 255]);
                        }
                    }
                    PictureLayout::I420 { .. } => {
                        p0[py * width + from..py * width + to].fill(c[0]);
                        let row = (py / 2) * cw;
                        p1[row + from / 2..row + to.div_ceil(2)].fill(c[1]);
                        p2[row + from / 2..row + to.div_ceil(2)].fill(c[2]);
                    }
                }
            }
        }
    }

    /// Darkens the picture to `brightness` (0 black, 1 as it is).
    pub fn dim(&mut self, brightness: f64) {
        let b = brightness.clamp(0.0, 1.0);
        if b >= 1.0 {
            return;
        }
        let scale = |v: u8, base: f64| (base + (f64::from(v) - base) * b).round() as u8;
        match self.layout {
            PictureLayout::Rgba => {
                for px in self.planes[0].chunks_exact_mut(4) {
                    for c in &mut px[..3] {
                        *c = scale(*c, 0.0);
                    }
                }
            }
            PictureLayout::I420 { full_range, .. } => {
                let black = if full_range { 0.0 } else { 16.0 };
                for v in &mut self.planes[0] {
                    *v = scale(*v, black);
                }
                for plane in &mut self.planes[1..] {
                    for v in plane.iter_mut() {
                        *v = scale(*v, 128.0);
                    }
                }
            }
        }
    }
}

/// `rgb` (sRGB bytes) as Y, U and V bytes of the BT.709 matrix (otherwise
/// BT.601), full range (otherwise limited): the inverse of the presenter's
/// conversion.
pub fn rgb_to_yuv(rgb: [u8; 3], bt709: bool, full_range: bool) -> [u8; 3] {
    let (kr, kb) = if bt709 {
        (0.2126f64, 0.0722f64)
    } else {
        (0.299, 0.114)
    };
    let kg = 1.0 - kr - kb;
    let [r, g, b] = rgb.map(|c| f64::from(c) / 255.0);
    let y = kr * r + kg * g + kb * b;
    let u = (b - y) / (2.0 * (1.0 - kb));
    let v = (r - y) / (2.0 * (1.0 - kr));
    let (y_scale, c_scale, y_offset) = if full_range {
        (255.0, 255.0, 0.0)
    } else {
        (219.0, 224.0, 16.0)
    };
    let byte = |x: f64| x.round().clamp(0.0, 255.0) as u8;
    [
        byte(y_offset + y * y_scale),
        byte(128.0 + u * c_scale),
        byte(128.0 + v * c_scale),
    ]
}

/// Whether the credits movie starts again after a pass ended as `end`,
/// with decoding stopped on `error` if it did: `Ok` when it loops, `Err`
/// with the reason when it cannot go on. A pass that showed pictures loops
/// even when decoding stopped on an error near its end (a damaged tail),
/// as a movie played that far counts as played to its end
/// ([`crate::movie::MovieProgress::end`]); only a pass that showed no
/// picture fails.
pub fn after_pass(end: MovieEnd, error: Option<String>) -> Result<(), String> {
    match end {
        MovieEnd::Finished => Ok(()),
        MovieEnd::Failed => Err(error.unwrap_or_else(|| "it showed no picture".into())),
        // No input reaches it.
        MovieEnd::Skipped => Err("it was skipped".into()),
    }
}

/// The credits movie playing behind the credits (see the module
/// documentation).
#[derive(Debug)]
pub struct CreditsMovie {
    playback: MoviePlayback,
    /// The next pass, opened ahead on a helper thread once this pass
    /// showed its first picture (`None` before, or when the helper could
    /// not start: the next pass opens at the loop point then).
    next: Option<JoinHandle<Option<MoviePlayback>>>,
    /// Times it started again from the beginning.
    loops: u32,
    /// The terminal heard that a pass stopped early on an error.
    told_damaged: bool,
    /// The newest picture, enlarged; black before the first.
    base: Canvas,
    /// The shape it shows at.
    aspect: f32,
    /// A new picture came since the last [`CreditsMovie::compose`].
    fresh: bool,
    /// The text drawn over it.
    mask: TextMask,
    /// What the text was made from (the screen, what is drawn over it and
    /// the brightness), to make it again only when that changes.
    key: Option<(ScreenScene, Vec<MenuSprite>, u64)>,
    /// The picture with the text, as last composed.
    shown: Canvas,
    builder: ScreenBuilder,
}

impl CreditsMovie {
    /// Opens the first of `candidates` that plays, muted. `None` when none
    /// plays (each source tried says why on the terminal).
    pub fn start(candidates: &[Source]) -> Option<CreditsMovie> {
        let playback = MoviePlayback::start(candidates, None)?;
        Some(CreditsMovie {
            playback,
            next: None,
            loops: 0,
            told_damaged: false,
            base: Canvas::black(),
            aspect: SCREEN_ASPECT,
            fresh: true,
            mask: TextMask::empty(),
            key: None,
            shown: Canvas::black(),
            builder: text_builder(),
        })
    }

    /// Where it plays from.
    pub fn source(&self) -> &Source {
        self.playback.source()
    }

    /// Times it started again from the beginning.
    pub fn loops(&self) -> u32 {
        self.loops
    }

    /// The shape it shows at, width over height (the credits' screen's
    /// before its first picture).
    pub fn aspect(&self) -> f32 {
        self.aspect
    }

    /// Holds it while the window is in the background, or lets it go on
    /// (see [`MoviePlayback::hold`]).
    pub fn hold(&mut self, on: bool) {
        self.playback.hold(on);
    }

    /// Checks how it plays: at the end of its stream it goes on from the
    /// first picture of the same source opened again (opened ahead, see
    /// the module documentation). A pass that showed pictures and then
    /// stopped on a decoding error (a damaged tail) ends like any other and
    /// loops ([`after_pass`]); the terminal hears of the error once. `Err` with the reason when it cannot go on: the stream
    /// ended, on an error or not, before any picture, or the source would
    /// not open again.
    pub fn poll(&mut self) -> Result<(), String> {
        let Some(end) = self.playback.end() else {
            return Ok(());
        };
        let error = self.playback.error();
        after_pass(end, error.clone())?;
        if let Some(e) = error
            && !self.told_damaged
        {
            self.told_damaged = true;
            eprintln!(
                "rusty-claw: the credits movie {} stopped early ({e}); it starts again from its first picture",
                self.playback.source().describe()
            );
        }
        let next = match self.next.take() {
            // Opened ahead: ready by now, or soon.
            Some(opening) => opening.join().ok().flatten(),
            None => MoviePlayback::start(std::slice::from_ref(self.playback.source()), None),
        };
        match next {
            Some(p) => {
                // Freeing the ended pass's decoders takes a millisecond or
                // two: on a helper thread (here when none starts).
                let ended = std::mem::replace(&mut self.playback, p);
                let _ = std::thread::Builder::new()
                    .name("credits-movie-ended".into())
                    .spawn(move || drop(ended));
                self.loops += 1;
                Ok(())
            }
            None => Err("it would not open again".into()),
        }
    }

    /// Opens the next pass on a helper thread, unless it is open or
    /// opening already. Its clock starts at its first reading, so it waits,
    /// its first pictures decoded, until the loop point.
    fn open_next(&mut self) {
        if self.next.is_some() {
            return;
        }
        let source = self.playback.source().clone();
        self.next = std::thread::Builder::new()
            .name("credits-movie-next".into())
            .spawn(move || MoviePlayback::start(std::slice::from_ref(&source), None))
            .ok();
    }

    /// Takes the newest picture due, if a new one is. `Err` when its planes
    /// do not hold what it claims.
    pub fn pull(&mut self) -> Result<bool, String> {
        let Some(picture) = self.playback.frame() else {
            return Ok(false);
        };
        let canvas = Canvas::from_picture(&picture).ok_or_else(|| "a bad picture".to_string())?;
        self.base = canvas;
        self.aspect = self.playback.aspect();
        self.fresh = true;
        self.open_next();
        Ok(true)
    }

    /// The picture with the text drawn over it, made again when a new
    /// picture came ([`CreditsMovie::pull`]) or the text's screen `scene`,
    /// what is drawn over it (`over`) or `brightness` differ from the last
    /// call's; `None` when nothing changed. `text` makes the text's mask
    /// with a builder from [`text_builder`] (only when the text changed),
    /// and `brightness` darkens the result.
    pub fn compose(
        &mut self,
        scene: &ScreenScene,
        over: &[MenuSprite],
        brightness: f64,
        text: impl FnOnce(&ScreenBuilder) -> TextMask,
    ) -> Option<&Canvas> {
        let bits = brightness.to_bits();
        let same_text = self
            .key
            .as_ref()
            .is_some_and(|(s, o, _)| s == scene && o.as_slice() == over);
        let same = same_text && self.key.as_ref().is_some_and(|k| k.2 == bits);
        if same && !self.fresh {
            return None;
        }
        if !same_text {
            self.mask = text(&self.builder);
        }
        if !same {
            self.key = Some((scene.clone(), over.to_vec(), bits));
        }
        self.fresh = false;
        self.shown.clone_from(&self.base);
        self.shown.overlay(&self.mask, self.aspect);
        self.shown.dim(brightness);
        Some(&self.shown)
    }

    /// Stops it; says why when decoding stopped early. A next pass still
    /// opening is let go when its helper thread is done.
    pub fn stop(self) {
        self.playback.stop(None);
    }
}
