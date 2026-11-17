// Shared top-safe-area background vocals with cached shaped text and a soft timed horizontal sweep.

use std::hash::{Hash, Hasher};
use std::sync::PoisonError;

use folia_core::{BackgroundVocal, Lyrics, Rgb, Theme};
use folia_lyrics::GraphemeTiming;
use folia_store::SubtitleSettings;
use iced::advanced::graphics::text::{cosmic_text as ct, font_system, to_attributes};
use iced::advanced::image::{Image, Renderer as _};
use iced::advanced::{
    Layout, Widget, layout,
    widget::{Tree, tree},
};
use iced::widget::image::Handle;
use iced::{Element, Font, Length, Point, Rectangle, Size, mouse};

pub fn view<'a, M: 'a>(
    lyrics: &'a Lyrics,
    time: f64,
    theme: &'a Theme,
    settings: &'a SubtitleSettings,
    fonts: [Font; 2],
    dpi: f32,
) -> Element<'a, M> {
    Element::new(Harmony { lyrics, time, theme, settings, fonts, dpi })
}

struct Harmony<'a> {
    lyrics: &'a Lyrics,
    time: f64,
    theme: &'a Theme,
    settings: &'a SubtitleSettings,
    fonts: [Font; 2],
    dpi: f32,
}

#[derive(Default)]
struct State {
    key: u64,
    active: Vec<(usize, usize)>,
    lines: Vec<Line>,
    background: Option<Raster>,
    width: f32,
    height: f32,
}

struct Raster {
    handle: Handle,
    size: Size,
}

struct Line {
    main: Raster,
    alternate: Option<Raster>,
    start: f64,
    end: f64,
    timeline: Vec<GraphemeTiming>,
    offsets: Vec<f32>,
    size: f32,
}

impl Line {
    fn height(&self) -> f32 {
        self.main.size.height + self.alternate.as_ref().map_or(0.0, |row| row.size.height + 2.0)
    }

    fn fill(&self, time: f64) -> f32 {
        if time <= self.start {
            return 0.0;
        }
        let full = self.offsets.last().copied().unwrap_or_default();
        if time >= self.end {
            return full;
        }
        for (index, timing) in self.timeline.iter().enumerate() {
            let start = timing.start.max(self.start);
            let end = timing.end.max(start);
            let left = self.offsets.get(index).copied().unwrap_or(full);
            let right = self.offsets.get(index + 1).copied().unwrap_or(left);
            if time < start {
                return left;
            }
            if time <= end {
                return left + (right - left) * ((time - start) / (end - start).max(0.001)) as f32;
            }
        }
        full
    }
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for Harmony<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let bounds = limits.max();
        let state = tree.state.downcast_mut::<State>();
        state.active.clear();
        for (line_index, line) in self.lyrics.lines.iter().enumerate() {
            for (vocal_index, vocal) in line.background_vocals.iter().enumerate() {
                if !vocal.text.trim().is_empty() && self.time >= vocal.start && self.time <= vocal.end + 0.2 {
                    state.active.push((line_index, vocal_index));
                }
            }
        }
        let vocal = |&(line, index): &(usize, usize)| &self.lyrics.lines[line].background_vocals[index];
        state
            .active
            .sort_by(|a, b| vocal(a).start.total_cmp(&vocal(b).start).then(vocal(a).end.total_cmp(&vocal(b).end)));
        if state.active.iter().any(|index| self.time <= vocal(index).end) {
            state.active.retain(|index| self.time <= vocal(index).end);
        } else if let Some(last) = state.active.iter().map(|index| vocal(index).end).max_by(f64::total_cmp) {
            state.active.retain(|index| vocal(index).end == last);
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.theme.hash(&mut hash);
        self.fonts.hash(&mut hash);
        (
            bounds.width.to_bits(),
            self.dpi.to_bits(),
            self.settings.font_scale.to_bits(),
            self.settings.hide_translation,
            self.settings.content_mode as u8,
            self.settings.harmony_background,
        )
            .hash(&mut hash);
        for index in &state.active {
            let vocal = vocal(index);
            (index, vocal.start.to_bits(), vocal.end.to_bits()).hash(&mut hash);
            (&vocal.text, &vocal.translation, &vocal.romanization).hash(&mut hash);
            for word in &vocal.words {
                (&word.text, word.start.to_bits(), word.end.to_bits()).hash(&mut hash);
            }
        }
        let key = hash.finish();
        if state.key != key {
            state.key = key;
            state.lines.clear();
            state.lines.extend(state.active.iter().map(|index| self.compile(vocal(index), bounds.width)));
            state.width = state
                .lines
                .iter()
                .map(|line| line.main.size.width.max(line.alternate.as_ref().map_or(0.0, |r| r.size.width)))
                .fold(0.0_f32, f32::max);
            state.height =
                state.lines.iter().map(Line::height).sum::<f32>() + state.lines.len().saturating_sub(1) as f32 * 6.0;
            state.background = (!state.lines.is_empty() && self.settings.harmony_background)
                .then(|| background(state.width + 112.0, state.height + 64.0, self.theme.background));
        }
        layout::Node::new(bounds)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &iced::advanced::renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        if state.lines.is_empty() {
            return;
        }
        let bounds = layout.bounds();
        let group_start = state.lines.iter().map(|line| line.start).fold(f64::INFINITY, f64::min);
        let group_end = state.lines.iter().map(|line| line.end).fold(0.0_f64, f64::max);
        let entrance =
            folia_visualizer::tween(0.0, 1.0, self.time - group_start, 0.2, folia_visualizer::Ease::OUT) as f32;
        let exit = if self.time > group_end {
            folia_visualizer::tween(1.0, 0.0, self.time - group_end, 0.2, folia_visualizer::Ease::OUT) as f32
        } else {
            1.0
        };
        let opacity = entrance * exit;
        let mut y = bounds.y + 76.0 - 12.0 * (1.0 - opacity) + if self.settings.harmony_background { 8.0 } else { 0.0 };
        if let Some(background) = &state.background {
            let rect =
                Rectangle::new(Point::new(bounds.center_x() - background.size.width * 0.5, y - 104.0), background.size);
            renderer.draw_image(Image::new(background.handle.clone()).opacity(opacity), rect, *viewport);
        }
        for line in &state.lines {
            let age = self.time - line.start;
            let line_alpha = folia_visualizer::tween(0.0, 1.0, age, 0.3, folia_visualizer::Ease::OUT) as f32 * opacity;
            let scale = folia_visualizer::spring(0.97, 1.0, age, 100.0, 10.0) as f32;
            let size = Size::new(line.main.size.width * scale, line.main.size.height * scale);
            let rect = Rectangle::new(Point::new(bounds.center_x() - size.width * 0.5, y), size);
            renderer.draw_image(Image::new(line.main.handle.clone()).opacity(0.56 * line_alpha), rect, *viewport);
            let edge = line.fill(self.time) * scale;
            let softness = (line.size * 1.35).clamp(18.0, 36.0) * scale;
            let start = (edge - softness).max(0.0);
            paint_slice(renderer, &line.main, rect, *viewport, 0.0, start, line_alpha);
            let step = 1.0 / self.dpi.max(1.0);
            let mut x = start;
            while x < edge.min(rect.width) {
                let end = (x + step).min(edge).min(rect.width);
                let distance = edge - (x + end) * 0.5;
                let mask = if distance < softness * 0.5 {
                    distance / (softness * 0.5) * 0.86
                } else {
                    0.86 + (distance / softness - 0.5) * 0.28
                };
                paint_slice(renderer, &line.main, rect, *viewport, x, end, mask * line_alpha);
                x = end;
            }
            if let Some(alternate) = &line.alternate {
                let rect = Rectangle::new(
                    Point::new(bounds.center_x() - alternate.size.width * 0.5, y + line.main.size.height + 2.0),
                    alternate.size,
                );
                renderer.draw_image(Image::new(alternate.handle.clone()).opacity(0.82 * line_alpha), rect, *viewport);
            }
            y += line.height() + 6.0;
        }
    }
}

fn paint_slice(
    renderer: &mut iced::Renderer,
    raster: &Raster,
    bounds: Rectangle,
    viewport: Rectangle,
    left: f32,
    right: f32,
    opacity: f32,
) {
    let clip = Rectangle { x: bounds.x + left, width: (right - left).max(0.0), ..bounds };
    if let Some(clip) = clip.intersection(&viewport) {
        renderer.draw_image(Image::new(raster.handle.clone()).opacity(opacity), bounds, clip);
    }
}

impl Harmony<'_> {
    fn compile(&self, vocal: &BackgroundVocal, width: f32) -> Line {
        let size = (width * 0.018).clamp(15.2, 20.8) * self.settings.font_scale;
        let max_width = (width - if self.settings.harmony_background { 72.0 } else { 40.0 }).clamp(1.0, 896.0);
        let line = folia_core::Line {
            text: vocal.text.clone(),
            words: vocal.words.clone(),
            start: vocal.start,
            end: vocal.end,
            ..Default::default()
        };
        let timeline = folia_lyrics::grapheme_timeline(&line);
        let mut offsets = vec![0.0];
        for range in folia_lyrics::graphemes(&vocal.text) {
            offsets.push(folia_visualizer::measure(&vocal.text[..range.end], size, self.fonts[0], f32::INFINITY).width);
        }
        let mut colors = Vec::new();
        let mut cursor = 0;
        for word in &vocal.words {
            if let Some(at) = vocal.text[cursor..].find(&word.text) {
                let start = cursor + at;
                cursor = start + word.text.len();
                let matched = folia_core::word_color_ranges(&word.text, &self.theme.word_colors);
                let color = matched
                    .iter()
                    .find(|entry| entry.range.start == 0 && entry.range.end == word.text.len())
                    .map_or(self.theme.accent, |entry| entry.color);
                colors.push((start..cursor, color));
            }
        }
        let main = raster(&vocal.text, size, max_width, self.fonts[0], self.dpi, self.theme.secondary, &colors);
        let alternate = (!self.settings.hide_translation)
            .then(|| vocal.alternate(self.settings.content_mode))
            .flatten()
            .map(|text| {
                raster(
                    text,
                    (width * 0.012).clamp(11.52, 14.4) * self.settings.font_scale,
                    max_width,
                    self.fonts[1],
                    self.dpi,
                    self.theme.secondary,
                    &[],
                )
            });
        Line { main, alternate, start: vocal.start, end: vocal.end, timeline, offsets, size }
    }
}

fn raster(
    text: &str,
    size: f32,
    width: f32,
    font: Font,
    dpi: f32,
    default: Rgb,
    colors: &[(std::ops::Range<usize>, Rgb)],
) -> Raster {
    let dpi = dpi.clamp(0.5, 4.0);
    let mut shared = font_system().write().unwrap_or_else(PoisonError::into_inner);
    let system = shared.raw();
    let mut buffer = ct::Buffer::new(system, ct::Metrics::new(size * dpi, size * dpi * 1.3));
    buffer.set_size(system, Some(width * dpi), None);
    buffer.set_wrap(system, ct::Wrap::WordOrGlyph);
    buffer.set_text(system, text, &to_attributes(font), ct::Shaping::Advanced, None);
    buffer.shape_until_scroll(system, false);
    let width = buffer.layout_runs().map(|run| run.line_w).fold(1.0_f32, f32::max).ceil() as u32;
    let height =
        buffer.layout_runs().map(|run| run.line_top + run.line_height).fold(size * dpi * 1.3, f32::max).ceil() as u32;
    let mut pixels = image::RgbaImage::new(width, height);
    let mut swash = ct::SwashCache::new();
    for run in buffer.layout_runs() {
        let offset = text.split_inclusive('\n').take(run.line_i).map(str::len).sum::<usize>();
        for glyph in run.glyphs {
            let color = colors
                .iter()
                .find(|(range, _)| range.contains(&(offset + glyph.start)))
                .map_or(default, |(_, color)| *color);
            let physical = glyph.physical(((width as f32 - run.line_w) * 0.5, run.line_y), 1.0);
            let Some(image) = swash.get_image_uncached(system, physical.cache_key) else {
                continue;
            };
            for y in 0..image.placement.height {
                for x in 0..image.placement.width {
                    let dx = physical.x + image.placement.left + x as i32;
                    let dy = physical.y - image.placement.top + y as i32;
                    if dx < 0 || dy < 0 || dx >= width as i32 || dy >= height as i32 {
                        continue;
                    }
                    let at = (y * image.placement.width + x) as usize;
                    let alpha = match image.content {
                        ct::SwashContent::Mask => image.data.get(at),
                        ct::SwashContent::Color => image.data.get(at * 4 + 3),
                        ct::SwashContent::SubpixelMask => image.data.get(at * 4 + 1),
                    }
                    .copied()
                    .unwrap_or(0);
                    let pixel = pixels.get_pixel_mut(dx as u32, dy as u32);
                    if alpha > pixel[3] {
                        *pixel = image::Rgba([color.r, color.g, color.b, alpha]);
                    }
                }
            }
        }
    }
    Raster {
        handle: Handle::from_rgba(width, height, pixels.into_raw()),
        size: Size::new(width as f32 / dpi, height as f32 / dpi),
    }
}

fn background(width: f32, height: f32, color: Rgb) -> Raster {
    let padding = 72;
    let (width, height) = (width.ceil() as u32, height.ceil() as u32);
    let alpha = image::GrayImage::from_fn(width + padding * 2, height + padding * 2, |x, y| {
        let dx = (x as f32 - padding as f32 - width as f32 * 0.5) / (width as f32 * 0.5);
        let dy = (y as f32 - padding as f32 - height as f32 * 0.5) / (height as f32 * 0.5);
        let radius = (dx * dx + dy * dy).sqrt();
        let alpha = if radius < 0.48 {
            0.7 + (0.42 - 0.7) * radius / 0.48
        } else {
            0.42 * (1.0 - (radius - 0.48) / 0.3).clamp(0.0, 1.0)
        };
        image::Luma([(alpha * 255.0).round() as u8])
    });
    let alpha = image::imageops::blur(&alpha, 24.0);
    let pixels = image::RgbaImage::from_fn(alpha.width(), alpha.height(), |x, y| {
        image::Rgba([color.r, color.g, color.b, alpha.get_pixel(x, y)[0]])
    });
    Raster {
        size: Size::new(pixels.width() as f32, pixels.height() as f32),
        handle: Handle::from_rgba(pixels.width(), pixels.height(), pixels.into_raw()),
    }
}
