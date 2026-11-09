// The runtime log on disk: the last lines kept in one file under the data directory, written by a background thread.

use std::collections::VecDeque;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::developer::LogMode;

/// Lines kept in the file; older ones drop off the top.
pub const LOG_LINES: usize = 500;
/// Lines the session log panel keeps in memory, the original `consoleLogBuffer` `LIMIT`.
pub const LOG_ENTRIES: usize = 1000;
const SYNC_WAIT: Duration = Duration::from_secs(1);

enum Note {
    Text(Vec<u8>),
    Sync(Sender<()>),
}

/// A `Write` handle for the log subscriber. Each clone feeds the same writer thread.
#[derive(Clone, Debug)]
pub struct LogFile {
    notes: Sender<Note>,
}

impl LogFile {
    /// Opens `path` in `mode`: append keeps the tail of the previous run, overwrite clears the file now.
    pub fn open_mode(path: &Path, limit: usize, mode: LogMode) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let lines = match mode {
            LogMode::Overwrite => {
                fs::write(path, [])?;
                VecDeque::new()
            }
            LogMode::Append => match fs::read_to_string(path) {
                Ok(text) => last_lines(&text, limit),
                Err(err) if err.kind() == io::ErrorKind::NotFound => VecDeque::new(),
                Err(err) => return Err(err),
            },
        };
        let (notes, inbox) = mpsc::channel();
        let writer = Writer { path: path.to_owned(), limit, lines, pending: Vec::new() };
        std::thread::Builder::new().name("folia-log".to_owned()).spawn(move || writer.run(inbox))?;
        Ok(Self { notes })
    }

    /// Waits until everything sent so far is on disk, at most one second (a panic hook calls this before exiting).
    pub fn sync(&self) {
        let (done, wait) = mpsc::channel();
        if self.notes.send(Note::Sync(done)).is_ok() {
            let _ = wait.recv_timeout(SYNC_WAIT);
        }
    }
}

impl Write for LogFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.notes
            .send(Note::Text(buf.to_vec()))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "log writer"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Writer {
    path: PathBuf,
    limit: usize,
    lines: VecDeque<String>,
    pending: Vec<u8>,
}

impl Writer {
    // Takes every note waiting, then rewrites the file once; ends when every handle is gone.
    fn run(mut self, inbox: Receiver<Note>) {
        while let Ok(note) = inbox.recv() {
            let mut syncs = Vec::new();
            self.take(note, &mut syncs);
            while let Ok(note) = inbox.try_recv() {
                self.take(note, &mut syncs);
            }
            // Nowhere else to report a failed log write; the next batch tries again.
            let _ = self.save();
            for done in syncs {
                let _ = done.send(());
            }
        }
    }

    fn take(&mut self, note: Note, syncs: &mut Vec<Sender<()>>) {
        match note {
            Note::Text(bytes) => self.push(&bytes),
            Note::Sync(done) => syncs.push(done),
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            self.lines.push_back(String::from_utf8_lossy(&line).trim_end().to_owned());
            if self.lines.len() > self.limit {
                self.lines.pop_front();
            }
        }
    }

    fn save(&self) -> io::Result<()> {
        let temp = self.path.with_extension("log.tmp");
        let mut file = io::BufWriter::new(fs::File::create(&temp)?);
        for line in &self.lines {
            writeln!(file, "{line}")?;
        }
        file.flush()?;
        drop(file);
        fs::rename(&temp, &self.path)
    }
}

fn last_lines(text: &str, limit: usize) -> VecDeque<String> {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(limit)..].iter().map(|line| (*line).to_owned()).collect()
}
