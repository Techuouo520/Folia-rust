// Online music: one facade (`Omni`) over every platform provider; only providers talk to platform APIs.

pub mod lx;
mod paging;
mod providers;
mod qr;
mod reads;
mod session;

use std::future::Future;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use folia_core::{AudioQuality, ChorusRange, Lyrics, OnlineId, Track, UNLISTED_AUDIO_QUALITY_LEVELS};
use folia_lyrics::{ChorusSource, resolve_chorus};

pub use paging::{
    BodianCursor, bodian_search_page, kugou_search_page, netease_search_page, qishui_search_continues,
    qq_anonymous_search_page, qq_session_search_page, spotify_search_done, spotify_search_page, ytm_search_page,
    ytm_walk_done,
};
pub use qr::LoginQr;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, thiserror::Error)]
pub enum OmniError {
    #[error("unknown provider {0}")]
    UnknownProvider(String),
    #[error("request failed")]
    Http(#[from] reqwest::Error),
    #[error("platform answered with code {0}")]
    Platform(i64),
    #[error("unexpected response: {0}")]
    Response(String),
    #[error("song is not playable")]
    Unavailable,
    #[error("a paid subscription is required to play this song")]
    SubscriptionRequired,
    #[error("not signed in")]
    SignedOut,
    /// The platform has no such capability in the original either.
    #[error("{0} is not supported on this platform")]
    Unsupported(&'static str),
    #[error("cannot draw the sign-in code")]
    Qr(#[from] qrcode::types::QrError),
    #[error("cannot read the sign-in code picture: {0}")]
    QrImage(String),
    #[error("system keyring refused")]
    Keyring(#[from] keyring::Error),
    #[error("cannot write {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: Option<u64>,
    /// Another page follows (`hasMore`).
    pub has_more: bool,
    /// The offset the next page is requested at (`nextOffset`).
    pub next_offset: u32,
}

impl<T> Page<T> {
    /// An empty page whose next request would repeat `offset`.
    pub fn empty(offset: u32) -> Self {
        Self { items: Vec::new(), total: None, has_more: false, next_offset: offset }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioSource {
    pub url: String,
    /// File extension of the stream, such as `mp3` or `flac`.
    pub extension: String,
    /// The platform only gave a preview clip, not the whole song.
    pub preview: bool,
    /// The level the platform actually gave, which is lower than the one asked for when it fell back.
    pub level: AudioQuality,
}

/// A song's whole file as the platform hands it out, decrypted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFile {
    pub bytes: Vec<u8>,
    /// File extension, such as `ogg`.
    pub extension: String,
    /// The level the platform actually gave.
    pub level: AudioQuality,
}

/// One platform the original lists in its sign-in window, with its mark: a letter on a colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    pub id: &'static str,
    pub display_name: &'static str,
    pub short_name: &'static str,
    pub letter: char,
    /// `0xRRGGBB`.
    pub color: u32,
    /// The provider is compiled into this build.
    pub available: bool,
    /// The platform's own sign-in words (`getLoginTexts`), as i18n keys.
    pub login_title: &'static str,
    pub login_note: &'static str,
}

/// The original's providers in registration order (`providerRegistry.ts`), with the marks and sign-in words they declare.
const PLATFORMS: [(&str, &str, &str, char, u32, &str, &str); 7] = [
    ("netease", "NetEase Cloud Music", "网易云", '云', 0xdc2626, "home.loginTitle", "home.loginNote"),
    ("kugou", "KuGou Music", "酷狗", 'K', 0x2563eb, "home.loginTitleKugou", "home.loginNoteKugou"),
    ("qq", "QQ Music", "QQ音乐", 'Q', 0x16a34a, "home.loginTitleQq", "home.loginNoteQq"),
    ("bodian", "波点音乐", "波点", '波', 0x0d9488, "home.loginTitleBodian", "home.loginNoteBodian"),
    ("ytm", "YouTube Music", "YT Music", 'Y', 0xe11d48, "home.loginTitleYtm", "home.loginNoteYtmBrowser"),
    ("spotify", "Spotify", "Spotify", 'S', 0x22c55e, "home.loginTitleSpotify", "home.loginNoteSpotify"),
    ("qishui", "汽水音乐", "汽水", '汽', 0x19a58c, "home.loginTitleQishui", "home.loginNoteQishui"),
];

/// A QR sign-in session: `key` is what the platform is asked about, `content` what the phone scans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginCode {
    pub key: String,
    pub content: QrContent,
}

/// What a sign-in code holds: an address to encode, or the platform's own picture of the code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrContent {
    Url(String),
    /// A PNG of the code as the platform drew it; its modules are read back from the picture.
    Png(Vec<u8>),
    /// A page to open in the system browser; the platform waits for the answer it sends back.
    Browser(String),
}

/// What the platform says about a sign-in code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginState {
    Waiting,
    Scanned,
    /// The phone confirmed; the provider now holds the session.
    Confirmed,
    Expired,
    Failed {
        code: i64,
        message: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
    /// The account is a paying member (`vipType` above zero), where the platform says so.
    pub vip: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CollectionKind {
    Playlist,
    Album,
    /// The account's daily songs.
    DailySongs,
    /// Personal FM: played straight away, not opened.
    PersonalFm,
}

/// A playlist or an album on a platform, as a home card shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub provider: String,
    pub id: String,
    pub kind: CollectionKind,
    pub name: String,
    pub cover_url: Option<String>,
    pub track_count: Option<u32>,
    /// The playlist's creator, or the album's artists joined with `, `.
    pub creator: Option<String>,
    pub description: Option<String>,
    /// The signed-in account made it, so its songs can be changed (`isOwned`).
    pub owned: bool,
}

impl Collection {
    /// A stable negative id for the wall, apart from track ids.
    pub fn wall_id(&self) -> i64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (&self.provider, &self.id, self.kind).hash(&mut hasher);
        -((hasher.finish() >> 1) as i64) - 1
    }
}

/// A collection's songs and the account's relation to it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CollectionDetail {
    pub tracks: Vec<Track>,
    /// Whether the account saved it; `None` where saving does not apply (daily songs, personal FM).
    pub subscribed: Option<bool>,
    /// The playlist creator's user id: an account cannot save its own playlist.
    pub owner: Option<String>,
    /// An album's release time, milliseconds since the Unix epoch (`publishedAt`).
    pub published: Option<i64>,
}

/// The radio tab: personal FM, the daily songs and the recommended playlists (`getHomeFeed`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RadioFeed {
    pub personal_fm: Vec<Track>,
    pub daily_songs: Vec<Track>,
    pub recommended: Vec<Collection>,
    /// One part failed and is empty.
    pub partial: bool,
}

/// Whose heading a shelf of the explore page carries: the platform's, or one the app names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shelf {
    DailySongs,
    DailyPlaylists,
    Charts,
    Block(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExploreEntry {
    Song(Track),
    Collection(Collection),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExploreSection {
    pub key: String,
    pub shelf: Shelf,
    pub entries: Vec<ExploreEntry>,
}

/// A platform's lyrics and the raw text chorus detection reads (`mainText`, else `wordByWordText`); `None` reads
/// the lines themselves.
pub struct ProviderLyrics {
    pub lyrics: Lyrics,
    pub source: Option<String>,
}

/// What one platform can do. Search results are tracks whose `online` id points back at the platform.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// `likes`: songs can be liked and the account's liked ids read.
    pub likes: bool,
    /// `playlistTrackMutations`: a song can be added to one of the account's own playlists.
    pub playlist_track_mutations: bool,
    /// `mutations.createPlaylist`: a playlist can be made on the spot.
    pub create_playlist: bool,
}

pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    /// Only the provider interprets its identifiers and maps them to an LX platform.
    fn lx_music_info(&self, _track: &Track) -> Option<lx::MusicInfo> {
        None
    }
    fn search_songs<'a>(
        &'a self,
        query: &'a str,
        limit: u32,
        offset: u32,
    ) -> BoxFuture<'a, Result<Page<Track>, OmniError>>;
    /// The levels this platform tells apart, lowest first; without one it is `UNLISTED_AUDIO_QUALITY_LEVELS`.
    fn audio_quality_levels(&self) -> &'static [AudioQuality] {
        &UNLISTED_AUDIO_QUALITY_LEVELS
    }
    fn audio_source<'a>(
        &'a self,
        song: &'a OnlineId,
        quality: AudioQuality,
    ) -> BoxFuture<'a, Result<AudioSource, OmniError>>;
    /// The song's file when the platform hands it out itself (decrypted) instead of at an address; `None` for
    /// platforms whose `audio_source` is downloaded.
    fn audio_file<'a>(
        &'a self,
        _song: &'a OnlineId,
        _quality: AudioQuality,
    ) -> Option<BoxFuture<'a, Result<AudioFile, OmniError>>> {
        None
    }
    fn lyrics<'a>(&'a self, song: &'a OnlineId) -> BoxFuture<'a, Result<Option<ProviderLyrics>, OmniError>>;
    /// The stretches the platform marks as chorus (`getChorusRanges`); none where the platform has no such data.
    fn chorus_ranges<'a>(&'a self, _song: &'a OnlineId) -> BoxFuture<'a, Result<Vec<ChorusRange>, OmniError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn login_code(&self) -> BoxFuture<'_, Result<LoginCode, OmniError>>;
    fn check_login<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<LoginState, OmniError>>;
    /// The signed-in account, or `None` when the session is missing or ran out.
    fn account(&self) -> BoxFuture<'_, Result<Option<Account>, OmniError>>;
    /// Tells the platform and drops the session held in memory, whatever it answers.
    fn logout(&self) -> BoxFuture<'_, Result<(), OmniError>>;
    /// The session to keep, serialised; `None` when nobody is signed in.
    fn session(&self) -> Option<String>;
    fn restore_session(&self, saved: &str) -> Result<(), OmniError>;
    fn user_playlists<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Result<Vec<Collection>, OmniError>>;
    fn collection_detail<'a>(
        &'a self,
        collection: &'a Collection,
    ) -> BoxFuture<'a, Result<CollectionDetail, OmniError>>;
    /// Saves (`true`) or unsaves a playlist or an album for the signed-in account.
    fn set_subscribed<'a>(
        &'a self,
        collection: &'a Collection,
        subscribed: bool,
    ) -> BoxFuture<'a, Result<(), OmniError>>;
    fn favorite_albums(&self) -> BoxFuture<'_, Result<Vec<Collection>, OmniError>>;
    fn radio_feed(&self, limit: u32) -> BoxFuture<'_, Result<RadioFeed, OmniError>>;
    /// Whether the platform has daily songs at all (`supportsDailySongs`); the radio tab shows no card otherwise.
    fn has_daily_songs(&self) -> bool {
        true
    }
    fn explore(&self) -> BoxFuture<'_, Result<Vec<ExploreSection>, OmniError>>;
    /// The address of `url` resized to `px` square on the platform's image CDN, when `url` is on it.
    fn sized_cover(&self, url: &str, px: u32) -> Option<String>;
    /// What the platform does with songs and playlists; all false unless a provider implements it.
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
    /// Likes or unlikes a song for the signed-in account (`mutations.likeSong`).
    fn like_song<'a>(&'a self, _song: &'a OnlineId, _liked: bool) -> BoxFuture<'a, Result<(), OmniError>> {
        Box::pin(async { Err(OmniError::Unsupported("liking songs")) })
    }
    /// The ids the account's liked list holds (`library.getLikedSongIds`); `None` where the platform reads none.
    fn liked_song_ids(&self) -> BoxFuture<'_, Result<Option<Vec<String>>, OmniError>> {
        Box::pin(async { Ok(None) })
    }
    /// Adds (`add`) songs to or removes them from one of the account's own playlists
    /// (`mutations.updatePlaylistTracks`). The songs carry the ids this provider gave them.
    fn update_playlist_tracks<'a>(
        &'a self,
        _playlist: &'a Collection,
        _songs: &'a [OnlineId],
        _add: bool,
    ) -> BoxFuture<'a, Result<(), OmniError>> {
        Box::pin(async { Err(OmniError::Unsupported("playlist track mutations")) })
    }
    /// Makes a playlist for the signed-in account (`mutations.createPlaylist`).
    fn create_playlist(&self, _name: &str) -> BoxFuture<'_, Result<Collection, OmniError>> {
        Box::pin(async { Err(OmniError::Unsupported("creating playlists")) })
    }
}

/// Cover downloads at once: what a browser opens to one host, so the first screen of tiles does not queue dozens.
const COVER_DOWNLOADS: usize = 6;

#[derive(Clone)]
pub struct Omni {
    providers: Arc<Vec<Box<dyn Provider>>>,
    http: reqwest::Client,
    cache_dir: PathBuf,
    keyring_service: String,
    covers: Arc<tokio::sync::Semaphore>,
    sources: Arc<lx::Sources>,
}

impl Omni {
    /// Registers every provider compiled in; downloads go under `cache_dir/audio`, sessions under `keyring_service`.
    pub fn new(cache_dir: PathBuf, keyring_service: String) -> Result<Self, OmniError> {
        let http = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36")
            .build()?;
        let covers = Arc::new(tokio::sync::Semaphore::new(COVER_DOWNLOADS));
        Ok(Self {
            providers: Arc::new(providers::all(&http)),
            http,
            cache_dir,
            keyring_service,
            covers,
            sources: Arc::default(),
        })
    }

    /// Uses an explicit provider registry, for embedders supplying their own native platforms.
    pub fn with_providers(
        cache_dir: PathBuf,
        keyring_service: String,
        providers: Vec<Box<dyn Provider>>,
    ) -> Result<Self, OmniError> {
        let mut omni = Self::new(cache_dir, keyring_service)?;
        omni.providers = Arc::new(providers);
        Ok(omni)
    }

    /// Loads and migrates the source domain, then starts every enabled script independently on background workers.
    pub fn restore_sources(
        &self,
        store: folia_store::Store,
        data_dir: PathBuf,
    ) -> Result<lx::SourceSettings, lx::SourceError> {
        self.sources.restore(store, data_dir, self.http.clone())
    }

    pub fn custom_sources(&self) -> lx::SourceSettings {
        self.sources.list()
    }

    pub fn import_source(&self, path: &Path) -> Result<lx::SourceSettings, lx::SourceError> {
        self.sources.import(path, self.http.clone())
    }

    /// Downloads an HTTP(S) script and imports it through the same bounded validation as a file.
    pub async fn import_source_url(&self, url: &str) -> Result<lx::SourceSettings, lx::SourceError> {
        let url = url.trim();
        if url.len() > 8192 || url.chars().any(char::is_whitespace) {
            return Err(lx::SourceError::Url);
        }
        let url = reqwest::Url::parse(url).map_err(|_| lx::SourceError::Url)?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(lx::SourceError::Url);
        }
        let download_error = |err: reqwest::Error| {
            if err.is_timeout() { lx::SourceError::Timeout } else { lx::SourceError::Http(err.without_url()) }
        };
        let mut response = self
            .http
            .get(url)
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(download_error)?;
        if response.content_length().is_some_and(|n| n > lx::SCRIPT_BYTES as u64) {
            return Err(lx::SourceError::Limit);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(download_error)? {
            if bytes.len() + chunk.len() > lx::SCRIPT_BYTES {
                return Err(lx::SourceError::Limit);
            }
            bytes.extend_from_slice(&chunk);
        }
        let code = String::from_utf8(bytes).map_err(|_| lx::SourceError::Header)?;
        let (sources, http) = (self.sources.clone(), self.http.clone());
        tokio::task::spawn_blocking(move || sources.import_text(code, http))
            .await
            .map_err(|_| lx::SourceError::Stopped)?
    }

    pub fn enable_source(&self, id: &str, enabled: bool) -> Result<lx::SourceSettings, lx::SourceError> {
        self.sources.enable(id, enabled, self.http.clone())
    }

    /// Changes the source's position in the playback fallback order without restarting any script.
    pub fn move_source(&self, id: &str, index: usize) -> Result<lx::SourceSettings, lx::SourceError> {
        self.sources.move_to(id, index)
    }

    pub fn remove_source(&self, id: &str) -> Result<lx::SourceSettings, lx::SourceError> {
        self.sources.remove(id)
    }

    pub fn source_update_alerts(&self) -> Vec<(String, lx::UpdateAlert)> {
        self.sources.take_alerts()
    }

    /// Tries enabled scripts in list order, lowering quality within each before moving to the next source.
    pub async fn resolve_audio_source(
        &self,
        track: &Track,
        preference: AudioQuality,
    ) -> Result<AudioSource, OmniError> {
        let song = track.online.as_ref().ok_or(OmniError::Unavailable)?;
        let provider = self.provider(&song.provider)?;
        if let Some(info) = provider.lx_music_info(track) {
            for (id, script) in self.sources.active(info.source) {
                if let Some(source) = Self::custom_audio_source(&id, &script, &info, preference).await {
                    return Ok(source);
                }
            }
        }
        tracing::info!(provider = song.provider, "resolving audio through native platform");
        provider.audio_source(song, resolve_quality(provider.audio_quality_levels(), preference)).await
    }

    async fn custom_audio_source(
        id: &str,
        script: &lx::Script,
        info: &lx::MusicInfo,
        preference: AudioQuality,
    ) -> Option<AudioSource> {
        let capability = script.sources().get(info.source)?;
        for quality in folia_core::UNLISTED_AUDIO_QUALITY_LEVELS.into_iter().rev().filter(|q| *q <= preference) {
            let name = lx::quality_name(quality);
            if !capability.qualitys.iter().any(|q| q == name) {
                continue;
            }
            match script.music_url(info.source, name, info.data.clone()).await {
                Ok(url) => {
                    tracing::info!(
                        source = id,
                        provider = info.source,
                        quality = name,
                        "audio resolved through custom source"
                    );
                    return Some(AudioSource {
                        url,
                        extension: if quality >= AudioQuality::Lossless { "flac" } else { "mp3" }.into(),
                        preview: false,
                        level: quality,
                    });
                }
                Err(err) => {
                    tracing::warn!(
                        source = id,
                        provider = info.source,
                        quality = name,
                        error = &err as &dyn std::error::Error,
                        "custom source failed; lowering quality or trying the next source"
                    );
                    if matches!(err, lx::SourceError::Timeout | lx::SourceError::Memory | lx::SourceError::Stopped) {
                        break;
                    }
                }
            }
        }
        tracing::info!(
            source = id,
            provider = info.source,
            "custom source quality chain exhausted; trying the next source"
        );
        None
    }

    /// `(id, display name)` of each provider, in registration order.
    pub fn providers(&self) -> Vec<(&'static str, &'static str)> {
        self.providers.iter().map(|provider| (provider.id(), provider.display_name())).collect()
    }

    /// Every platform the original lists, marked available when a provider for it is compiled in.
    pub fn platforms(&self) -> Vec<Platform> {
        PLATFORMS
            .iter()
            .filter(|&&(id, ..)| self.providers.iter().any(|provider| provider.id() == id))
            .map(|&(id, display_name, short_name, letter, color, login_title, login_note)| Platform {
                id,
                display_name,
                short_name,
                letter,
                color,
                available: true,
                login_title,
                login_note,
            })
            .collect()
    }

    pub async fn search(&self, provider: &str, query: &str, limit: u32, offset: u32) -> Result<Page<Track>, OmniError> {
        self.provider(provider)?.search_songs(query, limit, offset).await
    }

    /// The song's lyrics with chorus marks resolved as `getLyrics` does: existing marks, the platform's ranges, or
    /// the most repeated lines.
    pub async fn lyrics(&self, song: &OnlineId) -> Result<Option<Lyrics>, OmniError> {
        let Some(found) = self.provider(&song.provider)?.lyrics(song).await? else {
            log_chorus(song, ChorusSource::None, 0);
            return Ok(None);
        };
        let ProviderLyrics { mut lyrics, source } = found;
        let ranges = self.chorus_ranges(song).await.unwrap_or_else(|err| {
            tracing::warn!(
                provider = %song.provider,
                song = %song.id,
                error = &err as &dyn std::error::Error,
                "native chorus request failed; falling back to text detection"
            );
            Vec::new()
        });
        let mode = resolve_chorus(Some(&mut lyrics), &ranges, source.as_deref());
        log_chorus(song, mode, ranges.len());
        Ok(Some(lyrics))
    }

    /// The stretches the song's platform marks as chorus (`getChorusRanges`).
    pub async fn chorus_ranges(&self, song: &OnlineId) -> Result<Vec<ChorusRange>, OmniError> {
        self.provider(&song.provider)?.chorus_ranges(song).await
    }

    /// Asks for a sign-in code and draws it as a QR matrix.
    pub async fn login_qr(&self, provider: &str) -> Result<LoginQr, OmniError> {
        let code = self.provider(provider)?.login_code().await?;
        match code.content {
            QrContent::Url(url) => LoginQr::encode(code.key, &url),
            QrContent::Png(png) => LoginQr::decode_png(code.key, &png),
            QrContent::Browser(url) => Ok(LoginQr::browser(code.key, url)),
        }
    }

    pub async fn check_login(&self, provider: &str, key: &str) -> Result<LoginState, OmniError> {
        self.provider(provider)?.check_login(key).await
    }

    pub async fn account(&self, provider: &str) -> Result<Option<Account>, OmniError> {
        self.provider(provider)?.account().await
    }

    /// The sync button's read (`readAccount`): who is signed in and the playlists kept with the account.
    pub async fn sync_account(&self, provider: &str) -> Result<Option<(Account, Vec<Collection>)>, OmniError> {
        let target = self.provider(provider)?;
        let Some(account) = target.account().await? else { return Ok(None) };
        let playlists = target.user_playlists(&account.id).await?;
        Ok(Some((account, playlists)))
    }

    pub async fn logout(&self, provider: &str) -> Result<(), OmniError> {
        self.provider(provider)?.logout().await
    }

    /// Reads the kept session from the system keyring into the provider. Blocking: call it off the UI thread.
    pub fn restore_session(&self, provider: &str) -> Result<bool, OmniError> {
        let target = self.provider(provider)?;
        match session::load(&self.keyring_service, provider)? {
            Some(saved) => {
                target.restore_session(&saved)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Keeps the provider's session in the system keyring, or removes it when signed out. Blocking.
    pub fn save_session(&self, provider: &str) -> Result<(), OmniError> {
        match self.provider(provider)?.session() {
            Some(value) => session::save(&self.keyring_service, provider, &value),
            None => session::delete(&self.keyring_service, provider),
        }
    }

    /// Removes the kept session from the system keyring. Blocking.
    pub fn forget_session(&self, provider: &str) -> Result<(), OmniError> {
        session::delete(&self.keyring_service, provider)
    }

    pub async fn user_playlists(&self, provider: &str, user: &str) -> Result<Vec<Collection>, OmniError> {
        self.provider(provider)?.user_playlists(user).await
    }

    pub async fn collection_detail(&self, collection: &Collection) -> Result<CollectionDetail, OmniError> {
        self.provider(&collection.provider)?.collection_detail(collection).await
    }

    pub async fn set_subscribed(&self, collection: &Collection, subscribed: bool) -> Result<(), OmniError> {
        self.provider(&collection.provider)?.set_subscribed(collection, subscribed).await
    }

    pub async fn favorite_albums(&self, provider: &str) -> Result<Vec<Collection>, OmniError> {
        self.provider(provider)?.favorite_albums().await
    }

    pub async fn radio_feed(&self, provider: &str, limit: u32) -> Result<RadioFeed, OmniError> {
        self.provider(provider)?.radio_feed(limit).await
    }

    pub fn has_daily_songs(&self, provider: &str) -> bool {
        self.provider(provider).is_ok_and(|found| found.has_daily_songs())
    }

    /// The levels `provider` tells apart, lowest first; empty when it is not compiled in.
    pub fn audio_quality_levels(&self, provider: &str) -> &'static [AudioQuality] {
        self.provider(provider).map_or(&[], |found| found.audio_quality_levels())
    }

    /// The level `preference` means to `provider` (`audioQualityFor`): the nearest level it lists at or below, or its
    /// lowest where it lists none below.
    pub fn resolve_audio_quality(&self, provider: &str, preference: AudioQuality) -> AudioQuality {
        resolve_quality(self.audio_quality_levels(provider), preference)
    }

    pub async fn explore(&self, provider: &str) -> Result<Vec<ExploreSection>, OmniError> {
        self.provider(provider)?.explore().await
    }

    /// What `provider` does with songs and playlists.
    pub fn capabilities(&self, provider: &str) -> Capabilities {
        self.provider(provider).map_or(Capabilities::default(), |found| found.capabilities())
    }

    /// `canLikeSong`: the song's platform takes hearts.
    pub fn can_like(&self, song: &OnlineId) -> bool {
        self.capabilities(&song.provider).likes
    }

    /// `canAddSongToPlaylist`.
    pub fn can_add_to_playlist(&self, song: &OnlineId) -> bool {
        self.capabilities(&song.provider).playlist_track_mutations
    }

    /// `canCreatePlaylistForSong`.
    pub fn can_create_playlist(&self, song: &OnlineId) -> bool {
        let capabilities = self.capabilities(&song.provider);
        capabilities.playlist_track_mutations && capabilities.create_playlist
    }

    pub async fn like_song(&self, song: &OnlineId, liked: bool) -> Result<(), OmniError> {
        let provider = self.provider(&song.provider)?;
        if !provider.capabilities().likes {
            return Err(OmniError::Unsupported("liking songs"));
        }
        provider.like_song(song, liked).await
    }

    /// The account's liked song ids, when the platform can read them.
    pub async fn liked_song_ids(&self, provider: &str) -> Result<Option<Vec<String>>, OmniError> {
        self.provider(provider)?.liked_song_ids().await
    }

    /// `addSongToPlaylist` and the liked list's edits: the songs go in or come out of one of the account's own
    /// playlists.
    pub async fn update_playlist_tracks(
        &self,
        playlist: &Collection,
        songs: &[OnlineId],
        add: bool,
    ) -> Result<(), OmniError> {
        if songs.is_empty() {
            return Ok(());
        }
        let provider = self.provider(&playlist.provider)?;
        if playlist.kind != CollectionKind::Playlist || !provider.capabilities().playlist_track_mutations {
            return Err(OmniError::Unsupported("playlist track mutations"));
        }
        if songs.iter().any(|song| song.provider != playlist.provider) {
            return Err(OmniError::Unsupported("songs of another platform in this playlist"));
        }
        provider.update_playlist_tracks(playlist, songs, add).await
    }

    /// `createPlaylistWithSong`'s first half.
    pub async fn create_playlist(&self, provider: &str, name: &str) -> Result<Collection, OmniError> {
        self.provider(provider)?.create_playlist(name).await
    }

    /// Downloads the song once into the cache at the resolved level and returns the file to play. Cached files are
    /// kept under the level asked for, so a platform that falls back to a lower one is found again next time; a file
    /// from before levels were recorded plays for any preference, and a preview is never reused.
    pub async fn fetch_audio(&self, song: &OnlineId, preference: AudioQuality) -> Result<PathBuf, OmniError> {
        self.fetch_track_audio(&Track { online: Some(song.clone()), ..Track::default() }, preference).await
    }

    /// Keeps the song metadata available to custom sources, then uses the shared download cache.
    pub async fn fetch_track_audio(&self, track: &Track, preference: AudioQuality) -> Result<PathBuf, OmniError> {
        let song = track.online.as_ref().ok_or(OmniError::Unavailable)?;
        let provider = self.provider(&song.provider)?;
        let level = resolve_quality(provider.audio_quality_levels(), preference);
        let dir = self.cache_dir.join("audio");
        let stem = format!("{}-{}", song.provider, song.id);
        if let Some(info) = provider.lx_music_info(track) {
            for (id, script) in self.sources.active(info.source) {
                let custom_stem = format!("{stem}-lx-{id}-{}", preference.id());
                if let Some(cached) = cached_file(&dir, &custom_stem) {
                    return Ok(cached);
                }
                if let Some(source) = Self::custom_audio_source(&id, &script, &info, preference).await {
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(60),
                        self.cache_audio_source(&dir, &custom_stem, source),
                    )
                    .await
                    {
                        Ok(Ok(path)) => return Ok(path),
                        Ok(Err(err)) => tracing::warn!(
                            source = id,
                            provider = song.provider,
                            error = &err as &dyn std::error::Error,
                            "custom source download failed; trying the next source"
                        ),
                        Err(err) => tracing::warn!(
                            source = id,
                            error = &err as &dyn std::error::Error,
                            "custom source download timed out; trying the next source"
                        ),
                    }
                }
            }
        }
        let cache_stem = format!("{stem}-{}", level.id());
        if let Some(cached) = cached_file(&dir, &cache_stem) {
            return Ok(cached);
        }
        if let Some(legacy) = cached_file(&dir, &stem) {
            tracing::info!(song = %stem, "audio cached before levels is reused");
            return Ok(legacy);
        }

        if let Some(file) = provider.audio_file(song, level) {
            let AudioFile { bytes, extension, level: delivered } = file.await?;
            let path = dir.join(format!("{cache_stem}.{extension}"));
            let io_err = |source| OmniError::Io { path: path.clone(), source };
            std::fs::create_dir_all(&dir).map_err(io_err)?;
            let partial = path.with_extension("part");
            std::fs::write(&partial, &bytes).map_err(io_err)?;
            std::fs::rename(&partial, &path).map_err(io_err)?;
            tracing::info!(song = %stem, level = delivered.id(), bytes = bytes.len(), "audio cached from the platform");
            return Ok(path);
        }
        let source = provider.audio_source(song, level).await?;
        tracing::info!(provider = song.provider, "audio resolved through native platform");
        self.cache_audio_source(&dir, &cache_stem, source).await
    }

    async fn cache_audio_source(
        &self,
        dir: &Path,
        cache_stem: &str,
        source: AudioSource,
    ) -> Result<PathBuf, OmniError> {
        // A preview would stand in for the whole song once the account may play it, so it gets a name never looked up.
        let name = if source.preview { format!("{cache_stem}-preview") } else { cache_stem.to_owned() };
        let path = dir.join(format!("{name}.{}", source.extension));
        let io_err = |source| OmniError::Io { path: path.clone(), source };
        std::fs::create_dir_all(dir).map_err(io_err)?;
        let mut response = self
            .http
            .get(&source.url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|err| OmniError::Http(err.without_url()))?;
        let partial = path.with_extension("part");
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            bytes.extend_from_slice(&chunk);
        }
        std::fs::write(&partial, &bytes).map_err(io_err)?;
        std::fs::rename(&partial, &path).map_err(io_err)?;
        tracing::info!(
            song = cache_stem,
            level = source.level.id(),
            bytes = bytes.len(),
            preview = source.preview,
            "audio cached"
        );
        Ok(path)
    }

    /// Downloads a cover once into `covers/` and returns the cached file. The platform's CDN is asked for a `px`
    /// square (`getSizedCoverUrl`); `http://` is read as `https://`; at most `COVER_DOWNLOADS` run at once.
    pub async fn fetch_cover(&self, url: &str, px: u32) -> Result<PathBuf, OmniError> {
        let url = https(url);
        let url = self.providers.iter().find_map(|provider| provider.sized_cover(&url, px)).unwrap_or(url);
        let dir = self.cache_dir.join("covers");
        let path = dir.join(cover_cache_key(&url));
        if path.is_file() {
            return Ok(path);
        }
        let _permit =
            self.covers.acquire().await.map_err(|err| OmniError::Response(format!("cover queue closed: {err}")))?;
        if path.is_file() {
            return Ok(path);
        }
        let io_err = |source| OmniError::Io { path: path.clone(), source };
        std::fs::create_dir_all(&dir).map_err(io_err)?;
        let bytes = self.http.get(&url).send().await?.error_for_status()?.bytes().await?;
        let partial = path.with_extension("part");
        std::fs::write(&partial, &bytes).map_err(io_err)?;
        std::fs::rename(&partial, &path).map_err(io_err)?;
        tracing::debug!(bytes = bytes.len(), px, "cover cached");
        Ok(path)
    }

    fn provider(&self, id: &str) -> Result<&dyn Provider, OmniError> {
        self.providers
            .iter()
            .find(|provider| provider.id() == id)
            .map(|provider| provider.as_ref())
            .ok_or_else(|| OmniError::UnknownProvider(id.to_owned()))
    }
}

fn log_chorus(song: &OnlineId, mode: ChorusSource, ranges: usize) {
    let (provider, id) = (&song.provider, &song.id);
    match mode {
        ChorusSource::None => tracing::info!(provider, song = id, "no lyrics to mark chorus on"),
        ChorusSource::Existing => tracing::info!(provider, song = id, "using existing chorus markers"),
        ChorusSource::Native => tracing::info!(provider, song = id, ranges, "applied native chorus ranges"),
        ChorusSource::Text => {
            tracing::info!(provider, song = id, "native chorus unavailable or empty; applied text fallback")
        }
    }
}

/// Stable file name for a cover address, after `http://` is rewritten to `https://`.
pub fn cover_cache_key(url: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    https(url).hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn https(url: &str) -> String {
    url.strip_prefix("http://").map(|rest| format!("https://{rest}")).unwrap_or_else(|| url.to_owned())
}

/// The nearest level a provider lists at or below `preference`, or its lowest.
fn resolve_quality(levels: &[AudioQuality], preference: AudioQuality) -> AudioQuality {
    levels
        .iter()
        .rev()
        .copied()
        .find(|level| *level <= preference)
        .or_else(|| levels.first().copied())
        .unwrap_or(AudioQuality::Standard)
}

/// A finished download of `stem` in any format.
fn cached_file(dir: &Path, stem: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.filter_map(Result::ok).map(|entry| entry.path()).find(|path| {
        path.file_stem().is_some_and(|name| name == stem) && path.extension().is_some_and(|ext| ext != "part")
    })
}
