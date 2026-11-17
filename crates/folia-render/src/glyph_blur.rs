// Gaussian coverage tiles reuse the sharp atlas and keep tint and transforms outside the cache.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};

use super::{Atlas, Key, Mark, Row, Shared, Sprite, bind, key_of};

const SHADER: &str = include_str!("../shaders/glyph_blur.wgsl");
const FIRST_SIZE: [u32; 2] = [2048, 1024];
const MAX_PAGES: u32 = 8;
const JOB_BYTES: usize = std::mem::size_of::<Job>();
const ATTRIBUTES: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlurStats {
    pub size: [u32; 2],
    pub pages: u32,
    pub marks: usize,
    pub jobs: usize,
    pub rendered_texels: u64,
    pub allocated_bytes: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BlurKey {
    glyph: Key,
    sigma: u32,
}

#[derive(Clone, Copy)]
struct Tile {
    page: u32,
    row: usize,
    rect: [u32; 4],
    padding: u32,
    step: u32,
    seen: u64,
    ready: bool,
}

#[derive(Default)]
struct Page {
    rows: Vec<Row>,
    bottom: u32,
}

struct Texture {
    texture: wgpu::Texture,
    array: wgpu::TextureView,
    layers: Vec<wgpu::TextureView>,
    size: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Job {
    source: [f32; 4],
    scratch: [f32; 4],
    target: [f32; 4],
    values: [f32; 4],
}

struct Batch {
    start: u32,
    end: u32,
    page: u32,
}

pub(super) struct Pipelines {
    horizontal: wgpu::RenderPipeline,
    vertical: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    horizontal_uniform: wgpu::Buffer,
    vertical_uniform: wgpu::Buffer,
}

#[derive(Clone, Copy)]
pub(super) struct Context<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub atlas: &'a Atlas,
    pub shared: &'a Shared,
    pub pipelines: &'a Pipelines,
    pub frame: u64,
}

pub(super) struct Cache {
    texture: Texture,
    scratch: wgpu::Texture,
    scratch_view: wgpu::TextureView,
    scratch_size: [u32; 2],
    source_texture: wgpu::Texture,
    horizontal: wgpu::BindGroup,
    vertical: wgpu::BindGroup,
    pub bind: wgpu::BindGroup,
    pages: Vec<Page>,
    slots: HashMap<BlurKey, Tile>,
    pending: Vec<BlurKey>,
    repack: Vec<(BlurKey, Tile)>,
    jobs: Vec<Job>,
    batches: Vec<Batch>,
    buffer: wgpu::Buffer,
    kernels: wgpu::Buffer,
    weights: Vec<[f32; 2]>,
    kernel_slots: HashMap<u32, [u32; 2]>,
    capacity: usize,
    stats: BlurStats,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-glyph-gaussian"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-glyph-gaussian"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-glyph-gaussian"),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-glyph-gaussian"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let make = |format| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("folia-glyph-gaussian"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: JOB_BYTES as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &ATTRIBUTES,
                    }],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
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
        let uniform = || {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("folia-glyph-gaussian-view"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            horizontal: make(wgpu::TextureFormat::R16Float),
            vertical: make(wgpu::TextureFormat::R8Unorm),
            layout,
            sampler,
            horizontal_uniform: uniform(),
            vertical_uniform: uniform(),
        }
    }
}

pub(super) fn requested(sprite: &Sprite, scale: f32) -> Option<(Key, f32)> {
    let Mark::BlurredGlyph { size, sigma, .. } = sprite.mark else { return None };
    if sprite.color[3] <= 0.0 || !sigma.is_finite() || sigma <= 0.0 || !size.is_finite() || size <= 0.0 {
        return None;
    }
    let glyph = key_of(sprite, scale);
    let Key::Glyph { quarter_px, .. } = glyph else { return None };
    let physical = sigma * quarter_px as f32 * 0.25 / size;
    (physical.is_finite() && physical > 0.0).then_some((glyph, physical))
}

impl Cache {
    pub fn new(device: &wgpu::Device, atlas: &Atlas, shared: &Shared, pipelines: &Pipelines, capacity: usize) -> Self {
        let limit = super::LARGEST_SIDE.min(device.limits().max_texture_dimension_2d);
        let size = FIRST_SIZE.map(|side| side.min(limit));
        let texture = Texture::new(device, size, 1);
        let scratch = scratch(device, size);
        let scratch_view = scratch.create_view(&Default::default());
        let source = atlas.texture.create_view(&Default::default());
        let kernels = kernel_buffer(device, 1024);
        let horizontal = gaussian_bind(device, pipelines, &pipelines.horizontal_uniform, &source, &kernels);
        let vertical = gaussian_bind(device, pipelines, &pipelines.vertical_uniform, &scratch_view, &kernels);
        let combined = bind(device, &shared.layout, &shared.uniform, &source, &texture.array, &shared.sampler);
        Self {
            texture,
            scratch,
            scratch_view,
            scratch_size: size,
            source_texture: atlas.texture.clone(),
            horizontal,
            vertical,
            bind: combined,
            pages: vec![Page::default()],
            slots: HashMap::with_capacity(capacity.saturating_mul(2)),
            pending: Vec::with_capacity(capacity),
            repack: Vec::with_capacity(capacity),
            jobs: Vec::with_capacity(capacity),
            batches: Vec::with_capacity(capacity),
            buffer: job_buffer(device, capacity.max(1)),
            kernels,
            weights: Vec::with_capacity(1024),
            kernel_slots: HashMap::with_capacity(capacity),
            capacity: capacity.max(1),
            stats: BlurStats::default(),
        }
    }

    pub fn prepare(&mut self, context: Context<'_>, sprites: &[Sprite], scale: f32) {
        let Context { device, queue, atlas, shared, pipelines, frame } = context;
        self.pending.clear();
        self.jobs.clear();
        self.batches.clear();
        if self.source_texture != atlas.texture {
            self.source_texture = atlas.texture.clone();
            self.refresh_bindings(device, atlas, shared, pipelines);
        }
        self.slots.retain(|_, tile| tile.seen.saturating_add(1) >= frame);
        for sprite in sprites {
            let Some((glyph, sigma)) = requested(sprite, scale) else { continue };
            let key = BlurKey { glyph, sigma: sigma.to_bits() };
            if let Some(tile) = self.slots.get_mut(&key) {
                tile.seen = frame;
                self.pages[tile.page as usize].rows[tile.row].used = frame;
            }
        }
        let mut repacked = false;
        for sprite in sprites {
            let Some((glyph, sigma)) = requested(sprite, scale) else { continue };
            let key = BlurKey { glyph, sigma: sigma.to_bits() };
            if self.slots.contains_key(&key) {
                continue;
            }
            let Some(source) = atlas.slots.get(&glyph).filter(|slot| slot.width > 0) else { continue };
            let largest = super::LARGEST_SIDE.min(device.limits().max_texture_dimension_2d);
            if sigma > largest as f32 / 6.0 {
                tracing::warn!(sigma, "glyph blur exceeds scratch dimensions");
                continue;
            }
            let step = 2_u32.pow((sigma / 4.0).max(1.0).log2().floor() as u32);
            let padding = (sigma * 3.0).ceil() as u32 + step;
            let width = (source.width + padding * 2).div_ceil(step);
            let height = (source.height + padding * 2).div_ceil(step);
            if height * step > largest {
                tracing::warn!(sigma, "glyph blur exceeds scratch dimensions");
                continue;
            }
            let Some(mut tile) = self.allocate(context, [width, height], &mut repacked) else { continue };
            tile.padding = padding;
            tile.step = step;
            self.slots.insert(key, tile);
            self.pending.push(key);
        }
        self.encode(device, queue, atlas, pipelines);
        self.stats = BlurStats {
            size: self.texture.size,
            pages: self.pages.len() as u32,
            marks: self.slots.len(),
            jobs: self.jobs.len(),
            rendered_texels: self
                .jobs
                .iter()
                .map(|job| (job.scratch[2] * job.scratch[3] + job.target[2] * job.target[3]) as u64)
                .sum(),
            allocated_bytes: u64::from(self.texture.size[0])
                * u64::from(self.texture.size[1])
                * self.pages.len() as u64
                + u64::from(self.scratch_size[0]) * u64::from(self.scratch_size[1]) * 2,
        };
    }

    pub fn instance(&self, sprite: &Sprite, scale: f32, atlas: &Atlas, instance: &mut [f32; 20]) {
        let Some((glyph, sigma)) = requested(sprite, scale) else { return };
        let Some(tile) = self.slots.get(&BlurKey { glyph, sigma: sigma.to_bits() }).filter(|tile| tile.ready) else {
            instance[15] = 0.0;
            return;
        };
        let Key::Glyph { quarter_px, .. } = glyph else { return };
        let Mark::BlurredGlyph { size, .. } = sprite.mark else { return };
        if !atlas.slots.contains_key(&glyph) {
            return;
        }
        let pixel = size / (quarter_px as f32 * 0.25);
        instance[0] -= tile.padding as f32 * pixel;
        instance[1] -= tile.padding as f32 * pixel;
        instance[2] = tile.rect[2] as f32 * tile.step as f32 * pixel;
        instance[3] = tile.rect[3] as f32 * tile.step as f32 * pixel;
        instance[10] = 1.0;
        instance[11] = tile.page as f32;
        let [width, height] = self.texture.size.map(|value| value as f32);
        instance[16..20].copy_from_slice(&[
            tile.rect[0] as f32 / width,
            tile.rect[1] as f32 / height,
            (tile.rect[0] + tile.rect[2]) as f32 / width,
            (tile.rect[1] + tile.rect[3]) as f32 / height,
        ]);
    }

    pub fn stats(&self) -> BlurStats {
        self.stats
    }

    pub fn idle(&mut self) {
        self.stats.jobs = 0;
        self.stats.rendered_texels = 0;
    }

    pub fn reserve(&mut self, device: &wgpu::Device, capacity: usize) {
        if capacity <= self.capacity {
            return;
        }
        self.capacity = capacity;
        self.buffer = job_buffer(device, capacity);
        self.pending.reserve(capacity.saturating_sub(self.pending.len()));
        self.repack.reserve(capacity.saturating_sub(self.repack.len()));
        self.jobs.reserve(capacity.saturating_sub(self.jobs.len()));
        self.batches.reserve(capacity.saturating_sub(self.batches.len()));
        self.slots.reserve(capacity.saturating_mul(2).saturating_sub(self.slots.len()));
    }

    fn refresh_bindings(&mut self, device: &wgpu::Device, atlas: &Atlas, shared: &Shared, pipelines: &Pipelines) {
        let source = atlas.texture.create_view(&Default::default());
        self.horizontal = gaussian_bind(device, pipelines, &pipelines.horizontal_uniform, &source, &self.kernels);
        self.vertical =
            gaussian_bind(device, pipelines, &pipelines.vertical_uniform, &self.scratch_view, &self.kernels);
        self.bind = bind(device, &shared.layout, &shared.uniform, &source, &self.texture.array, &shared.sampler);
    }

    fn allocate(&mut self, context: Context<'_>, size: [u32; 2], repacked: &mut bool) -> Option<Tile> {
        let Context { device, queue, atlas, shared, pipelines, frame } = context;
        let largest = super::LARGEST_SIDE.min(device.limits().max_texture_dimension_2d);
        if size[0] > largest || size[1] > largest {
            tracing::warn!(?size, "glyph blur exceeds atlas dimensions");
            return None;
        }
        loop {
            if let Some(tile) = self.place(size, frame) {
                return Some(tile);
            }
            let live: u64 = self
                .slots
                .values()
                .filter(|tile| tile.seen == frame)
                .map(|tile| u64::from(tile.rect[2]) * u64::from(tile.rect[3]))
                .sum();
            let area = u64::from(self.texture.size[0]) * u64::from(self.texture.size[1]) * self.pages.len() as u64;
            if !*repacked && live * 2 < area {
                self.repack_live(device, queue, frame);
                self.refresh_bindings(device, atlas, shared, pipelines);
                *repacked = true;
                continue;
            }
            let [width, height] = self.texture.size;
            let next = if width < size[0] || (height == largest && width < largest) {
                [(width * 2).min(largest), height]
            } else {
                [width, (height * 2).min(largest)]
            };
            let pages = if next == self.texture.size { self.pages.len() as u32 + 1 } else { self.pages.len() as u32 };
            if pages > MAX_PAGES.min(device.limits().max_texture_array_layers) {
                tracing::warn!(pages, "glyph blur atlas is full");
                return None;
            }
            self.grow(device, queue, next, pages);
            self.refresh_bindings(device, atlas, shared, pipelines);
        }
    }

    fn place(&mut self, size: [u32; 2], frame: u64) -> Option<Tile> {
        let [width, height] = size;
        let [side_width, side_height] = self.texture.size;
        for (page_index, page) in self.pages.iter_mut().enumerate() {
            let row = page.rows.iter().position(|row| {
                row.height >= height && row.height <= height + height / 4 + 4 && row.x + width <= side_width
            });
            let row = match row {
                Some(row) => row,
                None if page.bottom + height.div_ceil(4) * 4 <= side_height && width <= side_width => {
                    let row = page.rows.len();
                    page.rows.push(Row { y: page.bottom, height: height.div_ceil(4) * 4, x: 0, used: 0 });
                    page.bottom += height.div_ceil(4) * 4;
                    row
                }
                None => {
                    let stale = page
                        .rows
                        .iter()
                        .enumerate()
                        .filter(|(_, row)| row.used < frame && row.height >= height && width <= side_width)
                        .min_by_key(|(_, row)| (row.used, row.height))
                        .map(|(index, _)| index);
                    let Some(row) = stale else { continue };
                    self.slots.retain(|_, tile| tile.page != page_index as u32 || tile.row != row);
                    page.rows[row].x = 0;
                    row
                }
            };
            let row_data = &mut page.rows[row];
            let x = row_data.x;
            row_data.x += width;
            row_data.used = frame;
            return Some(Tile {
                page: page_index as u32,
                row,
                rect: [x, row_data.y, width, height],
                padding: 0,
                step: 1,
                seen: frame,
                ready: false,
            });
        }
        None
    }

    fn grow(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, size: [u32; 2], pages: u32) {
        let next = Texture::new(device, size, pages);
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("folia-glyph-blur-grow") });
        encoder.copy_texture_to_texture(
            self.texture.texture.as_image_copy(),
            next.texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.texture.size[0],
                height: self.texture.size[1],
                depth_or_array_layers: self.pages.len() as u32,
            },
        );
        queue.submit([encoder.finish()]);
        self.pages.resize_with(pages as usize, Page::default);
        self.texture = next;
    }

    fn repack_live(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: u64) {
        self.repack.clear();
        self.repack.extend(self.slots.iter().filter(|(_, tile)| tile.seen == frame).map(|(key, tile)| (*key, *tile)));
        self.repack.sort_unstable_by_key(|(_, tile)| std::cmp::Reverse(tile.rect[3]));
        let size = self.texture.size;
        let previous = std::mem::replace(&mut self.texture, Texture::new(device, size, self.pages.len() as u32));
        for page in &mut self.pages {
            page.rows.clear();
            page.bottom = 0;
        }
        self.slots.clear();
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("folia-glyph-blur-repack") });
        for index in 0..self.repack.len() {
            let (key, old) = self.repack[index];
            let Some(mut next) = self.place([old.rect[2], old.rect[3]], frame) else { continue };
            next.padding = old.padding;
            next.step = old.step;
            next.ready = old.ready;
            if old.ready {
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &previous.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: old.rect[0], y: old.rect[1], z: old.page },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.texture.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: next.rect[0], y: next.rect[1], z: next.page },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d { width: old.rect[2], height: old.rect[3], depth_or_array_layers: 1 },
                );
            }
            self.slots.insert(key, next);
        }
        queue.submit([encoder.finish()]);
    }

    fn encode(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, atlas: &Atlas, pipelines: &Pipelines) {
        if self.pending.is_empty() {
            return;
        }
        self.weights.clear();
        self.kernel_slots.clear();
        for key in &self.pending {
            if self.kernel_slots.contains_key(&key.sigma) {
                continue;
            }
            let first = self.weights.len();
            append_kernel(&mut self.weights, f32::from_bits(key.sigma));
            self.kernel_slots.insert(key.sigma, [first as u32, (self.weights.len() - first) as u32]);
        }
        if self.weights.len() * 8 > self.kernels.size() as usize {
            self.kernels = kernel_buffer(device, self.weights.len().next_power_of_two());
            let source = atlas.texture.create_view(&Default::default());
            self.horizontal = gaussian_bind(device, pipelines, &pipelines.horizontal_uniform, &source, &self.kernels);
            self.vertical =
                gaussian_bind(device, pipelines, &pipelines.vertical_uniform, &self.scratch_view, &self.kernels);
        }
        self.pending.sort_unstable_by_key(|key| self.slots.get(key).map_or(0, |tile| tile.page));
        let mut needed = self.scratch_size;
        for key in &self.pending {
            if let Some(tile) = self.slots.get(key) {
                needed[0] = needed[0].max(tile.rect[2]);
                needed[1] = needed[1].max(tile.rect[3] * tile.step);
            }
        }
        let largest = super::LARGEST_SIDE.min(device.limits().max_texture_dimension_2d);
        needed = needed.map(|side| side.next_power_of_two().min(largest));
        if needed != self.scratch_size {
            self.scratch = scratch(device, needed);
            self.scratch_view = self.scratch.create_view(&Default::default());
            self.scratch_size = needed;
            self.vertical =
                gaussian_bind(device, pipelines, &pipelines.vertical_uniform, &self.scratch_view, &self.kernels);
        }
        let (mut x, mut y, mut row, mut page, mut first) = (0, 0, 0, u32::MAX, 0);
        for key in &self.pending {
            let (Some(tile), Some(source)) = (self.slots.get_mut(key), atlas.slots.get(&key.glyph)) else { continue };
            let width = tile.rect[2];
            let height = tile.rect[3] * tile.step;
            if width > self.scratch_size[0] || height > self.scratch_size[1] {
                continue;
            }
            if x + width > self.scratch_size[0] {
                x = 0;
                y += row;
                row = 0;
            }
            if page != tile.page || y + height > self.scratch_size[1] {
                if self.jobs.len() > first {
                    self.batches.push(Batch { start: first as u32, end: self.jobs.len() as u32, page });
                }
                first = self.jobs.len();
                page = tile.page;
                x = 0;
                y = 0;
                row = 0;
            }
            let Some(kernel) = self.kernel_slots.get(&key.sigma) else { continue };
            self.jobs.push(Job {
                source: [source.x, source.y, source.width, source.height].map(|value| value as f32),
                scratch: [x, y, width, height].map(|value| value as f32),
                target: tile.rect.map(|value| value as f32),
                values: [kernel[1] as f32, tile.padding as f32, tile.step as f32, kernel[0] as f32],
            });
            x += width;
            row = row.max(height);
            tile.ready = true;
        }
        if self.jobs.len() > first {
            self.batches.push(Batch { start: first as u32, end: self.jobs.len() as u32, page });
        }
        if self.jobs.len() > self.capacity {
            self.capacity = self.jobs.len().next_power_of_two();
            self.buffer = job_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.jobs));
        queue.write_buffer(&self.kernels, 0, bytemuck::cast_slice(&self.weights));
        queue.write_buffer(
            &pipelines.horizontal_uniform,
            0,
            bytemuck::cast_slice(&[self.scratch_size[0] as f32, self.scratch_size[1] as f32, 0.0, 0.0]),
        );
        queue.write_buffer(
            &pipelines.vertical_uniform,
            0,
            bytemuck::cast_slice(&[self.texture.size[0] as f32, self.texture.size[1] as f32, 1.0, 0.0]),
        );
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("folia-glyph-gaussian") });
        for batch in &self.batches {
            draw(&mut encoder, &pipelines.horizontal, &self.scratch_view, &self.horizontal, &self.buffer, batch, true);
            draw(
                &mut encoder,
                &pipelines.vertical,
                &self.texture.layers[batch.page as usize],
                &self.vertical,
                &self.buffer,
                batch,
                false,
            );
        }
        queue.submit([encoder.finish()]);
    }
}

impl Texture {
    fn new(device: &wgpu::Device, size: [u32; 2], layers: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-glyph-blur-atlas"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: layers },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let array = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layers = (0..layers)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        Self { texture, array, layers, size }
    }
}

fn scratch(device: &wgpu::Device, size: [u32; 2]) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("folia-glyph-blur-scratch"),
        size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

fn gaussian_bind(
    device: &wgpu::Device,
    pipelines: &Pipelines,
    uniform: &wgpu::Buffer,
    source: &wgpu::TextureView,
    kernels: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("folia-glyph-gaussian"),
        layout: &pipelines.layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(source) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&pipelines.sampler) },
            wgpu::BindGroupEntry { binding: 3, resource: kernels.as_entire_binding() },
        ],
    })
}

fn kernel_buffer(device: &wgpu::Device, count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("folia-glyph-gaussian-kernels"),
        size: (count * 8) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

// Adjacent Gaussian taps share one bilinear lookup; no coverage pixels are filtered on the CPU.
fn append_kernel(weights: &mut Vec<[f32; 2]>, sigma: f32) {
    let first = weights.len();
    weights.push([0.0, 1.0]);
    let variance = f64::from(sigma).powi(2).max(0.000001);
    let radius = (sigma * 3.0).ceil() as u32;
    let mut total = 1.0;
    for offset in (1..=radius).step_by(2) {
        let x = f64::from(offset);
        let a = (-0.5 * x * x / variance).exp();
        let b = if offset < radius { (-0.5 * (x + 1.0).powi(2) / variance).exp() } else { 0.0 };
        let combined = a + b;
        if combined > 0.0 {
            weights.push([((x * a + (x + 1.0) * b) / combined) as f32, combined as f32]);
            total += combined * 2.0;
        }
    }
    for weight in &mut weights[first..] {
        weight[1] /= total as f32;
    }
}

fn job_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("folia-glyph-blur-jobs"),
        size: (capacity * JOB_BYTES) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn draw(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    target: &wgpu::TextureView,
    bind: &wgpu::BindGroup,
    buffer: &wgpu::Buffer,
    batch: &Batch,
    clear: bool,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("folia-glyph-gaussian"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: if clear { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) } else { wgpu::LoadOp::Load },
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind, &[]);
    pass.set_vertex_buffer(0, buffer.slice(..));
    pass.draw(0..6, batch.start..batch.end);
}
