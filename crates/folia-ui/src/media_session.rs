// Bridges Linux desktop media controls through Shell and publishes coalesced playback snapshots.
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use folia_media::{Artwork, Command, Metadata, Snapshot, Status};
use folia_player::PlayerState;
use iced::advanced::image::{Handle, Id};
use iced::futures::SinkExt;
use iced::{Subscription, Task};
use tokio::sync::{mpsc, watch};

use crate::{Message, Playback, Shell};

static INSTANCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct Input {
    id: u64,
    cache: PathBuf,
    states: watch::Receiver<Snapshot>,
}

impl Hash for Input {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

#[derive(PartialEq)]
struct Version {
    track: Option<i64>,
    queue: u64,
    request: u64,
    status: PlayerState,
    volume: f32,
    duration: f64,
    buffering: bool,
    seek: u64,
    art: Option<Id>,
    next: bool,
    previous: bool,
}

pub(crate) struct Bridge {
    input: Input,
    sender: watch::Sender<Snapshot>,
    version: Option<Version>,
    at: Instant,
    artwork: Option<(Id, Arc<Artwork>)>,
}

impl Bridge {
    pub fn new(cache: PathBuf) -> Self {
        let (sender, states) = watch::channel(Snapshot::default());
        Self {
            input: Input { id: INSTANCE.fetch_add(1, Ordering::Relaxed), cache, states },
            sender,
            version: None,
            at: Instant::now(),
            artwork: None,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run_with(self.input.clone(), |input| {
            let input = input.clone();
            iced::stream::channel(16, async move |mut output| {
                let (commands, mut receiver) = mpsc::channel(32);
                let service = folia_media::mpris::Service::new(input.cache);
                let serving = service.run(None, input.states, commands);
                tokio::pin!(serving);
                loop {
                    tokio::select! {
                        _ = &mut serving => break,
                        command = receiver.recv() => {
                            let Some(command) = command else { break };
                            if output.send(Message::Media(command)).await.is_err() { break; }
                        }
                    }
                }
            })
        })
    }

    pub fn sync(&mut self, player: &Playback, art: Option<&Handle>) {
        let playing = player.is_playing();
        let version = Version {
            track: player.current().map(|track| track.id),
            queue: player.queue.revision(),
            request: player.request_id(),
            status: player.status.state,
            volume: if player.settings.muted { 0.0 } else { player.settings.volume },
            duration: player.duration(),
            buffering: player.is_buffering(),
            seek: player.seek_revision(),
            art: art.map(Handle::id),
            next: player.queue.next_index().is_some() || player.is_fm() && playing,
            previous: player.queue.previous_index().is_some(),
        };
        let now = Instant::now();
        if self.version.as_ref() == Some(&version) && (!playing || now.duration_since(self.at) < Duration::from_secs(1))
        {
            return;
        }
        if self.artwork.as_ref().map(|(id, _)| *id) != version.art {
            self.artwork = match art {
                Some(Handle::Rgba { id, width, height, pixels }) => {
                    Some((*id, Arc::new(Artwork { width: *width, height: *height, rgba: pixels.as_ref().to_vec() })))
                }
                _ => None,
            };
        }
        let snapshot = Snapshot {
            metadata: player.current().map(|track| Metadata::from_track(track, version.duration)),
            artwork: self.artwork.as_ref().map(|(_, art)| art.clone()),
            status: match player.status.state {
                PlayerState::Playing => Status::Playing,
                PlayerState::Paused => Status::Paused,
                _ => Status::Stopped,
            },
            volume: f64::from(version.volume),
            position: folia_media::microseconds(player.heard_position()),
            sampled_at: now,
            advancing: playing && !version.buffering,
            seek_revision: version.seek,
            can_next: version.next,
            can_previous: version.previous,
        };
        self.sender.send_replace(snapshot);
        self.version = Some(version);
        self.at = now;
    }
}

impl Shell {
    pub(crate) fn media_command(&mut self, command: Command) -> Task<Message> {
        if command == Command::Raise {
            return iced::window::oldest().then(|id| {
                id.map_or_else(Task::none, |id| iced::window::minimize(id, false).chain(iced::window::gain_focus(id)))
            });
        }
        self.handle(Message::Playback(crate::playback::Message::Media(command)))
    }
}
