// Reports the natural height of its content, as `useMeasuredSize` does for the panel's tab bodies.

use iced::advanced::mouse::{self, Cursor};
use iced::advanced::widget::Widget;
use iced::advanced::{Clipboard, Layout, Shell, layout, overlay, renderer, widget};
use iced::{Element, Event, Length, Rectangle, Size, Theme, Vector};

/// Lays `content` out without a height limit and publishes `on_height` when its height differs from `known`.
pub fn measured<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    known: f32,
    on_height: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(Measured { content: content.into(), known, natural: known, on_height: Box::new(on_height) })
}

struct Measured<'a, Message> {
    content: Element<'a, Message>,
    known: f32,
    natural: f32,
    on_height: Box<dyn Fn(f32) -> Message + 'a>,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Measured<'_, Message> {
    fn tag(&self) -> widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<widget::Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut widget::Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let free =
            layout::Limits::new(Size::new(limits.min().width, 0.0), Size::new(limits.max().width, f32::INFINITY));
        let node = self.content.as_widget_mut().layout(tree, renderer, &free);
        self.natural = node.size().height;
        node
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if (self.natural - self.known).abs() > 0.5 {
            shell.publish((self.on_height)(self.natural));
            self.known = self.natural;
        }
        self.content.as_widget_mut().update(tree, event, layout, cursor, renderer, clipboard, shell, viewport);
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content.as_widget_mut().operate(tree, layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(tree, layout, renderer, viewport, translation)
    }
}
