// Native rendering and accessibility preferences, kept outside shared appearance presets.
use serde::{Deserialize, Serialize};

use crate::Domain;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuPreference {
    Auto,
    #[default]
    HighPerformance,
    LowPower,
}

impl GpuPreference {
    pub const ALL: [Self; 3] = [Self::Auto, Self::HighPerformance, Self::LowPower];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionSurface {
    CollectionMorph,
    MonetBackground,
    UiMicroMotion,
    SettingsScroll,
}

impl MotionSurface {
    pub const ALL: [Self; 4] =
        [Self::CollectionMorph, Self::MonetBackground, Self::UiMicroMotion, Self::SettingsScroll];
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MotionSettings {
    pub follow_system: bool,
    pub collection_morph: bool,
    pub monet_background: bool,
    pub ui_micro_motion: bool,
    pub settings_scroll: bool,
}

impl MotionSettings {
    pub fn selected(&self, surface: MotionSurface) -> bool {
        match surface {
            MotionSurface::CollectionMorph => self.collection_morph,
            MotionSurface::MonetBackground => self.monet_background,
            MotionSurface::UiMicroMotion => self.ui_micro_motion,
            MotionSurface::SettingsScroll => self.settings_scroll,
        }
    }

    pub fn reduced(&self, surface: MotionSurface, system: bool) -> bool {
        self.selected(surface) || (self.follow_system && system)
    }

    pub fn set(&mut self, surface: MotionSurface, value: bool) {
        match surface {
            MotionSurface::CollectionMorph => self.collection_morph = value,
            MotionSurface::MonetBackground => self.monet_background = value,
            MotionSurface::UiMicroMotion => self.ui_micro_motion = value,
            MotionSurface::SettingsScroll => self.settings_scroll = value,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphicsSettings {
    pub static_mode: bool,
    pub enable_player_page_native_blur: bool,
    pub background_throttling: bool,
    pub gpu_preference: GpuPreference,
    pub motion: MotionSettings,
    pub lyric_render_quality: folia_core::LyricRenderQuality,
}

impl Domain for GraphicsSettings {
    const FILE: &'static str = "graphics.toml";
    const SCHEMA_VERSION: u32 = 1;
}
