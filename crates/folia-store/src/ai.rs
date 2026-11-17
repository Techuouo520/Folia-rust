// AI connection preferences and song-theme automation; secrets live in the operating-system keyring.

use serde::{Deserialize, Serialize};

use crate::Domain;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProtocol {
    #[default]
    Gemini,
    Openai,
    Anthropic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    pub protocol: AiProtocol,
    pub endpoint: String,
    pub model: String,
    pub temperature: f32,
    pub use_system_proxy: bool,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            protocol: AiProtocol::Gemini,
            endpoint: "https://generativelanguage.googleapis.com/v1beta".into(),
            model: "gemini-3-flash-preview".into(),
            temperature: 0.7,
            use_system_proxy: false,
        }
    }
}

impl Domain for AiSettings {
    const FILE: &'static str = "ai.toml";
    const SCHEMA_VERSION: u32 = 1;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeGenerationSource {
    #[default]
    Ai,
    Cover,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SongThemeSettings {
    pub auto_switch: bool,
    pub auto_generate: bool,
    pub generation_source: ThemeGenerationSource,
    pub prefer_custom: bool,
}

impl Default for SongThemeSettings {
    fn default() -> Self {
        Self {
            auto_switch: true,
            auto_generate: false,
            generation_source: ThemeGenerationSource::Ai,
            prefer_custom: false,
        }
    }
}

impl SongThemeSettings {
    pub fn set_auto_switch(&mut self, enabled: bool) {
        self.auto_switch = enabled;
        if enabled {
            self.prefer_custom = false;
        } else {
            self.auto_generate = false;
        }
    }

    pub fn set_auto_generate(&mut self, enabled: bool) {
        self.auto_generate = enabled;
        if enabled {
            self.auto_switch = true;
            self.prefer_custom = false;
        }
    }

    pub fn set_prefer_custom(&mut self, enabled: bool) {
        self.prefer_custom = enabled;
        if enabled {
            self.auto_switch = false;
            self.auto_generate = false;
        }
    }
}

impl Domain for SongThemeSettings {
    const FILE: &'static str = "song_theme.toml";
    const SCHEMA_VERSION: u32 = 1;
}
