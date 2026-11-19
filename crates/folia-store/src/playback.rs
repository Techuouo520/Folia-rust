// Playback preferences that survive a restart: volume and repeat mode.

use folia_core::{AudioQuality, Repeat};
use serde::{Deserialize, Serialize};

use crate::Domain;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackSettings {
    pub volume: f32,
    pub muted: bool,
    pub repeat: Repeat,
    /// One quality preference for every platform (`default_audio_quality`).
    pub audio_quality: AudioQuality,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self { muted: false, volume: 1.0, repeat: Repeat::Off, audio_quality: AudioQuality::High }
    }
}

impl Domain for PlaybackSettings {
    const FILE: &'static str = "playback.toml";
    const SCHEMA_VERSION: u32 = 2;
}

#[derive(Default, Serialize, Deserialize)]
struct PreviousPlayback {
    #[serde(flatten)]
    settings: PlaybackSettings,
}

impl Domain for PreviousPlayback {
    const FILE: &'static str = PlaybackSettings::FILE;
    const SCHEMA_VERSION: u32 = 1;
}

impl PlaybackSettings {
    pub fn load(store: &crate::Store) -> Result<Self, crate::StoreError> {
        match store.load::<Self>() {
            Err(crate::StoreError::SchemaVersion { found: 1, .. }) => {
                let previous: PreviousPlayback = store.load()?;
                store.save(&previous.settings)?;
                Ok(previous.settings)
            }
            result => result,
        }
    }
}
