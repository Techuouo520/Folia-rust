// Native GPU model execution and track analysis shared by visualizers.
pub mod accents;
pub mod audio;
pub mod beat_this;
pub mod cache;
pub mod gpu;
pub mod htdemucs;
pub mod models;
pub mod percussive;
pub mod rhythm;
pub mod signal;
pub mod worker;

#[derive(Clone, Debug)]
pub enum AnalysisData {
    Beats(std::sync::Arc<Vec<folia_core::BeatEvent>>),
    Stems(std::sync::Arc<folia_core::StemAnalysis>),
}

impl AnalysisData {
    pub fn model(&self) -> models::ModelKind {
        match self {
            Self::Beats(_) => models::ModelKind::BeatThis,
            Self::Stems(_) => models::ModelKind::Htdemucs,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("cannot read analysis model")]
    Io(#[from] std::io::Error),
    #[error("native analysis compiler is unavailable at {path}")]
    Runtime {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot decode audio for analysis")]
    Audio(#[from] folia_player::PlayerError),
    #[error("cannot resample audio for analysis: {0}")]
    Resample(String),
    #[error("invalid model manifest")]
    Manifest(#[from] serde_json::Error),
    #[error("invalid packaged model manifest")]
    ModelManifest(#[from] toml::de::Error),
    #[error("analysis cache failed")]
    Cache(#[from] rusqlite::Error),
    #[error("analysis model is invalid: {0}")]
    Model(String),
    #[error("GPU analysis is unavailable: {0}")]
    Gpu(String),
    #[error("analysis was cancelled")]
    Cancelled,
}
