// The platforms' app icons for their marks, decoded once; a platform without one keeps its letter on its colour.

use std::sync::LazyLock;

use folia_core::Rgb;
use folia_omni::Platform;
use iced::widget::image::Handle;
use iced::widget::{container, text};
use iced::{Background, Border, Color, Element, Length, Theme};

const SIDE: u32 = 128;

const ICONS: [(&str, &[u8]); 6] = [
    ("netease", include_bytes!("../../../assets/platforms/netease.png")),
    ("kugou", include_bytes!("../../../assets/platforms/kugou.png")),
    ("qq", include_bytes!("../../../assets/platforms/qq.png")),
    ("bodian", include_bytes!("../../../assets/platforms/bodian.png")),
    ("spotify", include_bytes!("../../../assets/platforms/spotify.png")),
    ("qishui", include_bytes!("../../../assets/platforms/qishui.png")),
];

static DECODED: LazyLock<Vec<(&'static str, Handle)>> = LazyLock::new(|| {
    ICONS
        .iter()
        .filter_map(|&(id, bytes)| match folia_library::cover_rgba(bytes, SIDE, SIDE) {
            Ok(cover) => Some((id, Handle::from_rgba(cover.width, cover.height, cover.rgba))),
            Err(err) => {
                tracing::warn!(id, error = &err as &dyn std::error::Error, "cannot decode platform icon");
                None
            }
        })
        .collect()
});

/// The app icon drawn in a platform's mark, or `None` for the letter.
pub fn platform_icon(id: &str) -> Option<Handle> {
    DECODED.iter().find(|(icon, _)| *icon == id).map(|(_, handle)| handle.clone())
}

/// `PlatformMark`: the platform's app icon, or its letter on its colour, 42% of the size, dark or white by contrast.
pub(crate) fn mark_faded<'a, M: 'a>(platform: Platform, size: f32, alpha: f32) -> Element<'a, M> {
    if let Some(handle) = platform_icon(platform.id) {
        return iced::widget::image(handle)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .border_radius(size / 2.0)
            .opacity(alpha)
            .into();
    }
    let color = folia_visualizer::color(Rgb::hex(platform.color), alpha);
    let on = Color { a: alpha, ..on_color(platform.color) };
    container(
        text(platform.letter.to_string())
            .size((size * 0.42).round())
            .font(crate::typeface::font(iced::font::Weight::Bold))
            .color(on),
    )
    .center(Length::Fixed(size))
    .style(move |_theme: &Theme| container::Style {
        background: Some(Background::Color(color)),
        border: Border { radius: (size / 2.0).into(), ..Border::default() },
        ..container::Style::default()
    })
    .into()
}

/// `onAccentColor`: `#17171a` or white, whichever contrasts more.
pub(crate) fn on_color(hex: u32) -> Color {
    let luminance = |hex: u32| {
        let channel = |shift: u32| {
            let value = ((hex >> shift) & 0xff) as f32 / 255.0;
            if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    };
    let contrast = |a: f32, b: f32| (a.max(b) + 0.05) / (a.min(b) + 0.05);
    let base = luminance(hex);
    if contrast(luminance(0x17171a), base) >= contrast(1.0, base) {
        Color::from_rgb8(0x17, 0x17, 0x1a)
    } else {
        Color::WHITE
    }
}
