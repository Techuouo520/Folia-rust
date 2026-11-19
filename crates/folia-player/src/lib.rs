// Audio playback: a decoder thread feeds a lock-free ring buffer that the device callback drains.

mod analyser;
mod convert;
mod decoder;
mod equalizer;
mod output;
mod room;
mod room_mix;
mod shared;
mod source;

pub use analyser::{Analyser, TAP_CAPACITY, push_samples};
pub use convert::map_channels;
pub use equalizer::{Biquad, Coefficients, Control, Parameters, Processor};
pub use folia_core::AudioFrame;
mod listen;
pub use listen::ListenTracker;
pub use room::{RoomConvolver, RoomError};
pub use room_mix::{Control as RoomControl, Mixer as RoomMixer};
pub use shared::PlayerState;
/// Packet decoding for playback and offline audio inspection, without opening an output device.
pub use source::Source as DecodedSource;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};

use cpal::traits::StreamTrait;

use crate::decoder::{Command, Decoder};
use crate::shared::Shared;

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("no audio output device")]
    NoDevice,
    #[error("audio output device error")]
    Device(#[source] cpal::Error),
    #[error("unsupported output sample format {0}")]
    UnsupportedFormat(String),
    #[error("cannot open {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("file has no playable audio track")]
    NoAudioTrack,
    #[error("cannot decode audio")]
    Decode(#[from] symphonia::core::errors::Error),
    #[error("cannot resample audio: {0}")]
    Resample(String),
    #[error("cannot start the decoder thread")]
    Thread(#[source] std::io::Error),
}

impl Drop for Player {
    fn drop(&mut self) {
        self.cancel_input();
    }
}

/// What the UI needs to draw the transport: state, position and length in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Status {
    pub state: PlayerState,
    pub position: f64,
    pub duration: f64,
}

pub struct Player {
    commands: Sender<Command>,
    shared: Arc<Shared>,
    stream: cpal::Stream,
    rate: u32,
    analyser: Analyser,
    equalizer: equalizer::Control,
    room: room_mix::Control,
    input: std::sync::Mutex<InputControl>,
}

#[derive(Default)]
struct InputControl {
    audio: Option<folia_core::AudioInput>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    waiting: Arc<std::sync::atomic::AtomicBool>,
    preparing: Arc<std::sync::atomic::AtomicBool>,
}

impl Player {
    pub fn new() -> Result<Self, PlayerError> {
        let shared = Arc::new(Shared::new());
        let (tap, analyser) = Analyser::pair();
        let (equalizer, equalizer_consumer) = equalizer::Control::pair();
        let (output, producer, room) = output::open(shared.clone(), tap, equalizer_consumer)?;
        let (commands, receiver) = mpsc::channel();
        let decoder = Decoder::new(shared.clone(), producer, output.channels, output.rate, receiver);
        std::thread::Builder::new()
            .name("folia-decoder".into())
            .spawn(move || decoder.run())
            .map_err(PlayerError::Thread)?;
        tracing::info!(rate = output.rate, channels = output.channels, "audio output opened");
        Ok(Self {
            commands,
            shared,
            stream: output.stream,
            rate: output.rate,
            analyser,
            equalizer,
            room,
            input: Default::default(),
        })
    }

    /// Hands the equalizer and effect settings to the audio callback. No lock is taken there.
    pub fn set_equalizer(&mut self, parameters: equalizer::Parameters) {
        self.equalizer.set(parameters);
    }

    /// The latest spectrum. `playing` selects analysis; otherwise the frame is the idle breath.
    pub fn audio_frame(&mut self, playing: bool, wall_ms: f64) -> AudioFrame {
        self.room.collect();
        *self.analyser.frame(playing, wall_ms)
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    pub fn set_room(&mut self, convolver: Option<Box<RoomConvolver>>) -> Result<(), RoomError> {
        if convolver.as_ref().is_some_and(|convolver| convolver.sample_rate() != self.rate) {
            return Err(RoomError::SampleRate);
        }
        let release = self.room.enabled() && convolver.is_none();
        self.room.set(convolver)?;
        if release && matches!(self.shared.state(), PlayerState::Idle | PlayerState::Ended | PlayerState::Failed) {
            self.run_stream(true);
        }
        Ok(())
    }

    pub fn collect_room(&mut self) {
        self.room.collect();
    }

    /// Collects a disabled room once its callback has finished fading, returning whether release is complete.
    pub fn finish_room_change(&mut self) -> bool {
        self.room.collect();
        if self.room.enabled() {
            return true;
        }
        if !self.room.released() {
            return false;
        }
        if matches!(self.shared.state(), PlayerState::Idle | PlayerState::Ended | PlayerState::Failed) {
            self.run_stream(false);
        }
        true
    }

    /// Starts playing `path` from the beginning.
    pub fn load(&self, path: &Path) {
        self.load_input(path.to_owned().into());
    }

    pub fn load_input(&self, input: folia_core::AudioInput) {
        self.load_input_at(input, 0.0);
    }

    /// Starts decoding at the saved media time without emitting the beginning of the track first.
    pub fn load_input_at(&self, input: folia_core::AudioInput, position: f64) {
        let position = if position.is_finite() { position.max(0.0) } else { 0.0 };
        let load = self.prepare_input(Some(input), position);
        self.shared.duration.store(0.0);
        self.shared.request_flush(position);
        // Mark the new track as playing now, so a status read before the decoder starts never sees the old end.
        self.shared.decoder_done.store(false, std::sync::atomic::Ordering::Release);
        self.shared.set_state(PlayerState::Playing);
        self.run_stream(true);
        if let Some(load) = load {
            self.send(Command::Load(load));
        }
    }

    pub fn pause(&self) {
        if self.shared.state() == PlayerState::Playing {
            self.shared.set_state(PlayerState::Paused);
        }
    }

    pub fn resume(&self) {
        if self.shared.state() == PlayerState::Paused {
            self.shared.set_state(PlayerState::Playing);
        }
    }

    pub fn seek(&self, seconds: f64) {
        let Some(load) = self.prepare_input(None, seconds.max(0.0)) else { return };
        self.shared.request_flush(seconds.max(0.0));
        self.shared.decoder_done.store(false, std::sync::atomic::Ordering::Release);
        self.run_stream(true);
        self.send(Command::Load(load));
    }

    /// Initial decoding, an outstanding seek, or uncached reads while the output has run dry.
    pub fn is_buffering(&self) -> bool {
        let input = self.input.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        input.preparing.load(std::sync::atomic::Ordering::Acquire)
            || (input.waiting.load(std::sync::atomic::Ordering::Acquire)
                && self.shared.starved.load(std::sync::atomic::Ordering::Acquire)
                && self.shared.state() == PlayerState::Playing)
    }

    pub fn stop(&self) {
        self.cancel_input();
        self.shared.set_state(PlayerState::Idle);
        self.send(Command::Stop);
        self.run_stream(!self.room.enabled() && !self.room.released());
    }

    fn prepare_input(&self, audio: Option<folia_core::AudioInput>, position: f64) -> Option<decoder::Load> {
        let mut input = self.input.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(audio) = audio {
            input.audio = Some(audio);
        }
        let audio = input.audio.clone()?;
        input.cancelled.store(true, std::sync::atomic::Ordering::Release);
        input.cancelled = Default::default();
        input.waiting = Default::default();
        input.preparing = Arc::new(std::sync::atomic::AtomicBool::new(true));
        Some(decoder::Load {
            audio,
            cancelled: input.cancelled.clone(),
            waiting: input.waiting.clone(),
            preparing: input.preparing.clone(),
            position,
        })
    }

    fn cancel_input(&self) {
        let mut input = self.input.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        input.cancelled.store(true, std::sync::atomic::Ordering::Release);
        input.waiting.store(false, std::sync::atomic::Ordering::Release);
        input.preparing.store(false, std::sync::atomic::Ordering::Release);
        input.audio = None;
    }

    pub fn volume(&self) -> f32 {
        self.shared.volume()
    }

    /// Sets the output gain, clamped to `0.0..=1.0`.
    pub fn set_volume(&self, volume: f32) {
        self.shared.set_volume(volume);
    }

    pub fn status(&self) -> Status {
        let state = self.shared.state();
        let duration = self.shared.duration.load();
        let position = match state {
            PlayerState::Idle => 0.0,
            _ => self.shared.position(self.rate).min(if duration > 0.0 { duration } else { f64::MAX }),
        };
        Status { state, position, duration }
    }

    /// The current track position at the output device, corrected for already queued audio.
    pub fn heard_position(&self) -> f64 {
        if self.shared.state() == PlayerState::Idle {
            return 0.0;
        }
        let now = u64::try_from(self.stream.now().as_nanos()).unwrap_or(u64::MAX);
        let duration = self.shared.duration.load();
        self.shared.heard_position(now, self.rate).min(if duration > 0.0 { duration } else { f64::MAX })
    }

    fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            tracing::error!("decoder thread has stopped");
        }
    }

    // The stream runs while a track is loaded, paused included, so flushes are always acknowledged.
    fn run_stream(&self, running: bool) {
        let result = if running { self.stream.play() } else { self.stream.pause() };
        if let Err(err) = result {
            tracing::warn!(error = &err as &dyn std::error::Error, running, "cannot change output stream state");
        }
    }
}
