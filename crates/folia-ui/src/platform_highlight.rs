// A platform list's measured highlight, independent of row hit testing and layout sizing.

use std::time::Instant;

use folia_visualizer::Ease;
use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Renderer, Shell, Widget, overlay, renderer};
use iced::{Border, Color, Element, Event, Length, Rectangle, Size, Theme, Vector, mouse};

use crate::motion::Tween;

const MOVE_MS: f64 = 220.0;
const OUT: Ease = Ease::new(0.22, 1.0, 0.36, 1.0);

/// The measured platform highlight's transform, width and opacity, matching `PlatformColumn` CSS transitions.
#[derive(Debug, Clone, Copy)]
pub struct ProviderHighlight {
    x: Tween,
    y: Tween,
    width: Tween,
    height: f32,
    alpha: Tween,
    placed: bool,
}

impl ProviderHighlight {
    pub fn new(now: Instant) -> Self {
        Self {
            x: Tween::new(0.0, MOVE_MS, OUT, now),
            y: Tween::new(0.0, MOVE_MS, OUT, now),
            width: Tween::new(0.0, MOVE_MS, OUT, now),
            height: 0.0,
            alpha: Tween::new(0.0, MOVE_MS, OUT, now),
            placed: false,
        }
    }

    /// Losing the pointed and chosen rows fades in place; first placement does not travel from the origin.
    pub fn set(&mut self, bounds: Option<Rectangle>, now: Instant) {
        let Some(bounds) = bounds else {
            self.alpha.set(0.0, now);
            return;
        };
        if self.placed {
            self.x.set(bounds.x, now);
            self.y.set(bounds.y, now);
            self.width.set(bounds.width, now);
        } else {
            self.x = Tween::new(bounds.x, MOVE_MS, OUT, now);
            self.y = Tween::new(bounds.y, MOVE_MS, OUT, now);
            self.width = Tween::new(bounds.width, MOVE_MS, OUT, now);
            self.placed = true;
        }
        self.height = bounds.height;
        self.alpha.set(1.0, now);
    }

    pub fn bounds(&self, now: Instant) -> Rectangle {
        Rectangle { x: self.x.value(now), y: self.y.value(now), width: self.width.value(now), height: self.height }
    }

    pub fn opacity(&self, now: Instant) -> f32 {
        self.alpha.value(now)
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.x.running(now) || self.y.running(now) || self.width.running(now) || self.alpha.running(now)
    }
}

pub(crate) struct Highlight<'a, Message> {
    content: Element<'a, Message>,
    lit: Option<usize>,
    color: Color,
}

impl<'a, Message> Highlight<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message>>, lit: Option<usize>, color: Color) -> Self {
        Self { content: content.into(), lit, color }
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Highlight<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ProviderHighlight>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(ProviderHighlight::new(Instant::now()))
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let node = self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits);
        let bounds = self.lit.and_then(|index| node.children().get(index)).map(layout::Node::bounds);
        tree.state.downcast_mut::<ProviderHighlight>().set(bounds, Instant::now());
        node
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
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
        if tree.state.downcast_ref::<ProviderHighlight>().animating(Instant::now()) {
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<ProviderHighlight>();
        let now = Instant::now();
        let mut bounds = state.bounds(now);
        bounds.x += layout.bounds().x;
        bounds.y += layout.bounds().y;
        if let Some(clip) = viewport.intersection(&layout.bounds()) {
            renderer.with_layer(clip, |renderer| {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        border: Border { radius: 8.0.into(), ..Border::default() },
                        ..renderer::Quad::default()
                    },
                    Color { a: self.color.a * state.opacity(now), ..self.color },
                )
            });
        }
        self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

impl<'a, Message: 'a> From<Highlight<'a, Message>> for Element<'a, Message> {
    fn from(highlight: Highlight<'a, Message>) -> Self {
        Element::new(highlight)
    }
}
