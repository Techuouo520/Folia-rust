// Search results (`SearchResultsList` / `SearchResultRow`): a 76px virtual list, no index and no playing block.

use folia_core::Track;
use iced::widget::scrollable::Viewport;
use iced::widget::{
    Id, button, column, container, image, mouse_area, responsive, row, scrollable, space, stack, text, tooltip,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Task, Theme, mouse};
use lucide_static_svg::Icon;

use crate::icons::{Variant, icon, variant};
use crate::scrollbar;
use crate::smooth_scroll::{self, Scroll};

const ROW: f32 = 76.0;
const OVERSCAN: usize = 6;
const COVER: f32 = 48.0;
/// The results list's scrollable id, used for the program scrolls.
const LIST: &str = "search-results";

/// `m:ss` (`formatDuration`). Minutes are not padded. `seconds` is the track length.
pub fn format_duration(seconds: f64) -> String {
    let total = if seconds.is_finite() { seconds.max(0.0).round() as u64 } else { 0 };
    format!("{}:{:02}", total / 60, total % 60)
}

#[derive(Debug, Default)]
pub struct List {
    scroll: Scroll,
    viewport: f32,
    pub hovered: Option<usize>,
    plus: Option<usize>,
}

impl List {
    pub fn scrolled(&mut self, viewport: &Viewport, now: std::time::Instant) {
        self.scroll.scrolled(viewport, now);
        self.viewport = viewport.bounds().height;
    }

    pub fn wheel(&mut self, delta: mouse::ScrollDelta, now: std::time::Instant) {
        self.scroll.wheel(delta, now);
    }

    /// One frame of a smooth scroll; the results page asks for frames while this moves.
    pub fn frame<Message>(&mut self, now: std::time::Instant) -> Task<Message> {
        match self.scroll.frame(now) {
            Some(_) => self.apply(),
            None => Task::none(),
        }
    }

    pub fn apply<Message>(&self) -> Task<Message> {
        self.scroll.apply(Id::new(LIST))
    }

    pub fn animating(&self, now: std::time::Instant) -> bool {
        self.scroll.animating(now)
    }

    pub fn reset(&mut self) {
        self.scroll.reset();
        self.viewport = 0.0;
        self.hovered = None;
        self.plus = None;
    }

    pub fn plus(&mut self, index: Option<usize>) {
        self.plus = index;
    }

    /// Rows to build: the window on screen plus six above and six below.
    pub fn visible(&self, count: usize) -> std::ops::Range<usize> {
        if count == 0 {
            return 0..0;
        }
        let window = if self.viewport > 1.0 { self.viewport } else { 800.0 };
        let offset = self.scroll.offset();
        let first = ((offset / ROW).floor() as usize).saturating_sub(OVERSCAN);
        let last = (((offset + window) / ROW).ceil() as usize + OVERSCAN).min(count);
        first..last.max(first)
    }

    pub fn view<'a, M: Clone + 'a>(
        &self,
        tracks: &'a [Track],
        covers: &'a [Option<image::Handle>],
        look: Look<'a>,
        bindings: Bindings<M>,
    ) -> Element<'a, M> {
        let range = self.visible(tracks.len());
        let rows = tracks[range.clone()].iter().enumerate().map(|(offset, track)| {
            let index = range.start + offset;
            let cover = covers.get(index).and_then(Option::as_ref);
            let state = RowState { hovered: self.hovered == Some(index), plus: self.plus == Some(index) };
            row_view(index, track, cover, look, state, &bindings)
        });
        let list = column![space().height(range.start as f32 * ROW)]
            .extend(rows)
            .push(space().height((tracks.len() - range.end) as f32 * ROW));
        let list = scrollable(list)
            .id(LIST)
            .on_scroll(bindings.scroll)
            .direction(scrollbar::direction())
            .style(scrollbar::style(look.daylight))
            .height(Length::Fill);
        smooth_scroll::smooth(list, bindings.wheel)
    }
}

/// Messages a result row sends. Grouped so the list view stays a short parameter list.
#[derive(Clone, Copy)]
pub struct Bindings<M> {
    pub scroll: fn(Viewport) -> M,
    pub wheel: fn(mouse::ScrollDelta) -> M,
    pub hover: fn(Option<usize>) -> M,
    pub play: fn(usize) -> M,
    pub enqueue: fn(usize) -> M,
    pub plus: fn(Option<usize>) -> M,
}

#[derive(Clone, Copy)]
struct RowState {
    hovered: bool,
    plus: bool,
}

#[derive(Clone, Copy)]
pub struct Look<'a> {
    pub text: Color,
    pub secondary: Color,
    pub daylight: bool,
    pub wide: bool,
    pub queue_tip: &'a str,
}

fn row_view<'a, M: Clone + 'a>(
    index: usize,
    track: &'a Track,
    cover: Option<&'a image::Handle>,
    look: Look<'a>,
    state: RowState,
    bindings: &Bindings<M>,
) -> Element<'a, M> {
    let card = container(card_row(index, track, cover, look, state, bindings))
        .width(Length::Fill)
        .center_y(Length::Fill)
        .padding([0.0, 12.0])
        .style(move |_| card_style(look.daylight, state.hovered));
    let body = container(card).padding([6.0, 8.0]).width(Length::Fill).height(Length::Fixed(ROW));
    mouse_area(body).on_enter((bindings.hover)(Some(index))).on_exit((bindings.hover)(None)).into()
}

fn card_style(daylight: bool, hovered: bool) -> container::Style {
    let (border, fill, hot) = if daylight { (0.05, 0.035, 0.07) } else { (0.06, 0.04, 0.08) };
    let ink = if daylight { Color::BLACK } else { Color::WHITE };
    container::Style {
        background: Some(Background::Color(Color { a: if hovered { hot } else { fill }, ..ink })),
        border: Border { color: Color { a: border, ..ink }, width: 1.0, radius: 16.0.into() },
        ..container::Style::default()
    }
}

fn card_row<'a, M: Clone + 'a>(
    index: usize,
    track: &'a Track,
    cover: Option<&'a image::Handle>,
    look: Look<'a>,
    state: RowState,
    bindings: &Bindings<M>,
) -> Element<'a, M> {
    let cover = cover_button(index, cover, look.text, state.hovered, bindings.play);
    let mut row = row![cover, titles(index, track, look, bindings.play)].spacing(12.0).align_y(Alignment::Center);
    if look.wide {
        row = row.push(duration_text(track, look.text));
    }
    row.push(queue_button(index, look, state, bindings)).into()
}

fn cover_button<'a, M: Clone + 'a>(
    index: usize,
    cover: Option<&'a image::Handle>,
    ink: Color,
    hovered: bool,
    on_play: fn(usize) -> M,
) -> Element<'a, M> {
    let plate = Color { r: 0x71 as f32 / 255.0, g: 0x71 as f32 / 255.0, b: 0x7a as f32 / 255.0, a: 0.15 };
    let face: Element<'a, M> = match cover {
        Some(handle) => image(handle.clone())
            .width(Length::Fixed(COVER))
            .height(Length::Fixed(COVER))
            .content_fit(iced::ContentFit::Cover)
            .border_radius(12.0)
            .into(),
        None => container(icon(Icon::Disc, 22.0, Color { a: 0.3, ..ink })).center(Length::Fixed(COVER)).into(),
    };
    // `opacity-0 group-hover:opacity-100`: the dim and the play mark only on the hovered row.
    let mut layers = stack![face];
    if hovered {
        layers = layers.push(
            container(variant(Variant::FilledPlay, 18.0, Color::WHITE)).center(Length::Fixed(COVER)).style(|_| {
                container::Style {
                    background: Some(Background::Color(Color { a: 0.45, ..Color::BLACK })),
                    ..container::Style::default()
                }
            }),
        );
    }
    let face = container(layers).width(Length::Fixed(COVER)).height(Length::Fixed(COVER)).clip(true).style(move |_| {
        container::Style {
            background: Some(Background::Color(plate)),
            border: Border { radius: 12.0.into(), ..Border::default() },
            ..container::Style::default()
        }
    });
    button(face).padding(0).on_press(on_play(index)).style(bare).into()
}

fn titles<'a, M: Clone + 'a>(
    index: usize,
    track: &'a Track,
    look: Look<'a>,
    on_play: fn(usize) -> M,
) -> Element<'a, M> {
    let title = button(
        text(&track.title)
            .size(14)
            .font(crate::typeface::font(iced::font::Weight::Semibold))
            .color(look.text)
            .wrapping(text::Wrapping::None),
    )
    .padding(0)
    .on_press(on_play(index))
    .style(bare);
    let name = container(title).width(Length::Fill).clip(true);
    column![name, meta_line(track, look)].spacing(4.0).width(Length::Fill).into()
}

/// `artist • album` on one 16px line. The album keeps its width up to 45%; the artist takes what is left.
fn meta_line<'a, M: 'a>(track: &'a Track, look: Look<'a>) -> Element<'a, M> {
    let artist = track.artist.as_deref().unwrap_or("");
    let Some(album) = track.album.as_deref().filter(|name| !name.is_empty()) else {
        return container(meta_text(artist, look.secondary)).width(Length::Fill).clip(true).into();
    };
    let line = responsive(move |space| {
        let natural = |value: &str| folia_visualizer::measure(value, 12.0, iced::Font::DEFAULT, f32::INFINITY).width;
        let dot_w = natural("•").ceil() + 8.0;
        let album_w = natural(album).ceil().min(space.width * 0.45);
        let artist_w = natural(artist).ceil().min((space.width - album_w - dot_w).max(0.0));
        let dot = text("•").size(12).color(Color { a: look.secondary.a * 0.4, ..look.secondary });
        row![
            container(meta_text(artist, look.secondary)).width(Length::Fixed(artist_w)).clip(true),
            dot,
            container(meta_text(album, look.secondary)).width(Length::Fixed(album_w)).clip(true),
        ]
        .spacing(4.0)
        .align_y(Alignment::Center)
        .into()
    });
    container(line).width(Length::Fill).height(Length::Fixed(16.0)).into()
}

fn meta_text<'a>(value: &'a str, color: Color) -> iced::widget::Text<'a> {
    text(value).size(12).color(color).wrapping(text::Wrapping::None)
}

fn duration_text<'a, M: 'a>(track: &Track, ink: Color) -> Element<'a, M> {
    text(format_duration(track.duration))
        .size(12)
        .font(iced::Font::MONOSPACE)
        .color(Color { a: ink.a * 0.35, ..ink })
        .into()
}

fn queue_button<'a, M: Clone + 'a>(
    index: usize,
    look: Look<'a>,
    state: RowState,
    bindings: &Bindings<M>,
) -> Element<'a, M> {
    let shown = !look.wide || state.hovered;
    let alpha = if !shown {
        0.0
    } else if state.plus {
        1.0
    } else {
        0.6
    };
    let face = icon(Icon::Plus, 16.0, Color { a: look.text.a * alpha, ..look.text });
    let press = shown.then_some((bindings.enqueue)(index));
    let button = button(face).padding(8).on_press_maybe(press).style(move |_, status| {
        let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let wash = if look.daylight { Color::BLACK } else { Color::WHITE };
        button::Style {
            background: (hot && shown).then_some(Background::Color(Color { a: 0.10, ..wash })),
            border: Border { radius: 999.0.into(), ..Border::default() },
            ..button::Style::default()
        }
    });
    let button = mouse_area(button).on_enter((bindings.plus)(Some(index))).on_exit((bindings.plus)(None));
    if shown {
        tooltip(button, text(look.queue_tip).size(12), tooltip::Position::Top).style(container::rounded_box).into()
    } else {
        button.into()
    }
}

fn bare(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style::default()
}
