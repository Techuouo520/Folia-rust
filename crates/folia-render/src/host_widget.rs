// Forwards an existing iced widget tree and its overlays to the compositor-owned renderer.

use crate::host::Renderer;
use iced_wgpu::graphics::core::{
    self, Clipboard, Element, Event, Layout, Length, Rectangle, Shell, Size, Vector, Widget, layout, mouse, overlay,
    renderer, widget,
};

pub fn host<'a, M: 'a, T: 'a>(content: Element<'a, M, T, iced_wgpu::Renderer>) -> Element<'a, M, T, Renderer> {
    Element::new(Host(content))
}
struct Host<'a, M, T>(Element<'a, M, T, iced_wgpu::Renderer>);

impl<M, T> Widget<M, T, Renderer> for Host<'_, M, T> {
    fn size(&self) -> Size<Length> {
        self.0.as_widget().size()
    }
    fn size_hint(&self) -> Size<Length> {
        self.0.as_widget().size_hint()
    }
    fn tag(&self) -> widget::tree::Tag {
        self.0.as_widget().tag()
    }
    fn state(&self) -> widget::tree::State {
        self.0.as_widget().state()
    }
    fn children(&self) -> Vec<widget::Tree> {
        self.0.as_widget().children()
    }
    fn diff(&self, tree: &mut widget::Tree) {
        self.0.as_widget().diff(tree);
    }
    fn layout(&mut self, tree: &mut widget::Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.0.as_widget_mut().layout(tree, &renderer.0, limits)
    }
    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &T,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.0.as_widget().draw(tree, &mut renderer.0, theme, style, layout, cursor, viewport);
    }
    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.0.as_widget_mut().operate(tree, layout, &renderer.0, operation);
    }
    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        self.0.as_widget_mut().update(tree, event, layout, cursor, &renderer.0, clipboard, shell, viewport);
    }
    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.0.as_widget().mouse_interaction(tree, layout, cursor, viewport, &renderer.0)
    }
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, M, T, Renderer>> {
        self.0.as_widget_mut().overlay(tree, layout, &renderer.0, viewport, translation).map(wrap_overlay)
    }
}

fn wrap_overlay<'a, M: 'a, T: 'a>(
    content: overlay::Element<'a, M, T, iced_wgpu::Renderer>,
) -> overlay::Element<'a, M, T, Renderer> {
    overlay::Element::new(Box::new(Overlay(content)))
}
struct Overlay<'a, M, T>(overlay::Element<'a, M, T, iced_wgpu::Renderer>);
impl<M, T> core::Overlay<M, T, Renderer> for Overlay<'_, M, T> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        self.0.as_overlay_mut().layout(&renderer.0, bounds)
    }
    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &T,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.0.as_overlay().draw(&mut renderer.0, theme, style, layout, cursor);
    }
    fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn widget::Operation) {
        self.0.as_overlay_mut().operate(layout, &renderer.0, operation);
    }
    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
    ) {
        self.0.as_overlay_mut().update(event, layout, cursor, &renderer.0, clipboard, shell);
    }
    fn mouse_interaction(&self, layout: Layout<'_>, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        self.0.as_overlay().mouse_interaction(layout, cursor, &renderer.0)
    }
    fn overlay<'a>(
        &'a mut self,
        layout: Layout<'a>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'a, M, T, Renderer>> {
        self.0.as_overlay_mut().overlay(layout, &renderer.0).map(wrap_overlay)
    }
    fn index(&self) -> f32 {
        self.0.as_overlay().index()
    }
}
