// Monet's text through iced's shared font system: shaping with glyph positions, wrapping, and an italic line slanted
// the way a browser synthesizes oblique for faces without an italic.

use std::collections::HashSet;
use std::sync::OnceLock;

use iced::advanced::graphics::text::cosmic_text::{self as ct, fontdb};
use iced::advanced::graphics::text::{font_system, to_attributes};
use iced::widget::image::Handle;
use iced::{Color, Font};

/// One shaped glyph: its byte range in the text and its pen position from the start of the line.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    pub start: usize,
    pub end: usize,
    pub x: f32,
    pub width: f32,
    font: fontdb::ID,
    id: u16,
    weight: fontdb::Weight,
}

/// A line shaped without wrapping.
#[derive(Debug, Clone, Default)]
pub struct Shaped {
    pub glyphs: Vec<Glyph>,
    pub width: f32,
    /// Glyphs that no installed face could draw (`.notdef`, glyph id 0).
    pub missing: usize,
    /// Baseline below the middle of any line box: `(ascent - descent) / 2` of the faces used, in pixels.
    pub baseline: f32,
    px: f32,
}

impl Shaped {
    /// Pen position of byte `at`: the start of the first glyph at or after it, or the line's end.
    pub fn x_at(&self, at: usize) -> f32 {
        self.glyphs.iter().find(|glyph| glyph.start >= at).map_or(self.width, |glyph| glyph.x)
    }

    /// Advance of the bytes `start..end`.
    pub fn span(&self, start: usize, end: usize) -> f32 {
        self.glyphs.iter().filter(|glyph| glyph.start >= start && glyph.start < end).map(|glyph| glyph.width).sum()
    }
}

/// A wrapped row: the bytes it shows and its width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Row {
    pub start: usize,
    pub end: usize,
    pub width: f32,
}

/// `text` in `font` at `px`, on one line. Italic text first gets the synthetic faces from `slanted_faces`.
pub fn shape(text: &str, px: f32, font: Font) -> Shaped {
    if font.style != iced::font::Style::Normal {
        slanted_faces();
    }
    let Ok(mut guard) = font_system().write() else { return Shaped::default() };
    let system = guard.raw();
    let mut buffer = ct::Buffer::new(system, ct::Metrics::new(px.max(1.0), px.max(1.0) * 1.2));
    buffer.set_size(system, None, None);
    buffer.set_text(system, text, &to_attributes(font), ct::Shaping::Advanced, None);
    buffer.shape_until_scroll(system, false);
    let mut shaped = Shaped { px, ..Shaped::default() };
    let starts = line_starts(text);
    for run in buffer.layout_runs() {
        let base = starts.get(run.line_i).copied().unwrap_or(0);
        for glyph in run.glyphs {
            shaped.missing += usize::from(glyph.glyph_id == 0);
            shaped.glyphs.push(Glyph {
                start: base + glyph.start,
                end: base + glyph.end,
                x: glyph.x,
                width: glyph.w,
                font: glyph.font_id,
                id: glyph.glyph_id,
                weight: glyph.font_weight,
            });
        }
        if run.line_i == 0 {
            shaped.baseline = run.line_y - run.line_top - run.line_height / 2.0;
        }
        shaped.width = shaped.width.max(run.line_w);
    }
    shaped
}

/// Each glyph's ink from its outline, as `(top, bottom)` below the baseline (negative is above).
pub fn ink(shaped: &Shaped) -> Vec<(f32, f32)> {
    let Ok(mut guard) = font_system().write() else { return vec![(0.0, 0.0); shaped.glyphs.len()] };
    let system = guard.raw();
    let mut swash = ct::SwashCache::new();
    shaped
        .glyphs
        .iter()
        .map(|glyph| {
            let (key, _, _) = ct::CacheKey::new(
                glyph.font,
                glyph.id,
                shaped.px,
                (0.0, 0.0),
                glyph.weight,
                ct::CacheKeyFlags::empty(),
            );
            let Some(commands) = swash.get_outline_commands_uncached(system, key) else { return (0.0, 0.0) };
            let mut high = f32::NEG_INFINITY;
            let mut low = f32::INFINITY;
            for command in commands.iter() {
                let ys: [Option<f32>; 3] = match command {
                    ct::Command::MoveTo(a) | ct::Command::LineTo(a) => [Some(a.y), None, None],
                    ct::Command::QuadTo(a, b) => [Some(a.y), Some(b.y), None],
                    ct::Command::CurveTo(a, b, c) => [Some(a.y), Some(b.y), Some(c.y)],
                    ct::Command::Close => [None; 3],
                };
                for y in ys.into_iter().flatten() {
                    high = high.max(y);
                    low = low.min(y);
                }
            }
            if high < low { (0.0, 0.0) } else { (-high, -low) }
        })
        .collect()
}

/// `text` wrapped at word boundaries to `width`, breaking inside a word only when it alone is wider.
pub fn wrap(text: &str, px: f32, font: Font, width: f32) -> Vec<Row> {
    let Ok(mut guard) = font_system().write() else { return Vec::new() };
    let system = guard.raw();
    let mut buffer = ct::Buffer::new(system, ct::Metrics::new(px.max(1.0), px.max(1.0) * 1.2));
    buffer.set_wrap(system, ct::Wrap::WordOrGlyph);
    buffer.set_size(system, Some(width.max(1.0)), None);
    buffer.set_text(system, text, &to_attributes(font), ct::Shaping::Advanced, None);
    buffer.shape_until_scroll(system, false);
    let starts = line_starts(text);
    buffer
        .layout_runs()
        .map(|run| {
            let base = starts.get(run.line_i).copied().unwrap_or(0);
            let start = run.glyphs.iter().map(|glyph| glyph.start).min().map_or(base, |at| base + at);
            let end = run.glyphs.iter().map(|glyph| glyph.end).max().map_or(base, |at| base + at);
            Row { start, end, width: run.line_w }
        })
        .collect()
}

fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(at, _)| at + 1)).collect()
}

/// A line rasterized off the canvas: straight-alpha RGBA at `scale` device pixels per stage pixel.
#[derive(Debug, Clone)]
pub struct Raster {
    pub handle: Handle,
    /// Size in stage pixels.
    pub width: f32,
    pub height: f32,
    /// Distance from the image's left edge to the pen origin, in stage pixels.
    pub left: f32,
    pub missing: usize,
}

/// `text` in italic `font` at `px` with a `line_height` line box. Glyphs from a family without an italic come
/// from the synthetic faces and are slanted like a browser's faux italic.
pub fn raster_italic(text: &str, px: f32, line_height: f32, font: Font, color: Color, scale: f32) -> Option<Raster> {
    let synthetic = slanted_faces();
    let Ok(mut guard) = font_system().write() else { return None };
    let system = guard.raw();
    let font = Font { style: iced::font::Style::Italic, ..font };
    let metrics = ct::Metrics::new(px * scale, line_height * scale);
    let mut buffer = ct::Buffer::new(system, metrics);
    buffer.set_size(system, None, None);
    buffer.set_text(system, text, &to_attributes(font), ct::Shaping::Advanced, None);
    buffer.shape_until_scroll(system, false);
    let pad = (px * 0.4 * scale).ceil();
    let width = buffer.layout_runs().map(|run| run.line_w).fold(0.0_f32, f32::max);
    let (w, h) = ((width + pad * 2.0).ceil().max(1.0) as u32, (line_height * scale).ceil().max(1.0) as u32);
    let mut alpha = vec![0u8; w as usize * h as usize];
    let mut missing = 0;
    let mut swash = ct::SwashCache::new();
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            missing += usize::from(glyph.glyph_id == 0);
            let mut physical = glyph.physical((pad, run.line_y), 1.0);
            if synthetic.contains(&glyph.font_id) {
                physical.cache_key.flags |= ct::CacheKeyFlags::FAKE_ITALIC;
            }
            let Some(image) = swash.get_image_uncached(system, physical.cache_key) else { continue };
            blit(&mut alpha, w, h, physical.x, physical.y, &image);
        }
    }
    let [r, g, b, a] = color.into_rgba8();
    let pixels: Vec<u8> =
        alpha.iter().flat_map(|coverage| [r, g, b, (u16::from(*coverage) * u16::from(a) / 255) as u8]).collect();
    Some(Raster {
        handle: Handle::from_rgba(w, h, pixels),
        width: w as f32 / scale,
        height: h as f32 / scale,
        left: pad / scale,
        missing,
    })
}

fn blit(alpha: &mut [u8], w: u32, h: u32, x: i32, y: i32, image: &ct::SwashImage) {
    let place = image.placement;
    for row in 0..place.height as i32 {
        for col in 0..place.width as i32 {
            let (dx, dy) = (x + place.left + col, y - place.top + row);
            if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                continue;
            }
            let src = (row * place.width as i32 + col) as usize;
            let coverage = match image.content {
                ct::SwashContent::Mask => image.data.get(src).copied(),
                ct::SwashContent::Color => image.data.get(src * 4 + 3).copied(),
                ct::SwashContent::SubpixelMask => image.data.get(src * 4 + 1).copied(),
            };
            let slot = &mut alpha[(dy as u32 * w + dx as u32) as usize];
            *slot = (*slot).max(coverage.unwrap_or(0));
        }
    }
}

/// Italic aliases for every family without an italic face, registered once. cosmic-text only falls back to faces
/// of the requested style, so italic Chinese had no face at all; a browser slants the upright face instead.
fn slanted_faces() -> &'static HashSet<fontdb::ID> {
    static SLANTED: OnceLock<HashSet<fontdb::ID>> = OnceLock::new();
    SLANTED.get_or_init(|| {
        let Ok(mut guard) = font_system().write() else { return HashSet::new() };
        let system = guard.raw();
        let family = |face: &fontdb::FaceInfo| face.families.first().map(|(name, _)| name.clone()).unwrap_or_default();
        let italic: HashSet<String> =
            system.db().faces().filter(|face| face.style != fontdb::Style::Normal).map(family).collect();
        let upright: Vec<fontdb::FaceInfo> = system
            .db()
            .faces()
            .filter(|face| face.style == fontdb::Style::Normal && !italic.contains(&family(face)))
            .cloned()
            .collect();
        let ids: HashSet<fontdb::ID> = upright
            .into_iter()
            .map(|mut alias| {
                alias.id = fontdb::ID::dummy();
                alias.style = fontdb::Style::Italic;
                system.db_mut().push_face_info(alias)
            })
            .collect();
        tracing::debug!(faces = ids.len(), "monet italic fallback faces");
        ids
    })
}
