// RadioBackdrop, RadioOrbits and RadioSpectrum geometry in logical pixels.

use iced::widget::canvas::{Frame, Path, Stroke, gradient};
use iced::{Color, Point, Vector};
use std::time::Instant;

use crate::radio::{Metrics, State};

pub(crate) fn draw(
    frame: &mut Frame,
    state: &State,
    theme: &folia_core::Theme,
    word: &str,
    now: Instant,
    opacity: f32,
) {
    let m = state.metrics;
    let ink = folia_visualizer::color(theme.primary, opacity);
    let accent = folia_visualizer::color(theme.accent, opacity);
    let seconds = now.saturating_duration_since(state.opened + state.hold).as_secs_f32();
    let fade = |delay: f32, duration: f32| {
        if crate::radio::reduced() {
            1.0
        } else {
            folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0)
                .at(f64::from(((seconds - delay) / duration).clamp(0.0, 1.0))) as f32
        }
    };
    let center = m.wave_center();
    let turn = (-state.at(now) * 3.0).to_radians();
    let wave = fade(0.12, 1.2);
    let radial_scale = 0.96 + 0.04 * wave;
    ring(frame, center, m.station_radius() * radial_scale, alpha(ink, 0.24 * wave), 1.0, false);
    let dots: [&[f32]; 6] = [
        &[-21.0, 9.0, 30.0],
        &[-34.0, -14.0, 18.0],
        &[-26.0, 3.0, 24.0, 38.0],
        &[-38.0, -8.0, 14.0],
        &[-30.0, -17.0, 27.0],
        &[-24.0, 6.0, 33.0],
    ];
    for (i, scale) in [0.62, 0.78, 0.9, 1.12, 1.26, 1.44].into_iter().enumerate() {
        let r = m.radius * scale * radial_scale;
        ring(frame, center, r, alpha(ink, 0.13 * wave), if i % 2 == 0 { 2.0 } else { 1.5 }, i % 2 == 0);
        for (j, degrees) in dots[i].iter().enumerate() {
            let color = if i == 0 && j == 0 { accent } else { alpha(ink, if j == 1 { 0.6 } else { 0.42 }) };
            frame.fill(
                &Path::circle(on_ring(center, r, degrees.to_radians() + turn), if j == 1 { 4.0 } else { 2.5 }),
                alpha(color, wave),
            );
        }
    }
    let upper_dots: [&[f32]; 3] = [&[-9.0, 13.0], &[-14.0, 4.0, 17.0], &[-6.0, 11.0]];
    for (i, r) in m.upper_rings().into_iter().enumerate() {
        ring(
            frame,
            center,
            r * radial_scale,
            alpha(ink, if i == 1 { 0.14 * wave } else { 0.09 * wave }),
            if i == 1 { 1.5 } else { 1.0 },
            i == 1,
        );
        for (j, degrees) in upper_dots[i].iter().enumerate() {
            frame.fill(
                &Path::circle(
                    on_ring(center, r * radial_scale, degrees.to_radians() + turn),
                    if j == 1 { 3.5 } else { 2.0 },
                ),
                alpha(ink, if j == 1 { 0.6 * wave } else { 0.42 * wave }),
            );
        }
    }
    dial(frame, state, ink, accent, fade(0.36, 0.9), now);
    transmitter(frame, state, ink, accent, fade(0.18, 0.9), fade(0.5, 1.2));
    packets(frame, m, state.elapsed, turn, alpha(ink, 0.8 * fade(0.5, 1.2)));
    if m.size.width >= 640.0 {
        let tick = fade(0.62, 0.9);
        for (offset, slide, width, color) in [
            (Vector::ZERO, 0.0, (m.size.width * 0.32).min(460.0) * fade(0.26, 0.9), alpha(ink, 0.62)),
            (Vector::new(22.0, -24.0), -24.0 * (1.0 - tick), 40.0, alpha(accent, tick)),
        ] {
            frame.with_save(|frame| {
                // Each CSS figure rotates around its own left centre; paths retain the rotated corners.
                frame.translate(Vector::new(m.size.width * 0.75 + offset.x, m.size.height * 0.94 + offset.y + 3.5));
                frame.rotate((-9.0_f32).to_radians());
                frame.fill(&Path::rectangle(Point::new(slide, -3.5), iced::Size::new(width, 7.0)), color);
            });
        }
        let hatch = Point::new(m.size.width * 0.91 - 56.0, m.size.height * 0.145 + 40.0 * (1.0 - fade(0.48, 0.9)));
        for i in 0..13 {
            let x = i as f32 * 9.0;
            line(
                frame,
                hatch + Vector::new(x.min(56.0), (x - 56.0).max(0.0)),
                hatch + Vector::new((x - 56.0).max(0.0), x.min(56.0)),
                alpha(ink, 0.42 * fade(0.48, 0.9)),
                2.0,
            );
        }
    }
    let size = if m.size.width < 640.0 { 56.0 } else { (m.size.height * 0.13).clamp(56.0, 160.0) };
    frame.fill_text(iced::widget::canvas::Text {
        content: word.to_owned(),
        position: Point::new(
            m.size.width * 0.03,
            m.size.height * 0.98 + m.size.height * 0.06 * (1.0 - fade(0.22, 0.9)),
        ),
        color: alpha(ink, 0.06 * fade(0.22, 0.9)),
        size: size.into(),
        font: crate::typeface::BLACK,
        align_y: iced::alignment::Vertical::Bottom,
        ..crate::typeface::canvas_defaults()
    });
}

fn dial(frame: &mut Frame, state: &State, ink: Color, accent: Color, fade: f32, now: Instant) {
    let m = state.metrics;
    let center = m.wave_center();
    let top = m.center.y - m.now_size / 2.0;
    let radius = center.y - (top - 47.0) - 6.0;
    let half = (m.size.width * 0.2).min(300.0);
    let turn = (-state.at(now) * 3.0).to_radians();
    let shift = (turn * radius).rem_euclid(16.65 * 4.0);
    for i in -85..=85 {
        let along = i as f32 * 16.65 + shift;
        let theta = along / radius;
        let p = on_ring(center, radius, theta);
        let distance = (p.x - m.center.x).abs();
        let strength = ((half - distance) / (half * 0.44)).clamp(0.0, 1.0) * fade;
        if strength <= 0.0 {
            continue;
        }
        let length = if i % 4 == 0 { 12.0 } else { 6.0 };
        line(
            frame,
            on_ring(center, radius - length / 2.0, theta),
            on_ring(center, radius + length / 2.0, theta),
            alpha(ink, strength * if i % 4 == 0 { 0.42 } else { 0.3 }),
            1.25,
        );
    }
    if state.current.is_none() {
        return;
    }
    let dx = m.rect(state.anchor - state.at(now)).bounds.center_x() - m.center.x;
    let opacity = ((half - dx.abs()) / (half * 0.44)).clamp(0.0, 1.0) * fade;
    let length = (top * 0.55).clamp(48.0, 128.0);
    let needle_top = top - 12.0 - length;
    let pivot = center.y - needle_top;
    let theta = (dx / pivot).clamp(-1.0, 1.0).asin();
    let tip = on_ring(center, pivot, theta);
    line(frame, tip, on_ring(center, pivot - length, theta), alpha(accent, opacity), 2.0);
    let triangle = Path::new(|p| {
        p.move_to(tip + Vector::new(-5.5, -6.0));
        p.line_to(tip + Vector::new(5.5, -6.0));
        p.line_to(tip + Vector::new(0.0, 1.0));
        p.close();
    });
    frame.fill(&triangle, alpha(accent, opacity));
}

fn transmitter(frame: &mut Frame, state: &State, ink: Color, accent: Color, fade: f32, orbit_fade: f32) {
    let size = state.metrics.size;
    let compact = size.width < 640.0;
    let side = size.height * if compact { 0.3 } else { 0.44 };
    let center = Point::new(
        if compact { -size.height * 0.1 } else { size.width * 0.035 } + side / 2.0,
        size.height * if compact { -0.06 } else { -0.09 } + side / 2.0,
    );
    let radius = side / 2.0 * (0.4 + 0.6 * fade);
    ring(frame, center, radius, alpha(ink, 0.22 * fade), 1.5, false);
    ring(frame, center, radius * 0.34, alpha(ink, 0.22 * fade), 1.5, true);
    frame.fill(&Path::circle(center, radius * 0.12), alpha(accent, fade));
    for (i, level) in state.levels.iter().enumerate() {
        let angle = i as f32 / 36.0 * std::f32::consts::TAU;
        line(
            frame,
            on_ring(center, side * 0.17, angle),
            on_ring(center, side * (0.19 + 0.25 * level), angle),
            alpha(ink, 0.7 * (0.35 + 0.55 * level) * fade),
            (side * 0.008).max(1.5),
        );
    }
    for birth in state.ripples.into_iter().flatten() {
        let progress = (state.elapsed - birth) / 2.6;
        if !(0.0..1.0).contains(&progress) {
            continue;
        }
        let r = radius * (0.33 + 1.32 * (1.0 - (1.0 - progress).powi(2)));
        ring(frame, center, r, alpha(ink, 0.8 * 0.32 * (1.0 - progress).powf(1.5) * orbit_fade), 1.0, false);
    }
    for (scale, period, dots) in [(1.22, 38.0, &[40.0_f32][..]), (1.5, 64.0, &[160.0_f32, 340.0][..])] {
        ring(frame, center, radius * scale, alpha(ink, 0.8 * 0.12 * orbit_fade), 1.0, true);
        for degrees in dots {
            let theta = degrees.to_radians() + state.elapsed / period * std::f32::consts::TAU;
            frame.fill(
                &Path::circle(on_ring(center, radius * scale, theta), if scale < 1.3 { 3.5 } else { 2.0 }),
                alpha(if scale < 1.3 { accent } else { alpha(ink, 0.6) }, orbit_fade),
            );
        }
    }
}

fn packets(frame: &mut Frame, m: Metrics, seconds: f32, turn: f32, ink: Color) {
    let center = m.wave_center();
    for (i, radius) in m.upper_rings().into_iter().enumerate() {
        let span = ((m.size.width / 2.0 + 128.0) / radius).min(1.0).asin();
        let speed = [16.0, 24.0, 20.0][i] / radius;
        let direction = if i % 2 == 0 { 1.0 } else { -1.0 };
        for packet in 0..2 {
            let base = (packet as f32 / 2.0 + i as f32 * 0.37) * span * 2.0;
            let travel = (base + seconds * speed).rem_euclid(span * 2.0);
            let head = direction * (travel - span) + turn;
            let tail = head - direction * 64.0 / radius;
            let from = on_ring(center, radius, tail);
            let to = on_ring(center, radius, head);
            let gradient = gradient::Linear::new(from, to)
                .add_stop(0.0, alpha(ink, 0.0))
                .add_stop(0.35, alpha(ink, 0.1))
                .add_stop(0.7, alpha(ink, 0.3))
                .add_stop(1.0, alpha(ink, 0.58));
            frame.stroke(
                &arc(center, radius, tail.min(head), tail.max(head)),
                Stroke {
                    width: 2.0,
                    style: iced::widget::canvas::Style::Gradient(gradient.into()),
                    ..Stroke::default()
                },
            );
            frame.fill(&Path::circle(to, 2.2), alpha(ink, 0.75));
        }
    }
}

fn arc(center: Point, radius: f32, from: f32, to: f32) -> Path {
    Path::new(|p| {
        p.arc(iced::widget::canvas::path::Arc {
            center,
            radius,
            start_angle: iced::Radians(from - std::f32::consts::FRAC_PI_2),
            end_angle: iced::Radians(to - std::f32::consts::FRAC_PI_2),
        })
    })
}

fn ring(frame: &mut Frame, center: Point, radius: f32, color: Color, width: f32, dashed: bool) {
    if center.y - radius > frame.height() {
        return;
    }
    let mut stroke = Stroke::default().with_width(width).with_color(color);
    let segments = [2.0, 10.0];
    if dashed {
        stroke.line_dash = iced::widget::canvas::LineDash { segments: &segments, offset: 0 };
    }
    let path = if center.y > frame.height() && radius > frame.height() {
        let from = ((-center.x - 16.0) / radius).clamp(-1.0, 1.0).asin();
        let to = ((frame.width() - center.x + 16.0) / radius).clamp(-1.0, 1.0).asin();
        arc(center, radius, from, to)
    } else {
        Path::circle(center, radius)
    };
    frame.stroke(&path, stroke);
}

pub(crate) fn line(frame: &mut Frame, from: Point, to: Point, color: Color, width: f32) {
    frame.stroke(
        &Path::line(from, to),
        Stroke::default().with_color(color).with_width(width).with_line_cap(iced::widget::canvas::LineCap::Round),
    );
}

pub(crate) fn alpha(color: Color, factor: f32) -> Color {
    Color { a: color.a * factor, ..color }
}

fn on_ring(center: Point, radius: f32, angle: f32) -> Point {
    center + Vector::new(radius * angle.sin(), -radius * angle.cos())
}
