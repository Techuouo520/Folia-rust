// Durable playback contents and media position, separate from player preferences.
use folia_core::{QueueSnapshot, Track};
use serde::{Deserialize, Serialize};

use crate::Domain;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackSession {
    pub queue: QueueSnapshot<Track>,
    pub position: f64,
    pub fm: Option<FmSession>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FmSession {
    pub provider: String,
    pub id: String,
    pub name: String,
}

impl Domain for PlaybackSession {
    const FILE: &'static str = "playback_session.toml";
    const SCHEMA_VERSION: u32 = 1;
}

impl PlaybackSession {
    /// Discards invalid indices and non-finite media times from an edited or damaged session.
    pub fn normalized(mut self) -> Self {
        self.queue.current = self.queue.current.filter(|&index| index < self.queue.items.len());
        if self.queue.current.is_some() {
            self.queue.detached = None;
        }
        for track in self.queue.items.iter_mut().chain(self.queue.detached.iter_mut()) {
            if track.online.is_some() {
                track.path.clear();
            }
            if !track.duration.is_finite() || track.duration < 0.0 {
                track.duration = 0.0;
            }
        }
        let current = self.queue.current.and_then(|index| self.queue.items.get(index)).or(self.queue.detached.as_ref());
        if !self.position.is_finite() || self.position < 0.0 || current.is_none() {
            self.position = 0.0;
        } else if let Some(track) = current.filter(|track| track.duration > 0.0) {
            self.position = self.position.min(track.duration);
        }
        self.fm = self.fm.filter(|fm| {
            !fm.provider.is_empty()
                && current.and_then(|track| track.online.as_ref()).is_some_and(|song| song.provider == fm.provider)
        });
        self
    }
}
