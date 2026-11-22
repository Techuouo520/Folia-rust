// Publishes what Folia plays to macOS Now Playing: Control Center, media keys and notch apps.
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, Once};

use folia_media::Command;
use folia_nowplaying::{Artwork, Info};
use iced::advanced::image::Handle;
use iced::futures::SinkExt;
use iced::{Subscription, Task};

use crate::{Message, Playback, Shell};

static INSTALL: Once = Once::new();
static RECEIVER: Mutex<Option<Receiver<folia_nowplaying::Command>>> = Mutex::new(None);

fn install_once() {
    INSTALL.call_once(|| {
        let (sender, receiver) = std::sync::mpsc::channel();
        if let Ok(mut slot) = RECEIVER.lock() {
            *slot = Some(receiver);
        }
        folia_nowplaying::install_sink(move |command| {
            let _ = sender.send(command);
        });
    });
}

fn translate(command: folia_nowplaying::Command) -> Command {
    match command {
        folia_nowplaying::Command::Play => Command::Play,
        folia_nowplaying::Command::Pause => Command::Pause,
        folia_nowplaying::Command::TogglePlayPause => Command::PlayPause,
        folia_nowplaying::Command::Next => Command::Next,
        folia_nowplaying::Command::Previous => Command::Previous,
    }
}

fn art_key(id: &iced::advanced::image::Id) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

#[derive(PartialEq, Eq)]
struct Key {
    track: i64,
    playing: bool,
    position_quarter_secs: i64,
    duration_ms: i64,
    seek: u64,
    art: Option<u64>,
}

pub(crate) struct Bridge {
    last: Option<Key>,
    artwork: Option<Arc<Artwork>>,
}

impl Bridge {
    pub fn new() -> Self {
        install_once();
        Self { last: None, artwork: None }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run_with(0u8, |_| {
            iced::stream::channel(16, async move |mut output| {
                let receiver = RECEIVER.lock().ok().and_then(|mut slot| slot.take());
                let Some(receiver) = receiver else { return };
                let (sender, mut inbound) = tokio::sync::mpsc::channel(32);
                std::thread::spawn(move || {
                    for command in receiver {
                        if sender.blocking_send(command).is_err() {
                            break;
                        }
                    }
                });
                while let Some(command) = inbound.recv().await {
                    if output.send(Message::Media(translate(command))).await.is_err() {
                        break;
                    }
                }
            })
        })
    }

    pub fn sync(&mut self, player: &Playback, art: Option<&Handle>) {
        let Some(track) = player.current() else {
            if self.last.is_some() {
                folia_nowplaying::clear();
                self.last = None;
                self.artwork = None;
            }
            return;
        };
        let playing = player.is_playing();
        let position = player.heard_position();
        let duration = player.duration();
        let art_id = art.map(|handle| art_key(&handle.id()));
        let key = Key {
            track: track.id,
            playing,
            position_quarter_secs: (position.max(0.0) * 4.0) as i64,
            duration_ms: (duration.max(0.0) * 1000.0) as i64,
            seek: player.seek_revision(),
            art: art_id,
        };
        // Four updates a second keep the system timeline honest without spinning on every frame.
        if self.last.as_ref() == Some(&key) {
            return;
        }
        self.last = Some(key);
        if self.artwork.as_ref().map(|art| art.id) != art_id {
            self.artwork = match art {
                Some(Handle::Rgba { id, width, height, pixels }) => Some(Arc::new(Artwork {
                    id: art_key(id),
                    width: *width,
                    height: *height,
                    rgba: pixels.as_ref().to_vec(),
                })),
                _ => None,
            };
        }
        let artist = track
            .online_info
            .as_ref()
            .map(|info| {
                info.artists
                    .iter()
                    .filter(|artist| !artist.name.is_empty())
                    .map(|artist| artist.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|names| !names.is_empty())
            .unwrap_or_else(|| track.artist.clone().unwrap_or_default());
        folia_nowplaying::publish(&Info {
            title: track.title.clone(),
            artist,
            album: track.album.clone().unwrap_or_default(),
            duration,
            position,
            playing,
            artwork: self.artwork.clone(),
        });
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
