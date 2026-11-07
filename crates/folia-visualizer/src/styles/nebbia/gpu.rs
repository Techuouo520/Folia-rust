// The nebbia passes, as the original nebbiaGl.ts: wiped segments into a half-size R8 mask (max blend),
// drop sprites into two drop textures, then the fullscreen window shader that composites them.
// Pipelines live as long as the renderer; textures change with the size, instance buffers only grow.

use bytemuck::{Pod, Zeroable};
use iced::widget::shader::{self, Primitive as ShaderPrimitive};
use iced::{Rectangle, Size};

use folia_core::Theme;

use super::palette::palette;
use super::place::{self, Knobs, MASK_SCALE};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Uniforms {
    pub resolution: [f32; 2],
    pub mask_size: [f32; 2],
    pub time: f32,
    pub bus: f32,
    pub neon: f32,
    pub mist: f32,
    pub spot_i: f32,
    pub spot_r: f32,
    /// Physical pixels per CSS pixel; set by the renderer.
    pub scale: f32,
    /// 1 when the target stores sRGB and the shader must hand back linear color; set by the renderer.
    pub encode: f32,
    pub spot_tgt: [f32; 2],
    /// The two spotlight sources' x in scene units; both stand at y 0.595.
    pub spot_src: [f32; 2],
    pub sky0: [f32; 4],
    pub sky1: [f32; 4],
    pub la: [f32; 4],
    pub lb: [f32; 4],
    pub lc: [f32; 4],
    /// Up to eight breath ellipses, xy centre, zw radii. A non-positive radius is unused.
    pub lobes: [[f32; 4]; 8],
    /// Eight (shape, dense) pairs, two per vec4.
    pub lobe_sd: [[f32; 4]; 4],
}

/// One frame for the window shader. The segments and drops it draws wait in `place`'s buffers.
#[derive(Clone, Copy, Debug)]
pub struct Pass {
    pub uniforms: Uniforms,
}

/// One frame: lays out the song (cached), collects this frame's wipes and drops, fills the uniforms.
pub fn frame(
    lyrics: &folia_core::Lyrics,
    time: f64,
    size: Size,
    theme: &Theme,
    font_scale: f32,
    tuning: &crate::tuning::Values,
) -> Pass {
    super::ink::warm(lyrics);
    let list = super::tuning::manifest();
    let (width, height) = (size.width.max(1.0), size.height.max(1.0));
    let scene = super::scene::scene(lyrics, width, height, font_scale, tuning);
    let knobs = Knobs {
        stroke_width: tuning.number(list, "strokeWidth"),
        write_speed: tuning.number(list, "writeSpeed").max(0.4),
        refog: f64::from(tuning.number(list, "refog")).max(0.5),
        rain_hit: tuning.number(list, "rainHit"),
        rain_big: tuning.number(list, "rainBig"),
    };
    let drive = place::paint(&scene, time, width, height, &knobs);
    let colors = palette(theme);
    let aspect = width / height;
    let spot_x = tuning.number(list, "spotX");
    let pad = |rgb: [f32; 3]| [rgb[0], rgb[1], rgb[2], 0.0];
    let [sx, sy, sr, si] = drive.spot;
    Pass {
        uniforms: Uniforms {
            resolution: [width, height],
            mask_size: mask_size(width, height).map(|v| v as f32),
            time: time as f32,
            bus: tuning.number(list, "busSpeed"),
            neon: tuning.number(list, "neon"),
            mist: tuning.number(list, "mist"),
            spot_i: si * tuning.number(list, "spot"),
            spot_r: (sr / height * 0.75).max(0.02),
            scale: 1.0,
            encode: 0.0,
            spot_tgt: [sx / width * aspect, sy / height],
            spot_src: [spot_x * aspect, (1.0 - spot_x) * aspect],
            sky0: pad(colors.sky0),
            sky1: pad(colors.sky1),
            la: pad(colors.la),
            lb: pad(colors.lb),
            lc: pad(colors.lc),
            lobes: drive.lobes,
            lobe_sd: drive.lobe_sd,
        },
    }
}

fn mask_size(width: f32, height: f32) -> [u32; 2] {
    [(width * MASK_SCALE).round().max(1.0) as u32, (height * MASK_SCALE).round().max(1.0) as u32]
}

const INSTANCE: u64 = std::mem::size_of::<[f32; 6]>() as u64;

/// A growing instance buffer and how many instances it holds this frame.
struct Instances {
    buffer: wgpu::Buffer,
    capacity: u64,
    count: u32,
}

impl Instances {
    fn new(device: &wgpu::Device) -> Self {
        Self { buffer: instance_buffer(device, 256), capacity: 256, count: 0 }
    }

    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, items: &[[f32; 6]]) {
        let n = items.len() as u64;
        if n > self.capacity {
            self.capacity = n.next_power_of_two();
            self.buffer = instance_buffer(device, self.capacity);
        }
        if n > 0 {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(items));
        }
        self.count = n as u32;
    }
}

struct Targets {
    css: [u32; 2],
    mask: wgpu::TextureView,
    drops: wgpu::TextureView,
    field: wgpu::TextureView,
    bind: wgpu::BindGroup,
}

/// Every nebbia pass for one target format. The app draws through it; anything with a wgpu device can too.
pub struct Renderer {
    window: wgpu::RenderPipeline,
    mask: wgpu::RenderPipeline,
    drop: wgpu::RenderPipeline,
    window_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    glass_bind: wgpu::BindGroup,
    targets: Option<Targets>,
    segments: Instances,
    drops: Instances,
    field: Instances,
    field_key: u64,
    encode: bool,
    bounds: [f32; 4],
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let window_module = shader_module(device, include_str!("../../../shaders/nebbia/window.wgsl"));
        let glass_module = shader_module(device, include_str!("../../../shaders/nebbia/glass.wgsl"));
        let window_layout =
            layout(device, &[uniform_entry(0), texture_entry(1), sampler_entry(2), texture_entry(3), texture_entry(4)]);
        let glass_layout = layout(device, &[uniform_entry(0), texture_entry(1), sampler_entry(2)]);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-nebbia-uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-nebbia"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        let sprite = sprite_view(device, queue);
        let glass_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-nebbia-glass"),
            layout: &glass_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&sprite) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let max = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Max,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        // raindrop-fx's "smoother": screen-like on color, over on alpha.
        let smoother = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::OneMinusDst,
                dst_factor: wgpu::BlendFactor::OneMinusSrc,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let glass = Stage { device, layout: &glass_layout, module: &glass_module };
        Self {
            window: Stage { device, layout: &window_layout, module: &window_module }.pipeline(
                "vs",
                "fs",
                format,
                wgpu::BlendState::REPLACE,
                false,
            ),
            mask: glass.pipeline("vs_mask", "fs_mask", wgpu::TextureFormat::R8Unorm, max, true),
            drop: glass.pipeline("vs_drop", "fs_drop", wgpu::TextureFormat::Rgba8Unorm, smoother, true),
            window_layout,
            sampler,
            uniform,
            glass_bind,
            targets: None,
            segments: Instances::new(device),
            drops: Instances::new(device),
            field: Instances::new(device),
            field_key: 0,
            encode: format.is_srgb(),
            bounds: [0.0; 4],
            device: device.clone(),
            queue: queue.clone(),
            format,
        }
    }

    /// Uploads this frame. `bounds` is the drawing area in physical pixels, `scale` physical per CSS pixel.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, pass: &Pass, bounds: [f32; 4], scale: f32) {
        let uniforms = Uniforms { scale, encode: if self.encode { 1.0 } else { 0.0 }, ..pass.uniforms };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniforms));
        self.bounds = bounds;
        let [width, height] = uniforms.resolution;
        let css = [width.round().max(1.0) as u32, height.round().max(1.0) as u32];
        if self.targets.as_ref().is_none_or(|targets| targets.css != css) {
            self.targets = Some(self.targets_for(device, css, mask_size(width, height)));
        }
        place::with_paint(|paint| {
            self.segments.write(device, queue, &paint.segments);
            self.drops.write(device, queue, &paint.drops);
        });
        let field = &mut self.field;
        self.field_key = super::drops::field(width, height, self.field_key, |drops| field.write(device, queue, drops));
    }

    /// Draws the glass passes, then the window into `target` within the prepared bounds, clipped to `clip`.
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, clip: [u32; 4]) {
        let Some(targets) = self.targets.as_ref() else { return };
        self.instanced(encoder, &targets.mask, &self.mask, &self.segments);
        self.instanced(encoder, &targets.drops, &self.drop, &self.drops);
        self.instanced(encoder, &targets.field, &self.drop, &self.field);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-nebbia-window"),
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
        let [x, y, w, h] = self.bounds;
        pass.set_viewport(x, y, w, h, 0.0, 1.0);
        pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
        pass.set_pipeline(&self.window);
        pass.set_bind_group(0, &targets.bind, &[]);
        pass.draw(0..3, 0..1);
    }

    /// The same window pass, into the glass blur's quarter-resolution picture.
    fn paint_blur(&self, encoder: &mut wgpu::CommandEncoder, time: f32, window: [f32; 2]) {
        let stamp = (time * 30.0).round() as i32 as u64;
        let drew =
            folia_render::draw_source(&self.device, &self.queue, self.format, window, stamp, |view, width, height| {
                let Some(targets) = self.targets.as_ref() else { return };
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("folia-nebbia-blur"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
                pass.set_scissor_rect(0, 0, width, height);
                pass.set_pipeline(&self.window);
                pass.set_bind_group(0, &targets.bind, &[]);
                pass.draw(0..3, 0..1);
            });
        if drew {
            folia_render::blur_source(encoder);
        }
    }

    fn instanced(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        pipeline: &wgpu::RenderPipeline,
        items: &Instances,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-nebbia-glass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
        if items.count == 0 {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.glass_bind, &[]);
        pass.set_vertex_buffer(0, items.buffer.slice(..u64::from(items.count) * INSTANCE));
        pass.draw(0..4, 0..items.count);
    }

    fn targets_for(&self, device: &wgpu::Device, css: [u32; 2], mask: [u32; 2]) -> Targets {
        let mask = target_view(device, mask, wgpu::TextureFormat::R8Unorm);
        let drops = target_view(device, css, wgpu::TextureFormat::Rgba8Unorm);
        let field = target_view(device, css, wgpu::TextureFormat::Rgba8Unorm);
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-nebbia-window"),
            layout: &self.window_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&mask) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&drops) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&field) },
            ],
        });
        Targets { css, mask, drops, field, bind }
    }
}

struct Stage<'a> {
    device: &'a wgpu::Device,
    layout: &'a wgpu::BindGroupLayout,
    module: &'a wgpu::ShaderModule,
}

impl Stage<'_> {
    fn pipeline(
        &self,
        vs: &str,
        fs: &str,
        format: wgpu::TextureFormat,
        blend: wgpu::BlendState,
        instanced: bool,
    ) -> wgpu::RenderPipeline {
        const ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x2];
        let instances = [wgpu::VertexBufferLayout {
            array_stride: INSTANCE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }];
        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-nebbia"),
            bind_group_layouts: &[self.layout],
            push_constant_ranges: &[],
        });
        self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("folia-nebbia"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: self.module,
                entry_point: Some(vs),
                buffers: if instanced { &instances } else { &[] },
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: self.module,
                entry_point: Some(fs),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: if instanced {
                    wgpu::PrimitiveTopology::TriangleStrip
                } else {
                    wgpu::PrimitiveTopology::TriangleList
                },
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }
}

fn shader_module(device: &wgpu::Device, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("folia-nebbia"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

fn layout(device: &wgpu::Device, entries: &[wgpu::BindGroupLayoutEntry]) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("folia-nebbia"), entries })
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn instance_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("folia-nebbia-instances"),
        size: capacity * INSTANCE,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn target_view(device: &wgpu::Device, size: [u32; 2], format: wgpu::TextureFormat) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-nebbia-glass"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

/// The raindrop sprite with its mip chain; a 1x1 clear texel when the image cannot be read.
fn sprite_view(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let empty = [(1, 1, vec![0u8; 4])];
    let levels: &[(u32, u32, Vec<u8>)] = super::drops::sprite().map_or(&empty, |sprite| &sprite.levels);
    let (width, height, _) = &levels[0];
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("folia-nebbia-sprite"),
        size: wgpu::Extent3d { width: *width, height: *height, depth_or_array_layers: 1 },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip, (w, h, pixels)) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(*h) },
            wgpu::Extent3d { width: *w, height: *h, depth_or_array_layers: 1 },
        );
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

impl ShaderPrimitive for Pass {
    type Pipeline = NebbiaGpu;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        let scale = viewport.scale_factor();
        let physical = [bounds.x * scale, bounds.y * scale, bounds.width * scale, bounds.height * scale];
        pipeline.0.prepare(device, queue, self, physical, scale);
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        pipeline.0.render(encoder, target, [clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height]);
        let window = [self.uniforms.resolution[0], self.uniforms.resolution[1]];
        pipeline.0.paint_blur(encoder, self.uniforms.time, window);
    }
}

pub struct NebbiaGpu(Renderer);

impl shader::Pipeline for NebbiaGpu {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self(Renderer::new(device, queue, format))
    }
}
