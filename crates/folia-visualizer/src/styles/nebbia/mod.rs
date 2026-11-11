// Nebbia (雾笺): a rainy night street behind glass, lyrics wiped into the fog.
// Small rain follows the vocal band. Drum hits are not analysed, so the bus keeps its idle sway.

mod cache;
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

/// One step of `VocalEnergy`: peak jumps to the raw byte, floor creeps, the span is at least 8.
pub fn vocal_energy(floor: f32, peak: f32, raw: f32, dt: f32) -> (f32, f32, f32) {
    let k = 1.0 - (-dt / 3.0).exp();
    let peak = raw.max(peak + (raw - peak) * k * 0.5);
    let floor = raw.min(floor + (raw - floor) * k * 0.3);
    let span = (peak - floor).max(8.0);
    let value = ((raw - floor) / span).clamp(0.0, 1.0).powf(1.2);
    (value, floor, peak)
}

/// Vocal-band follower kept by the street shader. Starts at floor 0, peak 1, like `VocalEnergy`.
#[derive(Clone)]
pub struct RainEnergy {
    floor: Cell<f32>,
    peak: Cell<f32>,
    last_ms: Cell<f64>,
}

impl Default for RainEnergy {
    fn default() -> Self {
        Self { floor: Cell::new(0.0), peak: Cell::new(1.0), last_ms: Cell::new(0.0) }
    }
}

impl RainEnergy {
    fn step(&self, raw: f32) -> f32 {
        let now = wall_ms();
        let previous = self.last_ms.get();
        let dt = if previous > 0.0 { ((now - previous) / 1000.0).clamp(0.0, 0.1) as f32 } else { 1.0 / 60.0 };
        self.last_ms.set(now);
        let (value, floor, peak) = vocal_energy(self.floor.get(), self.peak.get(), raw, dt);
        self.floor.set(floor);
        self.peak.set(peak);
        value
    }
}

fn wall_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}
pub use ink::glyphs_settled;
pub use strokes::{StrokesError, load_file};
pub use tuning::manifest;

use std::cell::Cell;
use std::marker::PhantomData;
use std::sync::Arc;

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
    type State = ViewState;
    type Primitive = gpu::LivePass;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        let energy = state.energy.step(self.scene.audio.vocal);
        let frame = frame(
            self.scene.lyrics,
            self.scene.time,
            bounds.size(),
            self.scene.theme,
            self.scene.font_scale,
            &self.scene.tuning,
            energy,
        );
        gpu::LivePass { frame, owner: Arc::downgrade(&state.owner) }
    }
}

/// The live widget owns the GPU resources' lifetime, independently of other windows' redraws.
#[derive(Default)]
pub struct ViewState {
    energy: RainEnergy,
    owner: Arc<()>,
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
