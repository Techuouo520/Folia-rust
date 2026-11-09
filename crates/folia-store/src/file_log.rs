// A log file that can be switched on, cleared and switched off while the app runs; all disk work stays on its thread.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::developer::LogMode;
use crate::log_file::LogFile;

const SYNC_WAIT: Duration = Duration::from_secs(1);

enum Note {
    Set { enabled: bool, mode: LogMode },
    Line(String),
    Sync(Sender<()>),
}

/// A `LogFile` whose path and limit are fixed but whose enabled state and mode are not.
///
/// Callers only send messages; opening, truncating and closing the file happen on one background thread, so the
/// interface thread never touches the disk. `enabled` is atomic so a disabled log can drop a line without a send.
#[derive(Debug, Clone)]
pub struct FileLog {
    path: PathBuf,
    enabled: Arc<AtomicBool>,
    notes: Sender<Note>,
}

impl FileLog {
    /// Starts the writer thread without touching the disk; the file is opened by the first `set(true, ..)`.
    pub fn start(path: PathBuf, limit: usize) -> Self {
        let (notes, inbox) = mpsc::channel();
        let enabled = Arc::new(AtomicBool::new(false));
        let state = enabled.clone();
        let owned = path.clone();
        let spawned = std::thread::Builder::new()
            .name("folia-log-file".to_owned())
            .spawn(move || run(owned, limit, state, inbox));
        if let Err(err) = spawned {
            tracing::warn!(path = %path.display(), error = &err as &dyn std::error::Error, "log writer thread");
        }
        Self { path, enabled, notes }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Opens (or clears, in overwrite mode) or closes the file. Takes effect in order with later lines.
    pub fn set(&self, enabled: bool, mode: LogMode) {
        self.enabled.store(enabled, Ordering::Relaxed);
        let _ = self.notes.send(Note::Set { enabled, mode });
    }

    /// Queues one line; nothing is allocated or sent while the log is off.
    pub fn write(&self, line: &str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let _ = self.notes.send(Note::Line(line.to_owned()));
    }

    /// Waits until everything queued is on disk, at most one second.
    pub fn sync(&self) {
        let (done, wait) = mpsc::channel();
        if self.notes.send(Note::Sync(done)).is_ok() {
            let _ = wait.recv_timeout(SYNC_WAIT);
        }
    }
}

fn run(path: PathBuf, limit: usize, enabled: Arc<AtomicBool>, inbox: Receiver<Note>) {
    let mut file: Option<LogFile> = None;
    while let Ok(note) = inbox.recv() {
        match note {
            Note::Set { enabled: on, mode } => {
                file = None;
                enabled.store(on, Ordering::Relaxed);
                if on {
                    match LogFile::open_mode(&path, limit, mode) {
                        Ok(opened) => file = Some(opened),
                        Err(err) => {
                            tracing::warn!(path = %path.display(), error = &err as &dyn std::error::Error, "log file");
                        }
                    }
                }
            }
            Note::Line(line) => {
                if let Some(file) = file.as_mut() {
                    let _ = writeln!(file, "{line}");
                }
            }
            Note::Sync(done) => {
                if let Some(file) = file.as_ref() {
                    file.sync();
                }
                let _ = done.send(());
            }
        }
    }
}
