//! Movie pictures on the GPU: uploaded each time a new one is due and drawn
//! centred at the movie's display aspect, on black, with bilinear
//! filtering.
//!
//! The renderer knows nothing of the decoder: a [`MoviePicture`] borrows the
//! planes of whatever the player decoded.
//!
//! - RGBA pictures are drawn as they are (sRGB values).
//! - I420 pictures go up as three one-byte planes (Y at full size, U and V
//!   at half size), each sampled bilinearly, and the shader turns Y, U and
//!   V into colour with the picture's matrix (BT.601 or BT.709) and range
//!   (limited or full), from [`yuv_to_rgb`]'s coefficients.
//!
//! Colours are sRGB values, converted for an sRGB target as the game's own
//! final pass does. [`crate::cpu::movie_frame`] computes the same pixels on
//! the CPU.

use bytemuck::{Pod, Zeroable};

use crate::gpu::Gpu;

const SHADER: &str = include_str!("shaders/movie.wgsl");

/// How a picture's planes are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureLayout {
    /// One plane of red, green, blue and alpha bytes per pixel, in sRGB.
    Rgba,
    /// Three 8-bit planes, Y at full size and U and V at half width and
    /// height.
    I420 {
        /// The BT.709 matrix (otherwise BT.601).
        bt709: bool,
        /// Full range (otherwise limited range).
        full_range: bool,
    },
}

/// A decoded picture, borrowed from the player.
#[derive(Debug, Clone, Copy)]
pub struct MoviePicture<'a> {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// How the planes are laid out.
    pub layout: PictureLayout,
    /// The planes; [`PictureLayout::Rgba`] uses the first only.
    pub planes: [&'a [u8]; 3],
    /// Bytes from one row to the next in each plane.
    pub strides: [usize; 3],
}

/// Why a picture could not be uploaded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MovieDrawError {
    /// The layout is not supported (kept for callers matching on it; every
    /// layout is drawn now).
    #[error("movie pictures laid out as {0} cannot be drawn")]
    Unsupported(String),
    /// The planes do not hold the picture they claim to.
    #[error("bad movie picture: {0}")]
    BadPicture(String),
}

/// Where a picture of `aspect` (width over height) shows in an output of
/// `output` pixels: the largest rectangle of that shape that fits, centred,
/// as (x, y, width, height). An aspect that is not a positive number fills
/// the output.
pub fn movie_rect(output: (u32, u32), aspect: f32) -> (i32, i32, u32, u32) {
    let (ow, oh) = output;
    if ow == 0 || oh == 0 {
        return (0, 0, 0, 0);
    }
    if !(aspect.is_finite() && aspect > 0.0) {
        return (0, 0, ow, oh);
    }
    let (fw, fh) = (f64::from(ow), f64::from(oh));
    let a = f64::from(aspect);
    let (w, h) = if fw / fh > a {
        (((fh * a).round() as u32).clamp(1, ow), oh)
    } else {
        (ow, ((fw / a).round() as u32).clamp(1, oh))
    };
    (((ow - w) / 2) as i32, ((oh - h) / 2) as i32, w, h)
}

/// The conversion from Y, U and V (each 0 to 1, a byte over 255) to
/// sRGB red, green and blue (0 to 1, before clamping): subtract
/// `offset`, then multiply by `rows`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YuvToRgb {
    /// Subtracted from Y, U and V first.
    pub offset: [f32; 3],
    /// Red, green and blue, each a row applied to the offset Y, U, V.
    pub rows: [[f32; 3]; 3],
}

/// The conversion for a picture of the BT.709 matrix (otherwise BT.601)
/// and of full range (otherwise limited: Y from 16 to 235, U and V from 16
/// to 240).
pub fn yuv_to_rgb(bt709: bool, full_range: bool) -> YuvToRgb {
    let (kr, kb) = if bt709 {
        (0.2126f64, 0.0722f64)
    } else {
        (0.299, 0.114)
    };
    let kg = 1.0 - kr - kb;
    // Y and the colour differences from the stored values.
    let (y_scale, c_scale, y_offset) = if full_range {
        (1.0, 1.0, 0.0)
    } else {
        (255.0 / 219.0, 255.0 / 224.0, 16.0 / 255.0)
    };
    let c_offset = 128.0 / 255.0;
    let rv = 2.0 * (1.0 - kr) * c_scale;
    let bu = 2.0 * (1.0 - kb) * c_scale;
    let gu = -bu * kb / kg;
    let gv = -rv * kr / kg;
    YuvToRgb {
        offset: [y_offset as f32, c_offset as f32, c_offset as f32],
        rows: [
            [y_scale as f32, 0.0, rv as f32],
            [y_scale as f32, gu as f32, gv as f32],
            [y_scale as f32, bu as f32, 0.0],
        ],
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct MovieU {
    srgb: u32,
    pad: [u32; 3],
    offset: [f32; 4],
    rows: [[f32; 4]; 3],
}

/// The bind group layouts of the two kinds of picture.
#[derive(Debug)]
struct Layouts {
    rgba: wgpu::BindGroupLayout,
    i420: wgpu::BindGroupLayout,
}

/// The kind of picture on the GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Rgba,
    I420,
}

/// The picture's textures (one for RGBA, Y, U and V for I420) and their
/// bind group.
#[derive(Debug)]
struct Uploaded {
    kind: Kind,
    textures: Vec<wgpu::Texture>,
    bind: wgpu::BindGroup,
    size: (u32, u32),
}

/// The plane sizes of a `w` x `h` I420 picture.
fn i420_sizes(w: u32, h: u32) -> [(u32, u32); 3] {
    let c = (w.div_ceil(2), h.div_ceil(2));
    [(w, h), c, c]
}

/// Draws movie pictures (see the module documentation).
#[derive(Debug)]
pub struct MoviePresenter {
    layouts: Layouts,
    format: wgpu::TextureFormat,
    rgba: wgpu::RenderPipeline,
    i420: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    picture: Option<Uploaded>,
}

impl MoviePresenter {
    /// A presenter drawing into targets of `format`.
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> MoviePresenter {
        let device = &gpu.device;
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let uniform = entry(
            0,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        );
        let sampler_entry = entry(
            2,
            wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        );
        let texture = |binding| {
            entry(
                binding,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            )
        };
        // Bindings: 0 the uniform, 2 the sampler, 1 the RGBA picture, 3 to
        // 5 the Y, U and V planes.
        let layouts = Layouts {
            rgba: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("movie rgba"),
                entries: &[uniform, texture(1), sampler_entry],
            }),
            i420: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("movie i420"),
                entries: &[uniform, sampler_entry, texture(3), texture(4), texture(5)],
            }),
        };
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("movie"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = |layout: &wgpu::BindGroupLayout, fragment: &str| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("movie"),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("movie"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_movie"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let rgba = pipeline(&layouts.rgba, "fs_rgba");
        let i420 = pipeline(&layouts.i420, "fs_i420");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("movie"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("movie"),
            size: std::mem::size_of::<MovieU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let presenter = MoviePresenter {
            layouts,
            format,
            rgba,
            i420,
            sampler,
            uniform,
            picture: None,
        };
        presenter.write_uniform(gpu, yuv_to_rgb(false, false));
        presenter
    }

    fn write_uniform(&self, gpu: &Gpu, yuv: YuvToRgb) {
        let [o0, o1, o2] = yuv.offset;
        let row = |r: [f32; 3]| [r[0], r[1], r[2], 0.0];
        gpu.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&MovieU {
                srgb: u32::from(self.format.is_srgb()),
                pad: [0; 3],
                offset: [o0, o1, o2, 0.0],
                rows: yuv.rows.map(row),
            }),
        );
    }

    /// Whether a picture was uploaded.
    pub fn has_picture(&self) -> bool {
        self.picture.is_some()
    }

    /// Uploads `picture`, which shows from the next [`MoviePresenter::render`]
    /// on. The previous picture stays when this fails.
    pub fn upload(&mut self, gpu: &Gpu, picture: &MoviePicture) -> Result<(), MovieDrawError> {
        let (w, h) = (picture.width, picture.height);
        let max = gpu.max_target();
        if w == 0 || h == 0 || w > max || h > max {
            return Err(MovieDrawError::BadPicture(format!("size {w}x{h}")));
        }
        let (kind, planes) = match picture.layout {
            PictureLayout::Rgba => (Kind::Rgba, vec![(w * 4, h)]),
            PictureLayout::I420 { .. } => (Kind::I420, i420_sizes(w, h).to_vec()),
        };
        // Check every plane before touching the textures.
        let mut needed = Vec::with_capacity(planes.len());
        for (i, &(row, rows)) in planes.iter().enumerate() {
            let (row, rows) = (row as usize, rows as usize);
            let stride = picture.strides[i];
            let bytes = stride * (rows - 1) + row;
            if stride < row || picture.planes[i].len() < bytes || stride > u32::MAX as usize {
                return Err(MovieDrawError::BadPicture(format!(
                    "plane {i} has {} bytes with rows of {stride} for {w}x{h}",
                    picture.planes[i].len()
                )));
            }
            needed.push(bytes);
        }
        if self
            .picture
            .as_ref()
            .is_none_or(|p| p.size != (w, h) || p.kind != kind)
        {
            self.picture = Some(self.make_textures(gpu, kind, (w, h)));
        }
        if let PictureLayout::I420 { bt709, full_range } = picture.layout {
            self.write_uniform(gpu, yuv_to_rgb(bt709, full_range));
        }
        let Some(up) = &self.picture else {
            return Ok(());
        };
        let sizes: Vec<(u32, u32)> = match kind {
            Kind::Rgba => vec![(w, h)],
            Kind::I420 => i420_sizes(w, h).to_vec(),
        };
        for (i, texture) in up.textures.iter().enumerate() {
            let (tw, th) = sizes[i];
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &picture.planes[i][..needed[i]],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(picture.strides[i] as u32),
                    rows_per_image: Some(th),
                },
                wgpu::Extent3d {
                    width: tw,
                    height: th,
                    depth_or_array_layers: 1,
                },
            );
        }
        Ok(())
    }

    fn make_textures(&self, gpu: &Gpu, kind: Kind, size: (u32, u32)) -> Uploaded {
        let texture = |size: (u32, u32), format| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("movie picture"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let (textures, layout, first) = match kind {
            Kind::Rgba => (
                vec![texture(size, wgpu::TextureFormat::Rgba8Unorm)],
                &self.layouts.rgba,
                1,
            ),
            Kind::I420 => (
                i420_sizes(size.0, size.1)
                    .iter()
                    .map(|&s| texture(s, wgpu::TextureFormat::R8Unorm))
                    .collect(),
                &self.layouts.i420,
                3,
            ),
        };
        let views: Vec<wgpu::TextureView> = textures
            .iter()
            .map(|t| t.create_view(&Default::default()))
            .collect();
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: self.uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            },
        ];
        for (i, view) in views.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: first + i as u32,
                resource: wgpu::BindingResource::TextureView(view),
            });
        }
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("movie"),
            layout,
            entries: &entries,
        });
        Uploaded {
            kind,
            textures,
            bind,
            size,
        }
    }

    /// Records the last uploaded picture into `encoder`: `target`, of
    /// `output` pixels, is cleared to black and the picture drawn at
    /// [`movie_rect`] for `aspect`. Without a picture the target is only
    /// cleared.
    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        output: (u32, u32),
        aspect: f32,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("movie"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        let Some(up) = &self.picture else { return };
        let (x, y, w, h) = movie_rect(output, aspect);
        if w == 0 || h == 0 {
            return;
        }
        pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
        pass.set_pipeline(match up.kind {
            Kind::Rgba => &self.rgba,
            Kind::I420 => &self.i420,
        });
        pass.set_bind_group(0, &up.bind, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Draws the last uploaded picture into `target` and submits it (see
    /// [`MoviePresenter::encode`]).
    pub fn render(&self, gpu: &Gpu, target: &wgpu::TextureView, output: (u32, u32), aspect: f32) {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        self.encode(&mut encoder, target, output, aspect);
        gpu.queue.submit([encoder.finish()]);
    }
}
