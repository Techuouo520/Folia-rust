// Monet (莫奈): a poster with the lyric rail on the left and a portrait on the right.

mod ink;
mod layout;
mod motion;
mod poster;
mod rail;
mod spectrum;
mod text;
mod tuning;

pub use layout::{Layout, layout};
pub use poster::{Cache, interaction, update};
pub use spectrum::{
    active_bar_height, energy, envelope, pulse, sample_raw_spectrum_profile, sample_spectrum_profile, wave,
};
pub use text::{Shaped, shape};
pub use tuning::manifest;

use iced::widget::canvas::Frame;
use iced::{Point, Size};

use crate::{Place, Scene};

/// The playback position a click at `at` starts from: the line box under it, or nothing.
pub fn line_at(scene: &Scene<'_>, size: Size, at: Point) -> Option<f64> {
    let offset = tuning::offset(&scene.tuning);
    let layout = layout::layout(scene, size, offset);
    let boxes = rail::boxes(scene, &layout);
    boxes
        .iter()
        .rev()
        .find(|(_, rect)| rect.contains(at))
        .and_then(|(index, _)| scene.lyrics.lines.get(*index))
        .map(|line| line.start)
}

/// The line boxes of the rail as drawn at `scene.time`; exposed for hit tests in other crates.
pub fn rail_boxes(scene: &Scene<'_>, layout: &Layout) -> Vec<(usize, iced::Rectangle)> {
    rail::boxes(scene, layout)
}

/// The portrait's horizontal offset after a drag on the hanger, `-150..=0` pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PortraitOffset(pub f32);

/// What Monet's canvas asks of the shell: its portrait moved, or a lyric line was clicked to play from there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Portrait(PortraitOffset),
    /// Seek to this playback position in seconds.
    Seek(f64),
}

impl From<PortraitOffset> for Event {
    fn from(offset: PortraitOffset) -> Self {
        Event::Portrait(offset)
    }
}

/// Draws the poster on a stage of `size`; `place` maps the stage onto the canvas for clipped parts.
pub fn draw(scene: &Scene<'_>, cache: &mut Cache, frame: &mut Frame, size: Size, place: Place) {
    decorations(scene, frame, size);
    poster::draw(scene, cache, frame, size, place);
}

fn decorations(scene: &Scene<'_>, frame: &mut Frame, size: Size) {
    let icons = &scene.theme.lyrics_icons;
    if icons.is_empty() {
        return;
    }
    for i in 0..10usize {
        if (i * 37 + 11) % 100 <= 20 {
            continue;
        }
        let Some(outline) = crate::theme_icons::outline(&icons[i % icons.len()]) else { continue };
        let width = (46 + (i * 53) % 40) as f32;
        let x = ((i * 127 + 43) % 80 + 10) as f32 * size.width / 100.0;
        let y = ((i * 211 + 17) % 80 + 5) as f32 * size.height / 100.0;
        let rotation = ((i * 97) % 360) as f32;
        let opacity = 0.06 + ((i * 31) % 12) as f32 / 100.0;
        let phase = if scene.stage.is_some() {
            0.0
        } else {
            ((scene.time - ((i * 41) % 80) as f64 / 10.0).max(0.0) / (20 + (i * 71) % 20) as f64).fract() * 4.0
        };
        let at = phase.floor() as usize;
        let blend = crate::anim::Ease::new(0.42, 0.0, 0.58, 1.0).at(phase.fract()) as f32;
        let sample = |values: [f32; 5]| values[at] + (values[at + 1] - values[at]) * blend;
        let direction = if i % 2 == 0 { -1.0 } else { 1.0 };
        let (dx, dy, turn, alpha) = if scene.stage.is_some() {
            (0.0, 0.0, rotation, opacity)
        } else {
            (
                sample([-15.0, 25.0, -10.0, 20.0, -15.0]) * direction,
                sample([40.0, -60.0, 20.0, -40.0, 40.0]) * direction,
                rotation + sample([0.0, 120.0, 60.0, 180.0, 0.0]) * direction,
                opacity * sample([0.6, 1.2, 1.0, 1.3, 0.7]),
            )
        };
        crate::theme_icons::draw(
            frame,
            &outline,
            Point::new(x + dx + width / 2.0, y + dy + width / 2.0),
            width,
            turn.to_radians(),
            crate::color(scene.theme.secondary, alpha),
            1.0,
        );
    }
}
