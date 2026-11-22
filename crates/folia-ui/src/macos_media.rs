// Publishes what Folia plays to macOS Now Playing: Control Center, media keys and notch apps.
use std::sync::mpsc::Receiver;
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use folia_media::Command;
use folia_nowplaying::Info;
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

pub(crate) struct Bridge {
    last: Option<(i64, bool, i64, i64, u64, Option<u64>)>,
    at: Option<Instant>,
    artwork: Option<(u64, u32, u32, Vec<u8>)>,
}

impl Bridge {
    pub fn new() -> Self {
        install_once();
        Self { last: None, at: None, artwork: None }
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
                self.at = None;
                self.artwork = None;
            }
            return;
        };
        let playing = player.is_playing();
        let position = player.heard_position();
        let duration = player.duration();
        let art_id = art.map(|handle| art_key(&handle.id()));
        let key = (
            track.id,
            playing,
            (position.max(0.0) * 4.0) as i64,
            (duration.max(0.0) * 1000.0) as i64,
            player.seek_revision(),
            art_id,
        );
        let now = Instant::now();
        // Four updates a second keep the system timeline honest without spinning on every frame.
        if self.last == Some(key) {
            return;
        }
        self.last = Some(key);
        self.at = Some(now);
        if self.artwork.as_ref().map(|(id, ..)| *id) != art_id {
            self.artwork = match art {
                Some(Handle::Rgba { id, width, height, pixels }) => {
                    Some((art_key(id), *width, *height, pixels.as_ref().to_vec()))
                }
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
