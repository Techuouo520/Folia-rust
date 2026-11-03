// Audio quality levels, lowest first, shared by the platform providers and the quality picker.

use serde::{Deserialize, Serialize};

/// The original's `AudioQualityPreference`; its order is the level's rank.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioQuality {
    Standard,
    #[default]
    High,
    Ultra,
    Lossless,
    Hires,
}

/// Every level, lowest first (`AUDIO_QUALITY_LEVELS`).
pub const AUDIO_QUALITY_LEVELS: [AudioQuality; 5] =
    [AudioQuality::Standard, AudioQuality::High, AudioQuality::Ultra, AudioQuality::Lossless, AudioQuality::Hires];

/// What a provider that lists none of its own tells apart (`UNLISTED_AUDIO_QUALITY_LEVELS`); `ultra` belongs only to
/// a provider that lists it.
pub const UNLISTED_AUDIO_QUALITY_LEVELS: [AudioQuality; 4] =
    [AudioQuality::Standard, AudioQuality::High, AudioQuality::Lossless, AudioQuality::Hires];

impl AudioQuality {
    /// The stored spelling, used in settings and cache file names.
    pub fn id(self) -> &'static str {
        match self {
            AudioQuality::Standard => "standard",
            AudioQuality::High => "high",
            AudioQuality::Ultra => "ultra",
            AudioQuality::Lossless => "lossless",
            AudioQuality::Hires => "hires",
        }
    }

    /// The label key of the quality picker (`account.quality*`).
    pub fn label_key(self) -> &'static str {
        match self {
            AudioQuality::Standard => "account.qualityStandard",
            AudioQuality::High => "account.qualityExhigh",
            AudioQuality::Ultra => "account.qualityUltra",
            AudioQuality::Lossless => "account.qualityLossless",
            AudioQuality::Hires => "account.qualityHires",
        }
    }
}
