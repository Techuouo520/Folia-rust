// Desktop companion preferences, lyric sources and portable appearance presets.

use serde::{Deserialize, Serialize};

use crate::Domain;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteBackground {
    #[default]
    Default,
    Cover,
    Transparent,
}

impl RemoteBackground {
    pub fn next(self) -> Self {
        match self {
            Self::Default => Self::Cover,
            Self::Cover => Self::Transparent,
            Self::Transparent => Self::Default,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteSource {
    Original,
    Translation,
    Romanization,
    Auto,
    None,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteChrome {
    Visible,
    #[default]
    Auto,
    Hidden,
}

impl RemoteSource {
    pub const MAIN: [Self; 3] = [Self::Original, Self::Translation, Self::Romanization];
    pub const SUB: [Self; 5] = [Self::Auto, Self::Translation, Self::Romanization, Self::Original, Self::None];

    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "options.remoteLyricSourceOriginal",
            Self::Translation => "options.subtitleContentTranslation",
            Self::Romanization => "options.subtitleContentRomanization",
            Self::Auto => "options.remoteLyricSourceAuto",
            Self::None => "options.remoteLyricSourceNone",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteAppearance {
    pub background: RemoteBackground,
    pub hide_titlebar: bool,
    pub expand_cover: bool,
    pub text_shade: bool,
    pub lyric_pairs: u8,
    pub lyric_rows: [RemoteSource; 4],
}

impl Default for RemoteAppearance {
    fn default() -> Self {
        Self {
            background: RemoteBackground::Default,
            hide_titlebar: false,
            expand_cover: true,
            text_shade: false,
            lyric_pairs: 1,
            lyric_rows: [RemoteSource::Original, RemoteSource::Auto, RemoteSource::Original, RemoteSource::Auto],
        }
    }
}

impl RemoteAppearance {
    pub fn normalize(&mut self) {
        self.lyric_pairs = self.lyric_pairs.clamp(1, 2);
        for index in [0, 2] {
            if !RemoteSource::MAIN.contains(&self.lyric_rows[index]) {
                self.lyric_rows[index] = RemoteSource::Original;
            }
        }
    }

    pub fn export(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(&RemotePreset { folia_remote_appearance: self.clone() })
    }

    pub fn import(text: &str) -> Result<Self, toml::de::Error> {
        let mut appearance = toml::from_str::<RemotePreset>(text)?.folia_remote_appearance;
        appearance.normalize();
        Ok(appearance)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemotePreset {
    folia_remote_appearance: RemoteAppearance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteSettings {
    pub appearance: RemoteAppearance,
    pub hide_taskbar_icon: bool,
    pub click_through: bool,
    pub always_on_top: bool,
    /// Physical desktop coordinates; logical size remains stable when DPI changes.
    pub position: Option<[i32; 2]>,
    pub size: [f32; 2],
    pub size_presets: [[u32; 2]; 3],
    pub selected_preset: usize,
    pub main_border: bool,
    pub main_pin: bool,
    pub main_chrome: RemoteChrome,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            appearance: RemoteAppearance::default(),
            hide_taskbar_icon: false,
            click_through: false,
            always_on_top: true,
            position: None,
            size: [602.0, 184.0],
            size_presets: [[1280, 720], [1920, 1080], [1080, 1920]],
            selected_preset: 1,
            main_border: true,
            main_pin: false,
            main_chrome: RemoteChrome::Auto,
        }
    }
}

impl RemoteSettings {
    pub fn normalize(&mut self) {
        self.appearance.normalize();
        for (dimension, minimum) in self.size.iter_mut().zip([602.0, 184.0]) {
            *dimension = if dimension.is_finite() { dimension.clamp(minimum, minimum * 2.0) } else { minimum };
        }
        for preset in &mut self.size_presets {
            *preset = preset.map(|side| side.clamp(240, 4320).div_ceil(2) * 2);
        }
        self.selected_preset = self.selected_preset.min(2);
    }
}

impl Domain for RemoteSettings {
    const FILE: &'static str = "remote.toml";
    const SCHEMA_VERSION: u32 = 1;
}
