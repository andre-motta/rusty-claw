//! The credits' background movie: the picture the credits text is painted
//! into (made here in memory), and the movie itself, looping and muted, on
//! a small movie made here (an AVI file of 32x24 pictures at 10 a second,
//! raw BGR, with stereo 16-bit sound at 8000 Hz). With `CLAW_INSTALL` set,
//! the install's own credits movie is found and plays. A build without
//! FFmpeg has no movie player, so there the movie tests say they skipped.

use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use claw_assets::Palette;
use claw_game::credits_movie::{
    Canvas, CreditsMovie, TEXT_SIZE, TextMask, on_shelf, rgb_to_yuv, text_builder,
};
use claw_game::settings::{LevelChoice, Settings};
use claw_movie::{FileKind, MovieIndex, MovieMode, Roots, Source, locate_all};
use claw_render::cpu::Composed;
use claw_render::movie::yuv_to_rgb;
use claw_render::{ChromaSiting, MoviePicture, PictureLayout, ScreenScene};

const W: u32 = 32;
const H: u32 = 24;
const FPS: u32 = 10;
const SOUND_RATE: u32 = 8000;

fn chunk(out: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) -> usize {
    let at = out.len();
    out.extend_from_slice(id);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
    at
}

fn list(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    let mut data = kind.to_vec();
    data.extend_from_slice(body);
    chunk(out, b"LIST", &data);
}

fn le(v: &mut Vec<u8>, words: &[u32]) {
    for w in words {
        v.extend_from_slice(&w.to_le_bytes());
    }
}

/// The grey of picture `i`.
fn grey(i: u32) -> u8 {
    (40 + i * 20) as u8
}

/// A test movie of `frames` pictures as AVI bytes: picture `i` all
/// [`grey`]`(i)`, its sound a quiet square wave stored right after it.
fn movie(frames: u32) -> Vec<u8> {
    let image = (W * 3 * H) as usize;
    let per_frame = SOUND_RATE / FPS;
    let block = 4u32;
    let mut avih = Vec::new();
    le(
        &mut avih,
        &[
            1_000_000 / FPS,
            0,
            0,
            0x10 | 0x100,
            frames,
            0,
            2,
            image as u32,
            W,
            H,
            0,
            0,
            0,
            0,
        ],
    );
    let mut vstrh = b"vids".to_vec();
    le(
        &mut vstrh,
        &[
            0,
            0,
            0,
            0,
            1,
            FPS,
            0,
            frames,
            image as u32,
            u32::MAX,
            0,
            0,
            0,
        ],
    );
    let mut vstrf = Vec::new();
    le(&mut vstrf, &[40, W, H]);
    vstrf.extend_from_slice(&1u16.to_le_bytes());
    vstrf.extend_from_slice(&24u16.to_le_bytes());
    le(&mut vstrf, &[0, image as u32, 0, 0, 0, 0]);
    let mut astrh = b"auds".to_vec();
    le(
        &mut astrh,
        &[
            0,
            0,
            0,
            0,
            block,
            SOUND_RATE * block,
            0,
            frames * per_frame,
            per_frame * block,
            u32::MAX,
            block,
            0,
            0,
        ],
    );
    let mut astrf = Vec::new();
    astrf.extend_from_slice(&1u16.to_le_bytes());
    astrf.extend_from_slice(&2u16.to_le_bytes());
    le(&mut astrf, &[SOUND_RATE, SOUND_RATE * block]);
    astrf.extend_from_slice(&(block as u16).to_le_bytes());
    astrf.extend_from_slice(&16u16.to_le_bytes());
    astrf.extend_from_slice(&0u16.to_le_bytes());

    let mut vstrl = Vec::new();
    chunk(&mut vstrl, b"strh", &vstrh);
    chunk(&mut vstrl, b"strf", &vstrf);
    let mut astrl = Vec::new();
    chunk(&mut astrl, b"strh", &astrh);
    chunk(&mut astrl, b"strf", &astrf);
    let mut hdrl = Vec::new();
    chunk(&mut hdrl, b"avih", &avih);
    list(&mut hdrl, b"strl", &vstrl);
    list(&mut hdrl, b"strl", &astrl);

    let mut movi = b"movi".to_vec();
    let mut idx = Vec::new();
    for i in 0..frames {
        let pixels = vec![grey(i); image];
        let at = chunk(&mut movi, b"00db", &pixels);
        idx.extend_from_slice(b"00db");
        le(&mut idx, &[0x10, at as u32, pixels.len() as u32]);
        let mut sound = Vec::new();
        for n in 0..per_frame {
            let s: i16 = if (n / 20) % 2 == 0 { 2000 } else { -2000 };
            sound.extend_from_slice(&s.to_le_bytes());
            sound.extend_from_slice(&s.to_le_bytes());
        }
        let at = chunk(&mut movi, b"01wb", &sound);
        idx.extend_from_slice(b"01wb");
        le(&mut idx, &[0x10, at as u32, sound.len() as u32]);
    }

    let mut body = b"AVI ".to_vec();
    list(&mut body, b"hdrl", &hdrl);
    list(&mut body, b"movi", &movi[4..]);
    chunk(&mut body, b"idx1", &idx);
    let mut out = Vec::new();
    chunk(&mut out, b"RIFF", &body);
    out
}

/// An RGBA picture `w` x `h` whose pixel (x, y) is (x, y, 7, 255), its
/// rows `pad` bytes longer than they need be.
fn rgba(w: u32, h: u32, pad: usize) -> (Vec<u8>, usize) {
    let stride = w as usize * 4 + pad;
    let mut data = vec![0xEE; stride * h as usize];
    for y in 0..h as usize {
        for x in 0..w as usize {
            let o = y * stride + x * 4;
            data[o..o + 4].copy_from_slice(&[x as u8, y as u8, 7, 255]);
        }
    }
    (data, stride)
}

fn rgba_at(c: &Canvas, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * c.width + x) * 4) as usize;
    c.planes[0][o..o + 4].try_into().unwrap()
}

#[test]
fn small_pictures_are_enlarged_to_the_credits_height_by_whole_factors() {
    assert_eq!(Canvas::factor(240), 2);
    assert_eq!(Canvas::factor(480), 1);
    assert_eq!(Canvas::factor(1080), 1);
    assert_eq!(Canvas::factor(200), 3);
    assert_eq!(Canvas::factor(24), 4, "at most four times");
    assert_eq!(Canvas::factor(0), 4);

    // A 320x240 picture (the archive's size), with padded rows: 640x480,
    // each pixel repeated twice each way.
    let (data, stride) = rgba(320, 240, 12);
    let p = MoviePicture {
        width: 320,
        height: 240,
        layout: PictureLayout::Rgba,
        planes: [&data, &[], &[]],
        strides: [stride, 0, 0],
    };
    let c = Canvas::from_picture(&p).unwrap();
    assert_eq!((c.width, c.height), TEXT_SIZE);
    assert_eq!(c.planes[0].len(), 640 * 480 * 4);
    for (x, y) in [(0, 0), (1, 1), (2, 3), (639, 479), (101, 200)] {
        assert_eq!(rgba_at(&c, x, y), [(x / 2) as u8, (y / 2) as u8, 7, 255]);
    }
    let shown = c.picture();
    assert_eq!(shown.strides[0], 640 * 4);
    assert_eq!((shown.width, shown.height), TEXT_SIZE);

    // Planes shorter than they claim are refused.
    let short = MoviePicture {
        planes: [&data[..data.len() - 20], &[], &[]],
        ..p
    };
    assert!(Canvas::from_picture(&short).is_none());
    let empty = MoviePicture { width: 0, ..p };
    assert!(Canvas::from_picture(&empty).is_none());
}

#[test]
fn i420_pictures_keep_their_planes_and_their_colour_samples_follow() {
    let layout = PictureLayout::I420 {
        bt709: true,
        full_range: false,
        chroma: ChromaSiting::Left,
    };
    // 6x4, 3x2 colour samples, Y rows padded to 8.
    let y: Vec<u8> = (0..32).collect();
    let u = [100u8, 101, 102, 110, 111, 112];
    let v = [200u8, 201, 202, 210, 211, 212];
    let p = MoviePicture {
        width: 6,
        height: 4,
        layout,
        planes: [&y, &u, &v],
        strides: [8, 3, 3],
    };
    // Enlarged four times: 24x16, colour samples 12x8.
    let c = Canvas::from_picture(&p).unwrap();
    assert_eq!((c.width, c.height), (24, 16));
    assert_eq!(c.layout, layout);
    assert_eq!(c.planes[0].len(), 24 * 16);
    assert_eq!(c.planes[1].len(), 12 * 8);
    assert_eq!(c.planes[0][0], 0);
    assert_eq!(c.planes[0][4], 1, "the second Y sample from x 4");
    assert_eq!(c.planes[0][24 * 4 + 5], 9, "row 1, column 1");
    // Colour sample (cx, cy) covers Y (2cx, 2cy) of the enlarged picture,
    // which is Y (2cx/4, 2cy/4) of the original, under its sample.
    assert_eq!(c.planes[1][0], 100);
    assert_eq!(c.planes[1][4], 101, "x 8 of 24 is x 2 of 6");
    assert_eq!(c.planes[2][12 * 4 + 11], 212);
    let shown = c.picture();
    assert_eq!(shown.strides, [24, 12, 12]);
}

#[test]
fn the_text_goes_into_the_four_by_three_middle_of_the_picture() {
    // A 4:3 picture of the credits' size: one pixel each.
    let mut c = Canvas::black();
    assert_eq!(c.text_area(4.0 / 3.0), (0, 0, 640, 480));
    let mut mask = TextMask::empty();
    assert!(mask.is_empty());
    mask.set(50, 100, Some([255, 255, 255]));
    mask.set(589, 479, Some([10, 20, 30]));
    assert!(!mask.is_empty());
    c.overlay(&mask, 4.0 / 3.0);
    assert_eq!(rgba_at(&c, 50, 100), [255, 255, 255, 255]);
    assert_eq!(rgba_at(&c, 589, 479), [10, 20, 30, 255]);
    assert_eq!(rgba_at(&c, 51, 100), [0, 0, 0, 255], "the movie elsewhere");

    // A wide picture: the text keeps its shape in the middle.
    let wide = Canvas {
        width: 1920,
        height: 1080,
        layout: PictureLayout::Rgba,
        planes: [vec![9; 1920 * 1080 * 4], Vec::new(), Vec::new()],
    };
    assert_eq!(wide.text_area(16.0 / 9.0), (240, 0, 1440, 1080));
    let mut wide2 = wide.clone();
    wide2.overlay(&mask, 16.0 / 9.0);
    // Game pixel (50, 100) is 2.25 picture pixels each way, from picture
    // pixel (240 + 112.5, 225): the first whole one is x 353.
    let (px, py) = (240 + 113, 225);
    assert_eq!(rgba_at(&wide2, px + 1, py + 1), [255, 255, 255, 255]);
    assert_eq!(rgba_at(&wide2, px, py), [255, 255, 255, 255]);
    assert_eq!(rgba_at(&wide2, px - 1, py), [9, 9, 9, 9]);
    assert_eq!(rgba_at(&wide2, 100, py), [9, 9, 9, 9], "the side");
    // Narrower than 4:3: the text is as wide as the picture, centred
    // vertically. Pixels that are not square count: 720x480 shown at 4:3
    // is all text area.
    let tall = Canvas {
        width: 480,
        height: 480,
        ..wide.clone()
    };
    assert_eq!(tall.text_area(1.0), (0, 60, 480, 360));
    let dvd = Canvas {
        width: 720,
        height: 480,
        ..wide
    };
    assert_eq!(dvd.text_area(4.0 / 3.0), (0, 0, 720, 480));
}

#[test]
fn text_in_an_i420_picture_keeps_its_colour() {
    for (bt709, full_range) in [(false, false), (true, false), (false, true), (true, true)] {
        let layout = PictureLayout::I420 {
            bt709,
            full_range,
            chroma: ChromaSiting::Left,
        };
        let mut c = Canvas {
            width: 640,
            height: 480,
            layout,
            planes: [
                vec![16; 640 * 480],
                vec![128; 320 * 240],
                vec![128; 320 * 240],
            ],
        };
        let mut mask = TextMask::empty();
        let colour = [200, 40, 90];
        mask.set(301, 77, Some(colour));
        c.overlay(&mask, 4.0 / 3.0);
        let yuv = [
            c.planes[0][77 * 640 + 301],
            c.planes[1][38 * 320 + 150],
            c.planes[2][38 * 320 + 150],
        ];
        assert_eq!(yuv, rgb_to_yuv(colour, bt709, full_range));
        // Back through the presenter's conversion: the same colour.
        let m = yuv_to_rgb(bt709, full_range);
        let f = yuv.map(|b| f32::from(b) / 255.0);
        for (i, row) in m.rows.iter().enumerate() {
            let v: f32 = (0..3).map(|j| row[j] * (f[j] - m.offset[j])).sum();
            let back = (v * 255.0).round();
            assert!(
                (back - f32::from(colour[i])).abs() <= 3.0,
                "channel {i}: {back} for {}",
                colour[i]
            );
        }
        assert_eq!(c.planes[0][77 * 640 + 303], 16, "the movie elsewhere");
    }
    // White is white at every matrix and range.
    assert_eq!(rgb_to_yuv([255; 3], false, false), [235, 128, 128]);
    assert_eq!(rgb_to_yuv([255; 3], true, true), [255, 128, 128]);
}

#[test]
fn a_fade_darkens_the_picture_and_its_text() {
    let mut c = Canvas::black();
    let mut mask = TextMask::empty();
    mask.set(0, 0, Some([200, 100, 50]));
    c.overlay(&mask, 4.0 / 3.0);
    c.dim(1.0);
    assert_eq!(rgba_at(&c, 0, 0), [200, 100, 50, 255]);
    c.dim(0.5);
    assert_eq!(rgba_at(&c, 0, 0), [100, 50, 25, 255]);
    let mut yuv = Canvas {
        width: 2,
        height: 2,
        layout: PictureLayout::I420 {
            bt709: false,
            full_range: false,
            chroma: ChromaSiting::Left,
        },
        planes: [vec![216; 4], vec![228], vec![28]],
    };
    yuv.dim(0.0);
    assert_eq!(yuv.planes, [vec![16; 4], vec![128], vec![128]]);
}

#[test]
fn the_text_mask_is_what_the_screen_draws_without_index_zero() {
    let (w, h) = TEXT_SIZE;
    let mut palette = Palette::default();
    palette.0[5] = [1, 2, 3];
    let mut index = vec![0u8; (w * h) as usize];
    let mut hd = vec![0u32; (w * h) as usize];
    index[w as usize * 3 + 4] = 5;
    hd[w as usize * 9 + 8] = 0xFF00_0A0B | (0x0C << 16);
    let c = Composed {
        width: w,
        height: h,
        index,
        hd,
    };
    let mask = TextMask::from_composed(&c, &palette);
    assert_eq!(mask.at(4, 3), Some([1, 2, 3]));
    assert_eq!(mask.at(8, 9), Some([0x0B, 0x0A, 0x0C]));
    assert_eq!(mask.at(5, 3), None);
    assert_eq!(mask.at(w, 0), None);
    // Anything but the credits' size draws nothing.
    let small = Composed {
        width: 4,
        height: 4,
        index: vec![5; 16],
        hd: vec![0; 16],
    };
    assert!(TextMask::from_composed(&small, &palette).is_empty());
    // The builder draws one game pixel per pixel.
    let b = text_builder();
    assert_eq!(b.composition().target, TEXT_SIZE);
    assert_eq!(b.composition().scale, 1);
}

#[test]
fn the_setting_and_the_sources_decide_whether_the_credits_have_a_movie() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("movies");
    std::fs::create_dir_all(&user).unwrap();
    let roots = Roots {
        install: dir.path().join("game"),
        user_folder: Some(user.clone()),
        mode: MovieMode::Upscaled,
    };
    let mut settings = Settings::new(dir.path().join("game"), LevelChoice::Retail(1));
    assert!(settings.credits_movie, "on by default");
    assert!(!on_shelf(&settings, &roots), "no movie found");
    std::fs::write(user.join("Credits.mkv"), b"x").unwrap();
    assert_eq!(on_shelf(&settings, &roots), claw_movie::available());
    settings.credits_movie = false;
    assert!(!on_shelf(&settings, &roots), "credits_movie = off");
}

/// Runs frames of 5 ms until `until` says stop or `limit` passes; true
/// when `until` said stop.
fn frames_until(
    m: &mut CreditsMovie,
    limit: Duration,
    mut until: impl FnMut(&mut CreditsMovie) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if until(m) {
            return true;
        }
        sleep(Duration::from_millis(5));
    }
    false
}

/// The grey of the newest picture, when a new one came.
fn pull_grey(m: &mut CreditsMovie) -> Option<u8> {
    if !m.pull().unwrap() {
        return None;
    }
    let scene = ScreenScene {
        background: None,
        palette: Palette::default(),
        sprites: Vec::new(),
        fade: None,
    };
    let c = m.compose(&scene, &[], 1.0, |_| TextMask::empty()).unwrap();
    // 32x24 enlarged four times.
    assert_eq!((c.width, c.height), (W * 4, H * 4));
    Some(c.planes[0][0])
}

fn file(dir: &std::path::Path, frames: u32) -> Source {
    let path = dir.join("CREDITS.avi");
    std::fs::write(&path, movie(frames)).unwrap();
    Source::File {
        path,
        kind: FileKind::Upscaled,
    }
}

#[test]
fn the_credits_movie_loops_muted_and_draws_the_text_only_when_something_changed() {
    if !claw_movie::available() {
        eprintln!("skipped: built without FFmpeg");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = file(dir.path(), 4);
    let mut m = CreditsMovie::start(std::slice::from_ref(&source)).expect("it opens");
    assert_eq!(m.source(), &source);
    assert_eq!(m.loops(), 0);
    // Three passes of its four pictures, each from the first.
    let mut greys = Vec::new();
    assert!(
        frames_until(&mut m, Duration::from_secs(10), |m| {
            m.poll().expect("it goes on");
            if let Some(g) = pull_grey(m) {
                greys.push(g);
            }
            m.loops() >= 3
        }),
        "it loops"
    );
    let firsts = greys.iter().filter(|&&g| g == grey(0)).count();
    assert!(firsts >= 3, "{greys:?}");
    assert!(greys.contains(&grey(3)), "{greys:?}");
    assert!((m.aspect() - 4.0 / 3.0).abs() < 1e-3);

    // Nothing new: nothing to draw again; a new text or brightness: the
    // same picture with it.
    let scene = ScreenScene {
        background: None,
        palette: Palette::default(),
        sprites: Vec::new(),
        fade: None,
    };
    let mut made = 0;
    let _ = m.compose(&scene, &[], 1.0, |_| {
        made += 1;
        TextMask::empty()
    });
    assert!(m.compose(&scene, &[], 1.0, |_| unreachable!()).is_none());
    let dim = m.compose(&scene, &[], 0.5, |_| unreachable!()).cloned();
    assert!(dim.is_some(), "a new brightness draws again");
    let mut other = scene.clone();
    other.palette.0[1] = [1, 1, 1];
    let mut mask = TextMask::empty();
    mask.set(0, 0, Some([255, 0, 0]));
    let c = m
        .compose(&other, &[], 0.5, |b| {
            assert_eq!(b.composition().target, TEXT_SIZE);
            mask.clone()
        })
        .expect("a new text draws again");
    assert_eq!(
        &c.planes[0][..4],
        &[128, 0, 0, 255],
        "red at half brightness"
    );
    assert!(made <= 1);
    m.stop();
}

#[test]
fn a_credits_movie_that_cannot_go_on_says_why() {
    if !claw_movie::available() {
        eprintln!("skipped: built without FFmpeg");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // Nothing that plays: no movie.
    let junk = dir.path().join("CREDITS.mp4");
    std::fs::write(&junk, b"no movie at all").unwrap();
    let junk = Source::File {
        path: junk,
        kind: FileKind::Upscaled,
    };
    assert!(CreditsMovie::start(std::slice::from_ref(&junk)).is_none());
    assert!(CreditsMovie::start(&[]).is_none());
    // The next candidate plays when the first does not.
    let good = file(dir.path(), 3);
    let m = CreditsMovie::start(&[junk, good.clone()]).expect("the second opens");
    assert_eq!(m.source(), &good);
    drop(m);

    // Gone before it could loop: it cannot open again.
    let mut m = CreditsMovie::start(std::slice::from_ref(&good)).unwrap();
    let Source::File { path, .. } = &good else {
        unreachable!()
    };
    std::fs::remove_file(path).unwrap();
    let mut why = None;
    assert!(frames_until(&mut m, Duration::from_secs(10), |m| {
        let _ = m.pull();
        why = m.poll().err();
        why.is_some()
    }));
    assert!(why.unwrap().contains("open again"));
    assert_eq!(m.loops(), 0);
}

#[test]
fn a_held_credits_movie_shows_no_new_picture_and_does_not_loop() {
    if !claw_movie::available() {
        eprintln!("skipped: built without FFmpeg");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = file(dir.path(), 3);
    let mut m = CreditsMovie::start(std::slice::from_ref(&source)).unwrap();
    assert!(frames_until(&mut m, Duration::from_secs(5), |m| m
        .pull()
        .unwrap()));
    m.hold(true);
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(600) {
        assert!(!m.pull().unwrap(), "no picture while held");
        m.poll().unwrap();
        sleep(Duration::from_millis(10));
    }
    assert_eq!(m.loops(), 0, "a held movie does not end");
    m.hold(false);
    assert!(frames_until(&mut m, Duration::from_secs(5), |m| {
        m.poll().unwrap();
        let _ = m.pull();
        m.loops() >= 1
    }));
}

#[test]
fn the_install_credits_movie_plays_behind_the_credits() {
    let Some(install) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let roots = Roots {
        install,
        user_folder: None,
        mode: MovieMode::Archive,
    };
    let found = locate_all(MovieIndex::CREDITS, &roots);
    if found.is_empty() {
        eprintln!("skipped: this install has no movie archive with the credits clip");
        return;
    }
    assert!(
        found
            .iter()
            .all(|s| matches!(s, Source::Archive { entry: 11, .. })),
        "{found:?}"
    );
    if !claw_movie::available() {
        eprintln!("skipped: built without FFmpeg");
        return;
    }
    let settings = Settings::new(roots.install.clone(), LevelChoice::Retail(1));
    assert!(on_shelf(&settings, &roots));
    let mut m = CreditsMovie::start(&found).expect("the credits clip opens");
    let mut size = None;
    assert!(frames_until(&mut m, Duration::from_secs(10), |m| {
        m.poll().expect("it plays");
        if m.pull().unwrap() {
            let scene = ScreenScene {
                background: None,
                palette: Palette::default(),
                sprites: Vec::new(),
                fade: None,
            };
            let mut mask = TextMask::empty();
            mask.set(320, 240, Some([255, 255, 255]));
            size = m
                .compose(&scene, &[], 1.0, |_| mask)
                .map(|c| (c.width, c.height, c.picture().width));
        }
        size.is_some()
    }));
    // The archive's 320x240 clip, twice its size: the credits' screen.
    assert_eq!(size, Some((640, 480, 640)));
    assert!((m.aspect() - 4.0 / 3.0).abs() < 1e-3);
}
