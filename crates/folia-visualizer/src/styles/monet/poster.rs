// Monet's poster around the rail: the heading, the description capsule, the portrait with its hanger, and the
// bottom spectrum, plus the hanger's click-to-edit drag (`VisualizerMonet.tsx` 210-575).

use std::time::Instant;

use folia_core::AnimationIntensity;
use iced::widget::canvas::{self, Fill, Frame, LineCap, Path, Stroke, Style, Text, gradient};
use iced::widget::image::Handle;
use iced::{Color, Point, Rectangle, Size, Vector, border::Radius, mouse};

use super::PortraitOffset;
use super::ink;
use super::layout::{self, ITALIC, Layout, SEMIBOLD};
use super::motion::{self, intro};
use super::rail::{self, Rail};
use super::text::{self, Raster};
use super::tuning;
use crate::{Place, Scene, color};

/// The canvas state of one Monet picture: measurements, the wheel, and the hanger drag preview.
#[derive(Default)]
pub struct Cache {
    rail: Rail,
    artist: Option<(ArtistKey, Option<Raster>)>,
    pin: Pin,
    hangers: Vec<(([u32; 6], folia_core::Rgb), Handle)>,
    /// Wall clock of the latest frame, for the hanger's pulse.
    now: Option<Instant>,
}

/// What the artist raster was made for: label, size, colour, pixel density and the width it was cut to.
type ArtistKey = (String, u32, [u8; 4], u32, u32);

/// The hanger: a click enters edit mode, a drag moves the portrait, a click again leaves.
#[derive(Debug, Default)]
struct Pin {
    editing: Option<Instant>,
    hover: bool,
    down: Option<Down>,
    /// The offset shown while editing, and after a publish until the saved value catches up.
    live: Option<f32>,
    pending: bool,
}

#[derive(Debug, Clone, Copy)]
struct Down {
    grab: f32,
    from: f32,
    moved: bool,
}

impl Pin {
    fn offset(&mut self, saved: f32) -> f32 {
        if self.pending && self.live.is_some_and(|live| (live - saved).abs() < 0.5) {
            self.pending = false;
            self.live = None;
        }
        if self.editing.is_none() && !self.pending {
            self.live = None;
        }
        self.live.unwrap_or(saved)
    }
}

pub fn draw(scene: &Scene<'_>, cache: &mut Cache, frame: &mut Frame, size: Size, place: Place) {
    let manifest = tuning::manifest();
    let values = &scene.tuning;
    let saved = tuning::offset(values);
    let offset = cache.pin.offset(saved);
    let layout = layout::layout(scene, size, offset);
    let time = scene.time;
    heading(frame, scene, cache, &layout, place);
    let rail_p = intro(time, 0.65, 1.2);
    rail::draw(frame, place, scene, &layout, &mut cache.rail, rail_p, 20.0 * (1.0 - rail_p));
    if let Some(capsule) = layout.capsule {
        description(frame, scene, &layout, capsule, intro(time, 0.95, 1.0));
    }
    let square = values.choice(manifest, "portraitStyle") == "square";
    let hanger = values.flag(manifest, "showPortraitDragHanger");
    portrait(frame, scene, cache, &layout, place, square, hanger);
    if values.flag(manifest, "showAudioVisualization") {
        let progress = intro(time, 0.8, 1.2);
        let rect = Rectangle { y: layout.audio.y + 15.0 * (1.0 - progress), ..layout.audio };
        draw_audio(frame, rect, scene, values.choice(manifest, "audioStyle") == "line", progress);
    }
}

/// Artist (italic), the fading rule, the title and the album (`VisualizerMonet.tsx` 236-302).
fn heading(frame: &mut Frame, scene: &Scene<'_>, cache: &mut Cache, layout: &Layout, place: Place) {
    let theme = scene.theme;
    let time = scene.time;
    let artist_p = intro(time, 0.15, 1.2);
    if artist_p > 0.0 {
        let label = if scene.artist.trim().is_empty() { "Monet" } else { scene.artist.trim() };
        let ink = color(theme.primary, 0.96);
        let density = (place.scale * 2.0).clamp(1.0, 3.0);
        let key = (
            label.to_owned(),
            layout.artist_px.to_bits(),
            ink.into_rgba8(),
            density.to_bits(),
            layout.header_width.to_bits(),
        );
        let fresh = cache.artist.as_ref().is_none_or(|(held, _)| *held != key);
        if fresh {
            let shown = truncate(label, layout.artist_px, ITALIC, layout.header_width);
            let raster =
                text::raster_italic(&shown, layout.artist_px, layout.artist.height, iced::Font::DEFAULT, ink, density);
            if let Some(raster) = raster.as_ref().filter(|raster| raster.missing > 0) {
                tracing::warn!(missing = raster.missing, "monet artist has glyphs no font covers");
            }
            cache.artist = Some((key, raster));
        }
        if let Some((_, Some(raster))) = &cache.artist {
            let at = Point::new(
                layout.artist.x - 30.0 * (1.0 - artist_p) - raster.left,
                layout.artist.y - 10.0 * (1.0 - artist_p),
            );
            let bounds = Rectangle::new(at, Size::new(raster.width, raster.height));
            place.clip(
                frame,
                Rectangle { x: layout.artist.x - 30.0, width: layout.header_width + 30.0, ..bounds },
                |inner| {
                    inner.draw_image(bounds, canvas::Image::new(raster.handle.clone()).opacity(artist_p));
                },
            );
        }
    }
    let rule_p = intro(time, 0.5, 1.5);
    if rule_p > 0.0 {
        let rule = layout.rule;
        let length = rule.height * rule_p;
        let fill = gradient::Linear::new(rule.position(), Point::new(rule.x, rule.y + length))
            .add_stop(0.0, color(theme.primary, 0.72))
            .add_stop(1.0, color(theme.primary, 0.0));
        frame.fill_rectangle(rule.position(), Size::new(rule.width, length), Fill::from(fill));
    }
    let title_p = intro(time, 0.3, 1.3);
    if title_p <= 0.0 {
        return;
    }
    let dx = -40.0 * (1.0 - title_p);
    let title = if scene.title.trim().is_empty() { "Monet" } else { scene.title.trim() };
    let rows = title_rows(title, layout.title_px, layout.header_width);
    let line_height = layout.title_px * 1.06;
    let shadow = ink::blur_offsets(18.0, (0.0, 14.0));
    let shade = color(theme.background, ink::split_alpha(0.28 * title_p, shadow.len()));
    for (index, row) in rows.iter().enumerate() {
        let pen = ink::Line {
            content: row,
            at: Point::new(layout.title.x + dx, layout.title.y + index as f32 * line_height),
            px: layout.title_px,
            line_height,
            font: SEMIBOLD,
        };
        let band = Rectangle {
            x: 0.0,
            y: pen.at.y - 60.0,
            width: frame.width() / place.scale.max(1.0e-3) + 200.0,
            height: line_height + 120.0,
        };
        ink::masked_copies(frame, place, pen, shade, band, &ink::opaque, &shadow);
        ink::masked(frame, place, pen, color(theme.primary, title_p), band, &ink::opaque);
    }
    let album = if scene.album.trim().is_empty() { "Monet".to_owned() } else { scene.album.trim().to_uppercase() };
    let album = truncate(&album, 14.0, iced::Font::DEFAULT, layout.header_width);
    put_text(
        frame,
        &album,
        Point::new(layout.album.x + dx, layout.album.y),
        14.0,
        20.0,
        iced::Font::DEFAULT,
        color(theme.secondary, 0.84 * title_p),
    );
}

/// At most two rows of the title, the second cut with an ellipsis (`line-clamp-2`, `overflow-wrap: anywhere`).
fn title_rows(title: &str, px: f32, width: f32) -> Vec<String> {
    let rows = text::wrap(title, px, SEMIBOLD, width);
    let piece = |row: &text::Row| title.get(row.start..row.end).unwrap_or_default().trim_end().to_owned();
    match rows.as_slice() {
        [] => vec![title.to_owned()],
        [one] => vec![piece(one)],
        [first, second] => vec![piece(first), piece(second)],
        [first, second, ..] => {
            vec![
                piece(first),
                truncate(&format!("{}\u{2026}", title.get(second.start..).unwrap_or_default()), px, SEMIBOLD, width),
            ]
        }
    }
}

/// `text` cut with an ellipsis to fit `width` on one line, like `truncate`.
fn truncate(content: &str, px: f32, font: iced::Font, width: f32) -> String {
    if text::shape(content, px, font).width <= width + 0.5 {
        return content.to_owned();
    }
    let ends: Vec<usize> = content.char_indices().map(|(at, _)| at).collect();
    let cut = |count: usize| format!("{}\u{2026}", content[..ends[count]].trim_end());
    let (mut fitting, mut overflowing) = (0, ends.len());
    while overflowing - fitting > 1 {
        let middle = (fitting + overflowing) / 2;
        if text::shape(&cut(middle), px, font).width <= width + 0.5 {
            fitting = middle;
        } else {
            overflowing = middle;
        }
    }
    cut(fitting)
}

fn put_text(frame: &mut Frame, content: &str, at: Point, px: f32, line_height: f32, font: iced::Font, ink: Color) {
    if content.is_empty() || ink.a <= 0.0 {
        return;
    }
    frame.fill_text(Text {
        content: content.to_owned(),
        position: at,
        color: ink,
        size: px.into(),
        line_height: iced::widget::text::LineHeight::Absolute(line_height.into()),
        font,
        align_x: iced::widget::text::Alignment::Left,
        align_y: iced::alignment::Vertical::Top,
        shaping: iced::widget::text::Shaping::Advanced,
        max_width: f32::INFINITY,
    });
}

/// The capsule: accent dot and the theme's description or name (`VisualizerMonet.tsx` 333-353).
fn description(frame: &mut Frame, scene: &Scene<'_>, layout: &Layout, rect: Rectangle, progress: f32) {
    if progress <= 0.0 {
        return;
    }
    let theme = scene.theme;
    let origin = layout.description.center();
    let scale = 0.9 + 0.1 * progress;
    frame.with_save(|frame| {
        frame.translate(Vector::new(origin.x, origin.y + 10.0 * (1.0 - progress)));
        frame.scale(scale);
        frame.translate(Vector::new(-origin.x, -origin.y));
        let pill = Path::rounded_rectangle(rect.position(), rect.size(), Radius::from(rect.height / 2.0));
        frame.fill(&pill, color(theme.background, 0.18 * progress));
        let edge = Rectangle { x: rect.x + 0.5, y: rect.y + 0.5, width: rect.width - 1.0, height: rect.height - 1.0 };
        frame.stroke(
            &Path::rounded_rectangle(edge.position(), edge.size(), Radius::from(edge.height / 2.0)),
            Stroke::default().with_width(1.0).with_color(color(theme.primary, 0.16 * progress)),
        );
        let middle = rect.y + rect.height / 2.0;
        frame.fill(&Path::circle(Point::new(rect.x + 1.0 + 16.0 + 5.0, middle), 5.0), color(theme.accent, progress));
        let label = layout::capsule_label(scene);
        put_text(
            frame,
            &label,
            Point::new(rect.x + 1.0 + 16.0 + 10.0 + 12.0, middle - 8.0),
            12.0,
            16.0,
            iced::Font::DEFAULT,
            color(theme.primary, 0.9 * progress),
        );
    });
}

/// The portrait frame, the hanger and, while editing, the drag range with its reset button.
fn portrait(
    frame: &mut Frame,
    scene: &Scene<'_>,
    cache: &mut Cache,
    layout: &Layout,
    place: Place,
    square: bool,
    hanger: bool,
) {
    let (Some(column), Some(rect), Some(wrapper)) = (layout.column, layout.portrait, layout.wrapper) else { return };
    let time = scene.time;
    let progress = intro(time, 0.25, 1.6);
    if progress <= 0.0 {
        return;
    }
    let theme = scene.theme;
    let grow = 0.95 + 0.05 * progress;
    let center = column.center();
    let shift = Vector::new(50.0 * (1.0 - progress), 0.0);
    let moved = |r: Rectangle| Rectangle {
        x: center.x + (r.x - center.x) * grow + shift.x,
        y: center.y + (r.y - center.y) * grow,
        width: r.width * grow,
        height: r.height * grow,
    };
    let sway = if theme.intensity == AnimationIntensity::Chaotic { motion::portrait_sway(time) } else { (0.0, 0.0) };
    let swayed = |r: Rectangle| Rectangle { x: r.x + sway.0, y: r.y + sway.1, ..moved(r) };
    let box_ = swayed(rect);
    if cache.pin.editing.is_some() {
        let left = if square { wrapper.x - wrapper.width * 0.35135 } else { wrapper.x } - 150.0;
        let range = moved(Rectangle {
            x: left,
            y: wrapper.y - 12.0,
            width: wrapper.x + wrapper.width - left,
            height: wrapper.height + 24.0,
        });
        draw_range(frame, scene, range, grow);
    }
    if square {
        let radius = 32.0 * grow;
        ink::box_shadow(frame, box_, radius, 36.0 * grow, 80.0 * grow, color(theme.background, 0.45 * progress));
        ink::box_shadow(frame, box_, radius, 20.0 * grow, 42.0 * grow, color(theme.accent, 0.22 * progress));
        let ring = Rectangle { x: box_.x - 0.5, y: box_.y - 0.5, width: box_.width + 1.0, height: box_.height + 1.0 };
        frame.stroke(
            &Path::rounded_rectangle(ring.position(), ring.size(), Radius::from(radius + 0.5)),
            Stroke::default().with_width(1.0).with_color(color(theme.primary, 0.06 * progress)),
        );
        plate(frame, scene, place, box_, radius, progress);
    } else {
        let radius = 34.4 * grow;
        ink::box_shadow(frame, box_, radius, 24.0 * grow, 60.0 * grow, color(theme.background, 0.35 * progress));
        frame.fill(
            &Path::rounded_rectangle(box_.position(), box_.size(), Radius::from(radius)),
            color(theme.background, 0.15 * progress),
        );
        let edge = Rectangle { x: box_.x + 0.5, y: box_.y + 0.5, width: box_.width - 1.0, height: box_.height - 1.0 };
        frame.stroke(
            &Path::rounded_rectangle(edge.position(), edge.size(), Radius::from(radius - 0.5)),
            Stroke::default().with_width(1.0).with_color(color(theme.primary, 0.1 * progress)),
        );
        let inset = 7.0 * grow;
        let inner = Rectangle {
            x: box_.x + inset,
            y: box_.y + inset,
            width: box_.width - inset * 2.0,
            height: box_.height - inset * 2.0,
        };
        plate(frame, scene, place, inner, 29.6 * grow, progress);
    }
    if hanger && let Some(pin) = layout.hanger {
        draw_hanger(frame, scene, cache, swayed(pin), progress, grow);
    }
}

/// `primary 0.08` under the cover, and the cover itself `object-cover`, cut to the rounded corners.
fn plate(frame: &mut Frame, scene: &Scene<'_>, place: Place, rect: Rectangle, radius: f32, progress: f32) {
    frame.fill(
        &Path::rounded_rectangle(rect.position(), rect.size(), Radius::from(radius)),
        color(scene.theme.primary, 0.08 * progress),
    );
    let Some(handle) = scene.portrait.as_ref() else { return };
    let (width, height) = match handle {
        Handle::Rgba { width, height, .. } => (*width as f32, *height as f32),
        _ => (1.0, 1.0),
    };
    let aspect = width / height.max(1.0);
    let cover = if aspect > rect.width / rect.height.max(1.0) {
        let w = rect.height * aspect;
        Rectangle { x: rect.x + (rect.width - w) / 2.0, width: w, ..rect }
    } else {
        let h = rect.width / aspect.max(1.0e-3);
        Rectangle { y: rect.y + (rect.height - h) / 2.0, height: h, ..rect }
    };
    let mut image = canvas::Image::new(handle.clone()).opacity(progress);
    image.border_radius = Radius::from(radius);
    place.clip(frame, rect, |inner| inner.draw_image(cover, image));
}

/// The dashed drag range and its reset button (`VisualizerMonet.tsx` 377-408).
fn draw_range(frame: &mut Frame, scene: &Scene<'_>, range: Rectangle, grow: f32) {
    let theme = scene.theme;
    let edge = Rectangle { x: range.x + 1.0, y: range.y + 1.0, width: range.width - 2.0, height: range.height - 2.0 };
    frame.stroke(
        &Path::rounded_rectangle(edge.position(), edge.size(), Radius::from(39.0 * grow)),
        Stroke { line_dash: canvas::LineDash { segments: &[6.0, 6.0], offset: 0 }, ..Stroke::default() }
            .with_width(2.0)
            .with_color(color(theme.primary, 0.24)),
    );
    let button = reset_button(range);
    let center = button.center();
    ink::box_shadow(frame, button, button.width / 2.0, 4.0, 12.0, Color { a: 0.12, ..Color::BLACK });
    frame.fill(&Path::circle(center, button.width / 2.0), color(theme.background, 0.86));
    frame.stroke(
        &Path::circle(center, button.width / 2.0 - 0.5),
        Stroke::default().with_width(1.0).with_color(color(theme.primary, 0.16)),
    );
    rotate_ccw(frame, center, color(theme.primary, 1.0));
}

/// Lucide `rotate-ccw` at 14px: `M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8` and `M3 3v5h5`.
fn rotate_ccw(frame: &mut Frame, center: Point, ink: Color) {
    let unit = 14.0 / 24.0;
    let at = |x: f32, y: f32| Point::new(center.x + (x - 12.0) * unit, center.y + (y - 12.0) * unit);
    let arc = Path::new(|path| {
        path.arc(canvas::path::Arc {
            center: at(12.0, 12.0),
            radius: 9.0 * unit,
            start_angle: iced::Radians(std::f32::consts::PI),
            end_angle: iced::Radians(std::f32::consts::PI * 2.5),
        });
        path.move_to(at(12.0, 3.0));
        path.quadratic_curve_to(at(8.1, 3.1), at(5.26, 5.74));
        path.line_to(at(3.0, 8.0));
        path.move_to(at(3.0, 3.0));
        path.line_to(at(3.0, 8.0));
        path.line_to(at(8.0, 8.0));
    });
    frame.stroke(&arc, Stroke::default().with_width(2.0 * unit).with_color(ink).with_line_cap(canvas::LineCap::Round));
}

/// The 12×56 hanger: a pulsing accent edge while editing, a faint one on hover (`VisualizerMonet.tsx` 457-513).
/// Drawn as an image: canvas meshes sit under images, and the hanger hangs over the cover.
fn draw_hanger(frame: &mut Frame, scene: &Scene<'_>, cache: &mut Cache, rect: Rectangle, progress: f32, grow: f32) {
    let theme = scene.theme;
    let pulse = cache.pin.editing.map(|since| {
        let elapsed = cache.now.map_or(0.0, |now| now.saturating_duration_since(since).as_secs_f64());
        let level =
            crate::keyframes(&[0.0, 1.0, 0.0], &[0.0, 0.5, 1.0], elapsed % 1.5, 1.5, crate::Ease::IN_OUT) as f32;
        (level * 16.0).round() / 16.0
    });
    let look = match pulse {
        Some(level) => Hanger {
            edge: Some(color(theme.accent, 0.3 + 0.7 * level)),
            glow: Some((2.0 + 8.0 * level, color(theme.accent, 0.2 + 0.45 * level))),
        },
        None if cache.pin.hover => Hanger { edge: Some(color(theme.primary, 0.36)), glow: None },
        None => Hanger { edge: None, glow: None },
    };
    let key = (look.key(), theme.background);
    let handle = match cache.hangers.iter().find(|(held, _)| *held == key) {
        Some((_, handle)) => handle.clone(),
        None => {
            let handle = look.paint(color(theme.background, 0.86));
            if cache.hangers.len() >= 24 {
                cache.hangers.remove(0);
            }
            cache.hangers.push((key, handle.clone()));
            handle
        }
    };
    let margin = HANGER_MARGIN * grow;
    let bounds = Rectangle {
        x: rect.x - margin,
        y: rect.y - margin,
        width: rect.width + margin * 2.0,
        height: rect.height + margin * 2.0,
    };
    frame.draw_image(bounds, canvas::Image::new(handle).opacity(progress));
}

const HANGER_MARGIN: f32 = 40.0;
const HANGER_DENSITY: f32 = 2.0;

/// The hanger's look: its border colour and, while editing, the accent halo.
struct Hanger {
    edge: Option<Color>,
    glow: Option<(f32, Color)>,
}

impl Hanger {
    fn key(&self) -> [u32; 6] {
        let pack = |color: Option<Color>| color.map_or(0, |color| u32::from_le_bytes(color.into_rgba8()));
        [
            pack(self.edge),
            self.glow.map_or(0, |(blur, _)| blur.to_bits()),
            pack(self.glow.map(|(_, color)| color)),
            0,
            0,
            0,
        ]
    }

    /// The pill with `0 8px 18px #0000003d`, the halo, the `fill` and a 1.5px edge, straight-alpha RGBA.
    fn paint(&self, fill: Color) -> Handle {
        let (w, h) = (12.0 + HANGER_MARGIN * 2.0, 56.0 + HANGER_MARGIN * 2.0);
        let (pw, ph) = ((w * HANGER_DENSITY) as u32, (h * HANGER_DENSITY) as u32);
        let pill = Rectangle { x: HANGER_MARGIN, y: HANGER_MARGIN, width: 12.0, height: 56.0 };
        let blurred = |x: f32, y: f32, rect: Rectangle, sigma: f32| {
            let axis = |at: f32, from: f32, to: f32| ink::edge((from - at) / sigma) - ink::edge((to - at) / sigma);
            axis(x, rect.x, rect.x + rect.width) * axis(y, rect.y, rect.y + rect.height)
        };
        let mut pixels = Vec::with_capacity((pw * ph * 4) as usize);
        for row in 0..ph {
            for col in 0..pw {
                let (x, y) = ((col as f32 + 0.5) / HANGER_DENSITY, (row as f32 + 0.5) / HANGER_DENSITY);
                let shadow = blurred(x, y, Rectangle { y: pill.y + 8.0, ..pill }, 9.0) * 0.24;
                let mut out = [0.0_f32; 4];
                over(&mut out, Color::BLACK, shadow);
                if let Some((blur, halo)) = self.glow {
                    over(&mut out, halo, blurred(x, y, pill, blur / 2.0) * halo.a);
                }
                let distance = rounded_distance(x, y, pill, 6.0);
                over(&mut out, fill, (0.5 - distance).clamp(0.0, 1.0) * fill.a);
                if let Some(edge) = self.edge {
                    let ring = (0.5 - distance).clamp(0.0, 1.0) * (distance + 1.5 + 0.5).clamp(0.0, 1.0);
                    over(&mut out, edge, ring * edge.a);
                }
                let alpha = out[3];
                let channel =
                    |value: f32| if alpha > 0.0 { (value / alpha * 255.0).round().clamp(0.0, 255.0) as u8 } else { 0 };
                pixels.extend([channel(out[0]), channel(out[1]), channel(out[2]), (alpha * 255.0).round() as u8]);
            }
        }
        Handle::from_rgba(pw, ph, pixels)
    }
}

/// Premultiplied source-over of `color` at coverage `alpha`.
fn over(out: &mut [f32; 4], color: Color, alpha: f32) {
    let keep = 1.0 - alpha;
    out[0] = color.r * alpha + out[0] * keep;
    out[1] = color.g * alpha + out[1] * keep;
    out[2] = color.b * alpha + out[2] * keep;
    out[3] = alpha + out[3] * keep;
}

/// Signed distance from `(x, y)` to a rounded rectangle, negative inside.
fn rounded_distance(x: f32, y: f32, rect: Rectangle, radius: f32) -> f32 {
    let (cx, cy) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    let qx = (x - cx).abs() - rect.width / 2.0 + radius;
    let qy = (y - cy).abs() - rect.height / 2.0 + radius;
    (qx.max(0.0).hypot(qy.max(0.0))) + qx.max(qy).min(0.0) - radius
}

fn reset_button(range: Rectangle) -> Rectangle {
    Rectangle { x: range.x + range.width - 32.0, y: range.y - 40.0, width: 32.0, height: 32.0 }
}

fn edit_range(scene: &Scene<'_>, layout: &Layout) -> Option<Rectangle> {
    let wrapper = layout.wrapper?;
    let square = scene.tuning.choice(tuning::manifest(), "portraitStyle") == "square";
    let left = if square { wrapper.x - wrapper.width * 0.35135 } else { wrapper.x } - 150.0;
    Some(Rectangle {
        x: left,
        y: wrapper.y - 12.0,
        width: wrapper.x + wrapper.width - left,
        height: wrapper.height + 24.0,
    })
}

/// Pointer and wheel input on a live stage. The drag preview stays in the canvas state; letting go publishes one
/// `PortraitOffset`.
pub fn update<Message: From<PortraitOffset>>(
    scene: &Scene<'_>,
    cache: &mut Cache,
    event: &canvas::Event,
    size: Size,
    at: Option<Point>,
) -> Option<canvas::Action<Message>> {
    let manifest = tuning::manifest();
    let saved = tuning::offset(&scene.tuning);
    let offset = cache.pin.offset(saved);
    let layout = layout::layout(scene, size, offset);
    let hanger = layout.hanger.filter(|_| scene.tuning.flag(manifest, "showPortraitDragHanger"));
    if hanger.is_none() {
        cache.pin = Pin::default();
    }
    match event {
        canvas::Event::Window(iced::window::Event::RedrawRequested(now)) => {
            cache.now = Some(*now);
            if cache.pin.editing.is_some() || rail::settling(&cache.rail, *now) {
                return Some(canvas::Action::request_redraw());
            }
            rail::release_due(&cache.rail).map(canvas::Action::request_redraw_at)
        }
        canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
            if !at.is_some_and(|at| rail::over(&layout, at)) {
                return None;
            }
            let (lines, pixels) = match delta {
                mouse::ScrollDelta::Lines { y, .. } => (-*y, 0.0),
                mouse::ScrollDelta::Pixels { y, .. } => (0.0, -*y),
            };
            rail::wheel(&mut cache.rail, scene, lines, pixels, Instant::now())
                .then(|| canvas::Action::request_redraw().and_capture())
        }
        canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
            let (hanger, at) = (hanger?, at?);
            let pin = &mut cache.pin;
            if pin.editing.is_some()
                && edit_range(scene, &layout).map(reset_button).is_some_and(|button| button.contains(at))
            {
                pin.live = Some(0.0);
                return Some(canvas::Action::request_redraw().and_capture());
            }
            if !hanger.contains(at) {
                return None;
            }
            pin.down = Some(Down { grab: at.x, from: offset, moved: false });
            Some(canvas::Action::request_redraw().and_capture())
        }
        canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
            let pin = &mut cache.pin;
            let hover = hanger.zip(at).is_some_and(|(hanger, at)| hanger.contains(at));
            let hovered = hover != pin.hover;
            pin.hover = hover;
            match (pin.down.as_mut(), pin.editing, at) {
                (Some(down), Some(_), Some(at)) if down.moved || (at.x - down.grab).abs() > 3.0 => {
                    down.moved = true;
                    pin.live = Some((down.from + at.x - down.grab).clamp(-150.0, 0.0));
                    Some(canvas::Action::request_redraw().and_capture())
                }
                _ => hovered.then(canvas::Action::request_redraw),
            }
        }
        canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            let pin = &mut cache.pin;
            let down = pin.down.take()?;
            let finish = |pin: &mut Pin| {
                let last = pin.live.unwrap_or(saved).round();
                if (last - saved).abs() < 0.5 {
                    return canvas::Action::request_redraw().and_capture();
                }
                pin.live = Some(last);
                pin.pending = true;
                canvas::Action::publish(Message::from(PortraitOffset(last))).and_capture()
            };
            if down.moved {
                return Some(finish(pin));
            }
            if pin.editing.take().is_some() {
                return Some(finish(pin));
            }
            pin.editing = Some(cache.now.unwrap_or_else(Instant::now));
            pin.live = Some(offset);
            Some(canvas::Action::request_redraw().and_capture())
        }
        _ => None,
    }
}

/// A pointer over the hanger, or its reset button while editing.
pub fn interaction(scene: &Scene<'_>, cache: &Cache, size: Size, at: Option<Point>) -> mouse::Interaction {
    let Some(at) = at else { return mouse::Interaction::None };
    if !scene.tuning.flag(tuning::manifest(), "showPortraitDragHanger") {
        return mouse::Interaction::None;
    }
    let saved = tuning::offset(&scene.tuning);
    let layout = layout::layout(scene, size, cache.pin.live.unwrap_or(saved));
    let over_reset = cache.pin.editing.is_some()
        && edit_range(scene, &layout).map(reset_button).is_some_and(|button| button.contains(at));
    if over_reset || layout.hanger.is_some_and(|hanger| hanger.contains(at)) {
        mouse::Interaction::Pointer
    } else {
        mouse::Interaction::None
    }
}

/// `audioPower` (0–255), the five bands over 255 as the original reads them, and the raw bins while playing.
fn playback_spectrum<'a>(scene: &Scene<'a>) -> (f64, [f64; 5], &'a [u8]) {
    let audio = scene.audio;
    let unit = |value: f32| f64::from(value) / 255.0;
    let bands = [unit(audio.bass), unit(audio.low_mid), unit(audio.mid), unit(audio.vocal), unit(audio.treble)];
    let raw: &[u8] = if audio.has_spectrum { &audio.spectrum } else { &[] };
    (f64::from(audio.power), bands, raw)
}

fn draw_audio(frame: &mut Frame, rect: Rectangle, scene: &Scene<'_>, line: bool, progress: f32) {
    if scene.preview {
        draw_static_audio(frame, rect, scene, line, progress);
    } else {
        draw_live_audio(frame, rect, scene, line, progress);
    }
}

/// `drawStaticBars`: the preview and the workbench. The line stroke is 2px; the 1.5 set above it is replaced.
fn draw_static_audio(frame: &mut Frame, rect: Rectangle, scene: &Scene<'_>, line: bool, progress: f32) {
    let y0 = rect.y;
    let height = rect.height;
    if line {
        let points = static_points(rect);
        let fill = gradient::Linear::new(Point::new(rect.x, y0), Point::new(rect.x, y0 + height))
            .add_stop(0.0, color(scene.theme.primary, 0.24 * progress))
            .add_stop(1.0, color(scene.theme.primary, 0.0));
        frame.fill(&area_under(&points, rect), Fill::from(fill));
        frame.stroke(&curve(&points), stroke(color(scene.theme.primary, 0.96 * progress), 2.0));
        return;
    }
    let gap = rect.width / 72.0;
    let ink = color(scene.theme.primary, 0.9 * progress);
    for index in 0..72 {
        let envelope = (index as f32 / 71.0 * std::f32::consts::PI).sin();
        let val = 0.12 + ((index as f32 * 0.35).sin() * 0.5 + 0.5) * 0.42;
        let bar = height * (0.02 + val * 0.8 * envelope);
        let x = rect.x + index as f32 * gap + gap * 0.14;
        frame.fill_rectangle(Point::new(x, y0 + height - bar), Size::new((gap * 0.34).max(1.35), bar), ink);
    }
}

/// The playing spectrum. `t` is the playback position in milliseconds, so a pause holds the frame.
fn draw_live_audio(frame: &mut Frame, rect: Rectangle, scene: &Scene<'_>, line: bool, progress: f32) {
    let (power, bands, raw) = playback_spectrum(scene);
    let millis = scene.time * 1000.0;
    let level = super::spectrum::energy(power);
    let y0 = rect.y;
    let height = f64::from(rect.height);
    let across = gradient::Linear::new(Point::new(rect.x, y0), Point::new(rect.x + rect.width, y0))
        .add_stop(0.0, color(scene.theme.primary, 0.72 * progress))
        .add_stop(0.5, color(scene.theme.primary, 0.94 * progress))
        .add_stop(1.0, color(scene.theme.primary, 0.72 * progress));
    if line {
        let points = (0..72)
            .map(|index| {
                let x = rect.x + index as f32 / 71.0 * rect.width;
                let at = index as f64 / 71.0;
                let band = band_at(bands, raw, at);
                let amplitude = level * 0.04 + band * 0.8 + super::spectrum::wave(index, millis) * 0.04;
                let y = y0 + rect.height - (height * amplitude * super::spectrum::envelope(index)).max(0.0) as f32;
                (x, y)
            })
            .collect::<Vec<_>>();
        let down = gradient::Linear::new(Point::new(rect.x, y0), Point::new(rect.x, y0 + rect.height))
            .add_stop(0.0, color(scene.theme.primary, 0.24 * progress))
            .add_stop(1.0, color(scene.theme.primary, 0.0));
        frame.fill(&area_under(&points, rect), Fill::from(down));
        frame.stroke(&curve(&points), stroke_gradient(across, 2.0));
        return;
    }
    let gap = rect.width / 72.0;
    for index in 0..72 {
        let band = band_at(bands, raw, index as f64 / 71.0);
        let bar = super::spectrum::active_bar_height(
            height,
            level,
            band,
            super::spectrum::pulse(index, millis),
            super::spectrum::envelope(index),
        ) as f32;
        let x = rect.x + index as f32 * gap + gap * 0.14;
        frame.fill_rectangle(
            Point::new(x, y0 + rect.height - bar),
            Size::new((gap * 0.34).max(1.35), bar),
            Fill::from(across),
        );
    }
}

fn band_at(bands: [f64; 5], raw: &[u8], at: f64) -> f64 {
    if raw.len() > 6 {
        super::spectrum::sample_raw_spectrum_profile(raw, at)
    } else {
        super::spectrum::sample_spectrum_profile(bands, at)
    }
}

fn static_points(rect: Rectangle) -> Vec<(f32, f32)> {
    (0..72)
        .map(|index| {
            let x = rect.x + index as f32 / 71.0 * rect.width;
            let envelope = (index as f32 / 71.0 * std::f32::consts::PI).sin();
            let val = 0.12 + ((index as f32 * 0.35).sin() * 0.5 + 0.5) * 0.42;
            let y = rect.y + rect.height - rect.height * val * envelope;
            (x, y)
        })
        .collect()
}

fn curve(points: &[(f32, f32)]) -> Path {
    Path::new(|builder| {
        let Some((first, rest)) = points.split_first() else { return };
        builder.move_to(Point::new(first.0, first.1));
        for (index, point) in rest.iter().enumerate() {
            let previous = if index == 0 { first } else { &points[index] };
            let mid = Point::new((previous.0 + point.0) / 2.0, (previous.1 + point.1) / 2.0);
            builder.quadratic_curve_to(Point::new(previous.0, previous.1), mid);
        }
        if let Some(last) = points.last() {
            builder.line_to(Point::new(last.0, last.1));
        }
    })
}

fn area_under(points: &[(f32, f32)], rect: Rectangle) -> Path {
    Path::new(|builder| {
        let Some((first, rest)) = points.split_first() else { return };
        builder.move_to(Point::new(first.0, first.1));
        for (index, point) in rest.iter().enumerate() {
            let previous = if index == 0 { first } else { &points[index] };
            let mid = Point::new((previous.0 + point.0) / 2.0, (previous.1 + point.1) / 2.0);
            builder.quadratic_curve_to(Point::new(previous.0, previous.1), mid);
        }
        if let Some(last) = points.last() {
            builder.line_to(Point::new(last.0, last.1));
            builder.line_to(Point::new(rect.x + rect.width, rect.y + rect.height));
            builder.line_to(Point::new(rect.x, rect.y + rect.height));
            builder.close();
        }
    })
}

fn stroke(color: iced::Color, width: f32) -> Stroke<'static> {
    Stroke::default().with_color(color).with_width(width).with_line_cap(LineCap::Round)
}

fn stroke_gradient(fill: gradient::Linear, width: f32) -> Stroke<'static> {
    Stroke { style: Style::Gradient(fill.into()), width, line_cap: LineCap::Round, ..Stroke::default() }
}
