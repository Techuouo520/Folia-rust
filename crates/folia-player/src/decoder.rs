// The decoder thread: opens tracks, decodes and converts them, and keeps the ring buffer filled.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use rtrb::Producer;

use crate::PlayerError;
use crate::convert::Converter;
use crate::shared::{PlayerState, Shared};
use crate::source::Source;

const FULL_WAIT: Duration = Duration::from_millis(10);
// A stopped stream never acknowledges; its next start drains the buffer instead.
const FLUSH_TIMEOUT: Duration = Duration::from_millis(300);
// Underruns are summed over this long so a struggling machine logs one line per window, not one per callback.
const UNDERRUN_REPORT: Duration = Duration::from_secs(1);

pub enum Command {
    Load(PathBuf),
    Seek(f64),
    Stop,
}

pub struct Decoder {
    shared: Arc<Shared>,
    producer: Producer<f32>,
    converter: Converter,
    commands: Receiver<Command>,
    source: Option<Source>,
    pending: Vec<f32>,
    pending_from: usize,
    rate: u32,
    last_report: Instant,
}

impl Decoder {
    pub fn new(
        shared: Arc<Shared>,
        producer: Producer<f32>,
        channels: usize,
        rate: u32,
        commands: Receiver<Command>,
    ) -> Self {
        let converter = Converter::new(channels, rate);
        Self {
            shared,
            producer,
            converter,
            commands,
            source: None,
            pending: Vec::new(),
            pending_from: 0,
            rate,
            last_report: Instant::now(),
        }
    }

    /// Runs until the control side drops its sender.
    pub fn run(mut self) {
        loop {
            let idle = self.pending_from >= self.pending.len()
                && (self.source.is_none() || self.shared.decoder_done.load(Ordering::Acquire));
            if idle || self.last_report.elapsed() >= UNDERRUN_REPORT {
                self.report_underruns();
            }
            let command = if idle {
                match self.commands.recv() {
                    Ok(command) => Some(command),
                    Err(_) => return,
                }
            } else {
                match self.commands.try_recv() {
                    Ok(command) => Some(command),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return,
                }
            };
            if let Some(command) = command {
                self.handle(command);
                continue;
            }

            if self.flush_pending() {
                continue;
            }
            match self.commands.recv_timeout(FULL_WAIT) {
                Ok(command) => self.handle(command),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Load(path) => {
                self.reset(0.0);
                match Source::open(&path) {
                    Ok(source) => {
                        self.shared.duration.store(source.duration);
                        self.source = Some(source);
                        self.shared.set_state(PlayerState::Playing);
                        tracing::info!(path = %path.display(), "track loaded");
                    }
                    Err(err) => self.fail(&err),
                }
            }
            Command::Seek(seconds) => {
                let Some(source) = &mut self.source else { return };
                match source.seek(seconds) {
                    Ok(reached) => self.reset(reached),
                    Err(err) => self.fail(&err),
                }
            }
            Command::Stop => {
                self.clear(0.0);
                self.source = None;
                self.shared.set_state(PlayerState::Idle);
            }
        }
    }

    /// Pushes converted samples; decodes more when none are waiting. Returns false when the buffer is full.
    fn flush_pending(&mut self) -> bool {
        if self.pending_from >= self.pending.len() {
            self.pending.clear();
            self.pending_from = 0;
            if !self.decode_next() {
                return true;
            }
        }
        let rest = &self.pending[self.pending_from..];
        let count = rest.len().min(self.producer.slots());
        if count == 0 {
            return false;
        }
        if let Ok(chunk) = self.producer.write_chunk_uninit(count) {
            self.pending_from += chunk.fill_from_iter(rest.iter().copied());
        }
        true
    }

    /// Decodes one packet into `pending`; false once the track has ended or failed.
    /// An ended source stays open so a seek can still rewind it.
    fn decode_next(&mut self) -> bool {
        if self.shared.decoder_done.load(Ordering::Acquire) {
            return false;
        }
        let Some(source) = &mut self.source else { return false };
        let result = match source.next() {
            Ok(Some(block)) => self.converter.push(&block, &mut self.pending),
            Ok(None) => {
                self.shared.decoder_done.store(true, Ordering::Release);
                return false;
            }
            Err(err) => Err(err),
        };
        if let Err(err) = result {
            self.fail(&err);
            return false;
        }
        true
    }

    /// Drops everything buffered and waits until the callback has done the same.
    fn reset(&mut self, position: f64) {
        let epoch = self.clear(position);
        let deadline = Instant::now() + FLUSH_TIMEOUT;
        while !self.shared.flush_done(epoch) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Drops the decoder's buffers and asks the callback to drop its own; returns the flush epoch.
    fn clear(&mut self, position: f64) -> u64 {
        self.pending.clear();
        self.pending_from = 0;
        self.converter.reset();
        self.shared.decoder_done.store(false, Ordering::Release);
        self.shared.request_flush(position)
    }

    fn report_underruns(&mut self) {
        self.last_report = Instant::now();
        let (count, frames) = self.shared.take_underruns();
        if count > 0 {
            let silent_ms = frames * 1000 / u64::from(self.rate.max(1));
            tracing::warn!(count, silent_ms, "audio underrun");
        }
    }

    fn fail(&mut self, err: &PlayerError) {
        tracing::error!(error = err as &dyn std::error::Error, "playback failed");
        self.source = None;
        self.shared.set_state(PlayerState::Failed);
    }
}
