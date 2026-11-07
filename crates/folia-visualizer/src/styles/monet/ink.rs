// Canvas stand-ins for the CSS Monet leans on: masks as clipped alpha bands, blur and text-shadow as offset copies,
// box-shadow and radial glows as stacked layers. Only text and images go through `Place::clip`: a clip draft's meshes
// are pasted ahead of the root frame's buffered ones and end up under the background.

use iced::widget::canvas::{Frame, Path, Text};
use iced::{Color, Point, Rectangle, Size, border::Radius};

use crate::Place;

/// A run of text drawn at `at` (top of its line box).
#[derive(Debug, Clone, Copy)]
pub struct Line<'a> {
    pub content: &'a str,
    pub at: Point,
    pub px: f32,
    pub line_height: f32,
    pub font: iced::Font,
}

impl Line<'_> {
    fn fill(&self, frame: &mut Frame, offset: iced::Vector, color: Color) {
        frame.fill_text(Text {
            content: self.content.to_owned(),
            position: self.at + offset,
            color,
            size: self.px.into(),
            line_height: iced::widget::text::LineHeight::Absolute(self.line_height.into()),
            font: self.font,
            align_x: iced::widget::text::Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: iced::widget::text::Shaping::Advanced,
            max_width: f32::INFINITY,
        });
    }
}

/// Alpha of a vertical mask at a stage `y`.
pub type Mask<'a> = &'a dyn Fn(f32) -> f32;

pub fn opaque(_: f32) -> f32 {
    1.0
}

const BAND: f32 = 6.0;

/// Draws `line` in `color`, clipped to `clip` and faded by `mask`: flat stretches once, fading stretches in bands.
pub fn masked(frame: &mut Frame, place: Place, line: Line<'_>, color: Color, clip: Rectangle, mask: Mask<'_>) {
    masked_copies(frame, place, line, color, clip, mask, &[(0.0, 0.0)]);
}

/// Like `masked`, with one copy of the text per offset, each in `color`.
pub fn masked_copies(
    frame: &mut Frame,
    place: Place,
    line: Line<'_>,
    color: Color,
    clip: Rectangle,
    mask: Mask<'_>,
    offsets: &[(f32, f32)],
) {
    if clip.width <= 0.0 || clip.height <= 0.0 || color.a <= 0.002 {
        return;
    }
    let flat = [clip.y, clip.y + clip.height / 2.0, clip.y + clip.height].iter().map(|y| mask(*y)).collect::<Vec<_>>();
    let mut draw = |region: Rectangle, alpha: f32| {
        if alpha <= 0.004 {
            return;
        }
        let tinted = Color { a: color.a * alpha, ..color };
        place.clip(frame, region, |inner| {
            for (dx, dy) in offsets {
                line.fill(inner, iced::Vector::new(*dx, *dy), tinted);
            }
        });
    };
    if flat.iter().all(|alpha| (alpha - flat[0]).abs() < 0.002) {
        draw(clip, flat[0]);
        return;
    }
    let bands = (clip.height / BAND).ceil().max(1.0) as usize;
    let step = clip.height / bands as f32;
    for band in 0..bands {
        let y = clip.y + band as f32 * step;
        draw(Rectangle { y, height: step, ..clip }, mask(y + step / 2.0));
    }
}

/// Offsets that stand in for a Gaussian of `sigma`: the centre and two rings of six. `split_alpha` gives each copy its
/// share.
pub fn blur_offsets(sigma: f32, shift: (f32, f32)) -> Vec<(f32, f32)> {
    let mut offsets = vec![shift];
    for ring in [0.7_f32, 1.5] {
        for step in 0..6 {
            let angle = (step as f32 + ring) * std::f32::consts::TAU / 6.0;
            offsets.push((shift.0 + angle.cos() * sigma * ring, shift.1 + angle.sin() * sigma * ring));
        }
    }
    offsets
}

/// Per-copy alpha so `copies` overlapping copies composite to `alpha`.
pub fn split_alpha(alpha: f32, copies: usize) -> f32 {
    1.0 - (1.0 - alpha.clamp(0.0, 0.999)).powf(1.0 / copies.max(1) as f32)
}

/// `box-shadow: 0 y blur spread color` under a rounded box, as stacked rounded rectangles following the Gaussian edge.
pub fn box_shadow(frame: &mut Frame, rect: Rectangle, radius: f32, y: f32, blur: f32, color: Color) {
    let sigma = (blur / 2.0).max(0.5);
    let steps = 14;
    let (near, far) = (-1.2 * sigma, 2.6 * sigma);
    let mut below = 0.0_f32;
    for step in 0..steps {
        let reach = far + (near - far) * step as f32 / (steps - 1) as f32;
        let target = color.a * edge(reach / sigma);
        let alpha = if below >= 0.999 { 0.0 } else { ((target - below) / (1.0 - below)).max(0.0) };
        below = target.max(below);
        if alpha <= 0.0005 {
            continue;
        }
        let grown = Rectangle {
            x: rect.x - reach,
            y: rect.y + y - reach,
            width: rect.width + reach * 2.0,
            height: rect.height + reach * 2.0,
        };
        if grown.width <= 0.0 || grown.height <= 0.0 {
            continue;
        }
        let corner = (radius + reach).max(0.0).min(grown.width / 2.0).min(grown.height / 2.0);
        frame.fill(
            &Path::rounded_rectangle(grown.position(), grown.size(), Radius::from(corner)),
            Color { a: alpha, ..color },
        );
    }
}

/// Coverage of a blurred straight edge `d` sigmas outside it: `erfc(d / √2) / 2` (Abramowitz–Stegun 7.1.26).
pub fn edge(d: f32) -> f32 {
    let x = (d / std::f32::consts::SQRT_2).abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t * (0.254_829_6 + t * (-0.284_496_7 + t * (1.421_414 + t * (-1.453_152 + t * 1.061_405))));
    let erfc = poly * (-x * x).exp();
    if d >= 0.0 { erfc / 2.0 } else { 1.0 - erfc / 2.0 }
}

/// `radial-gradient(closest-side, stops…)` filling `rect`, as nested ellipses. `stops` are `(alpha, at)` with `at` in
/// `0..=1` of the radius, increasing.
pub fn radial(frame: &mut Frame, rect: Rectangle, color: Color, stops: &[(f32, f32)]) {
    let center = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    let profile = |at: f32| {
        let pair = stops.windows(2).find(|pair| at <= pair[1].1);
        match pair {
            Some([(a0, s0), (a1, s1)]) => a0 + (a1 - a0) * ((at - s0) / (s1 - s0).max(1.0e-4)).clamp(0.0, 1.0),
            _ => stops.last().map_or(0.0, |(alpha, _)| *alpha),
        }
    };
    let steps = 12;
    let mut below = 0.0_f32;
    for step in 0..steps {
        let at = 1.0 - step as f32 / steps as f32;
        let target = color.a * profile(at - 0.5 / steps as f32);
        let alpha = ((target - below) / (1.0 - below)).max(0.0);
        below = target.max(below);
        if alpha <= 0.0005 {
            continue;
        }
        let size = Size::new(rect.width * at, rect.height * at);
        frame.with_save(|frame| {
            frame.translate(iced::Vector::new(center.x, center.y));
            frame.scale_nonuniform(iced::Vector::new(size.width / 2.0, size.height / 2.0));
            frame.fill(&Path::circle(Point::ORIGIN, 1.0), Color { a: alpha, ..color });
        });
    }
}

/// `color-mix` in sRGB, the way the original's `mixColors` blends two colours.
pub fn mix(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}
