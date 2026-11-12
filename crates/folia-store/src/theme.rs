// Theme choices: where the interface's colours come from, the custom pair, animation intensity and the random pool.

use folia_core::{AnimationIntensity, Rgb, Theme};
use serde::{Deserialize, Serialize};

use crate::Domain;

/// Which theme the interface takes: the built-in preset, or the listener's custom pair.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeSource {
    #[default]
    Default,
    Custom,
}

/// One side of a custom theme (`customTheme`): its four colours and its name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredTheme {
    pub name: String,
    pub background: Rgb,
    pub primary: Rgb,
    pub accent: Rgb,
    pub secondary: Rgb,
    #[serde(alias = "wordColors")]
    pub word_colors: Vec<folia_core::WordColor>,
    #[serde(alias = "lyricsIcons")]
    pub lyrics_icons: Vec<String>,
    pub description: Option<String>,
}

impl Default for StoredTheme {
    fn default() -> Self {
        let theme = Theme::MIDNIGHT;
        Self {
            name: theme.name.into_owned(),
            background: theme.background,
            primary: theme.primary,
            accent: theme.accent,
            secondary: theme.secondary,
            word_colors: Vec::new(),
            lyrics_icons: Vec::new(),
            description: None,
        }
    }
}

impl StoredTheme {
    /// The theme with this side's colours and the given animation intensity.
    pub fn theme(&self, intensity: AnimationIntensity) -> Theme {
        Theme {
            name: self.name.clone().into(),
            background: self.background,
            primary: self.primary,
            accent: self.accent,
            secondary: self.secondary,
            intensity,
            word_colors: self.word_colors.clone(),
            lyrics_icons: self.lyrics_icons.clone(),
            description: self.description.clone(),
        }
    }
}

/// A custom theme's light and dark sides (`custom_dual_theme`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DualTheme {
    pub light: StoredTheme,
    pub dark: StoredTheme,
}

impl Default for DualTheme {
    fn default() -> Self {
        Self {
            light: StoredTheme {
                name: Theme::DAYLIGHT.name.into_owned(),
                background: Theme::DAYLIGHT.background,
                primary: Theme::DAYLIGHT.primary,
                accent: Theme::DAYLIGHT.accent,
                secondary: Theme::DAYLIGHT.secondary,
                word_colors: Vec::new(),
                lyrics_icons: Vec::new(),
                description: None,
            },
            dark: StoredTheme::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeSettings {
    pub source: ThemeSource,
    /// Style ids the random choice draws from; `None` draws from every style (`randomPool`).
    pub random_pool: Option<Vec<String>>,
    /// `transparentPlayerBackground`.
    pub transparent_player_background: bool,
    /// `custom_dual_theme`.
    pub custom_dual_theme: Option<DualTheme>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            source: ThemeSource::Default,
            random_pool: None,
            transparent_player_background: false,
            custom_dual_theme: None,
        }
    }
}

impl Domain for ThemeSettings {
    const FILE: &'static str = "theme.toml";
    const SCHEMA_VERSION: u32 = 1;
}
