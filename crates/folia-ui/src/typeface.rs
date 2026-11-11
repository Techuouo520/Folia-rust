// Shared CSS typography for the application shell and body portals.
pub use folia_render::typeface::{SYSTEM, font, install, with};
use iced::font::Weight;
use std::sync::atomic::{AtomicBool, Ordering};

// A language switch changes only font identities; the shared immutable font data stays cached.
static VIETNAMESE: AtomicBool = AtomicBool::new(false);
pub fn language(language: folia_core::Language) {
    VIETNAMESE.store(language == folia_core::Language::Vi, Ordering::Relaxed);
}
pub fn adapt(font: iced::Font) -> iced::Font {
    folia_render::typeface::for_vietnamese(font, VIETNAMESE.load(Ordering::Relaxed))
}
pub fn interface_font_for(language: folia_core::Language) -> iced::Font {
    folia_render::typeface::for_vietnamese(SYSTEM, language == folia_core::Language::Vi)
}
pub fn measure(content: &str, size: f32, font: iced::Font, max_width: f32) -> iced::Size {
    folia_visualizer::measure(content, size, adapt(font), max_width)
}

pub mod text_input {
    pub use iced::widget::text_input::*;
}
pub fn text_input<'a, M: Clone + 'a>(placeholder: &str, value: &str) -> iced::widget::TextInput<'a, M> {
    iced::widget::text_input(placeholder, value).font(adapt(SYSTEM))
}

pub const MEDIUM: iced::Font = font(Weight::Medium);
pub const SEMIBOLD: iced::Font = font(Weight::Semibold);
pub const BOLD: iced::Font = font(Weight::Bold);
pub const EXTRA_BOLD: iced::Font = font(Weight::ExtraBold);
pub const BLACK: iced::Font = font(Weight::Black);

/// Fixed interface sizes; responsive wall and poster sizes are derived from their layout specifications.
pub mod size {
    pub const MICRO: f32 = 9.0;
    pub const SMALL_BADGE: f32 = 10.0;
    pub const QUEUE_ARTIST: f32 = 10.5;
    pub const CAPTION: f32 = 11.0;
    pub const ACCOUNT_STATUS: f32 = 11.5;
    pub const HINT: f32 = 12.0;
    pub const QUEUE_TITLE: f32 = 12.5;
    pub const CONTROL: f32 = 13.0;
    pub const BODY: f32 = 14.0;
    pub const ACTION: f32 = 15.0;
    pub const DIALOG_TITLE: f32 = 16.0;
    pub const SUBHEADING: f32 = 18.0;
    pub const POSTER_ARTIST: f32 = 19.0;
    pub const PAGE_TITLE: f32 = 20.0;
    pub const LARGE_TITLE: f32 = 24.0;
    pub const QUOTE: f32 = 28.0;
    pub const LARGE_QUOTE: f32 = 36.0;
    pub const LOGIN_INITIAL: f32 = 37.0;
    pub const LOGIN_WATERMARK: f32 = 196.0;
}

#[derive(Debug, Clone, Copy)]
pub struct TextStyle {
    pub name: &'static str,
    pub size: f32,
    pub weight: Weight,
    pub line_height: f32,
}

impl TextStyle {
    pub fn text<'a>(self, content: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
        text(content)
            .size(self.size)
            .font(adapt(font(self.weight)))
            .line_height(text::LineHeight::Absolute(self.line_height.into()))
    }
}

pub const SHEET_TITLE: TextStyle =
    TextStyle { name: "sheet-title", size: size::DIALOG_TITLE, weight: Weight::Semibold, line_height: 32.0 };
pub const PAGE_TITLE: TextStyle =
    TextStyle { name: "page-title", size: size::PAGE_TITLE, weight: Weight::Semibold, line_height: 32.0 };
pub const HEADLINE: TextStyle =
    TextStyle { name: "headline", size: size::PAGE_TITLE, weight: Weight::Semibold, line_height: 28.0 };
pub const GROUP: TextStyle =
    TextStyle { name: "group", size: size::CONTROL, weight: Weight::Semibold, line_height: 20.0 };
pub const LABEL: TextStyle = TextStyle { name: "label", size: size::BODY, weight: Weight::Normal, line_height: 20.0 };
pub const HINT: TextStyle = TextStyle { name: "hint", size: size::HINT, weight: Weight::Normal, line_height: 18.0 };
pub const ACCOUNT_STATUS: TextStyle =
    TextStyle { name: "account-status", size: size::ACCOUNT_STATUS, weight: Weight::Normal, line_height: 16.0 };
pub const PROVIDER: TextStyle =
    TextStyle { name: "provider", size: size::CONTROL, weight: Weight::Medium, line_height: 18.0 };
pub const LOG_ROW: TextStyle =
    TextStyle { name: "log-row", size: size::SMALL_BADGE, weight: Weight::Normal, line_height: 16.0 };
pub const UI_TEXT_STYLES: &[TextStyle] =
    &[SHEET_TITLE, PAGE_TITLE, HEADLINE, GROUP, LABEL, HINT, ACCOUNT_STATUS, PROVIDER, LOG_ROW];

pub fn log_caps<'a, M: 'a>(content: &str, heading: bool, color: iced::Color) -> iced::Element<'a, M> {
    folia_render::text_run::text(
        &content.to_uppercase(),
        folia_render::text_run::Style {
            font: adapt(SYSTEM),
            size: size::SMALL_BADGE,
            line_height: 15.0,
            tracking: if heading { 1.6 } else { 1.8 },
            vertical: false,
            upright: false,
            width: None,
        },
        color,
    )
}
pub mod text {
    pub use iced::widget::text::*;
}

/// Text shared by interface surfaces, including the settings kit and dialogs.
pub fn text<'a>(content: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    iced::widget::text(content).font(adapt(SYSTEM)).shaping(text::Shaping::Advanced).line_height(1.5)
}

/// Portals outside AppShell inherit the CSS body stack.
pub fn body_text<'a>(content: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    text(content).font(adapt(folia_render::typeface::BODY))
}

/// Canvas labels use the same family and shaping policy as widgets, retaining their explicit line metrics.
pub fn canvas_defaults() -> iced::widget::canvas::Text {
    iced::widget::canvas::Text {
        font: adapt(SYSTEM),
        shaping: text::Shaping::Advanced,
        ..iced::widget::canvas::Text::default()
    }
}

/// CSS line boxes shared by text widgets and canvas text.
pub mod leading {
    pub const EM_0_6: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(0.6);
    pub const EM_0_9: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(0.9);
    pub const EM_0_95: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(0.95);
    pub const EM_0_96: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(0.96);
    pub const EM_1_02: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(1.02);
    pub const EM_1_15: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(1.15);
    pub const EM_1_18: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(1.18);
    pub const EM_1_22: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(1.22);
    pub const EM_1_25: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(1.25);
    pub const EM_1_3: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(1.3);
    pub const EM_1_45: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(1.45);
    pub const EM_1_5: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(1.5);
    pub const EM_1_65: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Relative(1.65);
    pub const PX_18_0: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Absolute(iced::Pixels(18.0));
    pub const PX_20_0: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Absolute(iced::Pixels(20.0));
    pub const PX_21_0: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Absolute(iced::Pixels(21.0));
    pub const PX_24_0: iced::widget::text::LineHeight = iced::widget::text::LineHeight::Absolute(iced::Pixels(24.0));
}
