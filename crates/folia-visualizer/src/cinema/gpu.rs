// Native cinema rendering keeps the live projection and the hall's preset-density lens targets separate.

use std::sync::Arc;

use iced::Rectangle;
use wgpu::util::DeviceExt;

use super::frame::{Frame, Uniforms};
use super::geometry::{self, CinemaError};
use super::post::{self, Image, Post};

const ROOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const MESH_ATTRIBUTES: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x2];
const MATRIX_ATTRIBUTES: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4];
const GLOW_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
const DUST_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x4];
const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

#[cfg(feature = "dev")]
static TARGETS: parking_lot::Mutex<Vec<(usize, wgpu::Device, [u32; 2])>> = parking_lot::Mutex::new(Vec::new());
#[cfg(feature = "dev")]
static NEXT_TARGET: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

/// Live hall targets on one device, for offline density and resource-lifetime inspection.
#[cfg(feature = "dev")]
pub fn target_sizes(device: &wgpu::Device) -> Vec<[u32; 2]> {
    TARGETS.lock().iter().filter(|(_, owner, _)| owner == device).map(|(_, _, size)| *size).collect()
}

struct Mesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    instances: wgpu::Buffer,
    indices_count: u32,
    instances_count: u32,
}

impl Mesh {
    fn new(
        device: &wgpu::Device,
        bytes: &'static [u8],
        matrices: &[u8],
        name: &'static str,
    ) -> Result<Self, CinemaError> {
        let data = geometry::mesh(bytes, name)?;
        Ok(Self {
            vertices: vertex_buffer(device, data.vertices),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("folia-cinema-indices"),
                contents: data.indices,
                usage: wgpu::BufferUsages::INDEX,
            }),
            instances: vertex_buffer(device, matrices),
            indices_count: data.count,
            instances_count: (matrices.len() / 64) as u32,
        })
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.indices_count, 0, 0..self.instances_count);
    }
}

struct Targets {
    multisampled: Image,
    main: Image,
    depth: Image,
    lens: post::Targets,
    #[cfg(feature = "dev")]
    id: usize,
}

#[cfg(feature = "dev")]
impl Drop for Targets {
    fn drop(&mut self) {
        TARGETS.lock().retain(|(id, _, _)| *id != self.id);
    }
}

struct Pipelines {
    room: wgpu::RenderPipeline,
    curtain: wgpu::RenderPipeline,
    screen: wgpu::RenderPipeline,
    picture: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    dust: wgpu::RenderPipeline,
    light: wgpu::ComputePipeline,
}

/// The headless and live scene share these immutable assets and per-view GPU resources.
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    pipelines: Pipelines,
    uniform: wgpu::Buffer,
    scene: wgpu::BindGroup,
    lighting: wgpu::BindGroup,
    meshes: [Mesh; 4],
    glows: wgpu::Buffer,
    dust: wgpu::Buffer,
    noise: wgpu::TextureView,
    reels: super::reel::Reels,
    reel_changed: bool,
    post: Post,
    targets: Option<Targets>,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        picture_layout: &wgpu::BindGroupLayout,
    ) -> Result<Self, CinemaError> {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let result = Self::build(device, queue, format, picture_layout);
        if let Some(error) = iced::futures::executor::block_on(device.pop_error_scope()) {
            return Err(CinemaError::Graphics(Arc::new(error)));
        }
        result
    }

    // Builds the hall once; subsequent frames only replace uniforms and resize render targets.
    fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        picture_layout: &wgpu::BindGroupLayout,
    ) -> Result<Self, CinemaError> {
        let graphics_layout = scene_layout(device, false);
        let lighting_layout = scene_layout(device, true);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-cinema-frame"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lights = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-cinema-screen-lights"),
            size: 208,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-cinema-scene"),
            layout: &graphics_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: lights.as_entire_binding() },
            ],
        });
        let lighting = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-cinema-lighting"),
            layout: &lighting_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: lights.as_entire_binding() },
            ],
        });
        let identity = bytemuck::cast_slice(&IDENTITY);
        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            format,
            pipelines: Pipelines::new(device, format, picture_layout, &graphics_layout, &lighting_layout),
            uniform,
            scene,
            lighting,
            meshes: [
                Mesh::new(device, geometry::SEAT, geometry::SEAT_MATRICES, "seat")?,
                Mesh::new(device, geometry::AUDIENCE, geometry::AUDIENCE_MATRICES, "audience")?,
                Mesh::new(device, geometry::ROOM, identity, "room")?,
                Mesh::new(device, geometry::CURTAINS, identity, "curtains")?,
            ],
            glows: vertex_buffer(device, geometry::GLOWS),
            dust: vertex_buffer(device, include_bytes!("../../../../assets/cinema/dust.bin")),
            noise: smoke_texture(device, queue),
            reels: super::reel::Reels::new(device, queue, picture_layout),
            reel_changed: false,
            post: Post::new(device, format, &graphics_layout),
            targets: None,
        })
    }

    pub fn prepare(&mut self, frame: Frame, logical: iced::Size) {
        if frame.reel_change >= 1.0 {
            self.reel_changed |= self.reels.finish();
        }
        let limit = self.device.limits().max_texture_dimension_2d as f32;
        let density = frame.quality.density().min(limit / logical.width.max(1.0)).min(limit / logical.height.max(1.0));
        let size =
            [(logical.width * density).round().max(1.0) as u32, (logical.height * density).round().max(1.0) as u32];
        if self.targets.as_ref().is_none_or(|targets| targets.main.size != size) {
            let multisampled = Image::new(&self.device, size, ROOM_FORMAT, 4);
            let main = Image::new(&self.device, size, ROOM_FORMAT, 1);
            let depth = Image::new(&self.device, size, wgpu::TextureFormat::Depth32Float, 4);
            let lens = self.post.targets(&self.device, &main, &depth, &self.noise, self.reels.views());
            #[cfg(feature = "dev")]
            let id = NEXT_TARGET.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            #[cfg(feature = "dev")]
            TARGETS.lock().push((id, self.device.clone(), size));
            self.targets = Some(Targets {
                multisampled,
                main,
                depth,
                lens,
                #[cfg(feature = "dev")]
                id,
            });
        } else if self.reel_changed
            && let Some(targets) = &mut self.targets
        {
            targets.lens.update_reel(
                &self.post,
                &self.device,
                &targets.main,
                &targets.depth,
                &self.noise,
                self.reels.views(),
            );
        }
        self.reel_changed = false;
        let mut uniforms = Uniforms::of(frame, size, self.format.is_srgb());
        uniforms.reel[..2].copy_from_slice(&self.reels.aspects());
        [uniforms.reel_light_a, uniforms.reel_light_b] = self.reels.lights();
        self.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn set_cover(&mut self, cover: Option<&Arc<image::RgbaImage>>) -> Result<(), CinemaError> {
        self.reel_changed |= self.reels.sync(&self.device, &self.queue, cover)?;
        Ok(())
    }

    /// The scene-density target is independent of the supplied lyric texture's density.
    pub fn target_size(&self) -> Option<[u32; 2]> {
        self.targets.as_ref().map(|targets| targets.main.size)
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, picture: &wgpu::BindGroup) {
        let Some(targets) = &self.targets else { return };
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("folia-cinema-screen-light"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipelines.light);
            pass.set_bind_group(0, picture, &[]);
            pass.set_bind_group(1, &self.lighting, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("folia-cinema-hall"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.multisampled.view,
                    resolve_target: Some(&targets.main.view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth.view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, picture, &[]);
            pass.set_bind_group(1, &self.scene, &[]);
            pass.set_pipeline(&self.pipelines.room);
            for mesh in &self.meshes[..3] {
                mesh.draw(&mut pass);
            }
            pass.set_pipeline(&self.pipelines.curtain);
            self.meshes[3].draw(&mut pass);
            pass.set_pipeline(&self.pipelines.screen);
            pass.draw(0..6, 0..1);
            pass.set_pipeline(&self.pipelines.glow);
            pass.set_vertex_buffer(0, self.glows.slice(..));
            pass.draw(0..6, 0..(geometry::GLOWS.len() / 32) as u32);
            pass.set_pipeline(&self.pipelines.dust);
            pass.set_vertex_buffer(0, self.dust.slice(..));
            pass.draw(0..6, 0..520);
        }
        self.post.render(encoder, &targets.lens, &self.scene);
    }

    /// First projects the high-resolution lyric picture, then composites the hall's transparent screen opening.
    pub fn composite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        picture: &wgpu::BindGroup,
        viewport: Rectangle,
        clip: Rectangle<u32>,
    ) {
        let Some(targets) = &self.targets else { return };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("folia-cinema-projection"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_viewport(viewport.x, viewport.y, viewport.width, viewport.height, 0.0, 1.0);
            pass.set_scissor_rect(clip.x, clip.y, clip.width, clip.height);
            pass.set_pipeline(&self.pipelines.picture);
            pass.set_bind_group(0, picture, &[]);
            pass.set_bind_group(1, &self.scene, &[]);
            pass.draw(0..6, 0..1);
        }
        self.post.composite(encoder, target, &targets.lens, &self.scene, viewport, clip);
    }
}

impl Pipelines {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        picture: &wgpu::BindGroupLayout,
        scene: &wgpu::BindGroupLayout,
        lighting: &wgpu::BindGroupLayout,
    ) -> Self {
        let source = [
            include_str!("../../shaders/cinema/uniforms.wgsl"),
            folia_render::quality::ANTIALIAS_SHADER,
            include_str!("../../shaders/cinema/room.wgsl"),
            include_str!("../../shaders/cinema/geometry.wgsl"),
            include_str!("../../shaders/cinema/effects.wgsl"),
        ]
        .join("\n");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-cinema-material"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-cinema-material"),
            bind_group_layouts: &[picture, scene],
            push_constant_ranges: &[],
        });
        let mesh = [
            wgpu::VertexBufferLayout {
                array_stride: 40,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &MESH_ATTRIBUTES,
            },
            wgpu::VertexBufferLayout {
                array_stride: 64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &MATRIX_ATTRIBUTES,
            },
        ];
        let glow = [wgpu::VertexBufferLayout {
            array_stride: 32,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &GLOW_ATTRIBUTES,
        }];
        let dust = [wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &DUST_ATTRIBUTES,
        }];
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let create =
            |vertex, fragment, buffers: &[wgpu::VertexBufferLayout<'_>], output, blend, depth: Option<bool>| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("folia-cinema-material"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some(vertex),
                        buffers,
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(fragment),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: output,
                            blend: Some(blend),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
                    depth_stencil: depth.map(|write| wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: write,
                        depth_compare: wgpu::CompareFunction::LessEqual,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: if depth.is_some() { 4 } else { 1 },
                        ..Default::default()
                    },
                    multiview: None,
                    cache: None,
                })
            };
        let light_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-cinema-screen-light"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("../../shaders/cinema/uniforms.wgsl"),
                    include_str!("../../shaders/cinema/light.wgsl")
                )
                .into(),
            ),
        });
        let light_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-cinema-screen-light"),
            bind_group_layouts: &[picture, lighting],
            push_constant_ranges: &[],
        });
        Self {
            room: create("vs_room", "main", &mesh, ROOM_FORMAT, wgpu::BlendState::REPLACE, Some(true)),
            curtain: create("vs_curtain", "main", &mesh, ROOM_FORMAT, wgpu::BlendState::REPLACE, Some(true)),
            screen: create("vs_screen", "fs_screen", &[], ROOM_FORMAT, wgpu::BlendState::REPLACE, Some(true)),
            picture: create(
                "vs_picture",
                "fs_picture",
                &[],
                format,
                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
                None,
            ),
            glow: create("vs_glow", "fs_glow", &glow, ROOM_FORMAT, additive, Some(false)),
            dust: create("vs_dust", "fs_dust", &dust, ROOM_FORMAT, additive, Some(false)),
            light: device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("folia-cinema-screen-light"),
                layout: Some(&light_layout),
                module: &light_shader,
                entry_point: Some("sample_screen"),
                compilation_options: Default::default(),
                cache: None,
            }),
        }
    }
}

fn vertex_buffer(device: &wgpu::Device, contents: &[u8]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("folia-cinema-vertices"),
        contents,
        usage: wgpu::BufferUsages::VERTEX,
    })
}

fn scene_layout(device: &wgpu::Device, compute: bool) -> wgpu::BindGroupLayout {
    let visibility = if compute { wgpu::ShaderStages::COMPUTE } else { wgpu::ShaderStages::VERTEX_FRAGMENT };
    let entries = vec![
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: !compute },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
    ];
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("folia-cinema-scene"),
        entries: &entries,
    })
}

fn smoke_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let size = wgpu::Extent3d { width: 32, height: 32, depth_or_array_layers: 32 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("folia-cinema-smoke"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        include_bytes!("../../../../assets/cinema/smoke.bin"),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(32), rows_per_image: Some(32) },
        size,
    );
    texture.create_view(&Default::default())
}
