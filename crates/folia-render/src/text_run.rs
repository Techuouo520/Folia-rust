// CSS tracked and vertical runs, shaped with OpenType vertical metrics and rendered at physical resolution.

use crate::glyphs::{Glyphs, Mark, ShapedGlyph, Sprite};
use crate::pipeline::Slot;
use iced::advanced::graphics::text::{Raw, Renderer as _, cosmic_text as cosmic, font_system, to_attributes};
use iced::advanced::{
    Layout, Widget, layout, renderer,
    widget::{Tree, tree},
};
use iced::widget::shader;
use iced::{Color, Font, Rectangle, Size};
use iced::{Element, Length, Point, Theme, mouse};
use iced_wgpu::primitive::Renderer as _;
use std::sync::{Arc, PoisonError};

/// A shaped text widget with CSS tracking and vertical flow, sharing cluster boxes with its parent selector.
pub fn text<'a, Message: 'a>(content: &str, style: Style, color: Color) -> Element<'a, Message> {
    Element::new(TextRun { content: content.to_owned(), style, color })
}

struct TextRun {
    content: String,
    style: Style,
    color: Color,
}

#[derive(Default)]
struct State {
    content: String,
    style: Option<Style>,
    run: Arc<Run>,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for TextRun {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }
    fn layout(&mut self, tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let state = tree.state.downcast_mut::<State>();
        if state.content != self.content || state.style != Some(self.style) {
            state.content.clone_from(&self.content);
            state.style = Some(self.style);
            state.run = Arc::new(Run::new(&self.content, self.style));
        }
        let size = limits.resolve(Length::Shrink, Length::Shrink, state.run.size);
        let children =
            state.run.clusters.iter().map(|r| layout::Node::new(r.size()).move_to(Point::new(r.x, r.y))).collect();
        layout::Node::with_children(size, children)
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let run = tree.state.downcast_ref::<State>().run.clone();
        if let Some(buffer) = &run.buffer {
            renderer.fill_raw(Raw {
                buffer: Arc::downgrade(buffer),
                position: layout.position(),
                color: self.color,
                clip_bounds: *viewport,
            });
            return;
        }
        renderer.draw_primitive(layout.bounds(), Pass::new(run, self.color, layout.bounds().size()));
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub font: Font,
    pub size: f32,
    pub line_height: f32,
    /// Letter spacing in logical pixels, including the final cluster's trailing space.
    pub tracking: f32,
    pub vertical: bool,
    pub upright: bool,
    pub width: Option<f32>,
}

#[derive(Debug, Clone)]
struct Glyph {
    mark: ShapedGlyph,
    pen: [f32; 2],
    axes: [f32; 4],
}

/// Natural line box and cluster boxes, shared by painting and selection hit testing.
#[derive(Debug, Default)]
pub struct Run {
    pub size: Size,
    pub clusters: Vec<Rectangle>,
    glyphs: Vec<Glyph>,
    font_size: f32,
    buffer: Option<Arc<cosmic::Buffer>>,
}

impl Run {
    pub fn new(text: &str, style: Style) -> Self {
        let mut shared = font_system().write().unwrap_or_else(PoisonError::into_inner);
        let system = shared.raw();
        let mut buffer = cosmic::Buffer::new(system, cosmic::Metrics::new(style.size, style.line_height));
        buffer.set_size(system, style.width, None);
        buffer.set_wrap(system, cosmic::Wrap::WordOrGlyph);
        let mut features = cosmic::FontFeatures::new();
        if style.tracking != 0.0 {
            features.disable(cosmic::FeatureTag::STANDARD_LIGATURES);
            features.disable(cosmic::FeatureTag::CONTEXTUAL_LIGATURES);
        }
        let attrs = to_attributes(style.font)
            .letter_spacing(if style.vertical { 0.0 } else { style.tracking / style.size })
            .font_features(features);
        buffer.set_text(system, text, &attrs, cosmic::Shaping::Advanced, None);
        buffer.shape_until_scroll(system, false);
        let mut result = Self {
            font_size: style.size,
            clusters: vec![Rectangle::default(); text.chars().count()],
            ..Self::default()
        };
        if style.vertical {
            result.vertical(system, text, &buffer, style);
        } else {
            for line in buffer.layout_runs() {
                result.size = Size::new(result.size.width.max(line.line_w), line.line_top + line.line_height);
                let offset: usize = text.split_inclusive('\n').take(line.line_i).map(str::len).sum();
                for g in line.glyphs {
                    result.cluster_boxes(
                        text,
                        offset + g.start,
                        offset + g.end,
                        Rectangle { x: g.x, y: line.line_top, width: g.w, height: style.line_height },
                        false,
                    );
                }
            }
            result.buffer = Some(Arc::new(buffer));
        }
        result
    }

    // Shape each resolved font run top-to-bottom; horizontal Latin runs are turned as CSS mixed orientation.
    fn vertical(&mut self, system: &mut cosmic::FontSystem, text: &str, buffer: &cosmic::Buffer, style: Style) {
        let mut advance = 0.0;
        for line in buffer.layout_runs() {
            let mut start = 0;
            while start < line.glyphs.len() {
                let first = &line.glyphs[start];
                let upright = style.upright || text[first.start..].chars().next().is_some_and(upright);
                let mut end = start + 1;
                while end < line.glyphs.len()
                    && line.glyphs[end].font_id == first.font_id
                    && (style.upright || text[line.glyphs[end].start..].chars().next().is_some_and(self::upright))
                        == upright
                {
                    end += 1;
                }
                if let Some(font) = system.get_font(first.font_id, first.font_weight) {
                    let range = &line.glyphs[start..end];
                    let byte_start = range.iter().map(|g| g.start).min().unwrap_or(first.start);
                    let byte_end = range.iter().map(|g| g.end).max().unwrap_or(first.end);
                    let mut input = cosmic::harfrust::UnicodeBuffer::new();
                    input.push_str(&text[byte_start..byte_end]);
                    input.guess_segment_properties();
                    if upright {
                        input.set_direction(cosmic::harfrust::Direction::TopToBottom);
                    }
                    let features = [
                        cosmic::harfrust::Feature::new(cosmic::harfrust::Tag::new(b"liga"), 0, ..),
                        cosmic::harfrust::Feature::new(cosmic::harfrust::Tag::new(b"clig"), 0, ..),
                    ];
                    let shaped = font.shaper().shape(input, if style.tracking == 0.0 { &[] } else { &features });
                    let scale = style.size / font.shaper().units_per_em() as f32;
                    for (info, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
                        let step = if upright { -pos.y_advance } else { pos.x_advance } as f32 * scale;
                        let (pen, axes) = if upright {
                            (
                                [
                                    style.line_height * 0.5 + pos.x_offset as f32 * scale,
                                    advance - pos.y_offset as f32 * scale,
                                ],
                                [1.0, 0.0, 0.0, 1.0],
                            )
                        } else {
                            (
                                [style.line_height - line.line_y, advance + pos.x_offset as f32 * scale],
                                [0.0, 1.0, -1.0, 0.0],
                            )
                        };
                        let mut mark = mark(first);
                        mark.id = info.glyph_id as u16;
                        mark.cluster = byte_start + info.cluster as usize;
                        self.glyphs.push(Glyph { mark, pen, axes });
                        let cluster_end = shaped
                            .glyph_infos()
                            .iter()
                            .map(|g| g.cluster as usize + byte_start)
                            .filter(|offset| *offset > mark.cluster)
                            .min()
                            .unwrap_or(byte_end);
                        self.cluster_boxes(
                            text,
                            mark.cluster,
                            cluster_end,
                            Rectangle { x: 0.0, y: advance, width: style.line_height, height: step + style.tracking },
                            true,
                        );
                        advance += step + style.tracking;
                    }
                } else {
                    tracing::error!(font = ?first.font_id, "vertical text font could not be loaded");
                }
                start = end;
            }
        }
        self.size = Size::new(style.line_height, advance);
    }

    fn cluster_boxes(&mut self, text: &str, start: usize, end: usize, bounds: Rectangle, vertical: bool) {
        let first = text[..start].chars().count();
        let count = text[start..end].chars().count().max(1);
        for (index, rect) in self.clusters.iter_mut().skip(first).take(count).enumerate() {
            *rect = if vertical {
                Rectangle {
                    y: bounds.y + bounds.height * index as f32 / count as f32,
                    height: bounds.height / count as f32,
                    ..bounds
                }
            } else {
                Rectangle {
                    x: bounds.x + bounds.width * index as f32 / count as f32,
                    width: bounds.width / count as f32,
                    ..bounds
                }
            };
        }
    }
}

fn upright(ch: char) -> bool {
    !matches!(unicode_vo::char_orientation(ch), unicode_vo::Orientation::Rotated)
}

fn mark(g: &cosmic::LayoutGlyph) -> ShapedGlyph {
    ShapedGlyph {
        font: g.font_id,
        id: g.glyph_id,
        weight: g.font_weight.0,
        flags: g.cache_key_flags,
        cluster: g.start,
        x: 0.0,
        y: 0.0,
    }
}

/// Snap a pen to the physical pixel grid, after applying the window's DPI scale.
pub fn pixel_origin(value: f32, scale: f32) -> f32 {
    (value * scale).round() / scale
}

#[derive(Debug)]
pub struct Pass {
    pub run: Arc<Run>,
    pub color: Color,
    size: Size,
    start: Slot,
    end: Slot,
}

impl Pass {
    pub fn new(run: Arc<Run>, color: Color, size: Size) -> Self {
        Self { run, color, size, start: Slot::default(), end: Slot::default() }
    }
}

pub struct Pipeline {
    glyphs: Glyphs,
    sprites: Vec<Sprite>,
    physical: Size<u32>,
}

impl shader::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            glyphs: Glyphs::interface(device, format, 4096),
            sprites: Vec::with_capacity(4096),
            physical: Size::new(1, 1),
        }
    }
    fn trim(&mut self) {
        if self.sprites.is_empty() {
            self.glyphs.release();
        }
        self.sprites.clear();
    }
}

impl shader::Primitive for Pass {
    type Pipeline = Pipeline;
    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        let scale = viewport.scale_factor();
        let zx = bounds.width / self.size.width.max(0.001);
        let zy = bounds.height / self.size.height.max(0.001);
        let zoom = zx.abs().max(zy.abs()).max(0.001);
        self.start.set(pipeline.sprites.len());
        for glyph in &self.run.glyphs {
            pipeline.sprites.push(Sprite {
                mark: Mark::Glyph {
                    glyph: glyph.mark,
                    size: self.run.font_size * zoom,
                    outline: 0.0,
                    max_scale: f32::MAX,
                },
                anchor: [
                    pixel_origin(bounds.x + glyph.pen[0] * zx, scale),
                    pixel_origin(bounds.y + glyph.pen[1] * zy, scale),
                ],
                pen: [0.0, 0.0],
                axes: [
                    glyph.axes[0] * zx / zoom,
                    glyph.axes[1] * zy / zoom,
                    glyph.axes[2] * zx / zoom,
                    glyph.axes[3] * zy / zoom,
                ],
                color: [self.color.r, self.color.g, self.color.b, self.color.a],
            });
        }
        self.end.set(pipeline.sprites.len());
        pipeline.physical = viewport.physical_size();
        let logical = viewport.logical_size();
        pipeline.glyphs.prepare(device, queue, &pipeline.sprites, [logical.width, logical.height], scale);
    }
    fn draw(&self, pipeline: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        pass.set_viewport(0.0, 0.0, pipeline.physical.width as f32, pipeline.physical.height as f32, 0.0, 1.0);
        pipeline.glyphs.draw(pass, self.start.get() as u32..self.end.get() as u32);
        true
    }
}
