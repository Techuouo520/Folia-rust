// Serializes session writes and restores paused playback without overtaking a new user selection.
use std::time::{Duration, Instant};

use folia_player::PlayerState;
use folia_store::{PlaybackSession, Store};
use iced::Task;

use crate::background::blocking;
use crate::{Message as AppMessage, Playback};

const SAVE_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(Option<Box<PlaybackSession>>),
    Saved(bool),
}

#[derive(Clone, Copy, PartialEq)]
struct Version {
    queue: u64,
    index: Option<usize>,
    track: Option<i64>,
    state: PlayerState,
}

impl Version {
    fn of(player: &Playback) -> Self {
        Self {
            queue: player.queue.revision(),
            index: player.queue.current_index(),
            track: player.current().map(|track| track.id),
            state: player.status.state,
        }
    }
}

#[derive(Clone, Copy)]
struct Stamp {
    version: Version,
    position: f64,
}

impl Stamp {
    fn of(player: &Playback) -> Self {
        Self { version: Version::of(player), position: player.heard_position() }
    }

    fn same(&self, other: Self) -> bool {
        self.version == other.version && (self.position - other.position).abs() < 0.001
    }
}

pub struct Session {
    restore: bool,
    ready: bool,
    initial: Version,
    saved: Option<Stamp>,
    writing: Option<Stamp>,
    last_saved: Instant,
    retry_after: Option<Instant>,
    closing: bool,
    final_save: Option<(Stamp, PlaybackSession)>,
}

impl Session {
    pub fn new(player: &Playback, restore: bool) -> Self {
        let stamp = Stamp::of(player);
        Self {
            restore,
            ready: false,
            initial: stamp.version,
            saved: Some(stamp),
            writing: None,
            last_saved: Instant::now(),
            retry_after: None,
            closing: false,
            final_save: None,
        }
    }

    pub fn update(&mut self, message: Message, player: &mut Playback, store: &Store) -> Task<AppMessage> {
        match message {
            Message::Loaded(snapshot) if !self.ready && !self.closing => {
                self.ready = true;
                let untouched = Stamp::of(player).version == self.initial;
                let task = if self.restore && untouched {
                    snapshot.map_or_else(Task::none, |snapshot| player.restore(*snapshot).map(AppMessage::Playback))
                } else {
                    Task::none()
                };
                self.saved = untouched.then(|| Stamp::of(player));
                task
            }
            Message::Loaded(_) => Task::none(),
            Message::Saved(success) => {
                let Some(stamp) = self.writing.take() else { return Task::none() };
                self.last_saved = Instant::now();
                self.saved = success.then_some(stamp);
                self.retry_after = (!success).then(|| Instant::now() + SAVE_INTERVAL);
                if let Some((stamp, snapshot)) = self.final_save.take() {
                    self.write(stamp, snapshot, store)
                } else if self.closing {
                    iced::exit()
                } else {
                    Task::none()
                }
            }
        }
    }

    pub fn observe(&mut self, player: &Playback, store: &Store, seek_finished: bool) -> Task<AppMessage> {
        if !self.ready
            || self.writing.is_some()
            || self.closing
            || self.retry_after.is_some_and(|deadline| Instant::now() < deadline)
        {
            return Task::none();
        }
        if !seek_finished
            && self.last_saved.elapsed() < SAVE_INTERVAL
            && self.saved.is_some_and(|saved| saved.version == Version::of(player))
        {
            return Task::none();
        }
        let stamp = Stamp::of(player);
        let due = self.saved.is_none_or(|saved| {
            saved.version != stamp.version
                || ((seek_finished || self.last_saved.elapsed() >= SAVE_INTERVAL) && !saved.same(stamp))
        });
        if due { self.write(stamp, player.snapshot(), store) } else { Task::none() }
    }

    pub fn close(&mut self, player: &Playback, store: &Store) -> Task<AppMessage> {
        if self.closing {
            return Task::none();
        }
        self.closing = true;
        let stamp = Stamp::of(player);
        if self.writing.is_none()
            && ((!self.ready && stamp.version == self.initial) || self.saved.is_some_and(|saved| saved.same(stamp)))
        {
            return iced::exit();
        }
        let snapshot = player.snapshot();
        if self.writing.is_some() {
            self.final_save = Some((stamp, snapshot));
            Task::none()
        } else {
            self.write(stamp, snapshot, store)
        }
    }

    fn write(&mut self, stamp: Stamp, snapshot: PlaybackSession, store: &Store) -> Task<AppMessage> {
        self.writing = Some(stamp);
        let store = store.clone();
        Task::perform(
            async move {
                blocking(move || match store.save(&snapshot) {
                    Ok(()) => {
                        tracing::debug!(items = snapshot.queue.items.len(), "playback session saved");
                        true
                    }
                    Err(error) => {
                        tracing::error!(
                            error = &error as &dyn std::error::Error,
                            "playback session could not be saved"
                        );
                        false
                    }
                })
                .await
            },
            |result| {
                if result.is_none() {
                    tracing::error!("playback session writer stopped");
                }
                AppMessage::Session(Message::Saved(result.unwrap_or(false)))
            },
        )
    }
}

pub fn load(store: Store) -> Task<AppMessage> {
    Task::perform(
        async move {
            blocking(move || match store.load::<PlaybackSession>() {
                Ok(snapshot) => Some(Box::new(snapshot)),
                Err(error) => {
                    tracing::warn!(error = &error as &dyn std::error::Error, "playback session could not be restored");
                    None
                }
            })
            .await
        },
        |result| {
            if result.is_none() {
                tracing::error!("playback session reader stopped");
            }
            AppMessage::Session(Message::Loaded(result.flatten()))
        },
    )
}
