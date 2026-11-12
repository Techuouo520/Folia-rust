// Per-widget textured background pass, sharing the screen image with the glass underlay.

use crate::{Ground, clock, monet_background};
use iced::widget::{image, shader};
use iced::{Element, Length, Rectangle, mouse};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};

#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Uniforms {
    size: [f32; 4],
    drift: [f32; 4],
    background: [f32; 4],
    primary: [f32; 4],
    accent: [f32; 4],
    layout: [f32; 4],
}

pub fn view<'a, Message: 'a>(ground: Ground<'a>) -> Element<'a, Message> {
    shader(Layer { ground, marker: std::marker::PhantomData }).width(Length::Fill).height(Length::Fill).into()
}

struct Layer<'a, Message> {
    ground: Ground<'a>,
    marker: std::marker::PhantomData<Message>,
}

pub struct State {
    owner: Arc<AtomicBool>,
    effect: Cell<usize>,
    origin: Cell<f64>,
    drift: monet_background::Drift,
}
impl Default for State {
    fn default() -> Self {
        Self {
            owner: Arc::new(AtomicBool::new(false)),
            effect: Cell::new(usize::MAX),
            origin: Cell::new(0.0),
            drift: monet_background::Drift::default(),
        }
    }
}

impl<Message> shader::Program<Message> for Layer<'_, Message> {
    type State = State;
    type Primitive = Pass;
    fn update(
        &self,
        state: &mut State,
        _event: &iced::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<shader::Action<Message>> {
        let effect =
            if self.ground.mode == crate::Background::Nomand { 1 + self.ground.nomand.effect.index() } else { 0 };
        (state.effect.get() != effect || !state.owner.load(Ordering::Acquire)).then(|| {
            shader::Action::request_redraw_at(std::time::Instant::now() + std::time::Duration::from_millis(16))
        })
    }
    fn draw(&self, state: &State, _cursor: mouse::Cursor, bounds: Rectangle) -> Pass {
        let effect =
            if self.ground.mode == crate::Background::Nomand { 1 + self.ground.nomand.effect.index() } else { 0 };
        if state.effect.replace(effect) != effect {
            state.owner.store(false, Ordering::Release);
        }
        let now = clock::wall_ms();
        if state.origin.get() == 0.0 {
            state.origin.set(now);
        }
        if self.ground.mode == crate::Background::Nomand {
            let aspect = match self.ground.picture {
                Some(image::Handle::Rgba { width, height, .. }) => *width as f32 / (*height).max(1) as f32,
                _ => 1.0,
            };
            return Pass {
                owner: Arc::downgrade(&state.owner),
                picture: self.ground.picture.cloned(),
                publish: self.ground.publish,
                effect: 1 + self.ground.nomand.effect.index(),
                uniforms: crate::nomand::uniforms(
                    self.ground.nomand,
                    self.ground.theme,
                    [bounds.width, bounds.height],
                    aspect,
                    self.ground.daylight,
                    self.ground.picture.is_some(),
                ),
            };
        }
        let tuning = self.ground.monet;
        let moving = self.ground.player && tuning.drift && tuning.drift_strength > 0.0;
        let drift = if moving {
            state.drift.at((now - state.origin.get()) / 1000.0, tuning.drift_strength)
        } else {
            [0.0, 0.0, 1.0]
        };
        let rgb =
            |rgb: folia_core::Rgb| [f32::from(rgb.r) / 255.0, f32::from(rgb.g) / 255.0, f32::from(rgb.b) / 255.0, 1.0];
        Pass {
            owner: Arc::downgrade(&state.owner),
            picture: self.ground.picture.cloned(),
            publish: self.ground.publish,
            effect: 0,
            uniforms: {
                let values = Uniforms {
                    size: [bounds.width, bounds.height, 1920.0, 1080.0],
                    drift: [drift[0], drift[1], drift[2], tuning.overlay],
                    background: rgb(self.ground.theme.background),
                    primary: rgb(self.ground.theme.primary),
                    accent: rgb(self.ground.theme.accent),
                    layout: [
                        f32::from(tuning.half),
                        tuning.offset,
                        f32::from(self.ground.daylight),
                        f32::from(self.ground.picture.is_some()),
                    ],
                };
                let mut packed = [[0.0; 4]; 72];
                packed[..6].copy_from_slice(&bytemuck::cast::<Uniforms, [[f32; 4]; 6]>(values));
                packed
            },
        }
    }
}

#[derive(Debug)]
pub struct Pass {
    owner: Weak<AtomicBool>,
    picture: Option<image::Handle>,
    uniforms: [[f32; 4]; 72],
    effect: usize,
    publish: bool,
}
struct Target {
    owner: Weak<AtomicBool>,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    picture: Option<image::Handle>,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    blit: wgpu::BindGroup,
    bounds: [f32; 4],
    publish: bool,
    painted: AtomicU64,
    effect: usize,
}
pub struct Gpu {
    device: wgpu::Device,
    pipelines: Vec<Option<folia_render::Prepared<wgpu::RenderPipeline>>>,
    pipeline_owners: Vec<Arc<AtomicBool>>,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    noise: wgpu::TextureView,
    noise_sampler: wgpu::Sampler,
    blit: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    targets: Vec<Target>,
    format: wgpu::TextureFormat,
    _publisher: folia_render::PublisherGuard,
}

impl Gpu {
    fn target(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, pass: &Pass, bounds: [f32; 4]) -> &mut Target {
        let index = self.targets.iter().position(|target| target.owner.ptr_eq(&pass.owner));
        let budget =
            if pass.effect == 0 { 1.0 } else { ((1920.0 * 1080.0) / (bounds[2] * bounds[3]).max(1.0)).sqrt().min(1.0) };
        let width = (bounds[2] * budget).ceil().max(1.0) as u32;
        let height = (bounds[3] * budget).ceil().max(1.0) as u32;
        let unchanged = index.is_some_and(|i| {
            let target = &self.targets[i];
            target.picture.as_ref().map(image::Handle::id) == pass.picture.as_ref().map(image::Handle::id)
                && target.texture.width() == width
                && target.texture.height() == height
        });
        if !unchanged {
            let (w, h, pixels): (u32, u32, &[u8]) = match pass.picture.as_ref() {
                Some(image::Handle::Rgba { width, height, pixels, .. }) => (*width, *height, pixels.as_ref()),
                _ => (1, 1, &[0, 0, 0, 0]),
            };
            let source = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("folia-background-image"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                source.as_image_copy(),
                pixels,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
                source.size(),
            );
            let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
            let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("folia-image-background"),
                size: (72 * 16) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("folia-image-background"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&source_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.noise) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.noise_sampler) },
                ],
            });
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("folia-image-composite"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let blit = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("folia-image-composite"),
                layout: &self.blit_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            let target = Target {
                owner: pass.owner.clone(),
                uniform,
                bind,
                picture: pass.picture.clone(),
                texture,
                view,
                blit,
                bounds,
                publish: pass.publish,
                painted: AtomicU64::new(0),
                effect: pass.effect,
            };
            if let Some(i) = index {
                self.targets[i] = target;
            } else {
                self.targets.push(target);
            }
        }
        let i = index.unwrap_or(self.targets.len() - 1);
        let target = &mut self.targets[i];
        target.bounds = bounds;
        target.publish = pass.publish;
        target.effect = pass.effect;
        let mut uniforms = pass.uniforms;
        if pass.effect != 0 {
            uniforms[1][0] = width as f32 / uniforms[0][0].max(1.0);
            uniforms[0][0] = width as f32;
            uniforms[0][1] = height as f32;
        }
        queue.write_buffer(&target.uniform, 0, bytemuck::cast_slice(&uniforms));
        target
    }
}

impl shader::Primitive for Pass {
    type Pipeline = Gpu;
    fn prepare(
        &self,
        pipeline: &mut Gpu,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        if pipeline.pipelines[self.effect].is_none() {
            let sources = [
                include_str!("../shaders/backgrounds/monet.wgsl"),
                include_str!("../shaders/nomand/image-dithering.wgsl"),
                include_str!("../shaders/nomand/fluted-glass.wgsl"),
                include_str!("../shaders/nomand/paper-texture.wgsl"),
                include_str!("../shaders/nomand/halftone-dots.wgsl"),
                include_str!("../shaders/nomand/lens-distortion.wgsl"),
            ];
            let (device, format, layout, effect) =
                (device.clone(), pipeline.format, pipeline.layout.clone(), self.effect);
            pipeline.pipelines[effect] =
                Some(folia_render::Prepared::new(Arc::downgrade(&pipeline.pipeline_owners[effect]), move || {
                    make_pipeline(&device, format, &layout, sources[effect], if effect == 0 { "fs" } else { "main" })
                }));
        }
        if let Some(effect) = &mut pipeline.pipelines[self.effect] {
            effect.poll();
            if let Some(owner) = self.owner.upgrade() {
                owner.store(pipeline.pipeline_owners[self.effect].load(Ordering::Acquire), Ordering::Release);
            }
        }
        let scale = viewport.scale_factor();
        pipeline.target(
            device,
            queue,
            self,
            [bounds.x * scale, bounds.y * scale, bounds.width * scale, bounds.height * scale],
        );
    }
    fn draw(&self, _pipeline: &Gpu, _pass: &mut wgpu::RenderPass<'_>) -> bool {
        false
    }
    fn render(
        &self,
        pipeline: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        screen: &wgpu::TextureView,
        clip: &Rectangle<u32>,
    ) {
        let Some(effect) = pipeline.pipelines[self.effect].as_ref().and_then(folia_render::Prepared::get) else {
            return;
        };
        let Some(target) = pipeline.targets.iter().find(|target| target.owner.ptr_eq(&self.owner)) else { return };
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.picture.as_ref().map(image::Handle::id).hash(&mut hash);
        bytemuck::cast_slice::<[f32; 4], u8>(&self.uniforms).hash(&mut hash);
        self.effect.hash(&mut hash);
        let content = hash.finish().max(1);
        if target.painted.load(Ordering::Relaxed) != content {
            let mut pass = render_pass(encoder, &target.view, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT));
            pass.set_pipeline(effect);
            pass.set_bind_group(0, &target.bind, &[]);
            pass.draw(0..if self.effect == 0 { 3 } else { 6 }, 0..1);
            target.painted.store(content, Ordering::Relaxed);
        }
        if self.publish {
            folia_render::copy_underlay(
                &pipeline.device,
                encoder,
                &target.texture,
                target.texture.width(),
                target.texture.height(),
                content,
            );
        }
        let mut pass = render_pass(encoder, screen, wgpu::LoadOp::Load);
        let [x, y, w, h] = target.bounds;
        pass.set_viewport(x, y, w, h, 0.0, 1.0);
        pass.set_scissor_rect(clip.x, clip.y, clip.width, clip.height);
        pass.set_pipeline(&pipeline.blit);
        pass.set_bind_group(0, &target.blit, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn render_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("folia-image-background"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

impl shader::Pipeline for Gpu {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sample = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-image-background"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture(1),
                sample(2),
                texture(3),
                sample(4),
            ],
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-image-blit"),
            entries: &[texture(0), sample(1)],
        });
        let pipelines = (0..6).map(|_| None).collect();
        let blit = make_pipeline(device, format, &blit_layout, BLIT, "fs");
        let noise = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("paper-noise"),
            size: wgpu::Extent3d { width: 128, height: 128, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            noise.as_image_copy(),
            include_bytes!("../../../assets/paper-shaders/noise.rgba"),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(128 * 4), rows_per_image: Some(128) },
            noise.size(),
        );
        let noise = noise.create_view(&wgpu::TextureViewDescriptor::default());
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        Self {
            device: device.clone(),
            pipelines,
            pipeline_owners: (0..6).map(|_| Arc::new(AtomicBool::new(false))).collect(),
            layout,
            sampler,
            noise,
            noise_sampler,
            blit,
            blit_layout,
            targets: Vec::new(),
            format,
            _publisher: folia_render::PublisherGuard::register(),
        }
    }
    fn trim(&mut self) {
        self.targets.retain(|target| target.owner.strong_count() > 0);
        for (effect, pipeline) in self.pipelines.iter_mut().enumerate() {
            if pipeline.is_some() && !self.targets.iter().any(|target| target.effect == effect) {
                *pipeline = None;
                self.pipeline_owners[effect] = Arc::new(AtomicBool::new(false));
            }
        }
        self._publisher.report(self.targets.iter().any(|target| target.publish));
    }
}

fn make_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    bind: &wgpu::BindGroupLayout,
    source: &str,
    entry: &str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("folia-image-background"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[bind],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("folia-image-background"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(entry),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

const BLIT: &str = r#"
struct Vertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@vertex fn vs(@builtin(vertex_index) index: u32) -> Vertex {
    var corners = array<vec2<f32>, 3>(vec2<f32>(-1., -1.), vec2<f32>(3., -1.), vec2<f32>(-1., 3.));
    let p = corners[index];
    return Vertex(vec4<f32>(p, 0., 1.), vec2<f32>(p.x * .5 + .5, .5 - p.y * .5));
}
@fragment fn fs(in: Vertex) -> @location(0) vec4<f32> { return textureSample(image, image_sampler, in.uv); }
"#;
