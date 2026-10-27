//! Movie pictures: where they show at each output shape, the colours of
//! YUV pictures, and (with `CLAW_GPU_TESTS=1`, under the bench lock) RGBA
//! and I420 pictures drawn offscreen and compared with the CPU reference.

use claw_render::cpu::{movie_frame, movie_pixel};
use claw_render::gpu::COLOR_FORMAT;
use claw_render::movie::yuv_to_rgb;
use claw_render::{
    Gpu, MovieDrawError, MoviePicture, MoviePresenter, PictureLayout, movie_rect, wgpu,
};

const FOUR_THREE: f32 = 4.0 / 3.0;
const ORANGE: [u8; 4] = [230, 120, 20, 255];
const BLUE: [u8; 4] = [20, 60, 210, 255];

#[test]
fn a_four_three_movie_is_centred_with_bars_at_the_sides_of_wide_outputs() {
    assert_eq!(movie_rect((1920, 1080), FOUR_THREE), (240, 0, 1440, 1080));
    assert_eq!(movie_rect((3840, 1600), FOUR_THREE), (853, 0, 2133, 1600));
    assert_eq!(movie_rect((2560, 1440), FOUR_THREE), (320, 0, 1920, 1440));
}

#[test]
fn a_four_three_movie_fills_a_four_three_output() {
    assert_eq!(movie_rect((800, 600), FOUR_THREE), (0, 0, 800, 600));
    assert_eq!(movie_rect((640, 480), FOUR_THREE), (0, 0, 640, 480));
}

#[test]
fn a_movie_on_a_tall_output_has_its_bars_above_and_below() {
    assert_eq!(movie_rect((1280, 1024), FOUR_THREE), (0, 32, 1280, 960));
    assert_eq!(movie_rect((800, 1280), FOUR_THREE), (0, 340, 800, 600));
}

#[test]
fn a_movie_rect_stays_inside_odd_outputs() {
    assert_eq!(movie_rect((0, 600), FOUR_THREE), (0, 0, 0, 0));
    assert_eq!(movie_rect((1, 1), FOUR_THREE), (0, 0, 1, 1));
    assert_eq!(movie_rect((1920, 1080), f32::NAN), (0, 0, 1920, 1080));
    assert_eq!(movie_rect((1920, 1080), 0.0), (0, 0, 1920, 1080));
    for (w, h) in [(1921, 1081), (3, 1000), (1000, 3), (7, 5)] {
        for aspect in [FOUR_THREE, 16.0 / 9.0, 1.0, 2.39] {
            let (x, y, rw, rh) = movie_rect((w, h), aspect);
            assert!(x >= 0 && y >= 0 && rw >= 1 && rh >= 1);
            assert!(
                x as u32 + rw <= w && y as u32 + rh <= h,
                "{w}x{h} at {aspect}"
            );
        }
    }
}

fn gpu() -> Option<Gpu> {
    if std::env::var_os("CLAW_GPU_TESTS").is_none() {
        eprintln!("skipped the GPU part: set CLAW_GPU_TESTS=1 (under the bench lock)");
        return None;
    }
    Some(Gpu::headless().expect("a GPU"))
}

/// Draws the presenter's picture offscreen at `size` and reads it back as
/// RGBA bytes.
fn draw(gpu: &Gpu, presenter: &MoviePresenter, size: (u32, u32)) -> Vec<u8> {
    draw_as(gpu, presenter, size, COLOR_FORMAT)
}

/// [`draw`] into a target of `format`.
fn draw_as(
    gpu: &Gpu,
    presenter: &MoviePresenter,
    size: (u32, u32),
    format: wgpu::TextureFormat,
) -> Vec<u8> {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let row = (size.0 * 4).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * size.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    presenter.encode(&mut encoder, &view, size, FOUR_THREE);
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(size.1),
            },
        },
        wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    buf.map_async(wgpu::MapMode::Read, .., |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let data = buf.get_mapped_range(..).unwrap();
    let mut out = Vec::new();
    for y in 0..size.1 {
        let o = (y * row) as usize;
        out.extend_from_slice(&data[o..o + size.0 as usize * 4]);
    }
    out
}

#[test]
fn an_rgba_movie_picture_is_drawn_centred_on_black() {
    let Some(gpu) = gpu() else { return };
    let mut presenter = MoviePresenter::new(&gpu, COLOR_FORMAT);
    assert!(!presenter.has_picture());
    // An 8x6 picture, orange above and blue below, with padding at the end
    // of each row.
    let stride = 8 * 4 + 12;
    let mut bytes = vec![0u8; stride * 6];
    for y in 0..6 {
        let colour = if y < 3 { ORANGE } else { BLUE };
        for x in 0..8 {
            bytes[y * stride + x * 4..][..4].copy_from_slice(&colour);
        }
    }
    let picture = MoviePicture {
        width: 8,
        height: 6,
        layout: PictureLayout::Rgba,
        planes: [&bytes, &[], &[]],
        strides: [stride, 0, 0],
    };
    presenter.upload(&gpu, &picture).unwrap();
    assert!(presenter.has_picture());
    let size = (64, 24);
    let out = draw(&gpu, &presenter, size);
    let at = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * size.0 + x) * 4) as usize;
        [out[i], out[i + 1], out[i + 2], out[i + 3]]
    };
    // 4:3 in 64x24 is 32x24 at x 16.
    for (x, y) in [(0, 0), (15, 12), (48, 12), (63, 23)] {
        assert_eq!(&at(x, y)[..3], &[0, 0, 0], "bar at {x},{y}");
    }
    for (x, y, want) in [
        (16, 0, ORANGE),
        (32, 6, ORANGE),
        (47, 17, BLUE),
        (16, 23, BLUE),
    ] {
        let p = at(x, y);
        for (got, want) in p[..3].iter().zip(want) {
            assert!(got.abs_diff(want) <= 1, "{p:?} at {x},{y}");
        }
    }
    // Pictures with too few bytes are refused, and the last picture stays.
    let short = MoviePicture {
        planes: [&bytes[..100], &[], &[]],
        ..picture
    };
    assert!(matches!(
        presenter.upload(&gpu, &short),
        Err(MovieDrawError::BadPicture(_))
    ));
    assert!(presenter.has_picture());
}

/// Rounds a 0 to 1 colour to bytes.
fn bytes(rgb: [f32; 3]) -> [u8; 3] {
    rgb.map(|c| (c * 255.0).round() as u8)
}

/// The colour of one Y, U, V sample.
fn colour(yuv: [u8; 3], bt709: bool, full_range: bool) -> [u8; 3] {
    let planes = [[yuv[0]], [yuv[1]], [yuv[2]]];
    let picture = MoviePicture {
        width: 1,
        height: 1,
        layout: PictureLayout::I420 { bt709, full_range },
        planes: [&planes[0], &planes[1], &planes[2]],
        strides: [1, 1, 1],
    };
    bytes(movie_pixel(&picture, 0.5, 0.5))
}

#[test]
fn limited_range_yuv_has_black_at_16_and_white_at_235() {
    for bt709 in [false, true] {
        assert_eq!(colour([16, 128, 128], bt709, false), [0, 0, 0]);
        assert_eq!(colour([235, 128, 128], bt709, false), [255, 255, 255]);
        assert_eq!(colour([126, 128, 128], bt709, false), [128, 128, 128]);
        // Below black and above white clamp.
        assert_eq!(colour([4, 128, 128], bt709, false), [0, 0, 0]);
        assert_eq!(colour([250, 128, 128], bt709, false), [255, 255, 255]);
    }
}

#[test]
fn full_range_yuv_uses_every_value() {
    for bt709 in [false, true] {
        assert_eq!(colour([0, 128, 128], bt709, true), [0, 0, 0]);
        assert_eq!(colour([255, 128, 128], bt709, true), [255, 255, 255]);
        assert_eq!(colour([128, 128, 128], bt709, true), [128, 128, 128]);
    }
}

#[test]
fn primaries_come_out_of_each_matrix() {
    let near = |got: [u8; 3], want: [u8; 3]| got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 2);
    // The standard Y'CbCr values of pure red, green and blue.
    let bt601 = [
        ([81, 90, 240], [255, 0, 0]),
        ([145, 54, 34], [0, 255, 0]),
        ([41, 240, 110], [0, 0, 255]),
    ];
    for (yuv, rgb) in bt601 {
        let got = colour(yuv, false, false);
        assert!(near(got, rgb), "BT.601 {yuv:?} gave {got:?}");
    }
    let bt709 = [
        ([63, 102, 240], [255, 0, 0]),
        ([173, 42, 26], [0, 255, 0]),
        ([32, 240, 118], [0, 0, 255]),
    ];
    for (yuv, rgb) in bt709 {
        let got = colour(yuv, true, false);
        assert!(near(got, rgb), "BT.709 {yuv:?} gave {got:?}");
    }
    // The two matrices differ.
    assert_ne!(
        colour([81, 90, 240], true, false),
        colour([81, 90, 240], false, false)
    );
    let m = yuv_to_rgb(false, false);
    assert!((m.offset[0] - 16.0 / 255.0).abs() < 1e-6);
}

/// An I420 picture of `w` x `h`: smooth gradients in each plane, so a
/// bilinear sample anywhere is well defined, with a block of sharp
/// chroma squares at the top left.
struct I420 {
    w: u32,
    h: u32,
    planes: [Vec<u8>; 3],
    strides: [usize; 3],
}

impl I420 {
    fn new(w: u32, h: u32) -> I420 {
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        // Padding at the end of each row.
        let strides = [w as usize + 7, cw as usize + 3, cw as usize + 5];
        let mut planes = [
            vec![0u8; strides[0] * h as usize],
            vec![0u8; strides[1] * ch as usize],
            vec![0u8; strides[2] * ch as usize],
        ];
        for y in 0..h {
            for x in 0..w {
                planes[0][y as usize * strides[0] + x as usize] =
                    (16 + 219 * (x + y) / (w + h)) as u8;
            }
        }
        for y in 0..ch {
            for x in 0..cw {
                let (mut u, mut v) = (16 + 224 * x / cw, 240 - 224 * y / ch);
                if x < cw / 8 && y < ch / 8 {
                    let on = (x / 4 + y / 4) % 2 == 0;
                    (u, v) = if on { (60, 200) } else { (200, 60) };
                }
                planes[1][y as usize * strides[1] + x as usize] = u as u8;
                planes[2][y as usize * strides[2] + x as usize] = v as u8;
            }
        }
        I420 {
            w,
            h,
            planes,
            strides,
        }
    }

    fn picture(&self, bt709: bool, full_range: bool) -> MoviePicture<'_> {
        MoviePicture {
            width: self.w,
            height: self.h,
            layout: PictureLayout::I420 { bt709, full_range },
            planes: [&self.planes[0], &self.planes[1], &self.planes[2]],
            strides: self.strides,
        }
    }
}

/// The largest difference between two RGBA images, and where.
fn worst(a: &[u8], b: &[u8], width: u32) -> (u8, (u32, u32)) {
    assert_eq!(a.len(), b.len());
    let mut out = (0, (0, 0));
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        let d = x.abs_diff(*y);
        if d > out.0 {
            let p = (i / 4) as u32;
            out = (d, (p % width, p / width));
        }
    }
    out
}

/// The largest difference between `got` and the reference `want`, and
/// where, leaving out pixels next to a sharp edge of the reference (a step
/// of more than 8 to a neighbour). There the GPU's filter weights, which
/// have 8 fractional bits, may put a scaled picture off by 2 or 3; those
/// pixels are held to 3 instead.
fn worst_smooth(got: &[u8], want: &[u8], size: (u32, u32)) -> (u8, (u32, u32)) {
    let (w, h) = size;
    let at = |x: u32, y: u32, c: usize| want[((y * w + x) * 4) as usize + c];
    let mut out = (0, (0, 0));
    for y in 0..h {
        for x in 0..w {
            let sharp = (0..3).any(|c| {
                let v = at(x, y, c);
                [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ]
                .into_iter()
                .filter(|&(nx, ny)| nx < w && ny < h)
                .any(|(nx, ny)| at(nx, ny, c).abs_diff(v) > 8)
            });
            for c in 0..3 {
                let i = ((y * w + x) * 4) as usize + c;
                let d = got[i].abs_diff(want[i]);
                assert!(d <= 3, "off by {d} at {x},{y}");
                if !sharp && d > out.0 {
                    out = (d, (x, y));
                }
            }
        }
    }
    out
}

/// Every pixel of columns `xs` is black.
fn black_columns(out: &[u8], size: (u32, u32), xs: std::ops::Range<u32>) -> bool {
    (0..size.1).all(|y| {
        xs.clone().all(|x| {
            let i = ((y * size.0 + x) * 4) as usize;
            out[i..i + 3] == [0, 0, 0]
        })
    })
}

/// Some pixel of column `x` is not black.
fn lit_column(out: &[u8], size: (u32, u32), x: u32) -> bool {
    (0..size.1).any(|y| {
        let i = ((y * size.0 + x) * 4) as usize;
        out[i..i + 3] != [0, 0, 0]
    })
}

#[test]
fn an_i420_movie_picture_matches_the_cpu_reference_with_its_bars() {
    let Some(gpu) = gpu() else { return };
    let mut presenter = MoviePresenter::new(&gpu, COLOR_FORMAT);
    // 1440x1080 fills the 4:3 part of 1920x1080 one to one.
    let movie = I420::new(1440, 1080);
    for (bt709, full_range) in [(false, false), (true, false), (false, true), (true, true)] {
        let picture = movie.picture(bt709, full_range);
        presenter.upload(&gpu, &picture).unwrap();
        for size in [(1920, 1080), (3840, 1600)] {
            let out = draw(&gpu, &presenter, size);
            let want = movie_frame(&picture, size, FOUR_THREE);
            // One to one (1920x1080) every pixel is within 1; scaled
            // (3840x1600) the pixels at the sharp squares are within 3.
            let (diff, at) = if size == (1920, 1080) {
                worst(&out, &want, size.0)
            } else {
                worst_smooth(&out, &want, size)
            };
            assert!(
                diff <= 1,
                "{size:?} BT.709 {bt709} full {full_range}: off by {diff} at {at:?}"
            );
            let (x, _, w, _) = movie_rect(size, FOUR_THREE);
            let (x, w) = (x as u32, w);
            assert!(black_columns(&out, size, 0..x), "left bar at {size:?}");
            assert!(
                black_columns(&out, size, x + w..size.0),
                "right bar at {size:?}"
            );
            assert!(lit_column(&out, size, x) && lit_column(&out, size, x + w - 1));
        }
    }
}

#[test]
fn an_i420_picture_on_an_srgb_target_matches_the_reference() {
    let Some(gpu) = gpu() else { return };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut presenter = MoviePresenter::new(&gpu, format);
    let movie = I420::new(63, 47);
    let picture = movie.picture(true, false);
    presenter.upload(&gpu, &picture).unwrap();
    let size = (200, 120);
    let out = draw_as(&gpu, &presenter, size, format);
    let want = movie_frame(&picture, size, FOUR_THREE);
    let (diff, at) = worst(&out, &want, size.0);
    assert!(diff <= 1, "off by {diff} at {at:?}");
}

#[test]
fn the_presenter_switches_between_layouts_and_refuses_short_planes() {
    let Some(gpu) = gpu() else { return };
    let mut presenter = MoviePresenter::new(&gpu, COLOR_FORMAT);
    let size = (64, 48);
    let movie = I420::new(64, 48);
    let yuv = movie.picture(false, false);
    presenter.upload(&gpu, &yuv).unwrap();
    let (diff, _) = worst(
        &draw(&gpu, &presenter, size),
        &movie_frame(&yuv, size, FOUR_THREE),
        size.0,
    );
    assert!(diff <= 1);
    // An RGBA picture replaces it.
    let rgba: Vec<u8> = ORANGE.repeat(8 * 6);
    let flat = MoviePicture {
        width: 8,
        height: 6,
        layout: PictureLayout::Rgba,
        planes: [&rgba, &[], &[]],
        strides: [32, 0, 0],
    };
    presenter.upload(&gpu, &flat).unwrap();
    let out = draw(&gpu, &presenter, size);
    assert_eq!(&out[..3], &ORANGE[..3]);
    // And the YUV one again.
    presenter.upload(&gpu, &yuv).unwrap();
    let (diff, _) = worst(
        &draw(&gpu, &presenter, size),
        &movie_frame(&yuv, size, FOUR_THREE),
        size.0,
    );
    assert!(diff <= 1);
    // A chroma plane one row short is refused; the picture stays.
    let short = MoviePicture {
        planes: [
            yuv.planes[0],
            &yuv.planes[1][..yuv.planes[1].len() - yuv.strides[1]],
            yuv.planes[2],
        ],
        ..yuv
    };
    assert!(matches!(
        presenter.upload(&gpu, &short),
        Err(MovieDrawError::BadPicture(_))
    ));
    let narrow = MoviePicture {
        strides: [yuv.strides[0], 10, yuv.strides[2]],
        ..yuv
    };
    assert!(matches!(
        presenter.upload(&gpu, &narrow),
        Err(MovieDrawError::BadPicture(_))
    ));
    assert!(presenter.has_picture());
}
