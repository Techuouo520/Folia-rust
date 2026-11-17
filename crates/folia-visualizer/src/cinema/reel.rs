// The projector's current and previous cover retain the original linear-light mip chain.

use std::sync::Arc;

use super::CinemaError;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const SHADER: &str = include_str!("../../shaders/cinema/reel.wgsl");

#[derive(Clone)]
struct Cover {
    view: wgpu::TextureView,
    aspect: f32,
    light: [[f32; 4]; 3],
}

pub(super) struct Reels {
    current: Cover,
    previous: Cover,
    leader: Cover,
    key: Option<usize>,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Reels {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-cinema-reel-mip"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-cinema-reel-mip"),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("folia-cinema-reel-mip"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
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
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-cinema-reel"),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 4,
            ..Default::default()
        });
        let mut leader = upload(
            device,
            queue,
            &pipeline,
            layout,
            &sampler,
            Pixels { width: 64, height: 64, bytes: include_bytes!("../../../../assets/cinema/leader.rgba") },
        );
        leader.light = [[120.0, 116.0, 110.0], [128.0, 124.0, 118.0], [120.0, 116.0, 110.0]].map(as_light);
        Self {
            current: leader.clone(),
            previous: leader.clone(),
            leader,
            key: None,
            pipeline,
            layout: layout.clone(),
            sampler,
        }
    }

    pub fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cover: Option<&Arc<image::RgbaImage>>,
    ) -> Result<bool, CinemaError> {
        let key = cover.map(|image| Arc::as_ptr(image) as usize);
        if key == self.key {
            return Ok(false);
        }
        let next = match cover {
            Some(image) => {
                let limit = device.limits().max_texture_dimension_2d;
                if image.width() == 0 || image.height() == 0 || image.width() > limit || image.height() > limit {
                    return Err(CinemaError::Asset("cover dimensions"));
                }
                upload(
                    device,
                    queue,
                    &self.pipeline,
                    &self.layout,
                    &self.sampler,
                    Pixels { width: image.width(), height: image.height(), bytes: image.as_raw() },
                )
            }
            None => self.leader.clone(),
        };
        self.previous = std::mem::replace(&mut self.current, next);
        self.key = key;
        Ok(true)
    }

    pub fn finish(&mut self) -> bool {
        if self.previous.view == self.current.view {
            return false;
        }
        self.previous = self.current.clone();
        true
    }

    pub fn views(&self) -> [&wgpu::TextureView; 2] {
        [&self.current.view, &self.previous.view]
    }
    pub fn aspects(&self) -> [f32; 2] {
        [self.current.aspect, self.previous.aspect]
    }
    pub fn lights(&self) -> [[[f32; 4]; 3]; 2] {
        [self.current.light, self.previous.light]
    }
}

struct Pixels<'a> {
    width: u32,
    height: u32,
    bytes: &'a [u8],
}

fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::RenderPipeline,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    pixels: Pixels<'_>,
) -> Cover {
    let size = wgpu::Extent3d { width: pixels.width, height: pixels.height, depth_or_array_layers: 1 };
    let levels = pixels.width.max(pixels.height).ilog2() + 1;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("folia-cinema-reel"),
        size,
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        pixels.bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(pixels.width * 4),
            rows_per_image: Some(pixels.height),
        },
        size,
    );
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("folia-cinema-reel-mips") });
    for level in 1..levels {
        let source = texture.create_view(&wgpu::TextureViewDescriptor {
            base_mip_level: level - 1,
            mip_level_count: Some(1),
            ..Default::default()
        });
        let extent = wgpu::Extent3d {
            width: (pixels.width >> level).max(1),
            height: (pixels.height >> level).max(1),
            depth_or_array_layers: 1,
        };
        let scratch = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-cinema-reel-downsample"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target = scratch.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-cinema-reel-mip"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&source) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("folia-cinema-reel-mip"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_texture(
            scratch.as_image_copy(),
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            extent,
        );
    }
    queue.submit([encoder.finish()]);
    Cover {
        view: texture.create_view(&Default::default()),
        aspect: pixels.width as f32 / pixels.height as f32,
        light: third_colors(&pixels),
    }
}

fn third_colors(pixels: &Pixels<'_>) -> [[f32; 4]; 3] {
    let mut sums = [[0.0; 3]; 3];
    for y in 0..8 {
        let sy = ((y as f32 + 0.5) * pixels.height as f32 / 8.0 - 0.5).max(0.0);
        let y0 = (sy as u32).min(pixels.height - 1);
        let y1 = (y0 + 1).min(pixels.height - 1);
        for x in 0..24 {
            let sx = ((x as f32 + 0.5) * pixels.width as f32 / 24.0 - 0.5).max(0.0);
            let x0 = (sx as u32).min(pixels.width - 1);
            let x1 = (x0 + 1).min(pixels.width - 1);
            for (channel, sum) in sums[x / 8].iter_mut().enumerate() {
                let at = |x, y| f32::from(pixels.bytes[((y * pixels.width + x) * 4) as usize + channel]);
                let mix = |a, b, t| a + (b - a) * t;
                let top = mix(at(x0, y0), at(x1, y0), sx.fract());
                let bottom = mix(at(x0, y1), at(x1, y1), sx.fract());
                *sum += mix(top, bottom, sy.fract()) / 64.0;
            }
        }
    }
    sums.map(as_light)
}

fn as_light(rgb: [f32; 3]) -> [f32; 4] {
    let linear = rgb.map(|channel| folia_render::to_linear_channel(channel / 255.0));
    let luminance = linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
    let gain = if luminance < 0.05 { (0.05 / luminance.max(0.004)).sqrt() } else { 1.0 };
    let vivid = linear.map(|channel| (luminance + (channel - luminance) * 1.3).max(0.0) * gain);
    [vivid[0], vivid[1], vivid[2], 0.0]
}
