// Keeps poster geometry passes in distinct iced layers so images cannot cover later masks.

use iced::advanced::graphics::geometry::Renderer as _;
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Renderer as _, Shell, Widget, layout};
use iced::widget::canvas::Program as _;
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, mouse};

use crate::wall::{Message, WallProgram};
use crate::wall_pan::Pointer;

pub(super) struct WallCanvas<'a> {
    content: Element<'a, Message>,
    program: WallProgram<'a>,
}

impl<'a> WallCanvas<'a> {
    pub fn new(program: WallProgram<'a>) -> Self {
        let content = iced::widget::canvas(program).width(Length::Fill).height(Length::Fill).into();
        Self { content, program }
    }
}

impl Widget<Message, Theme, Renderer> for WallCanvas<'_> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
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
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let Some(clip) = bounds.intersection(viewport) else { return };
        let pointer = tree.children[0].state.downcast_ref::<Pointer>();
        let passes = self.program.draw(pointer, renderer, theme, bounds, cursor);
        for pass in passes {
            renderer.with_layer(clip, |renderer| {
                renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| renderer.draw_geometry(pass));
            });
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }
}
