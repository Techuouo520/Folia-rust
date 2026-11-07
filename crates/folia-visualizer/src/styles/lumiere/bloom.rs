// Multi-level dual bloom. Textures and bind groups are rebuilt only when the picture size changes.

use iced::Rectangle;

const SHADER: &str = include_str!("../../../shaders/lumiere/bloom.wgsl");
const MAX_LEVELS: usize = 6;

#[derive(Clone, Copy)]
pub struct Preset {
    pub strength: f32,
    pub threshold: f32,
    pub knee: f32,
    pub levels: u32,
    pub spread: f32,
}

pub const GRAPHICS: Preset = Preset { strength: 1.5, threshold: 0.42, knee: 0.3, levels: 6, spread: 0.9 };
pub const TEXT: Preset = Preset { strength: 1.15, threshold: 0.12, knee: 0.2, levels: 5, spread: 0.95 };

struct GpuTex {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl GpuTex {
    fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }
}

struct Down {
    image: GpuTex,
    uniform: wgpu::Buffer,
    texel: [f32; 2],
    bind: wgpu::BindGroup,
}

struct Up {
    image: GpuTex,
    uniform: wgpu::Buffer,
    texel: [f32; 2],
    from_down: wgpu::BindGroup,
    from_up: Option<wgpu::BindGroup>,
    add: wgpu::BindGroup,
}

pub struct Filter {
    down_pipe: wgpu::RenderPipeline,
    up_pipe: wgpu::RenderPipeline,
    combine_pipe: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    down_layout: wgpu::BindGroupLayout,
    sample_layout: wgpu::BindGroupLayout,
    add_layout: wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    scene: Option<GpuTex>,
    scene_bind: Option<wgpu::BindGroup>,
    downs: Vec<Down>,
    ups: Vec<Up>,
    combine_uniform: wgpu::Buffer,
    zero: wgpu::Buffer,
    combine_up: Option<wgpu::BindGroup>,
    combine_down: Option<wgpu::BindGroup>,
    passthrough: Option<wgpu::BindGroup>,
    active: u32,
}

impl Filter {
    /// `output` blends the result onto the target: `REPLACE` for the opaque graphics, premultiplied for the text.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, output: wgpu::BlendState) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-lumiere-bloom"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-lumiere-bloom"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        let sample_layout = sample_layout(device);
        let down_layout = down_layout(device);
        let add_layout = add_layout(device);
        let replace = wgpu::BlendState::REPLACE;
        let down_pipe = pipeline(device, &module, "fs_down", &[&down_layout], format, replace);
        Self {
            down_pipe,
            up_pipe: pipeline(device, &module, "fs_up", &[&sample_layout, &add_layout], format, replace),
            combine_pipe: pipeline(device, &module, "fs_combine", &[&sample_layout, &add_layout], format, output),
            sampler,
            down_layout,
            sample_layout,
            add_layout,
            format,
            size: (0, 0),
            scene: None,
            scene_bind: None,
            downs: Vec::new(),
            ups: Vec::new(),
            combine_uniform: uniform_buffer(device, "folia-lumiere-bloom-combine"),
            zero: uniform_buffer(device, "folia-lumiere-bloom-zero"),
            combine_up: None,
            combine_down: None,
            passthrough: None,
            active: 0,
        }
    }

    pub fn scene(&self) -> Option<&wgpu::TextureView> {
        self.scene.as_ref().map(|image| &image.view)
    }

    /// Rebuilds the pyramid when `width`/`height` change, then writes this preset's uniforms.
    pub fn sync(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32, preset: &Preset) {
        if width < 1 || height < 1 {
            return;
        }
        if self.size != (width, height) || self.scene.is_none() {
            self.size = (width, height);
            self.rebuild(device, width, height);
        }
        self.active = if preset.strength <= 0.0 {
            0
        } else {
            (preset.levels.clamp(1, MAX_LEVELS as u32) as usize).min(self.downs.len()) as u32
        };
        for (index, down) in self.downs.iter().enumerate() {
            let prefilter = if index == 0 { 1.0 } else { 0.0 };
            let threshold = if index == 0 { preset.threshold } else { 0.0 };
            let knee = if index == 0 { preset.knee } else { 0.0 };
            queue.write_buffer(&down.uniform, 0, &pack(down.texel, threshold, knee, prefilter));
        }
        for up in &self.ups {
            queue.write_buffer(&up.uniform, 0, &pack(up.texel, preset.spread, 0.0, 0.0));
        }
        queue.write_buffer(&self.combine_uniform, 0, &pack([0.0, 0.0], preset.strength, 0.0, 0.0));
    }

    /// Drops the scene and pyramid textures; the next `sync` builds them again.
    pub fn release(&mut self) {
        self.size = (0, 0);
        self.scene = None;
        self.scene_bind = None;
        self.downs.clear();
        self.ups.clear();
        self.combine_up = None;
        self.combine_down = None;
        self.passthrough = None;
    }

    /// Runs the pyramid and writes the combined picture into `target` at the given pixel rect.
    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        let (Some(scene), Some(base)) = (&self.scene, &self.scene_bind) else { return };
        if width == 0 || height == 0 || scene.size().0 == 0 {
            return;
        }
        let levels = self.active as usize;
        if levels == 0 {
            if let Some(glow) = &self.passthrough {
                self.blit(encoder, target, Rectangle { x, y, width, height }, base, glow);
            }
            return;
        }
        for down in self.downs.iter().take(levels) {
            self.full(encoder, &self.down_pipe, &down.image.view, &down.bind, None);
        }
        let glow = if levels == 1 {
            self.combine_down.as_ref()
        } else {
            let top = levels - 2;
            if let Some(up) = self.ups.get(top) {
                self.full(encoder, &self.up_pipe, &up.image.view, &up.from_down, Some(&up.add));
            }
            for index in (0..top).rev() {
                if let Some(up) = self.ups.get(index)
                    && let Some(sample) = &up.from_up
                {
                    self.full(encoder, &self.up_pipe, &up.image.view, sample, Some(&up.add));
                }
            }
            self.combine_up.as_ref()
        };
        if let Some(glow) = glow {
            self.blit(encoder, target, Rectangle { x, y, width, height }, base, glow);
        }
    }

    fn rebuild(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let scene = make_image(device, self.format, width, height);
        let downs = build_downs(device, &self.down_layout, &self.sampler, self.format, &scene, width, height);
        let ups = build_ups(device, &self.sample_layout, &self.add_layout, &self.sampler, self.format, &downs);
        self.scene_bind = Some(sample_bind(device, &self.sample_layout, &self.sampler, &scene.view));
        self.combine_up =
            ups.first().map(|up| add_bind(device, &self.add_layout, &up.image.view, &self.combine_uniform));
        self.combine_down =
            downs.first().map(|down| add_bind(device, &self.add_layout, &down.image.view, &self.combine_uniform));
        self.passthrough = Some(add_bind(device, &self.add_layout, &scene.view, &self.zero));
        self.scene = Some(scene);
        self.downs = downs;
        self.ups = ups;
    }

    fn full(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipe: &wgpu::RenderPipeline,
        target: &wgpu::TextureView,
        group0: &wgpu::BindGroup,
        group1: Option<&wgpu::BindGroup>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-lumiere-bloom"),
            color_attachments: &[Some(color_attachment(target, true))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(pipe);
        pass.set_bind_group(0, group0, &[]);
        if let Some(group) = group1 {
            pass.set_bind_group(1, group, &[]);
        }
        pass.draw(0..3, 0..1);
    }

    fn blit(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        rect: Rectangle<u32>,
        base: &wgpu::BindGroup,
        glow: &wgpu::BindGroup,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-lumiere-bloom-out"),
            color_attachments: &[Some(color_attachment(target, false))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_viewport(rect.x as f32, rect.y as f32, rect.width as f32, rect.height as f32, 0.0, 1.0);
        pass.set_scissor_rect(rect.x, rect.y, rect.width, rect.height);
        pass.set_pipeline(&self.combine_pipe);
        pass.set_bind_group(0, base, &[]);
        pass.set_bind_group(1, glow, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn build_downs(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    format: wgpu::TextureFormat,
    scene: &GpuTex,
    width: u32,
    height: u32,
) -> Vec<Down> {
    let mut images = Vec::new();
    let mut texels = Vec::new();
    let mut prev_w = width;
    let mut prev_h = height;
    while images.len() < MAX_LEVELS {
        let Some((w, h)) = half(prev_w, prev_h) else { break };
        images.push(make_image(device, format, w, h));
        texels.push([1.0 / prev_w as f32, 1.0 / prev_h as f32]);
        prev_w = w;
        prev_h = h;
    }
    let mut uniforms = Vec::new();
    let mut binds = Vec::new();
    for index in 0..images.len() {
        let uniform = uniform_buffer(device, "folia-lumiere-bloom-down");
        let source = if index == 0 { &scene.view } else { &images[index - 1].view };
        binds.push(down_bind(device, layout, sampler, source, &uniform));
        uniforms.push(uniform);
    }
    let mut downs = Vec::with_capacity(images.len());
    for ((image, texel), (uniform, bind)) in images.into_iter().zip(texels).zip(uniforms.into_iter().zip(binds)) {
        downs.push(Down { image, uniform, texel, bind });
    }
    downs
}

fn build_ups(
    device: &wgpu::Device,
    sample_layout: &wgpu::BindGroupLayout,
    add_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    format: wgpu::TextureFormat,
    downs: &[Down],
) -> Vec<Up> {
    if downs.len() < 2 {
        return Vec::new();
    }
    let mut images = Vec::new();
    for down in downs.iter().take(downs.len() - 1) {
        let (w, h) = down.image.size();
        images.push(make_image(device, format, w, h));
    }
    let mut ups = Vec::with_capacity(images.len());
    for (index, image) in images.into_iter().enumerate() {
        let smaller = &downs[index + 1].image;
        let (sw, sh) = smaller.size();
        let uniform = uniform_buffer(device, "folia-lumiere-bloom-up");
        let from_down = sample_bind(device, sample_layout, sampler, &smaller.view);
        let add = add_bind(device, add_layout, &downs[index].image.view, &uniform);
        ups.push(Up { image, uniform, texel: [1.0 / sw as f32, 1.0 / sh as f32], from_down, from_up: None, add });
    }
    let continued: Vec<wgpu::BindGroup> = (0..ups.len().saturating_sub(1))
        .map(|index| sample_bind(device, sample_layout, sampler, &ups[index + 1].image.view))
        .collect();
    for (index, bind) in continued.into_iter().enumerate() {
        ups[index].from_up = Some(bind);
    }
    ups
}

fn half(width: u32, height: u32) -> Option<(u32, u32)> {
    let w = width / 2;
    let h = height / 2;
    if w < 2 || h < 2 { None } else { Some((w, h)) }
}

fn pack(texel: [f32; 2], a: f32, b: f32, c: f32) -> [u8; 32] {
    let mut out = [0_u8; 32];
    let values = [texel[0], texel[1], 0.0, 0.0, a, b, c, 0.0];
    for (index, value) in values.iter().enumerate() {
        let start = index * 4;
        out[start..start + 4].copy_from_slice(&value.to_le_bytes());
    }
    out
}

fn make_image(device: &wgpu::Device, format: wgpu::TextureFormat, width: u32, height: u32) -> GpuTex {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("folia-lumiere-bloom"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    GpuTex { texture, view }
}

fn uniform_buffer(device: &wgpu::Device, label: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: 32,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn sample_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("folia-lumiere-bloom-sample"),
        entries: &[texture_entry(0), sampler_entry(1)],
    })
}

fn down_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("folia-lumiere-bloom-down"),
        entries: &[texture_entry(0), sampler_entry(1), uniform_entry(2)],
    })
}

fn add_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("folia-lumiere-bloom-add"),
        entries: &[texture_entry(0), uniform_entry(1)],
    })
}

fn pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    entry: &str,
    groups: &[&wgpu::BindGroupLayout],
    format: wgpu::TextureFormat,
    blend: wgpu::BlendState,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(entry),
        bind_group_layouts: groups,
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(entry),
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

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn sample_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("folia-lumiere-bloom-sample"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

fn down_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    view: &wgpu::TextureView,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("folia-lumiere-bloom-down"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: uniform.as_entire_binding() },
        ],
    })
}

fn add_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("folia-lumiere-bloom-add"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: uniform.as_entire_binding() },
        ],
    })
}

fn color_attachment(view: &wgpu::TextureView, clear: bool) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: if clear { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) } else { wgpu::LoadOp::Load },
            store: wgpu::StoreOp::Store,
        },
    }
}
