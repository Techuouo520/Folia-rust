// Monet's poster: title column, lyric rail, portrait and the bottom spectrum.

use folia_core::{AnimationIntensity, Line};
use folia_lyrics::{RenderHints, active_line};
use iced::widget::canvas::{Frame, Path, Text};
use iced::widget::image::Handle;
use iced::{Point, Rectangle, Size, Vector, border::Radius};

use crate::styles::monet::motion::{self, fade, intro};
use crate::styles::monet::tuning;
use crate::{BOLD, LayoutCache, Scene, color, measure};

const ITALIC: iced::Font = iced::Font { style: iced::font::Style::Italic, ..iced::Font::DEFAULT };
const SEMI: iced::Font = iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT };

pub struct LineBox {
    text: String,
    width: f32,
}

#[derive(Default)]
pub struct Cache {
    lines: LayoutCache<LineBox>,
    /// Click the hanger to edit, drag while the button is down, click again to store.
    editing: bool,
    /// Portrait shift while editing, in pixels, `-150..=0`.
    live_offset: Option<f32>,
    /// Set on the click that leaves edit mode, until the saved value catches up.
    pending: bool,
    down: bool,
    moved: bool,
    grab: Option<f32>,
}

struct Stage {
    left: f32,
    text_width: f32,
    portrait: Option<Rectangle>,
    hanger: Option<Rectangle>,
    audio: Rectangle,
    active_px: f32,
    inactive_px: f32,
    translation_px: f32,
    title_px: f32,
    artist_px: f32,
    album_px: f32,
}

pub fn draw(
    scene: &Scene<'_>,
    cache: &mut Cache,
    frame: &mut Frame,
    size: Size,
    portrait: Option<&Handle>,
    wash: Option<&Handle>,
) {
    let manifest = tuning::manifest();
    let values = &scene.tuning;
    let font_scale = values.number(manifest, "fontScale") * scene.font_scale;
    let shift = cache.resolve(values);
    let stage = stage(size, font_scale, values.choice(manifest, "portraitStyle") == "square", shift);
    if let Some(image) = wash {
        frame.draw_image(Rectangle::new(Point::ORIGIN, size), iced::widget::canvas::Image::new(image.clone()));
    }
    let theme = scene.theme;
    let artist = if scene.artist.is_empty() { "Monet" } else { scene.artist.as_str() };
    let title = if scene.title.is_empty() { "Monet" } else { scene.title.as_str() };
    let album = if scene.album.is_empty() { "Monet" } else { scene.album.as_str() };
    let artist_p = intro(scene.time, 0.15, 1.2);
    let title_p = intro(scene.time, 0.3, 1.3);
    let rule_p = intro(scene.time, 0.5, 1.5);
    let rail_p = intro(scene.time, 0.65, 1.2);
    let audio_p = intro(scene.time, 0.8, 1.2);
    let desc_p = intro(scene.time, 0.95, 1.0);
    let portrait_p = intro(scene.time, 0.25, 1.6);
    let decor = fade(scene.time, 2.2);
    dots(frame, size, theme, decor);
    let mut y = 48.0;
    text_at(
        frame,
        artist,
        Point::new(stage.left - 30.0 * (1.0 - artist_p), y),
        stage.artist_px,
        ITALIC,
        color(theme.primary, 0.96 * artist_p),
        stage.text_width,
    );
    y += stage.artist_px * 1.3 + 8.0;
    let rule = Path::line(Point::new(stage.left, y), Point::new(stage.left, y + 56.0 * rule_p));
    frame.stroke(
        &rule,
        iced::widget::canvas::Stroke::default().with_width(1.0).with_color(color(theme.primary, 0.72 * rule_p)),
    );
    y += 56.0 + 12.0;
    let title = crate::clamp_lines(title, stage.title_px, SEMI, stage.text_width, 2);
    text_at(
        frame,
        &title,
        Point::new(stage.left - 40.0 * (1.0 - title_p), y),
        stage.title_px,
        SEMI,
        color(theme.primary, title_p),
        stage.text_width,
    );
    y += stage.title_px * 1.06 * 2.0;
    let album = album.to_uppercase();
    text_at(
        frame,
        &album,
        Point::new(stage.left, y),
        stage.album_px,
        iced::Font::DEFAULT,
        color(theme.secondary, 0.84 * title_p),
        stage.text_width,
    );
    y += stage.album_px + 18.0;
    stage_lyrics(scene, cache, frame, &stage, y, rail_p);
    if values.flag(manifest, "showDescription") {
        capsule(frame, scene, Point::new(stage.left, size.height - 72.0), desc_p);
    }
    if let Some(rect) = stage.portrait {
        draw_portrait(frame, scene, rect, portrait, portrait_p, values.choice(manifest, "portraitStyle") == "square");
        if values.flag(manifest, "showPortraitDragHanger")
            && let Some(hanger) = stage.hanger
        {
            if cache.editing {
                draw_range(frame, scene, rect, shift);
                draw_reset(frame, scene, reset_button(hanger), portrait_p);
            }
            draw_hanger(frame, scene, hanger, cache.editing, portrait_p);
        }
    }
    if values.flag(manifest, "showAudioVisualization") {
        let mode_line = values.choice(manifest, "audioStyle") == "line";
        draw_audio(frame, stage.audio, scene, mode_line, audio_p);
    }
}

impl Cache {
    /// Live shift while editing or until a just-saved value lands. Otherwise the stored offset.
    fn resolve(&mut self, values: &crate::tuning::Values) -> f32 {
        let saved = tuning::offset(values);
        if self.pending && self.live_offset.is_some_and(|live| (live - saved).abs() < 0.5) {
            self.pending = false;
            self.live_offset = None;
            return saved;
        }
        if self.editing || self.pending {
            self.live_offset.unwrap_or(saved)
        } else {
            self.live_offset = None;
            saved
        }
    }
}

/// Hanger: a click enters edit mode, a held drag moves the portrait, the next click stores the offset.
pub fn interact(
    scene: &Scene<'_>,
    cache: &mut Cache,
    size: Size,
    cursor: Option<Point>,
    pressed: Option<bool>,
) -> bool {
    let manifest = tuning::manifest();
    if !scene.tuning.flag(manifest, "showPortraitDragHanger") {
        cache.editing = false;
        cache.down = false;
        return false;
    }
    let saved = cache.resolve(&scene.tuning);
    let stage = stage(
        size,
        scene.tuning.number(manifest, "fontScale") * scene.font_scale,
        scene.tuning.choice(manifest, "portraitStyle") == "square",
        saved,
    );
    let Some(hanger) = stage.hanger else { return false };
    match pressed {
        Some(true) => press(cache, hanger, cursor),
        Some(false) => release(cache, saved),
        None => drag(cache, cursor, saved),
    }
}

fn press(cache: &mut Cache, hanger: Rectangle, cursor: Option<Point>) -> bool {
    let Some(at) = cursor else { return false };
    if cache.editing && reset_button(hanger).contains(at) {
        cache.live_offset = Some(0.0);
        cache.down = false;
        cache.grab = None;
        cache.moved = false;
        return true;
    }
    if !hanger.contains(at) {
        return false;
    }
    cache.down = true;
    cache.moved = false;
    if cache.editing {
        cache.grab = Some(at.x);
    }
    true
}

fn drag(cache: &mut Cache, cursor: Option<Point>, saved: f32) -> bool {
    if !cache.editing || !cache.down {
        return false;
    }
    let (Some(at), Some(grab)) = (cursor, cache.grab) else { return false };
    let delta = at.x - grab;
    if !cache.moved && delta.abs() <= 3.0 {
        return false;
    }
    cache.moved = true;
    let next = (cache.live_offset.unwrap_or(saved) + delta).clamp(-150.0, 0.0);
    cache.grab = Some(at.x);
    cache.live_offset = Some(next);
    true
}

fn release(cache: &mut Cache, saved: f32) -> bool {
    let down = cache.down;
    let moved = cache.moved;
    cache.down = false;
    cache.grab = None;
    cache.moved = false;
    if !down || moved {
        return false;
    }
    if cache.editing {
        let offset = cache.live_offset.unwrap_or(saved);
        cache.live_offset = Some(offset);
        cache.pending = true;
        cache.editing = false;
        remember(offset);
    } else {
        cache.editing = true;
    }
    true
}

fn reset_button(hanger: Rectangle) -> Rectangle {
    Rectangle { x: hanger.x - 8.0, y: hanger.y - 30.0, width: 22.0, height: 22.0 }
}

fn remember(offset: f32) {
    if let Ok(mut slot) = PENDING.lock() {
        *slot = Some(offset);
    }
}

static PENDING: std::sync::Mutex<Option<f32>> = std::sync::Mutex::new(None);

pub fn take_offset() -> Option<f32> {
    PENDING.lock().ok().and_then(|mut slot| slot.take())
}

fn stage(size: Size, font_scale: f32, square: bool, offset: f32) -> Stage {
    let large = motion::large_screen(size.width);
    let extra = font_scale * large;
    let active_px = motion::clamp_font(1.34, 2.75, 2.28, size.width, extra);
    let show_portrait = size.width >= 768.0;
    let portrait_w = (size.width * 0.26).clamp(210.0, 380.0 * large);
    let portrait_h = if square { portrait_w } else { portrait_w / 0.74 };
    let column = (size.width * 0.28).clamp(220.0, 430.0 * large);
    let left = if size.width >= 1024.0 { 56.0 } else { 32.0 };
    let text_width =
        if show_portrait { (size.width - column - left * 2.0).max(180.0) } else { size.width - left * 2.0 };
    let portrait = show_portrait.then(|| {
        let x = size.width - left - portrait_w + offset;
        let y = (size.height - portrait_h) / 2.0;
        Rectangle { x, y, width: portrait_w, height: portrait_h }
    });
    let hanger =
        portrait.map(|rect| Rectangle { x: rect.x + rect.width - 44.0, y: rect.y - 12.0, width: 12.0, height: 56.0 });
    Stage {
        left,
        text_width,
        portrait,
        hanger,
        audio: Rectangle { x: left, y: size.height - 48.0, width: (size.width * 0.55).min(450.0), height: 40.0 },
        active_px,
        inactive_px: motion::clamp_font(1.08, 2.0, 1.48, size.width, extra),
        translation_px: motion::clamp_font(0.94, 1.28, 1.14, size.width, extra),
        title_px: motion::clamp_font(1.45, 3.3, 2.8 * large, size.width, 1.0),
        artist_px: motion::clamp_font(1.0, 1.8, 1.8 * large, size.width, 1.0),
        album_px: 14.0 * large,
    }
}

fn stage_lyrics(scene: &Scene<'_>, cache: &mut Cache, frame: &mut Frame, stage: &Stage, top: f32, progress: f32) {
    let lines = &scene.lyrics.lines;
    let active = active_line(lines, scene.time).or_else(|| scene.lyrics.line_at(scene.time));
    let anchor = active.unwrap_or(0);
    let start = anchor.saturating_sub(2);
    let end = (anchor + 2).min(lines.len().saturating_sub(1));
    if lines.is_empty() || end < start {
        return;
    }
    let mut y = top + 20.0 * (1.0 - progress);
    for (index, line) in lines.iter().enumerate().take(end + 1).skip(start) {
        let status = if Some(index) == active {
            "active"
        } else if active.is_some_and(|active| index < active) {
            "passed"
        } else {
            "waiting"
        };
        let distance = index.abs_diff(anchor).max(1);
        let (alpha, scale) = tone(status, distance);
        let elapsed = scene.time - line.start;
        let settle =
            motion::spring(14.0, 0.0, elapsed, motion::SCROLL_STIFFNESS, motion::SCROLL_DAMPING, motion::SCROLL_MASS)
                as f32;
        let shown = motion::spring(
            scale as f64,
            if status == "active" { 1.0 } else { scale as f64 },
            elapsed,
            motion::SCALE_STIFFNESS,
            motion::SCALE_DAMPING,
            motion::SCALE_MASS,
        ) as f32;
        let px = if status == "active" { stage.active_px } else { stage.inactive_px * shown };
        let laid = cache.lines.get_or_insert(line, Size::new(stage.text_width, px), scene.font_scale, 0, || {
            std::rc::Rc::new(LineBox {
                text: line.text.clone(),
                width: measure(&line.text, px, BOLD, stage.text_width).width,
            })
        });
        if laid.text.is_empty() && laid.width == 0.0 {
            continue;
        }
        draw_line(
            frame,
            scene,
            line,
            Point::new(stage.left, y + settle),
            px,
            color(scene.theme.primary, alpha * progress),
            status == "active",
        );
        y += px * 1.25;
        if status == "active"
            && let Some(translation) = line.translation.as_deref().filter(|text| !text.is_empty())
        {
            text_at(
                frame,
                translation,
                Point::new(stage.left, y),
                stage.translation_px,
                iced::Font::DEFAULT,
                color(scene.theme.secondary, 0.7 * progress),
                stage.text_width,
            );
            y += stage.translation_px * 1.3;
        }
    }
}

fn tone(status: &str, distance: usize) -> (f32, f32) {
    if status == "active" {
        return (1.0, 1.0);
    }
    let distance = distance as f32;
    let waiting = status == "waiting";
    let alpha = if waiting {
        (0.72 - (distance - 1.0) * 0.18).clamp(0.36, 0.72)
    } else {
        (0.52 - (distance - 1.0) * 0.12).clamp(0.28, 0.52)
    };
    let scale = (0.72 * 0.9_f32.powf(distance - 1.0)).clamp(0.68, 0.92);
    (alpha, scale)
}

fn draw_line(frame: &mut Frame, scene: &Scene<'_>, line: &Line, at: Point, px: f32, ink: iced::Color, active: bool) {
    let hints = RenderHints::of(line);
    let glow = if active { motion::glow(scene.time, line.start, line.end, hints.render_end) } else { 0.0 };
    let (sung, rest) = split_sung(line, scene.time);
    if glow > 0.0 && !sung.is_empty() {
        text_at(frame, &sung, at, px, BOLD, color(scene.theme.accent, glow.min(1.0)), f32::INFINITY);
        let width = measure(&sung, px, BOLD, f32::INFINITY).width;
        text_at(frame, &rest, Point::new(at.x + width, at.y), px, BOLD, ink, f32::INFINITY);
    } else {
        text_at(frame, &line.text, at, px, if active { BOLD } else { iced::Font::DEFAULT }, ink, f32::INFINITY);
    }
}

fn split_sung(line: &Line, time: f64) -> (String, String) {
    if line.words.is_empty() {
        let count = line.text.chars().count();
        let n = if line.end <= line.start {
            0
        } else {
            ((time - line.start) / (line.end - line.start) * count as f64).clamp(0.0, count as f64) as usize
        };
        let cut = line.text.chars().take(n).collect::<String>();
        let rest = line.text.chars().skip(n).collect::<String>();
        return (cut, rest);
    }
    let mut sung = String::new();
    let mut rest = String::new();
    let mut pending = false;
    for word in &line.words {
        let target = if pending || time < word.start {
            pending = true;
            &mut rest
        } else if time >= word.end {
            &mut sung
        } else {
            let count = word.text.chars().count().max(1);
            let n = (((time - word.start) / (word.end - word.start).max(0.001)) * count as f64) as usize;
            let mut chars = word.text.chars();
            sung.extend(chars.by_ref().take(n));
            rest.extend(chars);
            pending = true;
            continue;
        };
        target.push_str(&word.text);
    }
    (sung, rest)
}

fn text_at(frame: &mut Frame, content: &str, at: Point, size: f32, font: iced::Font, color: iced::Color, width: f32) {
    if content.is_empty() || size <= 0.0 {
        return;
    }
    frame.fill_text(Text {
        content: content.to_owned(),
        position: at,
        color,
        size: size.into(),
        font,
        align_x: iced::widget::text::Alignment::Left,
        align_y: iced::alignment::Vertical::Top,
        shaping: iced::widget::text::Shaping::Advanced,
        max_width: width,
        ..Text::default()
    });
}

fn capsule(frame: &mut Frame, scene: &Scene<'_>, at: Point, progress: f32) {
    let label = scene.theme.name.as_ref();
    let width = measure(label, 12.0, iced::Font::DEFAULT, 280.0).width + 36.0;
    let rect = Rectangle { x: at.x, y: at.y, width, height: 28.0 };
    frame.fill(
        &Path::rounded_rectangle(rect.position(), rect.size(), Radius::from(14.0)),
        color(scene.theme.background, 0.18 * progress),
    );
    frame.fill(&Path::circle(Point::new(rect.x + 16.0, rect.y + 14.0), 5.0), color(scene.theme.accent, progress));
    text_at(
        frame,
        label,
        Point::new(rect.x + 28.0, rect.y + 7.0),
        12.0,
        iced::Font::DEFAULT,
        color(scene.theme.primary, 0.9 * progress),
        width,
    );
}

fn draw_portrait(
    frame: &mut Frame,
    scene: &Scene<'_>,
    rect: Rectangle,
    image: Option<&Handle>,
    progress: f32,
    square: bool,
) {
    let radius = if square { 32.0 } else { 28.0 };
    let plate = if square {
        rect
    } else {
        Rectangle { x: rect.x + 6.0, y: rect.y + 6.0, width: rect.width - 12.0, height: rect.height - 12.0 }
    };
    frame.with_save(|frame| {
        let dx = 50.0 * (1.0 - progress);
        frame.translate(Vector::new(dx, 0.0));
        if scene.theme.intensity == AnimationIntensity::Chaotic {
            let (sx, sy, degrees) = motion::portrait_sway(scene.time);
            let center = Point::new(plate.x + plate.width / 2.0, plate.y + plate.height / 2.0);
            frame.translate(Vector::new(center.x + sx, center.y + sy));
            frame.rotate(degrees.to_radians());
            frame.translate(Vector::new(-center.x, -center.y));
        }
        frame.fill(
            &Path::rounded_rectangle(plate.position(), plate.size(), Radius::from(radius)),
            color(scene.theme.primary, 0.08 * progress),
        );
        if let Some(image) = image {
            frame.draw_image(plate, iced::widget::canvas::Image::new(image.clone()).opacity(progress));
        }
        frame.stroke(
            &Path::rounded_rectangle(plate.position(), plate.size(), Radius::from(radius)),
            iced::widget::canvas::Stroke::default()
                .with_width(1.0)
                .with_color(color(scene.theme.primary, 0.1 * progress)),
        );
    });
}

fn draw_range(frame: &mut Frame, scene: &Scene<'_>, rect: Rectangle, shift: f32) {
    let guide = Rectangle {
        x: rect.x - shift - 150.0,
        y: rect.y - 12.0,
        width: rect.width + 150.0,
        height: rect.height + 24.0,
    };
    frame.stroke(
        &Path::rounded_rectangle(guide.position(), guide.size(), Radius::from(28.0)),
        iced::widget::canvas::Stroke::default().with_width(1.5).with_color(color(scene.theme.primary, 0.24)),
    );
}

fn draw_reset(frame: &mut Frame, scene: &Scene<'_>, rect: Rectangle, progress: f32) {
    let center = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    frame.fill(&Path::circle(center, rect.width / 2.0), color(scene.theme.background, 0.86 * progress));
    frame.stroke(
        &Path::circle(center, rect.width / 2.0 - 1.0),
        iced::widget::canvas::Stroke::default().with_width(1.5).with_color(color(scene.theme.primary, 0.7 * progress)),
    );
}

fn draw_hanger(frame: &mut Frame, scene: &Scene<'_>, rect: Rectangle, editing: bool, progress: f32) {
    let ink = if editing { scene.theme.accent } else { scene.theme.primary };
    frame.fill(
        &Path::rounded_rectangle(rect.position(), rect.size(), Radius::from(6.0)),
        color(scene.theme.background, 0.86 * progress),
    );
    frame.stroke(
        &Path::rounded_rectangle(rect.position(), rect.size(), Radius::from(6.0)),
        iced::widget::canvas::Stroke::default()
            .with_width(1.5)
            .with_color(color(ink, if editing { 0.8 } else { 0.2 } * progress)),
    );
}

fn dots(frame: &mut Frame, size: Size, theme: &folia_core::Theme, alpha: f32) {
    // Themes carry no lyric icons, so the floating decor is a few soft dots in the secondary colour.
    for i in 0..6 {
        let x = ((i * 127 + 43) % 80 + 10) as f32 / 100.0 * size.width;
        let y = ((i * 211 + 17) % 80 + 5) as f32 / 100.0 * size.height;
        let radius = 3.0 + (i % 3) as f32;
        frame.fill(
            &Path::circle(Point::new(x, y), radius),
            color(theme.secondary, (0.06 + (i % 4) as f32 * 0.02) * alpha),
        );
    }
}

fn draw_audio(frame: &mut Frame, rect: Rectangle, scene: &Scene<'_>, line: bool, progress: f32) {
    let ink = color(scene.theme.primary, 0.9 * progress);
    let y0 = rect.y;
    if line {
        let path = Path::new(|builder| {
            for index in 0..72 {
                let t = index as f32 / 71.0;
                let x = rect.x + t * rect.width;
                let envelope = (t * std::f32::consts::PI).sin();
                let val = if scene.preview { 0.12 + ((index as f32 * 0.35).sin() * 0.5 + 0.5) * 0.42 } else { 0.02 };
                let y = y0 + rect.height - rect.height * val * envelope;
                let point = Point::new(x, y);
                if index == 0 {
                    builder.move_to(point);
                } else {
                    builder.line_to(point);
                }
            }
        });
        frame.stroke(&path, iced::widget::canvas::Stroke::default().with_width(1.5).with_color(ink));
        return;
    }
    let gap = rect.width / 72.0;
    for index in 0..72 {
        let t = index as f32 / 71.0;
        let envelope = (t * std::f32::consts::PI).sin();
        let val = if scene.preview { 0.12 + ((index as f32 * 0.35).sin() * 0.5 + 0.5) * 0.42 } else { 0.0 };
        let height = rect.height * (0.02 + val * 0.8 * envelope) * progress;
        let x = rect.x + index as f32 * gap;
        frame.fill_rectangle(Point::new(x, y0 + rect.height - height), Size::new((gap * 0.34).max(1.0), height), ink);
    }
}
