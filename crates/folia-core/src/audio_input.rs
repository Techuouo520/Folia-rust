// A progressively downloaded audio file shared by its background writer and decoder reader.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AudioInput {
    pub path: PathBuf,
    pub download: Option<Download>,
}

impl From<PathBuf> for AudioInput {
    fn from(path: PathBuf) -> Self {
        Self { path, download: None }
    }
}

#[derive(Debug)]
struct State {
    file: File,
    available: u64,
    total: Option<u64>,
    done: bool,
    error: Option<String>,
    ranges: Vec<(u64, u64)>,
    requests: Option<std::sync::mpsc::SyncSender<ReadRequest>>,
}

/// The decoder asks its producer for uncached bytes without doing network IO itself.
#[derive(Debug, Clone)]
pub struct ReadRequest {
    pub offset: u64,
    pub cancelled: Arc<AtomicBool>,
}

/// Readers block only on decoder threads, never in the audio device callback.
#[derive(Debug, Clone)]
pub struct Download(Arc<(Mutex<State>, Condvar)>);

impl Download {
    pub fn new(file: File, total: Option<u64>) -> Self {
        Self(Arc::new((
            Mutex::new(State {
                file,
                available: 0,
                total,
                done: false,
                error: None,
                ranges: Vec::new(),
                requests: None,
            }),
            Condvar::new(),
        )))
    }

    pub fn publish(&self, available: u64) {
        let mut state = self.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.available = state.available.max(available);
        absorb_ranges(&mut state);
        self.0.1.notify_all();
    }

    pub fn request_ranges(&self, sender: std::sync::mpsc::SyncSender<ReadRequest>) {
        self.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).requests = Some(sender);
    }

    pub fn disable_ranges(&self) {
        self.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).requests = None;
    }

    /// Publishes only bytes actually written and validated; sparse-file holes never become readable.
    pub fn publish_range(&self, start: u64, end: u64) {
        let mut state = self.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if start >= end || state.total.is_some_and(|total| end > total) {
            return;
        }
        state.ranges.push((start, end));
        state.ranges.sort_unstable();
        let mut merged: Vec<(u64, u64)> = Vec::with_capacity(state.ranges.len());
        for &(start, end) in &state.ranges {
            if let Some(last) = merged.last_mut()
                && start <= last.1
            {
                last.1 = last.1.max(end);
            } else {
                merged.push((start, end));
            }
        }
        state.ranges = merged;
        absorb_ranges(&mut state);
        self.0.1.notify_all();
    }

    pub fn finish(&self, result: Result<(), String>) {
        let mut state = self.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.done = true;
        state.error = result.err();
        state.total = Some(state.available);
        self.0.1.notify_all();
    }

    /// Available bytes, completion and a sanitized download error.
    pub fn progress(&self) -> (u64, bool, Option<String>) {
        let state = self.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        (state.available, state.done, state.error.clone())
    }

    pub fn reader(&self, cancelled: Arc<AtomicBool>) -> DownloadReader {
        self.reader_with_status(cancelled, Default::default())
    }

    pub fn reader_with_status(&self, cancelled: Arc<AtomicBool>, waiting: Arc<AtomicBool>) -> DownloadReader {
        DownloadReader { download: self.clone(), position: 0, cancelled, waiting }
    }
}

fn absorb_ranges(state: &mut State) {
    while state.ranges.first().is_some_and(|range| range.0 <= state.available) {
        state.available = state.available.max(state.ranges.remove(0).1);
    }
}

/// Seekable input whose temporary EOF waits for the producer, including seeks ahead of the download.
pub struct DownloadReader {
    download: Download,
    position: u64,
    cancelled: Arc<AtomicBool>,
    waiting: Arc<AtomicBool>,
}

impl DownloadReader {
    pub fn random_access(&self) -> bool {
        let state = self.download.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.done || state.requests.is_some()
    }
    pub fn content_len(&self) -> Option<u64> {
        self.download.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).total
    }
    pub fn byte_len(&self) -> Option<u64> {
        let state = self.download.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // A known HTTP length would make demuxers probe trailing tags before decoding the first packet.
        if state.done { state.total } else { None }
    }

    fn check(&self, state: &State) -> io::Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "audio load cancelled"));
        }
        if let Some(error) = &state.error {
            return Err(io::Error::other(error.clone()));
        }
        Ok(())
    }
}

impl Read for DownloadReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let _waiting = Waiting(&self.waiting);
        let mut requested = false;
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut state = self.download.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            self.check(&state)?;
            let end = state
                .ranges
                .iter()
                .find(|range| range.0 <= self.position && self.position < range.1)
                .map_or(state.available, |range| range.1);
            let available = end.saturating_sub(self.position);
            if available > 0 {
                let count = (available.min(buffer.len() as u64)) as usize;
                state.file.seek(SeekFrom::Start(self.position))?;
                let read = state.file.read(&mut buffer[..count])?;
                self.position += read as u64;
                return Ok(read);
            }
            if state.done || state.total.is_some_and(|total| self.position >= total) {
                return Ok(0);
            }
            self.waiting.store(true, Ordering::Release);
            if !requested && let Some(sender) = &state.requests {
                requested = !matches!(
                    sender.try_send(ReadRequest { offset: self.position, cancelled: self.cancelled.clone() }),
                    Err(std::sync::mpsc::TrySendError::Full(_))
                );
            }
            state = self
                .download
                .0
                .1
                .wait_timeout(state, Duration::from_millis(20))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
}

impl Seek for DownloadReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let _waiting = Waiting(&self.waiting);
        let mut state = self.download.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        while matches!(from, SeekFrom::End(_)) && state.total.is_none() && !state.done {
            self.check(&state)?;
            self.waiting.store(true, Ordering::Release);
            state = self
                .download
                .0
                .1
                .wait_timeout(state, Duration::from_millis(20))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        self.check(&state)?;
        let position = match from {
            SeekFrom::Start(position) => i128::from(position),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(state.total.unwrap_or(state.available)) + i128::from(offset),
        };
        self.position =
            u64::try_from(position).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid audio seek"))?;
        Ok(self.position)
    }
}

struct Waiting<'a>(&'a AtomicBool);
impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
