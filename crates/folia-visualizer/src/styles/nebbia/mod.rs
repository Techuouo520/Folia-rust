// Nebbia (雾笺): a rainy night street behind glass, lyrics wiped into the fog.
// Beat and chorus energy are 0 until the analysis crate exists, so the bus does not bump.

mod drops;
mod gpu;
mod ink;
mod layout;
mod palette;
mod place;
mod scene;
mod strokes;
mod tuning;

pub use gpu::{Pass, Renderer, frame};
pub use ink::glyphs_settled;
pub use strokes::{StrokesError, load_file};
pub use tuning::manifest;

use std::marker::PhantomData;

use iced::widget::canvas::{Frame, Text};
use iced::widget::shader;
use iced::{Point, Rectangle, Size, Vector, mouse};

use crate::tuning::Values;
use crate::{Scene, color};

/// The street and fog for one frame. The writing is the wiped mask, not a second copy of the letters.
pub struct View<'a, Message> {
    scene: Scene<'a>,
    _message: PhantomData<Message>,
}

pub fn view<'a, Message>(scene: Scene<'a>) -> View<'a, Message> {
    View { scene, _message: PhantomData }
}

impl<Message> shader::Program<Message> for View<'_, Message> {
    type State = ();
    type Primitive = gpu::Pass;

    fn draw(&self, _state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        frame(
            self.scene.lyrics,
            self.scene.time,
            bounds.size(),
            self.scene.theme,
            self.scene.font_scale,
            &self.scene.tuning,
        )
    }
}

/// Shelf thumbnails have no street shader, so they draw the letters. The playing view leaves this clear.
pub fn draw(scene: &Scene<'_>, frame: &mut Frame, size: Size) {
    if !scene.backdrop {
        return;
    }
    let laid = scene::scene(scene.lyrics, size.width, size.height, scene.font_scale, &scene.tuning);
    let refog = f64::from(scene.tuning.number(manifest(), "refog")).max(0.5);
    let glyphs = place::glyphs(&laid, scene.time, size.width, size.height, refog);
    let origin = Point::new(size.width / 2.0, size.height / 2.0);
    for glyph in glyphs {
        frame.with_save(|frame| {
            frame.translate(Vector::new(origin.x + glyph.x, origin.y + glyph.y));
            frame.rotate(glyph.rotation);
            frame.fill_text(Text {
                content: glyph.text,
                position: Point::ORIGIN,
                color: color(scene.theme.primary, glyph.alpha),
                size: glyph.size.into(),
                font: crate::BOLD,
                align_x: iced::widget::text::Alignment::Center,
                align_y: iced::alignment::Vertical::Center,
                shaping: iced::widget::text::Shaping::Advanced,
                ..Text::default()
            });
        });
    }
}

pub fn normalize(table: Option<&toml::map::Map<String, toml::Value>>, values: &mut Values) {
    tuning::normalize(table, values);
}

/// mulberry32, the original `createVisualizerRng`.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let state = self.0 as i32;
        let mixed = ((state as u32) ^ ((state as u32) >> 15)) as i32;
        let mut t = mixed.wrapping_mul(1 | state);
        let bits = t as u32;
        t = t.wrapping_add(((bits ^ (bits >> 7)) as i32).wrapping_mul(61 | t)) ^ t;
        f64::from((t as u32) ^ ((t as u32) >> 14)) / 4_294_967_296.0
    }

    fn unit(&mut self) -> f32 {
        self.next() as f32
    }
}

/// Smoothstep of `t` clamped to 0..1.
fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
