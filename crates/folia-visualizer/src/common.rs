// Common background geometry from GeometricBackground.tsx and GeometricBackground.css.
// The original draws a fresh Math.random layout whenever the seed changes. This layout is the
// same function of the seed, so a song keeps one picture across reloads.

use std::cell::RefCell;
use std::marker::PhantomData;

use folia_core::{AudioFrame, Rgb, Theme};
use iced::widget::canvas::{self, Frame, Geometry, Path};
use iced::{Color, Element, Length, Point, Radians, Rectangle, Renderer, Vector, mouse};

use crate::Background;
use crate::anim::Ease;
use crate::clock;

const SHAPES: usize = 15;
const PARTICLES: usize = 20;

#[derive(Clone, Copy)]
struct Shape {
    icon: Option<f64>,
    kind: u8,
    x: f32,
    y: f32,
    size: f32,
    duration: f64,
    delay: f64,
    opacity: f32,
    reverse: bool,
    filled: bool,
    rotation: f32,
}

#[derive(Clone, Copy)]
struct Particle {
    size: f32,
    x: f32,
    y: f32,
    opacity: f32,
    duration: f64,
    delay: f64,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 11) as f64 / ((1u64 << 53) as f64)
    }
}

fn rng(seed: &str) -> Rng {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in seed.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Rng(hash | 1)
}

fn shapes(seed: &str) -> [Shape; SHAPES] {
    let mut rng = rng(seed);
    let mut icon_slots = 0;
    std::array::from_fn(|_| {
        let icon = icon_slots < 6 && rng.next() > 0.7;
        let icon_slot = if icon {
            icon_slots += 1;
            Some(rng.next())
        } else {
            None
        };
        let kind = (rng.next() * 4.0).floor() as u8;
        let duration = if icon { 20.0 + rng.next() * 20.0 } else { 30.0 + rng.next() * 30.0 };
        Shape {
            icon: icon_slot,
            kind,
            x: (rng.next() * 100.0) as f32,
            y: (rng.next() * 100.0) as f32,
            size: (40.0 + rng.next() * 100.0) as f32,
            duration,
            delay: rng.next() * 5.0,
            opacity: (0.11 + rng.next() * 0.08) as f32,
            reverse: rng.next() > 0.5,
            filled: rng.next() < 0.3,
            rotation: (rng.next() * 360.0) as f32,
        }
    })
}

fn particles(seed: &str) -> [Particle; PARTICLES] {
    let mut rng = rng(&format!("{seed}:particles"));
    std::array::from_fn(|_| Particle {
        size: (rng.next() * 4.0 + 1.0) as f32,
        x: (rng.next() * 100.0) as f32,
        y: (rng.next() * 100.0) as f32,
        opacity: (rng.next() * 0.3) as f32,
        duration: 15.0 + rng.next() * 20.0,
        delay: rng.next() * 10.0,
    })
}

/// Translation and rotation of one drift loop. Linear, matching `geometric-drift`.
pub fn drift(delay: f64, duration: f64, reverse: bool, rotation: f32, time: f64) -> (f32, f32, f32) {
    let dir = if reverse { -1.0 } else { 1.0 };
    let (tx, ty, turn) = if time <= delay {
        (-15.0, 30.0, 0.0)
    } else {
        let phase = ((time - delay) % duration) / duration;
        let (from, to, t) = if phase < 0.5 {
            ((-15.0, 30.0, 0.0), (15.0, -30.0, 180.0), phase / 0.5)
        } else {
            ((15.0, -30.0, 180.0), (-15.0, 30.0, 360.0), (phase - 0.5) / 0.5)
        };
        (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t, from.2 + (to.2 - from.2) * t)
    };
    (tx as f32 * dir, ty as f32 * dir, rotation + turn as f32)
}

fn band_scale(value: f32) -> f32 {
    let t = ((value - 10.0) / 190.0).clamp(0.0, 1.0);
    0.95 + t * 0.50
}

fn scale_for(kind: u8, audio: &AudioFrame) -> f32 {
    let band = match kind {
        0 => audio.bass,
        1 => audio.low_mid,
        2 => audio.mid,
        3 => audio.treble,
        _ => audio.power,
    };
    band_scale(band)
}

/// One step of the framer-motion spring used for shape scale (`stiffness: 300`, `damping: 30`, mass 1).
///
/// The target is held for `dt`. The step is the closed form of that oscillator, so a dropped frame
/// stays on the same curve as several short ones instead of blowing up.
pub fn follow_spring(current: f32, velocity: f32, target: f32, dt: f32) -> (f32, f32) {
    if dt <= 0.0 {
        return (current, velocity);
    }
    let omega = 300.0_f64.sqrt();
    let zeta = 30.0 / (2.0 * omega);
    let t = f64::from(dt);
    let offset = f64::from(current - target);
    let v0 = f64::from(velocity);
    let decay = (-zeta * omega * t).exp();
    let omega_d = omega * (1.0 - zeta * zeta).sqrt();
    let (cos, sin) = ((omega_d * t).cos(), (omega_d * t).sin());
    let swing = (v0 + zeta * omega * offset) / omega_d;
    let x = decay * (offset * cos + swing * sin);
    let v = -zeta * omega * x + decay * omega_d * (-offset * sin + swing * cos);
    ((f64::from(target) + x) as f32, v as f32)
}

#[derive(Clone, Copy)]
struct Ink {
    ground: Color,
    secondary: Color,
    accent: Color,
}

/// What the shared background draws for one frame. `publish` hands the picture to the playing page's glass blur.
pub struct Ground<'a> {
    pub mode: Background,
    pub seed: &'a str,
    pub time: f64,
    pub theme: &'a Theme,
    pub audio: &'a AudioFrame,
    pub shapes: bool,
    pub vignette: bool,
    pub publish: bool,
    pub paused: bool,
    /// The playing page. A latent background with `dynamic_only` stays still everywhere else.
    pub player: bool,
    /// Representative colours of the current cover, heaviest first. Empty uses the theme.
    pub cover: &'a [folia_core::Rgb],
    pub latent: crate::latent::Tuning,
    /// The app's light mode. Sora's field is white then, and black otherwise.
    pub daylight: bool,
    pub sora: crate::sora::Tuning,
    pub monet: crate::monet_background::Tuning,
    pub nomand: crate::nomand::Tuning,
    pub picture: Option<&'a iced::widget::image::Handle>,
}

/// The shared background. Styles that hide it do not mount this.
pub fn background_view<'a, Message: 'a>(ground: Ground<'a>) -> Element<'a, Message> {
    match ground.mode {
        Background::Common => iced::widget::canvas(Common { ground, _message: PhantomData })
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        Background::Latent => crate::latent::view(ground),
        Background::Sora => crate::sora::view(ground),
        Background::Monet => crate::image_background::view(ground),
        Background::Nomand => crate::image_background::view(ground),
    }
}

struct Common<'a, Message> {
    ground: Ground<'a>,
    _message: PhantomData<Message>,
}

#[derive(Default)]
struct State(RefCell<Motion>);

impl<Message> canvas::Program<Message> for Common<'_, Message> {
    type State = State;

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut motion = state.0.borrow_mut();
        let now = clock::wall_ms();
        motion.advance(&self.ground, now);
        let seconds = motion.drift_seconds(now, self.ground.paused);
        let ink = motion.ink(self.ground.theme, now);
        let scales = motion.scales;
        drop(motion);
        let geometry = picture(renderer, bounds.size(), &self.ground, seconds, ink, scales);
        if self.ground.publish {
            offer_blur(picture(renderer, bounds.size(), &self.ground, seconds, ink, scales));
        }
        vec![geometry]
    }
}

thread_local! {
    static BLUR_UNDER: RefCell<Vec<Geometry>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn offer_blur(geometry: Geometry) {
    BLUR_UNDER.with(|slot| {
        let mut slot = slot.borrow_mut();
        slot.clear();
        slot.push(geometry);
    });
}

pub(crate) fn take_blur() -> Vec<Geometry> {
    BLUR_UNDER.with(|slot| std::mem::take(&mut *slot.borrow_mut()))
}

#[derive(Default)]
struct Motion {
    seed: Option<u64>,
    scales: [f32; SHAPES],
    velocity: [f32; SHAPES],
    last_ms: f64,
    springs_ready: bool,
    drift_origin: f64,
    drift_held: bool,
    drift_at: f64,
    color_ready: bool,
    blend_start: f64,
    from_bg: [f32; 3],
    to_bg: [f32; 3],
    from_secondary: [f32; 3],
    to_secondary: [f32; 3],
    from_accent: [f32; 3],
    to_accent: [f32; 3],
}

impl Motion {
    fn advance(&mut self, ground: &Ground<'_>, now: f64) {
        let salt = hash_seed(ground.seed);
        if self.seed != Some(salt) {
            self.seed = Some(salt);
            self.springs_ready = false;
            self.drift_origin = now;
            self.drift_held = false;
        }
        let targets = targets(ground.seed, ground.audio, &ground.theme.lyrics_icons);
        if !self.springs_ready {
            self.scales = targets;
            self.velocity = [0.0; SHAPES];
            self.last_ms = now;
            self.springs_ready = true;
            return;
        }
        let dt = ((now - self.last_ms) / 1000.0).max(0.0) as f32;
        self.last_ms = now;
        if dt == 0.0 {
            return;
        }
        for (scale, (velocity, target)) in self.scales.iter_mut().zip(self.velocity.iter_mut().zip(targets)) {
            let (next, next_velocity) = follow_spring(*scale, *velocity, target, dt);
            *scale = next;
            *velocity = next_velocity;
        }
    }

    /// CSS animation clock. Pausing holds the loops; resuming continues from that frame.
    fn drift_seconds(&mut self, now: f64, paused: bool) -> f64 {
        if paused {
            if !self.drift_held {
                self.drift_at = (now - self.drift_origin).max(0.0) / 1000.0;
                self.drift_held = true;
            }
            return self.drift_at;
        }
        if self.drift_held {
            self.drift_origin = now - self.drift_at * 1000.0;
            self.drift_held = false;
        }
        (now - self.drift_origin).max(0.0) / 1000.0
    }

    /// Theme colours glide over one second, `cubic-bezier(0.25, 0.1, 0.25, 1)`. The first frame snaps.
    fn ink(&mut self, theme: &Theme, now: f64) -> Ink {
        let bg = channels(theme.background);
        let secondary = channels(theme.secondary);
        let accent = channels(theme.accent);
        if !self.color_ready {
            self.from_bg = bg;
            self.to_bg = bg;
            self.from_secondary = secondary;
            self.to_secondary = secondary;
            self.from_accent = accent;
            self.to_accent = accent;
            self.blend_start = now;
            self.color_ready = true;
        } else if !same(self.to_bg, bg) || !same(self.to_secondary, secondary) || !same(self.to_accent, accent) {
            let (shown_bg, shown_secondary, shown_accent) = self.shown(now);
            self.from_bg = shown_bg;
            self.from_secondary = shown_secondary;
            self.from_accent = shown_accent;
            self.to_bg = bg;
            self.to_secondary = secondary;
            self.to_accent = accent;
            self.blend_start = now;
        }
        let (bg, secondary, accent) = self.shown(now);
        Ink { ground: Color::from_rgba(bg[0], bg[1], bg[2], 1.0), secondary: solid(secondary), accent: solid(accent) }
    }

    fn shown(&self, now: f64) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let elapsed = (now - self.blend_start).max(0.0);
        let t = if elapsed >= 1000.0 { 1.0 } else { THEME_EASE.at(elapsed / 1000.0) as f32 };
        (
            mix(self.from_bg, self.to_bg, t),
            mix(self.from_secondary, self.to_secondary, t),
            mix(self.from_accent, self.to_accent, t),
        )
    }
}

const THEME_EASE: Ease = Ease::new(0.25, 0.1, 0.25, 1.0);

fn picture(
    renderer: &Renderer,
    size: iced::Size,
    ground: &Ground<'_>,
    time: f64,
    ink: Ink,
    scales: [f32; SHAPES],
) -> Geometry {
    let mut frame = Frame::new(renderer, size);
    paint(
        &mut frame,
        size,
        Coat {
            seed: ground.seed,
            time,
            ink,
            scales: &scales,
            shapes: ground.shapes,
            vignette: ground.vignette,
            icons: &ground.theme.lyrics_icons,
        },
    );
    frame.into_geometry()
}

fn targets(seed: &str, audio: &AudioFrame, icons: &[String]) -> [f32; SHAPES] {
    let drawn = shapes(seed);
    std::array::from_fn(|index| {
        if drawn[index].icon.is_some() && !icons.is_empty() {
            band_scale(audio.vocal)
        } else {
            scale_for(drawn[index].kind, audio)
        }
    })
}

fn hash_seed(seed: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in seed.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn channels(rgb: Rgb) -> [f32; 3] {
    [f32::from(rgb.r) / 255.0, f32::from(rgb.g) / 255.0, f32::from(rgb.b) / 255.0]
}

fn solid(rgb: [f32; 3]) -> Color {
    Color::from_rgba(rgb[0], rgb[1], rgb[2], 1.0)
}

fn mix(from: [f32; 3], to: [f32; 3], t: f32) -> [f32; 3] {
    [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t, from[2] + (to[2] - from[2]) * t]
}

fn same(left: [f32; 3], right: [f32; 3]) -> bool {
    (left[0] - right[0]).abs() < 1.0 / 255.0
        && (left[1] - right[1]).abs() < 1.0 / 255.0
        && (left[2] - right[2]).abs() < 1.0 / 255.0
}

struct Coat<'a> {
    icons: &'a [String],
    seed: &'a str,
    time: f64,
    ink: Ink,
    scales: &'a [f32; SHAPES],
    shapes: bool,
    vignette: bool,
}

/// Shapes and specks on the theme ground. The vignette is the original radial gradient, not a second overlay.
fn paint(frame: &mut Frame, size: iced::Size, coat: Coat<'_>) {
    frame.fill_rectangle(Point::ORIGIN, size, coat.ink.ground);
    if coat.shapes {
        for (index, shape) in shapes(coat.seed).into_iter().enumerate() {
            if let Some(outline) = shape
                .icon
                .and_then(|slot| coat.icons.get((slot * coat.icons.len() as f64) as usize))
                .and_then(|name| crate::theme_icons::outline(name))
            {
                let (x, y, rotation) = drift(shape.delay, shape.duration, shape.reverse, shape.rotation, coat.time);
                let phase = ((coat.time - shape.delay).max(0.0) / (shape.duration * 0.5)).fract();
                let pulse = if phase < 0.5 { phase * 2.0 } else { (1.0 - phase) * 2.0 };
                let alpha = Ease::new(0.42, 0.0, 0.58, 1.0).at(pulse) as f32 * shape.opacity * 3.0;
                let center = Point::new(
                    size.width * shape.x / 100.0 + shape.size / 2.0 + x,
                    size.height * shape.y / 100.0 + shape.size / 2.0 + y,
                );
                crate::theme_icons::draw(
                    frame,
                    &outline,
                    center,
                    shape.size * coat.scales[index],
                    rotation.to_radians(),
                    Color { a: alpha, ..coat.ink.secondary },
                    coat.scales[index],
                );
                continue;
            }
            draw_shape(frame, size, coat.time, coat.ink.secondary, coat.scales[index], shape);
        }
        for particle in particles(coat.seed) {
            draw_particle(frame, size, coat.time, coat.ink.accent, particle);
        }
    }
    if coat.vignette {
        radial_vignette(frame, size);
    }
}

fn draw_shape(frame: &mut Frame, size: iced::Size, time: f64, secondary: iced::Color, scale: f32, shape: Shape) {
    let (tx, ty, degrees) = drift(shape.delay, shape.duration, shape.reverse, shape.rotation, time);
    let origin =
        Point::new(size.width * shape.x / 100.0 + shape.size / 2.0, size.height * shape.y / 100.0 + shape.size / 2.0);
    frame.with_save(|frame| {
        frame.translate(Vector::new(origin.x + tx, origin.y + ty));
        frame.rotate(Radians(degrees.to_radians()));
        frame.scale(scale);
        let path = shape_path(shape);
        let ink = iced::Color { a: shape.opacity, ..secondary };
        if shape.filled || shape.kind > 1 {
            frame.fill(&path, ink);
        } else {
            frame.stroke(&path, iced::widget::canvas::Stroke::default().with_color(ink).with_width(1.0));
        }
    });
}

fn shape_path(shape: Shape) -> Path {
    let s = shape.size;
    let h = s / 2.0;
    match shape.kind {
        0 => Path::circle(Point::ORIGIN, h),
        1 => Path::rectangle(Point::new(-h, -h), iced::Size::new(s, s)),
        2 => Path::new(|path| {
            path.move_to(Point::new(0.0, -h));
            path.line_to(Point::new(-h, h));
            path.line_to(Point::new(h, h));
            path.close();
        }),
        _ => Path::new(|path| {
            let p = |u: f32, v: f32| Point::new(s * u - h, s * v - h);
            path.move_to(p(0.2, 0.0));
            path.line_to(p(0.0, 0.2));
            path.line_to(p(0.3, 0.5));
            path.line_to(p(0.0, 0.8));
            path.line_to(p(0.2, 1.0));
            path.line_to(p(0.5, 0.7));
            path.line_to(p(0.8, 1.0));
            path.line_to(p(1.0, 0.8));
            path.line_to(p(0.7, 0.5));
            path.line_to(p(1.0, 0.2));
            path.line_to(p(0.8, 0.0));
            path.line_to(p(0.5, 0.3));
            path.close();
        }),
    }
}

fn draw_particle(frame: &mut Frame, size: iced::Size, time: f64, accent: iced::Color, particle: Particle) {
    if time <= particle.delay {
        return;
    }
    let phase = ((time - particle.delay) % particle.duration) / particle.duration;
    let along = phase as f32;
    let opacity = if along < 0.5 { particle.opacity * (along / 0.5) } else { particle.opacity * (1.0 - along) / 0.5 };
    let center = Point::new(size.width * particle.x / 100.0, size.height * particle.y / 100.0 - along * 100.0);
    frame.fill(&Path::circle(center, particle.size / 2.0), iced::Color { a: opacity, ..accent });
}

/// `radial-gradient(circle, transparent 40%, rgba(0,0,0,0.6) 100%)`, farthest-corner.
fn radial_vignette(frame: &mut Frame, size: iced::Size) {
    let center = Point::new(size.width / 2.0, size.height / 2.0);
    let radius = (size.width * size.width + size.height * size.height).sqrt() * 0.5;
    let inner = radius * 0.4;
    let slices = 96;
    for index in 0..slices {
        let a0 = index as f32 / slices as f32 * std::f32::consts::TAU;
        let a1 = (index + 1) as f32 / slices as f32 * std::f32::consts::TAU;
        let mid = (a0 + a1) * 0.5;
        let path = Path::new(|path| {
            path.move_to(Point::new(center.x + a0.cos() * inner, center.y + a0.sin() * inner));
            path.line_to(Point::new(center.x + a0.cos() * radius, center.y + a0.sin() * radius));
            path.line_to(Point::new(center.x + a1.cos() * radius, center.y + a1.sin() * radius));
            path.line_to(Point::new(center.x + a1.cos() * inner, center.y + a1.sin() * inner));
            path.close();
        });
        let gradient = iced::widget::canvas::gradient::Linear::new(
            Point::new(center.x + mid.cos() * inner, center.y + mid.sin() * inner),
            Point::new(center.x + mid.cos() * radius, center.y + mid.sin() * radius),
        )
        .add_stop(0.0, iced::Color::TRANSPARENT)
        .add_stop(1.0, iced::Color::from_rgba(0.0, 0.0, 0.0, 0.6));
        frame.fill(&path, gradient);
    }
}
