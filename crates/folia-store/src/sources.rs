// Imported LX sources and independent activation, with migration from the single-source domain.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Domain, Store, StoreError};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub homepage: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePlatform {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub actions: Vec<String>,
    pub qualitys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomSource {
    pub id: String,
    pub metadata: SourceMetadata,
    pub sources: BTreeMap<String, SourcePlatform>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceSettings {
    pub enabled: Vec<String>,
    pub scripts: Vec<CustomSource>,
    pub custom_only: bool,
}

impl Domain for SourceSettings {
    const FILE: &'static str = "sources.toml";
    const SCHEMA_VERSION: u32 = 3;
}

impl SourceSettings {
    /// Preserves source order and activation while migrating older playback routing settings.
    pub fn load(store: &Store) -> Result<Self, StoreError> {
        match store.load::<Self>() {
            Err(StoreError::SchemaVersion { found: 1, .. }) => {
                let previous: SingleSourceSettings = store.load()?;
                let settings = Self {
                    enabled: previous.enabled.into_iter().collect(),
                    scripts: previous.scripts,
                    custom_only: false,
                };
                store.save(&settings)?;
                tracing::info!(from = 1, "custom source settings migrated to playback routing");
                Ok(settings)
            }
            Err(StoreError::SchemaVersion { found: 2, .. }) => {
                let previous: MultipleSourceSettings = store.load()?;
                let settings = Self { enabled: previous.enabled, scripts: previous.scripts, custom_only: false };
                store.save(&settings)?;
                tracing::info!(from = 2, "custom source settings migrated to playback routing");
                Ok(settings)
            }
            result => result,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct SingleSourceSettings {
    enabled: Option<String>,
    scripts: Vec<CustomSource>,
}

impl Domain for SingleSourceSettings {
    const FILE: &'static str = SourceSettings::FILE;
    const SCHEMA_VERSION: u32 = 1;
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct MultipleSourceSettings {
    enabled: Vec<String>,
    scripts: Vec<CustomSource>,
}

impl Domain for MultipleSourceSettings {
    const FILE: &'static str = SourceSettings::FILE;
    const SCHEMA_VERSION: u32 = 2;
}
