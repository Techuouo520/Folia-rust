// Projects a live native widget tree into the cinema and maps clicks back through the seated camera.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use folia_core::LyricAntialias;
use folia_render::quality::{Offscreen, OffscreenDevice};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, renderer, widget};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};
use iced_wgpu::primitive::Renderer as _;
use parking_lot::Mutex;

use super::{Camera, CinemaError, Config, Frame, Renderer, Runtime, SCREEN_ASPECT, selected_seat};

#[derive(Debug, Clone)]
pub struct Failure {
    pub generation: u64,
    pub error: CinemaError,
}

pub fn view<'a, M: 'a>(source: Element<'a, M>, config: Config<'a>, report: fn(Failure) -> M) -> Element<'a, M> {
    Element::new(Cinema { source, config, report })
}

struct Cinema<'a, M> {
    source: Element<'a, M>,
    config: Config<'a>,
    report: fn(Failure) -> M,
}

struct State {
    source: Offscreen,
    runtime: RefCell<Runtime>,
    owner: Arc<AtomicBool>,
    // GPU prepare hands an initialization failure to the next iced update of this widget.
    error: Arc<Mutex<Option<Failure>>>,
    antialias: LyricAntialias,
    generation: u64,
    drag: Option<Drag>,
}

impl State {
    fn new(config: Config<'_>) -> Self {
        Self {
            source: Offscreen::default(),
            runtime: RefCell::new(Runtime::default()),
            owner: Arc::new(AtomicBool::new(false)),
            error: Arc::default(),
            antialias: config.render_quality.antialias,
            generation: config.generation,
            drag: None,
        }
    }

    fn camera(&self, config: Config<'_>, size: Size) -> Camera {
        Camera::new(selected_seat(config.options.viewpoint.as_deref()), &mut self.runtime.borrow_mut().head, size)
    }
}

struct Drag {
    start: Point,
    last: Point,
    moving: bool,
    content: bool,
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for Cinema<'_, M> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }
    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::new(self.config))
    }
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.source)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state.generation != self.config.generation {
            *state = State::new(self.config);
            tree.children[0] = widget::Tree::new(&self.source);
        } else if state.antialias != self.config.render_quality.antialias {
            state.source = Offscreen::default();
            state.antialias = self.config.render_quality.antialias;
            tree.children[0] = widget::Tree::new(&self.source);
        }
        tree.diff_children(std::slice::from_ref(&self.source));
    }

    fn layout(&mut self, tree: &mut widget::Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let size = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);
        let source = Size::new(size.width, size.width / SCREEN_ASPECT);
        let content =
            self.source.as_widget_mut().layout(&mut tree.children[0], renderer, &layout::Limits::new(source, source));
        layout::Node::with_children(size, vec![content])
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let Some(content) = layout.children().next() else { return };
        let source_bounds = content.bounds();
        let frame = state.runtime.borrow_mut().frame(self.config, bounds.size(), crate::clock::wall_ms());
        let cursor = projected_cursor(frame.camera, cursor, bounds, source_bounds);
        let recorded = state.source.record(source_bounds, self.config.render_quality.antialias, |inner| {
            self.source.as_widget().draw(&tree.children[0], inner, theme, style, content, cursor, &source_bounds);
        });
        renderer.draw_primitive(
            bounds,
            Pass {
                frame,
                source: state.source.clone(),
                source_bounds,
                quality: self.config.render_quality,
                cover: self.config.cover.cloned(),
                cover_pending: self.config.cover_pending,
                owner: Arc::downgrade(&state.owner),
                error: Arc::clone(&state.error),
                generation: self.config.generation,
                publish: self.config.publish_blur,
                recorded,
                physical: folia_render::Slot::default(),
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
        _: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        if let Some(failure) = state.error.lock().take() {
            shell.publish((self.report)(failure));
        }
        if !state.owner.load(Ordering::Acquire) || state.source.pending() {
            shell.request_redraw_at(std::time::Instant::now() + std::time::Duration::from_millis(16));
        }
        let bounds = layout.bounds();
        let Some(content) = layout.children().next() else { return };
        let source_bounds = content.bounds();
        let camera = state.camera(self.config, bounds.size());
        let mapped = if state.drag.as_ref().is_some_and(|drag| drag.content) {
            cursor
                .position()
                .and_then(|point| {
                    camera.screen_plane_point(
                        point - Vector::new(bounds.x, bounds.y),
                        bounds.size(),
                        source_bounds.size(),
                    )
                })
                .map(|point| mouse::Cursor::Available(point + Vector::new(source_bounds.x, source_bounds.y)))
                .unwrap_or(mouse::Cursor::Unavailable)
        } else if state.runtime.borrow().surface_visible() {
            projected_cursor(camera, cursor, bounds, source_bounds)
        } else {
            mouse::Cursor::Unavailable
        };
        let interaction =
            self.source.as_widget().mouse_interaction(&tree.children[0], content, mapped, &source_bounds, renderer);
        let mut replay = false;
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if cursor.is_over(bounds) => {
                if let Some(point) = cursor.position() {
                    let claims_drag = matches!(interaction, mouse::Interaction::Grab | mouse::Interaction::Grabbing);
                    state.drag = Some(Drag { start: point, last: point, moving: false, content: claims_drag });
                    if !claims_drag {
                        shell.capture_event();
                        return;
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(drag) = &mut state.drag
                    && !drag.content
                {
                    drag.moving |= drag.start.distance(*position) >= 5.0;
                    if drag.moving {
                        state.runtime.get_mut().head.turn(*position - drag.last);
                        shell.request_redraw();
                        shell.capture_event();
                        drag.last = *position;
                        return;
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(drag) = state.drag.take()
                    && !drag.content
                {
                    if drag.moving {
                        shell.capture_event();
                        return;
                    }
                    replay = true;
                }
            }
            Event::Window(iced::window::Event::Unfocused) => state.drag = None,
            _ => {}
        }
        if replay {
            self.source.as_widget_mut().update(
                &mut tree.children[0],
                &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                content,
                mapped,
                renderer,
                clipboard,
                shell,
                &source_bounds,
            );
        }
        let event = match (event, mapped.position()) {
            (Event::Mouse(mouse::Event::CursorMoved { .. }), Some(position)) => {
                Event::Mouse(mouse::Event::CursorMoved { position })
            }
            _ => event.clone(),
        };
        self.source.as_widget_mut().update(
            &mut tree.children[0],
            &event,
            content,
            mapped,
            renderer,
            clipboard,
            shell,
            &source_bounds,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        if state.drag.as_ref().is_some_and(|drag| drag.moving && !drag.content) {
            return mouse::Interaction::Grabbing;
        }
        let Some(content) = layout.children().next() else { return mouse::Interaction::None };
        if !state.runtime.borrow().surface_visible() {
            return mouse::Interaction::None;
        }
        let mapped = projected_cursor(
            state.camera(self.config, layout.bounds().size()),
            cursor,
            layout.bounds(),
            content.bounds(),
        );
        self.source.as_widget().mouse_interaction(&tree.children[0], content, mapped, &content.bounds(), renderer)
    }
}

fn projected_cursor(camera: Camera, cursor: mouse::Cursor, window: Rectangle, content: Rectangle) -> mouse::Cursor {
    cursor
        .position_in(window)
        .and_then(|point| camera.screen_point(point, window.size(), content.size()))
        .map(|point| mouse::Cursor::Available(point + Vector::new(content.x, content.y)))
        .unwrap_or(mouse::Cursor::Unavailable)
}

struct Pass {
    frame: Frame,
    source: Offscreen,
    source_bounds: Rectangle,
    quality: folia_core::LyricRenderQuality,
    cover: Option<Arc<image::RgbaImage>>,
    cover_pending: bool,
    owner: Weak<AtomicBool>,
    error: Arc<Mutex<Option<Failure>>>,
    generation: u64,
    publish: bool,
    recorded: bool,
    physical: folia_render::Slot,
}

impl std::fmt::Debug for Pass {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CinemaPass").field("frame", &self.frame).finish()
    }
}

struct Slot {
    owner: Weak<AtomicBool>,
    renderer: folia_render::Prepared<Result<Renderer, CinemaError>>,
    error: Arc<Mutex<Option<Failure>>>,
    generation: u64,
    reported: bool,
    physical: Rectangle,
    logical: Size,
}

pub struct Pipeline {
    source: OffscreenDevice,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    slots: Vec<Slot>,
}

impl iced::widget::shader::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            source: OffscreenDevice::new(device, queue, format),
            device: device.clone(),
            queue: queue.clone(),
            format,
            slots: Vec::new(),
        }
    }
    fn trim(&mut self) {
        self.slots.retain(|slot| slot.owner.strong_count() > 0);
    }
}

impl iced::widget::shader::Primitive for Pass {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::widget::shader::Viewport,
    ) {
        pipeline.source_prepare(self, bounds, viewport);
    }

    fn render(
        &self,
        pipeline: &Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip: &Rectangle<u32>,
    ) {
        if !self.recorded {
            return;
        }
        let Some(slot) = pipeline.slots.get(self.physical.get()) else { return };
        let Some(Ok(renderer)) = slot.renderer.get() else { return };
        let revision = self.source.revision();
        self.source.with_target(|_, picture, _| {
            renderer.render(encoder, picture);
            renderer.composite(encoder, target, picture, slot.physical, *clip);
            if self.publish {
                let stamp = revision ^ (self.owner.as_ptr() as u64).rotate_left(32) ^ self.generation;
                let drew = folia_render::draw_source(
                    &pipeline.device,
                    &pipeline.queue,
                    pipeline.format,
                    [slot.logical.width, slot.logical.height],
                    stamp,
                    |view, width, height| {
                        {
                            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("folia-cinema-glass-clear"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view,
                                    resolve_target: None,
                                    depth_slice: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                ..Default::default()
                            });
                        }
                        renderer.composite(
                            encoder,
                            view,
                            picture,
                            Rectangle::with_size(Size::new(width as f32, height as f32)),
                            Rectangle { x: 0, y: 0, width, height },
                        );
                    },
                );
                if drew {
                    folia_render::blur_source(encoder);
                }
            }
        });
    }
}

impl Pipeline {
    fn source_prepare(&mut self, pass: &Pass, bounds: &Rectangle, viewport: &iced::widget::shader::Viewport) {
        pass.source.prepare(&self.source, &pass.source_bounds, viewport, pass.quality, pass.recorded);
        let index = match self.slots.iter().position(|slot| slot.owner.ptr_eq(&pass.owner)) {
            Some(index) => index,
            None => {
                let (device, queue, format, layout) =
                    (self.device.clone(), self.queue.clone(), self.format, self.source.texture_layout().clone());
                let renderer = folia_render::Prepared::new(pass.owner.clone(), move || {
                    Renderer::new(&device, &queue, format, &layout)
                });
                self.slots.push(Slot {
                    owner: pass.owner.clone(),
                    renderer,
                    error: Arc::clone(&pass.error),
                    generation: pass.generation,
                    reported: false,
                    physical: Rectangle::default(),
                    logical: bounds.size(),
                });
                self.slots.len() - 1
            }
        };
        pass.physical.set(index);
        let slot = &mut self.slots[index];
        slot.physical = *bounds * viewport.scale_factor();
        slot.logical = bounds.size();
        match slot.renderer.poll() {
            Some(Ok(renderer)) => {
                if !pass.cover_pending
                    && let Err(error) = renderer.set_cover(pass.cover.as_ref())
                {
                    tracing::warn!(error = &error as &dyn std::error::Error, "cinema reel could not be loaded");
                }
                renderer.prepare(pass.frame, bounds.size());
            }
            Some(Err(error)) if !slot.reported => {
                tracing::error!(error = error as &dyn std::error::Error, "cinema graphics initialization failed");
                *slot.error.lock() = Some(Failure { generation: slot.generation, error: error.clone() });
                slot.reported = true;
            }
            _ => {}
        }
    }
}
