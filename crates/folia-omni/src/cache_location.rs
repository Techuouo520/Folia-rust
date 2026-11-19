// Shared cache root fencing: retry readers after relocation and stop detached download writers.

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock, watch};

use crate::{Omni, OmniError};

pub(crate) struct CacheLocation {
    path: Mutex<PathBuf>,
    gate: Arc<RwLock<()>>,
    moving: watch::Sender<bool>,
    lease: Mutex<Option<folia_store::CacheLease>>,
}

pub(crate) struct CacheRead {
    pub path: PathBuf,
    pub changed: watch::Receiver<bool>,
    _guard: OwnedRwLockReadGuard<()>,
    _cache: Arc<CacheLocation>,
}

impl CacheLocation {
    pub fn new(path: PathBuf) -> Self {
        Self { path: Mutex::new(path), gate: Arc::default(), moving: watch::channel(false).0, lease: Mutex::default() }
    }

    pub fn path(&self) -> PathBuf {
        self.path.lock().clone()
    }

    pub async fn read(self: &Arc<Self>) -> Result<CacheRead, OmniError> {
        let mut changed = self.moving.subscribe();
        loop {
            changed.wait_for(|moving| !*moving).await.map_err(|_| OmniError::CacheMoving)?;
            let guard = self.gate.clone().read_owned().await;
            if *changed.borrow_and_update() {
                continue;
            }
            return self.reader(guard, changed);
        }
    }

    pub fn writer(self: &Arc<Self>) -> Result<CacheRead, OmniError> {
        let changed = self.moving.subscribe();
        if *changed.borrow() {
            return Err(OmniError::CacheMoving);
        }
        let guard = self.gate.clone().try_read_owned().map_err(|_| OmniError::CacheMoving)?;
        self.reader(guard, changed)
    }

    fn reader(
        self: &Arc<Self>,
        guard: OwnedRwLockReadGuard<()>,
        changed: watch::Receiver<bool>,
    ) -> Result<CacheRead, OmniError> {
        let path = self.path();
        let mut lease = self.lease.lock();
        if lease.is_none() {
            *lease = Some(folia_store::CacheLease::shared(&path).map_err(OmniError::Cache)?);
        }
        Ok(CacheRead { path, changed, _guard: guard, _cache: self.clone() })
    }
}

/// Keeps the cached cover in place until its caller finishes decoding and thumbnail work.
pub struct CachedCover {
    pub(crate) path: PathBuf,
    pub(crate) _access: CacheRead,
}

impl Deref for CachedCover {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}
impl AsRef<Path> for CachedCover {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}
impl std::fmt::Debug for CachedCover {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.fmt(formatter)
    }
}
impl PartialEq<PathBuf> for CachedCover {
    fn eq(&self, other: &PathBuf) -> bool {
        self.path == *other
    }
}
impl PartialEq for CachedCover {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

/// Exclusive cache access retained across the filesystem move. Dropping it resumes blocked readers.
pub struct CacheSwitch {
    cache: Arc<CacheLocation>,
    _guard: Option<OwnedRwLockWriteGuard<()>>,
}

impl CacheSwitch {
    pub fn resume(self, path: PathBuf) {
        *self.cache.path.lock() = path;
    }
}

impl Drop for CacheSwitch {
    fn drop(&mut self) {
        self.cache.moving.send_replace(false);
    }
}

impl Omni {
    pub fn cache_path(&self) -> PathBuf {
        self.cache.path()
    }

    pub fn pause_cache(&self) {
        self.cache.moving.send_replace(true);
        self.audio_cache.invalidate();
    }

    pub async fn suspend_cache(&self) -> CacheSwitch {
        let mut switch = CacheSwitch { cache: self.cache.clone(), _guard: None };
        self.pause_cache();
        switch._guard = Some(self.cache.gate.clone().write_owned().await);
        self.audio_cache.clear();
        self.cache.lease.lock().take();
        switch
    }
}
