// Animates the settings pick list's native menu while retaining its selection and dismissal behavior.

use std::{cell::Cell, rc::Rc, time::Instant};

use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, overlay, renderer, widget};
use iced::{Element, Event, Length, Point, Rectangle, Size, Theme, Transformation, Vector, mouse};

use crate::motion::Tween;

const MENU_MS: f64 = 150.0;

pub fn view<'a, M: 'a>(content: Element<'a, M>, opacity: Rc<Cell<f32>>) -> Element<'a, M> {
    Element::new(Select { content, opacity })
}

struct Select<'a, M> {
    content: Element<'a, M>,
    opacity: Rc<Cell<f32>>,
}

struct State {
    open: bool,
    pop: Tween,
}

impl<M> Widget<M, Theme, iced::Renderer> for Select<'_, M> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State {
            open: false,
            pop: Tween::new(0.0, MENU_MS, folia_visualizer::Ease::OUT, Instant::now()),
        })
    }

    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut widget::Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
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
        if state.open && state.pop.running(Instant::now()) {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
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
    ) -> Option<overlay::Element<'a, M, Theme, iced::Renderer>> {
        let state = tree.state.downcast_mut::<State>();
        let content =
            self.content.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation);
        let Some(content) = content else {
            state.open = false;
            return None;
        };
        if !state.open {
            let now = Instant::now();
            state.open = true;
            state.pop = Tween::new(0.0, MENU_MS, folia_visualizer::Ease::OUT, now);
            state.pop.set(1.0, now);
        }
        Some(overlay::Element::new(Box::new(Popup {
            content,
            pop: state.pop,
            opacity: self.opacity.clone(),
            trigger: layout.bounds() + translation,
        })))
    }
}

struct Popup<'a, M> {
    content: overlay::Element<'a, M, Theme, iced::Renderer>,
    pop: Tween,
    opacity: Rc<Cell<f32>>,
    trigger: Rectangle,
}

impl<M> Popup<'_, M> {
    fn transform(&self, layout: Layout<'_>, cursor: mouse::Cursor) -> (Transformation, mouse::Cursor) {
        let progress = self.pop.value(Instant::now());
        let bounds = layout.bounds();
        let above = bounds.y < self.trigger.y;
        let anchor = Point::new(bounds.x + bounds.width, if above { bounds.y + bounds.height } else { bounds.y });
        let scale = 0.96 + 0.04 * progress;
        let lift = (if above { 8.0 } else { -8.0 }) * (1.0 - progress);
        let transform = Transformation::translate(anchor.x, anchor.y + lift)
            * Transformation::scale(scale)
            * Transformation::translate(-anchor.x, -anchor.y);
        let cursor = cursor.position().map_or(cursor, |point| {
            mouse::Cursor::Available(Point::new(
                anchor.x + (point.x - anchor.x) / scale,
                anchor.y + (point.y - anchor.y - lift) / scale,
            ))
        });
        (transform, cursor)
    }
}

impl<M> overlay::Overlay<M, Theme, iced::Renderer> for Popup<'_, M> {
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> layout::Node {
        self.content.as_overlay_mut().layout(renderer, bounds)
    }

    fn operate(&mut self, layout: Layout<'_>, renderer: &iced::Renderer, operation: &mut dyn widget::Operation) {
        self.content.as_overlay_mut().operate(layout, renderer, operation);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
    ) {
        let (_, cursor) = self.transform(layout, cursor);
        self.content.as_overlay_mut().update(event, layout, cursor, renderer, clipboard, shell);
        if self.pop.running(Instant::now()) {
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let (_, cursor) = self.transform(layout, cursor);
        self.content.as_overlay().mouse_interaction(layout, cursor, renderer)
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        use iced::advanced::Renderer as _;
        self.opacity.set(self.pop.value(Instant::now()));
        let (transform, cursor) = self.transform(layout, cursor);
        renderer.with_transformation(transform, |renderer| {
            self.content.as_overlay().draw(renderer, theme, style, layout, cursor);
        });
    }
}
