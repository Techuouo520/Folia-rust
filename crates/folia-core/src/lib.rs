// Core data types shared by every Folia crate; depends on no other folia crate.

mod audio_quality;
mod language;
mod lyrics;
mod queue;
mod theme;
mod track;

pub use audio_quality::{AUDIO_QUALITY_LEVELS, AudioQuality, UNLISTED_AUDIO_QUALITY_LEVELS};
pub use language::{Language, LanguagePreference};
pub use lyrics::{ChorusRange, INTERLUDE_TEXT, Line, Lyrics, Word};
pub use queue::{Origin, Queue, Repeat};
pub use theme::{AnimationIntensity, Rgb, Theme};
pub use track::{OnlineId, Track};
