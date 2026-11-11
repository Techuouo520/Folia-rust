// LX custom source API, isolated JavaScript workers and persistent source selection.

mod bridge;
mod crypto;
mod http;
mod metadata;
mod registry;
mod runtime;

use std::time::Duration;

pub use folia_store::{CustomSource, SourceMetadata, SourcePlatform, SourceSettings};
pub use metadata::parse_metadata;
pub(crate) use registry::Sources;
pub(crate) const SCRIPT_BYTES: usize = 2 * 1024 * 1024;
pub use runtime::Script;

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("invalid source download URL")]
    Url,
    #[error("invalid source header")]
    Header,
    #[error("invalid source protocol")]
    Protocol,
    #[error("source execution failed")]
    Script,
    #[error("source timed out")]
    Timeout,
    #[error("source memory limit reached")]
    Memory,
    #[error("source resource limit reached")]
    Limit,
    #[error("source worker stopped")]
    Stopped,
    #[error("source encryption failed")]
    Crypto,
    #[error("source compression failed")]
    Compression,
    #[error("source HTTP request failed")]
    Http(#[source] reqwest::Error),
    #[error("cannot access source file")]
    Io(#[from] std::io::Error),
    #[error("cannot save source settings")]
    Store(#[from] folia_store::StoreError),
    #[error("source already imported")]
    Duplicate,
    #[error("source not found")]
    NotFound,
    #[error("sources have not been configured")]
    NotConfigured,
}

impl SourceError {
    pub fn label_key(&self) -> &'static str {
        match self {
            Self::Url => "options.sourceErrorUrl",
            Self::Http(_) => "options.sourceErrorDownload",
            Self::Header => "options.sourceErrorHeader",
            Self::Timeout => "options.sourceErrorTimeout",
            Self::Memory | Self::Limit => "options.sourceErrorLimit",
            Self::Io(_) | Self::Store(_) => "options.sourceErrorStorage",
            Self::Duplicate => "options.sourceErrorDuplicate",
            _ => "options.sourceErrorScript",
        }
    }
}

/// Limits can be reduced by embedders; every script gets its own heap and operation deadline.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub memory_bytes: usize,
    pub init_timeout: Duration,
    pub request_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 32 * 1024 * 1024,
            init_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(20),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct UpdateAlert {
    pub log: String,
    #[serde(rename = "updateUrl")]
    pub update_url: Option<String>,
}

/// Provider-owned conversion to LX's legacy musicInfo wire format.
pub struct MusicInfo {
    pub source: &'static str,
    pub data: serde_json::Value,
}

impl MusicInfo {
    pub(crate) fn track(source: &'static str, track: &folia_core::Track) -> Self {
        let seconds = track.duration.max(0.0) as u64;
        Self {
            source,
            data: serde_json::json!({"name":track.title,"singer":track.artist.as_deref().unwrap_or(""),
            "albumName":track.album.as_deref().unwrap_or(""),"albumId":track.online_info.as_ref().map(|i| i.album_id.as_str()).unwrap_or(""),
            "source":source,"interval":format!("{:02}:{:02}",seconds / 60, seconds % 60),"img":track.cover_url,
            "lrc":null,"otherSource":null,"types":[],"_types":{},"typeUrl":{}}),
        }
    }
}

pub fn quality_name(quality: folia_core::AudioQuality) -> &'static str {
    use folia_core::AudioQuality;
    match quality {
        AudioQuality::Standard => "128k",
        AudioQuality::High | AudioQuality::Ultra => "320k",
        AudioQuality::Lossless => "flac",
        AudioQuality::Hires => "flac24bit",
    }
}

pub fn quality_level(name: &str) -> Option<folia_core::AudioQuality> {
    use folia_core::AudioQuality;
    match name {
        "128k" => Some(AudioQuality::Standard),
        "320k" => Some(AudioQuality::High),
        "flac" => Some(AudioQuality::Lossless),
        "flac24bit" => Some(AudioQuality::Hires),
        _ => None,
    }
}
