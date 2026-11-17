// Native scene preferences and room sound, stored independently of appearance presets.

use folia_core::{CinemaOptions, RoomOutput, SceneId};
use serde::{Deserialize, Serialize};

use crate::Domain;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneOptions {
    pub cinema: CinemaOptions,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneSettings {
    pub scene_id: Option<SceneId>,
    pub scene_options: SceneOptions,
    pub scene_room_sound: bool,
    pub scene_room_output: RoomOutput,
}

impl Domain for SceneSettings {
    const FILE: &'static str = "scene.toml";
    const SCHEMA_VERSION: u32 = 1;
}
