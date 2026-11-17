// Song-theme automation, asynchronous cache access and cancellable generation for the current playback request.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use folia_core::{Rgb, Track};
use folia_store::{AiSettings, DualTheme, SongThemeSettings, Store, ThemeGenerationSource};
use iced::Task;

use crate::background::{blocking, error_chain, save};
use crate::{Message as AppMessage, Playback};

#[derive(Clone)]
pub enum Message {
    Loaded(AiSettings, SongThemeSettings, Result<bool, String>, Option<DualTheme>),
    Cached(u64, Result<Option<DualTheme>, String>),
    Due(u64),
    Generated(u64, Result<DualTheme, String>),
    SavedManual(String, Result<DualTheme, String>),
    AutoSwitch(bool),
    AutoGenerate(bool),
    PreferCustom(bool),
    Source(ThemeGenerationSource),
    Generate,
    Protocol(folia_store::AiProtocol),
    Endpoint(String),
    Model(String),
    Key(String),
    Temperature(f32),
    Proxy(bool),
    SaveConnection,
    ClearKey,
    TestConnection,
    ConnectionSaved(u64, AiSettings, Result<bool, String>),
    Tested(u64, Result<(), String>),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("SongThemeMessage").field(&std::mem::discriminant(self)).finish()
    }
}

pub struct SongThemes {
    pub preferences: SongThemeSettings,
    pub connection: AiSettings,
    pub draft: AiSettings,
    pub key_draft: String,
    pub has_key: bool,
    pub connection_busy: bool,
    pub testing_connection: bool,
    pub connection_result: Option<bool>,
    pub current: Option<DualTheme>,
    pub applied: Option<DualTheme>,
    pub theme_changed: bool,
    pub generating: bool,
    ready: bool,
    service: String,
    database: PathBuf,
    request: Option<u64>,
    key: Option<String>,
    generation: u64,
    connection_generation: u64,
    cache_loaded: bool,
    lyrics_ready: bool,
    due: bool,
    attempted: HashSet<String>,
    task: Option<iced::task::Handle>,
}

impl SongThemes {
    pub fn is_ready(&self) -> bool {
        self.ready
    }
    pub fn new(store: Store, data: PathBuf, service: String) -> (Self, Task<AppMessage>) {
        let worker_service = service.clone();
        let last_database = data.join("song_themes.db");
        let task = Task::perform(
            blocking(move || {
                let connection = store.load_or_default::<AiSettings>();
                let preferences = store.load_or_default::<SongThemeSettings>();
                let key = folia_ai::has_key(&worker_service, &connection).map_err(|error| {
                    tracing::warn!(error = &error as &dyn std::error::Error, "AI credential status unavailable");
                    error_chain(&error)
                });
                let last =
                    folia_ai::ThemeCache::open(&last_database).and_then(|cache| cache.last()).unwrap_or_else(|error| {
                        tracing::warn!(error = &error as &dyn std::error::Error, "last song theme unavailable");
                        None
                    });
                Message::Loaded(connection, preferences, key, last)
            }),
            |message| {
                AppMessage::SongTheme(message.unwrap_or_else(|| {
                    Message::Loaded(AiSettings::default(), SongThemeSettings::default(), Ok(false), None)
                }))
            },
        );
        (
            Self {
                preferences: SongThemeSettings::default(),
                connection: AiSettings::default(),
                draft: AiSettings::default(),
                key_draft: String::new(),
                has_key: false,
                connection_busy: false,
                testing_connection: false,
                connection_result: None,
                current: None,
                applied: None,
                theme_changed: false,
                generating: false,
                ready: false,
                service,
                database: data.join("song_themes.db"),
                request: None,
                key: None,
                generation: 0,
                connection_generation: 0,
                cache_loaded: false,
                lyrics_ready: false,
                due: false,
                attempted: HashSet::new(),
                task: None,
            },
            task,
        )
    }

    pub fn observe(
        &mut self,
        playback: &Playback,
        colors: &[Rgb],
        cover_ready: bool,
        lyrics_settled: bool,
    ) -> Task<AppMessage> {
        if !self.ready {
            return Task::none();
        }
        let request = playback.current().map(|_| playback.request_id());
        if request != self.request {
            self.cancel();
            self.request = request;
            self.key = playback.current().map(folia_ai::song_key);
            self.current = None;
            self.cache_loaded = false;
            self.lyrics_ready = false;
            let Some(key) = self.key.clone() else {
                return Task::none();
            };
            let generation = playback.request_id();
            let database = self.database.clone();
            return Task::perform(
                blocking(move || {
                    folia_ai::ThemeCache::open(&database).and_then(|cache| cache.get(&key)).map_err(|error| {
                        tracing::warn!(error = &error as &dyn std::error::Error, "song theme cache unreadable");
                        error_chain(&error)
                    })
                }),
                move |result| {
                    AppMessage::SongTheme(Message::Cached(
                        generation,
                        result.unwrap_or_else(|| Err("theme cache task failed".into())),
                    ))
                },
            );
        }
        self.lyrics_ready |= lyrics_settled || !playback.lyrics_loading();
        let Some(key) = self.key.as_ref() else {
            return Task::none();
        };
        if !self.preferences.auto_generate
            || !self.preferences.auto_switch
            || self.current.is_some()
            || !self.cache_loaded
            || self.generating
            || self.attempted.contains(key)
        {
            return Task::none();
        }
        let source_ready = match self.preferences.generation_source {
            ThemeGenerationSource::Cover => cover_ready,
            ThemeGenerationSource::Ai => self.lyrics_ready && (self.has_key || cover_ready),
        };
        if !source_ready {
            return Task::none();
        }
        if self.due {
            return self.generate(playback, colors);
        }
        self.due = false;
        self.generating = true;
        let generation = self.generation;
        let (task, handle) = Task::perform(
            async move {
                tokio::time::sleep(Duration::from_millis(650)).await;
            },
            move |_| AppMessage::SongTheme(Message::Due(generation)),
        )
        .abortable();
        self.task = Some(handle.abort_on_drop());
        task
    }

    pub fn update(&mut self, message: Message, store: &Store, playback: &Playback, colors: &[Rgb]) -> Task<AppMessage> {
        if !self.ready && !matches!(message, Message::Loaded(..)) {
            return Task::none();
        }
        match message {
            Message::Loaded(connection, preferences, key, last) => {
                self.connection = connection.clone();
                self.draft = connection;
                self.preferences = preferences;
                self.has_key = key.unwrap_or(false);
                self.ready = true;
                self.applied = last;
            }
            Message::Cached(generation, result) if Some(generation) == self.request => {
                self.cache_loaded = true;
                if let Ok(theme) = result {
                    self.current = theme;
                    if self.preferences.auto_switch && self.current.is_some() {
                        self.applied = self.current.clone();
                        self.theme_changed = true;
                    }
                }
            }
            Message::Due(generation) if generation == self.generation => {
                self.task = None;
                self.generating = false;
                self.due = true;
            }
            Message::Generated(generation, result) if generation == self.generation => {
                self.task = None;
                self.generating = false;
                if let Ok(theme) = result {
                    self.current = Some(theme.clone());
                    self.applied = Some(theme);
                    self.theme_changed = true;
                }
            }
            Message::SavedManual(song, Ok(theme)) if self.key.as_ref() == Some(&song) => {
                self.current = Some(theme.clone());
                self.applied = Some(theme);
                self.theme_changed = true;
            }
            Message::AutoSwitch(enabled) => {
                self.preferences.set_auto_switch(enabled);
                self.cancel();
                if enabled && self.current.is_some() {
                    self.applied = self.current.clone();
                    self.theme_changed = true;
                }
                return save(store, &self.preferences);
            }
            Message::AutoGenerate(enabled) => {
                self.preferences.set_auto_generate(enabled);
                self.cancel();
                return save(store, &self.preferences);
            }
            Message::PreferCustom(enabled) => {
                self.preferences.set_prefer_custom(enabled);
                self.cancel();
                return save(store, &self.preferences);
            }
            Message::Source(source) => {
                self.preferences.generation_source = source;
                self.cancel();
                if let Some(key) = &self.key {
                    self.attempted.remove(key);
                }
                return save(store, &self.preferences);
            }
            Message::Generate => return self.generate(playback, colors),
            Message::Protocol(protocol) => {
                self.draft.protocol = protocol;
                let (endpoint, model) = match protocol {
                    folia_store::AiProtocol::Gemini => {
                        ("https://generativelanguage.googleapis.com/v1beta", "gemini-3-flash-preview")
                    }
                    folia_store::AiProtocol::Openai => ("https://api.openai.com/v1", "gpt-5.6-luna"),
                    folia_store::AiProtocol::Anthropic => ("https://api.anthropic.com/v1", "claude-haiku-5-5"),
                };
                self.draft.endpoint = endpoint.into();
                self.draft.model = model.into();
                self.key_draft.clear();
                self.connection_result = None;
            }
            Message::Endpoint(value) => {
                self.draft.endpoint = value;
                self.connection_result = None;
            }
            Message::Model(value) => {
                self.draft.model = value;
                self.connection_result = None;
            }
            Message::Key(value) => self.key_draft = value,
            Message::Temperature(value) => self.draft.temperature = value,
            Message::Proxy(value) => self.draft.use_system_proxy = value,
            Message::SaveConnection | Message::ClearKey | Message::TestConnection => {
                return self.connection_task(message, store);
            }
            Message::ConnectionSaved(generation, snapshot, result) if generation == self.connection_generation => {
                self.connection_busy = false;
                self.connection_result = Some(result.is_ok());
                if let Ok(has_key) = result {
                    if self.draft == snapshot {
                        self.key_draft.clear();
                    }
                    self.connection = snapshot;
                    self.has_key = has_key;
                }
            }
            Message::Tested(generation, result) if generation == self.connection_generation => {
                self.connection_busy = false;
                self.connection_result = Some(result.is_ok());
            }
            _ => {}
        }
        Task::none()
    }

    fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
        self.generating = false;
        self.due = false;
    }

    pub(crate) fn active_error(&self, message: &Message) -> bool {
        matches!(message,Message::Generated(generation,Err(_)) if *generation==self.generation)
            || matches!(message,Message::SavedManual(song,Err(_)) if self.key.as_ref()==Some(song))
    }

    pub fn remember(&self) -> Task<AppMessage> {
        let Some(theme) = self.applied.clone() else {
            return Task::none();
        };
        let database = self.database.clone();
        let revision = folia_ai::ThemeCache::reserve_revision();
        Task::future(blocking(move || {
            if let Err(error) = folia_ai::ThemeCache::open(&database).and_then(|cache| cache.remember(&theme, revision))
            {
                tracing::warn!(error = &error as &dyn std::error::Error, "last song theme not saved");
            }
        }))
        .discard()
    }

    pub fn can_generate(&self, playback: &Playback) -> bool {
        playback.current().is_some()
            && !self.generating
            && (self.preferences.generation_source == ThemeGenerationSource::Cover || !playback.lyrics_loading())
    }

    fn generate(&mut self, playback: &Playback, colors: &[Rgb]) -> Task<AppMessage> {
        let Some(track) = playback.current().cloned() else {
            return Task::none();
        };
        if !self.can_generate(playback) {
            return Task::none();
        }
        self.cancel();
        let generation = self.generation;
        let key = folia_ai::song_key(&track);
        self.attempted.insert(key.clone());
        let lyrics = playback
            .lyrics
            .as_ref()
            .map(|lyrics| lyrics.lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();
        let colors = colors.to_vec();
        let source = if self.preferences.generation_source == ThemeGenerationSource::Ai && !self.has_key {
            ThemeGenerationSource::Cover
        } else {
            self.preferences.generation_source
        };
        let settings = self.connection.clone();
        let service = self.service.clone();
        let destination = ThemeWrite::new(self.database.clone());
        self.generating = true;
        let (task, handle) = Task::perform(
            async move {
                generate(source, settings, service, track, lyrics, colors, destination).await.map_err(|error| {
                    tracing::warn!(error = &error as &dyn std::error::Error, "song theme generation failed");
                    error_chain(&error)
                })
            },
            move |result| AppMessage::SongTheme(Message::Generated(generation, result)),
        )
        .abortable();
        self.task = Some(handle.abort_on_drop());
        task
    }

    pub fn save_manual(&mut self, song: String, theme: DualTheme) -> Task<AppMessage> {
        if self.key.as_ref() == Some(&song) {
            self.cancel();
            self.attempted.insert(song.clone());
        }
        let destination = ThemeWrite::new(self.database.clone());
        let key = song.clone();
        Task::perform(async move { destination.save(key, theme).await.map_err(log_error) }, move |result| {
            AppMessage::SongTheme(Message::SavedManual(song.clone(), result))
        })
    }

    fn connection_task(&mut self, message: Message, store: &Store) -> Task<AppMessage> {
        if self.connection_busy {
            return Task::none();
        }
        self.connection_busy = true;
        self.testing_connection = matches!(message, Message::TestConnection);
        self.connection_result = None;
        self.connection_generation = self.connection_generation.wrapping_add(1);
        let generation = self.connection_generation;
        let settings = self.draft.clone();
        let key = self.key_draft.clone();
        let service = self.service.clone();
        let store = store.clone();
        Task::perform(
            async move {
                if matches!(message, Message::TestConnection) {
                    let client = if key.is_empty() {
                        folia_ai::Client::from_store(service, settings).await
                    } else {
                        folia_ai::Client::with_key(settings, key)
                    };
                    let result = match client {
                        Ok(client) => client.test_connection().await,
                        Err(error) => Err(error),
                    };
                    return Message::Tested(generation, result.map_err(log_error));
                }
                let snapshot = settings.clone();
                let result = tokio::task::spawn_blocking(move || -> Result<bool, String> {
                    if matches!(message, Message::ClearKey) {
                        folia_ai::remove_key(&service, &settings).map_err(log_error)?;
                    } else if !key.is_empty() {
                        folia_ai::save_key(&service, &settings, &key).map_err(log_error)?;
                    }
                    let has_key = folia_ai::has_key(&service, &settings).map_err(log_error)?;
                    store.save(&settings).map_err(|error| error_chain(&error))?;
                    Ok(has_key)
                })
                .await
                .map_err(|error| error_chain(&error))
                .and_then(|value| value);
                Message::ConnectionSaved(generation, snapshot, result)
            },
            AppMessage::SongTheme,
        )
    }
}

fn log_error(error: folia_ai::Error) -> String {
    tracing::warn!(error = &error as &dyn std::error::Error, "AI connection operation failed");
    error_chain(&error)
}

async fn generate(
    source: ThemeGenerationSource,
    settings: AiSettings,
    service: String,
    track: Track,
    lyrics: String,
    colors: Vec<Rgb>,
    destination: ThemeWrite,
) -> Result<DualTheme, folia_ai::Error> {
    let key = folia_ai::song_key(&track);
    let generated = match source {
        ThemeGenerationSource::Ai => match folia_ai::Client::from_store(service, settings).await {
            Ok(client) => client.generate_theme(&track.title, &lyrics).await.map(Some),
            Err(folia_ai::Error::MissingKey) => Ok(None),
            Err(error) => Err(error),
        }?,
        ThemeGenerationSource::Cover => None,
    };
    let theme = match generated {
        Some(theme) => theme,
        None => {
            let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()
                as u64;
            tokio::task::spawn_blocking(move || folia_ai::cover_theme(&colors, seed)).await?
        }
    };
    destination.save(key, theme).await
}

struct ThemeWrite {
    database: PathBuf,
    revision: i64,
}

impl ThemeWrite {
    fn new(database: PathBuf) -> Self {
        Self { database, revision: folia_ai::ThemeCache::reserve_revision() }
    }

    async fn save(self, song: String, theme: DualTheme) -> Result<DualTheme, folia_ai::Error> {
        tokio::task::spawn_blocking(move || {
            folia_ai::ThemeCache::open(&self.database)?.put_revision(&song, &theme, self.revision)?;
            Ok(theme)
        })
        .await?
    }
}
