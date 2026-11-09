// The picture behind the glass, drawn into our own texture because an iced shader cannot read the framebuffer.
// The cover pass and the plates live in different iced pipelines, so the textures sit behind one mutex.
// Only the render thread takes it.

use std::marker::PhantomData;

use iced::advanced::image::Handle;
use iced::widget::shader::{self, Primitive as ShaderPrimitive};
use iced::{Rectangle, mouse};
use parking_lot::Mutex;

use crate::blur::{self, Pyramid};

const COVERS: &str = include_str!("../shaders/covers.wgsl");
const MAX_SIDE: u32 = 256;
const ATLAS_LIMIT: u32 = 2048;

/// One cover rectangle in CSS pixels. `image` indexes [`Scene::images`], or `u32::MAX` for a flat gradient.
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub image: u32,
    pub color0: [f32; 4],
    pub color1: [f32; 4],
    /// 0 poster shade, 1 mosaic shade, 2 no shade.
    pub shade: f32,
    pub alpha: f32,
}

#[derive(Clone, Debug)]
pub struct Image {
    pub key: u64,
    pub width: u32,
    pub height: u32,
    pub pixels: Handle,
}

/// What to blur. `stamp` stays put while the picture does, so the pyramid is not run again.
#[derive(Clone, Debug)]
pub struct Scene {
    pub window: [f32; 2],
    pub ground: [f32; 4],
    pub shade: [f32; 4],
    /// Pivot x, pivot y, radians.
    pub tilt: [f32; 3],
    pub quads: Vec<Quad>,
    pub images: Vec<Image>,
    pub stamp: u64,
}

/// iced shader that only fills the offscreen picture. It does not draw on the window.
pub struct Program<Message> {
    pub scene: Scene,
    _message: PhantomData<Message>,
}

impl<Message> Program<Message> {
    pub fn new(scene: Scene) -> Self {
        Self { scene, _message: PhantomData }
    }
}

impl<Message> shader::Program<Message> for Program<Message> {
    type State = ();
    type Primitive = Pass;

    fn mouse_interaction(&self, _: &(), _: Rectangle, _: mouse::Cursor) -> mouse::Interaction {
        mouse::Interaction::None
    }

    fn draw(&self, _: &(), _: mouse::Cursor, _: Rectangle) -> Self::Primitive {
        Pass { scene: self.scene.clone() }
    }
}

#[derive(Debug)]
pub struct Pass {
    scene: Scene,
}

pub struct Gpu {
    format: wgpu::TextureFormat,
}

impl ShaderPrimitive for Pass {
    type Pipeline = Gpu;

    fn prepare(
        &self,
        pipeline: &mut Gpu,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &Rectangle,
        _: &shader::Viewport,
    ) {
        set_window(self.scene.window);
        let mut stage = lock_stage();
        if stage.gpu.is_none() {
            stage.gpu = Some(Picture::new(device, queue, pipeline.format));
        }
    }

    fn render(&self, _: &Gpu, encoder: &mut wgpu::CommandEncoder, _: &wgpu::TextureView, _: &Rectangle<u32>) {
        let mut stage = lock_stage();
        if stage.levels.is_empty() {
            return;
        }
        let levels = stage.levels.clone();
        let Some(gpu) = stage.gpu.as_mut() else { return };
        gpu.paint(encoder, &self.scene, &levels);
    }
}

impl shader::Pipeline for Gpu {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let mut stage = lock_stage();
        if stage.gpu.is_none() {
            stage.gpu = Some(Picture::new(device, queue, format));
        }
        Self { format }
    }

    fn trim(&mut self) {}
}

/// Whether a glass plate asked for the picture this frame.
pub fn wanted() -> bool {
    !lock_stage().levels.is_empty()
}

/// Whether a glass plate asked for the picture in the frame before the current one. The canvas is drawn before the
/// plates are prepared, so this is what tells it there is a blur to fill.
pub fn was_wanted() -> bool {
    lock_stage().last_live
}

/// Reads the quarter-resolution picture the glass samples, for visual tests.
pub fn read_picture(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Shot> {
    let stage = lock_stage();
    let gpu = stage.gpu.as_ref()?;
    let rgba = gpu.pyramid.read_source(device, queue)?;
    let (width, height) = blur::source_size(stage.window[0] as u32, stage.window[1] as u32);
    Some(Shot { rgba, width, height, bytes: gpu.bytes() })
}

/// A plate asks for the pyramid step of its CSS blur radius. Several plates share the deepest chain.
pub fn claim(level: u32) {
    if level == 0 {
        return;
    }
    let mut stage = lock_stage();
    stage.live = true;
    if !stage.levels.contains(&level) {
        stage.levels.push(level);
    }
}

pub fn set_window(window: [f32; 2]) {
    if window[0] > 1.0 && window[1] > 1.0 {
        lock_stage().window = window;
    }
}

pub fn window() -> [f32; 2] {
    lock_stage().window
}

/// Bumps when the blurred pictures are reallocated. A plate caches its sample group against this.
pub fn output_epoch() -> u64 {
    lock_stage().gpu.as_ref().map(|gpu| gpu.pyramid.output_epoch()).unwrap_or(0)
}

/// Clones the blurred view for `level`, after the picture pass has run this frame.
pub fn blurred(level: u32) -> Option<wgpu::TextureView> {
    let stage = lock_stage();
    let gpu = stage.gpu.as_ref()?;
    let place = stage.levels.iter().position(|item| *item == level)?;
    gpu.pyramid.output(place).cloned()
}

pub fn resident_bytes() -> u64 {
    lock_stage().gpu.as_ref().map(Picture::bytes).unwrap_or(0)
}

/// Drops the offscreen and the pyramid. The next open plate builds them again.
pub fn release() {
    let mut stage = lock_stage();
    stage.gpu = None;
    stage.levels.clear();
    stage.live = false;
    stage.last_live = false;
    stage.idle = 0;
}

/// Called from the plate pipeline once a frame. Two quiet frames after the last glass plate, the textures go.
pub fn trim() {
    let mut stage = lock_stage();
    stage.last_live = stage.live;
    if stage.live {
        stage.live = false;
        stage.levels.clear();
        stage.idle = 0;
        return;
    }
    stage.idle = stage.idle.saturating_add(1);
    if stage.idle == 2
        && let Some(gpu) = stage.gpu.as_mut()
    {
        gpu.release();
        tracing::debug!(bytes = 0, "backdrop textures released");
    }
}

/// Copies an existing scene texture (lumiere, or a pass that already drew into `from`) and blurs it.
pub fn paint_external(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    format: wgpu::TextureFormat,
    from: &wgpu::TextureView,
    stamp: u64,
    window: [f32; 2],
) {
    set_window(window);
    let mut stage = lock_stage();
    let count = stage.levels.len().min(blur::MAX_LEVELS as usize);
    if count == 0 {
        return;
    }
    let mut buffer = [0u32; blur::MAX_LEVELS as usize];
    buffer[..count].copy_from_slice(&stage.levels[..count]);
    let levels = &buffer[..count];
    let gpu = stage.gpu.get_or_insert_with(|| Picture::new(device, queue, format));
    let rebuilt = gpu.pyramid.ensure(device, queue, window[0] as u32, window[1] as u32, deepest(levels));
    if !rebuilt && gpu.stamp == stamp && gpu.levels.as_slice() == levels {
        return;
    }
    gpu.pyramid.copy_into_source(device, encoder, from);
    gpu.pyramid.blur(device, encoder, levels);
    gpu.stamp = stamp;
    gpu.levels.clear();
    gpu.levels.extend_from_slice(levels);
}

/// Lets a shader style draw straight into the quarter-resolution source. The encoder stays with the caller.
pub fn draw_source(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    window: [f32; 2],
    stamp: u64,
    draw: impl FnOnce(&wgpu::TextureView, u32, u32),
) -> bool {
    set_window(window);
    let mut stage = lock_stage();
    if stage.levels.is_empty() || window[0] < 1.0 {
        return false;
    }
    let levels = stage.levels.clone();
    let gpu = stage.gpu.get_or_insert_with(|| Picture::new(device, queue, format));
    let rebuilt = gpu.pyramid.ensure(device, queue, window[0] as u32, window[1] as u32, deepest(&levels));
    if !rebuilt && gpu.stamp == stamp && gpu.levels == levels && gpu.pyramid.output(0).is_some() {
        return false;
    }
    let Some(source) = gpu.pyramid.source().cloned() else { return false };
    let (width, height) = blur::source_size(window[0] as u32, window[1] as u32);
    drop(stage);
    draw(&source, width, height);
    let mut stage = lock_stage();
    if let Some(gpu) = stage.gpu.as_mut() {
        gpu.stamp = stamp;
        gpu.levels = levels;
    }
    true
}

/// Runs the pyramid after [`draw_source`] or any other fill of the source.
pub fn blur_source(encoder: &mut wgpu::CommandEncoder) {
    let mut stage = lock_stage();
    if stage.levels.is_empty() {
        return;
    }
    let levels = stage.levels.clone();
    let Some(gpu) = stage.gpu.as_mut() else { return };
    gpu.pyramid.blur(&gpu.device, encoder, &levels);
}

fn lock_stage() -> parking_lot::MutexGuard<'static, Stage> {
    STAGE.lock()
}

fn deepest(levels: &[u32]) -> u32 {
    levels.iter().copied().max().unwrap_or(1)
}

struct Stage {
    gpu: Option<Picture>,
    levels: Vec<u32>,
    window: [f32; 2],
    live: bool,
    last_live: bool,
    idle: u32,
}

struct Picture {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pyramid: Pyramid,
    covers: CoverGpu,
    atlas_key: u64,
    atlas_bind: Option<wgpu::BindGroup>,
    places: Vec<[f32; 4]>,
    verts: Option<wgpu::Buffer>,
    vert_count: u32,
    vert_cap: u64,
    stamp: u64,
    levels: Vec<u32>,
    format: wgpu::TextureFormat,
}

impl Picture {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            pyramid: Pyramid::new(device, format),
            covers: CoverGpu::new(device, format),
            atlas_key: 0,
            atlas_bind: None,
            places: Vec::new(),
            verts: None,
            vert_count: 0,
            vert_cap: 0,
            stamp: 0,
            levels: Vec::new(),
            format,
        }
    }

    fn bytes(&self) -> u64 {
        self.pyramid.resident_bytes() + self.covers.atlas_bytes
    }

    fn release(&mut self) {
        self.pyramid.release();
        self.atlas_bind = None;
        self.atlas_key = 0;
        self.places.clear();
        self.covers.atlas_bytes = 0;
        self.stamp = 0;
        self.levels.clear();
        self.vert_count = 0;
    }

    fn paint(&mut self, encoder: &mut wgpu::CommandEncoder, scene: &Scene, levels: &[u32]) {
        if scene.window[0] < 1.0 || scene.window[1] < 1.0 || levels.is_empty() {
            return;
        }
        let width = scene.window[0] as u32;
        let height = scene.window[1] as u32;
        let rebuilt = self.pyramid.ensure(&self.device, &self.queue, width, height, deepest(levels));
        if !rebuilt && self.stamp == scene.stamp && self.levels == levels && self.pyramid.output(0).is_some() {
            return;
        }
        self.sync_atlas(scene);
        self.write_frame(scene);
        self.write_verts(scene);
        self.draw_covers(encoder);
        self.pyramid.blur(&self.device, encoder, levels);
        self.stamp = scene.stamp;
        self.levels = levels.to_vec();
    }

    fn sync_atlas(&mut self, scene: &Scene) {
        let key = scene.images.iter().fold(0u64, |hash, image| hash.wrapping_mul(0x9E37).wrapping_add(image.key));
        if key == self.atlas_key && self.atlas_bind.is_some() {
            return;
        }
        let packed = pack(scene);
        let (width, height) = packed.size;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-cover-atlas"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (place, pixels) in &packed.uploads {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: place.x, y: place.y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(place.w * 4),
                    rows_per_image: Some(place.h),
                },
                wgpu::Extent3d { width: place.w, height: place.h, depth_or_array_layers: 1 },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.atlas_bind = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-cover-atlas"),
            layout: &self.covers.atlas_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.covers.sampler) },
            ],
        }));
        self.covers.atlas_bytes = u64::from(width) * u64::from(height) * 4;
        self.atlas_key = key;
        self.places = packed.places;
    }

    fn write_frame(&self, scene: &Scene) {
        let encode = if self.format.is_srgb() { 1.0_f32 } else { 0.0 };
        let words = [
            scene.window[0],
            scene.window[1],
            0.0,
            0.0,
            scene.shade[0],
            scene.shade[1],
            scene.shade[2],
            scene.shade[3],
            scene.tilt[0],
            scene.tilt[1],
            scene.tilt[2],
            0.0,
            encode,
            0.0,
            0.0,
            0.0,
        ];
        self.queue.write_buffer(&self.covers.frame, 0, bytemuck::cast_slice(&words));
    }

    fn write_verts(&mut self, scene: &Scene) {
        let mut verts = Vec::with_capacity((scene.quads.len() + 1) * 6);
        push_quad(
            &mut verts,
            [0.0, 0.0, scene.window[0], scene.window[1]],
            [0.0; 4],
            [scene.ground, scene.ground],
            [2.0, 1.0, 0.0, 0.0],
        );
        for quad in &scene.quads {
            let uv =
                self.places.get(quad.image as usize).copied().map(|place| map_uv(place, quad.uv)).unwrap_or(quad.uv);
            let mode = if quad.image == u32::MAX { 0.0 } else { 1.0 };
            push_quad(&mut verts, quad.rect, uv, [quad.color0, quad.color1], [quad.shade, quad.alpha, mode, 0.0]);
        }
        let bytes = bytemuck::cast_slice(&verts);
        let needed = bytes.len() as u64;
        if self.vert_cap < needed {
            self.vert_cap = needed.max(4096);
            self.verts = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("folia-covers"),
                size: self.vert_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buffer) = &self.verts {
            self.queue.write_buffer(buffer, 0, bytes);
        }
        self.vert_count = verts.len() as u32;
    }

    fn draw_covers(&self, encoder: &mut wgpu::CommandEncoder) {
        let (Some(source), Some(verts), Some(atlas)) =
            (self.pyramid.source(), self.verts.as_ref(), self.atlas_bind.as_ref())
        else {
            return;
        };
        if self.vert_count == 0 {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-covers"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: source,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.covers.pipeline);
        pass.set_bind_group(0, &self.covers.frame_bind, &[]);
        pass.set_bind_group(1, atlas, &[]);
        pass.set_vertex_buffer(0, verts.slice(..));
        pass.draw(0..self.vert_count, 0..1);
    }
}

struct CoverGpu {
    pipeline: wgpu::RenderPipeline,
    frame: wgpu::Buffer,
    frame_bind: wgpu::BindGroup,
    atlas_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    atlas_bytes: u64,
}

impl CoverGpu {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-covers"),
            source: wgpu::ShaderSource::Wgsl(COVERS.into()),
        });
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-covers-frame"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-covers-atlas"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-covers"),
            bind_group_layouts: &[&frame_layout, &atlas_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("folia-covers"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vert>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &VERT_ATTRS,
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let frame = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-covers-frame"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-covers-frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: frame.as_entire_binding() }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-covers"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..wgpu::SamplerDescriptor::default()
        });
        Self { pipeline, frame, frame_bind, atlas_layout, sampler, atlas_bytes: 0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vert {
    pos: [f32; 2],
    uv: [f32; 2],
    local: [f32; 2],
    pad: [f32; 2],
    color0: [f32; 4],
    color1: [f32; 4],
    params: [f32; 4],
}

const VERT_ATTRS: [wgpu::VertexAttribute; 6] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 8, shader_location: 1 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 16, shader_location: 2 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 3 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 4 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 64, shader_location: 5 },
];

static STAGE: Mutex<Stage> =
    Mutex::new(Stage { gpu: None, levels: Vec::new(), window: [0.0, 0.0], live: false, last_live: false, idle: 0 });

/// Frame time of the blur while the picture changes, and the bytes kept while the glass is open.
pub struct GlassBench {
    pub avg_ms: f32,
    pub max_ms: f32,
    pub bytes: u64,
}

/// `level` is the pyramid step (`4` is CSS blur 24px, `5` is 40px). `frames` should be at least 6.
pub fn bench_glass(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    level: u32,
    frames: u32,
) -> Option<GlassBench> {
    let mut scene = sample_wall(width as f32, height as f32);
    let mut picture = Picture::new(device, queue, wgpu::TextureFormat::Rgba8Unorm);
    let mut samples = Vec::new();
    for frame in 0..frames {
        scene.stamp = u64::from(frame) + 1;
        let started = std::time::Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        picture.paint(&mut encoder, &scene, &[level.max(1)]);
        queue.submit(Some(encoder.finish()));
        device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        samples.push(started.elapsed().as_secs_f32() * 1000.0);
    }
    let sum: f32 = samples.iter().sum();
    let avg_ms = sum / samples.len().max(1) as f32;
    let max_ms = samples.iter().copied().fold(0.0_f32, f32::max);
    Some(GlassBench { avg_ms, max_ms, bytes: picture.bytes() })
}

/// One offscreen frame of a wall with an open tools panel, for a visual check.
pub struct Shot {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// GPU bytes held while the glass is open: the pyramid plus the cover atlas.
    pub bytes: u64,
}

/// Draws a few posters and a 24px glass panel. The device is the caller's.
pub fn preview_wall(device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32) -> Option<Shot> {
    let scene = sample_wall(width as f32, height as f32);
    let mut picture = Picture::new(device, queue, wgpu::TextureFormat::Rgba8Unorm);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    picture.paint(&mut encoder, &scene, &[4]);
    queue.submit(Some(encoder.finish()));
    let sharp = picture.pyramid.read_source(device, queue)?;
    let blurred = picture.pyramid.read(device, queue, 0)?;
    let (sw, sh) = blur::source_size(width, height);
    let rgba = compose(&sharp, &blurred, sw, sh, width, height);
    Some(Shot { rgba, width, height, bytes: picture.bytes() })
}

fn sample_wall(width: f32, height: f32) -> Scene {
    let tiles = [[0.86, 0.22, 0.18, 1.0], [0.95, 0.78, 0.15, 1.0], [0.15, 0.55, 0.85, 1.0], [0.20, 0.72, 0.38, 1.0]];
    let mut quads = Vec::new();
    let side = 180.0_f32;
    let y = (height - 360.0).max(40.0);
    for (index, color) in tiles.iter().enumerate() {
        let x = 40.0 + index as f32 * (side + 16.0);
        quads.push(Quad {
            rect: [x, y, side, side],
            uv: [0.0, 0.0, 1.0, 1.0],
            image: u32::MAX,
            color0: *color,
            color1: [color[0] * 0.35, color[1] * 0.35, color[2] * 0.35, 1.0],
            shade: 0.0,
            alpha: 1.0,
        });
        quads.push(Quad {
            rect: [x + 24.0, y + side - 48.0, side - 48.0, 14.0],
            uv: [0.0; 4],
            image: u32::MAX,
            color0: [1.0, 1.0, 1.0, 1.0],
            color1: [1.0, 1.0, 1.0, 1.0],
            shade: 2.0,
            alpha: 1.0,
        });
    }
    Scene {
        window: [width, height],
        ground: [0.08, 0.08, 0.09, 1.0],
        shade: [0.0, 0.0, 0.0, 1.0],
        tilt: [0.0; 3],
        quads,
        images: Vec::new(),
        stamp: 1,
    }
}

fn compose(sharp: &[u8], blurred: &[u8], source_w: u32, source_h: u32, width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![0u8; (width * height * 4) as usize];
    let panel = [width.saturating_sub(320) as f32, height.saturating_sub(280) as f32, 280.0_f32, 200.0_f32];
    for y in 0..height {
        for x in 0..width {
            let sx = (x * source_w / width).min(source_w.saturating_sub(1));
            let sy = (y * source_h / height).min(source_h.saturating_sub(1));
            let index = ((sy * source_w + sx) * 4) as usize;
            let pixel = if rounded(x as f32, y as f32, panel, 24.0) {
                mix_black(sample(blurred, index), 0.40)
            } else {
                sample(sharp, index)
            };
            let dest = ((y * width + x) * 4) as usize;
            if let Some(slot) = out.get_mut(dest..dest + 4) {
                slot.copy_from_slice(&pixel);
            }
        }
    }
    out
}

fn sample(rgba: &[u8], index: usize) -> [u8; 4] {
    rgba.get(index..index + 4).map(|pixel| [pixel[0], pixel[1], pixel[2], 255]).unwrap_or([0, 0, 0, 255])
}

fn mix_black(pixel: [u8; 4], alpha: f32) -> [u8; 4] {
    let keep = 1.0 - alpha;
    [(f32::from(pixel[0]) * keep) as u8, (f32::from(pixel[1]) * keep) as u8, (f32::from(pixel[2]) * keep) as u8, 255]
}

fn rounded(x: f32, y: f32, rect: [f32; 4], radius: f32) -> bool {
    let (left, top, width, height) = (rect[0], rect[1], rect[2], rect[3]);
    if x < left || y < top || x > left + width || y > top + height {
        return false;
    }
    let dx = (x - (left + width / 2.0)).abs() - (width / 2.0 - radius);
    let dy = (y - (top + height / 2.0)).abs() - (height / 2.0 - radius);
    if dx <= 0.0 || dy <= 0.0 {
        return true;
    }
    dx * dx + dy * dy <= radius * radius
}

/// Mixes the allocation address, so scrolling does not upload the same cover again.
pub fn image_key(pixels: &[u8]) -> u64 {
    (pixels.as_ptr() as u64).wrapping_mul(0x0100_0000_01b3) ^ pixels.len() as u64
}

struct Spot {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

struct Packed {
    size: (u32, u32),
    places: Vec<[f32; 4]>,
    uploads: Vec<(Spot, Vec<u8>)>,
}

fn pack(scene: &Scene) -> Packed {
    let mut cursor_x = 0u32;
    let mut cursor_y = 0u32;
    let mut row = 0u32;
    let mut width = 1u32;
    let mut places = vec![[0.0_f32; 4]; scene.images.len()];
    let mut spots = Vec::new();
    let mut uploads = Vec::new();
    for (index, image) in scene.images.iter().enumerate() {
        let Handle::Rgba { width: src_w, height: src_h, pixels, .. } = &image.pixels else { continue };
        let (w, h, rgba) = fit(*src_w, *src_h, pixels);
        if cursor_x + w > ATLAS_LIMIT {
            cursor_x = 0;
            cursor_y += row;
            row = 0;
        }
        if cursor_y + h > ATLAS_LIMIT {
            break;
        }
        spots.push((index, Spot { x: cursor_x, y: cursor_y, w, h }));
        uploads.push((Spot { x: cursor_x, y: cursor_y, w, h }, rgba));
        cursor_x += w;
        row = row.max(h);
        width = width.max(cursor_x);
    }
    if uploads.is_empty() {
        uploads.push((Spot { x: 0, y: 0, w: 1, h: 1 }, vec![255, 255, 255, 255]));
        return Packed { size: (1, 1), places, uploads };
    }
    let size = (width.max(1), (cursor_y + row).max(1));
    for (index, spot) in spots {
        if let Some(place) = places.get_mut(index) {
            *place = [
                spot.x as f32 / size.0 as f32,
                spot.y as f32 / size.1 as f32,
                (spot.x + spot.w) as f32 / size.0 as f32,
                (spot.y + spot.h) as f32 / size.1 as f32,
            ];
        }
    }
    Packed { size, places, uploads }
}

fn fit(width: u32, height: u32, pixels: &[u8]) -> (u32, u32, Vec<u8>) {
    let longest = width.max(height).max(1);
    if longest <= MAX_SIDE {
        return (width.max(1), height.max(1), pixels.to_vec());
    }
    let w = (width as f32 * MAX_SIDE as f32 / longest as f32).round().max(1.0) as u32;
    let h = (height as f32 * MAX_SIDE as f32 / longest as f32).round().max(1.0) as u32;
    let mut out = vec![0u8; (w * h * 4) as usize];
    for row in 0..h {
        for col in 0..w {
            let sx = (col * width / w).min(width.saturating_sub(1));
            let sy = (row * height / h).min(height.saturating_sub(1));
            let from = ((sy * width + sx) * 4) as usize;
            let to = ((row * w + col) * 4) as usize;
            if let Some(pixel) = pixels.get(from..from + 4) {
                out[to..to + 4].copy_from_slice(pixel);
            }
        }
    }
    (w, h, out)
}

fn map_uv(place: [f32; 4], uv: [f32; 4]) -> [f32; 4] {
    let (u0, v0, u1, v1) = (place[0], place[1], place[2], place[3]);
    [u0 + uv[0] * (u1 - u0), v0 + uv[1] * (v1 - v0), u0 + uv[2] * (u1 - u0), v0 + uv[3] * (v1 - v0)]
}

fn push_quad(verts: &mut Vec<Vert>, rect: [f32; 4], uv: [f32; 4], colors: [[f32; 4]; 2], params: [f32; 4]) {
    let [x, y, w, h] = rect;
    let corners = [(0.0_f32, 0.0_f32), (1.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    for (lx, ly) in corners {
        verts.push(Vert {
            pos: [x + lx * w, y + ly * h],
            uv: [uv[0] + lx * (uv[2] - uv[0]), uv[1] + ly * (uv[3] - uv[1])],
            local: [lx, ly],
            pad: [0.0, 0.0],
            color0: colors[0],
            color1: colors[1],
            params,
        });
    }
}
