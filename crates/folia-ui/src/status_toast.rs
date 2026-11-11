// Subscription failures and sync results use the original StatusToast's single-line plate and lifetime.

use crate::typeface::body_text as text;
use folia_i18n::I18n;
use folia_omni::Platform;
use folia_render::Plate;
use iced::widget::{container, row, shader, stack};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::platform_icon::mark_faded;

pub fn loading_view<'a, M: 'a>(
    i18n: &I18n,
    daylight: bool,
    width: f32,
    opacity: f32,
    shift: f32,
    angle: f32,
) -> Element<'a, M> {
    let color = Color { a: opacity, ..Color::from_rgb8(0x3b, 0x82, 0xf6) };
    plate(
        crate::icons::spinner(20.0, color, angle),
        i18n.t("status.loadingSong").to_owned(),
        daylight,
        width,
        opacity,
        shift,
    )
}

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

/// A status line for the panel's actions: a tick or an exclamation, then the words.
pub fn message_view<'a, M: 'a>(
    ok: bool,
    words: String,
    daylight: bool,
    width: f32,
    opacity: f32,
    shift: f32,
) -> Element<'a, M> {
    let colour = if ok { Color::from_rgb8(0x22, 0xc5, 0x5e) } else { Color::from_rgb8(0xef, 0x44, 0x44) };
    let mark = iced::widget::container(crate::icons::icon(
        if ok { lucide_static_svg::Icon::CircleCheck } else { lucide_static_svg::Icon::CircleAlert },
        18.0,
        Color { a: opacity, ..colour },
    ))
    .center(Length::Fixed(20.0));
    plate(mark.into(), words, daylight, width, opacity, shift)
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
    let font = crate::typeface::with(folia_render::typeface::BODY, iced::font::Weight::Medium);
    let content = row![
        mark,
        text(title.clone())
            .size(crate::typeface::size::BODY)
            .font(crate::typeface::adapt(font))
            .color(ink)
            .line_height(crate::typeface::leading::PX_24_0)
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    // `statusToast.css`: `blur(14px) saturate(1.3)`. Opacity fades the blur, the fill, the ring and the shadow.
    let max_body = 640.0_f32.min((width - 32.0).max(1.0));
    let measured = crate::typeface::measure(&title, 14.0, font, (max_body - 62.0).max(1.0));
    let body_w = (measured.width + 62.0).min(max_body).max(1.0);
    let body_h = measured.height.max(26.0) + 18.0;
    let body_x = ((width - body_w) * 0.5).max(0.0);
    let body_y = 30.0 + shift;
    let reach = 48.0;
    let shadow_y = 12.0;
    let x0 = (body_x - reach).max(0.0);
    let y0 = (body_y - (reach - shadow_y)).max(0.0);
    let x1 = (body_x + body_w + reach).min(width.max(1.0));
    let y1 = body_y + body_h + reach + shadow_y;
    let (fill, ring, shadow) = if daylight {
        ([1.0, 1.0, 1.0, 0.72], [0.0, 0.0, 0.0, 0.05], [15.0 / 255.0, 15.0 / 255.0, 30.0 / 255.0, 0.12])
    } else {
        ([1.0, 1.0, 1.0, 0.1], [1.0, 1.0, 1.0, 0.08], [0.0, 0.0, 0.0, 0.38])
    };
    let plate = Plate {
        radius: 22.0,
        fill_origin: [body_x - x0, body_y - y0],
        fill_size: [body_w, body_h],
        fill,
        shadow,
        shadow_blur: 32.0,
        shadow_y,
        shadow_spread: 0.0,
        ring,
        backdrop: if opacity > 0.0 { 14.0 } else { 0.0 },
        fade: opacity,
        saturate: 1.3,
    };
    let glass = shader(plate).width(Length::Fixed((x1 - x0).max(1.0))).height(Length::Fixed((y1 - y0).max(1.0)));
    let glass = container(glass).width(Length::Fill).height(Length::Fill).padding(Padding {
        top: y0,
        right: 0.0,
        bottom: 0.0,
        left: x0,
    });
    let body = container(row![content, iced::widget::space().width(0).height(26)].align_y(Alignment::Center))
        .width(Length::Fixed(body_w))
        .height(Length::Fixed(body_h))
        .padding([9.0, 16.0]);
    let body = container(body).width(Length::Fill).height(Length::Fill).padding(Padding {
        top: body_y,
        right: 0.0,
        bottom: 0.0,
        left: body_x,
    });
    stack![glass, body].width(Length::Fill).height(Length::Fixed(y1.max(1.0))).into()
}
