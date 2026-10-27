//! Movie pictures: where they show at each output shape, and (with
//! `CLAW_GPU_TESTS=1`, under the bench lock) an RGBA picture drawn
//! offscreen.

use claw_render::gpu::COLOR_FORMAT;
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
        format: COLOR_FORMAT,
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
    // YUV pictures and pictures with too few bytes are refused, and the
    // last picture stays.
    let yuv = MoviePicture {
        layout: PictureLayout::I420 {
            bt709: false,
            full_range: false,
        },
        ..picture
    };
    assert!(matches!(
        presenter.upload(&gpu, &yuv),
        Err(MovieDrawError::Unsupported(_))
    ));
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
