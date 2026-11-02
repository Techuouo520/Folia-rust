// The player panel's queue tab (`QueueTab.tsx`): 48px rows, hover actions and a scrolling list without reordering.

use std::time::Instant;

use folia_core::Track;
use folia_i18n::I18n;
use folia_visualizer::Ease;
use iced::widget::operation::{self, AbsoluteOffset};
use iced::widget::scrollable::Viewport;
use iced::widget::{Id, button, column, container, mouse_area, pin, row, scrollable, space, stack, text, tooltip};
use iced::{Alignment, Background, Border, Color, Element, Length, Task};
use lucide_static_svg::Icon;

use crate::icons::icon;
use crate::motion::Tween;
use crate::settings_kit::{self, Tokens};

/// `ROW_PX`.
pub const ROW: f32 = 48.0;
/// `LIST_MAX_PX`: seven and a half rows, the half one says that more follow.
const VISIBLE_ROWS: f32 = 7.5;
/// The list never folds under two rows (or its own height when it has fewer).
pub const MIN_LIST: f32 = 2.0 * ROW;
/// react-window's `overscanCount`.
const OVERSCAN: usize = 5;
/// `behavior: 'smooth'`: Chromium's usual 300ms ease-in-out.
const SMOOTH_MS: f64 = 300.0;
const SMOOTH_EASE: Ease = Ease::new(0.42, 0.0, 0.58, 1.0);

/// As tall as the rows up to seven and a half of them.
pub fn list_height(rows: usize) -> f32 {
    (rows as f32 * ROW).min(VISIBLE_ROWS * ROW)
}

#[derive(Debug, Clone)]
pub enum Message {
    Scrolled(Viewport),
    Hover(Option<usize>),
    /// The pointer left this row; the next row may already have taken over.
    Unhover(usize),
    /// The row action under the pointer: the row and which of its three.
    ActionHover(Option<(usize, u8)>),
    /// The pointer left this action; only clears while it is still the current one.
    ActionUnhover((usize, u8)),
    Jump(usize),
    MoveNext(usize),
    MoveEnd(usize),
    Remove(usize),
}

/// What a row action asks of the queue.
pub enum Change {
    Jump(usize),
    MoveNext(usize),
    MoveEnd(usize),
    Remove(usize),
}

pub struct QueueList {
    id: Id,
    offset: f32,
    /// Waiting for the list to be on screen to scroll the row before the playing one to the top.
    center_pending: bool,
    following: Option<i64>,
    scroll: Option<Tween>,
    hover: Option<usize>,
    action_hover: Option<(usize, u8)>,
}

impl Default for QueueList {
    fn default() -> Self {
        Self {
            id: Id::unique(),
            offset: 0.0,
            center_pending: false,
            following: None,
            scroll: None,
            hover: None,
            action_hover: None,
        }
    }
}

impl QueueList {
    /// The list came on screen: the row before the playing one goes to the top once it is laid out.
    pub fn opened(&mut self) {
        self.offset = 0.0;
        self.scroll = None;
        self.center_pending = true;
        self.hover = None;
        self.action_hover = None;
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.center_pending || self.scroll.is_some_and(|tween| tween.running(now))
    }

    /// One frame: the pending first scroll and the smooth scroll to a new playing row.
    pub fn frame(&mut self, queue: &[Track], current: Option<usize>, list_h: f32, now: Instant) -> Task<Message> {
        let max = max_offset(queue.len(), list_h);
        if self.center_pending {
            self.center_pending = false;
            self.following = current.and_then(|index| queue.get(index)).map(|track| track.id);
            let Some(index) = current else { return Task::none() };
            return self.jump_to(top_of(index).clamp(0.0, max));
        }
        let Some(tween) = self.scroll else { return Task::none() };
        let value = tween.value(now).clamp(0.0, max);
        if !tween.running(now) {
            self.scroll = None;
        }
        self.jump_to(value)
    }

    /// The playing song changed: the row before it scrolls to the top (`scrollToRow` with `align: 'start'`).
    pub fn follow(&mut self, queue: &[Track], current: Option<usize>, list_h: f32, now: Instant) {
        let id = current.and_then(|index| queue.get(index)).map(|track| track.id);
        if id == self.following {
            return;
        }
        self.following = id;
        let Some(index) = current.filter(|_| !self.center_pending) else { return };
        let goal = top_of(index).clamp(0.0, max_offset(queue.len(), list_h));
        if (goal - self.offset).abs() < 0.5 {
            return;
        }
        let mut tween = Tween::new(self.offset, SMOOTH_MS, SMOOTH_EASE, now);
        tween.set(goal, now);
        self.scroll = Some(tween);
    }

    pub fn update(&mut self, message: Message, queue: &[Track]) -> (Task<Message>, Option<Change>) {
        let change =
            |index: usize, change: fn(usize) -> Change| (Task::none(), (index < queue.len()).then_some(change(index)));
        match message {
            Message::Scrolled(viewport) => {
                self.offset = viewport.absolute_offset().y;
                (Task::none(), None)
            }
            Message::Hover(index) => {
                self.hover = index;
                (Task::none(), None)
            }
            // The enter for the next row can arrive before this exit; only the row still under the pointer clears.
            Message::Unhover(index) => {
                if self.hover == Some(index) {
                    self.hover = None;
                }
                if self.action_hover.is_some_and(|(row, _)| row == index) {
                    self.action_hover = None;
                }
                (Task::none(), None)
            }
            Message::ActionHover(action) => {
                self.action_hover = action;
                (Task::none(), None)
            }
            Message::ActionUnhover(key) => {
                if self.action_hover == Some(key) {
                    self.action_hover = None;
                }
                (Task::none(), None)
            }
            Message::Jump(index) => change(index, Change::Jump),
            Message::MoveNext(index) => change(index, Change::MoveNext),
            Message::MoveEnd(index) => change(index, Change::MoveEnd),
            Message::Remove(index) => change(index, Change::Remove),
        }
    }

    fn jump_to(&mut self, offset: f32) -> Task<Message> {
        self.offset = offset;
        operation::scroll_to(self.id.clone(), AbsoluteOffset { x: None, y: Some(offset) })
    }

    pub fn view<'a>(
        &'a self,
        look: Look<'a>,
        queue: &'a [Track],
        current: Option<usize>,
        list_h: f32,
    ) -> Element<'a, Message> {
        let count = queue.len();
        let total = count as f32 * ROW;
        let first = ((self.offset / ROW).floor().max(0.0) as usize).saturating_sub(OVERSCAN);
        let last = (((self.offset + list_h) / ROW).ceil().max(0.0) as usize + OVERSCAN).min(count);
        let mut layers = vec![space().width(Length::Fill).height(Length::Fixed(total)).into()];
        for (index, track) in queue.iter().enumerate().take(last).skip(first) {
            let face = queue_row(self, look, track, index, current == Some(index));
            layers.push(pin(face).y(index as f32 * ROW).width(Length::Fill).into());
        }
        let rows = stack(layers).width(Length::Fill).height(Length::Fixed(total));
        scrollable(rows)
            .id(self.id.clone())
            .direction(settings_kit::scrollbar())
            .style(settings_kit::scroll_style(look.tokens))
            .on_scroll(Message::Scrolled)
            .width(Length::Fill)
            .height(Length::Fixed(list_h))
            .into()
    }
}

#[derive(Clone, Copy)]
pub struct Look<'a> {
    pub i18n: &'a I18n,
    pub tokens: Tokens,
    pub ink: Color,
}

/// `QueueRow`: the playing marker, the song, and on hover its three actions.
fn queue_row<'a>(
    list: &QueueList,
    look: Look<'a>,
    track: &'a Track,
    index: usize,
    active: bool,
) -> Element<'a, Message> {
    let ink = look.ink;
    let hot = list.hover == Some(index);
    let marker = container(space()).width(3).height(20).style(move |_theme| container::Style {
        background: active.then_some(Background::Color(look.tokens.accent)),
        border: Border {
            radius: iced::border::Radius { top_right: 2.0, bottom_right: 2.0, ..Default::default() },
            ..Border::default()
        },
        ..container::Style::default()
    });
    let marker = container(marker)
        .width(Length::Fixed(3.0))
        .height(Length::Fill)
        .align_x(Alignment::Start)
        .align_y(Alignment::Center);
    let title = text(track.title.as_str())
        .size(13)
        .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(19.0)))
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None);
    let title = if active { title.font(crate::typeface::font(iced::font::Weight::Semibold)) } else { title };
    let artist = text(track.artist.as_deref().unwrap_or(""))
        .size(12)
        .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(17.0)))
        .color(look.tokens.t3)
        .wrapping(iced::widget::text::Wrapping::None);
    let words = container(column![title, artist])
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .clip(true)
        .padding(iced::Padding { top: 0.0, right: 8.0, bottom: 0.0, left: 12.0 });
    let song = button(stack![words, marker])
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(0)
        .on_press(Message::Jump(index))
        .style(|_theme, _status| button::Style::default());
    let mut face = row![song].height(Length::Fill).align_y(Alignment::Center);
    if hot {
        let lit = |which: u8| list.action_hover == Some((index, which));
        let actions = row![
            action(look, Icon::ListPlus, "queue.playNext", Message::MoveNext(index), (index, 0), lit(0)),
            action(look, Icon::ListEnd, "queue.moveToEnd", Message::MoveEnd(index), (index, 1), lit(1)),
            action(look, Icon::Trash, "queue.remove", Message::Remove(index), (index, 2), lit(2)),
        ]
        .spacing(2)
        .align_y(Alignment::Center)
        .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 8.0 });
        face = face.push(actions);
    }
    let fill = if active || hot { Some(look.tokens.fill) } else { None };
    let body = container(face)
        .width(Length::Fill)
        .height(Length::Fixed(ROW))
        .padding(iced::Padding { top: 0.0, right: 6.0, bottom: 0.0, left: 0.0 })
        .align_y(Alignment::Center)
        .style(move |_theme| container::Style {
            background: fill.map(Background::Color),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..container::Style::default()
        });
    mouse_area(body).on_enter(Message::Hover(Some(index))).on_exit(Message::Unhover(index)).into()
}

/// `PanelIconButton` at `s`: 28px, radius 8, the glyph at `--st-t2` and `--st-t1` while hovered.
fn action<'a>(
    look: Look<'a>,
    glyph: Icon,
    label: &'static str,
    message: Message,
    key: (usize, u8),
    hot: bool,
) -> Element<'a, Message> {
    let color = if hot { look.ink } else { look.tokens.t2 };
    let glyph = icon(glyph, 15.0, color);
    let press = button(container(glyph).center(Length::Fixed(28.0))).padding(0).on_press(message).style(move |_, _| {
        button::Style {
            background: hot.then_some(Background::Color(look.tokens.fill)),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..button::Style::default()
        }
    });
    let press = mouse_area(press).on_enter(Message::ActionHover(Some(key))).on_exit(Message::ActionUnhover(key));
    tooltip(press, text(look.i18n.t(label).to_owned()).size(12), tooltip::Position::Top)
        .style(container::rounded_box)
        .into()
}

fn max_offset(count: usize, list_h: f32) -> f32 {
    (count as f32 * ROW - list_h).max(0.0)
}

/// The row before the playing one at the top, so the song just played stays in view.
fn top_of(index: usize) -> f32 {
    index.saturating_sub(1) as f32 * ROW
}
