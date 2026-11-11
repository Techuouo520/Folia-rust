// Movement-triggered reordering with pointer capture, a moving row preview and edge scrolling.

use std::ops::Range;
use std::time::{Duration, Instant};

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::widget::column;
use iced::{Border, Color, Element, Event, Length, Point, Rectangle, Size, Vector, keyboard, mouse, window};

const DRAG_THRESHOLD: f32 = 4.0;

/// Wraps rows whose original indices are in `draggable`; child buttons retain their normal click behavior.
pub fn drag_to_reorder<'a, Message: 'a, Theme: 'a, Renderer: renderer::Renderer + 'a>(
    rows: Vec<Element<'a, Message, Theme, Renderer>>,
    draggable: Range<usize>,
    accent: Color,
    surface: Color,
    on_move: impl Fn(usize, usize) -> Message + 'a,
    on_scroll: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer> {
    Element::new(Reorder {
        content: column(rows).into(),
        draggable,
        accent,
        surface,
        on_move: Box::new(on_move),
        on_scroll: Box::new(on_scroll),
    })
}

struct Reorder<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    draggable: Range<usize>,
    accent: Color,
    surface: Color,
    on_move: Box<dyn Fn(usize, usize) -> Message + 'a>,
    on_scroll: Box<dyn Fn(f32) -> Message + 'a>,
}

#[derive(Default)]
struct State {
    drag: Option<Drag>,
}

struct Drag {
    from: usize,
    to: usize,
    start: Point,
    cursor: Point,
    grab_y: f32,
    last_frame: Instant,
    active: bool,
}

#[derive(Default)]
struct Response {
    capture: bool,
    redraw: bool,
    moved: Option<(usize, usize)>,
    scroll: f32,
    wake: Option<Instant>,
}

impl State {
    fn update(&mut self, event: &Event, layout: Layout<'_>, range: &Range<usize>, viewport: &Rectangle) -> Response {
        let Some(drag) = self.drag.as_mut() else { return Response::default() };
        let mut response = Response::default();
        match event {
            Event::Window(window::Event::Unfocused) | Event::Mouse(mouse::Event::CursorLeft) => {
                self.drag = None;
                response.redraw = true;
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => {
                self.drag = None;
                response.capture = true;
                response.redraw = true;
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                drag.cursor = *position;
                if !drag.active && drag.start.distance(*position) >= DRAG_THRESHOLD {
                    drag.active = true;
                    drag.last_frame = Instant::now();
                }
                response.capture = true;
                response.redraw = drag.active;
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                if drag.active {
                    response.scroll = edge_scroll(drag.cursor, layout.bounds(), *viewport)
                        * now.saturating_duration_since(drag.last_frame).as_secs_f32().min(0.032);
                    drag.last_frame = *now;
                    if response.scroll != 0.0 {
                        response.wake = Some(*now + Duration::from_millis(16));
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                drag.to = target(layout, range, drag.from, drag.cursor);
                if drag.active
                    && viewport.contains(drag.cursor)
                    && layout.bounds().contains(drag.cursor)
                    && drag.from != drag.to
                {
                    response.moved = Some((drag.from, drag.to));
                }
                self.drag = None;
                response.capture = true;
                response.redraw = true;
            }
            Event::Mouse(mouse::Event::ButtonPressed(_)) => response.capture = true,
            _ => {}
        }
        if let Some(drag) = self.drag.as_mut()
            && drag.active
        {
            drag.to = target(layout, range, drag.from, drag.cursor);
        }
        response
    }
}

fn target(layout: Layout<'_>, range: &Range<usize>, from: usize, cursor: Point) -> usize {
    range.start
        + layout
            .children()
            .enumerate()
            .filter(|(index, row)| range.contains(index) && *index != from && row.bounds().center_y() < cursor.y)
            .count()
}

fn edge_scroll(cursor: Point, bounds: Rectangle, viewport: Rectangle) -> f32 {
    let Some(visible) = bounds.intersection(&viewport) else { return 0.0 };
    if !visible.contains(cursor) {
        return 0.0;
    }
    if cursor.y < visible.y + 24.0 {
        320.0
    } else if cursor.y > visible.y + visible.height - 24.0 {
        -320.0
    } else {
        0.0
    }
}

impl<Message, Theme, Renderer: renderer::Renderer> Widget<Message, Theme, Renderer>
    for Reorder<'_, Message, Theme, Renderer>
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }
    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state
            .drag
            .as_ref()
            .is_some_and(|drag| !self.draggable.contains(&drag.from) || !self.draggable.contains(&drag.to))
        {
            state.drag = None;
        }
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
        let state = tree.state.downcast_mut::<State>();
        if let (Some(drag), Some(position)) = (state.drag.as_mut(), cursor.position()) {
            drag.cursor = position;
        }
        let response = state.update(event, layout, &self.draggable, viewport);
        if response.redraw {
            shell.request_redraw();
        }
        if let Some(at) = response.wake {
            shell.request_redraw_at(at);
        }
        if response.scroll != 0.0 {
            shell.publish((self.on_scroll)(response.scroll));
        }
        if let Some((from, to)) = response.moved {
            shell.publish((self.on_move)(from, to));
        }
        if response.capture {
            shell.capture_event();
            return;
        }
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
        if shell.is_event_captured() || state.drag.is_some() {
            return;
        }
        if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)))
            && let Some(at) = cursor.position().filter(|at| viewport.contains(*at))
            && let Some((index, row)) = layout
                .children()
                .enumerate()
                .find(|(index, row)| self.draggable.contains(index) && row.bounds().contains(at))
        {
            let now = Instant::now();
            state.drag = Some(Drag {
                from: index,
                to: index,
                start: at,
                cursor: at,
                grab_y: at.y - row.bounds().y,
                last_frame: now,
                active: false,
            });
            shell.capture_event();
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
        if tree.state.downcast_ref::<State>().drag.as_ref().is_some_and(|drag| drag.active) {
            return mouse::Interaction::Grabbing;
        }
        let child = self.content.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer);
        if child != mouse::Interaction::None {
            return child;
        }
        if cursor.position().is_some_and(|at| {
            viewport.contains(at)
                && layout
                    .children()
                    .enumerate()
                    .any(|(index, row)| self.draggable.contains(&index) && row.bounds().contains(at))
        }) {
            return mouse::Interaction::Grab;
        }
        child
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
        let Some(drag) = tree.state.downcast_ref::<State>().drag.as_ref().filter(|drag| drag.active) else { return };
        let Some(source) = layout.children().nth(drag.from) else { return };
        let bounds = source.bounds();
        let shift = drag.cursor.y - bounds.y - drag.grab_y;
        let ghost = Rectangle { y: bounds.y + shift, ..bounds };
        renderer.with_layer(*viewport, |renderer| {
            renderer.fill_quad(renderer::Quad { bounds, ..Default::default() }, self.surface);
            if let Some(clip) = ghost.intersection(viewport) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: ghost,
                        border: Border { width: 1.0, color: self.accent, radius: 8.0.into() },
                        ..Default::default()
                    },
                    self.surface,
                );
                renderer.with_layer(clip, |renderer| {
                    renderer.with_translation(Vector::new(0.0, shift), |renderer| {
                        self.content.as_widget().draw(
                            &tree.children[0],
                            renderer,
                            theme,
                            style,
                            layout,
                            mouse::Cursor::Unavailable,
                            &Rectangle { y: viewport.y - shift, ..*viewport },
                        );
                    })
                });
            }
            if drag.from != drag.to
                && let Some(target) = layout.children().nth(drag.to)
            {
                let target = target.bounds();
                let y = if drag.to < drag.from { target.y } else { target.y + target.height };
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle { x: target.x, y: y - 1.0, width: target.width, height: 2.0 },
                        ..Default::default()
                    },
                    self.accent,
                );
            }
        });
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}
