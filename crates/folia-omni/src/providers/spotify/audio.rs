// Spotify's own audio through librespot (MIT): an access-point session with the account's token, the track's
// Ogg Vorbis file from the CDN, decrypted with its audio key, and Spotify's lead before `OggS` dropped.

use std::io::Read;

use folia_core::AudioQuality;
use librespot_audio::{AudioDecrypt, AudioFile};
use librespot_core::authentication::Credentials;
use librespot_core::{Session, SessionConfig, SpotifyId, SpotifyUri};
use librespot_metadata::audio::AudioFileFormat;
use librespot_metadata::{Metadata, Track};

use crate::OmniError;

/// `OGG_LEAD_BYTES`: Spotify's header before the Ogg stream.
const OGG_LEAD: usize = 0xa7;
/// `BITRATES`: `standard` is 160 kbit/s; `high` asks for 320 and falls back to 160.
const STANDARD: [AudioFileFormat; 1] = [AudioFileFormat::OGG_VORBIS_160];
const HIGH: [AudioFileFormat; 2] = [AudioFileFormat::OGG_VORBIS_320, AudioFileFormat::OGG_VORBIS_160];

fn formats(quality: AudioQuality) -> &'static [AudioFileFormat] {
    match quality {
        AudioQuality::Standard => &STANDARD,
        _ => &HIGH,
    }
}

/// One connected session, kept while its token is the one it was opened with.
pub struct Player {
    session: tokio::sync::Mutex<Option<(String, Session)>>,
}

impl Default for Player {
    fn default() -> Self {
        Self { session: tokio::sync::Mutex::new(None) }
    }
}

impl Player {
    /// Drops the session (sign-in, sign-out).
    pub async fn clear(&self) {
        if let Some((_, session)) = self.session.lock().await.take() {
            session.shutdown();
        }
    }

    /// The whole decrypted file of the track `id` (base62), at `quality` or the nearest file Spotify has.
    pub async fn file(
        &self,
        token: &str,
        id: &str,
        quality: AudioQuality,
    ) -> Result<(Vec<u8>, AudioQuality), OmniError> {
        let session = self.connect(token).await?;
        let track_id =
            SpotifyId::from_base62(id).map_err(|err| OmniError::Response(format!("not a Spotify track: {err}")))?;
        let track = Track::get(&session, &SpotifyUri::Track { id: track_id }).await.map_err(spotify_error)?;
        let (format, file_id) = formats(quality)
            .iter()
            .find_map(|format| track.files.get(format).map(|file| (*format, *file)))
            .ok_or(OmniError::Unavailable)?;
        let key = session.audio_key().request(track_id, file_id).await.map_err(spotify_error)?;
        let (bitrate, level) = if format == AudioFileFormat::OGG_VORBIS_320 {
            (320, AudioQuality::High)
        } else {
            (160, AudioQuality::Standard)
        };
        let encrypted = AudioFile::open(&session, file_id, bitrate * 1024 / 8).await.map_err(spotify_error)?;
        let bytes = tokio::task::spawn_blocking(move || {
            let mut decrypted = AudioDecrypt::new(Some(key), encrypted);
            let mut bytes = Vec::new();
            decrypted.read_to_end(&mut bytes).map(|_| bytes)
        })
        .await
        .map_err(|err| OmniError::Response(format!("spotify audio reader stopped: {err}")))?
        .map_err(|err| OmniError::Response(format!("spotify audio did not download: {err}")))?;
        let start = if bytes.get(OGG_LEAD..OGG_LEAD + 4) == Some(b"OggS") {
            OGG_LEAD
        } else {
            bytes.windows(4).position(|w| w == b"OggS").unwrap_or(0)
        };
        tracing::info!(bitrate, level = level.id(), bytes = bytes.len(), "spotify audio decrypted");
        Ok((bytes[start..].to_vec(), level))
    }

    /// A session connected with `token`, reused while the token stands.
    async fn connect(&self, token: &str) -> Result<Session, OmniError> {
        let mut kept = self.session.lock().await;
        if let Some((held, session)) = kept.as_ref()
            && held == token
            && !session.is_invalid()
        {
            return Ok(session.clone());
        }
        if let Some((_, old)) = kept.take() {
            old.shutdown();
        }
        let config = SessionConfig { client_id: super::signin::CLIENT_ID.to_owned(), ..SessionConfig::default() };
        let session = Session::new(config, None);
        session.connect(Credentials::with_access_token(token), false).await.map_err(spotify_error)?;
        tracing::info!("spotify audio session connected");
        *kept = Some((token.to_owned(), session.clone()));
        Ok(session)
    }
}

fn spotify_error(err: librespot_core::Error) -> OmniError {
    tracing::warn!(error = %err, "spotify audio refused");
    OmniError::Response(format!("Spotify audio: {err}"))
}
