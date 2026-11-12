// Monet's poster boxes, worked out from the original's CSS (`VisualizerMonet.tsx` 166-560, Tailwind 4 spacing).

use iced::{Rectangle, Size};

use super::motion::{clamp_font, large_screen};
use super::text;
use super::tuning;
use crate::Scene;

const REM: f32 = 16.0;
pub const SEMIBOLD: iced::Font = iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT };
pub const ITALIC: iced::Font = iced::Font { style: iced::font::Style::Italic, ..iced::Font::DEFAULT };

/// Every box of the poster in stage pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub lyric_px: f32,
    pub inactive_px: f32,
    pub translation_px: f32,
    pub artist_px: f32,
    pub title_px: f32,
    /// `glowBufferPx`: the rail's side padding and the line boxes' bleed.
    pub glow: f32,
    /// `headerMaxWidth`: artist and title stop here so they stay clear of the portrait.
    pub header_width: f32,
    /// The artist's line box, one line of `line-height: 1.5`.
    pub artist: Rectangle,
    /// The 1px rule under the artist, before its `scaleY` intro.
    pub rule: Rectangle,
    /// The title's line boxes, at most two of `leading-[1.06]`.
    pub title: Rectangle,
    pub title_rows: usize,
    pub album: Rectangle,
    /// Artist top to album bottom, `header_width` wide: the heading as a whole.
    pub heading: Rectangle,
    /// The rail's padding box, glow padding included; its line boxes start `glow` in from its left.
    pub rail: Rectangle,
    /// The description capsule's border box, when shown.
    pub capsule: Option<Rectangle>,
    /// The capsule's motion box, the origin of its `scale` intro.
    pub description: Rectangle,
    /// The right column's box, the origin of the portrait intro. `None` below 768px, where it is hidden.
    pub column: Option<Rectangle>,
    /// The portrait's bounding wrapper (`maxWidth: clamp(210px, 26vw, …)`) at rest.
    pub wrapper: Option<Rectangle>,
    /// The portrait frame with the drag offset applied.
    pub portrait: Option<Rectangle>,
    pub hanger: Option<Rectangle>,
    /// The spectrum strip's content box.
    pub audio: Rectangle,
}

/// Tailwind's min-width breakpoints: `sm` 640, `lg` 1024, `2xl` 1536.
fn by_width(width: f32, base: f32, sm: f32, lg: f32, xxl: f32) -> f32 {
    if width >= 1536.0 {
        xxl
    } else if width >= 1024.0 {
        lg
    } else if width >= 640.0 {
        sm
    } else {
        base
    }
}

/// `n.toFixed(3)` read back as a number.
fn fixed3(value: f32) -> f32 {
    (value * 1000.0).round() / 1000.0
}

/// The poster at stage `size`, with the portrait moved by `offset` (the live drag, or the saved value).
pub fn layout(scene: &Scene<'_>, size: Size, offset: f32) -> Layout {
    let manifest = tuning::manifest();
    let values = &scene.tuning;
    let font_scale = values.number(manifest, "fontScale");
    let square = values.choice(manifest, "portraitStyle") == "square";
    let saved = tuning::offset(values);
    let (w, h) = (size.width, size.height);
    let large = large_screen(w);
    let lyric_px = clamp_font(1.34, 2.75, 2.28, w, font_scale * large);
    let inactive_px = clamp_font(1.08, 2.0, 1.48, w, font_scale * large);
    let translation_px = clamp_font(0.94, 1.28, 1.14, w, font_scale * scene.font_scale * large);
    let row_width = w.min((1520.0 * large).round());
    let portrait_max = (430.0 * large).round();
    let inner_max = (380.0 * large).round();
    let title_px = clamp_font(1.45, 3.3, fixed3(2.8 * large), w, 1.0);
    let artist_px = clamp_font(1.0, 1.8, fixed3(1.8 * large), w, 1.0);
    let row_x = (w - row_width) / 2.0;
    let shows_portrait = w >= 768.0;
    let column_width = if shows_portrait { (0.28 * w).clamp(220.0, portrait_max.max(220.0)) } else { 0.0 };
    let pad_x = by_width(w, 20.0, 32.0, 56.0, 80.0);
    let pad_y = by_width(w, 20.0, 24.0, 32.0, 32.0);
    let content_x = row_x + pad_x;
    let content_width = (row_width - column_width - pad_x * 2.0).max(0.0);
    let inner_width = (0.26 * w).clamp(210.0, inner_max.max(210.0));
    let bleed = if square { (0.35135 * inner_width - 3.0 * REM).max(0.0) } else { 0.0 };
    let header_width = (content_width - bleed - saved.min(0.0).abs()).max(12.0 * REM).min(content_width);

    let artist_box = artist_px * 1.5;
    let title_text = if scene.title.is_empty() { "Monet" } else { scene.title.as_str() };
    let title_rows = text::wrap(title_text, title_px, scene.typography.font(SEMIBOLD), header_width).len().clamp(1, 2);
    let title_line = title_px * 1.06;
    // Padding-block 0.25em with margins -0.25em / -0.12em leaves 0.13em below the line boxes.
    let title_block = title_rows as f32 * title_line + 0.13 * title_px;
    let album_line = 20.0;
    let glow = (lyric_px * 1.2).round();
    let rail_max_height = (520.0 * large).max(lyric_px * 1.18 * 7.0).round();
    let rail_height = (0.52 * h).clamp(280.0, rail_max_height.max(280.0));
    let rail_width = (780.0 * large).round().min(content_width + glow * 2.0);
    let describe = values.flag(manifest, "showDescription");
    let description_height = if describe { 16.0 + 34.0 } else { 0.0 };
    let total = pad_y * 2.0
        + artist_box
        + 6.0
        + 56.0
        + 12.0
        + title_block
        + album_line
        + 24.0
        + rail_height
        + description_height;
    let top = (h - total) / 2.0 + pad_y;

    let artist = Rectangle { x: content_x, y: top, width: header_width, height: artist_box };
    let rule = Rectangle { x: content_x, y: artist.y + artist_box + 6.0, width: 1.0, height: 56.0 };
    let title = Rectangle {
        x: content_x,
        y: rule.y + 56.0 + 12.0,
        width: header_width,
        height: title_rows as f32 * title_line,
    };
    let album = Rectangle { x: content_x, y: title.y + title_block, width: header_width, height: album_line };
    let heading = Rectangle { x: content_x, y: artist.y, width: header_width, height: album.y + album_line - artist.y };
    let rail =
        Rectangle { x: content_x - glow, y: album.y + album_line + 24.0, width: rail_width, height: rail_height };
    let description = Rectangle { x: content_x, y: rail.y + rail_height, width: content_width, height: 50.0 };
    let capsule = describe.then(|| {
        let label = capsule_label(scene);
        let label_width = text::shape(&label, 12.0, iced::Font::DEFAULT).width;
        Rectangle { x: content_x, y: description.y + 16.0, width: 2.0 + 32.0 + 10.0 + 12.0 + label_width, height: 34.0 }
    });

    let column = shows_portrait.then_some(Rectangle {
        x: row_x + row_width - column_width,
        y: 0.0,
        width: column_width,
        height: h,
    });
    let wrapper = column.map(|column| {
        let (pad_left, content) = column_box(w, column.width);
        let width = inner_width.min(content);
        let x = if w >= 1024.0 {
            column.x + pad_left + content - width
        } else {
            column.x + pad_left + (content - width) / 2.0
        };
        // The square is 135.135% of the wrapper and `aspect-square`; the rectangle is `aspect-[0.74]`.
        let height = if square { width * 1.35135 } else { width / 0.74 };
        Rectangle { x, y: (h - height) / 2.0, width, height }
    });
    let column =
        column.zip(wrapper).map(|(column, wrapper)| Rectangle { y: wrapper.y, height: wrapper.height, ..column });
    let portrait = wrapper.map(|wrapper| {
        let width = if square { wrapper.width * 1.35135 } else { wrapper.width };
        let x = if square { wrapper.x - wrapper.width * 0.35135 } else { wrapper.x };
        Rectangle { x: x + offset, y: wrapper.y, width, height: wrapper.height }
    });
    let hanger = portrait.map(|frame| Rectangle {
        x: frame.x + frame.width - 32.0 - 12.0,
        y: frame.y - 12.0,
        width: 12.0,
        height: 56.0,
    });
    let audio_pad = by_width(w, 20.0, 32.0, 56.0, 56.0);
    let audio_width = (0.55 * w).min(450.0);
    let audio = Rectangle { x: audio_pad, y: h - 40.0, width: (audio_width - audio_pad * 2.0).max(0.0), height: 40.0 };
    Layout {
        lyric_px,
        inactive_px,
        translation_px,
        artist_px,
        title_px,
        glow,
        header_width,
        artist,
        rule,
        title,
        title_rows,
        album,
        heading,
        rail,
        capsule,
        description,
        column,
        wrapper,
        portrait,
        hanger,
        audio,
    }
}

/// The right column's left padding and content width: `px-3 pr-5 sm:pr-8 lg:pr-10 xl:pr-12`.
fn column_box(width: f32, column: f32) -> (f32, f32) {
    let right = if width >= 1280.0 { 48.0 } else { by_width(width, 20.0, 32.0, 40.0, 48.0) };
    (12.0, (column - 12.0 - right).max(0.0))
}

/// `theme.description || theme.name || 'Monet'`, upper-cased like the capsule's `uppercase`.
pub fn capsule_label(scene: &Scene<'_>) -> String {
    let name =
        scene.theme.description.as_deref().filter(|text| !text.trim().is_empty()).unwrap_or(&scene.theme.name).trim();
    if name.is_empty() { "MONET".to_owned() } else { name.to_uppercase() }
}
