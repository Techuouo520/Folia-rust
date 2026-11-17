// Immutable model-derived music timelines and the selected effects consumed by a visualizer frame.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnalysisSources {
    pub beats: bool,
    pub stems: bool,
}

impl AnalysisSources {
    pub fn any(self) -> bool {
        self.beats || self.stems
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BeatEvent {
    pub time: f64,
    pub downbeat: bool,
    pub impact: Option<f32>,
    pub rhythmic: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DrumHits {
    pub times: Vec<f64>,
    pub strengths: Vec<f32>,
    /// Kick, snare and hat are 0, 1 and 2 respectively.
    pub kinds: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VocalEnvelope {
    pub fps: u32,
    pub db: Vec<f32>,
    pub top: f32,
    pub onsets: Vec<u32>,
    /// First analysed frame in whole-track coordinates. Onsets use the same coordinate system.
    #[serde(default)]
    pub from: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StemAnalysis {
    /// None when the analysed stem has too little drum coverage to drive rhythmic effects.
    pub drums: Option<DrumHits>,
    pub vocal: VocalEnvelope,
}

#[derive(Clone, Copy, Debug)]
pub struct SmartBeat<'a> {
    pub beats: Option<&'a [BeatEvent]>,
    pub drums: Option<&'a DrumHits>,
    pub vocal: Option<&'a VocalEnvelope>,
    pub heard: f64,
    pub rain_strength: f32,
    pub bump_strength: f32,
    /// Jelly, weed, bubble and spotlight, in the original effect's order.
    pub aqua_strength: [f32; 4],
}

impl Default for SmartBeat<'_> {
    fn default() -> Self {
        Self {
            beats: None,
            drums: None,
            vocal: None,
            heard: 0.0,
            rain_strength: 1.0,
            bump_strength: 1.0,
            aqua_strength: [0.0; 4],
        }
    }
}
