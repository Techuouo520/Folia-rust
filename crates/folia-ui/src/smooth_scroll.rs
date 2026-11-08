// A scrollable that takes the wheel itself: wheel targets stack and ease towards each other over 150ms.

use std::time::Instant;

use folia_visualizer::Ease;
use iced::advanced::mouse::{self, Cursor, ScrollDelta};
use iced::advanced::widget::Widget;
use iced::advanced::{Clipboard, Layout, Shell, layout, renderer, widget};
use iced::widget::{Id, operation, scrollable::AbsoluteOffset};
use iced::{Element, Event, Length, Rectangle, Size, Task, Theme};

use crate::motion::Tween;

/// Chromium's smooth wheel lands about 150ms after the last notch.
pub const WHEEL_MS: f64 = 150.0;
/// `Ease` for a wheel notch: the browser's `cubic-bezier(0.25, 0.1, 0.25, 1)`.
pub const WHEEL_EASE: Ease = Ease::new(0.25, 0.1, 0.25, 1.0);
/// A programmatic scroll (to a row, to the current lyric line): 300ms ease-in-out.
pub const PROGRAM_MS: f64 = 300.0;
pub const PROGRAM_EASE: Ease = Ease::new(0.42, 0.0, 0.58, 1.0);
/// iced gives one wheel line 60px; Chromium gives about the same.
pub const LINE_PX: f32 = 60.0;

/// The scroll position of one list: the last offset applied, the target the wheel started for, and the
/// tween between them. A page owns one of these and pushes [`Scroll::offset`] into its scrollable.
#[derive(Debug, Clone, Copy)]
pub struct Scroll {
    offset: f32,
    max: f32,
    tween: Option<Tween>,
}

impl Default for Scroll {
    fn default() -> Self {
        // No viewport has been reported yet; the first `scrolled` message sets the real maximum.
        Self { offset: 0.0, max: f32::INFINITY, tween: None }
    }
}

impl Scroll {
    /// The offset to show. Frames move it; wheel notches leave it and move the target.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    pub fn at(offset: f32) -> Self {
        Self { offset, ..Self::default() }
    }

    pub fn reset(&mut self) {
        self.offset = 0.0;
        self.tween = None;
    }

    /// A list whose content height is known without a viewport report (the virtual queue list).
    pub fn set_max(&mut self, max: f32) {
        self.max = max.max(0.0);
        self.offset = self.offset.clamp(0.0, self.max);
    }

    /// The scrollable reported its viewport without changing the offset (an automatic scroll's echo).
    pub fn viewport(&mut self, content: f32, window: f32) {
        self.max = (content - window).max(0.0);
    }

    /// The scrollable reported its viewport: learn the maximum offset, or take an outside scroll.
    pub fn scrolled(&mut self, viewport: &iced::widget::scrollable::Viewport, now: Instant) -> bool {
        let (offset, content, window) = reported(viewport);
        self.scrolled_at(offset, content, window, now)
    }

    /// `scrolled` from the three numbers, for callers without a `Viewport`.
    /// `moved` is true when the movement did not come from this model (a scrollbar drag or a touch drag),
    /// so a caller that suppresses auto-scroll can notice it.
    pub fn scrolled_at(&mut self, offset: f32, content: f32, window: f32, now: Instant) -> bool {
        self.viewport(content, window);
        let actual = offset;
        if self.tween.is_some_and(|tween| tween.running(now)) {
            // Our own frames produce these; only the maximum is useful while a tween runs.
            return false;
        }
        self.tween = None;
        let moved = (actual - self.offset).abs() > 0.5;
        self.offset = actual.clamp(0.0, self.max);
        moved
    }

    /// One wheel event. Lines become a tween whose target stacks on the previous one; touchpad pixels
    /// stay 1:1 and drop any running tween.
    pub fn wheel(&mut self, delta: ScrollDelta, now: Instant) {
        match delta {
            ScrollDelta::Pixels { y, .. } => {
                self.tween = None;
                self.offset = (self.offset - y).clamp(0.0, self.max);
            }
            ScrollDelta::Lines { y, .. } => {
                let goal = (self.goal() - y * LINE_PX).clamp(0.0, self.max);
                self.start(goal, WHEEL_MS, WHEEL_EASE, now);
            }
        }
    }

    /// A scroll requested by the page (`scrollTo({ behavior: 'smooth' })`): 300ms ease-in-out.
    pub fn scroll_to(&mut self, offset: f32, now: Instant) {
        self.start(offset.clamp(0.0, self.max), PROGRAM_MS, PROGRAM_EASE, now);
    }

    /// A jump (`behavior: 'auto'`): no tween.
    pub fn jump(&mut self, offset: f32) {
        self.offset = offset.clamp(0.0, self.max);
        self.tween = None;
    }

    /// Advances the tween to `now`; `Some` when the visible offset moved and the scrollable must follow.
    pub fn frame(&mut self, now: Instant) -> Option<f32> {
        let tween = self.tween?;
        if !tween.running(now) {
            self.tween = None;
            let goal = tween.goal().clamp(0.0, self.max);
            if (goal - self.offset).abs() < 0.001 {
                return None;
            }
            self.offset = goal;
            return Some(goal);
        }
        let value = tween.value(now).clamp(0.0, self.max);
        if (value - self.offset).abs() < 0.01 {
            return None;
        }
        self.offset = value;
        Some(value)
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.tween.is_some_and(|tween| tween.running(now) || (tween.goal() - self.offset).abs() > 0.01)
    }

    /// The scrollable's id and this offset as a task, for a page that has just moved the model.
    pub fn apply<Message>(&self, id: Id) -> Task<Message> {
        operation::scroll_to(id, AbsoluteOffset { x: None, y: Some(self.offset) })
    }

    fn goal(&self) -> f32 {
        self.tween.map_or(self.offset, |tween| tween.goal())
    }

    fn start(&mut self, goal: f32, ms: f64, ease: Ease, now: Instant) {
        let from = self.tween.map_or(self.offset, |tween| tween.value(now));
        let mut tween = self.tween.unwrap_or(Tween::new(from, ms, ease, now));
        tween.set_with(goal, ms, ease, now);
        self.tween = Some(tween);
    }
}

/// The offset, content height and window height a scrollable's viewport reports.
pub fn reported(viewport: &iced::widget::scrollable::Viewport) -> (f32, f32, f32) {
    (viewport.absolute_offset().y, viewport.content_bounds().height, viewport.bounds().height)
}

/// Wraps `content` (a scrollable) so the wheel goes to `on_wheel` instead of the scrollable's own jump.
pub fn smooth<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    on_wheel: impl Fn(ScrollDelta) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(SmoothScroll { content: content.into(), on_wheel: Box::new(on_wheel) })
}

pub struct SmoothScroll<'a, Message> {
    content: Element<'a, Message>,
    on_wheel: Box<dyn Fn(ScrollDelta) -> Message + 'a>,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for SmoothScroll<'_, Message> {
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
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut widget::Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
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
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event
            && cursor.position_over(layout.bounds()).is_some()
        {
            shell.publish((self.on_wheel)(*delta));
            shell.capture_event();
            return;
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
}

impl<'a, Message: 'a> From<SmoothScroll<'a, Message>> for Element<'a, Message> {
    fn from(widget: SmoothScroll<'a, Message>) -> Self {
        Element::new(widget)
    }
}
