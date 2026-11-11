// Audio URL prefetch and progressive disk downloads shared by native and LX sources.

use crate::{AudioSource, Omni, OmniError, cached_file, resolve_quality};
use folia_core::{AudioInput, AudioQuality, Download, Queue, Repeat, Track};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const URL_TTL: Duration = Duration::from_secs(1200);
const MAX_ENTRIES: usize = 200;
const PLAYABLE_PREFIX: u64 = 64 * 1024;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

pub fn url_is_fresh(fetched: Instant, now: Instant) -> bool {
    now.saturating_duration_since(fetched) < URL_TTL
}

/// Forward first, two ahead and one behind; repeat-all also prepares the wraparound successor.
pub fn prefetch_indices(len: usize, current: Option<usize>, repeat: Repeat) -> Vec<usize> {
    let Some(current) = current.filter(|index| *index < len) else { return Vec::new() };
    let mut indices = Vec::with_capacity(4);
    if repeat == Repeat::All && current + 1 == len && len > 1 {
        indices.push(0);
    }
    indices.extend((current + 1..len).take(2));
    if current > 0 && !indices.contains(&(current - 1)) {
        indices.push(current - 1);
    }
    indices
}

/// A queue-window identity lets UI hosts restart work only when its resources actually change.
#[derive(Clone, Debug, PartialEq)]
pub struct PrefetchPlan {
    pub tracks: Vec<Track>,
    pub quality: AudioQuality,
}
impl PrefetchPlan {
    pub fn new(queue: &Queue<Track>, quality: AudioQuality) -> Self {
        let tracks = prefetch_indices(queue.items().len(), queue.current_index(), queue.repeat)
            .into_iter()
            .filter_map(|index| queue.items().get(index))
            .filter(|track| track.online.is_some())
            .cloned()
            .collect();
        Self { tracks, quality }
    }
}

#[derive(Default)]
struct Entry {
    source: Option<(AudioSource, Instant, u64)>,
    input: Option<AudioInput>,
    lyrics: Option<Option<folia_core::Lyrics>>,
}
type SharedEntry = Arc<tokio::sync::Mutex<Entry>>;

#[derive(Default)]
pub(crate) struct AudioCache {
    // Short-lived index lock, then one async lock per track/quality/source; unrelated songs never wait.
    entries: Mutex<VecDeque<(String, SharedEntry)>>,
    revision: std::sync::atomic::AtomicU64,
}
impl AudioCache {
    pub(crate) fn invalidate(&self) {
        self.revision.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }
    fn entry(&self, key: &str) -> SharedEntry {
        let mut entries = self.entries.lock();
        let entry = entries
            .iter()
            .position(|(name, _)| name == key)
            .and_then(|index| entries.remove(index))
            .map_or_else(|| Arc::new(tokio::sync::Mutex::new(Entry::default())), |(_, entry)| entry);
        entries.push_back((key.to_owned(), entry.clone()));
        // Do not evict a download/resolution another caller is using.
        if entries.len() > MAX_ENTRIES
            && let Some(index) = entries.iter().position(|(_, entry)| {
                Arc::strong_count(entry) == 1
                    && entry.try_lock().is_ok_and(|entry| {
                        entry
                            .input
                            .as_ref()
                            .and_then(|input| input.download.as_ref())
                            .is_none_or(|download| download.progress().1)
                    })
            })
        {
            entries.remove(index);
        }
        entry
    }
}

impl Omni {
    /// Memory-only check for the loading toast; disk cache checks remain on the background request.
    pub fn audio_is_prefetched(&self, track: &Track, quality: AudioQuality) -> bool {
        let Some(song) = &track.online else { return false };
        let Ok(provider) = self.provider(&song.provider) else { return false };
        let stem = format!("{}-{}", song.provider, song.id);
        let mut keys = Vec::new();
        if let Some(info) = provider.lx_music_info(track) {
            for (id, _) in self.sources.active(info.source) {
                keys.push(format!("{stem}-lx-{id}-{}", quality.id()));
            }
        }
        keys.push(format!("{stem}-{}", resolve_quality(provider.audio_quality_levels(), quality).id()));
        let entries = self.audio_cache.entries.lock();
        keys.iter().any(|key| {
            entries.iter().find(|(name, _)| name == key).is_some_and(|(_, entry)| {
                entry.try_lock().is_ok_and(|entry| {
                    entry.source.as_ref().is_some_and(|(_, at, revision)| {
                        *revision == self.audio_cache_revision() && url_is_fresh(*at, Instant::now())
                    })
                })
            })
        })
    }
    pub fn audio_cache_revision(&self) -> u64 {
        self.audio_cache.revision.load(std::sync::atomic::Ordering::Acquire)
    }

    pub async fn lyrics(&self, song: &folia_core::OnlineId) -> Result<Option<folia_core::Lyrics>, OmniError> {
        let key = format!("lyrics-{}-{}", song.provider, song.id);
        let entry = self.audio_cache.entry(&key);
        let mut entry = entry.lock().await;
        if let Some(lyrics) = &entry.lyrics {
            return Ok(lyrics.clone());
        }
        let lyrics = self.fetch_lyrics(song).await?;
        entry.lyrics = Some(lyrics.clone());
        Ok(lyrics)
    }
    /// Resolves the original lightweight prefetch window sequentially; dropping the future cancels the pass.
    pub async fn prefetch(&self, plan: PrefetchPlan) {
        for track in &plan.tracks {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if let Err(err) = self.track_input(track, plan.quality, Load::Prefetch, &[]).await {
                tracing::debug!(error = &err as &dyn std::error::Error, "audio prefetch unavailable");
            }
            if let Some(song) = &track.online
                && let Err(err) = self.lyrics(song).await
            {
                tracing::debug!(error = &err as &dyn std::error::Error, "lyrics prefetch unavailable");
            }
        }
    }

    /// Returns as soon as a prefix is available; the decoder waits for later bytes on its own thread.
    pub async fn prepare_track_audio(&self, track: &Track, preference: AudioQuality) -> Result<AudioInput, OmniError> {
        self.prepare_track_audio_after(track, preference, &[]).await
    }

    /// Continues source fallback after a stream failed beyond the initial playable prefix.
    pub async fn prepare_track_audio_after(
        &self,
        track: &Track,
        preference: AudioQuality,
        failed: &[PathBuf],
    ) -> Result<AudioInput, OmniError> {
        self.track_input(track, preference, Load::Stream, failed).await
    }

    /// Compatibility for callers that need the complete file, including platform downloads.
    pub async fn fetch_track_audio(&self, track: &Track, preference: AudioQuality) -> Result<PathBuf, OmniError> {
        Ok(self.track_input(track, preference, Load::Complete, &[]).await?.path)
    }

    async fn track_input(
        &self,
        track: &Track,
        preference: AudioQuality,
        load: Load,
        failed: &[PathBuf],
    ) -> Result<AudioInput, OmniError> {
        let song = track.online.as_ref().ok_or(OmniError::Unavailable)?;
        let provider = self.provider(&song.provider)?;
        let level = resolve_quality(provider.audio_quality_levels(), preference);
        let stem = format!("{}-{}", song.provider, song.id);
        if let Some(info) = provider.lx_music_info(track) {
            for (id, script) in self.sources.active(info.source) {
                let key = format!("{stem}-lx-{id}-{}", preference.id());
                if failed_key(failed, &key) {
                    continue;
                }
                let resolve = || async {
                    Self::custom_audio_source(&id, &script, &info, preference).await.ok_or(OmniError::Unavailable)
                };
                match self.load_source(&key, resolve, load).await {
                    Ok(input) => return Ok(input),
                    Err(err) => tracing::warn!(
                        source = id,
                        error = &err as &dyn std::error::Error,
                        "custom audio failed; trying next source"
                    ),
                }
            }
        }
        let key = format!("{stem}-{}", level.id());
        if failed_key(failed, &key) || failed_key(failed, &stem) {
            return Err(OmniError::Unavailable);
        }
        let dir = self.cache_dir.join("audio");
        if let Some(path) = cached_file(&dir, &key).or_else(|| cached_file(&dir, &stem)) {
            return Ok(path.into());
        }
        if let Some(file) = provider.audio_file(song, level) {
            if load == Load::Prefetch {
                return Ok(PathBuf::new().into());
            }
            let entry = self.audio_cache.entry(&key);
            let _guard = entry.lock().await;
            if let Some(path) = cached_file(&dir, &key) {
                return Ok(path.into());
            }
            let file = file.await?;
            validate_extension(&file.extension)?;
            let path = dir.join(format!("{key}.{}", file.extension));
            let io_err = |source| OmniError::Io { path: path.clone(), source };
            std::fs::create_dir_all(&dir).map_err(io_err)?;
            let partial = path.with_extension("part");
            std::fs::write(&partial, &file.bytes).map_err(io_err)?;
            std::fs::rename(partial, &path).map_err(io_err)?;
            return Ok(path.into());
        }
        self.load_source(&key, || provider.audio_source(song, level), load).await
    }

    async fn load_source<F: Future<Output = Result<AudioSource, OmniError>>>(
        &self,
        key: &str,
        resolve: impl FnOnce() -> F,
        load: Load,
    ) -> Result<AudioInput, OmniError> {
        let started = Instant::now();
        let dir = self.cache_dir.join("audio");
        let shared = self.audio_cache.entry(key);
        let mut entry = shared.lock().await;
        if let Some(path) = cached_file(&dir, key) {
            return Ok(path.into());
        }
        if entry.source.as_ref().is_some_and(|(source, at, revision)| {
            source.preview && (*revision != self.audio_cache_revision() || !url_is_fresh(*at, Instant::now()))
        }) {
            entry.input = None;
        }
        if let Some(input) = &entry.input {
            let result = ready(input, load).await;
            if result.is_ok() {
                return result;
            }
            entry.input = None;
            entry.source = None;
        }
        let revision = self.audio_cache_revision();
        if entry
            .source
            .as_ref()
            .is_none_or(|(_, at, generation)| *generation != revision || !url_is_fresh(*at, Instant::now()))
        {
            entry.source = Some((resolve().await?, Instant::now(), revision));
        }
        let Some((source, _, _)) = &entry.source else { return Err(OmniError::Unavailable) };
        tracing::debug!(
            song = key,
            resolve_ms = started.elapsed().as_secs_f64() * 1000.0,
            prefetch = load == Load::Prefetch,
            "playback address ready"
        );
        if load == Load::Prefetch {
            return Ok(PathBuf::new().into());
        }
        let input = match self.download(key, source, started).await {
            Ok(input) => input,
            Err(err) => {
                entry.source = None;
                return Err(err);
            }
        };
        entry.input = Some(input.clone());
        let result = ready(&input, load).await;
        if result.is_err() {
            entry.source = None;
            entry.input = None;
        }
        tracing::debug!(
            song = key,
            ready_ms = started.elapsed().as_secs_f64() * 1000.0,
            streaming = load == Load::Stream,
            "playback input ready"
        );
        result
    }

    async fn download(&self, key: &str, source: &AudioSource, started: Instant) -> Result<AudioInput, OmniError> {
        validate_extension(&source.extension)?;
        let name = if source.preview { format!("{key}-preview") } else { key.to_owned() };
        let dir = self.cache_dir.join("audio");
        let path = dir.join(format!("{name}.{}", source.extension));
        let io_err = |source| OmniError::Io { path: path.clone(), source };
        std::fs::create_dir_all(&dir).map_err(io_err)?;
        let deadline = tokio::time::Instant::now() + DOWNLOAD_TIMEOUT;
        let response = tokio::time::timeout_at(
            deadline,
            self.http.get(&source.url).header(reqwest::header::ACCEPT_ENCODING, "identity").send(),
        )
        .await
        .map_err(|_| OmniError::AudioTimeout("response headers"))?
        .and_then(reqwest::Response::error_for_status)
        .map_err(|err| OmniError::Http(err.without_url()))?;
        tracing::debug!(song = key, headers_ms = started.elapsed().as_secs_f64() * 1000.0, "playback download started");
        let partial = path.with_extension("part");
        let writer = File::create(&partial).map_err(io_err)?;
        let reader = File::open(&partial).map_err(io_err)?;
        let download = Download::new(reader, response.content_length());
        let ranges = crate::audio_ranges::start(self.http.clone(), &response, partial.clone(), download.clone());
        let input = AudioInput { path: path.clone(), download: Some(download.clone()) };
        let key = key.to_owned();
        tokio::spawn(async move {
            let result = write_download(response, writer, &download, &key, &path, started, deadline).await;
            if let Some(ranges) = ranges {
                ranges.abort();
                if let Err(err) = ranges.await
                    && !err.is_cancelled()
                {
                    tracing::warn!(error = &err as &dyn std::error::Error, "audio range worker stopped");
                }
            }
            let result = result.and_then(|()| {
                std::fs::rename(&partial, &path).map_err(|source| OmniError::Io { path: path.clone(), source })
            });
            if let Err(error) = &result {
                tracing::error!(song = key, error = error as &dyn std::error::Error, "audio download failed");
                if let Err(err) = std::fs::remove_file(&partial) {
                    tracing::warn!(error = &err as &dyn std::error::Error, "cannot remove incomplete audio");
                }
            }
            download.finish(result.map_err(|error| folia_store::error_chain(&error)));
        });
        Ok(input)
    }
}

fn failed_key(failed: &[PathBuf], key: &str) -> bool {
    failed.iter().any(|path| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem == key || stem.strip_suffix("-preview") == Some(key))
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Load {
    Prefetch,
    Stream,
    Complete,
}

async fn ready(input: &AudioInput, load: Load) -> Result<AudioInput, OmniError> {
    if let Some(download) = &input.download {
        loop {
            let (bytes, done, error) = download.progress();
            if let Some(error) = error {
                return Err(OmniError::Response(error));
            }
            if done || load == Load::Prefetch || (load == Load::Stream && bytes >= PLAYABLE_PREFIX) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    Ok(input.clone())
}

async fn write_download(
    mut response: reqwest::Response,
    mut file: File,
    download: &Download,
    key: &str,
    path: &std::path::Path,
    started: Instant,
    mut deadline: tokio::time::Instant,
) -> Result<(), OmniError> {
    let expected = response.content_length();
    let io_err = |source| OmniError::Io { path: path.to_owned(), source };
    let mut bytes = 0_u64;
    while let Some(chunk) = tokio::time::timeout_at(deadline, response.chunk())
        .await
        .map_err(|_| {
            OmniError::AudioTimeout(if bytes < PLAYABLE_PREFIX { "initial audio data" } else { "more audio data" })
        })?
        .map_err(|err| OmniError::Http(err.without_url()))?
    {
        if bytes == 0 {
            tracing::debug!(
                song = key,
                first_ms = started.elapsed().as_secs_f64() * 1000.0,
                "playback first audio bytes"
            );
        }
        file.write_all(&chunk).map_err(io_err)?;
        bytes += chunk.len() as u64;
        download.publish(bytes);
        if bytes >= PLAYABLE_PREFIX && !chunk.is_empty() {
            deadline = tokio::time::Instant::now() + DOWNLOAD_TIMEOUT;
        }
    }
    if bytes == 0 || expected.is_some_and(|length| length != bytes) {
        return Err(OmniError::Response("incomplete audio download".into()));
    }
    file.flush().map_err(io_err)?;
    tracing::debug!(
        song = key,
        bytes,
        complete_ms = started.elapsed().as_secs_f64() * 1000.0,
        "playback download complete"
    );
    Ok(())
}

fn validate_extension(extension: &str) -> Result<(), OmniError> {
    if extension.is_empty() || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(OmniError::Response("invalid audio extension".into()));
    }
    Ok(())
}
