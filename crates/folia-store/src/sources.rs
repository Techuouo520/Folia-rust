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
}

impl Domain for SourceSettings {
    const FILE: &'static str = "sources.toml";
    const SCHEMA_VERSION: u32 = 2;
}

impl SourceSettings {
    /// Migrates version 1 without changing the imported scripts or losing the selected source.
    pub fn load(store: &Store) -> Result<Self, StoreError> {
        match store.load::<Self>() {
            Err(StoreError::SchemaVersion { found: 1, .. }) => {
                let previous: SingleSourceSettings = store.load()?;
                let settings = Self { enabled: previous.enabled.into_iter().collect(), scripts: previous.scripts };
                store.save(&settings)?;
                tracing::info!("custom source settings migrated to multiple activation");
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
