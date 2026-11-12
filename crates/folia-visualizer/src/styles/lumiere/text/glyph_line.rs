// One lyric line cut into graphemes, each placed at its prefix width plus letter spacing (glyphLine.ts).

use std::ops::Range;

use folia_render::glyphs::{ShapedGlyph, shape};
use iced::Font;

/// The lyric font: the theme stack at weight 500 (`resolveThemeFontWeight(theme, 500)`).
pub const LYRIC_FONT: Font = Font { weight: iced::font::Weight::Medium, ..Font::DEFAULT };

pub struct Grapheme {
    /// Byte range in the line text.
    pub text: Range<usize>,
    /// Left edge and width of the character in the line (prefix widths, spacing included).
    pub char_x: f32,
    pub char_width: f32,
    /// Its glyphs in `GlyphLine::glyphs`, each with `pen` from the character center.
    pub glyphs: Range<usize>,
    pub blank: bool,
    /// Upright in vertical text (CJK and full-width forms); other characters turn 90°.
    pub upright: bool,
}

pub struct Placed {
    pub glyph: ShapedGlyph,
    pub pen: [f32; 2],
}

pub struct GlyphLine {
    pub graphemes: Vec<Grapheme>,
    pub glyphs: Vec<Placed>,
    pub font_px: f32,
}

/// Shapes `text` once and splits it at grapheme boundaries. `spacing` is letter spacing as a fraction of `font_px`.
pub fn build(text: &str, font_px: f32, spacing: f32, font: Font) -> GlyphLine {
    let shaped = shape(text, font_px, font);
    let ranges = folia_lyrics::graphemes(text);
    let prefix =
        |byte: usize| shaped.glyphs.iter().find(|glyph| glyph.cluster >= byte).map_or(shaped.width, |glyph| glyph.x);
    let gap = spacing * font_px;
    let mut offsets = Vec::with_capacity(ranges.len() + 1);
    offsets.push(0.0);
    for (index, range) in ranges.iter().enumerate() {
        let end = ranges.get(index + 1).map_or(shaped.width, |next| prefix(next.start));
        offsets.push(end.max(prefix(range.start)) + gap * (index + 1) as f32);
    }
    let mut glyphs = Vec::with_capacity(shaped.glyphs.len());
    let mut graphemes = Vec::with_capacity(ranges.len());
    for (index, range) in ranges.iter().enumerate() {
        let char_x = offsets[index];
        let char_width = (offsets[index + 1] - char_x).max(0.5);
        let origin = prefix(range.start);
        let first = glyphs.len();
        for glyph in shaped.glyphs.iter().filter(|glyph| range.contains(&glyph.cluster)) {
            glyphs
                .push(Placed { glyph: *glyph, pen: [glyph.x - origin - char_width * 0.5, shaped.baseline + glyph.y] });
        }
        let piece = &text[range.clone()];
        graphemes.push(Grapheme {
            text: range.clone(),
            char_x,
            char_width,
            glyphs: first..glyphs.len(),
            blank: piece.trim().is_empty(),
            upright: piece.chars().next().is_some_and(upright),
        });
    }
    GlyphLine { graphemes, glyphs, font_px }
}

/// `UPRIGHT` in glyphLine.ts.
fn upright(c: char) -> bool {
    matches!(c,
        '\u{1100}'..='\u{11FF}'
        | '\u{2E80}'..='\u{2FFF}'
        | '\u{3000}'..='\u{303F}'
        | '\u{3040}'..='\u{30FF}'
        | '\u{3100}'..='\u{31FF}'
        | '\u{3200}'..='\u{9FFF}'
        | '\u{AC00}'..='\u{D7AF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{FE30}'..='\u{FE4F}'
        | '\u{FF00}'..='\u{FFEF}'
        | '\u{20000}'..='\u{3134F}')
}
