// Renders the workbench sample at the real window size, scaled into an aspect-preserving preview box.

use iced::advanced::{Layout, Widget, layout, renderer, widget::Tree};
use iced::{Element, Length, Point, Rectangle, Size, Theme, Transformation, mouse};

pub fn view<'a, M: 'a>(content: Element<'a, M>, window: Size) -> Element<'a, M> {
    Element::new(Stage { content, window })
}

struct Stage<'a, M> {
    content: Element<'a, M>,
    window: Size,
}
impl<M> Widget<M, Theme, iced::Renderer> for Stage<'_, M> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let room = limits.max();
        let scale = (room.width / self.window.width.max(1.0)).min(room.height / self.window.height.max(1.0));
        let size = Size::new(self.window.width * scale, self.window.height * scale);
        let node = self.content.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &layout::Limits::new(self.window, self.window),
        );
        layout::Node::with_children(size, vec![node])
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        let bounds = layout.bounds();
        let Some(child) = layout.children().next() else { return };
        let scale = bounds.width / self.window.width.max(1.0);
        let transform = Transformation::translate(bounds.x, bounds.y)
            * Transformation::scale(scale)
            * Transformation::translate(-bounds.x, -bounds.y);
        let viewport = Rectangle::new(Point::new(bounds.x, bounds.y), self.window);
        renderer.with_layer(bounds, |renderer| {
            renderer.with_transformation(transform, |renderer| {
                self.content.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    child,
                    mouse::Cursor::Unavailable,
                    &viewport,
                );
            })
        });
    }
}
