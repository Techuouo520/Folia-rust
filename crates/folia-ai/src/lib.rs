// Native theme generation, endpoint-bound credentials and a source-aware song theme cache.

mod cache;
mod client;
mod color;
mod credentials;
mod palette;
mod palette_words;
mod theme;

pub use cache::{ThemeCache, song_key};
pub use client::{Client, temperature_supported};
pub use credentials::{has_key, remove_key, same_endpoint, save_key};
pub use palette::cover_theme;
pub use theme::{default_theme, parse_theme, theme_json, theme_prompt};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid AI connection settings: {0}")]
    Configuration(&'static str),
    #[error("AI API key is not configured")]
    MissingKey,
    #[error("cannot access the system credential store")]
    Credential(#[from] keyring::Error),
    #[error("AI request failed")]
    Network(#[source] reqwest::Error),
    #[error("AI service returned HTTP {0}")]
    Status(u16),
    #[error("AI response exceeds the size limit")]
    ResponseTooLarge,
    #[error("AI response did not contain a complete theme")]
    InvalidResponse,
    #[error("cannot read the theme cache")]
    Database(#[from] rusqlite::Error),
    #[error("cannot create the theme cache directory")]
    Io(#[from] std::io::Error),
    #[error("cannot parse the cached theme")]
    Json(#[from] serde_json::Error),
    #[error("background AI task ended unexpectedly")]
    Task(#[from] tokio::task::JoinError),
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Network(error.without_url())
    }
}
