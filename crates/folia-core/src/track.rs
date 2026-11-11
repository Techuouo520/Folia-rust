// A playable track, local or from an online platform.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Optional platform metadata retained for external address resolvers; never contains account credentials.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OnlineTrackInfo {
    pub album_id: String,
    pub audio_id: String,
    pub quality_hashes: std::collections::BTreeMap<crate::AudioQuality, String>,
}

/// Where an online track lives: a platform id such as `netease` and the platform's song id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OnlineId {
    pub provider: String,
    pub id: String,
}

impl OnlineId {
    /// A stable negative id, so online tracks never collide with the library's positive row ids.
    pub fn track_id(&self) -> i64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        -((hasher.finish() >> 1) as i64) - 1
    }
}

/// The gains a file's ReplayGain tags carry, in dB.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ReplayGain {
    pub track_db: Option<f32>,
    pub album_db: Option<f32>,
}

impl ReplayGain {
    /// What the mode applies: the track gain, the album gain falling back to the track gain, or none.
    pub fn gain_db(self, album: bool) -> Option<f32> {
        if album { self.album_db.or(self.track_db) } else { self.track_db }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: i64,
    /// The local file, or the cached download of an online track (empty until downloaded).
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    /// Length in seconds; zero when the source did not say.
    pub duration: f64,
    #[serde(default)]
    pub online: Option<OnlineId>,
    #[serde(default)]
    pub online_info: Option<OnlineTrackInfo>,
    /// Cover image address from an online search, when the platform sent one.
    #[serde(default)]
    pub cover_url: Option<String>,
    /// Gains the file's tags carry; online platforms do not hand any out.
    #[serde(default)]
    pub replay_gain: Option<ReplayGain>,
}

impl Track {
    /// `Artist - Title` when the artist is known, otherwise the title alone.
    pub fn display_name(&self) -> String {
        match &self.artist {
            Some(artist) => format!("{artist} - {}", self.title),
            None => self.title.clone(),
        }
    }
}
