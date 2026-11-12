// Cached subtitle glyph masks at window DPI, with the original one-pixel blur and two-pixel shadow.

use iced::advanced::graphics::text::{cosmic_text as ct, font_system, to_attributes};
use iced::advanced::image::Renderer as _;
use iced::advanced::{Layout, Widget, layout, widget::Tree};
use iced::widget::image::Handle;
use iced::{Color, Element, Font, Length, Rectangle, Size, mouse};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub font: Font,
    pub size: f32,
    pub width: f32,
    pub dpi: f32,
    pub color: Color,
    pub shadow: Color,
    pub blur: bool,
    pub single: bool,
}

struct Raster {
    handle: Handle,
    width: f32,
    height: f32,
    padding: f32,
    bytes: usize,
}
struct Entry {
    text: String,
    look: Look,
    raster: Arc<Raster>,
}

pub fn view<'a, M: 'a>(text: &str, look: Look) -> Element<'a, M> {
    static CACHE: OnceLock<Mutex<VecDeque<Entry>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    let mut entries = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let raster = if let Some(index) = entries.iter().position(|entry| entry.text == text && entry.look == look) {
        let entry = entries.remove(index);
        if let Some(entry) = entry {
            let raster = entry.raster.clone();
            entries.push_back(entry);
            raster
        } else {
            Arc::new(rasterize(text, look))
        }
    } else {
        let raster = Arc::new(rasterize(text, look));
        entries.push_back(Entry { text: text.to_owned(), look, raster: raster.clone() });
        while entries.len() > 8 || entries.iter().map(|entry| entry.raster.bytes).sum::<usize>() > 16 * 1024 * 1024 {
            entries.pop_front();
        }
        raster
    };
    Element::new(RasterWidget(raster))
}

struct RasterWidget(Arc<Raster>);
impl<M> Widget<M, iced::Theme, iced::Renderer> for RasterWidget {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }
    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::Node::new(limits.resolve(
            Length::Shrink,
            Length::Shrink,
            Size::new(self.0.width - self.0.padding * 2.0, self.0.height - self.0.padding * 2.0),
        ))
    }
    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &iced::advanced::renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        renderer.draw_image(
            iced::advanced::image::Image::new(self.0.handle.clone()),
            Rectangle {
                x: bounds.x - self.0.padding,
                y: bounds.y - self.0.padding,
                width: self.0.width,
                height: self.0.height,
            },
            *viewport,
        );
    }
}

fn rasterize(text: &str, look: Look) -> Raster {
    let scale = look.dpi.clamp(0.5, 4.0);
    let size = look.size.max(1.0) * scale;
    let line_height = size * 1.5;
    let padding = (8.0 * scale).ceil() as u32;
    let text = if look.single {
        folia_visualizer::clamp_lines(text, look.size, look.font, look.width, 1)
    } else {
        text.to_owned()
    };
    let mut guard = font_system().write().unwrap_or_else(std::sync::PoisonError::into_inner);
    let fonts = guard.raw();
    let mut buffer = ct::Buffer::new(fonts, ct::Metrics::new(size, line_height));
    buffer.set_size(fonts, Some(look.width.max(1.0) * scale), None);
    buffer.set_text(fonts, &text, &to_attributes(look.font), ct::Shaping::Advanced, None);
    buffer.shape_until_scroll(fonts, false);
    let width = buffer.layout_runs().map(|run| run.line_w).fold(0.0_f32, f32::max).ceil() as u32 + padding * 2;
    let height = buffer.layout_runs().map(|run| run.line_top + run.line_height).fold(line_height, f32::max).ceil()
        as u32
        + padding * 2;
    let mut mask = image::GrayImage::new(width, height);
    let mut swash = ct::SwashCache::new();
    for run in buffer.layout_runs() {
        let left = (width as f32 - run.line_w) * 0.5;
        for glyph in run.glyphs {
            let physical = glyph.physical((left, run.line_y + padding as f32), 1.0);
            let Some(image) = swash.get_image_uncached(fonts, physical.cache_key) else { continue };
            for y in 0..image.placement.height {
                for x in 0..image.placement.width {
                    let dx = physical.x + image.placement.left + x as i32;
                    let dy = physical.y - image.placement.top + y as i32;
                    if dx < 0 || dy < 0 || dx >= width as i32 || dy >= height as i32 {
                        continue;
                    }
                    let at = (y * image.placement.width + x) as usize;
                    let value = match image.content {
                        ct::SwashContent::Mask => image.data.get(at),
                        ct::SwashContent::Color => image.data.get(at * 4 + 3),
                        ct::SwashContent::SubpixelMask => image.data.get(at * 4 + 1),
                    }
                    .copied()
                    .unwrap_or(0);
                    let pixel = mask.get_pixel_mut(dx as u32, dy as u32);
                    pixel[0] = pixel[0].max(value);
                }
            }
        }
    }
    drop(guard);
    let shadow = image::imageops::blur(&mask, scale * if look.blur { 5.0_f32.sqrt() } else { 2.0 });
    let foreground = if look.blur { image::imageops::blur(&mask, scale) } else { mask };
    let offset = (scale * crate::overlay::SHADOW_OFFSET_Y).round() as u32;
    let pixels = image::RgbaImage::from_fn(width, height, |x, y| {
        let coverage = f32::from(foreground.get_pixel(x, y)[0]) / 255.0 * look.color.a;
        let shadow_alpha =
            if y >= offset { f32::from(shadow.get_pixel(x, y - offset)[0]) / 255.0 * look.shadow.a } else { 0.0 };
        let alpha = coverage + shadow_alpha * (1.0 - coverage);
        let color = [look.color.r, look.color.g, look.color.b];
        let behind = [look.shadow.r, look.shadow.g, look.shadow.b];
        let channel = |i: usize| {
            (((color[i] * coverage + behind[i] * shadow_alpha * (1.0 - coverage)) / alpha.max(0.00001)) * 255.0).round()
                as u8
        };
        image::Rgba([channel(0), channel(1), channel(2), (alpha * 255.0).round() as u8])
    });
    Raster {
        handle: Handle::from_rgba(width, height, pixels.into_raw()),
        width: width as f32 / scale,
        height: height as f32 / scale,
        padding: padding as f32 / scale,
        bytes: width as usize * height as usize * 4,
    }
}
