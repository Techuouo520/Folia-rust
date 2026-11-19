// Projects composed radio faces with CSS perspective while keeping their native widget tree and input.

use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, renderer, widget};
use iced::{Element, Event, Length, Rectangle, Size, mouse};
use iced_wgpu::primitive::Renderer as _;
use parking_lot::Mutex;
use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Plane {
    pub rect: [f32; 4],
    /// Rotation about x/y/z in radians, followed by uniform scale.
    pub rotation: [f32; 4],
    /// Vertical offset, opacity, perspective distance and horizontal face rotation.
    pub effect: [f32; 4],
    /// CSS grayscale, brightness and saturation of the composed face.
    pub color: [f32; 4],
}

impl Plane {
    pub fn contains(&self, point: iced::Point) -> bool {
        if self.effect[1] < 0.2 {
            return false;
        }
        let dx = point.x - self.rect[0] - self.rect[2] / 2.0;
        let dy = point.y - self.rect[1] - self.rect[3] / 2.0 - self.effect[0];
        let (sz, cz) = self.rotation[2].sin_cos();
        let (sx, sy) = (cz * dx + sz * dy, -sz * dx + cz * dy);
        let (sin_x, cos_x) = self.rotation[0].sin_cos();
        let (sin_y, cos_y) = self.rotation[1].sin_cos();
        let (c, d) = (-cos_x * sin_y, sin_x);
        let p = self.effect[2];
        let (a, b) = (cos_y + sx * c / p, sx * d / p);
        let (e, f) = (sin_x * sin_y + sy * c / p, cos_x + sy * d / p);
        let det = a * f - b * e;
        if det.abs() < 0.00001 || self.rotation[3].abs() < 0.00001 {
            return false;
        }
        let u = (sx * f - b * sy) / det / self.rotation[3];
        let v = (a * sy - sx * e) / det / self.rotation[3];
        let (sin, cos) = self.effect[3].sin_cos();
        let denominator = cos - u * sin / 1400.0;
        if denominator.abs() < 0.00001 {
            return false;
        }
        let x = u / denominator;
        let y = v * (1.0 + x * sin / 1400.0);
        x.abs() <= self.rect[2] / 2.0 && y.abs() <= self.rect[3] / 2.0
    }
}

#[derive(Clone, Default)]
pub struct Projection {
    frame: crate::quality::Offscreen,
    parameters: Arc<Mutex<Option<(wgpu::Device, wgpu::Buffer, wgpu::BindGroup)>>>,
    cached: Arc<Mutex<Option<Faces>>>,
}

struct Faces {
    bounds: Rectangle,
    planes: Vec<Plane>,
}

impl Projection {
    pub fn pending(&self) -> bool {
        self.frame.pending()
    }
    pub fn has_faces(&self) -> bool {
        self.cached.lock().is_some()
    }

    /// Fades the last composed card faces during a wall handoff, independently of its background.
    pub fn fade(&self, renderer: &mut iced::Renderer, opacity: f32) {
        let cached = self.cached.lock();
        let Some(faces) = &*cached else {
            return;
        };
        let mut planes = faces.planes.clone();
        for plane in &mut planes {
            plane.effect[1] *= opacity;
        }
        renderer.draw_primitive(faces.bounds, Pass { projection: self.clone(), planes, ready: true, record: false });
    }
}

pub fn surface<'a, M: 'a>(
    content: Element<'a, M>,
    projection: &'a Projection,
    planes: Vec<Plane>,
    images: Vec<iced::advanced::image::Handle>,
) -> Element<'a, M> {
    Element::new(Surface { content, projection, planes, images })
}

struct Surface<'a, M> {
    content: Element<'a, M>,
    projection: &'a Projection,
    planes: Vec<Plane>,
    images: Vec<iced::advanced::image::Handle>,
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for Surface<'_, M> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn layout(&mut self, tree: &mut widget::Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
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
        let bounds = layout.bounds();
        self.projection.frame.set_images(&self.images);
        let ready = self.projection.frame.record(bounds, folia_core::LyricAntialias::Auto, |inner| {
            self.content.as_widget().draw(&tree.children[0], inner, theme, style, layout, cursor, viewport)
        });
        if !ready {
            self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
        }
        if ready {
            let mut cached = self.projection.cached.lock();
            if let Some(faces) = cached.as_mut() {
                faces.bounds = bounds;
                faces.planes.clone_from(&self.planes);
            } else {
                *cached = Some(Faces { bounds, planes: self.planes.clone() });
            }
        }
        renderer.draw_primitive(
            bounds,
            Pass { projection: self.projection.clone(), planes: self.planes.clone(), ready, record: true },
        );
    }
}

struct Pass {
    projection: Projection,
    planes: Vec<Plane>,
    ready: bool,
    record: bool,
}
impl std::fmt::Debug for Pass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RadioProjection").field("planes", &self.planes.len()).field("ready", &self.ready).finish()
    }
}
struct Pipeline {
    target: crate::quality::OffscreenDevice,
    layout: wgpu::BindGroupLayout,
    render: wgpu::RenderPipeline,
}

impl iced_wgpu::primitive::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let target = crate::quality::OffscreenDevice::new(device, queue, format);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-radio-perspective"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/radio.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-radio-planes"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-radio-perspective"),
            bind_group_layouts: &[target.texture_layout(), &layout],
            push_constant_ranges: &[],
        });
        let render = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("folia-radio-perspective"),
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
        });
        Self { target, layout, render }
    }
}

impl iced_wgpu::primitive::Primitive for Pass {
    type Pipeline = Pipeline;
    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced_wgpu::graphics::Viewport,
    ) {
        if self.record {
            self.projection.frame.prepare(&pipeline.target, bounds, viewport, Default::default(), self.ready);
        }
        let mut parameters = self.projection.parameters.lock();
        if parameters.as_ref().is_none_or(|(owner, _, _)| owner != device) {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("folia-radio-planes"),
                size: 16 + 32 * std::mem::size_of::<Plane>() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("folia-radio-planes"),
                layout: &pipeline.layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
            });
            *parameters = Some((device.clone(), buffer, bind));
        }
        if let Some((_, buffer, _)) = &*parameters {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[bounds.width, bounds.height, 0.0, 0.0]));
            queue.write_buffer(buffer, 16, bytemuck::cast_slice(&self.planes[..self.planes.len().min(32)]));
        }
    }
    fn draw(&self, pipeline: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        if !self.ready {
            return true;
        }
        if let Some((_, _, parameters)) = &*self.projection.parameters.lock() {
            self.projection.frame.with_target(|_, texture, _| {
                pass.set_pipeline(&pipeline.render);
                pass.set_bind_group(0, texture, &[]);
                pass.set_bind_group(1, parameters, &[]);
                pass.draw(0..6, 0..self.planes.len().min(32) as u32);
            });
        }
        true
    }
}
