// Monet (莫奈): a poster with the lyric rail on the left and a portrait on the right.

mod ink;
mod layout;
mod motion;
mod poster;
mod rail;
mod text;
mod tuning;

pub use layout::{Layout, layout};
pub use poster::{Cache, interaction, update};
pub use text::{Shaped, shape};
pub use tuning::manifest;

use iced::Size;
use iced::widget::canvas::Frame;

use crate::{Place, Scene};

/// The portrait's horizontal offset after a drag on the hanger, `-150..=0` pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PortraitOffset(pub f32);

/// Draws the poster on a stage of `size`; `place` maps the stage onto the canvas for clipped parts.
pub fn draw(scene: &Scene<'_>, cache: &mut Cache, frame: &mut Frame, size: Size, place: Place) {
    poster::draw(scene, cache, frame, size, place);
}
