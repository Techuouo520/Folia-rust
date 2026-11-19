// App-wide preferences: the interface language, the home tab last opened and the online platform in use.

use folia_core::LanguagePreference;
use serde::{Deserialize, Serialize};

use crate::{Domain, Store, StoreError};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    pub language: LanguagePreference,
    /// The home tab to open on, like the original's `LAST_HOME_VIEW_TAB_KEY`.
    pub home_tab: HomeTab,
    /// The online platform last used (`activeProviderId`); never a credential.
    pub online_provider: Option<String>,
    /// `wall_tools_queue_island_open` (the original's localStorage key): the wall panel's queue island
    /// stays open across panel openings. Interface state only; not an appearance setting.
    pub wall_tools_queue_island_open: bool,
    /// Randomizes cover positions once per visit, independently of the playback queue.
    pub wall_randomize_on_entry: bool,
}

/// The home page's first-level tabs; the original starts on the playlists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HomeTab {
    #[default]
    Playlist,
    Radio,
    Explore,
    Albums,
    Local,
}

impl HomeTab {
    pub fn is_online(self) -> bool {
        self != HomeTab::Local
    }
}

impl Domain for GeneralSettings {
    const FILE: &'static str = "general.toml";
    const SCHEMA_VERSION: u32 = 2;
}

impl GeneralSettings {
    /// Preserves existing preferences while adding the optional wall ordering setting.
    pub fn load(store: &Store) -> Result<Self, StoreError> {
        match store.load::<Self>() {
            Err(StoreError::SchemaVersion { found: 1, .. }) => {
                let previous: PreviousGeneral = store.load()?;
                store.save(&previous.settings)?;
                tracing::info!("general settings migrated to schema 2");
                Ok(previous.settings)
            }
            result => result,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct PreviousGeneral {
    #[serde(flatten)]
    settings: GeneralSettings,
}

impl Domain for PreviousGeneral {
    const FILE: &'static str = GeneralSettings::FILE;
    const SCHEMA_VERSION: u32 = 1;
}
