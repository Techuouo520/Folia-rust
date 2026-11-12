// Helpers for work that must not run on the UI thread, and for persisting settings.

use std::error::Error;
use std::future::Future;

use folia_library::Cover;
use folia_omni::Omni;
use folia_store::{Domain, Store};
use iced::Task;
use iced::futures::channel::oneshot;

/// Runs blocking work on its own thread; the future yields `None` only if that thread died.
pub fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> impl Future<Output = Option<T>> {
    let (sender, receiver) = oneshot::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    async move { receiver.await.ok() }
}

/// A decoded cover plus the palette taken from its 50×50 sample. `image` is absent when there is nothing to show.
#[derive(Clone, Debug)]
pub struct PanelArt {
    pub image: Option<Cover>,
    pub colors: Vec<folia_core::Rgb>,
    pub background: Option<std::sync::Arc<image::RgbaImage>>,
}

impl PanelArt {
    pub fn missing() -> Self {
        Self { image: None, colors: Vec::new(), background: None }
    }
}

fn palette_of(cover: &Cover) -> Vec<folia_core::Rgb> {
    folia_visualizer::cover_palette(&cover.rgba, cover.width, cover.height, 6)
}

/// Downloads and decodes a panel cover, then samples its palette off the UI thread.
pub async fn panel_art_online(omni: Omni, url: String, px: u32, full: bool) -> PanelArt {
    let path = match omni.fetch_cover(&url, if full { 1920 } else { px }).await {
        Ok(path) => path,
        Err(err) => {
            tracing::warn!(error = &err as &dyn Error, "background cover not downloaded");
            return PanelArt::missing();
        }
    };
    blocking(move || match std::fs::read(path) {
        Ok(bytes) => panel_art_bytes(&bytes, px, full),
        Err(err) => {
            tracing::warn!(error = &err as &dyn Error, "background cover unreadable");
            PanelArt::missing()
        }
    })
    .await
    .unwrap_or_else(PanelArt::missing)
}

/// Reads a local cover and samples its palette. Both steps stay off the UI thread.
pub fn panel_art_file(path: std::path::PathBuf, px: u32, full: bool) -> PanelArt {
    folia_library::cover_bytes(&path).map_or_else(PanelArt::missing, |bytes| panel_art_bytes(&bytes, px, full))
}

fn panel_art_bytes(bytes: &[u8], px: u32, keep_full: bool) -> PanelArt {
    let full = match folia_library::cover_uncropped(bytes, if keep_full { 1920 } else { 50 }) {
        Ok(cover) => cover,
        Err(err) => {
            tracing::warn!(error = &err as &dyn Error, "background cover decode failed");
            return PanelArt::missing();
        }
    };
    let colors = palette_of(&full);
    let background = if keep_full {
        image::RgbaImage::from_raw(full.width, full.height, full.rgba).map(std::sync::Arc::new)
    } else {
        None
    };
    let image = folia_library::cover_rgba(bytes, px, px)
        .inspect_err(|err| {
            tracing::warn!(error = err as &dyn Error, "panel cover decode failed");
        })
        .ok();
    PanelArt { image, colors, background }
}

/// An online cover: downloaded through the facade, then decoded `px` square on a worker thread; failures are logged.
pub async fn online_cover(omni: Omni, url: String, px: u32) -> Option<Cover> {
    online_cover_sized(omni, url, px, px).await
}

/// Downloads the cover at `fetch_px` and decodes it to `decode_px` square.
pub async fn online_cover_sized(omni: Omni, url: String, fetch_px: u32, decode_px: u32) -> Option<Cover> {
    let px = decode_px;
    let path = match omni.fetch_cover(&url, fetch_px).await {
        Ok(path) => path,
        Err(err) => {
            tracing::warn!(error = &err as &dyn std::error::Error, "online cover not downloaded");
            return None;
        }
    };
    let decoded = blocking(move || {
        if px <= 64 {
            return folia_library::cached_cover_thumbnail(&path, px).map_err(|err| err.to_string());
        }
        let bytes = std::fs::read(&path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        folia_library::cover_rgba(&bytes, px, px).map_err(|err| format!("cannot decode {}: {err}", path.display()))
    })
    .await;
    match decoded {
        Some(Ok(cover)) => Some(cover),
        Some(Err(err)) => {
            tracing::warn!(error = %err, "online cover unusable");
            None
        }
        None => {
            tracing::warn!("online cover worker stopped");
            None
        }
    }
}

/// Saves a settings domain in the background, logging the outcome.
pub fn save<T, M>(store: &Store, value: &T) -> Task<M>
where
    T: Domain + Clone + Send + 'static,
    M: Send + 'static,
{
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, OnceLock, Weak};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    static WRITERS: OnceLock<Mutex<std::collections::HashMap<std::path::PathBuf, Weak<Mutex<u64>>>>> = OnceLock::new();
    let order = NEXT.fetch_add(1, Ordering::Relaxed);
    let writer = {
        let mut writers = WRITERS.get_or_init(Mutex::default).lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        writers.retain(|_, writer| writer.strong_count() > 0);
        let path = store.dir().join(T::FILE);
        match writers.get(&path).and_then(Weak::upgrade) {
            Some(writer) => writer,
            None => {
                let writer = Arc::new(Mutex::new(0));
                writers.insert(path, Arc::downgrade(&writer));
                writer
            }
        }
    };
    let (store, value) = (store.clone(), value.clone());
    Task::future(blocking(move || {
        // UI messages establish order before worker scheduling; a late old worker cannot overwrite a newer commit.
        let mut written = writer.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if order <= *written {
            return;
        }
        *written = order;
        match store.save(&value) {
            Ok(()) => tracing::debug!(file = T::FILE, "settings saved"),
            Err(err) => tracing::error!(file = T::FILE, error = &err as &dyn Error, "cannot save settings"),
        }
    }))
    .discard()
}

pub use folia_store::error_chain;

/// `mm:ss` like the original player.
pub fn format_time(seconds: f64) -> String {
    let total = if seconds.is_finite() { seconds.max(0.0) as u64 } else { 0 };
    format!("{:02}:{:02}", total / 60, total % 60)
}
