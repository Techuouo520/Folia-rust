// Claddagh glyph batches keep Gaussian depth blur and text shadows on the GPU.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use folia_core::Rgb;
use folia_render::glyphs::{Batch, Glyphs, Mark, Shaped, Sprite};
use iced::widget::canvas::{Frame, Geometry};
use iced::widget::{canvas, shader, stack};
use iced::{Element, Length, Rectangle, Renderer as CanvasRenderer, Size, Vector, mouse};

use super::{Cache, DrawCtx, layout, manifest};
use crate::Scene;

const COVERAGE_SCALE: f32 = 2.0;

#[cfg(feature = "dev")]
static LIVE_SLOTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "dev")]
static TEXTURE_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Live widget renderers and their actual sharp/Gaussian atlas texture allocation.
#[cfg(feature = "dev")]
pub fn gpu_stats() -> (usize, u64) {
    (LIVE_SLOTS.load(Ordering::Relaxed), TEXTURE_BYTES.load(Ordering::Relaxed))
}

struct TextLayer<'a, Message> {
    scene: Scene<'a>,
    capture: bool,
    marker: PhantomData<Message>,
}

pub fn view<'a, Message: 'a>(scene: Scene<'a>, capture: bool) -> Element<'a, Message> {
    let axis = canvas(Axis { scene: scene.clone(), capture }).width(Length::Fill).height(Length::Fill);
    let text = shader(TextLayer { scene, capture, marker: PhantomData }).width(Length::Fill).height(Length::Fill);
    if capture {
        stack![shader(folia_render::StyleScene::<Message>::new()).width(Length::Fill).height(Length::Fill), axis, text]
            .into()
    } else {
        stack![axis, text].into()
    }
}

struct Axis<'a> {
    scene: Scene<'a>,
    capture: bool,
}

impl<Message> canvas::Program<Message> for Axis<'_> {
    type State = RefCell<Cache>;
    fn draw(
        &self,
        state: &Self::State,
        renderer: &CanvasRenderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let under = crate::common::take_blur();
        let mut frame = Frame::new(renderer, bounds.size());
        if self.scene.backdrop {
            frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), crate::color(self.scene.theme.background, 1.0));
        }
        let (size, place) = self.scene.placement(bounds.size());
        frame.translate(place.offset);
        frame.scale(place.scale);
        let mut cache = state.borrow_mut();
        if self.scene.tuning.flag(manifest(), "showAxisLine")
            && let Some(ctx) = super::context(&self.scene, &mut cache, size)
        {
            frame.translate(Vector::new(
                size.width * self.scene.tuning.number(manifest(), "positionX") / 100.0,
                size.height * self.scene.tuning.number(manifest(), "positionY") / 100.0,
            ));
            super::draw_axis(&mut frame, size, &self.scene, ctx.tilt_deg, &cache);
        }
        let geometry = vec![frame.into_geometry()];
        if self.capture {
            if !self.scene.backdrop && self.scene.shows_shared_background() && !under.is_empty() {
                folia_render::capture_layers(&[&under, &geometry]);
            } else {
                folia_render::capture_canvas(&geometry);
            }
        }
        geometry
    }
}

#[derive(Default)]
pub struct State {
    cache: RefCell<Cache>,
    batch: Batch,
    owner: Arc<AtomicBool>,
}

impl<Message> shader::Program<Message> for TextLayer<'_, Message> {
    type State = State;
    type Primitive = Pass;

    fn update(
        &self,
        state: &mut State,
        _event: &iced::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<shader::Action<Message>> {
        (!state.owner.load(Ordering::Acquire)).then(|| {
            shader::Action::request_redraw_at(std::time::Instant::now() + std::time::Duration::from_millis(16))
        })
    }

    fn draw(&self, state: &State, _cursor: mouse::Cursor, bounds: Rectangle) -> Pass {
        let (size, place) = self.scene.placement(bounds.size());
        let mut sprites = state.batch.lock();
        sprites.clear();
        let mut cache = state.cache.borrow_mut();
        if let Some(ctx) = super::context(&self.scene, &mut cache, size) {
            let start = sprites.len();
            for index in
                ctx.center.saturating_sub(1)..=(ctx.center + 2).min(self.scene.lyrics.lines.len().saturating_sub(1))
            {
                line(&ctx, &mut cache, index, &mut sprites);
            }
            let offset = [
                size.width * self.scene.tuning.number(manifest(), "positionX") / 100.0,
                size.height * self.scene.tuning.number(manifest(), "positionY") / 100.0,
            ];
            for sprite in &mut sprites[start..] {
                sprite.anchor[0] = (sprite.anchor[0] + offset[0]) * place.scale + place.offset.x;
                sprite.anchor[1] = (sprite.anchor[1] + offset[1]) * place.scale + place.offset.y;
                for axis in &mut sprite.axes {
                    *axis *= place.scale;
                }
                if let Mark::BlurredGlyph { max_scale, .. } = &mut sprite.mark {
                    *max_scale *= place.scale;
                }
            }
        }
        drop(sprites);
        Pass {
            batch: state.batch.clone(),
            owner: Arc::downgrade(&state.owner),
            capture: self.capture,
            size: bounds.size(),
        }
    }
}

fn line(ctx: &DrawCtx<'_>, cache: &mut Cache, index: usize, sprites: &mut Vec<Sprite>) {
    let (line, moment, end) = super::line_frame(ctx, cache, index);
    let font = ctx.scene.typography.font(crate::BOLD);
    let shapes = line.shapes.get_or_init(|| {
        line.placed
            .iter()
            .map(|item| folia_render::glyphs::shape(&item.text, ctx.font_px * COVERAGE_SCALE, font))
            .collect()
    });
    for (index, (item, shaped)) in line.placed.iter().zip(shapes).enumerate() {
        let pose = layout::glyph(item, index, &moment);
        if pose.opacity <= 0.004 || pose.scale <= 0.01 || shaped.glyphs.is_empty() {
            continue;
        }
        let (blur, focus) = camera(item, &moment);
        let sung = super::sung_amount(item, ctx.scene.time);
        let highlight = line.colors.get(index).copied().flatten().unwrap_or(ctx.scene.theme.accent);
        let color = ctx.scene.theme.primary.mix(highlight, sung);
        let alpha = 0.55 + 0.45 * sung;
        let progress = ((ctx.scene.time - item.start) / (item.end - item.start).max(0.001)).clamp(0.0, 1.0) as f32;
        let flash = if progress > 0.0 && progress < 0.4 { (1.0 - progress / 0.4).powi(2) * 0.65 } else { 0.0 };
        let glow = if moment.chorus { (36.0 + moment.power * 24.0) * focus.powf(1.5) } else { 24.0 * focus.powi(2) }
            * (1.0 + flash);
        let fade = ((glow - 0.5) / 2.0).clamp(0.0, 1.0);
        if glow > 0.5 && fade > 0.01 {
            if moment.chorus {
                glyphs(
                    sprites,
                    shaped,
                    ctx,
                    pose,
                    ctx.font_px,
                    Ink { color, alpha: alpha * fade, sigma: (glow * 0.8).hypot(blur) },
                );
                glyphs(
                    sprites,
                    shaped,
                    ctx,
                    pose,
                    ctx.font_px,
                    Ink { color, alpha: alpha * fade, sigma: (glow * 0.5).hypot(blur) },
                );
                glyphs(
                    sprites,
                    shaped,
                    ctx,
                    pose,
                    ctx.font_px,
                    Ink {
                        color: color.mix(ctx.scene.theme.primary, 0.65),
                        alpha: fade,
                        sigma: (glow * 0.175).hypot(blur),
                    },
                );
            } else {
                glyphs(
                    sprites,
                    shaped,
                    ctx,
                    pose,
                    ctx.font_px,
                    Ink { color, alpha: alpha * fade, sigma: (glow * 0.5).hypot(blur) },
                );
            }
        }
        glyphs(sprites, shaped, ctx, pose, ctx.font_px, Ink { color, alpha, sigma: blur });
    }
    let size = super::translation_font_size(ctx);
    let font =
        ctx.scene.typography.subtitle_font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT });
    let shapes = line.translation_shapes.get_or_init(|| {
        line.translation
            .iter()
            .map(|item| folia_render::glyphs::shape(&item.text, size * COVERAGE_SCALE, font))
            .collect()
    });
    let sung = layout::playback_progress(&line.placed, ctx.scene.time, end) as f32 * line.translation.len() as f32;
    for (index, (item, shaped)) in line.translation.iter().zip(shapes).enumerate() {
        let (pose, blur) = layout::translation_glyph(item, index, &moment, size);
        if pose.opacity <= 0.004 || pose.scale <= 0.01 {
            continue;
        }
        let amount = (sung - index as f32).clamp(0.0, 1.0);
        glyphs(
            sprites,
            shaped,
            ctx,
            pose,
            size,
            Ink {
                color: ctx.scene.theme.primary.mix(ctx.scene.theme.accent, amount),
                alpha: 0.55 + 0.3 * amount,
                sigma: blur,
            },
        );
    }
}

fn camera(item: &layout::Placed, moment: &layout::RingMoment) -> (f32, f32) {
    let curve = moment.line_index as f64 * std::f64::consts::PI + item.nominal - moment.ring - moment.word_offset;
    let depth = ((curve.cos() + 1.0) * 0.5) as f32;
    let line_diff = (moment.ring - moment.line_index as f64 * std::f64::consts::PI).abs() / std::f64::consts::PI;
    let focus = (1.0 - line_diff).max(0.0) as f32 * (1.0 - (curve.abs() as f32 / 0.48).min(1.0)).powf(1.8);
    (8.0 * (1.0 - depth) * (1.0 - 0.5 * focus), focus)
}

struct Ink {
    color: Rgb,
    alpha: f32,
    sigma: f32,
}

fn glyphs(sprites: &mut Vec<Sprite>, shaped: &Shaped, ctx: &DrawCtx<'_>, pose: layout::Glyph, size: f32, ink: Ink) {
    let Ink { color, alpha, sigma } = ink;
    let (sin, cos) = pose.rotation.to_radians().sin_cos();
    let scale = pose.scale / COVERAGE_SCALE;
    let budget = (scale.max(0.03125).log2().ceil()).exp2();
    let rgba = crate::color(color, alpha * pose.opacity);
    sprites.extend(shaped.glyphs.iter().map(|glyph| Sprite {
        mark: Mark::BlurredGlyph {
            glyph: *glyph,
            size: size * COVERAGE_SCALE,
            outline: 0.0,
            max_scale: budget,
            sigma: if sigma < 0.2 { 0.0 } else { sigma * COVERAGE_SCALE },
        },
        anchor: [ctx.size.width * 0.5 + pose.x, ctx.size.height * 0.5 + pose.y],
        pen: [glyph.x - shaped.width * 0.5, glyph.y + shaped.baseline],
        axes: [cos * scale, sin * scale, -sin * scale, cos * scale],
        color: [rgba.r, rgba.g, rgba.b, rgba.a],
    }));
}

#[derive(Debug)]
pub struct Pass {
    batch: Batch,
    owner: Weak<AtomicBool>,
    capture: bool,
    size: Size,
}

struct Renderer {
    glyphs: Glyphs,
    sprites: Vec<Sprite>,
    count: u32,
    bounds: [f32; 4],
    stamp: u64,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    #[cfg(feature = "dev")]
    texture_bytes: u64,
}

impl Renderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let mut glyphs = Glyphs::new(device, format, 512, Vec::new());
        glyphs.warm_blur(device);
        Self {
            glyphs,
            sprites: Vec::new(),
            count: 0,
            bounds: [0.0; 4],
            stamp: 0,
            device: device.clone(),
            queue: queue.clone(),
            format,
            #[cfg(feature = "dev")]
            texture_bytes: 0,
        }
    }

    fn prepare(&mut self, pass: &Pass, bounds: &Rectangle, scale: f32) {
        use std::hash::{Hash, Hasher};
        self.sprites.clear();
        self.sprites.extend_from_slice(&pass.batch.lock());
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for sprite in &mut self.sprites {
            if let Mark::BlurredGlyph { max_scale, .. } = &mut sprite.mark {
                *max_scale *= scale;
            }
            if self.format.is_srgb() {
                for channel in &mut sprite.color[..3] {
                    *channel = folia_render::to_linear_channel(*channel);
                }
            }
            hash.write(bytemuck::cast_slice(&sprite.anchor));
            hash.write(bytemuck::cast_slice(&sprite.axes));
            hash.write(bytemuck::cast_slice(&sprite.pen));
            hash.write(bytemuck::cast_slice(&sprite.color));
            if let Mark::BlurredGlyph { sigma, glyph, size, .. } = sprite.mark {
                hash.write_u16(glyph.id);
                glyph.font.hash(&mut hash);
                glyph.weight.hash(&mut hash);
                glyph.flags.hash(&mut hash);
                hash.write_u32(size.to_bits());
                hash.write_u32(sigma.to_bits());
            }
        }
        hash.write_u32(pass.size.width.to_bits());
        hash.write_u32(pass.size.height.to_bits());
        self.stamp = hash.finish();
        self.count = self.sprites.len() as u32;
        self.glyphs.reserve(&self.device, self.sprites.len());
        self.glyphs.prepare(&self.device, &self.queue, &self.sprites, [pass.size.width, pass.size.height], scale);
        #[cfg(feature = "dev")]
        {
            let size = self.glyphs.stats().size;
            let bytes = u64::from(size[0]) * u64::from(size[1]) + self.glyphs.blur_stats().allocated_bytes;
            TEXTURE_BYTES.fetch_sub(self.texture_bytes, Ordering::Relaxed);
            TEXTURE_BYTES.fetch_add(bytes, Ordering::Relaxed);
            self.texture_bytes = bytes;
        }
        self.bounds = [bounds.x * scale, bounds.y * scale, bounds.width * scale, bounds.height * scale];
    }

    fn render(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, bounds: [f32; 4], clip: [u32; 4]) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-claddagh"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        pass.set_viewport(bounds[0], bounds[1], bounds[2], bounds[3], 0.0, 1.0);
        pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
        self.glyphs.draw(&mut pass, 0..self.count);
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        #[cfg(feature = "dev")]
        TEXTURE_BYTES.fetch_sub(self.texture_bytes, Ordering::Relaxed);
    }
}

struct Slot {
    key: usize,
    owner: Weak<AtomicBool>,
    renderer: folia_render::Prepared<Renderer>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        #[cfg(feature = "dev")]
        LIVE_SLOTS.fetch_sub(1, Ordering::Relaxed);
    }
}
pub struct Pipeline {
    slots: Vec<Slot>,
    format: wgpu::TextureFormat,
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
        let key = self.owner.as_ptr() as usize;
        let index = if let Some(index) = pipeline.slots.iter().position(|slot| slot.key == key) {
            index
        } else {
            #[cfg(feature = "dev")]
            LIVE_SLOTS.fetch_add(1, Ordering::Relaxed);
            let (device, queue, format) = (device.clone(), queue.clone(), pipeline.format);
            pipeline.slots.push(Slot {
                key,
                owner: self.owner.clone(),
                renderer: folia_render::Prepared::new(self.owner.clone(), move || {
                    Renderer::new(&device, &queue, format)
                }),
            });
            pipeline.slots.len() - 1
        };
        if let Some(renderer) = pipeline.slots[index].renderer.poll() {
            renderer.prepare(self, bounds, viewport.scale_factor());
        }
    }
    fn render(
        &self,
        pipeline: &Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip: &Rectangle<u32>,
    ) {
        let Some(renderer) = pipeline
            .slots
            .iter()
            .find(|slot| slot.key == self.owner.as_ptr() as usize)
            .and_then(|slot| slot.renderer.get())
        else {
            return;
        };
        renderer.render(encoder, target, renderer.bounds, [clip.x, clip.y, clip.width, clip.height]);
        if self.capture {
            let drew = folia_render::draw_source(
                &renderer.device,
                &renderer.queue,
                renderer.format,
                [self.size.width, self.size.height],
                renderer.stamp,
                |source, width, height| {
                    renderer.render(encoder, source, [0.0, 0.0, width as f32, height as f32], [0, 0, width, height]);
                },
            );
            if drew {
                folia_render::blur_source(encoder);
            }
        }
    }
}

impl shader::Pipeline for Pipeline {
    fn new(_device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self { slots: Vec::new(), format }
    }
    fn trim(&mut self) {
        self.slots.retain(|slot| slot.owner.strong_count() > 0);
    }
}
