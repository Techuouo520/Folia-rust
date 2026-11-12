// Core data types shared by every Folia crate; depends on no other folia crate.

mod audio;
mod audio_eq;
mod audio_input;
mod audio_quality;
mod language;
mod lyrics;
mod queue;
mod theme;
mod track;
mod word_color;

pub use audio::AudioFrame;
pub use audio_eq::{
    AudioEffects, BandShape, EQUALIZER_FREQUENCIES, EQUALIZER_MAX_GAIN_DB, EQUALIZER_MIN_GAIN_DB, EQUALIZER_Q,
    EqualizerSettings, EqualizerSlot, SOUND_PRESETS, SoundPreset, preset,
};
pub use audio_input::{AudioInput, Download, DownloadReader, ReadRequest};
pub use audio_quality::{AUDIO_QUALITY_LEVELS, AudioQuality, UNLISTED_AUDIO_QUALITY_LEVELS};
pub use language::{Language, LanguagePreference};
pub use lyrics::{ChorusRange, INTERLUDE_TEXT, Line, Lyrics, SubtitleContentMode, Word};
pub use queue::{Origin, Queue, Repeat};
pub use theme::{AnimationIntensity, Rgb, Theme};
pub use track::{OnlineId, OnlineTrackInfo, ReplayGain, Track};
pub use word_color::{WordColor, WordColorRange, parse_color, word_color_ranges};
