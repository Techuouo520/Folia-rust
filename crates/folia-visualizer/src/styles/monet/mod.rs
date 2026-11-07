// Monet (莫奈): a poster with the lyric rail on the left and a portrait on the right.

mod motion;
mod paint;
mod poster;
mod tuning;

pub use paint::{HEIGHT, WIDTH, Wash, paint};
pub use poster::{Cache, interact, take_offset};
pub use tuning::manifest;

use iced::widget::canvas::Frame;
use iced::{Point, Size};

use crate::Scene;

pub fn draw(scene: &Scene<'_>, cache: &mut Cache, frame: &mut Frame, size: Size) {
    poster::draw(scene, cache, frame, size, scene.portrait.as_ref(), scene.wash.as_ref());
}

/// `cursor` is in the same coordinates as `size` (the stage, after the picture scale).
pub fn pointer(scene: &Scene<'_>, cache: &mut Cache, size: Size, cursor: Option<Point>, pressed: Option<bool>) -> bool {
    interact(scene, cache, size, cursor, pressed)
}
