// Holds the two paragraph render targets and Pixi-compatible half-resolution transition filters.

use iced::Rectangle;

use super::transitions::Frame;

const SHADER: &str = include_str!("../../../shaders/lumiere/transition.wgsl");
const PADDING: f32 = 24.0;

struct Image {
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
}

impl Drop for Image {
    fn drop(&mut self) {
        folia_render::release_source(&self.view);
    }
}

struct Uniform {
    buffer: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

struct Layer {
    scene: Image,
    blur: [Image; 2],
    filtering: bool,
}

struct Pictures {
    size: [u32; 2],
    padding: [u32; 2],
    output: Image,
    layers: [Layer; 2],
}

pub(super) struct Composite {
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    blur: wgpu::RenderPipeline,
    compose: wgpu::RenderPipeline,
    filters: [[Uniform; 6]; 2],
    frames: [Uniform; 2],
    identity: Uniform,
    pictures: Option<Pictures>,
    format: wgpu::TextureFormat,
}

impl Composite {
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-lumiere-transition-texture"),
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
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-lumiere-transition-uniform"),
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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-lumiere-transition"),
            bind_group_layouts: &[&texture_layout, &uniform_layout],
            push_constant_ranges: &[],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-lumiere-transition"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-lumiere-transition"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let identity = Uniform::new(device, &uniform_layout);
        identity.write(queue, [1.0; 4]);
        Self {
            texture_layout,
            sampler,
            blur: pipeline(device, &layout, &module, format, "blur", wgpu::BlendState::REPLACE),
            compose: pipeline(
                device,
                &layout,
                &module,
                format,
                "compose",
                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            ),
            filters: std::array::from_fn(|_| std::array::from_fn(|_| Uniform::new(device, &uniform_layout))),
            frames: std::array::from_fn(|_| Uniform::new(device, &uniform_layout)),
            identity,
            pictures: None,
            format,
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
        logical: [f32; 2],
        frames: [Option<Frame>; 2],
    ) {
        let mut size = size;
        let mut padding = std::array::from_fn(|axis| (PADDING * size[axis] as f32 / logical[axis]).ceil() as u32);
        let largest = (size[0] + 2 * padding[0]).max(size[1] + 2 * padding[1]);
        let limit = device.limits().max_texture_dimension_2d;
        if largest > limit {
            let scale = limit.saturating_sub(2).max(1) as f32 / largest as f32;
            size = size.map(|dimension| (dimension as f32 * scale).floor().max(1.0) as u32);
            padding = std::array::from_fn(|axis| (PADDING * size[axis] as f32 / logical[axis]).ceil() as u32);
        }
        let full = std::array::from_fn(|axis| size[axis] + 2 * padding[axis]);
        if self.pictures.as_ref().is_none_or(|pictures| pictures.size != size || pictures.padding != padding) {
            let image = |size| Image::new(device, &self.texture_layout, &self.sampler, self.format, size);
            self.pictures = Some(Pictures {
                size,
                padding,
                output: image(size),
                layers: std::array::from_fn(|_| Layer {
                    scene: image(full),
                    blur: std::array::from_fn(|_| image(full.map(|dimension| dimension.div_ceil(2)))),
                    filtering: false,
                }),
            });
        }
        let Some(pictures) = &mut self.pictures else { return };
        for (index, frame) in frames.into_iter().enumerate() {
            let Some(frame) = frame else { continue };
            pictures.layers[index].filtering = frame.blur > 0.3;
            let ratio = [size[0] as f32 / full[0] as f32, size[1] as f32 / full[1] as f32];
            self.frames[index].write(queue, [ratio[0], ratio[1], frame.alpha, frame.scale]);
            let strength = frame.blur / 1.3125_f32.sqrt();
            for pass in 0..6 {
                let axis = usize::from(pass >= 3);
                let mut step = [0.0; 4];
                step[axis] = strength * 0.5_f32.powi((pass % 3) as i32) * ratio[axis] / logical[axis];
                self.filters[index][pass].write(queue, step);
            }
        }
    }

    pub(super) fn output(&self) -> Option<&wgpu::TextureView> {
        self.pictures.as_ref().map(|pictures| &pictures.output.view)
    }

    pub(super) fn scene(&self, index: usize) -> Option<(&wgpu::TextureView, Rectangle<u32>)> {
        self.pictures.as_ref().map(|pictures| {
            (
                &pictures.layers[index].scene.view,
                Rectangle {
                    x: pictures.padding[0],
                    y: pictures.padding[1],
                    width: pictures.size[0],
                    height: pictures.size[1],
                },
            )
        })
    }

    pub(super) fn layer(&self, encoder: &mut wgpu::CommandEncoder, index: usize) {
        let Some(pictures) = &self.pictures else { return };
        let layer = &pictures.layers[index];
        if layer.filtering {
            for pass in 0..6 {
                let source = if pass == 0 { &layer.scene.bind } else { &layer.blur[(pass - 1) % 2].bind };
                let mut render = clear(encoder, &layer.blur[pass % 2].view);
                render.set_pipeline(&self.blur);
                render.set_bind_group(0, source, &[]);
                render.set_bind_group(1, &self.filters[index][pass].bind, &[]);
                render.draw(0..3, 0..1);
            }
        }
        let source = if layer.filtering { &layer.blur[1].bind } else { &layer.scene.bind };
        let mut render = load(encoder, &pictures.output.view);
        render.set_pipeline(&self.compose);
        render.set_bind_group(0, source, &[]);
        render.set_bind_group(1, &self.frames[index].bind, &[]);
        render.draw(0..3, 0..1);
    }

    pub(super) fn render(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, clip: &Rectangle<u32>) {
        let Some(pictures) = &self.pictures else { return };
        let mut render = load(encoder, target);
        render.set_viewport(clip.x as f32, clip.y as f32, clip.width as f32, clip.height as f32, 0.0, 1.0);
        render.set_scissor_rect(clip.x, clip.y, clip.width, clip.height);
        render.set_pipeline(&self.compose);
        render.set_bind_group(0, &pictures.output.bind, &[]);
        render.set_bind_group(1, &self.identity.bind, &[]);
        render.draw(0..3, 0..1);
    }
}

impl Uniform {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-lumiere-transition"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-lumiere-transition"),
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
        });
        Self { buffer, bind }
    }

    fn write(&self, queue: &wgpu::Queue, values: [f32; 4]) {
        let mut bytes = [0; 16];
        for (slot, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(values) {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        queue.write_buffer(&self.buffer, 0, &bytes);
    }
}

impl Image {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        format: wgpu::TextureFormat,
        size: [u32; 2],
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-lumiere-transition"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-lumiere-transition"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        Self { view, bind }
    }
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    entry: &str,
    blend: wgpu::BlendState,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("folia-lumiere-transition"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

pub(super) fn clear<'a>(encoder: &'a mut wgpu::CommandEncoder, target: &wgpu::TextureView) -> wgpu::RenderPass<'a> {
    pass(encoder, target, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT))
}

pub(super) fn load<'a>(encoder: &'a mut wgpu::CommandEncoder, target: &wgpu::TextureView) -> wgpu::RenderPass<'a> {
    pass(encoder, target, wgpu::LoadOp::Load)
}

fn pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("folia-lumiere-transition"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}
