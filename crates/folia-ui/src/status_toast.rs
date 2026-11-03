// Subscription failures and sync results use the original StatusToast's single-line plate and lifetime.

use folia_i18n::I18n;
use folia_omni::Platform;
use iced::widget::{container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Theme, Vector};

use crate::platform_icon::mark_faded;

pub fn view<'a, M: 'a>(
    i18n: &I18n,
    platform: Platform,
    daylight: bool,
    width: f32,
    opacity: f32,
    shift: f32,
) -> Element<'a, M> {
    let title = i18n.format("status.songSubscriptionRequired", &[("provider", platform.short_name)]);
    plate(mark_faded(platform, 20.0, opacity), title, daylight, width, opacity, shift)
}

/// What the sync button's read did, as the toast says it.
#[derive(Debug, Clone, Copy)]
pub struct SyncOutcome {
    pub ok: bool,
    pub signed_out: bool,
}

/// The sync button's `StatusToast`: the platform's mark, then what the read did.
pub fn sync_view<'a, M: 'a>(
    i18n: &I18n,
    platform: Platform,
    outcome: SyncOutcome,
    daylight: bool,
    width: f32,
    opacity: f32,
    shift: f32,
) -> Element<'a, M> {
    let key = if outcome.signed_out {
        "status.loginExpired"
    } else if outcome.ok {
        "status.dataSynced"
    } else {
        "status.syncFailed"
    };
    let title = i18n.t(key).to_owned();
    plate(mark_faded(platform, 20.0, opacity), title, daylight, width, opacity, shift)
}

fn plate<'a, M: 'a>(
    mark: Element<'a, M>,
    title: String,
    daylight: bool,
    width: f32,
    opacity: f32,
    shift: f32,
) -> Element<'a, M> {
    let ink = if daylight { Color::from_rgb8(0x27, 0x27, 0x2a) } else { Color::WHITE };
    let ink = Color { a: opacity, ..ink };
    let content = row![
        mark,
        text(title)
            .size(14)
            .font(crate::typeface::font(iced::font::Weight::Medium))
            .color(ink)
            .line_height(iced::widget::text::LineHeight::Absolute(24.0.into()))
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let plate = container(row![content, iced::widget::space().width(0).height(26)].align_y(Alignment::Center))
        .padding([9.0, 16.0])
        .max_width(640.0_f32.min((width - 32.0).max(1.0)))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(
                1.0,
                1.0,
                1.0,
                opacity * if daylight { 0.72 } else { 0.1 },
            ))),
            border: Border {
                radius: 22.0.into(),
                width: 1.0,
                color: if daylight {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.05 * opacity)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.08 * opacity)
                },
            },
            shadow: Shadow {
                color: if daylight {
                    Color::from_rgba8(15, 15, 30, 0.12 * opacity)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.38 * opacity)
                },
                offset: Vector::new(0.0, 12.0),
                blur_radius: 32.0,
            },
            ..container::Style::default()
        });
    container(plate)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding(Padding::ZERO.top(30.0 + shift).left(16.0).right(16.0))
        .into()
}
