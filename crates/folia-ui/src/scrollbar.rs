// The original global scrollbar (`index.css` and `useAppPreferences`): 8px, 4px radius.

use iced::widget::scrollable::{AutoScroll, Rail, Scroller};
use iced::widget::{container, scrollable};
use iced::{Background, Border, Color, Shadow, Theme};

/// Vertical bar, 8px wide, the same width as its thumb.
pub fn direction() -> scrollable::Direction {
    scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(8.0).scroller_width(8.0))
}

/// Daylight track `#cccbcc`, thumb `#ececec`, hover `#ffffff`. Night `#18181b`, `#3f3f46`, `#52525b`.
pub fn style(daylight: bool) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style {
    move |_, status| {
        let hot = matches!(
            status,
            scrollable::Status::Hovered { is_vertical_scrollbar_hovered: true, .. }
                | scrollable::Status::Dragged { is_vertical_scrollbar_dragged: true, .. }
        );
        let (track, thumb, hover) = if daylight {
            (rgb(0xcc, 0xcb, 0xcc), rgb(0xec, 0xec, 0xec), Color::WHITE)
        } else {
            (rgb(0x18, 0x18, 0x1b), rgb(0x3f, 0x3f, 0x46), rgb(0x52, 0x52, 0x5b))
        };
        let rail = Rail {
            background: Some(Background::Color(track)),
            border: Border::default(),
            scroller: Scroller {
                background: Background::Color(if hot { hover } else { thumb }),
                border: Border { radius: 4.0.into(), ..Border::default() },
            },
        };
        scrollable::Style {
            container: container::Style::default(),
            vertical_rail: rail,
            horizontal_rail: rail,
            gap: None,
            auto_scroll: AutoScroll {
                background: Color::TRANSPARENT.into(),
                border: Border::default(),
                shadow: Shadow::default(),
                icon: Color::TRANSPARENT,
            },
        }
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb8(r, g, b)
}
