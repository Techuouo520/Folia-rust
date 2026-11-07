// The lumiere passes: the graphics group (light, echo, lines, stars, motes) and the text group (crosses, lyrics),
// each bloomed on its own. Front bokeh is its own pass.

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
const IDLE_TRIMS: u32 = 120;

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
    /// `LUMIERE_QUALITY_PROFILES.graphicsScale`.
    pub graphics_scale: f32,
}

/// Foreground bokeh drawn above the lyrics.
#[derive(Debug)]
pub struct FrontPass {
    pub bytes: [u8; UNIFORM_BYTES],
    pub decor: [u8; DECOR_BYTES],
    pub particles: Option<Vec<u8>>,
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
    /// Trims since the last prepare. Other windows trim too, so the textures go only after a long run without lumiere.
    idle: u32,
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
            bloom: bloom::Filter::new(device, format, wgpu::BlendState::REPLACE),
            text_bloom: bloom::Filter::new(device, format, wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            glyphs: Glyphs::new(device, format, MAX_SPRITES, super::sprite_images()),
            sprites: 0,
            idle: 0,
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
        let gpu = &mut pipeline.0;
        gpu.idle = 0;
        gpu.window = [bounds.width.max(1.0), bounds.height.max(1.0)];
        let (width, height) = pixel_size(bounds, viewport, self.graphics_scale);
        let strength = if self.text_only { 0.0 } else { bloom::GRAPHICS.strength * self.bloom };
        gpu.bloom.sync(device, queue, width, height, &bloom::Preset { strength, ..bloom::GRAPHICS });
        let strength = bloom::TEXT.strength * self.text_bloom;
        gpu.text_bloom.sync(device, queue, width, height, &bloom::Preset { strength, ..bloom::TEXT });
        let sprites = self.glyphs.lock();
        gpu.glyphs.prepare(device, queue, &sprites, [bounds.width, bounds.height], viewport.scale_factor());
        gpu.sprites = sprites.len().min(MAX_SPRITES) as u32;
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
        let gpu = &pipeline.0;
        let (Some(graphics), Some(text)) = (gpu.bloom.scene(), gpu.text_bloom.scene()) else { return };
        if clip.width == 0 || clip.height == 0 {
            return;
        }
        let ground = if self.text_only {
            wgpu::Color { r: f64::from(self.base[0]), g: f64::from(self.base[1]), b: f64::from(self.base[2]), a: 1.0 }
        } else {
            wgpu::Color::BLACK
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
        if let Some(scene) = gpu.bloom.scene() {
            let stamp = self
                .bytes
                .iter()
                .take(24)
                .fold(0u64, |hash, byte| hash.wrapping_mul(0x0000_0100_0000_01b3).wrapping_add(u64::from(*byte)));
            folia_render::paint_external(&gpu.device, &gpu.queue, encoder, gpu.format, scene, stamp, gpu.window);
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
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &shader::Viewport,
    ) {
        let gpu = &mut pipeline.0;
        queue.write_buffer(&gpu.light_uniform, 0, &self.bytes);
        queue.write_buffer(&gpu.decor_uniform, 0, &self.decor);
        if let Some(particles) = &self.particles {
            gpu.count = upload(queue, &gpu.particles, particles, PARTICLE_STRIDE);
        }
    }

    fn draw(&self, pipeline: &Self::Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let gpu = &pipeline.0;
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

pub struct StageGpu(Targets);
pub struct FrontStage(FrontGpu);

impl shader::Pipeline for StageGpu {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self(Targets::new(device, queue, format))
    }

    /// iced trims after every present of any window. After `IDLE_TRIMS` without lumiere, the full-size textures and
    /// the atlas go.
    fn trim(&mut self) {
        let gpu = &mut self.0;
        gpu.idle = gpu.idle.saturating_add(1);
        if gpu.idle == IDLE_TRIMS {
            gpu.bloom.release();
            gpu.text_bloom.release();
            gpu.glyphs.release();
            tracing::debug!("lumiere textures released");
        }
    }
}

impl shader::Pipeline for FrontStage {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self(FrontGpu::new(device, format))
    }

    fn trim(&mut self) {}
}
