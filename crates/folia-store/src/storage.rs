// Cache location preferences and recovery of interrupted live migrations.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Domain, Store, StoreError};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageSettings {
    pub cache_directory: Option<PathBuf>,
    pub migration: Option<crate::CacheMigration>,
}

impl Domain for StorageSettings {
    const FILE: &'static str = "storage.toml";
    const SCHEMA_VERSION: u32 = 2;
}

impl StorageSettings {
    /// Completes a persisted migration before the application's cache consumers are constructed.
    pub fn load_and_resume(store: &Store) -> Result<Self, StoreError> {
        let settings = Self::load(store)?;
        if settings.migration.is_none() {
            return Ok(settings);
        }
        match Self::finish_migration(store, |_, _| {}) {
            Ok(result) => Ok(result.settings),
            Err(error) => {
                tracing::warn!(error = &error as &dyn std::error::Error, "cache migration recovery deferred");
                Self::load(store)
            }
        }
    }

    pub fn load(store: &Store) -> Result<Self, StoreError> {
        let settings: Self = match store.load() {
            Err(StoreError::SchemaVersion { found: 1, .. }) => {
                let previous: PreviousStorage = store.load()?;
                store.save(&previous.settings)?;
                previous.settings
            }
            result => result?,
        };
        if let Some(path) = &settings.cache_directory {
            absolute(path)?;
        }
        Ok(settings)
    }

    pub fn cache_path<'a>(&'a self, default: &'a Path) -> &'a Path {
        self.cache_directory.as_deref().unwrap_or(default)
    }
}

#[derive(Default, Serialize, Deserialize)]
struct PreviousStorage {
    #[serde(flatten)]
    settings: StorageSettings,
}

impl Domain for PreviousStorage {
    const FILE: &'static str = StorageSettings::FILE;
    const SCHEMA_VERSION: u32 = 1;
}

fn absolute(path: &Path) -> Result<(), StoreError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(StoreError::Io {
            path: path.into(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "cache directory must be absolute"),
        })
    }
}
