// Playback controller: owns the player and the queue, follows track ends and loads lyrics.

use std::time::{Duration, Instant};

use folia_core::{AudioFrame, AudioInput, AudioQuality, ChorusRange, Lyrics, Queue, Repeat, ReplayGain, Track};
use folia_lyrics::{ChorusSource, chorus_ranges_of, resolve_chorus};
use folia_omni::{Collection, CollectionKind, Omni};
use folia_player::{ListenTracker, Player, PlayerState, Status};
use folia_store::{FmSession, PlaybackSession, PlaybackSettings, ReplayGainMode, Store};
use iced::Task;

use crate::background::{blocking, error_chain, save};

pub struct Playback {
    listen: ListenTracker,
    listened_track: Option<(Track, AudioQuality)>,
    report_task: Option<iced::task::Handle>,
    report_serial: u64,
    player: Option<Player>,
    omni: Option<Omni>,
    pub queue: Queue<Track>,
    pub lyrics: Option<Lyrics>,
    lyrics_pending: bool,
    pub status: Status,
    pub settings: PlaybackSettings,
    /// Silenced without losing `settings.volume`, like the original `isMuted`.
    queue_ended: bool,
    /// Seconds the playing song's lyrics are moved later (`lyricTimelineOffsetMs`); every song starts at 0.
    pub lyric_offset: f64,
    seeking: Option<f64>,
    restored: Option<(i64, f64)>,
    start_position: f64,
    seek_revision: u64,
    /// Tracks that failed one after another; playback stops once the whole queue has failed.
    failures: usize,
    /// Keep the requested direction while skipping unplayable candidates.
    failure_forward: bool,
    /// Personal FM's feed while the queue is FM (`isFmMode`): stepping on near its end fetches another batch.
    fm: Option<Collection>,
    /// A batch is on its way; steps wait for it.
    topping_up: bool,
    notice: Option<(Notice, Instant)>,
    notice_serial: u64,
    request: u64,
    pub loading: Loading,
    loading_at: Option<Instant>,
    loading_toast: bool,
    audio_task: Option<iced::task::Handle>,
    prefetch_task: Option<iced::task::Handle>,
    prefetch_key: u64,
    prefetch_dirty: bool,
    source_revision: u64,
    active_input: Option<AudioInput>,
    failed_inputs: Vec<std::path::PathBuf>,
    /// `cachedNeteaseChorusRanges`: the chorus ranges kept for one song, from lyrics that came marked.
    kept_chorus: Option<(i64, Vec<ChorusRange>)>,
    /// `replayGainMode` and the gains of the playing song.
    pub replay_gain: ReplayGainMode,
    gain: Option<ReplayGain>,
    applied_db: f32,
}

#[derive(Debug, Clone)]
pub enum AudioFailure {
    SubscriptionRequired,
    CustomSourceUnavailable,
    Other(String),
}

enum Notice {
    Subscription(String),
    CustomSourceUnavailable,
}

#[derive(Debug, Clone)]
pub enum Message {
    Scrobble(bool),
    Reported(u64),
    Media(folia_media::Command),
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
    AudioReady(u64, i64, Result<AudioInput, AudioFailure>),
    Prefetched,
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
                player.set_volume(if settings.muted { 0.0 } else { settings.volume });
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
            listen: ListenTracker::default(),
            listened_track: None,
            report_task: None,
            report_serial: 0,
            player,
            omni,
            queue,
            lyrics: None,
            lyrics_pending: false,
            status,
            settings,

            queue_ended: false,
            lyric_offset: 0.0,
            seeking: None,
            restored: None,
            start_position: 0.0,
            seek_revision: 0,
            failures: 0,
            failure_forward: true,
            fm: None,
            topping_up: false,
            notice: None,
            notice_serial: 0,
            request: 0,
            loading: Loading::default(),
            loading_at: None,
            loading_toast: false,
            audio_task: None,
            prefetch_task: None,
            prefetch_key: 0,
            prefetch_dirty: true,
            source_revision: 0,
            active_input: None,
            failed_inputs: Vec::new(),
            kept_chorus: None,
            replay_gain: ReplayGainMode::Off,
            gain: None,
            applied_db: 0.0,
        }
    }

    pub fn request_id(&self) -> u64 {
        self.request
    }

    pub fn seek_revision(&self) -> u64 {
        self.seek_revision
    }

    pub fn current(&self) -> Option<&Track> {
        self.queue.current()
    }

    pub fn snapshot(&self) -> PlaybackSession {
        let mut queue = self.queue.snapshot();
        let current = queue.current.and_then(|index| queue.items.get_mut(index)).or(queue.detached.as_mut());
        if let Some(track) = current {
            track.duration = self.duration();
        }
        PlaybackSession {
            queue,
            position: self.heard_position(),
            fm: self.fm.as_ref().map(|fm| FmSession {
                provider: fm.provider.clone(),
                id: fm.id.clone(),
                name: fm.name.clone(),
            }),
        }
        .normalized()
    }

    /// Restores transport metadata and lyrics without opening audio or prefetching the queue.
    pub fn restore(&mut self, session: PlaybackSession) -> Task<Message> {
        let session = session.normalized();
        self.clear();
        self.prefetch_task = None;
        self.queue.restore(session.queue);
        self.set_fm(session.fm.map(|fm| Collection {
            provider: fm.provider,
            id: fm.id,
            name: fm.name,
            kind: CollectionKind::PersonalFm,
            cover_url: None,
            track_count: None,
            creator: None,
            description: None,
            owned: false,
        }));
        tracing::info!(
            items = self.queue.items().len(),
            position = session.position,
            "playback session restored paused"
        );
        let Some(track) = self.current().cloned() else { return Task::none() };
        self.restored = Some((track.id, session.position));
        self.status = Status { state: PlayerState::Paused, position: session.position, duration: track.duration };
        self.prefetch_dirty = false;
        self.lyrics_pending = true;
        self.read_lyrics(&track, self.request)
    }

    /// The exact representation currently heard, shared with cancellable offline analysis readers.
    pub fn analysis_input(&self) -> Option<AudioInput> {
        if self.restored.is_some() {
            return None;
        }
        if !matches!(self.status.state, PlayerState::Playing | PlayerState::Paused) {
            return None;
        }
        self.active_input
            .clone()
            .or_else(|| self.current().filter(|track| track.online.is_none()).map(|track| track.path.clone().into()))
    }

    pub fn is_playing(&self) -> bool {
        self.status.state == PlayerState::Playing
    }

    /// A natural end of the last queue item, until playback starts again.
    pub fn queue_ended(&self) -> bool {
        self.queue_ended
    }

    pub fn is_buffering(&self) -> bool {
        self.loading.active() || self.player.as_ref().is_some_and(Player::is_buffering)
    }

    /// Hands the equalizer settings to the audio callback.
    pub fn set_equalizer(&mut self, parameters: folia_player::Parameters) {
        if let Some(player) = self.player.as_mut() {
            player.set_equalizer(parameters);
        }
    }

    pub fn room_sample_rate(&self) -> Option<u32> {
        self.player.as_ref().map(Player::sample_rate)
    }

    pub fn set_room(
        &mut self,
        convolver: Option<Box<folia_player::RoomConvolver>>,
    ) -> Result<(), folia_player::RoomError> {
        self.player.as_mut().map_or(Ok(()), |player| player.set_room(convolver))
    }

    pub fn finish_room_change(&mut self) -> bool {
        self.player.as_mut().is_none_or(Player::finish_room_change)
    }

    /// `replayGainMode` and the gains of the song that is playing.
    pub fn set_replay_gain(&mut self, mode: ReplayGainMode, gain: Option<ReplayGain>) {
        self.replay_gain = mode;
        self.gain = gain;
        self.apply_gain();
    }

    pub fn applied_gain_db(&self) -> f32 {
        self.applied_db
    }

    /// Replaces the shown lyrics, for a match or an imported file.
    pub fn set_lyrics(&mut self, lyrics: Option<Lyrics>) {
        self.lyrics = lyrics;
        self.lyrics_pending = false;
    }

    pub fn lyrics_loading(&self) -> bool {
        self.lyrics_pending
    }

    /// The gain the mode applies to the playing song's tags.
    fn gain_factor(&self) -> f32 {
        let Some(gain) = self.gain else { return 1.0 };
        let db = match self.replay_gain {
            ReplayGainMode::Off => return 1.0,
            ReplayGainMode::Auto | ReplayGainMode::Track => gain.gain_db(false),
            ReplayGainMode::Album => gain.gain_db(true),
        };
        let Some(db) = db else { return 1.0 };
        10f32.powf(db / 20.0)
    }

    fn apply_gain(&mut self) {
        self.applied_db = 20.0 * self.gain_factor().max(1e-6).log10();
        if self.replay_gain == ReplayGainMode::Off || self.gain.is_none() {
            self.applied_db = 0.0;
        }
        let level = if self.settings.muted { 0.0 } else { self.settings.volume * self.gain_factor() };
        self.with_player(|player| player.set_volume(level.clamp(0.0, 1.0)));
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

    pub fn heard_position(&self) -> f64 {
        if self.restored.is_some() {
            return self.position();
        }
        self.seeking.unwrap_or_else(|| self.player.as_ref().map_or(self.status.position, Player::heard_position))
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
        self.prefetch_dirty = true;
        self.queue.append(tracks, |a, b| a.id == b.id)
    }

    /// Only a local song and an entirely local queue can be saved to the library index.
    pub fn can_save_queue(&self) -> bool {
        let indexed = |track: &Track| track.online.is_none() && track.id > 0;
        !self.is_fm()
            && self.current().is_some_and(indexed)
            && !self.queue.items().is_empty()
            && self.queue.items().iter().all(indexed)
    }

    /// Shuffles the queue (`shuffleQueue`): the playing song to the head, the rest drawn from `seed`.
    pub fn shuffle(&mut self, seed: u64) -> bool {
        self.prefetch_dirty = true;
        if self.fm.is_some() {
            return false;
        }
        self.queue.shuffle(seed)
    }

    /// Clears the editable list without stopping the active audio request.
    pub fn clear_queue(&mut self) -> bool {
        self.prefetch_dirty = true;
        let cleared = !self.is_fm() && self.queue.clear_list();
        if cleared {
            self.failures = 0;
        }
        cleared
    }

    /// Empties the queue and stops, as signing out of the active platform does (`resetForProviderSwitch`).
    pub fn clear(&mut self) {
        self.listen = ListenTracker::default();
        self.listened_track = None;
        self.report_task = None;
        self.restored = None;
        self.start_position = 0.0;
        self.lyrics_pending = false;
        self.queue_ended = false;
        self.active_input = None;
        self.failed_inputs.clear();
        self.audio_task = None;
        self.end_loading();
        self.loading_toast = false;
        self.prefetch_dirty = true;
        self.request = self.request.wrapping_add(1);
        self.notice = None;
        self.notice_serial = self.notice_serial.wrapping_add(1);
        self.with_player(Player::stop);
        self.queue.replace(Vec::new(), 0);
        self.set_fm(None);
        self.lyrics = None;
        self.lyric_offset = 0.0;
        self.seeking = None;
        self.gain = None;
        self.apply_gain();
        self.refresh();
    }

    /// Applies an audio-source policy change without accepting requests started under the previous policy.
    pub fn source_mode_changed(&mut self) -> Task<Message> {
        self.prefetch_task = None;
        self.prefetch_dirty = true;
        self.failed_inputs.clear();
        if self.restored.is_some() {
            if self.loading.active() {
                self.audio_task = None;
                self.end_loading();
                self.loading_toast = false;
                self.request = self.request.wrapping_add(1);
                self.lyrics_pending = false;
            }
            self.notice = None;
            self.notice_serial = self.notice_serial.wrapping_add(1);
            return Task::none();
        }
        let stop = if self.current().is_some_and(|track| track.online.is_some()) {
            self.active_input = None;
            self.lyrics_pending = false;
            self.topping_up = false;
            self.seeking = None;
            self.failures = 0;
            self.notice = None;
            self.notice_serial = self.notice_serial.wrapping_add(1);
            self.stop_current()
        } else {
            Task::none()
        };
        Task::batch([stop, self.refresh_prefetch()])
    }

    pub fn update(&mut self, message: Message, store: &Store) -> Task<Message> {
        if matches!(
            &message,
            Message::CycleRepeat
                | Message::Quality(_)
                | Message::Remove(_)
                | Message::MoveNext(_)
                | Message::MoveEnd(_)
                | Message::Reorder(_, _)
        ) {
            self.prefetch_dirty = true;
        }
        match message {
            Message::Scrobble(enabled) => {
                if enabled && !self.omni.as_ref().is_some_and(|omni| omni.playback_reporting_ready("netease")) {
                    return Task::none();
                }
                self.settings.netease_scrobble_enabled = enabled;
                self.listen = ListenTracker::default();
                self.listened_track = None;
                if !enabled {
                    self.report_task = None;
                }
                save(store, &self.settings)
            }
            Message::Reported(serial) => {
                if serial == self.report_serial {
                    self.report_task = None;
                }
                Task::none()
            }
            Message::Media(command) => self.media_command(command, store),
            Message::Toggle => {
                self.listen.discontinuity();
                if self.loading.active() {
                    return Task::none();
                }
                if let Some((id, position)) = self.restored.take() {
                    self.failures = 0;
                    self.failure_forward = true;
                    self.failed_inputs.clear();
                    let position = if self.current().is_some_and(|track| track.id == id)
                        && (self.duration() <= 0.0 || position < self.duration())
                    {
                        position
                    } else {
                        0.0
                    };
                    return self.load_current_at(position);
                }
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
                if self.loading.active() {
                    return Task::none();
                }
                self.seeking = Some(position);
                Task::none()
            }
            Message::SeekDone => {
                self.listen.discontinuity();
                self.seek_revision = self.seek_revision.wrapping_add(1);
                if let Some((_, restored)) = self.restored.as_mut() {
                    if let Some(position) = self.seeking.take() {
                        *restored = if position.is_finite() { position.max(0.0) } else { 0.0 };
                        self.status.position = *restored;
                    }
                    return Task::none();
                }
                if let (Some(position), Some(player)) = (self.seeking.take(), &self.player) {
                    player.seek(position);
                }
                self.refresh();
                Task::none()
            }
            Message::Volume(volume) => {
                self.settings.volume = volume;
                self.settings.muted = false;
                self.apply_gain();
                Task::none()
            }
            Message::LyricOffset(seconds) => {
                self.lyric_offset = seconds;
                Task::none()
            }
            Message::ToggleMute => {
                self.settings.muted = !self.settings.muted;
                self.apply_gain();
                save(store, &self.settings)
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
                    self.lyrics_pending = false;
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
            Message::NoticeLeaving(_)
            | Message::NoticeGone(_)
            | Message::NoticeFrame
            | Message::Started(_)
            | Message::Prefetched => Task::none(),
            Message::AudioReady(request, id, result) => {
                if request != self.request {
                    return Task::none();
                }
                let Some(track) = self.queue.current_mut().filter(|track| track.id == id) else { return Task::none() };
                match result {
                    Ok(input) => {
                        self.restored = None;
                        self.active_input = Some(input.clone());
                        if input.download.is_none() {
                            self.loading_toast = false;
                        }
                        track.path = input.path.clone();
                        self.loading.decoding(request);
                        self.with_player(|player| player.load_input_at(input.clone(), self.start_position));
                        self.refresh();
                        Task::done(Message::Started(request))
                    }
                    Err(AudioFailure::SubscriptionRequired) => {
                        let provider = track.online.as_ref().map(|online| online.provider.clone());
                        self.with_player(Player::stop);
                        self.refresh();
                        self.end_loading();
                        self.loading_toast = false;
                        tracing::info!("online playback stopped: subscription required");
                        provider.map_or_else(Task::none, |provider| self.show_notice(Notice::Subscription(provider)))
                    }
                    Err(AudioFailure::CustomSourceUnavailable) => {
                        self.active_input = None;
                        self.lyrics_pending = false;
                        self.topping_up = false;
                        tracing::info!("online playback stopped: custom sources unavailable");
                        let stopped = self.stop_current();
                        Task::batch([stopped, self.show_notice(Notice::CustomSourceUnavailable)])
                    }
                    Err(AudioFailure::Other(err)) => {
                        tracing::warn!(track = %track.display_name(), error = %err, "online track unavailable");
                        self.end_loading();
                        self.loading_toast = false;
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

    fn media_command(&mut self, command: folia_media::Command, store: &Store) -> Task<Message> {
        use folia_media::Command;
        match command {
            Command::Raise => Task::none(),
            Command::Play if !self.is_playing() => self.update(Message::Toggle, store),
            Command::Pause if self.loading.active() => {
                let position = self.heard_position();
                let task = self.media_stop();
                if let Some(track) = self.current() {
                    self.restored = Some((track.id, position));
                    self.status = Status { state: PlayerState::Paused, position, duration: self.duration() };
                }
                task
            }
            Command::Pause if self.is_playing() => self.update(Message::Toggle, store),
            Command::Play | Command::Pause => Task::none(),
            Command::PlayPause => {
                let command = if self.is_playing() || self.loading.active() { Command::Pause } else { Command::Play };
                self.media_command(command, store)
            }
            Command::Stop => self.media_stop(),
            Command::Next | Command::Previous => {
                let forward = command == Command::Next;
                if self.is_playing() || self.loading.active() {
                    return self.skip(forward);
                }
                let state =
                    if self.status.state == PlayerState::Paused { PlayerState::Paused } else { PlayerState::Idle };
                let selected = if forward { self.queue.advance(false) } else { self.queue.back() };
                let Some(track) = selected.cloned() else { return Task::none() };
                let stop = self.media_stop();
                self.restored = Some((track.id, 0.0));
                self.status = Status { state, position: 0.0, duration: track.duration };
                self.lyrics = None;
                self.lyric_offset = 0.0;
                self.lyrics_pending = true;
                Task::batch([stop, self.read_lyrics(&track, self.request)])
            }
            Command::Volume(value) if value.is_finite() => {
                let change = self.update(Message::Volume(value.clamp(0.0, 1.0) as f32), store);
                Task::batch([change, self.update(Message::VolumeDone, store)])
            }
            Command::Volume(_) => Task::none(),
        }
    }

    fn media_stop(&mut self) -> Task<Message> {
        let duration = self.duration();
        self.restored = None;
        self.start_position = 0.0;
        self.seeking = None;
        self.topping_up = false;
        self.lyrics_pending = false;
        self.active_input = None;
        self.prefetch_task = None;
        let task = self.stop_current();
        self.status = Status { state: PlayerState::Idle, position: 0.0, duration };
        task
    }

    fn tick(&mut self) -> Task<Message> {
        self.refresh();
        self.observe_listen();
        if self.status.state == PlayerState::Playing && !self.is_buffering() {
            self.start_position = 0.0;
        }
        if self.status.position > 0.0 {
            self.failures = 0;
        }
        if self.status.state == PlayerState::Failed
            && let Some(input) = self.active_input.take()
            && input.download.as_ref().is_some_and(|download| download.progress().2.is_some())
        {
            self.failed_inputs.push(input.path);
            tracing::warn!(request = self.request, "audio stream failed; continuing source fallback");
            return self.load_current_at(self.start_position);
        }
        if self.loading.phase == LoadPhase::Decoding
            && ((self.status.position > 0.0 && !self.player.as_ref().is_some_and(Player::is_buffering))
                || self.status.state == PlayerState::Failed
                || self.player.is_none())
        {
            if self.status.position > 0.0
                && let Some(at) = self.loading_at
            {
                tracing::debug!(
                    request = self.request,
                    elapsed_ms = at.elapsed().as_secs_f64() * 1000.0,
                    "playback output started"
                );
            }
            self.end_loading();
        }
        match self.status.state {
            PlayerState::Ended => self.advance(true),
            PlayerState::Failed => self.fail(),
            _ => Task::none(),
        }
    }

    fn skip(&mut self, forward: bool) -> Task<Message> {
        self.failures = 0;
        if forward {
            return self.advance(false);
        }
        if self.queue.back().is_some() { self.start_current_in_direction(false) } else { Task::none() }
    }

    fn fail(&mut self) -> Task<Message> {
        self.failures += 1;
        if self.failures >= self.queue.items().len() {
            self.failures = 0;
            return self.stop_current();
        }
        if self.failure_forward {
            return self.advance(false);
        }
        if self.queue.back().is_some() { self.start_current_in_direction(false) } else { self.stop_current() }
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
            return self.start_current_in_direction(true);
        }
        self.queue_ended |= finished;
        self.stop_current()
    }

    fn stop_current(&mut self) -> Task<Message> {
        let report = self.settle_listen();
        self.audio_task = None;
        self.end_loading();
        self.loading_toast = false;
        self.request = self.request.wrapping_add(1);
        self.with_player(Player::stop);
        self.refresh();
        report
    }

    /// An explicit selection begins a new forward failure traversal.
    fn start_current(&mut self) -> Task<Message> {
        self.failures = 0;
        self.start_current_in_direction(true)
    }

    /// Advances within a traversal while retaining its consecutive failure budget.
    fn start_current_in_direction(&mut self, forward: bool) -> Task<Message> {
        let report = self.settle_listen();
        self.failure_forward = forward;
        self.failed_inputs.clear();
        Task::batch([report, self.load_current_at(0.0)])
    }

    fn observe_listen(&mut self) {
        if !self.settings.netease_scrobble_enabled {
            return;
        }
        let playing = self.is_playing() && !self.is_buffering() && self.restored.is_none();
        if self.listened_track.is_none()
            && playing
            && let (Some(track), Some(omni)) = (self.current(), &self.omni)
            && omni.can_report_playback(track)
            && omni.playback_reporting_ready("netease")
        {
            self.listened_track = Some((track.clone(), self.settings.audio_quality));
        }
        if let Some((track, _)) = self.listened_track.as_mut() {
            if self.status.duration > 0.0 {
                track.duration = self.status.duration;
            }
            self.listen.observe(self.status.position, Instant::now(), playing);
        }
    }

    fn settle_listen(&mut self) -> Task<Message> {
        let Some((track, quality)) = self.listened_track.take() else { return Task::none() };
        let seconds = self.listen.settle(track.duration);
        let (Some(played_seconds), Some(omni)) = (seconds, self.omni.clone()) else { return Task::none() };
        if !self.settings.netease_scrobble_enabled || self.report_task.is_some() {
            return Task::none();
        }
        let report = folia_omni::PlaybackReport { played_seconds, total_seconds: track.duration, quality };
        self.report_serial = self.report_serial.wrapping_add(1);
        let serial = self.report_serial;
        let (task, handle) = Task::perform(
            async move {
                if let Err(error) = omni.report_playback(&track, report).await {
                    tracing::warn!(error = &error as &dyn std::error::Error, "listening report failed");
                }
            },
            move |()| Message::Reported(serial),
        )
        .abortable();
        self.report_task = Some(handle.abort_on_drop());
        task
    }

    fn load_current_at(&mut self, position: f64) -> Task<Message> {
        self.restored = None;
        self.start_position = if position.is_finite() { position.max(0.0) } else { 0.0 };
        self.lyrics_pending = false;
        self.queue_ended = false;
        self.active_input = None;
        self.prefetch_dirty = true;
        self.audio_task = None;
        self.end_loading();
        self.loading_toast = false;
        let Some(track) = self.queue.current().cloned() else {
            self.request = self.request.wrapping_add(1);
            self.with_player(Player::stop);
            self.lyrics = None;
            self.refresh();
            return Task::none();
        };
        self.request = self.request.wrapping_add(1);
        let request = self.request;
        self.lyrics_pending = true;
        // A track started any other way overtakes a personal FM batch on its way.
        self.topping_up = false;
        self.notice = None;
        self.notice_serial = self.notice_serial.wrapping_add(1);
        self.lyrics = None;
        self.lyric_offset = 0.0;
        self.seeking = None;
        self.gain = track.replay_gain;
        self.apply_gain();
        tracing::info!(track = %track.display_name(), "playing");
        let id = track.id;
        let lyrics = self.read_lyrics(&track, request);
        let (Some(_), Some(omni)) = (track.online.clone(), self.omni.clone()) else {
            self.with_player(|player| player.load_input_at(track.path.clone().into(), self.start_position));
            self.refresh();
            return Task::batch([lyrics, Task::done(Message::Started(request))]);
        };

        if self.start_position > 0.0 {
            self.restored = Some((id, self.start_position));
            self.status =
                Status { state: PlayerState::Paused, position: self.start_position, duration: track.duration };
        }
        self.with_player(Player::stop);
        self.refresh();
        self.loading.begin(request);
        self.loading_at = Some(Instant::now());
        self.loading_toast = !omni.audio_is_prefetched(&track, self.settings.audio_quality);
        let quality = self.settings.audio_quality;
        let failed = self.failed_inputs.clone();
        let audio = Task::perform(
            async move { omni.prepare_track_audio_after(&track, quality, &failed).await },
            move |result| {
                Message::AudioReady(
                    request,
                    id,
                    result.map_err(|err| match err {
                        folia_omni::OmniError::SubscriptionRequired => AudioFailure::SubscriptionRequired,
                        folia_omni::OmniError::CustomSourceUnavailable => AudioFailure::CustomSourceUnavailable,
                        err => AudioFailure::Other(error_chain(&err)),
                    }),
                )
            },
        );
        let (audio, handle) = audio.abortable();
        self.audio_task = Some(handle.abort_on_drop());
        Task::batch([audio, lyrics])
    }

    fn read_lyrics(&self, track: &Track, request: u64) -> Task<Message> {
        let id = track.id;
        let kept = self.kept_chorus.as_ref().filter(|(song, _)| *song == id).map(|(_, ranges)| ranges.clone());
        if let (Some(online), Some(omni)) = (track.online.clone(), self.omni.clone()) {
            return Task::perform(
                async move {
                    let lyrics = omni.lyrics(&online).await.unwrap_or_else(|err| {
                        tracing::warn!(error = &err as &dyn std::error::Error, "online lyrics unavailable");
                        None
                    });
                    settle_chorus(lyrics, kept)
                },
                move |(lyrics, kept)| Message::LyricsLoaded(request, id, lyrics, kept),
            );
        }
        let path = track.path.clone();
        Task::perform(blocking(move || settle_chorus(folia_library::lyrics_for(&path), kept)), move |settled| {
            let (lyrics, kept) = settled.unwrap_or_default();
            Message::LyricsLoaded(request, id, lyrics, kept)
        })
    }

    pub fn notice_animating(&self, now: Instant) -> bool {
        self.notice.as_ref().is_some_and(|(_, started)| {
            let ms = now.saturating_duration_since(*started).as_millis();
            ms < 320 || (6000..6220).contains(&ms)
        })
    }

    pub fn subscription_notice(&self, now: Instant) -> Option<(&str, f32, f32)> {
        let (Notice::Subscription(provider), started) = self.notice.as_ref()? else { return None };
        let (opacity, shift) = notice_frame(*started, now);
        Some((provider, opacity, shift))
    }

    pub fn custom_source_notice(&self, now: Instant) -> Option<(f32, f32)> {
        let (Notice::CustomSourceUnavailable, started) = self.notice.as_ref()? else { return None };
        Some(notice_frame(*started, now))
    }

    fn show_notice(&mut self, notice: Notice) -> Task<Message> {
        self.notice_serial = self.notice_serial.wrapping_add(1);
        let serial = self.notice_serial;
        self.notice = Some((notice, Instant::now()));
        Task::perform(blocking(|| std::thread::sleep(Duration::from_secs(6))), move |_| Message::NoticeLeaving(serial))
    }

    fn refresh(&mut self) {
        if self.restored.is_some_and(|(id, _)| self.current().is_some_and(|track| track.id == id)) {
            return;
        }
        self.restored = None;
        if let Some(player) = &mut self.player {
            player.collect_room();
            self.status = player.status();
        }
    }

    fn with_player(&self, action: impl FnOnce(&Player)) {
        if let Some(player) = &self.player {
            action(player);
        }
    }

    pub(crate) fn shutdown(&mut self) {
        self.audio_task = None;
        self.prefetch_task = None;
        self.request = self.request.wrapping_add(1);
        self.with_player(Player::stop);
    }

    fn end_loading(&mut self) {
        self.loading.finish(self.request);
    }

    pub fn loading_notice(&self, now: Instant) -> Option<(f32, f32)> {
        if !self.loading_toast {
            return None;
        }
        let at = self.loading_at?;
        let entered =
            folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0)
                .at((now.saturating_duration_since(at).as_secs_f64() / 0.32).clamp(0.0, 1.0)) as f32;
        let left = folia_visualizer::Ease::new(0.4, 0.0, 1.0, 1.0).at((now
            .saturating_duration_since(at + Duration::from_secs(3))
            .as_secs_f64()
            / 0.22)
            .clamp(0.0, 1.0)) as f32;
        (left < 1.0).then_some((entered * (1.0 - left), -18.0 * (1.0 - entered) - 10.0 * left))
    }

    pub fn loading_angle(&self, now: Instant) -> f32 {
        self.loading_at
            .map_or(0.0, |at| now.saturating_duration_since(at).as_secs_f32().fract() * std::f32::consts::TAU)
    }

    /// Queue edits, shuffle, FM refill, quality and current-track changes all reach this single hook.
    pub fn refresh_prefetch(&mut self) -> Task<Message> {
        if self.restored.is_some() {
            self.prefetch_task = None;
            return Task::none();
        }
        let revision = self.omni.as_ref().map_or(0, Omni::audio_cache_revision);
        self.prefetch_dirty |= revision != self.source_revision;
        self.source_revision = revision;
        if !self.prefetch_dirty {
            return Task::none();
        }
        if self.loading.active() {
            self.prefetch_task = None;
            return Task::none();
        }
        self.prefetch_dirty = false;
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        revision.hash(&mut hash);
        self.queue.current_index().hash(&mut hash);
        self.settings.audio_quality.id().hash(&mut hash);
        (self.queue.repeat as u8).hash(&mut hash);
        for track in self.queue.items() {
            track.id.hash(&mut hash);
            track.online.hash(&mut hash);
        }
        let key = hash.finish();
        if key == self.prefetch_key {
            return Task::none();
        }
        self.prefetch_key = key;
        self.prefetch_task = None;
        let Some(omni) = self.omni.clone() else { return Task::none() };
        let plan = folia_omni::PrefetchPlan::new(&self.queue, self.settings.audio_quality);
        let (task, handle) =
            Task::perform(async move { omni.prefetch(plan).await }, |_| Message::Prefetched).abortable();
        self.prefetch_task = Some(handle.abort_on_drop());
        task
    }
}

fn notice_frame(started: Instant, now: Instant) -> (f32, f32) {
    let ms = now.saturating_duration_since(started).as_secs_f64() * 1000.0;
    let entered = folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0).at((ms / 320.0).clamp(0.0, 1.0)) as f32;
    let left = folia_visualizer::Ease::new(0.4, 0.0, 1.0, 1.0).at(((ms - 6000.0) / 220.0).clamp(0.0, 1.0)) as f32;
    (entered * (1.0 - left), -18.0 * (1.0 - entered) - 10.0 * left)
}

/// Request-scoped loading state prevents an old completion from clearing a newer song's indicator.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loading {
    request: Option<u64>,
    pub phase: LoadPhase,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadPhase {
    #[default]
    Idle,
    Resolving,
    Decoding,
}
impl Loading {
    pub fn begin(&mut self, request: u64) {
        self.request = Some(request);
        self.phase = LoadPhase::Resolving;
    }
    pub fn decoding(&mut self, request: u64) {
        if self.request == Some(request) {
            self.phase = LoadPhase::Decoding;
        }
    }
    pub fn finish(&mut self, request: u64) -> bool {
        if self.request != Some(request) {
            return false;
        }
        self.request = None;
        self.phase = LoadPhase::Idle;
        true
    }
    pub fn active(&self) -> bool {
        self.request.is_some()
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
