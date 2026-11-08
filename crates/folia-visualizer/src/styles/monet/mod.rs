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
    poster::draw(scene, cache, frame, size, place);
}
