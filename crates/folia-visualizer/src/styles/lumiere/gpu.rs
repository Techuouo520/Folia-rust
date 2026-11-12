// The lumiere passes: the graphics group (light, echo, lines, stars, motes) and the text group (crosses, lyrics),
// each bloomed on its own. Front bokeh is its own pass.

use std::sync::Weak;
#[cfg(feature = "dev")]
use std::sync::atomic::{AtomicUsize, Ordering};

use folia_render::glyphs::{Batch, Glyphs};
use iced::Rectangle;
use iced::widget::shader::{self, Primitive as ShaderPrimitive};

use super::art::MAX_LINE_FLOATS;
use super::bloom;
use super::field::UNIFORM_BYTES;

const LIGHT: &str = include_str!("../../../shaders/lumiere/light_field.wgsl");
const DECOR: &str = include_str!("../../../shaders/lumiere/decor.wgsl");
pub const DECOR_BYTES: usize = 544;
const LINE_STRIDE: u64 = 64;
const PARTICLE_STRIDE: u64 = 48;
pub(super) const MAX_PARTICLES: u64 = 4096;
const BURSTS: u32 = 84;
const MAX_SPRITES: usize = 4096;

#[cfg(feature = "dev")]
static STAGE_SLOTS: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "dev")]
static STAGE_TEXTURES: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "dev")]
static FRONT_SLOTS: AtomicUsize = AtomicUsize::new(0);

/// Graphics-group slots whose owner is still alive. Release builds omit the counter.
#[cfg(feature = "dev")]
pub fn stage_slots() -> usize {
    STAGE_SLOTS.load(Ordering::Relaxed)
}

/// Slots that still hold a bloom pyramid. Release builds omit the counter.
#[cfg(feature = "dev")]
pub fn stage_textures() -> usize {
    STAGE_TEXTURES.load(Ordering::Relaxed)
}

#[cfg(feature = "dev")]
pub fn front_slots() -> usize {
    FRONT_SLOTS.load(Ordering::Relaxed)
}

const LINE_ATTRIBUTES: [wgpu::VertexAttribute; 8] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 8, shader_location: 1 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 16, shader_location: 2 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 24, shader_location: 3 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 32, shader_location: 4 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 40, shader_location: 5 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 48, shader_location: 6 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 56, shader_location: 7 },
];

const PARTICLE_ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 1 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 2 },
];

/// Everything under the foreground bokeh. Geometry is uploaded only when it changes.
#[derive(Debug)]
pub struct Pass {
    pub bytes: [u8; UNIFORM_BYTES],
    pub decor: [u8; DECOR_BYTES],
    pub lines: Option<Vec<u8>>,
    pub particles: Option<Vec<u8>>,
    /// Particles `..split[0]` are the starfall, `..split[1]` the line-art nodes, the rest motes.
    pub split: [u32; 2],
    pub kind: &'static str,
    /// Multiplier on the graphics bloom preset. `1` keeps the original strength.
    pub bloom: f32,
    /// Multiplier on the text bloom preset.
    pub text_bloom: f32,
    /// This frame's glyphs: the first `echo` belong to the graphics group, the rest are the lyrics.
    pub glyphs: Batch,
    pub echo: u32,
    /// Only the lyrics: the graphics group is the flat `base` and is not bloomed.
    pub text_only: bool,
    pub base: [f32; 3],
    pub veil: [f32; 4],
    /// `LUMIERE_QUALITY_PROFILES.graphicsScale`.
    pub graphics_scale: f32,
    /// Widget identity. iced keeps one pipeline type for every lumiere scene in the frame.
    pub slot: u64,
    pub owner: Weak<std::sync::atomic::AtomicBool>,
    /// Only the playing page copies this scene into the glass blur source.
    pub publish_blur: bool,
}

/// Foreground bokeh drawn above the lyrics.
#[derive(Debug)]
pub struct FrontPass {
    pub bytes: [u8; UNIFORM_BYTES],
    pub decor: [u8; DECOR_BYTES],
    pub particles: Option<Vec<u8>>,
    pub slot: u64,
    pub owner: Weak<std::sync::atomic::AtomicBool>,
}

struct Targets {
    light: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    particle: wgpu::RenderPipeline,
    light_bind: wgpu::BindGroup,
    decor_bind: wgpu::BindGroup,
    light_uniform: wgpu::Buffer,
    decor_uniform: wgpu::Buffer,
    lines: wgpu::Buffer,
    particles: wgpu::Buffer,
    bursts: wgpu::Buffer,
    line_verts: u32,
    particle_count: u32,
    pending_bursts: Vec<u8>,
    bloom: bloom::Filter,
    text_bloom: bloom::Filter,
    glyphs: Glyphs,
    sprites: u32,
    /// Bloom pyramid is allocated. Dropped only with the slot, not because another window presented.
    textures_live: bool,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    window: [f32; 2],
}

struct FrontGpu {
    particle: wgpu::RenderPipeline,
    light_bind: wgpu::BindGroup,
    decor_bind: wgpu::BindGroup,
    light_uniform: wgpu::Buffer,
    decor_uniform: wgpu::Buffer,
    particles: wgpu::Buffer,
    count: u32,
}

impl Targets {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let light_module = module(device, "folia-lumiere", LIGHT);
        let decor_module = module(device, "folia-lumiere-decor", DECOR);
        let group0 = uniform_layout(device, "folia-lumiere-light", wgpu::ShaderStages::VERTEX_FRAGMENT, 0);
        let group1 = uniform_layout(device, "folia-lumiere-decor", wgpu::ShaderStages::VERTEX, 1);
        let light_layout = pipeline_layout(device, "folia-lumiere", &[&group0]);
        let decor_layout = pipeline_layout(device, "folia-lumiere-decor", &[&group0, &group1]);
        let light_uniform = uniform_buffer(device, "folia-lumiere-uniforms", UNIFORM_BYTES as u64);
        let decor_uniform = uniform_buffer(device, "folia-lumiere-decor", DECOR_BYTES as u64);
        let light_bind = bind(device, "folia-lumiere", &group0, 0, &light_uniform);
        let decor_bind = bind(device, "folia-lumiere-decor", &group1, 1, &decor_uniform);
        let line_layout = wgpu::VertexBufferLayout {
            array_stride: LINE_STRIDE,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &LINE_ATTRIBUTES,
        };
        let particle_layout = wgpu::VertexBufferLayout {
            array_stride: PARTICLE_STRIDE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &PARTICLE_ATTRIBUTES,
        };
        let light = color_pipeline(device, &light_module, "vs", &light_layout, &[], format, wgpu::BlendState::REPLACE);
        let line = color_pipeline(
            device,
            &decor_module,
            "vs_line",
            &decor_layout,
            &[line_layout],
            format,
            wgpu::BlendState::ALPHA_BLENDING,
        );
        let particle = color_pipeline(
            device,
            &decor_module,
            "vs_particle",
            &decor_layout,
            &[particle_layout],
            format,
            wgpu::BlendState::ALPHA_BLENDING,
        );
        let lines = vertex_buffer(device, "folia-lumiere-lines", MAX_LINE_FLOATS as u64 * 4);
        let particles = vertex_buffer(device, "folia-lumiere-particles", MAX_PARTICLES * PARTICLE_STRIDE);
        let bursts = vertex_buffer(device, "folia-lumiere-bursts", u64::from(BURSTS) * PARTICLE_STRIDE);
        Self {
            light,
            line,
            particle,
            light_bind,
            decor_bind,
            light_uniform,
            decor_uniform,
            lines,
            particles,
            bursts,
            line_verts: 0,
            particle_count: 0,
            pending_bursts: burst_instances(),
            bloom: bloom::Filter::new(device, format, wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            text_bloom: bloom::Filter::new(device, format, wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            glyphs: Glyphs::new(device, format, MAX_SPRITES, super::sprite_images()),
            sprites: 0,
            textures_live: false,
            device: device.clone(),
            queue: queue.clone(),
            format,
            window: [1.0, 1.0],
        }
    }
}

fn module(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

fn uniform_layout(
    device: &wgpu::Device,
    label: &str,
    visibility: wgpu::ShaderStages,
    binding: u32,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

fn pipeline_layout(device: &wgpu::Device, label: &str, groups: &[&wgpu::BindGroupLayout]) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: groups,
        push_constant_ranges: &[],
    })
}

fn uniform_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn vertex_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn bind(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    binding: u32,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[wgpu::BindGroupEntry { binding, resource: buffer.as_entire_binding() }],
    })
}

fn burst_instances() -> Vec<u8> {
    let mut bytes = vec![0_u8; BURSTS as usize * PARTICLE_STRIDE as usize];
    for index in 0..14_u32 {
        for part in 0..6_u32 {
            let slot = (index * 6 + part) as usize * PARTICLE_STRIDE as usize + 32;
            bytes[slot..slot + 4].copy_from_slice(&(index as f32).to_le_bytes());
            bytes[slot + 4..slot + 8].copy_from_slice(&7.0_f32.to_le_bytes());
            bytes[slot + 8..slot + 12].copy_from_slice(&(part as f32).to_le_bytes());
        }
    }
    bytes
}

fn color_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    entry: &str,
    layout: &wgpu::PipelineLayout,
    buffers: &[wgpu::VertexBufferLayout<'_>],
    format: wgpu::TextureFormat,
    blend: wgpu::BlendState,
) -> wgpu::RenderPipeline {
    let fragment = if entry == "vs" {
        "fragment"
    } else if entry == "vs_line" {
        "fs_line"
    } else {
        "fs_particle"
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(entry),
            buffers,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fragment),
            targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn upload(queue: &wgpu::Queue, buffer: &wgpu::Buffer, bytes: &[u8], stride: u64) -> u32 {
    let room = buffer.size() as usize;
    let end = bytes.len().min(room);
    let end = end - end % stride as usize;
    if end > 0 {
        queue.write_buffer(buffer, 0, &bytes[..end]);
    }
    (end as u64 / stride) as u32
}

impl FrontGpu {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let decor_module = module(device, "folia-lumiere-front", DECOR);
        let group0 = uniform_layout(device, "folia-lumiere-front-light", wgpu::ShaderStages::VERTEX_FRAGMENT, 0);
        let group1 = uniform_layout(device, "folia-lumiere-front-decor", wgpu::ShaderStages::VERTEX, 1);
        let layout = pipeline_layout(device, "folia-lumiere-front", &[&group0, &group1]);
        let light_uniform = uniform_buffer(device, "folia-lumiere-front-light", UNIFORM_BYTES as u64);
        let decor_uniform = uniform_buffer(device, "folia-lumiere-front-decor", DECOR_BYTES as u64);
        let particle_layout = wgpu::VertexBufferLayout {
            array_stride: PARTICLE_STRIDE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &PARTICLE_ATTRIBUTES,
        };
        Self {
            particle: color_pipeline(
                device,
                &decor_module,
                "vs_particle",
                &layout,
                &[particle_layout],
                format,
                wgpu::BlendState::ALPHA_BLENDING,
            ),
            light_bind: bind(device, "folia-lumiere-front", &group0, 0, &light_uniform),
            decor_bind: bind(device, "folia-lumiere-front-decor", &group1, 1, &decor_uniform),
            light_uniform,
            decor_uniform,
            particles: vertex_buffer(device, "folia-lumiere-front-particles", MAX_PARTICLES * PARTICLE_STRIDE),
            count: 0,
        }
    }
}

impl ShaderPrimitive for Pass {
    type Pipeline = StageGpu;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        let Some(gpu) = pipeline.slot(self.slot, self.owner.clone()) else { return };
        gpu.window = [bounds.width.max(1.0), bounds.height.max(1.0)];
        let (width, height) = pixel_size(bounds, viewport, self.graphics_scale);
        let strength = if self.text_only { 0.0 } else { bloom::GRAPHICS.strength * self.bloom };
        gpu.bloom.sync(device, queue, width, height, &bloom::Preset { strength, ..bloom::GRAPHICS });
        let strength = bloom::TEXT.strength * self.text_bloom;
        gpu.text_bloom.sync(device, queue, width, height, &bloom::Preset { strength, ..bloom::TEXT });
        let sprites = self.glyphs.lock();
        gpu.glyphs.prepare(device, queue, &sprites, [bounds.width, bounds.height], viewport.scale_factor());
        gpu.sprites = sprites.len().min(MAX_SPRITES) as u32;
        note_stage_textures(gpu);
        drop(sprites);
        queue.write_buffer(&gpu.light_uniform, 0, &self.bytes);
        queue.write_buffer(&gpu.decor_uniform, 0, &self.decor);
        if !gpu.pending_bursts.is_empty() {
            queue.write_buffer(&gpu.bursts, 0, &gpu.pending_bursts);
            gpu.pending_bursts = Vec::new();
        }
        if let Some(lines) = &self.lines {
            gpu.line_verts = upload(queue, &gpu.lines, lines, LINE_STRIDE);
        }
        if let Some(particles) = &self.particles {
            gpu.particle_count = upload(queue, &gpu.particles, particles, PARTICLE_STRIDE);
        }
    }

    fn draw(&self, _pipeline: &Self::Pipeline, _pass: &mut wgpu::RenderPass<'_>) -> bool {
        false
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip: &Rectangle<u32>,
    ) {
        let Some(gpu) = pipeline.targets(self.slot) else { return };
        let (Some(graphics), Some(text)) = (gpu.bloom.scene(), gpu.text_bloom.scene()) else { return };
        if clip.width == 0 || clip.height == 0 {
            return;
        }
        let ground = if self.text_only {
            wgpu::Color { r: f64::from(self.base[0]), g: f64::from(self.base[1]), b: f64::from(self.base[2]), a: 1.0 }
        } else {
            wgpu::Color::TRANSPARENT
        };
        {
            let mut pass = scene_pass(encoder, graphics, "folia-lumiere-graphics", ground);
            if !self.text_only {
                paint_graphics(gpu, &mut pass, self.echo, self.split);
            }
        }
        gpu.bloom.encode(encoder, target, clip.x, clip.y, clip.width, clip.height);
        {
            let mut pass = scene_pass(encoder, text, "folia-lumiere-text", wgpu::Color::TRANSPARENT);
            paint_text(gpu, &mut pass, self.echo);
        }
        gpu.text_bloom.encode(encoder, target, clip.x, clip.y, clip.width, clip.height);
        if self.publish_blur
            && let Some(scene) = gpu.bloom.scene()
        {
            let stamp = self
                .bytes
                .iter()
                .take(24)
                .fold(0u64, |hash, byte| hash.wrapping_mul(0x0000_0100_0000_01b3).wrapping_add(u64::from(*byte)));
            folia_render::paint_over_underlay(
                &gpu.device,
                &gpu.queue,
                encoder,
                gpu.format,
                folia_render::UnderlayOverlay { foreground: scene, stamp, window: gpu.window, veil: self.veil },
            );
        }
    }
}

fn scene_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    label: &str,
    clear: wgpu::Color,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

/// scene.ts `graphics`: light field, echo, starfall, line art (lines, then its nodes), motes. There are no icons yet.
fn paint_graphics(gpu: &Targets, pass: &mut wgpu::RenderPass<'_>, echo: u32, split: [u32; 2]) {
    pass.set_pipeline(&gpu.light);
    pass.set_bind_group(0, &gpu.light_bind, &[]);
    pass.draw(0..3, 0..1);
    gpu.glyphs.draw(pass, 0..echo);
    let stars = split[0].min(gpu.particle_count);
    let nodes = split[1].clamp(stars, gpu.particle_count);
    particles(gpu, pass, 0..stars);
    if gpu.line_verts > 0 {
        pass.set_pipeline(&gpu.line);
        pass.set_bind_group(0, &gpu.light_bind, &[]);
        pass.set_bind_group(1, &gpu.decor_bind, &[]);
        pass.set_vertex_buffer(0, gpu.lines.slice(..));
        pass.draw(0..gpu.line_verts, 0..1);
    }
    particles(gpu, pass, stars..nodes);
    particles(gpu, pass, nodes..gpu.particle_count);
}

fn particles(gpu: &Targets, pass: &mut wgpu::RenderPass<'_>, range: std::ops::Range<u32>) {
    if range.is_empty() {
        return;
    }
    pass.set_pipeline(&gpu.particle);
    pass.set_bind_group(0, &gpu.light_bind, &[]);
    pass.set_bind_group(1, &gpu.decor_bind, &[]);
    pass.set_vertex_buffer(0, gpu.particles.slice(..));
    pass.draw(0..6, range);
}

/// Cross bursts under the lyrics (scene.ts `text`), on a clear layer.
fn paint_text(gpu: &Targets, pass: &mut wgpu::RenderPass<'_>, echo: u32) {
    pass.set_pipeline(&gpu.particle);
    pass.set_bind_group(0, &gpu.light_bind, &[]);
    pass.set_bind_group(1, &gpu.decor_bind, &[]);
    pass.set_vertex_buffer(0, gpu.bursts.slice(..));
    pass.draw(0..6, 0..BURSTS);
    gpu.glyphs.draw(pass, echo..gpu.sprites);
}

fn pixel_size(bounds: &Rectangle, viewport: &shader::Viewport, graphics_scale: f32) -> (u32, u32) {
    let scale = viewport.scale_factor() * graphics_scale.clamp(0.5, 1.0);
    let width = (bounds.width * scale).round().max(1.0) as u32;
    let height = (bounds.height * scale).round().max(1.0) as u32;
    (width, height)
}

impl ShaderPrimitive for FrontPass {
    type Pipeline = FrontStage;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &shader::Viewport,
    ) {
        let Some(gpu) = pipeline.slot(self.slot, self.owner.clone(), device) else { return };
        queue.write_buffer(&gpu.light_uniform, 0, &self.bytes);
        queue.write_buffer(&gpu.decor_uniform, 0, &self.decor);
        if let Some(particles) = &self.particles {
            gpu.count = upload(queue, &gpu.particles, particles, PARTICLE_STRIDE);
        }
    }

    fn draw(&self, pipeline: &Self::Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some(gpu) = pipeline.gpu(self.slot) else { return true };
        if gpu.count == 0 {
            return true;
        }
        pass.set_pipeline(&gpu.particle);
        pass.set_bind_group(0, &gpu.light_bind, &[]);
        pass.set_bind_group(1, &gpu.decor_bind, &[]);
        pass.set_vertex_buffer(0, gpu.particles.slice(..));
        pass.draw(0..6, 0..gpu.count);
        true
    }
}

struct StageSlot {
    key: u64,
    owner: Weak<std::sync::atomic::AtomicBool>,
    gpu: folia_render::Prepared<Targets>,
}

impl Drop for StageSlot {
    fn drop(&mut self) {
        if let Some(gpu) = self.gpu.get_mut()
            && gpu.textures_live
        {
            #[cfg(feature = "dev")]
            STAGE_TEXTURES.fetch_sub(1, Ordering::Relaxed);
            gpu.textures_live = false;
        }
        #[cfg(feature = "dev")]
        STAGE_SLOTS.fetch_sub(1, Ordering::Relaxed);
    }
}

struct FrontSlot {
    key: u64,
    owner: Weak<std::sync::atomic::AtomicBool>,
    gpu: folia_render::Prepared<FrontGpu>,
}

impl Drop for FrontSlot {
    fn drop(&mut self) {
        #[cfg(feature = "dev")]
        FRONT_SLOTS.fetch_sub(1, Ordering::Relaxed);
    }
}

fn note_stage_textures(gpu: &mut Targets) {
    let live = gpu.bloom.scene().is_some();
    if live == gpu.textures_live {
        return;
    }
    #[cfg(feature = "dev")]
    if live {
        STAGE_TEXTURES.fetch_add(1, Ordering::Relaxed);
    } else {
        STAGE_TEXTURES.fetch_sub(1, Ordering::Relaxed);
    }
    gpu.textures_live = live;
}

pub struct StageGpu {
    slots: Vec<StageSlot>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
}

pub struct FrontStage {
    slots: Vec<FrontSlot>,
    format: wgpu::TextureFormat,
}

impl StageGpu {
    fn slot(&mut self, key: u64, owner: Weak<std::sync::atomic::AtomicBool>) -> Option<&mut Targets> {
        if let Some(index) = self.slots.iter().position(|slot| slot.key == key) {
            self.slots[index].owner = owner;
            return self.slots[index].gpu.poll();
        }
        #[cfg(feature = "dev")]
        STAGE_SLOTS.fetch_add(1, Ordering::Relaxed);
        let (device, queue, format) = (self.device.clone(), self.queue.clone(), self.format);
        let gpu = folia_render::Prepared::new(owner.clone(), move || Targets::new(&device, &queue, format));
        self.slots.push(StageSlot { key, owner, gpu });
        let index = self.slots.len() - 1;
        self.slots[index].gpu.poll()
    }

    fn targets(&self, key: u64) -> Option<&Targets> {
        self.slots.iter().find(|slot| slot.key == key).and_then(|slot| slot.gpu.get())
    }
}

impl FrontStage {
    fn slot(
        &mut self,
        key: u64,
        owner: Weak<std::sync::atomic::AtomicBool>,
        device: &wgpu::Device,
    ) -> Option<&mut FrontGpu> {
        if let Some(index) = self.slots.iter().position(|slot| slot.key == key) {
            self.slots[index].owner = owner;
            return self.slots[index].gpu.poll();
        }
        #[cfg(feature = "dev")]
        FRONT_SLOTS.fetch_add(1, Ordering::Relaxed);
        let (device, format) = (device.clone(), self.format);
        let gpu = folia_render::Prepared::new(owner.clone(), move || FrontGpu::new(&device, format));
        self.slots.push(FrontSlot { key, owner, gpu });
        let index = self.slots.len() - 1;
        self.slots[index].gpu.poll()
    }

    fn gpu(&self, key: u64) -> Option<&FrontGpu> {
        self.slots.iter().find(|slot| slot.key == key).and_then(|slot| slot.gpu.get())
    }
}

impl shader::Pipeline for StageGpu {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self { slots: Vec::new(), device: device.clone(), queue: queue.clone(), format }
    }

    /// Every present trims this storage, including another window that shares the engine.
    /// A living owner keeps its pyramid and atlas; only a dropped widget releases them.
    fn trim(&mut self) {
        self.slots.retain_mut(|slot| {
            if slot.owner.strong_count() == 0 {
                return false;
            }
            if let Some(gpu) = slot.gpu.get_mut() {
                note_stage_textures(gpu);
            }
            true
        });
    }
}

impl shader::Pipeline for FrontStage {
    fn new(_device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self { slots: Vec::new(), format }
    }

    fn trim(&mut self) {
        self.slots.retain(|slot| slot.owner.strong_count() > 0);
    }
}
