// Isolates a lyric widget in a native iced render target while retaining its logical layout and input.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use folia_core::{LyricAntialias, LyricRenderQuality};
use iced::advanced::image::{Allocation, Handle, Renderer as _};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, renderer, widget};
use iced::{Element, Event, Length, Rectangle, Size, Vector, mouse};
use iced_wgpu::graphics::{self, Viewport, core::Renderer as _};
use iced_wgpu::primitive::Renderer as _;
use parking_lot::Mutex;

static DEVICES: Mutex<Vec<Weak<DeviceRegistration>>> = Mutex::new(Vec::new());

pub const ANTIALIAS_SHADER: &str = include_str!("../shaders/antialias.wgsl");

#[cfg(feature = "dev")]
static TARGETS: Mutex<Vec<(usize, [u32; 2])>> = Mutex::new(Vec::new());
#[cfg(feature = "dev")]
static NEXT_TARGET: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

/// Actual allocated lyric targets, for offline resolution and lifetime checks.
#[cfg(feature = "dev")]
pub fn target_sizes() -> Vec<[u32; 2]> {
    TARGETS.lock().iter().map(|(_, size)| *size).collect()
}

/// Keeps an adapter available to independent lyric engines for the lifetime of its compositor.
pub struct DeviceRegistration {
    device: wgpu::Device,
    adapter: wgpu::Adapter,
    shell: graphics::Shell,
}

pub fn register_device(
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    shell: graphics::Shell,
) -> Arc<DeviceRegistration> {
    let registration = Arc::new(DeviceRegistration { device: device.clone(), adapter: adapter.clone(), shell });
    let mut devices = DEVICES.lock();
    devices.retain(|entry| entry.strong_count() > 0);
    devices.push(Arc::downgrade(&registration));
    registration
}

/// Renders the complete widget at the requested pixel density. The default stays on the direct path.
pub fn layer<'a, M: 'a>(content: Element<'a, M>, quality: LyricRenderQuality, publish_blur: bool) -> Element<'a, M> {
    if quality.normalized() == LyricRenderQuality::default() {
        return content;
    }
    Element::new(Layer {
        content,
        quality: quality.normalized(),
        publish_blur,
        wall: None,
        posters: None,
        images: None,
    })
}

/// Captures a wall composition for glass, or blurs the player surface in its own texture before the wall is drawn.
/// `None` draws directly while keeping the same widget tree across capture changes.
pub fn wall_layer<'a, M: 'a>(
    content: Element<'a, M>,
    mode: Option<WallLayer>,
    images: Option<ImageSources<'a>>,
) -> Element<'a, M> {
    Element::new(Layer {
        content,
        quality: LyricRenderQuality::default(),
        publish_blur: false,
        wall: mode,
        posters: None,
        images,
    })
}

/// One visible poster transformed from wall coordinates to texture UVs.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PosterRegion {
    pub center: [f32; 2],
    pub x_axis: [f32; 2],
    pub y_axis: [f32; 2],
    pub amount: f32,
    pub padding: f32,
}

type PosterRegions<'a> = Box<dyn Fn(&mut Vec<PosterRegion>, &mut Vec<Handle>) + 'a>;
type ImageSources<'a> = Box<dyn Fn(&mut Vec<Handle>) + 'a>;

/// Applies CSS saturation and brightness to the composed poster, including its artwork and lettering.
/// The callback also supplies asynchronous images that must be resident before switching to this renderer.
pub fn poster_surface<'a, M: 'a>(
    content: Element<'a, M>,
    regions: impl Fn(&mut Vec<PosterRegion>, &mut Vec<Handle>) + 'a,
) -> Element<'a, M> {
    Element::new(Layer {
        content,
        quality: LyricRenderQuality::default(),
        publish_blur: false,
        wall: None,
        posters: Some(Box::new(regions)),
        images: None,
    })
}

#[derive(Clone, Copy, PartialEq)]
pub enum WallLayer {
    GlassSource,
    BlurredPlayer,
}

struct Layer<'a, M> {
    content: Element<'a, M>,
    quality: LyricRenderQuality,
    publish_blur: bool,
    wall: Option<WallLayer>,
    posters: Option<PosterRegions<'a>>,
    images: Option<ImageSources<'a>>,
}

struct State {
    frame: Offscreen,
    antialias: LyricAntialias,
    wall: Option<WallLayer>,
}

#[derive(Default)]
struct Frame {
    owner: Arc<AtomicBool>,
    renderer: Option<crate::Prepared<iced_wgpu::Renderer>>,
    target: Option<Target>,
    antialias: LyricAntialias,
    device: Option<wgpu::Device>,
    stamp: u64,
    bounds: Rectangle,
    physical: Rectangle,
    regions: Vec<PosterRegion>,
    images: PosterImages,
}

#[derive(Default)]
struct PosterImages {
    handles: Vec<Handle>,
    uploads: Vec<PosterUpload>,
}

struct PosterUpload {
    handle: Handle,
    result: Arc<Mutex<Option<Result<Allocation, iced::advanced::image::Error>>>>,
}

impl PosterImages {
    fn pending(&self) -> bool {
        self.uploads.iter().any(|upload| upload.result.lock().is_none())
    }

    fn ready(&mut self, renderer: &mut iced::Renderer) -> bool {
        self.uploads.retain(|upload| self.handles.iter().any(|shown| shown.id() == upload.handle.id()));
        for handle in &self.handles {
            if !self.uploads.iter().any(|upload| upload.handle.id() == handle.id()) {
                let result = Arc::new(Mutex::new(None));
                let ready = result.clone();
                renderer.allocate_image(handle, move |allocation| {
                    if let Err(err) = &allocation {
                        tracing::warn!(error = err as &dyn std::error::Error, "poster filter image upload failed");
                    }
                    *ready.lock() = Some(allocation);
                });
                self.uploads.push(PosterUpload { handle: handle.clone(), result });
            }
            // Poll the renderer's image worker without waiting on the GPU or decoding on this thread.
            let _ = renderer.measure_image(handle);
        }
        self.uploads.iter().all(|upload| upload.result.lock().as_ref().is_some_and(Result::is_ok))
    }
}

struct Target {
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
    blurred: Option<(crate::Pyramid, Option<wgpu::BindGroup>)>,
    poster_filter: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    size: Size<u32>,
    #[cfg(feature = "dev")]
    id: usize,
}

impl Drop for Target {
    fn drop(&mut self) {
        crate::release_source(&self.view);
        #[cfg(feature = "dev")]
        TARGETS.lock().retain(|(id, _)| *id != self.id);
    }
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for Layer<'_, M> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn tag(&self) -> widget::tree::Tag {
        if self.posters.is_some() { widget::tree::Tag::of::<PosterRegion>() } else { widget::tree::Tag::of::<State>() }
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State {
            frame: Offscreen::default(),
            antialias: self.quality.antialias,
            wall: self.wall,
        })
    }

    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state.antialias != self.quality.antialias {
            state.frame = Offscreen::default();
            state.antialias = self.quality.antialias;
            tree.children[0] = widget::Tree::new(&self.content);
        } else if state.wall != self.wall {
            state.frame = Offscreen::default();
        }
        state.wall = self.wall;
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn layout(&mut self, tree: &mut widget::Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        if self.wall.is_none() && self.posters.is_none() && self.quality == LyricRenderQuality::default() {
            self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
            return;
        }
        if let Some(images) = &self.images {
            let mut frame = state.frame.0.lock();
            frame.images.handles.clear();
            images(&mut frame.images.handles);
        }
        if let Some(regions) = &self.posters {
            let mut frame = state.frame.0.lock();
            frame.regions.clear();
            frame.images.handles.clear();
            let Frame { regions: faces, images, .. } = &mut *frame;
            regions(faces, &mut images.handles);
            if frame.regions.is_empty() {
                drop(frame);
                self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
                return;
            }
        }
        let ready = state.frame.record(bounds, self.quality.antialias, |inner| {
            self.content.as_widget().draw(&tree.children[0], inner, theme, style, layout, cursor, viewport);
        });
        if !ready && (self.wall.is_some() || self.posters.is_some()) {
            self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
        }
        renderer.draw_primitive(
            bounds,
            Pass {
                frame: state.frame.clone(),
                quality: self.quality,
                ready,
                publish_blur: self.publish_blur,
                wall: self.wall,
            },
        );
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let state = tree.state.downcast_ref::<State>();
        if (self.wall.is_some()
            || self.quality != LyricRenderQuality::default()
            || (self.posters.is_some() && !state.frame.0.lock().regions.is_empty()))
            && state.frame.pending()
        {
            shell.request_redraw_at(std::time::Instant::now() + std::time::Duration::from_millis(16));
        }
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'a, M, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

struct Pass {
    frame: Offscreen,
    quality: LyricRenderQuality,
    ready: bool,
    publish_blur: bool,
    wall: Option<WallLayer>,
}

impl std::fmt::Debug for Pass {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("LyricLayer").field("quality", &self.quality).field("ready", &self.ready).finish()
    }
}

pub struct OffscreenDevice {
    adapter: Option<wgpu::Adapter>,
    shell: graphics::Shell,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

struct Pipeline {
    target: OffscreenDevice,
    plain: wgpu::RenderPipeline,
    poster: wgpu::RenderPipeline,
    poster_layout: wgpu::BindGroupLayout,
    antialias: wgpu::RenderPipeline,
}

impl iced_wgpu::primitive::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let target = OffscreenDevice::new(device, queue, format);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-lyric-quality"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("../shaders/antialias.wgsl"),
                    include_str!("../shaders/poster_color.wgsl"),
                    include_str!("../shaders/quality.wgsl")
                )
                .into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-lyric-quality"),
            bind_group_layouts: &[&target.layout],
            push_constant_ranges: &[],
        });
        let poster_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-poster-regions"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let filtered_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-poster-filter"),
            bind_group_layouts: &[&target.layout, &poster_layout],
            push_constant_ranges: &[],
        });
        let create = |fragment, layout| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("folia-lyric-quality"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fragment),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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
            target,
            plain: create("plain", &pipeline_layout),
            antialias: create("antialias", &pipeline_layout),
            poster: create("poster_filter", &filtered_layout),
            poster_layout,
        }
    }
}

impl iced_wgpu::primitive::Primitive for Pass {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        self.frame.prepare(&pipeline.target, bounds, viewport, self.quality, self.ready);
        if !self.ready {
            return;
        }
        let mut frame = self.frame.0.lock();
        let Frame { target: source, regions, .. } = &mut *frame;
        let Some(source) = source else { return };
        if !regions.is_empty() {
            let packed = bytemuck::cast_slice::<PosterRegion, u8>(regions);
            let needed = (16 + packed.len()) as u64;
            let gpu = source.poster_filter.get_or_insert_with(|| poster_buffer(pipeline, needed));
            if gpu.0.size() < needed {
                *gpu = poster_buffer(pipeline, needed);
            }
            let header = [regions.len() as u32, u32::from(pipeline.target.format.is_srgb()), 0, 0];
            pipeline.target.queue.write_buffer(&gpu.0, 0, bytemuck::cast_slice(&header));
            pipeline.target.queue.write_buffer(&gpu.0, 16, packed);
        }
    }

    fn draw(&self, pipeline: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let frame = self.frame.0.lock();
        if frame.regions.is_empty() {
            return false;
        }
        // Keep poster filters in submission order with the wall's vignette and grain.
        if self.ready
            && let Some(source) = &frame.target
            && let Some((_, params)) = &source.poster_filter
        {
            pass.set_pipeline(&pipeline.poster);
            pass.set_bind_group(0, &source.bind, &[]);
            pass.set_bind_group(1, params, &[]);
            pass.draw(0..3, 0..1);
        }
        true
    }

    fn render(
        &self,
        pipeline: &Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip: &Rectangle<u32>,
    ) {
        if !self.ready {
            return;
        }
        let mut frame = self.frame.0.lock();
        let physical = frame.physical;
        let bounds = frame.bounds;
        let stamp = frame.stamp;
        let Some(source) = &mut frame.target else { return };
        let device = &pipeline.target.device;
        let bind = if self.wall == Some(WallLayer::BlurredPlayer) {
            let (pyramid, bind) =
                source.blurred.get_or_insert_with(|| (crate::Pyramid::new(device, pipeline.target.format), None));
            let level = crate::levels_for_radius(24.0);
            let rebuilt = pyramid.ensure(device, &pipeline.target.queue, source.size.width, source.size.height, level);
            pyramid.copy_into_source(device, encoder, &source.view);
            pyramid.blur(device, encoder, &[level]);
            if rebuilt || bind.is_none() {
                *bind = pyramid.output(0).map(|view| pipeline.target.bind(view));
            }
            bind.as_ref().unwrap_or(&source.bind)
        } else {
            source.blurred = None;
            &source.bind
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("folia-lyric-quality"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_viewport(physical.x, physical.y, physical.width, physical.height, 0.0, 1.0);
            pass.set_scissor_rect(clip.x, clip.y, clip.width, clip.height);
            pass.set_pipeline(if self.quality.antialias == LyricAntialias::On {
                &pipeline.antialias
            } else {
                &pipeline.plain
            });
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
        if self.wall == Some(WallLayer::GlassSource) {
            crate::paint_external(
                device,
                &pipeline.target.queue,
                encoder,
                pipeline.target.format,
                &source.view,
                stamp,
                [bounds.width, bounds.height],
            );
        } else if self.publish_blur {
            crate::paint_over_underlay(
                &pipeline.target.device,
                &pipeline.target.queue,
                encoder,
                pipeline.target.format,
                crate::UnderlayOverlay {
                    foreground: &source.view,
                    stamp,
                    window: [bounds.width, bounds.height],
                    veil: [0.0; 4],
                },
            );
        }
    }
}

fn poster_buffer(pipeline: &Pipeline, needed: u64) -> (wgpu::Buffer, wgpu::BindGroup) {
    let device = &pipeline.target.device;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("folia-hover-faces"),
        size: needed.next_power_of_two(),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("folia-hover-faces"),
        layout: &pipeline.poster_layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
    });
    (buffer, bind)
}

impl OffscreenDevice {
    fn bind(&self, view: &wgpu::TextureView) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-lyric-quality"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    fn target(&self, size: Size<u32>) -> Target {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-lyric-quality"),
            size: wgpu::Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind = self.bind(&view);
        #[cfg(feature = "dev")]
        let id = NEXT_TARGET.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "dev")]
        TARGETS.lock().push((id, [size.width, size.height]));
        Target {
            view,
            bind,
            blurred: None,
            poster_filter: None,
            size,
            #[cfg(feature = "dev")]
            id,
        }
    }
}

/// A native widget's isolated render texture, reusable by flat lyrics and scene projections.
#[derive(Clone, Default)]
pub struct Offscreen(Arc<Mutex<Frame>>);

impl Offscreen {
    pub fn revision(&self) -> u64 {
        self.0.lock().stamp
    }
    pub fn pending(&self) -> bool {
        let frame = self.0.lock();
        !frame.owner.load(Ordering::Acquire) || frame.images.pending()
    }

    pub fn record(&self, bounds: Rectangle, antialias: LyricAntialias, draw: impl FnOnce(&mut iced::Renderer)) -> bool {
        let mut frame = self.0.lock();
        if frame.antialias != antialias {
            return false;
        }
        let Frame { renderer, images, .. } = &mut *frame;
        let Some(inner) = renderer.as_mut().and_then(crate::Prepared::poll) else { return false };
        if !images.ready(inner) {
            return false;
        }
        inner.reset(Rectangle::with_size(bounds.size()));
        inner.with_translation(Vector::new(-bounds.x, -bounds.y), draw);
        true
    }

    pub fn with_target<T>(&self, draw: impl FnOnce(&wgpu::TextureView, &wgpu::BindGroup, Size<u32>) -> T) -> Option<T> {
        let frame = self.0.lock();
        frame.target.as_ref().map(|target| draw(&target.view, &target.bind, target.size))
    }

    pub fn prepare(
        &self,
        pipeline: &OffscreenDevice,
        bounds: &Rectangle,
        viewport: &Viewport,
        quality: LyricRenderQuality,
        recorded: bool,
    ) {
        let (device, queue) = (&pipeline.device, &pipeline.queue);
        let mut frame = self.0.lock();
        frame.bounds = *bounds;
        frame.physical = *bounds * viewport.scale_factor();
        if frame.renderer.is_none() || frame.antialias != quality.antialias || frame.device.as_ref() != Some(device) {
            let Some(adapter) = pipeline.adapter.clone() else { return };
            frame.device = Some(device.clone());
            let (device, queue, format) = (device.clone(), queue.clone(), pipeline.format);
            let antialias = quality.antialias;
            let shell = pipeline.shell.clone();
            frame.owner.store(false, Ordering::Release);
            frame.renderer = Some(crate::Prepared::new(Arc::downgrade(&frame.owner), move || {
                let samples = (antialias == LyricAntialias::On).then_some(graphics::Antialiasing::MSAAx4);
                let engine = iced_wgpu::Engine::new(&adapter, device, queue, format, samples, shell);
                iced_wgpu::Renderer::new(engine, iced::Font::DEFAULT, iced::Pixels(16.0))
            }));
            frame.images.uploads.clear();
            frame.antialias = antialias;
            frame.target = None;
            return;
        }
        if !recorded {
            return;
        }
        let ratio = quality.pixel_ratio(viewport.scale_factor());
        let limit = device.limits().max_texture_dimension_2d as f32;
        let ratio = ratio.min(limit / bounds.width.max(1.0)).min(limit / bounds.height.max(1.0));
        let size =
            Size::new((bounds.width * ratio).round().max(1.0) as u32, (bounds.height * ratio).round().max(1.0) as u32);
        if frame.target.as_ref().is_none_or(|target| target.size != size) {
            frame.target = Some(pipeline.target(size));
        }
        let view = frame.target.as_ref().map(|target| target.view.clone());
        if let (Some(inner), Some(view)) = (frame.renderer.as_mut().and_then(crate::Prepared::poll), view) {
            inner.present(
                Some(iced::Color::TRANSPARENT),
                pipeline.format,
                &view,
                &Viewport::with_physical_size(size, ratio),
            );
            frame.stamp = frame.stamp.wrapping_add(1);
        }
    }
}

impl OffscreenDevice {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let registration = DEVICES.lock().iter().filter_map(Weak::upgrade).find(|entry| entry.device == *device);
        let adapter = registration.as_ref().map(|entry| entry.adapter.clone());
        let shell = registration.map_or_else(graphics::Shell::headless, |entry| entry.shell.clone());
        if adapter.is_none() {
            tracing::error!("lyric quality requires the compositor's registered graphics device");
        }
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-lyric-quality"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-lyric-quality"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self { adapter, shell, device: device.clone(), queue: queue.clone(), format, layout, sampler }
    }

    pub fn texture_layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}
