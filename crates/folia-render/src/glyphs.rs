// Glyph sprites: text shaped with iced's font system and small coverage images, rasterized once into an R8 atlas and
// drawn as transformed quads.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, PoisonError};

use parking_lot::{Mutex, MutexGuard};

use iced::Font;
use iced::advanced::graphics::text::cosmic_text::{
    Buffer, CacheKey, CacheKeyFlags, Command, FontSystem, Metrics, Shaping, SwashCache, SwashContent, SwashImage,
    fontdb,
};
use iced::advanced::graphics::text::{font_system, to_attributes};

const SHADER: &str = include_str!("../shaders/glyph.wgsl");
const INSTANCE_BYTES: usize = 80;
/// The atlas starts at 2048 × 1024 and doubles its height, then its width, up to 4096 × 4096.
const FIRST_SIZE: [u32; 2] = [2048, 1024];
const LARGEST_SIDE: u32 = 4096;
const GUTTER: u32 = 1;
/// Canvas `strokeText` over a `fillText` at this alpha, as the original outline glyphs are drawn.
const OUTLINE_FILL: f32 = 0.14;

const ATTRIBUTES: [wgpu::VertexAttribute; 5] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 1 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 2 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 3 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 64, shader_location: 4 },
];

/// One shaped glyph. `x`/`y` are its pen position from the left edge and baseline, in logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct ShapedGlyph {
    pub font: fontdb::ID,
    pub id: u16,
    pub weight: u16,
    pub flags: CacheKeyFlags,
    /// Byte offset of the glyph's cluster in the shaped text.
    pub cluster: usize,
    pub x: f32,
    pub y: f32,
}

/// A run of text set on one line.
#[derive(Clone, Debug, Default)]
pub struct Shaped {
    pub glyphs: Vec<ShapedGlyph>,
    /// Advance width in logical pixels.
    pub width: f32,
    /// Baseline below the middle of a `1.3 × size` line box, as canvas text centered on `y` sits.
    pub baseline: f32,
}

/// Sets `text` on one line at `size` logical pixels.
pub fn shape(text: &str, size: f32, font: Font) -> Shaped {
    let mut fonts = font_system().write().unwrap_or_else(PoisonError::into_inner);
    let system = fonts.raw();
    let line_height = size * 1.3;
    let mut buffer = Buffer::new(system, Metrics::new(size, line_height));
    let attrs = to_attributes(font);
    let mut shaped = Shaped::default();
    let mut view = buffer.borrow_with(system);
    view.set_size(None, None);
    view.set_text(text, &attrs, Shaping::Advanced, None);
    view.shape_until_scroll(false);
    let mut first = None;
    for run in view.layout_runs() {
        let top = *first.get_or_insert(run.line_y);
        shaped.baseline = top - line_height * 0.5;
        shaped.width = shaped.width.max(run.line_w);
        shaped.glyphs.extend(run.glyphs.iter().map(|glyph| ShapedGlyph {
            font: glyph.font_id,
            id: glyph.glyph_id,
            weight: glyph.font_weight.0,
            flags: glyph.cache_key_flags,
            cluster: glyph.start,
            x: glyph.x + glyph.font_size * glyph.x_offset,
            y: glyph.y - glyph.font_size * glyph.y_offset + run.line_y - top,
        }));
    }
    shaped
}

/// What a sprite shows.
#[derive(Clone, Copy, Debug)]
pub enum Mark {
    /// A glyph at `size` logical pixels, rasterized at no more than `max_scale` physical pixels per logical pixel.
    /// `outline` is the stroke width as a fraction of `size` for hollow glyphs; `0` fills.
    Glyph { glyph: ShapedGlyph, size: f32, outline: f32, max_scale: f32 },
    /// Image `n` given to `Glyphs::new`, covering the unit square centered on `pen`; `axes` give it its size.
    Image(u16),
}

/// A coverage image (one byte per texel) the sprites can show.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// One sprite to draw. Screen position = `anchor + axes × (pen + mark box)`, in logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub mark: Mark,
    pub anchor: [f32; 2],
    pub pen: [f32; 2],
    /// Local-to-screen linear map, columns `(a, b)` and `(c, d)`: rotation, scale and skew.
    pub axes: [f32; 4],
    /// Straight RGBA; the coverage multiplies the alpha.
    pub color: [f32; 4],
}

/// The sprites of one frame. Shared because `draw` (the program) fills it and `prepare` (the pipeline) reads it: both
/// hold the same vector, so frames reuse its capacity instead of moving a new one into each primitive. iced draws and
/// prepares in turn, so the lock is never contended.
#[derive(Clone, Debug, Default)]
pub struct Batch(Arc<Mutex<Vec<Sprite>>>);

impl Batch {
    pub fn lock(&self) -> MutexGuard<'_, Vec<Sprite>> {
        self.0.lock()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Glyph {
        font: fontdb::ID,
        id: u16,
        weight: u16,
        flags: CacheKeyFlags,
        /// Raster size in quarter pixels.
        quarter_px: u32,
        /// Stroke width in thousandths of the size.
        outline: u32,
    },
    Image(u16),
}

/// Atlas size (width, height), the marks it holds and the marks the last frame drew, with peaks since the pipeline
/// was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AtlasStats {
    pub size: [u32; 2],
    pub marks: usize,
    pub live_marks: usize,
    pub live_texels: u64,
    pub peak_size: [u32; 2],
    pub peak_live_marks: usize,
    pub peak_live_texels: u64,
}

#[derive(Clone, Copy)]
struct Slot {
    row: usize,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    left: i32,
    top: i32,
    /// Last frame a sprite showed it.
    seen: u64,
}

struct Row {
    y: u32,
    height: u32,
    x: u32,
    used: u64,
}

/// Rows of marks in one R8 texture. A full atlas frees the rows least recently drawn; when that is not enough, the
/// marks this frame uses are copied into a fresh atlas, larger only if they fill more than half of it.
struct Atlas {
    texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    size: [u32; 2],
    rows: Vec<Row>,
    bottom: u32,
    slots: HashMap<Key, Slot>,
}

impl Atlas {
    fn new(device: &wgpu::Device, size: [u32; 2], shared: &Shared) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-glyph-atlas"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = bind(device, &shared.layout, &shared.uniform, &view, &shared.sampler);
        Self { texture, bind, size, rows: Vec::new(), bottom: 0, slots: HashMap::new() }
    }

    /// Makes sure every sprite's glyph is in the atlas. `false` when one did not fit.
    fn ensure(
        &mut self,
        raster: &mut SwashCache,
        images: &[Image],
        queue: &wgpu::Queue,
        sprites: &[Sprite],
        scale: f32,
        frame: u64,
    ) -> bool {
        let mut fonts = None;
        for sprite in sprites {
            let key = key_of(sprite, scale);
            if let Some(slot) = self.slots.get_mut(&key) {
                slot.seen = frame;
                if slot.width > 0 {
                    self.rows[slot.row].used = frame;
                }
                continue;
            }
            let image = match key {
                Key::Image(index) => images.get(usize::from(index)).map_or(Mask::EMPTY, mask_of_image),
                Key::Glyph { .. } => {
                    let fonts =
                        fonts.get_or_insert_with(|| font_system().write().unwrap_or_else(PoisonError::into_inner));
                    rasterize(raster, fonts.raw(), &key)
                }
            };
            let Some(slot) = self.store(queue, &image, frame) else { return false };
            self.slots.insert(key, slot);
        }
        true
    }

    fn store(&mut self, queue: &wgpu::Queue, image: &Mask, frame: u64) -> Option<Slot> {
        if image.width == 0 || image.height == 0 {
            return Some(Slot { row: 0, x: 0, y: 0, width: 0, height: 0, left: 0, top: 0, seen: frame });
        }
        let cell_w = image.width + GUTTER * 2;
        let cell_h = image.height + GUTTER * 2;
        let (row, x, y) = self.place(cell_w, cell_h, frame)?;
        let mut cell = vec![0_u8; (cell_w * cell_h) as usize];
        for line in 0..image.height {
            let from = (line * image.width) as usize;
            let to = ((line + GUTTER) * cell_w + GUTTER) as usize;
            cell[to..to + image.width as usize].copy_from_slice(&image.pixels[from..from + image.width as usize]);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &cell,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(cell_w), rows_per_image: Some(cell_h) },
            wgpu::Extent3d { width: cell_w, height: cell_h, depth_or_array_layers: 1 },
        );
        Some(Slot { row, x, y, width: cell_w, height: cell_h, left: image.left, top: image.top, seen: frame })
    }

    /// Texels of the marks `frame` has shown so far, and how many there are.
    fn live(&self, frame: u64) -> (u64, usize) {
        self.slots
            .values()
            .filter(|slot| slot.seen == frame && slot.width > 0)
            .fold((0, 0), |(area, count), slot| (area + u64::from(slot.width) * u64::from(slot.height), count + 1))
    }

    /// A new atlas of `size` holding only the marks `frame` has shown, copied on the GPU (tallest first).
    fn repack(&self, device: &wgpu::Device, queue: &wgpu::Queue, size: [u32; 2], shared: &Shared, frame: u64) -> Self {
        let mut next = Atlas::new(device, size, shared);
        let mut live: Vec<(Key, Slot)> =
            self.slots.iter().filter(|(_, slot)| slot.seen == frame).map(|(key, slot)| (*key, *slot)).collect();
        live.sort_by_key(|(_, slot)| std::cmp::Reverse(slot.height));
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("folia-glyphs") });
        for (key, slot) in live {
            if slot.width == 0 {
                next.slots.insert(key, slot);
                continue;
            }
            let Some((row, x, y)) = next.place(slot.width, slot.height, frame) else { continue };
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: slot.x, y: slot.y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &next.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d { width: slot.width, height: slot.height, depth_or_array_layers: 1 },
            );
            next.slots.insert(key, Slot { row, x, y, ..slot });
        }
        queue.submit([encoder.finish()]);
        next
    }

    /// Finds room for a `width × height` cell drawn this `frame`, reusing a stale row before giving up.
    fn place(&mut self, width: u32, height: u32, frame: u64) -> Option<(usize, u32, u32)> {
        let [side_w, side_h] = self.size;
        if width > side_w || height > side_h {
            return None;
        }
        let fits = |row: &Row| row.height >= height && row.height <= height + height / 4 + 4;
        if let Some(index) = self.rows.iter().position(|row| fits(row) && row.x + width <= side_w) {
            return Some(self.claim(index, width, frame));
        }
        let rounded = height.div_ceil(4) * 4;
        if self.bottom + rounded <= side_h {
            self.rows.push(Row { y: self.bottom, height: rounded, x: 0, used: frame });
            self.bottom += rounded;
            return Some(self.claim(self.rows.len() - 1, width, frame));
        }
        let stale = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.used < frame && row.height >= height)
            .min_by_key(|(_, row)| (row.used, row.height))
            .map(|(index, _)| index);
        let (first, end) = match stale {
            Some(index) => (index, index + 1),
            None => self.stale_run(height, frame)?,
        };
        self.slots.retain(|_, slot| slot.row < first || slot.row >= end);
        let total: u32 = self.rows[first..end].iter().map(|row| row.height).sum();
        for row in &mut self.rows[first..end] {
            row.height = 0;
            row.x = 0;
        }
        self.rows[first].height = total;
        Some(self.claim(first, width, frame))
    }

    /// Adjacent rows none of which this frame drew, together at least `height` tall: the run whose newest use is
    /// oldest. Merging them lets a tall mark reuse rows of short ones.
    fn stale_run(&self, height: u32, frame: u64) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, u64)> = None;
        for first in 0..self.rows.len() {
            let (mut total, mut newest) = (0, 0);
            for end in first..self.rows.len() {
                let row = &self.rows[end];
                if row.used >= frame && row.height > 0 {
                    break;
                }
                total += row.height;
                newest = newest.max(row.used);
                if total >= height {
                    if best.is_none_or(|(_, _, age)| newest < age) {
                        best = Some((first, end + 1, newest));
                    }
                    break;
                }
            }
        }
        best.map(|(first, end, _)| (first, end))
    }

    fn claim(&mut self, index: usize, width: u32, frame: u64) -> (usize, u32, u32) {
        let row = &mut self.rows[index];
        let at = (index, row.x, row.y);
        row.x += width;
        row.used = frame;
        at
    }

    fn instance(&self, sprite: &Sprite, scale: f32) -> [f32; 20] {
        let key = key_of(sprite, scale);
        let slot = self.slots.get(&key).copied().filter(|slot| slot.width > 0);
        let Some(slot) = slot else { return [0.0; 20] };
        let rect = match (key, sprite.mark) {
            (Key::Glyph { quarter_px, .. }, Mark::Glyph { size, .. }) => {
                let per_px = size / (quarter_px as f32 * 0.25);
                [
                    sprite.pen[0] + (slot.left - GUTTER as i32) as f32 * per_px,
                    sprite.pen[1] - (slot.top + GUTTER as i32) as f32 * per_px,
                    slot.width as f32 * per_px,
                    slot.height as f32 * per_px,
                ]
            }
            _ => {
                let inner_w = (slot.width - GUTTER * 2).max(1) as f32;
                let inner_h = (slot.height - GUTTER * 2).max(1) as f32;
                [
                    sprite.pen[0] - 0.5 - GUTTER as f32 / inner_w,
                    sprite.pen[1] - 0.5 - GUTTER as f32 / inner_h,
                    slot.width as f32 / inner_w,
                    slot.height as f32 / inner_h,
                ]
            }
        };
        let [side_w, side_h] = [self.size[0] as f32, self.size[1] as f32];
        [
            rect[0],
            rect[1],
            rect[2],
            rect[3],
            sprite.axes[0],
            sprite.axes[1],
            sprite.axes[2],
            sprite.axes[3],
            sprite.anchor[0],
            sprite.anchor[1],
            0.0,
            0.0,
            sprite.color[0],
            sprite.color[1],
            sprite.color[2],
            sprite.color[3],
            slot.x as f32 / side_w,
            slot.y as f32 / side_h,
            (slot.x + slot.width) as f32 / side_w,
            (slot.y + slot.height) as f32 / side_h,
        ]
    }
}

/// What every atlas binds besides its texture.
struct Shared {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
}

/// The glyph sprite pipeline with a fixed instance buffer. The atlas is made on first use and dropped by `release`.
pub struct Glyphs {
    pipeline: wgpu::RenderPipeline,
    shared: Shared,
    instances: wgpu::Buffer,
    capacity: usize,
    atlas: Option<Atlas>,
    images: Vec<Image>,
    peak: AtlasStats,
    staging: Vec<u8>,
    raster: SwashCache,
    frame: u64,
    count: u32,
    interface: bool,
}

impl std::fmt::Debug for Glyphs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let size = self.atlas.as_ref().map_or([0, 0], |atlas| atlas.size);
        f.debug_struct("Glyphs").field("size", &size).field("count", &self.count).finish()
    }
}

impl Glyphs {
    /// Builds the pipeline for straight-alpha blending onto `format`, with room for `capacity` sprites a frame.
    /// `images` are the coverage images `Mark::Image` refers to by index.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, capacity: usize, images: Vec<Image>) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-glyphs"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = bind_layout(device);
        let pipeline = pipeline(device, &module, &layout, format);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-glyphs"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-glyphs-screen"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-glyphs-instances"),
            size: (capacity * INSTANCE_BYTES) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            shared: Shared { layout, sampler, uniform },
            instances,
            capacity,
            atlas: None,
            images,
            peak: AtlasStats::default(),
            staging: Vec::with_capacity(capacity * INSTANCE_BYTES),
            raster: SwashCache::new(),
            frame: 0,
            count: 0,
            interface: false,
        }
    }

    /// Calibrates grid-fitted interface glyphs against Chromium's grayscale raster.
    pub fn interface(device: &wgpu::Device, format: wgpu::TextureFormat, capacity: usize) -> Self {
        Self { interface: true, ..Self::new(device, format, capacity, Vec::new()) }
    }

    /// Rasterizes glyphs the atlas lacks and uploads one instance per sprite, so sprite `i` is instance `i`.
    /// `screen` is the target in logical pixels; `scale` is physical pixels per logical pixel.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprites: &[Sprite],
        screen: [f32; 2],
        scale: f32,
    ) {
        self.frame += 1;
        let sprites = &sprites[..sprites.len().min(self.capacity)];
        let largest = LARGEST_SIDE.min(device.limits().max_texture_dimension_2d);
        let shared = &self.shared;
        let first = [FIRST_SIZE[0].min(largest), FIRST_SIZE[1].min(largest)];
        let atlas = self.atlas.get_or_insert_with(|| Atlas::new(device, first, shared));
        let mut repacked = false;
        while !atlas.ensure(&mut self.raster, &self.images, queue, sprites, scale, self.frame) {
            let [width, height] = atlas.size;
            let grown = if height < width.max(largest) {
                [width, (height * 2).min(largest)]
            } else {
                [(width * 2).min(largest), height]
            };
            let (live, _) = atlas.live(self.frame);
            let crowded = live * 2 > u64::from(width) * u64::from(height);
            let size = if repacked || crowded { grown } else { atlas.size };
            if repacked && size == atlas.size {
                tracing::warn!(width, height, "glyph atlas is full; marks that do not fit are skipped this frame");
                break;
            }
            *atlas = atlas.repack(device, queue, size, shared, self.frame);
            repacked = true;
        }
        let mut uniform = [0_u8; 16];
        for (index, value) in [screen[0], screen[1], f32::from(self.interface), 0.0].into_iter().enumerate() {
            uniform[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        queue.write_buffer(&shared.uniform, 0, &uniform);
        self.staging.clear();
        for sprite in sprites {
            for value in atlas.instance(sprite, scale) {
                self.staging.extend_from_slice(&value.to_le_bytes());
            }
        }
        if !self.staging.is_empty() {
            queue.write_buffer(&self.instances, 0, &self.staging);
        }
        self.count = sprites.len() as u32;
        let marks = atlas.slots.values().filter(|slot| slot.width > 0).count();
        let (live_texels, live_marks) = atlas.live(self.frame);
        let area = |size: [u32; 2]| u64::from(size[0]) * u64::from(size[1]);
        if area(atlas.size) > area(self.peak.peak_size) {
            tracing::debug!(width = atlas.size[0], height = atlas.size[1], live_marks, "glyph atlas grown");
        }
        self.peak = AtlasStats {
            size: atlas.size,
            marks,
            live_marks,
            live_texels,
            peak_size: if area(atlas.size) > area(self.peak.peak_size) { atlas.size } else { self.peak.peak_size },
            peak_live_marks: self.peak.peak_live_marks.max(live_marks),
            peak_live_texels: self.peak.peak_live_texels.max(live_texels),
        };
    }

    /// The atlas as of the last `prepare`, with the peaks since this pipeline was made.
    pub fn stats(&self) -> AtlasStats {
        self.peak
    }

    /// Draws sprites `range` (indices into the slice given to `prepare`).
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, range: Range<u32>) {
        let range = range.start.min(self.count)..range.end.min(self.count);
        let Some(atlas) = &self.atlas else { return };
        if range.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &atlas.bind, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, range);
    }

    /// Drops the atlas texture; the next `prepare` starts a new one.
    pub fn release(&mut self) {
        self.atlas = None;
        self.count = 0;
    }
}

fn key_of(sprite: &Sprite, scale: f32) -> Key {
    match sprite.mark {
        Mark::Glyph { glyph, size, outline, max_scale } => Key::Glyph {
            font: glyph.font,
            id: glyph.id,
            weight: glyph.weight,
            flags: glyph.flags,
            quarter_px: (size * scale.min(max_scale) * 4.0).round().max(1.0) as u32,
            outline: (outline * 1000.0).round().max(0.0) as u32,
        },
        Mark::Image(index) => Key::Image(index),
    }
}

/// Coverage bitmap; `left`/`top` place its top-left corner from the pen, y up.
struct Mask {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    left: i32,
    top: i32,
}

impl Mask {
    const EMPTY: Self = Self { pixels: Vec::new(), width: 0, height: 0, left: 0, top: 0 };
}

fn rasterize(cache: &mut SwashCache, system: &mut FontSystem, key: &Key) -> Mask {
    let Key::Glyph { font, id, weight, flags, quarter_px, outline } = *key else { return Mask::EMPTY };
    let px = quarter_px as f32 * 0.25;
    let (cache_key, _, _) = CacheKey::new(font, id, px, (0.0, 0.0), fontdb::Weight(weight), flags);
    let fill = cache.get_image_uncached(system, cache_key).map(mask_of).unwrap_or(Mask::EMPTY);
    if outline == 0 {
        return fill;
    }
    let Some(commands) = cache.get_outline_commands_uncached(system, cache_key) else { return fill };
    stroke(&commands, px * outline as f32 / 1000.0, &fill)
}

fn mask_of_image(image: &Image) -> Mask {
    Mask { pixels: image.pixels.clone(), width: image.width, height: image.height, left: 0, top: 0 }
}

fn mask_of(image: SwashImage) -> Mask {
    let width = image.placement.width;
    let height = image.placement.height;
    let pixels = match image.content {
        SwashContent::Mask => image.data,
        SwashContent::Color => image.data.as_chunks::<4>().0.iter().map(|pixel| pixel[3]).collect(),
        SwashContent::SubpixelMask => image.data.as_chunks::<4>().0.iter().map(|pixel| pixel[1]).collect(),
    };
    Mask { pixels, width, height, left: image.placement.left, top: image.placement.top }
}

/// A `line_width` stroke of the outline (round joins) over its fill at `OUTLINE_FILL` alpha.
fn stroke(commands: &[Command], line_width: f32, fill: &Mask) -> Mask {
    let segments = flatten(commands);
    if segments.is_empty() {
        return Mask::EMPTY;
    }
    let half = line_width * 0.5;
    let reach = half + 1.0;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &[x0, y0, x1, y1] in &segments {
        min_x = min_x.min(x0.min(x1));
        max_x = max_x.max(x0.max(x1));
        min_y = min_y.min(y0.min(y1));
        max_y = max_y.max(y0.max(y1));
    }
    let left = (min_x - reach).floor() as i32;
    let top = (max_y + reach).ceil() as i32;
    let width = ((max_x + reach).ceil() as i32 - left).max(1) as u32;
    let height = (top - (min_y - reach).floor() as i32).max(1) as u32;
    let mut distance = vec![f32::MAX; (width * height) as usize];
    for &[x0, y0, x1, y1] in &segments {
        let column = |x: f32| ((x - left as f32).floor() as i32).clamp(0, width as i32 - 1) as u32;
        let line = |y: f32| ((top as f32 - y).floor() as i32).clamp(0, height as i32 - 1) as u32;
        for row in line(y0.max(y1) + reach)..=line(y0.min(y1) - reach) {
            let py = top as f32 - row as f32 - 0.5;
            for col in column(x0.min(x1) - reach)..=column(x0.max(x1) + reach) {
                let px = left as f32 + col as f32 + 0.5;
                let cell = &mut distance[(row * width + col) as usize];
                *cell = cell.min(segment_distance(px, py, x0, y0, x1, y1));
            }
        }
    }
    let mut pixels = Vec::with_capacity(distance.len());
    for (index, d) in distance.iter().enumerate() {
        let edge = (half + 0.5 - d).clamp(0.0, 1.0);
        let (row, col) = (index as u32 / width, index as u32 % width);
        let under = sample(fill, left + col as i32, top - row as i32) * OUTLINE_FILL;
        pixels.push(((edge + under * (1.0 - edge)) * 255.0).round() as u8);
    }
    Mask { pixels, width, height, left, top }
}

/// Fill coverage at the pixel whose top-left corner is `(x, y)` in pen space, y up.
fn sample(mask: &Mask, x: i32, y: i32) -> f32 {
    let col = x - mask.left;
    let row = mask.top - y;
    if col < 0 || row < 0 || col >= mask.width as i32 || row >= mask.height as i32 {
        return 0.0;
    }
    f32::from(mask.pixels[(row as u32 * mask.width + col as u32) as usize]) / 255.0
}

fn segment_distance(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 { (((px - x0) * dx + (py - y0) * dy) / length).clamp(0.0, 1.0) } else { 0.0 };
    let (ex, ey) = (px - x0 - dx * t, py - y0 - dy * t);
    (ex * ex + ey * ey).sqrt()
}

/// Outline commands as straight segments `[x0, y0, x1, y1]`, curves cut to about 1.5-pixel pieces.
fn flatten(commands: &[Command]) -> Vec<[f32; 4]> {
    let mut segments = Vec::new();
    let mut start = (0.0, 0.0);
    let mut at = (0.0, 0.0);
    for command in commands {
        match *command {
            Command::MoveTo(p) => {
                start = (p.x, p.y);
                at = start;
            }
            Command::LineTo(p) => {
                push(&mut segments, at, (p.x, p.y));
                at = (p.x, p.y);
            }
            Command::QuadTo(c, p) => {
                let (c, p) = ((c.x, c.y), (p.x, p.y));
                let steps = pieces(&[at, c, p]);
                let mut last = at;
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    let u = 1.0 - t;
                    let next = (
                        u * u * at.0 + 2.0 * u * t * c.0 + t * t * p.0,
                        u * u * at.1 + 2.0 * u * t * c.1 + t * t * p.1,
                    );
                    push(&mut segments, last, next);
                    last = next;
                }
                at = p;
            }
            Command::CurveTo(c1, c2, p) => {
                let (c1, c2, p) = ((c1.x, c1.y), (c2.x, c2.y), (p.x, p.y));
                let steps = pieces(&[at, c1, c2, p]);
                let mut last = at;
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    let next = (a * at.0 + b * c1.0 + c * c2.0 + d * p.0, a * at.1 + b * c1.1 + c * c2.1 + d * p.1);
                    push(&mut segments, last, next);
                    last = next;
                }
                at = p;
            }
            Command::Close => {
                if at != start {
                    push(&mut segments, at, start);
                }
                at = start;
            }
        }
    }
    segments
}

fn push(segments: &mut Vec<[f32; 4]>, from: (f32, f32), to: (f32, f32)) {
    segments.push([from.0, from.1, to.0, to.1]);
}

fn pieces(points: &[(f32, f32)]) -> u32 {
    let length: f32 = points.windows(2).map(|pair| (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1)).sum();
    (length / 1.5).ceil().clamp(1.0, 64.0) as u32
}

fn bind_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("folia-glyphs"),
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
        ],
    })
}

fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("folia-glyphs"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

fn pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    group: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("folia-glyphs"),
        bind_group_layouts: &[group],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("folia-glyphs"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: INSTANCE_BYTES as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &ATTRIBUTES,
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs"),
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
    })
}
