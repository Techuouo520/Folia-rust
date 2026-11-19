// The expanded queue poster's title, inline transport and square seek line.

use crate::{focus_card, icons::icon, lattice::Message, playback, typeface};
use folia_core::{Repeat, Track};
use iced::widget::{button, column, container, pin, row, slider, space, stack};
use iced::{Alignment, Border, Color, Element, Length, Point, Rectangle};
use lucide_static_svg::Icon;

pub(super) struct Poster<'a> {
    pub track: &'a Track,
    pub index: usize,
    pub player: &'a playback::Playback,
    pub rect: Rectangle,
    pub scale: f32,
    pub width: f32,
    pub ink: Color,
    pub details: f32,
    pub radio: bool,
}

pub(super) fn view<'a>(
    poster: Poster<'a>,
    lyrics: Option<Element<'a, Message>>,
    reveal: (f32, f32),
) -> Element<'a, Message> {
    let s = poster.scale;
    let rect = poster.rect;
    let ink = poster.ink;
    let artist = poster.track.artist.as_deref().unwrap_or("");
    let metadata = lyrics.is_some() || poster.radio;
    let narrow_radio = poster.radio && poster.width < 640.0;
    let title_size = if metadata { 22.0 } else { (poster.width * 0.06).clamp(48.0, 82.0) } * s;
    let title = column![
        container(
            typeface::text(&poster.track.title)
                .color(ink)
                .size(title_size)
                .font(typeface::adapt(typeface::BOLD))
                .line_height(iced::widget::text::LineHeight::Relative(if metadata { 1.2 } else { 0.95 }))
        )
        .max_height(title_size * if metadata { 1.2 } else { 2.85 })
        .clip(true),
        typeface::text(artist).size(if metadata { 13.0 } else { 17.0 } * s).color(Color { a: 0.78, ..ink }),
    ]
    .spacing(if metadata { 2.0 } else { 7.0 } * s);
    let top = if metadata {
        (if poster.radio && poster.width < 640.0 {
            56.0
        } else if poster.width < 640.0 {
            112.0
        } else {
            74.0
        }) * s
    } else {
        0.0
    };
    let mut title = container(title).width(
        (rect.width
            - if narrow_radio {
                92.0
            } else if poster.radio {
                132.0
            } else {
                64.0
            } * s)
            * if metadata { 1.0 } else { 0.76 },
    );
    if !metadata {
        title = title
            .height((rect.height - (if poster.width < 640.0 { 220.0 } else { 110.0 }) * s).max(0.0))
            .align_y(Alignment::End);
    }
    let title = pin(title).position(Point::new(if narrow_radio { 20.0 } else { 32.0 } * s, top));
    let controls = transport(&poster, reveal.0);
    let controls_height = if poster.radio {
        if poster.width < 640.0 {
            80.0
        } else if rect.width < 520.0 {
            64.0
        } else {
            72.0
        }
    } else if poster.width < 640.0 {
        104.0
    } else {
        66.0
    } * s;
    let mut body = stack![
        title,
        pin(controls).position(Point::new(
            if narrow_radio { 18.0 } else { 32.0 } * s,
            rect.height - if narrow_radio { 18.0 } else { 28.0 } * s - controls_height + reveal.1 * s
        ))
    ];
    if let Some(lyrics) = lyrics {
        body = body.push(lyrics);
    }
    pin(container(body).width(rect.width).height(rect.height).clip(true)).position(rect.position()).into()
}

fn transport<'a>(poster: &Poster<'a>, alpha: f32) -> Element<'a, Message> {
    let s = poster.scale;
    let player = poster.player;
    let current = player.current().is_some_and(|track| track.id == poster.track.id);
    let enabled = current && !player.loading.active();
    let playing = enabled && player.is_playing();
    let duration = if current { player.duration() } else { poster.track.duration }.max(0.0);
    let position = if current { player.position() } else { 0.0 };
    let mobile = poster.width < 640.0;
    let compact_radio = poster.radio && poster.rect.width <= 520.0;
    let (side, glyph, gap) = if poster.radio && mobile {
        (38.0, 18.0, 4.0)
    } else if compact_radio {
        (36.0, 18.0, 4.0)
    } else if mobile {
        (86.0, 30.0, 4.0)
    } else {
        (44.0, 20.0, 8.0)
    };
    let ink = Color { a: poster.ink.a * alpha, ..poster.ink };
    let control = |mark, action: Option<Message>, active: bool| {
        let glyph_ink = Color { a: ink.a * if action.is_none() { 0.25 } else { 1.0 }, ..ink };
        let mark = match mark {
            Icon::Play if poster.radio => {
                crate::icons::variant(crate::icons::Variant::FilledPlay, glyph * s, glyph_ink)
            }
            Icon::Pause if poster.radio => {
                crate::icons::variant(crate::icons::Variant::FilledPause, glyph * s, glyph_ink)
            }
            mark => icon(mark, glyph * s, glyph_ink),
        };
        let face = button(container(mark).center(Length::Fixed(side * s))).padding(0).on_press_maybe(action).style(
            move |_, status| {
                let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: (hover || active).then_some(Color { a: 0.16 * alpha, ..ink }.into()),
                    text_color: ink,
                    border: Border {
                        radius: (12.0 * s).into(),
                        width: if hover { s } else { 0.0 },
                        color: Color { a: 0.26 * alpha, ..ink },
                    },
                    ..Default::default()
                }
            },
        );
        Element::from(face)
    };
    let play = if current { playback::Message::Toggle } else { playback::Message::Jump(poster.index) };
    let mut buttons =
        row![control(if playing { Icon::Pause } else { Icon::Play }, Some(Message::Playback(play)), false)]
            .spacing(gap * s);
    if poster.details > 0.001 {
        let repeat = match player.settings.repeat {
            Repeat::Off => Icon::RepeatOff,
            Repeat::All => Icon::Repeat,
            Repeat::One => Icon::Repeat1,
        };
        let extra = row![]
            .push(control(
                Icon::SkipBack,
                (enabled && !player.is_fm()).then_some(Message::Playback(playback::Message::Previous)),
                false,
            ))
            .push(control(
                repeat,
                (enabled && !player.is_fm()).then_some(Message::Playback(playback::Message::CycleRepeat)),
                player.settings.repeat != Repeat::Off,
            ))
            .push(control(Icon::ChartBar, (enabled && player.lyrics.is_some()).then_some(Message::Timeline), false))
            .push(control(Icon::SkipForward, enabled.then_some(Message::Playback(playback::Message::Next)), false))
            .spacing(gap * s);
        buttons = buttons.push(container(extra).width((4.0 * side + 3.0 * gap) * s * poster.details).clip(true));
    }
    buttons = buttons.push(space().width(Length::Fill));
    if poster.details > 0.001 && !compact_radio {
        buttons = buttons.push(
            typeface::text(format!("{} / {}", crate::format_time(position), crate::format_time(duration)))
                .size(if mobile { 20.0 } else { 12.0 } * s)
                .color(Color { a: 0.6 * alpha * poster.details, ..ink }),
        );
    }
    buttons = buttons.push(control(Icon::ArrowUpRight, Some(Message::Card(focus_card::Action::Enter)), false));
    let progress: Element<'a, Message> = if enabled && duration > 0.0 {
        slider(0.0..=duration, position.min(duration), |time| Message::Playback(playback::Message::Seeking(time)))
            .on_release(Message::Playback(playback::Message::SeekDone))
            .step(0.05)
            .height(if poster.radio && mobile { 32.0 } else { 18.0 } * s)
            .style(move |_, _| slider::Style {
                rail: slider::Rail {
                    backgrounds: (ink.into(), Color { a: 0.2 * alpha, ..ink }.into()),
                    width: 2.0 * s,
                    border: Border::default(),
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Rectangle { width: 2, border_radius: 0.0.into() },
                    background: ink.into(),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            })
            .into()
    } else {
        container(space())
            .height(2.0 * s)
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Color { a: 0.2 * alpha, ..ink }.into()),
                ..Default::default()
            })
            .into()
    };
    column![
        buttons.align_y(Alignment::Center),
        container(progress).padding(iced::Padding { right: 12.0 * s, ..Default::default() })
    ]
    .spacing(if mobile && !poster.radio { 16.0 } else { 10.0 } * s)
    .width(
        (poster.rect.width - if poster.radio && mobile { 32.0 } else { 64.0 } * s)
            .min(if mobile && !poster.radio { 640.0 } else { 480.0 } * s),
    )
    .into()
}
