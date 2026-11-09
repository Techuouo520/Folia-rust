// The glass blur's picture for canvas-drawn lyric styles. The canvas program hands over the very geometry iced draws
// on screen; this pass tessellates nothing new, it rasterizes that geometry at window size for the blur source.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::PoisonError;

use iced::advanced::graphics::image as gimage;
use iced::advanced::graphics::mesh::Mesh;
use iced::advanced::graphics::text as gtext;
use iced::advanced::text as core_text;
use iced::widget::canvas::Geometry;
use iced::widget::image::Handle;
use iced::widget::shader::{self, Primitive as ShaderPrimitive};
use iced::{Font, Rectangle, Size, mouse};
use parking_lot::Mutex;

use crate::backdrop;
use crate::glyphs::{Glyphs, Mark, ShapedGlyph, Sprite};

const SHADER: &str = include_str!("../shaders/scene.wgsl");
const SCREEN_BYTES: usize = 16;
const SOLID_STRIDE: u64 = 24;
const GRADIENT_STRIDE: u64 = 104;
const IMAGE_STRIDE: u64 = 64;
const MAX_SPRITES: usize = 8192;
const MAX_IMAGES: usize = 16;
const TEXT_CACHE: usize = 64;
const IDLE_TRIMS: u32 = 120;

const SOLID_ATTRS: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 8, shader_location: 1 },
];

const GRADIENT_ATTRS: [wgpu::VertexAttribute; 7] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32x4, offset: 8, shader_location: 1 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32x4, offset: 24, shader_location: 2 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32x4, offset: 40, shader_location: 3 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32x4, offset: 56, shader_location: 4 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32x4, offset: 72, shader_location: 5 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 88, shader_location: 6 },
];

const IMAGE_ATTRS: [wgpu::VertexAttribute; 6] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 8, shader_location: 1 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 24, shader_location: 2 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 40, shader_location: 3 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32, offset: 56, shader_location: 4 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32, offset: 60, shader_location: 5 },
];

/// iced shader that only fills the glass blur source with the canvas picture.
pub struct Program<Message> {
    _message: PhantomData<Message>,
}

impl<Message> Program<Message> {
    pub fn new() -> Self {
        Self { _message: PhantomData }
    }
}

impl<Message> Default for Program<Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Message> shader::Program<Message> for Program<Message> {
    type State = ();
    type Primitive = Pass;

    fn mouse_interaction(&self, _: &(), _: Rectangle, _: mouse::Cursor) -> mouse::Interaction {
        mouse::Interaction::None
    }

    fn draw(&self, _: &(), _: mouse::Cursor, _: Rectangle) -> Self::Primitive {
        Pass
    }
}

#[derive(Debug)]
pub struct Pass;

struct MeshDraw {
    gradient: bool,
    first: u32,
    count: u32,
    clip: [f32; 4],
}

struct ImageDraw {
    handle: Handle,
    bounds: [f32; 4],
    clip: [f32; 4],
    radius: [f32; 4],
    rotation: f32,
    opacity: f32,
}

struct TextDraw {
    first: u32,
    count: u32,
    clip: [f32; 4],
}

struct Placed {
    glyph: ShapedGlyph,
    pen: [f32; 2],
    size: f32,
}

struct TextLayout {
    min: Size,
    glyphs: Vec<Placed>,
}

struct SceneData {
    solid: Vec<u8>,
    solid_indices: Vec<u32>,
    gradient: Vec<u8>,
    gradient_indices: Vec<u32>,
    meshes: Vec<MeshDraw>,
    images: Vec<ImageDraw>,
    texts: Vec<TextDraw>,
    sprites: Vec<Sprite>,
    layouts: Vec<(u64, std::sync::Arc<TextLayout>)>,
    hash: u64,
    frame: u64,
    valid: bool,
}

impl SceneData {
    const fn new() -> Self {
        Self {
            solid: Vec::new(),
            solid_indices: Vec::new(),
            gradient: Vec::new(),
            gradient_indices: Vec::new(),
            meshes: Vec::new(),
            images: Vec::new(),
            texts: Vec::new(),
            sprites: Vec::new(),
            layouts: Vec::new(),
            hash: 0,
            frame: 0,
            valid: false,
        }
    }
}

impl Default for SceneData {
    fn default() -> Self {
        Self::new()
    }
}

static SCENE: Mutex<SceneData> = Mutex::new(SceneData::new());

/// Records the geometry iced is about to draw for a canvas style, for the glass blur of the same frame.
pub fn capture(geometries: &[Geometry]) {
    let mut data = SCENE.lock();
    clear(&mut data);
    data.valid = false;
    if !backdrop::was_wanted() {
        return;
    }
    data.frame = data.frame.wrapping_add(1);
    let mut hash = Fnv(0xcbf2_9ce4_8422_2325);
    for geometry in geometries {
        let iced_wgpu::geometry::Geometry::Live { meshes, images, text } = geometry else { continue };
        for mesh in meshes {
            capture_mesh(&mut data, mesh, &mut hash);
        }
        for image in images {
            capture_image(&mut data, image, &mut hash);
        }
        for text in text {
            capture_text(&mut data, text, &mut hash);
        }
    }
    data.hash = hash.finish();
    data.valid = true;
}

fn clear(data: &mut SceneData) {
    data.solid.clear();
    data.solid_indices.clear();
    data.gradient.clear();
    data.gradient_indices.clear();
    data.meshes.clear();
    data.images.clear();
    data.texts.clear();
    data.sprites.clear();
}

fn capture_mesh(data: &mut SceneData, mesh: &Mesh, hash: &mut Fnv) {
    match mesh {
        Mesh::Solid { buffers, clip_bounds, .. } => {
            let base = (data.solid.len() / SOLID_STRIDE as usize) as u32;
            data.solid.extend_from_slice(bytemuck::cast_slice(&buffers.vertices));
            let count = buffers.indices.len() as u32;
            data.solid_indices.extend(buffers.indices.iter().map(|index| index + base));
            hash.write(bytemuck::cast_slice(&buffers.vertices));
            hash.write(&buffers.indices.len().to_le_bytes());
            hash_clip(hash, *clip_bounds);
            data.meshes.push(MeshDraw {
                gradient: false,
                first: data.solid_indices.len() as u32 - count,
                count,
                clip: clip(*clip_bounds),
            });
        }
        Mesh::Gradient { buffers, clip_bounds, .. } => {
            let base = (data.gradient.len() / GRADIENT_STRIDE as usize) as u32;
            data.gradient.extend_from_slice(bytemuck::cast_slice(&buffers.vertices));
            let count = buffers.indices.len() as u32;
            data.gradient_indices.extend(buffers.indices.iter().map(|index| index + base));
            hash.write(bytemuck::cast_slice(&buffers.vertices));
            hash.write(&buffers.indices.len().to_le_bytes());
            hash_clip(hash, *clip_bounds);
            data.meshes.push(MeshDraw {
                gradient: true,
                first: data.gradient_indices.len() as u32 - count,
                count,
                clip: clip(*clip_bounds),
            });
        }
    }
}

fn hash_clip(hash: &mut Fnv, rect: Rectangle) {
    for value in [rect.x, rect.y, rect.width, rect.height] {
        hash.write_u64(u64::from(value.to_bits()));
    }
}

fn capture_image(data: &mut SceneData, image: &gimage::Image, hash: &mut Fnv) {
    let gimage::Image::Raster { image, bounds, clip_bounds } = image else { return };
    let id = image.handle.id();
    hash.write_u64(pack_id(&id));
    hash.write(&bounds.x.to_bits().to_le_bytes());
    hash.write(&bounds.y.to_bits().to_le_bytes());
    hash.write(&bounds.width.to_bits().to_le_bytes());
    hash.write(&bounds.height.to_bits().to_le_bytes());
    hash.write_u64(u64::from(image.rotation.0.to_bits()));
    hash.write_u64(u64::from(image.opacity.to_bits()));
    for radius in [
        image.border_radius.top_left,
        image.border_radius.top_right,
        image.border_radius.bottom_right,
        image.border_radius.bottom_left,
    ] {
        hash.write_u64(u64::from(radius.to_bits()));
    }
    hash_clip(hash, *clip_bounds);
    data.images.push(ImageDraw {
        handle: image.handle.clone(),
        bounds: [bounds.x, bounds.y, bounds.width, bounds.height],
        clip: clip(*clip_bounds),
        radius: [
            image.border_radius.top_left,
            image.border_radius.top_right,
            image.border_radius.bottom_right,
            image.border_radius.bottom_left,
        ],
        rotation: image.rotation.0,
        opacity: image.opacity,
    });
}

fn pack_id(id: &iced::advanced::image::Id) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

fn capture_text(data: &mut SceneData, text: &gtext::Text, hash: &mut Fnv) {
    let gtext::Text::Cached { content, bounds, color, size, line_height, font, align_x, align_y, shaping, clip_bounds } =
        text
    else {
        return;
    };
    let request = TextRequest {
        key: text_key(content, *size, *line_height, *font, *align_x, *bounds, *shaping),
        content,
        size: *size,
        line_height: *line_height,
        font: *font,
        align_x: *align_x,
        bounds: *bounds,
        shaping: *shaping,
    };
    hash.write_u64(request.key);
    for value in [color.r, color.g, color.b, color.a] {
        hash.write_u64(u64::from(value.to_bits()));
    }
    hash.write_u64(match align_y {
        iced::alignment::Vertical::Top => 0,
        iced::alignment::Vertical::Center => 1,
        iced::alignment::Vertical::Bottom => 2,
    });
    hash_clip(hash, *clip_bounds);
    let first = data.sprites.len() as u32;
    let layout = layout_of(data, &request);
    let left = match align_x {
        core_text::Alignment::Center => bounds.x - layout.min.width / 2.0,
        core_text::Alignment::Right => bounds.x - layout.min.width,
        _ => bounds.x,
    };
    let top = match align_y {
        iced::alignment::Vertical::Center => bounds.y - layout.min.height / 2.0,
        iced::alignment::Vertical::Bottom => bounds.y - layout.min.height,
        iced::alignment::Vertical::Top => bounds.y,
    };
    let ink = [color.r, color.g, color.b, color.a];
    for placed in &layout.glyphs {
        data.sprites.push(Sprite {
            mark: Mark::Glyph { glyph: placed.glyph, size: placed.size, outline: 0.0, max_scale: 3.0 },
            anchor: [left, top],
            pen: placed.pen,
            axes: [1.0, 0.0, 0.0, 1.0],
            color: ink,
        });
    }
    data.texts.push(TextDraw { first, count: layout.glyphs.len() as u32, clip: clip(*clip_bounds) });
}

fn text_key(
    content: &str,
    size: iced::Pixels,
    line_height: iced::Pixels,
    font: Font,
    align_x: core_text::Alignment,
    bounds: Rectangle,
    shaping: core_text::Shaping,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    size.0.to_bits().hash(&mut hasher);
    line_height.0.to_bits().hash(&mut hasher);
    font.hash(&mut hasher);
    align_x.hash(&mut hasher);
    bounds.x.to_bits().hash(&mut hasher);
    bounds.y.to_bits().hash(&mut hasher);
    bounds.width.to_bits().hash(&mut hasher);
    shaping.hash(&mut hasher);
    hasher.finish()
}

struct TextRequest<'a> {
    key: u64,
    content: &'a str,
    size: iced::Pixels,
    line_height: iced::Pixels,
    font: Font,
    align_x: core_text::Alignment,
    bounds: Rectangle,
    shaping: core_text::Shaping,
}

fn layout_of(data: &mut SceneData, request: &TextRequest<'_>) -> std::sync::Arc<TextLayout> {
    let TextRequest { key, content, size, line_height, font, align_x, bounds, shaping } = *request;
    if let Some((_, layout)) = data.layouts.iter().find(|(held, _)| *held == key) {
        return layout.clone();
    }
    let px = size.0;
    let line = line_height.0.max(f32::MIN_POSITIVE);
    let mut fonts = gtext::font_system().write().unwrap_or_else(PoisonError::into_inner);
    let system = fonts.raw();
    let mut buffer = gtext::cosmic_text::Buffer::new(system, gtext::cosmic_text::Metrics::new(px, line));
    buffer.set_size(system, Some(bounds.width), Some(bounds.height.max(line)));
    buffer.set_text(system, content, &gtext::to_attributes(font), gtext::to_shaping(shaping, content), None);
    let min = gtext::align(&mut buffer, system, align_x);
    let mut glyphs = Vec::new();
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let y = run.line_y + glyph.y;
            glyphs.push(Placed {
                glyph: ShapedGlyph {
                    font: glyph.font_id,
                    id: glyph.glyph_id,
                    weight: glyph.font_weight.0,
                    flags: glyph.cache_key_flags,
                    cluster: glyph.start,
                    x: glyph.x,
                    y,
                },
                pen: [glyph.x, y],
                size: glyph.font_size,
            });
        }
    }
    drop(fonts);
    let layout = std::sync::Arc::new(TextLayout { min, glyphs });
    if data.layouts.len() == TEXT_CACHE {
        data.layouts.remove(0);
    }
    data.layouts.push((key, layout.clone()));
    layout
}

fn clip(rect: Rectangle) -> [f32; 4] {
    [rect.x, rect.y, rect.width, rect.height]
}

struct Fnv(u64);

impl Fnv {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

struct Texture {
    bind: wgpu::BindGroup,
    seen: u64,
}

struct ImageGpu {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: HashMap<u64, Texture>,
}

struct Targets {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

pub struct SceneGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    solid: wgpu::RenderPipeline,
    gradient: wgpu::RenderPipeline,
    image: wgpu::RenderPipeline,
    screen: wgpu::Buffer,
    screen_bind: wgpu::BindGroup,
    solid_verts: wgpu::Buffer,
    solid_cap: u64,
    solid_indices: wgpu::Buffer,
    solid_index_cap: u64,
    gradient_verts: wgpu::Buffer,
    gradient_cap: u64,
    gradient_indices: wgpu::Buffer,
    gradient_index_cap: u64,
    images: ImageGpu,
    image_instances: wgpu::Buffer,
    image_cap: u64,
    image_staging: Vec<[f32; 16]>,
    binds: Vec<Option<wgpu::BindGroup>>,
    glyphs: Glyphs,
    targets: Option<Targets>,
    window: [f32; 2],
    scale: f32,
    stamp: u64,
    valid: bool,
    idle: u32,
}

impl shader::Pipeline for SceneGpu {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-scene"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let screen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-scene-screen"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-scene-image"),
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
        let screen = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-scene-screen"),
            size: SCREEN_BYTES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let screen_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-scene-screen"),
            layout: &screen_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: screen.as_entire_binding() }],
        });
        let solid = pipeline(
            device,
            &module,
            format,
            Pipe {
                groups: &[&screen_layout],
                vertex: "solid_vs",
                fragment: "solid_fs",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: SOLID_STRIDE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &SOLID_ATTRS,
                }],
                blend: wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            },
        );
        let gradient = pipeline(
            device,
            &module,
            format,
            Pipe {
                groups: &[&screen_layout],
                vertex: "gradient_vs",
                fragment: "gradient_fs",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: GRADIENT_STRIDE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &GRADIENT_ATTRS,
                }],
                blend: wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            },
        );
        let image = pipeline(
            device,
            &module,
            format,
            Pipe {
                groups: &[&screen_layout, &image_layout],
                vertex: "image_vs",
                fragment: "image_fs",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: IMAGE_STRIDE,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &IMAGE_ATTRS,
                }],
                blend: wgpu::BlendState::ALPHA_BLENDING,
            },
        );
        let solid_verts = buffer(device, "folia-scene-solid", 4096, wgpu::BufferUsages::VERTEX);
        let solid_indices = buffer(device, "folia-scene-solid-index", 4096, wgpu::BufferUsages::INDEX);
        let gradient_verts = buffer(device, "folia-scene-gradient", 4096, wgpu::BufferUsages::VERTEX);
        let gradient_indices = buffer(device, "folia-scene-gradient-index", 4096, wgpu::BufferUsages::INDEX);
        let image_instances = buffer(device, "folia-scene-images", 4096, wgpu::BufferUsages::VERTEX);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-scene-image"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..wgpu::SamplerDescriptor::default()
        });
        Self {
            device: device.clone(),
            queue: queue.clone(),
            format,
            solid,
            gradient,
            image,
            screen,
            screen_bind,
            solid_verts,
            solid_cap: 4096,
            solid_indices,
            solid_index_cap: 4096,
            gradient_verts,
            gradient_cap: 4096,
            gradient_indices,
            gradient_index_cap: 4096,
            images: ImageGpu { layout: image_layout, sampler, textures: HashMap::new() },
            image_instances,
            image_cap: 4096,
            image_staging: Vec::new(),
            binds: Vec::new(),
            glyphs: Glyphs::new(device, format, MAX_SPRITES, Vec::new()),
            targets: None,
            window: [1.0, 1.0],
            scale: 1.0,
            stamp: 0,
            valid: false,
            idle: 0,
        }
    }

    fn trim(&mut self) {
        self.idle = self.idle.saturating_add(1);
        if self.idle == IDLE_TRIMS {
            self.release();
        }
    }
}

impl SceneGpu {
    fn release(&mut self) {
        self.targets = None;
        self.glyphs.release();
        self.images.textures.clear();
        self.binds.clear();
        tracing::debug!("style scene textures released");
    }

    fn ensure_targets(&mut self, width: u32, height: u32) {
        if self.targets.as_ref().is_some_and(|targets| targets.width == width && targets.height == height) {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-scene"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.targets = Some(Targets { _texture: texture, view, width, height });
    }
}

impl ShaderPrimitive for Pass {
    type Pipeline = SceneGpu;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        let gpu = pipeline;
        gpu.window = [bounds.width.max(1.0), bounds.height.max(1.0)];
        gpu.scale = viewport.scale_factor();
        backdrop::set_window(gpu.window);
        let data = SCENE.lock();
        gpu.stamp = data.hash;
        gpu.valid = data.valid;
        if !data.valid {
            gpu.idle = gpu.idle.saturating_add(1);
            if gpu.idle == IDLE_TRIMS {
                gpu.release();
            }
            return;
        }
        gpu.idle = 0;
        let width = (gpu.window[0] * gpu.scale).round().max(1.0) as u32;
        let height = (gpu.window[1] * gpu.scale).round().max(1.0) as u32;
        gpu.ensure_targets(width, height);
        write_screen(gpu, queue);
        gpu.glyphs.prepare(device, queue, &data.sprites, gpu.window, gpu.scale);
        upload(device, queue, &mut gpu.solid_verts, &mut gpu.solid_cap, &data.solid, wgpu::BufferUsages::VERTEX);
        upload(
            device,
            queue,
            &mut gpu.solid_indices,
            &mut gpu.solid_index_cap,
            bytemuck::cast_slice(&data.solid_indices),
            wgpu::BufferUsages::INDEX,
        );
        upload(
            device,
            queue,
            &mut gpu.gradient_verts,
            &mut gpu.gradient_cap,
            &data.gradient,
            wgpu::BufferUsages::VERTEX,
        );
        upload(
            device,
            queue,
            &mut gpu.gradient_indices,
            &mut gpu.gradient_index_cap,
            bytemuck::cast_slice(&data.gradient_indices),
            wgpu::BufferUsages::INDEX,
        );
        prepare_images(gpu, device, queue, &data);
    }

    fn draw(&self, _: &Self::Pipeline, _: &mut wgpu::RenderPass<'_>) -> bool {
        false
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        _: &wgpu::TextureView,
        _: &Rectangle<u32>,
    ) {
        if !backdrop::wanted() || !pipeline.valid {
            return;
        }
        let gpu = pipeline;
        let Some(targets) = gpu.targets.as_ref() else { return };
        let data = SCENE.lock();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("folia-scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            for mesh in &data.meshes {
                let (pipe, verts, indices) = if mesh.gradient {
                    (&gpu.gradient, &gpu.gradient_verts, &gpu.gradient_indices)
                } else {
                    (&gpu.solid, &gpu.solid_verts, &gpu.solid_indices)
                };
                set_scissor(&mut pass, mesh.clip, gpu.scale, targets.width, targets.height);
                pass.set_pipeline(pipe);
                pass.set_bind_group(0, &gpu.screen_bind, &[]);
                pass.set_vertex_buffer(0, verts.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(mesh.first..mesh.first + mesh.count, 0, 0..1);
            }
            for (index, image) in data.images.iter().enumerate() {
                let Some(bind) = gpu.binds.get(index).and_then(Option::as_ref) else { continue };
                set_scissor(&mut pass, image.clip, gpu.scale, targets.width, targets.height);
                pass.set_pipeline(&gpu.image);
                pass.set_bind_group(0, &gpu.screen_bind, &[]);
                pass.set_bind_group(1, bind, &[]);
                pass.set_vertex_buffer(0, gpu.image_instances.slice(..));
                pass.draw(0..6, index as u32..index as u32 + 1);
            }
            for text in &data.texts {
                if text.count == 0 {
                    continue;
                }
                set_scissor(&mut pass, text.clip, gpu.scale, targets.width, targets.height);
                gpu.glyphs.draw(&mut pass, text.first..text.first + text.count);
            }
        }
        backdrop::set_window(gpu.window);
        let stamp = gpu.stamp;
        backdrop::paint_external(&gpu.device, &gpu.queue, encoder, gpu.format, &targets.view, stamp, gpu.window);
    }
}

fn set_scissor(pass: &mut wgpu::RenderPass<'_>, clip: [f32; 4], scale: f32, width: u32, height: u32) {
    let x = (clip[0] * scale).floor().clamp(0.0, width as f32) as u32;
    let y = (clip[1] * scale).floor().clamp(0.0, height as f32) as u32;
    let right = ((clip[0] + clip[2]) * scale).ceil().clamp(0.0, width as f32) as u32;
    let bottom = ((clip[1] + clip[3]) * scale).ceil().clamp(0.0, height as f32) as u32;
    pass.set_scissor_rect(x, y, right.saturating_sub(x), bottom.saturating_sub(y));
}

struct Pipe<'a> {
    groups: &'a [&'a wgpu::BindGroupLayout],
    vertex: &'static str,
    fragment: &'static str,
    buffers: &'a [wgpu::VertexBufferLayout<'a>],
    blend: wgpu::BlendState,
}

fn pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    pipe: Pipe<'_>,
) -> wgpu::RenderPipeline {
    let Pipe { groups, vertex, fragment, buffers, blend } = pipe;
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(fragment),
        bind_group_layouts: groups,
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(vertex),
            buffers,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fragment),
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

fn buffer(device: &wgpu::Device, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &mut wgpu::Buffer,
    capacity: &mut u64,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
) {
    if bytes.is_empty() {
        return;
    }
    if bytes.len() as u64 > *capacity {
        *capacity = (bytes.len() as u64).next_power_of_two();
        *target = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-scene-buffer"),
            size: *capacity,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }
    queue.write_buffer(target, 0, bytes);
}

fn write_screen(gpu: &SceneGpu, queue: &wgpu::Queue) {
    let words = [gpu.window[0], gpu.window[1], gpu.scale, 0.0];
    queue.write_buffer(&gpu.screen, 0, bytemuck::cast_slice(&words));
}

fn prepare_images(gpu: &mut SceneGpu, device: &wgpu::Device, queue: &wgpu::Queue, data: &SceneData) {
    let frame = data.frame;
    gpu.binds.clear();
    gpu.binds.resize(data.images.len(), None);
    let mut instances = std::mem::take(&mut gpu.image_staging);
    instances.clear();
    instances.reserve(data.images.len());
    for (index, draw) in data.images.iter().enumerate() {
        let mut instance = [0.0_f32; 16];
        if let Handle::Rgba { width, height, pixels, .. } = &draw.handle {
            let key = pack_id(&draw.handle.id());
            if !gpu.images.textures.contains_key(&key) {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("folia-scene-image"),
                    size: wgpu::Extent3d { width: *width, height: *height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(*width * 4),
                        rows_per_image: Some(*height),
                    },
                    wgpu::Extent3d { width: *width, height: *height, depth_or_array_layers: 1 },
                );
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("folia-scene-image"),
                    layout: &gpu.images.layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&gpu.images.sampler),
                        },
                    ],
                });
                gpu.images.textures.insert(key, Texture { bind, seen: frame });
            }
            if let Some(texture) = gpu.images.textures.get_mut(&key) {
                texture.seen = frame;
                gpu.binds[index] = Some(texture.bind.clone());
                let center = [draw.bounds[0] + draw.bounds[2] / 2.0, draw.bounds[1] + draw.bounds[3] / 2.0];
                instance = [
                    center[0],
                    center[1],
                    draw.bounds[0],
                    draw.bounds[1],
                    draw.bounds[2],
                    draw.bounds[3],
                    draw.clip[0],
                    draw.clip[1],
                    draw.clip[2],
                    draw.clip[3],
                    draw.radius[0],
                    draw.radius[1],
                    draw.radius[2],
                    draw.radius[3],
                    draw.rotation,
                    draw.opacity,
                ];
            }
        }
        instances.push(instance);
    }
    if !instances.is_empty() {
        let bytes = bytemuck::cast_slice(&instances);
        upload(device, queue, &mut gpu.image_instances, &mut gpu.image_cap, bytes, wgpu::BufferUsages::VERTEX);
    }
    gpu.image_staging = instances;
    if gpu.images.textures.len() > MAX_IMAGES {
        let mut stale: Vec<(u64, u64)> = gpu
            .images
            .textures
            .iter()
            .filter(|(_, texture)| texture.seen != frame)
            .map(|(key, texture)| (*key, texture.seen))
            .collect();
        stale.sort_by_key(|(_, seen)| *seen);
        for (key, _) in stale.into_iter().take(gpu.images.textures.len() - MAX_IMAGES) {
            gpu.images.textures.remove(&key);
        }
    }
}
