// Audio processing preferences: the equalizer and the ReplayGain mode, under the original's storage keys.

use folia_core::EqualizerSettings;
use serde::{Deserialize, Serialize};

use crate::Domain;

/// `ReplayGainMode`: off, auto, the track's tag or the album's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplayGainMode {
    #[default]
    Off,
    Auto,
    Track,
    Album,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// `audio_equalizer_settings`.
    #[serde(rename = "audio_equalizer_settings")]
    pub equalizer: EqualizerSettings,
    pub replay_gain_mode: ReplayGainMode,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { equalizer: EqualizerSettings::default(), replay_gain_mode: ReplayGainMode::Off }
    }
}

impl Domain for AudioSettings {
    const FILE: &'static str = "audio.toml";
    const SCHEMA_VERSION: u32 = 1;
}
