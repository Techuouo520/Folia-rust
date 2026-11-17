// Cached cinema lens targets and the original ordered Gaussian, bloom and beam passes.

use iced::Rectangle;
use wgpu::util::DeviceExt;

pub(super) struct Image {
    _texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: [u32; 2],
}

impl Image {
    pub fn new(device: &wgpu::Device, size: [u32; 2], format: wgpu::TextureFormat, samples: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-cinema-target"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self { _texture: texture, view, size }
    }
}

pub(super) struct Post {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    noise_sampler: wgpu::Sampler,
    down: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    beam: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
}

struct Step {
    kind: Kind,
    target: usize,
    bind: wgpu::BindGroup,
}

enum Kind {
    Down,
    Blur,
    Beam,
}

pub(super) struct Targets {
    images: [Image; 8],
    steps: Vec<Step>,
    composite: wgpu::BindGroup,
    output: wgpu::BindGroup,
}

struct Samples<'a> {
    source: &'a wgpu::TextureView,
    depth: &'a wgpu::TextureView,
    noise: &'a wgpu::TextureView,
    film: Option<[&'a wgpu::TextureView; 2]>,
    extra: Option<[&'a wgpu::TextureView; 4]>,
}

impl Post {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, scene_layout: &wgpu::BindGroupLayout) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-cinema-lens"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("../../shaders/cinema/uniforms.wgsl"),
                    include_str!("../../shaders/cinema/post.wgsl")
                )
                .into(),
            ),
        });
        let mut entries = Vec::new();
        for binding in [0, 3, 4, 5, 6, 10, 11] {
            entries.push(texture_entry(binding, wgpu::TextureViewDimension::D2));
        }
        entries.push(texture_entry(8, wgpu::TextureViewDimension::D3));
        for binding in [1, 9] {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: true,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 7,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-cinema-lens"),
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-cinema-lens"),
            bind_group_layouts: &[&layout, scene_layout],
            push_constant_ranges: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-cinema-lens"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-cinema-smoke"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let pipe = |fragment, format, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("folia-cinema-lens"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(blend),
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
        };
        Self {
            down: pipe("down", wgpu::TextureFormat::Rgba16Float, wgpu::BlendState::REPLACE),
            blur: pipe("blur", wgpu::TextureFormat::Rgba16Float, wgpu::BlendState::REPLACE),
            beam: pipe("beam", wgpu::TextureFormat::Rgba16Float, wgpu::BlendState::REPLACE),
            composite: pipe("composite", wgpu::TextureFormat::Rgba8Unorm, wgpu::BlendState::REPLACE),
            blit: pipe("blit", format, wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            layout,
            sampler,
            noise_sampler,
        }
    }

    pub fn targets(
        &self,
        device: &wgpu::Device,
        main: &Image,
        depth: &Image,
        noise: &wgpu::TextureView,
        film: [&wgpu::TextureView; 2],
    ) -> Targets {
        let half = main.size.map(|value| (value / 2).max(1));
        let quarter = half.map(|value| (value / 2).max(1));
        let sizes = [half, half, quarter, quarter, quarter, quarter, half, main.size];
        let images: [Image; 8] = std::array::from_fn(|index| {
            Image::new(
                device,
                sizes[index],
                if index == 7 { wgpu::TextureFormat::Rgba8Unorm } else { wgpu::TextureFormat::Rgba16Float },
                1,
            )
        });
        let samples = |source| Samples { source, depth: &depth.view, noise, film: None, extra: None };
        let mut steps = vec![
            Step {
                kind: Kind::Beam,
                target: 6,
                bind: self.bind(device, Samples { film: Some(film), ..samples(&main.view) }, [0.0; 4]),
            },
            Step {
                kind: Kind::Down,
                target: 0,
                bind: self.bind(
                    device,
                    samples(&main.view),
                    [1.0 / main.size[0] as f32, 1.0 / main.size[1] as f32, 0.0, 1.0],
                ),
            },
            Step {
                kind: Kind::Down,
                target: 4,
                bind: self.bind(
                    device,
                    samples(&images[0].view),
                    [1.0 / half[0] as f32, 1.0 / half[1] as f32, 1.0, 0.0],
                ),
            },
        ];
        let blur_pair = |steps: &mut Vec<Step>, first: usize, second: usize, spread: f32| {
            let size = sizes[first];
            steps.push(Step {
                kind: Kind::Blur,
                target: second,
                bind: self.bind(device, samples(&images[first].view), [spread / size[0] as f32, 0.0, 0.0, 0.0]),
            });
            steps.push(Step {
                kind: Kind::Blur,
                target: first,
                bind: self.bind(device, samples(&images[second].view), [0.0, spread / size[1] as f32, 0.0, 0.0]),
            });
        };
        blur_pair(&mut steps, 0, 1, 1.4);
        steps.push(Step {
            kind: Kind::Down,
            target: 2,
            bind: self.bind(device, samples(&images[0].view), [1.0 / half[0] as f32, 1.0 / half[1] as f32, 0.0, 0.0]),
        });
        blur_pair(&mut steps, 2, 3, 1.6);
        blur_pair(&mut steps, 4, 5, 1.5);
        blur_pair(&mut steps, 4, 5, 3.2);
        let composite = self.bind(
            device,
            Samples {
                extra: Some([&images[0].view, &images[2].view, &images[4].view, &images[6].view]),
                ..samples(&main.view)
            },
            [0.0; 4],
        );
        let output = self.bind(device, samples(&images[7].view), [0.0; 4]);
        Targets { images, steps, composite, output }
    }

    fn bind(&self, device: &wgpu::Device, samples: Samples<'_>, parameters: [f32; 4]) -> wgpu::BindGroup {
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("folia-cinema-lens-parameters"),
            contents: bytemuck::cast_slice(&parameters),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let extra = samples.extra.unwrap_or([samples.source; 4]);
        let film = samples.film.unwrap_or([samples.source; 2]);
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-cinema-lens"),
            layout: &self.layout,
            entries: &[
                texture_binding(0, samples.source),
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                texture_binding(2, samples.depth),
                texture_binding(3, extra[0]),
                texture_binding(4, extra[1]),
                texture_binding(5, extra[2]),
                texture_binding(6, extra[3]),
                wgpu::BindGroupEntry { binding: 7, resource: uniform.as_entire_binding() },
                texture_binding(8, samples.noise),
                wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::Sampler(&self.noise_sampler) },
                texture_binding(10, film[0]),
                texture_binding(11, film[1]),
            ],
        })
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, targets: &Targets, scene: &wgpu::BindGroup) {
        for step in &targets.steps {
            let pipeline = match step.kind {
                Kind::Down => &self.down,
                Kind::Blur => &self.blur,
                Kind::Beam => &self.beam,
            };
            render_pass(encoder, pipeline, &targets.images[step.target].view, &step.bind, scene, None);
        }
        render_pass(encoder, &self.composite, &targets.images[7].view, &targets.composite, scene, None);
    }

    pub fn composite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        targets: &Targets,
        scene: &wgpu::BindGroup,
        viewport: Rectangle,
        clip: Rectangle<u32>,
    ) {
        render_pass(encoder, &self.blit, target, &targets.output, scene, Some((viewport, clip)));
    }
}

impl Targets {
    pub fn update_reel(
        &mut self,
        post: &Post,
        device: &wgpu::Device,
        main: &Image,
        depth: &Image,
        noise: &wgpu::TextureView,
        film: [&wgpu::TextureView; 2],
    ) {
        self.steps[0].bind = post.bind(
            device,
            Samples { source: &main.view, depth: &depth.view, noise, film: Some(film), extra: None },
            [0.0; 4],
        );
    }
}

fn render_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    target: &wgpu::TextureView,
    images: &wgpu::BindGroup,
    scene: &wgpu::BindGroup,
    area: Option<(Rectangle, Rectangle<u32>)>,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("folia-cinema-lens"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: if area.is_some() { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) },
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    if let Some((viewport, clip)) = area {
        pass.set_viewport(viewport.x, viewport.y, viewport.width, viewport.height, 0.0, 1.0);
        pass.set_scissor_rect(clip.x, clip.y, clip.width, clip.height);
    }
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, images, &[]);
    pass.set_bind_group(1, scene, &[]);
    pass.draw(0..3, 0..1);
}

fn texture_entry(binding: u32, dimension: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: dimension,
            multisampled: false,
        },
        count: None,
    }
}

fn texture_binding(binding: u32, view: &wgpu::TextureView) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::TextureView(view) }
}
