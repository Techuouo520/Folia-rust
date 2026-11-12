// Sora (空): 150 stars drifting on a black night field or a white daylight field.

use std::cell::Cell;
#[cfg(feature = "dev")]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use folia_core::Rgb;
use iced::widget::shader::{self, Primitive as ShaderPrimitive};
use iced::{Element, Length, Rectangle, mouse};

use crate::Ground;
use crate::clock;
use crate::latent::shader_budget;

const PARTICLES: u32 = 150;

const STARS: &str = r#"
struct Uniforms {
    resolution: vec2<f32>,
    time: f32,
    daylight: f32,
    primary: vec4<f32>,
    accent: vec4<f32>,
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color_type: f32,
    @location(1) intensity: f32,
    @location(2) point: vec2<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

@vertex
fn vs(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex];
    let seed = f32(instance) * 123.456;
    let speed_rand = hash2(vec2<f32>(seed, 1.1));
    let size_rand = hash2(vec2<f32>(seed, 2.2));
    let y_rand = hash2(vec2<f32>(seed, 3.3));
    let x_rand = hash2(vec2<f32>(seed, 4.4));
    let blink_rand = hash2(vec2<f32>(seed, 5.5));
    let color_type = hash2(vec2<f32>(seed, 6.6));
    let aspect = uniforms.resolution.x / max(uniforms.resolution.y, 1.0);
    let speed = 0.008 + speed_rand * 0.024;
    let size = 0.0006 + 0.0032 * pow(size_rand, 3.8);
    let y = -0.45 + y_rand * 0.9;
    let margin = 0.05;
    let width = aspect + margin * 2.0;
    var x = fract(x_rand + uniforms.time * speed);
    x = x * width - (aspect * 0.5 + margin);
    let wave = sin(uniforms.time * 0.3 + y_rand * 6.283) * 0.005;
    let ndc = vec2<f32>(x / (aspect * 0.5), (y + wave) / 0.5);
    let point_size = max(2.0, size * uniforms.resolution.y * 3.5);
    let offset = corner * point_size / uniforms.resolution;
    var out: VsOut;
    out.pos = vec4<f32>(ndc + offset, 0.0, 1.0);
    out.color_type = color_type;
    out.intensity = 0.3 + 0.7 * sin(uniforms.time * (1.0 + blink_rand * 2.5) + x_rand * 6.283);
    out.point = corner * 0.5 + vec2<f32>(0.5);
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let dist = length(in.point - vec2<f32>(0.5));
    let alpha = smoothstep(0.5, 0.1, dist) * in.intensity;
    if (alpha < 0.01) {
        discard;
    }
    let base = select(uniforms.primary.rgb, uniforms.accent.rgb, in.color_type > 0.90);
    return vec4<f32>(base * alpha, alpha);
}
"#;

const BLIT: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var source_tex: texture_2d<f32>;
@group(0) @binding(1) var source_samp: sampler;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = positions[index];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(source_tex, source_samp, in.uv);
}
"#;

/// `soraBackgroundTuning`. The only field is the static solid frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tuning {
    pub blank: bool,
}

impl Tuning {
    pub const DEFAULT: Tuning = Tuning { blank: false };

    pub fn read(value: Option<&toml::Value>) -> Self {
        let Some(toml::Value::Table(table)) = value else { return Self::DEFAULT };
        let blank = match table.get("blank") {
            Some(toml::Value::Boolean(value)) => *value,
            _ => Self::DEFAULT.blank,
        };
        Self { blank }
    }

    pub fn table(self) -> toml::map::Map<String, toml::Value> {
        let mut table = toml::map::Map::new();
        table.insert("blank".to_owned(), toml::Value::Boolean(self.blank));
        table
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    daylight: f32,
    primary: [f32; 4],
    accent: [f32; 4],
}

pub struct Layer<'a, Message> {
    ground: Ground<'a>,
    _message: std::marker::PhantomData<Message>,
}

pub fn view<'a, Message: 'a>(ground: Ground<'a>) -> Element<'a, Message> {
    iced::widget::shader(Layer { ground, _message: std::marker::PhantomData })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub struct Motion {
    owner: Arc<()>,
    last_ms: Cell<f64>,
    time: Cell<f32>,
}

impl Default for Motion {
    fn default() -> Self {
        Self { owner: Arc::new(()), last_ms: Cell::new(0.0), time: Cell::new(0.0) }
    }
}

impl Motion {
    /// Seconds the stars have been drifting. A pause, or the blank frame, holds and then starts again.
    fn advance(&self, paused: bool, now_ms: f64) -> f32 {
        let previous = self.last_ms.get();
        if paused {
            self.last_ms.set(0.0);
            return self.time.get();
        }
        if previous != 0.0 {
            let elapsed = (now_ms - previous).max(0.0);
            self.time.set(self.time.get() + (elapsed / 1000.0) as f32);
        }
        self.last_ms.set(now_ms);
        self.time.get()
    }
}

#[cfg(feature = "dev")]
static LIVE_TARGETS: AtomicUsize = AtomicUsize::new(0);

/// Budget textures still held by a living sora widget.
#[cfg(feature = "dev")]
pub fn live_targets() -> usize {
    LIVE_TARGETS.load(Ordering::Relaxed)
}

impl<Message> shader::Program<Message> for Layer<'_, Message> {
    type State = Motion;
    type Primitive = Pass;

    fn draw(&self, state: &Motion, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        let blank = self.ground.sora.blank;
        let time = if blank {
            state.time.set(0.0);
            state.last_ms.set(0.0);
            0.0
        } else {
            state.advance(self.ground.paused, clock::wall_ms())
        };
        let (width, height) = (bounds.width.max(1.0), bounds.height.max(1.0));
        Pass {
            css: [width, height],
            uniforms: Uniforms {
                resolution: [1.0, 1.0],
                time,
                daylight: if self.ground.daylight { 1.0 } else { 0.0 },
                primary: spot(self.ground.theme.primary),
                accent: spot(self.ground.theme.accent),
            },
            blank,
            slot: folia_render::Slot::default(),
            publish: self.ground.publish,
            owner: Arc::downgrade(&state.owner),
        }
    }
}

fn spot(rgb: Rgb) -> [f32; 4] {
    [f32::from(rgb.r) / 255.0, f32::from(rgb.g) / 255.0, f32::from(rgb.b) / 255.0, 1.0]
}

#[derive(Debug)]
pub struct Pass {
    css: [f32; 2],
    uniforms: Uniforms,
    blank: bool,
    slot: folia_render::Slot,
    publish: bool,
    owner: Weak<()>,
}

struct Target {
    owner: Weak<()>,
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    blit: wgpu::BindGroup,
    place: [f32; 4],
    publish: bool,
}

pub struct Gpu {
    device: wgpu::Device,
    stars: wgpu::RenderPipeline,
    star_layout: wgpu::BindGroupLayout,
    star_slots: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    star_used: usize,
    blit: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    targets: Vec<Target>,
    format: wgpu::TextureFormat,
    _guard: folia_render::PublisherGuard,
}

impl Gpu {
    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, slot: &folia_render::Slot, bytes: &[u8]) {
        if self.star_used == self.star_slots.len() {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("folia-sora"),
                size: std::mem::size_of::<Uniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("folia-sora"),
                layout: &self.star_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
            });
            self.star_slots.push((buffer, bind));
        }
        if let Some((buffer, _)) = self.star_slots.get(self.star_used) {
            queue.write_buffer(buffer, 0, bytes);
        }
        slot.set(self.star_used);
        self.star_used += 1;
    }

    fn ensure(
        &mut self,
        device: &wgpu::Device,
        owner: &Weak<()>,
        width: u32,
        height: u32,
        place: [f32; 4],
        publish: bool,
    ) {
        let key = owner.as_ptr() as usize;
        if let Some(target) = self.targets.iter_mut().find(|target| target.owner.as_ptr() as usize == key) {
            target.place = place;
            target.publish = publish;
            if target.width == width && target.height == height {
                return;
            }
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-sora"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let blit = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-sora-blit"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let target = Target { owner: owner.clone(), width, height, texture, view, blit, place, publish };
        if let Some(slot) = self.targets.iter_mut().find(|target| target.owner.as_ptr() as usize == key) {
            *slot = target;
        } else {
            self.targets.push(target);
        }
    }
}

impl ShaderPrimitive for Pass {
    type Pipeline = Gpu;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        let scale = viewport.scale_factor();
        let (_ratio, width, height) = shader_budget(self.css[0], self.css[1], scale);
        let mut uniforms = self.uniforms;
        uniforms.resolution = [width, height];
        pipeline.write(device, queue, &self.slot, bytemuck::bytes_of(&uniforms));
        let place = [bounds.x * scale, bounds.y * scale, bounds.width * scale, bounds.height * scale];
        pipeline.ensure(
            device,
            &self.owner,
            width.round().max(1.0) as u32,
            height.round().max(1.0) as u32,
            place,
            self.publish,
        );
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
        if clip.width == 0 || clip.height == 0 {
            return;
        }
        let key = self.owner.as_ptr() as usize;
        let Some(slot) = pipeline.targets.iter().find(|target| target.owner.as_ptr() as usize == key) else {
            return;
        };
        let place = slot.place;
        if place[2] < 1.0 || place[3] < 1.0 {
            return;
        }
        let clear = if self.uniforms.daylight > 0.5 { wgpu::Color::WHITE } else { wgpu::Color::BLACK };
        let view = slot.view.clone();
        let blit = slot.blit.clone();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("folia-sora"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if !self.blank
                && let Some((_, bind)) = pipeline.star_slots.get(self.slot.get())
            {
                pass.set_pipeline(&pipeline.stars);
                pass.set_bind_group(0, bind, &[]);
                pass.draw(0..6, 0..PARTICLES);
            }
        }
        if self.publish {
            let texture = slot.texture.clone();
            let (width, height) = (slot.width, slot.height);
            let mut uniforms = self.uniforms;
            uniforms.resolution = [width as f32, height as f32];
            folia_render::copy_underlay(
                &pipeline.device,
                encoder,
                &texture,
                width,
                height,
                picture_key(&uniforms, self.blank),
            );
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-sora-blit"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_viewport(place[0], place[1], place[2], place[3], 0.0, 1.0);
        pass.set_scissor_rect(clip.x, clip.y, clip.width, clip.height);
        pass.set_pipeline(&pipeline.blit);
        pass.set_bind_group(0, &blit, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn picture_key(uniforms: &Uniforms, blank: bool) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let bytes = if blank { bytemuck::bytes_of(&uniforms.daylight) } else { bytemuck::bytes_of(uniforms) };
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash ^= uniforms.resolution[0].to_bits() as u64;
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    hash ^ uniforms.resolution[1].to_bits() as u64
}

impl shader::Pipeline for Gpu {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let stars_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-sora"),
            source: wgpu::ShaderSource::Wgsl(STARS.into()),
        });
        let star_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-sora"),
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
        let star_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-sora"),
            bind_group_layouts: &[&star_layout],
            push_constant_ranges: &[],
        });
        let stars = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("folia-sora"),
            layout: Some(&star_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &stars_module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &stars_module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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
        let blit_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-sora-blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT.into()),
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-sora-blit"),
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
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-sora-blit"),
            bind_group_layouts: &[&blit_layout],
            push_constant_ranges: &[],
        });
        let blit = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("folia-sora-blit"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_module,
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-sora-blit"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        Self {
            device: device.clone(),
            stars,
            star_layout,
            star_slots: Vec::new(),
            star_used: 0,
            blit,
            blit_layout,
            sampler,
            targets: Vec::new(),
            format,
            _guard: folia_render::PublisherGuard::register(),
        }
    }

    fn trim(&mut self) {
        self.targets.retain(|target| target.owner.strong_count() > 0);
        #[cfg(feature = "dev")]
        LIVE_TARGETS.store(self.targets.len(), Ordering::Relaxed);
        self._guard.report(self.targets.iter().any(|target| target.publish));
        self.star_used = 0;
    }
}
