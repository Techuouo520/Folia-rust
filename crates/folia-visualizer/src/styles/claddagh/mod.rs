// Claddagh: graphemes on a tilted ellipse. Lyric motion follows playback time; the bass and vocal springs follow audio.

mod audio;
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

pub use layout::{RingMoment, glyph, placed, ring_angle};
pub use timeline::{adjust, line_marks, visual_length, word_offset};
pub use tuning::manifest;

const FONT_PX: f32 = 72.0;

pub struct LineLayout {
    placed: Vec<Placed>,
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
    let lines = &scene.lyrics.lines;
    let Some(center) = layout::center_line(lines, scene.time) else {
        return;
    };
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
    if scene.tuning.flag(manifest, "showAxisLine") {
        draw_axis(frame, size, scene, tilt_deg, cache);
    }
    let first = center.saturating_sub(1);
    let last = (center + 2).min(lines.len().saturating_sub(1));
    let ctx = DrawCtx {
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
        salt: scene.tuning.fingerprint(),
    };
    for index in first..=last {
        draw_line(&ctx, cache, frame, index);
    }
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
    let (bass, vocal) =
        if scene.audio.has_spectrum { (unit(cache.bass.value()), unit(cache.vocal.value())) } else { (0.0, 0.0) };
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
        frame.fill(
            &Path::rectangle(Point::new(-150.0 * scale_x, -2.0 * scale_y), Size::new(300.0 * scale_x, 4.0 * scale_y)),
            color(mixed, alpha),
        );
    });
}

fn draw_line(ctx: &DrawCtx<'_>, cache: &mut Cache, frame: &mut Frame, index: usize) {
    let DrawCtx { scene, size, center, rx, base_rx, font_px, ring, focus, tilt_deg, letter, power, salt } = *ctx;
    let line = &scene.lyrics.lines[index];
    let spacing = f64::from((1.0 + focus) / (1.0 + crate::Values::default().number(manifest(), "focusScaleRatio")));
    let layout = cache.layout.get_or_insert(line, size, scene.font_scale, salt, || {
        Rc::new(LineLayout { placed: layout::placed(line, font_px, base_rx, spacing, letter) })
    });
    let active = cache.layout.get_or_insert(&scene.lyrics.lines[center], size, scene.font_scale, salt, || {
        Rc::new(LineLayout { placed: layout::placed(&scene.lyrics.lines[center], font_px, base_rx, spacing, letter) })
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
    let origin = Point::new(size.width / 2.0, size.height / 2.0);
    for (glyph_index, item) in layout.placed.iter().enumerate() {
        let pose = layout::glyph(
            item,
            glyph_index,
            &RingMoment {
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
            },
        );
        paint(frame, origin, item, pose, font_px, scene);
    }
}

fn paint(frame: &mut Frame, origin: Point, item: &Placed, pose: Glyph, font_px: f32, scene: &Scene<'_>) {
    if pose.opacity <= 0.004 || pose.scale <= 0.01 {
        return;
    }
    let sung = sung_amount(item, scene.time);
    let body = if sung >= 1.0 { scene.theme.accent } else { scene.theme.primary.mix(scene.theme.accent, sung) };
    let alpha = (0.55 + 0.45 * sung) * pose.opacity;
    frame.with_save(|frame| {
        frame.translate(Vector::new(origin.x + pose.x, origin.y + pose.y));
        frame.rotate(pose.rotation.to_radians());
        frame.scale(pose.scale);
        frame.fill_text(Text {
            content: item.text.clone(),
            position: Point::ORIGIN,
            color: color(body, alpha),
            size: font_px.into(),
            font: iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT },
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
