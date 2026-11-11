// Replay pointer events through the actual reorder widget, without a GPU or the system clipboard.

use std::time::{Duration, Instant};

use iced::advanced::{Shell, clipboard, image, layout, renderer, widget::Tree};
use iced::widget::{button, row, space};
use iced::{
    Background, Color, Element, Event, Length, Point, Rectangle, Size, Theme, Transformation, keyboard, mouse, window,
};

#[derive(Debug, Clone, PartialEq)]
enum Message {
    Move(usize, usize),
    Scroll(f32),
    Button,
}

#[derive(Default)]
struct Painter {
    quads: Vec<Rectangle>,
}

impl renderer::Renderer for Painter {
    fn start_layer(&mut self, _: Rectangle) {}
    fn end_layer(&mut self) {}
    fn start_transformation(&mut self, _: Transformation) {}
    fn end_transformation(&mut self) {}
    fn fill_quad(&mut self, quad: renderer::Quad, _: impl Into<Background>) {
        self.quads.push(quad.bounds);
    }
    fn reset(&mut self, _: Rectangle) {
        self.quads.clear();
    }
    fn allocate_image(
        &mut self,
        _: &image::Handle,
        callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
        callback(Err(image::Error::Unsupported));
    }
}

struct List {
    widget: Element<'static, Message, Theme, Painter>,
    tree: Tree,
    layout: layout::Node,
    renderer: Painter,
    viewport: Rectangle,
}

impl List {
    fn new() -> Self {
        let rows = [40.0, 60.0, 80.0, 50.0]
            .into_iter()
            .map(|height| {
                row![
                    space().width(Length::Fill).height(height),
                    button(space().width(50).height(height)).padding(0).on_press(Message::Button)
                ]
                .into()
            })
            .collect();
        let mut widget =
            folia_ui::drag_to_reorder(rows, 1..4, Color::BLACK, Color::WHITE, Message::Move, Message::Scroll);
        let mut tree = Tree::new(&widget);
        let renderer = Painter::default();
        let layout = widget.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::new(300.0, 0.0), Size::new(300.0, 500.0)),
        );
        Self { widget, tree, layout, renderer, viewport: Rectangle::with_size(Size::new(300.0, 230.0)) }
    }

    fn event(&mut self, event: Event, point: Point) -> (Vec<Message>, bool) {
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        self.widget.as_widget_mut().update(
            &mut self.tree,
            &event,
            layout::Layout::new(&self.layout),
            mouse::Cursor::Available(point),
            &self.renderer,
            &mut clipboard::Null,
            &mut shell,
            &self.viewport,
        );
        let captured = shell.is_event_captured();
        (messages, captured)
    }

    fn press(&mut self, point: Point) {
        self.event(Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)), point);
    }
    fn hold(&mut self, point: Point) {
        self.event(Event::Window(window::Event::RedrawRequested(Instant::now() + Duration::from_millis(400))), point);
    }
    fn move_to(&mut self, point: Point) {
        self.event(Event::Mouse(mouse::Event::CursorMoved { position: point }), point);
    }
    fn release(&mut self, point: Point) -> Vec<Message> {
        self.event(Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)), point).0
    }
}

#[test]
fn dragging_starts_on_movement_without_a_timer_and_only_release_commits() {
    let mut list = List::new();
    let start = Point::new(30.0, 70.0);
    let end = Point::new(30.0, 210.0);
    list.press(start);
    assert!(list.event(Event::Mouse(mouse::Event::CursorMoved { position: end }), end).0.is_empty());
    assert_eq!(list.release(end), vec![Message::Move(1, 3)]);
    assert!(list.release(end).is_empty());
}

#[test]
fn a_click_small_jitter_or_waiting_without_motion_does_not_start_a_drag() {
    let mut list = List::new();
    let start = Point::new(30.0, 70.0);
    list.press(start);
    list.move_to(Point::new(33.0, 70.0));
    list.hold(Point::new(33.0, 70.0));
    assert_ne!(
        list.widget.as_widget().mouse_interaction(
            &list.tree,
            layout::Layout::new(&list.layout),
            mouse::Cursor::Available(start),
            &list.viewport,
            &list.renderer
        ),
        mouse::Interaction::Grabbing
    );
    assert!(list.release(start).is_empty());
}

#[test]
fn button_clicks_and_import_rows_never_start_dragging() {
    let mut list = List::new();
    let button = Point::new(275.0, 70.0);
    list.press(button);
    list.hold(button);
    assert_eq!(list.release(button), vec![Message::Button]);
    let import = Point::new(30.0, 20.0);
    list.press(import);
    list.hold(import);
    list.move_to(Point::new(30.0, 210.0));
    assert!(list.release(Point::new(30.0, 210.0)).is_empty());
}

#[test]
fn insertion_uses_actual_variable_row_bounds() {
    let mut list = List::new();
    let start = Point::new(30.0, 210.0);
    list.press(start);
    list.hold(start);
    list.move_to(Point::new(30.0, 130.0));
    assert_eq!(list.release(Point::new(30.0, 130.0)), vec![Message::Move(3, 2)]);
    list.press(start);
    list.hold(start);
    list.move_to(Point::new(30.0, 50.0));
    assert_eq!(list.release(Point::new(30.0, 50.0)), vec![Message::Move(3, 1)]);
}

#[test]
fn escape_focus_loss_window_exit_and_outside_drop_cancel_without_a_move() {
    let escape = Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(keyboard::key::Named::Escape),
        modified_key: keyboard::Key::Named(keyboard::key::Named::Escape),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
        text: None,
        repeat: false,
    });
    for cancel in [escape, Event::Window(window::Event::Unfocused), Event::Mouse(mouse::Event::CursorLeft)] {
        let mut list = List::new();
        let start = Point::new(30.0, 70.0);
        let end = Point::new(30.0, 210.0);
        list.press(start);
        list.hold(start);
        list.move_to(end);
        assert!(list.event(cancel, end).0.is_empty());
        assert!(list.release(end).is_empty());
    }
    let mut list = List::new();
    let start = Point::new(30.0, 70.0);
    let outside = Point::new(400.0, 210.0);
    list.press(start);
    list.hold(start);
    list.move_to(outside);
    assert!(list.release(outside).is_empty());
}

#[test]
fn active_drag_draws_a_following_row_and_scrolls_at_the_viewport_edge() {
    let mut list = List::new();
    list.viewport.height = 160.0;
    let start = Point::new(30.0, 70.0);
    let end = Point::new(30.0, 150.0);
    list.press(start);
    list.hold(start);
    list.move_to(end);
    let (messages, _) =
        list.event(Event::Window(window::Event::RedrawRequested(Instant::now() + Duration::from_millis(450))), end);
    assert!(messages.iter().any(|message| matches!(message, Message::Scroll(delta) if *delta < 0.0)));
    list.widget.as_widget().draw(
        &list.tree,
        &mut list.renderer,
        &Theme::Light,
        &renderer::Style::default(),
        layout::Layout::new(&list.layout),
        mouse::Cursor::Available(end),
        &list.viewport,
    );
    assert!(list.renderer.quads.iter().any(|rect| rect.y == 120.0 && rect.height == 60.0));
    assert_eq!(
        list.widget.as_widget().mouse_interaction(
            &list.tree,
            layout::Layout::new(&list.layout),
            mouse::Cursor::Available(end),
            &list.viewport,
            &list.renderer
        ),
        mouse::Interaction::Grabbing
    );
    assert_eq!(list.release(end), vec![Message::Move(1, 2)]);
}
