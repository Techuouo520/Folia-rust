// Native cinema scene, with the upstream hall geometry and the user's live lyric/background surface.

mod camera;
mod frame;
mod geometry;
mod gpu;
mod intermission;
mod layout;
mod post;
mod reel;
mod widget;

pub use camera::{Camera, Head};
pub use frame::{Config, Frame, Runtime};
pub use geometry::{CinemaError, acoustic_world};
pub use gpu::Renderer;
#[cfg(feature = "dev")]
pub use gpu::target_sizes;
pub use intermission::{CurtainPose, Intermission, IntermissionFrame};
pub use layout::{COLUMNS, ROWS, SCREEN_ASPECT, SPEAKERS, Seat, listener, seats, selected_seat};
pub use widget::{Failure, view};
