// Native room sound with a validated mesh and the original Steam Audio 4.8.1 simulation parameters.

#[allow(
    unsafe_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    clippy::upper_case_acronyms
)]
mod ffi;

mod cache;
mod engine;
mod impulse;
#[allow(unsafe_code)]
mod native;

pub use engine::{Engine, library_path};
pub use impulse::loudness_gain;
pub use native::Simulator;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot load the Steam Audio library")]
    Library(#[from] libloading::Error),
    #[error("invalid room request: {0}")]
    Invalid(&'static str),
    #[error("Steam Audio {operation} failed with status {status}")]
    Native { operation: &'static str, status: i32 },
    #[error("cannot read the room-sound resources")]
    Io(#[from] std::io::Error),
    #[error("cannot read or write the room-sound cache")]
    Database(#[from] rusqlite::Error),
    #[error("room cache is in use")]
    Cache(#[from] folia_store::StoreError),
    #[error("room-sound worker has stopped")]
    Worker,
}
