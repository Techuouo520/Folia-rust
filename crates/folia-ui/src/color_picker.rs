// Saturation/value square and hue strip for the workbench's six-digit RGB colour controls.

use folia_core::Rgb;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, gradient::Linear};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, mouse};

pub fn view<'a, M: Clone + 'a>(color: Rgb, change: impl Fn(Rgb) -> M + 'a, commit: M) -> Element<'a, M> {
    canvas::Canvas::new(Picker { color, change, commit }).width(Length::Fill).height(180.0).into()
}

struct Picker<M, F> {
    color: Rgb,
    change: F,
    commit: M,
}
#[derive(Default)]
pub struct State {
    dragging: Option<bool>,
    hue: Option<f32>,
}

impl<M: Clone, F: Fn(Rgb) -> M> canvas::Program<M> for Picker<M, F> {
    type State = State;
    fn update(
        &self,
        state: &mut State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<M>> {
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if cursor.is_over(bounds) => {
                let point = cursor.position_in(bounds)?;
                state.dragging = Some(point.y >= bounds.height - 24.0);
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging.is_some() => {}
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.dragging.take().is_some() =>
            {
                return Some(canvas::Action::publish(self.commit.clone()).and_capture());
            }
            _ => return None,
        }
        let point = cursor.position()? - bounds.position();
        let (hue, saturation, value) = hsv(self.color);
        let hue = state.hue.unwrap_or(hue);
        let color = if state.dragging == Some(true) {
            let hue = (point.x / bounds.width).clamp(0.0, 1.0) * 360.0;
            state.hue = Some(hue);
            rgb(hue, saturation, value)
        } else {
            rgb(hue, (point.x / bounds.width).clamp(0.0, 1.0), (1.0 - point.y / (bounds.height - 24.0)).clamp(0.0, 1.0))
        };
        Some(canvas::Action::publish((self.change)(color)).and_capture())
    }

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let (hue, saturation, value) = hsv(self.color);
        let hue = if saturation == 0.0 { state.hue.unwrap_or(hue) } else { hue };
        let height = bounds.height - 24.0;
        let square = Path::rectangle(Point::ORIGIN, Size::new(bounds.width, height));
        frame.fill(
            &square,
            Linear::new(Point::ORIGIN, Point::new(bounds.width, 0.0))
                .add_stop(0.0, Color::WHITE)
                .add_stop(1.0, ink(rgb(hue, 1.0, 1.0))),
        );
        frame.fill(
            &square,
            Linear::new(Point::ORIGIN, Point::new(0.0, height))
                .add_stop(0.0, Color::TRANSPARENT)
                .add_stop(1.0, Color::BLACK),
        );
        let mut spectrum = Linear::new(Point::new(0.0, height), Point::new(bounds.width, height));
        for stop in 0..=6 {
            spectrum = spectrum.add_stop(stop as f32 / 6.0, ink(rgb(stop as f32 * 60.0, 1.0, 1.0)));
        }
        frame.fill(&Path::rectangle(Point::new(0.0, height), Size::new(bounds.width, 24.0)), spectrum);
        for (center, fill) in [
            (Point::new(saturation * bounds.width, (1.0 - value) * height), ink(self.color)),
            (Point::new(hue / 360.0 * bounds.width, height + 12.0), ink(rgb(hue, 1.0, 1.0))),
        ] {
            let circle = Path::circle(center, 10.0);
            frame.fill(&circle, fill);
            frame.stroke(&circle, Stroke::default().with_color(Color::WHITE).with_width(3.0));
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, _state: &State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if cursor.is_over(bounds) { mouse::Interaction::Crosshair } else { mouse::Interaction::default() }
    }
}

fn ink(rgb: Rgb) -> Color {
    Color::from_rgb8(rgb.r, rgb.g, rgb.b)
}
fn hsv(rgb: Rgb) -> (f32, f32, f32) {
    let [r, g, b] = [rgb.r, rgb.g, rgb.b].map(|v| f32::from(v) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    (hue * 60.0, if max == 0.0 { 0.0 } else { delta / max }, max)
}
fn rgb(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(360.0) / 60.0;
    let chroma = v * s;
    let x = chroma * (1.0 - (h % 2.0 - 1.0).abs());
    let rgb = match h as u8 {
        0 => [chroma, x, 0.0],
        1 => [x, chroma, 0.0],
        2 => [0.0, chroma, x],
        3 => [0.0, x, chroma],
        4 => [x, 0.0, chroma],
        _ => [chroma, 0.0, x],
    };
    let [r, g, b] = rgb.map(|value| ((value + v - chroma) * 255.0).round() as u8);
    Rgb { r, g, b }
}
