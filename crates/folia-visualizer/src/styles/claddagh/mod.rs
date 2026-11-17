// Claddagh: graphemes on a tilted ellipse. Lyric motion follows playback time; the bass and vocal springs follow audio.

mod audio;
mod gpu;
mod layout;
mod timeline;
mod tuning;

use std::rc::Rc;

use folia_core::{AnimationIntensity, Rgb};
use iced::widget::canvas::{Frame, Path, Text};
use iced::{Point, Size, Vector};

use crate::styles::claddagh::audio::{Spring, unit};
use crate::styles::claddagh::layout::{Glyph, Placed};
use crate::{LayoutCache, Scene, color};

#[cfg(feature = "dev")]
pub use gpu::gpu_stats;
pub use gpu::view;
pub use layout::{RingMoment, glyph, placed, ring_angle};
pub use timeline::{adjust, line_marks, visual_length, word_offset};
pub use tuning::manifest;

const FONT_PX: f32 = 72.0;

pub struct LineLayout {
    placed: Vec<Placed>,
    translation: Vec<Placed>,
    colors: Vec<Option<Rgb>>,
    shapes: std::cell::OnceCell<Vec<folia_render::glyphs::Shaped>>,
    translation_shapes: std::cell::OnceCell<Vec<folia_render::glyphs::Shaped>>,
}

/// Measured lines plus the two band springs. The springs are the original `useSpring` calls.
pub struct Cache {
    layout: LayoutCache<LineLayout>,
    bass: Spring,
    vocal: Spring,
    last_ms: f64,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            layout: LayoutCache::default(),
            bass: Spring::new(150.0, 25.0),
            vocal: Spring::new(120.0, 24.0),
            last_ms: 0.0,
        }
    }
}

/// Draws the ring for `scene.time`. The ring's power is the smoothed bass band; a paused axis stays at rest.
pub fn draw(scene: &Scene<'_>, cache: &mut Cache, frame: &mut Frame, size: Size) {
    let Some(ctx) = context(scene, cache, size) else {
        return;
    };
    let manifest = manifest();
    let offset = Vector::new(
        size.width * scene.tuning.number(manifest, "positionX") / 100.0,
        size.height * scene.tuning.number(manifest, "positionY") / 100.0,
    );
    frame.with_save(|frame| {
        frame.translate(offset);
        if scene.tuning.flag(manifest, "showAxisLine") {
            draw_axis(frame, size, scene, ctx.tilt_deg, cache);
        }
        for index in ctx.center.saturating_sub(1)..=(ctx.center + 2).min(scene.lyrics.lines.len().saturating_sub(1)) {
            draw_line(&ctx, cache, frame, index);
        }
    });
}

fn context<'a>(scene: &'a Scene<'a>, cache: &mut Cache, size: Size) -> Option<DrawCtx<'a>> {
    let lines = &scene.lyrics.lines;
    let center = layout::center_line(lines, scene.time)?;
    let manifest = manifest();
    let font_px = FONT_PX * scene.font_scale;
    let radius_scale = scene.tuning.number(manifest, "radiusScale");
    let tilt_deg = scene.tuning.number(manifest, "ellipseTiltDeg");
    let focus = scene.tuning.number(manifest, "focusScaleRatio");
    let letter = scene.tuning.number(manifest, "letterSpacingOffset");
    let base_rx = (size.width * 0.44).min(560.0) * radius_scale;
    let ring = ring_angle(lines, scene.time);
    let power = follow(cache, scene);
    let scale_factor = radius_scale_of(scene.theme.intensity, power);
    let rx = base_rx * scale_factor;
    Some(DrawCtx {
        scene,
        size,
        center,
        rx,
        base_rx,
        font_px,
        ring,
        focus,
        tilt_deg,
        letter,
        power,
        salt: scene.layout_key(),
    })
}

struct DrawCtx<'a> {
    scene: &'a Scene<'a>,
    size: Size,
    center: usize,
    rx: f32,
    base_rx: f32,
    font_px: f32,
    ring: f64,
    focus: f32,
    tilt_deg: f32,
    letter: f32,
    power: f32,
    salt: u64,
}

fn draw_axis(frame: &mut Frame, size: Size, scene: &Scene<'_>, tilt_deg: f32, cache: &Cache) {
    let (bass, vocal) = if !scene.paused && scene.audio.has_spectrum {
        (unit(cache.bass.value()), unit(cache.vocal.value()))
    } else {
        (0.0, 0.0)
    };
    let color_power = bass.max(vocal);
    let ratio = ((color_power - 0.02).max(0.0) / 0.58).min(1.0);
    let from = scene.theme.primary;
    let mut to = scene.theme.accent;
    if to == from {
        to = scene.theme.secondary;
    }
    if to == from {
        to = Rgb::hex(0x00ff_ffff);
    }
    let mixed = from.mix(to, ratio);
    let alpha = 0.2 + 0.75 * ratio;
    let bass_sq = bass * bass;
    let scale_x = 1.0 + bass_sq * 1.5;
    let scale_y = 1.0 + bass_sq * 0.5;
    let angle = (90.0 - tilt_deg).to_radians();
    frame.with_save(|frame| {
        frame.translate(Vector::new(size.width / 2.0, size.height / 2.0));
        frame.rotate(angle);
        let gradient = iced::widget::canvas::gradient::Linear::new(
            Point::new(-150.0 * scale_x, 0.0),
            Point::new(150.0 * scale_x, 0.0),
        )
        .add_stop(0.0, color(mixed, 0.0))
        .add_stop(0.2, color(mixed, alpha))
        .add_stop(0.8, color(mixed, alpha))
        .add_stop(1.0, color(mixed, 0.0));
        frame.fill(
            &Path::rectangle(Point::new(-150.0 * scale_x, -2.0 * scale_y), Size::new(300.0 * scale_x, 4.0 * scale_y)),
            gradient,
        );
    });
}

fn draw_line(ctx: &DrawCtx<'_>, cache: &mut Cache, frame: &mut Frame, index: usize) {
    let (layout, moment, own_end) = line_frame(ctx, cache, index);
    for (glyph_index, item) in layout.placed.iter().enumerate() {
        let pose = layout::glyph(item, glyph_index, &moment);
        paint(ctx, frame, item, pose, layout.colors.get(glyph_index).copied().flatten());
    }
    paint_translation(ctx, frame, &layout, &moment, own_end);
}

fn line_frame(ctx: &DrawCtx<'_>, cache: &mut Cache, index: usize) -> (Rc<LineLayout>, RingMoment, Option<f64>) {
    let DrawCtx { scene, size, center, rx, font_px, ring, focus, tilt_deg, power, salt, .. } = *ctx;
    let line = &scene.lyrics.lines[index];
    let layout = cache.layout.get_or_insert(line, size, scene.font_scale, salt, || measured_line(ctx, line));
    let active = cache.layout.get_or_insert(&scene.lyrics.lines[center], size, scene.font_scale, salt, || {
        measured_line(ctx, &scene.lyrics.lines[center])
    });
    let end = layout::render_end(&scene.lyrics.lines, center);
    let own_end = layout::render_end(&scene.lyrics.lines, index);
    let active_offset = layout::sung_offset(&active.placed, scene.time, end);
    let own_offset = layout::sung_offset(&layout.placed, scene.time, own_end);
    let progress = layout::playback_progress(&active.placed, scene.time, end);
    let shift = layout::word_shift(index, center, own_offset, active_offset, progress, ring);
    let line_diff = (ring - index as f64 * std::f64::consts::PI).abs() / std::f64::consts::PI;
    let (fade, length_scale) = layout::length_factors(&scene.lyrics.lines, index, center, line_diff);
    let upcoming = layout::entry_progress(line, scene.time, index > center);
    let moment = RingMoment {
        line_index: index,
        center,
        ring,
        word_offset: shift,
        rx,
        focus_scale: focus,
        tilt_deg,
        power,
        chorus: line.is_chorus,
        length_fade: fade,
        length_scale,
        upcoming,
        font_px,
    };
    (layout, moment, own_end)
}

fn measured_line(ctx: &DrawCtx<'_>, line: &folia_core::Line) -> Rc<LineLayout> {
    let spacing = f64::from((1.0 + ctx.focus) / (1.0 + crate::Values::default().number(manifest(), "focusScaleRatio")));
    let placed = layout::placed_with_font(
        line,
        ctx.font_px,
        ctx.base_rx,
        spacing,
        ctx.letter,
        ctx.scene.typography.font(crate::BOLD),
    );
    let translation = if ctx.scene.tuning.flag(manifest(), "showTranslation") {
        let text = line.translation.as_deref().unwrap_or_default().trim();
        let marks = folia_lyrics::graphemes(text)
            .into_iter()
            .map(|range| timeline::Mark { text: text[range].to_owned(), start: line.start, end: line.end })
            .collect::<Vec<_>>();
        layout::place_marks(
            &marks,
            translation_font_size(ctx),
            ctx.base_rx,
            1.0,
            ctx.scene.tuning.number(manifest(), "translationLetterSpacingOffset"),
            ctx.scene
                .typography
                .subtitle_font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT }),
        )
    } else {
        Vec::new()
    };
    let ranges = folia_core::word_color_ranges(&line.text, &ctx.scene.theme.word_colors);
    let colors = folia_lyrics::graphemes(&line.text)
        .into_iter()
        .map(|range| {
            ranges
                .iter()
                .find(|color| range.start < color.range.end && color.range.start < range.end)
                .map(|entry| entry.color)
        })
        .collect();
    Rc::new(LineLayout {
        placed,
        translation,
        colors,
        shapes: Default::default(),
        translation_shapes: Default::default(),
    })
}

fn translation_font_size(ctx: &DrawCtx<'_>) -> f32 {
    ctx.font_px * 0.5 * ctx.scene.tuning.number(manifest(), "translationScale")
}

fn paint_translation(ctx: &DrawCtx<'_>, frame: &mut Frame, line: &LineLayout, moment: &RingMoment, end: Option<f64>) {
    let size = translation_font_size(ctx);
    let font =
        ctx.scene.typography.subtitle_font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT });
    let sung = layout::playback_progress(&line.placed, ctx.scene.time, end) as f32 * line.translation.len() as f32;
    for (index, item) in line.translation.iter().enumerate() {
        let (pose, blur) = layout::translation_glyph(item, index, moment, size);
        if pose.opacity <= 0.004 || pose.scale <= 0.01 {
            continue;
        }
        let amount = (sung - index as f32).clamp(0.0, 1.0);
        let ink = ctx.scene.theme.primary.mix(ctx.scene.theme.accent, amount);
        let opacity = (0.55 + 0.3 * amount) * pose.opacity;
        frame.with_save(|frame| {
            frame.translate(Vector::new(ctx.size.width * 0.5 + pose.x, ctx.size.height * 0.5 + pose.y));
            frame.rotate(pose.rotation.to_radians());
            frame.scale(pose.scale);
            let offsets: &[(f32, f32)] = if blur < 0.2 {
                &[(0.0, 0.0)]
            } else {
                &[
                    (0.0, 0.0),
                    (0.7, 0.0),
                    (0.35, 0.6062),
                    (-0.35, 0.6062),
                    (-0.7, 0.0),
                    (-0.35, -0.6062),
                    (0.35, -0.6062),
                    (1.5, 0.0),
                    (0.75, 1.299),
                    (-0.75, 1.299),
                    (-1.5, 0.0),
                    (-0.75, -1.299),
                    (0.75, -1.299),
                ]
            };
            let opacity = 1.0 - (1.0 - opacity).powf(1.0 / offsets.len() as f32);
            for (x, y) in offsets {
                frame.fill_text(Text {
                    content: item.text.clone(),
                    position: Point::new(x * blur, y * blur),
                    color: color(ink, opacity),
                    size: size.into(),
                    font,
                    align_x: iced::widget::text::Alignment::Center,
                    align_y: iced::alignment::Vertical::Center,
                    shaping: iced::widget::text::Shaping::Advanced,
                    ..Text::default()
                });
            }
        });
    }
}

fn paint(ctx: &DrawCtx<'_>, frame: &mut Frame, item: &Placed, pose: Glyph, highlight: Option<Rgb>) {
    if pose.opacity <= 0.004 || pose.scale <= 0.01 {
        return;
    }
    let sung = sung_amount(item, ctx.scene.time);
    let body = ctx.scene.theme.primary.mix(highlight.unwrap_or(ctx.scene.theme.accent), sung);
    frame.with_save(|frame| {
        frame.translate(Vector::new(ctx.size.width * 0.5 + pose.x, ctx.size.height * 0.5 + pose.y));
        frame.rotate(pose.rotation.to_radians());
        frame.scale(pose.scale);
        frame.fill_text(Text {
            content: item.text.clone(),
            position: Point::ORIGIN,
            color: color(body, (0.55 + 0.45 * sung) * pose.opacity),
            size: ctx.font_px.into(),
            font: ctx.scene.typography.font(crate::BOLD),
            align_x: iced::widget::text::Alignment::Center,
            align_y: iced::alignment::Vertical::Center,
            shaping: iced::widget::text::Shaping::Advanced,
            ..Text::default()
        });
    });
}

fn follow(cache: &mut Cache, scene: &Scene<'_>) -> f32 {
    let now = wall_ms();
    let dt = if cache.last_ms > 0.0 { ((now - cache.last_ms) / 1000.0).clamp(0.0, 0.05) } else { 1.0 / 60.0 };
    cache.last_ms = now;
    cache.bass.step(scene.audio.bass, dt as f32);
    cache.vocal.step(scene.audio.vocal, dt as f32);
    unit(cache.bass.value())
}

fn radius_scale_of(intensity: AnimationIntensity, power: f32) -> f32 {
    let (multiplier, max_scale) = match intensity {
        AnimationIntensity::Calm => (0.08, 1.08),
        AnimationIntensity::Normal => (0.25, 1.25),
        AnimationIntensity::Chaotic => (0.95, 1.95),
    };
    (1.0 + power * multiplier).min(max_scale)
}

fn wall_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

fn sung_amount(item: &Placed, time: f64) -> f32 {
    if time >= item.end {
        1.0
    } else if time > item.start {
        let duration = item.end - item.start;
        let progress = if duration > 0.0 { (time - item.start) / duration } else { 1.0 };
        (progress * 15.0).min(1.0) as f32
    } else {
        0.0
    }
}
