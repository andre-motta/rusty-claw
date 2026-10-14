//! The GPU renderer (Vulkan through wgpu).
//!
//! A frame is drawn in two stages:
//!
//! 1. Composition into two targets of the composition size: an 8-bit
//!    palette-index target and an RGBA layer for pack replacements. Planes
//!    are full-target triangles that walk their tile grid per pixel; sprites
//!    are quads. Draw modes that read the destination index (ghost, shadow,
//!    blend) first copy the area under the sprite to a scratch texture and
//!    read it there, because fixed-function blending cannot do a table
//!    lookup; each such sprite gets its own copy, so overlapping ones stay
//!    exact.
//! 2. The final pass turns indices into colours through the 256-entry
//!    palette texture (or takes the replacement colour) and scales the image
//!    to the output: whole-factor scaling is already done by the composition
//!    size, the rest is a bilinear step (sharp bilinear), or nothing for
//!    integer scaling.
//!
//! Only the Vulkan backend is used; there is no fallback.

use std::collections::HashMap;
use std::ops::Range;

use bytemuck::{Pod, Zeroable};

use crate::cpu::Composed;
use crate::images::{ImageRecord, LevelGraphics};
use crate::scene::{Item, PlaneParams, Scene, SpriteParams};

const COMPOSE: &str = include_str!("shaders/compose.wgsl");
const PRESENT: &str = include_str!("shaders/present.wgsl");

/// Format of the palette-index target.
pub const INDEX_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Uint;
/// Format of the replacement layer and of offscreen output.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Errors setting up the GPU.
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    /// No Vulkan adapter was found.
    #[error("no Vulkan GPU found: {0}")]
    Adapter(String),
    /// The device could not be created.
    #[error("cannot open the GPU: {0}")]
    Device(String),
    /// An output image larger than the device can create.
    #[error(
        "output size {width}x{height} is larger than this GPU allows ({max}x{max} at most{extra})"
    )]
    OutputTooLarge {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
        /// Largest width or height the device takes.
        max: u32,
        /// Further detail, empty or starting with a separator.
        extra: String,
    },
}

/// An opened Vulkan device.
#[derive(Debug, Clone)]
pub struct Gpu {
    /// The instance.
    pub instance: wgpu::Instance,
    /// The adapter.
    pub adapter: wgpu::Adapter,
    /// The device.
    pub device: wgpu::Device,
    /// The queue.
    pub queue: wgpu::Queue,
}

impl Gpu {
    /// A Vulkan-only instance.
    pub fn instance() -> wgpu::Instance {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        desc.backends = wgpu::Backends::VULKAN;
        wgpu::Instance::new(desc)
    }

    /// Opens the best Vulkan adapter of `instance`, compatible with
    /// `surface` when one is given.
    pub fn new(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Gpu, GpuError> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface,
            ..Default::default()
        }))
        .map_err(|e| GpuError::Adapter(e.to_string()))?;
        let have = adapter.limits();
        let mut limits = wgpu::Limits::default().using_resolution(have.clone());
        limits.max_storage_buffer_binding_size = have.max_storage_buffer_binding_size.min(1 << 30);
        limits.max_buffer_size = have.max_buffer_size.min(1 << 30);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rusty-claw"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            ..Default::default()
        }))
        .map_err(|e| GpuError::Device(e.to_string()))?;
        Ok(Gpu {
            instance,
            adapter,
            device,
            queue,
        })
    }

    /// Opens a device with no surface, for offscreen drawing.
    pub fn headless() -> Result<Gpu, GpuError> {
        Gpu::new(Gpu::instance(), None)
    }

    /// Adapter name and driver, for reports.
    pub fn describe(&self) -> String {
        let i = self.adapter.get_info();
        format!("{} ({:?}, {})", i.name, i.backend, i.driver)
    }

    /// Largest composition target width or height this device takes.
    pub fn max_target(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    /// Checks that an output image of `size` can be created on this device,
    /// and, with `read_back`, also copied back to memory in one piece.
    /// Drawing into a larger one would fail inside the GPU layer instead of
    /// returning an error.
    pub fn check_output(&self, size: (u32, u32), read_back: bool) -> Result<(), GpuError> {
        let limits = self.device.limits();
        check_output_size(
            size,
            limits.max_texture_dimension_2d,
            read_back.then_some(limits.max_buffer_size),
        )
    }
}

/// [`Gpu::check_output`] for given limits: the largest texture width or
/// height, and the largest buffer when the image is read back as RGBA.
pub fn check_output_size(
    size: (u32, u32),
    max_dimension: u32,
    max_buffer: Option<u64>,
) -> Result<(), GpuError> {
    let (width, height) = size;
    let too_large = |extra: String| GpuError::OutputTooLarge {
        width,
        height,
        max: max_dimension,
        extra,
    };
    if width > max_dimension || height > max_dimension {
        return Err(too_large(String::new()));
    }
    if let Some(max_buffer) = max_buffer {
        let row = (u64::from(width) * 4).div_ceil(256) * 256;
        let bytes = row * u64::from(height);
        if bytes > max_buffer {
            return Err(too_large(format!(
                ", and {max_buffer} bytes to read back; this needs {bytes}"
            )));
        }
    }
    Ok(())
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct FrameU {
    target_size: [f32; 2],
    pad: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PresentU {
    rect: [f32; 4],
    ratio: f32,
    brightness: f32,
    srgb: u32,
    pad: u32,
    target_size: [i32; 2],
    pad2: [i32; 2],
}

/// A growable storage buffer.
#[derive(Debug)]
struct Storage {
    label: &'static str,
    buffer: wgpu::Buffer,
    capacity: u64,
    /// Bytes already uploaded from the CPU copy (for append-only data).
    uploaded: u64,
}

impl Storage {
    fn new(device: &wgpu::Device, label: &'static str, size: u64) -> Storage {
        let capacity = size.max(256).next_power_of_two();
        Storage {
            label,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: capacity,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
            uploaded: 0,
        }
    }

    /// Makes room for `size` bytes; true when the buffer was replaced.
    fn reserve(&mut self, device: &wgpu::Device, size: u64) -> bool {
        if size <= self.capacity {
            return false;
        }
        *self = Storage::new(device, self.label, size);
        true
    }

    /// Uploads the part of `data` not uploaded yet (append-only data);
    /// returns true when the buffer was replaced.
    fn sync(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) -> bool {
        let len = data.len() as u64;
        let replaced = self.reserve(device, len);
        if replaced || len < self.uploaded {
            self.uploaded = 0;
        }
        let from = (self.uploaded as usize) & !3;
        if from < data.len() {
            let end = data.len() & !3;
            if end > from {
                queue.write_buffer(&self.buffer, from as u64, &data[from..end]);
            }
            self.uploaded = end as u64;
        }
        replaced
    }

    /// Uploads `data` from the start (per-frame data); returns true when the
    /// buffer was replaced.
    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) -> bool {
        let replaced = self.reserve(device, data.len() as u64);
        let end = data.len() & !3;
        if end > 0 {
            queue.write_buffer(&self.buffer, 0, &data[..end]);
        }
        replaced
    }
}

#[derive(Debug)]
struct Targets {
    size: (u32, u32),
    index: wgpu::Texture,
    index_view: wgpu::TextureView,
    hd: wgpu::Texture,
    hd_view: wgpu::TextureView,
    scratch: wgpu::Texture,
    scratch_view: wgpu::TextureView,
}

/// The renderer: pipelines, the level's data on the GPU and the targets.
#[derive(Debug)]
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    compose_bgl: wgpu::BindGroupLayout,
    present_bgl: wgpu::BindGroupLayout,
    plane_pipeline: wgpu::RenderPipeline,
    sprite_pipeline: wgpu::RenderPipeline,
    present_module: wgpu::ShaderModule,
    present_layout: wgpu::PipelineLayout,
    present_pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    frame_ubuf: wgpu::Buffer,
    present_ubuf: wgpu::Buffer,
    bytes: Storage,
    hd: Storage,
    records: Storage,
    slots: Storage,
    tables: Storage,
    planes: Storage,
    sprites: Storage,
    palette: wgpu::Texture,
    targets: Option<Targets>,
    compose_bg: Option<wgpu::BindGroup>,
    present_bg: Option<wgpu::BindGroup>,
}

fn layout_entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty,
        count: None,
    }
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    layout_entry(
        binding,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
    )
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    layout_entry(
        binding,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
    )
}

fn texture_entry(binding: u32, sample_type: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    layout_entry(
        binding,
        wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
    )
}

fn color_target(format: wgpu::TextureFormat) -> Option<wgpu::ColorTargetState> {
    Some(wgpu::ColorTargetState {
        format,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    })
}

fn pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    entry: (&str, &str),
    targets: &[Option<wgpu::ColorTargetState>],
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(entry.0),
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
            module,
            entry_point: Some(entry.1),
            compilation_options: Default::default(),
            targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn texture(
    device: &wgpu::Device,
    label: &str,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> (wgpu::Texture, wgpu::TextureView) {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    let v = t.create_view(&Default::default());
    (t, v)
}

fn copy_info(texture: &wgpu::Texture, x: u32, y: u32) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin: wgpu::Origin3d { x, y, z: 0 },
        aspect: wgpu::TextureAspect::All,
    }
}

/// A run of items drawn in one render pass, after an optional copy of the
/// index target to the scratch texture (x, y, width, height).
struct Segment {
    copy: Option<(u32, u32, u32, u32)>,
    items: Range<usize>,
}

fn segments(scene: &Scene) -> Vec<Segment> {
    let (tw, th) = (
        i64::from(scene.comp.target.0),
        i64::from(scene.comp.target.1),
    );
    let mut out = vec![Segment {
        copy: None,
        items: 0..0,
    }];
    for (i, item) in scene.items.iter().enumerate() {
        if let Item::Sprite(s) = item {
            let sp = &scene.sprites[*s as usize];
            if sp.effect().reads_destination() {
                let (x0, y0, x1, y1) = sp.quad();
                let (x0, y0) = (x0.clamp(0, tw), y0.clamp(0, th));
                let (x1, y1) = (x1.clamp(0, tw), y1.clamp(0, th));
                let copy = (x1 > x0 && y1 > y0).then_some((
                    x0 as u32,
                    y0 as u32,
                    (x1 - x0) as u32,
                    (y1 - y0) as u32,
                ));
                out.push(Segment {
                    copy,
                    items: i..i + 1,
                });
                continue;
            }
        }
        if let Some(last) = out.last_mut() {
            last.items.end = i + 1;
        }
    }
    out
}

impl Renderer {
    /// Pipelines and empty buffers on `gpu`.
    pub fn new(gpu: &Gpu) -> Renderer {
        let device = gpu.device.clone();
        let compose_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compose"),
            entries: &[
                uniform_entry(0),
                storage_entry(1),
                storage_entry(2),
                storage_entry(3),
                storage_entry(4),
                storage_entry(5),
                storage_entry(6),
                storage_entry(7),
                texture_entry(8, wgpu::TextureSampleType::Uint),
            ],
        });
        let present_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present"),
            entries: &[
                uniform_entry(0),
                texture_entry(1, wgpu::TextureSampleType::Uint),
                texture_entry(2, wgpu::TextureSampleType::Float { filterable: false }),
                texture_entry(3, wgpu::TextureSampleType::Float { filterable: false }),
            ],
        });
        let compose_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compose"),
            bind_group_layouts: &[Some(&compose_bgl)],
            immediate_size: 0,
        });
        let present_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present"),
            bind_group_layouts: &[Some(&present_bgl)],
            immediate_size: 0,
        });
        let compose = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compose"),
            source: wgpu::ShaderSource::Wgsl(COMPOSE.into()),
        });
        let present_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("present"),
            source: wgpu::ShaderSource::Wgsl(PRESENT.into()),
        });
        let targets = [color_target(INDEX_FORMAT), color_target(COLOR_FORMAT)];
        let plane_pipeline = pipeline(
            &device,
            "planes",
            &compose_layout,
            &compose,
            ("vs_plane", "fs_plane"),
            &targets,
        );
        let sprite_pipeline = pipeline(
            &device,
            "sprites",
            &compose_layout,
            &compose,
            ("vs_sprite", "fs_sprite"),
            &targets,
        );
        let ubuf = |label, size: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let frame_ubuf = ubuf("frame", std::mem::size_of::<FrameU>());
        let present_ubuf = ubuf("present", std::mem::size_of::<PresentU>());
        let (palette, _) = texture(
            &device,
            "palette",
            (256, 1),
            COLOR_FORMAT,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        Renderer {
            bytes: Storage::new(&device, "image bytes", 4),
            hd: Storage::new(&device, "replacement texels", 4),
            records: Storage::new(&device, "image records", 32),
            slots: Storage::new(&device, "plane slots", 4),
            tables: Storage::new(&device, "tables", 4),
            planes: Storage::new(&device, "planes", 48),
            sprites: Storage::new(&device, "sprites", 48),
            queue: gpu.queue.clone(),
            compose_bgl,
            present_bgl,
            plane_pipeline,
            sprite_pipeline,
            present_module,
            present_layout,
            present_pipelines: HashMap::new(),
            frame_ubuf,
            present_ubuf,
            palette,
            targets: None,
            compose_bg: None,
            present_bg: None,
            device,
        }
    }

    /// Uploads a level's slots and tables and every image loaded so far.
    pub fn set_level(&mut self, g: &LevelGraphics) {
        let slots: Vec<i32> = g.plane_slots.iter().flatten().copied().collect();
        self.slots
            .write(&self.device, &self.queue, bytemuck::cast_slice(&slots));
        self.tables
            .write(&self.device, &self.queue, &g.tables.packed());
        self.bytes.uploaded = 0;
        self.hd.uploaded = 0;
        self.records.uploaded = 0;
        self.sync_images(g);
        self.compose_bg = None;
    }

    /// Uploads images loaded since the last upload.
    pub fn sync_images(&mut self, g: &LevelGraphics) {
        let imgs = &g.images;
        let mut replaced = self.bytes.sync(&self.device, &self.queue, &imgs.bytes);
        replaced |= self
            .hd
            .sync(&self.device, &self.queue, bytemuck::cast_slice(&imgs.hd));
        replaced |= self.records.sync(
            &self.device,
            &self.queue,
            bytemuck::cast_slice::<ImageRecord, u8>(&imgs.records),
        );
        if replaced {
            self.compose_bg = None;
        }
    }

    fn ensure_targets(&mut self, size: (u32, u32)) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        use wgpu::TextureUsages as U;
        let attach = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC;
        let (index, index_view) = texture(&self.device, "index", size, INDEX_FORMAT, attach);
        let (hd, hd_view) = texture(&self.device, "replacements", size, COLOR_FORMAT, attach);
        let (scratch, scratch_view) = texture(
            &self.device,
            "scratch",
            size,
            INDEX_FORMAT,
            U::TEXTURE_BINDING | U::COPY_DST,
        );
        self.targets = Some(Targets {
            size,
            index,
            index_view,
            hd,
            hd_view,
            scratch,
            scratch_view,
        });
        self.compose_bg = None;
        self.present_bg = None;
    }

    fn bind_groups(&mut self) {
        let Some(t) = &self.targets else { return };
        if self.compose_bg.is_none() {
            fn buf(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
                wgpu::BindGroupEntry {
                    binding,
                    resource: b.as_entire_binding(),
                }
            }
            self.compose_bg = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compose"),
                layout: &self.compose_bgl,
                entries: &[
                    buf(0, &self.frame_ubuf),
                    buf(1, &self.bytes.buffer),
                    buf(2, &self.hd.buffer),
                    buf(3, &self.records.buffer),
                    buf(4, &self.slots.buffer),
                    buf(5, &self.tables.buffer),
                    buf(6, &self.planes.buffer),
                    buf(7, &self.sprites.buffer),
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: wgpu::BindingResource::TextureView(&t.scratch_view),
                    },
                ],
            }));
        }
        if self.present_bg.is_none() {
            let palette_view = self.palette.create_view(&Default::default());
            self.present_bg = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("present"),
                layout: &self.present_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.present_ubuf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&t.index_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&t.hd_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&palette_view),
                    },
                ],
            }));
        }
    }

    fn present_pipeline(&mut self, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        if let Some(p) = self.present_pipelines.get(&format) {
            return p.clone();
        }
        let p = pipeline(
            &self.device,
            "present",
            &self.present_layout,
            &self.present_module,
            ("vs_full", "fs_present"),
            &[color_target(format)],
        );
        self.present_pipelines.insert(format, p.clone());
        p
    }

    /// Uploads the per-frame data of `scene` and records its composition.
    fn encode_compose(
        &mut self,
        g: &LevelGraphics,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.sync_images(g);
        self.ensure_targets(scene.comp.target);
        let planes: &[PlaneParams] = &scene.planes;
        let sprites: &[SpriteParams] = &scene.sprites;
        if self
            .planes
            .write(&self.device, &self.queue, bytemuck::cast_slice(planes))
            | self
                .sprites
                .write(&self.device, &self.queue, bytemuck::cast_slice(sprites))
        {
            self.compose_bg = None;
        }
        let (tw, th) = scene.comp.target;
        self.queue.write_buffer(
            &self.frame_ubuf,
            0,
            bytemuck::bytes_of(&FrameU {
                target_size: [tw as f32, th as f32],
                pad: [0.0; 2],
            }),
        );
        self.bind_groups();
        let (Some(t), Some(bg)) = (&self.targets, &self.compose_bg) else {
            return;
        };
        for (n, seg) in segments(scene).into_iter().enumerate() {
            if let Some((x, y, w, h)) = seg.copy {
                encoder.copy_texture_to_texture(
                    copy_info(&t.index, x, y),
                    copy_info(&t.scratch, x, y),
                    wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                );
            }
            let load = |clear: wgpu::Color| {
                if n == 0 {
                    wgpu::LoadOp::Clear(clear)
                } else {
                    wgpu::LoadOp::Load
                }
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("compose"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &t.index_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: load(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &t.hd_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: load(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                ..Default::default()
            });
            pass.set_bind_group(0, bg, &[]);
            let items = &scene.items[seg.items.clone()];
            let mut i = 0;
            let mut current: Option<bool> = None;
            while i < items.len() {
                match items[i] {
                    Item::Plane(p) => {
                        if current != Some(false) {
                            pass.set_pipeline(&self.plane_pipeline);
                            current = Some(false);
                        }
                        pass.draw(0..3, p..p + 1);
                        i += 1;
                    }
                    Item::Sprite(first) => {
                        if current != Some(true) {
                            pass.set_pipeline(&self.sprite_pipeline);
                            current = Some(true);
                        }
                        // Consecutive sprites are consecutive in the sprite
                        // list; draw them as one instanced call, which keeps
                        // their order.
                        let mut last = first;
                        while i + 1 < items.len() && items[i + 1] == Item::Sprite(last + 1) {
                            last += 1;
                            i += 1;
                        }
                        pass.draw(0..6, first..last + 1);
                        i += 1;
                    }
                }
            }
        }
    }

    fn write_present(&mut self, scene: &Scene, srgb: bool) {
        let mut pal = [0u8; 1024];
        for (i, c) in scene.palette.0.iter().enumerate() {
            pal[i * 4..i * 4 + 3].copy_from_slice(c);
            pal[i * 4 + 3] = 255;
        }
        self.queue.write_texture(
            copy_info(&self.palette, 0, 0),
            &pal,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1024),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let c = &scene.comp;
        let u = PresentU {
            rect: [c.x as f32, c.y as f32, c.width as f32, c.height as f32],
            ratio: c.ratio,
            brightness: scene.brightness,
            srgb: u32::from(srgb),
            pad: 0,
            target_size: [c.target.0 as i32, c.target.1 as i32],
            pad2: [0; 2],
        };
        self.queue
            .write_buffer(&self.present_ubuf, 0, bytemuck::bytes_of(&u));
    }

    /// Records a whole frame into `encoder`: composition, then the final
    /// pass into `output` (of `format`, sized as the scene's output).
    pub fn encode(
        &mut self,
        g: &LevelGraphics,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) {
        self.encode_compose(g, scene, encoder);
        self.write_present(scene, format.is_srgb());
        let pipeline = self.present_pipeline(format);
        self.bind_groups();
        let Some(bg) = &self.present_bg else { return };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, bg, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Draws a whole frame and submits it.
    pub fn render(
        &mut self,
        g: &LevelGraphics,
        scene: &Scene,
        output: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode(g, scene, &mut encoder, output, format);
        self.queue.submit([encoder.finish()]);
    }

    /// Waits until the GPU has finished all submitted work.
    pub fn wait_idle(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    fn read_texture(
        &self,
        texture: &wgpu::Texture,
        size: (u32, u32),
        bytes_per_pixel: u32,
        encoder: wgpu::CommandEncoder,
    ) -> Vec<u8> {
        let mut encoder = encoder;
        let (w, h) = size;
        let row = (w * bytes_per_pixel).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(row) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            copy_info(texture, 0, 0),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        buf.map_async(wgpu::MapMode::Read, .., |_| {});
        self.wait_idle();
        let mut out = Vec::with_capacity((w * h * bytes_per_pixel) as usize);
        if let Ok(data) = buf.get_mapped_range(..) {
            for y in 0..h {
                let o = (y * row) as usize;
                out.extend_from_slice(&data[o..o + (w * bytes_per_pixel) as usize]);
            }
        }
        buf.unmap();
        out
    }

    /// Composes `scene` and reads both targets back.
    pub fn read_composed(&mut self, g: &LevelGraphics, scene: &Scene) -> Composed {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode_compose(g, scene, &mut encoder);
        self.queue.submit([encoder.finish()]);
        let size = scene.comp.target;
        let Some(t) = &self.targets else {
            return Composed {
                width: 0,
                height: 0,
                index: Vec::new(),
                hd: Vec::new(),
            };
        };
        let index = self.read_texture(
            &t.index,
            size,
            1,
            self.device.create_command_encoder(&Default::default()),
        );
        let hd_bytes = self.read_texture(
            &t.hd,
            size,
            4,
            self.device.create_command_encoder(&Default::default()),
        );
        let hd = hd_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Composed {
            width: size.0,
            height: size.1,
            index,
            hd,
        }
    }

    /// Draws `scene` into an offscreen image of its output size and reads it
    /// back as RGBA bytes.
    pub fn render_offscreen(&mut self, g: &LevelGraphics, scene: &Scene) -> Vec<u8> {
        let size = scene.comp.output;
        let (out, view) = texture(
            &self.device,
            "offscreen",
            size,
            COLOR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode(g, scene, &mut encoder, &view, COLOR_FORMAT);
        self.read_texture(&out, size, 4, encoder)
    }
}

/// An offscreen output texture to draw frames into without a window.
#[derive(Debug)]
pub struct Offscreen {
    /// The texture.
    pub texture: wgpu::Texture,
    /// Its view.
    pub view: wgpu::TextureView,
    /// Its size.
    pub size: (u32, u32),
}

impl Offscreen {
    /// An output of `size` on `gpu`.
    pub fn new(gpu: &Gpu, size: (u32, u32)) -> Offscreen {
        let (texture, view) = texture(
            &gpu.device,
            "output",
            size,
            COLOR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        Offscreen {
            texture,
            view,
            size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_sizes_are_checked_against_the_limits() {
        assert!(check_output_size((16384, 9216), 16384, None).is_ok());
        let err = check_output_size((20000, 10000), 16384, None).unwrap_err();
        assert!(matches!(
            err,
            GpuError::OutputTooLarge {
                width: 20000,
                height: 10000,
                max: 16384,
                ..
            }
        ));
        assert!(err.to_string().contains("20000x10000"), "{err}");
        assert!(check_output_size((1280, 20000), 16384, None).is_err());
        // Read back as RGBA with rows padded to 256 bytes.
        assert!(check_output_size((16384, 16384), 16384, Some(1 << 30)).is_ok());
        let err = check_output_size((16384, 16384), 16384, Some(256 << 20)).unwrap_err();
        assert!(err.to_string().contains("read back"), "{err}");
    }
}
