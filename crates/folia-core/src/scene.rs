// Persisted scene choices shared by the native player, scene renderer and sound processing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneId {
    Cinema,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneQuality {
    Low,
    #[default]
    Medium,
    High,
}

impl SceneQuality {
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    pub fn density(self) -> f32 {
        match self {
            Self::Low => 1.0,
            Self::Medium => 1.25,
            Self::High => 1.5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CinemaOptions {
    pub quality: SceneQuality,
    pub viewpoint: Option<String>,
    #[serde(rename = "houseLights")]
    pub house_lights: bool,
    #[serde(rename = "curtainsOnPause")]
    pub curtains_on_pause: bool,
}

impl Default for CinemaOptions {
    fn default() -> Self {
        Self { quality: SceneQuality::Medium, viewpoint: None, house_lights: false, curtains_on_pause: true }
    }
}
