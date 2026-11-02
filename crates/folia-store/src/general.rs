// App-wide preferences: the interface language, the home tab last opened and the online platform in use.

use folia_core::LanguagePreference;
use serde::{Deserialize, Serialize};

use crate::Domain;

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
    const SCHEMA_VERSION: u32 = 1;
}
