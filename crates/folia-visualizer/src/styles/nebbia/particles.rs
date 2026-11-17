// Shared instanced rendering for snow, frozen beads and dew fireflies, with the original fog illumination.

use super::snow::atlas::{self, Atlas};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct View {
    size: [f32; 2],
    shift: [f32; 2],
    flags: [f32; 4],
}

struct Batch {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    capacity: u64,
    count: u32,
}
impl Batch {
    fn new(device: &wgpu::Device, pipeline: wgpu::RenderPipeline) -> Self {
        Self { pipeline, buffer: instance_buffer(device, 10240), capacity: 10240, count: 0 }
    }
    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8], count: usize) {
        if data.len() as u64 > self.capacity {
            self.capacity = (data.len() as u64).next_power_of_two();
            self.buffer = instance_buffer(device, self.capacity);
        }
        if !data.is_empty() {
            queue.write_buffer(&self.buffer, 0, data);
        }
        self.count = count as u32;
    }
}

pub(super) struct Gpu {
    snow: Batch,
    flies: Batch,
    flies_active: bool,
    layout: wgpu::BindGroupLayout,
    atlas: wgpu::TextureView,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    bind: Option<wgpu::BindGroup>,
}

impl Gpu {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> (Self, Vec<Vec<atlas::Lump>>) {
        let Atlas { mut image, lumps } = atlas::build();
        for pixel in image.pixels_mut() {
            let alpha = u32::from(pixel[3]);
            for value in &mut pixel.0[..3] {
                *value = ((u32::from(*value) * alpha + 127) / 255) as u8;
            }
        }
        let levels = image.width().max(image.height()).ilog2() + 1;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-nebbia-snow-atlas"),
            size: wgpu::Extent3d { width: image.width(), height: image.height(), depth_or_array_layers: 1 },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for mip_level in 0..levels {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                image.as_raw(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width() * 4),
                    rows_per_image: Some(image.height()),
                },
                wgpu::Extent3d { width: image.width(), height: image.height(), depth_or_array_layers: 1 },
            );
            if mip_level + 1 < levels {
                image = image::imageops::resize(
                    &image,
                    (image.width() / 2).max(1),
                    (image.height() / 2).max(1),
                    image::imageops::FilterType::Triangle,
                );
            }
        }
        let atlas = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-nebbia-particle-sampler"),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mut entries = vec![super::gpu::uniform_entry(0)];
        entries.extend((1..=3).map(super::gpu::texture_entry));
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 4,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        entries[0].visibility = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let layout = super::gpu::layout(device, &entries);
        let snow = Batch::new(
            device,
            pipeline(
                device,
                &layout,
                format,
                include_str!("../../../shaders/nebbia/snow.wgsl"),
                40,
                &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x2],
            ),
        );
        let flies = Batch::new(
            device,
            pipeline(
                device,
                &layout,
                format,
                include_str!("../../../shaders/nebbia/fireflies.wgsl"),
                48,
                &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
            ),
        );
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-nebbia-particle-uniform"),
            size: std::mem::size_of::<View>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        (Self { snow, flies, flies_active: false, layout, atlas, sampler, uniform, bind: None }, lumps)
    }

    pub fn bind(&mut self, device: &wgpu::Device, glow: &wgpu::TextureView, behind: &wgpu::TextureView) {
        self.bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-nebbia-particle-bind"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.atlas) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(glow) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(behind) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        }));
    }

    fn view(&self, queue: &wgpu::Queue, uniforms: &super::gpu::Uniforms, encode: bool) {
        let size = uniforms.resolution;
        let view = View {
            size,
            shift: [uniforms.motion[0] * size[0] * 0.35, uniforms.motion[1] * size[1] * 0.35],
            flags: [f32::from(uniforms.environment[3] < 0.5), f32::from(encode), 2.0, 0.0],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&view));
    }

    pub fn snow(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniforms: &super::gpu::Uniforms,
        encode: bool,
        instances: &[[f32; 10]],
    ) {
        self.view(queue, uniforms, encode);
        self.flies_active = false;
        self.snow.upload(device, queue, bytemuck::cast_slice(instances), instances.len());
    }

    pub fn fireflies(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniforms: &super::gpu::Uniforms,
        encode: bool,
        instances: &[[f32; 12]],
    ) {
        self.view(queue, uniforms, encode);
        self.flies_active = true;
        self.flies.upload(device, queue, bytemuck::cast_slice(instances), instances.len());
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        bounds: [f32; 4],
        clip: [u32; 4],
    ) {
        let Some(bind) = &self.bind else { return };
        let batch = if self.flies_active { &self.flies } else { &self.snow };
        if batch.count == 0 {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-nebbia-particles"),
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
        pass.set_viewport(bounds[0], bounds[1], bounds[2], bounds[3], 0.0, 1.0);
        pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
        pass.set_pipeline(&batch.pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.set_vertex_buffer(0, batch.buffer.slice(..));
        pass.draw(0..4, 0..batch.count);
    }
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    source: &str,
    stride: u64,
    attributes: &[wgpu::VertexAttribute],
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("folia-nebbia-particles"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("folia-nebbia-particles"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("folia-nebbia-particles"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: stride,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes,
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

fn instance_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("folia-nebbia-particle-instances"),
        size: capacity,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
