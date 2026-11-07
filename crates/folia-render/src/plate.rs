// Rounded fill with a CSS box-shadow and a 1px ring. Each plate in a frame keeps its own uniforms.

use iced::widget::shader::{self, Primitive as ShaderPrimitive};
use iced::{Rectangle, mouse};

use crate::backdrop;
use crate::blur::levels_for_radius;
use crate::pipeline::{Fullscreen, Slot};

const SHADER: &str = concat!(include_str!("../shaders/fullscreen.wgsl"), include_str!("../shaders/plate.wgsl"));

/// sRGB byte channel to linear. The exact inverse of [`to_srgb_channel`].
pub fn to_linear_channel(encoded: f32) -> f32 {
    if encoded <= 0.04045 { encoded / 12.92 } else { ((encoded + 0.055) / 1.055).powf(2.4) }
}

/// Linear channel to sRGB. The exact inverse of [`to_linear_channel`].
pub fn to_srgb_channel(linear: f32) -> f32 {
    let linear = linear.max(0.0);
    if linear <= 0.0031308 { linear * 12.92 } else { 1.055 * linear.powf(1.0 / 2.4) - 0.055 }
}

/// CSS `saturate()` in sRGB: the SVG matrix with weights 0.213, 0.715 and 0.072.
pub fn css_saturate(rgb: [f32; 3], amount: f32) -> [f32; 3] {
    let s = amount;
    let dot = |row: [f32; 3]| row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2];
    [
        dot([0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s]),
        dot([0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s]),
        dot([0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s]),
    ]
}

/// Straight-alpha source-over in one color space. `src` is rgba.
pub fn source_over(dst: [f32; 3], src: [f32; 4]) -> [f32; 3] {
    let a = src[3];
    [src[0] * a + dst[0] * (1.0 - a), src[1] * a + dst[1] * (1.0 - a), src[2] * a + dst[2] * (1.0 - a)]
}

/// Browser glass: `saturate()` the sRGB backdrop, then paint the element's background over it.
pub fn glass_over(backdrop: [f32; 3], fill: [f32; 4], saturate: f32) -> [f32; 3] {
    source_over(css_saturate(backdrop, saturate), fill)
}

/// Builds both plate pipelines and waits until the device reports shader errors.
pub fn compile_plate(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) {
    let _gpu = <PlateGpu as shader::Pipeline>::new(device, queue, format);
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
}

/// A rounded rectangle of `fill` at `fill_origin` / `fill_size` with `0 shadow_y shadow_blur shadow_spread shadow`
/// and `0 0 0 1px ring` around it. A non-positive `fill_size.y` uses the widget height. The widget must be
/// large enough to hold the shadow; it fades out over the last 1.5px of the widget.
/// `backdrop` is the CSS `blur()` radius in pixels (0 keeps the flat fill). `fade` multiplies the whole
/// plate, blur included, so a closing panel does not leave the blur behind. `saturate` is the CSS
/// `backdrop-saturate` multiplier (1 leaves the blur's colour).
#[derive(Debug, Clone, Copy)]
pub struct Plate {
    pub radius: f32,
    pub fill_origin: [f32; 2],
    pub fill_size: [f32; 2],
    pub fill: [f32; 4],
    pub shadow: [f32; 4],
    pub shadow_blur: f32,
    pub shadow_y: f32,
    pub shadow_spread: f32,
    pub ring: [f32; 4],
    pub backdrop: f32,
    pub fade: f32,
    pub saturate: f32,
}

impl<Message> shader::Program<Message> for Plate {
    type State = ();
    type Primitive = PlatePass;

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        mouse::Interaction::None
    }

    fn draw(&self, _state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        let resolution = [bounds.width.max(1.0), bounds.height.max(1.0)];
        let mut size = self.fill_size;
        if size[1] <= 0.0 {
            size[1] = resolution[1];
        }
        let uniforms = Uniforms {
            resolution,
            encode: 0.0,
            radius: self.radius,
            origin: self.fill_origin,
            size,
            fill: self.fill,
            shadow: self.shadow,
            shadow_blur: self.shadow_blur,
            shadow_y: self.shadow_y,
            shadow_spread: self.shadow_spread,
            _pad: 0.0,
            ring: self.ring,
            widget: [0.0, 0.0],
            window: [1.0, 1.0],
            fade: self.fade,
            saturate: self.saturate,
            backdrop: self.backdrop,
            level: levels_for_radius(self.backdrop) as f32,
        };
        PlatePass(uniforms, Slot::default())
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    encode: f32,
    radius: f32,
    origin: [f32; 2],
    size: [f32; 2],
    fill: [f32; 4],
    shadow: [f32; 4],
    shadow_blur: f32,
    shadow_y: f32,
    shadow_spread: f32,
    _pad: f32,
    ring: [f32; 4],
    widget: [f32; 2],
    window: [f32; 2],
    fade: f32,
    saturate: f32,
    backdrop: f32,
    level: f32,
}

#[derive(Debug)]
pub struct PlatePass(Uniforms, Slot);

struct BlurSlot {
    buffer: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

/// `render` only receives a shared pipeline, so the sample group lives behind a lock.
struct CachedBlur {
    epoch: u64,
    level: u32,
    bind: wgpu::BindGroup,
}

pub struct PlateGpu {
    device: wgpu::Device,
    fullscreen: Fullscreen,
    blur_pipe: wgpu::RenderPipeline,
    uniform_layout: wgpu::BindGroupLayout,
    blur_layout: wgpu::BindGroupLayout,
    blur_sampler: wgpu::Sampler,
    blur_slots: Vec<BlurSlot>,
    blur_used: usize,
    view_cache: parking_lot::Mutex<Vec<Option<CachedBlur>>>,
    srgb_target: bool,
    rects: Vec<[f32; 4]>,
}

impl shader::Pipeline for PlateGpu {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fs_plate_blur"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fs_plate_blur"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-plate-blur"),
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
            label: Some("fs_plate_blur"),
            bind_group_layouts: &[&uniform_layout, &blur_layout],
            push_constant_ranges: &[],
        });
        let blur_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fs_plate_blur"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_plate_blur"),
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
        Self {
            device: device.clone(),
            fullscreen: Fullscreen::new(device, format, "fs_plate", SHADER, "fs_plate", size_of::<Uniforms>()),
            blur_pipe,
            uniform_layout,
            blur_layout,
            blur_sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("folia-plate-blur"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                ..wgpu::SamplerDescriptor::default()
            }),
            blur_slots: Vec::new(),
            blur_used: 0,
            view_cache: parking_lot::Mutex::new(Vec::new()),
            srgb_target: format.is_srgb(),
            rects: Vec::new(),
        }
    }

    fn trim(&mut self) {
        self.fullscreen.trim();
        self.blur_used = 0;
        self.rects.clear();
        backdrop::trim();
        if backdrop::resident_bytes() == 0 {
            self.view_cache.lock().clear();
        }
    }
}

impl PlateGpu {
    fn write_blur(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, slot: &Slot, uniforms: &Uniforms) {
        if self.blur_used == self.blur_slots.len() {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("fs_plate_blur"),
                size: size_of::<Uniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("fs_plate_blur"),
                layout: &self.uniform_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
            });
            self.blur_slots.push(BlurSlot { buffer, bind });
        }
        if let Some(slot_gpu) = self.blur_slots.get(self.blur_used) {
            queue.write_buffer(&slot_gpu.buffer, 0, bytemuck::bytes_of(uniforms));
        }
        while self.rects.len() <= self.blur_used {
            self.rects.push([0.0; 4]);
        }
        slot.set(self.blur_used);
        self.blur_used += 1;
    }
}

/// Reuses the sample group while the pyramid still owns the same blurred pictures.
fn cached_blur(pipeline: &PlateGpu, index: usize, level: u32, view: &wgpu::TextureView) -> wgpu::BindGroup {
    let epoch = backdrop::output_epoch();
    if let Some(hit) = pipeline.view_cache.lock().get(index).and_then(|slot| {
        slot.as_ref().filter(|hit| hit.epoch == epoch && hit.level == level).map(|hit| hit.bind.clone())
    }) {
        return hit;
    }
    let bind = pipeline.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("folia-plate-blur"),
        layout: &pipeline.blur_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&pipeline.blur_sampler) },
        ],
    });
    let mut cache = pipeline.view_cache.lock();
    if cache.len() <= index {
        cache.resize_with(index + 1, || None);
    }
    if let Some(slot) = cache.get_mut(index) {
        *slot = Some(CachedBlur { epoch, level, bind: bind.clone() });
    }
    bind
}

impl ShaderPrimitive for PlatePass {
    type Pipeline = PlateGpu;

    fn prepare(
        &self,
        pipeline: &mut PlateGpu,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        let mut uniforms = Uniforms { encode: if pipeline.srgb_target { 1.0 } else { 0.0 }, ..self.0 };
        uniforms.widget = [bounds.x, bounds.y];
        let window = backdrop::window();
        if window[0] > 1.0 {
            uniforms.window = window;
        }
        if uniforms.backdrop > 0.0 {
            backdrop::claim(uniforms.level as u32);
            let scale = viewport.scale_factor();
            pipeline.write_blur(device, queue, &self.1, &uniforms);
            let index = self.1.get();
            if let Some(rect) = pipeline.rects.get_mut(index) {
                *rect = [bounds.x * scale, bounds.y * scale, bounds.width * scale, bounds.height * scale];
            }
        } else {
            pipeline.fullscreen.write(device, queue, &self.1, bytemuck::bytes_of(&uniforms));
        }
    }

    fn draw(&self, pipeline: &PlateGpu, pass: &mut wgpu::RenderPass<'_>) -> bool {
        if self.0.backdrop > 0.0 {
            return false;
        }
        pipeline.fullscreen.draw(pass, &self.1);
        true
    }

    fn render(
        &self,
        pipeline: &PlateGpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip: &Rectangle<u32>,
    ) {
        if self.0.backdrop <= 0.0 {
            return;
        }
        let level = self.0.level as u32;
        let Some(view) = backdrop::blurred(level) else { return };
        let index = self.1.get();
        let Some(slot) = pipeline.blur_slots.get(index) else { return };
        let rect = pipeline.rects.get(index).copied().unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let bind = cached_blur(pipeline, index, level, &view);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-plate"),
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
        pass.set_viewport(rect[0], rect[1], rect[2].max(1.0), rect[3].max(1.0), 0.0, 1.0);
        pass.set_scissor_rect(clip.x, clip.y, clip.width.max(1), clip.height.max(1));
        pass.set_pipeline(&pipeline.blur_pipe);
        pass.set_bind_group(0, &slot.bind, &[]);
        pass.set_bind_group(1, &bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
