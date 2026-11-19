// Lattice lyrics compose cached Gaussian glyph halos below the shared Monet text renderer.

use super::rail::{self, Rail};
use crate::{Place, Scene};
use folia_render::glyphs::{Batch, Glyphs, Sprite};
use iced::advanced::Renderer as _;
use iced::advanced::graphics::geometry::Renderer as _;
use iced::advanced::{Layout, Widget, layout, renderer, widget};
use iced::widget::canvas::Frame;
use iced::{Element, Length, Rectangle, Renderer, Size, Theme, Vector, mouse};
use iced_wgpu::primitive::Renderer as _;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

pub fn view<'a, Message: 'a>(scene: Scene<'a>, scale: f32) -> Element<'a, Message> {
    Element::new(Card { scene, scale })
}

struct Card<'a> {
    scene: Scene<'a>,
    scale: f32,
}

#[derive(Default)]
struct State {
    rail: RefCell<Rail>,
    batch: Batch,
    owner: Arc<AtomicBool>,
}

impl<Message> Widget<Message, Theme, Renderer> for Card<'_> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }
    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }
    fn layout(&mut self, _: &mut widget::Tree, _: &Renderer, limits: &layout::Limits) -> layout::Node {
        layout::Node::new(limits.resolve(Length::Fill, Length::Fill, Size::ZERO))
    }
    fn update(
        &mut self,
        tree: &mut widget::Tree,
        _: &iced::Event,
        _: Layout<'_>,
        _: mouse::Cursor,
        _: &Renderer,
        _: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _: &Rectangle,
    ) {
        if !tree.state.downcast_ref::<State>().owner.load(Ordering::Acquire) {
            shell.request_redraw_at(std::time::Instant::now() + std::time::Duration::from_millis(16));
        }
    }
    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        _: &Theme,
        _: &renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let Some(clip) = bounds.intersection(viewport) else { return };
        let state = tree.state.downcast_ref::<State>();
        let mut frame = Frame::new(renderer, bounds.size());
        let scale = self.scale.max(0.01);
        let mut sprites = state.batch.lock();
        sprites.clear();
        rail::draw_card(
            &mut frame,
            Place { offset: Vector::ZERO, scale },
            &self.scene,
            &mut state.rail.borrow_mut(),
            Size::new(bounds.width / scale, bounds.height / scale),
            &mut sprites,
        );
        drop(sprites);
        renderer.with_layer(clip, |renderer| {
            renderer.draw_primitive(
                bounds,
                Pass { batch: state.batch.clone(), owner: Arc::downgrade(&state.owner), size: bounds.size() },
            );
        });
        renderer.with_layer(clip, |renderer| {
            renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
                renderer.draw_geometry(frame.into_geometry())
            })
        });
    }
}

#[derive(Debug)]
struct Pass {
    batch: Batch,
    owner: Weak<AtomicBool>,
    size: Size,
}
struct Slot {
    owner: Weak<AtomicBool>,
    glyphs: folia_render::Prepared<Glyphs>,
    sprites: Vec<Sprite>,
}
struct Pipeline {
    slots: Vec<Slot>,
    format: wgpu::TextureFormat,
}

impl iced_wgpu::primitive::Pipeline for Pipeline {
    fn new(_: &wgpu::Device, _: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self { slots: Vec::new(), format }
    }
    fn trim(&mut self) {
        self.slots.retain(|slot| slot.owner.strong_count() > 0);
    }
}

impl iced_wgpu::primitive::Primitive for Pass {
    type Pipeline = Pipeline;
    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let index = match pipeline.slots.iter().position(|slot| slot.owner.ptr_eq(&self.owner)) {
            Some(index) => index,
            None => {
                let (device, format) = (device.clone(), pipeline.format);
                pipeline.slots.push(Slot {
                    owner: self.owner.clone(),
                    sprites: Vec::new(),
                    glyphs: folia_render::Prepared::new(self.owner.clone(), move || {
                        Glyphs::new(&device, format, 128, Vec::new())
                    }),
                });
                pipeline.slots.len() - 1
            }
        };
        let slot = &mut pipeline.slots[index];
        let Some(glyphs) = slot.glyphs.poll() else { return };
        slot.sprites.clear();
        slot.sprites.extend_from_slice(&self.batch.lock());
        if pipeline.format.is_srgb() {
            for sprite in &mut slot.sprites {
                for channel in &mut sprite.color[..3] {
                    *channel = folia_render::to_linear_channel(*channel);
                }
            }
        }
        glyphs.reserve(device, slot.sprites.len());
        glyphs.prepare(device, queue, &slot.sprites, [self.size.width, self.size.height], viewport.scale_factor());
    }
    fn draw(&self, pipeline: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        if let Some(slot) = pipeline.slots.iter().find(|slot| slot.owner.ptr_eq(&self.owner))
            && let Some(glyphs) = slot.glyphs.get()
        {
            glyphs.draw(pass, 0..slot.sprites.len() as u32);
        }
        true
    }
}
