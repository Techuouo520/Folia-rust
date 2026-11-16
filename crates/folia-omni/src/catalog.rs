// Exact platform catalog references and the artist navigation facade.

use folia_core::{ArtistRef, OnlineId, Track};

use crate::{Collection, CollectionKind, Omni, OmniError, Page};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Artist {
    pub provider: String,
    pub id: String,
    pub name: String,
    pub picture_url: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackCatalog {
    pub artists: Vec<ArtistRef>,
    pub album: Option<Collection>,
}

impl TrackCatalog {
    /// Uses retained platform identifiers without interpreting display credits as identifiers.
    pub fn from_track(track: &Track) -> Self {
        let (Some(song), Some(info)) = (&track.online, &track.online_info) else {
            return Self::default();
        };
        Self {
            artists: info.artists.iter().filter(|artist| !artist.id.is_empty()).cloned().collect(),
            album: (!info.album_id.is_empty()).then(|| Collection {
                provider: song.provider.clone(),
                id: info.album_id.clone(),
                kind: CollectionKind::Album,
                name: track.album.clone().unwrap_or_default(),
                cover_url: track.cover_url.clone(),
                track_count: None,
                creator: track.album_artist.clone().or_else(|| track.artist.clone()),
                description: None,
                owned: false,
            }),
        }
    }
}

impl Omni {
    pub async fn search_artists(
        &self,
        provider: &str,
        query: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Page<Artist>, OmniError> {
        self.provider(provider)?.search_artists(query, limit, offset).await
    }

    pub async fn artist_detail(&self, artist: &OnlineId) -> Result<Artist, OmniError> {
        self.provider(&artist.provider)?.artist_detail(artist).await
    }

    pub async fn artist_tracks(&self, artist: &OnlineId, limit: u32, offset: u32) -> Result<Page<Track>, OmniError> {
        self.provider(&artist.provider)?.artist_tracks(artist, limit, offset).await
    }

    pub async fn artist_albums(
        &self,
        artist: &OnlineId,
        limit: u32,
        offset: u32,
    ) -> Result<Page<Collection>, OmniError> {
        self.provider(&artist.provider)?.artist_albums(artist, limit, offset).await
    }

    pub async fn track_catalog(&self, track: &Track) -> Result<TrackCatalog, OmniError> {
        let song = track.online.as_ref().ok_or(OmniError::Unavailable)?;
        self.provider(&song.provider)?.track_catalog(track).await
    }

    pub async fn album(&self, album: &OnlineId) -> Result<Collection, OmniError> {
        self.provider(&album.provider)?.album(album).await
    }
}
