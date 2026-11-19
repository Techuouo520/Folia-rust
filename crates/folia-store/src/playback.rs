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
    pub netease_scrobble_enabled: bool,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            muted: false,
            volume: 1.0,
            repeat: Repeat::Off,
            audio_quality: AudioQuality::High,
            netease_scrobble_enabled: false,
        }
    }
}

impl Domain for PlaybackSettings {
    const FILE: &'static str = "playback.toml";
    const SCHEMA_VERSION: u32 = 3;
}

#[derive(Default, Serialize, Deserialize)]
struct PreviousPlayback<const VERSION: u32> {
    #[serde(flatten)]
    settings: PlaybackSettings,
}

impl<const VERSION: u32> Domain for PreviousPlayback<VERSION> {
    const FILE: &'static str = PlaybackSettings::FILE;
    const SCHEMA_VERSION: u32 = VERSION;
}

impl PlaybackSettings {
    pub fn load(store: &crate::Store) -> Result<Self, crate::StoreError> {
        match store.load::<Self>() {
            Err(crate::StoreError::SchemaVersion { found: 1, .. }) => {
                let previous: PreviousPlayback<1> = store.load()?;
                store.save(&previous.settings)?;
                Ok(previous.settings)
            }
            Err(crate::StoreError::SchemaVersion { found: 2, .. }) => {
                let previous: PreviousPlayback<2> = store.load()?;
                store.save(&previous.settings)?;
                Ok(previous.settings)
            }
            result => result,
        }
    }
}
