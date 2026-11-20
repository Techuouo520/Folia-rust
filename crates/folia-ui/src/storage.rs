// Cache relocation coordinates paused playback, quiescent writers and verified background transfers.

use std::path::PathBuf;

use folia_i18n::I18n;
use folia_omni::Omni;
use folia_store::{CacheMoveResult, StorageSettings, Store};
use iced::futures::SinkExt;
use iced::widget::{button, column, container, opaque, progress_bar, row, space, stack};
use iced::{Alignment, Color, Element, Length, Task};

use crate::settings_kit::{self as kit, Group, Row, Tokens};
use crate::typeface::text;
use crate::{Playback, scene_sound::SceneSound};

#[derive(Debug, Clone)]
pub enum Message {
    Choose,
    Picked(Option<PathBuf>),
    Reset,
    Progress(u64, u64),
    Moved(Result<CacheMoveResult, String>),
}

pub struct Storage {
    settings: StorageSettings,
    default: PathBuf,
    pub active: PathBuf,
    busy: bool,
    pub moving: bool,
    close_after: bool,
    progress: (u64, u64),
    failed: bool,
    cleanup_pending: bool,
}

impl Storage {
    pub fn new(settings: StorageSettings, default: PathBuf) -> Self {
        let active = settings.cache_path(&default).to_path_buf();
        let cleanup_pending = settings.migration.as_ref().is_some_and(|plan| plan.cleanup);
        let failed = settings.migration.is_some() && !cleanup_pending;
        Self {
            settings,
            default,
            active,
            busy: false,
            moving: false,
            close_after: false,
            progress: (0, 0),
            failed,
            cleanup_pending,
        }
    }

    pub fn defer_close(&mut self) -> bool {
        if self.moving {
            self.close_after = true;
            true
        } else {
            false
        }
    }

    pub fn take_close(&mut self) -> bool {
        !self.moving && std::mem::take(&mut self.close_after)
    }

    pub fn update(
        &mut self,
        message: Message,
        store: &Store,
        omni: Option<Omni>,
        playback: &mut Playback,
        room: &mut SceneSound,
    ) -> Task<crate::Message> {
        match message {
            Message::Choose if !self.busy => {
                self.busy = true;
                self.failed = false;
                let path = self.active.clone();
                Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_directory(path)
                            .pick_folder()
                            .await
                            .map(|folder| folder.path().to_path_buf())
                    },
                    |path| crate::Message::Storage(Message::Picked(path)),
                )
            }
            Message::Picked(Some(path)) if !self.moving => self.migrate(Some(path), store, omni, playback, room),
            Message::Picked(None) if !self.moving => {
                self.busy = false;
                Task::none()
            }
            Message::Reset if !self.busy => self.migrate(None, store, omni, playback, room),
            Message::Progress(done, total) => {
                self.progress = (done, total);
                Task::none()
            }
            Message::Moved(result) => {
                self.busy = false;
                self.moving = false;
                self.failed = result.is_err();
                if let Ok(result) = result {
                    self.settings = result.settings;
                    self.active = self.settings.cache_path(&self.default).to_path_buf();
                    self.cleanup_pending = result.cleanup_pending;
                }
                Task::batch([
                    room.resume_cache(self.active.clone()),
                    playback.resume_cache(store, !self.close_after).map(crate::Message::Playback),
                ])
            }
            _ => Task::none(),
        }
    }

    fn migrate(
        &mut self,
        path: Option<PathBuf>,
        store: &Store,
        omni: Option<Omni>,
        playback: &mut Playback,
        room: &mut SceneSound,
    ) -> Task<crate::Message> {
        self.busy = true;
        self.moving = true;
        self.failed = false;
        self.progress = (0, 0);
        let release = playback.suspend_cache();
        let (engine, room_task) = room.pause_cache(playback);
        if let Some(omni) = &omni {
            omni.pause_cache();
        }
        let store = store.clone();
        let default = self.default.clone();
        let source = self.active.clone();
        let stream = iced::stream::channel::<Message>(
            8,
            move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                let switch = match &omni {
                    Some(omni) => {
                        match tokio::time::timeout(std::time::Duration::from_secs(30), omni.suspend_cache()).await {
                            Ok(switch) => Some(switch),
                            Err(error) => {
                                tracing::warn!(
                                    error = &error as &dyn std::error::Error,
                                    "cache readers did not finish in time"
                                );
                                let _ = output.send(Message::Moved(Err("cache readers are still busy".into()))).await;
                                return;
                            }
                        }
                    }
                    None => None,
                };
                let ready = release_inputs(release, engine, source.clone()).await;
                let result = match ready {
                    Ok(()) => {
                        let mut progress = output.clone();
                        let source = source.clone();
                        let default = default.clone();
                        crate::background::blocking(move || {
                            let mut sent = std::time::Instant::now();
                            StorageSettings::migrate(&store, &source, path, &default, |done, total| {
                                if done == 0 || done == total || sent.elapsed() >= std::time::Duration::from_millis(100)
                                {
                                    let _ = progress.try_send(Message::Progress(done, total));
                                    sent = std::time::Instant::now();
                                }
                            })
                            .map_err(|error| {
                                tracing::warn!(error = &error as &dyn std::error::Error, "cache migration failed");
                                folia_store::error_chain(&error)
                            })
                        })
                        .await
                        .unwrap_or_else(|| Err("cache migration worker stopped".into()))
                    }
                    Err(error) => Err(error),
                };
                let root = result.as_ref().map_or(source, |result| result.settings.cache_path(&default).to_path_buf());
                if let Some(switch) = switch {
                    switch.resume(root);
                }
                let _ = output.send(Message::Moved(result)).await;
            },
        );
        Task::batch([room_task, Task::run(stream, crate::Message::Storage)])
    }

    pub fn group<'a>(&self, i18n: &'a I18n, tokens: Tokens) -> Group<'a, crate::Message> {
        let action = |key, message, enabled: bool| -> Element<'a, crate::Message> {
            button(text(i18n.t(key)).size(13).color(tokens.t1))
                .padding([6, 12])
                .style(move |_, _| kit::rounded_button(tokens.fill, 8.0))
                .on_press_maybe(enabled.then_some(crate::Message::Storage(message)))
                .into()
        };
        let choose = action("options.chooseCacheDirectory", Message::Choose, !self.busy);
        let reset = action(
            "options.resetCacheDirectory",
            Message::Reset,
            !self.busy && self.settings.cache_directory.is_some(),
        );
        let foot = if self.failed {
            Some(i18n.t("options.cacheMigrationFailed").into())
        } else if self.cleanup_pending {
            Some(i18n.t("options.cacheMigrationCleanup").into())
        } else {
            None
        };
        Group {
            title: i18n.t("options.mediaCache").into(),
            bare: false,
            rows: vec![Row {
                note: None,
                label: i18n.t("options.cacheDirectory").into(),
                hint: Some(display_path(&self.active)),
                sub: false,
                below: None,
                control: row![choose, reset].spacing(8).into(),
            }],
            foot,
        }
    }

    pub fn overlay<'a>(&self, i18n: &'a I18n, tokens: Tokens) -> Element<'a, crate::Message> {
        let percent = if self.progress.1 == 0 {
            0.0
        } else {
            (100.0 * self.progress.0 as f32 / self.progress.1 as f32).min(99.0)
        };
        let card = container(
            column![
                text(i18n.t(if self.close_after {
                    "options.cacheMigratingBeforeClose"
                } else {
                    "options.cacheMigrating"
                }))
                .size(16),
                progress_bar(0.0..=100.0, percent),
                text(format!("{percent:.0}%")).size(13)
            ]
            .spacing(16),
        )
        .padding(24)
        .width(320)
        .style(move |_| kit::rounded(tokens.card, 16.0));
        opaque(stack![
            container(space())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| kit::rounded(Color::from_rgba(0.0, 0.0, 0.0, 0.3), 0.0)),
            container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
        ])
    }
}

async fn release_inputs(
    release: Option<std::sync::mpsc::Receiver<()>>,
    engine: Option<folia_acoustics::Engine>,
    source: PathBuf,
) -> Result<(), String> {
    if let Some(release) = release {
        crate::background::blocking(move || release.recv_timeout(std::time::Duration::from_secs(10)))
            .await
            .ok_or("decoder release worker stopped")?
            .map_err(|error| format!("decoder did not release its file: {error}"))?;
    }
    if let Some(engine) = engine {
        engine
            .set_cache_path(source.join("room_sound.sqlite"))
            .await
            .map_err(|error| crate::background::error_chain(&error))?;
    }
    Ok(())
}

fn display_path(path: &std::path::Path) -> String {
    let value = path.display().to_string();
    #[cfg(windows)]
    {
        if let Some(path) = value.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{path}");
        }
        if let Some(path) = value.strip_prefix(r"\\?\") {
            return path.to_owned();
        }
    }
    value
}
