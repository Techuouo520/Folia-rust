// The session log kept in memory for the developer page, and the same lines written to a file when asked.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use tracing::Level;
use tracing::field::{Field, Visit};

use crate::developer::LogMode;
use crate::file_log::FileLog;
use crate::log_file::{LOG_ENTRIES, LOG_LINES};

/// One recorded line: the tracing level, the module target it came from, and its rendered text.
#[derive(Debug, Clone)]
pub struct LogLine {
    pub id: u64,
    pub at: SystemTime,
    pub level: Level,
    pub target: String,
    pub message: String,
}

impl LogLine {
    /// `12:34:56 [Module] text`, the form the panel copies.
    pub fn text(&self) -> String {
        format!("{} [{}] {}", clock(self.at), self.target, self.message)
    }
}

#[derive(Debug)]
struct Inner {
    capture: AtomicBool,
    file_on: AtomicBool,
    lines: Mutex<VecDeque<LogLine>>,
    next: AtomicU64,
    file: FileLog,
}

/// The runtime log: bounded lines in memory, plus an optional file with the same 500-line tail rule.
///
/// Lines arrive on any thread that logs, so the buffer is a `parking_lot::Mutex` (no poisoning to handle on the
/// interface thread) and the flags are atomics; the disk itself is behind [`FileLog`]'s own thread.
#[derive(Debug, Clone)]
pub struct RuntimeLog {
    inner: Arc<Inner>,
}

impl RuntimeLog {
    /// Records tracing fields, including every cause of structured error fields.
    pub fn record_event(&self, event: &tracing::Event<'_>) {
        let mut text = EventText::default();
        event.record(&mut text);
        self.push(*event.metadata().level(), event.metadata().target(), &text.0);
    }

    pub fn new(path: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                capture: AtomicBool::new(true),
                file_on: AtomicBool::new(false),
                lines: Mutex::new(VecDeque::with_capacity(LOG_ENTRIES)),
                next: AtomicU64::new(0),
                file: FileLog::start(path, LOG_LINES),
            }),
        }
    }

    /// Applies the three switches together; turning capture off also drops the lines it was holding.
    pub fn configure(&self, capture: bool, file: bool, mode: LogMode) {
        self.inner.capture.store(capture, Ordering::Relaxed);
        self.inner.file_on.store(capture && file, Ordering::Relaxed);
        self.inner.file.set(capture && file, mode);
        if !capture {
            self.inner.lines.lock().clear();
        }
    }

    pub fn capture(&self) -> bool {
        self.inner.capture.load(Ordering::Relaxed)
    }

    pub fn path(&self) -> &Path {
        self.inner.file.path()
    }

    /// Records one event; called from the tracing layer on whichever thread logged it.
    pub fn push(&self, level: Level, target: &str, message: &str) {
        if !self.inner.capture.load(Ordering::Relaxed) {
            return;
        }
        let at = SystemTime::now();
        if self.inner.file_on.load(Ordering::Relaxed) {
            self.inner.file.write(&file_line(at, level, target, message));
        }
        let id = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let mut lines = self.inner.lines.lock();
        if lines.len() >= LOG_ENTRIES {
            lines.pop_front();
        }
        lines.push_back(LogLine { id, at, level, target: target.to_owned(), message: message.to_owned() });
    }

    pub fn entries(&self) -> Vec<LogLine> {
        self.inner.lines.lock().iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.inner.lines.lock().clear();
    }

    /// Flushes the file when it is on; the panic hook calls this before the process ends.
    pub fn sync(&self) {
        self.inner.file.sync();
    }

    /// Whether lines are also going to the file right now.
    pub fn file_enabled(&self) -> bool {
        self.inner.file_on.load(Ordering::Relaxed)
    }
}

fn file_line(at: SystemTime, level: Level, target: &str, message: &str) -> String {
    format!("{} [{}] [{}] {}", iso(at), level.as_str(), target, message)
}

/// `YYYY-MM-DDTHH:MM:SSZ`, UTC because the standard library has no local calendar.
pub fn iso(at: SystemTime) -> String {
    let (year, month, day, hour, minute, second) = calendar(at);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// `HH:MM:SS`, UTC.
pub fn clock(at: SystemTime) -> String {
    let (_, _, _, hour, minute, second) = calendar(at);
    format!("{hour:02}:{minute:02}:{second:02}")
}

fn calendar(at: SystemTime) -> (i64, u32, u32, u32, u32, u32) {
    let seconds = at.duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs());
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    (year, month, day, (rest / 3600) as u32, (rest % 3600 / 60) as u32, (rest % 60) as u32)
}

/// Howard Hinnant's days-from-civil inverse, kept here so no date crate is needed.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 { month_prime + 3 } else { month_prime - 9 };
    if month <= 2 {
        year += 1;
    }
    (year, month as u32, day as u32)
}

/// Renders one tracing event into the text the session log panel shows and the file keeps.
#[derive(Default)]
struct EventText(String);

impl Visit for EventText {
    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record(field, &error_chain(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record(field, &value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record(field, &format_args!("{value:?}"));
    }
}

impl EventText {
    fn record(&mut self, field: &Field, value: &dyn std::fmt::Display) {
        use std::fmt::Write as _;
        if field.name() == "message" {
            let _ = write!(self.0, "{value}");
        } else {
            let _ = write!(self.0, " {}={value}", field.name());
        }
    }
}

/// `error: cause: cause` on one line, for showing an error chain in the interface.
pub fn error_chain(err: &dyn std::error::Error) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}
