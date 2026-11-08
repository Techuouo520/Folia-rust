// Playback controller: owns the player and the queue, follows track ends and loads lyrics.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use folia_core::{AudioFrame, AudioQuality, ChorusRange, Lyrics, Queue, Repeat, Track};
use folia_lyrics::{ChorusSource, chorus_ranges_of, resolve_chorus};
use folia_omni::{Collection, Omni};
use folia_player::{Player, PlayerState, Status};
use folia_store::{PlaybackSettings, Store};
use iced::Task;

use crate::background::{blocking, error_chain, save};

pub struct Playback {
    player: Option<Player>,
    omni: Option<Omni>,
    pub queue: Queue<Track>,
    pub lyrics: Option<Lyrics>,
    pub status: Status,
    pub settings: PlaybackSettings,
    /// Silenced without losing `settings.volume`, like the original `isMuted`.
    pub muted: bool,
    /// Seconds the playing song's lyrics are moved later (`lyricTimelineOffsetMs`); every song starts at 0.
    pub lyric_offset: f64,
    seeking: Option<f64>,
    /// Tracks that failed one after another; playback stops once the whole queue has failed.
    failures: usize,
    /// Personal FM's feed while the queue is FM (`isFmMode`): stepping on near its end fetches another batch.
    fm: Option<Collection>,
    /// A batch is on its way; steps wait for it.
    topping_up: bool,
    notice: Option<(String, Instant)>,
    notice_serial: u64,
    request: u64,
    /// `cachedNeteaseChorusRanges`: the chorus ranges kept for one song, from lyrics that came marked.
    kept_chorus: Option<(i64, Vec<ChorusRange>)>,
}

#[derive(Debug, Clone)]
pub enum AudioFailure {
    SubscriptionRequired,
    Other(String),
}

#[derive(Debug, Clone)]
pub enum Message {
    Toggle,
    Next,
    Previous,
    Seeking(f64),
    SeekDone,
    /// Sets the level; moving the volume also lifts a mute.
    Volume(f32),
    ToggleMute,
    /// Sets the lyric timeline offset, in seconds.
    LyricOffset(f64),
    VolumeDone,
    CycleRepeat,
    /// Sets the default quality for later loads; the current song keeps playing.
    Quality(AudioQuality),
    /// Plays the queue item at `index`.
    Jump(usize),
    /// Removes the queue item at `index`. Removing the current item starts whatever takes its place.
    Remove(usize),
    /// Moves the queue item at `index` to immediately after the current item.
    MoveNext(usize),
    /// Moves the queue item at `index` to the end of the queue.
    MoveEnd(usize),
    /// Moves the queue item at the first index so that it ends up at the second (a drag in the queue list).
    Reorder(usize, usize),
    Tick,
    LyricsLoaded(u64, i64, Option<Lyrics>, Option<Vec<ChorusRange>>),
    AudioReady(u64, i64, Result<PathBuf, AudioFailure>),
    Started(u64),
    NoticeLeaving(u64),
    NoticeGone(u64),
    NoticeFrame,
    /// A personal FM batch for the step away from the track with this id; `finished` as in `Queue::advance`.
    FmBatch {
        from: i64,
        finished: bool,
        batch: Result<Vec<Track>, String>,
    },
}

impl Playback {
    pub fn new(settings: PlaybackSettings, omni: Option<Omni>) -> Self {
        let player = match Player::new() {
            Ok(player) => {
                player.set_volume(settings.volume);
                Some(player)
            }
            Err(err) => {
                tracing::error!(error = &err as &dyn std::error::Error, "audio output unavailable");
                None
            }
        };
        let mut queue = Queue::default();
        queue.repeat = settings.repeat;
        let status = Status { state: PlayerState::Idle, position: 0.0, duration: 0.0 };
        Self {
            player,
            omni,
            queue,
            lyrics: None,
            status,
            settings,
            muted: false,
            lyric_offset: 0.0,
            seeking: None,
            failures: 0,
            fm: None,
            topping_up: false,
            notice: None,
            notice_serial: 0,
            request: 0,
            kept_chorus: None,
        }
    }

    pub fn request_id(&self) -> u64 {
        self.request
    }

    pub fn current(&self) -> Option<&Track> {
        self.queue.current()
    }

    pub fn is_playing(&self) -> bool {
        self.status.state == PlayerState::Playing
    }

    /// Spectrum for the frame about to be drawn. Stopped playback is the idle breath.
    pub fn audio_frame(&mut self, wall_ms: f64) -> AudioFrame {
        let playing = self.is_playing();
        match self.player.as_mut() {
            Some(player) => player.audio_frame(playing, wall_ms),
            None => AudioFrame::breath(wall_ms),
        }
    }

    /// Personal FM has no editable queue (`isFmMode`).
    pub fn is_fm(&self) -> bool {
        self.fm.is_some()
    }

    /// The position to show: the slider's value while it is being dragged.
    pub fn position(&self) -> f64 {
        self.seeking.unwrap_or(self.status.position)
    }

    /// The time the lyrics are drawn at: a positive offset holds them back (`audioTime - offset`).
    pub fn lyric_time(&self) -> f64 {
        self.position() - self.lyric_offset
    }

    pub fn duration(&self) -> f64 {
        if self.status.duration > 0.0 {
            self.status.duration
        } else {
            self.current().map_or(0.0, |track| track.duration)
        }
    }

    /// Replaces the queue with `tracks` and plays the one at `index`. `origin` is where they were started.
    pub fn play(&mut self, tracks: Vec<Track>, index: usize, origin: Option<folia_core::Origin>) -> Task<Message> {
        match origin {
            Some(origin) => self.queue.replace_from(tracks, index, origin),
            None => self.queue.replace(tracks, index),
        }
        self.set_fm(None);
        self.start_current()
    }

    /// Plays personal FM's first batch; stepping on near the end fetches more from `fm` (`fmNeedsTopUpAt`).
    pub fn play_fm(&mut self, tracks: Vec<Track>, fm: Collection, origin: folia_core::Origin) -> Task<Message> {
        self.queue.replace_from(tracks, 0, origin);
        self.set_fm(Some(fm));
        self.start_current()
    }

    /// Personal FM never goes round (`queueLoopMode`): the queue's repeat is off while it plays.
    fn set_fm(&mut self, fm: Option<Collection>) {
        self.fm = fm;
        self.topping_up = false;
        self.queue.repeat = if self.fm.is_some() { Repeat::Off } else { self.settings.repeat };
    }

    /// Adds `tracks` to the end of the queue (`enqueueAll`), keeping what plays; returns how many were added.
    pub fn enqueue(&mut self, tracks: Vec<Track>) -> usize {
        self.queue.append(tracks, |a, b| a.id == b.id)
    }

    /// `clearQueue`: every queued item except the playing one leaves; the player keeps running.
    pub fn clear_queue(&mut self) -> bool {
        let cleared = self.queue.clear_except_current();
        if cleared {
            self.failures = 0;
        }
        cleared
    }

    /// Empties the queue and stops, as signing out of the active platform does (`resetForProviderSwitch`).
    pub fn clear(&mut self) {
        self.request = self.request.wrapping_add(1);
        self.notice = None;
        self.notice_serial = self.notice_serial.wrapping_add(1);
        self.with_player(Player::stop);
        self.queue.replace(Vec::new(), 0);
        self.set_fm(None);
        self.lyrics = None;
        self.lyric_offset = 0.0;
        self.seeking = None;
        self.refresh();
    }

    pub fn update(&mut self, message: Message, store: &Store) -> Task<Message> {
        match message {
            Message::Toggle => {
                match self.status.state {
                    PlayerState::Playing => self.with_player(Player::pause),
                    PlayerState::Paused => self.with_player(Player::resume),
                    _ if self.current().is_some() => return self.start_current(),
                    _ => {}
                }
                self.refresh();
                Task::none()
            }
            Message::Next => self.skip(true),
            Message::Previous => self.skip(false),
            Message::Seeking(position) => {
                self.seeking = Some(position);
                Task::none()
            }
            Message::SeekDone => {
                if let (Some(position), Some(player)) = (self.seeking.take(), &self.player) {
                    player.seek(position);
                }
                Task::none()
            }
            Message::Volume(volume) => {
                self.settings.volume = volume;
                self.muted = false;
                self.with_player(|player| player.set_volume(volume));
                Task::none()
            }
            Message::LyricOffset(seconds) => {
                self.lyric_offset = seconds;
                Task::none()
            }
            Message::ToggleMute => {
                self.muted = !self.muted;
                let level = if self.muted { 0.0 } else { self.settings.volume };
                self.with_player(|player| player.set_volume(level));
                Task::none()
            }
            Message::VolumeDone => save(store, &self.settings),
            Message::CycleRepeat => {
                self.settings.repeat = match self.settings.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                };
                if self.fm.is_none() {
                    self.queue.repeat = self.settings.repeat;
                }
                save(store, &self.settings)
            }
            Message::Quality(quality) => {
                self.settings.audio_quality = quality;
                save(store, &self.settings)
            }
            Message::Jump(index) => {
                if self.queue.select(index).is_some() {
                    self.start_current()
                } else {
                    Task::none()
                }
            }
            Message::Remove(index) => {
                let current = self.queue.current_index() == Some(index);
                if self.queue.remove(index).is_none() {
                    return Task::none();
                }
                if current { self.start_current() } else { Task::none() }
            }
            Message::MoveNext(index) => {
                self.queue.move_after_current(index);
                Task::none()
            }
            Message::MoveEnd(index) => {
                self.queue.move_to_end(index);
                Task::none()
            }
            Message::Reorder(from, to) => {
                self.queue.move_to(from, to);
                Task::none()
            }
            Message::Tick => self.tick(),
            Message::LyricsLoaded(request, id, lyrics, kept) => {
                if request == self.request && self.current().is_some_and(|track| track.id == id) {
                    self.lyrics = lyrics;
                    self.kept_chorus = kept.map(|ranges| (id, ranges));
                }
                Task::none()
            }
            Message::NoticeLeaving(serial) if serial == self.notice_serial => {
                Task::perform(blocking(|| std::thread::sleep(Duration::from_millis(220))), move |_| {
                    Message::NoticeGone(serial)
                })
            }
            Message::NoticeGone(serial) if serial == self.notice_serial => {
                self.notice = None;
                Task::none()
            }
            Message::NoticeLeaving(_) | Message::NoticeGone(_) | Message::NoticeFrame | Message::Started(_) => {
                Task::none()
            }
            Message::AudioReady(request, id, result) => {
                if request != self.request {
                    return Task::none();
                }
                let Some(track) = self.queue.current_mut().filter(|track| track.id == id) else { return Task::none() };
                match result {
                    Ok(path) => {
                        self.failures = 0;
                        track.path = path;
                        let path = track.path.clone();
                        self.with_player(|player| player.load(&path));
                        self.refresh();
                        Task::done(Message::Started(request))
                    }
                    Err(AudioFailure::SubscriptionRequired) => {
                        let provider = track.online.as_ref().map(|online| online.provider.clone());
                        self.with_player(Player::stop);
                        self.refresh();
                        self.notice_serial = self.notice_serial.wrapping_add(1);
                        let serial = self.notice_serial;
                        self.notice = provider.map(|provider| (provider, Instant::now()));
                        tracing::info!("online playback stopped: subscription required");
                        Task::perform(blocking(|| std::thread::sleep(Duration::from_secs(6))), move |_| {
                            Message::NoticeLeaving(serial)
                        })
                    }
                    Err(AudioFailure::Other(err)) => {
                        tracing::warn!(track = %track.display_name(), error = %err, "online track unavailable");
                        self.fail()
                    }
                }
            }
            Message::FmBatch { from, finished, batch } => {
                // A queue started meanwhile, or a song chosen meanwhile, overtakes the batch.
                if !self.topping_up || self.current().is_none_or(|track| track.id != from) {
                    return Task::none();
                }
                self.topping_up = false;
                match batch {
                    // `freshFmSongs`: only songs not in the queue yet.
                    Ok(batch) => {
                        self.queue.append(batch, |a, b| a.id == b.id);
                    }
                    Err(err) => tracing::warn!(error = %err, "personal FM batch unavailable"),
                }
                self.step(finished)
            }
        }
    }

    fn tick(&mut self) -> Task<Message> {
        self.refresh();
        match self.status.state {
            PlayerState::Ended => self.advance(true),
            PlayerState::Failed => self.fail(),
            _ => Task::none(),
        }
    }

    fn skip(&mut self, forward: bool) -> Task<Message> {
        if forward {
            return self.advance(false);
        }
        if self.queue.back().is_some() { self.start_current() } else { Task::none() }
    }

    fn fail(&mut self) -> Task<Message> {
        self.failures += 1;
        if self.failures >= self.queue.items().len() {
            self.failures = 0;
            self.with_player(Player::stop);
            self.refresh();
            return Task::none();
        }
        self.advance(false)
    }

    fn advance(&mut self, finished: bool) -> Task<Message> {
        if self.topping_up {
            return Task::none();
        }
        let near_end = self.queue.current_index().is_some_and(|index| index + 2 >= self.queue.items().len());
        if let (true, Some(fm), Some(omni), Some(from)) =
            (near_end, self.fm.clone(), self.omni.clone(), self.current().map(|track| track.id))
        {
            self.topping_up = true;
            return Task::perform(async move { omni.collection_detail(&fm).await }, move |result| Message::FmBatch {
                from,
                finished,
                batch: result.map(|detail| detail.tracks).map_err(|err| error_chain(&err)),
            });
        }
        self.step(finished)
    }

    fn step(&mut self, finished: bool) -> Task<Message> {
        if self.queue.advance(finished).is_some() {
            return self.start_current();
        }
        self.with_player(Player::stop);
        self.refresh();
        Task::none()
    }

    /// Starts the current track: local files play at once, online tracks are downloaded first.
    fn start_current(&mut self) -> Task<Message> {
        let Some(track) = self.queue.current().cloned() else { return Task::none() };
        self.request = self.request.wrapping_add(1);
        let request = self.request;
        // A track started any other way overtakes a personal FM batch on its way.
        self.topping_up = false;
        self.notice = None;
        self.notice_serial = self.notice_serial.wrapping_add(1);
        self.lyrics = None;
        self.lyric_offset = 0.0;
        self.seeking = None;
        tracing::info!(track = %track.display_name(), "playing");
        let id = track.id;
        let kept = self.kept_chorus.as_ref().filter(|(song, _)| *song == id).map(|(_, ranges)| ranges.clone());
        let (Some(online), Some(omni)) = (track.online.clone(), self.omni.clone()) else {
            self.with_player(|player| player.load(&track.path));
            self.refresh();
            let lyrics = Task::perform(
                blocking(move || settle_chorus(folia_library::lyrics_for(&track.path), kept)),
                move |settled| {
                    let (lyrics, kept) = settled.unwrap_or_default();
                    Message::LyricsLoaded(request, id, lyrics, kept)
                },
            );
            return Task::batch([lyrics, Task::done(Message::Started(request))]);
        };

        self.with_player(Player::stop);
        self.refresh();
        let (lyrics_omni, lyrics_id) = (omni.clone(), online.clone());
        let lyrics = Task::perform(
            async move {
                let lyrics = lyrics_omni.lyrics(&lyrics_id).await.unwrap_or_else(|err| {
                    tracing::warn!(error = &err as &dyn std::error::Error, "online lyrics unavailable");
                    None
                });
                settle_chorus(lyrics, kept)
            },
            move |(lyrics, kept)| Message::LyricsLoaded(request, id, lyrics, kept),
        );
        let quality = self.settings.audio_quality;
        let audio = Task::perform(async move { omni.fetch_audio(&online, quality).await }, move |result| {
            Message::AudioReady(
                request,
                id,
                result.map_err(|err| match err {
                    folia_omni::OmniError::SubscriptionRequired => AudioFailure::SubscriptionRequired,
                    err => AudioFailure::Other(error_chain(&err)),
                }),
            )
        });
        Task::batch([audio, lyrics])
    }

    pub fn notice_animating(&self, now: Instant) -> bool {
        self.notice.as_ref().is_some_and(|(_, started)| {
            let ms = now.saturating_duration_since(*started).as_millis();
            ms < 320 || (6000..6220).contains(&ms)
        })
    }

    pub fn subscription_notice(&self, now: Instant) -> Option<(&str, f32, f32)> {
        let (provider, started) = self.notice.as_ref()?;
        let ms = now.saturating_duration_since(*started).as_secs_f64() * 1000.0;
        let entered = folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0).at((ms / 320.0).clamp(0.0, 1.0)) as f32;
        let left = folia_visualizer::Ease::new(0.4, 0.0, 1.0, 1.0).at(((ms - 6000.0) / 220.0).clamp(0.0, 1.0)) as f32;
        Some((provider, entered * (1.0 - left), -18.0 * (1.0 - entered) - 10.0 * left))
    }

    fn refresh(&mut self) {
        if let Some(player) = &self.player {
            self.status = player.status();
        }
    }

    fn with_player(&self, action: impl FnOnce(&Player)) {
        if let Some(player) = &self.player {
            action(player);
        }
    }
}

/// `createLyricsSetter`'s chorus step for every song's lyrics: marked lyrics become the song's kept ranges;
/// otherwise the kept ranges mark them, otherwise the most repeated lines do.
fn settle_chorus(
    mut lyrics: Option<Lyrics>,
    kept: Option<Vec<ChorusRange>>,
) -> (Option<Lyrics>, Option<Vec<ChorusRange>>) {
    let Some(found) = lyrics.as_mut() else { return (lyrics, kept) };
    let mode = resolve_chorus(Some(found), kept.as_deref().unwrap_or_default(), None);
    tracing::info!(?mode, lines = found.lines.iter().filter(|line| line.is_chorus).count(), "song chorus settled");
    let kept = if mode == ChorusSource::Existing { Some(chorus_ranges_of(found)) } else { kept };
    (lyrics, kept)
}
