// The lyrics timeline window from `LyricsTimelineModal.tsx`: alternating lyric cards on a centre line, the current
// line centred, and a click seeks to that line.

use std::time::Instant;

use folia_core::{Line, Lyrics, Theme};
use folia_i18n::I18n;
use folia_render::Plate;
use folia_visualizer::Ease;
use iced::widget::scrollable::{Direction, Scrollbar, Viewport};
use iced::widget::{
    Id, button, column, container, mouse_area, opaque, pin, row, scrollable, shader, space, stack, text,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Size, Task, mouse};
use lucide_static_svg::Icon;

use crate::icons::icon;
use crate::motion::Tween;
use crate::smooth_scroll::{self, Scroll};

/// The scrollable id used for the automatic centring.
const LIST: &str = "lyrics-timeline";
/// `setTimeout(..., 3000)` after a user scroll.
pub const USER_SCROLL_MS: u64 = 3000;
/// framer's default enter and exit transition for opacity and scale.
const SHOWN_MS: f64 = 300.0;
const SHOWN_EASE: Ease = Ease::new(0.42, 0.0, 0.58, 1.0);
/// `mb-20` under a row and `py-8` around the whole list.
const ROW_GAP: f32 = 80.0;
const LIST_PAD: f32 = 32.0;
/// The card's `p-6` and the scrollable's `px-8`.
const CARD_PAD: f32 = 24.0;
const LIST_SIDE: f32 = 32.0;
const CARD_RADIUS: f32 = 8.0;
/// The scrollable reserves a 4px bar and 4px margins on both sides.
const BAR_ROOM: f32 = 12.0;
/// Extra rows built above and below the viewport.
const OVERSCAN: f32 = 240.0;
/// Tailwind `max-w-4xl` and `w-[90vw]`, `h-[80vh]`.
const CARD_MAX_W: f32 = 896.0;
const CARD_PAD_OUT: f32 = 32.0;
/// The header's 32px line and its `mb-8`.
const HEADER_H: f32 = 32.0;
const HEADER_GAP: f32 = 32.0;

/// The line sung at `time`: the first one whose span contains it (the original's `findIndex`).
pub fn active_line(lines: &[Line], time: f64) -> Option<usize> {
    lines.iter().position(|line| time >= line.start && time <= line.end)
}

/// `resolveLyricSeekMediaTime`: a line start on the lyric clock back to a position of the media.
pub fn seek_time(line_start: f64, lyric_offset: f64, duration: f64) -> Option<f64> {
    let target = line_start + lyric_offset;
    if !target.is_finite() {
        return None;
    }
    if duration.is_finite() && duration > 0.0 && target >= duration {
        return None;
    }
    Some(target.max(0.0))
}

/// The active line and the three seconds after a user scroll in which the window does not follow it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Follow {
    active: Option<usize>,
    user_at: Option<Instant>,
}

impl Follow {
    pub fn active(&self) -> Option<usize> {
        self.active
    }

    /// Moves the active line to `time`; `true` when it changed.
    pub fn track(&mut self, lines: &[Line], time: f64) -> bool {
        let Some(index) = active_line(lines, time) else { return false };
        if self.active == Some(index) {
            return false;
        }
        self.active = Some(index);
        true
    }

    pub fn user_scrolled(&mut self, now: Instant) {
        self.user_at = Some(now);
    }

    /// Auto-scroll resumes after three seconds; the caller schedules the wake-up.
    pub fn auto_allowed(&self, now: Instant) -> bool {
        self.user_at.is_none_or(|at| now.saturating_duration_since(at).as_millis() >= u128::from(USER_SCROLL_MS))
    }

    pub fn reset(&mut self) {
        self.active = None;
        self.user_at = None;
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Close,
    /// A row's card or dot: seek to the line and keep the window open.
    Seek(usize),
    Wheel(mouse::ScrollDelta),
    Scrolled(Viewport),
    Frame,
    /// The three seconds after a user scroll are over.
    Resume(u64),
    HoverCard(Option<usize>),
    HoverDot(Option<usize>),
}

/// What the shell does after a message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    None,
    Seek(f64),
}

#[derive(Debug, Clone, Copy)]
struct Row {
    y: f32,
    height: f32,
}

/// Colours and measurements shared by every row of one frame.
#[derive(Debug, Clone, Copy)]
struct Paint {
    daylight: bool,
    accent: Color,
    line: Color,
    shown: f32,
    row_w: f32,
    active: Option<usize>,
    disabled: bool,
}

/// The playing state the timeline follows each update.
#[derive(Clone, Copy)]
pub struct Sync<'a> {
    pub song: Option<i64>,
    pub lyrics: Option<&'a Lyrics>,
    pub duration: f64,
    pub time: f64,
    pub window: Size,
    pub disabled: bool,
}

pub struct Timeline {
    open: bool,
    closing: bool,
    shown: Tween,
    /// Positioning mode or another owner of the player controls: rows dim and ignore clicks (`opacity-65`).
    disabled: bool,
    follow: Follow,
    scroll: Scroll,
    /// Bumped on every user scroll; stale wake-ups are dropped.
    resume_serial: u64,
    /// The modal has only just opened; the first frame centres the line without a tween.
    center_pending: bool,
    rows: Vec<Row>,
    content_h: f32,
    viewport_h: f32,
    row_w: f32,
    /// An automatic scroll ran recently; its own scroll events are not the user's (the original's 1s flag).
    auto_until: Option<Instant>,
    song: Option<i64>,
    layout: Option<(u64, u64, u64, u64)>,
    hover_card: Option<usize>,
    hover_dot: Option<usize>,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            open: false,
            closing: false,
            shown: Tween::new(0.0, SHOWN_MS, SHOWN_EASE, Instant::now()),
            disabled: false,
            follow: Follow::default(),
            scroll: Scroll::default(),
            resume_serial: 0,
            center_pending: false,
            rows: Vec::new(),
            content_h: 0.0,
            viewport_h: 0.0,
            row_w: 0.0,
            auto_until: None,
            song: None,
            layout: None,
            hover_card: None,
            hover_dot: None,
        }
    }
}

impl Timeline {
    pub fn open(
        &mut self,
        song: Option<i64>,
        lyrics: Option<&Lyrics>,
        duration: f64,
        time: f64,
        window: Size,
        now: Instant,
    ) {
        self.open = true;
        self.closing = false;
        self.shown = Tween::new(0.0, SHOWN_MS, SHOWN_EASE, now);
        self.shown.set(1.0, now);
        self.follow.reset();
        self.scroll.reset();
        self.center_pending = true;
        self.song = song;
        self.layout = None;
        self.hover_card = None;
        self.hover_dot = None;
        if let Some(lyrics) = lyrics.filter(|_| duration > 0.0) {
            self.follow.track(&lyrics.lines, time);
        }
        self.measure(window, lyrics, duration);
    }

    pub fn close(&mut self, now: Instant) {
        if !self.open || self.closing {
            return;
        }
        self.closing = true;
        self.shown.set_with(0.0, SHOWN_MS, SHOWN_EASE, now);
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The window is on screen, entering or leaving.
    pub fn visible(&self, now: Instant) -> bool {
        self.open || self.shown.value(now) > 0.001
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.shown.running(now) || self.scroll.animating(now) || self.center_pending
    }

    /// Follows the lyrics clock and the playing song; called after every shell update.
    pub fn sync(&mut self, state: Sync<'_>, now: Instant) {
        if !self.open {
            return;
        }
        let Sync { song, lyrics, duration, time, window, disabled } = state;
        self.disabled = disabled;
        if song != self.song {
            self.song = song;
            self.follow.reset();
            self.scroll.reset();
            self.center_pending = true;
            self.layout = None;
        }
        self.measure(window, lyrics, duration);
        let Some(lyrics) = lyrics.filter(|_| duration > 0.0) else { return };
        if self.follow.track(&lyrics.lines, time) && !self.center_pending && self.follow.auto_allowed(now) {
            self.follow_active(now);
        }
    }

    pub fn update(
        &mut self,
        message: Message,
        lyrics: Option<&Lyrics>,
        lyric_offset: f64,
        duration: f64,
        now: Instant,
    ) -> (Task<Message>, Effect) {
        match message {
            Message::Close => {
                self.close(now);
                (Task::none(), Effect::None)
            }
            Message::Seek(index) => {
                let Some(line) = lyrics.and_then(|lyrics| lyrics.lines.get(index)) else {
                    return (Task::none(), Effect::None);
                };
                let effect = seek_time(line.start, lyric_offset, duration).map_or(Effect::None, Effect::Seek);
                (Task::none(), effect)
            }
            Message::Wheel(delta) => {
                self.scroll.wheel(delta, now);
                (self.user_scrolled(now), Effect::None)
            }
            Message::Scrolled(viewport) => {
                if self.auto_until.is_some_and(|until| now < until) {
                    let (_, content, window) = smooth_scroll::reported(&viewport);
                    self.scroll.viewport(content, window);
                    return (Task::none(), Effect::None);
                }
                self.auto_until = None;
                if self.scroll.scrolled(&viewport, now) {
                    (self.user_scrolled(now), Effect::None)
                } else {
                    (Task::none(), Effect::None)
                }
            }
            Message::Frame => (self.frame(now), Effect::None),
            Message::Resume(serial) => {
                if serial == self.resume_serial && self.open && self.follow.auto_allowed(now) {
                    self.follow_active(now);
                    (self.scroll.apply(Id::new(LIST)), Effect::None)
                } else {
                    (Task::none(), Effect::None)
                }
            }
            Message::HoverCard(index) => {
                self.hover_card = index;
                (Task::none(), Effect::None)
            }
            Message::HoverDot(index) => {
                self.hover_dot = index;
                (Task::none(), Effect::None)
            }
        }
    }

    fn user_scrolled(&mut self, now: Instant) -> Task<Message> {
        self.follow.user_scrolled(now);
        self.resume_serial = self.resume_serial.wrapping_add(1);
        let serial = self.resume_serial;
        let wait = crate::background::blocking(|| std::thread::sleep(std::time::Duration::from_millis(USER_SCROLL_MS)));
        Task::batch([self.scroll.apply(Id::new(LIST)), Task::perform(wait, move |_| Message::Resume(serial))])
    }

    fn frame(&mut self, now: Instant) -> Task<Message> {
        if self.center_pending {
            self.center_pending = false;
            if let Some(target) = self.active_offset() {
                self.scroll.jump(target);
                self.auto_until = Some(now + std::time::Duration::from_millis(1000));
                return self.scroll.apply(Id::new(LIST));
            }
        }
        let moved = self.scroll.frame(now);
        if self.closing && !self.shown.running(now) {
            self.open = false;
            self.closing = false;
        }
        match moved {
            Some(_) => self.scroll.apply(Id::new(LIST)),
            None => Task::none(),
        }
    }

    /// Starts the 300ms scroll that centres the active line.
    fn follow_active(&mut self, now: Instant) {
        if let Some(target) = self.active_offset() {
            self.scroll.scroll_to(target, now);
            self.auto_until = Some(now + std::time::Duration::from_millis(1000));
        }
    }

    fn active_offset(&self) -> Option<f32> {
        let row = self.rows.get(self.follow.active()?)?;
        let max = (self.content_h - self.viewport_h).max(0.0);
        Some((row.y + row.height / 2.0 - self.viewport_h / 2.0).clamp(0.0, max))
    }

    /// Row heights and offsets; only a new window, song or lyric list lays them out again.
    fn measure(&mut self, window: Size, lyrics: Option<&Lyrics>, duration: f64) {
        let lines = lyrics.filter(|_| duration > 0.0).map_or(&[][..], |lyrics| lyrics.lines.as_slice());
        let first = lines.first().map_or(0.0f64, |line| line.start);
        let last = lines.last().map_or(0.0f64, |line| line.end);
        let key = (
            window.width.to_bits().into(),
            window.height.to_bits().into(),
            (lines.len() as u64) << 32 | (first.to_bits() & 0xffff_ffff),
            last.to_bits(),
        );
        if self.layout == Some(key) {
            return;
        }
        self.layout = Some(key);
        let card_w = (window.width * 0.9).clamp(1.0, CARD_MAX_W);
        let card_h = (window.height * 0.8).max(1.0);
        let inner_w = (card_w - 2.0 * CARD_PAD_OUT).max(1.0);
        self.viewport_h = (card_h - 2.0 * CARD_PAD_OUT - HEADER_H - HEADER_GAP).max(1.0);
        self.row_w = (inner_w - 2.0 * LIST_SIDE - BAR_ROOM).max(1.0);
        let row_w = self.row_w;
        let text_w = ((row_w / 2.0 - 16.0) * 0.9 - 2.0 * CARD_PAD - 2.0).max(1.0);
        self.rows.clear();
        let mut y = LIST_PAD;
        for line in lines {
            let primary = folia_visualizer::measure(&line.text, 16.0, primary_font(), text_w).height;
            let translation = line.translation.as_deref().map_or(0.0, |translation| {
                4.0 + folia_visualizer::measure(translation, 14.0, secondary_font(), text_w).height
            });
            let height = (2.0 * CARD_PAD + primary + translation + 2.0).max(12.0);
            self.rows.push(Row { y, height });
            y += height + ROW_GAP;
        }
        self.content_h = y + LIST_PAD;
    }

    /// The whole window, or `None` once a closed window has finished leaving.
    pub fn view<'a>(
        &'a self,
        i18n: &'a I18n,
        theme: &'a Theme,
        daylight: bool,
        window: Size,
        lyrics: Option<&'a Lyrics>,
        now: Instant,
    ) -> Option<Element<'a, Message>> {
        if !self.visible(now) {
            return None;
        }
        let shown = self.shown.value(now);
        if shown <= 0.001 && !self.open {
            return None;
        }
        let accent = folia_visualizer::color(theme.accent, 1.0);
        let secondary = folia_visualizer::color(theme.secondary, 1.0);
        let card_w = (window.width * 0.9).clamp(1.0, CARD_MAX_W);
        let card_h = (window.height * 0.8).max(1.0);
        let scrim = shader(Plate {
            radius: 0.0,
            fill_origin: [0.0, 0.0],
            fill_size: [0.0, 0.0],
            fill: [0.0, 0.0, 0.0, 0.6 * shown],
            shadow: [0.0; 4],
            shadow_blur: 0.0,
            shadow_y: 0.0,
            shadow_spread: 0.0,
            ring: [0.0; 4],
            backdrop: 12.0,
            fade: shown,
            saturate: 1.0,
        })
        .width(Length::Fill)
        .height(Length::Fill);
        let scrim: Element<'a, Message> = opaque(mouse_area(scrim).on_press(Message::Close));

        let card = container(self.card(i18n, lyrics, daylight, accent, secondary, shown))
            .width(Length::Fixed(card_w))
            .height(Length::Fixed(card_h))
            .padding(CARD_PAD_OUT)
            .style(move |_| container::Style {
                background: Some(Background::Color(if daylight {
                    Color { a: 0.7 * shown, ..Color::WHITE }
                } else {
                    Color { a: 0.4 * shown, ..Color::BLACK }
                })),
                border: Border {
                    color: if daylight {
                        Color { a: 0.05 * shown, ..Color::BLACK }
                    } else {
                        Color { a: 0.1 * shown, ..Color::WHITE }
                    },
                    width: 1.0,
                    radius: 16.0.into(),
                },
                ..container::Style::default()
            });
        let card = container(card).width(Length::Fill).height(Length::Fill).center(Length::Fill);
        // The card stops a press from reaching the scrim (`e.stopPropagation()`); a press on its padding is a no-op.
        let card = mouse_area(card).on_press(Message::HoverCard(None));
        // Only the card scales; the scrim stays whole. A scaled float is painted above every layer.
        let card: Element<'a, Message> = if (shown - 1.0).abs() < 0.001 {
            card.into()
        } else {
            crate::float_layer::place(card, false, iced::Vector::new(0.0, 0.0001), 0.9 + 0.1 * shown)
        };
        let layer = stack![scrim, card];
        Some(opaque(layer))
    }

    fn card<'a>(
        &'a self,
        i18n: &'a I18n,
        lyrics: Option<&'a Lyrics>,
        daylight: bool,
        accent: Color,
        secondary: Color,
        shown: f32,
    ) -> Element<'a, Message> {
        let header_text = Color { a: 0.9 * shown, ..if daylight { zinc(0x27272a) } else { Color::WHITE } };
        let close_ink = Color { a: 0.7 * shown, ..if daylight { zinc(0x27272a) } else { Color::WHITE } };
        let title = text(i18n.t("timeline.title").to_owned())
            .size(24)
            .font(crate::typeface::font(iced::font::Weight::Bold))
            .color(header_text);
        // `p-2` around a 24px X: a 40px circle.
        let close = button(container(icon(Icon::X, 24.0, close_ink)).center(Length::Fill))
            .width(Length::Fixed(40.0))
            .height(Length::Fixed(40.0))
            .padding(0)
            .on_press(Message::Close)
            .style(move |_, status| button::Style {
                background: matches!(status, button::Status::Hovered | button::Status::Pressed).then_some(
                    Background::Color(if daylight {
                        Color { a: 0.05 * shown, ..Color::BLACK }
                    } else {
                        Color { a: 0.1 * shown, ..Color::WHITE }
                    }),
                ),
                border: Border { radius: 20.0.into(), ..Border::default() },
                ..button::Style::default()
            });
        let head =
            container(row![title, space().width(Length::Fill), close].align_y(Alignment::Center).width(Length::Fill))
                .padding(Padding { bottom: HEADER_GAP, ..Padding::ZERO });
        let list = match lyrics {
            Some(lyrics) if !self.rows.is_empty() => self.list(lyrics, daylight, accent, secondary, shown),
            _ => container(text(i18n.t("timeline.noLyrics").to_owned()).size(14).color(Color {
                a: (if daylight { 0.5 } else { 0.6 }) * shown,
                ..if daylight { zinc(0x71717a) } else { Color::WHITE }
            }))
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .into(),
        };
        column![head, list].width(Length::Fill).height(Length::Fill).into()
    }

    fn list<'a>(
        &'a self,
        lyrics: &'a Lyrics,
        daylight: bool,
        accent: Color,
        secondary: Color,
        shown: f32,
    ) -> Element<'a, Message> {
        let offset = self.scroll.offset();
        let first = self.rows.partition_point(|row| row.y + row.height < offset - OVERSCAN);
        let last = self.rows.partition_point(|row| row.y <= offset + self.viewport_h + OVERSCAN);
        let row_w = self.row_w.max(1.0);
        let mut layers: Vec<Element<'a, Message>> =
            vec![space().width(Length::Fill).height(Length::Fixed(self.content_h)).into()];
        let line_color = Color { a: 0.3 * shown, ..secondary };
        layers.push(
            pin(container(space()).width(Length::Fixed(2.0)).height(Length::Fixed(self.content_h)).style(move |_| {
                container::Style { background: Some(Background::Color(line_color)), ..container::Style::default() }
            }))
            .x((row_w / 2.0 - 1.0).max(0.0))
            .y(0.0)
            .into(),
        );
        let active = self.follow.active();
        let paint = Paint { daylight, accent, line: secondary, shown, row_w, active, disabled: self.disabled };
        for (index, (line, metric)) in lyrics.lines.iter().zip(&self.rows).enumerate().take(last).skip(first) {
            layers.push(pin(self.row_card(line, index, paint, metric)).y(metric.y).width(Length::Fixed(row_w)).into());
        }
        for (index, metric) in self.rows.iter().enumerate().take(last).skip(first) {
            layers.push(self.dot(index, paint, metric));
        }
        let body = stack(layers).width(Length::Fixed(row_w)).height(Length::Fixed(self.content_h));
        let view = scrollable(container(body).padding([0.0, LIST_SIDE]).width(Length::Fill).center_x(Length::Fill))
            .id(LIST)
            .direction(Direction::Vertical(Scrollbar::new().width(4).scroller_width(4).margin(4)))
            .style(move |_, _| iced::widget::scrollable::Style {
                container: container::Style::default(),
                vertical_rail: iced::widget::scrollable::Rail {
                    background: None,
                    border: Border::default(),
                    scroller: iced::widget::scrollable::Scroller {
                        background: Color { a: 0.2 * shown, ..Color::WHITE }.into(),
                        border: Border { radius: 2.0.into(), ..Border::default() },
                    },
                },
                horizontal_rail: iced::widget::scrollable::Rail {
                    background: None,
                    border: Border::default(),
                    scroller: iced::widget::scrollable::Scroller {
                        background: Color::TRANSPARENT.into(),
                        border: Border::default(),
                    },
                },
                gap: None,
                auto_scroll: iced::widget::scrollable::AutoScroll {
                    background: Color::TRANSPARENT.into(),
                    border: Border::default(),
                    shadow: Shadow::default(),
                    icon: Color::TRANSPARENT,
                },
            })
            .on_scroll(Message::Scrolled)
            .width(Length::Fill)
            .height(Length::Fill);
        smooth_scroll::smooth(view, Message::Wheel)
    }

    fn row_card<'a>(&'a self, line: &'a Line, index: usize, paint: Paint, metric: &Row) -> Element<'a, Message> {
        let active = paint.active == Some(index);
        let factor = if active { 1.0 } else { 0.7 };
        let dim = if paint.disabled { 0.65 } else { 1.0 };
        let alpha = paint.shown * factor * dim;
        let primary = if paint.daylight { zinc(0x27272a) } else { Color::WHITE };
        let secondary = if paint.daylight { zinc(0x71717a) } else { Color { a: 0.6, ..Color::WHITE } };
        let pad = if active { CARD_PAD } else { CARD_PAD * 0.85 };
        let align = if index.is_multiple_of(2) { Alignment::End } else { Alignment::Start };
        let mut body: iced::widget::Column<'a, Message> = column![
            text(line.text.clone())
                .size(if active { 16.0 } else { 14.0 })
                .font(if active { primary_font() } else { regular_font() })
                .line_height(iced::widget::text::LineHeight::Relative(1.22))
                .color(Color { a: alpha, ..primary })
                .align_x(align)
        ]
        .spacing(4);
        if let Some(translation) = &line.translation {
            body = body.push(
                text(translation.clone())
                    .size(if active { 14.0 } else { 12.0 })
                    .line_height(iced::widget::text::LineHeight::Relative(1.22))
                    .color(Color { a: alpha, ..secondary })
                    .align_x(align),
            );
        }
        let hot = !paint.disabled && self.hover_card == Some(index);
        let fill_color = match (paint.daylight, hot) {
            (true, false) => Color { a: 0.6 * alpha, ..Color::WHITE },
            (true, true) => Color { a: 0.8 * alpha, ..Color::WHITE },
            (false, false) => Color { a: 0.6 * alpha, ..Color::BLACK },
            (false, true) => Color { a: 0.7 * alpha, ..Color::BLACK },
        };
        let border = if paint.daylight {
            Color { a: 0.05 * alpha, ..Color::BLACK }
        } else {
            Color { a: 0.1 * alpha, ..Color::WHITE }
        };
        let card =
            container(body).padding(pad).max_width((paint.row_w / 2.0 - 16.0) * 0.9).style(move |_: &iced::Theme| {
                container::Style {
                    background: Some(Background::Color(fill_color)),
                    border: Border { color: border, width: 1.0, radius: CARD_RADIUS.into() },
                    ..container::Style::default()
                }
            });
        let card = mouse_area(card).on_enter(Message::HoverCard(Some(index))).on_exit(Message::HoverCard(None));
        let card: Element<'a, Message> =
            if paint.disabled { card.into() } else { card.on_press(Message::Seek(index)).into() };
        let half = paint.row_w / 2.0;
        let left = index.is_multiple_of(2);
        let slot = container(card)
            .width(Length::Fixed(half))
            .height(Length::Fixed(metric.height))
            .align_x(if left { Alignment::End } else { Alignment::Start })
            .padding(Padding {
                left: if left { 0.0 } else { 16.0 },
                right: if left { 16.0 } else { 0.0 },
                ..Padding::ZERO
            });
        let blank = || container(space()).width(Length::Fixed(half)).height(Length::Fixed(metric.height));
        if left {
            row![slot, blank()].width(Length::Fixed(paint.row_w)).height(Length::Fixed(metric.height)).into()
        } else {
            row![blank(), slot].width(Length::Fixed(paint.row_w)).height(Length::Fixed(metric.height)).into()
        }
    }

    fn dot<'a>(&'a self, index: usize, paint: Paint, metric: &Row) -> Element<'a, Message> {
        let active = paint.active == Some(index);
        let hovered = self.hover_dot == Some(index);
        let scale = if active {
            if hovered { 1.7 } else { 1.5 }
        } else if hovered {
            1.3
        } else {
            1.0
        };
        let size = 12.0 * scale;
        let color = if active { paint.accent } else { paint.line };
        let color = Color { a: color.a * paint.shown, ..color };
        let shadow = if active {
            Shadow {
                color: Color { a: 0.5 * paint.shown, ..paint.accent },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 20.0,
            }
        } else {
            Shadow::default()
        };
        let face = container(space()).width(Length::Fixed(size)).height(Length::Fixed(size)).style(move |_| {
            container::Style {
                background: Some(Background::Color(color)),
                border: Border { radius: (size / 2.0).into(), ..Border::default() },
                shadow,
                ..container::Style::default()
            }
        });
        let area = mouse_area(face).on_enter(Message::HoverDot(Some(index))).on_exit(Message::HoverDot(None));
        let area: Element<'a, Message> =
            if paint.disabled { area.into() } else { area.on_press(Message::Seek(index)).into() };
        pin(area).x((paint.row_w / 2.0 - size / 2.0).max(0.0)).y(metric.y + metric.height / 2.0 - size / 2.0).into()
    }
}

fn primary_font() -> iced::Font {
    crate::typeface::font(iced::font::Weight::Medium)
}

fn regular_font() -> iced::Font {
    crate::typeface::font(iced::font::Weight::Normal)
}

fn secondary_font() -> iced::Font {
    crate::typeface::font(iced::font::Weight::Normal)
}

fn zinc(value: u32) -> Color {
    Color {
        r: f32::from((value >> 16) as u8) / 255.0,
        g: f32::from((value >> 8) as u8) / 255.0,
        b: f32::from(value as u8) / 255.0,
        a: 1.0,
    }
}
