// Dual Kawase pyramid at a quarter of the window. Textures exist only while a glass plate is open.

const SHADER: &str = include_str!("../shaders/blur.wgsl");

/// One source pixel covers this many CSS pixels.
pub const SOURCE_DIVISOR: u32 = 4;
pub const MAX_LEVELS: u32 = 6;

/// CSS `blur(px)` (the Gaussian standard deviation) to dual-Kawase steps.
/// The source is already `1/SOURCE_DIVISOR` of the window, so the first step covers that many CSS pixels
/// and each further step doubles the support. The result is the smallest step whose support reaches `px`.
pub fn levels_for_radius(px: f32) -> u32 {
    if !px.is_finite() || px <= 0.0 {
        return 0;
    }
    let mut level = 1u32;
    let mut support = SOURCE_DIVISOR as f32;
    while support < px && level < MAX_LEVELS {
        level += 1;
        support *= 2.0;
    }
    level
}

/// RGBA bytes of the quarter-resolution source, the downsample chain, and one source-sized result per level.
pub fn pyramid_bytes(width: u32, height: u32, levels: u32) -> u64 {
    let (mut w, mut h) = source_size(width, height);
    let mut pixels = u64::from(w) * u64::from(h);
    for _ in 0..levels {
        let next = half(w, h);
        let Some((nw, nh)) = next else { break };
        pixels += u64::from(nw) * u64::from(nh);
        w = nw;
        h = nh;
    }
    let (sw, sh) = source_size(width, height);
    pixels += u64::from(levels.max(1)) * u64::from(sw) * u64::from(sh);
    pixels * 4
}

pub fn source_size(width: u32, height: u32) -> (u32, u32) {
    ((width / SOURCE_DIVISOR).max(1), (height / SOURCE_DIVISOR).max(1))
}

struct Image {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

struct Down {
    image: Image,
    bind: wgpu::BindGroup,
    /// Samples `image` for the upsample that reads this level.
    up_bind: wgpu::BindGroup,
}

struct Up {
    image: Image,
    /// Samples `image` after it has been written, for the next upsample.
    bind: wgpu::BindGroup,
}

pub struct Pyramid {
    down_pipe: wgpu::RenderPipeline,
    up_pipe: wgpu::RenderPipeline,
    copy_pipe: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    copy_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    source: Option<Image>,
    downs: Vec<Down>,
    ups: Vec<Up>,
    outputs: Vec<Image>,
    /// Bumps when `outputs` is rebuilt, so a plate can keep its sample bind group.
    output_epoch: u64,
    /// Sample group for the view `copy_into_source` last read, keyed by its address.
    copy: Option<(usize, wgpu::BindGroup)>,
}

impl Pyramid {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-blur"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-blur"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..wgpu::SamplerDescriptor::default()
        });
        let layout = bind_layout(device, true);
        let copy_layout = bind_layout(device, false);
        Self {
            down_pipe: pipe(device, &module, "fs_down", &[&layout], format),
            up_pipe: pipe(device, &module, "fs_up", &[&layout], format),
            copy_pipe: pipe(device, &module, "fs_copy", &[&copy_layout], format),
            layout,
            copy_layout,
            sampler,
            format,
            size: (0, 0),
            source: None,
            downs: Vec::new(),
            ups: Vec::new(),
            outputs: Vec::new(),
            output_epoch: 0,
            copy: None,
        }
    }

    /// Changes only when the blurred pictures are reallocated.
    pub fn output_epoch(&self) -> u64 {
        self.output_epoch
    }

    pub fn source(&self) -> Option<&wgpu::TextureView> {
        self.source.as_ref().map(|image| &image.view)
    }

    /// The blurred picture for `place` in the last `blur` call's level list.
    pub fn output(&self, place: usize) -> Option<&wgpu::TextureView> {
        self.outputs.get(place).map(|image| &image.view)
    }

    /// Reads one blurred picture back. Rows are padded to 256 bytes on the GPU and unpacked here.
    pub fn read(&self, device: &wgpu::Device, queue: &wgpu::Queue, place: usize) -> Option<Vec<u8>> {
        let image = self.outputs.get(place)?;
        read_rgba(device, queue, &image.texture, image.width, image.height)
    }

    pub fn read_source(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Vec<u8>> {
        let image = self.source.as_ref()?;
        read_rgba(device, queue, &image.texture, image.width, image.height)
    }

    pub fn resident_bytes(&self) -> u64 {
        let mut bytes = self.source.as_ref().map(Image::bytes).unwrap_or(0);
        bytes += self.downs.iter().map(|down| down.image.bytes()).sum::<u64>();
        bytes += self.ups.iter().map(|up| up.image.bytes()).sum::<u64>();
        bytes + self.outputs.iter().map(Image::bytes).sum::<u64>()
    }

    pub fn release(&mut self) {
        self.size = (0, 0);
        self.source = None;
        self.downs.clear();
        self.ups.clear();
        self.outputs.clear();
        self.copy = None;
        self.output_epoch = self.output_epoch.wrapping_add(1);
    }

    /// Rebuilds the chain when the window or the deepest step changes. Pipelines stay.
    pub fn ensure(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32, levels: u32) -> bool {
        let size = source_size(width, height);
        let levels = levels.clamp(1, MAX_LEVELS) as usize;
        if self.size == size && self.downs.len() >= levels && self.source.is_some() {
            return false;
        }
        self.size = size;
        let source = image(device, self.format, size.0, size.1);
        let mut downs = Vec::new();
        let mut prev_view = source.view.clone();
        let mut prev_w = size.0;
        let mut prev_h = size.1;
        for _ in 0..levels {
            let Some((w, h)) = half(prev_w, prev_h) else { break };
            let level = image(device, self.format, w, h);
            let down_bind = tap(device, queue, &self.layout, &self.sampler, &prev_view, prev_w, prev_h);
            let up_bind = tap(device, queue, &self.layout, &self.sampler, &level.view, w, h);
            prev_view = level.view.clone();
            prev_w = w;
            prev_h = h;
            downs.push(Down { image: level, bind: down_bind, up_bind });
        }
        let ups = downs
            .iter()
            .take(downs.len().saturating_sub(1))
            .map(|down| {
                let image = image(device, self.format, down.image.width, down.image.height);
                let bind = tap(device, queue, &self.layout, &self.sampler, &image.view, image.width, image.height);
                Up { image, bind }
            })
            .collect();
        self.source = Some(source);
        self.downs = downs;
        self.ups = ups;
        self.outputs.clear();
        self.copy = None;
        self.output_epoch = self.output_epoch.wrapping_add(1);
        true
    }

    /// Blurs the source. `levels` is 1-based; each entry gets one source-sized picture, in the same order.
    pub fn blur(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, levels: &[u32]) {
        if self.source.is_none() || self.downs.is_empty() || levels.is_empty() {
            return;
        }
        let Some(max) = levels.iter().copied().max() else { return };
        let max = max.min(self.downs.len() as u32);
        for down in self.downs.iter().take(max as usize) {
            draw(encoder, &self.down_pipe, &down.image.view, &down.bind);
        }
        let (width, height) = self.size;
        if self.outputs.len() != levels.len() {
            self.outputs = (0..levels.len()).map(|_| image(device, self.format, width, height)).collect();
            self.output_epoch = self.output_epoch.wrapping_add(1);
        }
        for (place, level) in levels.iter().copied().enumerate() {
            let Some(dest) = self.outputs.get(place) else { continue };
            self.upsample(encoder, level.min(max), &dest.view);
        }
    }

    /// One downsample from `from` would be a copy into the source. Copies `from` over the whole source.
    pub fn copy_into_source(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        from: &wgpu::TextureView,
    ) {
        let Some(source) = self.source.as_ref().map(|image| image.view.clone()) else { return };
        let key = from as *const wgpu::TextureView as usize;
        let bind = match &self.copy {
            Some((held, bind)) if *held == key => bind.clone(),
            _ => {
                let bind = copy_bind(device, &self.copy_layout, &self.sampler, from);
                self.copy = Some((key, bind.clone()));
                bind
            }
        };
        draw(encoder, &self.copy_pipe, &source, &bind);
    }

    fn upsample(&self, encoder: &mut wgpu::CommandEncoder, level: u32, into: &wgpu::TextureView) {
        let last = (level as usize).min(self.downs.len()).saturating_sub(1);
        let Some(deep) = self.downs.get(last) else { return };
        if last == 0 {
            draw(encoder, &self.up_pipe, into, &deep.up_bind);
            return;
        }
        let mut src = &deep.up_bind;
        for index in (0..last).rev() {
            let Some(dest) = self.ups.get(index) else {
                draw(encoder, &self.up_pipe, into, src);
                return;
            };
            draw(encoder, &self.up_pipe, &dest.image.view, src);
            src = &dest.bind;
        }
        draw(encoder, &self.up_pipe, into, src);
    }
}

impl Image {
    fn bytes(&self) -> u64 {
        u64::from(self.texture.width()) * u64::from(self.texture.height()) * 4
    }
}

fn half(width: u32, height: u32) -> Option<(u32, u32)> {
    let w = width / 2;
    let h = height / 2;
    (w >= 2 && h >= 2).then_some((w, h))
}

fn image(device: &wgpu::Device, format: wgpu::TextureFormat, width: u32, height: u32) -> Image {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("folia-blur"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Image { texture, view, width, height }
}

fn tap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    view: &wgpu::TextureView,
    width: u32,
    height: u32,
) -> wgpu::BindGroup {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("folia-blur"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let texel = [1.0 / width.max(1) as f32, 1.0 / height.max(1) as f32, 0.0, 0.0];
    queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&texel));
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("folia-blur"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: buffer.as_entire_binding() },
        ],
    })
}

fn copy_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("folia-blur-copy"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

fn bind_layout(device: &wgpu::Device, uniform: bool) -> wgpu::BindGroupLayout {
    let mut entries = vec![
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
    ];
    if uniform {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("folia-blur"), entries: &entries })
}

fn pipe(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    entry: &str,
    groups: &[&wgpu::BindGroupLayout],
    format: wgpu::TextureFormat,
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
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn read_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let padded = width.div_ceil(64) * 64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("folia-blur-read"),
        size: u64::from(padded) * u64::from(height) * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded * 4),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit(Some(encoder.finish()));
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
    receiver.recv().ok()?.ok()?;
    let mapped = buffer.slice(..).get_mapped_range();
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for row in 0..height {
        let src = (row * padded * 4) as usize;
        let dst = (row * width * 4) as usize;
        let end = dst + (width * 4) as usize;
        rgba.get_mut(dst..end)?.copy_from_slice(mapped.get(src..src + (width * 4) as usize)?);
    }
    drop(mapped);
    buffer.unmap();
    Some(rgba)
}

fn draw(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    target: &wgpu::TextureView,
    group: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("folia-blur"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}
