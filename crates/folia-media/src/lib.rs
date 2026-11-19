// Safe desktop media metadata and commands, independent of the audio engine.
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

#[cfg(target_os = "linux")]
mod artwork;
#[cfg(target_os = "linux")]
pub mod mpris;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Raise,
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    Volume(f64),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Stopped,
    Playing,
    Paused,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Playing => "Playing",
            Self::Paused => "Paused",
        }
    }
}

/// Only public music tags cross the session bus; source URLs and paths are never retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    track_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub length: i64,
}

impl Metadata {
    pub fn track_id(&self) -> &str {
        &self.track_id
    }

    pub fn from_track(track: &folia_core::Track, duration: f64) -> Self {
        let mut hash = DefaultHasher::new();
        if let Some(online) = &track.online {
            online.hash(&mut hash);
        } else {
            track.id.hash(&mut hash);
            track.path.hash(&mut hash);
        }
        let artists = track.online_info.as_ref().map(|info| {
            info.artists.iter().filter(|artist| !artist.name.is_empty()).map(|artist| artist.name.clone()).collect()
        });
        let artists = artists
            .filter(|names: &Vec<String>| !names.is_empty())
            .unwrap_or_else(|| track.artist.iter().filter(|artist| !artist.is_empty()).cloned().collect());
        Self {
            track_id: format!("/org/mpris/MediaPlayer2/track/t{:016x}", hash.finish()),
            title: track.title.clone(),
            artists,
            album: track.album.clone(),
            length: microseconds(duration),
        }
    }
}

/// Decoded pixels only: encoding strips original tags and all remote addresses.
#[derive(Clone, Debug)]
pub struct Artwork {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub metadata: Option<Metadata>,
    pub artwork: Option<Arc<Artwork>>,
    pub status: Status,
    pub volume: f64,
    pub position: i64,
    pub sampled_at: Instant,
    pub advancing: bool,
    pub seek_revision: u64,
    pub can_next: bool,
    pub can_previous: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            metadata: None,
            artwork: None,
            status: Status::Stopped,
            volume: 1.0,
            position: 0,
            sampled_at: Instant::now(),
            advancing: false,
            seek_revision: 0,
            can_next: false,
            can_previous: false,
        }
    }
}

impl Snapshot {
    pub fn position_at(&self, now: Instant) -> i64 {
        let Some(metadata) = &self.metadata else { return 0 };
        let elapsed = if self.status == Status::Playing && self.advancing {
            microseconds(now.saturating_duration_since(self.sampled_at).as_secs_f64())
        } else {
            0
        };
        let position = self.position.max(0).saturating_add(elapsed);
        if metadata.length > 0 { position.min(metadata.length) } else { position }
    }
}

pub fn microseconds(seconds: f64) -> i64 {
    if seconds.is_finite() { (seconds.max(0.0) * 1_000_000.0) as i64 } else { 0 }
}
