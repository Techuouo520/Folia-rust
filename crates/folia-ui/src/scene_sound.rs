// Scene room sound owns a cancellable worker request and transfers prepared convolution storage once to playback.

use crate::{Message as AppMessage, Playback};
use folia_core::{RoomOutput, RoomRequest};
use folia_player::RoomConvolver;
use iced::Task;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

type Prepared = Arc<Mutex<Option<Box<RoomConvolver>>>>;

#[derive(Clone)]
pub enum Message {
    Initialized(Result<folia_acoustics::Engine, String>),
    Ready(u64, Result<Prepared, String>),
    Released(u64),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("SceneSoundMessage").field(&std::mem::discriminant(self)).finish()
    }
}

#[derive(PartialEq, Eq)]
struct Key {
    seat: Option<String>,
    output: RoomOutput,
    rate: u32,
}

pub struct SceneSound {
    engine: Option<folia_acoustics::Engine>,
    pub available: bool,
    pub loading: bool,
    pub error: Option<String>,
    current: Option<Key>,
    generation: u64,
    task: Option<iced::task::Handle>,
}

impl SceneSound {
    pub fn new(cache: PathBuf) -> (Self, Task<AppMessage>) {
        let task = Task::perform(
            crate::background::blocking(move || -> Result<folia_acoustics::Engine, String> {
                let path = folia_acoustics::library_path().map_err(|error| crate::background::error_chain(&error))?;
                if !path.is_file() {
                    return Err("Steam Audio library is not installed for this build".into());
                }
                folia_acoustics::Engine::new(path, cache.join("room_sound.sqlite"))
                    .map_err(|error| crate::background::error_chain(&error))
            }),
            |result| {
                AppMessage::SceneSound(Message::Initialized(
                    result.unwrap_or_else(|| Err("room-sound worker initialization failed".into())),
                ))
            },
        );
        (
            Self {
                engine: None,
                available: false,
                loading: false,
                error: None,
                current: None,
                generation: 0,
                task: None,
            },
            task,
        )
    }

    pub fn sync(
        &mut self,
        settings: &folia_store::SceneSettings,
        playing_page: bool,
        playback: &mut Playback,
    ) -> Task<AppMessage> {
        let rate = (playing_page && settings.scene_id.is_some() && settings.scene_room_sound)
            .then(|| playback.room_sample_rate())
            .flatten();
        let same = match (rate, self.current.as_ref()) {
            (None, None) => true,
            (Some(rate), Some(current)) => {
                current.rate == rate
                    && current.output == settings.scene_room_output
                    && current.seat.as_deref() == settings.scene_options.cinema.viewpoint.as_deref()
            }
            _ => false,
        };
        if same {
            return Task::none();
        }
        self.current = rate.map(|rate| Key {
            seat: settings.scene_options.cinema.viewpoint.clone(),
            output: settings.scene_room_output,
            rate,
        });
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
        self.loading = false;
        let Some(key) = self.current.as_ref() else {
            return stop(playback, self.generation);
        };
        let Some(engine) = self.engine.clone() else {
            return Task::none();
        };
        let seat = key.seat.clone();
        let rate = key.rate;
        let output = key.output;
        let generation = self.generation;
        self.loading = true;
        self.error = None;
        let (task, handle) = Task::perform(
            async move {
                let world = folia_visualizer::cinema::acoustic_world()
                    .map_err(|error| crate::background::error_chain(&error))?;
                let listener = folia_visualizer::cinema::listener(seat.as_deref());
                let reference = folia_visualizer::cinema::listener(None);
                let request = RoomRequest {
                    world,
                    speakers: folia_visualizer::cinema::SPEAKERS,
                    listener,
                    output,
                    sample_rate: rate,
                };
                let impulse = engine.render(request, reference).await.map_err(|error| {
                    tracing::warn!(error = &error as &dyn std::error::Error, "scene room simulation failed");
                    crate::background::error_chain(&error)
                })?;
                let prepared = tokio::task::spawn_blocking(move || RoomConvolver::new(&impulse))
                    .await
                    .map_err(|error| crate::background::error_chain(&error))?
                    .map_err(|error| crate::background::error_chain(&error))?;
                // Iced messages are cloneable; only the first accepted delivery takes the prepared audio state.
                Ok(Arc::new(Mutex::new(Some(Box::new(prepared)))))
            },
            move |result| AppMessage::SceneSound(Message::Ready(generation, result)),
        )
        .abortable();
        self.task = Some(handle.abort_on_drop());
        task
    }

    pub fn update(&mut self, message: Message, playback: &mut Playback) -> Task<AppMessage> {
        match message {
            Message::Initialized(Ok(engine)) => {
                self.engine = Some(engine);
                self.available = true;
                self.current = None;
            }
            Message::Initialized(Err(error)) => {
                self.error = Some(error);
                self.available = false;
            }
            Message::Ready(generation, result) if generation == self.generation && self.current.is_some() => {
                self.task = None;
                self.loading = false;
                match result {
                    Ok(prepared) => {
                        let prepared = prepared.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
                        if let Some(prepared) = prepared
                            && let Err(error) = playback.set_room(Some(prepared))
                        {
                            self.error = Some(crate::background::error_chain(&error));
                            return stop(playback, self.generation);
                        }
                    }
                    Err(error) => {
                        self.error = Some(error);
                        return stop(playback, self.generation);
                    }
                }
            }
            Message::Released(generation) if generation == self.generation && !playback.finish_room_change() => {
                return release_after(generation, 100);
            }
            _ => {}
        }
        Task::none()
    }
}

fn stop(playback: &mut Playback, generation: u64) -> Task<AppMessage> {
    if let Err(error) = playback.set_room(None) {
        tracing::warn!(error = &error as &dyn std::error::Error, "room sound could not be disabled");
    }
    release_after(generation, 650)
}

fn release_after(generation: u64, milliseconds: u64) -> Task<AppMessage> {
    Task::perform(
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(milliseconds)).await;
        },
        move |_| AppMessage::SceneSound(Message::Released(generation)),
    )
}
