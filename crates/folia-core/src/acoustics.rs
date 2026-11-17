// Room geometry, listener pose and true-stereo impulse responses shared by scenes and audio output.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomOutput {
    #[default]
    Headphones,
    Speakers,
}

#[derive(Debug, Clone)]
pub struct AcousticMaterial {
    pub absorption: [f32; 3],
    pub scattering: f32,
    pub transmission: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct AcousticMesh {
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub triangle_materials: Vec<u16>,
}

#[derive(Debug, Clone)]
pub struct AcousticWorld {
    pub meshes: Vec<AcousticMesh>,
    pub materials: Vec<AcousticMaterial>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomListener {
    pub position: [f32; 3],
    pub forward: [f32; 3],
    pub up: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct RoomRequest {
    pub world: Arc<AcousticWorld>,
    pub speakers: [[f32; 3]; 2],
    pub listener: RoomListener,
    pub output: RoomOutput,
    pub sample_rate: u32,
}

#[derive(Debug, Clone)]
pub struct RoomImpulseResponse {
    pub sample_rate: u32,
    /// Left speaker to left/right ear, then right speaker to left/right ear.
    pub channels: [Vec<f32>; 4],
    pub gain: f32,
}
