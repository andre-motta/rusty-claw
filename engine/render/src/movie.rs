//! Movie pictures on the GPU: uploaded each time a new one is due and drawn
//! centred at the movie's display aspect, on black, with bilinear
//! filtering.
//!
//! The renderer knows nothing of the decoder: a [`MoviePicture`] borrows the
//! planes of whatever the player decoded. RGBA pictures are drawn as they
//! are (sRGB values, converted for an sRGB target as the game's own final
//! pass does); three-plane YUV pictures are not supported yet
//! ([`MovieDrawError::Unsupported`]).

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
    /// The layout is not supported.
    #[error("movie pictures laid out as {0} cannot be drawn yet")]
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct MovieU {
    srgb: u32,
    pad: [u32; 3],
}

/// The picture texture and its bind group.
#[derive(Debug)]
struct Uploaded {
    texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    size: (u32, u32),
}

/// Draws movie pictures (see the module documentation).
#[derive(Debug)]
pub struct MoviePresenter {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
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
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("movie"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(
                    1,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    2,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("movie"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("movie"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                entry_point: Some("fs_rgba"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
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
        gpu.queue.write_buffer(
            &uniform,
            0,
            bytemuck::bytes_of(&MovieU {
                srgb: u32::from(format.is_srgb()),
                pad: [0; 3],
            }),
        );
        MoviePresenter {
            layout,
            pipeline,
            sampler,
            uniform,
            picture: None,
        }
    }

    /// Whether a picture was uploaded.
    pub fn has_picture(&self) -> bool {
        self.picture.is_some()
    }

    /// Uploads `picture`, which shows from the next [`MoviePresenter::render`]
    /// on. The previous picture stays when this fails.
    pub fn upload(&mut self, gpu: &Gpu, picture: &MoviePicture) -> Result<(), MovieDrawError> {
        if let PictureLayout::I420 { .. } = picture.layout {
            return Err(MovieDrawError::Unsupported("I420".into()));
        }
        let (w, h) = (picture.width, picture.height);
        let max = gpu.max_target();
        if w == 0 || h == 0 || w > max || h > max {
            return Err(MovieDrawError::BadPicture(format!("size {w}x{h}")));
        }
        let row = w as usize * 4;
        let stride = picture.strides[0];
        let needed = stride * (h as usize - 1) + row;
        if stride < row || picture.planes[0].len() < needed {
            return Err(MovieDrawError::BadPicture(format!(
                "{} bytes with rows of {stride} for {w}x{h}",
                picture.planes[0].len()
            )));
        }
        if self.picture.as_ref().is_none_or(|p| p.size != (w, h)) {
            self.picture = Some(self.make_texture(gpu, (w, h)));
        }
        let Some(up) = &self.picture else {
            return Ok(());
        };
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &up.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &picture.planes[0][..needed],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride as u32),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }

    fn make_texture(&self, gpu: &Gpu, size: (u32, u32)) -> Uploaded {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("movie picture"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("movie"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Uploaded {
            texture,
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
        pass.set_pipeline(&self.pipeline);
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
